//! `nmm mods` 子命令组。

use crate::output::print_result;
use crate::{ModsAction, ModsArgs};
use nmm_core::mods::{self as core_mods, ToggleModInfo};
use nmm_core::AppContext;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct ModEntry {
    name: String,
    enabled: bool,
    size: u64,
    #[serde(rename = "isFolder")]
    is_folder: bool,
}

#[derive(Serialize)]
struct ModsListResult {
    #[serde(rename = "gamePath")]
    game_path: String,
    #[serde(rename = "modsDir")]
    mods_dir: String,
    mods: Vec<ModEntry>,
}

#[derive(Serialize)]
struct ModsActionResult {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    installed: Option<Vec<String>>,
    #[serde(rename = "backupPath", skip_serializing_if = "Option::is_none")]
    backup_path: Option<String>,
    #[serde(rename = "backupId", skip_serializing_if = "Option::is_none")]
    backup_id: Option<String>,
}

pub async fn run(ctx: &AppContext, json: bool, args: ModsArgs) -> Result<(), String> {
    match args.action {
        ModsAction::List { enabled, disabled } => list(ctx, json, enabled, disabled),
        ModsAction::Enable { name } => toggle(ctx, json, name, true),
        ModsAction::Disable { name } => toggle(ctx, json, name, false),
        ModsAction::Install { path } => install(ctx, json, path),
        ModsAction::Uninstall { name } => uninstall(ctx, json, name),
        ModsAction::Backup => backup(ctx, json),
        ModsAction::Restore { backup_id } => restore(ctx, json, backup_id),
    }
}

fn list(
    ctx: &AppContext,
    json: bool,
    only_enabled: bool,
    only_disabled: bool,
) -> Result<(), String> {
    let game_path = ctx
        .game_path
        .lock()
        .map_err(|e| format!("game_path lock poisoned: {}", e))?
        .clone()
        .ok_or_else(|| "尚未选择游戏路径，请用 `nmm games switch <domain>` 设置当前游戏".to_string())?;

    let mods_subdir = ctx
        .current_profile
        .lock()
        .map_err(|e| format!("current_profile lock poisoned: {}", e))?
        .as_ref()
        .map(core_mods::current_mods_subdir)
        .unwrap_or_else(|| "mods".to_string());

    let mods_dir = core_mods::get_mods_dir(&game_path, &mods_subdir);

    let raw_mods = core_mods::scan_mods_internal_with_subdir(&game_path, &mods_subdir);
    let filtered: Vec<ModEntry> = raw_mods
        .into_iter()
        .filter(|m| {
            if only_enabled {
                m.enabled
            } else if only_disabled {
                !m.enabled
            } else {
                true
            }
        })
        .map(|m| ModEntry {
            name: m.name.unwrap_or(m.folder_name),
            enabled: m.enabled,
            size: m.size,
            is_folder: m.is_folder,
        })
        .collect();

    let result = ModsListResult {
        game_path,
        mods_dir: mods_dir.to_string_lossy().to_string(),
        mods: filtered,
    };

    print_result(&result, json, |result| {
        let mut out = String::new();
        out.push_str(&format!("mods 目录: {}\n", result.mods_dir));
        out.push_str(&format!("共 {} 个 mod\n", result.mods.len()));
        out.push_str(&format!(
            "{:<8} {:<12} {}\n",
            "enabled", "size", "name"
        ));
        out.push_str(&"-".repeat(80));
        out.push('\n');
        for m in &result.mods {
            out.push_str(&format!(
                "{:<8} {:<12} {}\n",
                if m.enabled { "✓" } else { "✗" },
                human_size(m.size),
                m.name
            ));
        }
        out.trim_end().to_string()
    })
}

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit_idx])
    }
}

/// 取当前游戏的 (game_path, mods_subdir, mods_dir)。所有写命令都要用，统一封装一次。
fn require_game_context(ctx: &AppContext) -> Result<(String, String, PathBuf), String> {
    let game_path = ctx
        .game_path
        .lock()
        .map_err(|e| format!("game_path lock poisoned: {}", e))?
        .clone()
        .ok_or_else(|| "尚未选择游戏路径，请用 `nmm games switch <domain>` 设置当前游戏".to_string())?;
    let mods_subdir = ctx
        .current_profile
        .lock()
        .map_err(|e| format!("current_profile lock poisoned: {}", e))?
        .as_ref()
        .map(core_mods::current_mods_subdir)
        .unwrap_or_else(|| "mods".to_string());
    let mods_dir = core_mods::get_mods_dir(&game_path, &mods_subdir);
    Ok((game_path, mods_subdir, mods_dir))
}

/// 按名字（display name 或 folder name）查找目标 mod，构造 ToggleModInfo。
fn find_target_mod(
    game_path: &str,
    mods_subdir: &str,
    name: &str,
) -> Result<ToggleModInfo, String> {
    let scanned = core_mods::scan_mods_internal_with_subdir(game_path, mods_subdir);
    let needle = name.trim().to_lowercase();
    let target = scanned
        .iter()
        .find(|m| {
            m.folder_name.to_lowercase() == needle
                || m.name.as_deref().map(str::to_lowercase) == Some(needle.clone())
        })
        .ok_or_else(|| {
            format!(
                "未找到名为 `{}` 的 mod。\n请用 `nmm mods list` 查看可用 mod 列表。",
                name
            )
        })?;
    Ok(ToggleModInfo {
        is_folder: target.is_folder,
        folder_name: target.folder_name.clone(),
        files: Some(target.files.clone()),
        enabled: target.enabled,
    })
}

