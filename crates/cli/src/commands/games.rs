//! `nmm games` 子命令组。

use crate::output::print_result;
use crate::{GamesAction, GamesArgs};
use nmm_core::config::{self as core_config, GameConfig};
use nmm_core::game_profile::GameProfile;
use nmm_core::AppContext;
use serde::Serialize;

#[derive(Serialize)]
struct GameEntry {
    domain: String,
    name: String,
    #[serde(rename = "gamePath")]
    game_path: Option<String>,
    current: bool,
}

#[derive(Serialize)]
struct GamesListResult {
    #[serde(rename = "currentGame")]
    current_game: Option<String>,
    games: Vec<GameEntry>,
}

#[derive(Serialize)]
struct GamesMutationResult {
    #[serde(rename = "currentGame")]
    current_game: Option<String>,
    message: String,
}

pub async fn run(_ctx: &AppContext, json: bool, args: GamesArgs) -> Result<(), String> {
    match args.action {
        GamesAction::List => list(json),
        GamesAction::Switch { domain } => switch(json, domain),
        GamesAction::Add { domain, path } => add(json, domain, path),
        GamesAction::Remove { domain } => remove(json, domain),
    }
}

fn switch(json: bool, domain: String) -> Result<(), String> {
    let nexus_domain = domain.trim();
    if nexus_domain.is_empty() {
        return Err("game domain cannot be empty".to_string());
    }

    let mut cfg = core_config::load_config();
    if !cfg.games.contains_key(nexus_domain) {
        let profile = GameProfile::default_for(nexus_domain)
            .ok_or_else(|| format!("未知游戏 domain: {}\n请先用 `nmm games add <domain> <path>` 配置", nexus_domain))?;
        cfg.games.insert(
            nexus_domain.to_string(),
            core_config::default_game_config(profile),
        );
    }

    cfg.current_game = Some(nexus_domain.to_string());
    let game_path = core_config::resolve_game_path_from_config(&cfg);
    core_config::persist_current_game_path(&mut cfg, game_path);
    core_config::save_config_inner(&cfg)?;

    let result = GamesMutationResult {
        current_game: Some(nexus_domain.to_string()),
        message: format!("已切换到 {}", nexus_domain),
    };
    print_result(&result, json, |r| r.message.clone())
}

fn add(json: bool, domain: String, path: String) -> Result<(), String> {
    let domain = domain.trim().to_string();
    if domain.is_empty() {
        return Err("game domain cannot be empty".to_string());
    }

    let mut cfg = core_config::load_config();
    if cfg.games.contains_key(&domain) {
        return Err(format!("该 Nexus 域名已存在：{}（如需切换请用 `nmm games switch {}`）", domain, domain));
    }

    // 优先用 preset profile；不在 preset 时让用户用 GUI 添加自定义 profile（CLI 暂不接受 profile JSON 入参）
    let profile = GameProfile::default_for(&domain)
        .ok_or_else(|| format!(
            "未在 preset 列表中找到 domain `{}`。\n\
             CLI 当前仅支持添加 preset 游戏；自定义 profile 请用 GUI 添加。",
            domain
        ))?;

    let validated_path = core_config::validate_required_game_path(Some(&path))?;
    cfg.games.insert(
        domain.clone(),
        GameConfig {
            game_path: Some(validated_path),
            profile,
        },
    );
    cfg.current_game = Some(domain.clone());
    core_config::save_config_inner(&cfg)?;

    let result = GamesMutationResult {
        current_game: Some(domain.clone()),
        message: format!("已添加并切换到 {}", domain),
    };
    print_result(&result, json, |r| r.message.clone())
}

fn remove(json: bool, domain: String) -> Result<(), String> {
    let domain = domain.trim();
    if domain.is_empty() {
        return Err("game domain cannot be empty".to_string());
    }

    let mut cfg = core_config::load_config();
    cfg.games.remove(domain);

    // 与 GUI 端 config_remove_game 行为对齐：删除后若是 preset，重置为默认配置（仍可见但 game_path 清空）
    if let Some(profile) = GameProfile::default_for(domain) {
        cfg.games.insert(
            domain.to_string(),
            core_config::default_game_config(profile),
        );
    }

    core_config::normalize_current_game(&mut cfg);
    let game_path = core_config::resolve_game_path_from_config(&cfg);
    core_config::persist_current_game_path(&mut cfg, game_path);
    core_config::save_config_inner(&cfg)?;

    let result = GamesMutationResult {
        current_game: cfg.current_game.clone(),
        message: format!("已删除 {}", domain),
    };
    print_result(&result, json, |r| r.message.clone())
}

fn list(json: bool) -> Result<(), String> {
    let cfg = core_config::load_config();
    let current_domain = core_config::current_game_domain(&cfg).map(str::to_string);
    let available = core_config::collect_available_games(&cfg);

    let games: Vec<GameEntry> = available
        .into_iter()
        .map(|profile| {
            let domain = profile.nexus_domain.clone();
            let game_path = cfg
                .games
                .get(&domain)
                .and_then(|gc| gc.game_path.clone());
            let current = current_domain.as_deref() == Some(domain.as_str());
            GameEntry {
                domain,
                name: profile.display_name,
                game_path,
                current,
            }
        })
        .collect();

    let result = GamesListResult {
        current_game: current_domain,
        games,
    };

    print_result(&result, json, |result| {
        let mut out = String::new();
        out.push_str(&format!(
            "当前游戏: {}\n",
            result
                .current_game
                .as_deref()
                .unwrap_or("(未设置)")
        ));
        out.push_str(&format!(
            "{:<20} {:<24} {}\n",
            "domain", "name", "gamePath"
        ));
        out.push_str(&"-".repeat(80));
        out.push('\n');
        for game in &result.games {
            let marker = if game.current { "*" } else { " " };
            out.push_str(&format!(
                "{} {:<18} {:<24} {}\n",
                marker,
                game.domain,
                game.name,
                game.game_path.as_deref().unwrap_or("(未设置)")
            ));
        }
        out.trim_end().to_string()
    })
}
