//! `nmm nexus` 子命令组。

use crate::cli_reporter::CliReporter;
use crate::output::print_result;
use crate::subprocess;
use crate::{NexusAction, NexusArgs};
use nmm_core::nexus_api as core_nexus;
use nmm_core::nexus_download::{self as core_dl, NexusDownloadEvent, NexusDownloadReporter};
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

    let reporter = CliReporter { json_mode: json };

    // 第一段：Premium 直链
    let (final_mod_id, final_file_id) = match try_premium_download(
        ctx,
        &reporter,
        &game_domain,
        mod_id,
        file_id,
    )
    .await
    {
        Ok(fid) => (mod_id, Some(fid)),
        Err(reason) => {
            // 第二段：GUI 子进程兜底。fallback 前 emit Preparing 让用户知道延迟原因
            reporter.report(NexusDownloadEvent::Preparing {
                message: format!(
                    "Premium 直链不可用（{}），准备启动 GUI 子进程下载（首次约需 2-3 秒）...",
                    reason
                ),
            });

            let gui_exe = subprocess::resolve_gui_binary()?;
            let result = subprocess::run_headless_download(
                &reporter,
                &gui_exe,
                mod_id,
                file_id,
                &game_domain,
            )
            .await?;
            (result.mod_id, result.file_id)
        }
    };

    // 下载流程结束后再 emit 一行最终结果总结（JSON 模式：type: result；人类模式：✓ 已下载）
    if json {
        let payload = json!({
            "type": "result",
            "ok": true,
            "modId": final_mod_id,
            "fileId": final_file_id,
        });
        println!("{}", payload);
    } else {
        match final_file_id {
            Some(fid) => println!("✓ 已下载并安装 mod_id={} file_id={}", final_mod_id, fid),
            None => println!("✓ 已下载并安装 mod_id={}", final_mod_id),
        }
    }
    Ok(())
}

/// 第一段：Premium 直链下载。
///
/// 任一步骤失败都返回 Err（含简短原因，用于 fallback 时 emit 给用户）。
/// 成功时返回实际使用的 file_id（caller 用来 emit 最终 result 行）。
async fn try_premium_download(
    ctx: &AppContext,
    reporter: &CliReporter,
    game_domain: &str,
    mod_id: u64,
    file_id: Option<u64>,
) -> Result<u64, String> {
    let is_premium = core_nexus::ensure_premium_status(ctx).await.unwrap_or(false);
    if !is_premium {
        return Err("当前账号不是 Premium / API Key 未设置 / 探测失败".to_string());
    }

    let resolved_file_id = match file_id {
        Some(id) => id,
        None => core_dl::resolve_preferred_file_id(game_domain, mod_id)
            .await
            .map_err(|e| format!("解析默认文件失败: {}", e))?,
    };

    core_dl::download_premium_via_api(ctx, reporter, game_domain, mod_id, resolved_file_id)
        .await
        .map_err(|e| format!("Premium API 下载失败: {}", e))?;

    Ok(resolved_file_id)
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
