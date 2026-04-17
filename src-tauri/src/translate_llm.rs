use crate::app_paths;
use crate::translate::TranslateResult;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

const LEGACY_DEFAULT_SYSTEM_PROMPT: &str =
    "你是一个游戏MOD翻译助手，请将以下英文翻译成简体中文，保留专有名词不翻译。只返回翻译结果，不要添加任何解释。";
const DEFAULT_SYSTEM_PROMPT: &str = "你是一个游戏MOD翻译助手，请将以下英文翻译成简体中文，保留专有名词不翻译。保持原文的结构、段落、列表、换行和标点风格；如果原始文本存在明显的格式问题，例如换行错乱、段落断裂、列表混乱或空白异常，请在不改变原意的前提下做必要修复；如果原始文本格式正常，则不要额外调整格式。只返回最终翻译结果，不要添加任何解释。";
const LLM_CONNECT_TIMEOUT_SECS: u64 = 10;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct LlmConfig {
    pub enabled: bool,
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub engine_mode: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_url: String::new(),
            api_key: String::new(),
            model: String::new(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            engine_mode: "dual".to_string(),
        }
    }
}

#[allow(unreachable_code)]
fn config_dir() -> Result<PathBuf, String> {
    return Ok(app_paths::writable_config_dir());

    let base = dirs::config_dir().ok_or_else(|| "无法解析配置目录".to_string())?;
    Ok(base.join("NexusModManager"))
}

#[allow(unreachable_code)]
fn config_path() -> Result<PathBuf, String> {
    return Ok(app_paths::current_config_file("llm_config.json"));

    Ok(config_dir()?.join("llm_config.json"))
}

fn load_config_path() -> Option<PathBuf> {
    let current = app_paths::current_config_file("llm_config.json");
    if current.exists() {
        return Some(current);
    }

    app_paths::existing_config_file("llm_config.json")
}

fn normalize_engine_mode(engine_mode: &str, enabled: bool) -> String {
    match engine_mode.trim().to_lowercase().as_str() {
        "mymemory" => "mymemory".to_string(),
        "llm" => "llm".to_string(),
        "dual" => "dual".to_string(),
        _ if enabled => "dual".to_string(),
        _ => "mymemory".to_string(),
    }
}

fn normalize_system_prompt(system_prompt: &str) -> String {
    let trimmed = system_prompt.trim();
    if trimmed.is_empty() || trimmed == LEGACY_DEFAULT_SYSTEM_PROMPT {
        DEFAULT_SYSTEM_PROMPT.to_string()
    } else {
        trimmed.to_string()
    }
}

fn sanitize_for_runtime(mut config: LlmConfig) -> LlmConfig {
    config.api_url = config.api_url.trim().to_string();
    config.api_key = config.api_key.trim().to_string();
    config.model = config.model.trim().to_string();
    config.system_prompt = normalize_system_prompt(&config.system_prompt);
    config.engine_mode = normalize_engine_mode(&config.engine_mode, config.enabled);
    config
}

fn sanitize_for_save(mut config: LlmConfig) -> LlmConfig {
    config = sanitize_for_runtime(config);
    config.enabled = config.engine_mode != "mymemory";
    config
}

fn ensure_llm_ready(config: &LlmConfig) -> Result<(), String> {
    if !config.enabled {
        return Err("大模型翻译未启用".to_string());
    }
    if config.api_url.is_empty() {
        return Err("未配置大模型 API 地址".to_string());
    }
    if config.api_key.is_empty() {
        return Err("未配置大模型 API Key".to_string());
    }
    if config.model.is_empty() {
        return Err("未配置大模型模型名".to_string());
    }
    Ok(())
}

pub fn load_config() -> LlmConfig {
    let Some(path) = load_config_path() else {
        return LlmConfig::default();
    };

    let parsed = fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str::<LlmConfig>(&content).ok());
    let config = parsed
        .map(sanitize_for_runtime)
        .unwrap_or_else(LlmConfig::default);

    if path != app_paths::current_config_file("llm_config.json") {
        let _ = save_config(&config);
    }

    config
}

pub fn save_config(config: &LlmConfig) -> Result<(), String> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("创建配置目录失败: {}", e))?;

    let json =
        serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {}", e))?;
    let path = config_path()?;
    fs::write(&path, json).map_err(|e| format!("写入配置文件失败 ({}): {}", path.display(), e))
}

fn extract_message_content(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(content) => {
            let trimmed = content.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        serde_json::Value::Array(items) => {
            let combined = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(|text| text.as_str())
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
                .join("\n");
            (!combined.trim().is_empty()).then_some(combined)
        }
        _ => None,
    }
}

fn resolve_chat_completions_url(api_url: &str) -> Result<String, String> {
    let trimmed = api_url.trim();
    let mut parsed =
        reqwest::Url::parse(trimmed).map_err(|e| format!("大模型 API 地址无效: {}", e))?;

    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "大模型 API 地址协议无效: {}，仅支持 http 或 https",
                scheme
            ))
        }
    }

    let normalized_path = match parsed.path().trim_end_matches('/') {
        "" | "/" => "/chat/completions".to_string(),
        path if path.ends_with("/chat/completions") => path.to_string(),
        path => format!("{}/chat/completions", path),
    };

    parsed.set_path(&normalized_path);
    Ok(parsed.to_string())
}

pub async fn translate(text: &str, config: &LlmConfig) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("无内容".to_string());
    }

    ensure_llm_ready(config)?;
    let request_url = resolve_chat_completions_url(&config.api_url)?;

    let client = reqwest::Client::builder()
        // Only bound connection setup. Local/self-hosted backends may need much longer to
        // finish generating a translation for large mod descriptions.
        .connect_timeout(Duration::from_secs(LLM_CONNECT_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("初始化大模型 HTTP 客户端失败: {}", e))?;
    let payload = json!({
        "model": config.model.as_str(),
        "messages": [
            {
                "role": "system",
                "content": config.system_prompt.as_str(),
            },
            {
                "role": "user",
                "content": trimmed,
            }
        ],
        "temperature": 0.3
    });

    let response = client
        .post(&request_url)
        .header(AUTHORIZATION, format!("Bearer {}", config.api_key))
        .header(CONTENT_TYPE, "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("请求大模型 API 失败 ({}): {}", request_url, e))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取大模型响应失败: {}", e))?;

    if !status.is_success() {
        let detail = if body.trim().is_empty() {
            "空响应".to_string()
        } else {
            body
        };
        return Err(format!(
            "大模型 API 返回错误 {} ({}): {}",
            status, request_url, detail
        ));
    }

    if body.trim().is_empty() {
        return Err("大模型 API 返回空响应".to_string());
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("解析大模型响应失败: {}", e))?;

    let translated = parsed
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(extract_message_content)
        .ok_or_else(|| "大模型响应缺少 choices[0].message.content".to_string())?;

    Ok(translated)
}

#[tauri::command]
pub async fn translate_llm(text: String) -> TranslateResult {
    let config = load_config();
    match translate(&text, &config).await {
        Ok(translated) => TranslateResult::success(translated, "llm"),
        Err(error) => TranslateResult::failure(error),
    }
}

#[tauri::command]
pub fn translate_llm_config_save(config: LlmConfig) -> Result<(), String> {
    let config = sanitize_for_save(config);
    save_config(&config)
}

#[tauri::command]
pub fn translate_llm_config_load() -> LlmConfig {
    load_config()
}
