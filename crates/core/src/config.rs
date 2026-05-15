//! 配置读写与游戏路径解析。
//!
//! 本模块无 Tauri 依赖。命令薄壳（含 `app_select_game_path` 的 dialog 流程与
//! `sync_translations` 翻译同步等副作用）由 src-tauri 层负责调用本模块的纯逻辑函数。

use crate::game_profile::{preset_games, GameProfile};
use crate::paths;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
pub struct GameConfig {
    #[serde(rename = "gamePath", alias = "game_path")]
    pub game_path: Option<String>,
    pub profile: GameProfile,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct Config {
    #[serde(rename = "currentGame", alias = "current_game")]
    pub current_game: Option<String>,
    #[serde(rename = "nexusApiKey", alias = "nexus_api_key")]
    pub nexus_api_key: Option<String>,
    #[serde(
        rename = "nexusDownloadWindowVisible",
        alias = "nexus_download_window_visible"
    )]
    pub nexus_download_window_visible: Option<bool>,
    pub games: HashMap<String, GameConfig>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InitResult {
    pub game_path: Option<String>,
    pub mods_dir: Option<String>,
    pub current_game: Option<GameProfile>,
    pub available_games: Vec<GameProfile>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListEntry {
    pub profile: GameProfile,
    pub game_path: Option<String>,
    pub is_current: bool,
    pub is_preset: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListResult {
    pub current_game: Option<GameProfile>,
    pub games: Vec<ConfigListEntry>,
}

fn config_dir() -> PathBuf {
    paths::writable_config_dir()
}

fn config_path() -> PathBuf {
    paths::current_config_file("config.json")
}

fn load_config_path() -> Option<PathBuf> {
    let current = config_path();
    if current.exists() {
        return Some(current);
    }

    paths::existing_config_file("config.json")
}

fn default_current_game_domain() -> Option<String> {
    preset_games()
        .into_iter()
        .next()
        .map(|profile| profile.nexus_domain)
}

pub fn default_game_config(profile: GameProfile) -> GameConfig {
    GameConfig {
        game_path: None,
        profile,
    }
}

pub fn normalize_profile(mut profile: GameProfile) -> GameProfile {
    profile.nexus_domain = profile.nexus_domain.trim().to_lowercase();
    profile.display_name = profile.display_name.trim().to_string();

    if profile.mods_subdir.trim().is_empty() {
        profile.mods_subdir = "mods".to_string();
    } else {
        profile.mods_subdir = profile.mods_subdir.trim().to_string();
    }

    profile.exe_name = profile
        .exe_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    profile.process_name = profile
        .process_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    profile.steam_dir_name = profile
        .steam_dir_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    profile.appdata_dir_name = profile
        .appdata_dir_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    profile.logs_subdir = profile
        .logs_subdir
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    profile
}

fn merge_preset_games(cfg: &mut Config) {
    for preset in preset_games() {
        cfg.games
            .entry(preset.nexus_domain.clone())
            .or_insert_with(|| default_game_config(preset));
    }
}

fn normalize_games(cfg: &mut Config) {
    for (domain, game) in &mut cfg.games {
        game.profile = normalize_profile(game.profile.clone());
        if game.profile.nexus_domain.is_empty() {
            game.profile.nexus_domain = domain.clone();
        }
        game.game_path = validate_game_path(game.game_path.as_deref());
    }
}

pub fn normalize_current_game(cfg: &mut Config) {
    let current_is_valid = cfg
        .current_game
        .as_deref()
        .map(|domain| cfg.games.contains_key(domain))
        .unwrap_or(false);

    if current_is_valid {
        return;
    }

    cfg.current_game = cfg
        .games
        .iter()
        .filter_map(|(domain, game)| game.game_path.as_ref().map(|_| domain.clone()))
        .min();
}

fn legacy_game_path(raw: &Value) -> Option<String> {
    raw.get("gamePath")
        .and_then(Value::as_str)
        .and_then(|path| validate_game_path(Some(path)))
}

fn with_legacy_compat(mut cfg: Config, raw: Option<&Value>) -> Config {
    merge_preset_games(&mut cfg);
    normalize_games(&mut cfg);

    if let Some(path) = raw.and_then(legacy_game_path) {
        let default_domain = default_current_game_domain().unwrap_or_default();
        let entry = cfg.games.entry(default_domain.clone()).or_insert_with(|| {
            default_game_config(
                GameProfile::default_for(&default_domain).unwrap_or(GameProfile {
                    nexus_domain: default_domain.clone(),
                    display_name: "Default Game".to_string(),
                    steam_app_id: None,
                    exe_name: None,
                    process_name: None,
                    steam_dir_name: None,
                    mods_subdir: "mods".to_string(),
                    appdata_dir_name: None,
                    logs_subdir: None,
                    saves_enabled: false,
                    logs_enabled: false,
                    crash_analysis_enabled: false,
                }),
            )
        });
        if entry.game_path.is_none() {
            entry.game_path = Some(path);
        }
    }

    normalize_current_game(&mut cfg);
    cfg
}

pub fn validate_game_path(path: Option<&str>) -> Option<String> {
    let path = path?.trim();
    if path.is_empty() || !Path::new(path).exists() {
        return None;
    }
    Some(path.to_string())
}

pub fn validate_required_game_path(path: Option<&str>) -> Result<String, String> {
    let raw_path = path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| "game path cannot be empty".to_string())?;

    validate_game_path(Some(raw_path))
        .ok_or_else(|| format!("game path does not exist: {}", raw_path))
}

pub fn current_game_domain(cfg: &Config) -> Option<&str> {
    cfg.current_game
        .as_deref()
        .filter(|domain| cfg.games.contains_key(*domain))
}

pub fn current_game_config(cfg: &Config) -> Option<&GameConfig> {
    current_game_domain(cfg).and_then(|domain| cfg.games.get(domain))
}

pub fn load_current_profile() -> Option<GameProfile> {
    current_game_config(&load_config()).map(|game| game.profile.clone())
}

pub fn resolve_game_path_from_config(cfg: &Config) -> Option<String> {
    let game = current_game_config(cfg)?;
    game.game_path
        .clone()
        .filter(|path| Path::new(path).exists())
        .or_else(|| detect_game_path(&game.profile))
}

pub fn load_or_detect_game_path() -> Option<String> {
    resolve_game_path_from_config(&load_config())
}

pub fn load_config() -> Config {
    let target_path = config_path();
    let source_path = load_config_path();
    let raw = source_path
        .as_ref()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|content| serde_json::from_str::<Value>(&content).ok());
    let parsed = raw
        .as_ref()
        .and_then(|value| serde_json::from_value::<Config>(value.clone()).ok())
        .unwrap_or_default();
    let cfg = with_legacy_compat(parsed, raw.as_ref());

    if raw.is_some()
        && source_path
            .as_ref()
            .is_some_and(|path| path != &target_path)
    {
        let _ = save_config_inner(&cfg);
    }

    cfg
}

