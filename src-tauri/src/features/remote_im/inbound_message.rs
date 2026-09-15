fn validate_images(channel: &RemoteImChannelConfig, input: &RemoteImEnqueueInput) -> Vec<BinaryPart> {
    if channel.receive_files {
        input.payload.images.clone().unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn validate_audios(channel: &RemoteImChannelConfig, input: &RemoteImEnqueueInput) -> Vec<BinaryPart> {
    if channel.receive_files {
        input.payload.audios.clone().unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn validate_attachments(
    channel: &RemoteImChannelConfig,
    input: &RemoteImEnqueueInput,
) -> Vec<AttachmentMetaInput> {
    if channel.receive_files {
        input.payload.attachments.clone().unwrap_or_default()
    } else {
        Vec::new()
    }
}

fn resolve_channel_config(
    input: &RemoteImEnqueueInput,
    config: &AppConfig,
) -> Result<(String, RemoteImChannelConfig), String> {
    let channel_id = input.channel_id.trim().to_string();
    if channel_id.is_empty() {
        return Err("channel_id 不能为空".to_string());
    }
    let channel = remote_im_channel_by_id(config, &channel_id)
        .ok_or_else(|| format!("远程IM渠道不存在: {channel_id}"))?
        .clone();
    if !channel.enabled {
        return Err(format!("远程IM渠道未启用: {channel_id}"));
    }
    Ok((channel_id, channel))
}

fn resolve_contact_agent_id(
    state: &AppState,
    requested_agent_id: Option<&str>,
) -> Result<String, String> {
    let requested_agent_id = requested_agent_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let default_agent_id = state_service_get_assistant_agent_id(state)?;
    let hint_agent_id = if let Some(agent_id) = requested_agent_id.as_deref() {
        agent_id
    } else {
        default_agent_id.as_str()
    };

    let snapshot = load_runtime_organization_snapshot(state)?;
    // 显式绑定的人格已被删除时回落到助理人格，不让这条路由直接断掉。
    let agent = match runtime_agent_by_id(&snapshot, hint_agent_id) {
        Some(agent) => agent,
        None => {
            runtime_log_warn(format!(
                "[远程IM] 路由人格已不存在，回落到助理人格: agent_id={hint_agent_id}"
            ));
            runtime_agent_by_id(&snapshot, &default_agent_id)
                .ok_or_else(|| format!("助理人格不存在: {default_agent_id}"))?
        }
    };
    agent_primary_chat_api_config_id(&snapshot.config, agent)
        .ok_or_else(|| format!("人格模型未配置或不可用于聊天: {}", agent.id))?;
    Ok(agent.id.clone())
}

fn validate_enqueue_input(
    input: &RemoteImEnqueueInput,
    config: &AppConfig,
) -> Result<ValidatedEnqueueInput, String> {
    let text = input.payload.text.as_deref().unwrap_or("").trim().to_string();
    let (_channel_id, channel) = resolve_channel_config(input, config)?;
    let images = validate_images(&channel, input);
    let audios = validate_audios(&channel, input);
    let attachments = validate_attachments(&channel, input);

    let mut parts_has_content = false;
    let mut parts_image_count = 0usize;
    let mut parts_audio_count = 0usize;
    let mut parts_attachment_count = 0usize;
    if let Some(parts) = input.payload.parts.as_ref() {
        for part in parts {
            match part {
                ChatIngressPart::Text { text: t } => {
                    if !t.trim().is_empty() {
                        parts_has_content = true;
                    }
                }
                ChatIngressPart::Attachment { mime, .. } => {
                    if channel.receive_files {
                        parts_has_content = true;
                        let normalized_mime = mime.trim().to_ascii_lowercase();
                        if normalized_mime.starts_with("image/") {
                            parts_image_count += 1;
                        } else if normalized_mime.starts_with("audio/") {
                            parts_audio_count += 1;
                        } else {
                            parts_attachment_count += 1;
                        }
                    }
                }
            }
        }
    }

    if text.is_empty()
        && images.is_empty()
        && audios.is_empty()
        && attachments.is_empty()
        && !parts_has_content
    {
        return Err("远程IM消息内容为空".to_string());
    }

    Ok(ValidatedEnqueueInput {
        text,
        images,
        audios,
        attachments,
        parts_image_count,
        parts_audio_count,
        parts_attachment_count,
        channel,
    })
}

fn ensure_remote_im_contact_conversation_id(
    state: &AppState,
    contact: &mut RemoteImContact,
) -> Result<String, String> {
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
    if let Some(bound_conversation_id) = contact
        .bound_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|conversation_id| {
            conversation_service_v2()
                .get_conversation_meta(state, conversation_id)
                .ok()
                .filter(|conversation_meta| {
                    remote_im_meta_is_reusable_active_contact_conversation(conversation_meta)
                })
                .map(|conversation_meta| conversation_meta.id.to_string())
        })
    {
        contact.bound_conversation_id = Some(bound_conversation_id.clone());
        return Ok(bound_conversation_id);
    }

    let target_key = remote_im_contact_conversation_key(contact);
    if let Some(found_id) = state_read_chat_index_cached(state)?
        .conversations
        .iter()
        .filter_map(|item| conversation_service_v2().get_conversation_meta(state, item.id.as_str()).ok())
        .find(|conversation_meta| {
            remote_im_meta_is_reusable_active_contact_conversation(conversation_meta)
                && conversation_meta.root_conversation_id.as_deref() == Some(target_key.as_str())
        })
        .map(|conversation_meta| conversation_meta.id.to_string())
    {
        contact.bound_conversation_id = Some(found_id.clone());
        return Ok(found_id);
    }

    let agent_id = binding_agent_id.unwrap_or_default();
    let conversation = conversation_service_v2().create_remote_im_contact_conversation(
        state,
        &remote_im_contact_conversation_title(contact),
        &agent_id,
        &target_key,
    )?;
    let conversation_id = conversation.id.clone();
    contact.bound_conversation_id = Some(conversation_id.clone());
    Ok(conversation_id)
}

fn remote_im_contact_conversation_sync_lock(
    state: &AppState,
    contact_id: &str,
) -> std::sync::Arc<std::sync::Mutex<()>> {
    static LOCKS: std::sync::OnceLock<
        std::sync::Mutex<
            std::collections::HashMap<String, std::sync::Arc<std::sync::Mutex<()>>>,
        >,
    > = std::sync::OnceLock::new();
    let key = format!(
        "{}::{}",
        state.data_path.to_string_lossy(),
        contact_id.trim()
    );
    let locks = LOCKS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let mut locks = match locks.lock() {
        Ok(locks) => locks,
        Err(poisoned) => {
            runtime_log_warn("[远程IM] 联系人会话同步锁表中毒，已恢复".to_string());
            poisoned.into_inner()
        }
    };
    locks
        .entry(key)
        .or_insert_with(|| std::sync::Arc::new(std::sync::Mutex::new(())))
        .clone()
}

fn sync_remote_im_contact_conversation_binding(
    state: &AppState,
    contact: &RemoteImContact,
    conversation_id: &str,
) -> Result<(), String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Ok(());
    }
    let sync_lock = remote_im_contact_conversation_sync_lock(state, &contact.id);
    let _sync_guard = match sync_lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            runtime_log_warn(format!(
                "[远程IM] 联系人会话同步锁中毒，已恢复，contact_id={}",
                contact.id
            ));
            poisoned.into_inner()
        }
    };
    let original_meta = conversation_service_v2()
        .get_conversation_meta(state, normalized_conversation_id)?;
    let mut last_written = None::<(RemoteImContact, String)>;
    for attempt in 0..4 {
        let Some(authoritative_contact) =
            state_service_get_remote_im_contact(state, &contact.id)?
        else {
            runtime_log_warn(format!(
                "[远程IM] 跳过，任务=同步联系人会话绑定，contact_id={}，conversation_id={}，原因=联系人已删除",
                contact.id, normalized_conversation_id
            ));
            return Ok(());
        };
        if authoritative_contact
            .bound_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            != Some(normalized_conversation_id)
        {
            runtime_log_warn(format!(
                "[远程IM] 跳过，任务=同步联系人会话绑定，contact_id={}，conversation_id={}，原因=会话绑定已变化",
                contact.id, normalized_conversation_id
            ));
            return Ok(());
        }
        let agent_id = match resolve_contact_agent_id(
            state,
            authoritative_contact.bound_agent_id.as_deref(),
        ) {
            Ok(agent_id) => agent_id,
            Err(err) => {
                runtime_log_warn(format!(
                    "[远程IM] 跳过，任务=同步联系人会话绑定，contact_id={}，conversation_id={}，原因={} ",
                    contact.id, normalized_conversation_id, err
                ));
                return Ok(());
            }
        };
        let baseline = remote_im_contact_binding_snapshot(&authoritative_contact);
        last_written = Some((authoritative_contact.clone(), agent_id.clone()));
        if let Err(err) = sync_remote_im_contact_conversation_binding_unchecked(
            state,
            &authoritative_contact,
            normalized_conversation_id,
            &agent_id,
        ) {
            runtime_log_warn(format!(
                "[远程IM] 会话绑定写入失败，尝试条件回滚，contact_id={}，conversation_id={}，error={}",
                contact.id, normalized_conversation_id, err
            ));
            if let Err(rollback_err) = restore_remote_im_contact_conversation_binding(
                state,
                normalized_conversation_id,
                &original_meta,
                &authoritative_contact,
                &agent_id,
            ) {
                runtime_log_warn(format!(
                    "[远程IM] 会话绑定写入失败后的回滚降级，contact_id={}，conversation_id={}，error={}",
                    contact.id, normalized_conversation_id, rollback_err
                ));
            }
            return Err(err);
        }
        let latest = match state_service_get_remote_im_contact(state, &contact.id) {
            Ok(latest) => latest,
            Err(err) => {
                runtime_log_warn(format!(
                    "[远程IM] 会话绑定写后复核失败，回滚本次路由变更，contact_id={}，conversation_id={}，error={}",
                    contact.id, normalized_conversation_id, err
                ));
                restore_remote_im_contact_conversation_binding(
                    state,
                    normalized_conversation_id,
                    &original_meta,
                    &authoritative_contact,
                    &agent_id,
                )?;
                return Ok(());
            }
        };
        let Some(latest) = latest else {
            restore_remote_im_contact_conversation_binding(
                state,
                normalized_conversation_id,
                &original_meta,
                &authoritative_contact,
                &agent_id,
            )?;
            return Ok(());
        };
        if remote_im_contact_binding_matches(&latest, &baseline) {
            return Ok(());
        }
        runtime_log_warn(format!(
            "[远程IM] 联系人绑定在会话同步期间变化，按最新配置重试，contact_id={}，conversation_id={}，attempt={}",
            contact.id,
            normalized_conversation_id,
            attempt + 1
        ));
        if latest
            .bound_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            != Some(normalized_conversation_id)
        {
            restore_remote_im_contact_conversation_binding(
                state,
                normalized_conversation_id,
                &original_meta,
                &authoritative_contact,
                &agent_id,
            )?;
            return Ok(());
        }
    }
    runtime_log_warn(format!(
        "[远程IM] 联系人绑定持续变化，回滚本次会话路由变更，contact_id={}，conversation_id={}",
        contact.id, normalized_conversation_id
    ));
    if let Some((written_contact, written_agent_id)) = last_written {
        restore_remote_im_contact_conversation_binding(
            state,
            normalized_conversation_id,
            &original_meta,
            &written_contact,
            &written_agent_id,
        )?;
    }
    Ok(())
}