fn toggle(
    ctx: &AppContext,
    json: bool,
    name: String,
    target_enabled: bool,
) -> Result<(), String> {
    let (game_path, mods_subdir, _mods_dir) = require_game_context(ctx)?;
    let mod_info = find_target_mod(&game_path, &mods_subdir, &name)?;

    if mod_info.enabled == target_enabled {
        let state_label = if target_enabled { "已启用" } else { "已禁用" };
        let result = ModsActionResult {
            success: true,
            message: format!("{} 已是{}状态，无需变更", mod_info.folder_name, state_label),
            installed: None,
            backup_path: None,
            backup_id: None,
        };
        return print_result(&result, json, |r| r.message.clone());
    }

    let outcome = core_mods::toggle_mod(&game_path, &mods_subdir, &mod_info);
    if !outcome.success {
        return Err(outcome
            .error
            .unwrap_or_else(|| "toggle 失败但未返回原因".to_string()));
    }

    let action_label = if target_enabled { "已启用" } else { "已禁用" };
    let result = ModsActionResult {
        success: true,
        message: format!("{}：{}", action_label, mod_info.folder_name),
        installed: None,
        backup_path: None,
        backup_id: None,
    };
    print_result(&result, json, |r| r.message.clone())
}

fn install(ctx: &AppContext, json: bool, path: String) -> Result<(), String> {
    let (_game_path, _mods_subdir, mods_dir) = require_game_context(ctx)?;

    let archive_path = Path::new(&path);
    if !archive_path.exists() {
        return Err(format!("文件不存在: {}", path));
    }
    if !core_mods::is_supported_archive_path(archive_path) {
        return Err(format!(
            "不是支持的归档格式（仅支持 .zip / .rar / .7z）: {}",
            path
        ));
    }
    fs::create_dir_all(&mods_dir).map_err(|e| format!("无法创建 mods 目录: {}", e))?;

    let installed = core_mods::install_paths_to_mods_dir(&[path.clone()], &mods_dir)?;
    let result = ModsActionResult {
        success: true,
        message: format!("已安装 {} 个 mod", installed.len()),
        installed: Some(installed),
        backup_path: None,
        backup_id: None,
    };
    print_result(&result, json, |r| {
        match &r.installed {
            Some(items) => {
                let mut s = format!("{}\n", r.message);
                for item in items {
                    s.push_str(&format!("  + {}\n", item));
                }
                s.trim_end().to_string()
            }
            None => r.message.clone(),
        }
    })
}

fn uninstall(ctx: &AppContext, json: bool, name: String) -> Result<(), String> {
    let (game_path, mods_subdir, _mods_dir) = require_game_context(ctx)?;
    let mod_info = find_target_mod(&game_path, &mods_subdir, &name)?;

    let outcome = core_mods::uninstall_mod(&game_path, &mods_subdir, &mod_info);
    if !outcome.success {
        return Err(outcome
            .error
            .unwrap_or_else(|| "uninstall 失败但未返回原因".to_string()));
    }
    let result = ModsActionResult {
        success: true,
        message: format!("已卸载: {}", mod_info.folder_name),
        installed: None,
        backup_path: None,
        backup_id: None,
    };
    print_result(&result, json, |r| r.message.clone())
}

/// CLI backup 约定：写到 `<game_path>/mods-backups/mods_backup_<timestamp>.zip`，
/// 返回 backup id（含 .zip 扩展名的文件名），供 `restore <id>` 使用。
const BACKUP_SUBDIR: &str = "mods-backups";

fn backup(ctx: &AppContext, json: bool) -> Result<(), String> {
    let (game_path, _mods_subdir, mods_dir) = require_game_context(ctx)?;

    let backup_dir = Path::new(&game_path).join(BACKUP_SUBDIR);
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("无法创建备份目录 {}: {}", backup_dir.display(), e))?;

    let backup_filename = format!("mods_backup_{}.zip", core_mods::chrono_timestamp());
    let backup_path = backup_dir.join(&backup_filename);
    let backup_path_str = backup_path.to_string_lossy().to_string();

    core_mods::create_zip_from_mods_dir(&mods_dir, &backup_path_str)?;

    let result = ModsActionResult {
        success: true,
        message: format!("已备份到 {}", backup_path.display()),
        installed: None,
        backup_path: Some(backup_path_str),
        backup_id: Some(backup_filename),
    };
    print_result(&result, json, |r| r.message.clone())
}

fn restore(ctx: &AppContext, json: bool, backup_id_or_path: String) -> Result<(), String> {
    let (game_path, _mods_subdir, mods_dir) = require_game_context(ctx)?;

    // 含路径分隔符 → 当完整 path 用；否则按 backup-id 在 mods-backups/ 里查找
    let backup_path = if backup_id_or_path.contains('/') || backup_id_or_path.contains('\\') {
        PathBuf::from(&backup_id_or_path)
    } else {
        Path::new(&game_path)
            .join(BACKUP_SUBDIR)
            .join(&backup_id_or_path)
    };

    if !backup_path.exists() {
        let mut hint = format!("备份文件不存在: {}", backup_path.display());
        let backup_dir = Path::new(&game_path).join(BACKUP_SUBDIR);
        if let Ok(entries) = fs::read_dir(&backup_dir) {
            hint.push_str("\n可用备份：");
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".zip") {
                    hint.push_str(&format!("\n  {}", fname));
                }
            }
        }
        return Err(hint);
    }

    fs::create_dir_all(&mods_dir).map_err(|e| format!("无法创建 mods 目录: {}", e))?;
    let backup_path_str = backup_path.to_string_lossy().to_string();
    core_mods::smart_extract_archive(&backup_path_str, &mods_dir)?;

    let result = ModsActionResult {
        success: true,
        message: format!("已恢复备份 {}", backup_path.display()),
        installed: None,
        backup_path: Some(backup_path_str),
        backup_id: None,
    };
    print_result(&result, json, |r| r.message.clone())
}
