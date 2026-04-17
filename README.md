<div align="center">

# Nexus Mod Manager

跨游戏的 Nexus Mods 管理器，支持浏览、下载、安装和管理 Mod。

[![Release](https://img.shields.io/github/v/release/525300887039/nexus-mod-manager?style=flat-square&color=2563eb)](https://github.com/525300887039/nexus-mod-manager/releases)
[![Stars](https://img.shields.io/github/stars/525300887039/nexus-mod-manager?style=flat-square&color=f59e0b)](https://github.com/525300887039/nexus-mod-manager)
[![License](https://img.shields.io/badge/license-MIT-2563eb?style=flat-square)](LICENSE)
![Version](https://img.shields.io/badge/version-3.0.0-111827?style=flat-square)
![Platform](https://img.shields.io/badge/platform-Windows%2010%20%2F%2011-0f172a?style=flat-square)

<br>

<img src="docs/preview-mods.png" width="90%" alt="Mod 管理界面" />

<br><br>

<img src="docs/preview-saves.png" width="90%" alt="存档管理界面" />

</div>

## 功能

- 支持任意 Nexus Mods 游戏，当前配置会按游戏隔离。
- 内置热门游戏预设：STS2、Skyrim Special Edition、Baldur's Gate 3、Stardew Valley、Cyberpunk 2077、Monster Hunter: World、Fallout 4、The Witcher 3、Elden Ring、Starfield。
- 提供 Nexus Mods 热门、最新、最近更新浏览，以及 Mod 详情、文件列表和下载入口。
- 支持 ZIP / RAR / 7Z 一键安装、拖拽安装、启用/禁用切换、卸载、备份与恢复。
- 内置翻译工作流，支持本地缓存、免费翻译接口和 OpenAI 兼容模型配置。
- 支持多游戏快速切换，切换后会刷新当前游戏的 Mod 列表、缓存和配置档案。

## 针对特定游戏的增强功能

- Slay the Spire 2：存档管理、游戏日志查看、崩溃分析、自动路径探测。
- 其他游戏：默认提供通用 Nexus 浏览、下载、安装和 Mod 管理能力。

## 安装

- 发布页：<https://github.com/525300887039/nexus-mod-manager/releases>
- 首次启动时选择当前要管理的游戏；如果未自动识别路径，可以手动指定游戏目录。
- 使用 Nexus 浏览、详情和下载功能前，需要在设置页填写并验证自己的 Nexus Mods API Key。

## 构建

```bash
npm install
npm run tauri:dev
npm run tauri:build
```

## 开发环境

- Node.js 与 npm
- Rust toolchain
- Windows C++ 构建环境，例如 Visual Studio Build Tools

## 技术栈

```text
Frontend  React 18 + Tailwind CSS + Lucide React
Desktop   Tauri v2
Backend   Rust + Tauri Commands
Storage   SQLite + 本地 JSON 配置
```

## 项目结构

```text
src/            React 前端源码
src-tauri/      Tauri / Rust 后端源码与打包配置
dist-tauri/     Tauri 前端构建产物
docs/           README 截图素材
```

## 仓库

- `origin`: <https://github.com/525300887039/nexus-mod-manager>
- `upstream`: <https://github.com/ImogeneOctaviap794/nexus-mod-manager>
