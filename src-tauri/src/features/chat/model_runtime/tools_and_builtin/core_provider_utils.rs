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
        _ => "完成",
    };
    let send_desc = if send_result.is_ok() {
        "发送成功"
    } else {
        "发送失败"
    };
    eprintln!(
        "[工具调用] 名称={} 状态={} 消息={} 发送结果={}",
        tool_name, unified_status, message, send_desc
    );
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
