//! `nmm` CLI binary —— Nexus Mod Manager 命令行入口。
//!
//! 业务逻辑全部通过 `nmm-core` 调用；本 crate 只负责参数解析、AppContext 初始化、
//! reporter 桥接与输出格式化。**MUST NOT** 引入 GUI（src-tauri）依赖。
//!
//! 当前为骨架版本，后续 step 接入 clap subcommand 与各业务子命令。

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    println!("nmm CLI - skeleton");
}
