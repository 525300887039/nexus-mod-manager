use crate::{
    config,
    mods::{is_supported_archive_path, smart_extract_archive},
    nexus_api, AppState,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{
    webview::{DownloadEvent, WebviewWindowBuilder},
    AppHandle, Emitter, Manager, WebviewUrl,
};

const DOWNLOAD_WINDOW_LABEL: &str = "nexus-download";
const MAIN_WINDOW_LABEL: &str = "main";
const DOWNLOAD_STATE_EVENT: &str = "nexus-download-state";
const INSTALL_SUCCESS_EVENT: &str = "nexus-install-success";
const INSTALL_ERROR_EVENT: &str = "nexus-install-error";
const DOWNLOAD_FAILED_EVENT: &str = "nexus-download-failed";
const DOWNLOAD_SAVED_EVENT: &str = "nexus-download-saved";
const API_DOWNLOAD_TIMEOUT_SECS: u64 = 600;
const API_DOWNLOAD_USER_AGENT: &str = "NexusModManager/3.0";

/// 注入到 Nexus 下载弹窗的自动化脚本。用 `initialization_script_for_all_frames`
/// 注入，故在主框架和所有同源 / about:blank 子框架内都会运行。
/// 仅操作 DOM，不依赖 Tauri IPC（外部站点故意不授予 IPC 能力）。
///
/// 设计要点：
/// - 持续轮询模型：每 500ms 巡检一次，最长 3 分钟。容忍 AJAX 延迟、Cloudflare 拦截、登录跳转。
/// - 框架感知：Nexus 把 "Slow download / Fast download" 下载界面装在 iframe 里，主框架
///   `querySelector` 进不去；故脚本在每个框架内都跑，子框架只负责"找到并点下载按钮"。
/// - 真实点击：用完整 pointer/mouse 事件序列 + `.click()`，并递归穿透 shadow DOM 查找。
/// - Cloudflare 验证页只显示提示、不做副作用，验证通过页面重载后脚本重跑。
/// - 主框架顶部注入状态横幅，子框架通过 `window.top.__nmmBanner` 转发，无需开发者工具即可排查。
const INJECTION_SCRIPT: &str = r#"
(function () {
  'use strict';
  if (window.__NMM_INIT__) { return; }
  window.__NMM_INIT__ = true;

  var inFrame = false;
  try { inFrame = (window.top !== window.self); } catch (e) { inFrame = true; }
  var host = location.hostname;
  // 顶层框架必须是 nexusmods；子框架放行 nexusmods 或 about:blank（继承父源）
  if (!inFrame && host !== 'www.nexusmods.com') { return; }
  if (inFrame && host !== 'www.nexusmods.com' && host !== '') { return; }

  var LS_KEY = 'nmm_auto_download_target';
  var MAX_RUN_MS = 180000;
  var startedAt = Date.now();
  var manualClickCount = 0;
  var lastManualClickAt = 0;
  var clickedDownloadAction = false;
  var redirectedToLogin = false;
  var navigatedToFiles = false;
  var bannerEl = null;
  var intervalId = null;

  function log() {
    try {
      console.log.apply(console, ['[NMM' + (inFrame ? '/frame' : '') + ']'].concat([].slice.call(arguments)));
    } catch (e) {}
  }

  function setBanner(text, color) {
    if (inFrame) {
      try { window.top.__nmmBanner(text, color); } catch (e) {}
      return;
    }
    try {
      var hostEl = document.body || document.documentElement;
      if (!hostEl) { return; }
      if (!bannerEl || !bannerEl.isConnected) {
        bannerEl = document.createElement('div');
        bannerEl.id = '__nmm_banner__';
        bannerEl.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:2147483647;'
          + 'padding:8px 16px;font:13px/1.5 system-ui,-apple-system,sans-serif;color:#fff;'
          + 'text-align:center;box-shadow:0 1px 8px rgba(0,0,0,.35);pointer-events:none;';
        hostEl.appendChild(bannerEl);
      }
      bannerEl.textContent = '[Nexus Mod Manager 自动下载] ' + text;
      bannerEl.style.background = color || '#2563eb';
    } catch (e) {}
  }
  if (!inFrame) { try { window.__nmmBanner = setBanner; } catch (e) {} }

  // 递归查询，穿透 shadow DOM
  function deepQuery(selector) {
    var results = [];
    function walk(root) {
      var list;
      try { list = root.querySelectorAll(selector); } catch (e) { return; }
      for (var i = 0; i < list.length; i++) { results.push(list[i]); }
      var all;
      try { all = root.querySelectorAll('*'); } catch (e) { return; }
      for (var j = 0; j < all.length; j++) {
        if (all[j].shadowRoot) { walk(all[j].shadowRoot); }
      }
    }
    walk(document);
    return results;
  }

  function normText(el) {
    return (el.textContent || '').replace(/\s+/g, ' ').trim().toLowerCase();
  }
  function isClickable(el) {
    return !!(el && !el.disabled && (!el.getAttribute || el.getAttribute('aria-disabled') !== 'true'));
  }
  // 完整事件序列 + 原生 click，尽量触发 SPA 框架的点击处理器
  function realClick(el) {
    try {
      var r = el.getBoundingClientRect();
      var o = { bubbles: true, cancelable: true, view: window,
        clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, button: 0 };
      var seq = ['pointerover', 'pointerdown', 'mousedown', 'pointerup', 'mouseup'];
      for (var i = 0; i < seq.length; i++) {
        try {
          var type = seq[i];
          var C = (type.indexOf('pointer') === 0 && window.PointerEvent) ? window.PointerEvent : MouseEvent;
          el.dispatchEvent(new C(type, o));
        } catch (e) {}
      }
    } catch (e) {}
    try { el.focus(); } catch (e) {}
    try { el.click(); } catch (e) {}
  }

  function parseTargetFromLocation() {
    var m = location.pathname.match(/^\/([^\/]+)\/mods\/(\d+)/);
    if (!m) { return null; }
    var t = { gameDomain: m[1], modId: parseInt(m[2], 10), fileId: null };
    var fm = location.search.match(/[?&]file_id=(\d+)/);
    if (fm) { t.fileId = parseInt(fm[1], 10); }
    return t;
  }
  function saveTarget(t) {
    try { localStorage.setItem(LS_KEY, JSON.stringify(t)); } catch (e) {}
  }
  function loadSavedTarget() {
    try { var r = localStorage.getItem(LS_KEY); return r ? JSON.parse(r) : null; }
    catch (e) { return null; }
  }
  // URL 优先；登录跳回若丢了 file_id，用 localStorage 里保存的补回
  function readTarget() {
    var u = parseTargetFromLocation();
    var s = loadSavedTarget();
    if (u) {
      if (u.fileId == null && s && s.modId === u.modId && s.fileId != null) {
        u.fileId = s.fileId;
      }
      saveTarget(u);
      return u;
    }
    return s;
  }
  function filePageUrl(t) {
    var u = 'https://www.nexusmods.com/' + t.gameDomain + '/mods/' + t.modId + '?tab=files';
    if (t.fileId) { u += '&file_id=' + t.fileId; }
    return u;
  }

  // Cloudflare 人机验证 / 非正常页面识别
  function isChallengePage() {
    var t = (document.title || '').toLowerCase();
    if (t.indexOf('just a moment') !== -1 || t.indexOf('请稍候') !== -1
        || t.indexOf('attention required') !== -1 || t.indexOf('checking your browser') !== -1) {
      return true;
    }
    if (document.querySelector('#challenge-running, #cf-challenge-running, .cf-turnstile, script[src*="challenge-platform"]')) {
      return true;
    }
    return false;
  }

  function isModPage() {
    return /\/mods\/\d+/.test(location.pathname);
  }

  // 登录态：明确登出信号才算登出，无法判定时按已登录处理（避免误入登录循环）
  function isLoggedOut() {
    if (document.querySelector('a[href*="/users/myaccount"], a[href*="sign_out"], a[href*="logout"], #nav-account-button, .header-personal-menu, [data-e2eid*="user-menu"]')) {
      return false;
    }
    if (document.querySelector('a[href*="auth/sign_in"], a[href*="account/sign-in"], a[href*="users.nexusmods.com/auth"]')) {
      return true;
    }
    return false;
  }

  function isModManagerEl(el) {
    var href = (el.getAttribute && el.getAttribute('href')) || '';
    var t = normText(el);
    return t.indexOf('mod manager') !== -1 || t.indexOf('vortex') !== -1
      || href.indexOf('nmm=1') !== -1 || href.indexOf('nmm%3D1') !== -1;
  }

  // "Slow download" / "Fast download" —— 真正触发文件下载的按钮（可能在 iframe / shadow DOM 里）
  function findDownloadActionButton() {
    var nodes = deepQuery('button, a, [role="button"]');
    var slow = null, fast = null;
    for (var i = 0; i < nodes.length; i++) {
      var t = normText(nodes[i]);
      if (!t || t.indexOf('download') === -1 || !isClickable(nodes[i])) { continue; }
      if (t.indexOf('slow') !== -1) { slow = slow || nodes[i]; }
      else if (t.indexOf('fast') !== -1) { fast = fast || nodes[i]; }
    }
    return slow || fast; // 免费账号优先 slow；premium 回退到 webview 时用 fast
  }

  // 文件列表里的 "Manual download" / "Manual" 按钮（排除 Mod manager / Vortex）
  function findManualDownloadButton(target) {
    var nodes = deepQuery('button, a, [role="button"]');
    var matches = [];
    for (var i = 0; i < nodes.length; i++) {
      var el = nodes[i];
      if (isModManagerEl(el)) { continue; }
      var t = normText(el);
      if (t === 'manual download' || t === 'manual'
          || (t.indexOf('manual') !== -1 && t.indexOf('download') !== -1)) {
        matches.push(el);
      }
    }
    if (matches.length === 0) { return null; }
    // 按 file_id 精确定位到目标文件
    if (target && target.fileId != null) {
      var fid = String(target.fileId);
      for (var j = 0; j < matches.length; j++) {
        var node = matches[j];
        for (var d = 0; d < 16 && node; d++) {
          var did = (node.getAttribute && (node.getAttribute('data-id')
            || node.getAttribute('data-file-id'))) || '';
          var idAttr = node.id || '';
          var nh = (node.getAttribute && node.getAttribute('href')) || '';
          if (did === fid || (idAttr && idAttr.indexOf(fid) !== -1)
              || nh.indexOf('file_id=' + fid) !== -1 || nh.indexOf('id=' + fid) !== -1) {
            return matches[j];
          }
          node = node.parentElement;
        }
      }
    }
    for (var k = 0; k < matches.length; k++) {
      if (matches[k].offsetParent !== null) { return matches[k]; }
    }
    return matches[0];
  }

  function stop() {
    if (intervalId) { clearInterval(intervalId); intervalId = null; }
  }

  function tick() {
    try {
      if (Date.now() - startedAt > MAX_RUN_MS) {
        log('watcher timeout, stop');
        stop();
        return;
      }

      if (isChallengePage()) {
        setBanner('正在通过站点安全验证，请稍候（如出现验证勾选框请手动勾选）...', '#d97706');
        return;
      }

      // 1. Slow/Fast download 按钮：真正触发文件下载，最高优先级。
      //    在任何框架里找到就点（含 iframe / shadow DOM），单击一次。
      var dlBtn = findDownloadActionButton();
      if (dlBtn) {
        if (!clickedDownloadAction) {
          clickedDownloadAction = true;
          setBanner('已触发下载，正在传输文件（如有倒计时请稍候）...', '#16a34a');
          log('click download action button', normText(dlBtn));
          realClick(dlBtn);
        }
        return;
      }

      // 以下导航/登录/Manual 点击逻辑只在顶层框架执行
      if (inFrame) { return; }

      var target = readTarget();
      if (!target) { return; }

      var onModPage = isModPage();
      var onFilesTab = /[?&]tab=files/.test(location.search);
      var looksLikeHome = location.pathname === '/' || location.pathname === '';

      // 2. 在 mod 页但缺 tab=files，或被登录跳回到首页 → 跳到文件页
      if ((onModPage && !onFilesTab) || looksLikeHome) {
        if (!navigatedToFiles) {
          navigatedToFiles = true;
          setBanner('正在打开文件下载页...', '#2563eb');
          log('navigate to files tab', filePageUrl(target));
          location.href = filePageUrl(target);
        }
        return;
      }
      // 在下载流程的其它中间页（非 mod 页、非首页）→ 等待，不要跳走
      if (!onModPage) {
        setBanner('正在加载下载页面...', '#2563eb');
        return;
      }

      // 3. 未登录 → 跳登录页（登录后 Nexus 自带跳回机制会回到本页）
      if (isLoggedOut()) {
        if (!redirectedToLogin) {
          redirectedToLogin = true;
          saveTarget(target);
          setBanner('请在本窗口登录 Nexus 账号，登录后将自动继续下载', '#dc2626');
          var loginLink = document.querySelector('a[href*="auth/sign_in"], a[href*="account/sign-in"]');
          log('not logged in, go to login', loginLink && loginLink.href);
          if (loginLink && loginLink.href) { location.href = loginLink.href; }
        }
        return;
      }

      // 4. 已登录 → 找 Manual download 按钮并点击（点击后会出现 Slow/Fast download 界面）
      //    按钮可能是 SPA 组件，点击不一定跳转，故最多重试 4 次、间隔 3 秒
      var manualBtn = findManualDownloadButton(target);
      if (manualBtn) {
        if (manualClickCount < 4
            && (manualClickCount === 0 || Date.now() - lastManualClickAt > 3000)) {
          manualClickCount += 1;
          lastManualClickAt = Date.now();
          setBanner('已找到下载按钮，正在自动下载，请勿关闭窗口...', '#2563eb');
          log('click manual download (attempt ' + manualClickCount + ')');
          realClick(manualBtn);
        }
        return;
      }

      setBanner('正在查找下载按钮（文件列表加载中）...', '#2563eb');
    } catch (e) {
      log('tick error', e);
    }
  }

  function start() {
    log('injected on', location.href);
    if (!inFrame) { setBanner('自动下载已启动...', '#2563eb'); }
    tick();
    intervalId = setInterval(tick, 500);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }
})();
"#;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NexusDownloadStatePayload {
    phase: &'static str,
    message: String,
    file_name: Option<String>,
}

