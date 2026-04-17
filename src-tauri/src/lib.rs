mod app_paths;
mod config;
mod db;
mod game;
mod game_profile;
mod logs;
mod mods;
mod nexus_api;
mod nexus_download;
mod profiles;
mod saves;
mod translate;
mod translate_engine;
mod translate_llm;
mod translations;

use crate::game_profile::GameProfile;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

const LEGACY_DEFAULT_GAME_DOMAIN: &str = "slaythespire2";

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub game_path: Mutex<Option<String>>,
    pub game_state: Mutex<String>, // "idle" | "launching" | "running"
    pub nexus_mod_cache: Mutex<
        std::collections::HashMap<String, std::collections::HashMap<u64, nexus_api::NexusModInfo>>,
    >,
    pub current_profile: Mutex<Option<GameProfile>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyConfig {
    #[serde(rename = "gamePath", alias = "game_path")]
    game_path: Option<String>,
    #[serde(rename = "nexusApiKey", alias = "nexus_api_key")]
    nexus_api_key: Option<String>,
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
}

fn copy_file_if_missing(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.exists() || destination.exists() {
        return Ok(());
    }

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create dir {}: {}", parent.display(), error))?;
    }

    fs::copy(source, destination).map_err(|error| {
        format!(
            "failed to copy {} to {}: {}",
            source.display(),
            destination.display(),
            error
        )
    })?;
    Ok(())
}

fn copy_dir_if_missing(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }

    fs::create_dir_all(destination)
        .map_err(|error| format!("failed to create dir {}: {}", destination.display(), error))?;

    let entries = fs::read_dir(source)
        .map_err(|error| format!("failed to read dir {}: {}", source.display(), error))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read dir entry under {}: {}",
                source.display(),
                error
            )
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| {
            format!(
                "failed to inspect file type for {}: {}",
                source_path.display(),
                error
            )
        })?;

        if file_type.is_dir() {
            copy_dir_if_missing(&source_path, &destination_path)?;
        } else {
            copy_file_if_missing(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

fn copy_db_files_if_missing(source_dir: &Path, destination_dir: &Path) -> Result<(), String> {
    if !source_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(source_dir)
        .map_err(|error| format!("failed to read dir {}: {}", source_dir.display(), error))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "failed to read dir entry under {}: {}",
                source_dir.display(),
                error
            )
        })?;
        let source_path = entry.path();
        if !source_path.is_file() {
            continue;
        }

        let is_db = source_path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.eq_ignore_ascii_case("db"))
            .unwrap_or(false);
        if !is_db {
            continue;
        }

        copy_file_if_missing(&source_path, &destination_dir.join(entry.file_name()))?;
    }

    Ok(())
}

fn slay_the_spire_2_profile() -> Result<GameProfile, String> {
    GameProfile::default_for(LEGACY_DEFAULT_GAME_DOMAIN)
        .ok_or_else(|| "missing slaythespire2 preset".to_string())
}

fn migrate_config_if_needed(legacy_dir: &Path, current_dir: &Path) -> Result<(), String> {
    let legacy_config_path = legacy_dir.join("config.json");
    let current_config_path = current_dir.join("config.json");
    if !legacy_config_path.exists() || current_config_path.exists() {
        return Ok(());
    }

    let legacy_raw = fs::read_to_string(&legacy_config_path).map_err(|error| {
        format!(
            "failed to read legacy config {}: {}",
            legacy_config_path.display(),
            error
        )
    })?;
    let legacy_config: LegacyConfig = serde_json::from_str(&legacy_raw).map_err(|error| {
        format!(
            "failed to parse legacy config {}: {}",
            legacy_config_path.display(),
            error
        )
    })?;

    let mut games = HashMap::new();
    games.insert(
        LEGACY_DEFAULT_GAME_DOMAIN.to_string(),
        config::GameConfig {
            game_path: normalize_optional_string(legacy_config.game_path),
            profile: slay_the_spire_2_profile()?,
        },
    );

    let migrated_config = config::Config {
        current_game: Some(LEGACY_DEFAULT_GAME_DOMAIN.to_string()),
        nexus_api_key: normalize_optional_string(legacy_config.nexus_api_key),
        games,
    };
    let serialized = serde_json::to_string_pretty(&migrated_config)
        .map_err(|error| format!("failed to serialize migrated config: {}", error))?;
    fs::write(&current_config_path, serialized).map_err(|error| {
        format!(
            "failed to write migrated config {}: {}",
            current_config_path.display(),
            error
        )
    })?;
    Ok(())
}

