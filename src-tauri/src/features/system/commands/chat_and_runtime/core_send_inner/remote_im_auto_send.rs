fn remote_im_is_reply_decision_action(action: &str) -> bool {
    matches!(
        action.trim().to_ascii_lowercase().as_str(),
        "reply_async" | "send_async"
    )
}

#[derive(Debug, Clone)]
struct RemoteImReplyDecisionSummary {
    action: String,
    target: Option<RemoteImReplyTarget>,
}

fn remote_im_extract_reply_decision_from_tool_history(
    _events: &[Value],
) -> Option<RemoteImReplyDecisionSummary> {
    None
}

fn remote_im_message_has_reply_decision(message: &ChatMessage) -> bool {
    if let Some(action) = message
        .provider_meta
        .as_ref()
        .and_then(|meta| meta.get("remoteImDecision"))
        .and_then(|value| value.get("action"))
        .and_then(Value::as_str)
    {
        if remote_im_is_reply_decision_action(action) {
            return true;
        }
    }
    message
        .tool_call
        .as_ref()
        .and_then(|events| remote_im_extract_reply_decision_from_tool_history(events))
        .map(|summary| summary.action)
        .is_some()
}

fn remote_im_latest_idle_boundary_message_at(
    conversation: &Conversation,
    ignore_trailing_user_message: bool,
) -> Option<&str> {
    conversation
        .messages
        .iter()
        .rev()
        .skip_while(|message| {
            ignore_trailing_user_message
                && message.role.trim() == "user"
                && !is_context_compaction_message(message, message.role.trim())
        })
        .map(|message| message.created_at.trim())
        .find(|value| !value.is_empty())
}

fn remote_im_idle_seconds_since_latest_message(
    conversation: &Conversation,
    ignore_trailing_user_message: bool,
) -> Option<i64> {
    if !conversation_is_remote_im_contact(conversation)
        || conversation.messages.len() < REMOTE_IM_AUTO_COMPACTION_MIN_MESSAGES
    {
        return None;
    }
    let latest_message_at =
        remote_im_latest_idle_boundary_message_at(conversation, ignore_trailing_user_message)?;
    let latest_message_at = parse_iso(latest_message_at)?;
    let elapsed_seconds = (now_utc() - latest_message_at).whole_seconds();
    (elapsed_seconds >= 0).then_some(elapsed_seconds)
}

fn remote_im_auto_compaction_idle_hours_if_due(
    conversation: &Conversation,
    ignore_trailing_user_message: bool,
) -> Option<i64> {
    let elapsed_seconds =
        remote_im_idle_seconds_since_latest_message(conversation, ignore_trailing_user_message)?;
    let threshold_seconds = REMOTE_IM_AUTO_COMPACTION_IDLE_HOURS * 3600;
    if elapsed_seconds < threshold_seconds {
        return None;
    }
    Some(elapsed_seconds / 3600)
}

fn remote_im_activation_source_summary_line(source: &RemoteImActivationSource) -> String {
    let mut parts = vec![
        format!("channel_id={}", source.channel_id.trim()),
        format!("contact_id={}", source.remote_contact_id.trim()),
    ];
    if !source.remote_contact_name.trim().is_empty() {
        parts.push(format!("contact_name={}", source.remote_contact_name.trim()));
    }
    if !source.remote_contact_type.trim().is_empty() {
        parts.push(format!("contact_type={}", source.remote_contact_type.trim()));
    }
    parts.join(" | ")
}

fn build_remote_im_activation_runtime_block(
    sources: &[RemoteImActivationSource],
    ui_language: &str,
) -> Option<String> {
    if sources.is_empty() {
        return None;
    }
    let source_lines = sources
        .iter()
        .map(|source| remote_im_activation_source_summary_line(source))
        .collect::<Vec<_>>()
        .join("\n");
    let block = match (ui_language.trim(), sources.len()) {
        ("en-US", 1) => format!(
            "This round was activated by exactly one remote IM source, and this round is now bound to that current contact.\n{}\nThe system may automatically send your final assistant reply to the bound current contact at the end of this round.",
            source_lines
        ),
        ("en-US", _) => format!(
            "This round was activated by multiple remote IM sources.\n{}\nThe system will not auto-send any final reply in this round.\nDo not send anything outward in this round unless a later stage narrows the target to one current contact.",
            source_lines
        ),
        ("zh-TW", 1) => format!(
            "本輪由唯一一個遠端 IM 來源啟動，且本輪已綁定該目前聯絡人。\n{}\n系統可能會在本輪結束後自動將最終回覆發送給本輪綁定聯絡人。",
            source_lines
        ),
        ("zh-TW", _) => format!(
            "本輪由多個遠端 IM 來源共同啟動。\n{}\n系統不會自動外發本輪最終回覆。\n此時不要對外發送任何內容，應等待後續流程先收斂到唯一目前聯絡人。",
            source_lines
        ),
        (_, 1) => format!(
            "本轮由唯一一个远程 IM 来源激活，且本轮已绑定该当前联系人。\n{}\n系统可能会在本轮结束后自动将最终回复发送给本轮绑定联系人。",
            source_lines
        ),
        _ => format!(
            "本轮由多个远程 IM 来源共同激活。\n{}\n系统不会自动外发本轮最终回复。\n此时不要对外发送任何内容，应等待后续流程先收敛到唯一当前联系人。",
            source_lines
        ),
    };
    Some(prompt_xml_block("remote im runtime activation", block))
}

