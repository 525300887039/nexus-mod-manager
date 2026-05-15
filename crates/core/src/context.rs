//! `AppContext`：GUI 与 CLI 共享的核心运行时状态。
//!
//! 字段集合精确为 `{db, current_profile, game_path, nexus_mod_cache, nexus_is_premium}`。
//!
//! **不包含 `game_state` 字段** —— 该字段是 GUI 进程的本地状态机（"idle" / "launching" /
//! "running"），由 src-tauri 侧的 `AppState` 单独维护。详见
//! `openspec/specs/project-structure/spec.md` 的"后续 CLI 化 change 共享的全局技术决策"
//! Requirement 第 1 条。

use crate::game_profile::GameProfile;
use crate::types::nexus::NexusModInfo;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct AppContext {
    pub db: Mutex<Connection>,
    pub current_profile: Mutex<Option<GameProfile>>,
    pub game_path: Mutex<Option<String>>,
    pub nexus_mod_cache: Mutex<HashMap<String, HashMap<u64, NexusModInfo>>>,
    pub nexus_is_premium: Mutex<Option<bool>>,
}

impl AppContext {
    /// 初始化 AppContext：打开数据库（必要时执行迁移），其余字段为空默认值。
    ///
    /// `current_profile` / `game_path` 等运行时状态由调用方在 init 后注入
    /// （比如 src-tauri 的 setup 流程会从 config 加载当前游戏并填充）。
    pub fn init(db_path: &Path) -> Result<Arc<Self>, String> {
        let db = crate::db::init_db(db_path)?;
        Ok(Arc::new(Self {
            db: Mutex::new(db),
            current_profile: Mutex::new(None),
            game_path: Mutex::new(None),
            nexus_mod_cache: Mutex::new(HashMap::new()),
            nexus_is_premium: Mutex::new(None),
        }))
    }
}
