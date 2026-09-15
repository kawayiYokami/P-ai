#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationIdOnlyInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationCommandStatus {
    success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchArchiveConversationsInput {
    conversation_ids: Vec<String>,
    #[serde(alias = "apiConfigId")]
    reflection_api_config_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchArchiveSkippedConversation {
    conversation_id: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BatchArchiveConversationsOutput {
    success: bool,
    accepted_conversation_ids: Vec<String>,
    skipped: Vec<BatchArchiveSkippedConversation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    active_conversation_id: Option<String>,
}

#[tauri::command]
async fn archive_conversation(
    input: ConversationIdOnlyInput,
    state: State<'_, AppState>,
) -> Result<ConversationCommandStatus, String> {
    archive_conversation_inner(input, state.inner()).await
}

async fn archive_conversation_inner(
    input: ConversationIdOnlyInput,
    state: &AppState,
) -> Result<ConversationCommandStatus, String> {
    let requested_conversation_id = input.conversation_id.trim();
    if requested_conversation_id.is_empty() {
        return Err("conversationId is required".to_string());
    }
    runtime_log_info(format!(
        "[归档] 开始，任务=手动归档，conversation_id={}",
        requested_conversation_id
    ));
    let (selected_api, resolved_api, source, effective_agent_id) =
        match resolve_archive_request_conversation_by_id(state, requested_conversation_id) {
            Ok(resolved) => resolved,
            Err(err) => return Err(log_manual_archive_failure(requested_conversation_id, err)),
        };
    let already_archived = conversation_is_archived(&source);
    let main_conversation_id = {
        let state = state.clone();
        tokio::task::spawn_blocking(move || state_service_get_main_conversation_id(&state))
            .await
            .map_err(|err| log_manual_archive_failure(&source.id, format!("读取主会话 ID 失败：error={err}")))?
            .map_err(|err| log_manual_archive_failure(&source.id, err))?
    }
    .unwrap_or_default();
    if !already_archived && source.id.trim() == main_conversation_id {
        return Err(log_manual_archive_failure(
            &source.id,
            "系统通知会话暂不支持归档。".to_string(),
        ));
    }
    let conversation_runtime_state = get_conversation_runtime_state(state, &source.id)
        .map_err(|err| log_manual_archive_failure(&source.id, err))?;
    if !already_archived {
        let disabled_reason = match conversation_runtime_state {
            MainSessionState::AssistantStreaming => Some("当前会话正在流式输出，请稍后再归档。"),
            MainSessionState::OrganizingContext => Some("强制归档正在进行中，请稍候。"),
            MainSessionState::Idle => None,
        };
        if let Some(reason) = disabled_reason {
            return Err(log_manual_archive_failure(&source.id, reason.to_string()));
        }
    }
    let archive_result = instant_archive_conversation(state, &selected_api, &source)
        .map_err(|err| log_manual_archive_failure(&source.id, err))?;
    flush_pending_persists_blocking(state).map_err(|err| {
        log_manual_archive_failure(&source.id, format!("归档状态写入失败：{}", err))
    })?;
    // 归档语义：注册 watermark 删除（列表不再包含已归档会话），不再全量广播。
    if !archive_result.already_archived {
        overview_register_missing_item(&source.id);
    }
    let active_conversation_id = archive_result.active_conversation_id.clone();

    if !archive_result.already_archived {
        let state_cloned = state.clone();
        let selected_api_cloned = selected_api.clone();
        let resolved_api_cloned = resolved_api.clone();
        let source_conversation_id = source.id.clone();
        let effective_agent_id_cloned = effective_agent_id.clone();
        let active_conversation_id_for_background = active_conversation_id.clone();
        tauri::async_runtime::spawn(async move {
            let panic_safe_task = std::panic::AssertUnwindSafe(async {
                let source_cloned = match conversation_service_v2()
                    .read_archive_pipeline_source_conversation(
                        &state_cloned,
                        &source_conversation_id,
                    ) {
                    Ok(conversation) => conversation,
                    Err(err) => {
                        runtime_log_warn(format!(
                            "[归档] 失败，任务=后台归档维护，conversation_id={}，error=读取归档流水线源会话失败：{}",
                            source_conversation_id, err
                        ));
                        trigger_chat_queue_processing(&state_cloned);
                        return;
                    }
                };
                let result = run_archive_pipeline(
                    &state_cloned,
                    &selected_api_cloned,
                    &resolved_api_cloned,
                    &source_cloned,
                    &effective_agent_id_cloned,
                    Some(active_conversation_id_for_background.as_str()),
                    None,
                    "archive_conversation",
                    "ARCHIVE-FORCE",
                )
                .await;
                if let Err(err) = result {
                    runtime_log_warn(format!(
                        "[归档] 失败，任务=后台归档维护，conversation_id={}，error={}",
                        source_cloned.id, err
                    ));
                }
                trigger_chat_queue_processing(&state_cloned);
            });
            if futures_util::FutureExt::catch_unwind(panic_safe_task)
                .await
                .is_err()
            {
                runtime_log_error(format!(
                    "[归档] 失败，任务=后台归档维护，conversation_id={}，error=panic",
                    source_conversation_id
                ));
                trigger_chat_queue_processing(&state_cloned);
            }
        });
    }

    runtime_log_info(format!(
        "[归档] 完成，任务=手动归档，conversation_id={}，already_archived={}",
        source.id, archive_result.already_archived
    ));
    Ok(ConversationCommandStatus { success: true })
}

#[tauri::command]
async fn batch_archive_conversations(
    input: BatchArchiveConversationsInput,
    state: State<'_, AppState>,
) -> Result<BatchArchiveConversationsOutput, String> {
    batch_archive_conversations_inner(state.inner(), input).await
}

pub(crate) async fn batch_archive_conversations_inner(
    state: &AppState,
    input: BatchArchiveConversationsInput,
) -> Result<BatchArchiveConversationsOutput, String> {
    let started_at = std::time::Instant::now();
    let conversation_ids = normalize_batch_archive_conversation_ids(&input.conversation_ids);
    if conversation_ids.is_empty() {
        return Err("conversationIds is required".to_string());
    }
    let (reflection_api, reflection_resolved_api) =
        resolve_batch_archive_reflection_api_config(state, input.reflection_api_config_id.as_str())?;
    runtime_log_info(format!(
        "[批量归档] 开始，任务=批量归档，即时标记，reflection_api_config_id={}，requested_count={}",
        reflection_api.id,
        conversation_ids.len()
    ));

    let main_conversation_id = {
        let state = state.clone();
        tokio::task::spawn_blocking(move || state_service_get_main_conversation_id(&state))
            .await
            .map_err(|err| format!("读取主会话 ID 失败：error={err}"))??
            .unwrap_or_default()
    };
    let mut accepted = Vec::<BatchArchiveAcceptedConversation>::new();
    let mut skipped = Vec::<BatchArchiveSkippedConversation>::new();
    let mut latest_active_conversation_id = None::<String>;

    for conversation_id in conversation_ids {
        match prepare_batch_archive_conversation(
            state,
            &conversation_id,
            main_conversation_id.as_str(),
        ) {
            Ok((source, effective_agent_id)) => {
                match instant_batch_archive_conversation_metadata_only(state, &reflection_api, &source) {
                    Ok(archive_result) => {
                        latest_active_conversation_id =
                            Some(archive_result.active_conversation_id.clone());
                        if !archive_result.already_archived {
                            accepted.push(BatchArchiveAcceptedConversation {
                                conversation_id: source.id,
                                effective_agent_id,
                            });
                        } else {
                            skipped.push(BatchArchiveSkippedConversation {
                                conversation_id: source.id,
                                reason: "会话已经归档。".to_string(),
                            });
                        }
                    }
                    Err(err) => skipped.push(BatchArchiveSkippedConversation {
                        conversation_id: source.id,
                        reason: decorate_manual_archive_failure_reason(err),
                    }),
                }
            }
            Err(reason) => skipped.push(BatchArchiveSkippedConversation {
                conversation_id,
                reason: decorate_manual_archive_failure_reason(reason),
            }),
        }
    }

    if !accepted.is_empty() {
        flush_pending_persists_blocking(state)
            .map_err(|err| format!("批量归档状态写入失败：{}", err))?;
    }
    // 批量归档：逐会话注册 watermark 删除语义，不再全量广播列表。
    for accepted_conversation_id in accepted.iter().map(|item| item.conversation_id.as_str()) {
        overview_register_missing_item(accepted_conversation_id);
    }

    let accepted_conversation_ids = accepted
        .iter()
        .map(|item| item.conversation_id.clone())
        .collect::<Vec<_>>();
    spawn_batch_archive_pipeline(
        state.clone(),
        reflection_api.clone(),
        reflection_resolved_api.clone(),
        accepted,
        latest_active_conversation_id.clone(),
    );
    runtime_log_warn(format!(
        "[批量归档] 完成，任务=批量归档，即时标记，reflection_api_config_id={}，accepted_count={}，skipped_count={}，duration_ms={}",
        reflection_api.id,
        accepted_conversation_ids.len(),
        skipped.len(),
        started_at.elapsed().as_millis()
    ));
    Ok(BatchArchiveConversationsOutput {
        success: true,
        accepted_conversation_ids,
        skipped,
        active_conversation_id: latest_active_conversation_id,
    })
}

pub(crate) async fn run_archive_pipeline(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    effective_agent_id: &str,
    prepared_active_conversation_id: Option<&str>,
    _target_conversation_id: Option<&str>,
    archive_reason: &str,
    trace_tag: &str,
) -> Result<ForceArchiveResult, String> {
    let started_at = std::time::Instant::now();
    let trace_id = Uuid::new_v4().to_string();
    let reflection_source = conversation_service_v2()
        .read_archive_pipeline_last_block_conversation(state, &source.id)
        .map_err(|err| format!("读取归档反思消息锚定上下文失败：{}", err))?;

    runtime_log_debug(format!(
        "[归档流程] 开始: task=archive_maintenance, trace_id={}, agent_id={}, api_id={}, started_at={}",
        trace_id, effective_agent_id, selected_api.id, started_at.elapsed().as_millis()
    ));

    let result = run_archive_pipeline_inner(
        state,
        selected_api,
        resolved_api,
        source,
        &reflection_source,
        effective_agent_id,
        prepared_active_conversation_id,
        None,
        archive_reason,
        trace_tag,
        started_at,
        &trace_id,
    )
    .await;

    let elapsed_ms = started_at.elapsed().as_millis();
    runtime_log_debug(format!(
        "[归档流程] 完成: task=archive_maintenance, trace_id={}, agent_id={}, api_id={}, elapsed_ms={}",
        trace_id, effective_agent_id, selected_api.id, elapsed_ms
    ));

    result
}

#[derive(Debug, Clone)]
struct BatchArchiveAcceptedConversation {
    conversation_id: String,
    effective_agent_id: String,
}

fn normalize_batch_archive_conversation_ids(values: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::<String>::new();
    let mut out = Vec::<String>::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            out.push(trimmed.to_string());
        }
    }
    out
}

