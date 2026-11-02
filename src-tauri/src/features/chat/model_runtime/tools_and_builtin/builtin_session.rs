fn session_tool_source_conversation_id(session_id: &str) -> Result<String, String> {
    delegate_session_conversation_id(session_id)
        .ok_or_else(|| "固定会话工具缺少 conversation_id".to_string())
}

fn builtin_get_session(
    state: &AppState,
    _session_id: &str,
    args: GetSessionToolArgs,
) -> Result<Value, String> {
    let items = conversation_service_v2().list_tool_session_targets(
        state,
        args.keyword.as_deref(),
    )?;
    serde_json::to_value(items).map_err(|err| format!("序列化会话列表失败：{err}"))
}

fn builtin_inform_session(
    state: &AppState,
    session_id: &str,
    args: InformSessionToolArgs,
) -> Result<Value, String> {
    let source_conversation_id = session_tool_source_conversation_id(session_id)?;
    let result = conversation_service_v2().inform_session(
        state,
        &source_conversation_id,
        &args.session_id,
        &args.content,
    )?;
    Ok(serde_json::json!({
        "session_id": result.target_conversation_id,
        "remote_contact_id": result.remote_contact_id,
        "pushed_to_remote": result.pushed_to_remote,
        "message_id": result.message.id,
    }))
}

fn message_text_content(message: &ChatMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text, .. } => Some(text.trim().to_string()),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}
