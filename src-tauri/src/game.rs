use crate::AppState;
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Serialize)]
pub struct LaunchResult {
    pub success: bool,
    pub error: Option<String>,
    pub method: Option<String>,
}

#[derive(Serialize)]
pub struct GameVersion {
    pub version: Option<String>,
    pub engine: Option<String>,
}

#[derive(Serialize)]
pub struct CrashIssue {
    pub reason: String,
    pub detail: String,
    pub mods: Vec<String>,
}

#[derive(Serialize)]
pub struct InvolvedMod {
    pub name: String,
    #[serde(rename = "errorCount")]
    pub error_count: usize,
    pub sample: String,
}

#[derive(Serialize)]
pub struct CrashReport {
    pub issues: Vec<CrashIssue>,
    #[serde(rename = "logFile")]
    pub log_file: Option<String>,
    #[serde(rename = "errorCount")]
    pub error_count: usize,
    #[serde(rename = "warnCount")]
    pub warn_count: usize,
    #[serde(rename = "involvedMods")]
    pub involved_mods: Vec<InvolvedMod>,
    #[serde(rename = "loadedMods")]
    pub loaded_mods: Vec<String>,
    pub notices: Vec<String>,
}

fn get_appdata() -> Option<std::path::PathBuf> {
    dirs::config_dir()
}

#[tauri::command]
pub fn game_launch(state: tauri::State<'_, AppState>) -> LaunchResult {
    {
        let gs = state.game_state.lock().unwrap();
        if *gs != "idle" {
            return LaunchResult {
                success: false,
                error: Some("游戏已在运行".into()),
                method: None,
            };
        }
    }

    let mut launched_directly = false;
    {
        let gp = state.game_path.lock().unwrap();
        if let Some(ref p) = *gp {
            let exe_path = Path::new(p).join("SlayTheSpire2.exe");
            if exe_path.exists() {
                let _ = Command::new(&exe_path).current_dir(p).spawn();
                launched_directly = true;
            }
        }
    }

    if !launched_directly {
        // Fallback to Steam
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("cmd")
                .args(["/C", "start", "steam://rungameid/2868840"])
                .spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = opener::open_browser("steam://rungameid/2868840");
        }
    }

    {
        let mut gs = state.game_state.lock().unwrap();
        *gs = "launching".to_string();
    }

    // Note: process monitoring is handled by frontend polling game_get_state
    LaunchResult {
        success: true,
        error: None,
        method: Some(if launched_directly { "direct" } else { "steam" }.into()),
    }
}

#[tauri::command]
pub fn game_get_state(state: tauri::State<'_, AppState>) -> String {
    let gs = state.game_state.lock().unwrap();

    // Check if game process is running
    let running = is_game_running();
    let current = gs.clone();
    drop(gs);

    match current.as_str() {
        "launching" => {
            if running {
                let mut gs = state.game_state.lock().unwrap();
                *gs = "running".to_string();
                return "running".to_string();
            }
            "launching".to_string()
        }
        "running" => {
            if !running {
                let mut gs = state.game_state.lock().unwrap();
                *gs = "idle".to_string();
                return "idle".to_string();
            }
            "running".to_string()
        }
        _ => {
            if running {
                let mut gs = state.game_state.lock().unwrap();
                *gs = "running".to_string();
                return "running".to_string();
            }
            "idle".to_string()
        }
    }
}

fn is_game_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        sys.processes()
            .values()
            .any(|p| p.name().to_string_lossy().contains("SlayTheSpire2"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[tauri::command]
pub fn game_get_version() -> GameVersion {
    let appdata = match get_appdata() {
        Some(d) => d,
        None => return GameVersion { version: None, engine: None },
    };
    let logs_dir = appdata.join("SlayTheSpire2").join("logs");
    if !logs_dir.exists() {
        return GameVersion { version: None, engine: None };
    }

    // Find rotated logs
    let mut candidates: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&logs_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("godot2") && name.ends_with(".log") {
                if let Ok(meta) = entry.metadata() {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    candidates.push((name, mtime));
                }
            }
        }
    }
    candidates.sort_by(|a, b| b.1.cmp(&a.1));

    let mut file_names: Vec<String> = candidates.into_iter().map(|(n, _)| n).collect();
    file_names.push("godot.log".to_string());

    for fname in &file_names {
        let fp = logs_dir.join(fname);
        if !fp.exists() {
            continue;
        }
        if let Ok(meta) = fp.metadata() {
            let size = meta.len() as usize;
            let read_size = size.min(16384);
            if let Ok(content) = fs::read(&fp) {
                let start = if content.len() > read_size {
                    content.len() - read_size
                } else {
                    0
                };
                let tail = String::from_utf8_lossy(&content[start..]);
                let mut version = None;
                let mut engine = None;
                for line in tail.lines() {
                    if let Some(idx) = line.find("Release Version:") {
                        version = Some(line[idx + 16..].trim().to_string());
                    }
                    if let Some(idx) = line.find("Engine Version:") {
                        engine = Some(line[idx + 15..].trim().to_string());
                    }
                }
                if version.is_some() {
                    return GameVersion { version, engine };
                }
            }
        }
    }

    GameVersion { version: None, engine: None }
}