fn resolve_batch_archive_reflection_api_config(
    state: &AppState,
    reflection_api_config_id: &str,
) -> Result<(ApiConfig, ResolvedApiConfig), String> {
    let normalized_api_config_id = reflection_api_config_id.trim();
    if normalized_api_config_id.is_empty() {
        return Err("reflectionApiConfigId is required".to_string());
    }
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let app_config = runtime_snapshot.config;
    let resolved_api_config_id = resolve_model_role_api_config_id(&app_config, normalized_api_config_id)
        .ok_or_else(|| "reflectionApiConfigId is required".to_string())?;
    let selected_api = app_config
        .api_configs
        .iter()
        .find(|api| api.id.trim() == resolved_api_config_id.trim())
        .cloned()
        .ok_or_else(|| format!("归档反思模型不存在：{}", normalized_api_config_id))?;
    if !selected_api.enable_text || !selected_api.request_format.is_chat_text() {
        return Err(format!("归档反思模型不支持文本对话：{}", selected_api.id));
    }
    let resolved_api = resolve_api_config(&app_config, Some(selected_api.id.as_str()))?;
    Ok((selected_api, resolved_api))
}

fn instant_batch_archive_conversation_metadata_only(
    state: &AppState,
    replacement_seed_api: &ApiConfig,
    source: &Conversation,
) -> Result<InstantArchiveConversationMutationResult, String> {
    conversation_service_v2().archive_conversation(
        state,
        replacement_seed_api,
        source,
        "batch_archive_conversations",
    )
}

