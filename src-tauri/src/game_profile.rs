use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GameProfile {
    /// Nexus Mods game domain.
    pub nexus_domain: String,
    /// Human-readable game name.
    pub display_name: String,
    /// Optional Steam app id for launching through Steam.
    pub steam_app_id: Option<u64>,
    /// Optional executable file name for direct launching.
    pub exe_name: Option<String>,
    /// Optional process name used to detect a running game process.
    pub process_name: Option<String>,
    /// Optional Steam library directory name under steamapps/common.
    pub steam_dir_name: Option<String>,
    /// Mod directory relative to the game root.
    pub mods_subdir: String,
    /// Optional app data/config directory name for logs and saves.
    pub appdata_dir_name: Option<String>,
    /// Optional logs directory relative to the app data directory.
    pub logs_subdir: Option<String>,
    /// Whether save management is enabled for this game.
    pub saves_enabled: bool,
    /// Whether log browsing is enabled for this game.
    pub logs_enabled: bool,
    /// Whether crash analysis is enabled for this game.
    pub crash_analysis_enabled: bool,
}

impl GameProfile {
    pub fn default_for(nexus_domain: &str) -> Option<Self> {
        preset_games()
            .into_iter()
            .find(|profile| profile.nexus_domain == nexus_domain)
    }
}

fn preset_game(
    nexus_domain: &str,
    display_name: &str,
    steam_app_id: u64,
    exe_name: &str,
    process_name: &str,
    steam_dir_name: &str,
    mods_subdir: &str,
) -> GameProfile {
    GameProfile {
        nexus_domain: nexus_domain.to_string(),
        display_name: display_name.to_string(),
        steam_app_id: Some(steam_app_id),
        exe_name: Some(exe_name.to_string()),
        process_name: Some(process_name.to_string()),
        steam_dir_name: Some(steam_dir_name.to_string()),
        mods_subdir: mods_subdir.to_string(),
        appdata_dir_name: None,
        logs_subdir: None,
        saves_enabled: false,
        logs_enabled: false,
        crash_analysis_enabled: false,
    }
}

pub fn preset_games() -> Vec<GameProfile> {
    vec![
        GameProfile {
            nexus_domain: "slaythespire2".to_string(),
            display_name: "Slay the Spire 2".to_string(),
            steam_app_id: Some(2868840),
            exe_name: Some("SlayTheSpire2.exe".to_string()),
            process_name: Some("SlayTheSpire2".to_string()),
            steam_dir_name: Some("Slay the Spire 2".to_string()),
            mods_subdir: "mods".to_string(),
            appdata_dir_name: Some("SlayTheSpire2".to_string()),
            logs_subdir: Some("logs".to_string()),
            saves_enabled: true,
            logs_enabled: true,
            crash_analysis_enabled: true,
        },
        preset_game(
            "skyrimspecialedition",
            "Skyrim Special Edition",
            489830,
            "SkyrimSE.exe",
            "SkyrimSE",
            "Skyrim Special Edition",
            "Data",
        ),
        preset_game(
            "baldursgate3",
            "Baldur's Gate 3",
            1086940,
            "bg3.exe",
            "bg3",
            "Baldur's Gate 3",
            "Mods",
        ),
        preset_game(
            "stardewvalley",
            "Stardew Valley",
            413150,
            "Stardew Valley.exe",
            "Stardew Valley",
            "Stardew Valley",
            "Mods",
        ),
        preset_game(
            "cyberpunk2077",
            "Cyberpunk 2077",
            1091500,
            "Cyberpunk2077.exe",
            "Cyberpunk2077",
            "Cyberpunk 2077",
            "archive\\pc\\mod",
        ),
        preset_game(
            "monsterhunterworld",
            "Monster Hunter: World",
            582010,
            "MonsterHunterWorld.exe",
            "MonsterHunterWorld",
            "Monster Hunter World",
            "nativePC",
        ),
        preset_game(
            "fallout4",
            "Fallout 4",
            377160,
            "Fallout4.exe",
            "Fallout4",
            "Fallout 4",
            "Data",
        ),
        preset_game(
            "witcher3",
            "The Witcher 3",
            292030,
            "witcher3.exe",
            "witcher3",
            "The Witcher 3",
            "mods",
        ),
        preset_game(
            "eldenring",
            "Elden Ring",
            1245620,
            "eldenring.exe",
            "eldenring",
            "ELDEN RING",
            "mods",
        ),
        preset_game(
            "starfield",
            "Starfield",
            1716740,
            "Starfield.exe",
            "Starfield",
            "Starfield",
            "Data",
        ),
    ]
}
