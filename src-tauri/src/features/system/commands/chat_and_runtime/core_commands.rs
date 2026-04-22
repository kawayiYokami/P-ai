fn normalize_payload_image_attachments(
    raw: Option<&Vec<BinaryPart>>,
) -> Vec<serde_json::Value> {
    let mut out = Vec::<serde_json::Value>::new();
    let Some(images) = raw else {
        return out;
    };
    let mut seen = std::collections::HashSet::<String>::new();
    for image in images {
        let relative_path = image
            .saved_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.replace('\\', "/"));
        let Some(relative_path) = relative_path else {
            continue;
        };
        let file_name = std::path::Path::new(&relative_path)
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("attachment")
            .to_string();
        let mime = image.mime.trim().to_string();
        let dedup_key = format!("{}::{}", relative_path, mime);
        if !seen.insert(dedup_key) {
            continue;
        }
        out.push(serde_json::json!({
            "fileName": file_name,
            "relativePath": relative_path,
            "mime": mime,
        }));
    }
    out
}

fn normalize_payload_mentions(
    raw: Option<&Vec<UserMentionTargetInput>>,
) -> Vec<serde_json::Value> {
    let Some(items) = raw else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::<String>::new();
    let mut out = Vec::<serde_json::Value>::new();
    for item in items.iter().take(3) {
        let agent_id = item.agent_id.trim();
        let department_id = item.department_id.trim();
        if agent_id.is_empty() || department_id.is_empty() {
            continue;
        }
        let dedup_key = format!("{agent_id}::{department_id}");
        if !seen.insert(dedup_key) {
            continue;
        }
        out.push(serde_json::json!({
            "agentId": agent_id,
            "agentName": item.agent_name.as_deref().map(str::trim).filter(|value| !value.is_empty()).unwrap_or(agent_id),
            "departmentId": department_id,
            "departmentName": item.department_name.as_deref().map(str::trim).filter(|value| !value.is_empty()).unwrap_or(department_id),
        }));
    }
    out
}

fn build_user_message_provider_meta(
    input_provider_meta: Option<Value>,
    attachments: &[serde_json::Value],
    mentions: &[serde_json::Value],
) -> Option<Value> {
    let merged = merge_provider_meta_with_attachments(input_provider_meta, attachments);
    let mut root = match merged {
        Some(Value::Object(map)) => map,
        Some(other) => {
            let mut map = serde_json::Map::new();
            map.insert("_raw".to_string(), other);
            map
        }
        None => serde_json::Map::new(),
    };

    let message_meta_value = root
        .remove("message_meta")
        .or_else(|| root.remove("messageMeta"))
        .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
    let mut message_meta = match message_meta_value {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    message_meta.insert("kind".to_string(), Value::String("user_message".to_string()));
    if !mentions.is_empty() {
        message_meta.insert("mentions".to_string(), Value::Array(mentions.to_vec()));
    }
    root.insert("message_meta".to_string(), Value::Object(message_meta));
    Some(Value::Object(root))
}

#[derive(Debug, Clone)]
struct UserMentionPlan {
    root_conversation_id: String,
    source_department_id: String,
    source_agent_id: String,
    target_department_id: String,
    target_agent_id: String,
    target_agent_name: String,
    instruction: String,
    background: String,
    target_api_config_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct UserMentionFailurePlan {
    root_conversation_id: String,
    source_agent_id: String,
    target_department_id: String,
    target_agent_id: String,
    target_agent_name: String,
    reason: String,
}

fn build_user_mention_context_snapshot(
    conversation: &Conversation,
    data: &AppData,
    latest_user_text: &str,
) -> String {
    let mut lines = Vec::<String>::new();
    let recent_messages = conversation
        .messages
        .iter()
        .rev()
        .filter_map(|message| {
            let text = render_prompt_message_text(message);
            if text.trim().is_empty() {
                return None;
            }
            let speaker_name = match message.role.trim() {
                "user" => "用户".to_string(),
                "assistant" | "tool" => {
                    let speaker_agent_id = message
                        .speaker_agent_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .unwrap_or("");
                    data.agents
                        .iter()
                        .find(|agent| agent.id == speaker_agent_id)
                        .map(|agent| agent.name.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| "助理".to_string())
                }
                other => other.to_string(),
            };
            Some(format!("[{}] {}", speaker_name, text.trim()))
        })
        .take(12)
        .collect::<Vec<_>>();
    for line in recent_messages.into_iter().rev() {
        lines.push(line);
    }
    if !latest_user_text.trim().is_empty() {
        lines.push(format!("[当前用户问题] {}", latest_user_text.trim()));
    }
    lines.join("\n")
}

fn first_available_department_agent_id(
    department: &DepartmentConfig,
    data: &AppData,
) -> Option<String> {
    department
        .agent_ids
        .iter()
        .map(|item| item.trim())
        .find(|agent_id| {
            !agent_id.is_empty()
                && data
                    .agents
                    .iter()
                    .any(|agent| agent.id == *agent_id && !agent.is_built_in_user)
        })
        .map(ToOwned::to_owned)
}

fn resolve_session_binding_for_send<'a>(
    app_config: &'a AppConfig,
    data: &AppData,
    requested_department_id: Option<&str>,
    requested_agent_id: &str,
) -> Result<(&'a DepartmentConfig, String), String> {
    let requested_agent_id = requested_agent_id.trim();
    if requested_agent_id.is_empty() {
        return Err("会话信息不完整".to_string());
    }

    if let Some(department_id) = requested_department_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let department = department_by_id(app_config, department_id)
            .ok_or_else(|| "部门已经消失".to_string())?;
        if department
            .agent_ids
            .iter()
            .any(|item| item.trim() == requested_agent_id)
        {
            return Ok((department, requested_agent_id.to_string()));
        }
        let rebound_agent_id = first_available_department_agent_id(department, data)
            .ok_or_else(|| format!("部门没有可用人格：{}", department.id))?;
        return Ok((department, rebound_agent_id));
    }

    let department = department_for_agent_id(app_config, requested_agent_id)
        .or_else(|| assistant_department(app_config))
        .ok_or_else(|| "未找到可用部门".to_string())?;
    if department
        .agent_ids
        .iter()
        .any(|item| item.trim() == requested_agent_id)
    {
        return Ok((department, requested_agent_id.to_string()));
    }
    let rebound_agent_id = first_available_department_agent_id(department, data)
        .ok_or_else(|| format!("部门没有可用人格：{}", department.id))?;
    Ok((department, rebound_agent_id))
}

