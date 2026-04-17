use crate::{app_paths, game_profile::preset_games, AppState};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn current_game_domain(state: &tauri::State<'_, AppState>) -> Option<String> {
    state
        .current_profile
        .lock()
        .ok()
        .and_then(|profile| profile.as_ref().map(|profile| profile.nexus_domain.clone()))
}

fn profiles_path(nexus_domain: &str) -> PathBuf {
    app_paths::current_config_file(&format!("profiles_{nexus_domain}.json"))
}

fn default_game_domain() -> Option<String> {
    preset_games()
        .into_iter()
        .next()
        .map(|profile| profile.nexus_domain)
}

fn read_profiles(path: &Path) -> Option<Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
}

fn legacy_profiles_candidates(nexus_domain: &str) -> Vec<PathBuf> {
    if default_game_domain().as_deref() != Some(nexus_domain) {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for dir in [
        app_paths::current_config_dir(),
        app_paths::legacy_config_dir(),
    ]
    .into_iter()
    .flatten()
    {
        let candidate = dir.join("profiles.json");
        if !candidates.iter().any(|path: &PathBuf| path == &candidate) {
            candidates.push(candidate);
        }
    }

    candidates
}

fn migrate_profiles_if_needed(path: &Path, profiles: &Value) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(json) = serde_json::to_string_pretty(profiles) {
        let _ = fs::write(path, json);
    }
}

#[tauri::command]
pub fn profiles_load(state: tauri::State<'_, AppState>) -> Value {
    let Some(nexus_domain) = current_game_domain(&state) else {
        return serde_json::json!({});
    };

    let path = profiles_path(&nexus_domain);
    if let Some(value) = read_profiles(&path) {
        return value;
    }

    for legacy_path in legacy_profiles_candidates(&nexus_domain) {
        if let Some(value) = read_profiles(&legacy_path) {
            migrate_profiles_if_needed(&path, &value);
            return value;
        }
    }

    serde_json::json!({})
}

#[tauri::command]
pub fn profiles_save(state: tauri::State<'_, AppState>, profiles: Value) -> serde_json::Value {
    let Some(nexus_domain) = current_game_domain(&state) else {
        return serde_json::json!({
            "success": false,
            "error": "no current game selected"
        });
    };

    let path = profiles_path(&nexus_domain);
    migrate_profiles_if_needed(&path, &profiles);
    serde_json::json!({ "success": true })
}
