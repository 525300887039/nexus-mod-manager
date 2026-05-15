//! 存档（saves）扫描、导出、导入与备份。
//!
//! 本模块无 Tauri 依赖。GUI 端的 dialog 选路径副作用由 src-tauri 薄壳负责，
//! core 仅暴露纯路径输入的业务函数。

use crate::game_profile::{preset_games, GameProfile};
use crate::paths;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct CharacterStat {
    pub id: String,
    pub name: String,
    pub wins: u64,
    pub losses: u64,
    #[serde(rename = "maxAscension")]
    pub max_ascension: u64,
    pub playtime: u64,
    #[serde(rename = "bestStreak")]
    pub best_streak: u64,
}

#[derive(Serialize)]
pub struct ProgressSummary {
    #[serde(rename = "totalPlaytime")]
    pub total_playtime: u64,
    #[serde(rename = "floorsClimbed")]
    pub floors_climbed: u64,
    #[serde(rename = "currentScore")]
    pub current_score: u64,
    #[serde(rename = "totalUnlocks")]
    pub total_unlocks: u64,
    #[serde(rename = "discoveredCards")]
    pub discovered_cards: usize,
    #[serde(rename = "discoveredRelics")]
    pub discovered_relics: usize,
    pub epochs: usize,
    pub characters: Vec<CharacterStat>,
    #[serde(rename = "uniqueId")]
    pub unique_id: String,
}

#[derive(Serialize)]
pub struct SaveSlot {
    pub slot: String,
    pub modded: bool,
    pub path: String,
    #[serde(rename = "hasProgress")]
    pub has_progress: bool,
    #[serde(rename = "hasPrefs")]
    pub has_prefs: bool,
    pub empty: bool,
    #[serde(rename = "lastModified")]
    pub last_modified: Option<String>,
    pub size: u64,
    pub summary: Option<ProgressSummary>,
}

#[derive(Serialize)]
pub struct BackupEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub time: String,
}

#[derive(Serialize)]
pub struct SavesResult {
    pub slots: Vec<SaveSlot>,
    pub backups: Vec<BackupEntry>,
}

#[derive(Serialize)]
pub struct SimpleResult {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct SaveExportOpts {
    pub slot: String,
    pub modded: bool,
}

fn get_appdata() -> Option<PathBuf> {
    dirs::config_dir()
}

fn default_game_domain() -> Option<String> {
    preset_games()
        .into_iter()
        .next()
        .map(|profile| profile.nexus_domain)
}

pub fn get_steam_user_dir(profile: &GameProfile) -> Option<PathBuf> {
    let appdata = get_appdata()?;
    let appdata_dir_name = profile.appdata_dir_name.as_deref()?;
    let steam_dir = appdata.join(appdata_dir_name).join("steam");
    if !steam_dir.exists() {
        return None;
    }
    let users: Vec<_> = fs::read_dir(&steam_dir)
        .ok()?
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|s| s.chars().all(|c| c.is_ascii_digit()))
                .unwrap_or(false)
        })
        .collect();
    if users.is_empty() {
        return None;
    }
    Some(users[0].path())
}

fn get_save_backup_dir(profile: &GameProfile) -> PathBuf {
    let dir = paths::writable_config_dir()
        .join("save_backups")
        .join(&profile.nexus_domain);
    let _ = fs::create_dir_all(&dir);
    dir
}

fn legacy_save_backup_dirs(profile: &GameProfile) -> Vec<PathBuf> {
    if default_game_domain().as_deref() != Some(profile.nexus_domain.as_str()) {
        return Vec::new();
    }

    let mut dirs = Vec::new();
    for root in [paths::current_config_dir(), paths::legacy_config_dir()]
        .into_iter()
        .flatten()
        .map(|dir| dir.join("save_backups"))
    {
        if root.exists() && !dirs.iter().any(|path: &PathBuf| path == &root) {
            dirs.push(root);
        }
    }

    dirs
}

