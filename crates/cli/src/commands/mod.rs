//! 各 CLI 子命令实现。每组对应 main.rs 中 `Commands` enum 的一个 variant。
//!
//! 所有命令通过 `&AppContext` 与 nmm-core 业务函数交互，自身不重复业务逻辑。
//! 输出走 `cli::output::print_result` 统一处理人类 / JSON 双模式。

pub mod config;
pub mod games;
pub mod mods;
pub mod profiles;
pub mod saves;
