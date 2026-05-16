//! CLI 启动期 setup：
//!
//! - [`build_context`]：加载 config → init AppContext → 按当前 game 填充 profile / game_path；
//!   可选 `game_override` 参数实现 `--game <DOMAIN>` 临时覆盖（不写回 config）。
//! - [`detect_gui_running`]：Windows 上用命名 Mutex 检测 tauri-plugin-single-instance 占用。
//! - [`require_write_access`]：写命令在 GUI 在跑时拒绝执行。

use nmm_core::{config as core_config, db as core_db, AppContext};
use std::sync::Arc;

/// 加载 config → 初始化 AppContext → 按 current_game / `--game` 填充 profile + game_path。
pub fn build_context(game_override: Option<&str>) -> Result<Arc<AppContext>, String> {
    let mut cfg = core_config::load_config();

    if let Some(domain) = game_override {
        if !cfg.games.contains_key(domain) {
            return Err(format!(
                "未知游戏 domain: {}\n请先用 `nmm games add <domain> <path>` 配置该游戏",
                domain
            ));
        }
        cfg.current_game = Some(domain.to_string());
    }

    let db_path = core_db::cache_db_path()?;
    let ctx = AppContext::init(&db_path)?;

    if let Some(game) = core_config::current_game_config(&cfg) {
        *ctx.current_profile
            .lock()
            .map_err(|e| format!("AppContext.current_profile lock poisoned: {}", e))? =
            Some(game.profile.clone());
    }

    if let Some(path) = core_config::resolve_game_path_from_config(&cfg) {
        *ctx.game_path
            .lock()
            .map_err(|e| format!("AppContext.game_path lock poisoned: {}", e))? = Some(path);
    }

    Ok(ctx)
}

/// 写命令调用前的并发保护：GUI 在跑时拒绝执行。
///
/// `detect_gui_running` 在非 Windows 平台返回 false（项目本身是 Windows-first）。
pub fn require_write_access() -> Result<(), String> {
    if detect_gui_running() {
        Err(
            "Nexus Mod Manager GUI 正在运行，写命令被拒绝以避免 SQLite 并发冲突。\n\
             请先关闭 GUI，或通过 GUI 完成此操作。"
                .to_string(),
        )
    } else {
        Ok(())
    }
}

/// Windows: 用 tauri-plugin-single-instance 同名命名 Mutex（`{identifier}-sim`）检测 GUI 是否占用。
///
/// CreateMutexW 返回有效 handle 时：
/// - 若 GetLastError == ERROR_ALREADY_EXISTS：GUI 已持有同名 Mutex（在跑）
/// - 否则：本进程是第一个，立即 ReleaseHandle 避免拖住 Mutex
///
/// 非 Windows 平台：返回 false（项目当前 Windows-only）。
#[cfg(windows)]
pub fn detect_gui_running() -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError,
    };
    use windows_sys::Win32::System::Threading::CreateMutexW;

    let mutex_name = encode_wide(&format!("{}-sim", crate::BUNDLE_IDENTIFIER));

    // SAFETY: mutex_name 是 null-terminated UTF-16，符合 CreateMutexW 契约
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr());
        if handle.is_null() {
            // 创建失败：无法判断，保守起见返回 false（放行）
            return false;
        }
        let already_exists = GetLastError() == ERROR_ALREADY_EXISTS;
        // 不持有 mutex，立即释放（无论是不是新创建的）
        let _ = CloseHandle(handle);
        already_exists
    }
}

#[cfg(not(windows))]
pub fn detect_gui_running() -> bool {
    false
}

#[cfg(windows)]
fn encode_wide(text: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(Some(0))
        .collect()
}
