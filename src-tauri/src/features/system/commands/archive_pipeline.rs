fn mark_tasks_as_session_lost(data_path: &PathBuf, conversation_id: &str) {
    let Ok(tasks) = task_store_list_task_records(data_path) else {
        runtime_log_error(format!(
            "[TASK-CLEANUP] 查询任务列表失败: conversation_id={}",
            conversation_id
        ));
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
            runtime_log_error(format!(
                "[TASK-CLEANUP] 标记任务失败: task_id={}, conversation_id={}, error={}",
                task.task_id, conversation_id, err
            ));
        }
    }
}

#[derive(Debug, Clone, Default)]
struct PreparedArchiveMemoryDraft {
    input: Option<MemoryDraftInput>,
    is_profile: bool,
    skipped_profile: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct AppliedArchiveMemoryStats {
    merged_memories: usize,
    merged_groups: usize,
    applied_profile_memories: usize,
    skipped_profile_memories: usize,
}

fn archive_memory_draft_is_profile_candidate(tags: &[String]) -> bool {
    tags.iter().any(|tag| memory_tag_is_user_profile_category_tag(tag))
}

fn prepare_archive_memory_draft(
    draft: &ArchiveMemoryDraft,
    owner_agent_id: Option<&str>,
) -> PreparedArchiveMemoryDraft {
    let judgment = clean_text(draft.judgment.trim());
    if judgment.is_empty() {
        return PreparedArchiveMemoryDraft::default();
    }
    let tags = normalize_memory_keywords(&draft.tags);
    if tags.is_empty() {
        return PreparedArchiveMemoryDraft::default();
    }
    let is_profile_candidate = archive_memory_draft_is_profile_candidate(&tags);
    if is_profile_candidate {
        if tags.len() < 3 || !archive_profile_memory_type_allowed(&draft.memory_type)
        {
            return PreparedArchiveMemoryDraft {
                input: None,
                is_profile: false,
                skipped_profile: true,
            };
        }
    }
    PreparedArchiveMemoryDraft {
        input: Some(MemoryDraftInput {
            memory_type: draft.memory_type.clone(),
            judgment,
            reasoning: clean_text(draft.reasoning.trim()),
            tags,
            owner_agent_id: owner_agent_id
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned),
        }),
        is_profile: is_profile_candidate,
        skipped_profile: false,
    }
}

fn upsert_archive_memory_draft_with_ids(
    data_path: &PathBuf,
    draft: &ArchiveMemoryDraft,
    owner_agent_id: Option<&str>,
) -> Result<(Vec<String>, bool, bool), String> {
    let prepared = prepare_archive_memory_draft(draft, owner_agent_id);
    if prepared.skipped_profile {
        return Ok((Vec::new(), false, true));
    }
    let Some(input) = prepared.input else {
        return Ok((Vec::new(), false, false));
    };
    let (results, _) = memory_store_upsert_drafts(data_path, &[input])?;
    Ok((
        results.into_iter().filter_map(|r| r.id).collect::<Vec<_>>(),
        prepared.is_profile,
        false,
    ))
}