fn enqueue_user_mention_result_message(
    app_state: &AppState,
    root_conversation_id: &str,
    source_agent_id: &str,
    target_department_id: &str,
    target_agent_id: &str,
    text: &str,
    provider_meta: Value,
) -> Result<(), String> {
    let delegate_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(target_agent_id.to_string()),
        parts: vec![MessagePart::Text {
            text: text.to_string(),
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(provider_meta),
        tool_call: None,
        mcp_call: None,
    };
    append_delegate_result_message_and_emit(
        app_state,
        root_conversation_id,
        &delegate_message,
        false,
        Some(ChatSessionInfo {
            department_id: target_department_id.to_string(),
            agent_id: source_agent_id.to_string(),
        }),
    )
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversationMessageAppendedPayload {
    conversation_id: String,
    message: ChatMessage,
}

fn emit_conversation_message_appended_event(
    app_state: &AppState,
    conversation_id: &str,
    message: &ChatMessage,
) {
    let app_handle = match app_state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            eprintln!(
                "[聊天推送] append 消息事件发送失败：锁已损坏，conversation_id={}, error={:?}",
                conversation_id, err
            );
            None
        }
    };
    let Some(app_handle) = app_handle else {
        eprintln!(
            "[聊天推送] append 消息事件发送失败：app_handle 不可用，conversation_id={}",
            conversation_id
        );
        return;
    };
    let payload = ConversationMessageAppendedPayload {
        conversation_id: conversation_id.to_string(),
        message: message.clone(),
    };
    if let Err(err) = app_handle.emit(CHAT_CONVERSATION_MESSAGE_APPENDED_EVENT, payload) {
        eprintln!(
            "[聊天推送] append 消息事件发送失败：conversation_id={}, message_id={}, error={}",
            conversation_id,
            message.id,
            err
        );
    }
}

fn append_delegate_result_message_and_emit(
    app_state: &AppState,
    conversation_id: &str,
    message: &ChatMessage,
    continue_main_assistant: bool,
    session_info: Option<ChatSessionInfo>,
) -> Result<(), String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("缺少 conversation_id，无法写回委托结果".to_string());
    }
    let _guard = lock_conversation_with_metrics(app_state, "append_delegate_result_message")?;
    let mut data = state_read_app_data_cached(app_state)?;
    let conversation = data
        .conversations
        .iter_mut()
        .find(|item| {
            item.id == conversation_id
                && item.summary.trim().is_empty()
                && item.archived_at.as_deref().map(str::trim).unwrap_or("").is_empty()
                && !conversation_is_delegate(item)
        })
        .ok_or_else(|| format!("原会话不可写，conversationId={conversation_id}"))?;
    conversation.messages.push(message.clone());
    conversation.updated_at = message.created_at.clone();
    conversation.last_assistant_at = Some(message.created_at.clone());
    persist_single_conversation_runtime_fast(app_state, &data, conversation_id)?;
    emit_conversation_message_appended_event(app_state, conversation_id, message);

    if continue_main_assistant {
        let session_info = session_info.ok_or_else(|| "缺少 session_info，无法继续主助理".to_string())?;
        tauri::async_runtime::spawn({
            let state = app_state.clone();
            let conversation_id = conversation_id.to_string();
            async move {
                let oldest_queue_created_at = now_iso();
                if let Err(err) = activate_main_assistant(
                    &state,
                    &session_info,
                    &conversation_id,
                    None,
                    None,
                    Vec::new(),
                    &oldest_queue_created_at,
                )
                .await
                {
                    eprintln!(
                        "[委托结果] 追加后继续主助理失败: conversation_id={}, department_id={}, agent_id={}, error={}",
                        conversation_id,
                        session_info.department_id,
                        session_info.agent_id,
                        err
                    );
                }
            }
        });
    }

    Ok(())
}

fn spawn_user_mention_failure_message(app_state: AppState, failure: UserMentionFailurePlan) {
    tokio::spawn(async move {
        let text = format!("《用户@委托：{}》执行失败：{}", failure.target_agent_name.trim(), failure.reason.trim());
        if let Err(err) = enqueue_user_mention_result_message(
            &app_state,
            &failure.root_conversation_id,
            &failure.source_agent_id,
            &failure.target_department_id,
            &failure.target_agent_id,
            &text,
            serde_json::json!({
                "messageKind": "delegate_result",
                "delegateKind": DELEGATE_TOOL_KIND_USER_MENTION,
                "resultStatus": "failed",
                "speakerAgentId": failure.target_agent_id,
                "sourceAgentId": failure.source_agent_id,
                "targetAgentId": failure.target_agent_id,
                "error": failure.reason,
            }),
        ) {
            eprintln!(
                "[用户@委托] 写回失败结果消息失败: conversation_id={}, target_agent_id={}, error={}",
                failure.root_conversation_id,
                failure.target_agent_id,
                err
            );
        }
    });
}

