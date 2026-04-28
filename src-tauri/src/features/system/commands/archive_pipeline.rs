fn mark_tasks_as_session_lost(data_path: &PathBuf, conversation_id: &str) {
    let Ok(tasks) = task_store_list_task_records(data_path) else {
        eprintln!(
            "[TASK-CLEANUP] 查询任务列表失败: conversation_id={}",
            conversation_id
        );
        return;
    };
    for task in &tasks {
        if task.completion_state != TASK_STATE_ACTIVE {
            continue;
        }
        if task.conversation_id.as_deref() != Some(conversation_id) {
            continue;
        }
        if let Err(err) = task_store_complete_task(
            data_path,
            &TaskCompleteInput {
                task_id: task.task_id.clone(),
                completion_state: TASK_STATE_FAILED_COMPLETED.to_string(),
                completion_conclusion: "会话丢失".to_string(),
            },
        ) {
            eprintln!(
                "[TASK-CLEANUP] 标记任务失败: task_id={}, conversation_id={}, error={}",
                task.task_id, conversation_id, err
            );
        }
    }
}

fn upsert_memories_into_store_with_ids(
    data_path: &PathBuf,
    drafts: &[ArchiveMemoryDraft],
    owner_agent_id: Option<&str>,
) -> Result<Vec<String>, String> {
    let mut inputs = Vec::<MemoryDraftInput>::new();
    for d in drafts {
        let judgment = clean_text(d.judgment.trim());
        if judgment.is_empty() {
            continue;
        }
        let tags = normalize_memory_keywords(&d.tags);
        if tags.is_empty() {
            continue;
        }
        inputs.push(MemoryDraftInput {
            memory_type: d.memory_type.clone(),
            judgment,
            reasoning: clean_text(d.reasoning.trim()),
            tags,
            owner_agent_id: owner_agent_id
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned),
        });
    }
    let (results, _) = memory_store_upsert_drafts(data_path, &inputs)?;
    Ok(results.into_iter().filter_map(|r| r.id).collect::<Vec<_>>())
}

fn merge_memories_into_store(
    data_path: &PathBuf,
    drafts: &[ArchiveMemoryDraft],
    owner_agent_id: Option<&str>,
) -> Result<usize, String> {
    Ok(upsert_memories_into_store_with_ids(data_path, drafts, owner_agent_id)?.len())
}

fn merge_memory_groups_into_store(
    data_path: &PathBuf,
    groups: &[ArchiveMergeGroupDraft],
    owner_agent_id: Option<&str>,
) -> Result<usize, String> {
    let mut merged_groups = 0usize;
    for group in groups {
        let source_ids = group
            .source_ids
            .iter()
            .map(|id| id.trim())
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if source_ids.len() < 2 {
            continue;
        }
        let upserted_ids = upsert_memories_into_store_with_ids(
            data_path,
            &[group.target.clone()],
            owner_agent_id,
        )?;
        let retained = upserted_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<HashSet<_>>();
        let mut deleted_any = false;
        for source_id in source_ids {
            if retained.contains(source_id.as_str()) {
                continue;
            }
            match memory_store_delete_memory(data_path, &source_id) {
                Ok(_) => {
                    deleted_any = true;
                }
                Err(err) => {
                    eprintln!(
                        "[ARCHIVE-PIPELINE] delete merged source memory failed: id={}, err={}",
                        source_id, err
                    );
                }
            }
        }
        if deleted_any {
            merged_groups += 1;
        }
    }
    Ok(merged_groups)
}

fn resolve_archive_owner_context(
    state: &AppState,
    source: &Conversation,
) -> Result<(AgentProfile, String, String), String> {
    let mut config = state_read_config_cached(state)?;
    let runtime = state_read_runtime_state_cached(state)?;
    let user_alias = runtime.user_alias.clone();
    let mut agents = state_read_agents_cached(state)?;
    merge_private_organization_into_runtime(&state.data_path, &mut config, &mut agents)?;

    let owner_agent_id = resolve_archive_owner_agent_id(&config, &agents, source)?;
    let owner_agent = agents
        .iter()
        .find(|agent| agent.id == owner_agent_id)
        .cloned()
        .ok_or_else(|| {
            format!(
                "归档记忆归属人格不存在: conversation_id={}, agent_id={}",
                source.id, owner_agent_id
            )
        })?;

    Ok((owner_agent, owner_agent_id, user_alias))
}

fn archive_profile_memory_type_allowed(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "knowledge" | "skill" | "event"
    )
}

fn apply_profile_memories_into_store(
    data_path: &PathBuf,
    drafts: &[ArchiveProfileMemoryDraft],
    owner_agent_id: Option<&str>,
) -> Result<(usize, usize, usize), String> {
    let started_at = std::time::Instant::now();
    runtime_log_info(format!(
        "[用户画像] 开始，任务=apply_profile_memories_into_store，items={}",
        drafts.len()
    ));
    let mut memory_ids = Vec::<String>::new();
    let mut created_count = 0usize;
    let mut skipped_count = 0usize;
    let memory_map = memory_store_list_memories(data_path)?
        .into_iter()
        .map(|memory| (memory.id.clone(), memory))
        .collect::<HashMap<String, MemoryEntry>>();

    for item in drafts {
        let existing_id = item
            .memory_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(memory_id) = existing_id {
            if let Some(found) = memory_map.get(memory_id) {
                if memory_entry_allowed_for_profile(found) {
                    memory_ids.push(memory_id.to_string());
                } else {
                    skipped_count += 1;
                }
            } else {
                skipped_count += 1;
            }
            continue;
        }

        if let Some(memory) = item.memory.as_ref() {
            if !archive_profile_memory_type_allowed(&memory.memory_type) {
                skipped_count += 1;
                continue;
            }
            let inserted_ids =
                upsert_memories_into_store_with_ids(data_path, &[memory.clone()], owner_agent_id)?;
            if inserted_ids.is_empty() {
                skipped_count += 1;
                continue;
            }
            created_count += inserted_ids.len();
            memory_ids.extend(inserted_ids);
            continue;
        }

        skipped_count += 1;
    }

    let linked_count = memory_store_upsert_profile_memory_links(data_path, &memory_ids, "auto")?;
    runtime_log_info(format!(
        "[用户画像] 完成，任务=apply_profile_memories_into_store，linked_count={}，created_count={}，skipped_count={}，elapsed_ms={}",
        linked_count,
        created_count,
        skipped_count,
        started_at.elapsed().as_millis()
    ));
    Ok((linked_count, created_count, skipped_count))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForceArchiveResult {
    archived: bool,
    archive_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compaction_message: Option<ChatMessage>,
    summary: String,
    merged_memories: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_feedback: Option<MemoryArchiveFeedbackReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    merge_groups: Option<usize>,
}

fn build_context_compaction_followup_runtime_context() -> RuntimeContext {
    let mut runtime_context = runtime_context_new(
        "context_compaction",
        "context_compaction_followup",
    );
    runtime_context.request_id = Some(format!(
        "context-compaction-followup-{}",
        Uuid::new_v4()
    ));
    runtime_context
}

fn context_compaction_followup_department_id(
    state: &AppState,
    source: &Conversation,
    effective_agent_id: &str,
) -> Result<String, String> {
    let department_id = source.department_id.trim();
    if !department_id.is_empty() {
        return Ok(department_id.to_string());
    }
    Ok(department_for_agent_id(&state_read_config_cached(state)?, effective_agent_id)
        .map(|department| department.id.clone())
        .unwrap_or_else(|| ASSISTANT_DEPARTMENT_ID.to_string()))
}

fn enqueue_context_compaction_followup(
    state: &AppState,
    source: &Conversation,
    effective_agent_id: &str,
) -> Result<(), String> {
    if conversation_has_guided_queue_events(state, &source.id).unwrap_or(false) {
        return Ok(());
    }
    let followup_event = ChatPendingEvent {
        id: format!("context-compaction-followup-{}", Uuid::new_v4()),
        conversation_id: source.id.clone(),
        created_at: now_iso(),
        source: ChatEventSource::System,
        queue_mode: ChatQueueMode::Normal,
        messages: Vec::new(),
        activate_assistant: true,
        session_info: ChatSessionInfo {
            department_id: context_compaction_followup_department_id(state, source, effective_agent_id)?,
            agent_id: effective_agent_id.to_string(),
        },
        runtime_context: Some(build_context_compaction_followup_runtime_context()),
        sender_info: None,
    };
    match ingress_chat_event(state, followup_event)? {
        ChatEventIngress::Direct(event) => {
            trigger_chat_event_after_ingress(state, ChatEventIngress::Direct(event));
        }
        ChatEventIngress::Queued { event_id } => {
            runtime_log_info(format!(
                "[上下文整理] 完成后续激活已入队 conversation_id={} event_id={}",
                source.id, event_id
            ));
        }
        ChatEventIngress::Duplicate { event_id } => {
            runtime_log_info(format!(
                "[上下文整理] 完成后续激活重复，已忽略 conversation_id={} event_id={}",
                source.id, event_id
            ));
        }
    }
    trigger_chat_queue_processing(state);
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForceArchivePreviewResult {
    conversation_id: String,
    can_archive: bool,
    can_drop_conversation: bool,
    message_count: usize,
    has_assistant_reply: bool,
    is_empty: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    archive_disabled_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForceArchiveCurrentInput {
    session: SessionSelector,
    #[serde(default)]
    target_conversation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForceCompactionPreviewResult {
    conversation_id: String,
    can_compact: bool,
    message_count: usize,
    has_assistant_reply: bool,
    is_empty: bool,
    context_usage_percent: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    compaction_disabled_reason: Option<String>,
}

const SHORT_CONVERSATION_DELETE_THRESHOLD: usize = 3;

fn archive_report_scope_label(source: &Conversation) -> &'static str {
    let has_valid_fork_cursor = source
        .fork_message_cursor
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|cursor| {
            source
                .messages
                .iter()
                .any(|message| message.id.trim() == cursor)
                .then_some(())
        })
        .is_some();
    if has_valid_fork_cursor {
        "post_fork_discussion"
    } else {
        "full_conversation"
    }
}

fn build_archive_reporting_conversation(
    source: &Conversation,
) -> std::borrow::Cow<'_, Conversation> {
    let Some(fork_cursor) = source
        .fork_message_cursor
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return std::borrow::Cow::Borrowed(source);
    };
    let Some(fork_index) = source
        .messages
        .iter()
        .position(|message| message.id.trim() == fork_cursor)
    else {
        return std::borrow::Cow::Borrowed(source);
    };
    let mut reporting = source.clone();
    reporting.messages = source
        .messages
        .iter()
        .skip(fork_index + 1)
        .cloned()
        .collect();
    std::borrow::Cow::Owned(reporting)
}

