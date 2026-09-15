fn build_genai_message_state(
    prepared: &PreparedPrompt,
) -> Result<(Option<String>, Vec<genai::chat::ChatMessage>), String> {
    let request = build_genai_chat_request(prepared)?;
    Ok((request.system, request.messages))
}

async fn maybe_apply_auto_compaction_before_tool_continue_genai(
    state: Option<&AppState>,
    context: Option<&ToolLoopAutoCompactionContext>,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    transient_tool_history: &[Value],
    partial_assistant_text: &str,
    partial_activity_reasoning_text: &str,
    chat_session_key: &str,
    pending_tool_group_result_persists: &mut Vec<tauri::async_runtime::JoinHandle<Result<(), String>>>,
) -> Result<bool, String> {
    let Some(state) = state else {
        return Ok(false);
    };
    let Some(context) = context else {
        return Ok(false);
    };
    if context.remote_im_reply_delegate_id.is_some() {
        runtime_log_warn(format!(
            "[远程应答委托] 跳过，任务=工具续调前自动整理，conversation_id={}，reason=frozen_delegate_snapshot",
            context.conversation_id
        ));
        return Ok(false);
    }
    if transient_tool_history.is_empty() {
        return Ok(false);
    }

    let Some((source, prepared_before)) = build_tool_loop_prepared_for_continuation(
        state,
        context,
        selected_api,
        resolved_api,
        transient_tool_history,
    )?
    else {
        runtime_log_warn(format!(
            "[聊天] 工具续调前上下文整理检查 跳过 conversation_id={} 原因=会话不存在或已归档",
            context.conversation_id
        ));
        return Ok(false);
    };

    let base_usage = conversation_prompt_service().resolve_shared_trusted_prompt_usage_or_estimate(
        &context.trusted_prompt_usage,
        &prepared_before,
        selected_api,
        &context.agent,
    );
    let usage = base_usage;
    let (decision, decision_source) = decide_archive_before_send_from_usage(
        &usage,
        source.last_user_at.as_deref(),
        archive_pipeline_has_assistant_reply(&source),
        conversation_current_segment_is_compaction_summary_only(&source),
    );
    runtime_log_info(format!(
        "[聊天] 工具续调前上下文整理检查 conversation_id={} should_archive={} forced={} usage_ratio={:.4} source={} reason={} effective_prompt_tokens={} context_window_tokens={} estimated={}",
        context.conversation_id,
        decision.should_archive,
        decision.forced,
        decision.usage_ratio,
        decision_source,
        decision.reason,
        usage.effective_prompt_tokens,
        selected_api.context_window_tokens,
        usage.estimated_prompt_tokens.is_some(),
    ));
    if !decision.should_archive {
        conversation_prompt_service().store_shared_prompt_usage_resolution(
            &context.trusted_prompt_usage,
            &usage,
            selected_api,
        );
        return Ok(false);
    }

    // Tool-result appends are spawned so ordinary tool loops do not pay an I/O
    // barrier on every call. Context compaction is a history boundary: before
    // creating the summary, wait for the spawned appends to update the message
    // store and conversation cache, then read the refreshed conversation below.
    await_pending_tool_group_result_persists(
        pending_tool_group_result_persists,
        chat_session_key,
        "auto_before_tool_continue",
    )
    .await?;

    let checkpoint = persist_tool_loop_compaction_checkpoint(
        state,
        context,
        on_delta,
        &[],
        partial_assistant_text,
        partial_activity_reasoning_text,
        chat_session_key,
        "auto_before_tool_continue",
    )?;
    let archive_res = run_context_compaction_pipeline(
        state,
        selected_api,
        resolved_api,
        &checkpoint.refreshed_source,
        &context.agent.id,
        &decision.reason,
        "COMPACTION-BEFORE-TOOL-CONTINUE",
        &checkpoint.boundary_messages,
        false,
    )
    .await;

    let archive_result = match archive_res {
        Ok(result) => result,
        Err(err) => {
            return Err(format!("自动整理失败：{err}"));
        }
    };

    if let Some(warning) = archive_result
        .warning
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        runtime_log_warn(format!(
            "[聊天] 工具续调前上下文整理 完成 conversation_id={} warning={}",
            context.conversation_id, warning
        ));
    } else {
        runtime_log_info(format!(
            "[聊天] 工具续调前上下文整理 完成 conversation_id={} usage_ratio_before={:.4} source={} reason={} forced={} effective_prompt_tokens={} estimated_prompt_tokens={}",
            context.conversation_id,
            usage.usage_ratio,
            decision_source,
            decision.reason,
            decision.forced,
            usage.effective_prompt_tokens,
            usage.estimated_prompt_tokens.unwrap_or(0)
        ));
    }

    Err(CHAT_DISPATCH_RESTART_AFTER_COMPACTION.to_string())
}