fn spawn_user_mention_delegate(app_state: AppState, plan: UserMentionPlan) {
    tokio::spawn(async move {
        let delegate = match delegate_create_record(
            &app_state,
            DELEGATE_TOOL_KIND_USER_MENTION,
            &plan.root_conversation_id,
            None,
            &plan.source_department_id,
            &plan.target_department_id,
            &plan.source_agent_id,
            &plan.target_agent_id,
            &format!("用户@委托：{}", plan.target_agent_name.trim()),
            &plan.instruction,
            plan.background.clone(),
            String::new(),
            "请直接基于当前上下文作答，不要复述委托框架。".to_string(),
            false,
            vec![
                plan.source_department_id.clone(),
                plan.target_department_id.clone(),
            ],
        ) {
            Ok(value) => value,
            Err(err) => {
                spawn_user_mention_failure_message(
                    app_state,
                    UserMentionFailurePlan {
                        root_conversation_id: plan.root_conversation_id,
                        source_agent_id: plan.source_agent_id,
                        target_department_id: plan.target_department_id,
                        target_agent_id: plan.target_agent_id,
                        target_agent_name: plan.target_agent_name,
                        reason: err,
                    },
                );
                return;
            }
        };
        let target_agent_name = plan.target_agent_name.clone();
        let run_result = delegate_run_thread_to_completion(
            app_state.clone(),
            delegate.clone(),
            plan.target_api_config_ids.clone(),
            None,
        )
        .await;
        match run_result {
            Ok(run) => {
                let text = if run.assistant_text.trim().is_empty() {
                    format!("《用户@委托：{}》已处理完成。", target_agent_name.trim())
                } else {
                    run.assistant_text.clone()
                };
                if let Err(err) = enqueue_user_mention_result_message(
                    &app_state,
                    &plan.root_conversation_id,
                    &plan.source_agent_id,
                    &plan.target_department_id,
                    &plan.target_agent_id,
                    &text,
                    serde_json::json!({
                        "messageKind": "delegate_result",
                        "delegateId": delegate.delegate_id,
                        "delegateKind": DELEGATE_TOOL_KIND_USER_MENTION,
                        "resultStatus": "completed",
                        "speakerAgentId": plan.target_agent_id,
                        "sourceAgentId": plan.source_agent_id,
                        "targetAgentId": plan.target_agent_id,
                        "reasoningStandard": run.reasoning_standard,
                    }),
                ) {
                    eprintln!(
                        "[用户@委托] 写回完成结果消息失败: conversation_id={}, target_agent_id={}, error={}",
                        plan.root_conversation_id,
                        plan.target_agent_id,
                        err
                    );
                }
            }
            Err(err) => {
                let fail_text = format!("《用户@委托：{}》执行失败：{}", target_agent_name.trim(), err.trim());
                if let Err(enqueue_err) = enqueue_user_mention_result_message(
                    &app_state,
                    &plan.root_conversation_id,
                    &plan.source_agent_id,
                    &plan.target_department_id,
                    &plan.target_agent_id,
                    &fail_text,
                    serde_json::json!({
                        "messageKind": "delegate_result",
                        "delegateId": delegate.delegate_id,
                        "delegateKind": DELEGATE_TOOL_KIND_USER_MENTION,
                        "resultStatus": "failed",
                        "speakerAgentId": plan.target_agent_id,
                        "sourceAgentId": plan.source_agent_id,
                        "targetAgentId": plan.target_agent_id,
                        "error": err,
                    }),
                ) {
                    eprintln!(
                        "[用户@委托] 写回失败结果消息失败: conversation_id={}, target_agent_id={}, error={}",
                        plan.root_conversation_id,
                        plan.target_agent_id,
                        enqueue_err
                    );
                }
            }
        }
    });
}

