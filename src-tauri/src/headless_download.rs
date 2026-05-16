//! GUI 的 `--headless-download` 模式入口。
//!
//! CLI 端 `nmm nexus download` 在 Premium 直链不可用 / 失败时 spawn
//! `nexus-mod-manager.exe --headless-download <mod-id> [...]` 作为子进程，
//! 子进程在本模块内：
//! - 不创建主窗口、不加载前端
//! - 创建 `nexus-download` webview 跑既有的 INJECTION_SCRIPT + on_download
//! - 把每个 NexusDownloadEvent 序列化成 JSON Lines 写到 stdout
//! - 下载完成（success / error）后 `app.exit(<code>)`
//!
//! Section 3 实现进度：完整流程跑通——argv 解析 + tauri runtime + AppState 初始化 +
//! [`StdoutJsonReporter`] 把下载事件序列化到 stdout JSON Lines + 触发
//! [`crate::nexus_download::open_download_webview`] 复用既有 INJECTION_SCRIPT。

use crate::{config as gui_config, nexus_download, AppState};
use nmm_core::config as core_config;
use nmm_core::nexus_download::{ErrorKind, NexusDownloadEvent, NexusDownloadReporter};
use nmm_core::{db as core_db, AppContext};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

/// `--headless-download` argv 解析结果。
#[derive(Debug, Clone)]
pub struct HeadlessOptions {
    pub mod_id: u64,
    pub file_id: Option<u64>,
    pub game_domain: Option<String>,
}

/// 从 std::env::args() 解析 headless 模式参数。
///
/// 形态：`nexus-mod-manager.exe --headless-download <mod-id> [--file-id <id>] [--game-domain <domain>]`
///
/// 不依赖 clap（增加 ~3s 编译时间收益不值，手动解析 20 行够用）。
fn parse_args(args: &[String]) -> Result<HeadlessOptions, String> {
    // 跳过 argv[0]，定位 --headless-download
    let mut iter = args.iter().skip(1);
    let mut mod_id: Option<u64> = None;
    let mut file_id: Option<u64> = None;
    let mut game_domain: Option<String> = None;
    let mut seen_flag = false;

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--headless-download" => {
                seen_flag = true;
                let Some(value) = iter.next() else {
                    return Err(
                        "--headless-download 需要 mod-id 参数（用法：nexus-mod-manager.exe --headless-download <mod-id> [--file-id <id>] [--game-domain <domain>]）"
                            .to_string(),
                    );
                };
                mod_id = Some(value.parse::<u64>().map_err(|_| {
                    format!("mod-id 参数必须是无符号整数，实际收到 `{}`", value)
                })?);
            }
            "--file-id" => {
                let Some(value) = iter.next() else {
                    return Err("--file-id 需要一个无符号整数参数".to_string());
                };
                file_id = Some(value.parse::<u64>().map_err(|_| {
                    format!("--file-id 必须是无符号整数，实际收到 `{}`", value)
                })?);
            }
            "--game-domain" => {
                let Some(value) = iter.next() else {
                    return Err("--game-domain 需要一个 domain 参数（如 `stalker2`）".to_string());
                };
                game_domain = Some(value.clone());
            }
            _ => {
                // 未知 flag：忽略（容忍未来扩展 / 调试加的 flag）
            }
        }
    }

    if !seen_flag {
        return Err("未识别到 --headless-download flag（内部错误）".to_string());
    }
    let Some(mod_id) = mod_id else {
        return Err("--headless-download 需要 mod-id 参数".to_string());
    };

    Ok(HeadlessOptions {
        mod_id,
        file_id,
        game_domain,
    })
}

/// headless 模式的 `main`。返回值是进程退出码：
/// - 0：下载并安装成功
/// - 1：下载或安装失败 / runtime 错误
/// - 2：argv 解析错误
/// - 99：（仅 Section 2 stub）runtime 启动且 AppState 初始化成功
pub fn main(args: Vec<String>) -> i32 {
    let opts = match parse_args(&args) {
        Ok(opts) => opts,
        Err(error) => {
            eprintln!("[headless-download] {}", error);
            return 2;
        }
    };

    match run_headless(opts) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("[headless-download] {}", error);
            1
        }
    }
}