fn apply_memory_actions_into_store(
    data_path: &PathBuf,
    actions: &[ArchiveMemoryActionDraft],
    owner_agent_id: Option<&str>,
) -> Result<AppliedArchiveMemoryStats, String> {
    let mut stats = AppliedArchiveMemoryStats::default();
    let mut applied_memories = 0usize;
    for action in actions {
        match action.action {
            ArchiveMemoryActionKind::Create => {
                let (upserted_ids, is_profile, skipped_profile) =
                    upsert_archive_memory_draft_with_ids(data_path, &action.memory, owner_agent_id)?;
                if skipped_profile {
                    stats.skipped_profile_memories += 1;
                    continue;
                }
                applied_memories += upserted_ids.len();
                if is_profile {
                    stats.applied_profile_memories += upserted_ids.len();
                }
            }
            ArchiveMemoryActionKind::Update | ArchiveMemoryActionKind::Merge => {
                let source_ids = action
                    .source_memory_ids
                    .iter()
                    .map(|id| id.trim())
                    .filter(|id| !id.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>();
                if source_ids.is_empty() {
                    continue;
                }
                let (upserted_ids, is_profile, skipped_profile) =
                    upsert_archive_memory_draft_with_ids(data_path, &action.memory, owner_agent_id)?;
                if skipped_profile {
                    stats.skipped_profile_memories += 1;
                    continue;
                }
                if upserted_ids.is_empty() {
                    continue;
                }
                applied_memories += upserted_ids.len();
                if is_profile {
                    stats.applied_profile_memories += upserted_ids.len();
                }
                let retained = upserted_ids
                    .iter()
                    .map(|id| id.as_str())
                    .collect::<HashSet<_>>();
                for source_id in source_ids {
                    if retained.contains(source_id.as_str()) {
                        continue;
                    }
                    if let Err(err) = memory_store_delete_memory(data_path, &source_id) {
                        runtime_log_error(format!(
                            "[归档流程] 删除已合并来源记忆失败: id={}, err={}",
                            source_id, err
                        ));
                    }
                }
                stats.merged_groups += 1;
            }
        }
    }
    stats.merged_memories = applied_memories;
    Ok(stats)
}

fn resolve_archive_owner_context(
    state: &AppState,
    source: &Conversation,
) -> Result<(AgentProfile, String, String), String> {
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let user_alias = runtime_snapshot
        .agents
        .iter()
        .find(|agent| agent.id == USER_PERSONA_ID || agent.is_built_in_user)
        .map(|agent| agent.name.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    let owner_agent_id = resolve_archive_owner_agent_id(
        &runtime_snapshot.config,
        &runtime_snapshot.agents,
        source,
    )?;
    let owner_agent = runtime_snapshot
        .agents
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrimCompactionPreviewResult {
    conversation_id: String,
    can_compact: bool,
    message_count: usize,
    has_assistant_reply: bool,
    is_empty: bool,
    context_usage_percent: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    compaction_disabled_reason: Option<String>,
}

const ARCHIVE_MIN_BODY_MESSAGE_COUNT: usize = 3;
const ARCHIVE_REFLECTION_MIN_BODY_TOKENS: f64 = 1_000.0;

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

fn archive_message_body_text(message: &ChatMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text, .. } => Some(clean_text(text.trim())),
            _ => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn archive_message_memory_block(
    message: &ChatMessage,
    memories: &[MemoryEntry],
    seen_memory_ids: &mut HashSet<String>,
) -> Option<String> {
    let inject_ids = prompt_retrieved_memory_ids_from_message(message)
        .into_iter()
        .filter(|memory_id| seen_memory_ids.insert(memory_id.clone()))
        .collect::<Vec<_>>();
    build_memory_board_xml_from_recall_ids(memories, &inject_ids, false)
}

fn archive_message_stored_memory_blocks(message: &ChatMessage) -> Vec<String> {
    message
        .extra_text_blocks
        .iter()
        .filter_map(|block| {
            let trimmed = block.trim();
            if !trimmed.contains("<memory_context>")
                && !trimmed.contains("[MemoryBoard]")
                && !trimmed.contains("<memory_board")
            {
                return None;
            }
            let sanitized = sanitize_memory_block_xml(trimmed);
            let sanitized = sanitized.trim();
            if sanitized.is_empty() {
                None
            } else {
                Some(sanitized.to_string())
            }
        })
        .collect()
}

fn build_archive_body_reporting_conversation(
    source: &Conversation,
    memories: &[MemoryEntry],
) -> Conversation {
    let mut seen_memory_ids = HashSet::<String>::new();
    let mut reporting = source.clone();
    reporting.messages = source
        .messages
        .iter()
        .filter_map(|message| {
            let role = message.role.trim().to_ascii_lowercase();
            if role != "user" && role != "assistant" {
                return None;
            }
            if archive_pipeline_is_context_compaction_message(message) {
                return None;
            }
            let mut body_blocks = Vec::<String>::new();
            let body_text = archive_message_body_text(message);
            if !body_text.is_empty() {
                body_blocks.push(body_text);
            }
            let stored_memory_blocks = archive_message_stored_memory_blocks(message);
            let has_stored_memory_blocks = !stored_memory_blocks.is_empty();
            body_blocks.extend(stored_memory_blocks);
            if !has_stored_memory_blocks {
                if let Some(memory_block) =
                    archive_message_memory_block(message, memories, &mut seen_memory_ids)
                {
                    body_blocks.push(memory_block);
                }
            }
            let body = body_blocks.join("\n\n");
            if body.trim().is_empty() {
                return None;
            }
            let mut next = message.clone();
            next.role = role;
            next.parts = vec![MessagePart::Text {
                text: body,
                reasoning_content: None,
            }];
            next.extra_text_blocks.clear();
            next.provider_meta = None;
            next.tool_call = None;
            next.mcp_call = None;
            Some(next)
        })
        .collect();
    reporting
}

fn archive_body_token_count(source: &Conversation) -> f64 {
    source
        .messages
        .iter()
        .map(archive_message_body_text)
        .filter(|text| !text.is_empty())
        .map(|text| estimated_tokens_for_text(&text))
        .sum()
}

fn resolve_archive_request_conversation_by_id(
    state: &AppState,
    conversation_id: &str,
) -> Result<(ApiConfig, ResolvedApiConfig, Conversation, String), String> {
    conversation_service_v2().resolve_archive_request_conversation_by_id(state, conversation_id)
}

fn log_manual_archive_failure(conversation_id: &str, reason: String) -> String {
    let reason = decorate_manual_archive_failure_reason(reason);
    runtime_log_warn(format!(
        "[归档] 失败，任务=手动归档，conversation_id={}，error={}",
        conversation_id.trim(),
        reason
    ));
    reason
}

fn decorate_manual_archive_failure_reason(reason: String) -> String {
    if !should_suggest_deleting_unrepairable_conversation(&reason) {
        return reason;
    }
    if reason.contains("可直接删除该会话") {
        return reason;
    }
    format!(
        "{} 如确认该会话已无保留价值，可直接删除该会话。",
        reason.trim()
    )
}

fn should_suggest_deleting_unrepairable_conversation(reason: &str) -> bool {
    let trimmed = reason.trim();
    (trimmed.contains("消息存储")
        || trimmed.contains("会话块")
        || trimmed.contains("JSONL 索引与消息不一致")
        || trimmed.contains("解析 JSONL 消息失败")
        || trimmed.contains("读取 JSONL 消息失败")
        || trimmed.contains("目录型会话消息数量不一致")
        || trimmed.contains("目录型会话最后消息不一致"))
        && (trimmed.contains("无法")
            || trimmed.contains("失败")
            || trimmed.contains("不一致")
            || trimmed.contains("损坏")
            || trimmed.contains("缺少"))
}

fn instant_archive_conversation(
    state: &AppState,
    selected_api: &ApiConfig,
    source: &Conversation,
) -> Result<InstantArchiveConversationMutationResult, String> {
    conversation_service_v2().archive_conversation(
        state,
        selected_api,
        source,
        "manual_trim_conversation",
    )
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

fn archive_reflection_skip_reason(source: &Conversation) -> Option<String> {
    let message_count = archive_pipeline_message_count_for_delete(source);
    if message_count > ARCHIVE_MIN_BODY_MESSAGE_COUNT {
        return None;
    }
    Some(format!(
        "当前会话用户/助手消息不超过 {} 条，已跳过归档反思。",
        ARCHIVE_MIN_BODY_MESSAGE_COUNT
    ))
}

fn archive_pipeline_has_assistant_reply(source: &Conversation) -> bool {
    source
        .messages
        .iter()
        .any(|message| message.role.trim().eq_ignore_ascii_case("assistant"))
}

#[derive(Debug, Clone)]
enum SummaryContextModelError {
    EmptyReply(String),
    InvalidJson(String),
    NonRetryable(String),
}

impl SummaryContextModelError {
    fn is_invalid_json(&self) -> bool {
        matches!(self, SummaryContextModelError::InvalidJson(_))
    }
}

impl std::fmt::Display for SummaryContextModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SummaryContextModelError::EmptyReply(message)
            | SummaryContextModelError::InvalidJson(message)
            | SummaryContextModelError::NonRetryable(message) => f.write_str(message),
        }
    }
}

impl From<String> for SummaryContextModelError {
    fn from(value: String) -> Self {
        SummaryContextModelError::NonRetryable(value)
    }
}

async fn assemble_compaction_tool_definitions(
    state: &AppState,
    app_config: &AppConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    conversation_id: &str,
) -> Vec<ProviderToolDefinition> {
    if !selected_api.enable_tools {
        return Vec::new();
    }
    let chat_session_key = inflight_chat_key(&agent.id, Some(conversation_id));
    let assembly = assemble_runtime_tools(
        app_config,
        selected_api,
        agent,
        Some(state),
        &chat_session_key,
    )
    .await;
    assembly.tool_definitions
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
) -> Result<MemoryCurationDraft, SummaryContextModelError> {
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let app_config = runtime_snapshot.config;
    let agents = runtime_snapshot.agents;
    let response_style_id = {
        let state = state.clone();
        tokio::task::spawn_blocking(move || {
            state_service_get_response_style_id(&state)
        })
        .await
        .map_err(|err| SummaryContextModelError::NonRetryable(format!("读取响应风格 ID 失败：error={err}")))?
    }?;
    let user_intro = agents
        .iter()
        .find(|agent| agent.id == USER_PERSONA_ID || agent.is_built_in_user)
        .map(|agent| agent.system_prompt.trim().to_string())
        .unwrap_or_default();
    let mut prepared = build_prepared_prompt_for_mode(
        PromptBuildMode::Chat,
        source_conversation,
        agent,
        &agents,
        user_alias,
        &user_intro,
        &response_style_id,
        "zh-CN",
        Some(&state.data_path),
        None,
        None,
        Some(ChatPromptOverrides {
            latest_user_intent: Some(LatestUserPayloadIntent::SummaryContext {
                scene,
                user_alias: user_alias.to_string(),
            }),
            latest_images: Some(Vec::new()),
            latest_audios: Some(Vec::new()),
            ..ChatPromptOverrides::default()
        }),
        Some(state),
        Some(selected_api),
        Some(resolved_api),
    )?;
    // 与正常对话对齐：按模型能力处理历史图片/音频（vision 转文或丢弃），
    // 防止 image_url 泄漏给不支持图片输入的模型（如 DeepSeek 文本模型）。
    let _ = apply_prompt_image_fallbacks_to_prepared(
        state,
        &source_conversation.id,
        &app_config,
        selected_api,
        &mut prepared,
    )
    .await?;
    drop_unsupported_prepared_audios(selected_api, &mut prepared);
    let timeout_secs = 360u64;
    let tool_definitions = assemble_compaction_tool_definitions(
        state,
        &app_config,
        selected_api,
        agent,
        &source_conversation.id,
    )
    .await;
    let archive_summary_execution = call_archive_summary_model_with_timeout(
        state,
        resolved_api,
        selected_api,
        prepared,
        timeout_secs,
        tool_definitions,
    )
    .await;
    push_model_call_log_parts(Some(state), &archive_summary_execution);
    let reply = archive_summary_execution.result?;
    match model_reply_content_state(&reply) {
        ModelReplyContentState::Visible => {}
        ModelReplyContentState::ReasoningOnly => {
            return Err(SummaryContextModelError::EmptyReply(
                "SummaryContext 模型只返回 reasoning，没有最终 JSON".to_string(),
            ));
        }
        ModelReplyContentState::Empty => {
            return Err(SummaryContextModelError::EmptyReply(
                "SummaryContext 模型返回空内容".to_string(),
            ));
        }
    }
    let parsed = parse_memory_curation_draft(&reply.assistant_text).ok_or_else(|| {
        SummaryContextModelError::InvalidJson(format!(
            "SummaryContext JSON 解析失败，raw={}",
            reply.assistant_text
        ))
    })?;
    let open_loops = parsed
        .open_loops
        .iter()
        .map(|item| clean_text(item.trim()))
        .filter(|item| !item.is_empty())
        .take(7)
        .collect::<Vec<_>>();
    let summary = if matches!(scene, SummaryContextScene::Archive) {
        String::new()
    } else {
        compose_summary_context_summary(&parsed.summary, &open_loops, scene)
    };
    if !matches!(scene, SummaryContextScene::Archive) && summary.is_empty() {
        return Err(SummaryContextModelError::NonRetryable(
            "SummaryContext summary is empty".to_string(),
        ));
    }
    let id_alias_map = memory_curation_id_alias_map(memories);
    Ok(MemoryCurationDraft {
        title: if matches!(scene, SummaryContextScene::Archive) {
            String::new()
        } else {
            normalize_summary_context_title(&parsed.title).unwrap_or_default()
        },
        summary,
        open_loops: if matches!(scene, SummaryContextScene::Archive) {
            Vec::new()
        } else {
            open_loops
        },
        useful_memory_ids: resolve_memory_curation_ids(&parsed.useful_memory_ids, &id_alias_map),
        memory_actions: resolve_memory_action_drafts(&parsed.memory_actions, &id_alias_map),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SummaryContextScene {
    Compaction,
    Archive,
}

fn summary_context_prompt_template(scene: SummaryContextScene) -> &'static str {
    match scene {
        SummaryContextScene::Compaction => {
            include_str!("../../../../resources/prompts/summary-context.md")
        }
        SummaryContextScene::Archive => {
            include_str!("../../../../resources/prompts/archive-reflection.md")
        }
    }
}

fn extract_prompt_xml_block(raw: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = raw.find(&open)?;
    let body_start = start + open.len();
    let end = raw[body_start..].find(&close)? + body_start;
    Some(raw[start..end + close.len()].trim().to_string())
}

fn build_summary_context_requirement_block(scene: SummaryContextScene) -> String {
    extract_prompt_xml_block(summary_context_prompt_template(scene), "summary_requirement")
        .unwrap_or_default()
}

fn build_summary_context_system_remind_block(scene: SummaryContextScene) -> String {
    extract_prompt_xml_block(summary_context_prompt_template(scene), "system_remind")
        .unwrap_or_default()
}

fn build_summary_context_memory_block(
    scene: SummaryContextScene,
    agent: &AgentProfile,
    user_alias: &str,
) -> String {
    extract_prompt_xml_block(summary_context_prompt_template(scene), "memory_curation_context")
        .unwrap_or_default()
        .replace("{{assistant_name}}", agent.name.trim())
        .replace("{{user_name}}", user_alias.trim())
        .replace("{{memory_generation_rules}}", memory_generation_rules_body())
}

fn build_summary_context_json_contract_block(scene: SummaryContextScene) -> String {
    let json_example = match scene {
        SummaryContextScene::Compaction => memory_curation_example_output_block(),
        SummaryContextScene::Archive => archive_reflection_example_output_block(),
    };
    extract_prompt_xml_block(summary_context_prompt_template(scene), "json_contract")
        .unwrap_or_default()
        .replace("{{json_example}}", json_example)
}

#[cfg(test)]
fn archive_pipeline_message_plain_text(message: &ChatMessage) -> String {
    let mut blocks = Vec::<String>::new();
    if message.role.trim().eq_ignore_ascii_case("assistant") {
        for event in message.tool_call.iter().flatten() {
            let is_assistant = event
                .get("role")
                .and_then(Value::as_str)
                .is_some_and(|role| role.trim().eq_ignore_ascii_case("assistant"));
            if !is_assistant {
                continue;
            }
            let cleaned = event
                .get("content")
                .and_then(Value::as_str)
                .map(str::trim)
                .map(clean_text)
                .unwrap_or_default();
            if !cleaned.is_empty() {
                blocks.push(cleaned);
            }
        }
    }
    for part in &message.parts {
        if let MessagePart::Text { text, .. } = part {
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

fn resolve_memory_action_drafts(
    drafts: &[ArchiveMemoryActionDraft],
    id_alias_map: &HashMap<String, String>,
) -> Vec<ArchiveMemoryActionDraft> {
    drafts
        .iter()
        .map(|item| ArchiveMemoryActionDraft {
            action: item.action.clone(),
            source_memory_ids: resolve_memory_curation_ids(&item.source_memory_ids, id_alias_map),
            memory: item.memory.clone(),
        })
        .collect::<Vec<_>>()
}

fn compose_summary_context_summary(
    summary: &str,
    open_loops: &[String],
    scene: SummaryContextScene,
) -> String {
    let summary = normalize_markdown_block(summary);
    if open_loops.is_empty() {
        return summary;
    }
    let open_loop_lines = open_loops
        .iter()
        .enumerate()
        .map(|(idx, item)| format!("{}. {}", idx + 1, item))
        .collect::<Vec<_>>()
        .join("\n");
    let _ = scene;
    let section_title = "## 未完事项";
    if summary.is_empty() {
        format!("{}\n\n{}", section_title, open_loop_lines)
    } else {
        format!("{}\n\n{}\n\n{}", summary, section_title, open_loop_lines)
    }
}

#[allow(dead_code)]
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
            runtime_log_warn(format!(
                "[归档流程] history_flushed 事件发送跳过: app_handle unavailable, source_conversation_id={}, active_conversation_id={}",
                source_conversation_id, active_conversation_id
            ));
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
        runtime_log_error(format!(
            "[归档流程] history_flushed 事件发送失败: source_conversation_id={}, active_conversation_id={}, archive_id={}, error={}",
            source_conversation_id, active_conversation_id, archive_id, err
        ));
    } else {
        runtime_log_info(format!(
            "[归档流程] history_flushed 事件发送完成: source_conversation_id={}, active_conversation_id={}, archive_id={}",
            source_conversation_id, active_conversation_id, archive_id
        ));
    }
    if let Err(err) = emit_unarchived_conversation_overview_updated_from_state(state) {
        runtime_log_error(format!(
            "[会话概览] archive_history_flushed 后推送失败: source_conversation_id={}, error={}",
            source_conversation_id, err
        ));
    }
}

fn emit_compaction_history_flushed_event(
    state: &AppState,
    conversation_id: &str,
    boundary_messages: &[ChatMessage],
    compression_message: &ChatMessage,
    activate_after_flush: bool,
) {
    let app_handle = match state.app_handle.lock().ok().and_then(|guard| guard.clone()) {
        Some(handle) => handle,
        None => {
            runtime_log_warn(format!(
                "[归档流程] 上下文整理 history_flushed 发送跳过: app_handle 不可用, conversation_id={}",
                conversation_id
            ));
            return;
        }
    };
    let messages =
        build_compaction_history_flushed_messages(boundary_messages, compression_message);
    let message_count = messages.len();
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "messageCount": message_count,
        "messages": messages,
        "activateAssistant": activate_after_flush,
        "compactionApplied": true,
    });
    if let Err(err) = app_handle.emit(CHAT_HISTORY_FLUSHED_EVENT, payload) {
        runtime_log_error(format!(
            "[归档流程] 上下文整理 history_flushed 发送失败: conversation_id={}, error={}",
            conversation_id, err
        ));
    } else {
        runtime_log_info(format!(
            "[归档流程] 上下文整理 history_flushed 已发送: conversation_id={}, message_count={}",
            conversation_id, message_count
        ));
    }
    if let Err(err) = emit_unarchived_conversation_overview_updated_from_state(state) {
        runtime_log_error(format!(
            "[会话概览] compaction_history_flushed 后推送失败: conversation_id={}, error={}",
            conversation_id, err
        ));
    }
}

fn build_compaction_history_flushed_messages(
    boundary_messages: &[ChatMessage],
    compression_message: &ChatMessage,
) -> Vec<ChatMessage> {
    let mut messages = Vec::<ChatMessage>::new();
    let mut seen_ids = HashSet::<String>::new();
    for message in boundary_messages {
        let id = message.id.trim();
        if !id.is_empty() && !seen_ids.insert(id.to_string()) {
            continue;
        }
        messages.push(message.clone());
    }
    let compression_id = compression_message.id.trim();
    if compression_id.is_empty() || seen_ids.insert(compression_id.to_string()) {
        messages.push(compression_message.clone());
    }
    messages
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
            runtime_log_warn(format!(
                "[归档流程] 删除 history_flushed 发送跳过: app_handle 不可用, deleted_conversation_id={}, active_conversation_id={}",
                deleted_conversation_id, active_conversation_id
            ));
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
        runtime_log_error(format!(
            "[归档流程] 删除 history_flushed 发送失败: deleted_conversation_id={}, active_conversation_id={}, error={}",
            deleted_conversation_id, active_conversation_id, err
        ));
    } else {
        runtime_log_info(format!(
            "[归档流程] 删除 history_flushed 已发送: deleted_conversation_id={}, active_conversation_id={}",
            deleted_conversation_id, active_conversation_id
        ));
    }
}

fn delete_main_conversation_and_activate_latest(
    state: &AppState,
    selected_api: &ApiConfig,
    source: &Conversation,
) -> Result<String, String> {
    conversation_service_v2().delete_main_conversation_and_activate_latest(state, selected_api, source)
}

fn build_compaction_message(
    summary: &str,
    title: Option<&str>,
    compaction_reason: &str,
    preserved_dialogue: Option<&str>,
    background_shell_lines: &[String],
) -> ChatMessage {
    let now = now_iso();
    let reason = compaction_reason.trim();
    let summary_note = if reason.is_empty() {
        "- 以下内容为当前会话中较早历史对话的整理结果。\n\
         - 为保证连续性，后文保留了最近的原始对话，不包含在本段摘要中。\n\
         - 摘要中的助手发言统一使用当前人格昵称表示。"
            .to_string()
    } else {
        format!(
            "- 整理原因：{}\n\
             - 以下内容为当前会话中较早历史对话的整理结果。\n\
             - 为保证连续性，后文保留了最近的原始对话，不包含在本段摘要中。\n\
             - 摘要中的助手发言统一使用当前人格昵称表示。",
            reason
        )
    };
    let preserved_dialogue_text = preserved_dialogue
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_multiline_block)
        .unwrap_or_else(|| "（暂无保留对话）".to_string());
    let mut sections = vec![
        format!("## 摘要说明\n\n{}", normalize_markdown_block(&summary_note)),
        format!("## 摘要正文\n\n{}", clean_compaction_summary_text(summary)),
        format!("## 保留对话\n\n{}", preserved_dialogue_text),
    ];
    if !background_shell_lines.is_empty() {
        sections.push(format!(
            "## 运行中的后台任务\n\n{}",
            normalize_multiline_block(&background_shell_lines.join("\n"))
        ));
    }
    let text = sections.join("\n\n");
    let normalized_title = title.and_then(normalize_summary_context_title);
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now,
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text { text, reasoning_content: None }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "message_meta": {
                "kind": "context_compaction",
                "scene": "compaction",
                "schemaVersion": SUMMARY_CONTEXT_MESSAGE_SCHEMA_VERSION,
                "reason": reason,
                "title": normalized_title,
            }
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
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

fn normalize_markdown_block(input: &str) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = normalized
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>();
    while lines.first().map(|line| line.trim().is_empty()).unwrap_or(false) {
        lines.remove(0);
    }
    while lines.last().map(|line| line.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n")
}

fn clean_compaction_summary_text(input: &str) -> String {
    let trimmed = input.trim();
    if let Some((summary, active_plans)) = trimmed.split_once("<active_plans>") {
        let cleaned_summary = normalize_markdown_block(summary);
        let cleaned_active_plans = normalize_multiline_block(&format!("<active_plans>{active_plans}"));
        if cleaned_summary.is_empty() {
            return cleaned_active_plans;
        }
        return format!("{}\n\n{}", cleaned_summary, cleaned_active_plans);
    }
    normalize_markdown_block(trimmed)
}

fn build_initial_summary_context_message(
    current_todos: Option<&[ConversationTodoItem]>,
    title: Option<&str>,
) -> ChatMessage {
    let now = now_iso();
    let todo_snapshot = current_todos
        .and_then(todo_markdown_block)
        .map(|value| normalize_markdown_block(&value))
        .unwrap_or_default();
    let normalized_title = title.and_then(normalize_summary_context_title);
    let text = if todo_snapshot.is_empty() {
        "## 摘要说明\n\n- 这是新会话的初始背景，不包含历史对话摘要。".to_string()
    } else {
        format!(
            "## 摘要说明\n\n- 这是新会话的初始背景，不包含历史对话摘要。\n\n{}",
            todo_snapshot
        )
    };
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now,
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text { text, reasoning_content: None }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "message_meta": {
                "kind": "summary_context_seed",
                "scene": "seed",
                "schemaVersion": SUMMARY_CONTEXT_MESSAGE_SCHEMA_VERSION,
                "title": normalized_title,
            }
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

#[derive(Debug, Clone, Default)]
struct SummaryContextApplyReport {
    merged_memories: usize,
    merged_groups: usize,
    applied_profile_memories: usize,
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
    if !agent_memory_recall_enabled(host_agent) {
        runtime_log_info(format!(
            "[SummaryContext] 完成，任务=归档记忆落库，结果=跳过，原因=回忆方式=完全关闭，agent_id={}，丢弃memory_actions={}，丢弃useful_memory_ids={}",
            host_agent.id,
            draft.memory_actions.len(),
            draft.useful_memory_ids.len()
        ));
        return Ok(SummaryContextApplyReport::default());
    }
    let memory_feedback =
        memory_store_apply_archive_feedback(data_path, recall_ids, &draft.useful_memory_ids)?;
    let memory_stats =
        apply_memory_actions_into_store(data_path, &draft.memory_actions, owner_agent_id)?;
    Ok(SummaryContextApplyReport {
        merged_memories: memory_stats.merged_memories,
        merged_groups: memory_stats.merged_groups,
        applied_profile_memories: memory_stats.applied_profile_memories,
        skipped_profile_memories: memory_stats.skipped_profile_memories,
        memory_feedback,
    })
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
    const MAX_JSON_RETRIES: usize = 3;
    const RETRY_DELAY_SECS: u64 = 5;

    let deduped_recall = archive_pipeline_dedup_recall_table(&reporting_source.memory_recall_table);
    let mut last_err = String::new();

    for attempt in 1..=MAX_JSON_RETRIES {
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
            Ok(mut draft) => {
                draft.title.clear();
                draft.summary.clear();
                draft.open_loops.clear();
                return (draft, None);
            }
            Err(err) => {
                last_err = err.to_string();
                if !err.is_invalid_json() {
                    return (
                        MemoryCurationDraft {
                            title: String::new(),
                            summary: String::new(),
                            open_loops: Vec::new(),
                            useful_memory_ids: Vec::new(),
                            memory_actions: Vec::new(),
                        },
                        Some(format!(
                            "SummaryContext 归档反思失败，已跳过本轮记忆整理：{}",
                            last_err
                        )),
                    );
                }
                if attempt < MAX_JSON_RETRIES {
                    runtime_log_warn(format!(
                        "[SummaryContext] 归档反思 JSON 无效，准备重试: conversation_id={}, api_id={}, attempt={}，next_retry_secs={}，error={}",
                        reporting_source.id,
                        selected_api.id,
                        attempt,
                        RETRY_DELAY_SECS,
                        last_err
                    ));
                    tokio::time::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
                }
            }
        }
    }

    (
        empty_memory_curation_draft(),
        Some(format!(
            "SummaryContext 归档反思失败，JSON 解析已重试{}次仍失败：{}",
            MAX_JSON_RETRIES, last_err
        )),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactionSummaryModelRole {
    Conversation,
    Quick,
}

impl CompactionSummaryModelRole {
    fn label(self) -> &'static str {
        match self {
            CompactionSummaryModelRole::Conversation => "会话模型",
            CompactionSummaryModelRole::Quick => "快速模型",
        }
    }
}

fn empty_memory_curation_draft() -> MemoryCurationDraft {
    MemoryCurationDraft {
        title: String::new(),
        summary: String::new(),
        open_loops: Vec::new(),
        useful_memory_ids: Vec::new(),
        memory_actions: Vec::new(),
    }
}

fn resolve_context_compaction_primary_model_from_config(
    app_config: &AppConfig,
    source: &Conversation,
    fallback_api: &ApiConfig,
) -> Result<ApiConfig, String> {
    let Some(preferred_api_config_id) = source
        .preferred_api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(fallback_api.clone());
    };
    let resolved_api_config_id = resolve_model_role_api_config_id(app_config, preferred_api_config_id)
        .ok_or_else(|| format!("会话模型角色未配置：{}", preferred_api_config_id))?;
    app_config
        .api_configs
        .iter()
        .find(|api| {
            api.id.trim() == resolved_api_config_id
                && api.enable_text
                && api.request_format.is_chat_text()
        })
        .cloned()
        .ok_or_else(|| format!("会话模型不可用于文本对话：{}", preferred_api_config_id))
}

fn resolve_context_compaction_primary_model(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    trace_id: &str,
) -> Result<(ApiConfig, ResolvedApiConfig), String> {
    let app_config = state_read_config_cached(state)?;
    let primary_api =
        match resolve_context_compaction_primary_model_from_config(&app_config, source, selected_api)
        {
            Ok(api) => api,
            Err(err) => {
                runtime_log_warn(format!(
                    "[SummaryContext] 会话模型解析失败，沿用调用方模型: trace_id={}, conversation_id={}, fallback_api_id={}, err={}",
                    trace_id, source.id, selected_api.id, err
                ));
                selected_api.clone()
            }
        };
    if primary_api.id.trim() == selected_api.id.trim() {
        return Ok((selected_api.clone(), resolved_api.clone()));
    }
    let primary_resolved_api = resolve_api_config(&app_config, Some(primary_api.id.as_str()))?;
    runtime_log_debug(format!(
        "[SummaryContext] 使用会话模型执行压缩: trace_id={}, conversation_id={}, conversation_api_id={}, incoming_api_id={}",
        trace_id, source.id, primary_api.id, selected_api.id
    ));
    Ok((primary_api, primary_resolved_api))
}

fn resolve_compaction_quick_model_from_config(
    app_config: &AppConfig,
    quick_api_config_id: Option<&str>,
    conversation_api_id: &str,
) -> Result<Option<ApiConfig>, String> {
    let Some(quick_api_config_id) = quick_api_config_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let selected_api = app_config
        .api_configs
        .iter()
        .find(|api| api.id.trim() == quick_api_config_id)
        .cloned()
        .ok_or_else(|| format!("快速模型配置不存在：{}", quick_api_config_id))?;
    if selected_api.id.trim() == conversation_api_id.trim() {
        return Ok(None);
    }
    if !selected_api.enable_text || !selected_api.request_format.is_chat_text() {
        return Err(format!("快速模型不支持文本对话：{}", selected_api.id));
    }
    Ok(Some(selected_api))
}

fn resolve_compaction_quick_model(
    state: &AppState,
    conversation_api_id: &str,
) -> Result<Option<(ApiConfig, ResolvedApiConfig)>, String> {
    let app_config = state_read_config_cached(state)?;
    let Some(selected_api) = resolve_compaction_quick_model_from_config(
        &app_config,
        app_config.tool_review_api_config_id.as_deref(),
        conversation_api_id,
    )?
    else {
        return Ok(None);
    };
    let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
    Ok(Some((selected_api, resolved_api)))
}

async fn summarize_compaction_with_model_attempt(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    host_agent: &AgentProfile,
    user_alias: &str,
    source: &Conversation,
    visible_memories: &[MemoryEntry],
    deduped_recall: &[String],
    model_role: CompactionSummaryModelRole,
    trace_id: &str,
) -> Result<MemoryCurationDraft, String> {
    const MAX_JSON_RETRIES: usize = 3;
    const RETRY_DELAY_SECS: u64 = 5;

    let mut last_err = String::new();
    for attempt in 1..=MAX_JSON_RETRIES {
        match summarize_archived_conversation_with_model_v2(
            state,
            resolved_api,
            selected_api,
            host_agent,
            user_alias,
            source,
            SummaryContextScene::Compaction,
            visible_memories,
            deduped_recall,
        )
        .await
        {
            Ok(summary) => return Ok(summary),
            Err(err) => {
                last_err = err.to_string();
                if !err.is_invalid_json() {
                    runtime_log_warn(format!(
                        "[SummaryContext] 上下文整理失败，不重试非 JSON 错误: trace_id={}, conversation_id={}, model_role={}, api_id={}, attempt={}，err={}",
                        trace_id,
                        source.id,
                        model_role.label(),
                        selected_api.id,
                        attempt,
                        last_err
                    ));
                    return Err(format!(
                        "{}失败（非 JSON 错误不重试）：{}",
                        model_role.label(),
                        last_err
                    ));
                }
                if attempt < MAX_JSON_RETRIES {
                    runtime_log_warn(format!(
                        "[归档流程] 上下文整理 JSON 无效，准备重试: trace_id={}, conversation_id={}, model_role={}, api_id={}, attempt={}，next_retry_secs={}，error={}",
                        trace_id,
                        source.id,
                        model_role.label(),
                        selected_api.id,
                        attempt,
                        RETRY_DELAY_SECS,
                        last_err
                    ));
                    tokio::time::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
                }
            }
        }
    }

    runtime_log_error(format!(
        "[SummaryContext] 上下文整理失败: trace_id={}, conversation_id={}, model_role={}, api_id={}, attempts={}, err={}",
        trace_id,
        source.id,
        model_role.label(),
        selected_api.id,
        MAX_JSON_RETRIES,
        last_err
    ));
    Err(format!(
        "{}失败（JSON 解析已重试{}次）：{}",
        model_role.label(),
        MAX_JSON_RETRIES,
        last_err
    ))
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
    let visible_memories = match memory_store_list_memories_visible_for_agent(
        &state.data_path,
        &host_agent.id,
        host_agent.private_memory_enabled,
    ) {
        Ok(items) => items,
        Err(err) => {
            return (
                empty_memory_curation_draft(),
                Some(format!(
                    "SummaryContext 读取可见记忆失败，压缩摘要留空：{}",
                    err
                )),
            )
        }
    };
    let deduped_recall = archive_pipeline_dedup_recall_table(&source.memory_recall_table);
    let mut failures = Vec::<String>::new();

    match summarize_compaction_with_model_attempt(
        state,
        selected_api,
        resolved_api,
        host_agent,
        user_alias,
        source,
        &visible_memories,
        &deduped_recall,
        CompactionSummaryModelRole::Conversation,
        trace_id,
    )
    .await
    {
        Ok(summary) => return (summary, None),
        Err(err) => failures.push(err),
    }

    match resolve_compaction_quick_model(state, &selected_api.id) {
        Ok(Some((quick_selected_api, quick_resolved_api))) => {
            runtime_log_warn(format!(
                "[SummaryContext] 会话模型压缩失败，切换快速模型: trace_id={}, conversation_id={}, conversation_api_id={}, quick_api_id={}, reason={}",
                trace_id,
                source.id,
                selected_api.id,
                quick_selected_api.id,
                failures.last().map(String::as_str).unwrap_or("")
            ));
            match summarize_compaction_with_model_attempt(
                state,
                &quick_selected_api,
                &quick_resolved_api,
                host_agent,
                user_alias,
                source,
                &visible_memories,
                &deduped_recall,
                CompactionSummaryModelRole::Quick,
                trace_id,
            )
            .await
            {
                Ok(summary) => return (summary, None),
                Err(err) => failures.push(err),
            }
        }
        Ok(None) => {
            runtime_log_warn(format!(
                "[SummaryContext] 快速模型跳过: trace_id={}, conversation_id={}, conversation_api_id={}, reason=not_configured_or_same_as_conversation",
                trace_id, source.id, selected_api.id
            ));
            failures.push("快速模型未配置或与会话模型相同，已跳过".to_string());
        }
        Err(err) => {
            runtime_log_error(format!(
                "[SummaryContext] 快速模型解析失败: trace_id={}, conversation_id={}, conversation_api_id={}, err={}",
                trace_id, source.id, selected_api.id, err
            ));
            failures.push(format!("快速模型解析失败：{}", err));
        }
    }

    runtime_log_error(format!(
        "[SummaryContext] 上下文整理失败，压缩摘要留空继续主流程: trace_id={}, conversation_id={}, errors={}",
        trace_id,
        source.id,
        failures.join("；")
    ));
    (
        empty_memory_curation_draft(),
        Some(format!(
            "SummaryContext 上下文整理失败（会话模型失败后快速模型仍不可用），压缩摘要留空：{}",
            failures.join("；")
        )),
    )
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
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        }
    }

    fn test_memory_entry(id: &str, judgment: &str) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            memory_no: None,
            memory_type: "knowledge".to_string(),
            judgment: judgment.to_string(),
            reasoning: "回归测试".to_string(),
            tags: vec!["测试".to_string()],
            owner_agent_id: None,
            created_at: "2026-04-18T10:00:00Z".to_string(),
            updated_at: "2026-04-18T10:00:00Z".to_string(),
        }
    }

    fn test_conversation() -> Conversation {
        Conversation {
            id: "conversation-a".to_string(),
            title: "测试会话".to_string(),
            agent_id: "agent-a".to_string(),
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
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: vec![
                test_message("m1", "user", "前置问题"),
                test_message("m2", "assistant", "分叉点回答"),
                test_message("m3", "user", "分叉后的新问题"),
                test_message("m4", "assistant", "分叉后的最终结论"),
            ],
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    fn test_api_config(id: &str, model: &str) -> ApiConfig {
        let mut api = ApiConfig::default();
        api.id = id.to_string();
        api.name = id.to_string();
        api.api_key = "k".to_string();
        api.model = model.to_string();
        api
    }

    fn test_compaction_app_config(conversation_api_id: &str, quick_api_id: &str) -> AppConfig {
        AppConfig {
            api_configs: vec![
                test_api_config(conversation_api_id, "conversation-model"),
                test_api_config(quick_api_id, "quick-model"),
            ],
            expert_api_config_id: conversation_api_id.to_string(),
            tool_review_api_config_id: Some(quick_api_id.to_string()),
            ..AppConfig::default()
        }
    }

    /// 压缩场景的 prepared：历史消息带图片、最新消息图片为空（overrides 置空后的产物）。
    fn test_compaction_prepared_with_history_image() -> PreparedPrompt {
        PreparedPrompt {
            preamble: String::new(),
            history_messages: vec![PreparedHistoryMessage {
                role: "user".to_string(),
                text: "历史带图消息".to_string(),
                extra_text_blocks: Vec::new(),
                user_time_text: None,
                images: vec![PreparedBinaryPayload {
                    label: "图片#1".to_string(),
                    mime: "image/png".to_string(),
                    content: B64.encode(b"history-image"),
                    saved_path: Some("downloads/history.png".to_string()),
                }],
                audios: Vec::new(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            latest_user_text: "压缩请求文本".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        }
    }

    #[tokio::test]
    async fn compaction_media_fallback_should_drop_history_images_when_model_lacks_vision() {
        let state = AppState::new().expect("create test app state");
        // 无 vision 配置（vision_api_config_id=None）→ 图片转文不可用，应整体丢弃而非泄漏 image_url
        let app_config = AppConfig::default();
        let selected_api = ApiConfig {
            enable_image: false,
            ..ApiConfig::default()
        };
        let mut prepared = test_compaction_prepared_with_history_image();

        apply_prompt_image_fallbacks_to_prepared(
            &state,
            "conversation-compaction",
            &app_config,
            &selected_api,
            &mut prepared,
        )
        .await
        .expect("图片降级应成功");

        assert!(prepared
            .history_messages
            .iter()
            .all(|message| message.images.is_empty()));
        assert!(prepared.latest_images.is_empty());
        let raw = serde_json::to_string(&prepared_prompt_to_messages_json(&prepared))
            .expect("序列化应成功");
        assert!(
            !raw.contains("image_url"),
            "压缩请求体不应包含 image_url: {raw}"
        );
        assert!(raw.contains("压缩请求文本"));
    }

    #[tokio::test]
    async fn compaction_media_should_keep_history_images_when_model_supports_image() {
        let state = AppState::new().expect("create test app state");
        let app_config = AppConfig::default();
        let selected_api = ApiConfig {
            enable_image: true,
            ..ApiConfig::default()
        };
        let mut prepared = test_compaction_prepared_with_history_image();

        apply_prompt_image_fallbacks_to_prepared(
            &state,
            "conversation-compaction",
            &app_config,
            &selected_api,
            &mut prepared,
        )
        .await
        .expect("图片降级应成功");

        // enable_image=true 时图片保留（与正常对话行为一致）
        assert_eq!(prepared.history_messages[0].images.len(), 1);
        assert_eq!(prepared.history_messages[0].text, "历史带图消息");
    }

    #[test]
    fn archive_reflection_skip_reason_should_require_more_than_three_messages() {
        let mut source = test_conversation();
        source.messages = vec![
            test_message("m1", "user", "短问题"),
            test_message("m2", "assistant", "短回答"),
            test_message("m3", "user", "补充"),
        ];

        assert!(archive_reflection_skip_reason(&source).is_some());

        source.messages = vec![
            test_message("m1", "user", "问题1"),
            test_message("m2", "assistant", "回答1"),
            test_message("m3", "user", "问题2"),
            test_message("m4", "assistant", "回答2"),
        ];

        assert_eq!(archive_pipeline_message_count_for_delete(&source), 4);
        assert!(archive_reflection_skip_reason(&source).is_none());
    }

    #[test]
    fn context_compaction_primary_model_should_prefer_conversation_model() {
        let session_api = test_api_config("session-api", "session-model");
        let config = AppConfig {
            api_configs: vec![
                session_api.clone(),
                test_api_config("conversation-api", "conversation-model"),
            ],
            expert_api_config_id: "session-api".to_string(),
            ..AppConfig::default()
        };
        let mut source = test_conversation();
        source.preferred_api_config_id = Some("conversation-api".to_string());

        let selected =
            resolve_context_compaction_primary_model_from_config(&config, &source, &session_api)
                .expect("resolve primary model");

        assert_eq!(selected.id, "conversation-api");
        assert_eq!(selected.model, "conversation-model");
    }

    #[test]
    fn context_compaction_primary_model_should_keep_session_model_without_conversation_preference() {
        let session_api = test_api_config("session-api", "session-model");
        let config = AppConfig {
            api_configs: vec![session_api.clone()],
            expert_api_config_id: "session-api".to_string(),
            ..AppConfig::default()
        };
        let source = test_conversation();

        let selected =
            resolve_context_compaction_primary_model_from_config(&config, &source, &session_api)
                .expect("resolve primary model");

        assert_eq!(selected.id, "session-api");
    }

    #[test]
    fn compaction_quick_model_should_resolve_configured_fast_model() {
        let config = test_compaction_app_config("conversation-api", "quick-api");

        let selected = resolve_compaction_quick_model_from_config(
            &config,
            config.tool_review_api_config_id.as_deref(),
            "conversation-api",
        )
        .expect("resolve quick model")
        .expect("quick model");

        assert_eq!(selected.id, "quick-api");
        assert_eq!(selected.model, "quick-model");
    }

    #[test]
    fn compaction_quick_model_should_skip_when_same_as_conversation_model() {
        let config = AppConfig {
            api_configs: vec![test_api_config("same-api", "same-model")],
            expert_api_config_id: "same-api".to_string(),
            tool_review_api_config_id: Some("same-api".to_string()),
            ..AppConfig::default()
        };

        let selected = resolve_compaction_quick_model_from_config(
            &config,
            config.tool_review_api_config_id.as_deref(),
            "same-api",
        )
        .expect("resolve quick model");

        assert!(selected.is_none());
    }

    #[test]
    fn compaction_quick_model_should_skip_when_not_configured() {
        let mut config = test_compaction_app_config("conversation-api", "quick-api");
        config.tool_review_api_config_id = None;

        let selected = resolve_compaction_quick_model_from_config(
            &config,
            config.tool_review_api_config_id.as_deref(),
            "conversation-api",
        )
        .expect("resolve quick model");

        assert!(selected.is_none());
    }

    #[test]
    fn compaction_quick_model_should_not_fallback_when_configured_id_is_stale() {
        let config = test_compaction_app_config("conversation-api", "quick-api");

        let err = resolve_compaction_quick_model_from_config(
            &config,
            Some("deleted-quick-api"),
            "conversation-api",
        )
        .expect_err("stale quick model should not fallback");

        assert!(err.contains("快速模型配置不存在"));
    }

    #[test]
    fn compaction_quick_model_should_reject_non_text_model() {
        let mut quick_api = test_api_config("quick-api", "quick-model");
        quick_api.enable_text = false;
        let config = AppConfig {
            api_configs: vec![test_api_config("conversation-api", "conversation-model"), quick_api],
            expert_api_config_id: "conversation-api".to_string(),
            tool_review_api_config_id: Some("quick-api".to_string()),
            ..AppConfig::default()
        };

        let err = resolve_compaction_quick_model_from_config(
            &config,
            config.tool_review_api_config_id.as_deref(),
            "conversation-api",
        )
        .expect_err("non text quick model should fail");

        assert!(err.contains("快速模型不支持文本对话"));
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
    }

    #[test]
    fn build_archive_body_reporting_conversation_should_keep_body_and_memory_only() {
        let mut source = test_conversation();
        let mut user = test_message("u1", "user", "用户正文");
        user.extra_text_blocks = vec![
            "附件提取文本不应进入归档正文".to_string(),
            "<memory_context>\n<id:memory-a>\n用户偏好短回复\n> 回归测试\n</id:memory-a>\n</memory_context>".to_string(),
        ];
        user.provider_meta = Some(serde_json::json!({
            "hiddenPromptText": "隐藏提示不应进入归档正文",
            "retrieved_memory_ids": ["memory-a"],
            "attachments": [{
                "relativePath": "docs/large.pdf"
            }]
        }));
        user.tool_call = Some(vec![serde_json::json!({
            "role": "tool",
            "content": "工具结果不应进入归档正文"
        })]);
        let mut assistant = test_message("a1", "assistant", "助手正文");
        if let Some(MessagePart::Text {
            reasoning_content, ..
        }) = assistant.parts.first_mut()
        {
            *reasoning_content = Some("reasoning 不应进入归档正文".to_string());
        }
        source.messages = vec![user, assistant];
        let memories = vec![test_memory_entry("memory-a", "用户偏好短回复")];

        let reporting = build_archive_body_reporting_conversation(&source, &memories);
        let rendered = reporting
            .messages
            .iter()
            .map(archive_message_body_text)
            .collect::<Vec<_>>()
            .join("\n\n");

        assert!(rendered.contains("用户正文"));
        assert!(rendered.contains("助手正文"));
        assert!(rendered.contains("<memory_context>"));
        assert!(rendered.contains("用户偏好短回复"));
        assert!(!rendered.contains("附件提取文本"));
        assert!(!rendered.contains("隐藏提示"));
        assert!(!rendered.contains("docs/large.pdf"));
        assert!(!rendered.contains("工具结果"));
        assert!(!rendered.contains("reasoning"));
        assert!(reporting.messages.iter().all(|message| message.provider_meta.is_none()));
        assert!(reporting.messages.iter().all(|message| message.tool_call.is_none()));
    }

    #[test]
    fn archive_body_token_count_should_drive_reflection_threshold() {
        let mut short_source = test_conversation();
        short_source.messages = vec![
            test_message("u1", "user", "短正文"),
            test_message("a1", "assistant", "短回复"),
        ];
        let short_reporting = build_archive_body_reporting_conversation(&short_source, &[]);
        assert!(archive_body_token_count(&short_reporting) < ARCHIVE_REFLECTION_MIN_BODY_TOKENS);

        let mut long_source = test_conversation();
        long_source.messages = vec![
            test_message("u1", "user", &"很长的归档正文 ".repeat(50_000)),
            test_message("a1", "assistant", "收到"),
        ];
        let long_reporting = build_archive_body_reporting_conversation(&long_source, &[]);
        assert!(archive_body_token_count(&long_reporting) >= ARCHIVE_REFLECTION_MIN_BODY_TOKENS);
    }

    #[test]
    fn compose_summary_context_summary_should_append_open_loops() {
        let summary = compose_summary_context_summary(
            "## 当前进展\n\n- 已完成前端任务编辑器重构",
            &vec!["继续改 archive pipeline".to_string(), "补充 JSON 契约测试".to_string()],
            SummaryContextScene::Compaction,
        );
        assert!(summary.contains("已完成前端任务编辑器重构"));
        assert!(summary.contains("## 未完事项"));
        assert!(summary.contains("1. 继续改 archive pipeline"));
        assert!(summary.contains("2. 补充 JSON 契约测试"));
    }

    #[test]
    fn build_compaction_message_should_use_markdown_sections() {
        let message = build_compaction_message(
            "## 当前进展\n\n- 已完成摘要格式优化",
            Some("摘要格式"),
            "",
            None,
            &[],
        );
        let text = render_message_content_for_model(&message);

        assert!(!text.contains("## 用户画像"));
        assert!(text.contains("## 摘要正文"));
        assert!(text.contains("## 当前进展\n\n- 已完成摘要格式优化"));
        assert!(!text.contains("[上下文整理]"));
        assert!(!text.contains("## 运行中的后台任务"));
    }

    #[test]
    fn build_compaction_message_should_carry_background_shell_lines() {
        let message = build_compaction_message(
            "## 当前进展\n\n- 完成",
            None,
            "",
            None,
            &["- bg-shell-abc：description=跑 dev server，command=pnpm tauri dev".to_string()],
        );
        let text = render_message_content_for_model(&message);

        assert!(text.contains("## 运行中的后台任务"));
        assert!(text.contains("bg-shell-abc"));
        assert!(text.contains("pnpm tauri dev"));
    }

    #[test]
    fn compaction_history_flushed_messages_should_keep_boundary_before_summary() {
        let checkpoint = test_message("assistant-checkpoint", "assistant", "压缩前流式草稿");
        let duplicate_checkpoint =
            test_message("assistant-checkpoint", "assistant", "重复边界消息");
        let compression = test_message("compaction-summary", "assistant", "压缩摘要");

        let messages = build_compaction_history_flushed_messages(
            &[checkpoint.clone(), duplicate_checkpoint],
            &compression,
        );
        let ids = messages
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["assistant-checkpoint", "compaction-summary"]);
        assert_eq!(
            render_message_content_for_model(&messages[0]),
            "压缩前流式草稿"
        );
    }

    #[test]
    fn preserved_dialogue_should_stop_at_latest_token_budget() {
        let mut source = test_conversation();
        source.messages = vec![
            test_message(
                "m1",
                "user",
                "这是更早的一条用户短消息，用来确认不要被很长的助手回复挤出上下文窗口",
            ),
            test_message(
                "m2",
                "assistant",
                "这是更早的一条助手消息，需要被截断保留前缀以便下一轮知道对话脉络，并且这条消息故意写得很长，确保超过五十个字后会出现省略号",
            ),
            test_message(
                "m3",
                "assistant",
                "这是最近的一条超长助手回复。".repeat(200).as_str(),
            ),
        ];

        let latest_line = preserved_dialogue_message_line(
            source.messages.last().expect("latest message"),
            "用户",
            "PAI",
        )
        .expect("latest preserved line");
        let budget = estimated_tokens_for_text(&latest_line).ceil() as usize;
        let block = collect_block_preserved_dialogue(
            &source.messages,
            "用户",
            "PAI",
            PreservedDialogueBudget::Tokens(budget),
        );

        assert!(block.contains("PAI：这是最近的一条超长助手回复"));
        assert!(!block.contains("用户：这是更早的一条用户短消息"));
        assert!(!block.contains("PAI：这是更早的一条助手消息"));
    }

    #[test]
    fn resolve_memory_action_drafts_should_not_cap_actions_at_seven() {
        let drafts = (0..8)
            .map(|idx| ArchiveMemoryActionDraft {
                action: ArchiveMemoryActionKind::Create,
                source_memory_ids: Vec::new(),
                memory: ArchiveMemoryDraft {
                    memory_type: "knowledge".to_string(),
                    judgment: format!("测试记忆 {}", idx),
                    reasoning: "测试依据".to_string(),
                    tags: vec!["测试".to_string()],
                },
            })
            .collect::<Vec<_>>();
        let id_alias_map = HashMap::<String, String>::new();

        let resolved = resolve_memory_action_drafts(&drafts, &id_alias_map);

        assert_eq!(resolved.len(), 8);
    }

    #[test]
    fn decorate_manual_archive_failure_reason_should_suggest_delete_for_unrepairable_store_error() {
        let decorated = decorate_manual_archive_failure_reason(
            "校验会话块失败，conversation_id=test".to_string(),
        );

        assert!(decorated.contains("可直接删除该会话"));
    }

    #[test]
    fn off_memory_recall_mode_should_discard_archive_memory_actions() {
        let root = std::env::temp_dir()
            .join("easy_call_ai_tests")
            .join(format!("archive_off_mode_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        let data_path = root.join("config_mark");

        let mut host_agent = default_agent();
        host_agent.memory_recall_mode = MEMORY_RECALL_MODE_OFF.to_string();
        let draft = MemoryCurationDraft {
            title: String::new(),
            summary: String::new(),
            open_loops: Vec::new(),
            useful_memory_ids: Vec::new(),
            memory_actions: vec![ArchiveMemoryActionDraft {
                action: ArchiveMemoryActionKind::Create,
                source_memory_ids: Vec::new(),
                memory: ArchiveMemoryDraft {
                    memory_type: "knowledge".to_string(),
                    judgment: "完全关闭模式下归档记忆不应落库".to_string(),
                    reasoning: "回归测试".to_string(),
                    tags: vec!["关闭模式".to_string(), "回归".to_string()],
                },
            }],
        };

        let report = apply_summary_context_result(&data_path, &host_agent, &[], &draft)
            .expect("apply off mode report");

        assert_eq!(report.merged_memories, 0);
        assert_eq!(report.applied_profile_memories, 0);
        let memories = memory_store_list_memories(&data_path).expect("list memories");
        assert!(memories.is_empty());
    }

    #[test]
    fn decorate_manual_archive_failure_reason_should_not_suggest_delete_for_normal_error() {
        let decorated = decorate_manual_archive_failure_reason(
            "强制归档正在进行中，请稍候。".to_string(),
        );

        assert!(!decorated.contains("可直接删除该会话"));
    }
}