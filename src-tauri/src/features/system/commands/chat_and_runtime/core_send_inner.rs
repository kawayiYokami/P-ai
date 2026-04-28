fn remote_im_extract_action_from_tool_arguments(raw: &Value) -> Option<String> {
    match raw {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(trimmed)
                .ok()
                .and_then(|value| remote_im_extract_action_from_tool_arguments(&value))
        }
        Value::Object(map) => map
            .get("action")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase()),
        _ => None,
    }
}

fn remote_im_parse_tool_arguments(raw: &Value) -> Option<Value> {
    match raw {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(trimmed).ok()
        }
        Value::Object(_) => Some(raw.clone()),
        _ => None,
    }
}

fn remote_im_is_reply_decision_action(action: &str) -> bool {
    matches!(
        action.trim().to_ascii_lowercase().as_str(),
        "reply" | "send_files" | "reply_async" | "send" | "send_async" | "no_reply"
    )
}

#[derive(Debug, Clone)]
struct RemoteImReplyDecisionSummary {
    action: String,
    target: Option<RemoteImReplyTarget>,
}

fn remote_im_extract_reply_decision_from_tool_history(
    events: &[Value],
) -> Option<RemoteImReplyDecisionSummary> {
    let mut latest: Option<RemoteImReplyDecisionSummary> = None;
    for event in events {
        let Some(tool_calls) = event.get("tool_calls").and_then(Value::as_array) else {
            continue;
        };
        for tool_call in tool_calls {
            let Some(function) = tool_call.get("function") else {
                continue;
            };
            let Some(name) = function.get("name").and_then(Value::as_str) else {
                continue;
            };
            let action = match name.trim() {
                "remote_im_send" => function
                    .get("arguments")
                    .and_then(remote_im_extract_action_from_tool_arguments),
                "contact_reply" => Some("reply".to_string()),
                "contact_send_files" => Some("send_files".to_string()),
                "contact_no_reply" => Some("no_reply".to_string()),
                _ => None,
            };
            let Some(action) = action else {
                continue;
            };
            if remote_im_is_reply_decision_action(&action) {
                let target = if name.trim() == "remote_im_send" {
                    function
                        .get("arguments")
                        .and_then(remote_im_parse_tool_arguments)
                        .and_then(|value| {
                            let object = value.as_object()?;
                            let channel_id = object
                                .get("channel_id")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())?
                                .to_string();
                            let contact_id = object
                                .get("contact_id")
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())?
                                .to_string();
                            Some(RemoteImReplyTarget {
                                channel_id,
                                contact_id,
                            })
                        })
                } else {
                    None
                };
                latest = Some(RemoteImReplyDecisionSummary { action, target });
            }
        }
    }
    latest
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
    conversation_service().increment_conversation_unread_count_if_background(
        state,
        &mut conversation,
        1,
    );
    conversation
}

fn prompt_request_message_business_day_key(created_at: &str) -> Option<String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(created_at.trim()).ok()?;
    let local = parsed.with_timezone(&chrono::Local);
    let day = if local.time() < chrono::NaiveTime::from_hms_opt(4, 0, 0)? {
        local.date_naive().pred_opt()?
    } else {
        local.date_naive()
    };
    Some(day.format("%Y-%m-%d").to_string())
}

fn trim_conversation_for_prompt_request(conversation: &Conversation) -> Conversation {
    let mut trimmed = conversation.clone();
    if trimmed.messages.is_empty() {
        return trimmed;
    }
    let mut start_idx = 0usize;
    let allow_remote_im_day_blocks = conversation_is_remote_im_contact(conversation);
    let mut current_day = String::new();
    for (idx, message) in conversation.messages.iter().enumerate() {
        let next_day = prompt_request_message_business_day_key(&message.created_at)
            .or_else(|| {
                message
                    .created_at
                    .split('T')
                    .next()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })
            .unwrap_or_else(|| "unknown".to_string());
        let should_start_new = idx > 0
            && (is_context_compaction_message(message, message.role.trim())
                || (allow_remote_im_day_blocks && next_day != current_day));
        if should_start_new {
            start_idx = idx;
        }
        current_day = next_day;
    }
    trimmed.messages = conversation.messages[start_idx..].to_vec();
    trimmed
}

fn find_runtime_image_text_cache(
    runtime: &RuntimeStateFile,
    hash: &str,
    vision_api_id: &str,
) -> Option<String> {
    runtime
        .image_text_cache
        .iter()
        .find(|entry| entry.hash == hash && entry.vision_api_id == vision_api_id)
        .map(|entry| entry.text.clone())
}

fn upsert_runtime_image_text_cache(
    runtime: &mut RuntimeStateFile,
    hash: &str,
    vision_api_id: &str,
    text: &str,
) {
    if let Some(entry) = runtime
        .image_text_cache
        .iter_mut()
        .find(|entry| entry.hash == hash && entry.vision_api_id == vision_api_id)
    {
        entry.text = text.to_string();
        entry.updated_at = now_iso();
        return;
    }

    runtime.image_text_cache.push(ImageTextCacheEntry {
        hash: hash.to_string(),
        vision_api_id: vision_api_id.to_string(),
        text: text.to_string(),
        updated_at: now_iso(),
    });
    if runtime.image_text_cache.len() <= MAX_IMAGE_TEXT_CACHE_ENTRIES {
        return;
    }
    let Some((oldest_idx, _)) = runtime
        .image_text_cache
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.updated_at.cmp(&b.updated_at))
    else {
        return;
    };
    runtime.image_text_cache.remove(oldest_idx);
}

fn plan_mode_prompt_block() -> &'static str {
    "<plan mode>\n先不要直接开始实现。\n先理解用户目标，调查当前上下文或代码，并主动消除会明显改变计划骨架的关键疑问。\n当目标明确、约束明确、现状已调查充分，并且关键疑问已消除后，再调用 plan 工具的 present 动作呈现计划。\n在我明确确认之前，严禁直接修改代码或开始实施。\n</plan mode>"
}

fn conversation_latest_user_has_plan_mode_block(
    conversation: &Conversation,
    effective_agent_id: &str,
) -> bool {
    let plan_block = plan_mode_prompt_block().trim();
    conversation
        .messages
        .iter()
        .rev()
        .find(|message| prompt_role_for_message(message, effective_agent_id).as_deref() == Some("user"))
        .map(|message| {
            message
                .extra_text_blocks
                .iter()
                .any(|block| block.trim() == plan_block)
        })
        .unwrap_or(false)
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
            "This round was activated by exactly one remote IM source, and this round is now bound to that current contact.\n{}\nIf you do not call `contact_no_reply`, the system may automatically send your final assistant reply to the bound current contact at the end of this round.\nUse `contact_reply` for an immediate short acknowledgement, and `contact_send_files` when you need to send files or images first. When sending files, pass real local file paths in `contact_send_files.file_paths` instead of pasting file links into your reply body.",
            source_lines
        ),
        ("en-US", _) => format!(
            "This round was activated by multiple remote IM sources.\n{}\nThe system will not auto-send any final reply in this round.\nDo not send anything outward in this round unless a later stage narrows the target to one current contact.",
            source_lines
        ),
        ("zh-TW", 1) => format!(
            "本輪由唯一一個遠端 IM 來源啟動，且本輪已綁定該目前聯絡人。\n{}\n若你未呼叫 `contact_no_reply`，系統可能會在本輪結束後自動將最終回覆發送給本輪綁定聯絡人。\n若你只是要先回一句、告知正在處理，請使用 `contact_reply`；若要先發圖片或檔案，請使用 `contact_send_files`。傳送檔案時，應把真實本機檔案路徑放進 `contact_send_files.file_paths`，不要把檔案連結直接貼在正文裡。",
            source_lines
        ),
        ("zh-TW", _) => format!(
            "本輪由多個遠端 IM 來源共同啟動。\n{}\n系統不會自動外發本輪最終回覆。\n此時不要對外發送任何內容，應等待後續流程先收斂到唯一目前聯絡人。",
            source_lines
        ),
        (_, 1) => format!(
            "本轮由唯一一个远程 IM 来源激活，且本轮已绑定该当前联系人。\n{}\n如果你没有调用 `contact_no_reply`，系统可能会在本轮结束后自动将最终回复发送给本轮绑定联系人。\n如果你只是要先回一句、告知正在处理，请使用 `contact_reply`；如果要先发图片或文件，请使用 `contact_send_files`。发送文件时，应把真实本地文件路径放进 `contact_send_files.file_paths`，不要把文件链接直接贴进正文。",
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
    reply_decision: Option<&RemoteImReplyDecisionSummary>,
) -> Result<Option<RemoteImActivationSource>, String> {
    if activation_sources.is_empty() {
        return Ok(None);
    }
    if reply_decision
        .map(|decision| decision.action.eq_ignore_ascii_case("no_reply"))
        .unwrap_or(false)
    {
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

fn remote_im_find_contact_by_conversation<'a>(
    data: &'a AppData,
    conversation_id: &str,
) -> Option<&'a RemoteImContact> {
    conversation_service().find_remote_im_contact_by_conversation_in_data(data, conversation_id)
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
            "content": sanitize_tool_result_for_history(tool_name, tool_result)
        }),
    ]
}

// ==================== 图像回退 ====================

async fn resolve_image_description_with_vision_fallback(
    state: &AppState,
    vision_api: &ApiConfig,
    vision_resolved: &ResolvedApiConfig,
    image: &BinaryPart,
) -> Result<Option<String>, String> {
    let hash = compute_image_hash_hex(image)?;
    let cached = {
        let runtime = state_read_runtime_state_cached(state)?;
        find_runtime_image_text_cache(&runtime, &hash, &vision_api.id)
    };
    if let Some(text) = cached {
        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed));
        }
    }

    let converted =
        describe_image_with_vision_api(state, vision_resolved, vision_api, image).await?;
    let trimmed = converted.trim().to_string();
    if trimmed.is_empty() {
        return Ok(None);
    }

    {
        let mut runtime = state_read_runtime_state_cached(state)?;
        upsert_runtime_image_text_cache(&mut runtime, &hash, &vision_api.id, &trimmed);
        state_write_runtime_state_cached(state, &runtime)?;
    }

    Ok(Some(trimmed))
}

async fn apply_prompt_image_fallbacks_to_prepared(
    state: &AppState,
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    prepared: &mut PreparedPrompt,
) -> Result<bool, String> {
    if selected_api.enable_image {
        return Ok(false);
    }

    let vision_api = match resolve_vision_api_config(app_config) {
        Ok(api) => api,
        Err(_) => return Ok(false),
    };
    let vision_resolved = resolve_api_config(app_config, Some(vision_api.id.as_str()))?;
    if !vision_resolved.request_format.is_chat_text() {
        return Err(format!(
            "图转文模型请求格式 '{}' 暂未接入图片转文字链路。",
            vision_resolved.request_format
        ));
    }

    let mut changed = false;

    for message in &mut prepared.history_messages {
        if message.role.trim() != "user" || message.images.is_empty() {
            continue;
        }

        let original_images = std::mem::take(&mut message.images);
        let mut converted_blocks = Vec::<String>::new();
        for (index, image_payload) in original_images.into_iter().enumerate() {
            let image = BinaryPart {
                mime: image_payload.mime,
                bytes_base64: image_payload.content,
                saved_path: image_payload.saved_path,
            };
            if let Some(text) = resolve_image_description_with_vision_fallback(
                state,
                &vision_api,
                &vision_resolved,
                &image,
            )
            .await?
            {
                converted_blocks.push(format!("[图片{}]\n{}", index + 1, text));
            }
        }
        if !converted_blocks.is_empty() {
            message.extra_text_blocks.extend(converted_blocks);
            changed = true;
        }
    }

    if !prepared.latest_images.is_empty() {
        let original_images = std::mem::take(&mut prepared.latest_images);
        let mut converted_blocks = Vec::<String>::new();
        for (index, image_payload) in original_images.into_iter().enumerate() {
            let image = BinaryPart {
                mime: image_payload.mime,
                bytes_base64: image_payload.content,
                saved_path: image_payload.saved_path,
            };
            if let Some(text) = resolve_image_description_with_vision_fallback(
                state,
                &vision_api,
                &vision_resolved,
                &image,
            )
            .await?
            {
                converted_blocks.push(format!("[图片{}]\n{}", index + 1, text));
            }
        }
        if !converted_blocks.is_empty() {
            prepared_prompt_append_latest_user_extra_blocks(prepared, &converted_blocks);
            changed = true;
        }
    }

    Ok(changed)
}

