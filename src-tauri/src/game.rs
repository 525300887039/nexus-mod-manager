use crate::{game_profile::GameProfile, AppState};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
pub struct LaunchResult {
    pub success: bool,
    pub error: Option<String>,
    pub method: Option<String>,
}

#[derive(Serialize)]
pub struct GameVersion {
    pub version: Option<String>,
    pub engine: Option<String>,
}

#[derive(Serialize)]
pub struct CrashIssue {
    pub reason: String,
    pub detail: String,
    pub mods: Vec<String>,
}

#[derive(Serialize)]
pub struct InvolvedMod {
    pub name: String,
    #[serde(rename = "errorCount")]
    pub error_count: usize,
    pub sample: String,
}

#[derive(Serialize)]
pub struct CrashReport {
    pub issues: Vec<CrashIssue>,
    #[serde(rename = "logFile")]
    pub log_file: Option<String>,
    #[serde(rename = "errorCount")]
    pub error_count: usize,
    #[serde(rename = "warnCount")]
    pub warn_count: usize,
    #[serde(rename = "involvedMods")]
    pub involved_mods: Vec<InvolvedMod>,
    #[serde(rename = "loadedMods")]
    pub loaded_mods: Vec<String>,
    pub notices: Vec<String>,
}

fn get_appdata() -> Option<PathBuf> {
    dirs::config_dir()
}

fn current_profile(state: &tauri::State<'_, AppState>) -> Option<GameProfile> {
    state
        .current_profile
        .lock()
        .ok()
        .and_then(|profile| profile.clone())
}

fn current_game_path(state: &tauri::State<'_, AppState>) -> Option<String> {
    state
        .game_path
        .lock()
        .ok()
        .and_then(|game_path| game_path.clone())
}

fn has_process_detection(profile: Option<&GameProfile>) -> bool {
    profile
        .and_then(|profile| profile.process_name.as_deref())
        .is_some()
}

fn profile_logs_dir(profile: &GameProfile) -> Option<PathBuf> {
    if !profile.logs_enabled {
        return None;
    }

    let appdata = get_appdata()?;
    let appdata_dir_name = profile.appdata_dir_name.as_deref()?;
    let logs_subdir = profile.logs_subdir.as_deref()?;
    Some(appdata.join(appdata_dir_name).join(logs_subdir))
}

#[tauri::command]
pub fn game_launch(state: tauri::State<'_, AppState>) -> LaunchResult {
    {
        let gs = state.game_state.lock().unwrap();
        if *gs != "idle" {
            return LaunchResult {
                success: false,
                error: Some("game is already running".to_string()),
                method: None,
            };
        }
    }

    let Some(profile) = current_profile(&state) else {
        return LaunchResult {
            success: false,
            error: Some("no game selected".to_string()),
            method: None,
        };
    };
    let game_path = current_game_path(&state);

    let method = if profile.steam_app_id.is_some()
        && game_path
            .as_deref()
            .map(|path| path.to_lowercase().contains("steamapps"))
            .unwrap_or(true)
    {
        let steam_url = format!(
            "steam://rungameid/{}",
            profile.steam_app_id.expect("checked is_some above")
        );
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("cmd")
                .args(["/C", "start", &steam_url])
                .spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = opener::open_browser(&steam_url);
        }
        "steam"
    } else if let (Some(path), Some(exe_name)) = (game_path.as_deref(), profile.exe_name.as_deref())
    {
        let exe_path = Path::new(path).join(exe_name);
        if !exe_path.exists() {
            return LaunchResult {
                success: false,
                error: Some(format!("game executable not found: {}", exe_path.display())),
                method: None,
            };
        }
        let _ = Command::new(&exe_path).current_dir(path).spawn();
        "direct"
    } else if let Some(steam_app_id) = profile.steam_app_id {
        let steam_url = format!("steam://rungameid/{}", steam_app_id);
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("cmd")
                .args(["/C", "start", &steam_url])
                .spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = opener::open_browser(&steam_url);
        }
        "steam"
    } else {
        return LaunchResult {
            success: false,
            error: Some("this game does not have a configured launch method".to_string()),
            method: None,
        };
    };

    let mut gs = state.game_state.lock().unwrap();
    *gs = if has_process_detection(Some(&profile)) {
        "launching".to_string()
    } else {
        "idle".to_string()
    };

    LaunchResult {
        success: true,
        error: None,
        method: Some(method.to_string()),
    }
}

