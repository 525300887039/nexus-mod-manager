//! src-tauri 薄壳：1 个 `#[tauri::command]` 转发到 `nmm_core::translate`。

use nmm_core::translate as core_translate;

// Re-export 业务类型，让 src-tauri 内其他模块（translate_engine.rs / translate_llm.rs）
// 可继续通过 `crate::translate::xxx` 访问。
#[allow(unused_imports)]
pub use core_translate::{translate_via_mymemory, TranslateResult};

#[tauri::command]
pub async fn translate_text(text: String) -> TranslateResult {
    match core_translate::translate_via_mymemory(&text).await {
        Ok(translated) => TranslateResult::success(translated, "mymemory"),
        Err(error) => TranslateResult::failure(error),
    }
}
