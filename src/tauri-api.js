// Tauri API bridge - replaces Electron's preload.js
// Provides the same window.api interface using Tauri invoke

const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const appWindow = getCurrentWindow();

// Game state polling
let gameStateCallback = null;
let gameExitedCallback = null;
let pollingInterval = null;
let lastGameState = 'idle';

function startGameStatePolling() {
  if (pollingInterval) return;
  pollingInterval = setInterval(async () => {
    try {
      const state = await invoke('game_get_state');
      if (state !== lastGameState) {
        const prevState = lastGameState;
        lastGameState = state;
        if (gameStateCallback) gameStateCallback(state);
        // Detect game exit
        if (prevState === 'running' && state === 'idle') {
          if (gameExitedCallback) gameExitedCallback({ quick: false });
        }
        if (prevState === 'launching' && state === 'idle') {
          if (gameExitedCallback) gameExitedCallback({ quick: true });
        }
      }
      // Stop polling if idle
      if (state === 'idle' && pollingInterval) {
        clearInterval(pollingInterval);
        pollingInterval = null;
      }
    } catch (e) {
      // ignore polling errors
    }
  }, 2000);
}

window.api = {
  // App
  init: () => invoke('app_init'),
  selectGamePath: () => invoke('app_select_game_path'),

  // Window
  minimize: () => invoke('window_minimize'),
  maximize: () => invoke('window_maximize'),
  close: () => invoke('window_close'),

  // Mods
  scanMods: () => invoke('mods_scan'),
  toggleMod: (modInfo) => invoke('mods_toggle', { modInfo: { isFolder: modInfo.isFolder, folderName: modInfo.folderName, files: modInfo.files, enabled: modInfo.enabled } }),
  uninstallMod: (modInfo) => invoke('mods_uninstall', { modInfo: { isFolder: modInfo.isFolder, folderName: modInfo.folderName, files: modInfo.files, enabled: modInfo.enabled } }),
  installMod: () => invoke('mods_install'),
  installDrop: (filePaths) => invoke('mods_install_drop', { filePaths }),
  backupMods: () => invoke('mods_backup'),
  restoreMods: () => invoke('mods_restore'),

  // Shell
  openModsDir: () => invoke('shell_open_mods_dir'),
  openGameDir: () => invoke('shell_open_game_dir'),
  openLogsDir: () => invoke('shell_open_logs_dir'),
  openSavesDir: () => invoke('shell_open_saves_dir'),
  openUrl: (url) => invoke('shell_open_url', { url }),

  // Game
  launchGame: async () => {
    const result = await invoke('game_launch');
    if (result.success) {
      lastGameState = 'launching';
      startGameStatePolling();
    }
    return result;
  },
  getGameState: () => invoke('game_get_state'),
  getGameVersion: () => invoke('game_get_version'),
  analyzeCrash: () => invoke('game_analyze_crash'),
  onGameStateChanged: (cb) => { gameStateCallback = cb; },
  onGameExited: (cb) => { gameExitedCallback = cb; },

  // Logs
  getLatestLogs: () => invoke('logs_get_latest'),
  readLog: (fileName) => invoke('logs_read', { fileName }),

  // Profiles
  loadProfiles: () => invoke('profiles_load'),
  saveProfiles: (profiles) => invoke('profiles_save', { profiles }),

  // Translate
  translateText: (text) => invoke('translate_text', { text }),
  translateSmart: (text) => invoke('translate_smart', { text }),
  translateLlm: (text) => invoke('translate_llm', { text }),
  loadLlmConfig: () => invoke('translate_llm_config_load'),
  saveLlmConfig: (config) => invoke('translate_llm_config_save', { config }),
  getCachedTranslation: (text) => invoke('translation_cache_get', { sourceText: text }),
  setCachedTranslation: (text, translated, provider) => invoke('translation_cache_set', { sourceText: text, translated, provider }),
  batchGetTranslations: (texts) => invoke('translation_cache_batch_get', { texts }),
  getCacheCount: () => invoke('translation_cache_count'),
  clearTranslationCache: () => invoke('translation_cache_clear'),
  loadTranslations: () => invoke('translations_load'),
  saveTranslations: (data) => invoke('translations_save', { data }),
  loadNexusTranslations: () => invoke('nexus_translations_load'),
  saveNexusTranslations: (data) => invoke('nexus_translations_save', { data }),

  // Nexus API
  nexusValidateKey: (key) => invoke('nexus_validate_key', { key }),
  nexusGetTrending: () => invoke('nexus_get_trending'),
  nexusGetLatestAdded: () => invoke('nexus_get_latest_added'),
  nexusGetLatestUpdated: () => invoke('nexus_get_latest_updated'),
  nexusGetMod: (modId) => invoke('nexus_get_mod', { modId }),
  nexusGetModFiles: (modId) => invoke('nexus_get_mod_files', { modId }),
  nexusFindModByName: (name) => invoke('nexus_find_mod_by_name', { name }),
  openNexusDownload: (modId, fileId) => invoke('nexus_open_download_page', { modId, fileId }),
  saveNexusKey: (key) => invoke('config_save_nexus_key', { key }),
  getNexusKey: () => invoke('config_get_nexus_key'),

  // Saves
  scanSaves: () => invoke('saves_scan'),
  exportSave: (opts) => invoke('saves_export', { opts }),
  importSave: (opts) => invoke('saves_import', { opts }),
  deleteBackup: (backupPath) => invoke('saves_delete_backup', { backupPath }),
};