#[tauri::command]
pub fn game_get_state(state: tauri::State<'_, AppState>) -> String {
    let gs = state.game_state.lock().unwrap();
    let current = gs.clone();
    drop(gs);

    if !has_process_detection(current_profile(&state).as_ref()) {
        if current == "launching" || current == "running" {
            let mut gs = state.game_state.lock().unwrap();
            *gs = "idle".to_string();
        }
        return "idle".to_string();
    }

    let running = is_game_running(&state);

    match current.as_str() {
        "launching" => {
            if running {
                let mut gs = state.game_state.lock().unwrap();
                *gs = "running".to_string();
                return "running".to_string();
            }
            "launching".to_string()
        }
        "running" => {
            if !running {
                let mut gs = state.game_state.lock().unwrap();
                *gs = "idle".to_string();
                return "idle".to_string();
            }
            "running".to_string()
        }
        _ => {
            if running {
                let mut gs = state.game_state.lock().unwrap();
                *gs = "running".to_string();
                return "running".to_string();
            }
            "idle".to_string()
        }
    }
}

fn is_game_running(state: &tauri::State<'_, AppState>) -> bool {
    let process_name = current_profile(state).and_then(|profile| profile.process_name);
    let Some(process_name) = process_name else {
        return false;
    };

    #[cfg(target_os = "windows")]
    {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        sys.processes()
            .values()
            .any(|process| process.name().to_string_lossy().contains(&process_name))
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[tauri::command]
pub fn game_get_version(state: tauri::State<'_, AppState>) -> GameVersion {
    let logs_dir = match current_profile(&state).and_then(|profile| profile_logs_dir(&profile)) {
        Some(dir) => dir,
        None => {
            return GameVersion {
                version: None,
                engine: None,
            }
        }
    };
    if !logs_dir.exists() {
        return GameVersion {
            version: None,
            engine: None,
        };
    }

    let mut candidates: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&logs_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("godot2") && name.ends_with(".log") {
                if let Ok(meta) = entry.metadata() {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|duration| duration.as_millis() as u64)
                        .unwrap_or(0);
                    candidates.push((name, mtime));
                }
            }
        }
    }
    candidates.sort_by(|left, right| right.1.cmp(&left.1));

    let mut file_names: Vec<String> = candidates.into_iter().map(|(name, _)| name).collect();
    file_names.push("godot.log".to_string());

    for file_name in &file_names {
        let file_path = logs_dir.join(file_name);
        if !file_path.exists() {
            continue;
        }

        if let Ok(meta) = file_path.metadata() {
            let size = meta.len() as usize;
            let read_size = size.min(16_384);
            if let Ok(content) = fs::read(&file_path) {
                let start = if content.len() > read_size {
                    content.len() - read_size
                } else {
                    0
                };
                let tail = String::from_utf8_lossy(&content[start..]);
                let mut version = None;
                let mut engine = None;

                for line in tail.lines() {
                    if let Some(index) = line.find("Release Version:") {
                        version = Some(line[index + 16..].trim().to_string());
                    }
                    if let Some(index) = line.find("Engine Version:") {
                        engine = Some(line[index + 15..].trim().to_string());
                    }
                }

                if version.is_some() {
                    return GameVersion { version, engine };
                }
            }
        }
    }

    GameVersion {
        version: None,
        engine: None,
    }
}

struct CrashPattern {
    pattern: &'static str,
    reason: &'static str,
    detail: &'static str,
}

