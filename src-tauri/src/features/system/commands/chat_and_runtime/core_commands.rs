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
        if agent_id.is_empty() {
            continue;
        }
        if !seen.insert(agent_id.to_string()) {
            continue;
        }
        out.push(serde_json::json!({
            "agentId": agent_id,
            "agentName": item.agent_name.as_deref().map(str::trim).filter(|value| !value.is_empty()).unwrap_or(agent_id),
        }));
    }
    out
}

fn build_user_message_provider_meta(
    input_provider_meta: Option<Value>,
    attachments: &[serde_json::Value],
    mentions: &[serde_json::Value],
    request_id: Option<&str>,
) -> Option<Value> {
    let _ = attachments;
    let merged = provider_meta_without_legacy_attachments(input_provider_meta);
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
    if let Some(request_id) = request_id.map(str::trim).filter(|value| !value.is_empty()) {
        root.insert("requestId".to_string(), Value::String(request_id.to_string()));
    }
    Some(Value::Object(root))
}

fn attachment_display_name(input: &AttachmentMetaInput) -> Option<String> {
    let file_name = input.file_name.trim();
    if !file_name.is_empty() {
        return Some(file_name.to_string());
    }
    let relative_path = input.path.trim();
    if relative_path.is_empty() {
        return None;
    }
    std::path::Path::new(relative_path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn build_attachment_only_display_text(
    raw_display_text: Option<&str>,
    raw_text: Option<&str>,
    images: &[BinaryPart],
    attachments: &[AttachmentMetaInput],
) -> String {
    let preferred = raw_display_text
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| raw_text.map(str::trim).filter(|value| !value.is_empty()));
    if let Some(text) = preferred {
        return text.to_string();
    }

    let image_count = images.len();
    let attachment_names = attachments
        .iter()
        .filter_map(attachment_display_name)
        .fold(Vec::<String>::new(), |mut names, name| {
            if !names.iter().any(|item| item == &name) {
                names.push(name);
            }
            names
        });
    let attachment_count = attachments.len();
    let mut parts = Vec::<String>::new();
    if image_count > 0 {
        parts.push(format!("用户发送了{}张图片", image_count));
    }
    if attachment_count > 0 {
        let suffix = if attachment_names.is_empty() {
            String::new()
        } else {
            format!(
                "：{}",
                attachment_names
                    .iter()
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("、")
            )
        };
        parts.push(format!("用户发送了{}个附件{}", attachment_count, suffix));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("{}。请基于这些内容处理。", parts.join("，"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmPlanAndContinueInput {
    conversation_id: String,
    plan_message_id: String,
    #[serde(default)]
    agent_id: Option<String>,
}

fn plan_path_from_message_provider_meta(message: &ChatMessage) -> Option<String> {
    message
        .provider_meta
        .as_ref()
        .and_then(|meta| meta.get("planCard"))
        .and_then(Value::as_object)
        .and_then(|card| card.get("path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn resolve_plan_file_for_conversation_id(
    state: &AppState,
    conversation_id: &str,
    raw_path: &str,
) -> Result<ResolvedPlanFilePath, String> {
    let conversation = conversation_service_v2().get_conversation_metadata_record(state, conversation_id)?;
    let base_root = terminal_default_workspace_for_conversation_resolved(state, Some(&conversation))
        .map(|workspace| workspace.path)
        .or_else(|_| plan_assistant_space_canonical(state))?;
    resolve_plan_file_for_conversation(&base_root, raw_path)
}

fn plan_continue_prompt_block(plan_path: &str) -> String {
    format!(
        "<active_plans>\n以下为用户刚刚同意执行的计划文件。请读取该文件并开始执行；完成后调用 plan(action=complete) 并传入对应 path。\n<active_plan index=\"1\">\n{}\n</active_plan>\n</active_plans>",
        plan_path.trim()
    )
}

fn plan_continue_confirmation_message(plan_path: &str) -> ChatMessage {
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: vec![MessagePart::Text {
            text: "我同意，请执行。".to_string(),
                reasoning_content: None,
            }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "messageKind": "plan_confirm_continue",
            "message_meta": {
                "kind": "plan_confirm_continue"
            },
            "oneShotPromptExtraBlocks": [plan_continue_prompt_block(plan_path)]
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

fn resolve_runtime_control_agent_id(
    state: &AppState,
    requested_agent_id: Option<&str>,
    requested_conversation_id: Option<&str>,
) -> Result<String, String> {
    let runtime_org = load_runtime_organization_snapshot(state)?;
    let bound_agent_id = requested_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|conversation_id| {
            conversation_service_v2()
                .get_conversation_meta(state, conversation_id)
                .ok()
                .map(|conversation_meta| {
                    conversation_meta.agent_id.trim().to_string()
                })
        });
    let agent_id = match bound_agent_id {
        Some(agent_id) if !agent_id.is_empty() => agent_id,
        _ => requested_agent_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| "Missing session.agentId".to_string())?,
    };
    if available_non_user_agent(&runtime_org.agents, &agent_id).is_none() {
        return Err(format!("人格已经消失或不可用：{agent_id}"));
    }
    Ok(agent_id)
}

fn plan_confirm_context_usage_ratio(source: &Conversation, selected_api: &ApiConfig) -> f64 {
    conversation_prompt_service()
        .latest_real_prompt_usage(source, selected_api)
        .map(|usage| usage.usage_ratio.max(0.0))
        .unwrap_or(0.0)
}

async fn confirm_plan_and_continue_inner(
    state: &AppState,
    input: &ConfirmPlanAndContinueInput,
) -> Result<bool, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let plan_message_id = input.plan_message_id.trim();
    if plan_message_id.is_empty() {
        return Err("planMessageId is required.".to_string());
    }
    let plan_message = conversation_service_v2().get_message_by_id_for_frontend_display_only(
        state,
        conversation_id,
        plan_message_id,
    )?;
    let plan_path = plan_path_from_message_provider_meta(&plan_message)
        .ok_or_else(|| "指定消息不是可执行计划。".to_string())?;
    let resolved_plan_path =
        resolve_plan_file_for_conversation_id(state, conversation_id, &plan_path)?;
    message_store::active_plan_append_in_progress(
        &state.data_path,
        conversation_id,
        plan_message_id,
        &resolved_plan_path.display_path,
    )?;
    let conversation_meta = conversation_service_v2().get_conversation_meta(state, conversation_id)?;
    let requested_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            let agent_id = conversation_meta.agent_id.trim();
            (!agent_id.is_empty()).then(|| agent_id.to_string())
        });
    let (selected_api, resolved_api, agent_id) = {
        let runtime_org = load_runtime_organization_snapshot(state)?;
        let app_config = &runtime_org.config;
        let agent_id = requested_agent_id
            .as_deref()
            .ok_or_else(|| "缺少可用于继续执行计划的人格。".to_string())?
            .to_string();
        let agent = runtime_agent_by_id(&runtime_org, &agent_id)
            .filter(|agent| !agent.is_built_in_user)
            .ok_or_else(|| format!("计划继续执行的人格不存在或不可用: agent_id={agent_id}"))?;
        let api_config_id = agent_primary_chat_api_config_id(app_config, agent)
            .ok_or_else(|| format!("人格模型未配置或不可用于聊天: {}", agent.id))?;
        let selected_api = app_config
            .api_configs
            .iter()
            .find(|api| api.id == api_config_id)
            .cloned()
            .ok_or_else(|| format!("模型配置不存在: {api_config_id}"))?;
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        (
            selected_api,
            resolved_api,
            agent_id,
        )
    };
    let conversation = conversation_service_v2().get_conversation_prompt_context(state, conversation_id)?;
    let continue_event_id = format!("confirm-plan-continue-{}", Uuid::new_v4());
    let preview = build_trim_compaction_preview_result(state, &selected_api, &conversation)?;
    let should_compact_before_continue =
        preview.can_compact && plan_confirm_context_usage_ratio(&conversation, &selected_api) >= 0.60;
    if should_compact_before_continue {
        let compaction_result = run_context_compaction_pipeline(
            state,
            &selected_api,
            &resolved_api,
            &conversation,
            &agent_id,
            "confirm_plan_before_continue",
            "COMPACTION-CONFIRM-PLAN",
            &[],
            false,
        )
        .await;
        match compaction_result {
            Ok(result) => {
                runtime_log_info(format!(
                    "[上下文整理] 计划确认前压缩完成 conversation_id={} merged_memories={} warning={}",
                    conversation_id,
                    result.merged_memories,
                    result.warning.as_deref().unwrap_or("")
                ));
            }
            Err(err) => {
                return Err(err);
            }
        }
    }
    let mut runtime_context = runtime_context_new("plan_confirm", "context_compaction_followup");
    runtime_context.request_id = Some(continue_event_id.clone());
    runtime_context.dispatch_id = Some(continue_event_id.clone());
    runtime_context.origin_conversation_id = Some(conversation_id.to_string());
    runtime_context.target_conversation_id = Some(conversation_id.to_string());
    runtime_context.root_conversation_id = Some(conversation_id.to_string());
    runtime_context.executor_agent_id = Some(agent_id.clone());
    runtime_context.model_config_id = Some(selected_api.id.clone());
    let assistant_message_id = Uuid::new_v4().to_string();
    let event = ChatPendingEvent {
        id: continue_event_id,
        conversation_id: conversation_id.to_string(),
        created_at: now_iso(),
        source: ChatEventSource::System,
        queue_mode: ChatQueueMode::Normal,
        messages: vec![plan_continue_confirmation_message(&resolved_plan_path.display_path)],
        activate_assistant: true,
        assistant_message_id: Some(assistant_message_id),
        session_info: ChatSessionInfo {
                        agent_id,
        },
        runtime_context: Some(runtime_context),
        sender_info: None,
    };
    match ingress_chat_event(state, event)? {
        ChatEventIngress::Direct(event) => {
            trigger_chat_event_after_ingress(state, ChatEventIngress::Direct(event));
        }
        ChatEventIngress::Queued { event_id } => {
            runtime_log_info(format!(
                "[计划] 确认后继续执行已入队 conversation_id={} event_id={}",
                conversation_id, event_id
            ));
        }
    }
    trigger_chat_queue_processing(state);
    Ok(true)
}

#[tauri::command]
async fn confirm_plan_and_continue(
    input: ConfirmPlanAndContinueInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    confirm_plan_and_continue_inner(state.inner(), &input).await
}

#[tauri::command]
fn read_plan_file_content(
    conversation_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    read_plan_file_content_inner(&conversation_id, &path, state.inner())
}

fn read_plan_file_content_inner(
    conversation_id: &str,
    path: &str,
    state: &AppState,
) -> Result<String, String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let resolved = resolve_plan_file_for_conversation_id(
        state,
        normalized_conversation_id,
        path.trim(),
    )?;
    read_plan_markdown_file(&resolved.canonical_path)
}

#[derive(Debug, Clone)]
struct UserMentionPlan {
    root_conversation_id: String,
    source_agent_id: String,
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
    target_agent_id: String,
    target_agent_name: String,
    reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitUserAsyncDelegateInput {
    conversation_id: String,
    #[serde(default)]
    target_agent_id: Option<String>,
    #[serde(default)]
    preset_id: Option<String>,
    #[serde(default)]
    why: Option<String>,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    todo: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    question: Option<String>,
    #[serde(default)]
    focus: Option<String>,
    #[serde(default)]
    selected_message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SubmitUserAsyncDelegateOutput {
    delegate_id: String,
    conversation_id: String,
    target_agent_id: String,
    target_agent_name: String,
    selected_message_count: usize,
}

#[derive(Debug, Clone)]
struct UserAsyncDelegatePlan {
    root_conversation_id: String,
    source_agent_id: String,
    target_agent_id: String,
    target_agent_name: String,
    title: String,
    goal: String,
    why: String,
    todo: String,
    target_api_config_ids: Vec<String>,
}

fn build_user_mention_context_snapshot_from_messages(
    messages: &[ChatMessage],
    agents: &[AgentProfile],
    latest_user_text: &str,
) -> String {
    let mut lines = Vec::<String>::new();
    let recent_messages = messages
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
                    agents
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

fn read_user_mention_context_snapshot(
    state: &AppState,
    conversation_meta: &ConversationMetaView,
    agents: &[AgentProfile],
    latest_user_text: &str,
) -> Result<String, String> {
    let paths = message_store::message_store_paths(&state.data_path, &conversation_meta.id)?;
    if let Some(page) =
        message_store::chat_store_read_recent_messages_page_cached(&paths, 12)?
    {
        return Ok(build_user_mention_context_snapshot_from_messages(
            &page.messages,
            agents,
            latest_user_text,
        ));
    }
    let last_block = conversation_service_v2().get_conversation_last_block(state, &conversation_meta.id)?;
    Ok(build_user_mention_context_snapshot_from_messages(
        &last_block.messages,
        agents,
        latest_user_text,
    ))
}

fn build_user_mention_dispatch_plans(
    app_config: &AppConfig,
    root_conversation_id: &str,
    mention_background: &str,
    agents: &[AgentProfile],
    source_agent_id: &str,
    latest_user_text: &str,
    mentions: Option<&Vec<UserMentionTargetInput>>,
) -> Result<(Vec<UserMentionPlan>, Vec<UserMentionFailurePlan>), String> {
    let Some(items) = mentions.filter(|items| !items.is_empty()) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let mut mention_plans = Vec::<UserMentionPlan>::new();
    let mut mention_failures = Vec::<UserMentionFailurePlan>::new();
    let mut seen_mentions = std::collections::HashSet::<String>::new();
    for mention in items.iter().take(3) {
        let target_agent_id = mention.agent_id.trim().to_string();
        if target_agent_id.is_empty() {
            continue;
        }
        if !seen_mentions.insert(target_agent_id.clone()) {
            continue;
        }
        let Some(target_agent) = agents
            .iter()
            .find(|agent| agent.id == target_agent_id && !agent.is_built_in_user)
        else {
            mention_failures.push(UserMentionFailurePlan {
                root_conversation_id: root_conversation_id.to_string(),
                source_agent_id: source_agent_id.to_string(),
                target_agent_id: target_agent_id.clone(),
                target_agent_name: target_agent_id.clone(),
                reason: format!("目标人格不存在或不可用，agentId={target_agent_id}"),
            });
            continue;
        };
        let target_agent_name = if target_agent.name.trim().is_empty() {
            target_agent_id.clone()
        } else {
            target_agent.name.trim().to_string()
        };
        if target_agent_id == source_agent_id {
            mention_failures.push(UserMentionFailurePlan {
                root_conversation_id: root_conversation_id.to_string(),
                source_agent_id: source_agent_id.to_string(),
                target_agent_id: target_agent_id.clone(),
                target_agent_name: target_agent_name.clone(),
                reason: SAME_PERSONA_BACKGROUND_DELEGATE_REASON.to_string(),
            });
            continue;
        }
        let target_api_config_ids = delegate_target_chat_api_config_ids(app_config, target_agent);
        if target_api_config_ids.is_empty() {
            mention_failures.push(UserMentionFailurePlan {
                root_conversation_id: root_conversation_id.to_string(),
                source_agent_id: source_agent_id.to_string(),
                target_agent_id: target_agent_id.clone(),
                target_agent_name: target_agent_name.clone(),
                reason: format!("目标人格未配置可用模型，agentId={target_agent_id}"),
            });
            continue;
        }
        mention_plans.push(UserMentionPlan {
            root_conversation_id: root_conversation_id.to_string(),
            source_agent_id: source_agent_id.to_string(),
            target_agent_id,
            target_agent_name,
            instruction: latest_user_text.to_string(),
            background: mention_background.to_string(),
            target_api_config_ids,
        });
    }
    Ok((mention_plans, mention_failures))
}

fn user_async_delegate_message_text(message: &ChatMessage) -> String {
    if message.role.trim() != "user" && message.role.trim() != "assistant" {
        return String::new();
    }
    let mut chunks = Vec::<String>::new();
    for part in &message.parts {
        if let MessagePart::Text { text, .. } = part {
            let text = text.trim();
            if !text.is_empty() {
                chunks.push(text.to_string());
            }
        }
    }
    chunks.join("\n").trim().to_string()
}

fn user_async_delegate_speaker_name(message: &ChatMessage, agents: &[AgentProfile]) -> String {
    if message.role.trim() == "user" {
        return "用户".to_string();
    }
    let speaker_agent_id = message
        .speaker_agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    agents
        .iter()
        .find(|agent| agent.id == speaker_agent_id)
        .map(|agent| agent.name.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "助理".to_string())
}

fn user_async_delegate_truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let head = text.chars().take(max_chars).collect::<String>();
    format!("{head}\n[内容过长，已截断]")
}

fn build_user_async_delegate_selected_context(
    conversation: &Conversation,
    agents: &[AgentProfile],
    selected_message_ids: &[String],
) -> (String, usize) {
    let (selected_messages, _) =
        collect_selected_messages_for_branch(conversation, selected_message_ids);
    let mut lines = Vec::<String>::new();
    let mut count = 0usize;
    for message in selected_messages {
        let text = user_async_delegate_message_text(&message);
        if text.trim().is_empty() {
            continue;
        }
        count += 1;
        let speaker = user_async_delegate_speaker_name(&message, agents);
        lines.push(format!(
            "[{}] {}",
            speaker,
            user_async_delegate_truncate_chars(text.trim(), 4000)
        ));
    }
    let joined = lines.join("\n\n");
    (user_async_delegate_truncate_chars(joined.trim(), 30000), count)
}

fn normalize_user_async_delegate_why(raw_why: &str) -> String {
    let trimmed = raw_why.trim();
    if trimmed.eq_ignore_ascii_case("请使用review skill")
        || trimmed.eq_ignore_ascii_case("请使用 review skill")
    {
        return String::new();
    }
    trimmed.to_string()
}

fn build_user_async_delegate_why(
    raw_why: &str,
    selected_context: &str,
) -> String {
    let mut parts = Vec::<String>::new();
    let raw_why = normalize_user_async_delegate_why(raw_why);
    if !raw_why.is_empty() {
        parts.push(format!("用户补充背景：\n{raw_why}"));
    }
    let selected_context = selected_context.trim();
    if !selected_context.is_empty() {
        parts.push(format!("当前会话选中消息纯文本：\n{selected_context}"));
    }
    parts.join("\n\n")
}

fn user_async_delegate_title(goal: &str, preset_id: Option<&str>) -> String {
    let prefix = match preset_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some("review") => "审查委托",
        Some(_) => "异步委托",
        None => "异步委托",
    };
    let first_line = goal
        .trim()
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    let suffix = first_line.chars().take(24).collect::<String>();
    if suffix.trim().is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}：{suffix}")
    }
}

fn resolve_user_async_delegate_plan(
    app_state: &AppState,
    input: &SubmitUserAsyncDelegateInput,
) -> Result<(UserAsyncDelegatePlan, usize), String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required".to_string());
    }
    let target_agent_id = input
        .target_agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "targetAgentId is required".to_string())?;
    let goal = delegate_arg_new_or_legacy(&input.goal, &input.question);
    if goal.trim().is_empty() {
        return Err("goal is required".to_string());
    }
    let todo = delegate_arg_new_or_legacy(&input.todo, &input.focus);
    let raw_why = delegate_arg_new_or_legacy(&input.why, &input.background);
    let selected_message_ids = input
        .selected_message_ids
        .iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();

    let runtime_org = load_runtime_organization_snapshot(app_state)?;
    let app_config = &runtime_org.config;
    let agents = &runtime_org.agents;
    let conversation_meta = conversation_service_v2()
        .get_conversation_meta(app_state, conversation_id)
        .ok()
        .filter(|conversation_meta| {
            conversation_meta.status.trim() != "archived"
                && conversation_meta
                    .archived_at
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none()
                && conversation_meta.visible_in_foreground_lists
                && !conversation_meta.is_delegate
        })
        .ok_or_else(|| "当前会话不存在或已归档".to_string())?;
    let conversation_agent_id = conversation_meta.agent_id.trim();
    let source_agent_id = if conversation_agent_id.is_empty() {
        DEFAULT_AGENT_ID.to_string()
    } else {
        if available_non_user_agent(agents, conversation_agent_id).is_none() {
            return Err(format!(
                "当前会话绑定人格不存在或不可用，agentId={conversation_agent_id}"
            ));
        }
        conversation_agent_id.to_string()
    };
    if target_agent_id == source_agent_id {
        return Err(SAME_PERSONA_BACKGROUND_DELEGATE_REASON.to_string());
    }
    let conversation = conversation_service_v2()
        .get_conversation_metadata_record(app_state, conversation_id)?;
    let target_agent = agents
        .iter()
        .find(|agent| agent.id == target_agent_id && !agent.is_built_in_user)
        .ok_or_else(|| format!("目标委任人不存在，agentId={target_agent_id}"))?;
    let target_api_config_ids = delegate_target_chat_api_config_ids(app_config, target_agent);
    if target_api_config_ids.is_empty() {
        return Err(format!("目标人格没有可用模型，agentId={target_agent_id}"));
    }

    let (selected_context, selected_count) =
        build_user_async_delegate_selected_context(&conversation, agents, &selected_message_ids);
    let why = build_user_async_delegate_why(&raw_why, &selected_context);
    let title = user_async_delegate_title(&goal, input.preset_id.as_deref());
    Ok((
        UserAsyncDelegatePlan {
            root_conversation_id: conversation_meta.id.to_string(),
            source_agent_id,
            target_agent_id: target_agent_id.to_string(),
            target_agent_name: target_agent.name.trim().to_string(),
            title,
            goal,
            why,
            todo,
            target_api_config_ids,
        },
        selected_count,
    ))
}