fn unique_backup_path(dir: &Path, file_name: &str) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let file_path = Path::new(file_name);
    let stem = file_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("backup");
    let ext = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    for index in 1.. {
        let name = if ext.is_empty() {
            format!("{stem}_{index}")
        } else {
            format!("{stem}_{index}.{ext}")
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("backup path generator exhausted");
}

fn migrate_legacy_backups(profile: &GameProfile) -> PathBuf {
    let backup_dir = get_save_backup_dir(profile);

    for legacy_dir in legacy_save_backup_dirs(profile) {
        for entry in fs::read_dir(&legacy_dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("zip") {
                continue;
            }

            let file_name = entry.file_name().to_string_lossy().to_string();
            let target = unique_backup_path(&backup_dir, &file_name);
            let _ = fs::rename(&path, &target).or_else(|_| {
                fs::copy(&path, &target)?;
                fs::remove_file(&path)
            });
        }
    }

    backup_dir
}

fn collect_backups(backup_dir: &Path) -> Vec<BackupEntry> {
    let mut backups = Vec::new();
    if let Ok(entries) = fs::read_dir(backup_dir) {
        let mut bk_files: Vec<_> = entries
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|s| s.ends_with(".zip"))
                    .unwrap_or(false)
            })
            .collect();
        bk_files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
        for entry in bk_files {
            if let Ok(meta) = entry.metadata() {
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| chrono_from_timestamp(d.as_secs() as i64))
                    .unwrap_or_default();
                backups.push(BackupEntry {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: entry.path().to_string_lossy().to_string(),
                    size: meta.len(),
                    time: mtime,
                });
            }
        }
    }

    backups
}

const CHARACTER_NAMES: &[(&str, &str)] = &[
    ("CHARACTER.IRONCLAD", "铁甲战士"),
    ("CHARACTER.SILENT", "沉默猎手"),
    ("CHARACTER.REGENT", "摄政王"),
    ("CHARACTER.NECROBINDER", "缚灵师"),
    ("CHARACTER.DEFECT", "缺陷体"),
    ("CHARACTER.WATCHER", "观察者"),
];

fn char_name(id: &str) -> String {
    for (k, v) in CHARACTER_NAMES {
        if *k == id {
            return v.to_string();
        }
    }
    id.split('.').last().unwrap_or(id).to_string()
}

