use crate::{config, mods, AppState};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::State;

const NEXUS_API_BASE: &str = "https://api.nexusmods.com/v1";
const GAME_DOMAIN: &str = "slaythespire2";
const USER_AGENT: &str = "STS2ModManager/2.0";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(
    default,
    rename_all(serialize = "camelCase", deserialize = "snake_case")
)]
pub struct NexusModInfo {
    pub mod_id: u64,
    pub name: String,
    pub summary: String,
    pub description: Option<String>,
    pub picture_url: Option<String>,
    pub mod_downloads: u64,
    pub mod_unique_downloads: u64,
    pub endorsement_count: u64,
    pub version: String,
    pub author: String,
    pub uploaded_by: String,
    pub category_id: u64,
    pub created_timestamp: u64,
    pub updated_timestamp: u64,
    pub available: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(
    default,
    rename_all(serialize = "camelCase", deserialize = "snake_case")
)]
pub struct NexusFileInfo {
    pub file_id: u64,
    pub name: String,
    pub version: String,
    pub size_in_bytes: Option<u64>,
    pub file_name: String,
    pub uploaded_timestamp: u64,
    pub category_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(
    default,
    rename_all(serialize = "camelCase", deserialize = "snake_case")
)]
pub struct NexusValidateResult {
    pub user_id: u64,
    pub key: String,
    pub name: String,
    pub is_premium: bool,
    pub is_supporter: bool,
    pub email: String,
    pub profile_url: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct NexusFilesResponse {
    files: Vec<NexusFileInfo>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct NexusErrorResponse {
    message: Option<String>,
    error: Option<String>,
}

fn require_api_key(api_key: &str) -> Result<&str, String> {
    let trimmed = api_key.trim();
    if trimmed.is_empty() {
        Err("请先配置 Nexus Mods API Key".to_string())
    } else {
        Ok(trimmed)
    }
}

fn get_saved_api_key() -> Result<String, String> {
    config::load_config()
        .nexus_api_key
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .ok_or_else(|| "请先配置 Nexus Mods API Key".to_string())
}

fn format_nexus_error(status: reqwest::StatusCode, body: &str) -> String {
    let body = body.trim();

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return "Nexus API Key 无效或已失效，请重新验证后保存".to_string();
    }

    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return "Nexus Mods API 请求过于频繁，请稍后再试".to_string();
    }

    if let Ok(parsed) = serde_json::from_str::<NexusErrorResponse>(body) {
        if let Some(message) = parsed.message.or(parsed.error) {
            let trimmed = message.trim();
            if !trimmed.is_empty() {
                return format!("Nexus Mods API 请求失败: {}", trimmed);
            }
        }
    }

    if body.is_empty() {
        format!("Nexus Mods API 请求失败 ({})", status)
    } else {
        format!("Nexus Mods API 请求失败 ({}): {}", status, body)
    }
}

fn normalize_match_text(value: &str) -> String {
    value.trim().to_lowercase()
}

fn cache_nexus_mods(state: &State<'_, AppState>, mods: &[NexusModInfo]) -> Result<(), String> {
    let mut cache = state
        .nexus_mod_cache
        .lock()
        .map_err(|e| format!("Nexus 缓存锁已损坏: {}", e))?;

    for mod_info in mods {
        cache.insert(mod_info.mod_id, mod_info.clone());
    }

    Ok(())
}

fn read_cached_nexus_mods(state: &State<'_, AppState>) -> Result<Vec<NexusModInfo>, String> {
    let cache = state
        .nexus_mod_cache
        .lock()
        .map_err(|e| format!("Nexus 缓存锁已损坏: {}", e))?;

    Ok(cache.values().cloned().collect())
}

fn find_local_mod(name: &str, state: &State<'_, AppState>) -> Option<mods::ModInfo> {
    let game_path = state.game_path.lock().ok()?.clone()?;
    let lookup = normalize_match_text(name);

    mods::scan_mods_internal(&game_path).into_iter().find(|mod_info| {
        mod_info
            .name
            .as_deref()
            .map(normalize_match_text)
            .map(|value| value == lookup)
            .unwrap_or(false)
            || mod_info
                .id
                .as_deref()
                .map(normalize_match_text)
                .map(|value| value == lookup)
                .unwrap_or(false)
    })
}

fn find_matching_cached_mod(
    local_mod: &mods::ModInfo,
    cached_mods: &[NexusModInfo],
) -> Option<NexusModInfo> {
    if let Some(local_name) = local_mod.name.as_deref() {
        let normalized_name = normalize_match_text(local_name);
        if let Some(exact_name_match) = cached_mods
            .iter()
            .find(|mod_info| normalize_match_text(&mod_info.name) == normalized_name)
            .cloned()
        {
            return Some(exact_name_match);
        }
    }

    if let Some(local_id) = local_mod.id.as_deref() {
        let normalized_id = normalize_match_text(local_id);
        if let Some(fuzzy_match) = cached_mods
            .iter()
            .find(|mod_info| normalize_match_text(&mod_info.name).contains(&normalized_id))
            .cloned()
        {
            return Some(fuzzy_match);
        }
    }

    None
}

async fn fetch_mod_by_id(
    mod_id: u64,
    state: &State<'_, AppState>,
) -> Result<NexusModInfo, String> {
    let api_key = get_saved_api_key()?;
    let mod_info: NexusModInfo = nexus_get(
        &format!("/games/{}/mods/{}.json", GAME_DOMAIN, mod_id),
        &api_key,
    )
    .await?;
    cache_nexus_mods(state, &[mod_info.clone()])?;
    Ok(mod_info)
}

