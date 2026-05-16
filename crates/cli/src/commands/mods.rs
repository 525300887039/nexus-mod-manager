//! `nmm mods` 子命令组。

use crate::output::print_result;
use crate::{ModsAction, ModsArgs};
use nmm_core::mods as core_mods;
use nmm_core::AppContext;
use serde::Serialize;

#[derive(Serialize)]
struct ModEntry {
    name: String,
    enabled: bool,
    size: u64,
    #[serde(rename = "isFolder")]
    is_folder: bool,
}

#[derive(Serialize)]
struct ModsListResult {
    #[serde(rename = "gamePath")]
    game_path: String,
    #[serde(rename = "modsDir")]
    mods_dir: String,
    mods: Vec<ModEntry>,
}

pub async fn run(ctx: &AppContext, json: bool, args: ModsArgs) -> Result<(), String> {
    match args.action {
        ModsAction::List { enabled, disabled } => list(ctx, json, enabled, disabled),
        ModsAction::Enable { .. }
        | ModsAction::Disable { .. }
        | ModsAction::Install { .. }
        | ModsAction::Uninstall { .. }
        | ModsAction::Backup
        | ModsAction::Restore { .. } => {
            Err("mods enable/disable/install/uninstall/backup/restore 将在 step 5 实现".to_string())
        }
    }
}

fn list(
    ctx: &AppContext,
    json: bool,
    only_enabled: bool,
    only_disabled: bool,
) -> Result<(), String> {
    let game_path = ctx
        .game_path
        .lock()
        .map_err(|e| format!("game_path lock poisoned: {}", e))?
        .clone()
        .ok_or_else(|| "尚未选择游戏路径，请用 `nmm games switch <domain>` 设置当前游戏".to_string())?;

    let mods_subdir = ctx
        .current_profile
        .lock()
        .map_err(|e| format!("current_profile lock poisoned: {}", e))?
        .as_ref()
        .map(core_mods::current_mods_subdir)
        .unwrap_or_else(|| "mods".to_string());

    let mods_dir = core_mods::get_mods_dir(&game_path, &mods_subdir);

    let raw_mods = core_mods::scan_mods_internal_with_subdir(&game_path, &mods_subdir);
    let filtered: Vec<ModEntry> = raw_mods
        .into_iter()
        .filter(|m| {
            if only_enabled {
                m.enabled
            } else if only_disabled {
                !m.enabled
            } else {
                true
            }
        })
        .map(|m| ModEntry {
            name: m.name.unwrap_or(m.folder_name),
            enabled: m.enabled,
            size: m.size,
            is_folder: m.is_folder,
        })
        .collect();

    let result = ModsListResult {
        game_path,
        mods_dir: mods_dir.to_string_lossy().to_string(),
        mods: filtered,
    };

    print_result(&result, json, |result| {
        let mut out = String::new();
        out.push_str(&format!("mods 目录: {}\n", result.mods_dir));
        out.push_str(&format!("共 {} 个 mod\n", result.mods.len()));
        out.push_str(&format!(
            "{:<8} {:<12} {}\n",
            "enabled", "size", "name"
        ));
        out.push_str(&"-".repeat(80));
        out.push('\n');
        for m in &result.mods {
            out.push_str(&format!(
                "{:<8} {:<12} {}\n",
                if m.enabled { "✓" } else { "✗" },
                human_size(m.size),
                m.name
            ));
        }
        out.trim_end().to_string()
    })
}

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[unit_idx])
    }
}