async fn enqueue_user_mention_result_message(
    app_state: &AppState,
    root_conversation_id: &str,
    target_agent_id: &str,
    text: &str,
    provider_meta: Value,
) -> Result<(), String> {
    let mut delegate_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(target_agent_id.to_string()),
        parts: vec![MessagePart::Text {
            text: text.to_string(),
                reasoning_content: None,
            }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(provider_meta),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    };
    let delegate_message_seed = delegate_message.id.clone();
    delegate_message.meme_annotations = populate_assistant_meme_annotations(
        app_state,
        &delegate_message_seed,
        text,
    )?;
    append_delegate_result_message_and_emit(
        app_state,
        root_conversation_id,
        &delegate_message,
        false,
        None,
    )
    .await
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
            runtime_log_error(format!(
                "[聊天推送] append 消息事件发送失败：锁已损坏，conversation_id={}, error={:?}",
                conversation_id, err
            ));
            None
        }
    };
    let Some(app_handle) = app_handle else {
        runtime_log_error(format!(
            "[聊天推送] append 消息事件发送失败：app_handle 不可用，conversation_id={}",
            conversation_id
        ));
        return;
    };
    let payload = ConversationMessageAppendedPayload {
        conversation_id: conversation_id.to_string(),
        message: project_message_for_frontend_display_only(message.clone()),
    };
    ide_chat_broadcast_notification("conversation.messageAppended", serde_json::json!(&payload));
    if let Err(err) = app_handle.emit(CHAT_CONVERSATION_MESSAGE_APPENDED_EVENT, payload) {
        runtime_log_error(format!(
            "[聊天推送] append 消息事件发送失败：conversation_id={}, message_id={}, error={}",
            conversation_id,
            message.id,
            err
        ));
    }
}

