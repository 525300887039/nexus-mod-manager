use crate::{mods::smart_extract_zip, AppState};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{
    webview::{DownloadEvent, WebviewWindowBuilder},
    AppHandle, Emitter, Manager, WebviewUrl,
};

const DOWNLOAD_WINDOW_LABEL: &str = "nexus-download";
const MAIN_WINDOW_LABEL: &str = "main";
const DOWNLOAD_STATE_EVENT: &str = "nexus-download-state";
const INSTALL_SUCCESS_EVENT: &str = "nexus-install-success";
const INSTALL_ERROR_EVENT: &str = "nexus-install-error";
const DOWNLOAD_FAILED_EVENT: &str = "nexus-download-failed";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NexusDownloadStatePayload {
    phase: &'static str,
    message: String,
    file_name: Option<String>,
}

fn emit_download_state(
    app: &AppHandle,
    phase: &'static str,
    message: impl Into<String>,
    file_name: Option<String>,
) {
    let payload = NexusDownloadStatePayload {
        phase,
        message: message.into(),
        file_name,
    };
    let _ = app.emit_to(MAIN_WINDOW_LABEL, DOWNLOAD_STATE_EVENT, payload);
}

fn extract_file_name(path: &Path) -> Option<String> {
    path.file_name().map(|name| name.to_string_lossy().to_string())
}

fn decode_file_name_from_url(url: &tauri::webview::Url) -> String {
    url.path_segments()
        .and_then(|segments| segments.last())
        .filter(|segment| !segment.trim().is_empty())
        .map(|segment| {
            urlencoding::decode(segment)
                .map(|decoded| decoded.into_owned())
                .unwrap_or_else(|_| segment.to_string())
        })
        .unwrap_or_else(|| "mod.zip".to_string())
}

fn make_unique_destination(download_dir: &Path, file_name: &str) -> PathBuf {
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

#[tauri::command]
pub async fn nexus_open_download_page(
    app: AppHandle,
    mod_id: u64,
    file_id: Option<u64>,
) -> Result<(), String> {
    let url = if let Some(file_id) = file_id {
        format!(
            "https://www.nexusmods.com/slaythespire2/mods/{mod_id}?tab=files&file_id={file_id}"
        )
    } else {
        format!("https://www.nexusmods.com/slaythespire2/mods/{mod_id}?tab=files")
    };

    if let Some(existing) = app.get_webview_window(DOWNLOAD_WINDOW_LABEL) {
        let _ = existing.close();
    }

    let app_handle = app.clone();
    let external_url = url
        .parse()
        .map_err(|error| format!("无效的 Nexus 下载地址: {}", error))?;

    WebviewWindowBuilder::new(
        &app,
        DOWNLOAD_WINDOW_LABEL,
        WebviewUrl::External(external_url),
    )
    .title("Nexus Mods - 下载")
    .inner_size(1200.0, 800.0)
    .center()
    .on_download(move |_webview, event| {
        match event {
            DownloadEvent::Requested { url, destination } => {
                let download_dir = std::env::temp_dir().join("sts2-mod-downloads");
                if let Err(error) = fs::create_dir_all(&download_dir) {
                    let message = format!("无法创建临时下载目录: {}", error);
                    emit_download_state(&app_handle, "error", &message, None);
                    let _ = app_handle.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, message);
                    return false;
                }

                let file_name = decode_file_name_from_url(&url);
                let destination_path = make_unique_destination(&download_dir, &file_name);
                *destination = destination_path;

                emit_download_state(
                    &app_handle,
                    "downloading",
                    format!("正在下载 {}", file_name),
                    Some(file_name),
                );
            }
            DownloadEvent::Finished { url, path, success } => {
                let file_name = path
                    .as_deref()
                    .and_then(extract_file_name)
                    .unwrap_or_else(|| decode_file_name_from_url(&url));

                if !success {
                    let message = format!("下载失败: {}", file_name);
                    emit_download_state(&app_handle, "error", &message, Some(file_name.clone()));
                    let _ = app_handle.emit_to(MAIN_WINDOW_LABEL, DOWNLOAD_FAILED_EVENT, message);
                    return true;
                }

                let archive_path = match path {
                    Some(path) => path,
                    None => {
                        let message =
                            "下载已完成，但没有返回本地文件路径，无法自动安装".to_string();
                        emit_download_state(
                            &app_handle,
                            "error",
                            &message,
                            Some(file_name.clone()),
                        );
                        let _ =
                            app_handle.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, message);
                        return true;
                    }
                };

                emit_download_state(
                    &app_handle,
                    "installing",
                    format!("正在安装 {}", file_name),
                    Some(file_name.clone()),
                );

                let game_path = app_handle.state::<AppState>().game_path.lock().unwrap().clone();
                let Some(game_path) = game_path else {
                    let message = "尚未设置游戏目录，无法自动安装".to_string();
                    emit_download_state(
                        &app_handle,
                        "error",
                        &message,
                        Some(file_name.clone()),
                    );
                    let _ = app_handle.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, message);
                    return true;
                };

                let mods_dir = Path::new(&game_path).join("mods");
                let archive_path_str = archive_path.to_string_lossy().to_string();
                match smart_extract_zip(&archive_path_str, &mods_dir) {
                    Ok(_) => {
                        emit_download_state(
                            &app_handle,
                            "success",
                            format!("Mod 已安装: {}", file_name),
                            Some(file_name.clone()),
                        );
                        let _ =
                            app_handle.emit_to(MAIN_WINDOW_LABEL, INSTALL_SUCCESS_EVENT, file_name);
                        if let Some(window) = app_handle.get_webview_window(DOWNLOAD_WINDOW_LABEL) {
                            let _ = window.close();
                        }
                    }
                    Err(error) => {
                        emit_download_state(&app_handle, "error", &error, Some(file_name.clone()));
                        let _ = app_handle.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, error);
                    }
                }
            }
            _ => {}
        }

        true
    })
    .build()
    .map_err(|error| format!("创建 Nexus 下载窗口失败: {}", error))?;

    Ok(())
}
