//! `CliReporter`：实现 `nmm_core::nexus_download::NexusDownloadReporter`，
//! 把下载流程事件转成 CLI 双模式输出。
//!
//! - **人类模式**（默认）：进度信号走 **stderr**（让 stdout 留给"最终结果"输出 / pipe 友好）
//! - **JSON 模式**（`--json`）：每事件一行 JSON 到 stdout（含 `type: "progress"` 标签），
//!   下载完成后 dispatch 再 println 一行 `type: "result"` 总结
//!
//! 设计：reporter.report 始终同步（无异步通道），保持 core 业务流的简单语义。

use nmm_core::nexus_download::{ErrorKind, NexusDownloadEvent, NexusDownloadReporter};
use serde_json::json;

pub struct CliReporter {
    pub json_mode: bool,
}

impl NexusDownloadReporter for CliReporter {
    fn report(&self, event: NexusDownloadEvent) {
        let (phase, message, file_name) = decompose(&event);

        if self.json_mode {
            let mut payload = json!({
                "type": "progress",
                "phase": phase,
                "message": message,
                "fileName": file_name,
            });
            // Error 事件附带 kind 字段（便于调用方区分 download vs install 失败）
            if let NexusDownloadEvent::Error { kind, .. } = &event {
                payload["kind"] = json!(match kind {
                    ErrorKind::Download => "download",
                    ErrorKind::Install => "install",
                });
            }
            println!("{}", payload);
        } else {
            // 人类模式：stderr 打可读进度；用 [phase] 前缀方便扫读
            eprintln!("[{}] {}", phase, message);
        }
    }
}

/// 把 enum variant 拆出 (phase 名, message, file_name) 三元组。
fn decompose(event: &NexusDownloadEvent) -> (&'static str, String, Option<String>) {
    match event {
        NexusDownloadEvent::Preparing { message } => ("preparing", message.clone(), None),
        NexusDownloadEvent::Downloading { file_name, message } => {
            ("downloading", message.clone(), Some(file_name.clone()))
        }
        NexusDownloadEvent::Installing { file_name, message } => {
            ("installing", message.clone(), Some(file_name.clone()))
        }
        NexusDownloadEvent::Success { file_name, message } => {
            ("success", message.clone(), Some(file_name.clone()))
        }
        NexusDownloadEvent::Saved { file_name, message } => {
            ("success", message.clone(), Some(file_name.clone()))
        }
        NexusDownloadEvent::Error {
            file_name, message, ..
        } => ("error", message.clone(), file_name.clone()),
    }
}
