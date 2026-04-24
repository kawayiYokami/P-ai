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
        request_id: None,
        phase_id: None,
        reason: None,
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

fn send_stream_rebind_required_event(
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    request_id: Option<&str>,
    reason: &str,
) {
    let phase_id = uuid::Uuid::new_v4().to_string();
    let request_id_text = request_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let reason_text = reason.trim().to_string();
    let send_result = on_delta.send(AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("stream_rebind_required".to_string()),
        request_id: request_id_text.clone(),
        phase_id: Some(phase_id.clone()),
        reason: Some(reason_text.clone()),
        tool_name: None,
        tool_status: None,
        tool_args: None,
        message: None,
    });
    match send_result {
        Ok(_) => runtime_log_info(format!(
            "[聊天] 流式通道重绑请求 完成 reason={} phase_id={} request_id={}",
            reason_text,
            phase_id,
            request_id_text.unwrap_or_default()
        )),
        Err(err) => runtime_log_warn(format!(
            "[聊天] 流式通道重绑请求 失败 reason={} phase_id={} request_id={} error={}",
            reason_text,
            phase_id,
            request_id_text.unwrap_or_default(),
            err
        )),
    }
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
    _agent: &AgentProfile,
    current_department: Option<&DepartmentConfig>,
    id: &str,
) -> bool {
    if tool_restricted_by_department(current_department, id).is_some() {
        return false;
    }
    if builtin_tool_is_fixed_system(id) {
        return selected_api.enable_tools;
    }
    if builtin_tool_is_local_conversation_fixed(id) || builtin_tool_is_contact_only_hidden(id) {
        return false;
    }
    if id == "screenshot" && !selected_api.enable_image {
        return false;
    }
    if !selected_api.enable_tools {
        return false;
    }
    if tool_forced_by_department(current_department, id) {
        return true;
    }
    if !department_permission_allows_any_name(
        current_department,
        DepartmentPermissionCategory::BuiltinTool,
        &[id],
    ) {
        return false;
    }
    if let Some(tool) = default_agent_tools().iter().find(|tool| tool.id == id) {
        return tool.enabled;
    }
    true
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
        let raw = tool_failure_result_json("contact_send_files", "远程IM渠道未开启文件发送");
        let value: Value = serde_json::from_str(&raw).expect("tool failure json");
        assert_eq!(value.get("ok").and_then(Value::as_bool), Some(false));
        assert_eq!(
            value.get("tool").and_then(Value::as_str),
            Some("contact_send_files")
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
                .contains("工具 `contact_send_files` 调用失败")
        );
    }

    #[test]
    fn tool_enabled_should_use_system_defaults_instead_of_agent_specific_tool_flags() {
        let mut api = AppConfig::default()
            .api_configs
            .into_iter()
            .next()
            .expect("default api config");
        api.enable_tools = true;
        api.enable_image = true;

        let mut agent = default_agent();
        if let Some(tool) = agent.tools.iter_mut().find(|tool| tool.id == "fetch") {
            tool.enabled = false;
        }
        if let Some(tool) = agent.tools.iter_mut().find(|tool| tool.id == "plan") {
            tool.enabled = true;
        }

        assert!(tool_enabled(&api, &agent, None, "fetch"));
        assert!(!tool_enabled(&api, &agent, None, "plan"));
        assert!(!tool_enabled(&api, &agent, None, "contact_reply"));
    }
}