async fn nexus_get<T: DeserializeOwned>(endpoint: &str, api_key: &str) -> Result<T, String> {
    let api_key = require_api_key(api_key)?;
    let url = format!("{NEXUS_API_BASE}{endpoint}");
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("apikey", api_key)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("连接 Nexus Mods API 失败: {}", e))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 Nexus Mods API 响应失败: {}", e))?;

    if !status.is_success() {
        return Err(format_nexus_error(status, &body));
    }

    serde_json::from_str::<T>(&body).map_err(|e| format!("解析 Nexus Mods API 响应失败: {}", e))
}

async fn ensure_cached_mod_lists(state: &State<'_, AppState>) -> Result<(), String> {
    let cached_mods = read_cached_nexus_mods(state)?;
    if !cached_mods.is_empty() {
        return Ok(());
    }

    let api_key = get_saved_api_key()?;
    let mut collected = Vec::new();

    if let Ok(mods) = nexus_get::<Vec<NexusModInfo>>(
        &format!("/games/{}/mods/trending.json", GAME_DOMAIN),
        &api_key,
    )
    .await
    {
        collected.extend(mods);
    }

    if let Ok(mods) = nexus_get::<Vec<NexusModInfo>>(
        &format!("/games/{}/mods/latest_added.json", GAME_DOMAIN),
        &api_key,
    )
    .await
    {
        collected.extend(mods);
    }

    if let Ok(mods) = nexus_get::<Vec<NexusModInfo>>(
        &format!("/games/{}/mods/latest_updated.json", GAME_DOMAIN),
        &api_key,
    )
    .await
    {
        collected.extend(mods);
    }

    if collected.is_empty() {
        return Err("无法预热 Nexus 模组缓存".to_string());
    }

    cache_nexus_mods(state, &collected)
}

#[tauri::command]
pub async fn nexus_validate_key(key: String) -> Result<NexusValidateResult, String> {
    let key = key.trim().to_string();
    nexus_get("/users/validate.json", &key).await
}

#[tauri::command]
pub async fn nexus_get_trending(state: State<'_, AppState>) -> Result<Vec<NexusModInfo>, String> {
    let api_key = get_saved_api_key()?;
    let mods: Vec<NexusModInfo> = nexus_get(
        &format!("/games/{}/mods/trending.json", GAME_DOMAIN),
        &api_key,
    )
    .await?;
    cache_nexus_mods(&state, &mods)?;
    Ok(mods)
}

#[tauri::command]
pub async fn nexus_get_latest_added(
    state: State<'_, AppState>,
) -> Result<Vec<NexusModInfo>, String> {
    let api_key = get_saved_api_key()?;
    let mods: Vec<NexusModInfo> = nexus_get(
        &format!("/games/{}/mods/latest_added.json", GAME_DOMAIN),
        &api_key,
    )
    .await?;
    cache_nexus_mods(&state, &mods)?;
    Ok(mods)
}

#[tauri::command]
pub async fn nexus_get_latest_updated(
    state: State<'_, AppState>,
) -> Result<Vec<NexusModInfo>, String> {
    let api_key = get_saved_api_key()?;
    let mods: Vec<NexusModInfo> = nexus_get(
        &format!("/games/{}/mods/latest_updated.json", GAME_DOMAIN),
        &api_key,
    )
    .await?;
    cache_nexus_mods(&state, &mods)?;
    Ok(mods)
}

#[tauri::command]
pub async fn nexus_get_mod(
    mod_id: u64,
    state: State<'_, AppState>,
) -> Result<NexusModInfo, String> {
    fetch_mod_by_id(mod_id, &state).await
}

#[tauri::command]
pub async fn nexus_get_mod_files(mod_id: u64) -> Result<Vec<NexusFileInfo>, String> {
    let api_key = get_saved_api_key()?;
    let response: NexusFilesResponse = nexus_get(
        &format!("/games/{}/mods/{}/files.json", GAME_DOMAIN, mod_id),
        &api_key,
    )
    .await?;
    Ok(response.files)
}

#[tauri::command]
pub async fn nexus_find_mod_by_name(
    name: String,
    state: State<'_, AppState>,
) -> Result<Option<NexusModInfo>, String> {
    let lookup = name.trim();
    if lookup.is_empty() {
        return Ok(None);
    }

    let Some(local_mod) = find_local_mod(lookup, &state) else {
        return Ok(None);
    };

    if let Some(nexus_id) = local_mod.nexus_id {
        let cached_mods = read_cached_nexus_mods(&state)?;
        if let Some(exact_id_match) = cached_mods
            .iter()
            .find(|mod_info| mod_info.mod_id == nexus_id)
            .cloned()
        {
            return Ok(Some(exact_id_match));
        }

        match fetch_mod_by_id(nexus_id, &state).await {
            Ok(mod_info) => return Ok(Some(mod_info)),
            Err(error) => {
                eprintln!(
                    "Failed to resolve nexus_id {} directly from Nexus API: {}",
                    nexus_id, error
                );
            }
        }
    }

    if let Err(error) = ensure_cached_mod_lists(&state).await {
        eprintln!("Failed to prime Nexus cache: {}", error);
    }

    let cached_mods = read_cached_nexus_mods(&state)?;
    Ok(find_matching_cached_mod(&local_mod, &cached_mods).or_else(|| {
        cached_mods
            .into_iter()
            .find(|mod_info| normalize_match_text(&mod_info.name) == normalize_match_text(lookup))
    }))
}
