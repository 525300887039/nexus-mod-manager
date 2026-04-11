use crate::{config, db, mods, AppState};
use serde_json::{Map, Value};

fn resolve_translation_game_path(state: &tauri::State<'_, AppState>) -> Option<String> {
    state
        .game_path
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .or_else(config::load_or_detect_game_path)
}

fn collect_translation_cache_map(state: &tauri::State<'_, AppState>) -> Result<Value, String> {
    let game_path = resolve_translation_game_path(state);
    let mut db = state
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))?;
    if let Some(ref game_path) = game_path {
        db::sync_saved_translations_with_game_path_db(&mut db, game_path)?;
    }
    let saved_translations = db::saved_translations_load_db(&db)?;
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

fn persist_translation_cache_map(
    state: &tauri::State<'_, AppState>,
    data: &Value,
) -> Result<(), String> {
    let Some(entries) = data.as_object() else {
        return Err("translations_save 需要对象格式数据".to_string());
    };

    let game_path = resolve_translation_game_path(state);
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

    let db = state
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))?;

    for (mod_id, value) in entries {
        let Some(entry) = value.as_object() else {
            continue;
        };

        let translated_name = entry.get("name").and_then(|value| value.as_str());
        let translated_desc = entry.get("desc").and_then(|value| value.as_str());
        let source_name = mod_lookup.get(mod_id).and_then(|mod_info| mod_info.name.as_deref());
        let source_desc = mod_lookup
            .get(mod_id)
            .and_then(|mod_info| mod_info.description.as_deref());

        db::saved_translation_upsert_db(
            &db,
            mod_id,
            translated_name,
            translated_desc,
            source_name,
            source_desc,
        )?;

        if let (Some(source_text), Some(translated)) = (source_name, translated_name) {
            db::translation_cache_set_db(&db, source_text, translated, "compat")?;
        }
        if let (Some(source_text), Some(translated)) = (source_desc, translated_desc) {
            db::translation_cache_set_db(&db, source_text, translated, "compat")?;
        }
    }

    Ok(())
}

#[tauri::command]
pub fn translations_load(state: tauri::State<'_, AppState>) -> Value {
    match collect_translation_cache_map(&state) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("translations_load failed: {}", err);
            serde_json::json!({})
        }
    }
}

#[tauri::command]
pub fn translations_save(state: tauri::State<'_, AppState>, data: Value) -> Value {
    match persist_translation_cache_map(&state, &data) {
        Ok(()) => serde_json::json!({ "success": true }),
        Err(err) => {
            eprintln!("translations_save failed: {}", err);
            serde_json::json!({ "success": false, "error": err })
        }
    }
}