#[tauri::command]
async fn send_chat_message(
    input: SendChatRequest,
    state: State<'_, AppState>,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {
    if input
        .payload
        .mentions
        .as_ref()
        .map(|items| !items.is_empty())
        .unwrap_or(false)
    {
        return send_user_mention_message_inner(input, state.inner(), &on_delta).await;
    }

    // 如果是 trigger_only 模式（由调度器调用），直接执行
    if input.trigger_only {
        return send_chat_message_inner(input, state.inner(), &on_delta).await;
    }

    // 用户发言：构造消息并入队
    let text = input.payload.text.as_deref().unwrap_or("").trim();
    let images = input.payload.images.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
    let attachments = input
        .payload
        .attachments
        .as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    if text.is_empty() && images.is_empty() && attachments.is_empty() {
        return Err("消息内容为空".to_string());
    }

    let display_text = input
        .payload
        .display_text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(text)
        .to_string();
    let normalized_mentions = normalize_payload_mentions(input.payload.mentions.as_ref());

    let mut message_parts = Vec::new();
    if !text.is_empty() {
        message_parts.push(MessagePart::Text { text: text.to_string() });
    }
    for img in images {
        message_parts.push(MessagePart::Image {
            mime: img.mime.clone(),
            bytes_base64: img.bytes_base64.clone(),
            name: None,
            compressed: false,
        });
    }
    let user_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: message_parts,
        extra_text_blocks: input.payload.extra_text_blocks.clone().unwrap_or_default(),
        provider_meta: {
            let mut attachment_entries =
                normalize_payload_attachments(input.payload.attachments.as_ref());
            attachment_entries.extend(normalize_payload_image_attachments(
                input.payload.images.as_ref(),
            ));
            build_user_message_provider_meta(
                input.payload.provider_meta.clone(),
                &attachment_entries,
                &normalized_mentions,
            )
        },
        tool_call: None,
        mcp_call: None,
    };

    // 获取会话信息
    let session = input.session.as_ref().ok_or_else(|| "缺少会话信息".to_string())?;
    let requested_department_id = session
        .department_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let requested_agent_id = session.agent_id.trim().to_string();

    if requested_agent_id.is_empty() {
        return Err("会话信息不完整".to_string());
    }

    // 获取或创建会话ID
    let prepare_started_at = std::time::Instant::now();
    let (conversation_id, department_id, agent_id, model_config_id, mention_plans, mention_failures) = {
        let _guard = lock_conversation_with_metrics(&state, "send_chat_message_prepare_conversation")?;
        let config_started_at = std::time::Instant::now();
        let app_config = state_read_config_cached(&state)?;
        let config_elapsed_ms = config_started_at.elapsed().as_millis();
        let app_data_started_at = std::time::Instant::now();
        let mut data = state_read_app_data_cached(&state)?;
        let app_data_elapsed_ms = app_data_started_at.elapsed().as_millis();
        let department_started_at = std::time::Instant::now();
        let (department, agent_id) = resolve_session_binding_for_send(
            &app_config,
            &data,
            requested_department_id.as_deref(),
            &requested_agent_id,
        )?;
        let department_elapsed_ms = department_started_at.elapsed().as_millis();
        let api_config_id = department_primary_api_config_id(department);
        if api_config_id.trim().is_empty() {
            return Err(format!("部门模型未配置: {}", department.id));
        }

        let conversation_started_at = std::time::Instant::now();
        let conversation_id = if let Some(cid) = session
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if data.conversations.iter().any(|conv| {
                conv.id == cid
                    && conv.summary.trim().is_empty()
            }) {
                cid.to_string()
            } else {
                let idx = ensure_active_foreground_conversation_index_atomic(
                    &mut data,
                    &state.data_path,
                    &api_config_id,
                    &agent_id,
                );
                let fallback_id = data.conversations
                    .get(idx)
                    .map(|item| item.id.clone())
                    .ok_or_else(|| "活动会话索引超出范围".to_string())?;
                let reject_reason = data
                    .conversations
                    .iter()
                    .find(|conv| conv.id == cid)
                    .map(|conv| {
                        if !conv.summary.trim().is_empty() {
                            "summary_present"
                        } else {
                            "unknown"
                        }
                    })
                    .unwrap_or("not_found");
                eprintln!(
                    "[聊天] 会话 conversation_id 被拒绝，已选择回退会话: requested_cid={}, reject_reason={}, fallback_cid={}, department_id={}, agent_id={}",
                    cid,
                    reject_reason,
                    fallback_id,
                    department.id,
                    agent_id
                );
                fallback_id
            }
        } else {
            let idx = ensure_active_foreground_conversation_index_atomic(
                &mut data,
                &state.data_path,
                &api_config_id,
                &agent_id,
            );
            data.conversations
                .get(idx)
                .map(|item| item.id.clone())
                .ok_or_else(|| "活动会话索引超出范围".to_string())?
        };
        let conversation_elapsed_ms = conversation_started_at.elapsed().as_millis();
        let conversation_snapshot = data
            .conversations
            .iter()
            .find(|item| item.id == conversation_id && item.summary.trim().is_empty())
            .cloned()
            .ok_or_else(|| format!("未找到目标会话，conversationId={conversation_id}"))?;
        if let Some(conversation) = data
            .conversations
            .iter_mut()
            .find(|item| item.id == conversation_id && item.summary.trim().is_empty())
        {
            conversation.department_id = department.id.clone();
            conversation.agent_id = agent_id.clone();
        }
        let mention_background = build_user_mention_context_snapshot(
            &conversation_snapshot,
            &data,
            &display_text,
        );
        let mut mention_plans = Vec::<UserMentionPlan>::new();
        let mut mention_failures = Vec::<UserMentionFailurePlan>::new();
        let mut seen_mention_agents = std::collections::HashSet::<String>::new();
        for mention in input
            .payload
            .mentions
            .as_ref()
            .map(|items| items.iter().take(3))
            .into_iter()
            .flatten()
        {
            let target_agent_id = mention.agent_id.trim().to_string();
            let target_department_id = mention.department_id.trim().to_string();
            let target_agent_name = mention
                .agent_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| {
                    data.agents
                        .iter()
                        .find(|agent| agent.id == target_agent_id)
                        .map(|agent| agent.name.trim().to_string())
                        .filter(|value| !value.is_empty())
                })
                .unwrap_or_else(|| target_agent_id.clone());
            if target_agent_id.is_empty()
                || target_department_id.is_empty()
                || !seen_mention_agents.insert(target_agent_id.clone())
            {
                continue;
            }
            if target_agent_id == agent_id {
                continue;
            }
            let Some(target_department) = department_by_id(&app_config, &target_department_id) else {
                mention_failures.push(UserMentionFailurePlan {
                    root_conversation_id: conversation_id.clone(),
                    source_agent_id: agent_id.clone(),
                    target_department_id: target_department_id.clone(),
                    target_agent_id: target_agent_id.clone(),
                    target_agent_name: target_agent_name.clone(),
                    reason: format!("目标部门不存在，departmentId={target_department_id}"),
                });
                continue;
            };
            if !target_department
                .agent_ids
                .iter()
                .any(|item| item.trim() == target_agent_id)
            {
                mention_failures.push(UserMentionFailurePlan {
                    root_conversation_id: conversation_id.clone(),
                    source_agent_id: agent_id.clone(),
                    target_department_id: target_department_id.clone(),
                    target_agent_id: target_agent_id.clone(),
                    target_agent_name: target_agent_name.clone(),
                    reason: "目标人格已不再属于该部门".to_string(),
                });
                continue;
            }
            if !data.agents.iter().any(|agent| agent.id == target_agent_id && !agent.is_built_in_user) {
                mention_failures.push(UserMentionFailurePlan {
                    root_conversation_id: conversation_id.clone(),
                    source_agent_id: agent_id.clone(),
                    target_department_id: target_department_id.clone(),
                    target_agent_id: target_agent_id.clone(),
                    target_agent_name: target_agent_name.clone(),
                    reason: format!("目标人格不存在，agentId={target_agent_id}"),
                });
                continue;
            }
            let target_api_config_ids =
                delegate_target_chat_api_config_ids(&app_config, target_department);
            if target_api_config_ids.is_empty() {
                mention_failures.push(UserMentionFailurePlan {
                    root_conversation_id: conversation_id.clone(),
                    source_agent_id: agent_id.clone(),
                    target_department_id: target_department_id.clone(),
                    target_agent_id: target_agent_id.clone(),
                    target_agent_name: target_agent_name.clone(),
                    reason: format!("目标部门未配置可用模型，departmentId={target_department_id}"),
                });
                continue;
            }
            mention_plans.push(UserMentionPlan {
                root_conversation_id: conversation_id.clone(),
                source_department_id: department.id.clone(),
                source_agent_id: agent_id.clone(),
                target_department_id,
                target_agent_id,
                target_agent_name,
                instruction: display_text.clone(),
                background: mention_background.clone(),
                target_api_config_ids,
            });
        }

        let persist_started_at = std::time::Instant::now();
        persist_single_conversation_runtime_fast(&state, &data, &conversation_id)?;
        let persist_elapsed_ms = persist_started_at.elapsed().as_millis();
        eprintln!(
            "[聊天发送] 发送前准备耗时：总计={}ms，读取配置={}ms，读取应用数据={}ms，解析部门={}ms，定位会话={}ms，持久化={}ms，conversation_id={}，department_id={}，agent_id={}",
            prepare_started_at.elapsed().as_millis(),
            config_elapsed_ms,
            app_data_elapsed_ms,
            department_elapsed_ms,
            conversation_elapsed_ms,
            persist_elapsed_ms,
            conversation_id,
            department.id,
            agent_id
        );

        (
            conversation_id,
            department.id.clone(),
            agent_id,
            api_config_id,
            mention_plans,
            mention_failures,
        )
    };

    // 构造队列事件
    let event_id = Uuid::new_v4().to_string();
    let request_id = runtime_context_request_id_or_new(None, input.trace_id.as_deref(), "chat");
    let has_user_mentions = input
        .payload
        .mentions
        .as_ref()
        .map(|items| !items.is_empty())
        .unwrap_or(false);
    let mut runtime_context = runtime_context_new("user_message", "user_send");
    runtime_context.request_id = Some(request_id.clone());
    runtime_context.dispatch_id = Some(event_id.clone());
    runtime_context.origin_conversation_id = Some(conversation_id.clone());
    runtime_context.target_conversation_id = Some(conversation_id.clone());
    runtime_context.root_conversation_id = Some(conversation_id.clone());
    runtime_context.executor_agent_id = Some(agent_id.clone());
    runtime_context.executor_department_id = Some(department_id.clone());
    runtime_context.model_config_id = Some(model_config_id.clone());
    let event = ChatPendingEvent {
        id: event_id.clone(),
        conversation_id: conversation_id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::User,
        messages: vec![user_message],
        activate_assistant: !has_user_mentions,
        session_info: ChatSessionInfo {
            department_id: department_id.clone(),
            agent_id: agent_id.clone(),
        },
        runtime_context: Some(runtime_context.clone()),
        sender_info: None,
    };

    let (result_tx, result_rx) = tokio::sync::oneshot::channel();
    register_chat_event_runtime(state.inner(), &event_id, on_delta.clone(), result_tx)?;

    // 入队前先做阻塞判定：空闲且无排队则直写历史；否则入队。
    let ingress = match ingress_chat_event(state.inner(), event) {
        Ok(value) => value,
        Err(err) => {
            let _ = state
                .pending_chat_delta_channels
                .lock()
                .map(|mut map| map.remove(&event_id));
            let _ = state
                .pending_chat_result_senders
                .lock()
                .map(|mut map| map.remove(&event_id));
            return Err(err);
        }
    };

    // 根据 ingress 结果执行：直写或排队；排队仅在事件仍滞留时才通知前端。
    process_chat_event_after_ingress(state.inner(), ingress).await;

    for failure in mention_failures {
        spawn_user_mention_failure_message(state.inner().clone(), failure);
    }
    for plan in mention_plans {
        spawn_user_mention_delegate(state.inner().clone(), plan);
    }

    result_rx
        .await
        .map_err(|_| "聊天请求已取消或调度结果丢失".to_string())?
}

