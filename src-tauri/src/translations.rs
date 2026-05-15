//! src-tauri 薄壳：2 个 `#[tauri::command]` 转发到 `nmm_core::translations`。

use crate::AppState;
use nmm_core::translations as core_translations;
use serde_json::Value;

#[tauri::command]
pub fn translations_load(state: tauri::State<'_, AppState>) -> Value {
    match core_translations::collect_translation_cache_map(&state.ctx) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("translations_load failed: {}", err);
            serde_json::json!({})
        }
    }
}

#[tauri::command]
pub fn translations_save(state: tauri::State<'_, AppState>, data: Value) -> Value {
    match core_translations::persist_translation_cache_map(&state.ctx, &data) {
        Ok(()) => serde_json::json!({ "success": true }),
        Err(err) => {
            eprintln!("translations_save failed: {}", err);
            serde_json::json!({ "success": false, "error": err })
        }
    }
}