fn current_game_path(app: &AppHandle) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    state
        .game_path
        .lock()
        .map(|game_path| game_path.clone())
        .map_err(|e| format!("游戏路径状态锁已损坏: {}", e))
}

fn current_game_domain(app: &AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let domain = state
        .current_profile
        .lock()
        .map_err(|e| format!("game profile state lock poisoned: {}", e))?
        .as_ref()
        .map(|profile| profile.nexus_domain.clone())
        .ok_or_else(|| "please select a game first".to_string())?;
    Ok(domain)
}

fn emit_download_state(
    app: &AppHandle,
    phase: &'static str,
    message: impl Into<String>,
    file_name: Option<String>,
) {
    let payload = NexusDownloadStatePayload {
        phase,
        message: message.into(),
        file_name,
    };
    let _ = app.emit_to(MAIN_WINDOW_LABEL, DOWNLOAD_STATE_EVENT, payload);
}

fn extract_file_name(path: &Path) -> Option<String> {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
}

fn decode_file_name_from_url(url: &tauri::webview::Url) -> String {
    url.path_segments()
        .and_then(|segments| segments.last())
        .filter(|segment| !segment.trim().is_empty())
        .map(|segment| {
            urlencoding::decode(segment)
                .map(|decoded| decoded.into_owned())
                .unwrap_or_else(|_| segment.to_string())
        })
        .unwrap_or_else(|| "mod-download".to_string())
}