fn build_archive_delivery_message(
    source: &Conversation,
    archive_id: &str,
    summary: &str,
) -> ChatMessage {
    let source_title = if source.title.trim().is_empty() {
        conversation_preview_title(source)
    } else {
        source.title.trim().to_string()
    };
    let text = format!(
        "[归档汇报]\n来源会话：{}\n\n{}",
        clean_text(source_title.trim()),
        clean_text(summary.trim())
    );
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text { text }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "message_meta": {
                "kind": "archive_report_delivery",
                "scene": "archive_delivery",
            },
            "archiveReport": {
                "archiveId": archive_id,
                "sourceConversationId": source.id,
                "scope": archive_report_scope_label(source),
            }
        })),
        tool_call: None,
        mcp_call: None,
    }
}

fn append_archive_delivery_message_to_conversation(
    state: &AppState,
    target_conversation_id: &str,
    source: &Conversation,
    archive_id: &str,
    summary: &str,
) -> Result<(), String> {
    let target_conversation_id = target_conversation_id.trim();
    if target_conversation_id.is_empty() {
        return Ok(());
    }
    if target_conversation_id == source.id.trim() {
        return Err("归档投放目标不能与来源会话相同".to_string());
    }
    let message = build_archive_delivery_message(source, archive_id, summary);
    let mut target = state_read_conversation_cached(state, target_conversation_id)?
        .clone();
    if !target.summary.trim().is_empty() || !conversation_visible_in_foreground_lists(&target) {
        return Err(format!(
            "归档投放目标会话不符合投放条件：{target_conversation_id}"
        ));
    }
    target.messages.push(message.clone());
    target.updated_at = message.created_at.clone();
    target.last_assistant_at = Some(message.created_at.clone());
    state_schedule_conversation_persist(state, &target, true)?;
    emit_conversation_message_appended_event(state, target_conversation_id, &message);
    if let Err(err) = emit_unarchived_conversation_overview_updated_from_state(state) {
        runtime_log_warn(format!(
            "[归档投放] 警告，任务=emit_unarchived_overview_after_delivery，target_conversation_id={}，error={}",
            target_conversation_id, err
        ));
    }
    Ok(())
}

fn resolve_archive_target_conversation(
    state: &AppState,
    input: &SessionSelector,
) -> Result<(ApiConfig, ResolvedApiConfig, Conversation, String), String> {
    conversation_service().resolve_archive_target_conversation(state, input)
}

fn prepare_background_archive_active_conversation(
    state: &AppState,
    selected_api: &ApiConfig,
    source: &Conversation,
) -> Result<String, String> {
    conversation_service().prepare_background_archive_active_conversation(
        state,
        selected_api,
        source,
    )
}

fn build_force_compaction_preview_result(
    state: &AppState,
    selected_api: &ApiConfig,
    source: &Conversation,
) -> Result<ForceCompactionPreviewResult, String> {
    let message_count = archive_pipeline_message_count_for_delete(source);
    let has_assistant_reply = archive_pipeline_has_assistant_reply(source);
    let is_empty = source.messages.is_empty();
    let usage_ratio = conversation_prompt_service()
        .latest_real_prompt_usage(source, selected_api)
        .map(|usage| usage.usage_ratio.max(0.0))
        .unwrap_or(0.0);
    let context_usage_percent = usage_ratio.mul_add(100.0, 0.0).round().clamp(0.0, 100.0) as u32;
    let compaction_disabled_reason = if get_conversation_runtime_state(state, &source.id)?
        == MainSessionState::OrganizingContext
    {
        Some("当前会话正在整理上下文或归档处理中，请稍候。".to_string())
    } else if is_empty {
        Some("当前会话为空，无需整理。".to_string())
    } else if !has_assistant_reply {
        Some("当前会话还没有助理回复，暂不建议压缩。".to_string())
    } else {
        None
    };
    Ok(ForceCompactionPreviewResult {
        conversation_id: source.id.clone(),
        can_compact: compaction_disabled_reason.is_none(),
        message_count,
        has_assistant_reply,
        is_empty,
        context_usage_percent,
        compaction_disabled_reason,
    })
}

fn archive_pipeline_message_count_for_delete(source: &Conversation) -> usize {
    source
        .messages
        .iter()
        .filter(|message| {
            matches!(
                message.role.trim().to_ascii_lowercase().as_str(),
                "user" | "assistant"
            )
        })
        .count()
}

fn archive_pipeline_has_assistant_reply(source: &Conversation) -> bool {
    source
        .messages
        .iter()
        .any(|message| message.role.trim().eq_ignore_ascii_case("assistant"))
}

