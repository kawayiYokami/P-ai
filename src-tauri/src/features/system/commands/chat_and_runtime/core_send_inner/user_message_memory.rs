fn write_retrieved_memory_ids_into_provider_meta(
    provider_meta: &mut Option<Value>,
    recall_hit_ids: &[String],
) {
    let deduped_ids = memory_board_ids_from_current_hits(recall_hit_ids, recall_hit_ids.len());
    if deduped_ids.is_empty() {
        return;
    }
    let mut meta = provider_meta
        .take()
        .unwrap_or_else(|| serde_json::json!({}));
    if !meta.is_object() {
        meta = serde_json::json!({});
    }
    if let Some(obj) = meta.as_object_mut() {
        obj.insert(
            "retrieved_memory_ids".to_string(),
            Value::Array(
                deduped_ids
                    .into_iter()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
    *provider_meta = Some(meta);
}

fn append_user_message_to_conversation(
    state: &AppState,
    mut conversation: Conversation,
    user_message: ChatMessage,
    now: &str,
) -> Conversation {
    conversation.messages.push(user_message);
    conversation.updated_at = now.to_string();
    conversation.last_user_at = Some(now.to_string());
    conversation_service_v2().increment_unread_count_if_background(
        state,
        &mut conversation,
        1,
    );
    conversation
}

fn memory_recall_query_from_user_text(user_text: &str) -> String {
    clean_text(user_text.trim())
}

fn render_message_parts_text_for_recall(parts: &[MessagePart]) -> String {
    parts.iter()
        .filter_map(|part| match part {
            MessagePart::Text { text, .. } => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            MessagePart::Image { .. }
            | MessagePart::Audio { .. }
            | MessagePart::Attachment { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, Default)]
struct UserMessageRecallPayload {
    stored_ids: Vec<String>,
    raw_ids: Vec<String>,
}

fn with_memory_lock<T>(
    state: &AppState,
    task_name: &str,
    f: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let start = Instant::now();
    const MEMORY_LOCK_WARN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
    let _guard = state.memory_lock.lock().map_err(|err| {
        format!(
            "Failed to lock memory mutex at {}:{} {} for task={} err={}",
            file!(),
            line!(),
            module_path!(),
            task_name,
            err
        )
    })?;
    let waited = start.elapsed();
    if waited >= MEMORY_LOCK_WARN_TIMEOUT {
        runtime_log_warn(format!(
            "[记忆] 获取记忆锁耗时过长 任务名={} 耗时毫秒={} 阈值毫秒={}",
            task_name,
            waited.as_millis(),
            MEMORY_LOCK_WARN_TIMEOUT.as_millis()
        ));
    }
    f()
}

fn collect_recall_payload_for_user_message(
    data_path: &PathBuf,
    agents: &[AgentProfile],
    effective_agent_id: &str,
    message: &ChatMessage,
) -> Result<UserMessageRecallPayload, String> {
    if message.role.trim() != "user" {
        return Ok(UserMessageRecallPayload::default());
    }
    let memory_agent = agents
        .iter()
        .find(|a| a.id == effective_agent_id);
    if !memory_agent
        .map(agent_memory_rag_enabled)
        .unwrap_or(true)
    {
        return Ok(UserMessageRecallPayload::default());
    }
    let private_memory_enabled = memory_agent
        .map(|a| a.private_memory_enabled)
        .unwrap_or(false);
    let recall_query_text =
        memory_recall_query_from_user_text(&render_message_parts_text_for_recall(&message.parts));
    if recall_query_text.trim().is_empty() {
        return Ok(UserMessageRecallPayload::default());
    }
    let store_memories = memory_store_list_memories_visible_for_agent(
        data_path,
        effective_agent_id,
        private_memory_enabled,
    )?;
    let raw_ids = memory_recall_hit_ids(
        data_path,
        &store_memories,
        &recall_query_text,
        MEMORY_RERANK_MIN_SCORE_RAG,
    );
    let stored_ids = memory_board_ids_from_current_hits(&raw_ids, 7);
    Ok(UserMessageRecallPayload { stored_ids, raw_ids })
}