fn migrate_profiles_if_needed(legacy_dir: &Path, current_dir: &Path) -> Result<(), String> {
    copy_file_if_missing(
        &legacy_dir.join("profiles.json"),
        &current_dir.join(format!("profiles_{LEGACY_DEFAULT_GAME_DOMAIN}.json")),
    )
}

fn migrate_legacy_directory(
    legacy_dir: &Path,
    current_dir: &Path,
    include_config_files: bool,
) -> Result<(), String> {
    if !legacy_dir.exists() {
        return Ok(());
    }

    fs::create_dir_all(current_dir)
        .map_err(|error| format!("failed to create dir {}: {}", current_dir.display(), error))?;

    if include_config_files {
        migrate_config_if_needed(legacy_dir, current_dir)?;
        migrate_profiles_if_needed(legacy_dir, current_dir)?;
    }

    copy_db_files_if_missing(legacy_dir, current_dir)?;
    copy_dir_if_missing(
        &legacy_dir.join("save_backups"),
        &current_dir.join("save_backups"),
    )?;
    Ok(())
}

fn push_unique_path(paths: &mut Vec<PathBuf>, candidate: Option<PathBuf>) {
    let Some(candidate) = candidate else {
        return;
    };
    if !paths.iter().any(|existing| existing == &candidate) {
        paths.push(candidate);
    }
}