/// 构造 tauri runtime 并跑下载流程。无主窗口、无 invoke_handler、无 plugin。
fn run_headless(opts: HeadlessOptions) -> Result<i32, String> {
    // generate_context 默认包含 tauri.conf.json 的 `windows: [{ label: "main", ... }]`，
    // headless 模式必须清掉这个配置，否则 runtime 启动时会自动创建主窗口加载前端。
    let mut ctx = tauri::generate_context!();
    ctx.config_mut().app.windows.clear();

    let setup_opts = opts.clone();
    let result = tauri::Builder::default()
        .setup(move |app| -> Result<(), Box<dyn std::error::Error>> {
            init_app_state(app, &setup_opts)?;

            // 构造 reporter 并触发下载。reporter 在收到 Success / Saved / Error 时
            // 写一行 type:"result" 到 stdout 并 app.exit(<code>)，使 tauri runtime 退出。
            let reporter = Arc::new(StdoutJsonReporter {
                app_handle: app.handle().clone(),
                mod_id: setup_opts.mod_id,
                file_id: setup_opts.file_id,
                done: Mutex::new(false),
            });

            let game_domain = current_game_domain(app)?;
            let visible = gui_config::config_get_nexus_download_visible();
            nexus_download::open_download_webview(
                app.handle(),
                &game_domain,
                setup_opts.mod_id,
                setup_opts.file_id,
                visible,
                reporter.clone(),
            )
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

            Ok(())
        })
        .run(ctx);

    match result {
        Ok(()) => Ok(0),
        Err(error) => Err(format!("tauri runtime: {}", error)),
    }
}

/// 从已 manage 的 AppState 里读当前游戏 domain。
fn current_game_domain(app: &mut tauri::App) -> Result<String, Box<dyn std::error::Error>> {
    let state = app.state::<AppState>();
    let domain = state
        .current_profile
        .lock()
        .map_err(|e| format!("current_profile lock poisoned: {}", e))?
        .as_ref()
        .map(|profile| profile.nexus_domain.clone())
        .ok_or_else(|| "current_profile 未设置（init_app_state 异常）".to_string())?;
    Ok(domain)
}

/// 复刻 `lib.rs::run` 的 setup hook：cache_db_path → AppContext::init →
/// 用 `--game-domain` override current_game → 填 ctx.current_profile / game_path →
/// `app.manage(AppState)`。
///
/// 不做：translation migration / save translation sync / legacy app-data migration——
/// 这些 GUI 启动时的副作用对单次下载流程不必要。
fn init_app_state(
    app: &mut tauri::App,
    opts: &HeadlessOptions,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;

    let db_path = core_db::cache_db_path()?;
    let ctx = AppContext::init(&db_path)?;

    let mut cfg = core_config::load_config();
    if let Some(domain) = &opts.game_domain {
        if !cfg.games.contains_key(domain) {
            return Err(format!(
                "--game-domain `{}` 不在 config.json 的 games 列表里；请先 `nmm games add` 或检查 CLI 端 --game flag",
                domain
            )
            .into());
        }
        cfg.current_game = Some(domain.clone());
    }

    let profile = core_config::current_game_config(&cfg)
        .map(|game| game.profile.clone())
        .ok_or_else(|| {
            "config.json 没有 current_game，且 --game-domain 也未指定".to_string()
        })?;
    let game_path = core_config::resolve_game_path_from_config(&cfg);

    *ctx.current_profile
        .lock()
        .map_err(|e| format!("current_profile lock poisoned: {}", e))? = Some(profile);
    *ctx.game_path
        .lock()
        .map_err(|e| format!("game_path lock poisoned: {}", e))? = game_path;

    app.manage(AppState {
        ctx,
        game_state: Mutex::new("idle".to_string()),
    });
    Ok(())
}

