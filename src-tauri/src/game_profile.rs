use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GameProfile {
    /// Nexus Mods game domain.
    #[serde(rename = "nexusDomain", alias = "nexus_domain")]
    pub nexus_domain: String,
    /// Human-readable game name.
    #[serde(rename = "displayName", alias = "display_name")]
    pub display_name: String,
    /// Optional Steam app id for launching through Steam.
    #[serde(rename = "steamAppId", alias = "steam_app_id")]
    pub steam_app_id: Option<u64>,
    /// Optional executable file name for direct launching.
    #[serde(rename = "exeName", alias = "exe_name")]
    pub exe_name: Option<String>,
    /// Optional process name used to detect a running game process.
    #[serde(rename = "processName", alias = "process_name")]
    pub process_name: Option<String>,
    /// Optional Steam library directory name under steamapps/common.
    #[serde(rename = "steamDirName", alias = "steam_dir_name")]
    pub steam_dir_name: Option<String>,
    /// Mod directory relative to the game root.
    #[serde(rename = "modsSubdir", alias = "mods_subdir")]
    pub mods_subdir: String,
    /// Optional app data/config directory name for logs and saves.
    #[serde(rename = "appdataDirName", alias = "appdata_dir_name")]
    pub appdata_dir_name: Option<String>,
    /// Optional logs directory relative to the app data directory.
    #[serde(rename = "logsSubdir", alias = "logs_subdir")]
    pub logs_subdir: Option<String>,
    /// Whether save management is enabled for this game.
    #[serde(rename = "savesEnabled", alias = "saves_enabled")]
    pub saves_enabled: bool,
    /// Whether log browsing is enabled for this game.
    #[serde(rename = "logsEnabled", alias = "logs_enabled")]
    pub logs_enabled: bool,
    /// Whether crash analysis is enabled for this game.
    #[serde(
        rename = "crashAnalysisEnabled",
        alias = "crash_analysis_enabled"
    )]
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

#[cfg(test)]
mod tests {
    use super::GameProfile;

    #[test]
    fn serializes_profile_with_camel_case_keys() {
        let profile = GameProfile {
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
        };

        let value = serde_json::to_value(&profile).expect("profile should serialize");

        assert_eq!(value["nexusDomain"], "slaythespire2");
        assert_eq!(value["displayName"], "Slay the Spire 2");
        assert_eq!(value["steamAppId"], 2868840);
        assert!(value.get("nexus_domain").is_none());
    }

    #[test]
    fn deserializes_profile_from_legacy_snake_case_keys() {
        let profile: GameProfile = serde_json::from_value(serde_json::json!({
            "nexus_domain": "slaythespire2",
            "display_name": "Slay the Spire 2",
            "steam_app_id": 2868840,
            "exe_name": "SlayTheSpire2.exe",
            "process_name": "SlayTheSpire2",
            "steam_dir_name": "Slay the Spire 2",
            "mods_subdir": "mods",
            "appdata_dir_name": "SlayTheSpire2",
            "logs_subdir": "logs",
            "saves_enabled": true,
            "logs_enabled": true,
            "crash_analysis_enabled": true
        }))
        .expect("legacy profile should deserialize");

        assert_eq!(profile.nexus_domain, "slaythespire2");
        assert_eq!(profile.display_name, "Slay the Spire 2");
        assert_eq!(profile.steam_app_id, Some(2868840));
        assert_eq!(profile.mods_subdir, "mods");
        assert!(profile.saves_enabled);
        assert!(profile.logs_enabled);
        assert!(profile.crash_analysis_enabled);
    }
}