fn restore_remote_im_contact_conversation_binding(
    state: &AppState,
    conversation_id: &str,
    original_meta: &ConversationMetaView,
    written_contact: &RemoteImContact,
    written_agent_id: &str,
) -> Result<(), String> {
    let expected_root = remote_im_contact_conversation_key(written_contact);
    let (_, restored, _) = state_update_conversation_metadata_cached(
        state,
        conversation_id,
        |conversation| {
            if conversation.agent_id.trim() != written_agent_id.trim()
                || conversation.root_conversation_id.as_deref() != Some(expected_root.as_str())
            {
                return Ok(false);
            }
            conversation.agent_id = original_meta.agent_id.clone();
            conversation.root_conversation_id = original_meta.root_conversation_id.clone();
            conversation.conversation_kind = original_meta.conversation_kind.clone();
            if conversation.preferred_api_config_id.is_none() {
                conversation.preferred_api_config_id =
                    original_meta.preferred_api_config_id.clone();
            }
            Ok(true)
        },
    )?;
    if !restored {
        runtime_log_warn(format!(
            "[远程IM] 跳过过期会话路由回滚，conversation_id={}，原因=路由已被其他操作更新",
            conversation_id
        ));
    }
    Ok(())
}

fn sync_remote_im_contact_conversation_binding_unchecked(
    state: &AppState,
    contact: &RemoteImContact,
    conversation_id: &str,
    agent_id: &str,
) -> Result<(), String> {
    let conversation_meta = conversation_service_v2().get_conversation_meta(state, conversation_id)?;
    if conversation_meta.status.trim() == "archived"
        || conversation_meta
            .archived_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
        || conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_REMOTE_IM_CONTACT
    {
        return Ok(());
    }
    let target_key = remote_im_contact_conversation_key(contact);
    let agent_changed = conversation_meta.agent_id.trim() != agent_id;
    let root_changed = conversation_meta.root_conversation_id.as_deref() != Some(target_key.as_str());
    let preferred_api_changed = conversation_meta
        .preferred_api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    if agent_changed || root_changed || preferred_api_changed {
        state_update_conversation_metadata_cached(state, conversation_id, |conversation| {
            conversation.agent_id = agent_id.to_string();
            conversation.root_conversation_id = Some(target_key);
            conversation.preferred_api_config_id = None;
            Ok(())
        })?;
    }
    Ok(())
}