fn resolve_remote_im_auto_send_target(
    assistant_text: &str,
    activation_sources: &[RemoteImActivationSource],
    is_remote_reply_delegate: bool,
) -> Result<Option<RemoteImActivationSource>, String> {
    if !is_remote_reply_delegate {
        return Ok(None);
    }
    if activation_sources.is_empty() {
        return Ok(None);
    }
    if activation_sources.len() >= 2 {
        return Ok(None);
    }
    if assistant_text.trim().is_empty() {
        return Ok(None);
    }
    Ok(activation_sources.first().cloned())
}

fn remote_im_reply_delegate_visible_texts(request_messages: &[Value]) -> Vec<String> {
    request_messages
        .iter()
        .filter(|message| {
            message
                .get("role")
                .and_then(Value::as_str)
                .is_some_and(|role| role.trim().eq_ignore_ascii_case("assistant"))
        })
        .map(request_message_text_content)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect()
}

fn effective_bound_remote_im_activation_source(
    runtime_context: Option<&RuntimeContext>,
    activation_sources: &[RemoteImActivationSource],
) -> Option<RemoteImActivationSource> {
    runtime_context
        .and_then(|context| context.bound_remote_im_activation_source.clone())
        .or_else(|| resolve_bound_remote_im_activation_source(activation_sources))
}

fn remote_im_trim_conversation_for_qa_mode(conversation: &Conversation) -> Conversation {
    let last_processed_index = conversation
        .messages
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, message)| {
            if message.role.trim() == "assistant" && remote_im_message_has_reply_decision(message) {
                Some(index)
            } else {
                None
            }
        });

    let Some(boundary_index) = last_processed_index else {
        return conversation.clone();
    };

    let mut trimmed = conversation.clone();
    trimmed.messages = conversation
        .messages
        .iter()
        .skip(boundary_index + 1)
        .cloned()
        .collect();
    trimmed
}

fn conversation_upsert_final_assistant_message(
    conversation: &mut Conversation,
    current_agent_id: &str,
    assistant_message: ChatMessage,
    now: &str,
) -> Result<ChatMessage, String> {
    let target_id = assistant_message.id.trim();
    if target_id.is_empty() {
        return Err("assistantMessageId is required.".to_string());
    }
    let target_idx = conversation
        .messages
        .iter()
        .rposition(|message| message.id.trim() == target_id)
        .ok_or_else(|| format!("目标 assistant message 不存在：{target_id}"))?;
    let existing = conversation
        .messages
        .get_mut(target_idx)
        .ok_or_else(|| format!("目标 assistant message 不存在：{target_id}"))?;
    if existing.role.trim() != "assistant" {
        return Err(format!(
            "目标消息不是 assistant，assistantMessageId={target_id}"
        ));
    }
    let existing_agent_id = existing
        .speaker_agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    if existing_agent_id != current_agent_id.trim() {
        return Err(format!(
            "目标 assistant message 的 speaker_agent_id 不匹配，assistantMessageId={}，expectedAgentId={}，actualAgentId={}",
            target_id,
            current_agent_id.trim(),
            existing_agent_id
        ));
    }
    let existing_id = existing.id.clone();
    let existing_created_at = existing.created_at.clone();
    let existing_tool_call = existing.tool_call.take();
    *existing = assistant_message;
    existing.id = existing_id;
    existing.created_at = existing_created_at;
    if existing
        .tool_call
        .as_ref()
        .map(|items| items.is_empty())
        .unwrap_or(true)
    {
        existing.tool_call = existing_tool_call;
    }
    conversation.updated_at = now.to_string();
    conversation.last_assistant_at = Some(now.to_string());
    Ok(existing.clone())
}

fn remote_im_auto_send_source_for_contact_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Result<Option<RemoteImActivationSource>, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(None);
    }
    let contacts = state_service_list_remote_im_contacts(state, None)?;
    Ok(contacts
        .iter()
        .find(|contact| {
            contact
                .bound_conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                == Some(conversation_id)
        })
        .map(|contact| RemoteImActivationSource {
            channel_id: contact.channel_id.clone(),
            platform: contact.platform.clone(),
            remote_contact_type: contact.remote_contact_type.clone(),
            remote_contact_id: contact.remote_contact_id.clone(),
            remote_contact_name: contact.remote_contact_name.clone(),
        }))
}

fn resolve_remote_im_auto_send_source(
    state: &AppState,
    conversation_id: &str,
    is_remote_im_contact_conversation: bool,
    runtime_context: &RuntimeContext,
    activation_sources: &[RemoteImActivationSource],
) -> Result<Option<RemoteImActivationSource>, String> {
    if runtime_context
        .remote_im_reply_delegate_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some()
    {
        return Ok(effective_bound_remote_im_activation_source(
            Some(runtime_context),
            activation_sources,
        ));
    }
    if is_remote_im_contact_conversation {
        return remote_im_auto_send_source_for_contact_conversation(state, conversation_id);
    }
    Ok(None)
}