fn make_unique_destination(download_dir: &Path, file_name: &str) -> PathBuf {
    let candidate = download_dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let file_path = Path::new(file_name);
    let stem = file_path
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "mod".to_string());
    let extension = file_path
        .extension()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut index = 1;
    loop {
        let next_name = if extension.is_empty() {
            format!("{stem} ({index})")
        } else {
            format!("{stem} ({index}).{extension}")
        };
        let next_candidate = download_dir.join(next_name);
        if !next_candidate.exists() {
            return next_candidate;
        }
        index += 1;
    }
}

/// 把已下载到本地的归档文件解压安装到游戏 mods 目录，并 emit 对应状态事件。
/// webview 拦截下载（`close_window = true`，安装成功后关闭下载窗口）与
/// Premium API 直链下载（`close_window = false`，无窗口）共用此函数。
fn install_downloaded_archive(
    app: &AppHandle,
    archive_path: &Path,
    file_name: &str,
    close_window: bool,
) {
    if !archive_path.exists() {
        let message = format!("下载完成后未找到文件: {}", file_name);
        emit_download_state(app, "error", &message, Some(file_name.to_string()));
        let _ = app.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, message);
        return;
    }

    if !is_supported_archive_path(archive_path) {
        let message = format!(
            "{} 已下载到临时目录，但不是支持自动安装的归档文件",
            file_name
        );
        emit_download_state(app, "success", &message, Some(file_name.to_string()));
        let _ = app.emit_to(MAIN_WINDOW_LABEL, DOWNLOAD_SAVED_EVENT, message);
        return;
    }

    emit_download_state(
        app,
        "installing",
        format!("正在安装 {}", file_name),
        Some(file_name.to_string()),
    );

    let game_path = match current_game_path(app) {
        Ok(game_path) => game_path,
        Err(message) => {
            emit_download_state(app, "error", &message, Some(file_name.to_string()));
            let _ = app.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, message);
            return;
        }
    };
    let Some(game_path) = game_path else {
        let message = "尚未设置游戏目录，无法自动安装".to_string();
        emit_download_state(app, "error", &message, Some(file_name.to_string()));
        let _ = app.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, message);
        return;
    };

    let mods_dir = Path::new(&game_path).join("mods");
    if let Err(error) = fs::create_dir_all(&mods_dir) {
        let message = format!("无法创建 Mod 安装目录 {}: {}", mods_dir.display(), error);
        emit_download_state(app, "error", &message, Some(file_name.to_string()));
        let _ = app.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, message);
        return;
    }

    let archive_path_str = archive_path.to_string_lossy().to_string();
    match smart_extract_archive(&archive_path_str, &mods_dir) {
        Ok(_) => {
            emit_download_state(
                app,
                "success",
                format!("Mod 已安装: {}", file_name),
                Some(file_name.to_string()),
            );
            let _ = app.emit_to(MAIN_WINDOW_LABEL, INSTALL_SUCCESS_EVENT, file_name.to_string());
            if close_window {
                if let Some(window) = app.get_webview_window(DOWNLOAD_WINDOW_LABEL) {
                    let _ = window.close();
                }
            }
        }
        Err(error) => {
            emit_download_state(app, "error", &error, Some(file_name.to_string()));
            let _ = app.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, error);
        }
    }
}