async fn summarize_archived_conversation_with_model_v2(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    user_alias: &str,
    source_conversation: &Conversation,
    scene: SummaryContextScene,
    memories: &[MemoryEntry],
    _recall_table: &[String],
) -> Result<MemoryCurationDraft, String> {
    let app_config = state_read_config_cached(state)?;
    let current_user_profile = build_user_profile_memory_board(&state.data_path, agent)?
        .unwrap_or_else(|| "（无）".to_string());
    let agents = state_read_agents_cached(state)?;
    let mut prepared = build_prepared_prompt_for_mode(
        PromptBuildMode::SummaryContext,
        source_conversation,
        agent,
        &agents,
        &app_config.departments,
        user_alias,
        "",
        "concise",
        "zh-CN",
        None,
        None,
        None,
        Some(ChatPromptOverrides {
            latest_user_intent: Some(LatestUserPayloadIntent::SummaryContext {
                scene,
                user_alias: user_alias.to_string(),
                current_user_profile: current_user_profile.clone(),
                include_todo_block: build_summary_context_todo_block(source_conversation)
                    .is_some(),
            }),
            latest_images: Some(Vec::new()),
            latest_audios: Some(Vec::new()),
            ..ChatPromptOverrides::default()
        }),
        Some(state),
        Some(selected_api),
        Some(resolved_api),
        None,
    );
    prepared.latest_images.clear();
    prepared.latest_audios.clear();
    let timeout_secs = 360u64;
    let archive_summary_execution = call_archive_summary_model_with_timeout(
        state,
        resolved_api,
        selected_api,
        prepared,
        timeout_secs,
    )
    .await;
    push_model_call_log_parts(Some(state), &archive_summary_execution);
    let reply = archive_summary_execution.result?;
    let parsed = parse_memory_curation_draft(&reply.assistant_text).ok_or_else(|| {
        format!(
            "SummaryContext JSON 解析失败，raw={}",
            reply.assistant_text.chars().take(240).collect::<String>()
        )
    })?;
    let summary = clean_text(parsed.summary.trim());
    if summary.is_empty() {
        return Err("SummaryContext summary is empty".to_string());
    }
    let id_alias_map = memory_curation_id_alias_map(memories);
    Ok(MemoryCurationDraft {
        summary,
        useful_memory_ids: resolve_memory_curation_ids(&parsed.useful_memory_ids, &id_alias_map),
        new_memories: parsed.new_memories.into_iter().take(7).collect::<Vec<_>>(),
        merge_groups: parsed
            .merge_groups
            .into_iter()
            .take(7)
            .map(|group| ArchiveMergeGroupDraft {
                source_ids: resolve_memory_curation_ids(&group.source_ids, &id_alias_map),
                target: group.target,
            })
            .collect::<Vec<_>>(),
        profile_memories: resolve_profile_memory_drafts(
            &parsed
                .profile_memories
                .into_iter()
                .take(7)
                .collect::<Vec<_>>(),
            &id_alias_map,
        ),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SummaryContextScene {
    Compaction,
    Archive,
}

fn build_summary_context_requirement_block(scene: SummaryContextScene) -> String {
    let body = match scene {
        SummaryContextScene::Compaction => {
            "你正在执行一次“上下文检查点压缩（Context Checkpoint Compaction）”。\n\
             请为另一个将继续当前任务的语言模型生成一份交接摘要。\n\
             你的工具都已经被禁用，你只能生成 JSON 完成任务。\n\
             请包含以下内容：\n\
             - 当前进展，以及已经做出的关键决策\n\
             - 重要上下文、约束条件、或用户偏好\n\
             - 剩余待办事项（给出清晰的下一步）\n\
             - 为继续工作所需的关键数据、示例或引用\n\
             请保持内容简洁、结构化，并专注于帮助下一个语言模型无缝继续当前工作。"
                .to_string()
        }
        SummaryContextScene::Archive => {
            "你现在正在执行一次正式归档。\n\
             你的工具都已经被禁用，你只能生成 JSON 完成任务。\n\
             请不要复述完整过程，而是输出一份面向后续回看的结论汇报，核心回答：我们最终得出了什么结论。\n\
             summary 必须包含：\n\
             - 本轮最终结论与明确产出\n\
             - 已确认的关键决定、限制条件与责任分工\n\
             - 如果仍有遗留项，只保留确实影响后续工作的部分\n\
             请以可直接投放、可直接回看的完整汇报口吻输出，重点是结论而不是过程流水账。"
                .to_string()
        }
    };
    prompt_xml_block("summary_requirement", body)
}

fn build_summary_context_memory_block(
    agent: &AgentProfile,
    user_alias: &str,
    current_user_profile: &str,
) -> String {
    let instruction = build_memory_generation_instruction(agent, user_alias);
    prompt_xml_block(
        "memory_curation_context",
        format!(
            "{}\n\n【当前完整用户画像（带ID）】\n{}",
            instruction, current_user_profile
        ),
    )
}

fn build_summary_context_json_contract_block(scene: SummaryContextScene) -> String {
    let summary_rule = match scene {
        SummaryContextScene::Compaction => {
            "summary 表示本次上下文检查点压缩的交接摘要，必须方便下一个模型继续当前任务直接使用。"
        }
        SummaryContextScene::Archive => {
            "summary 表示本次会话归档结论汇报，必须能够让后续阅读者直接知道这轮最终得出了什么结论。"
        }
    };
    prompt_xml_block(
        "json_contract",
        format!(
            "你必须输出合法 JSON，且只能包含以下五个字段：summary/usefulMemoryIds/newMemories/mergeGroups/profileMemories。\n\
             不得输出 markdown、代码块、解释性前后缀。\n\
             {}\n\
             下面是唯一合法的 JSON 形状示例：\n{}",
            summary_rule,
            memory_curation_example_output_block()
        ),
    )
}

fn archive_pipeline_message_plain_text(message: &ChatMessage) -> String {
    let mut blocks = Vec::<String>::new();
    for part in &message.parts {
        if let MessagePart::Text { text } = part {
            let cleaned = clean_text(text.trim());
            if !cleaned.is_empty() {
                blocks.push(cleaned);
            }
        }
    }
    for block in &message.extra_text_blocks {
        let cleaned = clean_text(block.trim());
        if !cleaned.is_empty() {
            blocks.push(cleaned);
        }
    }
    clean_text(blocks.join("\n").trim())
}

fn archive_pipeline_recent_user_assistant_messages(
    source: &Conversation,
    max_messages: usize,
) -> Vec<(String, String)> {
    let mut recent = Vec::<(String, String)>::new();
    for message in source.messages.iter().rev() {
        let role = message.role.trim();
        if role != "user" && role != "assistant" {
            continue;
        }
        if archive_pipeline_is_context_compaction_message(message) {
            continue;
        }
        let text = archive_pipeline_message_plain_text(message);
        if text.is_empty() {
            continue;
        }
        recent.push((role.to_string(), text));
        if recent.len() >= max_messages {
            break;
        }
    }
    recent.reverse();
    recent
}

fn archive_pipeline_recent_user_assistant_messages_by_token_budget(
    source: &Conversation,
    max_tokens: usize,
) -> Vec<(String, String)> {
    if max_tokens == 0 {
        return Vec::new();
    }
    let mut recent = Vec::<(String, String)>::new();
    let mut consumed_tokens = 0.0f64;
    let token_budget = max_tokens as f64;
    for message in source.messages.iter().rev() {
        let role = message.role.trim();
        if role != "user" && role != "assistant" {
            continue;
        }
        if archive_pipeline_is_context_compaction_message(message) {
            continue;
        }
        let text = archive_pipeline_message_plain_text(message);
        if text.is_empty() {
            continue;
        }
        let next_tokens = estimated_tokens_for_text(&text);
        if !recent.is_empty() && consumed_tokens + next_tokens > token_budget {
            break;
        }
        consumed_tokens += next_tokens;
        recent.push((role.to_string(), text));
        if consumed_tokens >= token_budget {
            break;
        }
    }
    recent.reverse();
    recent
}

fn archive_pipeline_is_context_compaction_message(message: &ChatMessage) -> bool {
    if message.role.trim() != "user" {
        return false;
    }
    matches!(
        message
            .provider_meta
            .as_ref()
            .and_then(|meta| meta.get("message_meta"))
            .and_then(|meta| meta.get("kind"))
            .and_then(Value::as_str)
            .map(str::trim),
        Some("context_compaction") | Some("summary_context_seed")
    )
}

fn archive_pipeline_dedup_recall_table(recall_table: &[String]) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut deduped = Vec::<String>::new();
    for id in recall_table
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
    {
        if seen.insert(id.to_string()) {
            deduped.push(id.to_string());
        }
    }
    deduped
}

fn memory_curation_id_alias_map(memories: &[MemoryEntry]) -> HashMap<String, String> {
    let mut map = HashMap::<String, String>::new();
    for memory in memories {
        let canonical_id = memory.id.trim();
        if canonical_id.is_empty() {
            continue;
        }
        map.insert(canonical_id.to_string(), canonical_id.to_string());
        let display_id = memory.display_id();
        let short_id = display_id.trim();
        if !short_id.is_empty() {
            map.insert(short_id.to_string(), canonical_id.to_string());
        }
    }
    map
}

fn resolve_memory_curation_ids(
    items: &[String],
    id_alias_map: &HashMap<String, String>,
) -> Vec<String> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::<String>::new();
    for raw in items {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let resolved = id_alias_map
            .get(trimmed)
            .cloned()
            .unwrap_or_else(|| trimmed.to_string());
        if seen.insert(resolved.clone()) {
            out.push(resolved);
        }
    }
    out
}

fn resolve_profile_memory_drafts(
    drafts: &[ArchiveProfileMemoryDraft],
    id_alias_map: &HashMap<String, String>,
) -> Vec<ArchiveProfileMemoryDraft> {
    drafts
        .iter()
        .map(|item| ArchiveProfileMemoryDraft {
            memory_id: item
                .memory_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| {
                    id_alias_map
                        .get(value)
                        .cloned()
                        .unwrap_or_else(|| value.to_string())
                }),
            memory: item.memory.clone(),
        })
        .collect::<Vec<_>>()
}

