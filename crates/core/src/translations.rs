//! 旧版 mod 名/描述翻译的 JSON 兼容层（"saved translations"）。
//!
//! 前端旧 API 期望 `{ mod_id: { name: "...", desc: "..." } }` 形态的对象，
//! 但底层已迁至 SQLite。本模块提供两端转换。
//!
//! 本模块无 Tauri 依赖。持 state 的函数接收 `&AppContext`。

use crate::config;
use crate::context::AppContext;
use crate::db;
use crate::mods;
use serde_json::{Map, Value};

fn resolve_translation_game_path(ctx: &AppContext) -> Option<String> {
    ctx.game_path
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .or_else(config::load_or_detect_game_path)
}

fn current_game_domain(ctx: &AppContext) -> String {
    ctx.current_profile
        .lock()
        .ok()
        .and_then(|profile| profile.as_ref().map(|profile| profile.nexus_domain.clone()))
        .or_else(|| config::load_current_profile().map(|profile| profile.nexus_domain))
        .unwrap_or_default()
}

/// 加载已保存翻译为前端期望的 `{ mod_id: { name, desc } }` 对象形式。
pub fn collect_translation_cache_map(ctx: &AppContext) -> Result<Value, String> {
    let game_domain = current_game_domain(ctx);
    let game_path = resolve_translation_game_path(ctx);
    let mut db_conn = ctx
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {}", e))?;
    if let Some(ref game_path) = game_path {
        db::sync_saved_translations_with_game_path(&mut db_conn, &game_domain, game_path)?;
    }
    let saved_translations = db::saved_translations_load_db(&db_conn, &game_domain)?;
    let mut result = Map::new();

    for (mod_id, saved_row) in saved_translations {
        let mut entry = Map::new();

        if let Some(translated) = saved_row.name_translated {
            entry.insert("name".to_string(), Value::String(translated));
        }

        if let Some(translated) = saved_row.desc_translated {
            entry.insert("desc".to_string(), Value::String(translated));
        }

        if !entry.is_empty() {
            result.insert(mod_id, Value::Object(entry));
        }
    }

    Ok(Value::Object(result))
}

/// 把前端形式的翻译对象写回数据库，并同步当前 mod 元数据作为源文本。
pub fn persist_translation_cache_map(ctx: &AppContext, data: &Value) -> Result<(), String> {
    let Some(entries) = data.as_object() else {
        return Err("translations_save expects an object payload".to_string());
    };

    let game_domain = current_game_domain(ctx);
    let game_path = resolve_translation_game_path(ctx);
    let mod_lookup = game_path
        .as_deref()
        .map(|game_path| {
            mods::scan_mods_internal(game_path)
                .into_iter()
                .filter_map(|mod_info| {
                    let id = mod_info.id.clone()?;
                    Some((id, mod_info))
                })
                .collect::<std::collections::HashMap<_, _>>()
        })
        .unwrap_or_default();

    let db_conn = ctx
        .db
        .lock()
        .map_err(|e| format!("database lock poisoned: {}", e))?;

    for (mod_id, value) in entries {
        let Some(entry) = value.as_object() else {
            continue;
        };

        let translated_name = entry.get("name").and_then(|value| value.as_str());
        let translated_desc = entry.get("desc").and_then(|value| value.as_str());
        let source_name = mod_lookup
            .get(mod_id)
            .and_then(|mod_info| mod_info.name.as_deref());
        let source_desc = mod_lookup
            .get(mod_id)
            .and_then(|mod_info| mod_info.description.as_deref());

        db::saved_translation_upsert_db(
            &db_conn,
            &game_domain,
            mod_id,
            translated_name,
            translated_desc,
            source_name,
            source_desc,
        )?;

        if let (Some(source_text), Some(translated)) = (source_name, translated_name) {
            db::translation_cache_set_db(&db_conn, &game_domain, source_text, translated, "compat")?;
        }
        if let (Some(source_text), Some(translated)) = (source_desc, translated_desc) {
            db::translation_cache_set_db(&db_conn, &game_domain, source_text, translated, "compat")?;
        }
    }

    Ok(())
}
