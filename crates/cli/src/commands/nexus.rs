//! `nmm nexus` 子命令组。

use crate::cli_reporter::CliReporter;
use crate::output::print_result;
use crate::{NexusAction, NexusArgs};
use nmm_core::nexus_api as core_nexus;
use nmm_core::nexus_download as core_dl;
use nmm_core::types::nexus::NexusModInfo;
use nmm_core::AppContext;
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
struct ModListResult {
    mods: Vec<NexusModInfo>,
}

#[derive(Serialize)]
struct SearchResult {
    query: String,
    #[serde(rename = "mod")]
    matched: Option<NexusModInfo>,
}

pub async fn run(ctx: &AppContext, json: bool, args: NexusArgs) -> Result<(), String> {
    match args.action {
        NexusAction::Trending => trending(ctx, json).await,
        NexusAction::Info { mod_id } => info(ctx, json, mod_id).await,
        NexusAction::Search { query } => search(ctx, json, query).await,
        NexusAction::Download { mod_id, file_id } => download(ctx, json, mod_id, file_id).await,
    }
}

async fn download(
    ctx: &AppContext,
    json: bool,
    mod_id: u64,
    file_id: Option<u64>,
) -> Result<(), String> {
    // 取当前游戏 domain
    let game_domain = ctx
        .current_profile
        .lock()
        .map_err(|e| format!("current_profile lock poisoned: {}", e))?
        .as_ref()
        .map(|p| p.nexus_domain.clone())
        .ok_or_else(|| {
            "尚未选择游戏，请用 `nmm games switch <domain>` 设置当前游戏".to_string()
        })?;

    // 本 change 范围内仅支持 Premium 直链——非 Premium 明确失败
    let is_premium = core_nexus::ensure_premium_status(ctx).await.unwrap_or(false);
    if !is_premium {
        return Err(
            "nmm nexus download 当前仅支持 Premium 直链下载。\n\
             你的账号不是 Premium 会员（或 API Key 未设置 / API 探测失败）。\n\
             请使用 GUI（npm run tauri:dev）完成此下载，\n\
             或等待下一 change `add-cli-download-fallback` 引入 headless / GUI 子进程兜底。"
                .to_string(),
        );
    }

    // file_id 缺省时调 resolve_preferred_file_id 选 MAIN 分类的第一个
    let resolved_file_id = match file_id {
        Some(id) => id,
        None => core_dl::resolve_preferred_file_id(&game_domain, mod_id).await?,
    };

    let reporter = CliReporter { json_mode: json };

    core_dl::download_premium_via_api(ctx, &reporter, &game_domain, mod_id, resolved_file_id)
        .await?;

    // 下载流程结束后再 emit 一行最终结果总结（JSON 模式：type: result；人类模式：✓ 已下载）
    if json {
        let payload = json!({
            "type": "result",
            "ok": true,
            "modId": mod_id,
            "fileId": resolved_file_id,
        });
        println!("{}", payload);
    } else {
        println!("✓ 已下载并安装 mod_id={} file_id={}", mod_id, resolved_file_id);
    }
    Ok(())
}

async fn trending(ctx: &AppContext, json: bool) -> Result<(), String> {
    let mods = core_nexus::get_trending(ctx).await?;
    let result = ModListResult { mods };
    print_result(&result, json, |r| format_mod_list("Trending mods", &r.mods))
}

async fn info(ctx: &AppContext, json: bool, mod_id: u64) -> Result<(), String> {
    let m = core_nexus::get_mod(ctx, mod_id).await?;
    print_result(&m, json, format_mod_detail)
}

async fn search(ctx: &AppContext, json: bool, query: String) -> Result<(), String> {
    let matched = core_nexus::find_mod_by_name(ctx, &query).await?;
    let result = SearchResult {
        query: query.clone(),
        matched,
    };

    print_result(&result, json, |r| match &r.matched {
        Some(m) => format!(
            "查询 `{}` 命中 mod_id={} {}\n{}",
            r.query,
            m.mod_id,
            m.name,
            format_mod_detail(m)
        ),
        None => format!("未找到与 `{}` 匹配的 mod", r.query),
    })
}

fn format_mod_list(title: &str, mods: &[NexusModInfo]) -> String {
    if mods.is_empty() {
        return format!("{}: (空)", title);
    }
    let mut out = format!("{}（共 {} 个）\n", title, mods.len());
    out.push_str(&format!(
        "{:<10} {:<12} {:<12} {}\n",
        "modId", "endorsements", "downloads", "name"
    ));
    out.push_str(&"-".repeat(80));
    out.push('\n');
    for m in mods {
        out.push_str(&format!(
            "{:<10} {:<12} {:<12} {}\n",
            m.mod_id, m.endorsement_count, m.mod_downloads, m.name
        ));
    }
    out.trim_end().to_string()
}

fn format_mod_detail(m: &NexusModInfo) -> String {
    let mut out = String::new();
    out.push_str(&format!("Mod #{}: {}\n", m.mod_id, m.name));
    out.push_str(&format!("作者: {}\n", m.author));
    out.push_str(&format!("版本: {}\n", m.version));
    out.push_str(&format!("赞同: {}\n", m.endorsement_count));
    out.push_str(&format!(
        "下载: {}（独立 {}）\n",
        m.mod_downloads, m.mod_unique_downloads
    ));
    out.push_str(&format!("状态: {} (available: {})\n", m.status, m.available));
    if !m.summary.is_empty() {
        out.push_str(&format!("\n摘要:\n{}\n", m.summary));
    }
    out.trim_end().to_string()
}
