//! GUI 的 `--headless-download` 模式入口。
//!
//! CLI 端 `nmm nexus download` 在 Premium 直链不可用 / 失败时 spawn
//! `nexus-mod-manager.exe --headless-download <mod-id> [...]` 作为子进程，
//! 子进程在本模块内：
//! - 不创建主窗口、不加载前端
//! - 创建 `nexus-download` webview 跑既有的 INJECTION_SCRIPT + on_download
//! - 把每个 NexusDownloadEvent 序列化成 JSON Lines 写到 stdout
//! - 下载完成（success / error）后 `app.exit(<code>)`
//!
//! Section 1 实现进度：仅 argv 解析；runtime / 下载流程在后续 section 填充。

/// `--headless-download` argv 解析结果。
#[derive(Debug, Clone)]
pub struct HeadlessOptions {
    pub mod_id: u64,
    pub file_id: Option<u64>,
    pub game_domain: Option<String>,
}

/// 从 std::env::args() 解析 headless 模式参数。
///
/// 形态：`nexus-mod-manager.exe --headless-download <mod-id> [--file-id <id>] [--game-domain <domain>]`
///
/// 不依赖 clap（增加 ~3s 编译时间收益不值，手动解析 20 行够用）。
fn parse_args(args: &[String]) -> Result<HeadlessOptions, String> {
    // 跳过 argv[0]，定位 --headless-download
    let mut iter = args.iter().skip(1);
    let mut mod_id: Option<u64> = None;
    let mut file_id: Option<u64> = None;
    let mut game_domain: Option<String> = None;
    let mut seen_flag = false;

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--headless-download" => {
                seen_flag = true;
                let Some(value) = iter.next() else {
                    return Err(
                        "--headless-download 需要 mod-id 参数（用法：nexus-mod-manager.exe --headless-download <mod-id> [--file-id <id>] [--game-domain <domain>]）"
                            .to_string(),
                    );
                };
                mod_id = Some(value.parse::<u64>().map_err(|_| {
                    format!("mod-id 参数必须是无符号整数，实际收到 `{}`", value)
                })?);
            }
            "--file-id" => {
                let Some(value) = iter.next() else {
                    return Err("--file-id 需要一个无符号整数参数".to_string());
                };
                file_id = Some(value.parse::<u64>().map_err(|_| {
                    format!("--file-id 必须是无符号整数，实际收到 `{}`", value)
                })?);
            }
            "--game-domain" => {
                let Some(value) = iter.next() else {
                    return Err("--game-domain 需要一个 domain 参数（如 `stalker2`）".to_string());
                };
                game_domain = Some(value.clone());
            }
            _ => {
                // 未知 flag：忽略（容忍未来扩展 / 调试加的 flag）
            }
        }
    }

    if !seen_flag {
        return Err("未识别到 --headless-download flag（内部错误）".to_string());
    }
    let Some(mod_id) = mod_id else {
        return Err("--headless-download 需要 mod-id 参数".to_string());
    };

    Ok(HeadlessOptions {
        mod_id,
        file_id,
        game_domain,
    })
}

/// headless 模式的 `main`。返回值是进程退出码：
/// - 0：下载并安装成功
/// - 1：下载或安装失败
/// - 2：argv 解析错误
pub fn main(args: Vec<String>) -> i32 {
    let opts = match parse_args(&args) {
        Ok(opts) => opts,
        Err(error) => {
            eprintln!("[headless-download] {}", error);
            return 2;
        }
    };

    // TODO（Section 2-3）：构造 tauri::Builder + StdoutJsonReporter + 跑下载流程
    eprintln!(
        "[headless-download] (WIP) parsed opts: mod_id={} file_id={:?} game_domain={:?}",
        opts.mod_id, opts.file_id, opts.game_domain
    );
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn av(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_minimal() {
        let opts =
            parse_args(&av(&["nexus-mod-manager.exe", "--headless-download", "12345"])).unwrap();
        assert_eq!(opts.mod_id, 12345);
        assert!(opts.file_id.is_none());
        assert!(opts.game_domain.is_none());
    }

    #[test]
    fn parse_all_flags() {
        let opts = parse_args(&av(&[
            "nexus-mod-manager.exe",
            "--headless-download",
            "12345",
            "--file-id",
            "67890",
            "--game-domain",
            "stalker2",
        ]))
        .unwrap();
        assert_eq!(opts.mod_id, 12345);
        assert_eq!(opts.file_id, Some(67890));
        assert_eq!(opts.game_domain.as_deref(), Some("stalker2"));
    }

    #[test]
    fn parse_missing_mod_id() {
        let err = parse_args(&av(&["nexus-mod-manager.exe", "--headless-download"])).unwrap_err();
        assert!(err.contains("mod-id"));
    }

    #[test]
    fn parse_non_numeric_mod_id() {
        let err = parse_args(&av(&[
            "nexus-mod-manager.exe",
            "--headless-download",
            "abc",
        ]))
        .unwrap_err();
        assert!(err.contains("无符号整数"));
    }

    #[test]
    fn parse_ignores_unknown_flag() {
        let opts = parse_args(&av(&[
            "nexus-mod-manager.exe",
            "--headless-download",
            "12345",
            "--some-future-flag",
            "value",
        ]))
        .unwrap();
        assert_eq!(opts.mod_id, 12345);
    }
}