/// CLI 子进程协议端：实现 [`NexusDownloadReporter`]，把每个事件序列化为单行 JSON
/// 写到 stdout（JSON Lines），下载结束（Success / Saved / Error）时再写一行
/// `type: "result"` 总结并调 `app_handle.exit(<code>)` 终止 tauri runtime。
///
/// JSON schema 与 `crates/cli/src/cli_reporter.rs::CliReporter` 的 JSON 模式输出对齐：
/// - 进度行：`{"type":"progress","phase":"<name>","message":"...","fileName":"..."|null}`
///   Error 事件额外含 `"kind":"download"|"install"` 字段
/// - 终结行：`{"type":"result","ok":<bool>,"modId":<u64>,"fileId":<u64>|null}`
///
/// `done` 标志避免多次 emit result（防止 install 失败后又收 Saved 之类的边缘情况）。
pub(crate) struct StdoutJsonReporter {
    pub app_handle: AppHandle,
    pub mod_id: u64,
    pub file_id: Option<u64>,
    pub done: Mutex<bool>,
}

impl StdoutJsonReporter {
    fn emit_progress(
        &self,
        phase: &str,
        message: &str,
        file_name: Option<&str>,
        kind: Option<&'static str>,
    ) {
        let mut payload = json!({
            "type": "progress",
            "phase": phase,
            "message": message,
            "fileName": file_name,
        });
        if let Some(k) = kind {
            payload["kind"] = json!(k);
        }
        println!("{}", payload);
    }

    fn finish(&self, ok: bool) {
        // 仅 emit 一次 result，避免重复退出
        let mut done = match self.done.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if *done {
            return;
        }
        *done = true;

        let payload = json!({
            "type": "result",
            "ok": ok,
            "modId": self.mod_id,
            "fileId": self.file_id,
        });
        println!("{}", payload);

        self.app_handle.exit(if ok { 0 } else { 1 });
    }
}

impl NexusDownloadReporter for StdoutJsonReporter {
    fn report(&self, event: NexusDownloadEvent) {
        match event {
            NexusDownloadEvent::Preparing { message } => {
                self.emit_progress("preparing", &message, None, None);
            }
            NexusDownloadEvent::Downloading { file_name, message } => {
                self.emit_progress("downloading", &message, Some(&file_name), None);
            }
            NexusDownloadEvent::Installing { file_name, message } => {
                self.emit_progress("installing", &message, Some(&file_name), None);
            }
            NexusDownloadEvent::Success { file_name, message } => {
                self.emit_progress("success", &message, Some(&file_name), None);
                self.finish(true);
            }
            NexusDownloadEvent::Saved { file_name, message } => {
                self.emit_progress("success", &message, Some(&file_name), None);
                self.finish(true);
            }
            NexusDownloadEvent::Error {
                file_name,
                message,
                kind,
            } => {
                let kind_str = match kind {
                    ErrorKind::Download => "download",
                    ErrorKind::Install => "install",
                };
                self.emit_progress(
                    "error",
                    &message,
                    file_name.as_deref(),
                    Some(kind_str),
                );
                self.finish(false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn av(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_minimal() {
        let opts =
            parse_args(&av(&["nexus-mod-manager.exe", "--headless-download", "12345"])).unwrap();
        assert_eq!(opts.mod_id, 12345);
        assert!(opts.file_id.is_none());
        assert!(opts.game_domain.is_none());
    }

    #[test]
    fn parse_all_flags() {
        let opts = parse_args(&av(&[
            "nexus-mod-manager.exe",
            "--headless-download",
            "12345",
            "--file-id",
            "67890",
            "--game-domain",
            "stalker2",
        ]))
        .unwrap();
        assert_eq!(opts.mod_id, 12345);
        assert_eq!(opts.file_id, Some(67890));
        assert_eq!(opts.game_domain.as_deref(), Some("stalker2"));
    }

    #[test]
    fn parse_missing_mod_id() {
        let err = parse_args(&av(&["nexus-mod-manager.exe", "--headless-download"])).unwrap_err();
        assert!(err.contains("mod-id"));
    }

    #[test]
    fn parse_non_numeric_mod_id() {
        let err = parse_args(&av(&[
            "nexus-mod-manager.exe",
            "--headless-download",
            "abc",
        ]))
        .unwrap_err();
        assert!(err.contains("无符号整数"));
    }

    #[test]
    fn parse_ignores_unknown_flag() {
        let opts = parse_args(&av(&[
            "nexus-mod-manager.exe",
            "--headless-download",
            "12345",
            "--some-future-flag",
            "value",
        ]))
        .unwrap();
        assert_eq!(opts.mod_id, 12345);
    }
}
