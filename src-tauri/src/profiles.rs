//! src-tauri 薄壳：把 2 个 `#[tauri::command]` 转发到 `nmm_core::profiles`。
//!
//! 业务逻辑（profile JSON 读写、legacy 路径迁移）由 core 提供；本层仅负责
//! 从 `AppState.current_profile` 提取当前 `nexus_domain` 并转发。

use crate::AppState;
use nmm_core::profiles as core_profiles;
use serde_json::Value;

fn current_game_domain(state: &tauri::State<'_, AppState>) -> Option<String> {
    state
        .current_profile
        .lock()
        .ok()
        .and_then(|profile| profile.as_ref().map(|profile| profile.nexus_domain.clone()))
}

#[tauri::command]
pub fn profiles_load(state: tauri::State<'_, AppState>) -> Value {
    let Some(nexus_domain) = current_game_domain(&state) else {
        return serde_json::json!({});
    };
    core_profiles::load(&nexus_domain)
}

#[tauri::command]
pub fn profiles_save(state: tauri::State<'_, AppState>, profiles: Value) -> Value {
    let Some(nexus_domain) = current_game_domain(&state) else {
        return serde_json::json!({
            "success": false,
            "error": "no current game selected"
        });
    };
    core_profiles::save(&nexus_domain, profiles)
}
