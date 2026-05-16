//! `nmm config` 子命令组。

use crate::output::print_result;
use crate::{ConfigAction, ConfigArgs};
use nmm_core::config as core_config;
use nmm_core::AppContext;
use serde::Serialize;

#[derive(Serialize)]
struct NexusKeyResult {
    #[serde(rename = "nexusApiKey")]
    nexus_api_key: Option<String>,
}

pub async fn run(ctx: &AppContext, json: bool, args: ConfigArgs) -> Result<(), String> {
    match args.action {
        ConfigAction::GetNexusKey => get_nexus_key(json),
        ConfigAction::SetNexusKey { key } => set_nexus_key(ctx, json, key),
    }
}

fn get_nexus_key(json: bool) -> Result<(), String> {
    let cfg = core_config::load_config();
    let result = NexusKeyResult {
        nexus_api_key: cfg
            .nexus_api_key
            .filter(|key| !key.trim().is_empty()),
    };

    print_result(&result, json, |result| match &result.nexus_api_key {
        Some(key) => format!("Nexus API Key: {}", key),
        None => "Nexus API Key: (未设置)".to_string(),
    })
}

fn set_nexus_key(ctx: &AppContext, json: bool, key: String) -> Result<(), String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("API key 不能为空".to_string());
    }

    let mut cfg = core_config::load_config();
    cfg.nexus_api_key = Some(trimmed.to_string());
    core_config::save_config_inner(&cfg)?;

    // 与 GUI 端 config_save_nexus_key 行为对齐：换 key 后清空 premium 缓存
    if let Ok(mut premium) = ctx.nexus_is_premium.lock() {
        *premium = None;
    }

    let result = NexusKeyResult {
        nexus_api_key: Some(trimmed.to_string()),
    };
    print_result(&result, json, |_| {
        "Nexus API Key 已更新".to_string()
    })
}
