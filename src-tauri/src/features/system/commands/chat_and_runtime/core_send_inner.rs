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

fn remote_im_is_reply_decision_action(action: &str) -> bool {
    matches!(
        action.trim().to_ascii_lowercase().as_str(),
        "send" | "send_async" | "no_reply"
    )
}

fn remote_im_extract_reply_decision_from_tool_history(events: &[Value]) -> Option<String> {
    let mut latest_action: Option<String> = None;
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
            if name.trim() != "remote_im_send" {
                continue;
            }
            let Some(action) = function
                .get("arguments")
                .and_then(remote_im_extract_action_from_tool_arguments)
            else {
                continue;
            };
            if remote_im_is_reply_decision_action(&action) {
                latest_action = Some(action);
            }
        }
    }
    latest_action
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
        .clone()
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
    mut conversation: Conversation,
    user_message: ChatMessage,
    now: &str,
) -> Conversation {
    conversation.messages.push(user_message);
    conversation.updated_at = now.to_string();
    conversation.last_user_at = Some(now.to_string());
    conversation
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
            "This round was activated by exactly one remote IM source.\n{}\nIf you do not explicitly call `remote_im_send`, the system may automatically send your final assistant reply to that source.\nIf you need to reply to another target or choose not to send, you must explicitly call `remote_im_send`.",
            source_lines
        ),
        ("en-US", _) => format!(
            "This round was activated by multiple remote IM sources.\n{}\nThe system will not auto-send any final reply in this round.\nIf you need to send anything outward, you must explicitly call `remote_im_send` and specify the target `channel_id` + `contact_id`.",
            source_lines
        ),
        ("zh-TW", 1) => format!(
            "本輪由唯一一個遠端 IM 來源啟動。\n{}\n若你未明確呼叫 `remote_im_send`，系統可能會在本輪結束後自動將最終回覆發送到該來源。\n若要改發其他目標，或決定不外發，必須明確呼叫 `remote_im_send`。",
            source_lines
        ),
        ("zh-TW", _) => format!(
            "本輪由多個遠端 IM 來源共同啟動。\n{}\n系統不會自動外發本輪最終回覆。\n若需要對外發送，必須明確呼叫 `remote_im_send`，並指定目標 `channel_id` + `contact_id`。",
            source_lines
        ),
        (_, 1) => format!(
            "本轮由唯一一个远程 IM 来源激活。\n{}\n如果你未显式调用 `remote_im_send`，系统可能会在本轮结束后自动将最终回复发送到该来源。\n如果需要改发其他目标，或决定不外发，必须显式调用 `remote_im_send`。",
            source_lines
        ),
        _ => format!(
            "本轮由多个远程 IM 来源共同激活。\n{}\n系统不会自动外发本轮最终回复。\n如果需要对外发送，必须显式调用 `remote_im_send`，并指定目标 `channel_id` + `contact_id`。",
            source_lines
        ),
    };
    Some(prompt_xml_block("remote im runtime activation", block))
}

fn resolve_remote_im_auto_send_target(
    assistant_text: &str,
    activation_sources: &[RemoteImActivationSource],
    has_explicit_reply_decision: bool,
) -> Result<Option<RemoteImActivationSource>, String> {
    if has_explicit_reply_decision || activation_sources.is_empty() {
        return Ok(None);
    }
    if activation_sources.len() >= 2 {
        return Err(
            "本轮由多个远程IM来源共同激活，系统不会自动发送；如需外发，请显式调用 remote_im_send 指定目标。"
                .to_string(),
        );
    }
    if assistant_text.trim().is_empty() {
        return Err(
            "本轮由唯一远程IM来源激活，但未产生可自动发送的回复内容，也未调用 remote_im_send。"
                .to_string(),
        );
    }
    Ok(activation_sources.first().cloned())
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
    let conversation = data.conversations.iter().find(|item| item.id == conversation_id)?;
    let contact_conversation_key = conversation
        .root_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(key) = contact_conversation_key {
        return data.remote_im_contacts.iter().find(|contact| {
            remote_im_contact_conversation_key(contact) == key
        });
    }
    data.remote_im_contacts.iter().find(|contact| {
        contact
            .bound_conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            == Some(conversation_id)
    })
}

fn remote_im_send_tool_history_events(args: &RemoteImSendToolArgs, tool_result: &str) -> Vec<Value> {
    let args_value = serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string());
    let tool_call_id = format!("remote_im_send_auto_{}", Uuid::new_v4());
    vec![
        serde_json::json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": [{
                "id": tool_call_id,
                "type": "function",
                "function": {
                    "name": "remote_im_send",
                    "arguments": args_value
                }
            }]
        }),
        serde_json::json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "content": sanitize_tool_result_for_history("remote_im_send", tool_result)
        }),
    ]
}

