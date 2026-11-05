#[tauri::command]
async fn compact_conversation(
    input: ConversationIdOnlyInput,
    state: State<'_, AppState>,
) -> Result<ConversationCommandStatus, String> {
    compact_conversation_inner(input, state.inner()).await
}

fn compact_conversation_preview_inner(
    input: &ConversationIdOnlyInput,
    state: &AppState,
) -> Result<TrimCompactionPreviewResult, String> {
    let requested_conversation_id = input.conversation_id.trim();
    if requested_conversation_id.is_empty() {
        return Err("conversationId is required".to_string());
    }
    let (selected_api, _resolved_api, source, _effective_agent_id) =
        resolve_archive_request_conversation_by_id(state, requested_conversation_id)?;
    if conversation_is_archived(&source) {
        return Err("当前没有可压缩的活动对话。".to_string());
    }
    build_trim_compaction_preview_result(state, &selected_api, &source)
}

async fn compact_conversation_inner(
    input: ConversationIdOnlyInput,
    state: &AppState,
) -> Result<ConversationCommandStatus, String> {
    let requested_conversation_id = input.conversation_id.trim();
    if requested_conversation_id.is_empty() {
        return Err("conversationId is required".to_string());
    }
    let (selected_api, resolved_api, source, effective_agent_id) =
        resolve_archive_request_conversation_by_id(state, requested_conversation_id)?;
    if conversation_is_archived(&source) {
        return Err("当前没有可压缩的活动对话。".to_string());
    }
    let preview = build_trim_compaction_preview_result(state, &selected_api, &source)?;
    if !preview.can_compact {
        return Err(preview
            .compaction_disabled_reason
            .unwrap_or_else(|| "当前会话暂时不能压缩。".to_string()));
    }
    run_context_compaction_pipeline(
        state,
        &selected_api,
        &resolved_api,
        &source,
        &effective_agent_id,
        "compact_conversation",
        "COMPACTION-FORCE",
        &[],
        false,
    )
    .await?;
    trigger_chat_queue_processing(state);
    Ok(ConversationCommandStatus { success: true })
}

fn build_trim_compaction_preview_result(
    state: &AppState,
    selected_api: &ApiConfig,
    source: &Conversation,
) -> Result<TrimCompactionPreviewResult, String> {
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
    } else {
        None
    };
    Ok(TrimCompactionPreviewResult {
        conversation_id: source.id.clone(),
        can_compact: compaction_disabled_reason.is_none(),
        message_count,
        has_assistant_reply,
        is_empty,
        context_usage_percent,
        compaction_disabled_reason,
    })
}

