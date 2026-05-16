//! Nexus Mod 下载业务流（Premium API 直链 + 解压安装），完全独立于 Tauri runtime。
//!
//! 通过 [`NexusDownloadReporter`] trait 把进度 / 失败信号抽象出来，让同一段下载流程：
//! - 在 GUI 端由 `TauriEventReporter` 把事件转成现有的 `nexus-download-state` /
//!   `nexus-install-success` / `nexus-install-error` / `nexus-download-failed` /
//!   `nexus-download-saved` 5 个 Tauri 事件 emit；
//! - 在 CLI 端由后续的 stdout 实现把事件转成进度条或 JSON 行。
//!
//! 设计要点：
//! - core 函数 **不接受** `tauri::AppHandle` / `tauri::State` 等 Tauri runtime 类型；
//!   需要状态时统一收 `&AppContext`，需要事件 sink 时统一收 `&dyn NexusDownloadReporter`。
//! - `install_downloaded_archive` **不**关闭任何窗口；"成功后关下载窗口"是 GUI sink 自己
//!   在收到 [`NexusDownloadEvent::Success`] 时决定的副作用。
//! - dispatch 策略（API 失败回退 webview / headless / spawn GUI）**不**封装在 core，
//!   由调用方实现——core 只提供 [`download_premium_via_api`] 这一个原子操作。

use crate::context::AppContext;
use crate::mods::{is_supported_archive_path, smart_extract_archive};
use crate::nexus_api;
use reqwest::Url;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const API_DOWNLOAD_TIMEOUT_SECS: u64 = 600;
const API_DOWNLOAD_USER_AGENT: &str = "NexusModManager/3.0";

/// 区分下载阶段的失败与安装阶段的失败，决定 GUI 端最终 emit 哪个 Tauri 事件
/// （`nexus-download-failed` vs `nexus-install-error`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    /// HTTP / 网络 / 写盘等"下载尚未落地"的失败
    Download,
    /// 解压 / 创建目录 / 游戏路径缺失等"已落地但安装失败"的失败
    Install,
}

/// 下载流程对外暴露的事件枚举。新增阶段时在此追加 variant，
/// 所有 [`NexusDownloadReporter`] 实现按需扩展 match arm。
#[derive(Debug, Clone)]
pub enum NexusDownloadEvent {
    /// 流程启动 / 中间状态切换。无文件名（如"正在获取下载链接..."）。
    Preparing { message: String },
    /// 已开始下载文件本体。
    Downloading {
        file_name: String,
        message: String,
    },
    /// 文件已落地，正在解压安装到 mods 目录。
    Installing {
        file_name: String,
        message: String,
    },
    /// 安装成功。GUI sink 可在此关闭下载窗口。
    Success {
        file_name: String,
        message: String,
    },
    /// 下载完成但归档格式不支持自动安装（仅保存到临时目录）。
    Saved {
        file_name: String,
        message: String,
    },
    /// 下载或安装失败。`kind` 决定 GUI 端额外 emit 的辅助事件名。
    Error {
        file_name: Option<String>,
        message: String,
        kind: ErrorKind,
    },
}

/// 下载流程的事件 sink。实现端负责把 [`NexusDownloadEvent`] 转换为各自的 UI 信号
/// （GUI 走 Tauri emit_to，CLI 走 stdout / 进度条）。
///
/// `Send + Sync` 约束让 reporter 能跨 await 边界使用，且能被多线程持有
/// （core 当前是单线程逐个 emit，但保留扩展空间）。
pub trait NexusDownloadReporter: Send + Sync {
    fn report(&self, event: NexusDownloadEvent);
}

/// 从 URL 末段提取文件名（URL 解码）。空段回退 `"mod-download"`。
///
/// 用 [`reqwest::Url`]（即 `url::Url`，与 `tauri::webview::Url` 同型）作为入参，
/// 调用方传任何来源（reqwest GET / webview on_download）都能直接复用。
pub fn decode_file_name_from_url(url: &Url) -> String {
    url.path_segments()
        .and_then(|segments| segments.last())
        .filter(|segment| !segment.trim().is_empty())
        .map(nexus_api::decode_url_segment)
        .unwrap_or_else(|| "mod-download".to_string())
}