async fn append_delegate_result_message_and_emit(
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
    conversation_service_v2()
        .append_message(app_state, conversation_id, message)
        .await?;
    emit_conversation_message_appended_event(app_state, conversation_id, message);

    if continue_main_assistant {
        let session_info = session_info.ok_or_else(|| "缺少 session_info，无法继续主助理".to_string())?;
        tauri::async_runtime::spawn({
            let state = app_state.clone();
            let conversation_id = conversation_id.to_string();
            async move {
                let oldest_queue_created_at = now_iso();
                let mut runtime_context = runtime_context_new("delegate_result", "delegate_continue");
                runtime_context.request_id = Some(format!("delegate-continue-{}", Uuid::new_v4()));
                if let Err(err) = activate_main_assistant(
                    &state,
                    &session_info,
                    &conversation_id,
                    None,
                    None,
                    None,
                    Some(runtime_context),
                    Vec::new(),
                    &oldest_queue_created_at,
                )
                .await
                {
                    runtime_log_error(format!(
                        "[委托结果] 追加后继续主助理失败: conversation_id={}, agent_id={}, error={}",
                        conversation_id,
                        session_info.agent_id,
                        err
                    ));
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
        )
        .await
        {
            runtime_log_error(format!(
                "[用户@委托] 写回失败结果消息失败: conversation_id={}, target_agent_id={}, error={}",
                failure.root_conversation_id,
                failure.target_agent_id,
                err
            ));
        }
    });
}

fn spawn_user_async_delegate(app_state: AppState, plan: UserAsyncDelegatePlan) -> Result<String, String> {
    let delegate = delegate_create_record(
        &app_state,
        DELEGATE_TOOL_KIND_USER_MENTION,
        &plan.root_conversation_id,
        None,
        &plan.source_agent_id,
        &plan.target_agent_id,
        &plan.title,
        plan.why.clone(),
        plan.goal.clone(),
        plan.todo.clone(),
        false,
        vec![plan.source_agent_id.clone(), plan.target_agent_id.clone()],
    )?;
    let delegate_id = delegate.delegate_id.clone();
    let parent_chat_session_key = Some(inflight_chat_key(
        &plan.source_agent_id,
        Some(&plan.root_conversation_id),
    ));
    tokio::spawn(async move {
        let target_agent_name = plan.target_agent_name.clone();
        let run_result = delegate_run_thread_to_completion(
            app_state.clone(),
            delegate.clone(),
            plan.target_api_config_ids.clone(),
            parent_chat_session_key,
        )
        .await;
        match run_result {
            Ok(run) => {
                let text = if run.assistant_text.trim().is_empty() {
                    format!("《{}》已处理完成。", delegate.title.trim())
                } else {
                    run.assistant_text.clone()
                };
                if let Err(err) = conversation_service_v2().enqueue_delegate_completion_notification(
                    &app_state,
                    &plan.root_conversation_id,
                    &plan.target_agent_id,
                    &delegate.title,
                    &text,
                    "user_async_delegate_completion",
                ) {
                    runtime_log_error(format!(
                        "[用户异步委托] 投递完成系统通知失败: conversation_id={}, target_agent_id={}, error={}",
                        plan.root_conversation_id,
                        plan.target_agent_id,
                        err
                    ));
                }
                if let Err(err) = enqueue_user_mention_result_message(
                    &app_state,
                    &plan.root_conversation_id,
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
                    }),
                )
                .await
                {
                    runtime_log_error(format!(
                        "[用户异步委托] 写回完成结果消息失败: conversation_id={}, target_agent_id={}, error={}",
                        plan.root_conversation_id,
                        plan.target_agent_id,
                        err
                    ));
                }
            }
            Err(err) => {
                let text = format!("《{}》执行失败：{}", delegate.title.trim(), err);
                if let Err(enqueue_err) = enqueue_user_mention_result_message(
                    &app_state,
                    &plan.root_conversation_id,
                    &plan.target_agent_id,
                    &text,
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
                )
                .await
                {
                    runtime_log_error(format!(
                        "[用户异步委托] 写回失败结果消息失败: conversation_id={}, target_agent_id={}, error={}",
                        plan.root_conversation_id,
                        plan.target_agent_id,
                        enqueue_err
                    ));
                }
            }
        }
        runtime_log_info(format!(
            "[用户异步委托] 后台执行 完成 delegate_id={} target_agent_name={}",
            delegate.delegate_id,
            target_agent_name
        ));
    });
    Ok(delegate_id)
}