fn build_archive_summary_from_last_three_rounds(source: &Conversation) -> String {
    let recent = archive_pipeline_recent_user_assistant_messages(source, 6);
    if recent.is_empty() {
        return "归档降级摘要：最近对话暂无可用正文。".to_string();
    }
    let mut lines = Vec::<String>::new();
    lines.push("归档降级摘要（最后三轮正文对话）：".to_string());
    for (idx, (role, text)) in recent.iter().enumerate() {
        let speaker = if role == "user" { "用户" } else { "助理" };
        let snippet = text.chars().take(240).collect::<String>();
        lines.push(format!("{}. {}：{}", idx + 1, speaker, snippet));
    }
    lines.join("\n")
}

fn emit_archive_history_flushed_event(
    state: &AppState,
    source_conversation_id: &str,
    active_conversation_id: &str,
    archive_id: &str,
    archive_reason: &str,
) {
    let app_handle = match state.app_handle.lock().ok().and_then(|guard| guard.clone()) {
        Some(handle) => handle,
        None => {
            eprintln!(
                "[ARCHIVE-PIPELINE] history_flushed emit skipped: app_handle unavailable, source_conversation_id={}, active_conversation_id={}",
                source_conversation_id, active_conversation_id
            );
            return;
        }
    };
    let payload = serde_json::json!({
        "conversationId": active_conversation_id,
        "messageCount": 0,
        "messages": [],
        "activateAssistant": false,
        "archiveApplied": true,
        "archiveId": archive_id,
        "archiveReason": archive_reason,
        "sourceConversationId": source_conversation_id,
    });
    if let Err(err) = app_handle.emit(CHAT_HISTORY_FLUSHED_EVENT, payload) {
        eprintln!(
            "[ARCHIVE-PIPELINE] history_flushed emit failed: source_conversation_id={}, active_conversation_id={}, archive_id={}, error={}",
            source_conversation_id, active_conversation_id, archive_id, err
        );
    } else {
        eprintln!(
            "[ARCHIVE-PIPELINE] history_flushed emitted: source_conversation_id={}, active_conversation_id={}, archive_id={}",
            source_conversation_id, active_conversation_id, archive_id
        );
    }
    if let Err(err) = emit_unarchived_conversation_overview_updated_from_state(state) {
        eprintln!(
            "[会话概览] archive_history_flushed 后推送失败: source_conversation_id={}, error={}",
            source_conversation_id, err
        );
    }
}

fn emit_compaction_history_flushed_event(
    state: &AppState,
    conversation_id: &str,
    compression_message: &ChatMessage,
) {
    let app_handle = match state.app_handle.lock().ok().and_then(|guard| guard.clone()) {
        Some(handle) => handle,
        None => {
            eprintln!(
                "[ARCHIVE-PIPELINE] 上下文整理 history_flushed 发送跳过: app_handle 不可用, conversation_id={}",
                conversation_id
            );
            return;
        }
    };
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "messageCount": 1,
        "messages": [compression_message],
        "activateAssistant": false,
        "compactionApplied": true,
    });
    if let Err(err) = app_handle.emit(CHAT_HISTORY_FLUSHED_EVENT, payload) {
        eprintln!(
            "[ARCHIVE-PIPELINE] 上下文整理 history_flushed 发送失败: conversation_id={}, error={}",
            conversation_id, err
        );
    } else {
        eprintln!(
            "[ARCHIVE-PIPELINE] 上下文整理 history_flushed 已发送: conversation_id={}",
            conversation_id
        );
    }
    if let Err(err) = emit_unarchived_conversation_overview_updated_from_state(state) {
        eprintln!(
            "[会话概览] compaction_history_flushed 后推送失败: conversation_id={}, error={}",
            conversation_id, err
        );
    }
}

fn emit_deleted_history_flushed_event(
    state: &AppState,
    deleted_conversation_id: &str,
    active_conversation_id: &str,
    delete_reason: &str,
) {
    let app_handle = match state.app_handle.lock().ok().and_then(|guard| guard.clone()) {
        Some(handle) => handle,
        None => {
            eprintln!(
                "[ARCHIVE-PIPELINE] 删除 history_flushed 发送跳过: app_handle 不可用, deleted_conversation_id={}, active_conversation_id={}",
                deleted_conversation_id, active_conversation_id
            );
            return;
        }
    };
    let payload = serde_json::json!({
        "conversationId": active_conversation_id,
        "messageCount": 0,
        "messages": [],
        "activateAssistant": false,
        "archiveApplied": false,
        "deletedConversationId": deleted_conversation_id,
        "deleteReason": delete_reason,
    });
    if let Err(err) = app_handle.emit(CHAT_HISTORY_FLUSHED_EVENT, payload) {
        eprintln!(
            "[ARCHIVE-PIPELINE] 删除 history_flushed 发送失败: deleted_conversation_id={}, active_conversation_id={}, error={}",
            deleted_conversation_id, active_conversation_id, err
        );
    } else {
        eprintln!(
            "[ARCHIVE-PIPELINE] 删除 history_flushed 已发送: deleted_conversation_id={}, active_conversation_id={}",
            deleted_conversation_id, active_conversation_id
        );
    }
}

fn delete_main_conversation_and_activate_latest(
    state: &AppState,
    selected_api: &ApiConfig,
    source: &Conversation,
) -> Result<String, String> {
    conversation_service().delete_main_conversation_and_activate_latest(state, selected_api, source)
}

fn build_compaction_message(
    summary: &str,
    compaction_reason: &str,
    user_profile_snapshot: Option<&str>,
    current_todos: Option<&[ConversationTodoItem]>,
    preserved_dialogue: Option<&str>,
) -> ChatMessage {
    let now = now_iso();
    let reason = compaction_reason.trim();
    let profile_snapshot = user_profile_snapshot
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(clean_text)
        .unwrap_or_else(|| "（暂无用户画像）".to_string());
    let todo_snapshot = current_todos
        .and_then(todo_markdown_block)
        .map(|value| clean_text(&value))
        .unwrap_or_default();
    let user_profile_block = if todo_snapshot.is_empty() {
        profile_snapshot
    } else {
        format!("{}\n\n{}", profile_snapshot, todo_snapshot)
    };
    let summary_note = if reason.is_empty() {
        "以下内容为当前会话中较早历史对话的整理结果。\n\
         为保证连续性，后文按约 10K token 的预算保留了最近的原始对话，不包含在本段摘要中。\n\
         摘要中的助手发言统一使用当前人格昵称表示。"
            .to_string()
    } else {
        "以下内容为当前会话中较早历史对话的整理结果。\n\
         为保证连续性，后文按约 10K token 的预算保留了最近的原始对话，不包含在本段摘要中。\n\
         摘要中的助手发言统一使用当前人格昵称表示。"
            .to_string()
    };
    let preserved_dialogue_text = preserved_dialogue
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_multiline_block)
        .unwrap_or_else(|| "（暂无保留对话）".to_string());
    let text = format!(
        "[上下文整理]\n\n用户画像：\n{}\n\n摘要说明：\n{}\n\n摘要正文：\n{}\n\n保留对话：\n{}",
        user_profile_block,
        clean_text(summary_note.trim()),
        clean_compaction_summary_text(summary),
        preserved_dialogue_text
    );
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now,
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text { text }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "message_meta": {
                "kind": "context_compaction",
                "scene": "compaction",
                "reason": reason,
            }
        })),
        tool_call: None,
        mcp_call: None,
    }
}