/// 在目标目录里生成不冲突的文件名：已存在则追加 ` (1)` / ` (2)` ... 序号。
///
/// 无扩展名场景：`foo` → `foo (1)`；有扩展名：`foo.zip` → `foo (1).zip`。
pub fn make_unique_destination(download_dir: &Path, file_name: &str) -> PathBuf {
    let candidate = download_dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let file_path = Path::new(file_name);
    let stem = file_path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "mod".to_string());
    let extension = file_path
        .extension()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut index = 1;
    loop {
        let next_name = if extension.is_empty() {
            format!("{stem} ({index})")
        } else {
            format!("{stem} ({index}).{extension}")
        };
        let next_candidate = download_dir.join(next_name);
        if !next_candidate.exists() {
            return next_candidate;
        }
        index += 1;
    }
}

/// 取 Mod 的首选文件 id（MAIN 分类优先，否则第一个）。
///
/// 用于 Premium 路径未指定文件 id 时补齐。该 helper 不需要 ctx，纯调 Nexus API。
pub async fn resolve_preferred_file_id(game_domain: &str, mod_id: u64) -> Result<u64, String> {
    let files = nexus_api::get_mod_files_raw(game_domain, mod_id).await?;
    files
        .iter()
        .find(|file| file.category_name.eq_ignore_ascii_case("MAIN"))
        .or_else(|| files.first())
        .map(|file| file.file_id)
        .ok_or_else(|| "该 Mod 没有可下载的文件".to_string())
}

/// 把已下载到本地的归档解压安装到当前游戏的 mods 目录，按阶段向 reporter emit 事件。
///
/// **无 GUI 副作用**：本函数不关闭任何窗口、不显示任何弹窗——
/// "成功后关下载窗口"由 GUI sink 在收到 [`NexusDownloadEvent::Success`] 时自行决定。
pub fn install_downloaded_archive(
    ctx: &AppContext,
    reporter: &dyn NexusDownloadReporter,
    archive_path: &Path,
    file_name: &str,
) {
    if !archive_path.exists() {
        let message = format!("下载完成后未找到文件: {}", file_name);
        reporter.report(NexusDownloadEvent::Error {
            file_name: Some(file_name.to_string()),
            message,
            kind: ErrorKind::Install,
        });
        return;
    }

    if !is_supported_archive_path(archive_path) {
        let message = format!(
            "{} 已下载到临时目录，但不是支持自动安装的归档文件",
            file_name
        );
        reporter.report(NexusDownloadEvent::Saved {
            file_name: file_name.to_string(),
            message,
        });
        return;
    }

    reporter.report(NexusDownloadEvent::Installing {
        file_name: file_name.to_string(),
        message: format!("正在安装 {}", file_name),
    });

    let game_path = match ctx.game_path.lock() {
        Ok(guard) => guard.clone(),
        Err(error) => {
            reporter.report(NexusDownloadEvent::Error {
                file_name: Some(file_name.to_string()),
                message: format!("游戏路径状态锁已损坏: {}", error),
                kind: ErrorKind::Install,
            });
            return;
        }
    };
    let Some(game_path) = game_path else {
        reporter.report(NexusDownloadEvent::Error {
            file_name: Some(file_name.to_string()),
            message: "尚未设置游戏目录，无法自动安装".to_string(),
            kind: ErrorKind::Install,
        });
        return;
    };

    let mods_dir = Path::new(&game_path).join("mods");
    if let Err(error) = fs::create_dir_all(&mods_dir) {
        reporter.report(NexusDownloadEvent::Error {
            file_name: Some(file_name.to_string()),
            message: format!("无法创建 Mod 安装目录 {}: {}", mods_dir.display(), error),
            kind: ErrorKind::Install,
        });
        return;
    }

    let archive_path_str = archive_path.to_string_lossy().to_string();
    match smart_extract_archive(&archive_path_str, &mods_dir) {
        Ok(_) => {
            reporter.report(NexusDownloadEvent::Success {
                file_name: file_name.to_string(),
                message: format!("Mod 已安装: {}", file_name),
            });
        }
        Err(error) => {
            reporter.report(NexusDownloadEvent::Error {
                file_name: Some(file_name.to_string()),
                message: error,
                kind: ErrorKind::Install,
            });
        }
    }
}

