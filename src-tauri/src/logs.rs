use crate::{game_profile::GameProfile, AppState};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_LOG_SIZE: u64 = 512 * 1024;
const MAX_LOG_FILES: usize = 50;

#[derive(Serialize)]
pub struct LogsResult {
    pub files: Vec<String>,
    pub content: String,
}

fn current_profile(state: &tauri::State<'_, AppState>) -> Option<GameProfile> {
    state
        .current_profile
        .lock()
        .ok()
        .and_then(|profile| profile.clone())
}

fn get_logs_dir(profile: &GameProfile) -> Option<PathBuf> {
    if !profile.logs_enabled {
        return None;
    }

    let appdata = dirs::config_dir()?;
    let appdata_dir_name = profile.appdata_dir_name.as_deref()?;
    let logs_subdir = profile.logs_subdir.as_deref()?;
    let dir = appdata.join(appdata_dir_name).join(logs_subdir);
    dir.exists().then_some(dir)
}

fn read_log_safe(path: &Path) -> String {
    if !path.exists() {
        return String::new();
    }
    if let Ok(meta) = path.metadata() {
        if meta.len() <= MAX_LOG_SIZE {
            return fs::read_to_string(path).unwrap_or_default();
        }
        if let Ok(content) = fs::read(path) {
            let start = if content.len() > MAX_LOG_SIZE as usize {
                content.len() - MAX_LOG_SIZE as usize
            } else {
                0
            };
            let text = String::from_utf8_lossy(&content[start..]).to_string();
            if let Some(newline) = text.find('\n') {
                return format!(
                    "[... log truncated, showing tail only ...]\n{}",
                    &text[newline + 1..]
                );
            }
            return text;
        }
    }
    String::new()
}

#[tauri::command]
pub fn logs_get_latest(state: tauri::State<'_, AppState>) -> LogsResult {
    let Some(profile) = current_profile(&state) else {
        return LogsResult {
            files: vec![],
            content: String::new(),
        };
    };
    let Some(logs_dir) = get_logs_dir(&profile) else {
        return LogsResult {
            files: vec![],
            content: String::new(),
        };
    };

    let mut files: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&logs_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".log") {
                if let Ok(meta) = entry.metadata() {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|duration| duration.as_millis() as u64)
                        .unwrap_or(0);
                    files.push((name, mtime));
                }
            }
        }
    }
    files.sort_by(|left, right| right.1.cmp(&left.1));
    let file_names: Vec<String> = files
        .into_iter()
        .take(MAX_LOG_FILES)
        .map(|(name, _)| name)
        .collect();

    let content = file_names
        .first()
        .map(|file_name| read_log_safe(&logs_dir.join(file_name)))
        .unwrap_or_default();

    LogsResult {
        files: file_names,
        content,
    }
}

#[tauri::command]
pub fn logs_read(state: tauri::State<'_, AppState>, file_name: String) -> String {
    let Some(profile) = current_profile(&state) else {
        return String::new();
    };
    let Some(logs_dir) = get_logs_dir(&profile) else {
        return String::new();
    };
    read_log_safe(&logs_dir.join(file_name))
}
