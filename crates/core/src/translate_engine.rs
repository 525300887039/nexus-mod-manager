//! 翻译智能路由：缓存命中 → MyMemory → LLM，根据 `LlmConfig.engine_mode` 选择顺序。
//!
//! 本模块无 Tauri 依赖。所有持状态的辅助函数接收 `&AppContext`。

use crate::context::AppContext;
use crate::db;
use crate::translate::{translate_via_mymemory, TranslateResult};
use crate::translate_llm;

fn current_game_domain(ctx: &AppContext) -> Result<String, String> {
    ctx.current_profile
        .lock()
        .map_err(|e| format!("game profile lock poisoned: {}", e))?
        .as_ref()
        .map(|profile| profile.nexus_domain.clone())
        .ok_or_else(|| "please select a game first".to_string())
}

fn read_cached_translation(ctx: &AppContext, text: &str) -> Result<Option<String>, String> {
    let db_conn = ctx
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))?;
    let game_domain = current_game_domain(ctx)?;
    db::translation_cache_get_db(&db_conn, &game_domain, text)
}

fn write_cached_translation(
    ctx: &AppContext,
    text: &str,
    translated: &str,
    provider: &str,
) -> Result<(), String> {
    let db_conn = ctx
        .db
        .lock()
        .map_err(|e| format!("数据库锁已损坏: {}", e))?;
    let game_domain = current_game_domain(ctx)?;
    db::translation_cache_set_db(&db_conn, &game_domain, text, translated, provider)
}

fn llm_mode(config: &translate_llm::LlmConfig) -> &str {
    match config.engine_mode.as_str() {
        "mymemory" => "mymemory",
        "llm" if config.enabled => "llm",
        "dual" if config.enabled => "dual",
        _ => {
            if config.enabled {
                "dual"
            } else {
                "mymemory"
            }
        }
    }
}

/// 智能翻译入口：先查缓存，未命中则按 `LlmConfig.engine_mode` 顺序调引擎，
/// 成功后回写缓存。返回 `TranslateResult` 永远 `Ok`（错误信息在 `result.error` 字段）。
pub async fn translate_smart(ctx: &AppContext, text: &str) -> Result<TranslateResult, String> {
    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Ok(TranslateResult::failure("无内容"));
    }

    match read_cached_translation(ctx, &trimmed) {
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
            "mymemory" => translate_via_mymemory(&trimmed).await,
            "llm" => translate_llm::translate(&trimmed, &config).await,
            _ => Err("未知翻译引擎".to_string()),
        };

        match result {
            Ok(translated) => {
                if let Err(error) = write_cached_translation(ctx, &trimmed, &translated, provider) {
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
