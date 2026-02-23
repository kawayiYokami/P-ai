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
        user_alias,
        "",
        "concise",
        "zh-CN",
        None,
        None,
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
    let parsed = parse_archive_summary_draft(&reply.assistant_text).ok_or_else(|| {
        format!(
            "Parse archive summary JSON failed. raw={}",
            reply.assistant_text.chars().take(240).collect::<String>()
        )
    })?;
    let summary = clean_text(parsed.summary.trim());
    if summary.is_empty() {
        return Err("Archive summary is empty".to_string());
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

#[tauri::command]
async fn force_archive_current(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<ForceArchiveResult, String> {
    let (selected_api, resolved_api, source, effective_agent_id) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;
        let app_config = read_config(&state.config_path)?;
        let selected_api = resolve_selected_api_config(&app_config, input.api_config_id.as_deref())
            .ok_or_else(|| "No API config configured. Please add one.".to_string())?;
        let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
        let mut data = read_app_data(&state.data_path)?;
        ensure_default_agent(&mut data);
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
            .any(|a| a.id == data.selected_agent_id && !a.is_built_in_user)
        {
            data.selected_agent_id.clone()
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

    run_archive_pipeline(
        &state,
        &selected_api,
        &resolved_api,
        &source,
        &effective_agent_id,
        "manual_force_archive",
        "ARCHIVE-FORCE",
    )
    .await
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

    if source.messages.is_empty() {
        return Ok(ForceArchiveResult {
            archived: false,
            archive_id: None,
            summary: "当前对话为空，无需归档。".to_string(),
            merged_memories: 0,
            warning: None,
            reason_code: Some("empty_conversation".to_string()),
            elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
            memory_feedback: None,
            merge_groups: None,
        });
    }

    let (host_agent, host_agent_id, user_alias, memories) = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|_| "Failed to lock state mutex".to_string())?;
        let mut data = read_app_data(&state.data_path)?;
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

    let parsed = summarize_archived_conversation_with_model_v2(
        state,
        resolved_api,
        selected_api,
        &host_agent,
        &user_alias,
        source,
        &memories,
        &source.memory_recall_table,
        trace_tag,
        &trace_id,
    )
    .await?;
    let summary = parsed.summary;
    let useful_memory_ids = parsed.useful_memory_ids;
    let summary_memories = parsed.new_memories;
    let merge_groups = parsed.merge_groups;

    let guard = state
        .state_lock
        .lock()
        .map_err(|_| "Failed to lock state mutex".to_string())?;
    let mut data = read_app_data(&state.data_path)?;
    ensure_default_agent(&mut data);
    let archive_id = archive_conversation_now(&mut data, &source.id, archive_reason, &summary)
        .ok_or_else(|| "活动对话已变化，请重试归档。".to_string())?;
    let active_idx = ensure_active_conversation_index(&mut data, &selected_api.id, &source.agent_id);
    if data.conversations.get(active_idx).is_none() {
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
    write_app_data(&state.data_path, &data)?;
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
        summary,
        merged_memories,
        warning: None,
        reason_code: None,
        elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        memory_feedback: Some(memory_feedback),
        merge_groups: Some(merged_groups),
    })
}
