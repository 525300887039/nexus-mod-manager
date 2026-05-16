//! `nmm nexus` 子命令组。

use crate::output::print_result;
use crate::{NexusAction, NexusArgs};
use nmm_core::nexus_api as core_nexus;
use nmm_core::types::nexus::NexusModInfo;
use nmm_core::AppContext;
use serde::Serialize;

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
        NexusAction::Download { .. } => Err("nexus download 将在 step 8 实现".to_string()),
    }
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