fn remote_im_contact_tool_history_events(
    tool_name: &str,
    args_value: Value,
    tool_result: &str,
) -> Vec<Value> {
    let tool_call_id = format!("{}_auto_{}", tool_name, Uuid::new_v4());
    vec![
        serde_json::json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": [{
                "id": tool_call_id,
                "type": "function",
                "function": {
                    "name": tool_name,
                    "arguments": args_value
                }
            }]
        }),
        serde_json::json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "content": tool_result
        }),
    ]
}

// ==================== 图像回退 ====================

const IMAGE_FALLBACK_RECENT_USER_MESSAGE_LIMIT: usize = 7;

// 群聊长度门改写（默认禁用，保留待重新启用）。
#[allow(dead_code)]
fn remote_im_record_reply_rewrite_fast_request(
    state: &AppState,
    delegate_id: &str,
    request_text: &str,
    response_text: &str,
    success: bool,
    error: Option<String>,
    model_name: Option<String>,
    duration_ms: Option<u64>,
) {
    let turn = build_fast_request_turn(
        FAST_REQUEST_KIND_REMOTE_IM_REPLY_REWRITE,
        request_text,
        response_text,
        success,
        error,
        model_name,
        duration_ms,
    );
    match delegate_runtime_thread_append_fast_request(state, delegate_id, turn) {
        Ok(true) => {}
        Ok(false) => runtime_log_warn(format!(
            "[群聊长度门] 杂务记录跳过，delegate_id={}，reason=应答委托会话不存在",
            delegate_id
        )),
        Err(err) => runtime_log_warn(format!(
            "[群聊长度门] 杂务记录失败，delegate_id={}，error={}",
            delegate_id, err
        )),
    }
}

// 群聊长度门改写（默认禁用，保留待重新启用）。
#[allow(dead_code)]
fn remote_im_group_reply_rewrite_prompt(text: &str, max_units: u32) -> PreparedPrompt {
    let count = count_remote_im_multilingual_text_units(text);
    let preamble = if count.contains_japanese_kana {
        format!(
            "グループチャットの返信を短く書き直してください。最終本文だけを出力し、必要な情報と自然な文意を保ってください。日本語・中国語・韓国語は書記素単位、英語などは単語単位、絵文字は1個を1単位として、必ず {max_units} 単位以内にしてください。説明やMarkdownは不要です。"
        )
    } else if count.east_asian_graphemes > 0 {
        format!(
            "你负责压缩一条群聊回复。只输出最终正文，保留必要信息和完整自然的语义。中文、日文、韩文按可见字形计数，英文等按单词计数，Emoji 每个计 1 个单位；严格不超过 {max_units} 个有效文本单位。不要解释，不要使用 Markdown。"
        )
    } else {
        format!(
            "Rewrite this group-chat reply concisely. Output only the final reply and preserve the necessary meaning. Count Unicode words and emoji as one unit each, and count each CJK grapheme as one unit. Stay within {max_units} effective text units. Do not explain or use Markdown."
        )
    };
    PreparedPrompt {
        preamble,
        history_messages: Vec::new(),
        latest_user_text: text.to_string(),
        latest_user_meta_text: String::new(),
        latest_user_extra_text: String::new(),
        latest_user_extra_blocks: Vec::new(),
        latest_images: Vec::new(),
        latest_audios: Vec::new(),
    }
}