const CRASH_PATTERNS: &[CrashPattern] = &[
    CrashPattern {
        pattern: "State divergence",
        reason: "State divergence",
        detail: "Client and host state diverged. Verify both players have the same mods enabled.",
    },
    CrashPattern {
        pattern: "StateDivergence",
        reason: "State divergence",
        detail: "Client and host state diverged. Verify both players have the same mods enabled.",
    },
    CrashPattern {
        pattern: "OutOfMemoryException",
        reason: "Out of memory",
        detail:
            "The game ran out of memory. Close other applications or reduce the active mod set.",
    },
    CrashPattern {
        pattern: "out of memory",
        reason: "Out of memory",
        detail:
            "The game ran out of memory. Close other applications or reduce the active mod set.",
    },
    CrashPattern {
        pattern: "StackOverflowException",
        reason: "Stack overflow",
        detail:
            "A mod may be triggering unbounded recursion. Disable mods one by one to isolate it.",
    },
    CrashPattern {
        pattern: "NullReferenceException",
        reason: "Null reference",
        detail: "A mod or the game hit a null reference during execution.",
    },
    CrashPattern {
        pattern: "is missing the 'id' field",
        reason: "Invalid manifest",
        detail: "One or more mod manifests are missing the required id field.",
    },
    CrashPattern {
        pattern: "Connection timed out",
        reason: "Connection timed out",
        detail: "A multiplayer or remote service request timed out.",
    },
    CrashPattern {
        pattern: "FATAL",
        reason: "Fatal error",
        detail: "The game reported a fatal error before shutting down.",
    },
    CrashPattern {
        pattern: "Unhandled exception",
        reason: "Unhandled exception",
        detail: "The game encountered an unhandled exception before shutting down.",
    },
    CrashPattern {
        pattern: "Application crashed",
        reason: "Application crashed",
        detail: "The game reported a crash in the current log.",
    },
    CrashPattern {
        pattern: "rendering device lost",
        reason: "Rendering device lost",
        detail: "The GPU device was lost. Update drivers or lower graphics settings.",
    },
];

fn read_log_safe(path: &Path) -> String {
    const MAX_SIZE: u64 = 512 * 1024;
    if !path.exists() {
        return String::new();
    }
    if let Ok(meta) = path.metadata() {
        if meta.len() <= MAX_SIZE {
            return fs::read_to_string(path).unwrap_or_default();
        }
        if let Ok(content) = fs::read(path) {
            let start = if content.len() > MAX_SIZE as usize {
                content.len() - MAX_SIZE as usize
            } else {
                0
            };
            let text = String::from_utf8_lossy(&content[start..]).to_string();
            if let Some(newline) = text.find('\n') {
                return format!(
                    "[... log truncated, showing tail only ...]\n{}",
                    &text[newline + 1..]
                );
            }
            return text;
        }
    }
    String::new()
}

