//! Mods profile（mod 组合方案）的持久化读写。
//!
//! 每个游戏（`nexus_domain`）一份 `profiles_<domain>.json`，存在用户配置目录下。
//! 早期版本只有单一 `profiles.json`，迁移逻辑兼容旧路径。
//!
//! 本模块无 Tauri 依赖；GUI / CLI 通过传入 `nexus_domain` 字符串调用。

use crate::game_profile::preset_games;
use crate::paths;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn profiles_path(nexus_domain: &str) -> PathBuf {
    paths::current_config_file(&format!("profiles_{nexus_domain}.json"))
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
    for dir in [paths::current_config_dir(), paths::legacy_config_dir()]
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

/// 加载指定游戏的 profile 集合。若新路径不存在，会尝试从旧 `profiles.json` 迁移。
pub fn load(nexus_domain: &str) -> Value {
    let path = profiles_path(nexus_domain);
    if let Some(value) = read_profiles(&path) {
        return value;
    }

    for legacy_path in legacy_profiles_candidates(nexus_domain) {
        if let Some(value) = read_profiles(&legacy_path) {
            migrate_profiles_if_needed(&path, &value);
            return value;
        }
    }

    serde_json::json!({})
}

/// 保存指定游戏的 profile 集合到 JSON 文件。
pub fn save(nexus_domain: &str, profiles: Value) -> Value {
    let path = profiles_path(nexus_domain);
    migrate_profiles_if_needed(&path, &profiles);
    serde_json::json!({ "success": true })
}
