//! src-tauri 薄壳：7 个 `#[tauri::command]` 转发到 `nmm_core::mods`。
//!
//! 业务逻辑（mod 扫描、启停、卸载、安装、备份、还原）由 core 提供；
//! 本层仅负责：
//! - 从 `AppState` 锁出 `game_path` 与当前 profile 的 `mods_subdir`
//! - `mods_install` / `mods_backup` / `mods_restore` 弹 dialog 选路径
//! - 把核心调用结果包装成 `ModResult`

use crate::AppState;
use nmm_core::mods as core_mods;
use tauri_plugin_dialog::DialogExt;

// Re-export 业务符号，让 src-tauri 其他模块（nexus_api / nexus_download / translations / lib.rs）
// 可继续通过 `crate::mods::xxx` 访问，零下游改动。
#[allow(unused_imports)]
pub use core_mods::{
    create_zip_from_mods_dir, get_mods_dir, install_paths_to_mods_dir, is_supported_archive_path,
    scan_mods_internal, scan_mods_internal_with_subdir, smart_extract_archive, ModInfo, ModResult,
    ToggleModInfo,
};

fn current_mods_subdir_from_state(state: &tauri::State<'_, AppState>) -> String {
    state
        .current_profile
        .lock()
        .ok()
        .and_then(|profile| profile.as_ref().map(core_mods::current_mods_subdir))
        .unwrap_or_else(|| "mods".to_string())
}

fn current_game_path(state: &tauri::State<'_, AppState>) -> Option<String> {
    state.game_path.lock().ok().and_then(|gp| gp.clone())
}

#[tauri::command]
pub fn mods_scan(state: tauri::State<'_, AppState>) -> Vec<ModInfo> {
    let Some(game_path) = current_game_path(&state) else {
        return vec![];
    };
    let mods_subdir = current_mods_subdir_from_state(&state);
    core_mods::scan_mods_internal_with_subdir(&game_path, &mods_subdir)
}

#[tauri::command]
pub fn mods_toggle(state: tauri::State<'_, AppState>, mod_info: ToggleModInfo) -> ModResult {
    let Some(game_path) = current_game_path(&state) else {
        return ModResult {
            success: false,
            error: Some("Game path not set".into()),
            mods: None,
            installed: None,
        };
    };
    let mods_subdir = current_mods_subdir_from_state(&state);
    core_mods::toggle_mod(&game_path, &mods_subdir, &mod_info)
}

#[tauri::command]
pub fn mods_uninstall(state: tauri::State<'_, AppState>, mod_info: ToggleModInfo) -> ModResult {
    let Some(game_path) = current_game_path(&state) else {
        return ModResult {
            success: false,
            error: Some("Game path not set".into()),
            mods: None,
            installed: None,
        };
    };
    let mods_subdir = current_mods_subdir_from_state(&state);
    core_mods::uninstall_mod(&game_path, &mods_subdir, &mod_info)
}

#[tauri::command]
pub async fn mods_install(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ModResult, String> {
    let Some(game_path) = current_game_path(&state) else {
        return Ok(ModResult {
            success: false,
            error: Some("Game path not set".into()),
            mods: None,
            installed: None,
        });
    };
    let mods_subdir = current_mods_subdir_from_state(&state);
    let mods_dir = core_mods::get_mods_dir(&game_path, &mods_subdir);

    let files = app
        .dialog()
        .file()
        .set_title("Select MOD Archive")
        .add_filter("Archives", &["zip", "rar", "7z"])
        .blocking_pick_files();

    let Some(file_paths) = files else {
        return Ok(ModResult {
            success: false,
            error: Some("Cancelled".into()),
            mods: None,
            installed: None,
        });
    };

    let paths: Vec<String> = file_paths.into_iter().map(|fp| fp.to_string()).collect();
    match core_mods::install_paths_to_mods_dir(&paths, &mods_dir) {
        Ok(installed) => Ok(ModResult {
            success: true,
            error: None,
            mods: Some(core_mods::scan_mods_internal_with_subdir(
                &game_path,
                &mods_subdir,
            )),
            installed: Some(installed),
        }),
        Err(e) => Ok(ModResult {
            success: false,
            error: Some(e),
            mods: None,
            installed: None,
        }),
    }
}

#[tauri::command]
pub fn mods_install_drop(state: tauri::State<'_, AppState>, file_paths: Vec<String>) -> ModResult {
    let Some(game_path) = current_game_path(&state) else {
        return ModResult {
            success: false,
            error: Some("Game path not set".into()),
            mods: None,
            installed: None,
        };
    };
    let mods_subdir = current_mods_subdir_from_state(&state);
    let mods_dir = core_mods::get_mods_dir(&game_path, &mods_subdir);

    match core_mods::install_paths_to_mods_dir(&file_paths, &mods_dir) {
        Ok(installed) => ModResult {
            success: true,
            error: None,
            mods: Some(core_mods::scan_mods_internal_with_subdir(
                &game_path,
                &mods_subdir,
            )),
            installed: Some(installed),
        },
        Err(e) => ModResult {
            success: false,
            error: Some(e),
            mods: None,
            installed: None,
        },
    }
}

#[tauri::command]
pub async fn mods_backup(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ModResult, String> {
    let Some(game_path) = current_game_path(&state) else {
        return Ok(ModResult {
            success: false,
            error: Some("Game path not set".into()),
            mods: None,
            installed: None,
        });
    };
    let mods_subdir = current_mods_subdir_from_state(&state);
    let mods_dir = core_mods::get_mods_dir(&game_path, &mods_subdir);
    let default_name = format!("mods_backup_{}.zip", core_mods::chrono_timestamp());

    let save_path = app
        .dialog()
        .file()
        .set_title("Save MOD Backup")
        .set_file_name(&default_name)
        .add_filter("ZIP Archive", &["zip"])
        .blocking_save_file();

    match save_path {
        Some(path) => match core_mods::create_zip_from_mods_dir(&mods_dir, &path.to_string()) {
            Ok(()) => Ok(ModResult {
                success: true,
                error: None,
                mods: None,
                installed: None,
            }),
            Err(e) => Ok(ModResult {
                success: false,
                error: Some(e),
                mods: None,
                installed: None,
            }),
        },
        None => Ok(ModResult {
            success: false,
            error: None,
            mods: None,
            installed: None,
        }),
    }
}

#[tauri::command]
pub async fn mods_restore(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ModResult, String> {
    let Some(game_path) = current_game_path(&state) else {
        return Ok(ModResult {
            success: false,
            error: Some("Game path not set".into()),
            mods: None,
            installed: None,
        });
    };
    let mods_subdir = current_mods_subdir_from_state(&state);
    let mods_dir = core_mods::get_mods_dir(&game_path, &mods_subdir);

    let file = app
        .dialog()
        .file()
        .set_title("Select MOD Backup")
        .add_filter("ZIP Archive", &["zip"])
        .blocking_pick_file();

    match file {
        Some(path) => match core_mods::smart_extract_archive(&path.to_string(), &mods_dir) {
            Ok(()) => Ok(ModResult {
                success: true,
                error: None,
                mods: Some(core_mods::scan_mods_internal_with_subdir(
                    &game_path,
                    &mods_subdir,
                )),
                installed: None,
            }),
            Err(e) => Ok(ModResult {
                success: false,
                error: Some(e),
                mods: None,
                installed: None,
            }),
        },
        None => Ok(ModResult {
            success: false,
            error: None,
            mods: None,
            installed: None,
        }),
    }
}