// 群聊长度门改写（默认禁用，保留待重新启用）。
#[allow(dead_code)]
async fn remote_im_reply_delegate_finalize_group_reply_draft(
    state: &AppState,
    delegate_id: &str,
    text: &str,
    max_units: u32,
) -> String {
    let draft = text.trim().to_string();
    let original_units = remote_im_multilingual_text_units(&draft);
    if draft.is_empty()
        || !remote_im_multilingual_text_units_exceed_ratio(&draft, max_units, 2, 1)
    {
        return draft;
    }
    if !remote_im_reply_delegate_is_active(state, delegate_id) {
        runtime_log_error(format!(
            "[群聊长度门] 跳过，任务=应答委托杂务改写，delegate_id={}，reason=应答委托已结束",
            delegate_id
        ));
        return draft;
    }
    let prepared = remote_im_group_reply_rewrite_prompt(&draft, max_units);
    let request_text = prepared_prompt_to_fast_request_text(&prepared);
    let api_config_id = match current_tool_review_api_config_id(state) {
        Ok(Some(value)) => value,
        Ok(None) => {
            let error = "未配置快速模型".to_string();
            remote_im_record_reply_rewrite_fast_request(
                state,
                delegate_id,
                &request_text,
                "",
                false,
                Some(error.clone()),
                None,
                None,
            );
            runtime_log_warn(format!(
                "[群聊长度门] 失败，任务=应答委托杂务改写，delegate_id={}，original_units={}，max_units={}，error={}",
                delegate_id, original_units, max_units, error
            ));
            return draft;
        }
        Err(err) => {
            remote_im_record_reply_rewrite_fast_request(
                state,
                delegate_id,
                &request_text,
                "",
                false,
                Some(err.clone()),
                None,
                None,
            );
            runtime_log_warn(format!(
                "[群聊长度门] 失败，任务=应答委托杂务改写，delegate_id={}，original_units={}，max_units={}，error={}",
                delegate_id, original_units, max_units, err
            ));
            return draft;
        }
    };
    let model_name = state_read_config_cached(state)
        .ok()
        .and_then(|config| {
            let selected = resolve_selected_api_config(&config, Some(&api_config_id))?;
            let resolved = resolve_api_config(&config, Some(&api_config_id)).ok()?;
            Some(resolved_model_name_for_quick_request(&selected, &resolved))
        });
    let started = std::time::Instant::now();
    match invoke_quick_model_reply_with_prepared_prompt(
        state,
        &api_config_id,
        prepared,
        Some(30),
    )
    .await
    {
        Ok(reply) => {
            let candidate = if reply.final_response_text.trim().is_empty() {
                reply.assistant_text
            } else {
                reply.final_response_text
            };
            let candidate = candidate.trim().to_string();
            let duration_ms = elapsed_ms_u64(started);
            let final_units = remote_im_multilingual_text_units(&candidate);
            if !candidate.is_empty() && final_units <= max_units as usize {
                remote_im_record_reply_rewrite_fast_request(
                    state,
                    delegate_id,
                    &request_text,
                    &candidate,
                    true,
                    None,
                    model_name,
                    Some(duration_ms),
                );
                runtime_log_info(format!(
                    "[群聊长度门] 完成，任务=应答委托杂务改写，delegate_id={}，original_units={}，max_units={}，final_units={}，elapsed_ms={}",
                    delegate_id, original_units, max_units, final_units, duration_ms
                ));
                candidate
            } else {
                let error = if candidate.is_empty() {
                    "杂务改写返回空正文".to_string()
                } else {
                    format!(
                        "杂务改写仍超出长度要求：final_units={}，max_units={}",
                        final_units, max_units
                    )
                };
                remote_im_record_reply_rewrite_fast_request(
                    state,
                    delegate_id,
                    &request_text,
                    &candidate,
                    false,
                    Some(error.clone()),
                    model_name,
                    Some(duration_ms),
                );
                runtime_log_warn(format!(
                    "[群聊长度门] 失败，任务=应答委托杂务改写，delegate_id={}，original_units={}，max_units={}，error={}",
                    delegate_id, original_units, max_units, error
                ));
                draft
            }
        }
        Err(err) => {
            let duration_ms = elapsed_ms_u64(started);
            remote_im_record_reply_rewrite_fast_request(
                state,
                delegate_id,
                &request_text,
                "",
                false,
                Some(err.clone()),
                model_name,
                Some(duration_ms),
            );
            runtime_log_warn(format!(
                "[群聊长度门] 失败，任务=应答委托杂务改写，delegate_id={}，original_units={}，max_units={}，error={}",
                delegate_id, original_units, max_units, err
            ));
            draft
        }
    }
}

