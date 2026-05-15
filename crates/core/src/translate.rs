//! 翻译通道 1：MyMemory 公共翻译 API（免费，无需 Key）。
//!
//! 本模块无 Tauri 依赖。async fn 由调用方 runtime 驱动。

use serde::Serialize;

#[derive(Serialize)]
pub struct TranslateResult {
    pub success: bool,
    pub translated: Option<String>,
    pub error: Option<String>,
    pub provider: Option<String>,
}

impl TranslateResult {
    pub fn success(translated: String, provider: &str) -> Self {
        Self {
            success: true,
            translated: Some(translated),
            error: None,
            provider: Some(provider.to_string()),
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            translated: None,
            error: Some(error.into()),
            provider: None,
        }
    }
}

/// 调 MyMemory API 把英文翻成简体中文。
pub async fn translate_via_mymemory(text: &str) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("无内容".into());
    }

    let encoded = urlencoding::encode(trimmed);
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair=en|zh-CN",
        encoded
    );

    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    let data = resp
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;

    let status = data
        .get("responseStatus")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if status != 200 {
        let detail = data
            .get("responseDetails")
            .and_then(|v| v.as_str())
            .unwrap_or("翻译失败");
        return Err(detail.to_string());
    }

    let translated = data
        .get("responseData")
        .and_then(|d| d.get("translatedText"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "翻译结果为空".to_string())?;

    Ok(translated.to_string())
}