/// Premium 路径：调官方接口拿直链 → reqwest 下载到临时目录 → 复用安装逻辑。
///
/// **失败语义**：下载阶段任意失败 → 先重置 `ctx.nexus_is_premium = Some(false)` 再返回
/// `Err(_)`；**不** emit `NexusDownloadEvent::Error`——由调用方根据回退策略决定向用户
/// 展示什么（GUI dispatch 会 emit `Preparing { "直链下载失败，切换为浏览器下载方式..." }`
/// 然后开 webview；CLI dispatch 后续会 emit Preparing 跳到下一段或最终 emit Error）。
///
/// 这与既有 GUI 行为一致：原 `download_via_api` 失败时不直接 emit error，让 dispatch
/// 层用"切换提示"覆盖，避免 UI 先红后蓝的闪烁。
///
/// **安装阶段**失败（解压 / 权限 / 游戏路径缺失）由 `install_downloaded_archive` 内部
/// emit `Error { kind: Install }`，与回退策略无关——故 install 失败时本函数仍返回 `Ok(())`。
pub async fn download_premium_via_api(
    ctx: &AppContext,
    reporter: &dyn NexusDownloadReporter,
    game_domain: &str,
    mod_id: u64,
    file_id: u64,
) -> Result<(), String> {
    reporter.report(NexusDownloadEvent::Preparing {
        message: "正在获取下载链接...".to_string(),
    });

    let download_url = nexus_api::get_premium_download_link(game_domain, mod_id, file_id)
        .await
        .map_err(|error| fail_premium(ctx, error))?;

    let parsed_url = download_url
        .parse::<Url>()
        .map_err(|error| fail_premium(ctx, format!("无效的下载地址: {}", error)))?;
    let file_name = decode_file_name_from_url(&parsed_url);

    let download_dir = std::env::temp_dir().join("nexus-mod-downloads");
    fs::create_dir_all(&download_dir)
        .map_err(|error| fail_premium(ctx, format!("无法创建临时下载目录: {}", error)))?;
    let destination = make_unique_destination(&download_dir, &file_name);

    reporter.report(NexusDownloadEvent::Downloading {
        file_name: file_name.clone(),
        message: format!("正在下载 {}", file_name),
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(API_DOWNLOAD_TIMEOUT_SECS))
        .build()
        .map_err(|error| fail_premium(ctx, format!("初始化下载客户端失败: {}", error)))?;

    let response = client
        .get(download_url.as_str())
        .header(reqwest::header::USER_AGENT, API_DOWNLOAD_USER_AGENT)
        .send()
        .await
        .map_err(|error| fail_premium(ctx, format!("连接下载服务器失败: {}", error)))?;
    if !response.status().is_success() {
        return Err(fail_premium(
            ctx,
            format!("下载失败 (HTTP {})", response.status()),
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| fail_premium(ctx, format!("读取下载内容失败: {}", error)))?;

    fs::write(&destination, &bytes)
        .map_err(|error| fail_premium(ctx, format!("写入下载文件失败: {}", error)))?;

    install_downloaded_archive(ctx, reporter, &destination, &file_name);
    Ok(())
}

/// 下载阶段失败的统一处理：重置 premium 缓存 + 透传错误字符串。
/// 不 emit 任何 reporter 事件——见 [`download_premium_via_api`] 的失败语义说明。
fn fail_premium(ctx: &AppContext, message: String) -> String {
    invalidate_premium_cache(ctx);
    message
}

/// 把 `ctx.nexus_is_premium` 缓存重置为 `Some(false)`。
///
/// API 路径任意阶段失败时调用，让下一次 `ensure_premium_status` 重新探测，
/// 而不是命中"上次以为是 premium"的旧缓存。锁失败时静默——重置失败本身
/// 不应该影响上层错误传播。
fn invalidate_premium_cache(ctx: &AppContext) {
    if let Ok(mut guard) = ctx.nexus_is_premium.lock() {
        *guard = Some(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    /// 测试用 reporter：把收到的事件全收集到 Vec，方便断言事件序列。
    struct MockReporter(Mutex<Vec<NexusDownloadEvent>>);

    impl MockReporter {
        fn new() -> Self {
            Self(Mutex::new(Vec::new()))
        }

        fn events(&self) -> Vec<NexusDownloadEvent> {
            self.0.lock().unwrap().clone()
        }
    }

    impl NexusDownloadReporter for MockReporter {
        fn report(&self, event: NexusDownloadEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    /// 为每个测试分配唯一临时目录，drop 时清理。
    /// 不引入 tempfile crate，沿用本 crate 既有的 std::env::temp_dir 测试模式。
    struct TempDirGuard {
        path: PathBuf,
    }
    static TMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let id = TMP_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "nmm-core-test-{}-{}-{}",
                label,
                std::process::id(),
                id
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn make_unique_destination_returns_original_when_free() {
        let dir = TempDirGuard::new("make-unique-free");
        let dest = make_unique_destination(dir.path(), "foo.zip");
        assert_eq!(dest, dir.path().join("foo.zip"));
    }

    #[test]
    fn make_unique_destination_appends_numbered_suffix_with_extension() {
        let dir = TempDirGuard::new("make-unique-ext");
        fs::write(dir.path().join("foo.zip"), b"x").unwrap();
        let dest = make_unique_destination(dir.path(), "foo.zip");
        assert_eq!(dest, dir.path().join("foo (1).zip"));

        fs::write(&dest, b"x").unwrap();
        let dest2 = make_unique_destination(dir.path(), "foo.zip");
        assert_eq!(dest2, dir.path().join("foo (2).zip"));
    }

    #[test]
    fn make_unique_destination_handles_files_without_extension() {
        let dir = TempDirGuard::new("make-unique-noext");
        fs::write(dir.path().join("foo"), b"x").unwrap();
        let dest = make_unique_destination(dir.path(), "foo");
        assert_eq!(dest, dir.path().join("foo (1)"));
    }

    #[test]
    fn decode_file_name_extracts_last_segment_with_url_decoding() {
        let url: Url = "https://example.com/files/My%20Mod%20v1.2.zip"
            .parse()
            .unwrap();
        assert_eq!(decode_file_name_from_url(&url), "My Mod v1.2.zip");
    }

    #[test]
    fn decode_file_name_falls_back_when_last_segment_empty() {
        let url: Url = "https://example.com/files/".parse().unwrap();
        assert_eq!(decode_file_name_from_url(&url), "mod-download");
    }

    #[test]
    fn install_emits_saved_when_archive_format_unsupported() {
        let dir = TempDirGuard::new("install-saved");
        let txt_path = dir.path().join("readme.txt");
        fs::write(&txt_path, b"not an archive").unwrap();

        let ctx = make_ctx_in(dir.path());
        let reporter = MockReporter::new();
        install_downloaded_archive(&ctx, &reporter, &txt_path, "readme.txt");

        let events = reporter.events();
        assert_eq!(events.len(), 1);
        assert!(
            matches!(events[0], NexusDownloadEvent::Saved { ref file_name, .. } if file_name == "readme.txt"),
            "expected single Saved event, got {:?}",
            events
        );
    }

    #[test]
    fn install_emits_install_error_when_file_missing() {
        let dir = TempDirGuard::new("install-missing");
        let missing = dir.path().join("does-not-exist.zip");
        let ctx = make_ctx_in(dir.path());
        let reporter = MockReporter::new();
        install_downloaded_archive(&ctx, &reporter, &missing, "does-not-exist.zip");

        let events = reporter.events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            NexusDownloadEvent::Error {
                kind: ErrorKind::Install,
                ..
            }
        ));
    }

    /// 创建一个临时 db + game_path 设置好的 AppContext，给 install 测试用。
    fn make_ctx_in(game_dir: &Path) -> AppContext {
        let db_path = game_dir.join("test.sqlite");
        let arc = AppContext::init(&db_path).expect("init AppContext");
        *arc.game_path.lock().unwrap() = Some(game_dir.to_string_lossy().to_string());
        Arc::try_unwrap(arc).ok().expect("unique Arc")
    }
}
