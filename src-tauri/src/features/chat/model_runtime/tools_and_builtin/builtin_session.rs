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
    // 只留定位与识别用得到的几项：会话主键、数字短号、类型、标题。
    // 人格名与标题重复，三个远程联系人字段合成的信息标题里已经有了，更新时间没人看。
    // shortId 是给模型与脚本用的短寻址入口：能直接回填进 inform_session 的 session_id。
    let rows = items
        .into_iter()
        .map(|item| {
            serde_json::json!({
                "sessionId": item.session_id,
                "shortId": item.short_id,
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
    let target_reference = args.session_id.trim();
    if target_reference.is_empty() {
        return Err("session_id 不能为空".to_string());
    }
    // 目标可以是数字短 ID，也可以是完整会话主键；统一在这里解析成主键再交给服务层。
    let target_conversation_id = resolve_conversation_id_reference(state, target_reference)?
        .ok_or_else(|| format!("未找到目标会话: {target_reference}"))?;
    let result = conversation_service_v2().inform_session(
        state,
        &source_conversation_id,
        &target_conversation_id,
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