struct CrashPattern {
    pattern: &'static str,
    reason: &'static str,
    detail: &'static str,
}

const CRASH_PATTERNS: &[CrashPattern] = &[
    CrashPattern { pattern: "State divergence", reason: "联机状态不同步", detail: "你的客户端状态与房主不一致，被服务器踢出。确保双方 MOD 完全相同。" },
    CrashPattern { pattern: "StateDivergence", reason: "联机状态不同步", detail: "你的客户端状态与房主不一致，被服务器踢出。确保双方 MOD 完全相同。" },
    CrashPattern { pattern: "OutOfMemoryException", reason: "内存不足", detail: "游戏耗尽内存。尝试关闭后台程序，或减少加载的 MOD 数量。" },
    CrashPattern { pattern: "out of memory", reason: "内存不足", detail: "游戏耗尽内存。尝试关闭后台程序，或减少加载的 MOD 数量。" },
    CrashPattern { pattern: "StackOverflowException", reason: "堆栈溢出", detail: "可能是某个 MOD 导致无限递归。尝试逐个禁用 MOD 排查。" },
    CrashPattern { pattern: "NullReferenceException", reason: "空引用异常", detail: "MOD 或游戏内部发生空引用异常。" },
    CrashPattern { pattern: "is missing the 'id' field", reason: "MOD 清单格式错误", detail: "部分 MOD 的 manifest 文件缺少 id 字段，游戏无法加载这些 MOD。" },
    CrashPattern { pattern: "Connection timed out", reason: "网络连接超时", detail: "联机服务器连接超时，检查网络状况或更换服务器。" },
    CrashPattern { pattern: "FATAL", reason: "致命错误", detail: "游戏发生未处理的异常导致崩溃。" },
    CrashPattern { pattern: "Unhandled exception", reason: "致命错误", detail: "游戏发生未处理的异常导致崩溃。" },
    CrashPattern { pattern: "Application crashed", reason: "致命错误", detail: "游戏发生未处理的异常导致崩溃。" },
    CrashPattern { pattern: "rendering device lost", reason: "显卡驱动崩溃", detail: "渲染设备丢失，尝试更新显卡驱动或降低画质设置。" },
];