async fn remote_im_auto_send_assistant_reply_to_source(
    state: &AppState,
    source: &RemoteImActivationSource,
    assistant_text: &str,
    assistant_message: Option<&ChatMessage>,
    group_policy: Option<RemoteImGroupReplyDispatchPolicy>,
) -> Result<Option<(String, Vec<Value>)>, String> {
    let message_text = if let Some(message) = assistant_message {
        message
            .parts
            .iter()
            .find_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .unwrap_or(assistant_text)
    } else {
        assistant_text
    };
    let config = state_read_config_cached(state)?;
    let channel = remote_im_channel_by_id(&config, &source.channel_id)
        .ok_or_else(|| "自动发送目标渠道不存在或已删除".to_string())?
        .clone();
    if !channel.enabled {
        return Err("自动发送目标渠道未启用".to_string());
    }
    let contact = state_service_find_remote_im_contact_by_identity(
        state,
        &source.channel_id,
        &source.remote_contact_type,
        &source.remote_contact_id,
    )?
    .ok_or_else(|| {
        format!("未找到自动发送目标联系人：{}", remote_im_activation_source_log_label(source))
    })?;
    if !contact.allow_send {
        return Err(format!("联系人“{}”未开启发送", remote_im_contact_log_label(&contact)));
    }
    if remote_im_contact_is_muted(state, &contact.id)? {
        return Err(format!(
            "联系人“{}”处于闭嘴状态，已拦截外发",
            remote_im_contact_log_label(&contact)
        ));
    }
    let outbound_text = message_text.trim().to_string();
    if outbound_text.is_empty() {
        return Ok(None);
    }
    let original_segments = if let Some(message) = assistant_message {
        resolve_text_and_meme_annotations_to_inline_segments(
            state,
            message_text,
            message.meme_annotations.as_deref(),
        )?
    } else {
        None
    };
    let persisted_segments = if outbound_text.trim() == message_text.trim() {
        original_segments
    } else {
        original_segments.map(|segments| {
            let mut rewritten = vec![PersistedInlineMessageSegment::Text {
                text: outbound_text.clone(),
            }];
            rewritten.extend(
                segments
                    .into_iter()
                    .filter(|segment| !matches!(segment, PersistedInlineMessageSegment::Text { .. })),
            );
            rewritten
        })
    };
    let content = if let Some(segments) = persisted_segments.as_ref() {
        inline_segments_to_remote_im_content_items(segments).await?
    } else {
        remote_im_build_text_content_items(
            state,
            &outbound_text,
            &format!(
                "remote_im_auto_send::{}::{}::{}",
                source.channel_id, source.remote_contact_id, outbound_text
            ),
        )
        .await?
    };
    let delivery_marker = match group_policy {
        Some(policy) => Some(remote_im_prepare_group_reply_delivery(
            state,
            &contact,
            policy.generation,
            &outbound_text,
        )?),
        None => None,
    };
    let send_result = match remote_im_send_content_payload_with_stage(
        state,
        &channel,
        &contact,
        content,
        false,
        "reply_async",
    )
    .await
    {
        Ok(result) => result,
        Err(send_error) => {
            if let (Some(policy), Some(marker)) = (group_policy, delivery_marker) {
                match send_error.stage {
                    RemoteImSendContentErrorStage::Preflight => {
                        if let Err(err) = remote_im_cancel_prepared_group_reply_delivery(
                            state,
                            &contact.id,
                            &marker,
                            &send_error.message,
                        ) {
                            runtime_log_warn(format!(
                                "[群聊巡检] 外发前置失败标记落盘降级，当前批次仍在内存重排，contact_id={}，error={}",
                                contact.id, err
                            ));
                        }
                        remote_im_group_reply_retry_generation(
                            state,
                            &contact.id,
                            policy.generation,
                            &format!("外发前置检查失败：{}", send_error.message),
                        );
                        return Err(format!(
                            "[GROUP_SEND_PREFLIGHT] {}",
                            send_error.message
                        ));
                    }
                    RemoteImSendContentErrorStage::DeliveryAttempted => {
                        remote_im_group_reply_complete_after_send(
                            state,
                            &contact,
                            policy.generation,
                            marker,
                            None,
                            RemoteImGroupReplySettlementStatus::Uncertain,
                        );
                        return Err(format!(
                            "[GROUP_DELIVERY_UNCERTAIN] {}",
                            send_error.message
                        ));
                    }
                }
            }
            return Err(send_error.message);
        }
    };
    let platform_message_id = send_result
        .get("platform_message_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let (Some(policy), Some(marker)) = (group_policy, delivery_marker) {
        remote_im_group_reply_complete_after_send(
            state,
            &contact,
            policy.generation,
            marker,
            platform_message_id,
            RemoteImGroupReplySettlementStatus::Delivered,
        );
    }
    let tool_result = tool_value_readable_text(&send_result);
    let args_value = serde_json::json!({
        "text": outbound_text
    });
    Ok(Some((
        "reply_async".to_string(),
        remote_im_contact_tool_history_events("remote_im_auto_send", args_value, &tool_result),
    )))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RemoteImAutoSendExecutionOutcome {
    SkippedEmptyReply,
    SkippedMuted,
    Sent { action: String },
    DeliveryUncertain { error: String },
    PreflightDeferred { error: String },
}

fn remote_im_auto_send_error_is_muted_gate(error: &str) -> bool {
    error.contains("联系人处于闭嘴状态")
}

fn remote_im_auto_send_error_is_delivery_uncertain(error: &str) -> bool {
    error.starts_with("[GROUP_DELIVERY_UNCERTAIN]")
}

fn remote_im_auto_send_error_is_preflight_deferred(error: &str) -> bool {
    error.starts_with("[GROUP_SEND_PREFLIGHT]")
}

fn remote_im_should_auto_send_after_core_round(runtime_context: &RuntimeContext) -> bool {
    !runtime_context.remote_im_defer_auto_send
}

async fn remote_im_auto_send_and_record_decision(
    state: &AppState,
    activation_source: &RemoteImActivationSource,
    conversation_id: &str,
    assistant_text: &str,
    assistant_message: Option<&ChatMessage>,
    assistant_message_id: Option<&str>,
    group_policy: Option<RemoteImGroupReplyDispatchPolicy>,
) -> Result<RemoteImAutoSendExecutionOutcome, String> {
    match remote_im_auto_send_assistant_reply_to_source(
        state,
        activation_source,
        assistant_text,
        assistant_message,
        group_policy,
    )
    .await
    {
        Ok(Some((action, _))) => {
            if let Err(err) = update_remote_im_reply_decision_for_message(
                state,
                conversation_id,
                assistant_message_id,
                &action,
                None,
            ) {
                runtime_log_warn(format!(
                    "[远程IM][自动发送] 发送已成功但决策回写失败，禁止重复发送，error={err}"
                ));
            }
            Ok(RemoteImAutoSendExecutionOutcome::Sent { action })
        }
        Ok(None) => Ok(RemoteImAutoSendExecutionOutcome::SkippedEmptyReply),
        Err(err) if remote_im_auto_send_error_is_preflight_deferred(&err) => {
            if let Err(update_err) = update_remote_im_reply_decision_for_message(
                state,
                conversation_id,
                assistant_message_id,
                "preflight_deferred",
                Some(err.as_str()),
            ) {
                runtime_log_warn(format!(
                    "[远程IM][自动发送] 前置失败状态回写降级，批次仍已保留重排，error={update_err}"
                ));
            }
            Ok(RemoteImAutoSendExecutionOutcome::PreflightDeferred { error: err })
        }
        Err(err) if remote_im_auto_send_error_is_delivery_uncertain(&err) => {
            if let Err(update_err) = update_remote_im_reply_decision_for_message(
                state,
                conversation_id,
                assistant_message_id,
                "delivery_uncertain",
                Some(err.as_str()),
            ) {
                runtime_log_warn(format!(
                    "[远程IM][自动发送] 不确定结果回写失败，已禁止重复发送，error={update_err}"
                ));
            }
            Ok(RemoteImAutoSendExecutionOutcome::DeliveryUncertain { error: err })
        }
        Err(err) if remote_im_auto_send_error_is_muted_gate(&err) => {
            if let Err(update_err) = update_remote_im_reply_decision_for_message(
                state,
                conversation_id,
                assistant_message_id,
                "muted_blocked",
                Some(err.as_str()),
            ) {
                runtime_log_warn(format!(
                    "[远程IM][自动发送] 闭嘴拦截状态回写失败，error={update_err}"
                ));
            }
            Ok(RemoteImAutoSendExecutionOutcome::SkippedMuted)
        }
        Err(err) => {
            if let Err(update_err) = update_remote_im_reply_decision_for_message(
                state,
                conversation_id,
                assistant_message_id,
                "send_failed",
                Some(err.as_str()),
            ) {
                return Err(format!(
                    "远程IM自动发送失败：{err}；回写失败状态失败：{update_err}"
                ));
            }
            Err(err)
        }
    }
}

fn remote_im_auto_send_log_labels(
    state: &AppState,
    source: &RemoteImActivationSource,
) -> (String, String) {
    let channel_label = state_read_config_cached(state)
        .ok()
        .and_then(|config| remote_im_channel_by_id(&config, &source.channel_id).cloned())
        .map(|channel| {
            let name = channel.name.trim();
            if name.is_empty() {
                "当前渠道".to_string()
            } else {
                name.to_string()
            }
        })
        .unwrap_or_else(|| "当前渠道".to_string());
    let contact_label = state_service_list_remote_im_contacts(state, None)
        .ok()
        .and_then(|contacts| {
            contacts
                .iter()
                .find(|contact| {
                    contact.channel_id == source.channel_id
                        && contact.remote_contact_type == source.remote_contact_type
                        && contact.remote_contact_id == source.remote_contact_id
                })
                .map(remote_im_contact_log_label)
        })
        .unwrap_or_else(|| remote_im_activation_source_log_label(source));
    (channel_label, contact_label)
}

fn remote_im_append_contact_log_for_activation_source(
    state: &AppState,
    source: &RemoteImActivationSource,
    level: &str,
    message: String,
) {
    let contact = state_service_list_remote_im_contacts(state, None)
        .ok()
        .and_then(|contacts| {
            contacts
                .into_iter()
                .find(|contact| {
                    contact.channel_id == source.channel_id
                        && contact.remote_contact_type == source.remote_contact_type
                        && contact.remote_contact_id == source.remote_contact_id
                })
        });
    if let Some(contact) = contact {
        remote_im_append_contact_log(&contact, level, message);
    } else {
        remote_im_append_channel_log(&source.channel_id, level, message);
    }
}

fn spawn_remote_im_auto_send_contact_assistant_reply(
    state: AppState,
    activation_source: RemoteImActivationSource,
    conversation_id: String,
    assistant_text: String,
    assistant_message: Option<ChatMessage>,
    assistant_message_id: Option<String>,
    group_dispatch: Option<(String, RemoteImGroupReplyDispatchPolicy)>,
) {
    tauri::async_runtime::spawn(async move {
        let group_policy = group_dispatch.as_ref().map(|(_, policy)| *policy);
        let started = std::time::Instant::now();
        let (channel_label, contact_label) = remote_im_auto_send_log_labels(&state, &activation_source);
        let reply_preview = remote_im_preview_text(&assistant_text, 100);
        runtime_log_info(format!(
            "[远程IM][自动发送] 开始：渠道={}，联系人={}，内容={}",
            channel_label, contact_label, reply_preview
        ));
        match remote_im_auto_send_and_record_decision(
            &state,
            &activation_source,
            &conversation_id,
            &assistant_text,
            assistant_message.as_ref(),
            assistant_message_id.as_deref(),
            group_policy,
        )
        .await
        {
            Ok(RemoteImAutoSendExecutionOutcome::Sent { action }) => {
                let _ = remote_im_finalize_async_send_result(
                    &state,
                    &activation_source,
                    true,
                    &now_iso(),
                    None,
                );
                runtime_log_info(format!(
                    "[远程IM][自动发送] 完成：渠道={}，联系人={}，内容={}，动作={}，耗时毫秒={}",
                    channel_label,
                    contact_label,
                    reply_preview,
                    action,
                    started.elapsed().as_millis()
                ));
            }
            Ok(RemoteImAutoSendExecutionOutcome::DeliveryUncertain { error }) => {
                let _ = remote_im_finalize_async_send_result(
                    &state,
                    &activation_source,
                    false,
                    &now_iso(),
                    Some(&error),
                );
                runtime_log_warn(format!(
                    "[远程IM][自动发送] 结果不确定，已消费当前批次并禁止自动重发：渠道={}，联系人={}，内容={}，异常={}，耗时毫秒={}",
                    channel_label,
                    contact_label,
                    reply_preview,
                    error,
                    started.elapsed().as_millis()
                ));
            }
            Ok(RemoteImAutoSendExecutionOutcome::PreflightDeferred { error }) => {
                let _ = remote_im_finalize_async_send_result(
                    &state,
                    &activation_source,
                    false,
                    &now_iso(),
                    Some(&error),
                );
                runtime_log_warn(format!(
                    "[远程IM][自动发送] 前置检查失败，正文未发送且批次已保留重排：渠道={}，联系人={}，内容={}，异常={}，耗时毫秒={}",
                    channel_label,
                    contact_label,
                    reply_preview,
                    error,
                    started.elapsed().as_millis()
                ));
            }
            Ok(RemoteImAutoSendExecutionOutcome::SkippedEmptyReply) => {
                runtime_log_warn(format!(
                    "[远程IM][自动发送] 跳过：渠道={}，联系人={}，原因=模型返回空回复，耗时毫秒={}",
                    channel_label,
                    contact_label,
                    started.elapsed().as_millis()
                ));
                remote_im_append_contact_log_for_activation_source(
                    &state,
                    &activation_source,
                    "info",
                    format!(
                        "[联系人消息] 发出跳过: contact={}, action=reply_async, reason=empty_reply",
                        remote_im_activation_source_log_label(&activation_source),
                    ),
                );
                if let Some((contact_id, policy)) = group_dispatch.as_ref() {
                    remote_im_group_reply_retry_after_dispatch_failure(
                        &state,
                        contact_id,
                        policy.generation,
                        "模型返回空回复",
                    );
                }
            }
            Ok(RemoteImAutoSendExecutionOutcome::SkippedMuted) => {
                runtime_log_warn(format!(
                    "[远程IM][自动发送] 跳过：渠道={}，联系人={}，原因=联系人处于闭嘴状态，耗时毫秒={}",
                    channel_label,
                    contact_label,
                    started.elapsed().as_millis()
                ));
                remote_im_append_contact_log_for_activation_source(
                    &state,
                    &activation_source,
                    "info",
                    format!(
                        "[联系人消息] 发出跳过: contact={}, action=reply_async, reason=muted",
                        remote_im_activation_source_log_label(&activation_source),
                    ),
                );
                if let Some((contact_id, policy)) = group_dispatch.as_ref() {
                    remote_im_group_reply_retry_after_dispatch_failure(
                        &state,
                        contact_id,
                        policy.generation,
                        "发送时联系人处于闭嘴状态",
                    );
                }
            }
            Err(err) => {
                let _ = remote_im_finalize_async_send_result(
                    &state,
                    &activation_source,
                    false,
                    &now_iso(),
                    Some(&err),
                );
                runtime_log_error(format!(
                    "[远程IM][自动发送] 失败：渠道={}，联系人={}，内容={}，异常={}，耗时毫秒={}",
                    channel_label,
                    contact_label,
                    reply_preview,
                    err,
                    started.elapsed().as_millis()
                ));
                if let Some((contact_id, policy)) = group_dispatch.as_ref() {
                    remote_im_group_reply_retry_after_dispatch_failure(
                        &state,
                        contact_id,
                        policy.generation,
                        &format!("远程外发失败：{err}"),
                    );
                }
            }
        }
    });
}

/// 中间正文中途直接外发（对齐 angel_heart：中途消息实时发出、不落会话历史）。
/// 纯发送、不更新 reply decision、不做群聊派发，最终回复仍由既有完成点统一外发一次。
fn spawn_remote_im_auto_send_intermediate_text(
    state: AppState,
    activation_source: RemoteImActivationSource,
    assistant_text: String,
) {
    tauri::async_runtime::spawn(async move {
        let (channel_label, contact_label) =
            remote_im_auto_send_log_labels(&state, &activation_source);
        let reply_preview = remote_im_preview_text(&assistant_text, 100);
        runtime_log_info(format!(
            "[远程IM][中间外发] 开始：渠道={}，联系人={}，内容={}",
            channel_label, contact_label, reply_preview
        ));
        match remote_im_auto_send_assistant_reply_to_source(
            &state,
            &activation_source,
            &assistant_text,
            None,
            None,
        )
        .await
        {
            Ok(Some(_)) => {
                runtime_log_info(format!(
                    "[远程IM][中间外发] 完成：渠道={}，联系人={}，内容={}",
                    channel_label, contact_label, reply_preview
                ));
            }
            Ok(None) => {
                runtime_log_warn(format!(
                    "[远程IM][中间外发] 跳过：渠道={}，联系人={}，原因=空回复",
                    channel_label, contact_label
                ));
            }
            Err(err) => {
                runtime_log_warn(format!(
                    "[远程IM][中间外发] 失败：渠道={}，联系人={}，内容={}，异常={}",
                    channel_label, contact_label, reply_preview, err
                ));
            }
        }
    });
}

/// 远程应答委托最终落账：把本次调度的工具事件对补写进历史会话（V4 工具行）。
/// 工具事件按序排列：assistant tool_calls 事件 + 对应 tool 结果事件成对出现；
/// 逐对调用 append_tool_event_to_assistant_message，服务内部按 tool_call_id 去重。
/// 返回写入的事件对数。
fn append_remote_im_delegate_tool_events_to_history(
    state: &AppState,
    conversation_id: &str,
    assistant_message_id: &str,
    tool_history_events: &[Value],
) -> Result<usize, String> {
    let mut written = 0usize;
    let mut pending_assistant_event: Option<Value> = None;
    for event in tool_history_events {
        let role = event
            .get("role")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if role.eq_ignore_ascii_case("assistant") {
            pending_assistant_event = Some(event.clone());
            continue;
        }
        if role.eq_ignore_ascii_case("tool") {
            let Some(assistant_event) = pending_assistant_event.as_ref() else {
                runtime_log_debug(format!(
                    "[远程应答委托] 跳过无配对 assistant 的工具事件，conversation_id={}，tool_call_id={}",
                    conversation_id,
                    event
                        .get("tool_call_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                ));
                continue;
            };
            conversation_service_v2().append_tool_event_to_assistant_message(
                state,
                &AssistantMessageToolAppendInput {
                    conversation_id: conversation_id.to_string(),
                    assistant_message_id: assistant_message_id.to_string(),
                    assistant_tool_event: assistant_event.clone(),
                    tool_result_event: event.clone(),
                },
            )?;
            written += 1;
        }
    }
    Ok(written)
}

fn update_remote_im_reply_decision_for_message(
    state: &AppState,
    conversation_id: &str,
    assistant_message_id: Option<&str>,
    action: &str,
    error: Option<&str>,
) -> Result<(), String> {
    let assistant_message_id = assistant_message_id
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let provider_meta_patch = || {
        let mut remote_im_decision = serde_json::Map::new();
        remote_im_decision.insert("action".to_string(), serde_json::json!(action));
        remote_im_decision.insert(
            "error".to_string(),
            serde_json::json!(error.unwrap_or("")),
        );
        remote_im_decision.insert(
            "processingMode".to_string(),
            serde_json::json!("continuous"),
        );
        remote_im_decision.insert(
            "conversationKind".to_string(),
            serde_json::json!("remote_im_contact"),
        );
        Value::Object(
            [(
                "remoteImDecision".to_string(),
                Value::Object(remote_im_decision),
            )]
            .into_iter()
            .collect(),
        )
    };

    let update_message = |message: &mut ChatMessage| {
        let mut meta = message
            .provider_meta
            .take()
            .unwrap_or_else(|| serde_json::json!({}));
        if !meta.is_object() {
            meta = serde_json::json!({});
        }
        let mut remote_im_decision = meta
            .as_object()
            .and_then(|obj| obj.get("remoteImDecision"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        remote_im_decision.insert("action".to_string(), serde_json::json!(action));
        remote_im_decision.insert(
            "error".to_string(),
            serde_json::json!(error.unwrap_or("")),
        );
        remote_im_decision
            .entry("processingMode".to_string())
            .or_insert_with(|| serde_json::json!("continuous"));
        remote_im_decision
            .entry("conversationKind".to_string())
            .or_insert_with(|| serde_json::json!("remote_im_contact"));
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("remoteImDecision".to_string(), Value::Object(remote_im_decision));
        }
        message.provider_meta = Some(meta);
    };

    let unarchived_patch_error = if let Some(target_id) = assistant_message_id {
        match conversation_service_v2().patch_provider_meta_on_assistant_message(
            state,
            &AssistantMessageProviderMetaPatchInput {
                conversation_id: conversation_id.to_string(),
                assistant_message_id: target_id.to_string(),
                provider_meta_patch: provider_meta_patch(),
            },
        ) {
            Ok(_) => return Ok(()),
            Err(err) => Some(err),
        }
    } else {
        Some(
            "更新 assistant providerMeta 失败：正常会话必须提供 assistant_message_id".to_string(),
        )
    };

    if let Some(mut conversation) = delegate_runtime_thread_conversation_get(state, conversation_id)? {
        if let Some(message) = conversation.messages.iter_mut().rev().find(|message| {
            message.role.trim() == "assistant"
                && assistant_message_id
                    .map(|target_id| message.id == target_id)
                    .unwrap_or(true)
        }) {
            update_message(message);
            delegate_runtime_thread_conversation_update(state, conversation_id, conversation)?;
            return Ok(());
        }
    }
    if let Some(err) = unarchived_patch_error {
        return Err(err);
    }
    Ok(())
}
