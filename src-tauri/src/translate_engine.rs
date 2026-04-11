use crate::translate::TranslateResult;
use crate::{db, translate, translate_llm, AppState};
use rusqlite::Connection;
use tauri::State;

fn lock_db<'a>(
    state: &'a State<'_, AppState>,
) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    state
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))
}

fn read_cached_translation(
    state: &State<'_, AppState>,
    text: &str,
) -> Result<Option<String>, String> {
    let db = lock_db(state)?;
    db::translation_cache_get_db(&db, text)
}

fn write_cached_translation(
    state: &State<'_, AppState>,
    text: &str,
    translated: &str,
    provider: &str,
) -> Result<(), String> {
    let db = lock_db(state)?;
    db::translation_cache_set_db(&db, text, translated, provider)
}

fn llm_mode(config: &translate_llm::LlmConfig) -> &str {
    match config.engine_mode.as_str() {
        "mymemory" => "mymemory",
        "llm" => "llm",
        "dual" => "dual",
        _ => {
            if config.enabled {
                "dual"
            } else {
                "mymemory"
            }
        }
    }
}

#[tauri::command]
pub async fn translate_smart(
    text: String,
    state: State<'_, AppState>,
) -> Result<TranslateResult, String> {
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Ok(TranslateResult::failure("无内容"));
    }

    match read_cached_translation(&state, &trimmed) {
        Ok(Some(cached)) => return Ok(TranslateResult::success(cached, "cache")),
        Ok(None) => {}
        Err(error) => {
            eprintln!("Failed to read translation cache: {}", error);
        }
    }

    let config = translate_llm::load_config();
    let mut errors = Vec::new();

    let engine_order: &[&str] = match llm_mode(&config) {
        "mymemory" => &["mymemory"],
        "llm" => &["llm"],
        _ => &["mymemory", "llm"],
    };

    for provider in engine_order {
        let provider = *provider;
        let result = match provider {
            "mymemory" => translate::translate_via_mymemory(&trimmed).await,
            "llm" => translate_llm::translate(&trimmed, &config).await,
            _ => Err("未知翻译引擎".to_string()),
        };

        match result {
            Ok(translated) => {
                if let Err(error) =
                    write_cached_translation(&state, &trimmed, &translated, provider)
                {
                    eprintln!(
                        "Failed to write translation cache ({}): {}",
                        provider, error
                    );
                }
                return Ok(TranslateResult::success(translated, provider));
            }
            Err(error) => {
                errors.push(format!("{}: {}", provider, error));
            }
        }
    }

    if errors.is_empty() {
        Ok(TranslateResult::failure("所有翻译渠道均失败"))
    } else {
        Ok(TranslateResult::failure(format!(
            "所有翻译渠道均失败: {}",
            errors.join(" | ")
        )))
    }
}
