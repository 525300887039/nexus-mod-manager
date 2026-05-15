//! src-tauri 薄壳：3 个 `#[tauri::command]` 转发到 `nmm_core::translate_llm`。

use crate::translate::TranslateResult;
use nmm_core::translate_llm as core_llm;

// Re-export 业务符号，让 src-tauri 内其他模块（translate_engine.rs）
// 可继续通过 `crate::translate_llm::xxx` 访问。
#[allow(unused_imports)]
pub use core_llm::{
    load_config, resolve_chat_completions_url, sanitize_for_save, save_config, translate, LlmConfig,
};

#[tauri::command]
pub async fn translate_llm(text: String) -> TranslateResult {
    let config = core_llm::load_config();
    match core_llm::translate(&text, &config).await {
        Ok(translated) => TranslateResult::success(translated, "llm"),
        Err(error) => TranslateResult::failure(error),
    }
}

#[tauri::command]
pub fn translate_llm_config_save(config: LlmConfig) -> Result<(), String> {
    let config = core_llm::sanitize_for_save(config);
    core_llm::save_config(&config)
}

#[tauri::command]
pub fn translate_llm_config_load() -> LlmConfig {
    core_llm::load_config()
}
