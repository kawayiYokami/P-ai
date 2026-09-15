#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImSecretaryMessageDigest {
    time_text: String,
    speaker: String,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImSecretaryDecisionReply {
    #[serde(default, alias = "should_reply")]
    should_reply: bool,
    #[serde(default, alias = "target_delegate_id")]
    target_delegate_id: Option<String>,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Clone)]
struct RemoteImSecretaryDecision {
    should_reply: bool,
    target_delegate_id: Option<String>,
    reason: String,
    model_name: String,
    emit_log: bool,
}

fn remote_im_secretary_contact_type_label(contact_type: &str) -> &str {
    match contact_type.trim().to_ascii_lowercase().as_str() {
        "group" => "群聊",
        "private" => "私聊",
        _ => "联系人",
    }
}

fn remote_im_secretary_contact_display_name(contact: &RemoteImContact) -> String {
    let remote_id = contact.remote_contact_id.trim();
    let remark_name = contact.remark_name.trim();
    if !remark_name.is_empty() && remark_name != remote_id {
        return remark_name.to_string();
    }
    let remote_name = contact.remote_contact_name.trim();
    if !remote_name.is_empty() && remote_name != remote_id {
        return remote_name.to_string();
    }
    remote_im_secretary_contact_type_label(&contact.remote_contact_type).to_string()
}

fn remote_im_secretary_context_display_name(name: &str, id: &str, fallback_name: &str) -> String {
    let name = name.trim();
    let id = id.trim();
    if !name.is_empty() && name != id {
        name.to_string()
    } else {
        fallback_name.to_string()
    }
}

fn remote_im_secretary_named_label(
    prefix: &str,
    name: &str,
    id: &str,
    fallback_name: &str,
    include_id: bool,
) -> String {
    let prefix = prefix.trim();
    let name = name.trim();
    let id = id.trim();
    let resolved_name = if !name.is_empty() && name != id {
        name
    } else if !fallback_name.trim().is_empty() {
        fallback_name.trim()
    } else if !prefix.is_empty() {
        prefix
    } else {
        "未知"
    };
    let base_label = if prefix.is_empty() || prefix == resolved_name {
        resolved_name.to_string()
    } else {
        format!("{prefix} {resolved_name}")
    };
    if include_id && !id.is_empty() {
        format!("{base_label}/{id}")
    } else {
        base_label
    }
}

fn remote_im_secretary_current_assistant_context(
    state: &AppState,
    conversation_id: &str,
) -> Result<RemoteImConversationAssistantContext, String> {
    get_conversation_remote_im_assistant_context(state, conversation_id)?
        .ok_or_else(|| format!("缺少当前助理上下文: conversation_id={}", conversation_id.trim()))
}

fn remote_im_resolve_contact_assistant_context(
    state: &AppState,
    contact: &RemoteImContact,
) -> Result<RemoteImConversationAssistantContext, String> {
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let agent_id = contact
        .bound_agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("联系人未设置应答人格: {}", contact.id))?;
    let agent = runtime_agent_by_id(&runtime_snapshot, agent_id)
        .ok_or_else(|| format!("路由人格不存在: {agent_id}"))?;
    let agent_name = if agent.name.trim().is_empty() {
        agent.id.clone()
    } else {
        agent.name.trim().to_string()
    };
    Ok(RemoteImConversationAssistantContext {
        agent_id: agent.id.clone(),
        agent_name,
    })
}