async fn remote_im_auto_send_assistant_reply_to_source(
    state: &AppState,
    source: &RemoteImActivationSource,
    assistant_text: &str,
) -> Result<Option<(String, Vec<Value>)>, String> {
    let trimmed_text = assistant_text.trim();
    if trimmed_text.is_empty() {
        return Ok(None);
    }
    let args = RemoteImSendToolArgs {
        action: "send".to_string(),
        channel_id: Some(source.channel_id.clone()),
        contact_id: Some(source.remote_contact_id.clone()),
        text: Some(trimmed_text.to_string()),
        status: "done".to_string(),
        file_paths: None,
    };
    let tool_result = serde_json::to_string(
        &builtin_remote_im_send(state, args.clone()).await?
    )
    .map_err(|err| format!("serialize auto remote_im_send result failed: {err}"))?;
    Ok(Some((
        "send".to_string(),
        remote_im_send_tool_history_events(&args, &tool_result),
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
    assistant_message_id: Option<&str>,
) -> Result<RemoteImAutoSendExecutionOutcome, String> {
    match remote_im_auto_send_assistant_reply_to_source(state, activation_source, assistant_text).await {
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
            assistant_message_id.as_deref(),
        )
        .await
        {
            Ok(RemoteImAutoSendExecutionOutcome::Sent { action }) => {
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
    let _guard = lock_conversation_with_metrics(
        state,
        "update_remote_im_reply_decision_for_message",
    )?;
    let mut data = state_read_app_data_cached(state)?;

    let update_message = |message: &mut ChatMessage| {
        let mut meta = message
            .provider_meta
            .clone()
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

    if let Some(conversation) = data
        .conversations
        .iter_mut()
        .find(|item| item.id == conversation_id && item.summary.trim().is_empty())
    {
        if let Some(message) = conversation.messages.iter_mut().rev().find(|message| {
            message.role.trim() == "assistant"
                && assistant_message_id
                    .map(|target_id| message.id == target_id)
                    .unwrap_or(true)
        }) {
            update_message(message);
            let _ = state_schedule_app_data_persist(state, &data)?;
            return Ok(());
        }
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
    let queue_wait_ms = oldest_queue_created_at
        .as_deref()
        .and_then(parse_iso)
        .map(|created_at| (now_utc() - created_at).whole_milliseconds())
        .filter(|ms| *ms > 0)
        .map(|ms| ms.min(i128::from(u64::MAX)) as u64);
    let session_for_log = input.session.clone();
    let remote_im_activation_sources = input.remote_im_activation_sources.clone();

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
        prompt_conversation_before: Conversation,
        cached_effective_prompt_tokens_before_send: u64,
        cached_usage_ratio_before_send: f64,
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
            last_read_message_id: String::new(),
            conversation_kind: String::new(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: String::new(),
            updated_at: String::new(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
            status: String::new(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
        }
    };
    let requested_conversation_id_for_prepare = requested_conversation_id.clone();
    let runtime_conversation_id_for_prepare = runtime_conversation_id.clone();
    let runtime_conversation_for_prepare = runtime_conversation.clone();
    let build_prepare_snapshot = |
        data: &mut AppData,
        runtime_agents: &[AgentProfile],
        selected_api: &ApiConfig,
        effective_agent_id: &str,
    | -> Result<ConversationPrepareSnapshot, String> {
        let current_agent = runtime_agents
            .iter()
            .find(|a| a.id == effective_agent_id)
            .cloned()
            .ok_or_else(|| "Selected agent not found.".to_string())?;
        let requested_conversation_idx = requested_conversation_id_for_prepare.as_deref().and_then(|conversation_id| {
            data.conversations
                .iter()
                .position(|item| item.id == conversation_id && item.summary.trim().is_empty())
        });
        let is_runtime_conversation =
            requested_conversation_id_for_prepare.is_some()
                && requested_conversation_idx.is_none()
                && runtime_conversation_id_for_prepare.is_some();
        let idx = if let Some(requested_idx) = requested_conversation_idx {
            Some(requested_idx)
        } else if is_runtime_conversation {
            None
        } else if let Some(conversation_id) = requested_conversation_id_for_prepare.as_deref() {
            Some(
                data.conversations
                    .iter()
                    .position(|item| {
                        item.id == conversation_id && item.summary.trim().is_empty()
                    })
                    .ok_or_else(|| format!("指定会话不存在或不可用：{conversation_id}"))?,
            )
        } else {
            Some(ensure_active_foreground_conversation_index_atomic(
                data,
                &state.data_path,
                &selected_api.id,
                effective_agent_id,
            ))
        };
        if idx.is_some() {
            for conversation in &mut data.conversations {
                if conversation_is_delegate(conversation)
                    || !conversation.summary.trim().is_empty()
                {
                    continue;
                }
                conversation.status = "active".to_string();
            }
        }

        let conversation_before = if let Some(actual_idx) = idx {
            data.conversations[actual_idx].clone()
        } else {
            runtime_conversation_for_prepare.clone()
        };
        let is_remote_im_contact_conversation =
            conversation_is_remote_im_contact(&conversation_before);
        let remote_im_contact_processing_mode = if is_remote_im_contact_conversation {
            remote_im_find_contact_by_conversation(data, &conversation_before.id)
                .map(|contact| normalize_contact_processing_mode(&contact.processing_mode))
                .unwrap_or_else(|| "continuous".to_string())
        } else {
            "continuous".to_string()
        };
        let cached_effective_prompt_tokens_before_send =
            conversation_before.last_effective_prompt_tokens;
        let cached_usage_ratio_before_send =
            if conversation_before.last_context_usage_ratio.is_finite() {
                conversation_before.last_context_usage_ratio.max(0.0)
            } else {
                0.0
            };
        let last_archive_summary =
            if is_runtime_conversation || conversation_is_delegate(&conversation_before) {
                None
            } else {
                data.conversations
                    .iter()
                    .rev()
                    .find(|c| !conversation_is_delegate(c) && !c.summary.trim().is_empty())
                    .map(|c| c.summary.clone())
            };
        let prompt_conversation_before = if is_remote_im_contact_conversation
            && remote_im_contact_processing_mode == "qa"
        {
            let trimmed = remote_im_trim_conversation_for_qa_mode(&conversation_before);
            eprintln!(
                "[远程IM] 问答模式裁剪会话上下文: conversation_id={}, original_messages={}, trimmed_messages={}",
                conversation_before.id,
                conversation_before.messages.len(),
                trimmed.messages.len()
            );
            trimmed
        } else {
            conversation_before.clone()
        };

        Ok(ConversationPrepareSnapshot {
            current_agent,
            agents: runtime_agents.to_vec(),
            response_style_id: data.response_style_id.clone(),
            user_name: user_persona_name(data),
            user_intro: user_persona_intro(data),
            last_archive_summary,
            prompt_conversation_before,
            cached_effective_prompt_tokens_before_send,
            cached_usage_ratio_before_send,
            is_remote_im_contact_conversation,
            remote_im_contact_processing_mode,
            enable_pdf_images: data.pdf_read_mode == "image" && selected_api.enable_image,
            is_runtime_conversation,
            runtime_conversation_id: runtime_conversation_id_for_prepare.clone(),
        })
    };
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
        let requested_conversation_idx = requested_conversation_id_for_prepare
            .as_deref()
            .and_then(|conversation_id| {
                data.conversations
                    .iter()
                    .position(|item| item.id == conversation_id && item.summary.trim().is_empty())
            });
        let is_runtime_conversation =
            requested_conversation_id_for_prepare.is_some()
                && requested_conversation_idx.is_none()
                && runtime_conversation_id_for_prepare.is_some();
        let idx = if let Some(requested_idx) = requested_conversation_idx {
            Some(requested_idx)
        } else if is_runtime_conversation {
            None
        } else if requested_conversation_id_for_prepare.is_some() {
            return Ok(None);
        } else {
            active_foreground_conversation_index_read_only(data, effective_agent_id)
        };
        let conversation_before = if let Some(actual_idx) = idx {
            data.conversations
                .get(actual_idx)
                .cloned()
                .ok_or_else(|| "前台会话索引无效".to_string())?
        } else {
            runtime_conversation_for_prepare.clone()
        };
        let is_remote_im_contact_conversation =
            conversation_is_remote_im_contact(&conversation_before);
        let remote_im_contact_processing_mode = if is_remote_im_contact_conversation {
            remote_im_find_contact_by_conversation(data, &conversation_before.id)
                .map(|contact| normalize_contact_processing_mode(&contact.processing_mode))
                .unwrap_or_else(|| "continuous".to_string())
        } else {
            "continuous".to_string()
        };
        let cached_effective_prompt_tokens_before_send =
            conversation_before.last_effective_prompt_tokens;
        let cached_usage_ratio_before_send =
            if conversation_before.last_context_usage_ratio.is_finite() {
                conversation_before.last_context_usage_ratio.max(0.0)
            } else {
                0.0
            };
        let last_archive_summary =
            if is_runtime_conversation || conversation_is_delegate(&conversation_before) {
                None
            } else {
                data.conversations
                    .iter()
                    .rev()
                    .find(|c| !conversation_is_delegate(c) && !c.summary.trim().is_empty())
                    .map(|c| c.summary.clone())
            };
        let prompt_conversation_before = if is_remote_im_contact_conversation
            && remote_im_contact_processing_mode == "qa"
        {
            let trimmed = remote_im_trim_conversation_for_qa_mode(&conversation_before);
            eprintln!(
                "[远程IM] 问答模式裁剪会话上下文: conversation_id={}, original_messages={}, trimmed_messages={}",
                conversation_before.id,
                conversation_before.messages.len(),
                trimmed.messages.len()
            );
            trimmed
        } else {
            conversation_before.clone()
        };
        Ok(Some(ConversationPrepareSnapshot {
            current_agent,
            agents: runtime_agents.to_vec(),
            response_style_id: data.response_style_id.clone(),
            user_name: user_persona_name(data),
            user_intro: user_persona_intro(data),
            last_archive_summary,
            prompt_conversation_before,
            cached_effective_prompt_tokens_before_send,
            cached_usage_ratio_before_send,
            is_remote_im_contact_conversation,
            remote_im_contact_processing_mode,
            enable_pdf_images: data.pdf_read_mode == "image" && selected_api.enable_image,
            is_runtime_conversation,
            runtime_conversation_id: runtime_conversation_id_for_prepare.clone(),
        }))
    };

    let (app_config, selected_api, resolved_api, effective_department_id, effective_agent_id, candidate_api_ids, preloaded_prepare_snapshot) = {
        let prepare_started = std::time::Instant::now();
        let mut prepare_detail_parts = Vec::<String>::new();
        let lock_wait_started = std::time::Instant::now();
        let _guard = lock_conversation_with_metrics(
            state,
            "send_chat_message_inner_runtime_and_session_ready",
        )?;
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
        let (assistant_department_agent_id, mut runtime_agents, app_data_read_detail) = with_app_data_cached_ref(
            state,
            |cached_data, detail| {
                Ok((
                    cached_data.assistant_department_agent_id.clone(),
                    cached_data.agents.clone(),
                    detail.clone(),
                ))
            },
        )?;
        let app_data_read_ms = app_data_started
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        prepare_detail_parts.push(format!(
            "应用数据读取={}ms(source={}, dirty_fast_path={}, mtime_before={}ms, cache_lookup={}ms, disk_read={}ms, mtime_after={}ms, cache_write={}ms, total={}ms)",
            app_data_read_ms,
            app_data_read_detail.source,
            app_data_read_detail.dirty_fast_path,
            app_data_read_detail.mtime_before_ms,
            app_data_read_detail.cache_lookup_ms,
            app_data_read_detail.disk_read_ms,
            app_data_read_detail.mtime_after_ms,
            app_data_read_detail.cache_write_ms,
            app_data_read_detail.total_ms,
        ));
        log_chat_stage("runtime_and_session_ready.app_data_read_done");
        prepare_detail_parts.push(format!("运行时人格列表克隆=0ms(count={})", runtime_agents.len()));
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
            return Err(format!(
                "Agent '{effective_agent_id}' is not assigned to department '{}'.",
                effective_department.id
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
        let preloaded_prepare_snapshot = match with_app_data_cached_ref(state, |cached_data, _detail| {
            build_prepare_snapshot_read_only(
                cached_data,
                &runtime_agents,
                &selected_api,
                &effective_agent_id,
            )
        })? {
            Some(snapshot) => Some(snapshot),
            None => {
                let mut snapshot_data = with_app_data_cached_ref(state, |cached_data, _detail| {
                    Ok(cached_data.clone())
                })?;
                Some(build_prepare_snapshot(
                    &mut snapshot_data,
                    &runtime_agents,
                    &selected_api,
                    &effective_agent_id,
                )?)
            }
        };
        (
            app_config,
            selected_api,
            resolved_api,
            effective_department_id,
            effective_agent_id,
            candidate_api_ids,
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
                        let _guard =
                            lock_conversation_with_metrics(&state, "image_text_cache_read")?;
                        let data = state_read_app_data_cached(&state)?;
                        find_image_text_cache(&data, &hash, &vision_api.id)
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

                    let _guard =
                        lock_conversation_with_metrics(&state, "image_text_cache_write")?;
                    let mut data = state_read_app_data_cached(&state)?;
                    let mapped = if let Some(existing) =
                        find_image_text_cache(&data, &hash, &vision_api.id)
                    {
                        format!("[图片{}]\n{}", idx + 1, existing)
                    } else {
                        upsert_image_text_cache(&mut data, &hash, &vision_api.id, &converted);
                        let _ = state_schedule_app_data_persist(&state, &data)?;
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
    let effective_images = effective_user_parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Image {
                mime, bytes_base64, ..
            } => Some((mime.clone(), bytes_base64.clone())),
            _ => None,
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
        .collect::<Vec<_>>();

    let mut archived_before_send = false;

    let mut preloaded_prepare_snapshot = preloaded_prepare_snapshot;
    let mut prepare_request_context = |persist_user_message: bool| -> Result<_, String> {
        log_run_stage("prepare_context.begin");
        let snapshot = if let Some(snapshot) = preloaded_prepare_snapshot.take() {
            log_run_stage("prepare_context.foreground_conversation_ready");
            snapshot
        } else {
            let _guard =
                lock_conversation_with_metrics(&state, "prepare_context_snapshot")?;
            log_run_stage("prepare_context.conversation_lock_wait_done");
            let mut data = state_read_app_data_cached(&state)?;
            log_run_stage("prepare_context.app_data_read_done");
            let mut runtime_agents = data.agents.clone();
            let mut runtime_config = app_config.clone();
            merge_private_organization_into_runtime(
                &state.data_path,
                &mut runtime_config,
                &mut runtime_agents,
            )?;
            let snapshot = build_prepare_snapshot(
                &mut data,
                &runtime_agents,
                &selected_api,
                &effective_agent_id,
            )?;
            log_run_stage("prepare_context.foreground_conversation_ready");
            snapshot
        };
        log_run_stage("prepare_context.conversation_snapshot_ready");
        log_run_stage("prepare_context.archive_summary_ready");
        log_run_stage("prepare_context.prompt_conversation_ready");
        log_run_stage("prepare_context.base_context_ready");
        let is_delegate_conversation =
            snapshot.prompt_conversation_before.conversation_kind.trim() == CONVERSATION_KIND_DELEGATE;
        let conversation = if trigger_only {
            let latest_message = snapshot
                .prompt_conversation_before
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
            snapshot.prompt_conversation_before.clone()
        } else if !persist_user_message {
            snapshot.prompt_conversation_before.clone()
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
            let user_message = ChatMessage {
                id: Uuid::new_v4().to_string(),
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
                snapshot.prompt_conversation_before.clone(),
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
                let updated_data = {
                    let _guard = lock_conversation_with_metrics(
                        &state,
                        "prepare_context_commit_user_message",
                    )?;
                    let mut data = state_read_app_data_cached(&state)?;
                    let actual_idx = data
                        .conversations
                        .iter()
                        .position(|item| {
                            item.id == updated_conversation.id
                                && item.summary.trim().is_empty()
                        })
                        .ok_or_else(|| {
                            format!("指定会话不存在或不可用：{}", updated_conversation.id)
                        })?;
                    data.conversations[actual_idx] = updated_conversation.clone();
                    for memory_id in &recall_payload.raw_ids {
                        data.conversations[actual_idx]
                            .memory_recall_table
                            .push(memory_id.clone());
                    }
                    data
                };
                log_run_stage("prepare_context.user_message_committed");
                let _ = state_schedule_app_data_persist(&state, &updated_data)?;
                log_run_stage("prepare_context.state_persist_scheduled");
                updated_conversation
            }
        };
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
        let mut chat_overrides = ChatPromptOverrides::default();
        chat_overrides
            .system_preamble_blocks
            .push(build_hidden_skill_snapshot_block(&state));
        if todo_enabled {
            chat_overrides
                .system_preamble_blocks
                .push(build_todo_guide_block());
        }
        if let Some(runtime_block) = build_remote_im_activation_runtime_block(
            &remote_im_activation_sources,
            &app_config.ui_language,
        ) {
            chat_overrides.system_preamble_blocks.push(runtime_block);
        }
        if !trigger_only {
            chat_overrides.latest_user_text = Some(latest_user_text.clone());
            if !is_delegate_conversation {
                if let Some(task_board) = build_hidden_task_board_block(&state) {
                    chat_overrides.latest_user_extra_blocks.push(task_board);
                }
            }
            if todo_enabled {
                if let Some(todo_board) = build_conversation_todo_board_block(&conversation) {
                    chat_overrides.latest_user_extra_blocks.push(todo_board);
                }
            }
            let attachment_meta = normalize_payload_attachments(input.payload.attachments.as_ref());
            for item in attachment_meta {
                let relative_path = item
                    .get("relativePath")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .unwrap_or("");
                if relative_path.is_empty() {
                    continue;
                }
                chat_overrides.latest_user_extra_blocks.push(format!(
                    "用户上传了附件，文件位于你工作区的 downloads 目录（路径：{}）。\n你可以先用 shell 工具定位或查看基础文件信息；具体解析方式应按文件类型选择合适 skill 或在线检索正确方法。\n仅当用户明确要求处理该附件时再处理；若用户未明确要求，请先询问用户想如何处理。",
                    relative_path
                ));
            }
            chat_overrides.latest_images = Some(effective_images.clone());
            chat_overrides.latest_audios = Some(effective_audios.clone());
        }
        log_run_stage("prepare_context.overrides_built");
        let prompt_mode = if is_delegate_conversation {
            PromptBuildMode::Delegate
        } else {
            PromptBuildMode::Chat
        };
        let chat_overrides = Some(chat_overrides);
        let terminal_block = terminal_prompt_trusted_roots_block(&state, &selected_api);
        log_run_stage("prepare_context.prompt_build_begin");
        let prepared_prompt = build_prepared_prompt_for_mode(
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
            terminal_block.clone(),
            chat_overrides.clone(),
            Some(&state),
            Some(&resolved_api),
            Some(snapshot.enable_pdf_images),
        );
        log_run_stage("prepare_context.prompt_built");
        let tool_loop_auto_compaction_context = if snapshot.is_runtime_conversation {
            None
        } else {
            Some(ToolLoopAutoCompactionContext {
                conversation_id: conversation.id.clone(),
                prompt_mode,
                agent: snapshot.current_agent.clone(),
                agents: snapshot.agents.clone(),
                departments: app_config.departments.clone(),
                user_name: snapshot.user_name.clone(),
                user_intro: snapshot.user_intro.clone(),
                response_style_id: snapshot.response_style_id.clone(),
                ui_language: app_config.ui_language.clone(),
                last_archive_summary: snapshot.last_archive_summary.clone(),
                terminal_block,
                chat_overrides: chat_overrides.clone(),
                enable_pdf_images: snapshot.enable_pdf_images,
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
        let estimated_prompt_tokens =
            if snapshot.cached_effective_prompt_tokens_before_send > 0
                || snapshot.cached_usage_ratio_before_send > 0.0
            {
                None
            } else {
                let estimated = estimate_prepared_prompt_tokens(
                    &prepared_prompt,
                    &selected_api,
                    &snapshot.current_agent,
                );
                log_run_stage("prepare_context.prompt_tokens_estimated");
                Some(estimated)
            };
        log_run_stage("prepare_context.done");
        Ok((
            model_name,
            prepared_prompt,
            conversation_id,
            latest_user_text,
            snapshot.current_agent,
            estimated_prompt_tokens,
            snapshot.cached_effective_prompt_tokens_before_send,
            snapshot.cached_usage_ratio_before_send,
            snapshot.is_remote_im_contact_conversation,
            snapshot.remote_im_contact_processing_mode,
            tool_loop_auto_compaction_context,
            conversation,
            snapshot.is_runtime_conversation,
        ))
    };
    let mut prepared_context = prepare_request_context(true)?;
    let conversation_for_compaction = prepared_context.11.clone();
    let estimated_prompt_tokens_before_send = prepared_context.5;
    let cached_effective_prompt_tokens_before_send = prepared_context.6;
    let cached_usage_ratio_before_send = prepared_context.7;
    let is_runtime_conversation = prepared_context.12;
    if is_runtime_conversation {
        if let Some(conversation_id) = requested_conversation_id.as_deref() {
            eprintln!(
                "[ARCHIVE] check before user message skipped: conversation_id={}, reason=delegate_runtime_thread",
                conversation_id
            );
        }
    } else {
        let (decision, decision_source) = decide_archive_before_send_with_fallback(
            cached_effective_prompt_tokens_before_send,
            cached_usage_ratio_before_send,
            estimated_prompt_tokens_before_send,
            selected_api.context_window_tokens,
            conversation_for_compaction.last_user_at.as_deref(),
            archive_pipeline_has_assistant_reply(&conversation_for_compaction),
        );
        eprintln!(
            "[ARCHIVE] check before user message: should_archive={}, forced={}, reason={}, usage_ratio={:.4}, source={}, cached_effective_prompt_tokens={}, cached_usage_ratio={:.4}, estimated_prompt_tokens={:?}, context_window_tokens={}",
            decision.should_archive,
            decision.forced,
            decision.reason,
            decision.usage_ratio,
            decision_source,
            cached_effective_prompt_tokens_before_send,
            cached_usage_ratio_before_send,
            estimated_prompt_tokens_before_send,
            selected_api.context_window_tokens
        );
        if decision.should_archive {
            if decision.forced {
                let _ = on_delta.send(AssistantDeltaEvent {
                    delta: "".to_string(),
                    kind: Some("tool_status".to_string()),
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
                    archived_before_send = result.archived;
                    if decision.forced {
                        let done_message = if result.warning.as_deref().unwrap_or("").trim().is_empty() {
                            "整理完成，将继续当前会话。".to_string()
                        } else {
                            format!(
                                "整理完成（降级摘要）：{}",
                                result.warning.unwrap_or_default()
                            )
                        };
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: "".to_string(),
                            kind: Some("tool_status".to_string()),
                            tool_name: Some("archive".to_string()),
                            tool_status: Some("done".to_string()),
                            tool_args: None,
                            message: Some(done_message),
                        });
                    }
                    prepared_context = prepare_request_context(false)?;
                }
                Err(err) => {
                    if decision.forced {
                        let _ = on_delta.send(AssistantDeltaEvent {
                            delta: "".to_string(),
                            kind: Some("tool_status".to_string()),
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
        _cached_effective_prompt_tokens_before_send,
        _cached_usage_ratio_before_send,
        is_remote_im_contact_conversation,
        remote_im_contact_processing_mode,
        tool_loop_auto_compaction_context,
        _conversation_for_request,
        _is_runtime_conversation,
    ) = prepared_context;
    log_run_stage("prompt_ready");

    let mut model_reply: Option<ModelReply> = None;
    let mut active_selected_api = selected_api.clone();
    let mut active_resolved_api = resolved_api.clone();
    let mut fallback_errors = Vec::<String>::new();
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
        let candidate_resolved_api =
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
        let max_failure_retries = FIXED_MODEL_RETRY_COUNT;
        let mut candidate_final_error: Option<String> = None;
        for attempt in 0..=max_failure_retries {
            let request_start_stage = format!(
                "model_request.start[candidate_api_id={},attempt={}]",
                candidate_selected_api.id,
                attempt + 1
            );
            log_run_stage(&request_start_stage);
            let reply_result = call_model_openai_style(
                &candidate_resolved_api,
                &app_config,
                &candidate_selected_api,
                &current_agent,
                &candidate_model_name,
                prepared_prompt.clone(),
                Some(&state),
                tool_loop_auto_compaction_context.as_ref(),
                on_delta,
                app_config.tool_max_iterations as usize,
                &chat_session_key,
            )
            .await;
            let request_finish_stage = format!(
                "model_request.finish[candidate_api_id={},attempt={}]",
                candidate_selected_api.id,
                attempt + 1
            );
            log_run_stage(&request_finish_stage);

            let (reason_text, final_error_text) = match reply_result {
                Ok(reply) => {
                    if model_reply_has_visible_content(&reply) {
                        active_selected_api = candidate_selected_api.clone();
                        active_resolved_api = candidate_resolved_api.clone();
                        model_reply = Some(reply);
                        candidate_final_error = None;
                        break;
                    }
                    if request_format_supports_non_stream_fallback(
                        candidate_selected_api.request_format,
                    ) {
                        if let Err(mark_err) = provider_mark_streaming_disabled(
                            Some(&state),
                            &candidate_resolved_api.base_url,
                        ) {
                            runtime_log_warn(format!(
                                "[聊天] 空响应后标记本次运行内非流式失败: base_url={}, err={}",
                                candidate_resolved_api.base_url, mark_err
                            ));
                        } else {
                            runtime_log_info(format!(
                                "[聊天] 模型返回空响应，已在本次运行内切换非流式重试: base_url={}, model={}",
                                candidate_resolved_api.base_url, candidate_model_name
                            ));
                        }
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
    let tool_history_events = model_reply.tool_history_events;
    let suppress_assistant_message = model_reply.suppress_assistant_message;
    let mut remote_im_reply_decision =
        remote_im_extract_reply_decision_from_tool_history(&tool_history_events);
    let pending_remote_im_auto_send_target = resolve_remote_im_auto_send_target(
        &assistant_text,
        &remote_im_activation_sources,
        remote_im_reply_decision.is_some(),
    )?;
    if pending_remote_im_auto_send_target.is_some() && remote_im_reply_decision.is_none() {
        remote_im_reply_decision = Some("send_async".to_string());
    }
    let trusted_input_tokens = model_reply.trusted_input_tokens;
    let estimated_prompt_tokens = estimated_prompt_tokens.unwrap_or_else(|| {
        if trusted_input_tokens.is_some() {
            0
        } else {
            estimate_prepared_prompt_tokens(&prepared_prompt, &active_selected_api, &current_agent)
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
    let provider_meta = {
        let standard = reasoning_standard.trim();
        let inline = reasoning_inline.trim();
        if standard.is_empty()
            && inline.is_empty()
            && trusted_input_tokens.is_none()
            && estimated_prompt_tokens == 0
            && remote_im_reply_decision.is_none()
        {
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
            if let Some(action) = remote_im_reply_decision.as_deref() {
                if let Some(obj) = meta.as_object_mut() {
                    obj.insert(
                        "remoteImDecision".to_string(),
                        serde_json::json!({
                            "action": action,
                            "processingMode": remote_im_contact_processing_mode,
                            "conversationKind": remote_im_conversation_kind,
                            "activationSourceCount": remote_im_activation_sources.len()
                        }),
                    );
                }
            }
            Some(meta)
        }
    };
    log_run_stage("model_reply_ready");

    let mut persisted_assistant_message: Option<ChatMessage> = None;
    let mut app_data_for_persist: Option<AppData> = None;
    {
        let _guard =
            lock_conversation_with_metrics(&state, "assistant_message_commit")?;

        let mut data = state_read_app_data_cached(&state)?;
        if let Some(conversation) = data
            .conversations
            .iter_mut()
            .find(|c| c.id == conversation_id && c.summary.trim().is_empty())
        {
            let now = now_iso();
            if !suppress_assistant_message {
                let assistant_message = ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "assistant".to_string(),
                    created_at: now.clone(),
                    speaker_agent_id: Some(effective_agent_id.clone()),
                    parts: vec![MessagePart::Text {
                        text: assistant_text_for_storage.clone(),
                    }],
                    extra_text_blocks: Vec::new(),
                    provider_meta: provider_meta.clone(),
                    tool_call: if tool_history_events.is_empty() {
                        None
                    } else {
                        Some(tool_history_events.clone())
                    },
                    mcp_call: None,
                };
                conversation.messages.push(assistant_message.clone());
                persisted_assistant_message = Some(assistant_message);
                conversation.updated_at = now.clone();
                conversation.last_assistant_at = Some(now);
            }
            conversation.last_effective_prompt_tokens = effective_prompt_tokens;
            conversation.last_context_usage_ratio = context_usage_ratio;
            app_data_for_persist = Some(data);
        } else if let Some(mut conversation) =
            delegate_runtime_thread_conversation_get(&state, &conversation_id)?
        {
            let now = now_iso();
            if !suppress_assistant_message {
                let assistant_message = ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "assistant".to_string(),
                    created_at: now.clone(),
                    speaker_agent_id: Some(effective_agent_id.clone()),
                    parts: vec![MessagePart::Text {
                        text: assistant_text_for_storage.clone(),
                    }],
                    extra_text_blocks: Vec::new(),
                    provider_meta: provider_meta.clone(),
                    tool_call: if tool_history_events.is_empty() {
                        None
                    } else {
                        Some(tool_history_events.clone())
                    },
                    mcp_call: None,
                };
                conversation.messages.push(assistant_message.clone());
                persisted_assistant_message = Some(assistant_message);
                conversation.updated_at = now.clone();
                conversation.last_assistant_at = Some(now);
            }
            conversation.last_effective_prompt_tokens = effective_prompt_tokens;
            conversation.last_context_usage_ratio = context_usage_ratio;
            delegate_runtime_thread_conversation_update(&state, &conversation_id, conversation)?;
        }
    }
    if let Some(data) = app_data_for_persist.as_ref() {
        let _ = state_schedule_app_data_persist(&state, data)?;
    }
    log_run_stage("assistant_message_persist_scheduled");

    if let Some(activation_source) = pending_remote_im_auto_send_target {
        spawn_remote_im_auto_send_contact_assistant_reply(
            state.clone(),
            activation_source,
            conversation_id.clone(),
            assistant_text.clone(),
            persisted_assistant_message.as_ref().map(|message| message.id.clone()),
        );
    }

    Ok(SendChatResult {
        conversation_id,
        latest_user_text,
        assistant_text,
        reasoning_standard,
        reasoning_inline,
        archived_before_send,
        assistant_message: persisted_assistant_message,
        provider_prompt_tokens: trusted_input_tokens,
        estimated_prompt_tokens: Some(estimated_prompt_tokens),
        effective_prompt_tokens: Some(effective_prompt_tokens),
        effective_prompt_source: Some(effective_prompt_source.to_string()),
        context_window_tokens: Some(active_selected_api.context_window_tokens),
        max_output_tokens: active_resolved_api.max_output_tokens,
        context_usage_percent: Some(context_usage_percent),
    })
    };

    let result = futures_util::future::Abortable::new(run, abort_registration).await;
    log_chat_stage("send_chat_message_inner.finish");
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
    let timeline = stage_timeline.lock().ok().map(|items| items.clone());
    let (mut pipeline_headers, pipeline_tools) = latest_chat_round_headers_and_tools(
        state,
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
        "chat_pipeline",
        resolved_api_for_log.request_format,
        &selected_api_for_log.name,
        &selected_api_for_log.model,
        &resolved_api_for_log.base_url,
        pipeline_headers,
        pipeline_tools,
        serde_json::json!({
            "triggerOnly": trigger_only,
            "queueWaitMs": queue_wait_ms,
            "oldestQueueCreatedAt": oldest_queue_created_at,
            "chatSessionKey": chat_session_key_for_log,
            "effectiveDepartmentId": effective_department_id,
            "session": session_for_log,
            "runtimeContext": runtime_context,
        }),
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

#[cfg(test)]
mod core_send_inner_tests {
    use super::*;

    fn test_chat_api(id: &str, enable_image: bool) -> ApiConfig {
        ApiConfig {
            id: id.to_string(),
            name: id.to_string(),
            request_format: RequestFormat::OpenAI,
            enable_text: true,
            enable_image,
            enable_audio: false,
            enable_tools: false,
            tools: vec![],
            base_url: "https://example.com/v1".to_string(),
            api_key: "k".to_string(),
            model: format!("model-{id}"),
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