fn read_log_safe(path: &Path) -> String {
    const MAX_SIZE: u64 = 512 * 1024;
    if !path.exists() {
        return String::new();
    }
    if let Ok(meta) = path.metadata() {
        if meta.len() <= MAX_SIZE {
            return fs::read_to_string(path).unwrap_or_default();
        }
        // Read tail
        if let Ok(content) = fs::read(path) {
            let start = if content.len() > MAX_SIZE as usize {
                content.len() - MAX_SIZE as usize
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
pub fn game_analyze_crash() -> CrashReport {
    let empty = CrashReport {
        issues: vec![],
        log_file: None,
        error_count: 0,
        warn_count: 0,
        involved_mods: vec![],
        loaded_mods: vec![],
        notices: vec![],
    };

    let appdata = match get_appdata() {
        Some(d) => d,
        None => return empty,
    };
    let logs_dir = appdata.join("SlayTheSpire2").join("logs");
    if !logs_dir.exists() {
        return empty;
    }

    // Find latest rotated log
    let mut files: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = fs::read_dir(&logs_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("godot2") && name.ends_with(".log") {
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
    if files.is_empty() {
        return empty;
    }

    let latest_file = &files[0].0;
    let file_path = logs_dir.join(latest_file);
    let content = read_log_safe(&file_path);

    // Analyze loaded mods
    let mut loaded_mods: Vec<(String, String)> = Vec::new(); // (name, id)
    let mut failed_manifests: Vec<(String, String)> = Vec::new(); // (dir, file)
    let mut error_mods: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();

    for line in content.lines() {
        // Track loaded mods
        if line.contains("Finished mod initialization for '") {
            if let Some(start) = line.find("for '") {
                let rest = &line[start + 5..];
                if let Some(end) = rest.find("' (") {
                    let name = rest[..end].to_string();
                    let rest2 = &rest[end + 3..];
                    if let Some(end2) = rest2.find(')') {
                        let id = rest2[..end2].to_string();
                        loaded_mods.push((name, id));
                    }
                }
            }
            continue;
        }

        // Track failed manifests
        if line.contains("[ERROR]") && line.contains("Mod manifest") && line.contains("is missing") {
            // Extract dir and file from the path
            if let Some(mods_idx) = line.find("mods") {
                let rest = &line[mods_idx..];
                let parts: Vec<&str> = rest.split(|c| c == '\\' || c == '/').collect();
                if parts.len() >= 3 {
                    failed_manifests.push((parts[1].to_string(), parts[2].trim().to_string()));
                }
            }
            continue;
        }

        // Track errors mentioning mods
        if line.contains("[ERROR]") && !line.contains("Mod manifest") && !line.contains("is missing the") {
            if let Some(mods_idx) = line.find("mods") {
                let rest = &line[mods_idx..];
                let parts: Vec<&str> = rest.split(|c| c == '\\' || c == '/').collect();
                if parts.len() >= 2 {
                    let mod_name = parts[1].trim_end_matches(".json").trim_end_matches(".dll").trim_end_matches(".pck").to_string();
                    let entry = error_mods.entry(mod_name).or_default();
                    let msg = line.replace("[ERROR]", "").trim().chars().take(120).collect::<String>();
                    entry.push(msg);
                }
            }
        }
    }

    // Cross-reference
    let loaded_ids: std::collections::HashSet<String> = loaded_mods.iter().map(|(_, id)| id.clone()).collect();
    let mut really_failed: Vec<String> = Vec::new();
    let mut config_warnings: Vec<String> = Vec::new();

    for (dir, file) in &failed_manifests {
        if loaded_ids.contains(dir) || loaded_mods.iter().any(|(_, id)| id == dir) {
            config_warnings.push(format!("{}/{}: {} 不是 MOD 清单，是配置文件（MOD 已正常加载）", dir, file, file));
        } else {
            really_failed.push(dir.clone());
        }
    }

    // Pattern-based issues
    let mut issues: Vec<CrashIssue> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for cp in CRASH_PATTERNS {
        let content_lower = content.to_lowercase();
        let pattern_lower = cp.pattern.to_lowercase();
        if content_lower.contains(&pattern_lower) && !seen.contains(cp.reason) {
            if cp.reason == "MOD 清单格式错误" && really_failed.is_empty() {
                continue;
            }
            seen.insert(cp.reason.to_string());
            let mut issue = CrashIssue {
                reason: cp.reason.to_string(),
                detail: cp.detail.to_string(),
                mods: vec![],
            };
            if cp.reason == "MOD 清单格式错误" && !really_failed.is_empty() {
                issue.mods = really_failed.clone();
                issue.detail = format!("以下 MOD 的 manifest 文件缺少 id 字段: {}", really_failed.join(", "));
            }
            issues.push(issue);
        }
    }

    // Build involved mods
    let mut involved: Vec<InvolvedMod> = Vec::new();
    for (name, errors) in &error_mods {
        involved.push(InvolvedMod {
            name: name.clone(),
            error_count: errors.len(),
            sample: errors.first().cloned().unwrap_or_default(),
        });
    }
    for m in &really_failed {
        if !involved.iter().any(|i| &i.name == m) {
            involved.push(InvolvedMod {
                name: m.clone(),
                error_count: 1,
                sample: "manifest 格式不正确，MOD 未加载".to_string(),
            });
        }
    }
    involved.sort_by(|a, b| b.error_count.cmp(&a.error_count));

    let error_count = content.lines().filter(|l| l.contains("[ERROR]")).count();
    let warn_count = content.lines().filter(|l| l.contains("[WARN]")).count();

    CrashReport {
        issues,
        log_file: Some(latest_file.clone()),
        error_count,
        warn_count,
        involved_mods: involved,
        loaded_mods: loaded_mods.into_iter().map(|(name, _)| name).collect(),
        notices: config_warnings,
    }
}