fn normalize_multiline_block(input: &str) -> String {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn clean_compaction_summary_text(input: &str) -> String {
    let trimmed = input.trim();
    if let Some((summary, active_plans)) = trimmed.split_once("<active_plans>") {
        let cleaned_summary = clean_text(summary.trim());
        let cleaned_active_plans = normalize_multiline_block(&format!("<active_plans>{active_plans}"));
        if cleaned_summary.is_empty() {
            return cleaned_active_plans;
        }
        return format!("{}\n\n{}", cleaned_summary, cleaned_active_plans);
    }
    clean_text(trimmed)
}

fn build_initial_summary_context_message(
    last_archive_summary: Option<&str>,
    user_profile_snapshot: Option<&str>,
    current_todos: Option<&[ConversationTodoItem]>,
) -> ChatMessage {
    let summary = last_archive_summary
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("（暂无历史归档摘要）");
    let mut message =
        build_compaction_message(summary, "", user_profile_snapshot, current_todos, None);
    message.provider_meta = Some(serde_json::json!({
        "message_meta": {
            "kind": "summary_context_seed",
            "scene": "seed",
        }
    }));
    message
}

#[derive(Debug, Clone, Default)]
struct SummaryContextApplyReport {
    merged_memories: usize,
    merged_groups: usize,
    linked_profile_memories: usize,
    created_profile_memories: usize,
    skipped_profile_memories: usize,
    memory_feedback: MemoryArchiveFeedbackReport,
}

fn apply_summary_context_result(
    data_path: &PathBuf,
    host_agent: &AgentProfile,
    recall_ids: &[String],
    draft: &MemoryCurationDraft,
) -> Result<SummaryContextApplyReport, String> {
    let owner_agent_id = if host_agent.private_memory_enabled && !host_agent.is_built_in_user {
        Some(host_agent.id.as_str())
    } else {
        None
    };
    let memory_feedback =
        memory_store_apply_archive_feedback(data_path, recall_ids, &draft.useful_memory_ids)?;
    let merged_memories =
        merge_memories_into_store(data_path, &draft.new_memories, owner_agent_id)?;
    let merged_groups =
        merge_memory_groups_into_store(data_path, &draft.merge_groups, owner_agent_id)?;
    let (linked_profile_memories, created_profile_memories, skipped_profile_memories) =
        apply_profile_memories_into_store(data_path, &draft.profile_memories, owner_agent_id)?;
    Ok(SummaryContextApplyReport {
        merged_memories,
        merged_groups,
        linked_profile_memories,
        created_profile_memories,
        skipped_profile_memories,
        memory_feedback,
    })
}

fn build_compaction_preserved_dialogue_block(
    source: &Conversation,
    user_alias: &str,
    assistant_name: &str,
    max_tokens: usize,
) -> String {
    archive_pipeline_recent_user_assistant_messages_by_token_budget(source, max_tokens)
        .into_iter()
        .map(|(role, text)| {
            let speaker = if role.eq_ignore_ascii_case("assistant") {
                assistant_name.trim()
            } else {
                user_alias.trim()
            };
            let speaker = if speaker.is_empty() {
                if role.eq_ignore_ascii_case("assistant") {
                    "助手"
                } else {
                    "用户"
                }
            } else {
                speaker
            };
            format!("{}：{}", speaker, text)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn summarize_archive_summary_with_fallback(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    host_agent: &AgentProfile,
    user_alias: &str,
    reporting_source: &Conversation,
    memories: &[MemoryEntry],
) -> (MemoryCurationDraft, Option<String>) {
    let deduped_recall = archive_pipeline_dedup_recall_table(&reporting_source.memory_recall_table);
    match summarize_archived_conversation_with_model_v2(
        state,
        resolved_api,
        selected_api,
        host_agent,
        user_alias,
        reporting_source,
        SummaryContextScene::Archive,
        memories,
        &deduped_recall,
    )
    .await
    {
        Ok(draft) => (draft, None),
        Err(err) => (
            MemoryCurationDraft {
                summary: build_archive_summary_from_last_three_rounds(reporting_source),
                useful_memory_ids: Vec::new(),
                new_memories: Vec::new(),
                merge_groups: Vec::new(),
                profile_memories: Vec::new(),
            },
            Some(format!(
                "SummaryContext 归档失败，已使用最后三轮正文对话降级摘要：{}",
                err
            )),
        ),
    }
}

async fn summarize_compaction_with_fallback(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    host_agent: &AgentProfile,
    user_alias: &str,
    source: &Conversation,
    trace_id: &str,
) -> (MemoryCurationDraft, Option<String>) {
    const MAX_ATTEMPTS: usize = 3;
    const RETRY_DELAY_SECS: u64 = 5;

    let mut last_err = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        let visible_memories = match memory_store_list_memories_visible_for_agent(
            &state.data_path,
            &host_agent.id,
            host_agent.private_memory_enabled,
        ) {
            Ok(items) => items,
            Err(err) => {
                return (
                    MemoryCurationDraft {
                        summary: String::new(),
                        useful_memory_ids: Vec::new(),
                        new_memories: Vec::new(),
                        merge_groups: Vec::new(),
                        profile_memories: Vec::new(),
                    },
                    Some(format!(
                        "SummaryContext 读取可见记忆失败，压缩摘要留空：{}",
                        err
                    )),
                )
            }
        };
        let deduped_recall = archive_pipeline_dedup_recall_table(&source.memory_recall_table);
        match summarize_archived_conversation_with_model_v2(
            state,
            resolved_api,
            selected_api,
            host_agent,
            user_alias,
            source,
            SummaryContextScene::Compaction,
            &visible_memories,
            &deduped_recall,
        )
        .await
        {
            Ok(summary) => return (summary, None),
            Err(err) => {
                last_err = err;
                if attempt < MAX_ATTEMPTS {
                    eprintln!(
                        "[ARCHIVE-PIPELINE] 上下文整理模型跳过: stage=retry_after_error, trace_id={}, conversation_id={}, attempt={}，next_retry_secs={}，error={}",
                        trace_id,
                        source.id,
                        attempt,
                        RETRY_DELAY_SECS,
                        last_err
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
                }
            }
        }
    }

    eprintln!(
        "[SummaryContext] 上下文整理失败，压缩摘要留空继续主流程: trace_id={}, conversation_id={}, attempts={}, err={}",
        trace_id, source.id, MAX_ATTEMPTS, last_err
    );
    (
        MemoryCurationDraft {
            summary: String::new(),
            useful_memory_ids: Vec::new(),
            new_memories: Vec::new(),
            merge_groups: Vec::new(),
            profile_memories: Vec::new(),
        },
        Some(format!(
            "SummaryContext 上下文整理失败（已重试{}次），压缩摘要留空：{}",
            MAX_ATTEMPTS, last_err
        )),
    )
}

#[tauri::command]
async fn force_archive_current(
    input: ForceArchiveCurrentInput,
    state: State<'_, AppState>,
) -> Result<ForceArchiveResult, String> {
    let (selected_api, resolved_api, source, effective_agent_id) =
        resolve_archive_target_conversation(state.inner(), &input.session)?;
    let runtime = state_read_runtime_state_cached(state.inner())?;
    let main_conversation_id = runtime
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    if source.id.trim() == main_conversation_id {
        return Err("主会话暂不支持归档。".to_string());
    }
    if get_conversation_runtime_state(state.inner(), &source.id)?
        == MainSessionState::OrganizingContext
    {
        return Err("强制归档正在进行中，请稍候。".to_string());
    }
    let active_conversation_id =
        prepare_background_archive_active_conversation(state.inner(), &selected_api, &source)?;

    let state_cloned = state.inner().clone();
    let selected_api_cloned = selected_api.clone();
    let resolved_api_cloned = resolved_api.clone();
    let source_cloned = source.clone();
    let effective_agent_id_cloned = effective_agent_id.clone();
    let active_conversation_id_for_background = active_conversation_id.clone();
    let target_conversation_id = input
        .target_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    tauri::async_runtime::spawn(async move {
        let panic_safe_task = std::panic::AssertUnwindSafe(async {
            let result = run_archive_pipeline(
                &state_cloned,
                &selected_api_cloned,
                &resolved_api_cloned,
                &source_cloned,
                &effective_agent_id_cloned,
                Some(active_conversation_id_for_background.as_str()),
                target_conversation_id.as_deref(),
                "manual_force_archive",
                "ARCHIVE-FORCE",
            )
            .await;
            if let Err(err) = result {
                eprintln!(
                    "[ARCHIVE-FORCE] 失败，任务=background_force_archive，conversation_id={}，error={}",
                    source_cloned.id, err
                );
            }
            trigger_chat_queue_processing(&state_cloned);
        });
        if futures_util::FutureExt::catch_unwind(panic_safe_task)
            .await
            .is_err()
        {
            eprintln!(
                "[ARCHIVE-FORCE] 失败，任务=background_force_archive，conversation_id={}，error=panic",
                source_cloned.id
            );
            if let Err(err) = set_conversation_runtime_state(
                &state_cloned,
                &source_cloned.id,
                MainSessionState::Idle,
            ) {
                eprintln!(
                    "[ARCHIVE-FORCE] 警告，任务=background_force_archive_reset_state，conversation_id={}，error={}",
                    source_cloned.id, err
                );
            }
            trigger_chat_queue_processing(&state_cloned);
        }
    });

    Ok(ForceArchiveResult {
        archived: false,
        archive_id: None,
        active_conversation_id: Some(active_conversation_id),
        compaction_message: None,
        summary: String::new(),
        merged_memories: 0,
        warning: None,
        reason_code: Some("background_started".to_string()),
        elapsed_ms: None,
        memory_feedback: None,
        merge_groups: Some(0),
    })
}

#[tauri::command]
async fn force_compact_current(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ForceArchiveResult, String> {
    let (selected_api, resolved_api, source, effective_agent_id) =
        resolve_archive_target_conversation(state.inner(), &input)?;
    let preview = build_force_compaction_preview_result(state.inner(), &selected_api, &source)?;
    if !preview.can_compact {
        return Err(preview
            .compaction_disabled_reason
            .unwrap_or_else(|| "当前会话暂时不能压缩。".to_string()));
    }
    let result = run_context_compaction_pipeline(
        state.inner(),
        &selected_api,
        &resolved_api,
        &source,
        &effective_agent_id,
        "manual_force_compaction",
        "COMPACTION-FORCE",
    )
    .await?;
    trigger_chat_queue_processing(state.inner());
    Ok(result)
}

#[tauri::command]
fn preview_force_archive_current(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ForceArchivePreviewResult, String> {
    let (_selected_api, _resolved_api, source, _effective_agent_id) =
        resolve_archive_target_conversation(state.inner(), &input)?;
    let runtime = state_read_runtime_state_cached(state.inner())?;
    let main_conversation_id = runtime
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    let is_main_conversation = source.id.trim() == main_conversation_id;
    let message_count = archive_pipeline_message_count_for_delete(&source);
    let has_assistant_reply = archive_pipeline_has_assistant_reply(&source);
    let is_empty = source.messages.is_empty();
    let archive_disabled_reason = if is_main_conversation {
        Some("主会话暂不支持归档。".to_string())
    } else if get_conversation_runtime_state(state.inner(), &source.id)?
        == MainSessionState::OrganizingContext
    {
        Some("当前会话正在后台归档或整理上下文，请稍候。".to_string())
    } else if is_empty {
        Some("当前会话为空，不能归档。".to_string())
    } else if !has_assistant_reply {
        Some("当前会话还没有助理回复，不能归档。".to_string())
    } else if message_count <= SHORT_CONVERSATION_DELETE_THRESHOLD {
        Some(format!(
            "当前会话过短（仅 {} 条用户/助理消息），暂不建议归档。",
            message_count
        ))
    } else {
        None
    };
    Ok(ForceArchivePreviewResult {
        conversation_id: source.id,
        can_archive: archive_disabled_reason.is_none(),
        can_drop_conversation: !is_main_conversation,
        message_count,
        has_assistant_reply,
        is_empty,
        archive_disabled_reason,
    })
}

#[tauri::command]
fn preview_force_compact_current(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ForceCompactionPreviewResult, String> {
    let (selected_api, _resolved_api, source, _effective_agent_id) =
        resolve_archive_target_conversation(state.inner(), &input)?;
    build_force_compaction_preview_result(state.inner(), &selected_api, &source)
}

pub(crate) async fn run_archive_pipeline(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    effective_agent_id: &str,
    prepared_active_conversation_id: Option<&str>,
    target_conversation_id: Option<&str>,
    archive_reason: &str,
    trace_tag: &str,
) -> Result<ForceArchiveResult, String> {
    let started_at = std::time::Instant::now();
    let trace_id = Uuid::new_v4().to_string();

    // 设置状态为 OrganizingContext（仅影响所属会话）
    set_conversation_runtime_state(state, &source.id, MainSessionState::OrganizingContext)?;
    eprintln!(
        "[ARCHIVE-PIPELINE] 开始: task=archive_pipeline, trace_id={}, agent_id={}, api_id={}, started_at={}",
        trace_id, effective_agent_id, selected_api.id, started_at.elapsed().as_millis()
    );

    // 确保在所有退出路径都恢复状态
    let result = run_archive_pipeline_inner(
        state,
        selected_api,
        resolved_api,
        source,
        effective_agent_id,
        prepared_active_conversation_id,
        target_conversation_id,
        archive_reason,
        trace_tag,
        started_at,
        &trace_id,
    )
    .await;

    // 归档完成，切换回 Idle（即使内部失败也要恢复状态）
    let elapsed_ms = started_at.elapsed().as_millis();
    if let Err(state_err) =
        set_conversation_runtime_state(state, &source.id, MainSessionState::Idle)
    {
        eprintln!(
            "[ARCHIVE-PIPELINE] 警告: 状态恢复失败, trace_id={}, elapsed_ms={}, error={}",
            trace_id, elapsed_ms, state_err
        );
    } else {
        eprintln!(
            "[ARCHIVE-PIPELINE] 完成: task=archive_pipeline, trace_id={}, agent_id={}, api_id={}, elapsed_ms={}",
            trace_id, effective_agent_id, selected_api.id, elapsed_ms
        );
    }

    // 注意：不在这里触发 process_chat_queue，由调用方负责

    result
}

pub(crate) async fn run_context_compaction_pipeline(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    effective_agent_id: &str,
    compaction_reason: &str,
    trace_tag: &str,
) -> Result<ForceArchiveResult, String> {
    let started_at = std::time::Instant::now();
    let trace_id = Uuid::new_v4().to_string();

    set_conversation_runtime_state(state, &source.id, MainSessionState::OrganizingContext)?;
    eprintln!(
        "[ARCHIVE-PIPELINE] 开始: task=context_compaction, trace_id={}, agent_id={}, api_id={}, started_at={}",
        trace_id, effective_agent_id, selected_api.id, started_at.elapsed().as_millis()
    );

    let result = run_context_compaction_pipeline_inner(
        state,
        selected_api,
        resolved_api,
        source,
        effective_agent_id,
        compaction_reason,
        trace_tag,
        started_at,
        &trace_id,
    )
    .await;

    let elapsed_ms = started_at.elapsed().as_millis();
    if let Err(state_err) =
        set_conversation_runtime_state(state, &source.id, MainSessionState::Idle)
    {
        eprintln!(
            "[ARCHIVE-PIPELINE] 警告: 状态恢复失败, trace_id={}, elapsed_ms={}, error={}",
            trace_id, elapsed_ms, state_err
        );
    } else {
        eprintln!(
            "[ARCHIVE-PIPELINE] 完成: task=context_compaction, trace_id={}, agent_id={}, api_id={}, elapsed_ms={}",
            trace_id, effective_agent_id, selected_api.id, elapsed_ms
        );
    }

    result
}

async fn run_context_compaction_pipeline_inner(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    _effective_agent_id: &str,
    compaction_reason: &str,
    trace_tag: &str,
    started_at: std::time::Instant,
    trace_id: &str,
) -> Result<ForceArchiveResult, String> {
    if source.messages.is_empty() {
        return Ok(ForceArchiveResult {
            archived: false,
            archive_id: None,
            active_conversation_id: Some(source.id.clone()),
            compaction_message: None,
            summary: "当前对话为空，无需整理。".to_string(),
            merged_memories: 0,
            warning: None,
            reason_code: Some("empty_conversation".to_string()),
            elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            memory_feedback: None,
            merge_groups: None,
        });
    }

    if !archive_pipeline_has_assistant_reply(source) {
        let active_conversation_id =
            delete_main_conversation_and_activate_latest(state, selected_api, source)?;
        emit_deleted_history_flushed_event(
            state,
            &source.id,
            &active_conversation_id,
            "no_assistant_reply_deleted",
        );
        eprintln!(
            "[ARCHIVE-PIPELINE] 整理前直接删除：conversation_id={}, reason=no_assistant_reply_deleted, next_conversation_id={}",
            source.id, active_conversation_id
        );
        return Ok(ForceArchiveResult {
            archived: false,
            archive_id: None,
            active_conversation_id: Some(active_conversation_id),
            compaction_message: None,
            summary: String::new(),
            merged_memories: 0,
            warning: None,
            reason_code: Some("no_assistant_reply_deleted".to_string()),
            elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            memory_feedback: None,
            merge_groups: None,
        });
    }

    let (owner_agent, owner_agent_id, user_alias) = resolve_archive_owner_context(state, source)?;

    eprintln!(
        "[{}] trace={} begin api={} model={} format={} conversation={} ownerAgent={}",
        trace_tag,
        trace_id,
        selected_api.id,
        selected_api.model,
        resolved_api.request_format,
        source.id,
        owner_agent_id
    );

    let (summary_draft, compaction_warning) = summarize_compaction_with_fallback(
        state,
        selected_api,
        resolved_api,
        &owner_agent,
        &user_alias,
        source,
        trace_id,
    )
    .await;
    let deduped_recall = archive_pipeline_dedup_recall_table(&source.memory_recall_table);
    let applied_report = apply_summary_context_result(
        &state.data_path,
        &owner_agent,
        &deduped_recall,
        &summary_draft,
    )?;
    let summary_with_pending_plan = match message_store::active_plan_prompt_block(
        &state.data_path,
        &source.id,
    )? {
        Some(plan_block) if summary_draft.summary.trim().is_empty() => {
            format!("\n{}", plan_block.trim())
        }
        Some(plan_block) => format!("{}\n\n{}", summary_draft.summary.trim(), plan_block.trim()),
        None => summary_draft.summary.clone(),
    };
    let user_profile_snapshot =
        if conversation_is_delegate(source) || conversation_is_remote_im_contact(source) {
            None
        } else {
            build_user_profile_snapshot_block(&state.data_path, &owner_agent, 12)?
        };

    let compression_message = build_compaction_message(
        &summary_with_pending_plan,
        compaction_reason,
        user_profile_snapshot.as_deref(),
        Some(&source.current_todos),
        Some(&build_compaction_preserved_dialogue_block(
            source,
            &user_alias,
            &owner_agent.name,
            10_000,
        )),
    );
    let persist_result = conversation_service().persist_compaction_message(
        state,
        source,
        &compression_message,
        user_profile_snapshot.clone(),
    )?;
    let active_conversation_id = persist_result.active_conversation_id;
    let compression_message_id = persist_result.compression_message_id;
    eprintln!(
        "[ARCHIVE-PIPELINE] 上下文整理消息写入校验通过: conversation_id={}, message_id={}",
        source.id, compression_message_id
    );
    match clear_apply_patch_temp(&state.data_path) {
        Ok((record_count, blob_count)) => {
            eprintln!(
                "[apply_patch缓存] 完成，任务=clear_temp_on_compaction，conversation_id={}，记录条数={}，备份条数={}",
                source.id, record_count, blob_count
            );
        }
        Err(err) => {
            eprintln!(
                "[apply_patch缓存] 失败，任务=clear_temp_on_compaction，conversation_id={}，error={}",
                source.id, err
            );
        }
    }
    emit_compaction_history_flushed_event(state, &source.id, &compression_message);

    eprintln!(
        "[SummaryContext] 完成，场景=compaction，trace_id={}，conversation_id={}，merged_memories={}，merged_groups={}，profile_linked={}，profile_created={}，profile_skipped={}，useful_accept={}，penalized={}，natural_decay={}",
        trace_id,
        source.id,
        applied_report.merged_memories,
        applied_report.merged_groups,
        applied_report.linked_profile_memories,
        applied_report.created_profile_memories,
        applied_report.skipped_profile_memories,
        applied_report.memory_feedback.useful_accepted_count,
        applied_report.memory_feedback.penalized_count,
        applied_report.memory_feedback.natural_decay_count
    );

    Ok(ForceArchiveResult {
        archived: false,
        archive_id: None,
        active_conversation_id,
        compaction_message: Some(compression_message),
        summary: summary_draft.summary,
        merged_memories: applied_report.merged_memories,
        warning: compaction_warning,
        reason_code: None,
        elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        memory_feedback: Some(applied_report.memory_feedback),
        merge_groups: Some(applied_report.merged_groups),
    })
}

async fn run_archive_pipeline_inner(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    _effective_agent_id: &str,
    prepared_active_conversation_id: Option<&str>,
    target_conversation_id: Option<&str>,
    archive_reason: &str,
    trace_tag: &str,
    started_at: std::time::Instant,
    trace_id: &str,
) -> Result<ForceArchiveResult, String> {
    let runtime = state_read_runtime_state_cached(state)?;
    let is_main_conversation = runtime
        .main_conversation_id
        .as_deref()
        .map(str::trim)
        == Some(source.id.as_str());

    if source.messages.is_empty() {
        if !is_main_conversation {
            mark_tasks_as_session_lost(&state.data_path, &source.id);
        }
        let active_conversation_id =
            delete_main_conversation_and_activate_latest(state, selected_api, source)?;
        emit_deleted_history_flushed_event(
            state,
            &source.id,
            &active_conversation_id,
            "empty_conversation_deleted",
        );
        return Ok(ForceArchiveResult {
            archived: false,
            archive_id: None,
            active_conversation_id: Some(active_conversation_id),
            compaction_message: None,
            summary: String::new(),
            merged_memories: 0,
            warning: None,
            reason_code: Some("empty_conversation_deleted".to_string()),
            elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            memory_feedback: None,
            merge_groups: None,
        });
    }

    if !archive_pipeline_has_assistant_reply(source) {
        if !is_main_conversation {
            mark_tasks_as_session_lost(&state.data_path, &source.id);
        }
        let active_conversation_id =
            delete_main_conversation_and_activate_latest(state, selected_api, source)?;
        emit_deleted_history_flushed_event(
            state,
            &source.id,
            &active_conversation_id,
            "no_assistant_reply_deleted",
        );
        eprintln!(
            "[ARCHIVE-PIPELINE] 无助理回复会话直接删除: conversation_id={}, next_conversation_id={}",
            source.id, active_conversation_id
        );
        return Ok(ForceArchiveResult {
            archived: false,
            archive_id: None,
            active_conversation_id: Some(active_conversation_id),
            compaction_message: None,
            summary: String::new(),
            merged_memories: 0,
            warning: None,
            reason_code: Some("no_assistant_reply_deleted".to_string()),
            elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            memory_feedback: None,
            merge_groups: None,
        });
    }

    let message_count = archive_pipeline_message_count_for_delete(source);
    if message_count <= SHORT_CONVERSATION_DELETE_THRESHOLD {
        if !is_main_conversation {
            mark_tasks_as_session_lost(&state.data_path, &source.id);
        }
        let active_conversation_id =
            delete_main_conversation_and_activate_latest(state, selected_api, source)?;
        emit_deleted_history_flushed_event(
            state,
            &source.id,
            &active_conversation_id,
            "short_conversation_deleted",
        );
        eprintln!(
            "[ARCHIVE-PIPELINE] 短对话直接删除: conversation_id={}, message_count={}, next_conversation_id={}",
            source.id, message_count, active_conversation_id
        );

        return Ok(ForceArchiveResult {
            archived: false,
            archive_id: None,
            active_conversation_id: Some(active_conversation_id),
            compaction_message: None,
            summary: String::new(),
            merged_memories: 0,
            warning: None,
            reason_code: Some("short_conversation_deleted".to_string()),
            elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            memory_feedback: None,
            merge_groups: None,
        });
    }

    let (owner_agent, owner_agent_id, user_alias) = resolve_archive_owner_context(state, source)?;
    let memories = memory_store_list_memories_visible_for_agent(
        &state.data_path,
        &owner_agent_id,
        owner_agent.private_memory_enabled,
    )?;

    eprintln!(
        "[{}] trace={} begin api={} model={} format={} conversation={} ownerAgent={}",
        trace_tag,
        trace_id,
        selected_api.id,
        selected_api.model,
        resolved_api.request_format,
        source.id,
        owner_agent_id
    );

    let reporting_source = build_archive_reporting_conversation(source);
    let (summary_draft, archive_warning_main) = summarize_archive_summary_with_fallback(
        state,
        resolved_api,
        selected_api,
        &owner_agent,
        &user_alias,
        reporting_source.as_ref(),
        &memories,
    )
    .await;
    let mut archive_warning = archive_warning_main;
    let deduped_recall = archive_pipeline_dedup_recall_table(&source.memory_recall_table);
    let applied_report = apply_summary_context_result(
        &state.data_path,
        &owner_agent,
        &deduped_recall,
        &summary_draft,
    )?;

    let mut archived_conversation = state_read_conversation_cached(state, &source.id)
        .map_err(|_| "归档前会话已变化，请重试归档。".to_string())?;
    if !archived_conversation.summary.trim().is_empty() {
        return Err("归档前会话已变化，请重试归档。".to_string());
    }
    let previous_status = archived_conversation.status.clone();
    let now = now_iso();
    archived_conversation.status = "archived".to_string();
    archived_conversation.summary = summary_draft.summary.clone();
    archived_conversation.archived_at = Some(now.clone());
    archived_conversation.updated_at = now;
    let archive_id = archived_conversation.id.clone();
    eprintln!(
        "[会话] 已归档: conversation_id={}, previous_status={}, reason=\"{}\", summary=\"{}\"",
        archived_conversation.id,
        previous_status,
        archive_reason,
        summary_draft.summary
    );
    clear_screenshot_artifact_cache();
    if !is_main_conversation {
        mark_tasks_as_session_lost(&state.data_path, &source.id);
    }
    let active_conversation_id = prepared_active_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            match conversation_service().resolve_latest_foreground_conversation_id(state, "") {
                Ok(value) => value,
                Err(err) => {
                    runtime_log_warn(format!(
                        "[归档] 警告，任务=resolve_latest_foreground_conversation_id_after_archive，source_conversation_id={}，error={}",
                        source.id, err
                    ));
                    None
                }
            }
        })
        .ok_or_else(|| "归档后未能确定当前前台会话。".to_string())?;
    state_schedule_conversation_persist(state, &archived_conversation, true)?;

    // 清理PDF缓存
    if let Err(e) = cleanup_pdf_cache_for_conversation(&state, &source.id) {
        eprintln!(
            "[归档] 清理 PDF 缓存失败: conversation={}, error={}",
            source.id, e
        );
    }

    emit_archive_history_flushed_event(
        state,
        &source.id,
        &active_conversation_id,
        &archive_id,
        archive_reason,
    );

    if let Some(target_conversation_id) = target_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Err(err) = append_archive_delivery_message_to_conversation(
            state,
            target_conversation_id,
            source,
            &archive_id,
            &summary_draft.summary,
        ) {
            runtime_log_warn(format!(
                "[归档投放] 警告，任务=append_archive_delivery_message，source_conversation_id={}，target_conversation_id={}，archive_id={}，error={}",
                source.id, target_conversation_id, archive_id, err
            ));
            let next_warning = format!("归档汇报投放失败：{}", err);
            archive_warning = Some(match archive_warning {
                Some(existing) if !existing.trim().is_empty() => {
                    format!("{}\n{}", existing, next_warning)
                }
                _ => next_warning,
            });
        }
    }

    eprintln!(
        "[SummaryContext] 完成，场景=archive，trace_id={}，conversation_id={}，merged_memories={}，merged_groups={}，profile_linked={}，profile_created={}，profile_skipped={}，useful_accept={}，penalized={}，natural_decay={}",
        trace_id,
        source.id,
        applied_report.merged_memories,
        applied_report.merged_groups,
        applied_report.linked_profile_memories,
        applied_report.created_profile_memories,
        applied_report.skipped_profile_memories,
        applied_report.memory_feedback.useful_accepted_count,
        applied_report.memory_feedback.penalized_count,
        applied_report.memory_feedback.natural_decay_count
    );

    Ok(ForceArchiveResult {
        archived: true,
        archive_id: Some(archive_id),
        active_conversation_id: Some(active_conversation_id),
        compaction_message: None,
        summary: summary_draft.summary,
        merged_memories: applied_report.merged_memories,
        warning: archive_warning,
        reason_code: None,
        elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        memory_feedback: Some(applied_report.memory_feedback),
        merge_groups: Some(applied_report.merged_groups),
    })
}

#[cfg(test)]
mod archive_pipeline_tests {
    use super::*;

    fn test_message(id: &str, role: &str, text: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: "2026-04-18T10:00:00Z".to_string(),
            speaker_agent_id: Some("agent-a".to_string()),
            parts: vec![MessagePart::Text {
                text: text.to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    fn test_conversation() -> Conversation {
        Conversation {
            id: "conversation-a".to_string(),
            title: "测试会话".to_string(),
            agent_id: "agent-a".to_string(),
            department_id: "dept-a".to_string(),
            bound_conversation_id: None,
            parent_conversation_id: Some("parent-a".to_string()),
            child_conversation_ids: Vec::new(),
            fork_message_cursor: Some("m2".to_string()),
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: "2026-04-18T10:00:00Z".to_string(),
            updated_at: "2026-04-18T10:03:00Z".to_string(),
            last_user_at: Some("2026-04-18T10:02:00Z".to_string()),
            last_assistant_at: Some("2026-04-18T10:03:00Z".to_string()),
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: vec![
                test_message("m1", "user", "前置问题"),
                test_message("m2", "assistant", "分叉点回答"),
                test_message("m3", "user", "分叉后的新问题"),
                test_message("m4", "assistant", "分叉后的最终结论"),
            ],
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        }
    }

    #[test]
    fn build_archive_reporting_conversation_should_only_keep_post_fork_messages() {
        let source = test_conversation();
        let reporting = build_archive_reporting_conversation(&source);
        let ids = reporting
            .messages
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["m3", "m4"]);
        assert_eq!(archive_report_scope_label(&source), "post_fork_discussion");
    }
}