async fn send_user_mention_message_inner(
    input: SendChatRequest,
    state: &AppState,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {
    let mention_count = input
        .payload
        .mentions
        .as_ref()
        .map(|items| items.iter().filter(|item| !item.agent_id.trim().is_empty()).count())
        .unwrap_or(0);
    if mention_count == 0 {
        return Err("缺少有效的@目标".to_string());
    }

    let text = input.payload.text.as_deref().unwrap_or("").trim();
    let images = input.payload.images.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
    let attachments = input
        .payload
        .attachments
        .as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    if text.is_empty() && images.is_empty() && attachments.is_empty() {
        return Err("消息内容为空".to_string());
    }

    let display_text = input
        .payload
        .display_text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(text)
        .to_string();
    let normalized_mentions = normalize_payload_mentions(input.payload.mentions.as_ref());

    let mut message_parts = Vec::new();
    if !text.is_empty() {
        message_parts.push(MessagePart::Text { text: text.to_string() });
    }
    for img in images {
        message_parts.push(MessagePart::Image {
            mime: img.mime.clone(),
            bytes_base64: img.bytes_base64.clone(),
            name: None,
            compressed: false,
        });
    }
    let user_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: message_parts,
        extra_text_blocks: input.payload.extra_text_blocks.clone().unwrap_or_default(),
        provider_meta: {
            let mut attachment_entries =
                normalize_payload_attachments(input.payload.attachments.as_ref());
            attachment_entries.extend(normalize_payload_image_attachments(
                input.payload.images.as_ref(),
            ));
            build_user_message_provider_meta(
                input.payload.provider_meta.clone(),
                &attachment_entries,
                &normalized_mentions,
            )
        },
        tool_call: None,
        mcp_call: None,
    };

    let session = input.session.as_ref().ok_or_else(|| "缺少会话信息".to_string())?;
    let requested_department_id = session
        .department_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let requested_agent_id = session.agent_id.trim().to_string();

    if requested_agent_id.is_empty() {
        return Err("会话信息不完整".to_string());
    }

    let prepare_started_at = std::time::Instant::now();
    let (conversation_id, department_id, agent_id, model_config_id, mention_plans, mention_failures) = {
        let _guard =
            lock_conversation_with_metrics(state, "send_user_mention_message_prepare_conversation")?;
        let config_started_at = std::time::Instant::now();
        let app_config = state_read_config_cached(state)?;
        let config_elapsed_ms = config_started_at.elapsed().as_millis();
        let app_data_started_at = std::time::Instant::now();
        let mut data = state_read_app_data_cached(state)?;
        let app_data_elapsed_ms = app_data_started_at.elapsed().as_millis();
        let department_started_at = std::time::Instant::now();
        let (department, agent_id) = resolve_session_binding_for_send(
            &app_config,
            &data,
            requested_department_id.as_deref(),
            &requested_agent_id,
        )?;
        let department_elapsed_ms = department_started_at.elapsed().as_millis();
        let api_config_id = department_primary_api_config_id(department);
        if api_config_id.trim().is_empty() {
            return Err(format!("部门模型未配置: {}", department.id));
        }

        let conversation_started_at = std::time::Instant::now();
        let conversation_id = if let Some(cid) = session
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if data
                .conversations
                .iter()
                .any(|conv| conv.id == cid && conv.summary.trim().is_empty())
            {
                cid.to_string()
            } else {
                let idx = ensure_active_foreground_conversation_index_atomic(
                    &mut data,
                    &state.data_path,
                    &api_config_id,
                    &agent_id,
                );
                data.conversations
                    .get(idx)
                    .map(|item| item.id.clone())
                    .ok_or_else(|| "活动会话索引超出范围".to_string())?
            }
        } else {
            let idx = ensure_active_foreground_conversation_index_atomic(
                &mut data,
                &state.data_path,
                &api_config_id,
                &agent_id,
            );
            data.conversations
                .get(idx)
                .map(|item| item.id.clone())
                .ok_or_else(|| "活动会话索引超出范围".to_string())?
        };
        let conversation_elapsed_ms = conversation_started_at.elapsed().as_millis();
        let conversation_snapshot = data
            .conversations
            .iter()
            .find(|item| item.id == conversation_id && item.summary.trim().is_empty())
            .cloned()
            .ok_or_else(|| format!("未找到目标会话，conversationId={conversation_id}"))?;
        if let Some(conversation) = data
            .conversations
            .iter_mut()
            .find(|item| item.id == conversation_id && item.summary.trim().is_empty())
        {
            conversation.department_id = department.id.clone();
            conversation.agent_id = agent_id.clone();
        }
        let mention_background = build_user_mention_context_snapshot(
            &conversation_snapshot,
            &data,
            &display_text,
        );
        let mut mention_plans = Vec::<UserMentionPlan>::new();
        let mut mention_failures = Vec::<UserMentionFailurePlan>::new();
        let mut seen_mention_agents = std::collections::HashSet::<String>::new();
        for mention in input
            .payload
            .mentions
            .as_ref()
            .map(|items| items.iter().take(3))
            .into_iter()
            .flatten()
        {
            let target_agent_id = mention.agent_id.trim().to_string();
            let target_department_id = mention.department_id.trim().to_string();
            let target_agent_name = mention
                .agent_name
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| {
                    data.agents
                        .iter()
                        .find(|agent| agent.id == target_agent_id)
                        .map(|agent| agent.name.trim().to_string())
                        .filter(|value| !value.is_empty())
                })
                .unwrap_or_else(|| target_agent_id.clone());
            if target_agent_id.is_empty()
                || target_department_id.is_empty()
                || !seen_mention_agents.insert(target_agent_id.clone())
            {
                continue;
            }
            if target_agent_id == agent_id {
                continue;
            }
            let Some(target_department) = department_by_id(&app_config, &target_department_id) else {
                mention_failures.push(UserMentionFailurePlan {
                    root_conversation_id: conversation_id.clone(),
                    source_agent_id: agent_id.clone(),
                    target_department_id: target_department_id.clone(),
                    target_agent_id: target_agent_id.clone(),
                    target_agent_name: target_agent_name.clone(),
                    reason: format!("目标部门不存在，departmentId={target_department_id}"),
                });
                continue;
            };
            if !target_department
                .agent_ids
                .iter()
                .any(|item| item.trim() == target_agent_id)
            {
                mention_failures.push(UserMentionFailurePlan {
                    root_conversation_id: conversation_id.clone(),
                    source_agent_id: agent_id.clone(),
                    target_department_id: target_department_id.clone(),
                    target_agent_id: target_agent_id.clone(),
                    target_agent_name: target_agent_name.clone(),
                    reason: "目标人格已不再属于该部门".to_string(),
                });
                continue;
            }
            if !data
                .agents
                .iter()
                .any(|agent| agent.id == target_agent_id && !agent.is_built_in_user)
            {
                mention_failures.push(UserMentionFailurePlan {
                    root_conversation_id: conversation_id.clone(),
                    source_agent_id: agent_id.clone(),
                    target_department_id: target_department_id.clone(),
                    target_agent_id: target_agent_id.clone(),
                    target_agent_name: target_agent_name.clone(),
                    reason: format!("目标人格不存在，agentId={target_agent_id}"),
                });
                continue;
            }
            let target_api_config_ids =
                delegate_target_chat_api_config_ids(&app_config, target_department);
            if target_api_config_ids.is_empty() {
                mention_failures.push(UserMentionFailurePlan {
                    root_conversation_id: conversation_id.clone(),
                    source_agent_id: agent_id.clone(),
                    target_department_id: target_department_id.clone(),
                    target_agent_id: target_agent_id.clone(),
                    target_agent_name: target_agent_name.clone(),
                    reason: format!("目标部门未配置可用模型，departmentId={target_department_id}"),
                });
                continue;
            }
            mention_plans.push(UserMentionPlan {
                root_conversation_id: conversation_id.clone(),
                source_department_id: department.id.clone(),
                source_agent_id: agent_id.clone(),
                target_department_id,
                target_agent_id,
                target_agent_name,
                instruction: display_text.clone(),
                background: mention_background.clone(),
                target_api_config_ids,
            });
        }

        let persist_started_at = std::time::Instant::now();
        persist_single_conversation_runtime_fast(state, &data, &conversation_id)?;
        let persist_elapsed_ms = persist_started_at.elapsed().as_millis();
        eprintln!(
            "[聊天发送] 用户@委托发送前准备耗时：总计={}ms，读取配置={}ms，读取应用数据={}ms，解析部门={}ms，定位会话={}ms，持久化={}ms，conversation_id={}，department_id={}，agent_id={}，mention_count={}",
            prepare_started_at.elapsed().as_millis(),
            config_elapsed_ms,
            app_data_elapsed_ms,
            department_elapsed_ms,
            conversation_elapsed_ms,
            persist_elapsed_ms,
            conversation_id,
            department.id,
            agent_id,
            mention_count
        );

        (
            conversation_id,
            department.id.clone(),
            agent_id,
            api_config_id,
            mention_plans,
            mention_failures,
        )
    };

    let event_id = Uuid::new_v4().to_string();
    let request_id = runtime_context_request_id_or_new(None, input.trace_id.as_deref(), "chat");
    let mut runtime_context = runtime_context_new("user_message", "user_mention_send");
    runtime_context.request_id = Some(request_id.clone());
    runtime_context.dispatch_id = Some(event_id.clone());
    runtime_context.origin_conversation_id = Some(conversation_id.clone());
    runtime_context.target_conversation_id = Some(conversation_id.clone());
    runtime_context.root_conversation_id = Some(conversation_id.clone());
    runtime_context.executor_agent_id = Some(agent_id.clone());
    runtime_context.executor_department_id = Some(department_id.clone());
    runtime_context.model_config_id = Some(model_config_id.clone());
    let event = ChatPendingEvent {
        id: event_id.clone(),
        conversation_id: conversation_id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::User,
        messages: vec![user_message],
        activate_assistant: false,
        session_info: ChatSessionInfo {
            department_id: department_id.clone(),
            agent_id: agent_id.clone(),
        },
        runtime_context: Some(runtime_context.clone()),
        sender_info: None,
    };

    let (result_tx, result_rx) = tokio::sync::oneshot::channel();
    register_chat_event_runtime(state, &event_id, on_delta.clone(), result_tx)?;

    let ingress = match ingress_chat_event(state, event) {
        Ok(value) => value,
        Err(err) => {
            let _ = state
                .pending_chat_delta_channels
                .lock()
                .map(|mut map| map.remove(&event_id));
            let _ = state
                .pending_chat_result_senders
                .lock()
                .map(|mut map| map.remove(&event_id));
            return Err(err);
        }
    };

    process_chat_event_after_ingress(state, ingress).await;

    for failure in mention_failures {
        spawn_user_mention_failure_message(state.clone(), failure);
    }
    for plan in mention_plans {
        spawn_user_mention_delegate(state.clone(), plan);
    }

    result_rx
        .await
        .map_err(|_| "聊天请求已取消或调度结果丢失".to_string())?
}

