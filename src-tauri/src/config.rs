//! src-tauri 薄壳层：把 10 个 config 命令转发到 `nmm_core::config`，并实现
//! GUI 专属的副作用（`sync_state` 写回 `AppState`、`sync_translations` 调 mods 依赖、
//! `app_select_game_path` 弹文件选择对话框）。

use crate::AppState;
use nmm_core::config as core_config;
use nmm_core::game_profile::GameProfile;
use tauri_plugin_dialog::DialogExt;

// Re-export 业务类型与函数，让 src-tauri 其他模块（lib.rs / mods / saves / ...）
// 可继续通过 `crate::config::xxx` 访问。
#[allow(unused_imports)]
pub use core_config::{
    build_init_result, collect_available_games, current_game_config, current_game_domain,
    detect_game_path, load_config, load_current_profile, load_or_detect_game_path, mods_dir_for,
    persist_current_game_path, resolve_game_path_from_config, save_config, save_config_inner,
    validate_game_path, validate_required_game_path, Config, ConfigListEntry, ConfigListResult,
    GameConfig, InitResult,
};

fn sync_state(
    state: &tauri::State<'_, AppState>,
    profile: Option<GameProfile>,
    game_path: Option<String>,
) {
    if let Ok(mut current_profile) = state.current_profile.lock() {
        *current_profile = profile.clone();
    }
    if let Ok(mut state_game_path) = state.game_path.lock() {
        *state_game_path = game_path;
    }
}

fn sync_translations(
    state: &tauri::State<'_, AppState>,
    profile: Option<&GameProfile>,
    game_path: Option<&str>,
) -> Result<(), String> {
    let Some(game_path) = game_path else {
        return Ok(());
    };
    let Some(profile) = profile else {
        return Ok(());
    };

    let mut db = state
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {}", e))?;
    crate::db::sync_saved_translations_with_game_path(&mut db, &profile.nexus_domain, game_path)
}

#[tauri::command]
pub fn app_init(state: tauri::State<'_, AppState>) -> InitResult {
    let mut cfg = core_config::load_config();
    let detected = core_config::resolve_game_path_from_config(&cfg);
    let current_profile = core_config::current_game_config(&cfg).map(|game| game.profile.clone());

    if let Some(path) = detected.clone() {
        let should_persist = core_config::current_game_config(&cfg)
            .and_then(|game| game.game_path.clone())
            .as_deref()
            != Some(path.as_str());
        if should_persist {
            core_config::persist_current_game_path(&mut cfg, Some(path));
            core_config::save_config(&cfg);
        }
    }

    sync_state(&state, current_profile.clone(), detected.clone());
    let _ = sync_translations(&state, current_profile.as_ref(), detected.as_deref());

    core_config::build_init_result(&cfg, detected)
}