pub fn save_config_inner(cfg: &Config) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir)
        .map_err(|e| format!("failed to create config dir {}: {}", dir.display(), e))?;
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| format!("failed to serialize config: {}", e))?;
    fs::write(config_path(), json).map_err(|e| format!("failed to write config file: {}", e))?;
    Ok(())
}

pub fn save_config(cfg: &Config) {
    let _ = save_config_inner(cfg);
}

pub fn detect_game_path(profile: &GameProfile) -> Option<String> {
    let steam_dir_name = profile.steam_dir_name.as_deref()?;
    let steam_paths = vec![
        r"C:\Program Files (x86)\Steam",
        r"C:\Program Files\Steam",
        r"D:\Steam",
        r"D:\SteamLibrary",
        r"E:\SteamLibrary",
    ];

    for steam_path in &steam_paths {
        let vdf_path = Path::new(steam_path)
            .join("steamapps")
            .join("libraryfolders.vdf");
        if vdf_path.exists() {
            if let Ok(content) = fs::read_to_string(&vdf_path) {
                for line in content.lines() {
                    if let Some(start) = line.find("\"path\"") {
                        let rest = &line[start + 6..];
                        if let Some(quote_start) = rest.find('"') {
                            let rest = &rest[quote_start + 1..];
                            if let Some(quote_end) = rest.find('"') {
                                let library_path = rest[..quote_end].replace("\\\\", "\\");
                                let game_dir = Path::new(&library_path)
                                    .join("steamapps")
                                    .join("common")
                                    .join(steam_dir_name);
                                if game_dir.exists() {
                                    return Some(game_dir.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        let game_dir = Path::new(steam_path)
            .join("steamapps")
            .join("common")
            .join(steam_dir_name);
        if game_dir.exists() {
            return Some(game_dir.to_string_lossy().to_string());
        }
    }

    for base in [r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"] {
        let game_dir = Path::new(base)
            .join("steamapps")
            .join("common")
            .join(steam_dir_name);
        if game_dir.exists() {
            return Some(game_dir.to_string_lossy().to_string());
        }
    }

    None
}

pub fn collect_available_games(cfg: &Config) -> Vec<GameProfile> {
    let mut seen = std::collections::HashSet::new();
    let mut available = Vec::new();

    for preset in preset_games() {
        if let Some(configured) = cfg.games.get(&preset.nexus_domain) {
            seen.insert(configured.profile.nexus_domain.clone());
            available.push(configured.profile.clone());
        } else {
            seen.insert(preset.nexus_domain.clone());
            available.push(preset);
        }
    }

    let mut custom_games = cfg
        .games
        .values()
        .filter(|game| !seen.contains(&game.profile.nexus_domain))
        .map(|game| game.profile.clone())
        .collect::<Vec<_>>();
    custom_games.sort_by(|left, right| left.nexus_domain.cmp(&right.nexus_domain));
    available.extend(custom_games);

    available
}

pub fn mods_dir_for(game_path: Option<&str>, profile: Option<&GameProfile>) -> Option<String> {
    let game_path = game_path?;
    let profile = profile?;
    Some(
        Path::new(game_path)
            .join(&profile.mods_subdir)
            .to_string_lossy()
            .to_string(),
    )
}

pub fn build_init_result(cfg: &Config, game_path: Option<String>) -> InitResult {
    let current_game = current_game_config(cfg).map(|game| game.profile.clone());
    let mods_dir = mods_dir_for(game_path.as_deref(), current_game.as_ref());

    InitResult {
        game_path,
        mods_dir,
        current_game,
        available_games: collect_available_games(cfg),
    }
}

pub fn persist_current_game_path(cfg: &mut Config, game_path: Option<String>) {
    let Some(domain) = current_game_domain(cfg).map(str::to_string) else {
        return;
    };
    if let Some(game) = cfg.games.get_mut(&domain) {
        game.game_path = game_path;
    }
}
