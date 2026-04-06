use serde::Serialize;

#[derive(Serialize)]
pub struct TranslateResult {
    pub success: bool,
    pub translated: Option<String>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn translate_text(text: String) -> TranslateResult {
    if text.trim().is_empty() {
        return TranslateResult {
            success: false,
            translated: None,
            error: Some("无内容".into()),
        };
    }

    let encoded = urlencoding::encode(text.trim());
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair=en|zh-CN",
        encoded
    );

    match reqwest::get(&url).await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(data) => {
                let status = data
                    .get("responseStatus")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if status == 200 {
                    let translated = data
                        .get("responseData")
                        .and_then(|d| d.get("translatedText"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    TranslateResult {
                        success: true,
                        translated,
                        error: None,
                    }
                } else {
                    let detail = data
                        .get("responseDetails")
                        .and_then(|v| v.as_str())
                        .unwrap_or("翻译失败");
                    TranslateResult {
                        success: false,
                        translated: None,
                        error: Some(detail.to_string()),
                    }
                }
            }
            Err(e) => TranslateResult {
                success: false,
                translated: None,
                error: Some(e.to_string()),
            },
        },
        Err(e) => TranslateResult {
            success: false,
            translated: None,
            error: Some(e.to_string()),
        },
    }
}
