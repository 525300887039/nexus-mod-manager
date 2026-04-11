use crate::config;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

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

#[tauri::command]
pub async fn nexus_validate_key(key: String) -> Result<NexusValidateResult, String> {
    let key = key.trim().to_string();
    nexus_get("/users/validate.json", &key).await
}

#[tauri::command]
pub async fn nexus_get_trending() -> Result<Vec<NexusModInfo>, String> {
    let api_key = get_saved_api_key()?;
    nexus_get(
        &format!("/games/{}/mods/trending.json", GAME_DOMAIN),
        &api_key,
    )
    .await
}

#[tauri::command]
pub async fn nexus_get_latest_added() -> Result<Vec<NexusModInfo>, String> {
    let api_key = get_saved_api_key()?;
    nexus_get(
        &format!("/games/{}/mods/latest_added.json", GAME_DOMAIN),
        &api_key,
    )
    .await
}

#[tauri::command]
pub async fn nexus_get_latest_updated() -> Result<Vec<NexusModInfo>, String> {
    let api_key = get_saved_api_key()?;
    nexus_get(
        &format!("/games/{}/mods/latest_updated.json", GAME_DOMAIN),
        &api_key,
    )
    .await
}

#[tauri::command]
pub async fn nexus_get_mod(mod_id: u64) -> Result<NexusModInfo, String> {
    let api_key = get_saved_api_key()?;
    nexus_get(
        &format!("/games/{}/mods/{}.json", GAME_DOMAIN, mod_id),
        &api_key,
    )
    .await
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
