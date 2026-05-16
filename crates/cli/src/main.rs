//! `nmm` CLI binary —— Nexus Mod Manager 命令行入口。
//!
//! 业务逻辑全部通过 `nmm-core` 调用；本 crate 只负责参数解析、AppContext 初始化、
//! reporter 桥接与输出格式化。**MUST NOT** 引入 GUI（src-tauri）依赖。
//!
//! Windows 下通过命名 Mutex 检测 GUI 是否在跑——
//! tauri-plugin-single-instance 用 `{bundle-identifier}-sim` 这一约定。

use clap::{Parser, Subcommand};

mod cli_reporter;
mod commands;
mod output;
mod setup;
mod subprocess;

/// Tauri GUI 的 bundle identifier，需与 `src-tauri/tauri.conf.json` 的 `identifier` 一致。
/// 用于拼接 tauri-plugin-single-instance 的 Named Mutex 名 `{identifier}-sim`。
const BUNDLE_IDENTIFIER: &str = "com.nexus.mod-manager";

/// Nexus Mod Manager CLI
///
/// 通用 Nexus Mods 管理器的命令行版本。所有命令默认输出人类可读文本；
/// 加 `--json` 切换为机器可读 JSON（适合 AI 工具或 shell 脚本调用）。
#[derive(Parser, Debug)]
#[command(name = "nmm", version, about = "Nexus Mod Manager 命令行入口")]
struct Cli {
    /// 输出 JSON 格式（适合 AI 工具或脚本解析）
    #[arg(long, global = true)]
    json: bool,

    /// 临时指定游戏 domain，不写回 config.json
    #[arg(long, global = true, value_name = "DOMAIN")]
    game: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 已配置游戏的列表 / 切换 / 增删
    Games(GamesArgs),
    /// 本地 mods 管理
    Mods(ModsArgs),
    /// Nexus 远程查询与下载
    Nexus(NexusArgs),
    /// 游戏存档管理
    Saves(SavesArgs),
    /// 配置管理（API Key 等）
    Config(ConfigArgs),
    /// Profile 管理
    Profiles(ProfilesArgs),
}

/// 占位子命令枚举，后续 step 填充具体子命令
#[derive(clap::Args, Debug)]
struct GamesArgs {
    #[command(subcommand)]
    action: GamesAction,
}

#[derive(Subcommand, Debug)]
enum GamesAction {
    /// 列出已配置游戏
    List,
    /// 切换当前游戏
    Switch { domain: String },
    /// 添加游戏 domain 与路径
    Add { domain: String, path: String },
    /// 删除游戏
    Remove { domain: String },
}

#[derive(clap::Args, Debug)]
struct ModsArgs {
    #[command(subcommand)]
    action: ModsAction,
}

#[derive(Subcommand, Debug)]
enum ModsAction {
    /// 列出当前游戏的 mods
    List {
        /// 仅列已启用
        #[arg(long, conflicts_with = "disabled")]
        enabled: bool,
        /// 仅列已禁用
        #[arg(long)]
        disabled: bool,
    },
    /// 启用某个 mod
    Enable { name: String },
    /// 禁用某个 mod
    Disable { name: String },
    /// 安装本地归档到 mods 目录
    Install { path: String },
    /// 卸载某个 mod
    Uninstall { name: String },
    /// 备份当前 mods 目录
    Backup,
    /// 恢复某次备份
    Restore { backup_id: String },
}

#[derive(clap::Args, Debug)]
struct NexusArgs {
    #[command(subcommand)]
    action: NexusAction,
}

#[derive(Subcommand, Debug)]
enum NexusAction {
    /// 按"已安装 mod"的名字查找对应 Nexus 远程条目（与 GUI 翻译流程一致；不是全网搜索）
    Search { query: String },
    /// 查看 mod 详情（按 mod_id 直接拉 Nexus API）
    Info { mod_id: u64 },
    /// 拉取当前游戏的 trending mod 列表
    Trending,
    /// 下载某个 mod（当前仅支持 Premium 直链）
    Download {
        mod_id: u64,
        /// 指定文件 id；缺省时自动选 MAIN 分类的第一个
        #[arg(long, value_name = "ID")]
        file_id: Option<u64>,
    },
}

#[derive(clap::Args, Debug)]
struct SavesArgs {
    #[command(subcommand)]
    action: SavesAction,
}

#[derive(Subcommand, Debug)]
enum SavesAction {
    /// 列出存档
    List,
    /// 导出存档槽位到 zip。save_id 形如 `profile1` 或 `modded:profile1`
    Export { save_id: String, path: String },
    /// 从 zip 导入到目标 save_id（形如 `profile1` 或 `modded:profile1`）；目标已存在时会自动备份
    Import { save_id: String, path: String },
    /// 删除存档备份（id 为 `saves list` 中 backup 的 name 字段）
    DeleteBackup { id: String },
}

#[derive(clap::Args, Debug)]
struct ConfigArgs {
    #[command(subcommand)]
    action: ConfigAction,
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// 获取当前 Nexus API Key
    GetNexusKey,
    /// 设置 Nexus API Key
    SetNexusKey { key: String },
}

#[derive(clap::Args, Debug)]
struct ProfilesArgs {
    #[command(subcommand)]
    action: ProfilesAction,
}

#[derive(Subcommand, Debug)]
enum ProfilesAction {
    /// 列出 profiles
    List,
    /// 加载某个 profile
    Load { name: String },
    /// 保存当前为某个 profile
    Save { name: String },
}

/// 子命令是否需要写权限（写命令在 GUI 在跑时会被拒绝）。
fn requires_write_access(cmd: &Commands) -> bool {
    match cmd {
        Commands::Games(GamesArgs { action }) => !matches!(action, GamesAction::List),
        Commands::Mods(ModsArgs { action }) => !matches!(action, ModsAction::List { .. }),
        Commands::Nexus(NexusArgs { action }) => matches!(action, NexusAction::Download { .. }),
        Commands::Saves(SavesArgs { action }) => !matches!(action, SavesAction::List),
        Commands::Config(ConfigArgs { action }) => {
            !matches!(action, ConfigAction::GetNexusKey)
        }
        Commands::Profiles(ProfilesArgs { action }) => !matches!(action, ProfilesAction::List),
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let cli = Cli::parse();
    match dispatch(cli).await {
        Ok(()) => {}
        Err(err) => {
            eprintln!("nmm: {}", err);
            std::process::exit(1);
        }
    }
}

async fn dispatch(cli: Cli) -> Result<(), String> {
    if requires_write_access(&cli.command) {
        setup::require_write_access()?;
    }

    let ctx = setup::build_context(cli.game.as_deref())?;
    let json = cli.json;

    match cli.command {
        Commands::Games(args) => commands::games::run(&ctx, json, args).await,
        Commands::Mods(args) => commands::mods::run(&ctx, json, args).await,
        Commands::Config(args) => commands::config::run(&ctx, json, args).await,
        Commands::Saves(args) => commands::saves::run(&ctx, json, args).await,
        Commands::Profiles(args) => commands::profiles::run(&ctx, json, args).await,
        Commands::Nexus(args) => commands::nexus::run(&ctx, json, args).await,
    }
}
