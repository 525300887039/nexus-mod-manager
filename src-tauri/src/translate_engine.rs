//! src-tauri 薄壳：1 个 `#[tauri::command]` 转发到 `nmm_core::translate_engine`。

use crate::translate::TranslateResult;
use crate::AppState;
use nmm_core::translate_engine as core_engine;
use tauri::State;

#[tauri::command]
pub async fn translate_smart(
    text: String,
    state: State<'_, AppState>,
) -> Result<TranslateResult, String> {
    core_engine::translate_smart(&state.ctx, &text).await
}
