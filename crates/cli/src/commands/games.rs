//! `nmm games` 子命令组。

use crate::output::print_result;
use crate::{GamesAction, GamesArgs};
use nmm_core::config as core_config;
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

pub async fn run(_ctx: &AppContext, json: bool, args: GamesArgs) -> Result<(), String> {
    match args.action {
        GamesAction::List => list(json),
        GamesAction::Switch { .. }
        | GamesAction::Add { .. }
        | GamesAction::Remove { .. } => {
            Err("games switch/add/remove 将在 step 4 实现".to_string())
        }
    }
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
