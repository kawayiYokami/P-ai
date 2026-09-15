fn render_preserved_conversation_message_text(message: &ChatMessage) -> String {
    let mut blocks = Vec::<String>::new();
    if message.role.trim().eq_ignore_ascii_case("assistant") {
        for event in message.tool_call.iter().flatten() {
            let is_assistant = event
                .get("role")
                .and_then(Value::as_str)
                .is_some_and(|role| role.trim().eq_ignore_ascii_case("assistant"));
            if !is_assistant {
                continue;
            }
            let content = event
                .get("content")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            if let Some(content) = content {
                blocks.push(content.to_string());
            }
        }
    }
    let rendered = render_prompt_message_text(message);
    if !rendered.trim().is_empty() {
        blocks.push(rendered);
    }
    blocks.join("\n")
}

impl ConversationServiceV2 {
    fn ensure_unarchived_conversation(
        &self,
        conversation: &Conversation,
        conversation_id: &str,
    ) -> Result<(), String> {
        if !conversation_is_unarchived(conversation) {
            return Err(format!(
                "Unarchived conversation not found: {}",
                conversation_id.trim()
            ));
        }
        Ok(())
    }

    fn read_persisted_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Conversation, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let conversation_meta =
            self.get_conversation_meta(state, normalized_conversation_id)?;
        let store_paths =
            message_store::message_store_paths(&state.data_path, normalized_conversation_id)?;
        ensure_chat_store_conversation_readable(
            state,
            normalized_conversation_id,
            &store_paths,
        )?;
        let messages =
            message_store::chat_store_read_all_messages(&store_paths)?.unwrap_or_default();
        let mut conversation = self.build_conversation_record_from_meta_view(&conversation_meta);
        conversation.messages = messages;
        Ok(conversation)
    }

    fn read_archive_pipeline_source_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Conversation, String> {
        self.read_persisted_conversation(state, conversation_id)
    }

    /// 压缩/归档输入读取最后 block 完整消息（不过滤旧压缩消息，上一轮摘要保留在输入内）。
    /// 远程唤醒精简读取器已随远程唤醒 LLM 压缩移除，压缩输入不再受字符上限裁剪。
    fn read_archive_pipeline_last_block_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Conversation, String> {
        let source = self.read_persisted_conversation(state, conversation_id)?;
        let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
        let mut block_messages = if let Some(page) =
            message_store::chat_store_read_block_page(&store_paths, None)?
        {
            page.messages
        } else {
            source.messages.clone()
        };
        materialize_chat_message_parts_from_media_refs(&mut block_messages, &state.data_path);
        let mut last_block = source.clone();
        last_block.messages = block_messages;
        Ok(last_block)
    }

    fn try_read_persisted_conversation(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Option<Conversation>, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Ok(None);
        }
        match self.read_persisted_conversation(state, normalized_conversation_id) {
            Ok(conversation) => Ok(Some(conversation)),
            Err(err) if err.contains("not found") || err.contains("不存在") => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn collect_unarchived_conversation_summaries_cached(
        &self,
        state: &AppState,
    ) -> Result<Vec<UnarchivedConversationSummary>, String> {
        let main_conversation_id = main_conversation_id_downgraded(state)
            .map(|id| id.trim().to_string())
            .unwrap_or_default();
        let chat_index = state_read_chat_index_cached(state)?;
        let visible_conversations = chat_index
            .conversations
            .iter()
            .filter(|item| !chat_index_item_is_archived(item))
            .filter_map(|item| {
                let conversation_meta = match self.get_conversation_meta(state, item.id.as_str()) {
                    Ok(conversation_meta) => conversation_meta,
                    Err(err) => {
                        runtime_log_error(format!(
                            "[会话索引读取] 状态=失败，任务=collect_unarchived_conversation_summaries_cached，conversation_id={}，error={}",
                            item.id, err
                        ));
                        return None;
                    }
                };
                (self.conversation_meta_is_unarchived_meta_view(&conversation_meta)
                    && conversation_meta.visible_in_foreground_lists)
                    .then_some(conversation_meta)
            })
            .collect::<Vec<_>>();
        let visible_ids = visible_conversations
            .iter()
            .map(|conversation_meta| conversation_meta.id.trim().to_string())
            .filter(|conversation_id: &String| !conversation_id.is_empty())
            .collect::<std::collections::HashSet<_>>();
        let mut seen_pins = std::collections::HashSet::<String>::new();
        let pinned_conversation_ids = pinned_conversation_ids_downgraded(state)
            .iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .filter(|item| *item != main_conversation_id)
            .filter(|item| visible_ids.contains(item))
            .filter(|item| seen_pins.insert(item.clone()))
            .collect::<Vec<_>>();
        let summaries = visible_conversations
            .iter()
            .map(|conversation_meta| {
                let hydrated_conversation_meta =
                    self.fill_summary_preview_messages_fallback(state, conversation_meta);
                build_unarchived_conversation_summary_from_meta_view(
                    state,
                    &main_conversation_id,
                    &pinned_conversation_ids,
                    &hydrated_conversation_meta,
                    Some(DESKTOP_CHAT_VIEWER_ID),
                )
            })
            .collect::<Vec<_>>();
        Ok(sort_unarchived_conversation_summaries(summaries))
    }

}
