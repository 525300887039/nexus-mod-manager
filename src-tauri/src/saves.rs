//! src-tauri 薄壳：4 个 `#[tauri::command]` 转发到 `nmm_core::saves`。
//!
//! 业务逻辑（saves 扫描、ZIP 打包导入导出、备份目录管理）由 core 提供；
//! 本层仅负责弹文件 dialog（`saves_export` / `saves_import` 用）以及从
//! `AppState.current_profile` 取 profile。

use crate::game_profile::GameProfile;
use crate::AppState;
use nmm_core::saves as core_saves;
use tauri_plugin_dialog::DialogExt;

// Re-export 业务类型，让 src-tauri 内其他模块（如 lib.rs 命令注册）可继续通过
// `crate::saves::xxx` 访问。
#[allow(unused_imports)]
pub use core_saves::{
    BackupEntry, CharacterStat, ProgressSummary, SaveExportOpts, SaveSlot, SavesResult,
    SimpleResult,
};

fn current_profile(state: &tauri::State<'_, AppState>) -> Option<GameProfile> {
    state
        .current_profile
        .lock()
        .ok()
        .and_then(|profile| profile.clone())
}

#[tauri::command]
pub fn saves_scan(state: tauri::State<'_, AppState>) -> SavesResult {
    let Some(profile) = current_profile(&state) else {
        return SavesResult {
            slots: vec![],
            backups: vec![],
        };
    };
    if !profile.saves_enabled {
        return SavesResult {
            slots: vec![],
            backups: vec![],
        };
    }
    core_saves::scan_saves(&profile)
}

#[tauri::command]
pub async fn saves_export(
    app: tauri::AppHandle,
    opts: SaveExportOpts,
    state: tauri::State<'_, AppState>,
) -> Result<SimpleResult, String> {
    let Some(profile) = current_profile(&state) else {
        return Ok(SimpleResult {
            success: false,
            error: Some("No game selected".into()),
        });
    };
    if !profile.saves_enabled {
        return Ok(SimpleResult {
            success: false,
            error: Some("Save management is disabled for the current game".into()),
        });
    }

    let tag = if opts.modded {
        format!("{}_modded", opts.slot)
    } else {
        opts.slot.clone()
    };
    let ts = core_saves::timestamp_string();
    let default_name = format!("Save_{}_{}.zip", tag, ts);

    let save_path = app
        .dialog()
        .file()
        .set_title("导出存档")
        .set_file_name(&default_name)
        .add_filter("ZIP Archive", &["zip"])
        .blocking_save_file();

    let Some(dest) = save_path else {
        return Ok(SimpleResult {
            success: false,
            error: None,
        });
    };

    core_saves::export_save_to_zip(&profile, &opts, &dest.to_string())
}

#[tauri::command]
pub async fn saves_import(
    app: tauri::AppHandle,
    opts: SaveExportOpts,
    state: tauri::State<'_, AppState>,
) -> Result<SimpleResult, String> {
    let Some(profile) = current_profile(&state) else {
        return Ok(SimpleResult {
            success: false,
            error: Some("No game selected".into()),
        });
    };
    if !profile.saves_enabled {
        return Ok(SimpleResult {
            success: false,
            error: Some("Save management is disabled for the current game".into()),
        });
    }

    let file = app
        .dialog()
        .file()
        .set_title(&format!("导入存档到 {}", opts.slot))
        .add_filter("ZIP Archive", &["zip"])
        .blocking_pick_file();

    let Some(zip_path) = file else {
        return Ok(SimpleResult {
            success: false,
            error: None,
        });
    };

    core_saves::import_save_from_zip(&profile, &opts, &zip_path.to_string())
}

#[tauri::command]
pub fn saves_delete_backup(backup_path: String) -> SimpleResult {
    core_saves::delete_backup(&backup_path)
}
