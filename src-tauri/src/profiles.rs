use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn profiles_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("STS2ModManager");
    let _ = fs::create_dir_all(&dir);
    dir.join("profiles.json")
}

#[tauri::command]
pub fn profiles_load() -> Value {
    let p = profiles_path();
    if p.exists() {
        if let Ok(content) = fs::read_to_string(&p) {
            if let Ok(val) = serde_json::from_str(&content) {
                return val;
            }
        }
    }
    serde_json::json!({})
}

#[tauri::command]
pub fn profiles_save(profiles: Value) -> serde_json::Value {
    let p = profiles_path();
    if let Ok(json) = serde_json::to_string_pretty(&profiles) {
        let _ = fs::write(&p, json);
    }
    serde_json::json!({ "success": true })
}
