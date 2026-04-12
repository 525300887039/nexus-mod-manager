use crate::{config, db, mods, AppState};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::State;

const NEXUS_API_BASE: &str = "https://api.nexusmods.com/v1";
const NEXUS_GRAPHQL_URL: &str = "https://api-router.nexusmods.com/graphql";
const GAME_DOMAIN: &str = "slaythespire2";
const USER_AGENT: &str = "STS2ModManager/2.0";
const NEXUS_API_TIMEOUT_SECS: u64 = 20;
const DEFAULT_RECENTLY_UPDATED_PERIOD: &str = "1m";
const DEFAULT_POPULAR_PERIOD: &str = "1w";
const MAX_RECENTLY_UPDATED_PAGE_SIZE: u64 = 50;
const SECONDS_PER_DAY: u64 = 86_400;
const PAGED_MOD_CACHE_MAX_AGE_MILLIS: i64 = 60 * 60 * 1000;
const POPULAR_MODS_LISTING_QUERY: &str = r#"
query ModsListing($count: Int = 0, $facets: ModsFacet, $filter: ModsFilter, $offset: Int, $postFilter: ModsFilter, $sort: [ModsSort!]) {
  mods(
    count: $count
    facets: $facets
    filter: $filter
    offset: $offset
    postFilter: $postFilter
    sort: $sort
    viewUserBlockedContent: false
  ) {
    totalCount
    nodes {
      modId
      name
      summary
      downloads
      endorsements
      thumbnailUrl
      status
      modCategory {
        categoryId
      }
      uploader {
        name
      }
    }
  }
}
"#;

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(
    default,
    rename_all(serialize = "camelCase", deserialize = "snake_case")
)]
struct NexusUpdatedEntry {
    pub mod_id: u64,
    pub latest_file_update: u64,
    pub latest_mod_activity: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(
    default,
    rename_all(serialize = "camelCase", deserialize = "snake_case")
)]
pub struct NexusPagedModsResult {
    pub items: Vec<NexusModInfo>,
    pub page: u64,
    pub page_size: u64,
    pub total_items: u64,
    pub total_pages: u64,
    pub has_prev: bool,
    pub has_next: bool,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct NexusPopularListingResponse {
    #[serde(default)]
    data: Option<NexusPopularListingData>,
    #[serde(default)]
    errors: Vec<NexusGraphqlError>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct NexusGraphqlError {
    message: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct NexusPopularListingData {
    mods: NexusPopularListingConnection,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct NexusPopularListingConnection {
    total_count: u64,
    nodes: Vec<NexusPopularModNode>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct NexusPopularModNode {
    mod_id: u64,
    name: String,
    summary: String,
    downloads: u64,
    endorsements: u64,
    thumbnail_url: Option<String>,
    status: String,
    mod_category: NexusPopularModCategory,
    uploader: NexusPopularUploader,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct NexusPopularModCategory {
    category_id: u64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct NexusPopularUploader {
    name: String,
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

impl From<NexusPopularModNode> for NexusModInfo {
    fn from(value: NexusPopularModNode) -> Self {
        let uploader_name = value.uploader.name;

        Self {
            mod_id: value.mod_id,
            name: value.name,
            summary: value.summary,
            description: None,
            picture_url: value.thumbnail_url,
            mod_downloads: value.downloads,
            mod_unique_downloads: 0,
            endorsement_count: value.endorsements,
            version: String::new(),
            author: uploader_name.clone(),
            uploaded_by: uploader_name,
            category_id: value.mod_category.category_id,
            created_timestamp: 0,
            updated_timestamp: 0,
            available: value.status == "published",
            status: value.status,
        }
    }
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

fn format_nexus_graphql_error(status: reqwest::StatusCode, body: &str) -> String {
    let body = body.trim();

    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return "Nexus 网页热门列表请求过于频繁，请稍后再试".to_string();
    }

    if body.is_empty() {
        format!("Nexus 网页热门列表请求失败 ({})", status)
    } else {
        format!("Nexus 网页热门列表请求失败 ({}): {}", status, body)
    }
}

fn normalize_match_text(value: &str) -> String {
    value.trim().to_lowercase()
}

fn normalize_recently_updated_period(period: &str) -> Result<&str, String> {
    let trimmed = period.trim();
    let effective = if trimmed.is_empty() {
        DEFAULT_RECENTLY_UPDATED_PERIOD
    } else {
        trimmed
    };

    match effective {
        "1d" | "1w" | "1m" => Ok(effective),
        _ => Err("无效的分页浏览时间范围，仅支持 1d / 1w / 1m".to_string()),
    }
}

fn normalize_popular_period(period: &str) -> Result<&str, String> {
    let trimmed = period.trim();
    let effective = if trimmed.is_empty() {
        DEFAULT_POPULAR_PERIOD
    } else {
        trimmed
    };

    normalize_recently_updated_period(effective)
}

fn normalize_page(page: u64) -> u64 {
    page.max(1)
}

fn normalize_page_size(page_size: u64) -> u64 {
    page_size.max(1).min(MAX_RECENTLY_UPDATED_PAGE_SIZE)
}

fn period_to_days(period: &str) -> u64 {
    match period {
        "1d" => 1,
        "1w" => 7,
        "1m" => 30,
        _ => 30,
    }
}

fn page_to_offset(page: u64, page_size: u64) -> u64 {
    normalize_page(page)
        .saturating_sub(1)
        .saturating_mul(page_size)
}

fn updated_after_filter_value(period: &str) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("系统时间异常，无法计算热门列表时间范围: {}", e))?
        .as_secs();
    let age_seconds = period_to_days(period).saturating_mul(SECONDS_PER_DAY);
    Ok(now.saturating_sub(age_seconds).to_string())
}

fn cache_nexus_mods_in_memory(
    state: &State<'_, AppState>,
    mods: &[NexusModInfo],
) -> Result<(), String> {
    let mut cache = state
        .nexus_mod_cache
        .lock()
        .map_err(|e| format!("Nexus 缓存锁已损坏: {}", e))?;

    for mod_info in mods {
        cache.insert(mod_info.mod_id, mod_info.clone());
    }

    Ok(())
}

fn cache_nexus_mods(state: &State<'_, AppState>, mods: &[NexusModInfo]) -> Result<(), String> {
    cache_nexus_mods_in_memory(state, mods)?;

    let db = state
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))?;
    db::nexus_mod_cache_upsert_db(&db, mods)?;

    Ok(())
}

fn read_cached_nexus_mods(state: &State<'_, AppState>) -> Result<Vec<NexusModInfo>, String> {
    {
        let cache = state
            .nexus_mod_cache
            .lock()
            .map_err(|e| format!("Nexus 缓存锁已损坏: {}", e))?;
        if !cache.is_empty() {
            return Ok(cache.values().cloned().collect());
        }
    }

    let cached_mods = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("数据库锁已损坏: {}", e))?;
        db::nexus_mod_cache_load_db(&db)?
    };

    if !cached_mods.is_empty() {
        cache_nexus_mods_in_memory(state, &cached_mods)?;
    }

    Ok(cached_mods)
}

fn read_cached_nexus_mods_by_ids(
    state: &State<'_, AppState>,
    mod_ids: &[u64],
    max_age_millis: Option<i64>,
) -> Result<HashMap<u64, NexusModInfo>, String> {
    if mod_ids.is_empty() {
        return Ok(HashMap::new());
    }

    if let Some(max_age_millis) = max_age_millis {
        let db_cached = {
            let db = state
                .db
                .lock()
                .map_err(|e| format!("数据库锁已损坏: {}", e))?;
            db::nexus_mod_cache_get_many_db(&db, mod_ids, Some(max_age_millis))?
        };

        if !db_cached.is_empty() {
            let mods_to_hydrate = db_cached.values().cloned().collect::<Vec<_>>();
            cache_nexus_mods_in_memory(state, &mods_to_hydrate)?;
        }

        return Ok(db_cached);
    }

    let mut cached = HashMap::new();
    let mut missing_ids = Vec::new();

    {
        let memory_cache = state
            .nexus_mod_cache
            .lock()
            .map_err(|e| format!("Nexus 缓存锁已损坏: {}", e))?;

        for mod_id in mod_ids {
            if let Some(mod_info) = memory_cache.get(mod_id) {
                cached.insert(*mod_id, mod_info.clone());
            } else {
                missing_ids.push(*mod_id);
            }
        }
    }

    if missing_ids.is_empty() {
        return Ok(cached);
    }

    let db_cached = {
        let db = state
            .db
            .lock()
            .map_err(|e| format!("数据库锁已损坏: {}", e))?;
        db::nexus_mod_cache_get_many_db(&db, &missing_ids, None)?
    };

    if !db_cached.is_empty() {
        let mods_to_hydrate = db_cached.values().cloned().collect::<Vec<_>>();
        cache_nexus_mods_in_memory(state, &mods_to_hydrate)?;
        cached.extend(db_cached);
    }

    Ok(cached)
}

fn find_local_mod(name: &str, state: &State<'_, AppState>) -> Option<mods::ModInfo> {
    let game_path = state.game_path.lock().ok()?.clone()?;
    let lookup = normalize_match_text(name);

    mods::scan_mods_internal(&game_path)
        .into_iter()
        .find(|mod_info| {
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

async fn fetch_mod_by_id(mod_id: u64, state: &State<'_, AppState>) -> Result<NexusModInfo, String> {
    let api_key = get_saved_api_key()?;
    fetch_mod_by_id_with_key(mod_id, state, &api_key).await
}

async fn fetch_mod_by_id_with_key(
    mod_id: u64,
    state: &State<'_, AppState>,
    api_key: &str,
) -> Result<NexusModInfo, String> {
    let mod_info: NexusModInfo = nexus_get(
        &format!("/games/{}/mods/{}.json", GAME_DOMAIN, mod_id),
        api_key,
    )
    .await?;
    cache_nexus_mods(state, &[mod_info.clone()])?;
    Ok(mod_info)
}

async fn nexus_get<T: DeserializeOwned>(endpoint: &str, api_key: &str) -> Result<T, String> {
    let api_key = require_api_key(api_key)?;
    let url = format!("{NEXUS_API_BASE}{endpoint}");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(NEXUS_API_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("初始化 Nexus Mods HTTP 客户端失败: {}", e))?;
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

    if body.trim().is_empty() {
        return Err("Nexus Mods API 返回空响应".to_string());
    }

    serde_json::from_str::<T>(&body).map_err(|e| format!("解析 Nexus Mods API 响应失败: {}", e))
}

async fn nexus_graphql_post(payload: serde_json::Value) -> Result<NexusPopularListingData, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(NEXUS_API_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("初始化 Nexus 网页列表 HTTP 客户端失败: {}", e))?;
    let response = client
        .post(NEXUS_GRAPHQL_URL)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("连接 Nexus 网页热门列表失败: {}", e))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 Nexus 网页热门列表响应失败: {}", e))?;

    if !status.is_success() {
        return Err(format_nexus_graphql_error(status, &body));
    }

    let parsed = serde_json::from_str::<NexusPopularListingResponse>(&body)
        .map_err(|e| format!("解析 Nexus 网页热门列表响应失败: {}", e))?;

    if !parsed.errors.is_empty() {
        let message = parsed
            .errors
            .into_iter()
            .map(|error| error.message.trim().to_string())
            .filter(|message| !message.is_empty())
            .collect::<Vec<_>>()
            .join("；");
        if message.is_empty() {
            return Err("Nexus 网页热门列表返回 GraphQL 错误".to_string());
        }
        return Err(format!("Nexus 网页热门列表返回 GraphQL 错误: {}", message));
    }

    parsed
        .data
        .ok_or_else(|| "Nexus 网页热门列表返回空数据".to_string())
}

async fn fetch_popular_listing_connection(
    period: &str,
    page: u64,
    page_size: u64,
) -> Result<NexusPopularListingConnection, String> {
    let payload = build_popular_listing_payload(period, page, page_size)?;
    Ok(nexus_graphql_post(payload).await?.mods)
}

fn build_popular_listing_payload(
    period: &str,
    page: u64,
    page_size: u64,
) -> Result<serde_json::Value, String> {
    let updated_after = updated_after_filter_value(period)?;
    Ok(json!({
        "query": POPULAR_MODS_LISTING_QUERY,
        "variables": {
            "count": page_size,
            "facets": {
                "categoryName": [],
                "languageName": [],
                "tag": [],
            },
            "filter": {
                "adultContent": [
                    {
                        "op": "EQUALS",
                        "value": false,
                    }
                ],
                "filter": [
                    {
                        "op": "AND",
                        "updatedAt": [
                            {
                                "op": "GTE",
                                "value": updated_after,
                            }
                        ],
                    }
                ],
                "gameDomainName": [
                    {
                        "op": "EQUALS",
                        "value": GAME_DOMAIN,
                    }
                ],
                "name": [],
            },
            "offset": page_to_offset(page, page_size),
            "postFilter": {},
            "sort": {
                "endorsements": {
                    "direction": "DESC",
                }
            },
        },
        "operationName": "ModsListing",
    }))
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

async fn fetch_recently_updated_entries(
    period: &str,
    api_key: &str,
) -> Result<Vec<NexusUpdatedEntry>, String> {
    nexus_get(
        &format!("/games/{}/mods/updated.json?period={}", GAME_DOMAIN, period),
        api_key,
    )
    .await
}

async fn resolve_recently_updated_page_items(
    state: &State<'_, AppState>,
    api_key: &str,
    page_mod_ids: &[u64],
    force_refresh: bool,
) -> Result<(Vec<NexusModInfo>, Option<String>), String> {
    let fresh_cached_mods = if force_refresh {
        HashMap::new()
    } else {
        read_cached_nexus_mods_by_ids(state, page_mod_ids, Some(PAGED_MOD_CACHE_MAX_AGE_MILLIS))?
    };
    let fallback_cached_mods = if force_refresh {
        HashMap::new()
    } else {
        read_cached_nexus_mods_by_ids(state, page_mod_ids, None)?
    };
    let mut resolved = Vec::with_capacity(page_mod_ids.len());
    let mut fallback_count = 0_u64;
    let mut skipped_count = 0_u64;

    for mod_id in page_mod_ids {
        if !force_refresh {
            if let Some(cached) = fresh_cached_mods.get(mod_id) {
                resolved.push(cached.clone());
                continue;
            }
        }

        match fetch_mod_by_id_with_key(*mod_id, state, api_key).await {
            Ok(mod_info) => resolved.push(mod_info),
            Err(error) => {
                if let Some(cached) = fresh_cached_mods
                    .get(mod_id)
                    .or_else(|| fallback_cached_mods.get(mod_id))
                {
                    eprintln!(
                        "Failed to refresh Nexus mod {} from API, falling back to cache: {}",
                        mod_id, error
                    );
                    resolved.push(cached.clone());
                    fallback_count += 1;
                } else {
                    eprintln!("Failed to resolve Nexus mod {} from API: {}", mod_id, error);
                    skipped_count += 1;
                }
            }
        }
    }

    let warning = match (fallback_count, skipped_count) {
        (0, 0) => None,
        (fallback, 0) => Some(format!(
            "{} 个 Mod 详情刷新失败，已回退到本地缓存。",
            fallback
        )),
        (0, skipped) => Some(format!("{} 个 Mod 详情加载失败，已跳过。", skipped)),
        (fallback, skipped) => Some(format!(
            "{} 个 Mod 已回退缓存，{} 个 Mod 加载失败并被跳过。",
            fallback, skipped
        )),
    };

    Ok((resolved, warning))
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
pub async fn nexus_get_recently_updated_page(
    period: String,
    page: u64,
    page_size: u64,
    force_refresh: Option<bool>,
    state: State<'_, AppState>,
) -> Result<NexusPagedModsResult, String> {
    let api_key = get_saved_api_key()?;
    let period = normalize_recently_updated_period(&period)?.to_string();
    let requested_page_size = normalize_page_size(page_size);
    let entries = fetch_recently_updated_entries(&period, &api_key).await?;

    let mut unique_ids = Vec::with_capacity(entries.len());
    let mut seen_ids = HashSet::new();
    for entry in entries {
        if seen_ids.insert(entry.mod_id) {
            unique_ids.push(entry.mod_id);
        }
    }

    let total_items = unique_ids.len() as u64;
    let total_pages = if total_items == 0 {
        0
    } else {
        total_items.div_ceil(requested_page_size)
    };
    let page = if total_pages == 0 {
        1
    } else {
        normalize_page(page).min(total_pages)
    };

    let start = ((page - 1) * requested_page_size) as usize;
    let end = (start + requested_page_size as usize).min(unique_ids.len());
    let page_mod_ids = if start < end {
        unique_ids[start..end].to_vec()
    } else {
        Vec::new()
    };

    let (items, warning) = resolve_recently_updated_page_items(
        &state,
        &api_key,
        &page_mod_ids,
        force_refresh.unwrap_or(false),
    )
    .await?;

    Ok(NexusPagedModsResult {
        items,
        page,
        page_size: requested_page_size,
        total_items,
        total_pages,
        has_prev: total_pages > 0 && page > 1,
        has_next: total_pages > 0 && page < total_pages,
        warning,
    })
}

#[tauri::command]
pub async fn nexus_get_popular_page(
    period: String,
    page: u64,
    page_size: u64,
    _force_refresh: Option<bool>,
) -> Result<NexusPagedModsResult, String> {
    let period = normalize_popular_period(&period)?.to_string();
    let requested_page = normalize_page(page);
    let requested_page_size = normalize_page_size(page_size);

    let mut listing =
        fetch_popular_listing_connection(&period, requested_page, requested_page_size).await?;
    let total_items = listing.total_count;
    let total_pages = if total_items == 0 {
        0
    } else {
        total_items.div_ceil(requested_page_size)
    };
    let page = if total_pages == 0 {
        1
    } else {
        requested_page.min(total_pages)
    };

    if total_pages > 0 && page != requested_page {
        listing = fetch_popular_listing_connection(&period, page, requested_page_size).await?;
    }

    // Keep list results out of the full mod-detail cache because the GraphQL listing
    // omits fields such as version and detailed descriptions.
    let items = listing
        .nodes
        .into_iter()
        .map(NexusModInfo::from)
        .collect::<Vec<_>>();

    Ok(NexusPagedModsResult {
        items,
        page,
        page_size: requested_page_size,
        total_items,
        total_pages,
        has_prev: total_pages > 0 && page > 1,
        has_next: total_pages > 0 && page < total_pages,
        warning: None,
    })
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
    Ok(
        find_matching_cached_mod(&local_mod, &cached_mods).or_else(|| {
            cached_mods.into_iter().find(|mod_info| {
                normalize_match_text(&mod_info.name) == normalize_match_text(lookup)
            })
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popular_period_uses_expected_default() {
        assert_eq!(normalize_popular_period("").unwrap(), "1w");
        assert_eq!(normalize_popular_period("1d").unwrap(), "1d");
    }

    #[test]
    fn page_offset_matches_count_offset_model() {
        assert_eq!(page_to_offset(1, 20), 0);
        assert_eq!(page_to_offset(2, 20), 20);
        assert_eq!(page_to_offset(3, 20), 40);
    }

    #[test]
    fn popular_payload_sorts_by_endorsements_and_filters_updated_at() {
        let payload = build_popular_listing_payload("1w", 2, 20).unwrap();

        assert_eq!(payload["variables"]["offset"], 20);
        assert_eq!(payload["variables"]["count"], 20);
        assert_eq!(
            payload["variables"]["sort"]["endorsements"]["direction"],
            "DESC"
        );
        assert!(payload["variables"]["filter"]["filter"][0]
            .get("updatedAt")
            .is_some());
        assert!(payload["variables"]["filter"]["filter"][0]
            .get("createdAt")
            .is_none());
    }

    #[test]
    fn graphql_listing_nodes_stay_partial() {
        let node = NexusPopularModNode {
            mod_id: 461,
            name: "STS2 Mod Manager".to_string(),
            summary: "manager".to_string(),
            downloads: 321,
            endorsements: 8,
            thumbnail_url: Some("https://example.com/cover.jpg".to_string()),
            status: "published".to_string(),
            mod_category: NexusPopularModCategory { category_id: 3 },
            uploader: NexusPopularUploader {
                name: "author".to_string(),
            },
        };

        let mod_info = NexusModInfo::from(node);
        assert_eq!(mod_info.mod_id, 461);
        assert_eq!(mod_info.mod_downloads, 321);
        assert_eq!(mod_info.endorsement_count, 8);
        assert_eq!(mod_info.category_id, 3);
        assert_eq!(mod_info.version, "");
        assert!(mod_info.description.is_none());
        assert_eq!(mod_info.mod_unique_downloads, 0);
    }
}
