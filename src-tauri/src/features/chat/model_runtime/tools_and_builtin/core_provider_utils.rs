fn debug_value_snippet(value: &Value, max_chars: usize) -> String {
    let raw = serde_json::to_string(value).unwrap_or_else(|_| "<invalid json>".to_string());
    if raw.chars().count() <= max_chars {
        raw
    } else {
        let head = raw.chars().take(max_chars).collect::<String>();
        format!("{head}...")
    }
}

fn send_tool_status_event(
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    tool_name: &str,
    tool_status: &str,
    tool_args: Option<&str>,
    message: &str,
) {
    let send_result = on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("tool_status".to_string()),
        tool_name: Some(tool_name.to_string()),
        tool_status: Some(tool_status.to_string()),
        tool_args: tool_args.map(|v| v.to_string()),
        message: Some(message.to_string()),
    });
    let unified_status = match tool_status.trim().to_ascii_lowercase().as_str() {
        "start" | "running" | "begin" => "开始",
        "done" | "success" | "ok" | "completed" => "完成",
        "skip" | "skipped" => "跳过",
        "error" | "failed" | "fail" => "失败",
        _ => "未知",
    };
    let delivery_desc = if send_result.is_ok() {
        "投递成功"
    } else {
        "投递失败"
    };
    runtime_log_debug(format!(
        "[工具调用] 名称={} 状态={} 消息={} 事件投递结果={}",
        tool_name, unified_status, message, delivery_desc
    ));
}

fn tool_failure_result_json(tool_name: &str, err_text: &str) -> String {
    let tool_name = tool_name.trim();
    let err_text = err_text.trim();
    serde_json::json!({
        "ok": false,
        "tool": tool_name,
        "error": err_text,
        "message": format!("工具 `{}` 调用失败：{}", tool_name, err_text)
    })
    .to_string()
}

fn tool_enabled(
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    current_department: Option<&DepartmentConfig>,
    id: &str,
) -> bool {
    if id == "screenshot" && !selected_api.enable_image {
        return false;
    }
    if tool_restricted_by_department(current_department, id).is_some() {
        return false;
    }
    selected_api.enable_tools
        && agent
            .tools
            .iter()
            .any(|tool| tool.id == id && tool.enabled)
}

#[derive(Debug)]
struct ToolInvokeError(String);

impl std::fmt::Display for ToolInvokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ToolInvokeError {}

impl From<String> for ToolInvokeError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

fn clean_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_image_unsupported_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("unknown variant `image_url`")
        || lower.contains("expected `text`")
        || lower.contains("does not support image")
        || lower.contains("image input")
}

fn truncate_by_chars(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if input.chars().count() <= max_chars {
        return input.to_string();
    }
    let mut out = String::new();
    for (idx, ch) in input.chars().enumerate() {
        if idx >= max_chars {
            break;
        }
        out.push(ch);
    }
    out.push_str("...");
    out
}

#[cfg(test)]
mod core_provider_utils_tests {
    use super::*;

    #[test]
    fn tool_failure_result_json_marks_failure_explicitly() {
        let raw = tool_failure_result_json("remote_im_send", "远程IM渠道未开启文件发送");
        let value: Value = serde_json::from_str(&raw).expect("tool failure json");
        assert_eq!(value.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            value.get("tool").and_then(Value::as_str),
            Some("remote_im_send")
        );
        assert_eq!(
            value.get("error").and_then(Value::as_str),
            Some("远程IM渠道未开启文件发送")
        );
        assert!(
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("工具 `remote_im_send` 调用失败")
        );
    }
}