fn prepare_batch_archive_conversation(
    state: &AppState,
    conversation_id: &str,
    main_conversation_id: &str,
) -> Result<(Conversation, String), String> {
    let normalized_conversation_id = conversation_id.trim();
    if normalized_conversation_id.is_empty() {
        return Err("conversationId is required".to_string());
    }
    let guard = state.conversation_lock.lock().map_err(|err| {
        format!(
            "Failed to lock state mutex at {}:{} {}: {err}",
            file!(),
            line!(),
            module_path!()
        )
    })?;
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let source_meta = conversation_service_v2()
        .get_conversation_meta(state, normalized_conversation_id)
        .map_err(|_| "当前没有可归档的活动对话。".to_string())?;
    if !conversation_service_v2().conversation_meta_is_local_normal_chat_meta_view(&source_meta)
        && source_meta.status.trim() != "archived"
    {
        drop(guard);
        return Err("当前没有可归档的活动对话。".to_string());
    }
    let source = conversation_service_v2().build_conversation_record_from_meta_view(&source_meta);
    if conversation_is_archived(&source) {
        drop(guard);
        return Err("会话已经归档。".to_string());
    }
    if source.id.trim() == main_conversation_id.trim() {
        drop(guard);
        return Err("系统通知会话暂不支持归档。".to_string());
    }
    let effective_agent_id = resolve_batch_archive_effective_agent_id(&runtime_snapshot, &source);
    let conversation_runtime_state = get_conversation_runtime_state(state, &source.id)?;
    let disabled_reason = match conversation_runtime_state {
        MainSessionState::AssistantStreaming => Some("当前会话正在流式输出，请稍后再归档。"),
        MainSessionState::OrganizingContext => Some("强制归档正在进行中，请稍候。"),
        MainSessionState::Idle => None,
    };
    if let Some(reason) = disabled_reason {
        drop(guard);
        return Err(reason.to_string());
    }
    drop(guard);
    Ok((source, effective_agent_id))
}

