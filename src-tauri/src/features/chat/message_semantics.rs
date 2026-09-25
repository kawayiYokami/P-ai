#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageToolHistoryView {
    Display,
    PromptReplay,
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
    let arguments_value = unwrap_tool_arguments_value(&raw_arguments);
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
                if matches!(view, MessageToolHistoryView::PromptReplay) && tool_calls.is_empty() {
                    continue;
                }
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
            if tool_calls.is_empty() {
                continue;
            }
            history_messages.push(PreparedHistoryMessage {
                role: "assistant".to_string(),
                text: event.text,
                extra_text_blocks: Vec::new(),
                user_time_text: None,
                images: Vec::new(),
                audios: Vec::new(),
                tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                tool_call_id: None,
                // tool-call 级 reasoning_content 必须只来自对应事件本身；
                reasoning_content: event.reasoning_content.clone(),
            });
            continue;
        }
        if event.role == "tool" && (!event.text.trim().is_empty() || event.tool_call_id.is_some()) {
            history_messages.push(PreparedHistoryMessage {
                role: "tool".to_string(),
                text: event.text,
                extra_text_blocks: Vec::new(),
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

#[derive(Debug, Clone, PartialEq)]
struct FoldedAssistantRequestMessages {
    assistant_text: String,
    activity_reasoning_text: Option<String>,
    tool_history_events: Vec<Value>,
}

fn final_assistant_reasoning_from_request_sequence(
    cumulative_reasoning: &str,
    tool_history_events: &[Value],
) -> String {
    let trimmed = cumulative_reasoning.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let tool_reasoning_rounds = tool_history_events
        .iter()
        .filter(|event| {
            event
                .get("role")
                .and_then(Value::as_str)
                .is_some_and(|role| role.eq_ignore_ascii_case("assistant"))
        })
        .filter_map(|event| event.get("reasoning_content").and_then(Value::as_str))
        .map(str::trim)
        .filter(|reasoning| !reasoning.is_empty())
        .collect::<Vec<_>>();
    if tool_reasoning_rounds.is_empty() {
        return trimmed.to_string();
    }

    let mut remaining = trimmed;
    for round_reasoning in tool_reasoning_rounds {
        let round_aligned = remaining.trim_start();
        let Some(stripped) = round_aligned.strip_prefix(round_reasoning) else {
            return trimmed.to_string();
        };
        remaining = stripped;
    }
    remaining.trim().to_string()
}

fn assistant_request_sequence_from_tool_history(
    tool_history_events: &[Value],
    assistant_text: &str,
    cumulative_activity_reasoning_text: &str,
) -> Vec<Value> {
    let mut request_messages = tool_history_events.to_vec();
    let final_reasoning =
        final_assistant_reasoning_from_request_sequence(cumulative_activity_reasoning_text, tool_history_events);
    let should_append_final_assistant =
        !assistant_text.trim().is_empty() || !final_reasoning.is_empty();
    if !should_append_final_assistant {
        return request_messages;
    }

    let mut assistant_message = serde_json::Map::new();
    assistant_message.insert("role".to_string(), Value::String("assistant".to_string()));
    if assistant_text.trim().is_empty() {
        assistant_message.insert("content".to_string(), Value::Null);
    } else {
        assistant_message.insert(
            "content".to_string(),
            Value::String(assistant_text.to_string()),
        );
    }
    if !final_reasoning.is_empty() {
        assistant_message.insert(
            "reasoning_content".to_string(),
            Value::String(final_reasoning),
        );
    }
    request_messages.push(Value::Object(assistant_message));
    request_messages
}

fn request_message_has_tool_calls(message: &Value) -> bool {
    message
        .get("tool_calls")
        .and_then(Value::as_array)
        .is_some_and(|calls| !calls.is_empty())
}

fn request_message_text_content(message: &Value) -> String {
    match message.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn extract_final_assistant_text_and_meta(request_messages: &[Value]) -> (String, Option<String>) {
    request_messages
        .iter()
        .rev()
        .find(|message| {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            role.eq_ignore_ascii_case("assistant") && !request_message_has_tool_calls(message)
        })
        .map(|message| {
            let reasoning_text = message
                .get("reasoning_content")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            (request_message_text_content(message), reasoning_text)
        })
        .unwrap_or_else(|| (String::new(), None))
}

fn fold_request_messages_to_assistant_content(
    request_messages: &[Value],
) -> FoldedAssistantRequestMessages {
    let final_assistant_index = request_messages
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, message)| {
            let role = message
                .get("role")
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or_default();
            (role.eq_ignore_ascii_case("assistant") && !request_message_has_tool_calls(message))
                .then_some(idx)
        });

    let mut assistant_text = String::new();
    let mut activity_reasoning_text = None::<String>;
    let mut tool_history_events = Vec::<Value>::new();
    for (idx, message) in request_messages.iter().enumerate() {
        if Some(idx) == final_assistant_index {
            assistant_text = request_message_text_content(message);
            activity_reasoning_text = message
                .get("reasoning_content")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            continue;
        }
        tool_history_events.push(message.clone());
    }

    FoldedAssistantRequestMessages {
        assistant_text,
        activity_reasoning_text,
        tool_history_events,
    }
}

fn normalize_assistant_provider_meta(provider_meta: Option<Value>) -> Option<Value> {
    let mut merged = provider_meta?;
    if !merged.is_object() {
        let raw_provider_meta = std::mem::replace(&mut merged, serde_json::json!({}));
        merged = serde_json::json!({
            "_raw_provider_meta": raw_provider_meta,
        });
    }
    Some(merged)
}

fn build_assistant_message_from_request_sequence(
    id: String,
    agent_id: &str,
    created_at: String,
    request_messages: &[Value],
    provider_meta: Option<Value>,
) -> ChatMessage {
    let folded = fold_request_messages_to_assistant_content(request_messages);
    let activity_events = folded.tool_history_events;
    ChatMessage {
        id,
        role: "assistant".to_string(),
        created_at,
        speaker_agent_id: Some(agent_id.to_string()),
        parts: vec![MessagePart::Text {
            text: folded.assistant_text,
            reasoning_content: folded.activity_reasoning_text,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: normalize_assistant_provider_meta(provider_meta),
        tool_call: if activity_events.is_empty() {
            None
        } else {
            Some(activity_events)
        },
        mcp_call: None,
        meme_annotations: None,
    }
}

fn populate_assistant_meme_annotations(
    state: &AppState,
    seed_source: &str,
    assistant_text: &str,
) -> Result<Option<Vec<MemeAnnotation>>, String> {
    let annotations = build_meme_annotations(state, assistant_text, seed_source)?;
    let annotation_tokens = annotations
        .iter()
        .map(|item| item.meme.trim().to_string())
        .collect::<Vec<_>>()
        .join(",");
    runtime_log_debug(format!(
        "[表情替换] 生成，seed_source={}，text_len={}，annotation_count={}，tokens=[{}]，text={}",
        seed_source,
        assistant_text.chars().count(),
        annotations.len(),
        annotation_tokens,
        assistant_text.replace('\n', "\\n")
    ));
    Ok(if annotations.is_empty() {
        None
    } else {
        Some(annotations)
    })
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

#[cfg(test)]
mod message_semantics_tests {
    use super::*;

    #[test]
    fn normalize_tool_call_arguments_should_unwrap_double_escaped_object() {
        let raw = serde_json::json!("{\"path\":\"a.md\",\"content\":\"# 标题\\n内容\"}");
        let (value, text, _) = normalize_tool_call_arguments(Some(&raw));
        assert!(value.is_object());
        assert_eq!(value["path"], "a.md");
        assert!(value["content"].as_str().unwrap().contains("内容"));
        assert!(text.contains("path"));
    }

    fn test_message(role: &str, text: &str, created_at: &str) -> ChatMessage {
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: role.to_string(),
            created_at: created_at.to_string(),
            speaker_agent_id: Some("agent".to_string()),
            parts: vec![MessagePart::Text {
                text: text.to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
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
    fn normalize_message_tool_history_events_should_trim_partial_group_for_prompt_replay() {
        let now = now_iso();
        let mut assistant = test_message("assistant", "处理中", &now);
        assistant.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "先同时读取两个文件",
                "tool_calls": [
                    {
                        "id": "call-a",
                        "call_id": "call-a",
                        "type": "function",
                        "function": {
                            "name": "read",
                            "arguments": "{\"path\":\"a.rs\"}"
                        }
                    },
                    {
                        "id": "call-b",
                        "call_id": "call-b",
                        "type": "function",
                        "function": {
                            "name": "read",
                            "arguments": "{\"path\":\"b.rs\"}"
                        }
                    }
                ]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-a",
                "content": "工具 A 结果"
            }),
        ]);

        let display_events =
            normalize_message_tool_history_events(&assistant, MessageToolHistoryView::Display);
        let prompt_events =
            normalize_message_tool_history_events(&assistant, MessageToolHistoryView::PromptReplay);

        assert_eq!(display_events.len(), 2);
        assert_eq!(display_events[0].tool_calls.len(), 2);
        assert_eq!(prompt_events.len(), 2);
        assert_eq!(prompt_events[0].tool_calls.len(), 1);
        assert_eq!(prompt_events[0].tool_calls[0].invocation_id.as_deref(), Some("call-a"));
        assert_eq!(prompt_events[1].tool_call_id.as_deref(), Some("call-a"));
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
                        "name": "contact_reply",
                        "arguments": "{\"text\":\"我查好了\"}"
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
    fn extract_final_assistant_text_and_meta_should_ignore_tool_history_without_cloning_it() {
        let request_messages = vec![
            serde_json::json!({
                "role": "assistant",
                "content": "先调用工具",
                "tool_calls": [{"id": "call-1"}]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-1",
                "content": "工具结果"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": "最终答复",
                "reasoning_content": "  已核对工具结果。  "
            }),
        ];

        let (text, reasoning) = extract_final_assistant_text_and_meta(&request_messages);

        assert_eq!(text, "最终答复");
        assert_eq!(reasoning.as_deref(), Some("已核对工具结果。"));
    }

    #[test]
    fn extract_final_assistant_text_and_meta_should_return_empty_when_only_tool_rounds_exist() {
        let request_messages = vec![
            serde_json::json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{"id": "call-1"}]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-1",
                "content": "工具结果"
            }),
        ];

        let (text, reasoning) = extract_final_assistant_text_and_meta(&request_messages);

        assert!(text.is_empty());
        assert!(reasoning.is_none());
    }

    #[test]
    fn build_prepared_history_messages_from_tool_history_should_not_invent_reasoning_content_without_event() {
        let now = now_iso();
        let mut assistant = test_message("assistant", "我查好了", &now);
        assistant.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "fc_1",
                    "type": "function",
                    "function": {
                        "name": "tavily_search",
                        "arguments": "{\"query\":\"明日方舟终末地 最新版本\"}"
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
        assert_eq!(history[0].reasoning_content, None);
    }

    #[test]
    fn build_prepared_history_messages_from_tool_history_should_preserve_event_level_reasoning_per_tool_call() {
        let now = now_iso();
        let mut assistant = test_message("assistant", "我查好了", &now);
        assistant.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第一次工具思考",
                "tool_calls": [{
                    "id": "fc_1",
                    "type": "function",
                    "function": {
                        "name": "tavily_search",
                        "arguments": "{\"query\":\"明日方舟终末地 最新版本\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "fc_1",
                "content": "{\"ok\":true}"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第二次工具思考",
                "tool_calls": [{
                    "id": "fc_2",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\":\"README.md\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "fc_2",
                "content": "{\"ok\":true}"
            }),
        ]);

        let history = build_prepared_history_messages_from_tool_history(
            &assistant,
            MessageToolHistoryView::PromptReplay,
        );

        assert_eq!(history.len(), 4);
        assert_eq!(history[0].role, "assistant");
        assert_eq!(history[0].reasoning_content.as_deref(), Some("第一次工具思考"));
        assert_eq!(history[2].role, "assistant");
        assert_eq!(history[2].reasoning_content.as_deref(), Some("第二次工具思考"));
    }

    #[test]
    fn build_prepared_history_messages_from_tool_history_should_keep_tool_group_as_single_assistant() {
        let now = now_iso();
        let mut assistant = test_message("assistant", "我查好了", &now);
        assistant.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "先同时读取两个文件",
                "tool_calls": [
                    {
                        "id": "call-a",
                        "call_id": "call-a",
                        "type": "function",
                        "function": {
                            "name": "read",
                            "arguments": "{\"path\":\"a.rs\"}"
                        }
                    },
                    {
                        "id": "call-b",
                        "call_id": "call-b",
                        "type": "function",
                        "function": {
                            "name": "read",
                            "arguments": "{\"path\":\"b.rs\"}"
                        }
                    }
                ]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-a",
                "content": "工具 A 结果"
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-b",
                "content": "工具 B 结果"
            }),
        ]);

        let history = build_prepared_history_messages_from_tool_history(
            &assistant,
            MessageToolHistoryView::PromptReplay,
        );

        assert_eq!(history.len(), 3);
        assert_eq!(history[0].role, "assistant");
        assert_eq!(history[0].reasoning_content.as_deref(), Some("先同时读取两个文件"));
        assert_eq!(history[0].tool_calls.as_ref().map(Vec::len), Some(2));
        assert_eq!(history[1].role, "tool");
        assert_eq!(history[1].tool_call_id.as_deref(), Some("call-a"));
        assert_eq!(history[1].reasoning_content, None);
        assert_eq!(history[2].role, "tool");
        assert_eq!(history[2].tool_call_id.as_deref(), Some("call-b"));
        assert_eq!(history[2].reasoning_content, None);
    }

    #[test]
    fn assistant_request_sequence_from_tool_history_should_keep_only_final_reasoning_on_last_assistant(
    ) {
        let tool_history_events = vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "先调用终端工具查看 PowerShell 版本。",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "exec",
                        "arguments": "{\"command\":\"pwsh --version\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "PowerShell 7.5.4"
            }),
        ];

        let request_messages = assistant_request_sequence_from_tool_history(
            &tool_history_events,
            "终端版本是 PowerShell 7.5.4。",
            "先调用终端工具查看 PowerShell 版本。我已经拿到工具结果，现在直接回答用户终端版本。",
        );
        let folded = fold_request_messages_to_assistant_content(&request_messages);

        assert_eq!(request_messages.len(), 3);
        assert_eq!(request_messages[2]["role"], "assistant");
        assert_eq!(
            request_messages[2]["reasoning_content"].as_str(),
            Some("我已经拿到工具结果，现在直接回答用户终端版本。")
        );
        assert_eq!(folded.assistant_text, "终端版本是 PowerShell 7.5.4。");
        assert_eq!(
            folded.activity_reasoning_text.as_deref(),
            Some("我已经拿到工具结果，现在直接回答用户终端版本。")
        );
        assert_eq!(
            folded.tool_history_events[0]["reasoning_content"].as_str(),
            Some("先调用终端工具查看 PowerShell 版本。")
        );
    }

    #[test]
    fn assistant_request_sequence_from_tool_history_should_not_concat_tool_round_content_into_final_text(
    ) {
        let tool_history_events = vec![
            serde_json::json!({
                "role": "assistant",
                "content": "三个并发 shell 跑完后我再汇总。",
                "reasoning_content": "先同时跑 node/pnpm/rustc。",
                "tool_calls": [{
                    "id": "call_node",
                    "type": "function",
                    "function": {
                        "name": "exec",
                        "arguments": "{\"command\":\"node -v\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_node",
                "content": "Node.js v24.2.0"
            }),
        ];

        let request_messages = assistant_request_sequence_from_tool_history(
            &tool_history_events,
            "三个并发 shell 跑完，结果是：\n\nNode.js v24.2.0",
            "先同时跑 node/pnpm/rustc。现在汇总结果。",
        );
        let message = build_assistant_message_from_request_sequence(
            "assistant-final".to_string(),
            "agent-a",
            "2026-06-03T21:41:00Z".to_string(),
            &request_messages,
            None,
        );

        assert_eq!(request_messages.len(), 3);
        assert_eq!(
            request_messages[0]["content"].as_str(),
            Some("三个并发 shell 跑完后我再汇总。")
        );
        assert_eq!(
            request_messages[2]["content"].as_str(),
            Some("三个并发 shell 跑完，结果是：\n\nNode.js v24.2.0")
        );
        assert_eq!(
            message
                .tool_call
                .as_ref()
                .and_then(|events| events.first())
                .and_then(|event| event.get("content"))
                .and_then(Value::as_str),
            Some("三个并发 shell 跑完后我再汇总。")
        );
        match &message.parts[0] {
            MessagePart::Text { text, .. } => {
                assert_eq!(
                    text,
                    "三个并发 shell 跑完，结果是：\n\nNode.js v24.2.0"
                );
            }
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn build_assistant_message_from_request_sequence_should_inject_markers_into_matching_final_text() {
        let tool_history_events = vec![
            serde_json::json!({
                "role": "assistant",
                "content": "先说明第一步",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": "{\"path\":\"a.txt\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "文件内容"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": "再说明第二步",
                "tool_calls": [{
                    "id": "call_2",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": "{\"path\":\"b.txt\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_2",
                "content": "更多内容"
            }),
        ];

        let request_messages = assistant_request_sequence_from_tool_history(
            &tool_history_events,
            "先说明第一步\n\n再说明第二步\n\n最后汇总",
            "",
        );
        let message = build_assistant_message_from_request_sequence(
            "assistant-final".to_string(),
            "agent-a",
            "2026-06-03T21:41:00Z".to_string(),
            &request_messages,
            None,
        );

        match &message.parts[0] {
            MessagePart::Text { text, .. } => {
                assert_eq!(
                    text,
                    "先说明第一步\n\n再说明第二步\n\n最后汇总"
                );
            }
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn build_assistant_message_from_request_sequence_should_preserve_final_text_verbatim() {
        let final_text = "\n```python\ndef hello():\n    name = \"Incremark\"\n    if name:\n        print(name)\n```\n\n[toolcall:literal]\n";
        let request_messages = assistant_request_sequence_from_tool_history(&[], final_text, "");
        let message = build_assistant_message_from_request_sequence(
            "assistant-final".to_string(),
            "agent-a",
            "2026-06-03T21:41:00Z".to_string(),
            &request_messages,
            None,
        );

        match &message.parts[0] {
            MessagePart::Text { text, .. } => assert_eq!(text, final_text),
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn build_assistant_message_from_request_sequence_should_keep_marker_only_rounds_before_final_text() {
        let tool_history_events = vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_a",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": "{\"path\":\"a.txt\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_a",
                "content": "A"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_b",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": "{\"path\":\"b.txt\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_b",
                "content": "B"
            }),
        ];

        let request_messages = assistant_request_sequence_from_tool_history(
            &tool_history_events,
            "最后汇总",
            "",
        );
        let message = build_assistant_message_from_request_sequence(
            "assistant-final".to_string(),
            "agent-a",
            "2026-06-03T21:41:00Z".to_string(),
            &request_messages,
            None,
        );

        match &message.parts[0] {
            MessagePart::Text { text, .. } => {
                assert_eq!(text, "最后汇总");
            }
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn build_assistant_message_from_request_sequence_should_keep_raw_final_text_with_marker_like_content() {
        let tool_history_events = vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_a",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": "{\"path\":\"a.txt\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_a",
                "content": "A"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_b",
                    "type": "function",
                    "function": {
                        "name": "read",
                        "arguments": "{\"path\":\"b.txt\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_b",
                "content": "B"
            }),
        ];

        let request_messages = assistant_request_sequence_from_tool_history(
            &tool_history_events,
            "[toolcall:call_a] [toolcall:call_b]\n\n最后汇总",
            "",
        );
        let message = build_assistant_message_from_request_sequence(
            "assistant-final".to_string(),
            "agent-a",
            "2026-06-03T21:41:00Z".to_string(),
            &request_messages,
            None,
        );

        match &message.parts[0] {
            MessagePart::Text { text, .. } => {
                assert_eq!(text, "[toolcall:call_a] [toolcall:call_b]\n\n最后汇总");
            }
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn assistant_request_sequence_from_tool_history_should_strip_tool_reasoning_prefix_with_inter_round_newlines(
    ) {
        let tool_history_events = vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第1轮先搜版本",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "search",
                        "arguments": "{\"q\":\"PowerShell version\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"ok\":true}"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第2轮再核对发布时间",
                "tool_calls": [{
                    "id": "call_2",
                    "type": "function",
                    "function": {
                        "name": "search",
                        "arguments": "{\"q\":\"PowerShell 7.5.4 release date\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_2",
                "content": "{\"ok\":true}"
            }),
        ];

        let request_messages = assistant_request_sequence_from_tool_history(
            &tool_history_events,
            "PowerShell 7.5.4 已发布。",
            "第1轮先搜版本\n\n第2轮再核对发布时间\n\n现在可以直接回答用户。",
        );

        assert_eq!(request_messages.len(), 5);
        assert_eq!(
            request_messages[4]["reasoning_content"].as_str(),
            Some("现在可以直接回答用户。")
        );
    }

    #[test]
    fn assistant_request_sequence_from_tool_history_should_not_persist_tool_reasoning_as_final_reasoning_when_no_final_text(
    ) {
        let tool_history_events = vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第1轮思考",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\":\"README.md\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"ok\":true}"
            }),
        ];

        let request_messages = assistant_request_sequence_from_tool_history(
            &tool_history_events,
            "",
            "第1轮思考",
        );
        let message = build_assistant_message_from_request_sequence(
            "assistant-stop".to_string(),
            "agent-a",
            "2026-05-08T12:00:00Z".to_string(),
            &request_messages,
            None,
        );

        assert_eq!(request_messages, tool_history_events);
        assert!(message.provider_meta.is_none());
        assert_eq!(
            message
                .tool_call
                .as_ref()
                .and_then(|events| events.first())
                .and_then(|event| event.get("reasoning_content"))
                .and_then(Value::as_str),
            Some("第1轮思考")
        );
        match &message.parts[0] {
            MessagePart::Text { text, .. } => {
                assert_eq!(text, "");
            }
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn assistant_request_sequence_from_tool_history_should_append_reasoning_only_final_assistant_when_interrupted_after_tool_rounds(
    ) {
        let tool_history_events = vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第1轮先调用工具",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\":\"README.md\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"ok\":true}"
            }),
        ];

        let request_messages = assistant_request_sequence_from_tool_history(
            &tool_history_events,
            "",
            "第1轮先调用工具\n\n我已经拿到结果，准备组织最终答复。",
        );
        let message = build_assistant_message_from_request_sequence(
            "assistant-stop".to_string(),
            "agent-a",
            "2026-05-08T12:00:00Z".to_string(),
            &request_messages,
            None,
        );

        assert_eq!(request_messages.len(), 3);
        assert!(request_messages[2]["content"].is_null());
        assert_eq!(
            request_messages[2]["reasoning_content"].as_str(),
            Some("我已经拿到结果，准备组织最终答复。")
        );
        assert!(message.provider_meta.is_none());
    }

    #[test]
    fn build_assistant_message_from_request_sequence_should_keep_tool_reasoning_separate_from_provider_meta(
    ) {
        let request_messages = vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "先检查上下文",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "grep",
                        "arguments": "{\"pattern\":\"quest\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"ok\":true}"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": "最终答案",
                "reasoning_content": "我已经拿到结果，现在直接回答。"
            }),
        ];

        let message = build_assistant_message_from_request_sequence(
            "assistant-final".to_string(),
            "agent-a",
            "2026-05-08T12:00:00Z".to_string(),
            &request_messages,
            Some(serde_json::json!({
                "dispatchElapsedMs": 1234
            })),
        );

        assert_eq!(message.parts.len(), 1);
        match &message.parts[0] {
            MessagePart::Text {
                text,
                reasoning_content,
            } => {
                assert_eq!(text, "最终答案");
                assert_eq!(
                    reasoning_content.as_deref(),
                    Some("我已经拿到结果，现在直接回答。")
                );
            }
            other => panic!("unexpected message part: {:?}", other),
        }
        assert_eq!(
            message
                .provider_meta
                .as_ref()
                .and_then(|meta| meta.get("dispatchElapsedMs"))
                .and_then(Value::as_u64),
            Some(1234)
        );
        assert_eq!(
            message
                .tool_call
                .as_ref()
                .and_then(|events| events.first())
                .and_then(|event| event.get("reasoning_content"))
                .and_then(Value::as_str),
            Some("先检查上下文")
        );
    }

    #[test]
    fn build_assistant_message_from_request_sequence_should_keep_final_reasoning_on_text_part_without_tools(
    ) {
        let request_messages = assistant_request_sequence_from_tool_history(
            &[],
            "不太确定，展开说说？",
            "先判断用户提到的工具指代。",
        );
        let message = build_assistant_message_from_request_sequence(
            "assistant-final".to_string(),
            "agent-a",
            "2026-05-08T12:00:00Z".to_string(),
            &request_messages,
            None,
        );

        match &message.parts[0] {
            MessagePart::Text {
                text,
                reasoning_content,
            } => {
                assert_eq!(text, "不太确定，展开说说？");
                assert_eq!(
                    reasoning_content.as_deref(),
                    Some("先判断用户提到的工具指代。")
                );
            }
            other => panic!("unexpected message part: {:?}", other),
        }
        assert!(message.tool_call.is_none());
    }
}
