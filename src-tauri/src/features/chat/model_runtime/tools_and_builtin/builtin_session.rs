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
    // 只留定位与识别用得到的几项：数字短号（优先作为 sessionId 与 shortId）、类型、标题。
    // 人格名与标题重复，三个远程联系人字段合成的信息标题里已经有了，更新时间没人看。
    // 有了数字短 ID 之后，对外呈现与回执中彻底杜绝 36 位长 UUID，降低模型认知负担。
    let rows = items
        .into_iter()
        .map(|item| {
            let session_id_str = item
                .short_id
                .map(|id| id.to_string())
                .unwrap_or(item.session_id);
            serde_json::json!({
                "sessionId": session_id_str,
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
    // 回执展示目标标识：优先使用短号或调用方传入的引用，避免将底层长 UUID 暴露给模型
    let display_id = target_reference.to_string();
    Ok(serde_json::json!({
        "session_id": display_id,
        "target_conversation_id": result.target_conversation_id,
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
