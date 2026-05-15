//! src-tauri 薄壳层：把 db 相关的 7 个 `#[tauri::command]` 转发到 `nmm_core::db`。
//!
//! `translations_migrate_json_to_db` 与 `sync_saved_translations_with_game_path` 已迁回
//! `nmm_core::db`（mods 模块也已搬到 core，不再有跨层桥接需求）；下游模块通过下方
//! `pub use core_db::{...}` re-export 继续访问。

use crate::AppState;
use nmm_core::db as core_db;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;

// Re-export 业务函数与类型，让 src-tauri 其他模块（translations / nexus_api / translate_engine /
// lib.rs::setup）可继续通过 `crate::db::xxx` 访问，无需变更 use 路径。
#[allow(unused_imports)]
pub use core_db::{
    cache_db_path, default_game_domain, legacy_backup_path_for, legacy_translations_path,
    nexus_mod_cache_get_many_db, nexus_mod_cache_load_db, nexus_mod_cache_upsert_db,
    nexus_saved_translation_upsert_db, nexus_saved_translations_load_db,
    saved_translation_upsert_db, saved_translations_load_db, sync_saved_translations_with_game_path,
    translation_cache_batch_get_db, translation_cache_clear_db, translation_cache_count_db,
    translation_cache_get_db, translation_cache_set_db, translations_migrate_json_to_db,
    upsert_translation_row, NexusSavedTranslationRow, SavedTranslationRow,
};

// ───────────────────────── Tauri commands 薄壳 ─────────────────────────

fn lock_db<'a>(
    state: &'a tauri::State<'_, AppState>,
) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))
}

fn current_game_domain(state: &tauri::State<'_, AppState>) -> Result<String, String> {
    state
        .current_profile
        .lock()
        .map_err(|e| format!("game profile lock poisoned: {}", e))?
        .as_ref()
        .map(|profile| profile.nexus_domain.clone())
        .ok_or_else(|| "please select a game first".to_string())
}

#[tauri::command]
pub fn translation_cache_get(
    state: tauri::State<'_, AppState>,
    source_text: String,
) -> Result<Option<String>, String> {
    let db = lock_db(&state)?;
    let game_domain = current_game_domain(&state)?;
    core_db::translation_cache_get_db(&db, &game_domain, &source_text)
}

#[tauri::command]
pub fn translation_cache_set(
    state: tauri::State<'_, AppState>,
    source_text: String,
    translated: String,
    provider: String,
) -> Result<(), String> {
    let db = lock_db(&state)?;
    let game_domain = current_game_domain(&state)?;
    core_db::translation_cache_set_db(&db, &game_domain, &source_text, &translated, &provider)
}

#[tauri::command]
pub fn translation_cache_batch_get(
    state: tauri::State<'_, AppState>,
    texts: Vec<String>,
) -> Result<HashMap<String, String>, String> {
    let db = lock_db(&state)?;
    let game_domain = current_game_domain(&state)?;
    core_db::translation_cache_batch_get_db(&db, &game_domain, texts)
}

#[tauri::command]
pub fn translation_cache_count(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    let db = lock_db(&state)?;
    let game_domain = current_game_domain(&state)?;
    core_db::translation_cache_count_db(&db, &game_domain)
}

#[tauri::command]
pub fn translation_cache_clear(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = lock_db(&state)?;
    let game_domain = current_game_domain(&state)?;
    core_db::translation_cache_clear_db(&db, &game_domain)
}

#[tauri::command]
pub fn nexus_translations_load(state: tauri::State<'_, AppState>) -> Value {
    let game_domain = match current_game_domain(&state) {
        Ok(domain) => domain,
        Err(err) => {
            eprintln!("nexus_translations_load failed: {}", err);
            return serde_json::json!({});
        }
    };
    let db = match lock_db(&state) {
        Ok(db) => db,
        Err(err) => {
            eprintln!("nexus_translations_load failed: {}", err);
            return serde_json::json!({});
        }
    };
    match core_db::collect_nexus_translation_map(&db, &game_domain) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("nexus_translations_load failed: {}", err);
            serde_json::json!({})
        }
    }
}

#[tauri::command]
pub fn nexus_translations_save(state: tauri::State<'_, AppState>, data: Value) -> Value {
    let game_domain = match current_game_domain(&state) {
        Ok(domain) => domain,
        Err(err) => {
            eprintln!("nexus_translations_save failed: {}", err);
            return serde_json::json!({ "success": false, "error": err });
        }
    };
    let db = match lock_db(&state) {
        Ok(db) => db,
        Err(err) => {
            eprintln!("nexus_translations_save failed: {}", err);
            return serde_json::json!({ "success": false, "error": err });
        }
    };
    match core_db::persist_nexus_translation_map(&db, &game_domain, &data) {
        Ok(()) => serde_json::json!({ "success": true }),
        Err(err) => {
            eprintln!("nexus_translations_save failed: {}", err);
            serde_json::json!({ "success": false, "error": err })
        }
    }
}