/// 取 Mod 的首选文件 id（MAIN 分类优先，否则第一个），用于 Premium 路径未指定文件时补齐。
async fn resolve_preferred_file_id(game_domain: &str, mod_id: u64) -> Result<u64, String> {
    let files = nexus_api::get_mod_files_raw(game_domain, mod_id).await?;
    files
        .iter()
        .find(|file| file.category_name.eq_ignore_ascii_case("MAIN"))
        .or_else(|| files.first())
        .map(|file| file.file_id)
        .ok_or_else(|| "该 Mod 没有可下载的文件".to_string())
}

/// Premium 会员路径：调官方接口拿直链 → reqwest 下载到临时目录 → 复用安装逻辑。
async fn download_via_api(
    app: &AppHandle,
    game_domain: &str,
    mod_id: u64,
    file_id: u64,
) -> Result<(), String> {
    emit_download_state(app, "preparing", "正在获取下载链接...", None);
    let download_url =
        nexus_api::get_premium_download_link(game_domain, mod_id, file_id).await?;

    let parsed_url = download_url
        .parse::<tauri::webview::Url>()
        .map_err(|e| format!("无效的下载地址: {}", e))?;
    let file_name = decode_file_name_from_url(&parsed_url);

    let download_dir = std::env::temp_dir().join("nexus-mod-downloads");
    fs::create_dir_all(&download_dir).map_err(|e| format!("无法创建临时下载目录: {}", e))?;
    let destination = make_unique_destination(&download_dir, &file_name);

    emit_download_state(
        app,
        "downloading",
        format!("正在下载 {}", file_name),
        Some(file_name.clone()),
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(API_DOWNLOAD_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("初始化下载客户端失败: {}", e))?;
    let response = client
        .get(download_url.as_str())
        .header(reqwest::header::USER_AGENT, API_DOWNLOAD_USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("连接下载服务器失败: {}", e))?;
    if !response.status().is_success() {
        return Err(format!("下载失败 (HTTP {})", response.status()));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取下载内容失败: {}", e))?;
    fs::write(&destination, &bytes).map_err(|e| format!("写入下载文件失败: {}", e))?;

    install_downloaded_archive(app, &destination, &file_name, false);
    Ok(())
}

/// Rust 侧导航监听：登录页 → 强制显示窗口并提示登录；回到 mod 页 → 提示自动下载中。
fn handle_navigation(app: &AppHandle, url: &tauri::webview::Url) {
    match url.host_str().unwrap_or("") {
        "users.nexusmods.com" => {
            // 即使设置为静默，也必须显示窗口让用户登录
            if let Some(window) = app.get_webview_window(DOWNLOAD_WINDOW_LABEL) {
                let _ = window.show();
                let _ = window.set_focus();
            }
            emit_download_state(
                app,
                "login-required",
                "请在弹窗中登录 Nexus 账号，登录后将自动继续下载",
                None,
            );
        }
        "www.nexusmods.com" if url.path().contains("/mods/") => {
            emit_download_state(app, "auto-downloading", "正在自动下载 Mod，请稍候...", None);
        }
        _ => {}
    }
}

/// 免费账号路径：打开（或复用）Nexus 下载弹窗，注入自动化脚本驱动下载。
fn open_download_webview(
    app: &AppHandle,
    game_domain: &str,
    mod_id: u64,
    file_id: Option<u64>,
    visible: bool,
) -> Result<(), String> {
    let url = if let Some(file_id) = file_id {
        format!("https://www.nexusmods.com/{game_domain}/mods/{mod_id}?tab=files&file_id={file_id}")
    } else {
        format!("https://www.nexusmods.com/{game_domain}/mods/{mod_id}?tab=files")
    };

    let external_url = url
        .parse()
        .map_err(|error| format!("无效的 Nexus 下载地址: {}", error))?;

    emit_download_state(app, "auto-downloading", "正在自动下载 Mod，请稍候...", None);

    if let Some(existing) = app.get_webview_window(DOWNLOAD_WINDOW_LABEL) {
        existing
            .navigate(external_url)
            .map_err(|error| format!("切换 Nexus 下载页面失败: {}", error))?;
        if visible {
            let _ = existing.show();
            let _ = existing.set_focus();
        } else {
            let _ = existing.hide();
        }
        return Ok(());
    }

    let download_app = app.clone();
    let nav_app = app.clone();

    let download_window = WebviewWindowBuilder::new(
        app,
        DOWNLOAD_WINDOW_LABEL,
        WebviewUrl::External(external_url.clone()),
    )
    .title("Nexus Mods - 自动下载")
    .inner_size(1200.0, 800.0)
    .center()
    .visible(visible)
    .initialization_script_for_all_frames(INJECTION_SCRIPT)
    .on_navigation(move |url| {
        handle_navigation(&nav_app, url);
        true
    })
    .on_download(move |_webview, event| {
        let app_handle = &download_app;
        match event {
            DownloadEvent::Requested { url, destination } => {
                let download_dir = std::env::temp_dir().join("nexus-mod-downloads");
                if let Err(error) = fs::create_dir_all(&download_dir) {
                    let message = format!("无法创建临时下载目录: {}", error);
                    emit_download_state(app_handle, "error", &message, None);
                    let _ = app_handle.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, message);
                    return false;
                }

                let file_name = decode_file_name_from_url(&url);
                let destination_path = make_unique_destination(&download_dir, &file_name);
                *destination = destination_path;

                emit_download_state(
                    app_handle,
                    "downloading",
                    format!("正在下载 {}", file_name),
                    Some(file_name),
                );
            }
            DownloadEvent::Finished { url, path, success } => {
                let file_name = path
                    .as_deref()
                    .and_then(extract_file_name)
                    .unwrap_or_else(|| decode_file_name_from_url(&url));

                if !success {
                    let message = format!("下载失败: {}", file_name);
                    emit_download_state(app_handle, "error", &message, Some(file_name.clone()));
                    let _ = app_handle.emit_to(MAIN_WINDOW_LABEL, DOWNLOAD_FAILED_EVENT, message);
                    return true;
                }

                let Some(archive_path) = path else {
                    let message =
                        "下载已完成，但没有返回本地文件路径，无法自动安装".to_string();
                    emit_download_state(app_handle, "error", &message, Some(file_name.clone()));
                    let _ = app_handle.emit_to(MAIN_WINDOW_LABEL, INSTALL_ERROR_EVENT, message);
                    return true;
                };

                install_downloaded_archive(app_handle, &archive_path, &file_name, true);
            }
            _ => {}
        }

        true
    })
    .build()
    .map_err(|error| format!("创建 Nexus 下载窗口失败: {}", error))?;

    // debug 构建下自动打开开发者工具，便于查看注入脚本的 [NMM] 日志
    #[cfg(debug_assertions)]
    download_window.open_devtools();
    #[cfg(not(debug_assertions))]
    let _ = &download_window;

    Ok(())
}

/// 全自动下载安装统一入口：Premium 走 API 直链，免费账号/无 Key 走 webview 注入脚本。
#[tauri::command]
pub async fn nexus_start_download(
    app: AppHandle,
    mod_id: u64,
    file_id: Option<u64>,
) -> Result<(), String> {
    let game_domain = current_game_domain(&app)?;

    let is_premium = nexus_api::ensure_premium_status(&app).await.unwrap_or(false);
    if is_premium {
        let resolved_file_id = match file_id {
            Some(id) => Some(id),
            None => resolve_preferred_file_id(&game_domain, mod_id).await.ok(),
        };
        if let Some(resolved_file_id) = resolved_file_id {
            match download_via_api(&app, &game_domain, mod_id, resolved_file_id).await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    eprintln!("Premium API 下载失败，回退到浏览器下载: {}", error);
                    if let Ok(mut premium) = app.state::<AppState>().nexus_is_premium.lock() {
                        *premium = Some(false);
                    }
                    emit_download_state(
                        &app,
                        "preparing",
                        "直链下载失败，切换为浏览器下载方式...",
                        None,
                    );
                }
            }
        }
    }

    let visible = config::config_get_nexus_download_visible();
    open_download_webview(&app, &game_domain, mod_id, file_id, visible)
}
