fn persist_failed_chat_completed_tool_history(
    state: &AppState,
    requested_conversation_id: Option<&str>,
    agent_id: &str,
    chat_key: &str,
    error: &str,
) -> Result<bool, String> {
    let completed_tool_history = inflight_completed_tool_history(state, chat_key)?;
    if completed_tool_history.is_empty() {
        return Ok(false);
    }
    let persist_result = conversation_service_v2().persist_stop_chat_partial_message(
        state,
        requested_conversation_id,
        agent_id,
        "",
        "",
        "",
        &completed_tool_history,
        None,
    )?;
    runtime_log_error(format!(
        "[聊天] 失败前工具历史落盘检查 完成 已落盘={} 工具事件数={} 错误={}",
        persist_result.persisted,
        completed_tool_history.len(),
        error
    ));
    Ok(persist_result.persisted)
}

