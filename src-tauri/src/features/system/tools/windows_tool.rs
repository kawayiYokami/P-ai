// ==================== windows 工具：列出窗口 / 激活窗口 ====================
//
// 独立于 operate 的窗口管理工具：模型可用 list windows 获取全量窗口清单
// （含 PAI 自身窗口，xcap 枚举会过滤当前进程窗口，这里用 EnumWindows 全量枚举），
// 再用 activate window 把目标窗口切到前台，配合 operate 的 focused_window 截图使用。
//
// 语法（一行一个动作，与 operate 同风格）：
//   list windows
//   activate window id=<windowId 或 0x 前缀十六进制>
//
// 平台实现见 platform/ 模块（windows: EnumWindows + UIA；linux: xcap + xcb + atspi；macos: 占位）。

use crate::platform::WindowInfo;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, rmcp::schemars::JsonSchema)]
#[schemars(description = "窗口管理脚本请求。只接收一个 script 字段；script 必须是多行字符串，一行一个动作。")]
struct WindowsRequest {
    #[schemars(description = "窗口管理脚本文本，一行一个动作。")]
    script: String,
    #[serde(default)]
    #[schemars(description = "本次工具调用的超时时间，单位毫秒；未指定时默认 60000ms。")]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsResponse {
    ok: bool,
    steps: Vec<WindowsStepResult>,
}

/// 单步结果只留「这一步做成了没有」和窗口清单。
/// `summary` 那句话没有消费者：种类由 `kind` 表达，成败由 `ok` 表达。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowsStepResult {
    kind: WindowsStepKind,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_id: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    windows: Option<Vec<WindowInfo>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
enum WindowsStepKind {
    ListWindows,
    ActivateWindow,
}

#[derive(Debug, Clone)]
enum WindowsAction {
    ListWindows { line: usize },
    ActivateWindow { line: usize, window_id: usize },
}

fn windows_invalid(message: impl Into<String>) -> DesktopToolError {
    DesktopToolError::invalid_params(message)
}

fn parse_windows_script(script: &str) -> DesktopToolResult<Vec<WindowsAction>> {
    let mut actions = Vec::new();
    for (idx, raw_line) in script.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let tokens = tokenize_script_line(trimmed)
            .map_err(|err| windows_invalid(format!("第 {line_no} 行：{err}")))?;
        if tokens.is_empty() {
            continue;
        }
        let head = tokens[0].to_ascii_lowercase();
        let action = match head.as_str() {
            "list" => {
                if tokens.len() < 2 || tokens[1].to_ascii_lowercase() != "windows" {
                    return Err(windows_invalid(format!("第 {line_no} 行：list 后必须是 windows，当前为 `{}`", tokens.join(" "))));
                }
                WindowsAction::ListWindows { line: line_no }
            }
            "activate" => {
                if tokens.len() < 2 || tokens[1].to_ascii_lowercase() != "window" {
                    return Err(windows_invalid(format!("第 {line_no} 行：activate 后必须是 window，当前为 `{}`", tokens.join(" "))));
                }
                let id_raw = tokens[2..].join(" ");
                // 兼容 `id=xxx` 与裸 `xxx` 两种写法（描述以 id= 形式给出）
                let id_raw = id_raw
                    .strip_prefix("id=")
                    .or_else(|| id_raw.strip_prefix("ID="))
                    .unwrap_or(&id_raw);
                let id = parse_window_id(id_raw)
                    .ok_or_else(|| windows_invalid(format!("第 {line_no} 行：activate 缺少合法 id，当前为 `{id_raw}`")))?;
                WindowsAction::ActivateWindow { line: line_no, window_id: id }
            }
            other => return Err(windows_invalid(format!("第 {line_no} 行：不支持的窗口动作 `{other}`"))),
        };
        actions.push(action);
    }
    Ok(actions)
}

/// 解析窗口 id：支持十进制数字（windowId）或 0x 前缀十六进制。
fn parse_window_id(raw: &str) -> Option<usize> {
    let raw = raw.trim();
    if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        usize::from_str_radix(hex, 16).ok()
    } else {
        raw.parse::<usize>().ok()
    }
}

fn run_windows_tool(input: WindowsRequest) -> DesktopToolResult<WindowsResponse> {
    let actions = parse_windows_script(&input.script)?;
    let mut steps = Vec::with_capacity(actions.len());
    for action in actions {
        match action {
            WindowsAction::ListWindows { line } => {
                let windows = platform::list_all_windows();
                runtime_log_info(format!(
                    "[窗口工具] 步骤完成，任务=run_windows_tool，line={}，kind=ListWindows，count={}",
                    line,
                    windows.len()
                ));
                steps.push(WindowsStepResult {
                    kind: WindowsStepKind::ListWindows,
                    ok: true,
                    window_id: None,
                    windows: Some(windows),
                });
            }
            WindowsAction::ActivateWindow { line, window_id } => {
                let (title, activated) = platform::activate_window(window_id);
                runtime_log_info(format!(
                    "[窗口工具] 步骤完成，任务=run_windows_tool，line={}，kind=ActivateWindow，title={title:?}，activated={activated}",
                    line
                ));
                steps.push(WindowsStepResult {
                    kind: WindowsStepKind::ActivateWindow,
                    ok: activated,
                    window_id: Some(window_id),
                    windows: None,
                });
            }
        }
    }
    let ok = steps.iter().all(|step| step.ok);
    Ok(WindowsResponse { ok, steps })
}

#[cfg(test)]
mod windows_tool_tests {
    use super::*;

    #[test]
    fn parse_window_id_should_support_decimal_and_hex() {
        assert_eq!(parse_window_id("123"), Some(123));
        assert_eq!(parse_window_id("0x1A2B"), Some(0x1A2B));
        assert_eq!(parse_window_id("0Xff"), Some(255));
        assert_eq!(parse_window_id("abc"), None);
        assert_eq!(parse_window_id(""), None);
    }

    #[test]
    fn parse_script_should_accept_list_and_activate() {
        let actions = parse_windows_script("list windows\nactivate window 0x1234\n").unwrap();
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], WindowsAction::ListWindows { .. }));
        assert!(matches!(actions[1], WindowsAction::ActivateWindow { window_id: 0x1234, .. }));
    }

    #[test]
    fn parse_script_should_accept_id_equals_form() {
        // 描述以 id= 形式给出，也兼容裸 id
        let actions = parse_windows_script("activate window id=7541586\nactivate window 7541586\n").unwrap();
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], WindowsAction::ActivateWindow { window_id: 7541586, .. }));
        assert!(matches!(actions[1], WindowsAction::ActivateWindow { window_id: 7541586, .. }));
    }

    #[test]
    fn parse_script_should_reject_unknown_action() {
        assert!(parse_windows_script("list windows\nfoo bar\n").is_err());
        assert!(parse_windows_script("activate\n").is_err());
        assert!(parse_windows_script("list\n").is_err());
        assert!(parse_windows_script("activate window abc\n").is_err());
    }
}