#[tauri::command]
pub fn game_analyze_crash(state: tauri::State<'_, AppState>) -> CrashReport {
    let empty = CrashReport {
        issues: vec![],
        log_file: None,
        error_count: 0,
        warn_count: 0,
        involved_mods: vec![],
        loaded_mods: vec![],
        notices: vec![],
    };

    let Some(profile) = current_profile(&state) else {
        return empty;
    };
    if !profile.crash_analysis_enabled {
        return empty;
    }

    let Some(logs_dir) = profile_logs_dir(&profile) else {
        return empty;
    };
    if !logs_dir.exists() {
        return empty;
    }

    let mut files: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&logs_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("godot2") && name.ends_with(".log") {
                if let Ok(meta) = entry.metadata() {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|duration| duration.as_millis() as u64)
                        .unwrap_or(0);
                    files.push((name, mtime));
                }
            }
        }
    }
    files.sort_by(|left, right| right.1.cmp(&left.1));
    if files.is_empty() {
        return empty;
    }

    let latest_file = &files[0].0;
    let file_path = logs_dir.join(latest_file);
    let content = read_log_safe(&file_path);

    let mut loaded_mods: Vec<(String, String)> = Vec::new();
    let mut failed_manifests: Vec<(String, String)> = Vec::new();
    let mut error_mods: HashMap<String, Vec<String>> = HashMap::new();

    for line in content.lines() {
        if line.contains("Finished mod initialization for '") {
            if let Some(start) = line.find("for '") {
                let rest = &line[start + 5..];
                if let Some(end) = rest.find("' (") {
                    let name = rest[..end].to_string();
                    let rest = &rest[end + 3..];
                    if let Some(end) = rest.find(')') {
                        let id = rest[..end].to_string();
                        loaded_mods.push((name, id));
                    }
                }
            }
            continue;
        }

        if line.contains("[ERROR]") && line.contains("Mod manifest") && line.contains("is missing")
        {
            if let Some(mods_index) = line.find("mods") {
                let rest = &line[mods_index..];
                let parts: Vec<&str> = rest.split(|ch| ch == '\\' || ch == '/').collect();
                if parts.len() >= 3 {
                    failed_manifests.push((parts[1].to_string(), parts[2].trim().to_string()));
                }
            }
            continue;
        }

        if line.contains("[ERROR]")
            && !line.contains("Mod manifest")
            && !line.contains("is missing the")
        {
            if let Some(mods_index) = line.find("mods") {
                let rest = &line[mods_index..];
                let parts: Vec<&str> = rest.split(|ch| ch == '\\' || ch == '/').collect();
                if parts.len() >= 2 {
                    let mod_name = parts[1]
                        .trim_end_matches(".json")
                        .trim_end_matches(".dll")
                        .trim_end_matches(".pck")
                        .to_string();
                    let entry = error_mods.entry(mod_name).or_default();
                    let sample = line
                        .replace("[ERROR]", "")
                        .trim()
                        .chars()
                        .take(120)
                        .collect::<String>();
                    entry.push(sample);
                }
            }
        }
    }

    let loaded_ids: HashSet<String> = loaded_mods.iter().map(|(_, id)| id.clone()).collect();
    let mut really_failed = Vec::new();
    let mut config_warnings = Vec::new();

    for (dir, file) in &failed_manifests {
        if loaded_ids.contains(dir) || loaded_mods.iter().any(|(_, id)| id == dir) {
            config_warnings.push(format!(
                "{}/{} looks like a config file, not a mod manifest; the mod still loaded.",
                dir, file
            ));
        } else {
            really_failed.push(dir.clone());
        }
    }

    let content_lower = content.to_lowercase();
    let mut issues = Vec::new();
    let mut seen_reasons = HashSet::new();

    for pattern in CRASH_PATTERNS {
        if content_lower.contains(&pattern.pattern.to_lowercase())
            && seen_reasons.insert(pattern.reason.to_string())
        {
            if pattern.reason == "Invalid manifest" && really_failed.is_empty() {
                continue;
            }

            let mut issue = CrashIssue {
                reason: pattern.reason.to_string(),
                detail: pattern.detail.to_string(),
                mods: vec![],
            };

            if pattern.reason == "Invalid manifest" && !really_failed.is_empty() {
                issue.mods = really_failed.clone();
                issue.detail = format!(
                    "The following mod manifests are missing the id field: {}",
                    really_failed.join(", ")
                );
            }

            issues.push(issue);
        }
    }

    let mut involved_mods = Vec::new();
    for (name, errors) in &error_mods {
        involved_mods.push(InvolvedMod {
            name: name.clone(),
            error_count: errors.len(),
            sample: errors.first().cloned().unwrap_or_default(),
        });
    }
    for mod_name in &really_failed {
        if !involved_mods.iter().any(|item| &item.name == mod_name) {
            involved_mods.push(InvolvedMod {
                name: mod_name.clone(),
                error_count: 1,
                sample: "manifest is invalid and the mod did not load".to_string(),
            });
        }
    }
    involved_mods.sort_by(|left, right| right.error_count.cmp(&left.error_count));

    let error_count = content
        .lines()
        .filter(|line| line.contains("[ERROR]"))
        .count();
    let warn_count = content
        .lines()
        .filter(|line| line.contains("[WARN]"))
        .count();

    CrashReport {
        issues,
        log_file: Some(latest_file.clone()),
        error_count,
        warn_count,
        involved_mods,
        loaded_mods: loaded_mods.into_iter().map(|(name, _)| name).collect(),
        notices: config_warnings,
    }
}