fn spawn_user_mention_delegate(app_state: AppState, plan: UserMentionPlan) {
    tokio::spawn(async move {
        let delegate = match delegate_create_record(
            &app_state,
            DELEGATE_TOOL_KIND_USER_MENTION,
            &plan.root_conversation_id,
            None,
            &plan.source_agent_id,
            &plan.target_agent_id,
            &format!("用户@委托：{}", plan.target_agent_name.trim()),
            plan.background.clone(),
            plan.instruction.clone(),
            "请直接基于当前上下文作答，不要复述委托框架。".to_string(),
            false,
            vec![plan.source_agent_id.clone(), plan.target_agent_id.clone()],
        ) {
            Ok(value) => value,
            Err(err) => {
                spawn_user_mention_failure_message(
                    app_state,
                    UserMentionFailurePlan {
                        root_conversation_id: plan.root_conversation_id,
                        source_agent_id: plan.source_agent_id,
                        target_agent_id: plan.target_agent_id,
                        target_agent_name: plan.target_agent_name,
                        reason: err,
                    },
                );
                return;
            }
        };
        let target_agent_name = plan.target_agent_name.clone();
        let parent_chat_session_key = Some(inflight_chat_key(
            &plan.source_agent_id,
            Some(&plan.root_conversation_id),
        ));
        let run_result = delegate_run_thread_to_completion(
            app_state.clone(),
            delegate.clone(),
            plan.target_api_config_ids.clone(),
            parent_chat_session_key,
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
                    }),
                )
                .await
                {
                    runtime_log_error(format!(
                        "[用户@委托] 写回完成结果消息失败: conversation_id={}, target_agent_id={}, error={}",
                        plan.root_conversation_id,
                        plan.target_agent_id,
                        err
                    ));
                }
            }
            Err(err) => {
                let fail_text = format!("《用户@委托：{}》执行失败：{}", target_agent_name.trim(), err.trim());
                if let Err(enqueue_err) = enqueue_user_mention_result_message(
                    &app_state,
                    &plan.root_conversation_id,
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
                )
                .await
                {
                    runtime_log_error(format!(
                        "[用户@委托] 写回失败结果消息失败: conversation_id={}, target_agent_id={}, error={}",
                        plan.root_conversation_id,
                        plan.target_agent_id,
                        enqueue_err
                    ));
                }
            }
        }
    });
}

fn spawn_user_mention_after_message_flushed(
    app_state: AppState,
    event_id: String,
    result_rx: tokio::sync::oneshot::Receiver<Result<SendChatResult, String>>,
    mention_failures: Vec<UserMentionFailurePlan>,
    mention_plans: Vec<UserMentionPlan>,
) {
    tokio::spawn(async move {
        match result_rx.await {
            Ok(Ok(_)) => {
                for failure in mention_failures {
                    spawn_user_mention_failure_message(app_state.clone(), failure);
                }
                for plan in mention_plans {
                    spawn_user_mention_delegate(app_state.clone(), plan);
                }
            }
            Ok(Err(err)) => {
                runtime_log_warn(format!(
                    "[用户@委托] 用户消息落库前调度失败，跳过委托: event_id={}, error={}",
                    event_id, err
                ));
            }
            Err(_) => {
                runtime_log_warn(format!(
                    "[用户@委托] 用户消息落库结果丢失，跳过委托: event_id={}",
                    event_id
                ));
            }
        }
    });
}

const ACCEPTED_SUBMIT_TRACE_ID_LIMIT: usize = 5000;

