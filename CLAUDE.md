# Nexus Mod Manager — 项目指引

Tauri 2 + React 18 桌面应用，面向多游戏管理 Nexus Mods。Windows 优先（生产构建 NSIS 安装包）。

## 架构概览

- **前端 (`src/`)**：React 18 + Tailwind CSS + Webpack。入口 `src/index-tauri.jsx`，主组件 `src/App.jsx`。Tauri 命令桥接集中在 `src/tauri-api.js`（暴露为 `window.api`）。
- **后端 (`src-tauri/src/`)**：Rust，按功能切模块：`config.rs`、`mods.rs`、`nexus_api.rs`、`nexus_download.rs`、`game.rs`、`saves.rs`、`logs.rs`、`translate*.rs`、`db.rs`、`profiles.rs`。所有 `#[tauri::command]` 集中在 `lib.rs::run()` 的 `invoke_handler!` 注册。
- **共享状态 `AppState`（`lib.rs`）**：`Mutex` 包裹的当前游戏路径 / 当前 `GameProfile` / SQLite 连接 / Nexus 模组缓存 / premium 状态。
- **窗口与能力**：主窗口 label=`main` 加载 `dist-tauri/`；下载弹窗 label=`nexus-download` 加载外部 Nexus 网页，**故意不授予任何 IPC capability**（`src-tauri/capabilities/default.json` 只授权 `main`，注释明确写"External Nexus content must not receive IPC access"）。

## 开发与构建

```bash
npm install
npm run tauri:dev        # 前端 webpack + 后端 cargo 联调
npm run tauri:build      # 生产 NSIS 安装包
npm run build:tauri-fe   # 仅构建前端到 dist-tauri/
```

后端单独编译：`cd src-tauri && cargo build`。

## 提交规范

中文 + 全角冒号格式：`<type>：<描述>`，常见类型 `feat / fix / chore / docs / ci / style`，通常**不写正文**。一个 commit 对应一个功能。例：

```
feat：实现 Mod 一键全自动下载安装
fix：兼容旧版游戏配置字段命名
docs：更新 README 界面预览并补充截图脚本
```

代码注释、用户可见文案、提交消息一律用中文。

## 项目特定约束（非显然的）

### Nexus 网站

- `www.nexusmods.com` 在 Cloudflare 人机验证后面 —— 浏览器自动化工具（agent-browser headless/headed、WebFetch）一律被拦死，连 headed 模式手动勾选都会无限循环。但 app 内 WebView2 是真浏览器，能正常通过。需要排查 Nexus DOM 时只能让用户在自己浏览器的 devtools 里跑代码片段反馈。
- Nexus 下载流程：files 标签 → 点 "Manual"（旧版 `<a class="btn inline-flex">`）→ **iframe 内**渲染 "Slow download / Fast download" 选择界面 → 点 Slow download 进入 3 秒倒计时 → Nexus 自动开始下载，被 Tauri 的 `on_download` 拦截。
- 注入脚本 `nexus_download.rs::INJECTION_SCRIPT` 用 `initialization_script_for_all_frames` 在主框架和所有同源 / `about:blank` 子框架内运行；按钮匹配一律走可见文本（`nxm-button-*` 等 class 会随改版变）；必须避开 "Mod manager download" / "Vortex"（生成 nxm:// 链接，下载拦截不到）。
- 全自动下载分两路：Premium 走 `nexus_api::get_premium_download_link` 拿直链 reqwest 下载，免费账号走 webview 脚本注入；API 失败时自动回退 webview，并把 `AppState.nexus_is_premium` 缓存重置为 false。

### 配置存储

- 用户配置在 `%APPDATA%/NexusModManager/config.json`，`config::Config` 结构 `#[serde(default)]` 兼容缺字段。**注意**：`lib.rs::migrate_config_if_needed` 里有手写的 `Config { ... }` 字面量，新增字段时必须同步补齐，否则编译报 E0063。
- Nexus API Key 以**明文**存在 config.json（已知风险，未加密）。

### 多游戏数据隔离

- 当前游戏由 `Config.current_game`（`nexus_domain`）选定，对应 `Config.games` 里的 `GameConfig`。每个游戏有独立的 mods 目录、profiles 文件、翻译缓存（DB 按 domain 分）。新增游戏相关功能时务必维持这种隔离。
- 切换游戏走 `config_switch_game` 命令；前端 `App.jsx` 监听后会重扫 mods、刷新缓存等。

## 已知文件位置（高频）

- 下载流程：`src-tauri/src/nexus_download.rs`
- Nexus API：`src-tauri/src/nexus_api.rs`（含 premium 判定、download_link 调用）
- 命令注册：`src-tauri/src/lib.rs::run()`
- 前端事件监听：`src/App.jsx` 第 340 行附近（`nexus-download-state` 等事件）
- 下载进度展示：`src/components/DownloadProgress.jsx`（`PHASE_META` 决定不同 phase 的样式）
- Nexus 设置（含 API Key、弹窗可见性开关）：`src/components/NexusSettings.jsx`