fn parse_progress(path: &Path) -> Option<ProgressSummary> {
    if !path.exists() {
        return None;
    }
    let mut content = fs::read_to_string(path).ok()?;
    if content.starts_with('\u{feff}') {
        content = content[3..].to_string();
    }
    let data: serde_json::Value = serde_json::from_str(&content).ok()?;

    let characters: Vec<CharacterStat> = data
        .get("character_stats")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| {
                    let id = c.get("id")?.as_str()?.to_string();
                    let wins = c.get("total_wins").and_then(|v| v.as_u64()).unwrap_or(0);
                    let losses = c.get("total_losses").and_then(|v| v.as_u64()).unwrap_or(0);
                    if wins == 0 && losses == 0 {
                        return None;
                    }
                    Some(CharacterStat {
                        name: char_name(&id),
                        id,
                        wins,
                        losses,
                        max_ascension: c.get("max_ascension").and_then(|v| v.as_u64()).unwrap_or(0),
                        playtime: c.get("playtime").and_then(|v| v.as_u64()).unwrap_or(0),
                        best_streak: c
                            .get("best_win_streak")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Some(ProgressSummary {
        total_playtime: data
            .get("total_playtime")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        floors_climbed: data
            .get("floors_climbed")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        current_score: data
            .get("current_score")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        total_unlocks: data
            .get("total_unlocks")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        discovered_cards: data
            .get("discovered_cards")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0),
        discovered_relics: data
            .get("discovered_relics")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0),
        epochs: data
            .get("epochs")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0),
        characters,
        unique_id: data
            .get("unique_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

fn walk_size_and_mtime(dir: &Path) -> (u64, u64) {
    let mut total_size = 0u64;
    let mut last_modified = 0u64;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let (s, m) = walk_size_and_mtime(&p);
                total_size += s;
                if m > last_modified {
                    last_modified = m;
                }
            } else if let Ok(meta) = p.metadata() {
                total_size += meta.len();
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                if mtime > last_modified {
                    last_modified = mtime;
                }
            }
        }
    }
    (total_size, last_modified)
}

fn scan_save_slot(user_dir: &Path, slot: &str, modded: bool) -> Option<SaveSlot> {
    let prefix = if modded {
        user_dir.join("modded").join(slot)
    } else {
        user_dir.join(slot)
    };
    if !prefix.exists() {
        return None;
    }

    let saves_dir = prefix.join("saves");
    let progress_path = saves_dir.join("progress.save");
    let has_progress = progress_path.exists();
    let has_prefs = saves_dir.join("prefs.save").exists();

    let (mut total_size, last_modified) = if saves_dir.exists() {
        walk_size_and_mtime(&saves_dir)
    } else {
        (0, 0)
    };

    let replays_dir = prefix.join("replays");
    if replays_dir.exists() {
        let (rs, _) = walk_size_and_mtime(&replays_dir);
        total_size += rs;
    }

    let summary = parse_progress(&progress_path);

    let last_mod_str = if last_modified > 0 {
        let secs = (last_modified / 1000) as i64;
        let naive = chrono_from_timestamp(secs);
        Some(naive)
    } else {
        None
    };

    Some(SaveSlot {
        slot: slot.to_string(),
        modded,
        path: prefix.to_string_lossy().to_string(),
        has_progress,
        has_prefs,
        empty: !has_progress && !has_prefs,
        last_modified: last_mod_str,
        size: total_size,
        summary,
    })
}

fn chrono_from_timestamp(secs: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let d = UNIX_EPOCH + Duration::from_secs(secs as u64);
    let datetime: std::time::SystemTime = d;
    format!("{:?}", datetime)
}

pub fn timestamp_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let s = secs % 60;
    format!(
        "{}-{:02}-{:02}T{:02}-{:02}-{:02}",
        1970 + secs / 31557600,
        ((secs % 31557600) / 2629800) + 1,
        ((secs % 2629800) / 86400) + 1,
        hours,
        mins,
        s
    )
}

