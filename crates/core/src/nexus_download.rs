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

use serde::Serialize;

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
