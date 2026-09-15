fn build_archive_replacement_conversation(
    state: &AppState,
    agents: &[AgentProfile],
    assistant_agent_id: &str,
    selected_api: &ApiConfig,
    _source: &Conversation,
) -> Result<Conversation, String> {
    let mut conversation = build_conversation_record(
        &selected_api.id,
        "",
        "",
        CONVERSATION_KIND_CHAT,
        None,
        None,
    );
    let profile_snapshot = agents
        .iter()
        .find(|item| item.id == assistant_agent_id)
        .and_then(|agent| match build_user_profile_snapshot_block(&state.data_path, agent, 12) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                runtime_log_error(format!(
                    "[用户画像] 失败，任务=prepare_archive_active_conversation_seed_snapshot，agent_id={}，error={}",
                    agent.id,
                    err
                ));
                None
            }
        });
    if let Some(snapshot) = profile_snapshot {
        conversation.user_profile_snapshot = snapshot;
    }
    let summary_message =
        build_initial_summary_context_message(Some(&conversation.current_todos), None);
    conversation.last_user_at = Some(summary_message.created_at.clone());
    conversation.updated_at = summary_message.created_at.clone();
    conversation.messages.push(summary_message);
    Ok(conversation)
}