pub(crate) async fn run_context_compaction_pipeline(
    state: &AppState,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    source: &Conversation,
    effective_agent_id: &str,
    compaction_reason: &str,
    trace_tag: &str,
    boundary_messages: &[ChatMessage],
    activate_after_flush: bool,
) -> Result<ForceArchiveResult, String> {
    let started_at = std::time::Instant::now();
    let trace_id = Uuid::new_v4().to_string();
    // 调度事件：压缩开始（仅委托，latest Run）
    let _ = schedule_event_push_to_latest_run(
        state,
        &source.id,
        "compaction_start",
        0,
        None,
        serde_json::json!({
            "reason": compaction_reason,
            "traceTag": trace_tag,
            "traceId": trace_id,
        }),
    );
    let (selected_api, resolved_api) = resolve_context_compaction_primary_model(
        state,
        selected_api,
        resolved_api,
        source,
        &trace_id,
    )?;

    set_conversation_runtime_state_and_emit(
        state,
        &source.id,
        MainSessionState::OrganizingContext,
    )?;
    runtime_log_debug(format!(
        "[归档流程] 开始: task=context_compaction, trace_id={}, agent_id={}, api_id={}, started_at={}",
        trace_id, effective_agent_id, selected_api.id, started_at.elapsed().as_millis()
    ));

    let result = run_context_compaction_pipeline_inner(
        state,
        &selected_api,
        &resolved_api,
        source,
        effective_agent_id,
        compaction_reason,
        trace_tag,
        boundary_messages,
        activate_after_flush,
        started_at,
        &trace_id,
    )
    .await;

    let elapsed_ms = started_at.elapsed().as_millis();
    // 调度事件：压缩结束
    {
        let success = result.is_ok();
        let detail = if let Some(err) = result.as_ref().err() {
            serde_json::json!({
                "reason": compaction_reason,
                "traceTag": trace_tag,
                "traceId": trace_id,
                "error": err,
                "elapsedMs": elapsed_ms,
            })
        } else {
            serde_json::json!({
                "reason": compaction_reason,
                "traceTag": trace_tag,
                "traceId": trace_id,
                "elapsedMs": elapsed_ms,
            })
        };
        let _ = schedule_event_push_to_latest_run(
            state,
            &source.id,
            "compaction_end",
            0,
            Some(success),
            detail,
        );
    }
    if let Err(state_err) =
        set_conversation_runtime_state_and_emit(state, &source.id, MainSessionState::Idle)
    {
        runtime_log_error(format!(
            "[归档流程] 警告: 状态恢复失败, trace_id={}, elapsed_ms={}, error={}",
            trace_id, elapsed_ms, state_err
        ));
    } else {
        runtime_log_debug(format!(
            "[归档流程] 完成: task=context_compaction, trace_id={}, agent_id={}, api_id={}, elapsed_ms={}",
            trace_id, effective_agent_id, selected_api.id, elapsed_ms
        ));
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
    boundary_messages: &[ChatMessage],
    activate_after_flush: bool,
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
        runtime_log_info(format!(
            "[归档流程] 整理前直接删除：conversation_id={}, reason=no_assistant_reply_deleted, next_conversation_id={}",
            source.id, active_conversation_id
        ));
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

    let compaction_source = conversation_service_v2()
        .read_archive_pipeline_last_block_conversation(state, &source.id)
        .map_err(|err| format!("读取消息锚定压缩上下文失败：{}", err))?;
    let (owner_agent, owner_agent_id, user_alias) = resolve_archive_owner_context(state, source)?;
    let preserved_dialogue = conversation_service_v2()
        .read_block_preserved_dialogue(
            state,
            &source.id,
            None,
            None,
            &user_alias,
            &owner_agent.name,
            ACTIVE_COMPACTION_PRESERVED_DIALOGUE_BUDGET,
        )
        .map_err(|err| format!("读取 block 保留对话失败：{}", err))?;

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

    let (summary_draft, compaction_warning) = summarize_compaction_with_fallback(
        state,
        selected_api,
        resolved_api,
        &owner_agent,
        &user_alias,
        &compaction_source,
        trace_id,
    )
    .await;
    let deduped_recall =
        archive_pipeline_dedup_recall_table(&compaction_source.memory_recall_table);
    let applied_report = apply_summary_context_result(
        &state.data_path,
        &owner_agent,
        &deduped_recall,
        &summary_draft,
    )?;
    let refreshed_user_profile_snapshot =
        if conversation_is_delegate(source) || conversation_is_remote_im_contact(source) {
            None
        } else {
            build_user_profile_snapshot_block(&state.data_path, &owner_agent, 12)?
        };
    let background_shell_lines =
        terminal_background_shell_running_summary_lines(state, &source.id);
    let teammate_names =
        available_delegate_teammate_names(&state_read_agents_cached(state)?, &source.agent_id);
    let compression_message = build_compaction_message(
        &summary_draft.summary,
        Some(summary_draft.title.as_str()),
        compaction_reason,
        Some(&preserved_dialogue),
        &background_shell_lines,
        &teammate_names,
    );
    let persist_result = conversation_service_v2().persist_compaction_message(
        state,
        source,
        &compression_message,
        refreshed_user_profile_snapshot,
    )?;
    let active_conversation_id = persist_result.active_conversation_id;
    let compression_message_id = persist_result.compression_message_id;
    runtime_log_info(format!(
        "[归档流程] 上下文整理消息写入校验通过: conversation_id={}, message_id={}",
        source.id, compression_message_id
    ));
    // 压缩后刷新该会话的项目 Skill 缓存，下次装配提示词时重新扫描当前项目 `.pai/skills`。
    invalidate_conversation_project_skills(state, &source.id);
    if let Err(err) = update_conversation_todos_and_emit(state, &source.id, Vec::new()) {
        runtime_log_warn(format!(
            "[归档流程] 清空会话待办失败: conversation_id={}, error={}",
            source.id, err
        ));
    } else {
        runtime_log_info(format!(
            "[归档流程] 清空会话待办完成: conversation_id={}",
            source.id
        ));
    }
    match clear_apply_patch_temp(&state.data_path) {
        Ok((record_count, blob_count)) => {
            runtime_log_info(format!(
                "[apply_patch缓存] 完成，任务=clear_temp_on_compaction，conversation_id={}，记录条数={}，备份条数={}",
                source.id, record_count, blob_count
            ));
        }
        Err(err) => {
            runtime_log_error(format!(
                "[apply_patch缓存] 失败，任务=clear_temp_on_compaction，conversation_id={}，error={}",
                source.id, err
            ));
        }
    }
    match clear_operate_screenshots_temp(&state.data_path, &source.id) {
        Ok((file_count, dir_count)) => {
            runtime_log_info(format!(
                "[operate截图缓存] 完成，任务=clear_temp_on_compaction，conversation_id={}，截图文件数={}，子目录数={}",
                source.id, file_count, dir_count
            ));
        }
        Err(err) => {
            runtime_log_error(format!(
                "[operate截图缓存] 失败，任务=clear_temp_on_compaction，conversation_id={}，error={}",
                source.id, err
            ));
        }
    }
    emit_compaction_history_flushed_event(
        state,
        &source.id,
        boundary_messages,
        &compression_message,
        activate_after_flush,
    );

    runtime_log_debug(format!(
        "[SummaryContext] 完成，场景=compaction，trace_id={}，conversation_id={}，merged_memories={}，merged_groups={}，profile_applied={}，profile_skipped={}，useful_accept={}，penalized={}，natural_decay={}",
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