fn remote_im_meta_is_reusable_active_contact_conversation(
    conversation_meta: &ConversationMetaView,
) -> bool {
    conversation_meta.status.trim() != "archived"
        && conversation_meta
            .archived_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        && conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_REMOTE_IM_CONTACT
}

fn resolve_contact_session_target(
    state: &AppState,
    contact: &mut RemoteImContact,
) -> Result<(String, String), String> {
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let effective_route_mode =
        remote_im_resolve_effective_route_mode(&runtime_snapshot.config, contact);
    contact.route_mode = effective_route_mode.clone();

    let agent_id = resolve_contact_agent_id(state, contact.bound_agent_id.as_deref())?;
    contact.bound_agent_id = Some(agent_id.clone());
    let conversation_id = ensure_remote_im_contact_conversation_id(state, contact)?;
    Ok((agent_id, conversation_id))
}

fn build_chat_message_from_input(
    state: &AppState,
    input: &RemoteImEnqueueInput,
    conversation_id: &str,
    contact: &RemoteImContact,
    now: &str,
    text: &str,
    images: &[BinaryPart],
    audios: &[BinaryPart],
    attachments: &[AttachmentMetaInput],
) -> ChatMessage {
    let mut parts = Vec::<MessagePart>::new();
    let mut warnings = Vec::<String>::new();
    let contact_id = contact.id.trim();
    let downloads_subdir = remote_im_contact_downloads_subdir(contact);
    if let Some(ordered_parts) = input
        .payload
        .parts
        .as_ref()
        .filter(|items| !items.is_empty())
    {
        for ingress_part in ordered_parts {
            match ingress_part {
                ChatIngressPart::Text { text } => {
                    if !text.trim().is_empty() {
                        parts.push(MessagePart::Text {
                            text: text.trim().to_string(),
                            reasoning_content: None,
                        });
                    }
                }
                ChatIngressPart::Attachment {
                    path,
                    bytes_base64,
                    mime,
                    name,
                } => push_normalized_attachment_ingress(
                    state,
                    AttachmentIngressInput {
                        path: path.clone(),
                        bytes_base64: bytes_base64.clone(),
                        mime: mime.clone(),
                        name: name.clone(),
                        storage_subdir: Some(downloads_subdir.clone()),
                    },
                    &mut parts,
                    &mut warnings,
                ),
            }
        }
    } else {
        if !text.is_empty() {
            parts.push(MessagePart::Text {
                text: text.to_string(),
                reasoning_content: None,
            });
        }
        for image in images {
            push_normalized_attachment_ingress(
                state,
                AttachmentIngressInput {
                    path: image.saved_path.clone(),
                    bytes_base64: Some(image.bytes_base64.clone()),
                    mime: image.mime.clone(),
                    name: image
                        .saved_path
                        .as_deref()
                        .and_then(|path| std::path::Path::new(path).file_name())
                        .and_then(|value| value.to_str())
                        .unwrap_or("image")
                        .to_string(),
                    storage_subdir: Some(downloads_subdir.clone()),
                },
                &mut parts,
                &mut warnings,
            );
        }
        for audio in audios {
            push_normalized_attachment_ingress(
                state,
                AttachmentIngressInput {
                    path: audio.saved_path.clone(),
                    bytes_base64: Some(audio.bytes_base64.clone()),
                    mime: audio.mime.clone(),
                    name: audio
                        .saved_path
                        .as_deref()
                        .and_then(|path| std::path::Path::new(path).file_name())
                        .and_then(|value| value.to_str())
                        .unwrap_or("audio")
                        .to_string(),
                    storage_subdir: Some(downloads_subdir.clone()),
                },
                &mut parts,
                &mut warnings,
            );
        }
        for attachment in attachments {
            push_normalized_attachment_ingress(
                state,
                AttachmentIngressInput {
                    path: Some(attachment.path.clone()),
                    bytes_base64: None,
                    mime: attachment.mime.clone(),
                    name: attachment.file_name.clone(),
                    storage_subdir: Some(downloads_subdir.clone()),
                },
                &mut parts,
                &mut warnings,
            );
        }
    }
    if parts.is_empty() {
        parts.push(MessagePart::Text {
            text: "[附件不可用：本次远程消息中的附件未能完成规范化]".to_string(),
            reasoning_content: None,
        });
    }
    for warning in warnings {
        runtime_log_warn(format!(
            "[远程IM] 附件入站降级继续，conversation_id={}，contact_id={}，warning={}",
            conversation_id, contact_id, warning
        ));
    }

    let origin_meta = remote_im_set_sender_origin_meta(input, conversation_id, contact_id);
    let mut base_meta = input
        .payload
        .provider_meta
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(base_obj) = base_meta.as_object_mut() {
        base_obj.insert("origin".to_string(), origin_meta["origin"].clone());
    } else {
        base_meta = origin_meta;
    }
    let merged_meta = provider_meta_without_legacy_attachments(Some(base_meta));

    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now.to_string(),
        speaker_agent_id: None,
        parts,
        extra_text_blocks: input.payload.extra_text_blocks.clone().unwrap_or_default(),
        provider_meta: merged_meta,
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

fn create_pending_event(
    event_id: String,
    conversation_id: String,
    messages: Vec<ChatMessage>,
    activate_assistant: bool,
    session_info: ChatSessionInfo,
    sender_info: RemoteImMessageSource,
) -> ChatPendingEvent {
    let queue_mode = if activate_assistant && sender_info.remote_contact_type.trim().eq_ignore_ascii_case("private")
    {
        ChatQueueMode::Guided
    } else {
        ChatQueueMode::Normal
    };
    ChatPendingEvent {
        id: event_id,
        conversation_id,
        created_at: now_iso(),
        source: ChatEventSource::RemoteIm,
        queue_mode,
        messages,
        activate_assistant,
        assistant_message_id: None,
        session_info,
        runtime_context: None,
        sender_info: Some(sender_info),
    }
}