fn add_dir_to_zip(
    zip_writer: &mut zip::ZipWriter<fs::File>,
    base: &Path,
    current: &Path,
    options: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    if let Ok(entries) = fs::read_dir(current) {
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path
                .strip_prefix(base)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                let _ = zip_writer.add_directory(&format!("{}/", rel), options);
                add_dir_to_zip(zip_writer, base, &path, options)?;
            } else {
                zip_writer
                    .start_file(&rel, options)
                    .map_err(|e| e.to_string())?;
                let mut f = fs::File::open(&path).map_err(|e| e.to_string())?;
                let mut buf = Vec::new();
                f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                std::io::Write::write_all(zip_writer, &buf).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// 扫描指定游戏的所有 save slot（含 vanilla / modded）以及备份列表。
/// 不读取 `saves_enabled` 标志，调用方负责判断。
pub fn scan_saves(profile: &GameProfile) -> SavesResult {
    let user_dir = match get_steam_user_dir(profile) {
        Some(d) => d,
        None => {
            return SavesResult {
                slots: vec![],
                backups: vec![],
            }
        }
    };

    let mut slots = Vec::new();
    for s in &["profile1", "profile2", "profile3"] {
        if let Some(slot) = scan_save_slot(&user_dir, s, false) {
            slots.push(slot);
        }
        if let Some(slot) = scan_save_slot(&user_dir, s, true) {
            slots.push(slot);
        }
    }

    let backup_dir = migrate_legacy_backups(profile);
    let backups = collect_backups(&backup_dir);

    SavesResult { slots, backups }
}

/// 把指定 slot 的存档打包到 `dest_path` 的 ZIP。返回 `SimpleResult`。
pub fn export_save_to_zip(
    profile: &GameProfile,
    opts: &SaveExportOpts,
    dest_path: &str,
) -> Result<SimpleResult, String> {
    let user_dir = match get_steam_user_dir(profile) {
        Some(d) => d,
        None => {
            return Ok(SimpleResult {
                success: false,
                error: Some("未找到游戏存档目录".into()),
            })
        }
    };

    let prefix = if opts.modded {
        user_dir.join("modded").join(&opts.slot)
    } else {
        user_dir.join(&opts.slot)
    };
    if !prefix.exists() {
        return Ok(SimpleResult {
            success: false,
            error: Some("该存档槽位为空".into()),
        });
    }

    let file = fs::File::create(dest_path).map_err(|e| e.to_string())?;
    let mut zip_writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let meta_json = serde_json::json!({
        "slot": opts.slot,
        "modded": opts.modded,
        "exportTime": timestamp_string(),
    });
    zip_writer
        .start_file("_meta.json", options)
        .map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zip_writer, meta_json.to_string().as_bytes())
        .map_err(|e| e.to_string())?;

    add_dir_to_zip(
        &mut zip_writer,
        prefix.parent().unwrap_or(&prefix),
        &prefix,
        options,
    )?;
    zip_writer.finish().map_err(|e| e.to_string())?;

    Ok(SimpleResult {
        success: true,
        error: None,
    })
}

/// 把 ZIP 内的存档导入到指定 slot，若目标已存在则先备份。
pub fn import_save_from_zip(
    profile: &GameProfile,
    opts: &SaveExportOpts,
    zip_path: &str,
) -> Result<SimpleResult, String> {
    let user_dir = match get_steam_user_dir(profile) {
        Some(d) => d,
        None => {
            return Ok(SimpleResult {
                success: false,
                error: Some("未找到游戏存档目录".into()),
            })
        }
    };

    let target_dir = if opts.modded {
        user_dir.join("modded").join(&opts.slot)
    } else {
        user_dir.join(&opts.slot)
    };

    if target_dir.exists() {
        let backup_dir = migrate_legacy_backups(profile);
        let tag = if opts.modded {
            format!("{}_modded", opts.slot)
        } else {
            opts.slot.clone()
        };
        let ts = timestamp_string();
        let backup_path = backup_dir.join(format!("auto_backup_{}_{}.zip", tag, ts));

        let bk_file = fs::File::create(&backup_path).map_err(|e| e.to_string())?;
        let mut bk_zip = zip::ZipWriter::new(bk_file);
        let bk_options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        add_dir_to_zip(
            &mut bk_zip,
            target_dir.parent().unwrap_or(&target_dir),
            &target_dir,
            bk_options,
        )?;
        bk_zip.finish().map_err(|e| e.to_string())?;
    }

    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    let mut folders: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            let name = entry.name().to_string();
            if let Some(first) = name.split('/').next() {
                if !folders.contains(&first.to_string()) {
                    folders.push(first.to_string());
                }
            }
        }
    }
    let source_slot = folders
        .iter()
        .find(|f| f.starts_with("profile"))
        .cloned()
        .unwrap_or_else(|| folders.first().cloned().unwrap_or_default());

    let file = fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name == "_meta.json" {
            continue;
        }
        let rel_path = if name.starts_with(&format!("{}/", source_slot)) {
            name[source_slot.len() + 1..].to_string()
        } else {
            name
        };
        let dest = target_dir.join(&rel_path);
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        fs::write(&dest, &buf).map_err(|e| e.to_string())?;
    }

    Ok(SimpleResult {
        success: true,
        error: None,
    })
}

/// 删除备份文件。
pub fn delete_backup(backup_path: &str) -> SimpleResult {
    let p = Path::new(backup_path);
    if p.exists() {
        if let Err(e) = fs::remove_file(p) {
            return SimpleResult {
                success: false,
                error: Some(e.to_string()),
            };
        }
    }
    SimpleResult {
        success: true,
        error: None,
    }
}
