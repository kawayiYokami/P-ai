impl ConversationServiceV2 {
    fn get_chat_snapshot(
        &self,
        state: &AppState,
        input: &SessionSelector,
    ) -> Result<ChatSnapshot, String> {
        let requested_conversation_id = input
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let fallback_main_conversation_id = if requested_conversation_id.is_none() {
            match state_service_get_main_conversation_id(state) {
                Ok(value) => value.and_then(|value| {
                    let trimmed = value.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                }),
                Err(err) => {
                    runtime_log_warn(format!(
                        "[会话快照] 读取主会话 ID 失败，按无默认会话降级继续：error={err}"
                    ));
                    None
                }
            }
        } else {
            None
        };
        if let Some(conversation_id) = requested_conversation_id
            .clone()
            .or(fallback_main_conversation_id)
        {
            let store_paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
            let snapshot = if let Some(snapshot) =
                message_store::chat_store_read_chat_snapshot(&store_paths)?
            {
                let mut latest_user = snapshot.latest_user;
                let mut latest_assistant = snapshot.latest_assistant;
                if let Some(message) = latest_user.as_mut() {
                    materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
                }
                if let Some(message) = latest_assistant.as_mut() {
                    materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
                }
                Some(ChatSnapshot {
                    conversation_id: conversation_id.clone(),
                    latest_user: latest_user.map(project_message_for_frontend_display_only),
                    latest_assistant: latest_assistant.map(project_message_for_frontend_display_only),
                    active_message_count: snapshot.active_message_count,
                })
            } else {
                self.try_read_unarchived_conversation(state, &conversation_id)?
                    .filter(|conversation| conversation_visible_in_foreground_lists(conversation))
                    .map(|conversation| {
                        let mut latest_user = conversation
                            .messages
                            .iter()
                            .rev()
                            .find(|message| message.role == "user")
                            .cloned();
                        let mut latest_assistant = conversation
                            .messages
                            .iter()
                            .rev()
                            .find(|message| message.role == "assistant")
                            .cloned();
                        if let Some(message) = latest_user.as_mut() {
                            materialize_message_parts_from_media_refs(
                                &mut message.parts,
                                &state.data_path,
                            );
                        }
                        if let Some(message) = latest_assistant.as_mut() {
                            materialize_message_parts_from_media_refs(
                                &mut message.parts,
                                &state.data_path,
                            );
                        }
                        ChatSnapshot {
                            conversation_id: conversation.id.clone(),
                            latest_user: latest_user.map(project_message_for_frontend_display_only),
                            latest_assistant: latest_assistant.map(project_message_for_frontend_display_only),
                            active_message_count: conversation.messages.len(),
                        }
                    })
            };
            if let Some(snapshot) = snapshot {
                return Ok(snapshot);
            }
        }

        let _guard = lock_conversation_with_metrics(state, "get_chat_snapshot")?;

        let mut app_config = state_read_config_cached(state)?;
        let agents = state_read_agents_cached(state)?;
        let runtime_snapshot = build_runtime_organization_snapshot_from_parts(
            &state.data_path,
            &mut app_config,
            &agents,
        )?;
        let runtime_agents = runtime_snapshot.agents;
        let assistant_agent_id = assistant_agent_id_downgraded(state);
        let requested_agent_id = input.agent_id.trim();
        let effective_agent_id = if !requested_agent_id.is_empty() {
            if runtime_agents
                .iter()
                .any(|agent| agent.id == requested_agent_id && !agent.is_built_in_user)
            {
                requested_agent_id.to_string()
            } else {
                return Err(format!("Selected agent '{requested_agent_id}' not found."));
            }
        } else if runtime_agents
            .iter()
            .any(|agent| agent.id == assistant_agent_id && !agent.is_built_in_user)
        {
            assistant_agent_id.clone()
        } else {
            runtime_agents
                .iter()
                .find(|agent| !agent.is_built_in_user)
                .map(|agent| agent.id.clone())
                .ok_or_else(|| "Selected agent not found.".to_string())?
        };

        if let Some(conversation_id) =
            self.resolve_latest_foreground_conversation_id(state, &effective_agent_id)?
        {
            let store_paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
            if let Some(snapshot) = message_store::chat_store_read_chat_snapshot(&store_paths)? {
                let mut latest_user = snapshot.latest_user;
                let mut latest_assistant = snapshot.latest_assistant;
                if let Some(message) = latest_user.as_mut() {
                    materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
                }
                if let Some(message) = latest_assistant.as_mut() {
                    materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
                }
                return Ok(ChatSnapshot {
                    conversation_id,
                    latest_user: latest_user.map(project_message_for_frontend_display_only),
                    latest_assistant: latest_assistant.map(project_message_for_frontend_display_only),
                    active_message_count: snapshot.active_message_count,
                });
            }
        }

        Ok(ChatSnapshot {
            conversation_id: String::new(),
            latest_user: None,
            latest_assistant: None,
            active_message_count: 0,
        })
    }

    fn get_conversation_recent_messages(
        &self,
        state: &AppState,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        self.get_conversation_meta(state, normalized_conversation_id)?;
        let store_paths =
            message_store::message_store_paths(&state.data_path, normalized_conversation_id)?;
        ensure_chat_store_conversation_readable(
            state,
            normalized_conversation_id,
            &store_paths,
        )?;
        Ok(message_store::chat_store_read_recent_messages(&store_paths, limit)?
            .unwrap_or_default())
    }

    fn get_conversation_prompt_context(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Conversation, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let conversation_meta = self.get_conversation_meta(state, normalized_conversation_id)?;
        let store_paths =
            message_store::message_store_paths(&state.data_path, normalized_conversation_id)?;
        ensure_chat_store_conversation_readable(
            state,
            normalized_conversation_id,
            &store_paths,
        )?;
        let messages = message_store::chat_store_read_current_compaction_segment(&store_paths)?
            .map(|segment| segment.messages)
            .unwrap_or_default();
        let mut conversation = self.build_conversation_record_from_meta_view(&conversation_meta);
        conversation.messages = messages;
        Ok(conversation)
    }

    fn get_current_compaction_segment_messages_through(
        &self,
        state: &AppState,
        conversation_id: &str,
        end_message_id: &str,
    ) -> Result<Vec<ChatMessage>, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        self.get_conversation_meta(state, normalized_conversation_id)?;
        let store_paths =
            message_store::message_store_paths(&state.data_path, normalized_conversation_id)?;
        ensure_chat_store_conversation_readable(
            state,
            normalized_conversation_id,
            &store_paths,
        )?;
        let mut messages = message_store::chat_store_read_current_compaction_segment(&store_paths)?
            .map(|segment| segment.messages)
            .unwrap_or_default();
        let end_message_id = end_message_id.trim();
        let end_position = messages
            .iter()
            .position(|message| message.id == end_message_id)
            .ok_or_else(|| format!("当前压缩段不包含目标消息：{end_message_id}"))?;
        messages.truncate(end_position.saturating_add(1));
        Ok(messages)
    }

    fn get_conversation_snapshot(
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

    fn try_get_conversation_snapshot(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Option<Conversation>, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Ok(None);
        }
        match self.get_conversation_snapshot(state, normalized_conversation_id) {
            Ok(conversation) => Ok(Some(conversation)),
            Err(err) if err.contains("not found") || err.contains("不存在") => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn get_conversation_last_block(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<ConversationBlockPageResult, String> {
        // 传 None 让存储层选最新块；块号从 0 开始编号，传具体数字 0 会命中第一块。
        self.get_conversation_block(state, conversation_id, None)
    }

    fn get_conversation_block(
        &self,
        state: &AppState,
        conversation_id: &str,
        block_id: Option<u32>,
    ) -> Result<ConversationBlockPageResult, String> {
        self.with_unarchived_conversation_by_id_fast(state, conversation_id, |conversation| {
            let store_paths = message_store::message_store_paths(&state.data_path, &conversation.id)?;
            if let Some(page) =
                message_store::chat_store_read_block_page(&store_paths, block_id)?
            {
                let mut messages = page.messages;
                materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
                return Ok(ConversationBlockPageResult {
                    blocks: page
                        .blocks
                        .into_iter()
                        .map(|item| ConversationBlockSummaryResult {
                            block_id: item.block_id,
                            message_count: item.message_count,
                            first_message_id: item.first_message_id,
                            last_message_id: item.last_message_id,
                            first_created_at: item.first_created_at,
                            last_created_at: item.last_created_at,
                            is_latest: item.is_latest,
                        })
                        .collect(),
                    selected_block_id: page.selected_block_id,
                    messages: project_messages_for_frontend_display_only(messages),
                    has_prev_block: page.has_prev_block,
                    has_next_block: page.has_next_block,
                });
            }

            let mut messages = conversation.messages.clone();
            materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
            Ok(ConversationBlockPageResult {
                blocks: vec![ConversationBlockSummaryResult {
                    block_id: 0,
                    message_count: messages.len(),
                    first_message_id: messages
                        .first()
                        .map(|message| message.id.clone())
                        .unwrap_or_default(),
                    last_message_id: messages
                        .last()
                        .map(|message| message.id.clone())
                        .unwrap_or_default(),
                    first_created_at: messages.first().map(|message| message.created_at.clone()),
                    last_created_at: messages.last().map(|message| message.created_at.clone()),
                    is_latest: true,
                }],
                selected_block_id: 0,
                messages: project_messages_for_frontend_display_only(messages),
                has_prev_block: false,
                has_next_block: false,
            })
        })
    }

    // 前端消息展示专用读取：返回值已经做过展示投影，禁止写路径/撤回路径复用。
    fn get_recent_messages_for_frontend_display_only(
        &self,
        state: &AppState,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, String> {
        Ok(project_messages_for_frontend_display_only(
            self.get_raw_recent_messages(state, conversation_id, limit)?,
        ))
    }

    fn get_all_messages(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Vec<ChatMessage>, String> {
        let mut messages =
            self.with_unarchived_conversation_by_id_fast(state, conversation_id, |conversation| {
                Ok(conversation.messages.clone())
            })?;
        materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
        Ok(project_messages_for_frontend_display_only(messages))
    }

    fn get_recent_block_messages(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Vec<ChatMessage>, String> {
        let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
        let mut messages = if let Some(page) =
            message_store::chat_store_read_recent_messages_page_cached(
                &store_paths,
                DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT,
            )?
        {
            page.messages
        } else {
            let conversation = state_read_conversation_cached(state, conversation_id)?;
            self.ensure_unarchived_conversation(&conversation, conversation_id)?;
            let total = conversation.messages.len();
            let start = total.saturating_sub(DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT);
            conversation.messages[start..].to_vec()
        };
        materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
        Ok(project_messages_for_frontend_display_only(messages))
    }

    fn get_active_conversation_messages(
        &self,
        state: &AppState,
        input: &SessionSelector,
    ) -> Result<Vec<ChatMessage>, String> {
        let conversation_id = input
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "conversationId is required.".to_string())?;
        self.get_all_messages(state, conversation_id)
    }

    // 前端消息展示专用读取：返回值已经做过展示投影，禁止写路径/撤回路径复用。
    fn get_message_by_id_for_frontend_display_only(
        &self,
        state: &AppState,
        conversation_id: &str,
        message_id: &str,
    ) -> Result<ChatMessage, String> {
        Ok(project_message_for_frontend_display_only(
            self.get_raw_message_by_id(state, conversation_id, message_id)?,
        ))
    }

    fn read_messages_before_internal(
        &self,
        state: &AppState,
        conversation_id: &str,
        before_message_id: &str,
        limit: usize,
    ) -> Result<(Vec<ChatMessage>, bool), String> {
        let normalized_before_message_id = before_message_id.trim();
        if normalized_before_message_id.is_empty() {
            return Err("beforeMessageId is required.".to_string());
        }
        let normalized_limit = limit.clamp(1, 100);
        let conversation_id = conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }

        let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
        let (mut page, has_more) = if let Some(page) =
            message_store::chat_store_read_messages_before(
                &store_paths,
                normalized_before_message_id,
                normalized_limit,
            )?
        {
            (page.messages, page.has_more)
        } else {
            self.with_unarchived_conversation_by_id_fast(state, conversation_id, |conversation| {
                clone_messages_before_page(
                    &conversation.messages,
                    normalized_before_message_id,
                    normalized_limit,
                )
            })?
        };

        materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
        Ok((project_messages_for_frontend_display_only(page), has_more))
    }

    fn read_messages_after_internal(
        &self,
        state: &AppState,
        conversation_id: &str,
        after_message_id: &str,
        limit: usize,
    ) -> Result<Vec<ChatMessage>, String> {
        let normalized_after_message_id = after_message_id.trim();
        if normalized_after_message_id.is_empty() {
            return Err("afterMessageId is required.".to_string());
        }
        let normalized_limit = limit.clamp(1, 100);
        let conversation_id = conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }

        let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
        let mut page = if let Some(page) = message_store::chat_store_read_messages_after(
            &store_paths,
            normalized_after_message_id,
            normalized_limit,
        )? {
            page.messages
        } else {
            self.with_unarchived_conversation_by_id_fast(state, conversation_id, |conversation| {
                clone_messages_after_page(
                    &conversation.messages,
                    normalized_after_message_id,
                    normalized_limit,
                )
            })?
        };

        materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
        Ok(project_messages_for_frontend_display_only(page))
    }

    fn get_messages_before(
        &self,
        state: &AppState,
        conversation_id: &str,
        anchor_message_id: &str,
        limit: usize,
    ) -> Result<ConversationMessagePageView, String> {
        let (messages, has_more) = self.read_messages_before_internal(
            state,
            conversation_id,
            anchor_message_id,
            limit,
        )?;
        Ok(build_message_page_view_v2(messages, has_more, false))
    }

    fn get_compaction_segment_before_anchor(
        &self,
        state: &AppState,
        conversation_id: &str,
        anchor_message_id: &str,
    ) -> Result<ConversationMessagePageView, String> {
        let normalized_anchor = anchor_message_id.trim();
        if normalized_anchor.is_empty() {
            return Err("anchorMessageId is required.".to_string());
        }
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let store_paths = message_store::message_store_paths(&state.data_path, normalized_conversation_id)?;
        let segment = if let Some(segment) = message_store::chat_store_read_compaction_segment_before_anchor(
            &store_paths,
            normalized_anchor,
        )? {
            segment
        } else {
            self.with_unarchived_conversation_by_id_fast(state, normalized_conversation_id, |conversation| {
                let messages = &conversation.messages;
                let anchor_idx = messages
                    .iter()
                    .position(|m| m.id.trim() == normalized_anchor)
                    .ok_or_else(|| format!("Message not found: {normalized_anchor}"))?;
                let boundaries: Vec<usize> = messages
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, m)| {
                        let kind = m.provider_meta.as_ref()
                            .and_then(|meta| meta.get("message_meta").or_else(|| meta.get("messageMeta")))
                            .and_then(|meta| meta.get("kind")).and_then(|v| v.as_str()).map(str::trim).unwrap_or("");
                        if kind == "context_compaction" || kind == "summary_context_seed" { Some(idx) } else { None }
                    })
                    .collect();
                let anchor_is_compaction = boundaries.contains(&anchor_idx);
                let start = if anchor_is_compaction {
                    let pos = boundaries.iter().position(|&idx| idx == anchor_idx).unwrap();
                    if pos == 0 { 0 } else { boundaries[pos - 1] }
                } else {
                    boundaries.iter().rev().find(|&&idx| idx < anchor_idx).copied().unwrap_or(0)
                };
                let has_previous = start > 0;
                Ok(message_store::MessageStoreCompactionSegment {
                    messages: messages[start..anchor_idx].to_vec(),
                    boundary_message_id: messages.get(start).and_then(|m| {
                        let kind = m.provider_meta.as_ref()
                            .and_then(|meta| meta.get("message_meta").or_else(|| meta.get("messageMeta")))
                            .and_then(|meta| meta.get("kind")).and_then(|v| v.as_str()).map(str::trim).unwrap_or("");
                        if kind == "context_compaction" || kind == "summary_context_seed" { Some(m.id.trim().to_string()) } else { None }
                    }),
                    previous_boundary_message_id: None,
                    has_previous_segment: has_previous,
                })
            })?
        };
        let mut messages = segment.messages;
        materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
        let projected = project_messages_for_frontend_display_only(messages);
        Ok(build_message_page_view_v2(projected, segment.has_previous_segment, false))
    }

    fn get_messages_after(
        &self,
        state: &AppState,
        conversation_id: &str,
        anchor_message_id: &str,
        limit: usize,
    ) -> Result<ConversationMessagePageView, String> {
        let messages = self.read_messages_after_internal(
            state,
            conversation_id,
            anchor_message_id,
            limit,
        )?;
        let has_more_after = messages.len() >= limit.clamp(1, 100);
        Ok(build_message_page_view_v2(
            messages,
            false,
            has_more_after,
        ))
    }

    fn get_messages_after_with_fallback(
        &self,
        state: &AppState,
        conversation_id: &str,
        after_message_id: Option<&str>,
        fallback_limit: usize,
    ) -> Result<(Vec<ChatMessage>, Option<String>), String> {
        let trimmed_after = after_message_id
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
        let (mut page, fallback_mode) = if let Some(after_id) = trimmed_after {
            if let Some(after_page) =
                message_store::chat_store_read_messages_after(&store_paths, after_id, 100)?
            {
                (after_page.messages, None)
            } else {
                self.with_unarchived_conversation_by_id_fast(state, conversation_id, |conversation| {
                    let messages = &conversation.messages;
                    let page_result = if let Some(after_idx) =
                        messages.iter().position(|item| item.id == after_id)
                    {
                        (messages[(after_idx + 1)..].to_vec(), None)
                    } else {
                        let start = messages.len().saturating_sub(fallback_limit);
                        (messages[start..].to_vec(), Some("recent_limit".to_string()))
                    };
                    Ok(page_result)
                })?
            }
        } else if let Some(page) =
            message_store::chat_store_read_recent_messages_page_cached(
                &store_paths,
                fallback_limit,
            )?
        {
            (
                page.messages,
                Some("recent_limit_in_latest_block".to_string()),
            )
        } else {
            self.with_unarchived_conversation_by_id_fast(state, conversation_id, |conversation| {
                let messages = &conversation.messages;
                let start = messages.len().saturating_sub(fallback_limit);
                Ok((messages[start..].to_vec(), Some("recent_limit".to_string())))
            })?
        };
        materialize_chat_message_parts_from_media_refs(&mut page, &state.data_path);
        Ok((project_messages_for_frontend_display_only(page), fallback_mode))
    }

}
