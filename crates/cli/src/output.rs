//! 统一的 stdout 输出 helper：人类模式直接打 text；JSON 模式打一行 JSON 字符串。

use serde::Serialize;

/// 把结果输出到 stdout：
/// - JSON 模式：序列化为一行紧凑 JSON
/// - 人类模式：调用 `human_fmt` 闭包生成可读文本
pub fn print_result<T, F>(value: &T, json: bool, human_fmt: F) -> Result<(), String>
where
    T: Serialize + ?Sized,
    F: FnOnce(&T) -> String,
{
    if json {
        let line =
            serde_json::to_string(value).map_err(|e| format!("序列化 JSON 失败: {}", e))?;
        println!("{}", line);
    } else {
        let text = human_fmt(value);
        if !text.is_empty() {
            println!("{}", text);
        }
    }
    Ok(())
}
