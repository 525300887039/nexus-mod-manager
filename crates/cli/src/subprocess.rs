//! GUI 子进程下载兜底：CLI 端 `nmm nexus download` 在 Premium 直链不可用 / 失败时，
//! spawn `nexus-mod-manager.exe --headless-download <mod-id>` 作为子进程，捕获其
//! stdout JSON Lines 转发给 [`CliReporter`]，等待子进程退出。
//!
//! 协议见 `src-tauri/src/headless_download.rs::StdoutJsonReporter`：
//! - 进度行：`{"type":"progress","phase":..,"message":..,"fileName":..,"kind"?:..}`
//! - 终结行：`{"type":"result","ok":..,"modId":..,"fileId":..}`

use crate::cli_reporter::CliReporter;
use nmm_core::nexus_download::{
    ErrorKind, NexusDownloadEvent, NexusDownloadReporter,
};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// `run_headless_download` 的最终结果，从子进程的 `type:"result"` 行还原。
#[derive(Debug, Clone)]
pub struct HeadlessResult {
    pub mod_id: u64,
    pub file_id: Option<u64>,
}

/// 定位 GUI binary：
/// 1. CLI 自身 exe 同目录下的 `nexus-mod-manager.exe`（发布场景：NSIS 安装目录）
/// 2. 回退到 cargo target dir（dev 场景：`target/debug/nexus-mod-manager.exe`）
///
/// 都找不到时返回带可操作提示的 Err。
pub fn resolve_gui_binary() -> Result<PathBuf, String> {
    let exe_name = if cfg!(windows) {
        "nexus-mod-manager.exe"
    } else {
        "nexus-mod-manager"
    };

    let cur = std::env::current_exe()
        .map_err(|e| format!("无法定位 CLI binary（current_exe 失败）: {}", e))?;

    let mut tried: Vec<PathBuf> = Vec::new();

    if let Some(parent) = cur.parent() {
        let same_dir = parent.join(exe_name);
        if same_dir.exists() {
            return Ok(same_dir);
        }
        tried.push(same_dir);
    }

    // dev 模式：CLI exe 在 target/{debug,release}/nmm.exe，
    // GUI exe 在同一 target 目录下的 nexus-mod-manager.exe（cargo workspace 共享 target）
    for ancestor in cur.ancestors() {
        if let Some(name) = ancestor.file_name() {
            if name == "debug" || name == "release" {
                let candidate = ancestor.join(exe_name);
                if candidate.exists() {
                    return Ok(candidate);
                }
                tried.push(candidate);
                break;
            }
        }
    }

    let mut msg = format!("找不到 GUI binary `{}`，已尝试以下路径：", exe_name);
    for p in &tried {
        msg.push_str(&format!("\n  - {}", p.display()));
    }
    msg.push_str(
        "\n\n请先构建 GUI（`npm run tauri:build` 生成 NSIS 安装包，或 `cargo build -p nexus-mod-manager` 仅本地调试），并保证 nmm.exe 与 nexus-mod-manager.exe 同目录。",
    );
    Err(msg)
}

/// Spawn GUI 子进程跑 headless 下载，把子进程 stdout 的 JSON Lines 进度转发给 reporter，
/// 等待子进程退出。退出码非 0 时返回 Err（无论 stderr 是否有内容——stderr 已 inherit 直通）。
///
/// `Ctrl-C` 处理：用 `tokio::select!` 同时 await child.wait() 与 signal::ctrl_c()；
/// 收到 Ctrl-C 时显式 kill 子进程后再返回 Err，避免遗留 GUI 进程。
pub async fn run_headless_download(
    reporter: &CliReporter,
    gui_exe: &std::path::Path,
    mod_id: u64,
    file_id: Option<u64>,
    game_domain: &str,
) -> Result<HeadlessResult, String> {
    let mut cmd = Command::new(gui_exe);
    cmd.arg("--headless-download")
        .arg(mod_id.to_string())
        .arg("--game-domain")
        .arg(game_domain);
    if let Some(fid) = file_id {
        cmd.arg("--file-id").arg(fid.to_string());
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("spawn GUI 子进程失败 ({}): {}", gui_exe.display(), e))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "子进程 stdout 未挂上 pipe（内部错误）".to_string())?;
    let mut lines = BufReader::new(stdout).lines();

    let mut final_result: Option<HeadlessResult> = None;

    loop {
        tokio::select! {
            biased;

            sig = tokio::signal::ctrl_c() => {
                if sig.is_ok() {
                    let _ = child.kill().await;
                    return Err("用户按 Ctrl-C 中断；GUI 子进程已被杀".to_string());
                }
            }

            line = lines.next_line() => {
                match line {
                    Ok(Some(text)) => {
                        match parse_event_line(&text) {
                            ParsedLine::Progress(event) => reporter.report(event),
                            ParsedLine::Result(r) => final_result = Some(r),
                            ParsedLine::Unknown => {
                                // 子进程 stdout 偶尔混入非 JSON 行（如 tauri 内部 log）；
                                // 不致命，转 stderr 提示
                                eprintln!("[subprocess] 跳过非 JSON 行: {}", text);
                            }
                            ParsedLine::Malformed(err) => {
                                eprintln!("[subprocess] JSON 解析失败: {} (line: {})", err, text);
                            }
                        }
                    }
                    Ok(None) => break, // stdout 关闭，子进程退出中
                    Err(e) => return Err(format!("读子进程 stdout 失败: {}", e)),
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("等子进程退出失败: {}", e))?;

    if !status.success() {
        let code_desc = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "<no exit code>".to_string());
        return Err(format!(
            "GUI 子进程下载失败，退出码 {}（stderr 见上方 GUI 内部日志）",
            code_desc
        ));
    }

    final_result.ok_or_else(|| {
        "GUI 子进程退出码 0，但未输出 `type:\"result\"` 终结行（内部协议异常）".to_string()
    })
}

/// stdout 一行 JSON 的可能形态。
enum ParsedLine {
    Progress(NexusDownloadEvent),
    Result(HeadlessResult),
    /// 合法 JSON 但 `type` 字段未识别 / 缺失，或不是 object。
    Unknown,
    /// 完全不是 JSON。
    Malformed(serde_json::Error),
}

fn parse_event_line(text: &str) -> ParsedLine {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return ParsedLine::Unknown;
    }

    let value: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(e) => return ParsedLine::Malformed(e),
    };

    let obj = match value.as_object() {
        Some(o) => o,
        None => return ParsedLine::Unknown,
    };

    match obj.get("type").and_then(Value::as_str) {
        Some("progress") => match progress_to_event(obj) {
            Some(event) => ParsedLine::Progress(event),
            None => ParsedLine::Unknown,
        },
        Some("result") => match result_obj(obj) {
            Some(r) => ParsedLine::Result(r),
            None => ParsedLine::Unknown,
        },
        _ => ParsedLine::Unknown,
    }
}

