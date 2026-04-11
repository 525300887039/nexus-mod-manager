use crate::translate::TranslateResult;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

const DEFAULT_SYSTEM_PROMPT: &str = "你是一个游戏MOD翻译助手，请将以下英文翻译成简体中文，保留专有名词不翻译。只返回翻译结果，不要添加任何解释。";

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

fn config_dir() -> Result<PathBuf, String> {
    let base = dirs::config_dir().ok_or_else(|| "无法解析配置目录".to_string())?;
    Ok(base.join("STS2ModManager"))
}

fn config_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("llm_config.json"))
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

fn sanitize_for_runtime(mut config: LlmConfig) -> LlmConfig {
    config.api_url = config.api_url.trim().to_string();
    config.api_key = config.api_key.trim().to_string();
    config.model = config.model.trim().to_string();
    if config.system_prompt.trim().is_empty() {
        config.system_prompt = DEFAULT_SYSTEM_PROMPT.to_string();
    } else {
        config.system_prompt = config.system_prompt.trim().to_string();
    }
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
    let Ok(path) = config_path() else {
        return LlmConfig::default();
    };

    if !path.exists() {
        return LlmConfig::default();
    }

    let parsed = fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str::<LlmConfig>(&content).ok());

    parsed
        .map(sanitize_for_runtime)
        .unwrap_or_else(LlmConfig::default)
}

pub fn save_config(config: &LlmConfig) -> Result<(), String> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("创建配置目录失败: {}", e))?;

    let json =
        serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {}", e))?;
    let path = config_path()?;
    fs::write(&path, json).map_err(|e| format!("写入配置文件失败 ({}): {}", path.display(), e))
}

pub async fn translate(text: &str, config: &LlmConfig) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("无内容".to_string());
    }

    ensure_llm_ready(config)?;

    let client = reqwest::Client::new();
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
        .post(&config.api_url)
        .header(AUTHORIZATION, format!("Bearer {}", config.api_key))
        .header(CONTENT_TYPE, "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("请求大模型 API 失败: {}", e))?;

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
        return Err(format!("大模型 API 返回错误 {}: {}", status, detail));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("解析大模型响应失败: {}", e))?;

    let translated = parsed
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .ok_or_else(|| "大模型响应缺少 choices[0].message.content".to_string())?;

    Ok(translated.to_string())
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