fn resolve_batch_archive_effective_agent_id(
    runtime_snapshot: &RuntimeOrganizationSnapshot,
    source: &Conversation,
) -> String {
    let effective_agent_id = source.agent_id.trim();
    if effective_agent_id.is_empty() {
        runtime_log_warn(format!(
            "[批量归档] 跳过人格校验，任务=批量归档元数据校验，conversation_id={}，原因=会话未绑定人格，不再回退到其他人格；后续如无法确定归档归属，将直接跳过归档反思",
            source.id
        ));
        return String::new();
    }
    if runtime_snapshot
        .agents
        .iter()
        .any(|agent| agent.id == effective_agent_id && !agent.is_built_in_user)
    {
        return effective_agent_id.to_string();
    }
    runtime_log_warn(format!(
        "[批量归档] 跳过人格校验，任务=批量归档元数据校验，conversation_id={}，agent_id={}，原因=会话绑定人格不存在或不可用，不再回退到其他人格；后续如无法确定归档归属，将直接跳过归档反思",
        source.id,
        effective_agent_id
    ));
    effective_agent_id.to_string()
}

fn spawn_batch_archive_pipeline(
    state: AppState,
    selected_api: ApiConfig,
    resolved_api: ResolvedApiConfig,
    accepted: Vec<BatchArchiveAcceptedConversation>,
    active_conversation_id: Option<String>,
) {
    if accepted.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let total_count = accepted.len();
        let panic_safe_task = std::panic::AssertUnwindSafe(async {
            runtime_log_info(format!(
                "[批量归档] 开始，任务=后台串行归档维护，api_config_id={}，accepted_count={}",
                selected_api.id, total_count
            ));
            for item in accepted {
                let item_started_at = std::time::Instant::now();
                let source = match conversation_service_v2()
                    .read_archive_pipeline_source_conversation(&state, &item.conversation_id)
                {
                    Ok(conversation) => conversation,
                    Err(err) => {
                        runtime_log_warn(format!(
                            "[批量归档] 失败，任务=后台串行归档维护，conversation_id={}，error=读取归档流水线源会话失败：{}",
                            item.conversation_id, err
                        ));
                        continue;
                    }
                };
                if let Err(err) = run_archive_pipeline(
                    &state,
                    &selected_api,
                    &resolved_api,
                    &source,
                    &item.effective_agent_id,
                    active_conversation_id.as_deref(),
                    None,
                    "batch_archive_conversations",
                    "ARCHIVE-BATCH",
                )
                .await
                {
                    runtime_log_warn(format!(
                        "[批量归档] 失败，任务=后台串行归档维护，conversation_id={}，error={}，duration_ms={}",
                        source.id,
                        err,
                        item_started_at.elapsed().as_millis()
                    ));
                    continue;
                }
                runtime_log_info(format!(
                    "[批量归档] 完成，任务=后台串行归档维护，conversation_id={}，duration_ms={}",
                    source.id,
                    item_started_at.elapsed().as_millis()
                ));
            }
            trigger_chat_queue_processing(&state);
            runtime_log_info(format!(
                "[批量归档] 完成，任务=后台串行归档维护，api_config_id={}，accepted_count={}",
                selected_api.id, total_count
            ));
        });
        if futures_util::FutureExt::catch_unwind(panic_safe_task)
            .await
            .is_err()
        {
            runtime_log_error(format!(
                "[批量归档] 失败，任务=后台串行归档维护，api_config_id={}，error=panic",
                selected_api.id
            ));
            trigger_chat_queue_processing(&state);
        }
    });
}

