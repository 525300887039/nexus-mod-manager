use serde::Serialize;
use std::fs;
use std::path::Path;

const MAX_LOG_SIZE: u64 = 512 * 1024;
const MAX_LOG_FILES: usize = 50;

#[derive(Serialize)]
pub struct LogsResult {
    pub files: Vec<String>,
    pub content: String,
}

fn get_logs_dir() -> Option<std::path::PathBuf> {
    let appdata = dirs::config_dir()?;
    let dir = appdata.join("SlayTheSpire2").join("logs");
    if dir.exists() {
        Some(dir)
    } else {
        None
    }
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
            if let Some(nl) = text.find('\n') {
                return format!("[... 日志过长，仅显示末尾部分 ...]\n{}", &text[nl + 1..]);
            }
            return text;
        }
    }
    String::new()
}

#[tauri::command]
pub fn logs_get_latest() -> LogsResult {
    let logs_dir = match get_logs_dir() {
        Some(d) => d,
        None => return LogsResult { files: vec![], content: String::new() },
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
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    files.push((name, mtime));
                }
            }
        }
    }
    files.sort_by(|a, b| b.1.cmp(&a.1));
    let file_names: Vec<String> = files.into_iter().take(MAX_LOG_FILES).map(|(n, _)| n).collect();

    let content = if !file_names.is_empty() {
        read_log_safe(&logs_dir.join(&file_names[0]))
    } else {
        String::new()
    };

    LogsResult {
        files: file_names,
        content,
    }
}

#[tauri::command]
pub fn logs_read(file_name: String) -> String {
    let logs_dir = match get_logs_dir() {
        Some(d) => d,
        None => return String::new(),
    };
    read_log_safe(&logs_dir.join(&file_name))
}
