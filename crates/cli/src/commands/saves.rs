//! `nmm saves` 子命令组。

use crate::output::print_result;
use crate::{SavesAction, SavesArgs};
use nmm_core::game_profile::GameProfile;
use nmm_core::saves::{self as core_saves, SaveExportOpts};
use nmm_core::AppContext;
use serde::Serialize;

#[derive(Serialize)]
struct SavesActionResult {
    success: bool,
    message: String,
}

pub async fn run(ctx: &AppContext, json: bool, args: SavesArgs) -> Result<(), String> {
    let profile = require_profile(ctx)?;
    if !profile.saves_enabled {
        return Err(format!(
            "当前游戏 {} 未启用存档管理（saves_enabled = false）",
            profile.nexus_domain
        ));
    }

    match args.action {
        SavesAction::List => list(&profile, json),
        SavesAction::Export { save_id, path } => export(&profile, json, &save_id, path),
        SavesAction::Import { save_id, path } => import(&profile, json, &save_id, path),
        SavesAction::DeleteBackup { id } => delete_backup(&profile, json, &id),
    }
}

fn require_profile(ctx: &AppContext) -> Result<GameProfile, String> {
    ctx.current_profile
        .lock()
        .map_err(|e| format!("current_profile lock poisoned: {}", e))?
        .as_ref()
        .cloned()
        .ok_or_else(|| {
            "尚未选择游戏，请用 `nmm games switch <domain>` 设置当前游戏".to_string()
        })
}

fn parse_save_id(save_id: &str) -> SaveExportOpts {
    if let Some(stripped) = save_id.strip_prefix("modded:") {
        SaveExportOpts {
            slot: stripped.to_string(),
            modded: true,
        }
    } else {
        SaveExportOpts {
            slot: save_id.to_string(),
            modded: false,
        }
    }
}

fn list(profile: &GameProfile, json: bool) -> Result<(), String> {
    let result = core_saves::scan_saves(profile);

    print_result(&result, json, |r| {
        let mut out = String::new();
        out.push_str(&format!("存档槽位（共 {} 个）\n", r.slots.len()));
        out.push_str(&format!(
            "{:<22} {:<8} {:<10} {}\n",
            "saveId", "empty", "size", "lastModified"
        ));
        out.push_str(&"-".repeat(80));
        out.push('\n');
        for slot in &r.slots {
            let id = if slot.modded {
                format!("modded:{}", slot.slot)
            } else {
                slot.slot.clone()
            };
            out.push_str(&format!(
                "{:<22} {:<8} {:<10} {}\n",
                id,
                if slot.empty { "✓" } else { "✗" },
                slot.size,
                slot.last_modified.as_deref().unwrap_or("-")
            ));
        }
        out.push_str(&format!("\n备份（共 {} 个）\n", r.backups.len()));
        for b in &r.backups {
            out.push_str(&format!("  {} ({} bytes, {})\n", b.name, b.size, b.time));
        }
        out.trim_end().to_string()
    })
}

fn export(
    profile: &GameProfile,
    json: bool,
    save_id: &str,
    dest_path: String,
) -> Result<(), String> {
    let opts = parse_save_id(save_id);
    let outcome = core_saves::export_save_to_zip(profile, &opts, &dest_path)?;
    if !outcome.success {
        return Err(outcome
            .error
            .unwrap_or_else(|| "export 失败但未返回原因".to_string()));
    }
    let result = SavesActionResult {
        success: true,
        message: format!("已导出 {} → {}", save_id, dest_path),
    };
    print_result(&result, json, |r| r.message.clone())
}

fn import(
    profile: &GameProfile,
    json: bool,
    save_id: &str,
    zip_path: String,
) -> Result<(), String> {
    let opts = parse_save_id(save_id);
    let outcome = core_saves::import_save_from_zip(profile, &opts, &zip_path)?;
    if !outcome.success {
        return Err(outcome
            .error
            .unwrap_or_else(|| "import 失败但未返回原因".to_string()));
    }
    let result = SavesActionResult {
        success: true,
        message: format!("已导入 {} → {}", zip_path, save_id),
    };
    print_result(&result, json, |r| r.message.clone())
}

fn delete_backup(profile: &GameProfile, json: bool, backup_id: &str) -> Result<(), String> {
    let scanned = core_saves::scan_saves(profile);
    let backup = scanned
        .backups
        .iter()
        .find(|b| b.name == backup_id)
        .ok_or_else(|| {
            let mut hint = format!("未找到 backup id `{}`", backup_id);
            if !scanned.backups.is_empty() {
                hint.push_str("\n可用 backup id：");
                for b in &scanned.backups {
                    hint.push_str(&format!("\n  {}", b.name));
                }
            }
            hint
        })?;

    let outcome = core_saves::delete_backup(&backup.path);
    if !outcome.success {
        return Err(outcome
            .error
            .unwrap_or_else(|| "delete_backup 失败但未返回原因".to_string()));
    }
    let result = SavesActionResult {
        success: true,
        message: format!("已删除备份: {}", backup_id),
    };
    print_result(&result, json, |r| r.message.clone())
}