async fn run_archive_pipeline_inner(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    reflection_source: &Conversation,
    _effective_agent_id: &str,
    prepared_active_conversation_id: Option<&str>,
    _target_conversation_id: Option<&str>,
    archive_reason: &str,
    trace_tag: &str,
    started_at: std::time::Instant,
    trace_id: &str,
) -> Result<ForceArchiveResult, String> {
    let reflection_skip_warning = archive_reflection_skip_reason(reflection_source);
    runtime_log_info(format!(
        "[归档反思] 开始，任务=最后块正文反思，conversation_id={}，full_body_message_count={}，last_block_body_message_count={}",
        source.id,
        archive_pipeline_message_count_for_delete(source),
        archive_pipeline_message_count_for_delete(reflection_source)
    ));
    if let Some(reason) = reflection_skip_warning.as_ref() {
        runtime_log_warn(format!(
            "[归档] 跳过归档反思，任务=后台归档维护，conversation_id={}，原因={}，行为=直接完成归档，不阻塞主流程",
            source.id, reason
        ));
    }

    let reporting_source = build_archive_reporting_conversation(reflection_source);
    let (archive_warning, applied_report, archive_body_tokens) =
        if let Some(reason) = reflection_skip_warning.clone() {
            (
                Some(reason),
                None,
                archive_body_token_count(reporting_source.as_ref()),
            )
        } else {
            match resolve_archive_owner_context(state, source) {
                Ok((owner_agent, owner_agent_id, user_alias)) => {
                    let memories = memory_store_list_memories_visible_for_agent(
                        &state.data_path,
                        &owner_agent_id,
                        owner_agent.private_memory_enabled,
                    )?;

                    runtime_log_debug(format!(
                        "[{}] trace={} 开始，api={} model={} format={} conversation={} ownerAgent={}",
                        trace_tag,
                        trace_id,
                        selected_api.id,
                        selected_api.model,
                        resolved_api.request_format,
                        source.id,
                        owner_agent_id
                    ));

                    let body_reporting_source = build_archive_body_reporting_conversation(
                        reporting_source.as_ref(),
                        &memories,
                    );
                    let archive_body_tokens = archive_body_token_count(&body_reporting_source);
                    if archive_body_tokens >= ARCHIVE_REFLECTION_MIN_BODY_TOKENS {
                        let (summary_draft, archive_warning) =
                            summarize_archive_summary_with_fallback(
                                state,
                                resolved_api,
                                selected_api,
                                &owner_agent,
                                &user_alias,
                                &body_reporting_source,
                                &memories,
                            )
                            .await;
                        let deduped_recall =
                            archive_pipeline_dedup_recall_table(&source.memory_recall_table);
                        let applied_report = apply_summary_context_result(
                            &state.data_path,
                            &owner_agent,
                            &deduped_recall,
                            &summary_draft,
                        )?;
                        (archive_warning, Some(applied_report), archive_body_tokens)
                    } else {
                        runtime_log_warn(format!(
                            "[SummaryContext] 跳过，场景=archive，conversation_id={}，原因=正文不足1000token，body_tokens={:.0}，threshold={:.0}",
                            source.id,
                            archive_body_tokens,
                            ARCHIVE_REFLECTION_MIN_BODY_TOKENS
                        ));
                        (None, None, archive_body_tokens)
                    }
                }
                Err(err) => {
                    runtime_log_warn(format!(
                        "[归档] 跳过归档反思，任务=后台归档维护，conversation_id={}，原因={}，行为=直接完成归档，不阻塞主流程",
                        source.id, err
                    ));
                    (
                        Some(format!("归档反思已跳过：{}", err)),
                        None,
                        archive_body_token_count(reporting_source.as_ref()),
                    )
                }
            }
        };

    let archived_conversation = conversation_service_v2()
        .get_conversation_meta(state, &source.id)
        .map_err(|_| "归档后维护失败：会话已不存在。".to_string())?;
    if archived_conversation.status.trim() != "archived"
        && archived_conversation
            .archived_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        return Err("归档后维护失败：会话尚未标记为已归档。".to_string());
    }
    let archive_id = archived_conversation.id.to_string();
    runtime_log_info(format!(
        "[归档] 开始，任务=后台维护，conversation_id={}，reason=\"{}\"",
        archived_conversation.id, archive_reason
    ));
    clear_screenshot_artifact_cache();
    mark_tasks_as_session_lost(&state.data_path, &source.id);
    let active_conversation_id = prepared_active_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            match conversation_service_v2().resolve_latest_foreground_conversation_id(state, "") {
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
    match delegate_runtime_thread_conversation_delete_by_root(state, &source.id) {
        Ok(deleted_count) => runtime_log_info(format!(
            "[委托会话] 完成，任务=随会话归档级联清理，root_conversation_id={}，deleted_count={}",
            source.id, deleted_count
        )),
        Err(err) => runtime_log_warn(format!(
            "[委托会话] 失败，任务=随会话归档级联清理，root_conversation_id={}，error={}",
            source.id, err
        )),
    }

    match cleanup_backup_records_from_messages(&state.data_path, &source.messages) {
        Ok(cleaned) if cleaned > 0 => {
            runtime_log_info(format!(
                "[归档] apply_patch 备份清理完成: conversation={}, cleaned={}",
                source.id, cleaned
            ));
        }
        Err(err) => {
            runtime_log_error(format!(
                "[归档] apply_patch 备份清理失败: conversation={}, error={}",
                source.id, err
            ));
        }
        _ => {}
    }

    if let Err(e) = cleanup_pdf_cache_for_conversation(&state, &source.id) {
        runtime_log_error(format!(
            "[归档] 清理 PDF 缓存失败: conversation={}, error={}",
            source.id, e
        ));
    }

    if let Some(applied_report) = applied_report.as_ref() {
        runtime_log_debug(format!(
            "[SummaryContext] 完成，场景=archive，trace_id={}，conversation_id={}，merged_memories={}，merged_groups={}，profile_applied={}，profile_skipped={}，useful_accept={}，penalized={}，natural_decay={}",
            trace_id,
            source.id,
            applied_report.merged_memories,
            applied_report.merged_groups,
            applied_report.applied_profile_memories,
            applied_report.skipped_profile_memories,
            applied_report.memory_feedback.useful_accepted_count,
            applied_report.memory_feedback.penalized_count,
            applied_report.memory_feedback.natural_decay_count
        ));
    } else {
        runtime_log_warn(format!(
            "[SummaryContext] 跳过完成，场景=archive，trace_id={}，conversation_id={}，body_tokens={:.0}",
            trace_id,
            source.id,
            archive_body_tokens
        ));
    }
    let merged_memories = applied_report
        .as_ref()
        .map(|report| report.merged_memories)
        .unwrap_or(0);
    let merge_groups = applied_report
        .as_ref()
        .map(|report| report.merged_groups);
    let memory_feedback = applied_report.map(|report| report.memory_feedback);

    Ok(ForceArchiveResult {
        archived: true,
        archive_id: Some(archive_id),
        active_conversation_id: Some(active_conversation_id),
        compaction_message: None,
        summary: String::new(),
        merged_memories,
        warning: archive_warning,
        reason_code: None,
        elapsed_ms: Some(started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64),
        memory_feedback,
        merge_groups,
    })
}
