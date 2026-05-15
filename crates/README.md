# crates/

本目录为 Cargo workspace 的扩展位，承载从 `src-tauri/` 拆分出的纯业务库与独立 binary。

## 规划中的 sub-crate

| 目录 | 角色 | 引入的 change |
|------|------|--------------|
| `core/` | 无 Tauri 依赖的业务逻辑库（数据库、配置、mods/saves/profiles、nexus API、翻译、下载等） | `extract-core-infrastructure` 起，分多个 change 逐步搬入 |
| `cli/` | 独立 CLI binary（`nmm`），依赖 `core` + `clap` | `add-cli-binary` |

## 当前状态

本目录暂为占位，未创建任何 sub-crate。具体拆分规划记录在团队内部 OpenSpec 规格中（`openspec/specs/project-structure/spec.md`，未提交到 git 仓库）。

`src-tauri/` 在 CLI 化期间仍是 workspace 的 GUI binary 入口；逐步搬迁完成后，它会变成依赖 `core` 的薄壳。

## 跨平台路径约定

workspace 根 `Cargo.toml` 的 `members` 字段一律使用 forward slash（`crates/core`、`crates/cli`），不使用 Windows 反斜杠，确保跨平台一致。