fn normalize_send_extra_text_blocks(payload: &ChatInputPayload) -> Vec<String> {
    payload
        .extra_text_blocks
        .as_ref()
        .map(|items| {
            items
                .iter()
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn claim_submit_trace_id(state: &AppState, trace_id: &str) -> Result<bool, String> {
    let trace_id = trace_id.trim();
    if trace_id.is_empty() {
        return Ok(true);
    }
    let mut accepted = state
        .accepted_submit_trace_ids
        .lock()
        .map_err(|_| "Failed to lock accepted submit trace ids".to_string())?;
    if accepted.iter().any(|value| value == trace_id) {
        return Ok(false);
    }
    accepted.push_back(trace_id.to_string());
    while accepted.len() > ACCEPTED_SUBMIT_TRACE_ID_LIMIT {
        accepted.pop_front();
    }
    Ok(true)
}

fn release_submit_trace_id(state: &AppState, trace_id: &str) {
    let trace_id = trace_id.trim();
    if trace_id.is_empty() {
        return;
    }
    if let Ok(mut accepted) = state.accepted_submit_trace_ids.lock() {
        if let Some(index) = accepted.iter().position(|value| value == trace_id) {
            accepted.remove(index);
        }
    }
}

async fn submit_chat_message_inner(
    input: SendChatRequest,
    state: &AppState,
    on_delta: Option<tauri::ipc::Channel<AssistantDeltaEvent>>,
) -> Result<SubmitChatResult, String> {
    if input.trigger_only {
        return Err("submit_chat_message 不支持 trigger_only".to_string());
    }

    let images = input.payload.images.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
    let attachments = input
        .payload
        .attachments
        .as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);
    let extra_text_blocks = normalize_send_extra_text_blocks(&input.payload);

    if !chat_input_payload_has_content(&input.payload) && extra_text_blocks.is_empty() {
        return Err("消息内容为空".to_string());
    }

    let display_text = build_attachment_only_display_text(
        input.payload.display_text.as_deref(),
        input.payload.text.as_deref(),
        images,
        attachments,
    );
    let normalized_mentions = normalize_payload_mentions(input.payload.mentions.as_ref());
    let has_user_mentions = !normalized_mentions.is_empty();

    let (message_parts, attachment_warnings) =
        normalize_chat_input_payload_to_message_parts(state, &input.payload, None);
    for warning in attachment_warnings {
        runtime_log_warn(format!("[附件入站] 提交消息降级继续：{warning}"));
    }

    let request_id = runtime_context_request_id_or_new(None, input.trace_id.as_deref(), "chat");
    if !claim_submit_trace_id(state, &request_id)? {
        let session = input.session.as_ref().ok_or_else(|| "缺少会话信息".to_string())?;
        let conversation_id = session
            .conversation_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("")
            .to_string();
        return Ok(SubmitChatResult {
            accepted: false,
            duplicate: true,
            event_id: String::new(),
            conversation_id,
            trace_id: request_id,
            ingress: "duplicate".to_string(),
            user_message_id: None,
            assistant_message_id: None,
        });
    }
    let user_message_id = Uuid::new_v4().to_string();
    let user_message = ChatMessage {
        id: user_message_id.clone(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: message_parts,
        extra_text_blocks,
        provider_meta: {
            let attachment_entries = collect_payload_attachment_meta_entries(&input.payload);
            build_user_message_provider_meta(
                input.payload.provider_meta.clone(),
                &attachment_entries,
                &normalized_mentions,
                Some(request_id.as_str()),
            )
        },
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    };

    let session = input.session.as_ref().ok_or_else(|| "缺少会话信息".to_string())?;
    let conversation_id = session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            runtime_log_warn(format!("[聊天发送] 缺少 conversation_id，拒绝提交用户消息"));
            "缺少 conversation_id".to_string()
        })?;

    let prepare_started_at = std::time::Instant::now();
    let (agent_id, model_config_id, mention_plans, mention_failures) = {
        let config_started_at = std::time::Instant::now();
        let runtime_org = load_runtime_organization_snapshot(state)?;
        let app_config = runtime_org.config.clone();
        let config_elapsed_ms = config_started_at.elapsed().as_millis();
        let agents = runtime_org.agents.clone();
        let app_data_elapsed_ms = 0u128;
        let agent_started_at = std::time::Instant::now();
        let agent_id = session.agent_id.trim().to_string();
        if agent_id.is_empty() {
            return Err("缺少执行人格：session.agentId 为空".to_string());
        }
        if !agents
            .iter()
            .any(|agent| agent.id == agent_id && !agent.is_built_in_user)
        {
            return Err(format!("执行人格不存在或不可用: agentId={agent_id}"));
        }
        let agent_elapsed_ms = agent_started_at.elapsed().as_millis();
        let agent = runtime_agent_by_id(&runtime_org, &agent_id)
            .ok_or_else(|| format!("执行人格已经消失：{agent_id}"))?;
        let api_config_id = agent_primary_chat_api_config_id(&app_config, agent)
            .ok_or_else(|| format!("人格模型未配置或不可用于聊天: {}", agent.id))?;

        let conversation_started_at = std::time::Instant::now();
        let conversation_meta =
            conversation_service_v2().get_conversation_meta(state, &conversation_id)?;
        let mention_background = read_user_mention_context_snapshot(
            state,
            &conversation_meta,
            &agents,
            &display_text,
        )?;
        let conversation_elapsed_ms = conversation_started_at.elapsed().as_millis();
        let (mention_plans, mention_failures) = build_user_mention_dispatch_plans(
            &app_config,
            &conversation_meta.id,
            &mention_background,
            &agents,
            &agent_id,
            &display_text,
            input.payload.mentions.as_ref(),
        )?;

        runtime_log_info(format!(
            "[聊天发送] 提交前准备耗时：总计={}ms，读取配置={}ms，读取应用数据={}ms，解析人格={}ms，会话解析={}ms，conversation_id={}，agent_id={}",
            prepare_started_at.elapsed().as_millis(),
            config_elapsed_ms,
            app_data_elapsed_ms,
            agent_elapsed_ms,
            conversation_elapsed_ms,
            conversation_id,
            agent_id
        ));

        (
            agent_id,
            api_config_id,
            mention_plans,
            mention_failures,
        )
    };

    let event_id = Uuid::new_v4().to_string();
    let assistant_message_id = (!has_user_mentions).then(|| Uuid::new_v4().to_string());
    let mut runtime_context = runtime_context_new(
        "user_message",
        if has_user_mentions { "user_mention_send" } else { "user_send" },
    );
    runtime_context.request_id = Some(request_id.clone());
    runtime_context.dispatch_id = Some(event_id.clone());
    runtime_context.origin_conversation_id = Some(conversation_id.clone());
    runtime_context.target_conversation_id = Some(conversation_id.clone());
    runtime_context.root_conversation_id = Some(conversation_id.clone());
    runtime_context.executor_agent_id = Some(agent_id.clone());
    runtime_context.model_config_id = Some(model_config_id.clone());
    let event = ChatPendingEvent {
        id: event_id.clone(),
        conversation_id: conversation_id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::User,
        queue_mode: ChatQueueMode::Normal,
        messages: vec![user_message],
        activate_assistant: !has_user_mentions,
        assistant_message_id: assistant_message_id.clone(),
        session_info: ChatSessionInfo {
            agent_id: agent_id.clone(),
        },
        runtime_context: Some(runtime_context),
        sender_info: None,
    };

    let mention_result_rx = if has_user_mentions {
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        state
            .pending_chat_result_senders
            .lock()
            .map_err(|_| "Failed to lock pending chat result senders".to_string())?
            .insert(event_id.clone(), result_tx);
        Some(result_rx)
    } else {
        None
    };

    if let Some(on_delta) = on_delta {
        register_chat_event_delta_channel(state, &event_id, on_delta)?;
    }

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
            release_submit_trace_id(state, &request_id);
            return Err(err);
        }
    };

    let (accepted, ingress_label) = match &ingress {
        ChatEventIngress::Direct(_) => (true, "direct"),
        ChatEventIngress::Queued { .. } => (true, "queued"),
    };

    trigger_chat_event_after_ingress(state, ingress);

    if accepted {
        if let Some(result_rx) = mention_result_rx {
            spawn_user_mention_after_message_flushed(
                state.clone(),
                event_id.clone(),
                result_rx,
                mention_failures,
                mention_plans,
            );
        }
    }

    Ok(SubmitChatResult {
        accepted,
        duplicate: false,
        event_id,
        conversation_id,
        trace_id: request_id,
        ingress: ingress_label.to_string(),
        user_message_id: Some(user_message_id),
        assistant_message_id,
    })
}