fn migrate_legacy_app_data() -> Result<(), String> {
    let legacy_config_dir = app_paths::legacy_config_dir();
    let legacy_data_dir = app_paths::legacy_data_dir();

    let has_legacy_data = legacy_config_dir
        .as_ref()
        .map(|path| path.exists())
        .unwrap_or(false)
        || legacy_data_dir
            .as_ref()
            .map(|path| path.exists())
            .unwrap_or(false);
    if !has_legacy_data {
        return Ok(());
    }

    migrate_legacy_directory(
        &app_paths::legacy_config_dir().unwrap_or_default(),
        &app_paths::writable_config_dir(),
        true,
    )?;

    let current_config_dir = app_paths::writable_config_dir();
    let current_data_dir = app_paths::writable_data_dir();
    let mut extra_legacy_roots = Vec::new();
    push_unique_path(
        &mut extra_legacy_roots,
        legacy_data_dir.filter(|path| {
            app_paths::legacy_config_dir()
                .as_ref()
                .map(|config_path| config_path != path)
                .unwrap_or(true)
        }),
    );

    for legacy_root in extra_legacy_roots {
        let destination = if current_data_dir == current_config_dir {
            &current_config_dir
        } else {
            &current_data_dir
        };
        migrate_legacy_directory(&legacy_root, destination, false)?;
    }

    Ok(())
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| -> Result<(), Box<dyn std::error::Error>> {
            if let Err(error) = migrate_legacy_app_data() {
                eprintln!("Legacy app-data migration failed: {}", error);
            }
            let mut db_conn = db::init_db(app.handle())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            let current_profile = config::load_current_profile();
            let current_game_path = config::load_or_detect_game_path();

            if let Err(err) = db::translations_migrate_json_to_db(&mut db_conn) {
                eprintln!("Translation migration failed: {}", err);
            }
            if let Some(game_path) = current_game_path.as_deref() {
                let game_domain = current_profile
                    .as_ref()
                    .map(|profile| profile.nexus_domain.as_str())
                    .unwrap_or_default();
                if let Err(err) = db::sync_saved_translations_with_game_path_db(
                    &mut db_conn,
                    game_domain,
                    game_path,
                ) {
                    eprintln!("Saved translation sync failed: {}", err);
                }
            }

            app.manage(AppState {
                db: Mutex::new(db_conn),
                game_path: Mutex::new(current_game_path),
                game_state: Mutex::new("idle".to_string()),
                nexus_mod_cache: Mutex::new(std::collections::HashMap::new()),
                current_profile: Mutex::new(current_profile),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // App
            config::app_init,
            config::app_select_game_path,
            config::config_list_games,
            config::config_switch_game,
            config::config_add_game,
            config::config_remove_game,
            config::config_save_nexus_key,
            config::config_get_nexus_key,
            // Window
            window_minimize,
            window_maximize,
            window_close,
            // Mods
            mods::mods_scan,
            mods::mods_toggle,
            mods::mods_uninstall,
            mods::mods_install,
            mods::mods_install_drop,
            mods::mods_backup,
            mods::mods_restore,
            // Shell
            shell_open_mods_dir,
            shell_open_game_dir,
            shell_open_logs_dir,
            shell_open_saves_dir,
            shell_open_url,
            // Game
            game::game_launch,
            game::game_get_state,
            game::game_get_version,
            game::game_analyze_crash,
            // Logs
            logs::logs_get_latest,
            logs::logs_read,
            // Profiles
            profiles::profiles_load,
            profiles::profiles_save,
            // Translate
            translate::translate_text,
            translate_engine::translate_smart,
            translate_llm::translate_llm,
            translate_llm::translate_llm_config_save,
            translate_llm::translate_llm_config_load,
            db::translation_cache_get,
            db::translation_cache_set,
            db::translation_cache_batch_get,
            db::translation_cache_count,
            db::translation_cache_clear,
            db::nexus_translations_load,
            db::nexus_translations_save,
            // Translations persistence
            translations::translations_load,
            translations::translations_save,
            // Nexus Mods
            nexus_api::nexus_validate_key,
            nexus_api::nexus_get_trending,
            nexus_api::nexus_get_latest_added,
            nexus_api::nexus_get_latest_updated,
            nexus_api::nexus_get_recently_updated_page,
            nexus_api::nexus_get_popular_page,
            nexus_api::nexus_get_mod,
            nexus_api::nexus_get_mod_files,
            nexus_api::nexus_find_mod_by_name,
            nexus_download::nexus_open_download_page,
            // Saves
            saves::saves_scan,
            saves::saves_export,
            saves::saves_import,
            saves::saves_delete_backup,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = app {
        eprintln!("Tauri error: {}", e);
        let log_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("NexusModManager");
        let _ = std::fs::create_dir_all(&log_dir);
        let log_path = log_dir.join("launch.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            use std::io::Write;
            let _ = writeln!(f, "Tauri error: {}", e);
        }
        std::process::exit(1);
    }
}

// ── Window commands ──

#[tauri::command]
fn window_minimize(window: tauri::Window) {
    let _ = window.minimize();
}

#[tauri::command]
fn window_maximize(window: tauri::Window) {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
}

#[tauri::command]
fn window_close(window: tauri::Window) {
    let _ = window.close();
}

// ── Shell commands ──

fn get_appdata_dir() -> Option<std::path::PathBuf> {
    dirs::config_dir()
}

fn current_game_path(state: &tauri::State<'_, AppState>) -> Option<String> {
    state
        .game_path
        .lock()
        .map(|game_path| game_path.clone())
        .map_err(|e| {
            eprintln!("Game path state lock is poisoned: {}", e);
            e
        })
        .ok()
        .flatten()
}

fn current_profile(state: &tauri::State<'_, AppState>) -> Option<GameProfile> {
    state
        .current_profile
        .lock()
        .map(|profile| profile.clone())
        .map_err(|e| {
            eprintln!("Game profile state lock is poisoned: {}", e);
            e
        })
        .ok()
        .flatten()
}

#[tauri::command]
fn shell_open_mods_dir(state: tauri::State<'_, AppState>) {
    if let (Some(game_path), Some(profile)) = (current_game_path(&state), current_profile(&state)) {
        let mods_dir = Path::new(&game_path).join(&profile.mods_subdir);
        if mods_dir.exists() {
            let _ = opener::open(mods_dir.to_string_lossy().to_string());
        }
    }
}

#[tauri::command]
fn shell_open_game_dir(state: tauri::State<'_, AppState>) {
    if let Some(ref p) = current_game_path(&state) {
        let _ = opener::open(p.clone());
    }
}

#[tauri::command]
fn shell_open_logs_dir(state: tauri::State<'_, AppState>) {
    if let (Some(appdata), Some(profile)) = (get_appdata_dir(), current_profile(&state)) {
        let Some(appdata_dir_name) = profile.appdata_dir_name.as_deref() else {
            return;
        };
        let Some(logs_subdir) = profile.logs_subdir.as_deref() else {
            return;
        };
        let logs_dir = appdata.join(appdata_dir_name).join(logs_subdir);
        if logs_dir.exists() {
            let _ = opener::open(logs_dir.to_string_lossy().to_string());
        }
    }
}

#[tauri::command]
fn shell_open_saves_dir(state: tauri::State<'_, AppState>) {
    if let (Some(appdata), Some(profile)) = (get_appdata_dir(), current_profile(&state)) {
        let Some(appdata_dir_name) = profile.appdata_dir_name.as_deref() else {
            return;
        };
        let saves_dir = appdata.join(appdata_dir_name);
        if saves_dir.exists() {
            let _ = opener::open(saves_dir.to_string_lossy().to_string());
        }
    }
}

#[tauri::command]
fn shell_open_url(url: String) {
    if url.starts_with("https://") || url.starts_with("http://") {
        let _ = opener::open_browser(url);
    }
}