fn remote_im_secretary_message_speaker_label(
    message: &ChatMessage,
    contact: &RemoteImContact,
    agents: &[AgentProfile],
    current_assistant: &RemoteImConversationAssistantContext,
) -> Option<String> {
    match message.role.trim() {
        "assistant" => {
            let speaker_id = message
                .speaker_agent_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(current_assistant.agent_id.as_str());
            let speaker_name = agents
                .iter()
                .find(|agent| agent.id == speaker_id)
                .map(|agent| agent.name.trim().to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| {
                    if speaker_id == current_assistant.agent_id {
                        current_assistant.agent_name.clone()
                    } else if speaker_id.is_empty() {
                        "当前助理".to_string()
                    } else {
                        speaker_id.to_string()
                    }
                });
            Some(remote_im_secretary_named_label(
                "",
                &speaker_name,
                speaker_id,
                "当前助理",
                false,
            ))
        }
        "user" => {
            if let Some(origin) = remote_im_origin_from_message(message) {
                let contact_type = remote_im_origin_string(origin, "contact_type")
                    .unwrap_or(contact.remote_contact_type.as_str());
                if contact_type.eq_ignore_ascii_case("group") {
                    let sender_name = remote_im_origin_string(origin, "sender_name").unwrap_or("");
                    let sender_id = remote_im_origin_string(origin, "sender_id").unwrap_or("");
                    return Some(remote_im_secretary_named_label(
                        "群友",
                        sender_name,
                        sender_id,
                        "群友",
                        true,
                    ));
                }
                let fallback_contact_name = remote_im_secretary_contact_display_name(contact);
                let contact_name = remote_im_origin_string(origin, "contact_name")
                    .unwrap_or(fallback_contact_name.as_str());
                let contact_id = remote_im_origin_string(origin, "contact_id")
                    .unwrap_or(contact.remote_contact_id.as_str());
                return Some(remote_im_secretary_named_label(
                    "",
                    contact_name,
                    contact_id,
                    "联系人",
                    true,
                ));
            }
            let fallback_contact_name = remote_im_secretary_contact_display_name(contact);
            Some(remote_im_secretary_named_label(
                "",
                &fallback_contact_name,
                contact.remote_contact_id.as_str(),
                "联系人",
                true,
            ))
        }
        _ => None,
    }
}

fn remote_im_secretary_truncate_text(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect::<String>()
}

fn remote_im_secretary_message_time_text(created_at: &str) -> String {
    let time_text = format_utc_storage_time_to_local_relative_label(created_at);
    if time_text.trim().is_empty() {
        "时间未知".to_string()
    } else {
        time_text
    }
}

fn remote_im_secretary_message_line(
    item: &RemoteImSecretaryMessageDigest,
    latest_suffix: &str,
) -> String {
    format!(
        "[{}]({}){}：{}",
        item.speaker, item.time_text, latest_suffix, item.text
    )
}

fn remote_im_secretary_message_digest(
    message: &ChatMessage,
    contact: &RemoteImContact,
    agents: &[AgentProfile],
    current_assistant: &RemoteImConversationAssistantContext,
) -> Option<RemoteImSecretaryMessageDigest> {
    if is_context_compaction_message(message, message.role.trim()) {
        return None;
    }
    let speaker = remote_im_secretary_message_speaker_label(
        message,
        contact,
        agents,
        current_assistant,
    )?;
    let mut chunks = Vec::<String>::new();
    for part in &message.parts {
        match part {
            MessagePart::Text { text, .. } => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    chunks.push(trimmed.to_string());
                }
            }
            MessagePart::Image { .. } => chunks.push("[图片]".to_string()),
            MessagePart::Audio { .. } => chunks.push("[音频]".to_string()),
            MessagePart::Attachment { mime, .. } => chunks.push(match message_attachment_kind(mime) {
                "image" => "[图片]".to_string(),
                "audio" => "[音频]".to_string(),
                "pdf" => "[PDF]".to_string(),
                _ => "[附件]".to_string(),
            }),
        }
    }
    for block in &message.extra_text_blocks {
        let trimmed = block.trim();
        if !trimmed.is_empty() {
            chunks.push(trimmed.to_string());
        }
    }
    if chunks.is_empty() {
        return None;
    }
    Some(RemoteImSecretaryMessageDigest {
        time_text: remote_im_secretary_message_time_text(&message.created_at),
        speaker,
        text: remote_im_secretary_truncate_text(&chunks.join("\n"), 100),
    })
}

fn remote_im_collect_secretary_recent_messages(
    messages: &[ChatMessage],
    limit: usize,
    contact: &RemoteImContact,
    agents: &[AgentProfile],
    current_assistant: &RemoteImConversationAssistantContext,
) -> Vec<RemoteImSecretaryMessageDigest> {
    if limit == 0 {
        return Vec::new();
    }
    let mut selected = Vec::<RemoteImSecretaryMessageDigest>::new();
    for message in messages.iter().rev() {
        if let Some(digest) =
            remote_im_secretary_message_digest(message, contact, agents, current_assistant)
        {
            selected.push(digest);
            if selected.len() >= limit {
                break;
            }
        }
    }
    selected.reverse();
    selected
}