async fn remote_im_auto_send_assistant_reply_to_source(
    state: &AppState,
    source: &RemoteImActivationSource,
    assistant_text: &str,
    assistant_message: Option<&ChatMessage>,
) -> Result<Option<(String, Vec<Value>)>, String> {
    let trimmed_text = assistant_text.trim();
    let persisted_segments = assistant_message
        .and_then(|message| provider_meta_meme_segments(message.provider_meta.as_ref()));
    if trimmed_text.is_empty() && persisted_segments.is_none() {
        return Ok(None);
    }
    let config = state_read_config_cached(state)?;
    let channel = remote_im_channel_by_id(&config, &source.channel_id)
        .ok_or_else(|| format!("远程IM渠道不存在: {}", source.channel_id))?
        .clone();
    if !channel.enabled {
        return Err(format!("远程IM渠道未启用: {}", source.channel_id));
    }
    let runtime = state_read_runtime_state_cached(state)?;
    let contact = runtime
        .remote_im_contacts
        .iter()
        .find(|item| {
            item.channel_id == source.channel_id
                && item.remote_contact_id == source.remote_contact_id
        })
        .ok_or_else(|| {
            format!(
                "未找到自动发送目标联系人: channel_id={}, contact_id={}",
                source.channel_id, source.remote_contact_id
            )
        })?
        .clone();
    if !contact.allow_send {
        return Err(format!(
            "用户已禁止向该联系人发送消息: channel_id={}, contact_id={}",
            source.channel_id, source.remote_contact_id
        ));
    }
    let content = if let Some(segments) = persisted_segments.as_ref() {
        remote_im_text_content_items_from_segments(segments)
    } else {
        remote_im_build_text_content_items(
            state,
            trimmed_text,
            &format!(
                "remote_im_auto_send::{}::{}::{}",
                source.channel_id, source.remote_contact_id, trimmed_text
            ),
        )
        .await?
    };
    let send_result =
        remote_im_send_content_payload(&channel, &contact, content, false, "reply_async").await?;
    let tool_result = serde_json::to_string(&send_result)
        .map_err(|err| format!("序列化自动 contact_reply 结果失败: {err}"))?;
    let args_value = serde_json::json!({
        "text": trimmed_text
    });
    Ok(Some((
        "reply_async".to_string(),
        remote_im_contact_tool_history_events("contact_reply", args_value, &tool_result),
    )))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RemoteImAutoSendExecutionOutcome {
    SkippedEmptyReply,
    Sent { action: String },
}

async fn remote_im_auto_send_and_record_decision(
    state: &AppState,
    activation_source: &RemoteImActivationSource,
    conversation_id: &str,
    assistant_text: &str,
    assistant_message: Option<&ChatMessage>,
    assistant_message_id: Option<&str>,
) -> Result<RemoteImAutoSendExecutionOutcome, String> {
    match remote_im_auto_send_assistant_reply_to_source(
        state,
        activation_source,
        assistant_text,
        assistant_message,
    )
    .await
    {
        Ok(Some((action, _))) => {
            update_remote_im_reply_decision_for_message(
                state,
                conversation_id,
                assistant_message_id,
                &action,
                None,
            )
            .map_err(|err| format!("远程IM自动发送成功，但回写回复决策失败: {err}"))?;
            Ok(RemoteImAutoSendExecutionOutcome::Sent { action })
        }
        Ok(None) => Ok(RemoteImAutoSendExecutionOutcome::SkippedEmptyReply),
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

fn spawn_remote_im_auto_send_contact_assistant_reply(
    state: AppState,
    activation_source: RemoteImActivationSource,
    conversation_id: String,
    assistant_text: String,
    assistant_message: Option<ChatMessage>,
    assistant_message_id: Option<String>,
) {
    tauri::async_runtime::spawn(async move {
        let started = std::time::Instant::now();
        eprintln!(
            "[远程IM][自动发送] 开始: conversation_id={}, channel_id={}, contact_id={}, text_len={}",
            conversation_id,
            activation_source.channel_id,
            activation_source.remote_contact_id,
            assistant_text.chars().count()
        );
        match remote_im_auto_send_and_record_decision(
            &state,
            &activation_source,
            &conversation_id,
            &assistant_text,
            assistant_message.as_ref(),
            assistant_message_id.as_deref(),
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
                eprintln!(
                    "[远程IM][自动发送] 完成: conversation_id={}, channel_id={}, contact_id={}, action={}, elapsed_ms={}",
                    conversation_id,
                    activation_source.channel_id,
                    activation_source.remote_contact_id,
                    action,
                    started.elapsed().as_millis()
                );
            }
            Ok(RemoteImAutoSendExecutionOutcome::SkippedEmptyReply) => {
                eprintln!(
                    "[远程IM][自动发送] 跳过: conversation_id={}, channel_id={}, contact_id={}, reason=empty_reply, elapsed_ms={}",
                    conversation_id,
                    activation_source.channel_id,
                    activation_source.remote_contact_id,
                    started.elapsed().as_millis()
                );
            }
            Err(err) => {
                let _ = remote_im_finalize_async_send_result(
                    &state,
                    &activation_source,
                    false,
                    &now_iso(),
                    Some(&err),
                );
                eprintln!(
                    "[远程IM][自动发送] 失败: conversation_id={}, channel_id={}, contact_id={}, error={}, elapsed_ms={}",
                    conversation_id,
                    activation_source.channel_id,
                    activation_source.remote_contact_id,
                    err,
                    started.elapsed().as_millis()
                );
            }
        }
    });
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

    match conversation_service().read_persisted_conversation(state, conversation_id) {
        Ok(mut conversation) => {
            if conversation.summary.trim().is_empty() {
                if let Some(message) = conversation.messages.iter_mut().rev().find(|message| {
                    message.role.trim() == "assistant"
                        && assistant_message_id
                            .map(|target_id| message.id == target_id)
                            .unwrap_or(true)
                }) {
                    update_message(message);
                    conversation_service().persist_conversation_with_chat_index(
                        state,
                        &conversation,
                    )?;
                    return Ok(());
                }
            }
        }
        Err(err) if !err.contains("not found") => return Err(err),
        Err(_) => {}
    }

    if let Some(mut conversation) = delegate_runtime_thread_conversation_get(state, conversation_id)? {
        if let Some(message) = conversation.messages.iter_mut().rev().find(|message| {
            message.role.trim() == "assistant"
                && assistant_message_id
                    .map(|target_id| message.id == target_id)
                    .unwrap_or(true)
        }) {
            update_message(message);
            delegate_runtime_thread_conversation_update(state, conversation_id, conversation)?;
        }
    }
    Ok(())
}

fn prioritize_requested_chat_api_id(
    requested_api_id: Option<&str>,
    candidate_api_ids: &mut Vec<String>,
    app_config: &AppConfig,
) -> Result<(), String> {
    let Some(requested_api_id) = requested_api_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };

    let Some(requested_api) = app_config
        .api_configs
        .iter()
        .find(|api| api.id == requested_api_id)
    else {
        return Err(format!(
            "会话指定模型不存在：session.api_config_id={requested_api_id}"
        ));
    };

    if !requested_api.request_format.is_chat_text() {
        return Err(format!(
            "会话指定模型不是聊天文本模型：session.api_config_id={}, request_format={:?}",
            requested_api_id,
            requested_api.request_format
        ));
    }

    if let Some(index) = candidate_api_ids.iter().position(|id| id == requested_api_id) {
        if index > 0 {
            let api_id = candidate_api_ids.remove(index);
            candidate_api_ids.insert(0, api_id);
        }
        return Ok(());
    }

    candidate_api_ids.insert(0, requested_api_id.to_string());
    Ok(())
}

fn memory_recall_query_from_user_text(user_text: &str) -> String {
    clean_text(user_text.trim())
}

fn render_message_parts_text_for_recall(parts: &[MessagePart]) -> String {
    parts.iter()
        .filter_map(|part| match part {
            MessagePart::Text { text } => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            }
            MessagePart::Image { .. } | MessagePart::Audio { .. } => None,
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
    let private_memory_enabled = agents
        .iter()
        .find(|a| a.id == effective_agent_id)
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
    let raw_ids = memory_recall_hit_ids(data_path, &store_memories, &recall_query_text);
    let stored_ids = memory_board_ids_from_current_hits(&raw_ids, 7);
    Ok(UserMessageRecallPayload { stored_ids, raw_ids })
}

async fn send_chat_message_inner(
    input: SendChatRequest,
    state: &AppState,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {
    const FIXED_MODEL_RETRY_COUNT: usize = 3;
    const FIXED_MODEL_RETRY_WAIT_SECONDS: u64 = 5;

    let mut runtime_context = input.runtime_context.clone().unwrap_or_default();
    let trace_id = runtime_context_request_id_or_new(
        Some(&runtime_context),
        input.trace_id.as_deref(),
        "chat",
    );
    if runtime_context.request_id.is_none() {
        runtime_context.request_id = Some(trace_id.clone());
    }
    let oldest_queue_created_at = input
        .oldest_queue_created_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let _queue_wait_ms = oldest_queue_created_at
        .as_deref()
        .and_then(parse_iso)
        .map(|created_at| (now_utc() - created_at).whole_milliseconds())
        .filter(|ms| *ms > 0)
        .map(|ms| ms.min(i128::from(u64::MAX)) as u64);
    let _session_for_log = input.session.clone();
    let remote_im_activation_sources = input.remote_im_activation_sources.clone();
    if runtime_context.bound_remote_im_activation_source.is_none() {
        runtime_context.bound_remote_im_activation_source =
            effective_bound_remote_im_activation_source(None, &remote_im_activation_sources);
    }

    let chat_started_at = std::time::Instant::now();
    let stage_timeline = std::sync::Arc::new(std::sync::Mutex::new(Vec::<LlmRoundLogStage>::new()));
    let stage_timeline_for_chat = stage_timeline.clone();
    let should_record_chat_stage = |stage: &str| -> bool {
        matches!(
            stage,
            "send_chat_message_inner.start"
                | "runtime_and_session_ready"
                | "run.begin"
                | "attachments_processed"
                | "prepare_context.begin"
                | "prepare_context.conversation_lock_wait_done"
                | "prepare_context.skill_snapshot_ready"
                | "prepare_context.workspace_agents_ready"
                | "prepare_context.todo_guide_ready"
                | "prepare_context.im_runtime_ready"
                | "prepare_context.task_board_ready"
                | "prepare_context.todo_board_ready"
                | "prepare_context.attachment_hints_ready"
                | "prepare_context.overrides_built"
                  | "prepare_context.terminal_block_ready"
                  | "prepare_context.prompt_build_begin"
                  | "prepare_context.prompt_fixed_system_ready"
                  | "prepare_context.prompt_conversation_payload_ready"
                  | "prepare_context.prompt_system_cache_hit"
                  | "prepare_context.prompt_system_cache_rebuilt"
                  | "prepare_context.prompt_system_finalize_ready"
                  | "prepare_context.prompt_built"
                | "prepare_context.prompt_tokens_estimated"
                | "prepare_context.done"
                | "pre_send_archive_checked"
                | "prompt_ready"
                | "model_reply_ready"
                | "assistant_message_persist_scheduled"
                | "send_chat_message_inner.finish"
        ) || stage.starts_with("model_request.start[")
            || stage.starts_with("model_request.finish[")
    };
    let describe_chat_stage = |stage: &str| -> String {
        let title = if stage == "send_chat_message_inner.start" {
            "开始发送消息".to_string()
        } else if stage == "runtime_and_session_ready" {
            "运行时与会话准备完成".to_string()
        } else if stage == "run.begin" {
            "进入执行阶段".to_string()
        } else if stage == "attachments_processed" {
            "附件处理完成".to_string()
        } else if stage == "prepare_context.begin" {
            "开始准备请求上下文".to_string()
        } else if stage == "prepare_context.conversation_lock_wait_done" {
            "会话锁等待完成".to_string()
        } else if stage == "prepare_context.skill_snapshot_ready" {
            "技能快照准备完成".to_string()
        } else if stage == "prepare_context.workspace_agents_ready" {
            "AGENTS 注入准备完成".to_string()
        } else if stage == "prepare_context.todo_guide_ready" {
            "Todo 指南准备完成".to_string()
        } else if stage == "prepare_context.im_runtime_ready" {
            "IM 运行块准备完成".to_string()
        } else if stage == "prepare_context.task_board_ready" {
            "任务板准备完成".to_string()
        } else if stage == "prepare_context.todo_board_ready" {
            "会话 Todo 板准备完成".to_string()
        } else if stage == "prepare_context.attachment_hints_ready" {
            "附件提示块准备完成".to_string()
        } else if stage == "prepare_context.overrides_built" {
            "提示词附加块准备完成".to_string()
        } else if stage == "prepare_context.terminal_block_ready" {
            "终端环境块准备完成".to_string()
        } else if stage == "prepare_context.prompt_build_begin" {
            "开始生成提示词主结构".to_string()
        } else if stage == "prepare_context.prompt_fixed_system_ready" {
            "主结构前置整理完成".to_string()
        } else if stage == "prepare_context.prompt_conversation_payload_ready" {
            "对话侧提示词生成完成".to_string()
        } else if stage == "prepare_context.prompt_system_cache_hit" {
            "系统提示词缓存命中".to_string()
        } else if stage == "prepare_context.prompt_system_cache_rebuilt" {
            "系统提示词缓存重建完成".to_string()
        } else if stage == "prepare_context.prompt_system_finalize_ready" {
            "系统提示词收口完成".to_string()
        } else if stage == "prepare_context.prompt_built" {
            "提示词主结构生成完成".to_string()
        } else if stage == "prepare_context.prompt_tokens_estimated" {
            "提示词 token 估算完成".to_string()
        } else if stage == "prepare_context.done" {
            "请求上下文准备完成".to_string()
        } else if stage == "pre_send_archive_checked" {
            "发送前归档检查完成".to_string()
        } else if stage == "prompt_ready" {
            "提示词准备完成".to_string()
        } else if stage.starts_with("model_request.start[") {
            "模型请求开始".to_string()
        } else if stage.starts_with("model_request.finish[") {
            "模型请求完成".to_string()
        } else if stage == "model_reply_ready" {
            "模型回复已就绪".to_string()
        } else if stage == "assistant_message_persist_scheduled" {
            "助理消息持久化已调度".to_string()
        } else if stage == "send_chat_message_inner.finish" {
            "发送消息结束".to_string()
        } else {
            "未命名阶段".to_string()
        };
        title
    };
    let conversation_id_for_work_status = input
        .session
        .as_ref()
        .and_then(|s| s.conversation_id.clone())
        .unwrap_or_default();
    let request_id_for_work_status = trace_id.clone();
    let emit_conversation_work_status = |status: &str| {
        let conversation_id = conversation_id_for_work_status.trim();
        if conversation_id.is_empty() {
            return;
        }
        if let Some(app_handle) = state.app_handle.lock().ok().and_then(|guard| guard.clone()) {
            let _ = app_handle.emit("conversation_work_status", serde_json::json!({
                "conversationId": conversation_id,
                "requestId": request_id_for_work_status,
                "status": status
            }));
        }
    };
    emit_conversation_work_status("working");
    let log_chat_stage = |stage: &str| {
        if !should_record_chat_stage(stage) {
            return;
        }
        let elapsed_ms = chat_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        if let Ok(mut timeline) = stage_timeline_for_chat.lock() {
            let since_prev_ms = timeline
                .last()
                .map(|last| elapsed_ms.saturating_sub(last.elapsed_ms))
                .unwrap_or(elapsed_ms);
            timeline.push(LlmRoundLogStage {
                stage: stage.to_string(),
                elapsed_ms,
                since_prev_ms,
            });
        }
    };
    let flush_chat_timeline = |reason: &str| {
        let Ok(timeline) = stage_timeline.lock() else {
            return;
        };
        if timeline.is_empty() {
            return;
        }
        let summary = timeline
            .iter()
            .map(|item| {
                format!(
                    "{}:{}ms（较上阶段 +{}ms）",
                    describe_chat_stage(&item.stage),
                    item.elapsed_ms,
                    item.since_prev_ms
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        eprintln!(
            "[聊天耗时] 汇总 原因={}，阶段={}",
            reason,
            summary
        );
    };
    log_chat_stage("send_chat_message_inner.start");

    let trigger_only = input.trigger_only;
    let requested_department_id = input
        .session
        .as_ref()
        .and_then(|s| s.department_id.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    let requested_api_config_id = input
        .session
        .as_ref()
        .and_then(|s| s.api_config_id.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);
    let requested_agent_id = input
        .session
        .as_ref()
        .map(|s| s.agent_id.trim().to_string())
        .filter(|v| !v.is_empty());
    let requested_conversation_id = input
        .session
        .as_ref()
        .and_then(|s| s.conversation_id.as_deref())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned);

    #[derive(Clone)]
    struct ConversationPrepareSnapshot {
        current_agent: AgentProfile,
        agents: Vec<AgentProfile>,
        response_style_id: String,
        user_name: String,
        user_intro: String,
        last_archive_summary: Option<String>,
        storage_conversation_before: Conversation,
        prompt_conversation_before: Conversation,
        is_remote_im_contact_conversation: bool,
        remote_im_contact_processing_mode: String,
        enable_pdf_images: bool,
        is_runtime_conversation: bool,
        runtime_conversation_id: Option<String>,
    }

    let runtime_conversation_id = requested_conversation_id
        .as_deref()
        .filter(|conversation_id| {
            delegate_runtime_thread_conversation_get(&state, conversation_id)
                .ok()
                .flatten()
                .is_some()
        })
        .map(ToOwned::to_owned);
    let runtime_conversation = if let Some(conversation_id) = runtime_conversation_id.as_deref() {
        delegate_runtime_thread_conversation_get(&state, conversation_id)?
            .ok_or_else(|| format!("指定临时会话不存在：{conversation_id}"))?
    } else {
        Conversation {
            id: String::new(),
            title: String::new(),
            agent_id: String::new(),
            department_id: String::new(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: String::new(),
            updated_at: String::new(),
            last_user_at: None,
            last_assistant_at: None,
            status: String::new(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        }
    };
    let requested_conversation_id_for_prepare = requested_conversation_id.clone();
    let requested_conversation_id_for_build = requested_conversation_id_for_prepare.clone();
    let runtime_conversation_id_for_prepare = runtime_conversation_id.clone();
    let runtime_conversation_for_prepare = runtime_conversation.clone();
    let build_prepare_snapshot_read_only = |
        data: &AppData,
        runtime_agents: &[AgentProfile],
        selected_api: &ApiConfig,
        effective_agent_id: &str,
    | -> Result<Option<ConversationPrepareSnapshot>, String> {
        let current_agent = runtime_agents
            .iter()
            .find(|a| a.id == effective_agent_id)
            .cloned()
            .ok_or_else(|| "Selected agent not found.".to_string())?;
        let Some(resolved) =
            conversation_service().resolve_prompt_prepare_conversation_from_data_read_only(
                data,
                &state.data_path,
                runtime_conversation_id_for_prepare.as_deref(),
                &runtime_conversation_for_prepare,
                selected_api,
                effective_agent_id,
                requested_conversation_id_for_build.as_deref(),
            )?
        else {
            return Ok(None);
        };
        Ok(Some(ConversationPrepareSnapshot {
            current_agent,
            agents: runtime_agents.to_vec(),
            response_style_id: resolved.response_style_id,
            user_name: resolved.user_name,
            user_intro: resolved.user_intro,
            last_archive_summary: resolved.last_archive_summary,
            storage_conversation_before: resolved.conversation_before.clone(),
            prompt_conversation_before: trim_conversation_for_prompt_request(
                &resolved.conversation_before,
            ),
            is_remote_im_contact_conversation: resolved.is_remote_im_contact_conversation,
            remote_im_contact_processing_mode: resolved.remote_im_contact_processing_mode,
            enable_pdf_images: resolved.enable_pdf_images,
            is_runtime_conversation: resolved.is_runtime_conversation,
            runtime_conversation_id: runtime_conversation_id_for_prepare.clone(),
        }))
    };
    let build_prepare_snapshot_for_requested_conversation_read_only = |
        requested_conversation_id: &str,
        runtime_agents: &[AgentProfile],
        selected_api: &ApiConfig,
        effective_agent_id: &str,
    | -> Result<Option<ConversationPrepareSnapshot>, String> {
        if runtime_conversation_id_for_prepare.as_deref() == Some(requested_conversation_id) {
            let runtime_state = state_read_runtime_state_cached(state)?;
            let chat_index = state_read_chat_index_cached(state)?;
            let mut data = AppData::default();
            data.agents = runtime_agents.to_vec();
            data.assistant_department_agent_id = runtime_state.assistant_department_agent_id.clone();
            data.user_alias = runtime_state.user_alias.clone();
            data.response_style_id = runtime_state.response_style_id.clone();
            data.pdf_read_mode = runtime_state.pdf_read_mode.clone();
            data.background_voice_screenshot_keywords =
                runtime_state.background_voice_screenshot_keywords.clone();
            data.background_voice_screenshot_mode =
                runtime_state.background_voice_screenshot_mode.clone();
            data.instruction_presets = runtime_state.instruction_presets.clone();
            data.main_conversation_id = runtime_state.main_conversation_id.clone();
            data.pinned_conversation_ids = runtime_state.pinned_conversation_ids.clone();
            data.remote_im_contacts = runtime_state.remote_im_contacts.clone();
            data.remote_im_contact_checkpoints = runtime_state.remote_im_contact_checkpoints.clone();
            if let Some(summary_item) = chat_index
                .conversations
                .iter()
                .rev()
                .find(|item| !item.summary.trim().is_empty())
            {
                data.conversations.push(Conversation {
                    id: summary_item.id.clone(),
                    title: String::new(),
                    agent_id: String::new(),
                    department_id: String::new(),
                    bound_conversation_id: None,
                    parent_conversation_id: None,
                    child_conversation_ids: Vec::new(),
                    fork_message_cursor: None,
                    unread_count: 0,
                    conversation_kind: String::new(),
                    root_conversation_id: None,
                    delegate_id: None,
                    created_at: summary_item.updated_at.clone(),
                    updated_at: summary_item.updated_at.clone(),
                    last_user_at: None,
                    last_assistant_at: None,
                    status: summary_item.status.clone(),
                    summary: summary_item.summary.clone(),
                    user_profile_snapshot: String::new(),
                    shell_workspace_path: None,
                    shell_workspaces: Vec::new(),
                    archived_at: summary_item.archived_at.clone(),
                    messages: Vec::new(),
                    current_todos: Vec::new(),
                    memory_recall_table: Vec::new(),
                    plan_mode_enabled: false,
                });
            }
            return build_prepare_snapshot_read_only(
                &data,
                runtime_agents,
                selected_api,
                effective_agent_id,
            );
        }
        let requested_conversation = state_read_conversation_cached(state, requested_conversation_id)?;
        if !requested_conversation.summary.trim().is_empty() {
            return Ok(None);
        }
        let runtime_state = state_read_runtime_state_cached(state)?;
        let chat_index = state_read_chat_index_cached(state)?;
        let mut data = AppData::default();
        data.agents = runtime_agents.to_vec();
        data.assistant_department_agent_id = runtime_state.assistant_department_agent_id.clone();
        data.user_alias = runtime_state.user_alias.clone();
        data.response_style_id = runtime_state.response_style_id.clone();
        data.pdf_read_mode = runtime_state.pdf_read_mode.clone();
        data.background_voice_screenshot_keywords =
            runtime_state.background_voice_screenshot_keywords.clone();
        data.background_voice_screenshot_mode =
            runtime_state.background_voice_screenshot_mode.clone();
        data.instruction_presets = runtime_state.instruction_presets.clone();
        data.main_conversation_id = runtime_state.main_conversation_id.clone();
        data.pinned_conversation_ids = runtime_state.pinned_conversation_ids.clone();
        data.remote_im_contacts = runtime_state.remote_im_contacts.clone();
        data.remote_im_contact_checkpoints = runtime_state.remote_im_contact_checkpoints.clone();
        data.conversations.push(requested_conversation);
        if let Some(summary_item) = chat_index
            .conversations
            .iter()
            .rev()
            .find(|item| !item.summary.trim().is_empty())
        {
            data.conversations.push(Conversation {
                id: summary_item.id.clone(),
                title: String::new(),
                agent_id: String::new(),
                department_id: String::new(),
                bound_conversation_id: None,
                parent_conversation_id: None,
                child_conversation_ids: Vec::new(),
                fork_message_cursor: None,
                unread_count: 0,
                conversation_kind: String::new(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: summary_item.updated_at.clone(),
                updated_at: summary_item.updated_at.clone(),
                last_user_at: None,
                last_assistant_at: None,
                status: summary_item.status.clone(),
                summary: summary_item.summary.clone(),
                user_profile_snapshot: String::new(),
                shell_workspace_path: None,
                shell_workspaces: Vec::new(),
                archived_at: summary_item.archived_at.clone(),
                messages: Vec::new(),
                current_todos: Vec::new(),
                memory_recall_table: Vec::new(),
                plan_mode_enabled: false,
            });
        }
        build_prepare_snapshot_read_only(&data, runtime_agents, selected_api, effective_agent_id)
    };
    let build_prepare_snapshot_for_main_conversation_read_only = |
        main_conversation_id: &str,
        runtime_agents: &[AgentProfile],
        selected_api: &ApiConfig,
        effective_agent_id: &str,
    | -> Result<Option<ConversationPrepareSnapshot>, String> {
        let main_conversation = state_read_conversation_cached(state, main_conversation_id)?;
        if !main_conversation.summary.trim().is_empty()
            || !conversation_visible_in_foreground_lists(&main_conversation)
        {
            return Ok(None);
        }
        build_prepare_snapshot_for_requested_conversation_read_only(
            main_conversation_id,
            runtime_agents,
            selected_api,
            effective_agent_id,
        )
    };

    let (
        app_config,
        selected_api,
        resolved_api,
        _effective_department_id,
        effective_agent_id,
        candidate_api_ids,
        runtime_main_conversation_id,
        runtime_agents,
        preloaded_prepare_snapshot,
    ) = {
        let prepare_started = std::time::Instant::now();
        let mut prepare_detail_parts = Vec::<String>::new();
        let lock_wait_started = std::time::Instant::now();
        let lock_wait_ms = lock_wait_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!("会话锁等待={}ms", lock_wait_ms));
        log_chat_stage("runtime_and_session_ready.lock_wait_done");
        let config_started = std::time::Instant::now();
        let (mut app_config, config_read_detail) = state_read_config_cached_with_detail(state)?;
        let config_read_ms = config_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!(
            "配置读取={}ms(source={}, dirty_fast_path={}, mtime_before={}ms, cache_lookup={}ms, disk_read={}ms, mtime_after={}ms, cache_write={}ms, total={}ms)",
            config_read_ms,
            config_read_detail.source,
            config_read_detail.dirty_fast_path,
            config_read_detail.mtime_before_ms,
            config_read_detail.cache_lookup_ms,
            config_read_detail.disk_read_ms,
            config_read_detail.mtime_after_ms,
            config_read_detail.cache_write_ms,
            config_read_detail.total_ms,
        ));
        log_chat_stage("runtime_and_session_ready.config_read_done");
        let app_data_started = std::time::Instant::now();
        let runtime_state = state_read_runtime_state_cached(state)?;
        let assistant_department_agent_id = runtime_state.assistant_department_agent_id.clone();
        let runtime_main_conversation_id = runtime_state.main_conversation_id.clone();
        let mut runtime_agents = state_read_agents_cached(state)?;
        let app_data_read_ms = app_data_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!(
            "运行时分片读取={}ms(agents={}, assistant_department_agent_id={})",
            app_data_read_ms,
            runtime_agents.len(),
            assistant_department_agent_id
        ));
        log_chat_stage("runtime_and_session_ready.app_data_read_done");
        prepare_detail_parts.push(format!("运行时人格列表就绪=0ms(count={})", runtime_agents.len()));
        log_chat_stage("runtime_and_session_ready.runtime_data_cloned");
        let private_org_started = std::time::Instant::now();
        merge_private_organization_into_runtime(
            &state.data_path,
            &mut app_config,
            &mut runtime_agents,
        )?;
        let private_org_ms = private_org_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!("私有组织合并={}ms", private_org_ms));
        log_chat_stage("runtime_and_session_ready.private_org_merged");
        let agent_resolve_started = std::time::Instant::now();
        let effective_agent_id = if let Some(agent_id) = requested_agent_id.as_deref() {
            if runtime_agents
                .iter()
                .any(|a| a.id == agent_id && !a.is_built_in_user)
            {
                agent_id.to_string()
            } else {
                return Err(format!("Selected agent '{agent_id}' not found."));
            }
        } else if runtime_agents
            .iter()
            .any(|a| a.id == assistant_department_agent_id && !a.is_built_in_user)
        {
            assistant_department_agent_id.clone()
        } else {
            runtime_agents
                .iter()
                .find(|a| !a.is_built_in_user)
                .map(|a| a.id.clone())
                .ok_or_else(|| "No assistant agent configured.".to_string())?
        };
        let agent_resolve_ms = agent_resolve_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!("人格解析={}ms", agent_resolve_ms));
        log_chat_stage("runtime_and_session_ready.agent_resolved");
        let department_resolve_started = std::time::Instant::now();
        let effective_department = if let Some(department_id) = requested_department_id.as_deref() {
            department_by_id(&app_config, department_id)
                .ok_or_else(|| format!("Department '{department_id}' not found."))?
        } else {
            department_for_agent_id(&app_config, &effective_agent_id)
                .or_else(|| assistant_department(&app_config))
                .ok_or_else(|| "No assistant department configured.".to_string())?
        };
        if !effective_department
            .agent_ids
            .iter()
            .any(|id| id.trim() == effective_agent_id)
        {
            let effective_agent_name = runtime_agents
                .iter()
                .find(|agent| agent.id == effective_agent_id)
                .map(|agent| agent.name.trim())
                .filter(|name| !name.is_empty())
                .unwrap_or(effective_agent_id.as_str());
            let effective_department_name = effective_department.name.trim();
            let effective_department_name = if effective_department_name.is_empty() {
                effective_department.id.as_str()
            } else {
                effective_department_name
            };
            return Err(format!(
                "部门人格配置不合法：部门“{}”（{}）不能使用人格“{}”（{}）。请到部门设置中为该部门选择已分配的人格。",
                effective_department_name,
                effective_department.id,
                effective_agent_name,
                effective_agent_id
            ));
        }
        let effective_department_id = effective_department.id.clone();
        let department_resolve_ms = department_resolve_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!("部门解析={}ms", department_resolve_ms));
        log_chat_stage("runtime_and_session_ready.department_resolved");
        let candidate_models_started = std::time::Instant::now();
        let mut candidate_api_ids = department_api_config_ids(effective_department)
            .into_iter()
            .filter(|api_id| {
                app_config
                    .api_configs
                    .iter()
                    .any(|api| api.id == *api_id && api.request_format.is_chat_text())
            })
            .collect::<Vec<_>>();
        if candidate_api_ids.is_empty() {
            let fallback = resolve_selected_api_config(&app_config, None)
                .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
            candidate_api_ids.push(fallback.id.clone());
        }
        prioritize_requested_chat_api_id(
            requested_api_config_id.as_deref(),
            &mut candidate_api_ids,
            &app_config,
        )?;
        let candidate_models_ms = candidate_models_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!("候选模型构建={}ms(count={})", candidate_models_ms, candidate_api_ids.len()));
        let selected_api_started = std::time::Instant::now();
        let selected_api_id = candidate_api_ids
            .first()
            .cloned()
            .ok_or_else(|| format!("Department '{}' has no available chat model.", effective_department_id))?;
        let selected_api = app_config
            .api_configs
            .iter()
            .find(|a| a.id == selected_api_id)
            .cloned()
            .ok_or_else(|| format!("Selected API config '{}' not found.", selected_api_id))?;
        let selected_api_ms = selected_api_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!("选定模型查找={}ms(api={})", selected_api_ms, selected_api.id));
        let resolved_api_started = std::time::Instant::now();
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        let resolved_api_ms = resolved_api_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!(
            "模型配置解析={}ms(format={}, model={})",
            resolved_api_ms,
            resolved_api.request_format.as_str(),
            selected_api.model
        ));
        let prepare_total_ms = prepare_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        runtime_log_info(format!(
            "[会话准备耗时] total={}ms，{}",
            prepare_total_ms,
            prepare_detail_parts.join(" | ")
        ));
        log_chat_stage("runtime_and_session_ready.candidate_models_ready");
        let preloaded_prepare_snapshot_candidate = match requested_conversation_id_for_prepare
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(requested_conversation_id) => {
                build_prepare_snapshot_for_requested_conversation_read_only(
                    requested_conversation_id,
                    &runtime_agents,
                    &selected_api,
                    &effective_agent_id,
                )?
            }
            None => {
                if let Some(main_conversation_id) = runtime_main_conversation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    build_prepare_snapshot_for_main_conversation_read_only(
                        main_conversation_id,
                        &runtime_agents,
                        &selected_api,
                        &effective_agent_id,
                    )?
                } else {
                    None
                }
            }
        };
        let preloaded_prepare_snapshot = preloaded_prepare_snapshot_candidate;
        (
            app_config,
            selected_api,
            resolved_api,
            effective_department_id,
            effective_agent_id,
            candidate_api_ids,
            runtime_main_conversation_id,
            runtime_agents,
            preloaded_prepare_snapshot,
        )
    };
    log_chat_stage("runtime_and_session_ready");

    let chat_key = inflight_chat_key(
        &effective_agent_id,
        requested_conversation_id.as_deref(),
    );
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    {
        let mut inflight = state
            .inflight_chat_abort_handles
            .lock()
            .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
        if let Some(previous) = inflight.insert(chat_key.clone(), abort_handle) {
            previous.abort();
        }
    }
    reset_inflight_completed_tool_history(state, &chat_key)?;
    let _ = abort_inflight_tool_abort_handle(state, &chat_key);

    let chat_session_key = chat_key.clone();
    let chat_session_key_for_log = chat_session_key.clone();
    let selected_api_for_log = selected_api.clone();
    let resolved_api_for_log = resolved_api.clone();
    let state_for_run = state.clone();
    let stage_timeline_for_run = stage_timeline.clone();
    let run = async move {
    let state = state_for_run;
    let log_run_stage = |stage: &str| {
        if !should_record_chat_stage(stage) {
            return;
        }
        let elapsed_ms = chat_started_at
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        if let Ok(mut timeline) = stage_timeline_for_run.lock() {
            let since_prev_ms = timeline
                .last()
                .map(|last| elapsed_ms.saturating_sub(last.elapsed_ms))
                .unwrap_or(elapsed_ms);
            timeline.push(LlmRoundLogStage {
                stage: stage.to_string(),
                elapsed_ms,
                since_prev_ms,
            });
        }
    };
    log_run_stage("run.begin");
    if !resolved_api.request_format.is_chat_text() {
        return Err(format!(
            "Request format '{}' is not implemented in chat router yet.",
            resolved_api.request_format
        ));
    }

    let mut effective_payload = input.payload.clone();
    let extra_block_count = effective_payload
        .extra_text_blocks
        .as_ref()
        .map(|v| v.len())
        .unwrap_or(0);
    let attachment_count = effective_payload
        .attachments
        .as_ref()
        .map(|v| v.len())
        .unwrap_or(0);
    if extra_block_count > 0 {
        eprintln!(
            "[CHAT][ATTACHMENT] payload carries extra_text_blocks: count={}",
            extra_block_count
        );
    }
    if attachment_count > 0 {
        eprintln!(
            "[CHAT][ATTACHMENT] payload carries attachments json: count={}",
            attachment_count
        );
    }
    let audios = effective_payload.audios.clone().unwrap_or_default();
    if !audios.is_empty() {
        return Err("当前版本仅支持本地语音识别，发送消息不支持语音附件。".to_string());
    }
    if !trigger_only {
        let images = effective_payload.images.clone().unwrap_or_default();
        if !images.is_empty() {
            let notices = persist_payload_images_to_workspace_downloads(&state, &images);
            if !notices.is_empty() {
                let notice_text = notices.join("\n\n");
                let merged_text = effective_payload
                    .text
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(|text| format!("{text}\n\n{notice_text}"))
                    .unwrap_or(notice_text);
                effective_payload.text = Some(merged_text);
            }
        }
    }

    if !selected_api.enable_image {
        let images = effective_payload.images.clone().unwrap_or_default();
        if !images.is_empty() {
            let vision_api = resolve_vision_api_config(&app_config).ok();
            if let Some(vision_api) = vision_api {
                let vision_resolved =
                    resolve_api_config(&app_config, Some(vision_api.id.as_str()))?;
                if !vision_resolved.request_format.is_chat_text() {
                    return Err(format!(
                        "Vision request format '{}' is not implemented in image conversion router yet.",
                        vision_resolved.request_format
                    ));
                }

                let mut converted_texts = Vec::<String>::new();
                for (idx, image) in images.iter().enumerate() {
                    let hash = compute_image_hash_hex(image)?;
                    let cached = {
                        let runtime = state_read_runtime_state_cached(&state)?;
                        find_runtime_image_text_cache(&runtime, &hash, &vision_api.id)
                    };

                    if let Some(text) = cached {
                        let mapped = format!("[图片{}]\n{}", idx + 1, text);
                        converted_texts.push(mapped);
                        continue;
                    }

                    let converted =
                        describe_image_with_vision_api(&state, &vision_resolved, &vision_api, image)
                            .await?;
                    let converted = converted.trim().to_string();
                    if converted.is_empty() {
                        continue;
                    }

                    let mut runtime = state_read_runtime_state_cached(&state)?;
                    let mapped = if let Some(existing) =
                        find_runtime_image_text_cache(&runtime, &hash, &vision_api.id)
                    {
                        format!("[图片{}]\n{}", idx + 1, existing)
                    } else {
                        upsert_runtime_image_text_cache(
                            &mut runtime,
                            &hash,
                            &vision_api.id,
                            &converted,
                        );
                        state_write_runtime_state_cached(&state, &runtime)?;
                        format!("[图片{}]\n{}", idx + 1, converted)
                    };
                    converted_texts.push(mapped);
                }

                if !converted_texts.is_empty() {
                    let converted_all = converted_texts.join("\n\n");
                    let merged_text = effective_payload
                        .text
                        .as_deref()
                        .map(str::trim)
                        .filter(|v| !v.is_empty())
                        .map(|text| format!("{text}\n\n{converted_all}"))
                        .unwrap_or(converted_all);
                    effective_payload.text = Some(merged_text);
                }
                effective_payload.images = None;
            } else {
                eprintln!(
                    "[CHAT] Image input filtered out because current chat API does not support image and no vision fallback is configured."
                );
                let filtered_notice = match app_config.ui_language.trim() {
                    "en-US" => "[SYSTEM NOTICE] Image attachment was filtered out: current model has image input disabled and no vision fallback model is configured.",
                    "zh-TW" => "[系統提示] 已過濾圖片附件：當前模型未啟用圖片輸入，且未配置視覺回退模型。",
                    _ => "[系统提示] 已过滤图片附件：当前模型未启用图片输入，且未配置视觉回退模型。",
                };
                let merged_text = effective_payload
                    .text
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(|text| format!("{text}\n\n{filtered_notice}"))
                    .unwrap_or_else(|| filtered_notice.to_string());
                effective_payload.text = Some(merged_text);
                effective_payload.images = None;
            }
        }
    }
    log_run_stage("attachments_processed");

    let effective_user_parts = if trigger_only {
        Vec::new()
    } else {
        build_user_parts(&effective_payload, &selected_api)?
    };
    let effective_user_text = effective_user_parts
        .iter()
        .map(|part| match part {
            MessagePart::Text { text } => text.clone(),
            MessagePart::Image { mime, .. } => {
                if mime.trim().eq_ignore_ascii_case("application/pdf") {
                    "[pdf]".to_string()
                } else {
                    "[image]".to_string()
                }
            }
            MessagePart::Audio { .. } => "[audio]".to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let image_saved_paths = effective_payload
        .images
        .as_ref()
        .map(|items| {
            items.iter()
                .map(|item| item.saved_path.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let audio_saved_paths = effective_payload
        .audios
        .as_ref()
        .map(|items| {
            items.iter()
                .map(|item| item.saved_path.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let effective_images = effective_user_parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Image {
                mime, bytes_base64, ..
            } => Some((mime.clone(), bytes_base64.clone())),
            _ => None,
        })
        .enumerate()
        .map(|(index, (mime, bytes_base64))| PreparedBinaryPayload {
            mime,
            content: bytes_base64,
            saved_path: image_saved_paths.get(index).cloned().flatten(),
        })
        .collect::<Vec<_>>();
    let effective_audios = effective_user_parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Audio {
                mime, bytes_base64, ..
            } => Some((mime.clone(), bytes_base64.clone())),
            _ => None,
        })
        .enumerate()
        .map(|(index, (mime, bytes_base64))| PreparedBinaryPayload {
            mime: mime.clone(),
            content: bytes_base64.clone(),
            saved_path: audio_saved_paths.get(index).cloned().flatten(),
        })
        .collect::<Vec<_>>();

    let mut archived_before_send_any = false;
    let mut compaction_restart_count = 0usize;
    let mut persist_user_message_on_next_prepare = true;

    let mut preloaded_prepare_snapshot = preloaded_prepare_snapshot;
    'dispatch: loop {

    let mut prepare_request_context = |persist_user_message: bool| -> Result<_, String> {
        log_run_stage("prepare_context.begin");
        let snapshot = if let Some(snapshot) = preloaded_prepare_snapshot.take() {
            log_run_stage("prepare_context.foreground_conversation_ready");
            snapshot
        } else if let Some(requested_conversation_id) = requested_conversation_id_for_prepare
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let snapshot = build_prepare_snapshot_for_requested_conversation_read_only(
                requested_conversation_id,
                &runtime_agents,
                &selected_api,
                &effective_agent_id,
            )?
            .ok_or_else(|| format!("指定会话不存在或不可用：{requested_conversation_id}"))?;
            log_run_stage("prepare_context.foreground_conversation_ready");
            snapshot
        } else if let Some(main_conversation_id) = runtime_main_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let snapshot = build_prepare_snapshot_for_main_conversation_read_only(
                main_conversation_id,
                &runtime_agents,
                &selected_api,
                &effective_agent_id,
            )?
            .ok_or_else(|| format!("主会话不存在或不可用：{main_conversation_id}"))?;
            log_run_stage("prepare_context.foreground_conversation_ready");
            snapshot
        } else {
            eprintln!(
                "[聊天发送] 缺少 conversation_id 且未找到 runtime 主会话，拒绝构建请求上下文"
            );
            return Err("缺少 conversation_id".to_string());
        };
        log_run_stage("prepare_context.conversation_snapshot_ready");
        log_run_stage("prepare_context.archive_summary_ready");
        log_run_stage("prepare_context.prompt_conversation_ready");
        log_run_stage("prepare_context.base_context_ready");
        let is_delegate_conversation =
            snapshot.prompt_conversation_before.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE;
        let requested_plan_mode_enabled = get_conversation_plan_mode_enabled(
            &state,
            &snapshot.prompt_conversation_before.id,
        )
        .unwrap_or(snapshot.prompt_conversation_before.plan_mode_enabled);
        let storage_conversation = if trigger_only {
            let latest_message = snapshot
                .storage_conversation_before
                .messages
                .last()
                .ok_or_else(|| "当前对话没有可供继续处理的消息。".to_string())?;
            if latest_message
                .speaker_agent_id
                .as_deref()
                .map(str::trim)
                == Some(effective_agent_id.as_str())
            {
                return Err("当前最后一条消息来自助理自身，无需重复激活。".to_string());
            }
            snapshot.storage_conversation_before.clone()
        } else if !persist_user_message {
            snapshot.storage_conversation_before.clone()
        } else {
            let mut storage_api = selected_api.clone();
            storage_api.enable_image = true;
            storage_api.enable_audio = true;
            let mut storage_payload = input.payload.clone();
            if let Some(display_text) = input.payload.display_text.as_deref() {
                storage_payload.text = Some(display_text.trim().to_string());
            }
            let mut user_parts = build_user_parts(&storage_payload, &storage_api)?;
            externalize_message_parts_to_media_refs(&mut user_parts, &state.data_path)?;
            let attachment_meta = normalize_payload_attachments(input.payload.attachments.as_ref());
            let mut user_provider_meta = merge_provider_meta_with_attachments(
                input.payload.provider_meta.clone(),
                &attachment_meta,
            );
            if let Some(request_id) = runtime_context
                .request_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let mut meta = user_provider_meta.unwrap_or_else(|| serde_json::json!({}));
                if !meta.is_object() {
                    meta = serde_json::json!({});
                }
                if let Some(obj) = meta.as_object_mut() {
                    obj.insert("requestId".to_string(), Value::String(request_id.to_string()));
                }
                user_provider_meta = Some(meta);
            }
            let recall_payload = if is_delegate_conversation {
                UserMessageRecallPayload::default()
            } else {
                let draft_message = ChatMessage {
                    id: String::new(),
                    role: "user".to_string(),
                    created_at: String::new(),
                    speaker_agent_id: None,
                    parts: user_parts.clone(),
                    extra_text_blocks: input.payload.extra_text_blocks.clone().unwrap_or_default(),
                    provider_meta: None,
                    tool_call: None,
                    mcp_call: None,
                };
                with_memory_lock(&state, "prepare_context_user_message_recall", || {
                    collect_recall_payload_for_user_message(
                        &state.data_path,
                        &snapshot.agents,
                        &effective_agent_id,
                        &draft_message,
                    )
                })?
            };
            if !recall_payload.stored_ids.is_empty() {
                write_retrieved_memory_ids_into_provider_meta(
                    &mut user_provider_meta,
                    &recall_payload.stored_ids,
                );
            }
            log_run_stage("prepare_context.memory_recall_done");
            let now = now_iso();
            let user_message_id = Uuid::new_v4().to_string();
            let git_ghost_snapshot_record = if snapshot.is_runtime_conversation {
                None
            } else {
                tauri::async_runtime::block_on(
                    git_ghost_snapshot::create_main_workspace_git_ghost_snapshot_record(
                        &state,
                        &snapshot.storage_conversation_before,
                        &user_message_id,
                    ),
                )
            };
            if let Some(record) = git_ghost_snapshot_record {
                if let Err(err) = git_ghost_snapshot::write_git_snapshot_record_into_provider_meta(
                    &mut user_provider_meta,
                    &record,
                )
                {
                    runtime_log_error(format!(
                        "[Git幽灵快照] 失败，conversation_id={}，message_id={}，stage=write_provider_meta，error={}",
                        snapshot.storage_conversation_before.id, user_message_id, err
                    ));
                }
            }
            let user_message = ChatMessage {
                id: user_message_id,
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(input.speaker_agent_id.clone().unwrap_or_else(|| USER_PERSONA_ID.to_string())),
                parts: user_parts,
                extra_text_blocks: input.payload.extra_text_blocks.clone().unwrap_or_default(),
                provider_meta: user_provider_meta,
                tool_call: None,
                mcp_call: None,
            };
            let updated_conversation = append_user_message_to_conversation(
                &state,
                snapshot.storage_conversation_before.clone(),
                user_message,
                &now,
            );
            log_run_stage("prepare_context.user_message_composed");
            if snapshot.is_runtime_conversation {
                delegate_runtime_thread_conversation_update(
                    &state,
                    snapshot.runtime_conversation_id.as_deref().unwrap_or_default(),
                    updated_conversation.clone(),
                )?;
                log_run_stage("prepare_context.user_message_committed");
                updated_conversation
            } else {
                let updated_conversation = {
                    let mut conversation = conversation_service().read_persisted_conversation(
                        &state,
                        &updated_conversation.id,
                    )?;
                    if !conversation.summary.trim().is_empty() {
                        return Err(format!(
                            "指定会话不存在或不可用：{}",
                            updated_conversation.id
                        ));
                    }
                    conversation = updated_conversation.clone();
                    for memory_id in &recall_payload.raw_ids {
                        conversation.memory_recall_table.push(memory_id.clone());
                    }
                    conversation_service().persist_conversation_with_chat_index(
                        &state,
                        &conversation,
                    )?;
                    conversation
                };
                log_run_stage("prepare_context.user_message_committed");
                log_run_stage("prepare_context.state_persist_scheduled");
                updated_conversation
            }
        };
        let conversation = trim_conversation_for_prompt_request(&storage_conversation);
        let latest_user_text = if trigger_only {
            conversation
                .messages
                .iter()
                .rev()
                .find(|message| prompt_role_for_message(message, &effective_agent_id).as_deref() == Some("user"))
                .map(render_message_content_for_model)
                .unwrap_or_default()
        } else {
            effective_user_text.clone()
        };
        let current_department = department_for_agent_id(&app_config, &snapshot.current_agent.id);
        let todo_enabled =
            tool_enabled(&selected_api, &snapshot.current_agent, current_department, "todo");
        let attachment_relative_paths = normalize_payload_attachments(input.payload.attachments.as_ref())
            .into_iter()
            .filter_map(|item| {
                item.get("relativePath")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
            })
            .collect::<Vec<_>>();
        let chat_overrides = ChatPromptOverrides {
            latest_user_intent: Some(LatestUserPayloadIntent::ChatRequest {
                trigger_only,
                submitted_user_text: latest_user_text.clone(),
                include_task_board: !trigger_only && !is_delegate_conversation,
                include_todo_board: !trigger_only && todo_enabled,
                attachment_relative_paths,
            }),
            todo_tool_enabled: todo_enabled,
            remote_im_activation_sources: remote_im_activation_sources.clone(),
            latest_images: (!trigger_only).then_some(effective_images.clone()),
            latest_audios: (!trigger_only).then_some(effective_audios.clone()),
            ..Default::default()
        };
        log_run_stage("prepare_context.overrides_built");
        let prompt_mode = if is_delegate_conversation {
            PromptBuildMode::Delegate
        } else {
            PromptBuildMode::Chat
        };
        let chat_overrides = Some(chat_overrides);
        log_run_stage("prepare_context.prompt_build_begin");
        let mut prepared_prompt = build_prepared_prompt_for_mode_with_stage_logger(
            prompt_mode,
            &conversation,
            &snapshot.current_agent,
            &snapshot.agents,
            &app_config.departments,
            &snapshot.user_name,
            &snapshot.user_intro,
            &snapshot.response_style_id,
            &app_config.ui_language,
            Some(&state.data_path),
            snapshot.last_archive_summary.as_deref(),
            None,
            chat_overrides.clone(),
            Some(&state),
            Some(&log_run_stage),
            Some(&selected_api),
            Some(&resolved_api),
            Some(snapshot.enable_pdf_images),
        );
        if requested_plan_mode_enabled
            && !conversation_latest_user_has_plan_mode_block(&conversation, &effective_agent_id)
        {
            let plan_block = plan_mode_prompt_block().trim();
            let existing_meta = prepared_prompt.latest_user_meta_text.trim();
            prepared_prompt.latest_user_meta_text = if existing_meta.is_empty() {
                plan_block.to_string()
            } else {
                format!("{plan_block}\n{existing_meta}")
            };
        }
        log_run_stage("prepare_context.prompt_built");
        let tool_loop_auto_compaction_context = if snapshot.is_runtime_conversation {
            None
        } else {
            Some(ToolLoopAutoCompactionContext {
                conversation_id: conversation.id.clone(),
                data_path: state.data_path.clone(),
                request_id: runtime_context
                    .request_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned),
                prompt_mode,
                agent: snapshot.current_agent.clone(),
                agents: snapshot.agents.clone(),
                departments: app_config.departments.clone(),
                user_name: snapshot.user_name.clone(),
                user_intro: snapshot.user_intro.clone(),
                response_style_id: snapshot.response_style_id.clone(),
                ui_language: app_config.ui_language.clone(),
                last_archive_summary: snapshot.last_archive_summary.clone(),
                chat_overrides: chat_overrides.clone(),
                enable_pdf_images: snapshot.enable_pdf_images,
                trusted_prompt_usage: std::sync::Arc::new(std::sync::Mutex::new(None)),
            })
        };

        // Use persisted API config as the source of truth to avoid stale
        // frontend model overrides after editing/saving config.
        let model_name = selected_api.model.trim().to_string();
        let model_name = if model_name.trim().is_empty() {
            resolved_api.model.clone()
        } else {
            model_name
        };
        let conversation_id = conversation.id.clone();
        let usage_resolution = conversation_prompt_service().resolve_prompt_usage(
            &prepared_prompt,
            &selected_api,
            &snapshot.current_agent,
            &conversation,
        );
        if usage_resolution.estimated_prompt_tokens.is_some() {
            log_run_stage("prepare_context.prompt_tokens_estimated");
        }
        log_run_stage("prepare_context.done");
        Ok((
            model_name,
            prepared_prompt,
            conversation_id,
            latest_user_text,
            snapshot.current_agent,
            usage_resolution.estimated_prompt_tokens,
            snapshot.is_remote_im_contact_conversation,
            snapshot.remote_im_contact_processing_mode,
            tool_loop_auto_compaction_context,
            conversation,
            snapshot.is_runtime_conversation,
        ))
    };
    let mut prepared_context = prepare_request_context(persist_user_message_on_next_prepare)?;
    if apply_prompt_image_fallbacks_to_prepared(
        &state,
        &app_config,
        &selected_api,
        &mut prepared_context.1,
    )
    .await?
        && prepared_context.5.is_some()
    {
        prepared_context.5 = Some(conversation_prompt_service().estimate_prepared_prompt_tokens(
            &prepared_context.1,
            &selected_api,
            &prepared_context.4,
        ));
    }
    let conversation_for_compaction = prepared_context.9.clone();
    let estimated_prompt_tokens_before_send = prepared_context.5;
    let is_runtime_conversation = prepared_context.10;
    if is_runtime_conversation {
        if let Some(conversation_id) = requested_conversation_id.as_deref() {
            eprintln!(
                "[归档] 发送前检查 跳过: conversation_id={}, reason=delegate_runtime_thread",
                conversation_id
            );
        }
    } else {
        conversation_prompt_service().prime_runtime_trusted_prompt_usage(
            &mut runtime_context,
            &conversation_for_compaction,
            &selected_api,
        );
        let latest_real_usage = runtime_context.trusted_prompt_usage.as_ref().copied();
        let usage_resolution = conversation_prompt_service()
            .consume_runtime_trusted_prompt_usage_or_estimate(
            &mut runtime_context,
            &prepared_context.1,
            &selected_api,
            &prepared_context.4,
        );
        let (decision, decision_source) = decide_archive_before_send_from_usage(
            &usage_resolution,
            conversation_for_compaction.last_user_at.as_deref(),
            archive_pipeline_has_assistant_reply(&conversation_for_compaction),
        );
        eprintln!(
            "[归档] 发送前检查: should_archive={}, forced={}, reason={}, usage_ratio={:.4}, source={}, latest_real_effective_prompt_tokens={:?}, latest_real_usage_ratio={:?}, estimated_prompt_tokens={:?}, context_window_tokens={}",
            decision.should_archive,
            decision.forced,
            decision.reason,
            decision.usage_ratio,
            decision_source,
            latest_real_usage.map(|usage| usage.effective_prompt_tokens),
            latest_real_usage.map(|usage| usage.context_usage_ratio),
            usage_resolution.estimated_prompt_tokens.or(estimated_prompt_tokens_before_send),
            selected_api.context_window_tokens
        );
        if decision.should_archive {
            if decision.forced {
                let _ = on_delta.send(AssistantDeltaEvent {
                    delta: "".to_string(),
                    kind: Some("tool_status".to_string()),
                    request_id: None,
                    activation_id: runtime_context.request_id.clone(),
                    phase_id: None,
                    reason: None,
                    tool_name: Some("archive".to_string()),
                    tool_status: Some("running".to_string()),
                    tool_args: None,
                    message: Some("正在整理上下文...".to_string()),
                });
            }

            let archive_res = run_context_compaction_pipeline(
                &state,
                &selected_api,
                &resolved_api,
                &conversation_for_compaction,
                &effective_agent_id,
                &decision.reason,
                "COMPACTION-AUTO",
            )
            .await;

            match archive_res {
                Ok(result) => {
                    archived_before_send_any = archived_before_send_any || result.archived;
                    if decision.forced {
                        let done_message = if result.warning.as_deref().unwrap_or("").trim().is_empty() {
                            "整理完成，正在重新开始当前调度...".to_string()
                        } else {
                            format!(
                                "整理完成（降级摘要），正在重新开始当前调度：{}",
                                result.warning.unwrap_or_default()
                            )
                        };
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: "".to_string(),
                            kind: Some("tool_status".to_string()),
                            request_id: None,
                            activation_id: runtime_context.request_id.clone(),
                            phase_id: None,
                            reason: None,
                            tool_name: Some("archive".to_string()),
                            tool_status: Some("done".to_string()),
                            tool_args: None,
                            message: Some(done_message),
                        });
                    }
                    compaction_restart_count = compaction_restart_count.saturating_add(1);
                    if compaction_restart_count > 3 {
                        return Err("上下文整理后仍无法恢复发送，重开调度次数已超过上限。".to_string());
                    }
                    runtime_log_info(format!(
                        "[聊天调度] 发送前整理命中，当前调度闭口并准备重开: conversation_id={}，restart_count={}，reason={}",
                        conversation_for_compaction.id,
                        compaction_restart_count,
                        decision.reason
                    ));
                    runtime_context.request_id = Some(format!("chat-{}", Uuid::new_v4()));
                    runtime_context.dispatch_id = Some(Uuid::new_v4().to_string());
                    runtime_context.event_source =
                        runtime_context_trimmed(Some("compaction_restart"));
                    runtime_context.dispatch_reason =
                        runtime_context_trimmed(Some("after_auto_compaction"));
                    runtime_context.trusted_prompt_usage = None;
                    preloaded_prepare_snapshot = None;
                    persist_user_message_on_next_prepare = false;
                    continue 'dispatch;
                }
                Err(err) => {
                    if decision.forced {
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: "".to_string(),
                            kind: Some("tool_status".to_string()),
                            request_id: None,
                            activation_id: runtime_context.request_id.clone(),
                            phase_id: None,
                            reason: None,
                            tool_name: Some("archive".to_string()),
                            tool_status: Some("failed".to_string()),
                            tool_args: None,
                            message: Some(format!("整理失败：{err}")),
                        });
                    }
                    return Err(format!("整理失败：{err}"));
                }
            }
        }
    }
    log_run_stage("pre_send_archive_checked");

    let (
        _primary_model_name,
        prepared_prompt,
        conversation_id,
        latest_user_text,
        current_agent,
        estimated_prompt_tokens,
        is_remote_im_contact_conversation,
        remote_im_contact_processing_mode,
        tool_loop_auto_compaction_context,
        conversation_for_request,
        is_runtime_conversation,
    ) = prepared_context;
    if let Some(context) = tool_loop_auto_compaction_context.as_ref() {
        let mut guard = cache_lock_recover(
            "trusted_prompt_usage",
            &context.trusted_prompt_usage,
        );
        *guard = runtime_context.trusted_prompt_usage.take();
    }
    log_run_stage("prompt_ready");

    let mut model_reply: Option<ModelReply> = None;
    let mut active_selected_api = selected_api.clone();
    let mut active_resolved_api = resolved_api.clone();
    let mut fallback_errors = Vec::<String>::new();
    let prepared_prompt = prepared_prompt;
    let mut conversation_for_request = conversation_for_request;
    for (candidate_index, candidate_api_id) in candidate_api_ids.iter().enumerate() {
        let candidate_stage = format!(
            "model_candidate.start[candidate_index={},candidate_api_id={}]",
            candidate_index, candidate_api_id
        );
        log_run_stage(&candidate_stage);
        let mut candidate_selected_api = if candidate_api_id == &selected_api.id {
            selected_api.clone()
        } else {
            match resolve_selected_api_config(&app_config, Some(candidate_api_id.as_str())) {
                Some(api) => api,
                None => {
                    fallback_errors.push(format!("{candidate_api_id}: 候选模型不存在"));
                    continue;
                }
            }
        };
        let mut candidate_resolved_api =
            match resolve_api_config(&app_config, Some(candidate_selected_api.id.as_str())) {
                Ok(api) => api,
                Err(error) => {
                    fallback_errors.push(format!("{}: {}", candidate_selected_api.name, error));
                    continue;
                }
            };
        let candidate_model_name = if candidate_selected_api.model.trim().is_empty() {
            candidate_resolved_api.model.clone()
        } else {
            candidate_selected_api.model.trim().to_string()
        };
        let mut candidate_prepared_prompt = prepared_prompt.clone();
        if let Err(error) = maybe_prepare_aliyun_multimodal_urls_for_candidate(
            &state,
            &candidate_selected_api,
            &mut candidate_resolved_api,
            &candidate_model_name,
            &mut candidate_prepared_prompt,
            &mut conversation_for_request,
            is_runtime_conversation,
            true,
        )
        .await
        {
            fallback_errors.push(format!(
                "{}: 百炼多模态 URL 预处理失败: {}",
                candidate_selected_api.name, error
            ));
            continue;
        }
        let max_failure_retries = FIXED_MODEL_RETRY_COUNT;
        let mut candidate_final_error: Option<String> = None;
        for attempt in 0..=max_failure_retries {
            let request_start_stage = format!(
                "model_request.start[candidate_api_id={},attempt={}]",
                candidate_selected_api.id,
                attempt + 1
            );
            log_run_stage(&request_start_stage);
            let chat_round_execution = call_model_openai_style(
                &candidate_resolved_api,
                &app_config,
                &candidate_selected_api,
                &current_agent,
                &candidate_model_name,
                candidate_prepared_prompt.clone(),
                Some(&state),
                tool_loop_auto_compaction_context.as_ref(),
                on_delta,
                app_config.tool_max_iterations as usize,
                &chat_session_key,
            )
            .await;
            let restart_after_compaction = matches!(
                &chat_round_execution.result,
                Err(error) if error == CHAT_DISPATCH_RESTART_AFTER_COMPACTION
            );
            if !restart_after_compaction {
                let ModelCallLogParts {
                    scene,
                    request_format,
                    provider_name,
                    model_name,
                    base_url,
                    headers,
                    tools,
                    response,
                    error,
                    elapsed_ms,
                    timeline,
                } = chat_round_execution.log_parts;
                push_llm_round_log(
                    Some(&state),
                    Some(format!("round-{chat_session_key}")),
                    Some(chat_session_key.to_string()),
                    scene,
                    request_format,
                    &provider_name,
                    &model_name,
                    &base_url,
                    headers,
                    tools,
                    response,
                    error,
                    elapsed_ms,
                    timeline,
                );
            }
            let request_finish_stage = format!(
                "model_request.finish[candidate_api_id={},attempt={}]",
                candidate_selected_api.id,
                attempt + 1
            );
            log_run_stage(&request_finish_stage);

            if restart_after_compaction {
                compaction_restart_count = compaction_restart_count.saturating_add(1);
                if compaction_restart_count > 3 {
                    return Err("上下文整理后仍无法恢复发送，重开调度次数已超过上限。".to_string());
                }
                runtime_log_info(format!(
                    "[聊天调度] 续调整理命中，当前调度闭口并准备重开: conversation_id={}，restart_count={}",
                    conversation_id,
                    compaction_restart_count
                ));
                runtime_context.request_id = Some(format!("chat-{}", Uuid::new_v4()));
                runtime_context.dispatch_id = Some(Uuid::new_v4().to_string());
                runtime_context.event_source =
                    runtime_context_trimmed(Some("compaction_restart"));
                runtime_context.dispatch_reason =
                    runtime_context_trimmed(Some("after_tool_continue_compaction"));
                runtime_context.trusted_prompt_usage = None;
                preloaded_prepare_snapshot = None;
                persist_user_message_on_next_prepare = false;
                continue 'dispatch;
            }

            let (reason_text, final_error_text) = match chat_round_execution.result {
                Ok(reply) => {
                    if model_reply_has_visible_content(&reply) {
                        active_selected_api = candidate_selected_api.clone();
                        active_resolved_api = candidate_resolved_api.clone();
                        model_reply = Some(reply);
                        candidate_final_error = None;
                        break;
                    }
                    (
                        "模型返回空响应".to_string(),
                        "模型持续返回空响应，已停止重试，请稍后再试或切换模型。"
                            .to_string(),
                    )
                }
                Err(error) => {
                    if candidate_selected_api.enable_image
                        && error_indicates_image_input_unsupported(&error)
                    {
                        match auto_disable_api_image_input(&state, &candidate_selected_api.id) {
                            Ok(true) => {
                                candidate_selected_api.enable_image = false;
                                (
                                    "检测到当前模型不支持图片输入，已自动关闭该模型的图片模态并重试"
                                        .to_string(),
                                    format!("模型请求重试后仍失败: {error}"),
                                )
                            }
                            Ok(false) => (
                                "模型请求失败".to_string(),
                                format!("模型请求重试后仍失败: {error}"),
                            ),
                            Err(write_err) => {
                                runtime_log_warn(format!(
                                    "[聊天] 自动关闭图片模态失败: api_config_id={}, error={}",
                                    candidate_selected_api.id, write_err
                                ));
                                (
                                    "模型请求失败".to_string(),
                                    format!("模型请求重试后仍失败: {error}"),
                                )
                            }
                        }
                    } else {
                        (
                            "模型请求失败".to_string(),
                            format!("模型请求重试后仍失败: {error}"),
                        )
                    }
                }
            };

            if attempt < max_failure_retries {
                let retry_index = attempt + 1;
                let wait_seconds = FIXED_MODEL_RETRY_WAIT_SECONDS;
                let _ = on_delta.send(AssistantDeltaEvent {
                    delta: "".to_string(),
                    kind: Some("tool_status".to_string()),
                    request_id: None,
                    activation_id: runtime_context.request_id.clone(),
                    phase_id: None,
                    reason: None,
                    tool_name: None,
                    tool_status: Some("running".to_string()),
                    tool_args: None,
                    message: Some(format!(
                        "{reason_text}，正在重试 ({retry_index}/{max_failure_retries})，等待 {wait_seconds} 秒..."
                    )),
                });
                tokio::time::sleep(std::time::Duration::from_secs(wait_seconds)).await;
                continue;
            }
            let total_attempts = max_failure_retries + 1;
            candidate_final_error = Some(format!(
                "{final_error_text} (attempted {total_attempts} times)"
            ));
        }
        if model_reply.is_some() {
            break;
        }
        if let Some(error) = candidate_final_error {
            fallback_errors.push(format!("{}: {}", candidate_selected_api.name, error));
        }
        if candidate_index + 1 < candidate_api_ids.len() {
            let _ = on_delta.send(AssistantDeltaEvent {
                delta: "".to_string(),
                kind: Some("tool_status".to_string()),
                request_id: None,
                activation_id: runtime_context.request_id.clone(),
                phase_id: None,
                reason: None,
                tool_name: None,
                tool_status: Some("running".to_string()),
                tool_args: None,
                message: Some(format!(
                    "当前模型失败，正在切换到下一个候选模型（{}/{}）...",
                    candidate_index + 2,
                    candidate_api_ids.len()
                )),
            });
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
    let model_reply =
        model_reply.ok_or_else(|| {
            if fallback_errors.is_empty() {
                "模型回复无效：未收到可用内容。".to_string()
            } else {
                format!("所有候选模型均失败：{}", fallback_errors.join(" | "))
            }
        })?;
    let assistant_text = model_reply.assistant_text;
    let reasoning_standard = model_reply.reasoning_standard;
    let reasoning_inline = model_reply.reasoning_inline;
    let assistant_provider_meta_override = model_reply.assistant_provider_meta;
    let tool_history_events = model_reply.tool_history_events;
    let suppress_assistant_message = model_reply.suppress_assistant_message;
    let mut remote_im_reply_decision =
        remote_im_extract_reply_decision_from_tool_history(&tool_history_events);
    let pending_remote_im_auto_send_target = resolve_remote_im_auto_send_target(
        &assistant_text,
        effective_bound_remote_im_activation_source(
            Some(&runtime_context),
            &remote_im_activation_sources,
        )
        .as_ref()
        .map(std::slice::from_ref)
        .unwrap_or(&[]),
        remote_im_reply_decision.as_ref(),
    )?;
    if let Some(target) = pending_remote_im_auto_send_target.as_ref() {
        if remote_im_reply_decision.is_none() {
            remote_im_reply_decision = Some(RemoteImReplyDecisionSummary {
                action: "send_async".to_string(),
                target: Some(RemoteImReplyTarget {
                    channel_id: target.channel_id.clone(),
                    contact_id: target.remote_contact_id.clone(),
                }),
            });
        }
    }
    let trusted_input_tokens = model_reply.trusted_input_tokens;
    let estimated_prompt_tokens = estimated_prompt_tokens.unwrap_or_else(|| {
        if trusted_input_tokens.is_some() {
            0
        } else {
            conversation_prompt_service().estimate_prepared_prompt_tokens(
                &prepared_prompt,
                &active_selected_api,
                &current_agent,
            )
        }
    });
    let (effective_prompt_tokens, effective_prompt_source) =
        effective_prompt_tokens_from_provider(estimated_prompt_tokens, trusted_input_tokens);
    let context_usage_ratio =
        effective_prompt_tokens as f64 / f64::from(active_selected_api.context_window_tokens.max(1));
    let context_usage_percent = context_usage_ratio.mul_add(100.0, 0.0).round().clamp(0.0, 100.0) as u32;

    let assistant_text_for_storage = assistant_text.clone();
    let remote_im_conversation_kind = if is_remote_im_contact_conversation {
        "remote_im_contact"
    } else {
        "standard_conversation"
    };
    let mut provider_meta = {
        let standard = reasoning_standard.trim();
        let inline = reasoning_inline.trim();
        if !should_create_assistant_provider_meta(
            &active_selected_api.request_format,
            standard,
            inline,
            assistant_provider_meta_override.as_ref(),
            trusted_input_tokens,
            estimated_prompt_tokens,
            remote_im_reply_decision.is_some(),
        ) {
            None
        } else {
            let mut meta = serde_json::json!({
                "reasoningStandard": standard,
                "reasoningInline": inline,
                "providerPromptTokens": trusted_input_tokens,
                "estimatedPromptTokens": estimated_prompt_tokens,
                "effectivePromptTokens": effective_prompt_tokens,
                "effectivePromptSource": effective_prompt_source,
                "contextUsagePercent": context_usage_percent,
                "contextUsageRatio": context_usage_ratio
            });
            if let Some(decision) = remote_im_reply_decision.as_ref() {
                if let Some(obj) = meta.as_object_mut() {
                    obj.insert(
                        "remoteImDecision".to_string(),
                        serde_json::json!({
                            "action": decision.action,
                            "processingMode": remote_im_contact_processing_mode,
                            "conversationKind": remote_im_conversation_kind,
                            "activationSourceCount": remote_im_activation_sources.len(),
                            "target": decision.target,
                        }),
                    );
                }
            }
            Some(meta)
        }
    };
    if let Some(extra_meta) = assistant_provider_meta_override {
        let mut merged = provider_meta.take().unwrap_or_else(|| serde_json::json!({}));
        if !merged.is_object() {
            runtime_log_warn(format!(
                "[聊天] 助理 provider_meta 不是对象，合并前已保留原始值: value={}",
                merged
            ));
            let raw_provider_meta = std::mem::replace(&mut merged, serde_json::json!({}));
            merged = serde_json::json!({
                "_raw_provider_meta": raw_provider_meta,
            });
        }
        if let Some(target) = merged.as_object_mut() {
            if let Some(extra_object) = extra_meta.as_object() {
                for (key, value) in extra_object {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
        provider_meta = Some(merged);
    }
    let assistant_message_id = Uuid::new_v4().to_string();
    let persisted_meme_segments =
        resolve_text_to_persisted_meme_segments(&state, &assistant_text_for_storage, &assistant_message_id)?;
    persist_meme_segments_into_provider_meta(
        &mut provider_meta,
        persisted_meme_segments.as_deref(),
    );
    log_run_stage("model_reply_ready");

    let mut persisted_assistant_message: Option<ChatMessage> = None;
    {
        match conversation_service().read_persisted_conversation(&state, &conversation_id) {
            Ok(mut conversation) => {
                if !conversation.summary.trim().is_empty() {
                    return Err(format!("指定会话不存在或不可用：{}", conversation_id));
                }
                let now = now_iso();
                if !suppress_assistant_message {
                    let assistant_message = ChatMessage {
                        id: assistant_message_id.clone(),
                        role: "assistant".to_string(),
                        created_at: now.clone(),
                        speaker_agent_id: Some(effective_agent_id.clone()),
                        parts: vec![MessagePart::Text {
                            text: assistant_text_for_storage.clone(),
                        }],
                        extra_text_blocks: Vec::new(),
                        provider_meta,
                        tool_call: if tool_history_events.is_empty() {
                            None
                        } else {
                            Some(tool_history_events)
                        },
                        mcp_call: None,
                    };
                    conversation.messages.push(assistant_message.clone());
                    increment_conversation_unread_count(&mut conversation, 1);
                    persisted_assistant_message = Some(assistant_message);
                    conversation.updated_at = now.clone();
                    conversation.last_assistant_at = Some(now);
                }
                conversation_service().persist_conversation_with_chat_index(
                    &state,
                    &conversation,
                )?;
            }
            Err(err) if !err.contains("not found") => return Err(err),
            Err(_) => {
                if let Some(mut conversation) =
                    delegate_runtime_thread_conversation_get(&state, &conversation_id)?
                {
                    let now = now_iso();
                    if !suppress_assistant_message {
                        let assistant_message = ChatMessage {
                            id: assistant_message_id.clone(),
                            role: "assistant".to_string(),
                            created_at: now.clone(),
                            speaker_agent_id: Some(effective_agent_id.clone()),
                            parts: vec![MessagePart::Text {
                                text: assistant_text_for_storage.clone(),
                            }],
                            extra_text_blocks: Vec::new(),
                            provider_meta,
                            tool_call: if tool_history_events.is_empty() {
                                None
                            } else {
                                Some(tool_history_events)
                            },
                            mcp_call: None,
                        };
                        conversation.messages.push(assistant_message.clone());
                        increment_conversation_unread_count(&mut conversation, 1);
                        persisted_assistant_message = Some(assistant_message);
                        conversation.updated_at = now.clone();
                        conversation.last_assistant_at = Some(now);
                    }
                    delegate_runtime_thread_conversation_update(&state, &conversation_id, conversation)?;
                }
            }
        }
    }
    log_run_stage("assistant_message_persist_scheduled");

    if let Some(activation_source) = pending_remote_im_auto_send_target {
        spawn_remote_im_auto_send_contact_assistant_reply(
            state.clone(),
            activation_source,
            conversation_id.clone(),
            assistant_text.clone(),
            persisted_assistant_message.clone(),
            persisted_assistant_message.as_ref().map(|message| message.id.clone()),
        );
    }

        break Ok(SendChatResult {
            conversation_id,
            latest_user_text,
            assistant_text,
            final_response_text: model_reply.final_response_text,
            reasoning_standard,
            reasoning_inline,
            archived_before_send: archived_before_send_any,
            assistant_message: persisted_assistant_message,
            provider_prompt_tokens: trusted_input_tokens,
            estimated_prompt_tokens: Some(estimated_prompt_tokens),
            effective_prompt_tokens: Some(effective_prompt_tokens),
            effective_prompt_source: Some(effective_prompt_source.to_string()),
            context_window_tokens: Some(active_selected_api.context_window_tokens),
            max_output_tokens: active_resolved_api.max_output_tokens,
            context_usage_percent: Some(context_usage_percent),
            remote_im_reply_decision: remote_im_reply_decision
                .as_ref()
                .map(|item| item.action.clone()),
            remote_im_reply_target: remote_im_reply_decision.and_then(|item| item.target),
        });
    }
    };

    let result = futures_util::future::Abortable::new(run, abort_registration).await;
    emit_conversation_work_status(match &result {
        Ok(Ok(_)) | Err(_) => "completed",
        Ok(Err(_)) => "error",
    });
    flush_chat_timeline("send_chat_message_inner.finish");
    {
        let mut inflight = state
            .inflight_chat_abort_handles
            .lock()
            .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
        inflight.remove(&chat_key);
    }
    if let Err(err) = clear_inflight_tool_abort_handle(state, &chat_key) {
        eprintln!(
            "[聊天] 清理进行中工具中断句柄失败 (session={}): {}",
            chat_key, err
        );
    }
    let final_result = match result {
        Ok(inner) => inner,
        Err(_) => {
            eprintln!(
                "[聊天] 用户中止聊天请求 (session={})",
                chat_key
            );
            Err(CHAT_ABORTED_BY_USER_ERROR.to_string())
        }
    };
    if final_result
        .as_ref()
        .err()
        .map(|err| err != CHAT_ABORTED_BY_USER_ERROR)
        .unwrap_or(true)
    {
        if let Err(err) = clear_inflight_completed_tool_history(state, &chat_key) {
            eprintln!(
                "[聊天] 清理已完成工具历史缓存失败 (session={}): {}",
                chat_key, err
            );
        }
    }
    let timeline = stage_timeline.lock().ok().map(|items| items.clone());
    let (mut pipeline_headers, pipeline_tools) = latest_chat_round_headers_and_tools(
        state,
        Some(&chat_session_key_for_log),
        resolved_api_for_log.request_format,
        &selected_api_for_log.name,
        &selected_api_for_log.model,
        &resolved_api_for_log.base_url,
    );
    if pipeline_headers.is_empty() {
        pipeline_headers = masked_auth_headers(&selected_api_for_log.api_key);
    }
    push_llm_round_log(
        Some(state),
        Some(trace_id),
        Some(chat_session_key_for_log.clone()),
        "chat_pipeline",
        resolved_api_for_log.request_format,
        &selected_api_for_log.name,
        &selected_api_for_log.model,
        &resolved_api_for_log.base_url,
        pipeline_headers,
        pipeline_tools,
        final_result
            .as_ref()
            .ok()
            .map(|value| serde_json::json!({
                "conversationId": value.conversation_id,
                "assistantTextLength": value.assistant_text.chars().count(),
                "reasoningStandardLength": value.reasoning_standard.chars().count(),
                "reasoningInlineLength": value.reasoning_inline.chars().count(),
                "usage": {
                    "rigPromptTokens": value.provider_prompt_tokens,
                    "estimatedPromptTokens": value.estimated_prompt_tokens,
                    "effectivePromptTokens": value.effective_prompt_tokens,
                    "effectivePromptSource": value.effective_prompt_source,
                    "contextWindowTokens": value.context_window_tokens,
                    "maxOutputTokens": value.max_output_tokens,
                    "contextUsagePercent": value.context_usage_percent
                }
            })),
        final_result.as_ref().err().cloned(),
        chat_started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        timeline,
    );
    // 兜底催处理：归档阶段可能短暂阻塞出队，这里在当前轮次结束后补一次调度触发。
    trigger_chat_queue_processing(state);
    final_result
}

fn should_create_assistant_provider_meta(
    request_format: &RequestFormat,
    reasoning_standard: &str,
    reasoning_inline: &str,
    assistant_provider_meta_override: Option<&Value>,
    trusted_input_tokens: Option<u64>,
    estimated_prompt_tokens: u64,
    remote_im_reply_decision_present: bool,
) -> bool {
    !reasoning_standard.trim().is_empty()
        || !reasoning_inline.trim().is_empty()
        || assistant_provider_meta_override.is_some()
        || trusted_input_tokens.is_some()
        || estimated_prompt_tokens > 0
        || remote_im_reply_decision_present
        || matches!(request_format, RequestFormat::DeepSeekKimi)
}

#[cfg(test)]
mod core_send_inner_tests {
    use super::*;

    fn test_chat_api(id: &str, enable_image: bool) -> ApiConfig {
        ApiConfig {
            id: id.to_string(),
            name: id.to_string(),
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: false,
            enable_text: true,
            enable_image,
            enable_audio: false,
            enable_tools: false,
            tools: vec![],
            base_url: "https://example.com/v1".to_string(),
            api_key: "k".to_string(),
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
            model: format!("model-{id}"),
            reasoning_effort: default_reasoning_effort(),
            temperature: 0.7,
            custom_temperature_enabled: false,
            context_window_tokens: 128_000,
            max_output_tokens: 4_096,
            custom_max_output_tokens_enabled: false,
            failure_retry_count: 0,
        }
    }

    #[test]
    fn prioritize_requested_chat_api_id_should_move_requested_id_to_front() {
        let app_config = AppConfig {
            api_configs: vec![test_chat_api("text-a", false), test_chat_api("vision-b", true)],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let mut candidate_api_ids = vec!["text-a".to_string(), "vision-b".to_string()];

        prioritize_requested_chat_api_id(Some("vision-b"), &mut candidate_api_ids, &app_config)
            .expect("prioritize requested api id");

        assert_eq!(
            candidate_api_ids,
            vec!["vision-b".to_string(), "text-a".to_string()]
        );
    }

    #[test]
    fn prioritize_requested_chat_api_id_should_insert_requested_chat_model_not_in_department_list() {
        let app_config = AppConfig {
            api_configs: vec![test_chat_api("text-a", false), test_chat_api("vision-b", true)],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let mut candidate_api_ids = vec!["text-a".to_string()];

        prioritize_requested_chat_api_id(Some("vision-b"), &mut candidate_api_ids, &app_config)
            .expect("prioritize requested api id");

        assert_eq!(
            candidate_api_ids,
            vec!["vision-b".to_string(), "text-a".to_string()]
        );
    }

    #[test]
    fn prioritize_requested_chat_api_id_should_reject_non_chat_or_missing_model() {
        let mut embedding_api = test_chat_api("embed-a", false);
        embedding_api.request_format = RequestFormat::OpenAIEmbedding;
        let app_config = AppConfig {
            api_configs: vec![test_chat_api("text-a", false), embedding_api],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let mut candidate_api_ids = vec!["text-a".to_string()];

        let non_chat_err =
            prioritize_requested_chat_api_id(Some("embed-a"), &mut candidate_api_ids, &app_config)
                .expect_err("non-chat model should be rejected");
        let missing_err =
            prioritize_requested_chat_api_id(Some("missing"), &mut candidate_api_ids, &app_config)
                .expect_err("missing model should be rejected");

        assert_eq!(candidate_api_ids, vec!["text-a".to_string()]);
        assert!(non_chat_err.contains("不是聊天文本模型"));
        assert!(missing_err.contains("模型不存在"));
    }

    #[test]
    fn should_create_assistant_provider_meta_should_preserve_empty_reasoning_for_deepseek() {
        assert!(should_create_assistant_provider_meta(
            &RequestFormat::DeepSeekKimi,
            "",
            "",
            None,
            None,
            0,
            false,
        ));
        assert!(!should_create_assistant_provider_meta(
            &RequestFormat::OpenAI,
            "",
            "",
            None,
            None,
            0,
            false,
        ));
    }
}
fn error_indicates_image_input_unsupported(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("no endpoints found that support image input")
        || normalized.contains("does not support image input")
        || normalized.contains("image input disabled")
}

fn auto_disable_api_image_input(
    state: &AppState,
    api_config_id: &str,
) -> Result<bool, String> {
    let mut config = read_config(&state.config_path)?;
    let (api_id, api_name) = {
        let Some(target) = config.api_configs.iter_mut().find(|item| item.id == api_config_id) else {
            return Ok(false);
        };
        if !target.enable_image {
            return Ok(false);
        }
        target.enable_image = false;
        (target.id.clone(), target.name.clone())
    };
    state_write_config_cached(state, &config)?;
    runtime_log_info(format!(
        "[聊天] 自动关闭图片模态: api_config_id={}, api_name={}",
        api_id, api_name
    ));
    Ok(true)
}