#[tauri::command]
async fn send_user_mention_message(
    input: SendChatRequest,
    state: State<'_, AppState>,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {
    send_user_mention_message_inner(input, state.inner(), &on_delta).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BindActiveChatViewStreamInput {
    #[serde(default)]
    conversation_id: Option<String>,
}

#[tauri::command]
async fn bind_active_chat_view_stream(
    input: BindActiveChatViewStreamInput,
    state: State<'_, AppState>,
    window: tauri::Window,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<(), String> {
    let window_label = window.label().to_string();
    let conversation_id = input
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(conversation_id) = conversation_id {
        set_active_chat_view_stream_binding(
            state.inner(),
            &window_label,
            Some(conversation_id),
            on_delta.clone(),
        )?;
        runtime_log_debug(format!(
            "[聊天] 已绑定活动聊天流: window={}, conversation_id={}",
            window_label, conversation_id
        ));
    } else {
        // 空会话视图仍保留绑定，作为单窗口通配接收端，避免远程消息落地后前端无推送。
        set_active_chat_view_stream_binding(
            state.inner(),
            &window_label,
            Some("*"),
            on_delta,
        )?;
    }
    Ok(())
}

#[tauri::command]
async fn stop_chat_message(
    input: StopChatRequest,
    state: State<'_, AppState>,
) -> Result<StopChatResult, String> {
    let agent_id = input.session.agent_id.trim().to_string();
    if agent_id.is_empty() {
        return Err("Missing session.agentId".to_string());
    }
    let requested_conversation_id = input
        .session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let requested_department_id = input
        .session
        .department_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let chat_key = inflight_chat_key(
        &agent_id,
        requested_conversation_id.as_deref(),
    );
    let aborted_chat = {
        let mut inflight = state
            .inflight_chat_abort_handles
            .lock()
            .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
        if let Some(handle) = inflight.remove(&chat_key) {
            handle.abort();
            true
        } else {
            false
        }
    };
    let aborted_tool = abort_inflight_tool_abort_handle(state.inner(), &chat_key)?;
    let aborted_delegate_children =
        abort_delegate_runtime_descendants_by_parent_session(state.inner(), &chat_key)?;
    let aborted = aborted_chat || aborted_tool || aborted_delegate_children > 0;
    if aborted_delegate_children > 0 {
        eprintln!(
            "[聊天] 停止请求已级联到同步委托子会话: session={}, child_count={}",
            chat_key,
            aborted_delegate_children
        );
    }

    let partial_assistant_text = input.partial_assistant_text.trim().to_string();
    let partial_reasoning_standard = input.partial_reasoning_standard.trim().to_string();
    let partial_reasoning_inline = input.partial_reasoning_inline.trim().to_string();
    let completed_tool_history = inflight_completed_tool_history(state.inner(), &chat_key)?;
    let build_stop_result =
        |persisted: bool,
         conversation_id: Option<String>,
         assistant_message: Option<ChatMessage>|
         -> StopChatResult {
            StopChatResult {
                aborted,
                persisted,
                conversation_id,
                assistant_text: partial_assistant_text.clone(),
                reasoning_standard: partial_reasoning_standard.clone(),
                reasoning_inline: partial_reasoning_inline.clone(),
                assistant_message,
            }
        };
    let should_persist = !partial_assistant_text.is_empty()
        || !partial_reasoning_standard.is_empty()
        || !partial_reasoning_inline.is_empty()
        || !completed_tool_history.is_empty();
    if !should_persist {
        clear_inflight_completed_tool_history(state.inner(), &chat_key)?;
        return Ok(build_stop_result(false, None, None));
    }

    let _guard = lock_conversation_with_metrics(&state, "stop_chat_generation_persist_partial")?;
    let app_config = state_read_config_cached(&state)?;
    let mut data = state_read_app_data_cached(&state)?;
    let api_config_id = if let Some(_conversation_id) = requested_conversation_id.as_deref() {
        requested_department_id
            .as_deref()
            .and_then(|id| department_by_id(&app_config, id))
            .map(department_primary_api_config_id)
            .or_else(|| {
                department_for_agent_id(&app_config, &agent_id).map(department_primary_api_config_id)
            })
            .or_else(|| resolve_selected_api_config(&app_config, None).map(|api| api.id.clone()))
            .ok_or_else(|| "Missing available API config for stop request".to_string())?
    } else {
        requested_department_id
            .as_deref()
            .and_then(|id| department_by_id(&app_config, id))
            .map(department_primary_api_config_id)
            .or_else(|| {
                department_for_agent_id(&app_config, &agent_id).map(department_primary_api_config_id)
            })
            .or_else(|| resolve_selected_api_config(&app_config, None).map(|api| api.id.clone()))
            .ok_or_else(|| "Missing available API config for stop request".to_string())?
    };
    if !app_config.api_configs.iter().any(|api| api.id == api_config_id) {
        return Err(format!("Selected API config '{api_config_id}' not found."));
    }
    let runtime_requested = requested_conversation_id
        .as_deref()
        .filter(|conversation_id| {
            delegate_runtime_thread_conversation_get(state.inner(), conversation_id)
                .ok()
                .flatten()
                .is_some()
        })
        .map(ToOwned::to_owned);
    let mut runtime_conversation = if let Some(conversation_id) = runtime_requested.as_deref() {
        delegate_runtime_thread_conversation_get(state.inner(), conversation_id)?
    } else {
        None
    };
    let idx = if runtime_conversation.is_some() {
        None
    } else {
        latest_active_conversation_index(&data, "", &agent_id)
    };
    let conversation = if let Some(conversation) = runtime_conversation.as_mut() {
        conversation
    } else {
        let Some(idx) = idx else {
            return Ok(build_stop_result(false, None, None));
        };
        data.conversations
            .get_mut(idx)
            .ok_or_else(|| "Active conversation index is out of bounds.".to_string())?
    };

    // If the latest message is already an assistant message, do not append duplicate partial output.
    if conversation
        .messages
        .last()
        .map(|m| m.role == "assistant")
        .unwrap_or(false)
    {
        let conversation_id = conversation.id.clone();
        let assistant_message = conversation.messages.last().cloned();
        clear_inflight_completed_tool_history(state.inner(), &chat_key)?;
        return Ok(build_stop_result(false, Some(conversation_id), assistant_message));
    }

    let provider_meta = if partial_reasoning_standard.is_empty() && partial_reasoning_inline.is_empty()
    {
        None
    } else {
        Some(serde_json::json!({
            "reasoningStandard": partial_reasoning_standard,
            "reasoningInline": partial_reasoning_inline
        }))
    };

    let now = now_iso();
    let assistant_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now.clone(),
        speaker_agent_id: Some(agent_id.clone()),
        parts: vec![MessagePart::Text {
            text: partial_assistant_text.clone(),
        }],
        extra_text_blocks: Vec::new(),
        provider_meta,
        tool_call: if completed_tool_history.is_empty() {
            None
        } else {
            Some(completed_tool_history)
        },
        mcp_call: None,
    };
    conversation.messages.push(assistant_message.clone());
    conversation.updated_at = now.clone();
    conversation.last_assistant_at = Some(now);
    let conversation_id = conversation.id.clone();

    if let Some(conversation) = runtime_conversation {
        delegate_runtime_thread_conversation_update(state.inner(), &conversation_id, conversation)?;
    } else {
        let persisted_conversation = data
            .conversations
            .iter()
            .find(|item| item.id == conversation_id)
            .cloned()
            .ok_or_else(|| "活动会话不存在，无法保存中断回复。".to_string())?;
        state_write_conversation_with_chat_index_cached(state.inner(), &persisted_conversation)?;
    }
    clear_inflight_completed_tool_history(state.inner(), &chat_key)?;

    Ok(build_stop_result(true, Some(conversation_id), Some(assistant_message)))
}

#[tauri::command]
async fn get_chat_queue_snapshot(
    state: State<'_, AppState>,
) -> Result<Vec<ChatQueueEventSummary>, String> {
    get_queue_snapshot(state.inner())
}

#[tauri::command]
async fn remove_chat_queue_event(
    event_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let removed = remove_from_queue(state.inner(), &event_id)?;
    Ok(removed.is_some())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InterruptConversationRuntimeResult {
    aborted: bool,
    cleared_queue_count: usize,
}

#[tauri::command]
async fn interrupt_conversation_runtime(
    session: SessionSelector,
    state: State<'_, AppState>,
) -> Result<InterruptConversationRuntimeResult, String> {
    let agent_id = session.agent_id.trim().to_string();
    if agent_id.is_empty() {
        return Err("Missing session.agentId".to_string());
    }
    let conversation_id = session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "Missing session.conversationId".to_string())?;

    let chat_key = inflight_chat_key(&agent_id, Some(&conversation_id));
    let aborted_chat = {
        let mut inflight = state
            .inflight_chat_abort_handles
            .lock()
            .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
        if let Some(handle) = inflight.remove(&chat_key) {
            handle.abort();
            true
        } else {
            false
        }
    };
    let aborted_tool = abort_inflight_tool_abort_handle(state.inner(), &chat_key)?;
    let aborted_delegate_children =
        abort_delegate_runtime_descendants_by_parent_session(state.inner(), &chat_key)?;
    let cleared_queue_count = clear_conversation_queue(
        state.inner(),
        &conversation_id,
        "消息已因会话撤回被清出队列",
    )?;
    let _ = release_conversation_processing_claim(state.inner(), &conversation_id);
    let _ = set_conversation_runtime_state(state.inner(), &conversation_id, MainSessionState::Idle);
    let _ = set_conversation_remote_im_activation_sources(state.inner(), &conversation_id, Vec::new());

    let aborted = aborted_chat || aborted_tool || aborted_delegate_children > 0;
    eprintln!(
        "[聊天调度] 会话运行已中断: conversation_id={}, aborted={}, cleared_queue_count={}, child_abort_count={}",
        conversation_id,
        aborted,
        cleared_queue_count,
        aborted_delegate_children
    );
    Ok(InterruptConversationRuntimeResult {
        aborted,
        cleared_queue_count,
    })
}

#[tauri::command]
async fn get_main_session_state_snapshot(
    state: State<'_, AppState>,
) -> Result<MainSessionState, String> {
    get_main_session_state(state.inner())
}
