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

pub async fn run(_ctx: &AppContext, json: bool, args: ConfigArgs) -> Result<(), String> {
    match args.action {
        ConfigAction::GetNexusKey => get_nexus_key(json),
        ConfigAction::SetNexusKey { .. } => {
            Err("config set-nexus-key 将在 step 4 实现".to_string())
        }
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
