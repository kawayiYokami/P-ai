#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageToolHistoryView {
    Display,
    PromptReplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolCallProtocolFamily {
    OpenAiChatLike,
    OpenAiResponses,
    Gemini,
    Anthropic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructuredToolReplayCapability {
    Structured,
    TextOnly,
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
struct NormalizedToolCallRecord {
    invocation_id: Option<String>,
    provider_call_id: Option<String>,
    tool_type: Option<String>,
    tool_name: Option<String>,
    arguments_value: Value,
    arguments_text: String,
    raw_arguments: Value,
}

#[derive(Debug, Clone, PartialEq)]
struct NormalizedMessageToolEvent {
    role: String,
    text: String,
    reasoning_content: Option<String>,
    tool_calls: Vec<NormalizedToolCallRecord>,
    tool_call_id: Option<String>,
}

fn normalize_tool_call_arguments(raw: Option<&Value>) -> (Value, String, Value) {
    let raw_arguments = raw
        .cloned()
        .unwrap_or_else(|| Value::String("{}".to_string()));
    let arguments_text = match &raw_arguments {
        Value::String(text) => text.trim().to_string(),
        other => other.to_string(),
    };
    let arguments_value = match &raw_arguments {
        Value::String(text) => serde_json::from_str::<Value>(text).unwrap_or_else(|_| raw_arguments.clone()),
        _ => raw_arguments.clone(),
    };
    (arguments_value, arguments_text, raw_arguments)
}

fn normalize_prompt_tool_calls(raw_calls: &[Value]) -> Vec<NormalizedToolCallRecord> {
    raw_calls
        .iter()
        .map(|raw| {
            let invocation_id = raw
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let provider_call_id = raw
                .get("call_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let tool_type = raw
                .get("type")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let tool_name = raw
                .get("function")
                .and_then(|func| func.get("name"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let (arguments_value, arguments_text, raw_arguments) = normalize_tool_call_arguments(
                raw.get("function").and_then(|func| func.get("arguments")),
            );
            NormalizedToolCallRecord {
                invocation_id,
                provider_call_id,
                tool_type,
                tool_name,
                arguments_value,
                arguments_text,
                raw_arguments,
            }
        })
        .collect()
}

fn normalize_message_tool_history_events(
    message: &ChatMessage,
    view: MessageToolHistoryView,
) -> Vec<NormalizedMessageToolEvent> {
    let source_events = match (view, message.tool_call.as_ref()) {
        (_, None) => return Vec::new(),
        (MessageToolHistoryView::Display, Some(events)) => events.clone(),
        (MessageToolHistoryView::PromptReplay, Some(events)) => sanitize_tool_history_events(events),
    };

    let mut normalized = Vec::<NormalizedMessageToolEvent>::new();
    for event in source_events {
        let role = event
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        match role.as_str() {
            "assistant" => {
                let tool_calls = event
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .map(|calls| normalize_prompt_tool_calls(calls))
                    .unwrap_or_default();
                normalized.push(NormalizedMessageToolEvent {
                    role,
                    text: event
                        .get("content")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    reasoning_content: event
                        .get("reasoning_content")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned),
                    tool_calls,
                    tool_call_id: None,
                });
            }
            "tool" => {
                normalized.push(NormalizedMessageToolEvent {
                    role,
                    text: event
                        .get("content")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    reasoning_content: None,
                    tool_calls: Vec::new(),
                    tool_call_id: event
                        .get("tool_call_id")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned),
                });
            }
            _ => {}
        }
    }
    normalized
}

fn normalized_tool_call_to_history_value(call: &NormalizedToolCallRecord) -> Option<Value> {
    let invocation_id = call.invocation_id.as_deref()?.trim();
    let tool_name = call.tool_name.as_deref()?.trim();
    if invocation_id.is_empty() || tool_name.is_empty() {
        return None;
    }
    let mut func = serde_json::Map::new();
    func.insert("name".to_string(), Value::String(tool_name.to_string()));
    func.insert("arguments".to_string(), call.raw_arguments.clone());

    let mut obj = serde_json::Map::new();
    obj.insert("id".to_string(), Value::String(invocation_id.to_string()));
    obj.insert(
        "type".to_string(),
        Value::String(
            call.tool_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("function")
                .to_string(),
        ),
    );
    obj.insert("function".to_string(), Value::Object(func));
    if let Some(provider_call_id) = call
        .provider_call_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        obj.insert("call_id".to_string(), Value::String(provider_call_id.to_string()));
    }
    Some(Value::Object(obj))
}

fn build_prepared_history_messages_from_tool_history(
    message: &ChatMessage,
    view: MessageToolHistoryView,
) -> Vec<PreparedHistoryMessage> {
    let mut history_messages = Vec::<PreparedHistoryMessage>::new();
    for event in normalize_message_tool_history_events(message, view) {
        if event.role == "assistant" {
            let tool_calls = event
                .tool_calls
                .iter()
                .filter_map(normalized_tool_call_to_history_value)
                .collect::<Vec<_>>();
            history_messages.push(PreparedHistoryMessage {
                role: "assistant".to_string(),
                text: event.text,
                user_time_text: None,
                images: Vec::new(),
                audios: Vec::new(),
                tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                tool_call_id: None,
                reasoning_content: event.reasoning_content,
            });
            continue;
        }
        if event.role == "tool" && (!event.text.trim().is_empty() || event.tool_call_id.is_some()) {
            history_messages.push(PreparedHistoryMessage {
                role: "tool".to_string(),
                text: event.text,
                user_time_text: None,
                images: Vec::new(),
                audios: Vec::new(),
                tool_calls: None,
                tool_call_id: event.tool_call_id,
                reasoning_content: None,
            });
        }
    }
    history_messages
}

fn tool_history_markdown_lines_from_message(message: &ChatMessage) -> Vec<String> {
    let mut out = Vec::<String>::new();
    for event in normalize_message_tool_history_events(message, MessageToolHistoryView::Display) {
        if event.role == "assistant" {
            for call in event.tool_calls {
                let tool_name = call.tool_name.as_deref().unwrap_or("unknown");
                let args = call.arguments_text.trim();
                if args.is_empty() {
                    out.push(format!("- 工具调用: {tool_name}"));
                } else {
                    out.push(format!("- 工具调用: {tool_name} | 参数: {args}"));
                }
            }
            continue;
        }
        if event.role == "tool" {
            let content = event.text.trim();
            if !content.is_empty() {
                let snippet = if content.chars().count() > 300 {
                    format!("{}...", content.chars().take(300).collect::<String>())
                } else {
                    content.to_string()
                };
                out.push(format!("- 工具结果: {snippet}"));
            }
        }
    }
    out
}

fn tool_call_replay_capability(
    protocol_family: ToolCallProtocolFamily,
    call: &NormalizedToolCallRecord,
) -> StructuredToolReplayCapability {
    let has_invocation_id = call
        .invocation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    let has_tool_name = call
        .tool_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    if !has_invocation_id || !has_tool_name {
        return StructuredToolReplayCapability::Invalid;
    }
    match protocol_family {
        ToolCallProtocolFamily::OpenAiResponses => {
            if call
                .provider_call_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
            {
                StructuredToolReplayCapability::Structured
            } else {
                StructuredToolReplayCapability::TextOnly
            }
        }
        ToolCallProtocolFamily::OpenAiChatLike
        | ToolCallProtocolFamily::Gemini
        | ToolCallProtocolFamily::Anthropic => StructuredToolReplayCapability::Structured,
    }
}

fn tool_result_replay_capability(
    protocol_family: ToolCallProtocolFamily,
    tool_call_id: &str,
    provider_call_id: Option<&str>,
) -> StructuredToolReplayCapability {
    if tool_call_id.trim().is_empty() {
        return StructuredToolReplayCapability::Invalid;
    }
    match protocol_family {
        ToolCallProtocolFamily::OpenAiResponses => {
            if provider_call_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_some()
            {
                StructuredToolReplayCapability::Structured
            } else {
                StructuredToolReplayCapability::TextOnly
            }
        }
        ToolCallProtocolFamily::OpenAiChatLike
        | ToolCallProtocolFamily::Gemini
        | ToolCallProtocolFamily::Anthropic => StructuredToolReplayCapability::Structured,
    }
}

#[cfg(test)]
mod message_semantics_tests {
    use super::*;

    fn test_message(role: &str, text: &str, created_at: &str) -> ChatMessage {
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: role.to_string(),
            created_at: created_at.to_string(),
            speaker_agent_id: Some("agent".to_string()),
            parts: vec![MessagePart::Text {
                text: text.to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    #[test]
    fn normalize_message_tool_history_events_should_preserve_display_but_drop_prompt_orphans() {
        let now = now_iso();
        let mut assistant = test_message("assistant", "处理中", &now);
        assistant.tool_call = Some(vec![serde_json::json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "bing_search",
                    "arguments": "{\"query\":\"rust\"}"
                }
            }]
        })]);

        let display_events =
            normalize_message_tool_history_events(&assistant, MessageToolHistoryView::Display);
        let prompt_events =
            normalize_message_tool_history_events(&assistant, MessageToolHistoryView::PromptReplay);

        assert_eq!(display_events.len(), 1);
        assert_eq!(display_events[0].tool_calls.len(), 1);
        assert!(prompt_events.is_empty());
    }

    #[test]
    fn build_prepared_history_messages_from_tool_history_should_expand_sidecar_events() {
        let now = now_iso();
        let mut assistant = test_message("assistant", "我查好了", &now);
        assistant.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "fc_1",
                    "call_id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "remote_im_send",
                        "arguments": "{\"action\":\"send\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "fc_1",
                "content": "{\"ok\":true}"
            }),
        ]);

        let history = build_prepared_history_messages_from_tool_history(
            &assistant,
            MessageToolHistoryView::PromptReplay,
        );

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, "assistant");
        assert_eq!(
            history[0]
                .tool_calls
                .as_ref()
                .and_then(|calls| calls.first())
                .and_then(|call| call.get("call_id"))
                .and_then(Value::as_str),
            Some("call_1")
        );
        assert_eq!(history[1].role, "tool");
        assert_eq!(history[1].tool_call_id.as_deref(), Some("fc_1"));
    }

    #[test]
    fn tool_call_replay_capability_should_require_provider_call_id_only_for_responses() {
        let call = NormalizedToolCallRecord {
            invocation_id: Some("fc_1".to_string()),
            provider_call_id: None,
            tool_type: Some("function".to_string()),
            tool_name: Some("bing_search".to_string()),
            arguments_value: serde_json::json!({ "query": "rust" }),
            arguments_text: "{\"query\":\"rust\"}".to_string(),
            raw_arguments: Value::String("{\"query\":\"rust\"}".to_string()),
        };

        assert_eq!(
            tool_call_replay_capability(ToolCallProtocolFamily::OpenAiChatLike, &call),
            StructuredToolReplayCapability::Structured
        );
        assert_eq!(
            tool_call_replay_capability(ToolCallProtocolFamily::Gemini, &call),
            StructuredToolReplayCapability::Structured
        );
        assert_eq!(
            tool_call_replay_capability(ToolCallProtocolFamily::Anthropic, &call),
            StructuredToolReplayCapability::Structured
        );
        assert_eq!(
            tool_call_replay_capability(ToolCallProtocolFamily::OpenAiResponses, &call),
            StructuredToolReplayCapability::TextOnly
        );
    }
}
