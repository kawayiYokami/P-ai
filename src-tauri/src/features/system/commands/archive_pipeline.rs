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
        let upserted_ids =
            upsert_memories_into_store_with_ids(data_path, &[group.target.clone()], owner_agent_id)?;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForceArchiveResult {
    archived: bool,
    archive_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_conversation_id: Option<String>,
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

const SHORT_CONVERSATION_DELETE_THRESHOLD: usize = 3;

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

async fn summarize_archived_conversation_with_model_v2(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    user_alias: &str,
    source_conversation: &Conversation,
    memories: &[MemoryEntry],
    recall_table: &[String],
    _trace_tag: &str,
    _trace_id: &str,
) -> Result<ArchiveSummaryDraft, String> {
    let used_memories = archive_used_memories_block(memories, recall_table);

    let instruction = build_archive_instruction(agent, user_alias);

    let mut prepared = build_prepared_prompt_for_mode(
        PromptBuildMode::Archive,
        source_conversation,
        agent,
        &[],
        &[],
        user_alias,
        "",
        "concise",
        "zh-CN",
        None,
        None,
        None,
        None,
        Some(state),
        None,
        None,
    );
    prepared.latest_user_text =
        build_archive_latest_user_text(&instruction, &used_memories, archive_example_output_block());
    let timeout_secs = 360u64;

    let reply = call_archive_summary_model_with_timeout(
        state,
        resolved_api,
        selected_api,
        prepared,
        timeout_secs,
    )
    .await?;
    let parsed = match parse_archive_summary_draft(&reply.assistant_text) {
        Some(parsed) => parsed,
        None => {
            let fallback_summary = clean_text(reply.assistant_text.trim());
            if fallback_summary.is_empty() {
                return Err("Archive summary is empty".to_string());
            }
            eprintln!(
                "[ARCHIVE-PIPELINE] parse archive JSON failed; fallback to raw summary and skip memory generation. raw={}",
                reply.assistant_text.chars().take(240).collect::<String>()
            );
            return Ok(ArchiveSummaryDraft {
                summary: fallback_summary,
                useful_memory_ids: Vec::new(),
                new_memories: Vec::new(),
                merge_groups: Vec::new(),
            });
        }
    };
    let summary = clean_text(parsed.summary.trim());
    if summary.is_empty() {
        let fallback_summary = clean_text(reply.assistant_text.trim());
        if fallback_summary.is_empty() {
            return Err("Archive summary is empty".to_string());
        }
        return Ok(ArchiveSummaryDraft {
            summary: fallback_summary,
            useful_memory_ids: parsed
                .useful_memory_ids
                .into_iter()
                .map(|id| id.trim().to_string())
                .filter(|id| !id.is_empty())
                .collect::<Vec<_>>(),
            new_memories: parsed.new_memories.into_iter().take(7).collect::<Vec<_>>(),
            merge_groups: parsed.merge_groups.into_iter().take(7).collect::<Vec<_>>(),
        });
    }
    Ok(ArchiveSummaryDraft {
        summary,
        useful_memory_ids: parsed
            .useful_memory_ids
            .into_iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect::<Vec<_>>(),
        new_memories: parsed.new_memories.into_iter().take(7).collect::<Vec<_>>(),
        merge_groups: parsed.merge_groups.into_iter().take(7).collect::<Vec<_>>(),
    })
}

async fn summarize_context_compaction_with_model_v1(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    agent: &AgentProfile,
    user_alias: &str,
    source_conversation: &Conversation,
) -> Result<String, String> {
    let mut prepared = build_prepared_prompt_for_mode(
        PromptBuildMode::Archive,
        source_conversation,
        agent,
        &[],
        &[],
        user_alias,
        "",
        "concise",
        "zh-CN",
        None,
        None,
        None,
        None,
        Some(state),
        None,
        None,
    );
    prepared.latest_user_text = build_compaction_instruction().to_string();
    let timeout_secs = 360u64;
    let reply = call_archive_summary_model_with_timeout(
        state,
        resolved_api,
        selected_api,
        prepared,
        timeout_secs,
    )
    .await?;
    let summary = clean_text(reply.assistant_text.trim());
    if summary.is_empty() {
        return Err("Compaction summary is empty".to_string());
    }
    Ok(summary)
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

fn archive_pipeline_is_compression_text(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    lower.contains("上下文压缩")
        || lower.contains("压缩摘要")
        || lower.contains("context compression")
        || lower.contains("context compact")
}

fn archive_pipeline_collect_compression_texts(source: &Conversation) -> Vec<String> {
    let mut blocks = Vec::<String>::new();
    for message in &source.messages {
        if message.role.trim() != "user" && message.role.trim() != "assistant" {
            continue;
        }
        let text = archive_pipeline_message_plain_text(message);
        if text.is_empty() || !archive_pipeline_is_compression_text(&text) {
            continue;
        }
        blocks.push(text);
    }
    blocks
}

fn build_archive_summary_from_compression_and_last_three_rounds(
    source: &Conversation,
) -> Result<String, String> {
    let compression_blocks = archive_pipeline_collect_compression_texts(source);
    if compression_blocks.is_empty() {
        return Err("no compression messages found".to_string());
    }
    let recent = archive_pipeline_recent_user_assistant_messages(source, 6);
    let mut lines = Vec::<String>::new();
    lines.push("归档降级摘要（压缩内容 + 最后三轮正文对话）：".to_string());
    lines.push("【压缩内容】".to_string());
    for (idx, block) in compression_blocks.iter().enumerate() {
        let snippet = block.chars().take(360).collect::<String>();
        lines.push(format!("C{}. {}", idx + 1, snippet));
    }
    lines.push("【最近对话】".to_string());
    if recent.is_empty() {
        lines.push("最近对话暂无可用正文。".to_string());
    } else {
        for (idx, (role, text)) in recent.iter().enumerate() {
            let speaker = if role == "user" { "用户" } else { "助理" };
            let snippet = text.chars().take(240).collect::<String>();
            lines.push(format!("{}. {}：{}", idx + 1, speaker, snippet));
        }
    }
    Ok(lines.join("\n"))
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
    let app_handle = match state
        .app_handle
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
    {
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
}

fn emit_compaction_history_flushed_event(
    state: &AppState,
    conversation_id: &str,
    compression_message: &ChatMessage,
) {
    let app_handle = match state
        .app_handle
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
    {
        Some(handle) => handle,
        None => {
            eprintln!(
                "[ARCHIVE-PIPELINE] 压缩 history_flushed 发送跳过: app_handle 不可用, conversation_id={}",
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
            "[ARCHIVE-PIPELINE] 压缩 history_flushed 发送失败: conversation_id={}, error={}",
            conversation_id, err
        );
    } else {
        eprintln!(
            "[ARCHIVE-PIPELINE] 压缩 history_flushed 已发送: conversation_id={}",
            conversation_id
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
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(state)?;
    ensure_default_agent(&mut data);

    let before = data.conversations.len();
    data.conversations.retain(|conversation| {
        !(conversation.id == source.id
            && conversation.summary.trim().is_empty()
            && !conversation_is_delegate(conversation))
    });
    if data.conversations.len() == before {
        return Err("活动对话已变化，请重试归档。".to_string());
    }

    let active_idx = if let Some(existing_idx) = latest_main_conversation_index(&data, "") {
        for (idx, conversation) in data.conversations.iter_mut().enumerate() {
            if conversation_is_delegate(conversation) || !conversation.summary.trim().is_empty() {
                continue;
            }
            conversation.status = if idx == existing_idx {
                "active".to_string()
            } else {
                "inactive".to_string()
            };
        }
        existing_idx
    } else {
        ensure_active_conversation_index(&mut data, &selected_api.id, "")
    };
    let active_conversation_id = data
        .conversations
        .get(active_idx)
        .map(|item| item.id.clone())
        .ok_or_else(|| "Failed to ensure active conversation after delete.".to_string())?;
    state_write_app_data_cached(state, &data)?;
    drop(guard);

    cleanup_pdf_session_memory_cache_for_conversation(&source.id);
    Ok(active_conversation_id)
}

fn build_compaction_message(summary: &str, compaction_reason: &str) -> ChatMessage {
    let now = now_iso();
    let reason = compaction_reason.trim();
    let reason_line = if reason.is_empty() {
        String::new()
    } else {
        format!("触发原因：{}\n", reason)
    };
    let text = format!(
        "[上下文压缩]\n{}压缩摘要：\n{}",
        reason_line,
        clean_text(summary.trim())
    );
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now,
        speaker_agent_id: None,
        parts: vec![MessagePart::Text { text }],
        extra_text_blocks: Vec::new(),
        provider_meta: None,
        tool_call: None,
        mcp_call: None,
    }
}

async fn summarize_archive_draft_with_fallback(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    host_agent: &AgentProfile,
    user_alias: &str,
    source: &Conversation,
    memories: &[MemoryEntry],
    trace_tag: &str,
    trace_id: &str,
) -> (ArchiveSummaryDraft, Option<String>) {
    match summarize_archived_conversation_with_model_v2(
        state,
        resolved_api,
        selected_api,
        host_agent,
        user_alias,
        source,
        memories,
        &source.memory_recall_table,
        trace_tag,
        trace_id,
    )
    .await
    {
        Ok(parsed) => (parsed, None),
        Err(err) => {
            let fallback_summary = match build_archive_summary_from_compression_and_last_three_rounds(source) {
                Ok(summary) => {
                    eprintln!(
                        "[ARCHIVE-PIPELINE] 归档模型失败，已降级到 压缩内容+最后三轮 正文摘要: trace_id={}, conversation_id={}, err={}",
                        trace_id, source.id, err
                    );
                    (
                        summary,
                        Some(format!("归档模型失败，已使用压缩内容+最后三轮正文对话降级摘要：{err}")),
                    )
                }
                Err(compression_err) => {
                    eprintln!(
                        "[ARCHIVE-PIPELINE] 归档模型失败，压缩降级也失败，已降级到 最后三轮 正文摘要: trace_id={}, conversation_id={}, err={}, compression_err={}",
                        trace_id, source.id, err, compression_err
                    );
                    (
                        build_archive_summary_from_last_three_rounds(source),
                        Some(format!(
                            "归档模型失败，压缩降级失败，已使用最后三轮正文对话降级摘要：{}（压缩降级原因：{}）",
                            err, compression_err
                        )),
                    )
                }
            };
            (
                ArchiveSummaryDraft {
                    summary: fallback_summary.0,
                    useful_memory_ids: Vec::new(),
                    new_memories: Vec::new(),
                    merge_groups: Vec::new(),
                },
                fallback_summary.1,
            )
        }
    }
}

async fn summarize_compaction_text_with_fallback(
    state: &AppState,
    resolved_api: &ResolvedApiConfig,
    selected_api: &ApiConfig,
    host_agent: &AgentProfile,
    user_alias: &str,
    source: &Conversation,
    trace_id: &str,
) -> (String, Option<String>) {
    match summarize_context_compaction_with_model_v1(
        state,
        resolved_api,
        selected_api,
        host_agent,
        user_alias,
        source,
    )
    .await
    {
        Ok(summary) => (summary, None),
        Err(err) => match build_archive_summary_from_compression_and_last_three_rounds(source) {
            Ok(summary) => {
                eprintln!(
                    "[ARCHIVE-PIPELINE] 压缩模型失败，已降级到 压缩内容+最后三轮 正文摘要: trace_id={}, conversation_id={}, err={}",
                    trace_id, source.id, err
                );
                (
                    summary,
                    Some(format!("压缩模型失败，已使用压缩内容+最后三轮正文对话降级摘要：{err}")),
                )
            }
            Err(compression_err) => {
                eprintln!(
                    "[ARCHIVE-PIPELINE] 压缩模型失败，压缩降级也失败，已降级到 最后三轮 正文摘要: trace_id={}, conversation_id={}, err={}, compression_err={}",
                    trace_id, source.id, err, compression_err
                );
                (
                    build_archive_summary_from_last_three_rounds(source),
                    Some(format!(
                        "压缩模型失败，压缩降级失败，已使用最后三轮正文对话降级摘要：{}（压缩降级原因：{}）",
                        err, compression_err
                    )),
                )
            }
        },
    }
}

#[tauri::command]
async fn force_archive_current(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ForceArchiveResult, String> {
    let (selected_api, resolved_api, source, effective_agent_id) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut app_config = read_config(&state.config_path)?;
        let mut data = state_read_app_data_cached(&state)?;
        ensure_default_agent(&mut data);
        merge_private_organization_into_runtime_data(&state.data_path, &mut app_config, &mut data)?;
        let selected_api = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
            .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        let requested_agent_id = input.agent_id.trim();
        let effective_agent_id = if data
            .agents
            .iter()
            .any(|a| a.id == requested_agent_id && !a.is_built_in_user)
        {
            requested_agent_id.to_string()
        } else if data
            .agents
            .iter()
            .any(|a| a.id == data.assistant_department_agent_id && !a.is_built_in_user)
        {
            data.assistant_department_agent_id.clone()
        } else {
            data.agents
                .iter()
                .find(|a| !a.is_built_in_user)
                .map(|a| a.id.clone())
                .ok_or_else(|| "Selected agent not found.".to_string())?
        };
        let source_idx = latest_active_conversation_index(&data, &selected_api.id, &effective_agent_id)
            .ok_or_else(|| "当前没有可归档的活动对话。".to_string())?;
        let source = data
            .conversations
            .get(source_idx)
            .cloned()
            .ok_or_else(|| "当前没有可归档的活动对话。".to_string())?;
        drop(guard);
        (selected_api, resolved_api, source, effective_agent_id)
    };

    let result = run_archive_pipeline(
        &state,
        &selected_api,
        &resolved_api,
        &source,
        &effective_agent_id,
        "manual_force_archive",
        "ARCHIVE-FORCE",
    )
    .await;
    trigger_chat_queue_processing(state.inner());
    result
}

pub(crate) async fn run_archive_pipeline(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    effective_agent_id: &str,
    archive_reason: &str,
    trace_tag: &str,
) -> Result<ForceArchiveResult, String> {
    let started_at = std::time::Instant::now();
    let trace_id = Uuid::new_v4().to_string();

    // 设置状态为 OrganizingContext
    set_main_session_state(state, MainSessionState::OrganizingContext)?;
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
        archive_reason,
        trace_tag,
        started_at,
        &trace_id,
    ).await;

    // 归档完成，切换回 Idle（即使内部失败也要恢复状态）
    let elapsed_ms = started_at.elapsed().as_millis();
    if let Err(state_err) = set_main_session_state(state, MainSessionState::Idle) {
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

    set_main_session_state(state, MainSessionState::OrganizingContext)?;
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
    if let Err(state_err) = set_main_session_state(state, MainSessionState::Idle) {
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
    effective_agent_id: &str,
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
            summary: "当前对话为空，无需压缩。".to_string(),
            merged_memories: 0,
            warning: None,
            reason_code: Some("empty_conversation".to_string()),
            elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            memory_feedback: None,
            merge_groups: None,
        });
    }

    let (host_agent, host_agent_id, user_alias) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut data = state_read_app_data_cached(&state)?;
        ensure_default_agent(&mut data);
        let user_alias = data.user_alias.clone();
        let host_agent_id = choose_archive_host_agent_id(&data, source, effective_agent_id);
        let host_agent = data
            .agents
            .iter()
            .find(|a| a.id == host_agent_id)
            .cloned()
            .ok_or_else(|| "Host agent not found.".to_string())?;
        drop(guard);
        (host_agent, host_agent_id, user_alias)
    };

    eprintln!(
        "[{}] trace={} begin api={} model={} format={} conversation={} hostAgent={}",
        trace_tag,
        trace_id,
        selected_api.id,
        selected_api.model,
        resolved_api.request_format,
        source.id,
        host_agent_id
    );

    let (summary, compaction_warning) = summarize_compaction_text_with_fallback(
        state,
        resolved_api,
        selected_api,
        &host_agent,
        &user_alias,
        source,
        trace_id,
    )
    .await;

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    ensure_default_agent(&mut data);

    let conversation_idx = data
        .conversations
        .iter()
        .position(|item| item.id == source.id && item.summary.trim().is_empty())
        .ok_or_else(|| "活动对话已变化，请重试压缩。".to_string())?;
    let compression_message = build_compaction_message(&summary, compaction_reason);
    {
        let conversation = data
            .conversations
            .get_mut(conversation_idx)
            .ok_or_else(|| "活动对话索引无效，请重试压缩。".to_string())?;
        conversation.messages.push(compression_message.clone());
        let now = now_iso();
        conversation.updated_at = now.clone();
        conversation.last_user_at = Some(now);
        conversation.last_context_usage_ratio =
            compute_context_usage_ratio(conversation, selected_api.context_window_tokens);
    }
    let active_conversation_id = data
        .conversations
        .get(conversation_idx)
        .map(|item| item.id.clone());
    state_write_app_data_cached(&state, &data)?;

    emit_compaction_history_flushed_event(state, &source.id, &compression_message);

    drop(guard);

    eprintln!(
        "[{}] trace={} done compaction=true merged_memories=0 merged_groups=0",
        trace_tag,
        trace_id,
    );

    Ok(ForceArchiveResult {
        archived: false,
        archive_id: None,
        active_conversation_id,
        summary,
        merged_memories: 0,
        warning: compaction_warning,
        reason_code: None,
        elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        memory_feedback: None,
        merge_groups: Some(0),
    })
}

async fn run_archive_pipeline_inner(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    effective_agent_id: &str,
    archive_reason: &str,
    trace_tag: &str,
    started_at: std::time::Instant,
    trace_id: &str,
) -> Result<ForceArchiveResult, String> {
    if source.messages.is_empty() {
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
            summary: String::new(),
            merged_memories: 0,
            warning: None,
            reason_code: Some("empty_conversation_deleted".to_string()),
            elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            memory_feedback: None,
            merge_groups: None,
        });
    }

    let message_count = archive_pipeline_message_count_for_delete(source);
    if message_count <= SHORT_CONVERSATION_DELETE_THRESHOLD {
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
            summary: String::new(),
            merged_memories: 0,
            warning: None,
            reason_code: Some("short_conversation_deleted".to_string()),
            elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            memory_feedback: None,
            merge_groups: None,
        });
    }

    let (host_agent, host_agent_id, user_alias, memories) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let mut data = state_read_app_data_cached(&state)?;
        ensure_default_agent(&mut data);
        let user_alias = data.user_alias.clone();
        let host_agent_id = choose_archive_host_agent_id(&data, source, effective_agent_id);
        let host_agent = data
            .agents
            .iter()
            .find(|a| a.id == host_agent_id)
            .cloned()
            .ok_or_else(|| "Host agent not found.".to_string())?;
        let host_private_memory_enabled = host_agent.private_memory_enabled;
        drop(guard);
        let memories = memory_store_list_memories_visible_for_agent(
            &state.data_path,
            &host_agent_id,
            host_private_memory_enabled,
        )?;
        (host_agent, host_agent_id, user_alias, memories)
    };

    eprintln!(
        "[{}] trace={} begin api={} model={} format={} conversation={} hostAgent={}",
        trace_tag,
        trace_id,
        selected_api.id,
        selected_api.model,
        resolved_api.request_format,
        source.id,
        host_agent_id
    );

    let (parsed, archive_warning) = summarize_archive_draft_with_fallback(
        state,
        resolved_api,
        selected_api,
        &host_agent,
        &user_alias,
        source,
        &memories,
        trace_tag,
        &trace_id,
    )
    .await;
    let summary = parsed.summary;
    let useful_memory_ids = parsed.useful_memory_ids;
    let summary_memories = parsed.new_memories;
    let merge_groups = parsed.merge_groups;

    let guard = state
        .state_lock
        .lock()
        .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
    let mut data = state_read_app_data_cached(&state)?;
    ensure_default_agent(&mut data);
    let archive_id = archive_conversation_now(&mut data, &source.id, archive_reason, &summary)
        .ok_or_else(|| "活动对话已变化，请重试归档。".to_string())?;
    let active_idx = ensure_active_conversation_index(&mut data, &selected_api.id, "");
    let active_conversation_id = data
        .conversations
        .get(active_idx)
        .map(|item| item.id.clone());
    if active_conversation_id.is_none() {
        eprintln!(
            "[ARCHIVE-PIPELINE] ensure active conversation index invalid: api={}, agent={}, idx={}",
            selected_api.id, source.agent_id, active_idx
        );
        return Err("Failed to ensure active conversation after archive.".to_string());
    }
    let owner_agent_id = data
        .agents
        .iter()
        .find(|a| a.id == host_agent_id && !a.is_built_in_user && a.private_memory_enabled)
        .map(|a| a.id.as_str());
    let memory_feedback = memory_store_apply_archive_feedback(
        &state.data_path,
        &source.memory_recall_table,
        &useful_memory_ids,
    )?;
    let merged_memories =
        merge_memories_into_store(&state.data_path, &summary_memories, owner_agent_id)?;
    let merged_groups =
        merge_memory_groups_into_store(&state.data_path, &merge_groups, owner_agent_id)?;
    state_write_app_data_cached(&state, &data)?;

    // 清理PDF缓存
    if let Err(e) = cleanup_pdf_cache_for_conversation(&state, &source.id) {
        eprintln!("[归档] 清理 PDF 缓存失败: conversation={}, error={}", source.id, e);
    }

    if let Some(active_conversation_id_value) = active_conversation_id.as_deref() {
        emit_archive_history_flushed_event(
            state,
            &source.id,
            active_conversation_id_value,
            &archive_id,
            archive_reason,
        );
    }

    drop(guard);

    eprintln!(
        "[{}] trace={} done archived=true merged_memories={} merged_groups={} useful_accept={} penalized={} natural_decay={}",
        trace_tag,
        trace_id,
        merged_memories,
        merged_groups,
        memory_feedback.useful_accepted_count,
        memory_feedback.penalized_count,
        memory_feedback.natural_decay_count,
    );

    Ok(ForceArchiveResult {
        archived: true,
        archive_id: Some(archive_id),
        active_conversation_id,
        summary,
        merged_memories,
        warning: archive_warning,
        reason_code: None,
        elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        memory_feedback: Some(memory_feedback),
        merge_groups: Some(merged_groups),
    })
}