fn remote_im_secretary_messages_to_text(
    messages: &[RemoteImSecretaryMessageDigest],
    mark_latest_last: bool,
) -> String {
    if messages.is_empty() {
        return "（无）".to_string();
    }
    messages
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let latest_suffix = if mark_latest_last && idx + 1 == messages.len() {
                "（最新）"
            } else {
                ""
            };
            remote_im_secretary_message_line(item, latest_suffix)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_remote_im_secretary_prepared_prompt(
    language: &str,
    contact: &RemoteImContact,
    response_guidance: &str,
    current_assistant: &RemoteImConversationAssistantContext,
    history_messages: &[RemoteImSecretaryMessageDigest],
    new_batch_messages: &[RemoteImSecretaryMessageDigest],
    work_ledger: &str,
) -> PreparedPrompt {
    let guidance = normalize_remote_im_channel_response_guidance(response_guidance);
    let contact_name = remote_im_secretary_contact_display_name(contact);
    let contact_type = remote_im_secretary_contact_type_label(&contact.remote_contact_type);
    let agent_name = remote_im_secretary_context_display_name(
        &current_assistant.agent_name,
        &current_assistant.agent_id,
        "当前助理",
    );
    PreparedPrompt {
        preamble: format!(
            "请使用{language}完成远程联系人应答判断。\n\
你是正式处理助理入场前的秘书，只负责判断这一次是否应该回应，不负责代写回复。\n\
你会收到两段内容：最近 7 条已处理历史消息，以及本次未处理新消息。每条消息以 [发言人/ID](本地差异时间标签) 开头，助理消息可能没有 ID；正文只保留了前 100 个字，信息不足时不要过度推断。\n\
“未处理边界”之后的消息按时间从旧到新排列，最后一条就是最新消息，应优先围绕它判断是否需要回应。\n\
请优先遵守“什么时候应该回答”这段规则；如果规则不够，再按常识判断。\n\
如果无法确定，倾向于 shouldReply=false。\n\
只返回一个 JSON 对象，不要输出 Markdown、代码块或额外解释。\n\
JSON 只能包含字段：shouldReply, targetDelegateId, reason。"
        ),
        history_messages: Vec::new(),
        latest_user_text: format!(
            "当前助理：\n\
- 名称：{}\n\n\
当前联系人：\n\
- 名称：{contact_name}\n\
- 类型：{contact_type}\n\n\
什么时候应该回答：\n{guidance}\n\n\
最近 7 条已处理历史消息\n{}\n\n\
================ 未处理边界 ================\n\
以下是本次未处理新消息，按时间从旧到新排列，最后一条是最新消息\n{}\n\n\
助理工作账本：
{}

如果新消息应继续账本中某个运行中委托，targetDelegateId 必须填该委托 ID；如果是独立问题或没有运行中委托，targetDelegateId 留空。\n\n请直接输出 JSON。",
            agent_name,
            remote_im_secretary_messages_to_text(history_messages, false),
            remote_im_secretary_messages_to_text(new_batch_messages, true),
            work_ledger,
        ),
        latest_user_meta_text: String::new(),
        latest_user_extra_text: String::new(),
        latest_user_extra_blocks: Vec::new(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    }
}

fn remote_im_secretary_extract_json(raw: &str) -> &str {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        return stripped.trim().trim_end_matches("```").trim();
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        return stripped.trim().trim_end_matches("```").trim();
    }
    trimmed
}

fn remote_im_resolve_secretary_contact(
    state: &AppState,
    activated_sources: &[RemoteImActivationSource],
) -> Result<Option<RemoteImContact>, String> {
    let Some(source) = activated_sources.first() else {
        return Ok(None);
    };
    if activated_sources.len() > 1 {
        runtime_log_warn(format!(
            "[远程联系人秘书] 本轮激活联系人超过 1 个，跳过秘书判断: source_count={}",
            activated_sources.len()
        ));
        return Ok(None);
    }
    let contact = state_service_find_remote_im_contact_by_identity(
        state,
        &source.channel_id,
        &source.remote_contact_type,
        &source.remote_contact_id,
    )?;
    Ok(contact)
}

async fn run_remote_im_secretary_decision(
    state: &AppState,
    contact: &RemoteImContact,
    current_assistant: &RemoteImConversationAssistantContext,
    history_messages: &[RemoteImSecretaryMessageDigest],
    new_batch_messages: &[RemoteImSecretaryMessageDigest],
    work_ledger: &str,
    active_delegate_ids: &[String],
) -> Result<RemoteImSecretaryDecision, String> {
    if effective_remote_im_contact_response_strategy(contact) == "always_reply" {
        return Ok(RemoteImSecretaryDecision {
            should_reply: true,
            target_delegate_id: None,
            reason: String::new(),
            model_name: String::new(),
            emit_log: false,
        });
    }

    let review_api_config_id = current_tool_review_api_config_id(state)?
        .ok_or_else(|| "未配置快速模型".to_string())?;
    let app_config = state_read_config_cached(state)?;
    let selected_api = resolve_selected_api_config(&app_config, Some(&review_api_config_id))
        .ok_or_else(|| format!("快速模型配置不存在：{}", review_api_config_id))?;
    if !selected_api.enable_text || !selected_api.request_format.is_chat_text() {
        return Err("快速模型不支持文本对话".to_string());
    }
    let resolved_api = resolve_api_config(&app_config, Some(&review_api_config_id))?;
    let model_name = if selected_api.model.trim().is_empty() {
        resolved_api.model.clone()
    } else {
        selected_api.model.trim().to_string()
    };
    let language = terminal_smart_review_language(&app_config.ui_language);
    let prepared = build_remote_im_secretary_prepared_prompt(
        language,
        contact,
        &effective_remote_im_channel_response_guidance(state, contact),
        current_assistant,
        history_messages,
        new_batch_messages,
        work_ledger,
    );
    let request_text = prepared_prompt_to_fast_request_text(&prepared);
    let record_conversation_id = contact
        .bound_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let execution = invoke_model_with_policy(
        &resolved_api,
        &model_name,
        prepared,
        CallPolicy {
            scene: "Remote IM secretary review",
            timeout_secs: Some(60),
            json_only: true,
        },
        Some(state),
        Vec::new(),
    )
    .await;
    push_model_call_log_parts(Some(state), &execution);
    let duration_ms = execution.log_parts.elapsed_ms;
    let reply = match execution.result {
        Ok(reply) => reply,
        Err(err) => {
            if let Some(conversation_id) = record_conversation_id.as_deref() {
                record_fast_request_turn_best_effort(
                    state,
                    conversation_id,
                    build_fast_request_turn(
                        FAST_REQUEST_KIND_REMOTE_IM_REPLY_DECISION,
                        &request_text,
                        "",
                        false,
                        Some(err.clone()),
                        Some(model_name.clone()),
                        Some(duration_ms),
                    ),
                );
            }
            return Err(err);
        }
    };
    let raw_text = if reply.final_response_text.trim().is_empty() {
        reply.assistant_text.trim()
    } else {
        reply.final_response_text.trim()
    };
    let parsed = match serde_json::from_str::<RemoteImSecretaryDecisionReply>(
        remote_im_secretary_extract_json(raw_text),
    )
    {
        Ok(parsed) => {
            if let Some(conversation_id) = record_conversation_id.as_deref() {
                record_fast_request_turn_best_effort(
                    state,
                    conversation_id,
                    build_fast_request_turn(
                        FAST_REQUEST_KIND_REMOTE_IM_REPLY_DECISION,
                        &request_text,
                        raw_text,
                        true,
                        None,
                        Some(model_name.clone()),
                        Some(duration_ms),
                    ),
                );
            }
            parsed
        }
        Err(err) => {
            let message = format!("解析秘书 JSON 失败: {err}; raw={}", raw_text.trim());
            if let Some(conversation_id) = record_conversation_id.as_deref() {
                record_fast_request_turn_best_effort(
                    state,
                    conversation_id,
                    build_fast_request_turn(
                        FAST_REQUEST_KIND_REMOTE_IM_REPLY_DECISION,
                        &request_text,
                        raw_text,
                        false,
                        Some(message.clone()),
                        Some(model_name.clone()),
                        Some(duration_ms),
                    ),
                );
            }
            return Err(message);
        }
    };
    Ok(RemoteImSecretaryDecision {
        should_reply: parsed.should_reply,
        target_delegate_id: parsed
            .target_delegate_id
            .map(|value| value.trim().to_string())
            .filter(|value| active_delegate_ids.iter().any(|item| item == value)),
        reason: parsed.reason.trim().to_string(),
        model_name,
        emit_log: true,
    })
}
