impl ConversationServiceV2 {
    fn append_tool_event_to_assistant_message(
        &self,
        state: &AppState,
        input: &AssistantMessageToolAppendInput,
    ) -> Result<AssistantMessageToolAppendResult, String> {
        let conversation_id = input.conversation_id.trim();
        let assistant_message_id = input.assistant_message_id.trim();
        if conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        if assistant_message_id.is_empty() {
            return Err("assistantMessageId is required.".to_string());
        }
        let target_message = self.read_current_writable_assistant_message(
            state,
            conversation_id,
            assistant_message_id,
        )?;
        let agent_id = target_message
            .speaker_agent_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                ConversationServiceV2Error::new(
                    ConversationServiceV2ErrorCode::MessageNotWritable,
                    format!(
                        "目标 assistant message 缺少 speaker_agent_id，conversationId={}，assistantMessageId={}",
                        conversation_id, assistant_message_id
                    ),
                )
                .into_string()
            })?;
        with_conversation_mutation(
            state,
            conversation_id,
            "append_tool_event_to_assistant_message",
            || {
                let mut target_message = self.read_current_writable_assistant_message(
                    state,
                    conversation_id,
                    assistant_message_id,
                )?;
                if assistant_message_tool_append_closed(&target_message) {
                    return Err(
                        ConversationServiceV2Error::new(
                            ConversationServiceV2ErrorCode::ToolAppendClosed,
                            format!(
                                "目标 assistant message 已关闭 tool append，conversationId={}，assistantMessageId={}",
                                conversation_id, assistant_message_id
                            ),
                        )
                        .into_string(),
                    );
                }
                let normalized_agent_id = agent_id.trim();
                let target_agent_id = target_message
                    .speaker_agent_id
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default();
                if target_agent_id != normalized_agent_id {
                    return Err(
                        ConversationServiceV2Error::new(
                            ConversationServiceV2ErrorCode::MessageNotWritable,
                            format!(
                                "目标 assistant message 的 speaker_agent_id 不匹配，conversationId={}，assistantMessageId={}，expectedAgentId={}，actualAgentId={}",
                                conversation_id,
                                assistant_message_id,
                                normalized_agent_id,
                                target_agent_id
                            ),
                        )
                        .into_string(),
                    );
                }
                let tool_call_id = validate_tool_group_result_append_v2(
                    &input.assistant_tool_event,
                    &input.tool_result_event,
                )?;
                let group_call_ids =
                    tool_call_ids_from_assistant_tool_event_v2(&input.assistant_tool_event);
                let events = target_message.tool_call.get_or_insert_with(Vec::new);
                if !tool_history_contains_tool_result_id_v2(events, &tool_call_id) {
                    if !tool_history_contains_assistant_tool_group_v2(events, &group_call_ids) {
                        events.push(input.assistant_tool_event.clone());
                    }
                    events.push(input.tool_result_event.clone());
                }
                // 此处不改 provider_meta：用量等 meta 由 final text 落盘统一写入，
                // 工具追加时改 meta 会让 D14 组内追加回退整块重写。
                let tool_event_count = target_message.tool_call.as_ref().map(Vec::len).unwrap_or(0);
                self.persist_appended_ready_message_locked(state, conversation_id, &target_message)?;
                Ok(AssistantMessageToolAppendResult {
                    conversation_id: conversation_id.to_string(),
                    assistant_message_id: assistant_message_id.to_string(),
                    tool_event_count,
                    tool_append_closed: false,
                })
            },
        )
    }

    fn append_final_text_to_assistant_message(
        &self,
        state: &AppState,
        input: &AssistantMessageFinalTextAppendInput,
    ) -> Result<AssistantMessageFinalTextAppendResult, String> {
        let conversation_id = input.conversation_id.trim();
        let assistant_message_id = input.assistant_message_id.trim();
        if conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        if assistant_message_id.is_empty() {
            return Err("assistantMessageId is required.".to_string());
        }
        let final_text = input.final_text.as_str();
        let target_message = with_conversation_mutation(
            state,
            conversation_id,
            "append_final_text_to_assistant_message",
            || {
                let mut target_message = self.read_current_writable_assistant_message(
                    state,
                    conversation_id,
                    assistant_message_id,
                )?;
                if assistant_message_tool_append_closed(&target_message) {
                    return Err(
                        ConversationServiceV2Error::new(
                            ConversationServiceV2ErrorCode::FinalTextAlreadyCommitted,
                            format!(
                                "目标 assistant message 已有 final 正文，禁止重复提交，conversationId={}，assistantMessageId={}",
                                conversation_id, assistant_message_id
                            ),
                        )
                        .into_string(),
                    );
                }
                let mut text_part_updated = false;
                for part in &mut target_message.parts {
                    if let MessagePart::Text {
                        text,
                        reasoning_content,
                    } = part
                    {
                        *text = final_text.to_string();
                        merge_optional_text_block_v2(reasoning_content, input.reasoning_text.clone());
                        text_part_updated = true;
                        break;
                    }
                }
                if !text_part_updated {
                    target_message.parts.push(MessagePart::Text {
                        text: final_text.to_string(),
                        reasoning_content: input.reasoning_text.clone(),
                    });
                }
                merge_provider_meta_patch_v2(
                    &mut target_message.provider_meta,
                    input.provider_meta_patch.clone(),
                );
                // 用量聚合：meta 尚无真实用量时，直接取最后一个自带用量的工具调用事件
                // （工具轮真实用量随事件落盘，此处只兜底，不覆盖外部写入的最终 call 用量）
                merge_last_tool_call_usage_into_provider_meta(
                    &mut target_message.provider_meta,
                    &target_message.tool_call,
                );
                runtime_log_debug(format!(
                    "[表情替换] FinalAppend开始，conversation_id={}，assistant_message_id={}，existing_annotation_count={}，incoming_annotation_count={}，incoming_tokens=[{}]",
                    conversation_id,
                    assistant_message_id,
                    target_message
                        .meme_annotations
                        .as_ref()
                        .map(Vec::len)
                        .unwrap_or(0),
                    input
                        .meme_annotations
                        .as_ref()
                        .map(Vec::len)
                        .unwrap_or(0),
                    input
                        .meme_annotations
                        .as_ref()
                        .map(|items| items.iter().map(|item| item.meme.trim().to_string()).collect::<Vec<_>>().join(","))
                        .unwrap_or_default()
                ));
                target_message.meme_annotations = input.meme_annotations.clone();
                mark_stream_final_committed_v2(&mut target_message.provider_meta);

                self.persist_appended_ready_message_locked(state, conversation_id, &target_message)?;
                Ok(target_message)
            },
        )?;
        if let Err(err) = emit_unarchived_conversation_overview_item_updated_from_state(
            state,
            conversation_id,
        ) {
            runtime_log_warn(format!(
                "[会话概览] 跳过，任务=assistant final 写回后推送单会话，conversation_id={}，error={}",
                conversation_id, err
            ));
        }
        runtime_log_debug(format!(
            "[表情替换] FinalAppend完成，conversation_id={}，assistant_message_id={}，stored_annotation_count={}，stored_tokens=[{}]",
            conversation_id,
            assistant_message_id,
            target_message
                .meme_annotations
                .as_ref()
                .map(Vec::len)
                .unwrap_or(0),
            target_message
                .meme_annotations
                .as_ref()
                .map(|items| items.iter().map(|item| item.meme.trim().to_string()).collect::<Vec<_>>().join(","))
                .unwrap_or_default()
        ));
        Ok(AssistantMessageFinalTextAppendResult {
            conversation_id: conversation_id.to_string(),
            assistant_message_id: assistant_message_id.to_string(),
            final_text_committed: true,
            tool_append_closed: true,
        })
    }

    fn bootstrap_streaming_assistant_message(
        &self,
        state: &AppState,
        input: &AssistantMessageBootstrapInput,
    ) -> Result<AssistantMessageBootstrapResult, String> {
        let conversation_id = input.conversation_id.trim();
        let assistant_message_id = input.assistant_message_id.trim();
        let speaker_agent_id = input.speaker_agent_id.trim();
        if conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        if assistant_message_id.is_empty() {
            return Err("assistantMessageId is required.".to_string());
        }
        if speaker_agent_id.is_empty() {
            return Err("speakerAgentId is required.".to_string());
        }
        let created_at = input
            .created_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(now_iso);
        // 调度开始持有的 assistant_message_id 是唯一真相：先创建本轮空壳，
        // 再把压缩续调继承的工具组挂回 tool_call，保持等价于工具结果返回后的下一次模型调用。
        let mut message = ChatMessage {
            id: assistant_message_id.to_string(),
            role: "assistant".to_string(),
            created_at,
            speaker_agent_id: Some(speaker_agent_id.to_string()),
            parts: vec![MessagePart::Text {
                text: String::new(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        };
        if let Some(preserved) = input.compaction_preserved_messages.as_ref() {
            if !preserved.tool_history_events.is_empty() {
                let mut inherited_message = message.clone();
                inherited_message.tool_call = Some(preserved.tool_history_events.clone());
                if assistant_message_tool_append_closed(&inherited_message) {
                    runtime_log_error(format!(
                        "[聊天调度] 失败，任务=继承压缩保留工具组，原因=继承后 assistant message 已带 final 正文，conversation_id={}，assistant_message_id={}，preserved_events={}",
                        conversation_id,
                        assistant_message_id,
                        preserved.tool_history_events.len()
                    ));
                } else {
                    message = inherited_message;
                }
            }
        }
        merge_provider_meta_patch_v2(&mut message.provider_meta, input.provider_meta_patch.clone());

        // 委托线程的 conversation_id 固定等于 delegate_id（delegate-<UUID>），
        // 只能写入委托存储；缺失时不可回退到正式会话存储。
        if conversation_id.starts_with("delegate-") {
            let mut conversation = delegate_runtime_thread_conversation_get(state, conversation_id)?
                .ok_or_else(|| format!("委托会话不存在，conversationId={conversation_id}"))?;
            if conversation
                .messages
                .iter()
                .any(|existing| existing.id.trim() == assistant_message_id)
            {
                return Ok(AssistantMessageBootstrapResult {
                    conversation_id: conversation_id.to_string(),
                    assistant_message_id: assistant_message_id.to_string(),
                    created: false,
                });
            }
            conversation.messages.push(message);
            conversation.updated_at = now_iso();
            conversation.last_assistant_at = Some(conversation.updated_at.clone());
            increment_conversation_unread_count(&mut conversation, 1);
            delegate_runtime_thread_conversation_update(state, conversation_id, conversation)?;
            return Ok(AssistantMessageBootstrapResult {
                conversation_id: conversation_id.to_string(),
                assistant_message_id: assistant_message_id.to_string(),
                created: true,
            });
        }
        if self
            .get_raw_message_by_id(state, conversation_id, assistant_message_id)
            .is_ok()
        {
            return Ok(AssistantMessageBootstrapResult {
                conversation_id: conversation_id.to_string(),
                assistant_message_id: assistant_message_id.to_string(),
                created: false,
            });
        }
        with_conversation_mutation(
            state,
            conversation_id,
            "bootstrap_streaming_assistant_message",
            || self.append_message_locked(state, conversation_id, &message),
        )?;
        Ok(AssistantMessageBootstrapResult {
            conversation_id: conversation_id.to_string(),
            assistant_message_id: assistant_message_id.to_string(),
            created: true,
        })
    }

    fn patch_provider_meta_on_assistant_message(
        &self,
        state: &AppState,
        input: &AssistantMessageProviderMetaPatchInput,
    ) -> Result<AssistantMessageProviderMetaPatchResult, String> {
        let conversation_id = input.conversation_id.trim();
        let assistant_message_id = input.assistant_message_id.trim();
        if conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        if assistant_message_id.is_empty() {
            return Err("assistantMessageId is required.".to_string());
        }
        if !input.provider_meta_patch.is_object() {
            return Err("providerMetaPatch must be an object.".to_string());
        }
        with_conversation_mutation(
            state,
            conversation_id,
            "patch_provider_meta_on_assistant_message",
            || {
                let mut target_message = self.read_current_writable_assistant_message(
                    state,
                    conversation_id,
                    assistant_message_id,
                )?;
                merge_provider_meta_patch_v2(
                    &mut target_message.provider_meta,
                    Some(input.provider_meta_patch.clone()),
                );
                self.persist_replaced_ready_message_locked(state, conversation_id, &target_message)?;
                Ok(AssistantMessageProviderMetaPatchResult {
                    conversation_id: conversation_id.to_string(),
                    assistant_message_id: assistant_message_id.to_string(),
                })
            },
        )
    }

    fn patch_message_provider_meta_batch(
        &self,
        state: &AppState,
        input: &MessageProviderMetaBatchPatchInput,
    ) -> Result<(), String> {
        let conversation_id = input.conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        if input.items.is_empty() {
            return Ok(());
        }
        let mut patch_by_id = std::collections::HashMap::<String, Option<Value>>::new();
        for item in &input.items {
            let message_id = item.message_id.trim();
            if message_id.is_empty() {
                return Err("messageId is required.".to_string());
            }
            if let Some(meta) = item.provider_meta.as_ref() {
                if !meta.is_object() {
                    return Err(format!(
                        "providerMeta 必须是 object 或 null，messageId={message_id}"
                    ));
                }
            }
            patch_by_id.insert(message_id.to_string(), item.provider_meta.clone());
        }
        with_conversation_mutation(state, conversation_id, "patch_message_provider_meta_batch", || {
            let conversation_meta = self.get_conversation_meta(state, conversation_id)?;
            if !self.conversation_meta_is_unarchived_meta_view(&conversation_meta) {
                return Err(format!("Unarchived conversation not found: {conversation_id}"));
            }
            let paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
            ensure_chat_store_conversation_readable(state, conversation_id, &paths)?;
            let mut ready_meta = message_store::chat_store_read_meta(&paths)?
                .ok_or_else(|| {
                    format!(
                        "批量更新消息 providerMeta 失败：缺少 ready 消息元数据，conversation_id={conversation_id}"
                    )
                })?;
            ready_meta.apply_metadata_fields_from_meta_view(&conversation_meta);
            let mut previous_messages = Vec::with_capacity(patch_by_id.len());
            let mut updated_messages = Vec::with_capacity(patch_by_id.len());
            for (message_id, provider_meta) in &patch_by_id {
                let mut message = message_store::chat_store_read_message_by_id(&paths, message_id)?
                    .ok_or_else(|| {
                        format!(
                            "批量更新消息 providerMeta 失败：消息不存在，conversation_id={}，message_id={}",
                            conversation_id, message_id
                        )
                    })?;
                previous_messages.push(message.clone());
                message.provider_meta = provider_meta.clone();
                updated_messages.push(message);
            }
            ready_meta.apply_replaced_messages(&previous_messages, &updated_messages, || {
                message_store::chat_store_recompute_latest_summary_title_after_replace(
                    &paths,
                    &updated_messages,
                )
            })?;
            message_store::chat_store_replace_messages(
                &paths,
                &ready_meta.to_persist_meta(),
                &updated_messages,
            )?;
            self.mark_conversation_metadata_cached_persisted(state, conversation_id)?;
            Ok(())
        })
    }
    fn persist_stop_chat_partial_message(
        &self,
        state: &AppState,
        requested_conversation_id: Option<&str>,
        agent_id: &str,
        partial_assistant_text: &str,
        partial_activity_reasoning_text: &str,
        partial_inline_activity_text: &str,
        completed_tool_history: &[Value],
        assistant_message_id: Option<&str>,
    ) -> Result<StopChatPersistResult, String> {
        let should_persist = !partial_assistant_text.trim().is_empty()
            || !partial_activity_reasoning_text.trim().is_empty()
            || !completed_tool_history.is_empty();
        if !should_persist {
            return Ok(StopChatPersistResult {
                persisted: false,
                conversation_id: None,
                assistant_message: None,
            });
        }

        let organization = load_runtime_organization_snapshot(state)?;
        let app_config = organization.config;
        let api_config_id =
            resolve_stop_chat_api_config_id(&app_config, &organization.agents, agent_id)?;
        if !app_config.api_configs.iter().any(|api| api.id == api_config_id) {
            return Err(format!("Selected API config '{api_config_id}' not found."));
        }
        let Some(target) = resolve_stop_chat_target(state, requested_conversation_id, agent_id)? else {
            return Ok(StopChatPersistResult {
                persisted: false,
                conversation_id: None,
                assistant_message: None,
            });
        };

        let target_assistant_message_id = assistant_message_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        // 调度开始已 bootstrap 时，必须按该 UUID 原地更新；不能再因“尾消息是 assistant”跳过。
        if target_assistant_message_id.is_none() {
            if let Some(result) = build_stop_chat_skip_result(&target) {
                return Ok(result);
            }
        }

        let conversation_id = target.conversation_id().to_string();
        let mut assistant_message = if let Some(assistant_message_id) = target_assistant_message_id.as_deref()
        {
            match &target {
                StopChatConversationTarget::Runtime(conversation) => {
                    let existing = conversation
                        .messages
                        .iter()
                        .rev()
                        .find(|message| message.id.trim() == assistant_message_id)
                        .ok_or_else(|| {
                            format!(
                                "目标 assistant message 不存在，assistantMessageId={assistant_message_id}"
                            )
                        })?;
                    if existing.role.trim() != "assistant" {
                        return Err(format!(
                            "目标消息不是 assistant，assistantMessageId={assistant_message_id}"
                        ));
                    }
                    build_stop_chat_partial_assistant_message_for_id(
                        assistant_message_id,
                        agent_id,
                        &existing.created_at,
                        existing.speaker_agent_id.clone(),
                        existing.tool_call.clone(),
                        existing.provider_meta.clone(),
                        partial_assistant_text,
                        partial_activity_reasoning_text,
                        completed_tool_history,
                    )
                }
                StopChatConversationTarget::PersistedRef { .. } => {
                    let existing = self.read_current_writable_assistant_message(
                        state,
                        &conversation_id,
                        assistant_message_id,
                    )?;
                    build_stop_chat_partial_assistant_message_for_id(
                        assistant_message_id,
                        agent_id,
                        &existing.created_at,
                        existing.speaker_agent_id.clone(),
                        existing.tool_call.clone(),
                        existing.provider_meta.clone(),
                        partial_assistant_text,
                        partial_activity_reasoning_text,
                        completed_tool_history,
                    )
                }
            }
        } else {
            build_stop_chat_partial_assistant_message(
                agent_id,
                partial_assistant_text,
                partial_activity_reasoning_text,
                partial_inline_activity_text,
                completed_tool_history,
            )
        };
        let assistant_message_seed = assistant_message.id.clone();
        assistant_message.meme_annotations = populate_assistant_meme_annotations(
            state,
            &assistant_message_seed,
            assistant_message
                .parts
                .iter()
                .find_map(|part| match part {
                    MessagePart::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .unwrap_or(""),
        )?;
        let conversation_id = match target {
            StopChatConversationTarget::Runtime(mut conversation) => {
                if target_assistant_message_id.is_some() {
                    let target_id =
                        apply_stop_chat_partial_message_by_id(&mut conversation, &assistant_message)?;
                    delegate_runtime_thread_conversation_update(state, &target_id, conversation)
                        .map(|_| target_id)?
                } else {
                    let target_id =
                        apply_stop_chat_partial_message(&mut conversation, &assistant_message);
                    delegate_runtime_thread_conversation_update(state, &target_id, conversation)
                        .map(|_| target_id.to_string())?
                }
            }
            StopChatConversationTarget::PersistedRef { conversation_id, .. } => {
                let target_id = conversation_id.to_string();
                with_conversation_mutation(
                    state,
                    &target_id,
                    "persist_stop_chat_partial_message",
                    || {
                        if target_assistant_message_id.is_some() {
                            self.persist_replaced_ready_message_locked(
                                state,
                                &target_id,
                                &assistant_message,
                            )?;
                        } else {
                            self.append_message_locked(state, &target_id, &assistant_message)?;
                        }
                        Ok(())
                    },
                )?;
                target_id
            }
        };

        if let Err(err) = emit_unarchived_conversation_overview_item_updated_from_state(
            state,
            &conversation_id,
        ) {
            runtime_log_warn(format!(
                "[会话概览] 跳过，任务=停止生成持久化后推送单会话，conversation_id={}，error={}",
                conversation_id, err
            ));
        }

        Ok(StopChatPersistResult {
            persisted: true,
            conversation_id: Some(conversation_id),
            assistant_message: Some(assistant_message),
        })
    }

}
