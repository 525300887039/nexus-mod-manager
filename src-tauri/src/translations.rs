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
    let Some(game_path) = resolve_translation_game_path(state) else {
        return Ok(serde_json::json!({}));
    };

    let installed_mods = mods::scan_mods_internal(&game_path);
    if installed_mods.is_empty() {
        return Ok(serde_json::json!({}));
    }

    let mut source_texts = Vec::new();
    for mod_info in &installed_mods {
        if let Some(name) = mod_info.name.as_ref() {
            source_texts.push(name.clone());
        }
        if let Some(description) = mod_info.description.as_ref() {
            source_texts.push(description.clone());
        }
    }

    let cached_translations = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("数据库锁已损坏: {}", e))?;
        db::translation_cache_batch_get_db(&db, source_texts)?
    };

    let mut result = Map::new();

    for mod_info in installed_mods {
        let Some(mod_id) = mod_info.id else {
            continue;
        };

        let mut entry = Map::new();

        if let Some(name) = mod_info.name.as_ref() {
            if let Some(translated) = cached_translations.get(name) {
                entry.insert("name".to_string(), Value::String(translated.clone()));
            }
        }

        if let Some(description) = mod_info.description.as_ref() {
            if let Some(translated) = cached_translations.get(description) {
                entry.insert("desc".to_string(), Value::String(translated.clone()));
            }
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
    let Some(game_path) = resolve_translation_game_path(state) else {
        return Ok(());
    };

    let Some(entries) = data.as_object() else {
        return Err("translations_save 需要对象格式数据".to_string());
    };

    let mod_lookup = mods::scan_mods_internal(&game_path)
        .into_iter()
        .filter_map(|mod_info| {
            let id = mod_info.id.clone()?;
            Some((id, mod_info))
        })
        .collect::<std::collections::HashMap<_, _>>();

    let db = state
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))?;

    for (mod_id, value) in entries {
        let Some(mod_info) = mod_lookup.get(mod_id) else {
            continue;
        };
        let Some(entry) = value.as_object() else {
            continue;
        };

        if let Some(translated_name) = entry.get("name").and_then(|value| value.as_str()) {
            if let Some(source_text) = mod_info.name.as_deref() {
                db::translation_cache_set_db(&db, source_text, translated_name, "compat")?;
            }
        }

        if let Some(translated_desc) = entry.get("desc").and_then(|value| value.as_str()) {
            if let Some(source_text) = mod_info.description.as_deref() {
                db::translation_cache_set_db(&db, source_text, translated_desc, "compat")?;
            }
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