#[tauri::command]
async fn submit_chat_message(
    input: SendChatRequest,
    state: State<'_, AppState>,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SubmitChatResult, String> {
    submit_chat_message_inner(input, state.inner(), Some(on_delta)).await
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
    let images = input.payload.images.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
    let attachments = input
        .payload
        .attachments
        .as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    if !chat_input_payload_has_content(&input.payload) {
        return Err("消息内容为空".to_string());
    }

    let display_text = build_attachment_only_display_text(
        input.payload.display_text.as_deref(),
        input.payload.text.as_deref(),
        images,
        attachments,
    );
    let normalized_mentions = normalize_payload_mentions(input.payload.mentions.as_ref());

    let (message_parts, attachment_warnings) =
        normalize_chat_input_payload_to_message_parts(state.inner(), &input.payload, None);
    for warning in attachment_warnings {
        runtime_log_warn(format!("[附件入站] 发送消息降级继续：{warning}"));
    }
    // 先确定 requestId，再写入用户消息 provider_meta，保证重复发送可按已落地消息幂等识别。
    let request_id = runtime_context_request_id_or_new(None, input.trace_id.as_deref(), "chat");
    let user_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: message_parts,
        extra_text_blocks: input.payload.extra_text_blocks.clone().unwrap_or_default(),
        provider_meta: {
            let attachment_entries = collect_payload_attachment_meta_entries(&input.payload);
            build_user_message_provider_meta(
                input.payload.provider_meta.clone(),
                &attachment_entries,
                &normalized_mentions,
                Some(request_id.as_str()),
            )
        },
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    };

    // 获取会话信息
    let session = input.session.as_ref().ok_or_else(|| "缺少会话信息".to_string())?;
    let conversation_id = session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            runtime_log_warn(format!("[聊天发送] 缺少 conversation_id，拒绝发送用户消息"));
            "缺少 conversation_id".to_string()
        })?;

    let prepare_started_at = std::time::Instant::now();
    let (agent_id, model_config_id, mention_plans, mention_failures) = {
        let config_started_at = std::time::Instant::now();
        let runtime_org = load_runtime_organization_snapshot(&state)?;
        let app_config = runtime_org.config.clone();
        let config_elapsed_ms = config_started_at.elapsed().as_millis();
        let agents = runtime_org.agents.clone();
        let app_data_elapsed_ms = 0u128;
        let agent_started_at = std::time::Instant::now();
        let agent_id = session.agent_id.trim().to_string();
        if agent_id.is_empty() {
            return Err("缺少执行人格：session.agentId 为空".to_string());
        }
        if !agents
            .iter()
            .any(|agent| agent.id == agent_id && !agent.is_built_in_user)
        {
            return Err(format!("执行人格不存在或不可用: agentId={agent_id}"));
        }
        let agent_elapsed_ms = agent_started_at.elapsed().as_millis();
        let agent = runtime_agent_by_id(&runtime_org, &agent_id)
            .ok_or_else(|| format!("执行人格已经消失：{agent_id}"))?;
        let api_config_id = agent_primary_chat_api_config_id(&app_config, agent)
            .ok_or_else(|| format!("人格模型未配置或不可用于聊天: {}", agent.id))?;

        let conversation_started_at = std::time::Instant::now();
        let conversation_meta =
            conversation_service_v2().get_conversation_meta(&state, &conversation_id)?;
        let mention_background = read_user_mention_context_snapshot(
            &state,
            &conversation_meta,
            &agents,
            &display_text,
        )?;
        let conversation_elapsed_ms = conversation_started_at.elapsed().as_millis();
        let (mention_plans, mention_failures) = build_user_mention_dispatch_plans(
            &app_config,
            &conversation_meta.id,
            &mention_background,
            &agents,
            &agent_id,
            &display_text,
            input.payload.mentions.as_ref(),
        )?;

        runtime_log_info(format!(
            "[聊天发送] 发送前准备耗时：总计={}ms，读取配置={}ms，读取应用数据={}ms，解析人格={}ms，会话解析={}ms，conversation_id={}，agent_id={}",
            prepare_started_at.elapsed().as_millis(),
            config_elapsed_ms,
            app_data_elapsed_ms,
            agent_elapsed_ms,
            conversation_elapsed_ms,
            conversation_id,
            agent_id
        ));

        (
            agent_id,
            api_config_id,
            mention_plans,
            mention_failures,
        )
    };

    // 构造队列事件
    let event_id = Uuid::new_v4().to_string();
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
    runtime_context.model_config_id = Some(model_config_id.clone());
    let event = ChatPendingEvent {
        id: event_id.clone(),
        conversation_id: conversation_id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::User,
        queue_mode: ChatQueueMode::Normal,
        messages: vec![user_message],
        activate_assistant: !has_user_mentions,
        assistant_message_id: None,
        session_info: ChatSessionInfo {
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
    trigger_chat_event_after_ingress(state.inner(), ingress);

    let send_result = result_rx
        .await
        .map_err(|_| "聊天请求已取消或调度结果丢失".to_string())?;
    let send_result = send_result?;

    for failure in mention_failures {
        spawn_user_mention_failure_message(state.inner().clone(), failure);
    }
    for plan in mention_plans {
        spawn_user_mention_delegate(state.inner().clone(), plan);
    }

    Ok(send_result)
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

    let images = input.payload.images.as_ref().map(|v| v.as_slice()).unwrap_or(&[]);
    let attachments = input
        .payload
        .attachments
        .as_ref()
        .map(|v| v.as_slice())
        .unwrap_or(&[]);

    if !chat_input_payload_has_content(&input.payload) {
        return Err("消息内容为空".to_string());
    }

    let display_text = build_attachment_only_display_text(
        input.payload.display_text.as_deref(),
        input.payload.text.as_deref(),
        images,
        attachments,
    );
    let normalized_mentions = normalize_payload_mentions(input.payload.mentions.as_ref());

    let (message_parts, attachment_warnings) =
        normalize_chat_input_payload_to_message_parts(state, &input.payload, None);
    for warning in attachment_warnings {
        runtime_log_warn(format!("[附件入站] @消息降级继续：{warning}"));
    }
    // 先确定 requestId，再写入用户消息 provider_meta，保证重复发送可按已落地消息幂等识别。
    let request_id = runtime_context_request_id_or_new(None, input.trace_id.as_deref(), "chat");
    let user_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: message_parts,
        extra_text_blocks: input.payload.extra_text_blocks.clone().unwrap_or_default(),
        provider_meta: {
            let attachment_entries = collect_payload_attachment_meta_entries(&input.payload);
            build_user_message_provider_meta(
                input.payload.provider_meta.clone(),
                &attachment_entries,
                &normalized_mentions,
                Some(request_id.as_str()),
            )
        },
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    };

    let session = input.session.as_ref().ok_or_else(|| "缺少会话信息".to_string())?;
    let conversation_id = session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            runtime_log_warn(format!("[聊天发送] 缺少 conversation_id，拒绝发送用户@委托消息"));
            "缺少 conversation_id".to_string()
        })?;

    let prepare_started_at = std::time::Instant::now();
    let (agent_id, model_config_id, mention_plans, mention_failures) = {
        let config_started_at = std::time::Instant::now();
        let runtime_org = load_runtime_organization_snapshot(state)?;
        let app_config = runtime_org.config.clone();
        let config_elapsed_ms = config_started_at.elapsed().as_millis();
        let agents = runtime_org.agents.clone();
        let app_data_elapsed_ms = 0u128;
        let agent_started_at = std::time::Instant::now();
        let agent_id = session.agent_id.trim().to_string();
        if agent_id.is_empty() {
            return Err("缺少执行人格：session.agentId 为空".to_string());
        }
        if !agents
            .iter()
            .any(|agent| agent.id == agent_id && !agent.is_built_in_user)
        {
            return Err(format!("执行人格不存在或不可用: agentId={agent_id}"));
        }
        let agent_elapsed_ms = agent_started_at.elapsed().as_millis();
        let agent = runtime_agent_by_id(&runtime_org, &agent_id)
            .ok_or_else(|| format!("执行人格已经消失：{agent_id}"))?;
        let api_config_id = agent_primary_chat_api_config_id(&app_config, agent)
            .ok_or_else(|| format!("人格模型未配置或不可用于聊天: {}", agent.id))?;

        let conversation_started_at = std::time::Instant::now();
        let conversation_meta =
            conversation_service_v2().get_conversation_meta(state, &conversation_id)?;
        let mention_background = read_user_mention_context_snapshot(
            state,
            &conversation_meta,
            &agents,
            &display_text,
        )?;
        let conversation_elapsed_ms = conversation_started_at.elapsed().as_millis();
        let (mention_plans, mention_failures) = build_user_mention_dispatch_plans(
            &app_config,
            &conversation_meta.id,
            &mention_background,
            &agents,
            &agent_id,
            &display_text,
            input.payload.mentions.as_ref(),
        )?;

        runtime_log_info(format!(
            "[聊天发送] 用户@委托发送前准备耗时：总计={}ms，读取配置={}ms，读取应用数据={}ms，解析人格={}ms，会话解析={}ms，conversation_id={}，agent_id={}，mention_count={}",
            prepare_started_at.elapsed().as_millis(),
            config_elapsed_ms,
            app_data_elapsed_ms,
            agent_elapsed_ms,
            conversation_elapsed_ms,
            conversation_id,
            agent_id,
            mention_count
        ));

        (
            agent_id,
            api_config_id,
            mention_plans,
            mention_failures,
        )
    };

    let event_id = Uuid::new_v4().to_string();
    let mut runtime_context = runtime_context_new("user_message", "user_mention_send");
    runtime_context.request_id = Some(request_id.clone());
    runtime_context.dispatch_id = Some(event_id.clone());
    runtime_context.origin_conversation_id = Some(conversation_id.clone());
    runtime_context.target_conversation_id = Some(conversation_id.clone());
    runtime_context.root_conversation_id = Some(conversation_id.clone());
    runtime_context.executor_agent_id = Some(agent_id.clone());
    runtime_context.model_config_id = Some(model_config_id.clone());
    let event = ChatPendingEvent {
        id: event_id.clone(),
        conversation_id: conversation_id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::User,
        queue_mode: ChatQueueMode::Normal,
        messages: vec![user_message],
        activate_assistant: false,
        assistant_message_id: None,
        session_info: ChatSessionInfo {
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

    trigger_chat_event_after_ingress(state, ingress);

    let send_result = result_rx
        .await
        .map_err(|_| "聊天请求已取消或调度结果丢失".to_string())?;
    let send_result = send_result?;

    for failure in mention_failures {
        spawn_user_mention_failure_message(state.clone(), failure);
    }
    for plan in mention_plans {
        spawn_user_mention_delegate(state.clone(), plan);
    }

    Ok(send_result)
}

#[tauri::command]
async fn send_user_mention_message(
    input: SendChatRequest,
    state: State<'_, AppState>,
    on_delta: tauri::ipc::Channel<AssistantDeltaEvent>,
) -> Result<SendChatResult, String> {
    send_user_mention_message_inner(input, state.inner(), &on_delta).await
}

#[tauri::command]
async fn submit_user_async_delegate(
    input: SubmitUserAsyncDelegateInput,
    state: State<'_, AppState>,
) -> Result<SubmitUserAsyncDelegateOutput, String> {
    submit_user_async_delegate_internal(input, state.inner()).await
}

async fn submit_user_async_delegate_internal(
    input: SubmitUserAsyncDelegateInput,
    state: &AppState,
) -> Result<SubmitUserAsyncDelegateOutput, String> {
    let (plan, selected_message_count) = resolve_user_async_delegate_plan(state, &input)?;
    let output = SubmitUserAsyncDelegateOutput {
        delegate_id: String::new(),
        conversation_id: plan.root_conversation_id.clone(),
        target_agent_id: plan.target_agent_id.clone(),
        target_agent_name: plan.target_agent_name.clone(),
        selected_message_count,
    };
    let delegate_id = spawn_user_async_delegate(state.clone(), plan)?;
    runtime_log_info(format!(
        "[用户异步委托] 发起 完成 conversation_id={} delegate_id={} target_agent_id={} selected_message_count={}",
        output.conversation_id,
        delegate_id,
        output.target_agent_id,
        output.selected_message_count
    ));
    Ok(SubmitUserAsyncDelegateOutput {
        delegate_id,
        ..output
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BindActiveChatViewStreamInput {
    #[serde(default)]
    binding_id: String,
    #[serde(default)]
    conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct UnbindActiveChatViewStreamInput {
    #[serde(default)]
    binding_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProbeActiveChatViewStreamInput {
    #[serde(default)]
    binding_id: String,
    #[serde(default)]
    conversation_id: Option<String>,
    probe_id: String,
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
            &input.binding_id,
            Some(conversation_id),
            on_delta.clone(),
        )?;
        runtime_log_debug(format!(
            "[聊天] 已绑定活动聊天流: window={}, binding_id={}, conversation_id={}",
            window_label,
            normalize_active_chat_view_binding_id(&input.binding_id),
            conversation_id,
        ));
    } else {
        set_active_chat_view_stream_binding(
            state.inner(),
            &window_label,
            &input.binding_id,
            None,
            on_delta,
        )?;
        runtime_log_debug(format!(
            "[聊天] 已取消活动聊天流绑定: window={}, binding_id={}",
            window_label,
            normalize_active_chat_view_binding_id(&input.binding_id),
        ));
    }
    Ok(())
}

#[tauri::command]
async fn unbind_active_chat_view_stream(
    input: Option<UnbindActiveChatViewStreamInput>,
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<(), String> {
    let window_label = window.label().to_string();
    let binding_id = input.unwrap_or_default().binding_id;
    clear_active_chat_view_stream_binding(state.inner(), &window_label, &binding_id)?;
    runtime_log_debug(format!(
        "[聊天] 已取消活动聊天流订阅: window={}, binding_id={}",
        window_label,
        normalize_active_chat_view_binding_id(&binding_id),
    ));
    Ok(())
}

#[tauri::command]
async fn clear_window_chat_view_stream_bindings_command(
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<(), String> {
    let window_label = window.label().to_string();
    clear_window_chat_view_stream_bindings(state.inner(), &window_label)
}

#[tauri::command]
async fn probe_active_chat_view_stream(
    input: ProbeActiveChatViewStreamInput,
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<bool, String> {
    let window_label = window.label().to_string();
    let conversation_id = input
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    let probe_id = input.probe_id.trim();
    if conversation_id.is_empty() || probe_id.is_empty() {
        return Ok(false);
    }
    let binding_key = active_chat_view_binding_key(&window_label, &input.binding_id);
    let binding = state
        .active_chat_view_bindings
        .lock()
        .map_err(|_| "Failed to lock active chat view bindings".to_string())?
        .get(&binding_key)
        .cloned();
    let Some(binding) = binding else {
        return Ok(false);
    };
    if binding.conversation_id.trim() != conversation_id {
        return Ok(false);
    }
    let event = AssistantDeltaEvent {
        delta: String::new(),
        kind: Some("stream_probe".to_string()),
        request_id: None,
        activation_id: None,
        phase_id: None,
        reason: None,
        tool_name: None,
        tool_call_id: None,
        tool_status: None,
        tool_args: None,
        message: Some(probe_id.to_string()),
        stream_cache: None,
    };
    match binding.delta_channel.send(event) {
        Ok(_) => Ok(true),
        Err(_) => {
            let _ = state
                .active_chat_view_bindings
                .lock()
                .map(|mut bindings| bindings.remove(&binding_key));
            Ok(false)
        }
    }
}

#[tauri::command]
async fn stop_chat_message(
    input: StopChatRequest,
    state: State<'_, AppState>,
) -> Result<StopChatResult, String> {
    stop_chat_message_inner(input, state.inner())
}

fn stop_chat_message_inner(
    input: StopChatRequest,
    state: &AppState,
) -> Result<StopChatResult, String> {
    let requested_conversation_id = input
        .session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let agent_id = resolve_runtime_control_agent_id(
        state,
        Some(input.session.agent_id.as_str()),
        requested_conversation_id.as_deref(),
    )?;

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
    let aborted_tool = abort_inflight_tool_abort_handle(state, &chat_key)?;
    let aborted_delegate_children =
        abort_delegate_runtime_descendants_by_parent_context(
            state,
            &chat_key,
            requested_conversation_id.as_deref(),
        )?;
    let aborted = aborted_chat || aborted_tool || aborted_delegate_children > 0;
    if aborted {
        if let Some(conversation_id) = requested_conversation_id.as_deref() {
            mark_goal_continue_suppressed_by_user_interrupt(
                state,
                conversation_id,
                "stop_chat_message",
            )?;
        }
    }
    if aborted_delegate_children > 0 {
        runtime_log_info(format!(
            "[聊天] 停止请求已级联到同步委托子会话: session={}, child_count={}",
            chat_key,
            aborted_delegate_children
        ));
    }
    let conversation_id = requested_conversation_id.clone();
    Ok(StopChatResult {
        aborted,
        persisted: false,
        conversation_id,
        assistant_text: String::new(),
        assistant_message: None,
    })
}

#[tauri::command]
async fn get_chat_queue_snapshot(
    state: State<'_, AppState>,
) -> Result<Vec<ChatQueueEventSummary>, String> {
    get_queue_snapshot(state.inner())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetConversationFastRequestTurnsInput {
    conversation_id: String,
}

#[tauri::command]
fn get_conversation_fast_request_turns(
    input: GetConversationFastRequestTurnsInput,
    state: State<'_, AppState>,
) -> Result<Vec<FastRequestTurn>, String> {
    conversation_service_v2()
        .get_conversation_fast_request_turns(state.inner(), &input.conversation_id)
}

#[tauri::command]
async fn recall_chat_queue_event(
    event_id: String,
    state: State<'_, AppState>,
) -> Result<ChatQueueRecallResult, String> {
    recall_chat_queue_event_inner(&event_id, state.inner())
}

fn recall_chat_queue_event_inner(
    event_id: &str,
    state: &AppState,
) -> Result<ChatQueueRecallResult, String> {
    let removed = recall_queue_event(state, event_id)?;
    let message_text = removed
        .as_ref()
        .and_then(|event| {
            event.messages.first().and_then(|msg| {
                msg.parts.iter().find_map(|part| match part {
                    MessagePart::Text { text, .. } => Some(text.clone()),
                    _ => None,
                })
            })
        })
        .unwrap_or_default();
    Ok(ChatQueueRecallResult {
        removed: removed.is_some(),
        message_text,
        not_in_queue: removed.is_none(),
    })
}

#[tauri::command]
async fn mark_chat_queue_event_guided(
    event_id: String,
    state: State<'_, AppState>,
) -> Result<ChatQueueMarkGuidedResult, String> {
    mark_chat_queue_event_guided_inner(&event_id, state.inner())
}

fn mark_chat_queue_event_guided_inner(
    event_id: &str,
    state: &AppState,
) -> Result<ChatQueueMarkGuidedResult, String> {
    let conversation_id = mark_queue_event_guided(state, event_id)?;
    if let Some(conversation_id) = conversation_id {
        trigger_guided_queue_processing(state, &conversation_id);
        return Ok(ChatQueueMarkGuidedResult {
            updated: true,
            not_in_queue: false,
        });
    }
    Ok(ChatQueueMarkGuidedResult {
        updated: false,
        not_in_queue: true,
    })
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
    let conversation_id = session
        .conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "Missing session.conversationId".to_string())?;
    let agent_id = resolve_runtime_control_agent_id(
        state.inner(),
        Some(session.agent_id.as_str()),
        Some(&conversation_id),
    )?;

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
        abort_delegate_runtime_descendants_by_parent_context(
            state.inner(),
            &chat_key,
            Some(&conversation_id),
        )?;
    let cleared_queue_count = clear_conversation_queue(
        state.inner(),
        &conversation_id,
        "消息已因会话撤回被清出队列",
    )?;
    let _ = release_conversation_processing_claim(state.inner(), &conversation_id);
    let _ = set_conversation_runtime_state_and_emit(
        state.inner(),
        &conversation_id,
        MainSessionState::Idle,
    );
    let _ = set_conversation_remote_im_activation_sources(state.inner(), &conversation_id, Vec::new());

    let aborted = aborted_chat || aborted_tool || aborted_delegate_children > 0;
    if aborted || cleared_queue_count > 0 {
        mark_goal_continue_suppressed_by_user_interrupt(
            state.inner(),
            &conversation_id,
            "interrupt_conversation_runtime",
        )?;
    }
    runtime_log_info(format!(
        "[聊天调度] 会话运行已中断: conversation_id={}, aborted={}, cleared_queue_count={}, child_abort_count={}",
        conversation_id,
        aborted,
        cleared_queue_count,
        aborted_delegate_children
    ));
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

fn assistant_text_from_stream_blocks(blocks: &[AssistantStreamBlock]) -> String {
    blocks
        .iter()
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("")
}

fn reasoning_text_from_stream_blocks(blocks: &[AssistantStreamBlock]) -> String {
    blocks
        .iter()
        .map(|block| block.reasoning.trim())
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod stop_stream_block_tool_history_tests {
    use super::*;

    #[test]
    fn canonical_submit_should_accept_and_normalize_extra_text_only_payload() {
        let request: SendChatRequest = serde_json::from_value(serde_json::json!({
            "payload": {
                "text": null,
                "images": [],
                "attachments": [],
                "extraTextBlocks": ["  context  ", "  "]
            },
            "session": {
                "apiConfigId": null,
                "agentId": "agent-1",
                "conversationId": "conversation-1"
            }
        }))
        .unwrap();

        assert_eq!(normalize_send_extra_text_blocks(&request.payload), vec!["context"]);
    }

    #[test]
    fn build_attachment_only_display_text_should_describe_images_when_text_missing() {
        let text = build_attachment_only_display_text(
            None,
            None,
            &[BinaryPart {
                mime: "image/png".to_string(),
                bytes_base64: "abc".to_string(),
                saved_path: Some("media/a.png".to_string()),
            }],
            &[],
        );

        assert_eq!(text, "用户发送了1张图片。请基于这些内容处理。");
    }

    #[test]
    fn build_attachment_only_display_text_should_include_attachment_names() {
        let text = build_attachment_only_display_text(
            None,
            None,
            &[],
            &[AttachmentMetaInput {
                file_name: "report.pdf".to_string(),
                path: "exports/report.pdf".to_string(),
                mime: "application/pdf".to_string(),
            }],
        );

        assert_eq!(text, "用户发送了1个附件：report.pdf。请基于这些内容处理。");
    }

}

#[tauri::command]
async fn get_conversation_runtime_snapshot(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationRuntimeSnapshot, String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    let snapshot = read_conversation_runtime_snapshot(state.inner(), normalized_conversation_id)?;
    runtime_log_debug(format!(
        "[聊天运行态恢复] 完成，任务=读取会话运行态快照，conversation_id={}，runtime_state={:?}，is_processing={}，pending_queue_count={}，has_visible_progress={}，assistant_text_len={}，stream_block_count={}",
        snapshot.conversation_id,
        snapshot.runtime_state,
        snapshot.is_processing,
        snapshot.pending_queue_count,
        snapshot.stream_cache.has_visible_progress,
        snapshot.stream_cache.assistant_text.chars().count(),
        snapshot.stream_cache.stream_blocks.len()
    ));
    Ok(snapshot)
}
