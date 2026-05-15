//! Nexus Mod Manager 业务逻辑核心库
//!
//! 本 crate 提供 GUI 与 CLI 共享的业务逻辑（数据库、配置、Nexus 数据结构等），
//! 必须保持**无 Tauri 依赖**——任何 `tauri` / `tauri-plugin-*` 引用都属于约束违反，
//! 参见 OpenSpec capability `core-context` 中的 "crates/core 必须是无 Tauri 依赖的库 crate"。
//!
//! AppContext 字段集合精确为 `{db, current_profile, game_path, nexus_mod_cache,
//! nexus_is_premium}`，**不包含 `game_state`**（该字段为 GUI 进程的本地状态机，
//! 沿用 project-structure 全局决策 #1）。

pub mod paths;
pub mod game_profile;
pub mod db;
pub mod config;
pub mod mods;
pub mod profiles;
pub mod saves;
pub mod translate;
pub mod translate_engine;
pub mod translate_llm;
pub mod translations;
pub mod types;

mod context;
pub use context::AppContext;
