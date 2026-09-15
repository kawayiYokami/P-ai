impl ConversationServiceV2 {
    fn list_tool_session_targets(
        &self,
        state: &AppState,
        keyword: Option<&str>,
    ) -> Result<Vec<ToolSessionTargetSummary>, String> {
        let runtime_snapshot = load_runtime_organization_snapshot(state)?;
        let agents = runtime_snapshot.agents;
        let local_items = self
            .collect_unarchived_conversation_summaries_cached(state)?
            .into_iter()
            .filter(|item| !item.is_system_notification_conversation)
            .filter_map(|item| {
                let conversation_meta =
                    self.get_conversation_meta(state, &item.conversation_id).ok()?;
                if !self.conversation_meta_is_local_normal_chat_meta_view(&conversation_meta) {
                    return None;
                }
                let persona_name = agents
                    .iter()
                    .find(|agent| agent.id == conversation_meta.agent_id)
                    .map(|agent| agent.name.trim().to_string())
                    .filter(|name| !name.is_empty());
                let title = if !item.title.trim().is_empty() {
                    item.title.trim().to_string()
                } else if let Some(summary_title) = item.summary_title.as_deref().map(str::trim) {
                    summary_title.to_string()
                } else {
                    item.conversation_id.clone()
                };
                let haystacks = vec![
                    title.clone(),
                    item.summary_title.unwrap_or_default(),
                    persona_name.clone().unwrap_or_default(),
                ];
                if !session_search_hit(&haystacks, keyword) {
                    return None;
                }
                Some(ToolSessionTargetSummary {
                    session_id: item.conversation_id.clone(),
                    kind: "local_unarchived".to_string(),
                    title,
                    persona_name,
                    remote_contact_id: None,
                    remote_contact_name: None,
                    channel_name: None,
                    updated_at: item.updated_at,
                })
            })
            .collect::<Vec<_>>();

        let remote_items = self
            .list_remote_im_contact_conversations(state)?
            .into_iter()
            .filter_map(|item| {
                let persona_name = item
                    .bound_agent_id
                    .as_deref()
                    .and_then(|agent_id| {
                        agents
                            .iter()
                            .find(|agent| agent.id.trim() == agent_id.trim())
                            .map(|agent| agent.name.trim().to_string())
                    })
                    .filter(|value| !value.is_empty());
                let haystacks = vec![
                    item.title.clone(),
                    item.contact_display_name.clone(),
                    item.channel_name.clone().unwrap_or_default(),
                    persona_name.clone().unwrap_or_default(),
                ];
                if !session_search_hit(&haystacks, keyword) {
                    return None;
                }
                Some(ToolSessionTargetSummary {
                    session_id: item.conversation_id,
                    kind: "remote_im_contact".to_string(),
                    title: item.title,
                    persona_name,
                    remote_contact_id: Some(item.contact_id),
                    remote_contact_name: Some(item.contact_display_name),
                    channel_name: item.channel_name,
                    updated_at: item.updated_at,
                })
            })
            .collect::<Vec<_>>();

        let mut items = Vec::<ToolSessionTargetSummary>::new();
        items.extend(local_items);
        items.extend(remote_items);
        items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then_with(|| a.title.cmp(&b.title)));
        Ok(items)
    }

    fn list_remote_im_contact_conversations(
        &self,
        state: &AppState,
    ) -> Result<Vec<RemoteImContactConversationSummary>, String> {
        let mut contacts = state_service_list_remote_im_contacts(state, None)?;
        let config = load_runtime_organization_snapshot(state)?.config;
        let mut resolved_pairs = Vec::<(RemoteImContact, String)>::new();
        let mut sync_pairs = Vec::<(RemoteImContact, String)>::new();
        let mut runtime_changed = false;
        let mut binding_updates = Vec::<(
            String,
            RemoteImContactBindingSnapshot,
            RemoteImContactBindingSnapshot,
        )>::new();
        for contact in contacts.iter_mut() {
            let binding_baseline = remote_im_contact_binding_snapshot(contact);
            if remote_im_channel_by_id(&config, &contact.channel_id).is_none() {
                if contact
                    .bound_conversation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some()
                {
                    contact.bound_conversation_id = None;
                    runtime_changed = true;
                    binding_updates.push((
                        contact.id.clone(),
                        binding_baseline,
                        remote_im_contact_binding_snapshot(contact),
                    ));
                }
                continue;
            }
            let previous_bound_conversation_id = contact
                .bound_conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let previous_bound_agent_id = contact
                .bound_agent_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let binding_agent_id = match resolve_contact_agent_id(state, contact.bound_agent_id.as_deref()) {
                Ok(agent_id) => Some(agent_id),
                Err(err) => {
                    runtime_log_warn(format!(
                        "[远程IM] 跳过，任务=同步联系人会话绑定，contact_id={}，原因={}",
                        contact.id, err
                    ));
                    None
                }
            };
            if let Some(agent_id) = binding_agent_id.as_ref() {
                contact.bound_agent_id = Some(agent_id.clone());
            }
            let target_key = remote_im_contact_conversation_key(contact);
            let conversation_id = previous_bound_conversation_id
                .as_deref()
                .and_then(|conversation_id| {
                    self.get_conversation_meta(state, conversation_id)
                        .ok()
                        .filter(|conversation_meta| {
                            self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                                && conversation_meta.is_remote_im_contact
                        })
                        .map(|conversation_meta| conversation_meta.id.to_string())
                })
                .or_else(|| {
                    state_read_chat_index_cached(state)
                        .ok()?
                        .conversations
                        .iter()
                        .filter_map(|item| self.get_conversation_meta(state, item.id.as_str()).ok())
                        .find(|conversation_meta| {
                            self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                                && conversation_meta.is_remote_im_contact
                                && conversation_meta.root_conversation_id.as_deref()
                                    == Some(target_key.as_str())
                        })
                        .map(|conversation_meta| conversation_meta.id.to_string())
                });
            let Some(conversation_id) = conversation_id else {
                if previous_bound_conversation_id.is_some() {
                    contact.bound_conversation_id = None;
                    runtime_changed = true;
                    binding_updates.push((
                        contact.id.clone(),
                        binding_baseline,
                        remote_im_contact_binding_snapshot(contact),
                    ));
                }
                continue;
            };
            contact.bound_conversation_id = Some(conversation_id.clone());
            let binding_changed = previous_bound_conversation_id.as_deref() != Some(conversation_id.as_str())
                || previous_bound_agent_id.as_deref()
                    != contact.bound_agent_id.as_deref().map(str::trim);
            if binding_changed {
                runtime_changed = true;
                binding_updates.push((
                    contact.id.clone(),
                    binding_baseline,
                    remote_im_contact_binding_snapshot(contact),
                ));
            }
            resolved_pairs.push((contact.clone(), conversation_id.clone()));
            if binding_agent_id.is_some() {
                sync_pairs.push((contact.clone(), conversation_id));
            }
        }
        if runtime_changed {
            for (contact_id, baseline, resolved) in binding_updates {
                let Some(mut latest_contact) =
                    state_service_get_remote_im_contact(state, &contact_id)?
                else {
                    continue;
                };
                if !remote_im_contact_binding_matches(&latest_contact, &baseline) {
                    runtime_log_warn(format!(
                        "[联系人会话] 跳过过期绑定修复，contact_id={}，原因=用户配置已变化",
                        contact_id
                    ));
                    continue;
                }
                remote_im_apply_contact_binding_snapshot(&mut latest_contact, &resolved);
                state_service_upsert_remote_im_contact(state, &latest_contact)?;
            }
        }
        for (contact, conversation_id) in &sync_pairs {
            if let Err(err) = sync_remote_im_contact_conversation_binding(
                state,
                contact,
                conversation_id,
            ) {
                runtime_log_warn(format!(
                    "[联系人会话] 会话列表继续返回，路由同步降级，contact_id={}，conversation_id={}，error={}",
                    contact.id, conversation_id, err
                ));
            }
        }
        let mut items = Vec::<RemoteImContactConversationSummary>::new();
        for (contact, conversation_id) in resolved_pairs {
            let store_paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
            let channel = remote_im_channel_by_id(&config, &contact.channel_id);
            let summary = if let Some(meta) = message_store::chat_store_read_meta(&store_paths)? {
                let current_store_status = message_store::chat_store_read_status(&store_paths)?
                    .ok_or_else(|| format!("联系人会话缺少消息存储状态：{conversation_id}"))?;
                let preview_messages = self
                    .read_remote_im_contact_preview_messages(state, &conversation_id, 2)
                    .unwrap_or_default();
                Some(RemoteImContactConversationSummary {
                    contact_id: contact.id.clone(),
                    conversation_id: conversation_id.clone(),
                    title: remote_im_contact_conversation_title(&contact),
                    updated_at: meta.updated_at().to_string(),
                    last_message_at: meta
                        .last_assistant_at()
                        .map(ToOwned::to_owned)
                        .or_else(|| meta.last_user_at().map(ToOwned::to_owned))
                        .or_else(|| Some(meta.updated_at().to_string())),
                    message_count: current_store_status.message_count,
                    channel_id: contact.channel_id.clone(),
                    channel_name: channel
                        .as_ref()
                        .map(|item| item.name.trim().to_string())
                        .filter(|value| !value.is_empty()),
                    channel_enabled: channel.as_ref().map(|item| item.enabled).unwrap_or(false),
                    platform: contact.platform.clone(),
                    contact_display_name: remote_im_contact_display_name(&contact),
                    bound_agent_id: contact.bound_agent_id.clone(),
                    processing_mode: normalize_contact_processing_mode(&contact.processing_mode),
                    preview_messages,
                })
            } else {
                let conversation = match self.try_read_unarchived_conversation(state, &conversation_id)? {
                    Some(conversation) if conversation_is_remote_im_contact(&conversation) => conversation,
                    _ => continue,
                };
                Some(RemoteImContactConversationSummary {
                    contact_id: contact.id.clone(),
                    conversation_id: conversation.id.clone(),
                    title: remote_im_contact_conversation_title(&contact),
                    updated_at: conversation.updated_at.clone(),
                    last_message_at: conversation.messages.last().map(|message| message.created_at.clone()),
                    message_count: conversation.messages.len(),
                    channel_id: contact.channel_id.clone(),
                    channel_name: channel
                        .as_ref()
                        .map(|item| item.name.trim().to_string())
                        .filter(|value| !value.is_empty()),
                    channel_enabled: channel.as_ref().map(|item| item.enabled).unwrap_or(false),
                    platform: contact.platform.clone(),
                    contact_display_name: remote_im_contact_display_name(&contact),
                    bound_agent_id: contact.bound_agent_id.clone(),
                    processing_mode: normalize_contact_processing_mode(&contact.processing_mode),
                    preview_messages: build_conversation_preview_messages(&conversation, 2),
                })
            };
            if let Some(item) = summary {
                items.push(item);
            }
        }
        items.sort_by(|a, b| {
            let bk = b.last_message_at.as_deref().unwrap_or(b.updated_at.as_str());
            let ak = a.last_message_at.as_deref().unwrap_or(a.updated_at.as_str());
            bk.cmp(ak).then_with(|| b.updated_at.cmp(&a.updated_at))
        });
        Ok(items)
    }

    fn get_remote_im_contact_conversation_messages(
        &self,
        state: &AppState,
        contact_id: &str,
    ) -> Result<Vec<ChatMessage>, String> {
        let normalized_contact_id = contact_id.trim();
        if normalized_contact_id.is_empty() {
            return Err("contact_id 为必填项。".to_string());
        }
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let runtime_contact = state_service_get_remote_im_contact(state, normalized_contact_id)?
            .ok_or_else(|| format!("未找到远程联系人：{normalized_contact_id}"))?;
        let conversation_id = if let Some(conversation_id) = runtime_contact
            .bound_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            conversation_id.to_string()
        } else {
            let target_key = remote_im_contact_conversation_key(&runtime_contact);
            let chat_index = state_read_chat_index_cached(state)?;
            chat_index
                .conversations
                .iter()
                .filter_map(|item| self.get_conversation_meta(state, item.id.as_str()).ok())
                .find(|conversation_meta| {
                    self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                        && conversation_meta.is_remote_im_contact
                        && conversation_meta.root_conversation_id.as_deref()
                            == Some(target_key.as_str())
                })
                .map(|conversation_meta| conversation_meta.id.to_string())
                .ok_or_else(|| format!("联系人未绑定联系人会话：{normalized_contact_id}"))?
        };
        drop(guard);
        let store_paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
        let mut messages = if let Some(page) =
            message_store::chat_store_read_recent_messages_page_cached(
                &store_paths,
                DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT,
            )?
        {
            let _ = self.retain_message_store_block_cache_whitelist(state);
            page.messages
        } else {
            self.with_unarchived_conversation_by_id_fast(state, &conversation_id, |conversation| {
                let total = conversation.messages.len();
                let start = total.saturating_sub(DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT);
                Ok(conversation.messages[start..].to_vec())
            })?
        };
        materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
        Ok(project_messages_for_frontend_display_only(messages))
    }

    fn get_remote_im_contact_conversation_block_page(
        &self,
        state: &AppState,
        contact_id: &str,
        requested_block_id: Option<u32>,
    ) -> Result<ConversationBlockPageResult, String> {
        let normalized_contact_id = contact_id.trim();
        if normalized_contact_id.is_empty() {
            return Err("contact_id 为必填项。".to_string());
        }
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let runtime_contact = state_service_get_remote_im_contact(state, normalized_contact_id)?
            .ok_or_else(|| format!("未找到远程联系人：{normalized_contact_id}"))?;
        let conversation_id = if let Some(conversation_id) = runtime_contact
            .bound_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            conversation_id.to_string()
        } else {
            let target_key = remote_im_contact_conversation_key(&runtime_contact);
            let chat_index = state_read_chat_index_cached(state)?;
            chat_index
                .conversations
                .iter()
                .filter_map(|item| self.get_conversation_meta(state, item.id.as_str()).ok())
                .find(|conversation_meta| {
                    self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                        && conversation_meta.is_remote_im_contact
                        && conversation_meta.root_conversation_id.as_deref()
                            == Some(target_key.as_str())
                })
                .map(|conversation_meta| conversation_meta.id.to_string())
                .ok_or_else(|| format!("联系人未绑定联系人会话：{normalized_contact_id}"))?
        };
        let conversation_meta = self.get_conversation_meta(state, &conversation_id)?;
        if !self.conversation_meta_is_unarchived_meta_view(&conversation_meta)
            || !conversation_meta.is_remote_im_contact
        {
            drop(guard);
            return Err(format!("联系人未绑定联系人会话：{normalized_contact_id}"));
        }
        drop(guard);

        let store_paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
        if let Some(page) =
            message_store::chat_store_read_block_page(&store_paths, requested_block_id)?
        {
            let _ = self.retain_message_store_block_cache_whitelist(state);
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

        let conversation = self.read_persisted_conversation(state, &conversation_id)?;
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
    }

    fn clear_remote_im_contact_conversation(
        &self,
        state: &AppState,
        contact_id: &str,
    ) -> Result<bool, String> {
        let normalized_contact_id = contact_id.trim();
        if normalized_contact_id.is_empty() {
            return Err("contact_id 为必填项。".to_string());
        }
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let contact = state_service_get_remote_im_contact(state, normalized_contact_id)?
            .ok_or_else(|| format!("未找到远程联系人：{normalized_contact_id}"))?;
        let conversation_id = contact
            .bound_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| {
                let target_key = remote_im_contact_conversation_key(&contact);
                match state_read_chat_index_cached(state) {
                    Ok(chat_index) => chat_index
                        .conversations
                        .iter()
                        .filter_map(|item| match self.get_conversation_meta(state, item.id.as_str()) {
                            Ok(conversation_meta) => Some(conversation_meta),
                            Err(err) => {
                                runtime_log_warn(format!(
                                    "[联系人会话] 警告，任务=clear_remote_im_contact_conversation_lookup，conversation_id={}，contact_id={}，error={}",
                                    item.id,
                                    normalized_contact_id,
                                    err
                                ));
                                None
                            }
                        })
                        .find(|conversation_meta| {
                            self.conversation_meta_is_unarchived_meta_view(conversation_meta)
                                && conversation_meta.is_remote_im_contact
                                && conversation_meta.root_conversation_id.as_deref()
                                    == Some(target_key.as_str())
                        })
                        .map(|conversation_meta| conversation_meta.id.to_string()),
                    Err(err) => {
                        runtime_log_warn(format!(
                            "[联系人会话] 警告，任务=clear_remote_im_contact_read_chat_index，contact_id={}，error={}",
                            normalized_contact_id, err
                        ));
                        None
                    }
                }
            });
        let Some(conversation_id) = conversation_id else {
            drop(guard);
            return Ok(false);
        };
        let conversation_meta = match self.get_conversation_meta(state, &conversation_id) {
            Ok(conversation_meta)
                if self.conversation_meta_is_unarchived_meta_view(&conversation_meta)
                    && conversation_meta.is_remote_im_contact =>
            {
                conversation_meta
            }
            _ => {
                drop(guard);
                return Ok(false);
            }
        };

        drop(guard);
        let cleared = state_service_clear_remote_im_contact_binding_if_matches(
            state,
            normalized_contact_id,
            &conversation_meta.id,
        )?;
        if !cleared {
            runtime_log_warn(format!(
                "[联系人会话] 跳过清空联系人会话，contact_id={}，conversation_id={}，原因=绑定已被其他请求修改",
                normalized_contact_id, conversation_meta.id
            ));
            return Ok(false);
        }
        let latest_checkpoint = state_service_get_remote_im_contact_checkpoint(state, normalized_contact_id)?;
        let atomic_revision = latest_checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.atomic_revision.saturating_add(1).max(1))
            .unwrap_or(1);
        state_service_set_remote_im_contact_checkpoint(
            state,
            &RemoteImContactCheckpoint {
                contact_id: normalized_contact_id.to_string(),
                atomic_revision,
                updated_at: Some(now_iso()),
                ..RemoteImContactCheckpoint::default()
            },
        )?;
        state_schedule_conversation_delete(state, &conversation_meta.id)?;
        Ok(true)
    }

    fn inform_session(
        &self,
        state: &AppState,
        source_conversation_id: &str,
        target_session_id: &str,
        content: &str,
    ) -> Result<InformSessionMutationResult, String> {
        let normalized_target_session_id = target_session_id.trim();
        if normalized_target_session_id.is_empty() {
            return Err("session_id 不能为空".to_string());
        }
        let body = build_session_notification_body(state, source_conversation_id, content)?;
        let message = build_session_notification_message(&body);
        enqueue_session_notification_dispatch(
            state,
            normalized_target_session_id,
            &body,
            &message,
            "inform_session",
        )?;
        Ok(InformSessionMutationResult {
            target_conversation_id: normalized_target_session_id.to_string(),
            target_kind: "queued".to_string(),
            remote_contact_id: None,
            pushed_to_remote: false,
            message,
        })
    }

    async fn update_unarchived_conversation_by_id<T>(
        &self,
        state: &AppState,
        conversation_id: &str,
        updater: impl FnOnce(&mut Conversation) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String>
    where
        T: Send + 'static,
    {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required.".to_string());
        }
        let service = conversation_service_v2();
        let state_for_mutation = state.clone();
        let conversation_id_for_mutation = normalized_conversation_id.to_string();
        with_conversation_mutation_async(
            state.clone(),
            conversation_id_for_mutation.clone(),
            "update_unarchived_conversation_by_id".to_string(),
            move || {
                // 工具审查按 call_id 定位，而现有 locator 未索引 tool_call_id；这是该入口必须读取
                // 全量正文的唯一原因。当前发布仍限制为变更消息的 block 级替换，禁止回退整会话快照。
                let mut conversation = service.read_persisted_conversation(
                    &state_for_mutation,
                    &conversation_id_for_mutation,
                )?;
                service.ensure_unarchived_conversation(
                    &conversation,
                    &conversation_id_for_mutation,
                )?;
                let original_messages = conversation.messages.clone();
                let result = updater(&mut conversation)?;
                let store_paths = message_store::message_store_paths(
                    &state_for_mutation.data_path,
                    &conversation_id_for_mutation,
                )?;
                if conversation.messages.len() != original_messages.len()
                    || conversation
                        .messages
                        .iter()
                        .zip(original_messages.iter())
                        .any(|(updated, original)| updated.id != original.id)
                {
                    return Err(format!(
                        "当前消息存储不支持通过 update_unarchived_conversation_by_id 改变消息结构，conversation_id={conversation_id_for_mutation}"
                    ));
                }
                let changed_messages = conversation
                    .messages
                    .iter()
                    .zip(original_messages.iter())
                    .filter_map(|(updated, original)| {
                        (serde_json::to_value(updated).ok() != serde_json::to_value(original).ok())
                            .then(|| updated.clone())
                    })
                    .collect::<Vec<_>>();
                let (updated_meta_conversation, (), _) =
                    state_update_conversation_metadata_cached_unlocked(
                        &state_for_mutation,
                        &conversation_id_for_mutation,
                        |cached| {
                            preserve_field_level_conversation_metadata(cached, &conversation);
                            Ok(())
                        },
                    )?;
                if !changed_messages.is_empty() {
                    let mut ready_meta = service.ensure_appendable_ready_message_store(
                        &state_for_mutation,
                        &conversation_id_for_mutation,
                    )?;
                    ready_meta
                        .apply_metadata_fields_from_conversation(&updated_meta_conversation);
                    let previous_messages = changed_messages
                        .iter()
                        .filter_map(|updated| {
                            original_messages
                                .iter()
                                .find(|original| original.id == updated.id)
                                .cloned()
                        })
                        .collect::<Vec<_>>();
                    ready_meta.apply_replaced_messages(&previous_messages, &changed_messages, || {
                        Ok(conversation_latest_summary_title(&conversation))
                    })?;
                    message_store::chat_store_replace_messages(
                        &store_paths,
                        &ready_meta.to_persist_meta(),
                        &changed_messages,
                    )?;
                    service.mark_conversation_metadata_cached_persisted(
                        &state_for_mutation,
                        &conversation_id_for_mutation,
                    )?;
                    state_override_conversation_metadata_cached(
                        &state_for_mutation,
                        &conversation_id_for_mutation,
                        &ready_meta,
                    )?;
                }
                Ok(result)
            },
        )
        .await
    }

    fn append_fast_request_turn_if_unarchived_exists(
        &self,
        state: &AppState,
        conversation_id: &str,
        turn: FastRequestTurn,
    ) -> Result<bool, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Ok(false);
        }
        with_conversation_mutation(
            state,
            normalized_conversation_id,
            "append_fast_request_turn_if_unarchived_exists",
            || {
                let conversation_meta = match self.get_conversation_meta(state, normalized_conversation_id) {
                    Ok(conversation_meta) => conversation_meta,
                    Err(_) => return Ok(false),
                };
                if !self.conversation_meta_is_unarchived_meta_view(&conversation_meta) {
                    return Ok(false);
                }
                state_update_conversation_meta_cached_unlocked(state, normalized_conversation_id, |meta| {
                    meta.push_fast_request_turn(turn);
                    Ok(())
                })?;
                Ok(true)
            },
        )
    }

    fn get_conversation_fast_request_turns(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Vec<FastRequestTurn>, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required".to_string());
        }
        match state_read_conversation_metadata_cached(state, normalized_conversation_id) {
            Ok(meta) => Ok(meta.fast_request_turns().to_vec()),
            Err(root_error) => match delegate_runtime_thread_conversation_get_any(
                state,
                normalized_conversation_id,
            ) {
                Ok(Some(conversation)) => Ok(conversation.fast_request_turns),
                Ok(None) => Err(root_error),
                Err(delegate_error) => Err(format!(
                    "读取会话杂务失败：root_error={}，delegate_error={}",
                    root_error, delegate_error
                )),
            },
        }
    }

    fn prune_expired_remote_im_fast_request_turns(
        &self,
        state: &AppState,
        cutoff: OffsetDateTime,
    ) -> Result<(usize, Vec<String>), String> {
        let index = state_read_chat_index_cached(state)?;
        let mut removed = 0usize;
        let mut errors = Vec::new();
        for item in index.conversations {
            let result = with_conversation_mutation(
                state,
                &item.id,
                "prune_expired_remote_im_fast_request_turns",
                || {
                    let meta = self.get_conversation_meta(state, &item.id).map_err(|err| {
                        format!("conversation_id={}，读取元数据失败：{}", item.id, err)
                    })?;
                    if !meta.is_remote_im_contact || meta.fast_request_turns.is_empty() {
                        return Ok(0usize);
                    }
                    let expired_count = meta.fast_request_turns.iter()
                        .filter(|turn| remote_im_maintenance_is_expired_at(&turn.created_at, cutoff))
                        .count();
                    if expired_count == 0 {
                        return Ok(0usize);
                    }
                    state_update_conversation_meta_cached_unlocked(state, &item.id, |stored| {
                        stored.retain_fast_request_turns(|turn| {
                            !remote_im_maintenance_is_expired_at(&turn.created_at, cutoff)
                        });
                        Ok(())
                    })
                    .map_err(|err| format!("conversation_id={}，清理杂务失败：{}", item.id, err))?;
                    Ok(expired_count)
                },
            );
            match result {
                Ok(expired_count) => removed = removed.saturating_add(expired_count),
                Err(err) => errors.push(err),
            }
        }
        Ok((removed, errors))
    }

    fn get_active_goal(
        &self,
        state: &AppState,
        conversation_id: &str,
    ) -> Result<Option<ConversationGoalState>, String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required".to_string());
        }
        if let Ok(conversation_meta) = self.get_conversation_meta(state, normalized_conversation_id)
        {
            return Ok(conversation_meta
                .active_goal
                .as_ref()
                .filter(|goal| conversation_goal_is_active(goal))
                .cloned());
        }
        let conversation = delegate_runtime_thread_conversation_get(state, normalized_conversation_id)?
            .ok_or_else(|| format!("Conversation not found: {normalized_conversation_id}"))?;
        Ok(conversation
            .active_goal
            .as_ref()
            .filter(|goal| conversation_goal_is_active(goal))
            .cloned())
    }

    fn update_goal_conversation<T>(
        &self,
        state: &AppState,
        conversation_id: &str,
        task_name: &str,
        updater: impl FnOnce(&mut Conversation) -> Result<T, String>,
    ) -> Result<(Conversation, T), String> {
        let normalized_conversation_id = conversation_id.trim();
        if normalized_conversation_id.is_empty() {
            return Err("conversationId is required".to_string());
        }
        if self
            .get_conversation_meta(state, normalized_conversation_id)
            .is_ok()
        {
            return with_conversation_mutation(
                state,
                normalized_conversation_id,
                task_name,
                || {
                    let (conversation, result, _) = state_update_conversation_metadata_cached_unlocked(
                        state,
                        normalized_conversation_id,
                        updater,
                    )?;
                    Ok((conversation, result))
                },
            );
        }
        let _guard = lock_conversation_with_metrics(state, task_name)?;
        let mut conversation = delegate_runtime_thread_conversation_get(state, normalized_conversation_id)?
            .ok_or_else(|| format!("Conversation not found: {normalized_conversation_id}"))?;
        let result = updater(&mut conversation)?;
        delegate_runtime_thread_conversation_update(
            state,
            normalized_conversation_id,
            conversation.clone(),
        )?;
        Ok((conversation, result))
    }

    fn remote_im_runtime_state_should_cache_blocks(
        &self,
        runtime_state: &RemoteImContactRuntimeState,
    ) -> bool {
        runtime_state.presence_state == RemoteImPresenceState::Present
            || runtime_state.work_state == RemoteImWorkState::Busy
            || runtime_state.has_pending
    }

    fn collect_block_cache_whitelist_conversation_ids(
        &self,
        state: &AppState,
    ) -> Result<std::collections::HashSet<String>, String> {
        let mut ids = std::collections::HashSet::<String>::new();
        if let Ok(bindings) = state.active_chat_view_bindings.lock() {
            for binding in bindings.values() {
                let conversation_id = binding.conversation_id.trim();
                if !conversation_id.is_empty() {
                    ids.insert(conversation_id.to_string());
                }
            }
        }
        let active_contact_ids = state
            .remote_im_contact_runtime_states
            .lock()
            .map(|runtime_states| {
                runtime_states
                    .iter()
                    .filter(|(_, runtime_state)| {
                        self.remote_im_runtime_state_should_cache_blocks(runtime_state)
                    })
                    .map(|(contact_id, _)| contact_id.trim().to_string())
                    .filter(|contact_id| !contact_id.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !active_contact_ids.is_empty() {
            let contact_ids = active_contact_ids
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            let contacts = state_service_list_remote_im_contacts(state, None)?;
            let mut unresolved_contact_ids = std::collections::HashSet::<String>::new();
            for contact in contacts
                .iter()
                .filter(|contact| contact_ids.contains(contact.id.trim()))
            {
                if let Some(bound_conversation_id) = contact
                    .bound_conversation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    ids.insert(bound_conversation_id.to_string());
                } else {
                    unresolved_contact_ids.insert(contact.id.trim().to_string());
                }
            }
            if !unresolved_contact_ids.is_empty() {
                let chat_index = state_read_chat_index_cached(state)?;
                let conversation_key_map = contacts
                    .iter()
                    .filter(|contact| unresolved_contact_ids.contains(contact.id.trim()))
                    .map(|contact| {
                        (
                            remote_im_contact_conversation_key(contact),
                            contact.id.trim().to_string(),
                        )
                    })
                    .collect::<std::collections::HashMap<_, _>>();
                let mapped_ids = chat_index
                    .conversations
                    .iter()
                    .filter_map(|item| {
                        let conversation_meta = match self.get_conversation_meta(
                            state,
                            item.id.as_str(),
                        ) {
                            Ok(conversation_meta) => conversation_meta,
                            Err(err) => {
                                runtime_log_error(format!(
                                    "[会话索引读取] 状态=失败，任务=collect_block_cache_whitelist_conversation_ids，conversation_id={}，error={}",
                                    item.id, err
                                ));
                                return None;
                            }
                        };
                        let root_key = conversation_meta.root_conversation_id.as_deref()?;
                        if !self.conversation_meta_is_unarchived_meta_view(&conversation_meta)
                            || !conversation_meta.is_remote_im_contact
                            || !conversation_key_map.contains_key(root_key)
                        {
                            return None;
                        }
                        Some(conversation_meta.id.to_string())
                    })
                    .collect::<Vec<_>>();
                ids.extend(mapped_ids);
            }
        }
        Ok(ids)
    }

    fn retain_message_store_block_cache_whitelist(
        &self,
        state: &AppState,
    ) -> Result<(), String> {
        let conversation_ids = self.collect_block_cache_whitelist_conversation_ids(state)?;
        let mut allowed_paths = std::collections::HashSet::<PathBuf>::new();
        for conversation_id in conversation_ids {
            let paths = message_store::message_store_paths(&state.data_path, &conversation_id)?;
            if let Some(block_paths) =
                message_store::chat_store_read_latest_block_paths(&paths, 2)?
            {
                allowed_paths.extend(block_paths);
            }
        }
        message_store::retain_message_store_block_file_cache_paths(&allowed_paths);
        Ok(())
    }

    fn read_remote_im_contact_preview_messages(
        &self,
        state: &AppState,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<ConversationPreviewMessage>, String> {
        let normalized_limit = limit.max(1);
        let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
        if let Some(page) = message_store::chat_store_read_recent_messages_page_cached(
            &store_paths,
            normalized_limit,
        )? {
            let mut messages = page.messages;
            materialize_chat_message_parts_from_media_refs(&mut messages, &state.data_path);
            return Ok(build_preview_messages_from_chat_messages(&messages, normalized_limit));
        }
        self.with_unarchived_conversation_by_id_fast(state, conversation_id, |conversation| {
            Ok(build_conversation_preview_messages(conversation, normalized_limit))
        })
    }

    fn resolve_remote_im_contact_conversation_id_for_notification(
        &self,
        state: &AppState,
        remote_contact_id: &str,
    ) -> Result<String, String> {
        let normalized_remote_contact_id = remote_contact_id.trim();
        if normalized_remote_contact_id.is_empty() {
            return Err("remoteContactId 不能为空".to_string());
        }
        let contact = state_service_get_remote_im_contact(state, normalized_remote_contact_id)?
            .ok_or_else(|| format!("未找到远程联系人：{normalized_remote_contact_id}"))?;
        let config = state_read_config_cached(state)?;
        let channel = remote_im_channel_by_id(&config, &contact.channel_id)
            .ok_or_else(|| format!("远程联系人所属渠道不存在：{}", contact.channel_id))?;
        if !channel.enabled {
            return Err(format!("远程联系人所属渠道未启用：{}", contact.channel_id));
        }
        let previous_bound_conversation_id = contact
            .bound_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let (_, conversation_id, _contact) =
            remote_im_resolve_contact_session_target_atomic(
                state,
                normalized_remote_contact_id,
                contact,
            )?;
        if previous_bound_conversation_id.as_deref() != Some(conversation_id.as_str()) {
            runtime_log_info(format!(
                "[自动推送] 完成，任务=修复远程联系人绑定会话，remote_contact_id={}，conversation_id={}，previous_conversation_id={}",
                normalized_remote_contact_id,
                conversation_id,
                previous_bound_conversation_id.as_deref().unwrap_or("")
            ));
        } else {
            runtime_log_info(format!(
                "[自动推送] 完成，任务=解析远程联系人绑定会话，remote_contact_id={}，conversation_id={}",
                normalized_remote_contact_id, conversation_id
            ));
        }
        Ok(conversation_id)
    }

    async fn deliver_session_notification(
        &self,
        state: &AppState,
        target_session_id: &str,
        body: &str,
        message: &ChatMessage,
        action: &str,
    ) -> Result<(), String> {
        let normalized_target_session_id = target_session_id.trim();
        runtime_log_info(format!(
            "[会话通知] 节点，任务=投递入口，action={}，target_conversation_id={}，message_id={}",
            action,
            normalized_target_session_id,
            message.id
        ));
        let app_config = state_read_config_cached(state)?;
        runtime_log_info(format!(
            "[会话通知] 节点，任务=投递配置读取完成，action={}，target_conversation_id={}，message_id={}",
            action,
            normalized_target_session_id,
            message.id
        ));
        let target_conversation_meta = self
            .get_conversation_meta(state, normalized_target_session_id)
            .map_err(|_| "目标会话不存在".to_string())?;
        runtime_log_info(format!(
            "[会话通知] 节点，任务=投递元数据读取完成，action={}，target_conversation_id={}，message_id={}",
            action,
            normalized_target_session_id,
            message.id
        ));
        if !self.conversation_meta_is_unarchived_meta_view(&target_conversation_meta) {
            return Err("目标会话不存在".to_string());
        }

        if target_conversation_meta.is_remote_im_contact {
            let contact = state_service_list_remote_im_contacts(state, None)?
                .into_iter()
                .find(|contact| {
                    contact
                        .bound_conversation_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        == Some(normalized_target_session_id)
                })
                .ok_or_else(|| "目标远程联系人不存在".to_string())?;
            let channel = remote_im_channel_by_id(&app_config, &contact.channel_id)
                .cloned()
                .ok_or_else(|| format!("远程 IM 渠道不存在: {}", contact.channel_id))?;
            if !channel.enabled {
                return Err(format!("远程 IM 渠道未启用: {}", contact.channel_id));
            }
            if !contact.allow_send {
                return Err("当前联系人不允许发送消息".to_string());
            }
            runtime_log_info(format!(
                "[会话通知] 开始，任务=远程联系人投递，action={}，target_conversation_id={}，remote_contact_id={}，channel_id={}，message_id={}",
                action,
                normalized_target_session_id,
                contact.id,
                contact.channel_id,
                message.id
            ));
            remote_im_send_content_payload(
                state,
                &channel,
                &contact,
                vec![serde_json::json!({
                    "type": "text",
                    "text": body,
                })],
                false,
                action,
            )
            .await?;
            runtime_log_info(format!(
                "[会话通知] 完成，任务=远程联系人投递，action={}，target_conversation_id={}，remote_contact_id={}，channel_id={}，message_id={}",
                action,
                normalized_target_session_id,
                contact.id,
                contact.channel_id,
                message.id
            ));
        } else if !target_conversation_meta.visible_in_foreground_lists
            || !self.conversation_meta_is_local_normal_chat_meta_view(&target_conversation_meta)
        {
            return Err("目标会话不存在".to_string());
        }

        runtime_log_info(format!(
            "[会话通知] 节点，任务=投递前置检查完成，action={}，target_conversation_id={}，message_id={}，is_remote_im_contact={}",
            action,
            normalized_target_session_id,
            message.id,
            target_conversation_meta.is_remote_im_contact
        ));
        self.append_message(state, normalized_target_session_id, message).await?;
        runtime_log_info(format!(
            "[会话通知] 节点，任务=通知消息写入完成，action={}，target_conversation_id={}，message_id={}",
            action,
            normalized_target_session_id,
            message.id
        ));
        emit_conversation_message_appended_event(state, normalized_target_session_id, message);
        Ok(())
    }

}
