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
    // 只留定位与识别用得到的三项：会话 id、类型、标题。
    // 人格名与标题重复，三个远程联系人字段合成的信息标题里已经有了，更新时间没人看。
    let rows = items
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "sessionId": item.session_id,
                "kind": item.kind,
                "title": item.title,
            })
        })
        .collect::<Vec<_>>();
    Ok(Value::Array(rows))
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
        "pushed_to_remote": result.pushed_to_remote,
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