#[tauri::command]
pub async fn app_select_game_path(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Option<InitResult>, String> {
    let mut cfg = core_config::load_config();
    let current_game = core_config::current_game_config(&cfg)
        .cloned()
        .ok_or_else(|| "no current game selected".to_string())?;

    let folder = app
        .dialog()
        .file()
        .set_title(format!(
            "Select {} directory",
            current_game.profile.display_name
        ))
        .blocking_pick_folder();

    let Some(folder_path) = folder else {
        return Ok(None);
    };

    let game_path = folder_path.to_string();
    core_config::persist_current_game_path(&mut cfg, Some(game_path.clone()));
    core_config::save_config_inner(&cfg)?;
    sync_state(
        &state,
        Some(current_game.profile.clone()),
        Some(game_path.clone()),
    );
    sync_translations(&state, Some(&current_game.profile), Some(&game_path))?;

    Ok(Some(core_config::build_init_result(&cfg, Some(game_path))))
}

#[tauri::command]
pub fn config_list_games() -> ConfigListResult {
    let cfg = core_config::load_config();
    let current_domain = core_config::current_game_domain(&cfg).map(str::to_string);

    let games = core_config::collect_available_games(&cfg)
        .into_iter()
        .map(|profile| {
            let entry = cfg.games.get(&profile.nexus_domain);
            ConfigListEntry {
                game_path: entry.and_then(|game| game.game_path.clone()),
                is_current: current_domain.as_deref() == Some(profile.nexus_domain.as_str()),
                is_preset: GameProfile::default_for(&profile.nexus_domain).is_some(),
                profile,
            }
        })
        .collect();

    ConfigListResult {
        current_game: core_config::current_game_config(&cfg).map(|game| game.profile.clone()),
        games,
    }
}

#[tauri::command]
pub fn config_switch_game(
    state: tauri::State<'_, AppState>,
    nexus_domain: String,
) -> Result<InitResult, String> {
    let nexus_domain = nexus_domain.trim();
    if nexus_domain.is_empty() {
        return Err("game domain cannot be empty".to_string());
    }

    let mut cfg = core_config::load_config();
    if !cfg.games.contains_key(nexus_domain) {
        let profile = GameProfile::default_for(nexus_domain)
            .ok_or_else(|| format!("unknown game domain: {}", nexus_domain))?;
        cfg.games.insert(
            nexus_domain.to_string(),
            core_config::default_game_config(profile),
        );
    }

    cfg.current_game = Some(nexus_domain.to_string());
    let game_path = core_config::resolve_game_path_from_config(&cfg);
    core_config::persist_current_game_path(&mut cfg, game_path.clone());
    core_config::save_config_inner(&cfg)?;

    let current_profile = core_config::current_game_config(&cfg).map(|game| game.profile.clone());
    sync_state(&state, current_profile.clone(), game_path.clone());
    sync_translations(&state, current_profile.as_ref(), game_path.as_deref())?;

    Ok(core_config::build_init_result(&cfg, game_path))
}

#[tauri::command]
pub fn config_add_game(
    state: tauri::State<'_, AppState>,
    profile: GameProfile,
    game_path: Option<String>,
) -> Result<InitResult, String> {
    let profile = core_config::normalize_profile(profile);
    if profile.nexus_domain.is_empty() {
        return Err("game domain cannot be empty".to_string());
    }
    if profile.display_name.is_empty() {
        return Err("display name cannot be empty".to_string());
    }

    let mut cfg = core_config::load_config();
    if cfg.games.contains_key(&profile.nexus_domain) {
        return Err(format!("该 Nexus 域名已存在：{}", profile.nexus_domain));
    }
    let game_path = core_config::validate_required_game_path(game_path.as_deref())?;
    cfg.games.insert(
        profile.nexus_domain.clone(),
        GameConfig {
            game_path: Some(game_path.clone()),
            profile: profile.clone(),
        },
    );
    cfg.current_game = Some(profile.nexus_domain.clone());
    core_config::save_config_inner(&cfg)?;

    sync_state(&state, Some(profile.clone()), Some(game_path.clone()));
    sync_translations(&state, Some(&profile), Some(game_path.as_str()))?;

    Ok(core_config::build_init_result(&cfg, Some(game_path)))
}

#[tauri::command]
pub fn config_remove_game(
    state: tauri::State<'_, AppState>,
    nexus_domain: String,
) -> Result<InitResult, String> {
    let nexus_domain = nexus_domain.trim();
    if nexus_domain.is_empty() {
        return Err("game domain cannot be empty".to_string());
    }

    let mut cfg = core_config::load_config();
    cfg.games.remove(nexus_domain);

    if let Some(profile) = GameProfile::default_for(nexus_domain) {
        cfg.games.insert(
            nexus_domain.to_string(),
            core_config::default_game_config(profile),
        );
    }

    core_config::normalize_current_game(&mut cfg);
    let game_path = core_config::resolve_game_path_from_config(&cfg);
    core_config::persist_current_game_path(&mut cfg, game_path.clone());
    core_config::save_config_inner(&cfg)?;

    let current_profile = core_config::current_game_config(&cfg).map(|game| game.profile.clone());
    sync_state(&state, current_profile.clone(), game_path.clone());
    sync_translations(&state, current_profile.as_ref(), game_path.as_deref())?;

    Ok(core_config::build_init_result(&cfg, game_path))
}

#[tauri::command]
pub fn config_save_nexus_key(
    state: tauri::State<'_, AppState>,
    key: String,
) -> Result<(), String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("API key cannot be empty".to_string());
    }

    let mut cfg = core_config::load_config();
    cfg.nexus_api_key = Some(trimmed.to_string());
    core_config::save_config_inner(&cfg)?;

    // 换 Key 后清空 premium 缓存，下次下载时重新判定会员状态
    if let Ok(mut premium) = state.nexus_is_premium.lock() {
        *premium = None;
    }
    Ok(())
}

#[tauri::command]
pub fn config_get_nexus_key() -> Option<String> {
    core_config::load_config()
        .nexus_api_key
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

#[tauri::command]
pub fn config_set_nexus_download_visible(visible: bool) -> Result<(), String> {
    let mut cfg = core_config::load_config();
    cfg.nexus_download_window_visible = Some(visible);
    core_config::save_config_inner(&cfg)
}

#[tauri::command]
pub fn config_get_nexus_download_visible() -> bool {
    core_config::load_config()
        .nexus_download_window_visible
        .unwrap_or(true)
}
