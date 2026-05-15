//! src-tauri 薄壳：9 个 `#[tauri::command]` 转发到 `nmm_core::nexus_api`。
//!
//! 业务函数（reqwest 客户端、GraphQL 查询、缓存读写、premium 判定）由 core 提供；
//! 本层仅负责把 `tauri::State<'_, AppState>` 桥接为 `&AppContext`。

use crate::AppState;
use nmm_core::nexus_api as core_api;
use tauri::State;

// Re-export 业务符号，让 src-tauri 内其他模块（nexus_download.rs）可继续通过
// `crate::nexus_api::xxx` 访问。
#[allow(unused_imports)]
pub use core_api::{
    decode_url_segment, ensure_premium_status, get_mod_files_raw, get_premium_download_link,
    NexusFileInfo, NexusModInfo, NexusPagedModsResult, NexusValidateResult,
};

#[tauri::command]
pub async fn nexus_validate_key(key: String) -> Result<NexusValidateResult, String> {
    core_api::validate_key(&key).await
}

#[tauri::command]
pub async fn nexus_get_trending(state: State<'_, AppState>) -> Result<Vec<NexusModInfo>, String> {
    core_api::get_trending(&state.ctx).await
}

#[tauri::command]
pub async fn nexus_get_latest_added(
    state: State<'_, AppState>,
) -> Result<Vec<NexusModInfo>, String> {
    core_api::get_latest_added(&state.ctx).await
}

#[tauri::command]
pub async fn nexus_get_latest_updated(
    state: State<'_, AppState>,
) -> Result<Vec<NexusModInfo>, String> {
    core_api::get_latest_updated(&state.ctx).await
}

#[tauri::command]
pub async fn nexus_get_recently_updated_page(
    period: String,
    page: u64,
    page_size: u64,
    force_refresh: Option<bool>,
    state: State<'_, AppState>,
) -> Result<NexusPagedModsResult, String> {
    core_api::get_recently_updated_page(&state.ctx, &period, page, page_size, force_refresh).await
}

#[tauri::command]
pub async fn nexus_get_popular_page(
    period: String,
    page: u64,
    page_size: u64,
    force_refresh: Option<bool>,
    state: State<'_, AppState>,
) -> Result<NexusPagedModsResult, String> {
    core_api::get_popular_page(&state.ctx, &period, page, page_size, force_refresh).await
}

#[tauri::command]
pub async fn nexus_get_mod(
    mod_id: u64,
    state: State<'_, AppState>,
) -> Result<NexusModInfo, String> {
    core_api::get_mod(&state.ctx, mod_id).await
}

#[tauri::command]
pub async fn nexus_get_mod_files(
    mod_id: u64,
    state: State<'_, AppState>,
) -> Result<Vec<NexusFileInfo>, String> {
    core_api::get_mod_files(&state.ctx, mod_id).await
}

#[tauri::command]
pub async fn nexus_find_mod_by_name(
    name: String,
    state: State<'_, AppState>,
) -> Result<Option<NexusModInfo>, String> {
    core_api::find_mod_by_name(&state.ctx, &name).await
}