fn progress_to_event(
    obj: &serde_json::Map<String, Value>,
) -> Option<NexusDownloadEvent> {
    let phase = obj.get("phase").and_then(Value::as_str)?;
    let message = obj
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let file_name = obj
        .get("fileName")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    Some(match phase {
        "preparing" => NexusDownloadEvent::Preparing { message },
        "downloading" => NexusDownloadEvent::Downloading {
            file_name: file_name.unwrap_or_default(),
            message,
        },
        "installing" => NexusDownloadEvent::Installing {
            file_name: file_name.unwrap_or_default(),
            message,
        },
        "success" => NexusDownloadEvent::Success {
            file_name: file_name.unwrap_or_default(),
            message,
        },
        "error" => {
            let kind = match obj.get("kind").and_then(Value::as_str) {
                Some("install") => ErrorKind::Install,
                // 缺省 / 未知 / "download" 都按 Download
                _ => ErrorKind::Download,
            };
            NexusDownloadEvent::Error {
                file_name,
                message,
                kind,
            }
        }
        _ => return None,
    })
}

fn result_obj(obj: &serde_json::Map<String, Value>) -> Option<HeadlessResult> {
    let mod_id = obj.get("modId").and_then(Value::as_u64)?;
    let file_id = obj.get("fileId").and_then(Value::as_u64);
    Some(HeadlessResult { mod_id, file_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_progress_downloading() {
        let line = r#"{"type":"progress","phase":"downloading","message":"正在下载 foo.zip","fileName":"foo.zip"}"#;
        match parse_event_line(line) {
            ParsedLine::Progress(NexusDownloadEvent::Downloading { file_name, message }) => {
                assert_eq!(file_name, "foo.zip");
                assert!(message.contains("foo.zip"));
            }
            _ => panic!("expected Downloading event"),
        }
    }

    #[test]
    fn parse_progress_error_with_kind() {
        let line = r#"{"type":"progress","phase":"error","message":"x","fileName":null,"kind":"install"}"#;
        match parse_event_line(line) {
            ParsedLine::Progress(NexusDownloadEvent::Error { kind, .. }) => {
                assert!(matches!(kind, ErrorKind::Install));
            }
            _ => panic!("expected Error/install event"),
        }
    }

    #[test]
    fn parse_progress_error_kind_default_download() {
        let line = r#"{"type":"progress","phase":"error","message":"x","fileName":null}"#;
        match parse_event_line(line) {
            ParsedLine::Progress(NexusDownloadEvent::Error { kind, .. }) => {
                assert!(matches!(kind, ErrorKind::Download));
            }
            _ => panic!("expected Error/download event"),
        }
    }

    #[test]
    fn parse_result_with_file_id() {
        let line = r#"{"type":"result","ok":true,"modId":12345,"fileId":67890}"#;
        match parse_event_line(line) {
            ParsedLine::Result(r) => {
                assert_eq!(r.mod_id, 12345);
                assert_eq!(r.file_id, Some(67890));
            }
            _ => panic!("expected Result"),
        }
    }

    #[test]
    fn parse_result_with_null_file_id() {
        let line = r#"{"type":"result","ok":false,"modId":12345,"fileId":null}"#;
        match parse_event_line(line) {
            ParsedLine::Result(r) => {
                assert_eq!(r.mod_id, 12345);
                assert!(r.file_id.is_none());
            }
            _ => panic!("expected Result"),
        }
    }

    #[test]
    fn parse_malformed_returns_malformed() {
        let line = "not json at all";
        assert!(matches!(parse_event_line(line), ParsedLine::Malformed(_)));
    }

    #[test]
    fn parse_unknown_type_returns_unknown() {
        let line = r#"{"type":"foo","payload":"bar"}"#;
        assert!(matches!(parse_event_line(line), ParsedLine::Unknown));
    }
}