struct ToolLoopCompactionCheckpoint {
    refreshed_source: Conversation,
    boundary_messages: Vec<ChatMessage>,
}

fn persist_tool_loop_compaction_checkpoint(
    state: &AppState,
    context: &ToolLoopAutoCompactionContext,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    transient_tool_history: &[Value],
    partial_assistant_text: &str,
    partial_activity_reasoning_text: &str,
    chat_session_key: &str,
    reason: &str,
) -> Result<ToolLoopCompactionCheckpoint, String> {
    let history_for_checkpoint = transient_tool_history.to_vec();
    let should_persist = !partial_assistant_text.trim().is_empty()
        || !partial_activity_reasoning_text.trim().is_empty()
        || !history_for_checkpoint.is_empty();
    let persist_result = if should_persist {
        let persist_result = conversation_service_v2().persist_stop_chat_partial_message(
            state,
            Some(context.conversation_id.as_str()),
            &context.agent.id,
            partial_assistant_text,
            partial_activity_reasoning_text,
            "",
            &history_for_checkpoint,
            context.assistant_message_id.as_deref(),
        )?;
        runtime_log_info(format!(
            "[上下文整理] 完成，任务=interrupt_checkpoint，conversation_id={}，reason={}，persisted={}，assistant_message_id={}，tool_event_count={}",
            context.conversation_id,
            reason,
            persist_result.persisted,
            persist_result
                .assistant_message
                .as_ref()
                .map(|message| message.id.as_str())
                .unwrap_or(""),
            history_for_checkpoint.len()
        ));
        clear_inflight_completed_tool_history(state, chat_session_key)?;
        persist_result
    } else {
        runtime_log_warn(format!(
            "[上下文整理] 跳过，任务=interrupt_checkpoint，conversation_id={}，reason={}，原因=无可落盘内容",
            context.conversation_id, reason
        ));
        StopChatPersistResult {
            persisted: false,
            conversation_id: Some(context.conversation_id.clone()),
            assistant_message: None,
        }
    };
    let _ = on_delta.send(round_completed_delta_event(
        &context.conversation_id,
        context.request_id.as_deref(),
        partial_assistant_text,
        persist_result.assistant_message.as_ref(),
    ));
    let boundary_messages = persist_result
        .assistant_message
        .clone()
        .into_iter()
        .collect::<Vec<_>>();
    if let Err(err) = clear_conversation_stream_runtime_cache(state, &context.conversation_id) {
        runtime_log_warn(format!(
            "[聊天流式缓存] 压缩前清理失败 conversation_id={} reason={} error={}",
            context.conversation_id, reason, err
        ));
    }
    let refreshed_source = tool_loop_active_conversation_snapshot(state, &context.conversation_id)?
        .ok_or_else(|| "上下文整理前重新读取会话失败：会话不存在或已归档。".to_string())?;
    Ok(ToolLoopCompactionCheckpoint {
        refreshed_source,
        boundary_messages,
    })
}

