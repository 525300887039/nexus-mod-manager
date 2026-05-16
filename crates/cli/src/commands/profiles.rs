//! `nmm profiles` 子命令组。
//!
//! profiles 数据结构（与 GUI 前端 `ProfileManager.jsx` 对齐）：
//! ```json
//! {
//!   "<profile-name>": {
//!     "snapshot": { "<mod-id>": <bool>, ... },
//!     "savedAt": "<iso8601>"
//!   }
//! }
//! ```
//! GUI 没有"当前激活的 profile"字段；每次 apply 通过对比 snapshot 与 mods 状态做 toggle。

use crate::output::print_result;
use crate::{ProfilesAction, ProfilesArgs};
use nmm_core::mods::{self as core_mods, ToggleModInfo};
use nmm_core::profiles as core_profiles;
use nmm_core::AppContext;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Serialize)]
struct ProfilesActionResult {
    success: bool,
    message: String,
}

pub async fn run(ctx: &AppContext, json: bool, args: ProfilesArgs) -> Result<(), String> {
    let nexus_domain = ctx
        .current_profile
        .lock()
        .map_err(|e| format!("current_profile lock poisoned: {}", e))?
        .as_ref()
        .map(|p| p.nexus_domain.clone())
        .ok_or_else(|| "尚未选择游戏，请用 `nmm games switch <domain>` 设置".to_string())?;

    match args.action {
        ProfilesAction::List => list(&nexus_domain, json),
        ProfilesAction::Load { name } => load(ctx, &nexus_domain, json, name),
        ProfilesAction::Save { name } => save(ctx, &nexus_domain, json, name),
    }
}

fn list(nexus_domain: &str, json: bool) -> Result<(), String> {
    let profiles = core_profiles::load(nexus_domain);
    let obj = profiles.as_object().cloned().unwrap_or_default();

    print_result(&profiles, json, |_| {
        if obj.is_empty() {
            return "无配置档案。`nmm profiles save <name>` 保存当前 mod 状态为档案".to_string();
        }
        let mut out = format!("共 {} 个配置档案\n", obj.len());
        out.push_str(&format!("{:<28} {:<10} {}\n", "name", "modCount", "savedAt"));
        out.push_str(&"-".repeat(80));
        out.push('\n');
        for (name, value) in &obj {
            let mod_count = value
                .get("snapshot")
                .and_then(Value::as_object)
                .map(|s| s.len())
                .unwrap_or(0);
            let saved_at = value
                .get("savedAt")
                .and_then(Value::as_str)
                .unwrap_or("-");
            out.push_str(&format!("{:<28} {:<10} {}\n", name, mod_count, saved_at));
        }
        out.trim_end().to_string()
    })
}

fn save(
    ctx: &AppContext,
    nexus_domain: &str,
    json: bool,
    name: String,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("profile 名称不能为空".to_string());
    }

    let (game_path, mods_subdir) = require_paths(ctx)?;
    let scanned = core_mods::scan_mods_internal_with_subdir(&game_path, &mods_subdir);

    let mut snapshot = Map::new();
    for m in &scanned {
        let id = m.id.clone().unwrap_or_else(|| m.folder_name.clone());
        snapshot.insert(id, Value::Bool(m.enabled));
    }
    let mut entry = Map::new();
    entry.insert("snapshot".into(), Value::Object(snapshot));
    entry.insert(
        "savedAt".into(),
        Value::String(now_iso8601()),
    );

    let mut profiles = core_profiles::load(nexus_domain);
    let obj = profiles
        .as_object_mut()
        .ok_or_else(|| "profiles 数据格式异常（非 object），手动检查 profiles_*.json".to_string())?;
    obj.insert(name.to_string(), Value::Object(entry));

    let outcome = core_profiles::save(nexus_domain, profiles);
    if outcome.get("success").and_then(Value::as_bool) != Some(true) {
        return Err(format!("保存失败：{:?}", outcome));
    }

    let result = ProfilesActionResult {
        success: true,
        message: format!("已保存 {} 个 mod 状态到档案 `{}`", scanned.len(), name),
    };
    print_result(&result, json, |r| r.message.clone())
}

fn load(
    ctx: &AppContext,
    nexus_domain: &str,
    json: bool,
    name: String,
) -> Result<(), String> {
    let name = name.trim();
    let profiles = core_profiles::load(nexus_domain);
    let snapshot = profiles
        .get(name)
        .and_then(|v| v.get("snapshot"))
        .and_then(Value::as_object)
        .ok_or_else(|| {
            let available: Vec<String> = profiles
                .as_object()
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            let mut msg = format!("未找到档案 `{}`", name);
            if !available.is_empty() {
                msg.push_str("\n可用档案：");
                for n in &available {
                    msg.push_str(&format!("\n  {}", n));
                }
            }
            msg
        })?
        .clone();

    let (game_path, mods_subdir) = require_paths(ctx)?;
    let scanned = core_mods::scan_mods_internal_with_subdir(&game_path, &mods_subdir);

    let mut toggled = 0_u32;
    for m in &scanned {
        let id = m.id.clone().unwrap_or_else(|| m.folder_name.clone());
        let Some(target) = snapshot.get(&id).and_then(Value::as_bool) else {
            continue;
        };
        if target == m.enabled {
            continue;
        }
        let info = ToggleModInfo {
            is_folder: m.is_folder,
            folder_name: m.folder_name.clone(),
            files: Some(m.files.clone()),
            enabled: m.enabled,
        };
        let outcome = core_mods::toggle_mod(&game_path, &mods_subdir, &info);
        if !outcome.success {
            return Err(format!(
                "toggle {} 失败: {}",
                m.folder_name,
                outcome.error.unwrap_or_else(|| "未知".to_string())
            ));
        }
        toggled += 1;
    }

    let result = ProfilesActionResult {
        success: true,
        message: format!(
            "已应用档案 `{}`：切换了 {} 个 mod 的状态",
            name, toggled
        ),
    };
    print_result(&result, json, |r| r.message.clone())
}

fn require_paths(ctx: &AppContext) -> Result<(String, String), String> {
    let game_path = ctx
        .game_path
        .lock()
        .map_err(|e| format!("game_path lock poisoned: {}", e))?
        .clone()
        .ok_or_else(|| "尚未选择游戏路径".to_string())?;
    let mods_subdir = ctx
        .current_profile
        .lock()
        .map_err(|e| format!("current_profile lock poisoned: {}", e))?
        .as_ref()
        .map(core_mods::current_mods_subdir)
        .unwrap_or_else(|| "mods".to_string());
    Ok((game_path, mods_subdir))
}

fn now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // 简化版 ISO 8601（不含时区精确秒），CLI 用途够用
    format!("{}Z", secs_to_iso(secs))
}

fn secs_to_iso(secs: u64) -> String {
    // 简单 Unix 秒 → ISO 8601（UTC）转换，避免引入 chrono dep
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    let (y, mo, d) = days_to_ymd(days as i64);
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", y, mo, d, h, m, s)
}

fn days_to_ymd(days_since_epoch: i64) -> (i32, u32, u32) {
    // 1970-01-01 + days_since_epoch
    let mut year = 1970_i32;
    let mut remaining = days_since_epoch;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if remaining < dy {
            break;
        }
        remaining -= dy;
        year += 1;
    }
    let months_normal: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let months_leap: [i64; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let months = if is_leap(year) { months_leap } else { months_normal };
    let mut m = 0_usize;
    while m < 12 && remaining >= months[m] {
        remaining -= months[m];
        m += 1;
    }
    (year, (m + 1) as u32, (remaining + 1) as u32)
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
