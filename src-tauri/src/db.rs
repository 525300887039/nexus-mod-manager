//! src-tauri 薄壳层：把 db 相关的 7 个 `#[tauri::command]` 转发到 `nmm_core::db`。
//!
//! 本文件还保留两个边界函数（它们依赖 mods 模块，按 design 决策本 change 暂不搬到 core）：
//! - `translations_migrate_json_to_db`
//! - `sync_saved_translations_with_game_path_db`
//!
//! 待 `extract-local-business-logic` change 把 mods 搬到 core 之后，这两个函数也会迁走。

use crate::{mods, AppState};
use nmm_core::db as core_db;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;

// Re-export 业务函数与类型，让 src-tauri 其他模块（translations / nexus_api / translate_engine 等）
// 可继续通过 `crate::db::xxx_db` 访问，无需变更 use 路径。
#[allow(unused_imports)]
pub use core_db::{
    cache_db_path, default_game_domain, legacy_backup_path_for, legacy_translations_path,
    nexus_mod_cache_get_many_db, nexus_mod_cache_load_db, nexus_mod_cache_upsert_db,
    nexus_saved_translation_upsert_db, nexus_saved_translations_load_db,
    saved_translation_upsert_db, saved_translations_load_db, translation_cache_batch_get_db,
    translation_cache_clear_db, translation_cache_count_db, translation_cache_get_db,
    translation_cache_set_db, upsert_translation_row, NexusSavedTranslationRow,
    SavedTranslationRow,
};

/// 包装 `nmm_core::db::init_db`，保留原签名供 `lib.rs::setup` 直接调用。
///
/// Step 6 lib.rs 重构后改为直接 `nmm_core::AppContext::init(...)`，本 wrapper 可删除。
pub fn init_db(_app_handle: &tauri::AppHandle) -> Result<Connection, String> {
    let db_path = core_db::cache_db_path()?;
    core_db::init_db(&db_path)
}

/// 构建 mod 源数据 lookup（依赖 mods，本 change 暂留 src-tauri 层）。
fn build_mod_source_lookup(game_path: &str) -> core_db::ModSourceLookup {
    mods::scan_mods_internal(game_path)
        .into_iter()
        .filter_map(|mod_info| {
            mod_info
                .id
                .map(|id| (id, (mod_info.name, mod_info.description)))
        })
        .collect()
}

/// 同步已保存翻译与当前 game_path 下的 mod 元数据。
///
/// 调用 core 的 `sync_saved_translations_with_lookup`，自身仅负责 lookup 构建。
pub fn sync_saved_translations_with_game_path_db(
    db: &mut Connection,
    game_domain: &str,
    game_path: &str,
) -> Result<(), String> {
    let lookup = build_mod_source_lookup(game_path);
    core_db::sync_saved_translations_with_lookup(db, game_domain, lookup)
}

/// Legacy `translations.json` → SQLite 迁移。
///
/// 暂留此处（依赖 `build_mod_source_lookup` → mods + `crate::config::*`）。
pub fn translations_migrate_json_to_db(db: &mut Connection) -> Result<(), String> {
    let Some(legacy_path) = core_db::legacy_translations_path() else {
        return Ok(());
    };

    if !legacy_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&legacy_path)
        .map_err(|e| format!("读取旧 translations.json 失败: {}", e))?;
    let parsed: Value = serde_json::from_str(&content)
        .map_err(|e| format!("解析旧 translations.json 失败: {}", e))?;
    let Some(entries) = parsed.as_object() else {
        return Err("旧 translations.json 根节点必须是对象".to_string());
    };

    let mod_lookup = crate::config::load_or_detect_game_path()
        .map(|game_path| build_mod_source_lookup(&game_path))
        .unwrap_or_default();
    let game_domain = crate::config::load_current_profile()
        .map(|profile| profile.nexus_domain)
        .unwrap_or_else(core_db::default_game_domain);

    let tx = db
        .transaction()
        .map_err(|e| format!("开启翻译迁移事务失败: {}", e))?;

    for (key, value) in entries {
        if let Some(translated) = value.as_str() {
            if !key.trim().is_empty() && !translated.trim().is_empty() {
                core_db::upsert_translation_row(&tx, &game_domain, key, translated, "legacy")?;
            }
            continue;
        }

        let Some(legacy_entry) = value.as_object() else {
            continue;
        };
        let name_translated = legacy_entry.get("name").and_then(|value| value.as_str());
        let desc_translated = legacy_entry.get("desc").and_then(|value| value.as_str());
        let (source_name, source_desc) = mod_lookup.get(key).cloned().unwrap_or((None, None));

        core_db::saved_translation_upsert_db(
            &tx,
            &game_domain,
            key,
            name_translated,
            desc_translated,
            source_name.as_deref(),
            source_desc.as_deref(),
        )?;

        if let Some(translated_name) = name_translated {
            if let Some(source_text) = source_name.as_deref() {
                if !source_text.trim().is_empty() && !translated_name.trim().is_empty() {
                    core_db::upsert_translation_row(
                        &tx,
                        &game_domain,
                        source_text,
                        translated_name,
                        "legacy",
                    )?;
                }
            }
        }

        if let Some(translated_desc) = desc_translated {
            if let Some(source_text) = source_desc.as_deref() {
                if !source_text.trim().is_empty() && !translated_desc.trim().is_empty() {
                    core_db::upsert_translation_row(
                        &tx,
                        &game_domain,
                        source_text,
                        translated_desc,
                        "legacy",
                    )?;
                }
            }
        }
    }

    tx.commit()
        .map_err(|e| format!("提交翻译迁移事务失败: {}", e))?;

    let backup_path = core_db::legacy_backup_path_for(&legacy_path);
    fs::rename(&legacy_path, &backup_path).map_err(|e| {
        format!(
            "迁移完成后重命名旧 translations.json 失败 ({} -> {}): {}",
            legacy_path.display(),
            backup_path.display(),
            e
        )
    })?;

    Ok(())
}

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
