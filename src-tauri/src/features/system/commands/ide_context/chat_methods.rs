fn ide_chat_runtime_for_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Option<ConversationRuntimeSnapshot> {
    read_conversation_runtime_snapshot(state, conversation_id).ok()
}

fn ide_chat_sidebar_window_label(client_id: &str) -> String {
    format!("vscode-sidebar:{}", client_id.trim())
}

fn ide_chat_emit_overview_updated(state: &AppState) -> Result<(), String> {
    let overview_payload = conversation_service_v2().refresh_unarchived_conversation_overview(state)?;
    emit_unarchived_conversation_overview_updated_payload(state, &overview_payload);
    Ok(())
}

fn ide_chat_release_sidebar_conversation(
    state: &AppState,
    sidebar_label: &str,
) -> Result<(), String> {
    if let Some(client_id) = ide_chat_sidebar_client_id_from_label(sidebar_label) {
        if let Ok(mut conversations) = ide_context_chat_client_conversations().lock() {
            conversations.remove(&client_id);
        }
    }
    if unregister_detached_chat_window_by_label(sidebar_label).is_some() {
        ide_chat_emit_overview_updated(state)?;
    }
    Ok(())
}

fn ide_chat_register_sidebar_conversation(
    state: &AppState,
    conversation_id: &str,
    sidebar_label: &str,
    opened_conversation_id: &mut Option<String>,
) -> Result<(), String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required".to_string());
    }
    let conversation_meta = conversation_service_v2().get_conversation_meta(state, conversation_id)?;
    if conversation_meta.id.trim() == SYSTEM_NOTIFICATION_CONVERSATION_ID
        || conversation_meta.conversation_kind.trim() == CONVERSATION_KIND_SYSTEM_NOTIFICATION
    {
        if opened_conversation_id.as_deref() != Some(conversation_id) {
            ide_chat_release_sidebar_conversation(state, sidebar_label)?;
        }
        if let Some(client_id) = ide_chat_sidebar_client_id_from_label(sidebar_label) {
            if let Ok(mut conversations) = ide_context_chat_client_conversations().lock() {
                conversations.remove(&client_id);
            }
        }
        *opened_conversation_id = Some(conversation_id.to_string());
        return Ok(());
    }
    if opened_conversation_id.as_deref() != Some(conversation_id) {
        ide_chat_release_sidebar_conversation(state, sidebar_label)?;
    }
    register_detached_chat_window(conversation_id, sidebar_label)?;
    if let Some(client_id) = ide_chat_sidebar_client_id_from_label(sidebar_label) {
        if let Ok(mut conversations) = ide_context_chat_client_conversations().lock() {
            conversations.insert(client_id, conversation_id.to_string());
        }
    }
    *opened_conversation_id = Some(conversation_id.to_string());
    ide_chat_emit_overview_updated(state)?;
    Ok(())
}

fn ide_chat_ensure_sidebar_workspace(
    state: &AppState,
    conversation_id: &str,
    workspace_path: &str,
    _workspace_name: Option<&str>,
) -> Result<(), String> {
    let conversation_meta = conversation_service_v2().get_conversation_meta(state, conversation_id)?;
    let mut workspaces = conversation_meta.shell_workspaces.clone();
    let has_main = workspaces.iter().any(|ws| {
        normalize_shell_workspace_level_text(&ws.level) == SHELL_WORKSPACE_LEVEL_MAIN
    });
    if has_main {
        return Ok(());
    }
    let name = std::path::Path::new(workspace_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| workspace_path.to_string());
    workspaces.push(ShellWorkspaceConfig {
        id: "vscode-sidebar-main-workspace".to_string(),
        name: name.to_string(),
        path: workspace_path.to_string(),
        level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
        access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
        built_in: false,
    });
    let normalized_workspaces = normalize_conversation_shell_workspaces(state, &workspaces);
    apply_conversation_chat_workspace_changes(
        state,
        conversation_id,
        Some(None),
        Some(normalized_workspaces),
        None,
        None,
        None,
    )?;
    Ok(())
}

fn ide_chat_conversation_list(state: &AppState, current_viewer_id: &str) -> Result<Value, String> {
    let viewer_id = current_viewer_id.trim();
    let summaries = conversation_service_v2()
        .list_unarchived_conversation_summaries(state)?
        .summaries
        .into_iter()
        .map(|mut item| {
            item.runtime_state = ide_chat_runtime_for_conversation(state, &item.conversation_id)
                .map(|snapshot| snapshot.runtime_state);
            item.state.current_viewer_id = Some(viewer_id.to_string());
            item
        })
        .collect::<Vec<_>>();
    let remote_im_contact_conversations = conversation_service_v2().list_remote_im_contact_conversations(state)?;
    let persona = ide_chat_persona_payload(state, None)?;
    Ok(serde_json::json!({
        "conversations": summaries,
        "unarchivedConversations": summaries,
        "remoteImContactConversations": remote_im_contact_conversations,
        "persona": persona,
        "viewerId": viewer_id,
    }))
}

async fn ide_chat_conversation_changed_since(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ListUnarchivedConversationsChangedSinceInput>(params)?;
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        serde_json::to_value(list_unarchived_conversations_changed_since_blocking(&app_state, &input)?)
            .map_err(|err| format!("Serialize conversation changed-since result failed: {err}"))
    })
    .await
    .map_err(|err| format!("读取未归档会话列表差量任务异常：{err}"))?
}

async fn ide_chat_conversation_block_page(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatConversationBlockPageInput>(params)?;
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required".to_string());
    }
    let block_id = input.block_id;
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        let page = if let Some(block_id) = block_id {
            conversation_service_v2().get_conversation_block(&app_state, &conversation_id, Some(block_id))?
        } else {
            conversation_service_v2().get_conversation_last_block(&app_state, &conversation_id)?
        };
        Ok(serde_json::json!({
            "blocks": page.blocks.into_iter().map(|item| {
                serde_json::json!({
                    "blockId": item.block_id,
                    "messageCount": item.message_count,
                    "firstMessageId": item.first_message_id,
                    "lastMessageId": item.last_message_id,
                    "firstCreatedAt": item.first_created_at,
                    "lastCreatedAt": item.last_created_at,
                    "isLatest": item.is_latest,
                })
            }).collect::<Vec<_>>(),
            "selectedBlockId": page.selected_block_id,
            "messages": page.messages,
            "hasPrevBlock": page.has_prev_block,
            "hasNextBlock": page.has_next_block,
        }))
    })
    .await
    .map_err(|err| format!("读取会话块分页任务异常：{err}"))?
}

fn ide_chat_conversation_fast_request_turns(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<GetConversationFastRequestTurnsInput>(params)?;
    serde_json::to_value(
        conversation_service_v2()
            .get_conversation_fast_request_turns(state, &input.conversation_id)?,
    )
    .map_err(|err| format!("Serialize fast request turns failed: {err}"))
}

fn ide_chat_conversation_fast_request_turns_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<GetConversationFastRequestTurnsInput>(params, "input")?;
    ide_chat_serialize(
        conversation_service_v2().get_conversation_fast_request_turns(state, &input.conversation_id)?,
    )
}

async fn ide_chat_conversation_block_page_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<GetConversationBlockPageInput>(params, "input")?;
    ide_chat_conversation_block_page(state, ide_chat_serialize(input)?).await
}

async fn ide_chat_mark_conversation_read_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<MarkConversationReadInput>(params, "input")?;
    ide_chat_mark_conversation_read(state, ide_chat_serialize(input)?).await
}

async fn ide_chat_conversation_message_by_id_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<GetUnarchivedConversationMessageByIdInput>(params, "input")?;
    let conversation_id = input.conversation_id.trim().to_string();
    let message_id = input.message_id.trim().to_string();
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        ide_chat_serialize(conversation_service_v2().get_message_by_id_for_frontend_display_only(
            &app_state,
            &conversation_id,
            &message_id,
        )?)
    })
    .await
    .map_err(|err| format!("读取会话单条消息任务异常：{err}"))?
}

async fn ide_chat_conversation_messages_before_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<GetActiveConversationMessagesBeforeInput>(params, "input")?;
    let before_message_id = input.before_message_id.trim().to_string();
    if before_message_id.is_empty() {
        return Err("beforeMessageId is required.".to_string());
    }
    let conversation_id = input
        .conversation_id
        .as_deref()
        .or_else(|| input.session.as_ref().and_then(|session| session.conversation_id.as_deref()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "conversationId is required.".to_string())?;
    let limit = input.limit.clamp(1, 100);
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        let page = conversation_service_v2().get_messages_before(
            &app_state,
            &conversation_id,
            &before_message_id,
            limit,
        )?;
        ide_chat_serialize(GetActiveConversationMessagesBeforeOutput {
            messages: page.messages,
            has_more: page.has_more,
        })
    })
    .await
    .map_err(|err| format!("读取活动会话历史消息任务异常：{err}"))?
}

async fn ide_chat_conversation_compaction_segment_before_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<GetActiveConversationCompactionSegmentBeforeInput>(params, "input")?;
    let anchor_message_id = input.anchor_message_id.trim().to_string();
    if anchor_message_id.is_empty() {
        return Err("anchorMessageId is required.".to_string());
    }
    let conversation_id = input
        .conversation_id
        .as_deref()
        .or_else(|| input.session.as_ref().and_then(|session| session.conversation_id.as_deref()))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "conversationId is required.".to_string())?;
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        let page = conversation_service_v2().get_compaction_segment_before_anchor(
            &app_state,
            &conversation_id,
            &anchor_message_id,
        )?;
        ide_chat_serialize(GetActiveConversationCompactionSegmentBeforeOutput {
            messages: page.messages,
            has_more: page.has_more,
        })
    })
    .await
    .map_err(|err| format!("读取压缩段分页任务异常：{err}"))?
}

async fn ide_chat_conversation_light_snapshot_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ForegroundConversationLightSnapshotInput>(params, "input")?;
    let app_state = state.clone();
    let output = tokio::task::spawn_blocking(move || {
        get_foreground_conversation_light_snapshot_blocking(input, &app_state)
    })
    .await
    .map_err(|err| format!("读取前台轻量快照任务异常：{err}"))??;
    ide_chat_serialize(output)
}

async fn ide_chat_conversation_freshness_snapshot_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ForegroundConversationFreshnessInput>(params, "input")?;
    let app_state = state.clone();
    let output = tokio::task::spawn_blocking(move || {
        get_foreground_conversation_freshness_snapshot_blocking(input, &app_state)
    })
    .await
    .map_err(|err| format!("读取前台 freshness 快照任务异常：{err}"))??;
    ide_chat_serialize(output)
}

fn ide_chat_set_active_conversation_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SetActiveUnarchivedConversationInput>(params, "input")?;
    let conversation_id = conversation_service_v2().set_active_conversation(state, &input)?;
    ide_chat_serialize(SetActiveUnarchivedConversationOutput { conversation_id })
}

async fn ide_chat_rebind_conversation_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RebindUnarchivedConversationRecipientInput>(params, "input")?;
    ide_chat_serialize(rebind_unarchived_conversation_recipient_inner(input, state).await?)
}

async fn ide_chat_rewind_conversation_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RewindConversationInput>(params, "input")?;
    ide_chat_rewind_conversation(state, ide_chat_serialize(input)?).await
}

fn ide_chat_set_plan_mode_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SetConversationPlanModeInput>(params, "input")?;
    ide_chat_serialize(set_conversation_plan_mode_inner(input, state)?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatReadPlanFileInput {
    conversation_id: String,
    path: String,
}

fn ide_chat_read_plan_file(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatReadPlanFileInput>(params)?;
    let content = read_plan_file_content_inner(&input.conversation_id, &input.path, state)?;
    Ok(serde_json::json!({ "content": content }))
}

async fn ide_chat_preview_rewind_conversation(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_params::<RewindConversationInput>(params)?;
    let mut preview = preview_rewind_conversation_from_message_inner(input, state).await?;
    // Web 无法直接回滚本机文件；协议仍共用同一预览入口，但只开放消息撤回。
    preview.can_undo_patch = false;
    ide_chat_serialize(preview)
}

fn ide_chat_set_preferred_model_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SetConversationPreferredModelInput>(params, "input")?;
    ide_chat_serialize(set_conversation_preferred_model_inner(input, state)?)
}

async fn ide_chat_confirm_plan_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ConfirmPlanAndContinueInput>(params, "input")?;
    ide_chat_serialize(confirm_plan_and_continue_inner(state, &input).await?)
}

fn ide_chat_resolve_terminal_approval_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ResolveTerminalApprovalInput>(params, "input")?;
    let _ = resolve_terminal_approval_request(state, &input.request_id, input.approved, input.reason.as_deref())?;
    ide_chat_serialize(())
}

fn ide_chat_goal_current_command(state: &AppState, params: Value) -> Result<Value, String> {
    let conversation_id = ide_chat_parse_param_field::<String>(params, "conversationId")?;
    ide_chat_serialize(goal_get_current_inner(state, &conversation_id)?)
}

fn ide_chat_goal_create_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<GoalCreateInput>(params, "input")?;
    ide_chat_serialize(goal_create_goal_inner(state, &input.conversation_id, &input.objective)?)
}

fn ide_chat_goal_cancel_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<GoalCancelInput>(params, "input")?;
    ide_chat_serialize(goal_cancel_goal_inner(state, &input.conversation_id)?)
}

fn ide_chat_query_ide_context_command(
    params: Value,
    ide_context_runtime: &IdeContextRuntime,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<IdeContextWorkspaceQueryInput>(params, "input")?;
    ide_chat_serialize(query_ide_context_references_internal(input, ide_context_runtime)?)
}

fn ide_chat_list_archives_command(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(list_archives_inner(state)?)
}

async fn ide_chat_archive_block_page_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<GetArchiveBlockPageInput>(params, "input")?;
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        ide_chat_serialize(get_archive_block_page_inner(input, &app_state)?)
    })
    .await
    .map_err(|err| format!("读取归档块分页任务异常：{err}"))?
}

fn ide_chat_delete_archive_command(state: &AppState, params: Value) -> Result<Value, String> {
    let archive_id = ide_chat_parse_param_field::<String>(params, "archiveId")?;
    delete_archive_inner(state, &archive_id)?;
    ide_chat_serialize(())
}

fn ide_chat_unarchive_command(state: &AppState, params: Value) -> Result<Value, String> {
    let archive_id = ide_chat_parse_param_field::<String>(params, "archiveId")?;
    unarchive_archive_inner(state, &archive_id)?;
    ide_chat_serialize(())
}

async fn ide_chat_archive_conversation_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ConversationIdOnlyInput>(params, "input")?;
    ide_chat_serialize(archive_conversation_inner(input, state).await?)
}

async fn ide_chat_batch_archive_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<BatchArchiveConversationsInput>(params, "input")?;
    ide_chat_serialize(batch_archive_conversations_inner(state, input).await?)
}

fn ide_chat_delegate_statuses_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ListConversationDelegateStatusesInput>(params, "input")?;
    ide_chat_serialize(list_conversation_delegate_statuses_inner(input, state)?)
}

fn ide_chat_delegate_abort_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<AbortDelegateConversationInput>(params, "input")?;
    ide_chat_serialize(abort_delegate_conversation_inner(input, state)?)
}

fn ide_chat_delegate_block_page_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<GetConversationBlockPageInput>(params, "input")?;
    ide_chat_serialize(get_delegate_conversation_block_page_inner(input, state)?)
}

fn ide_chat_delete_delegate_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<DeleteDelegateConversationInput>(params, "input")?;
    ide_chat_serialize(delete_delegate_conversation_inner(input, state)?)
}

async fn ide_chat_branch_selection_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<BranchUnarchivedConversationFromSelectionInput>(params, "input")?;
    ide_chat_serialize(branch_unarchived_conversation_from_selection_internal(input, state).await?)
}

async fn ide_chat_branch_message_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CreateConversationBranchFromMessageInput>(params, "input")?;
    ide_chat_serialize(create_conversation_branch_from_message_internal(input, state).await?)
}

async fn ide_chat_branch_from_current_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<BranchUnarchivedConversationFromCurrentInput>(params, "input")?;
    ide_chat_serialize(branch_unarchived_conversation_from_current_internal(input, state).await?)
}

async fn ide_chat_clear_chat_error_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ClearChatErrorInput>(params, "input")?;
    ide_chat_serialize(clear_chat_error_inner(input, state).await?)
}

fn ide_chat_list_schedule_runs_command(state: &AppState, params: Value) -> Result<Value, String> {
    let extract_id = |value: &Value| {
        value
            .get("conversationId")
            .or_else(|| value.get("conversation_id"))
            .and_then(Value::as_str)
            .map(|raw| raw.trim().to_string())
            .filter(|trimmed| !trimmed.is_empty())
    };
    // 优先解析 input 为 Value：对象直接取字段，字符串仅此时再 from_str
    if let Ok(input_value) = ide_chat_parse_param_field::<Value>(params.clone(), "input") {
        match &input_value {
            Value::String(raw) => {
                if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
                    if let Some(conversation_id) = extract_id(&parsed) {
                        return ide_chat_serialize(schedule_event_list_runs_inner(state, &conversation_id)?);
                    }
                }
            }
            Value::Object(_) => {
                if let Some(conversation_id) = extract_id(&input_value) {
                    return ide_chat_serialize(schedule_event_list_runs_inner(state, &conversation_id)?);
                }
            }
            _ => {}
        }
    }
    if let Some(conversation_id) = extract_id(&params) {
        return ide_chat_serialize(schedule_event_list_runs_inner(state, &conversation_id)?);
    }
    let conversation_id = ide_chat_parse_param_field::<String>(params.clone(), "conversationId")
        .or_else(|_| ide_chat_parse_param_field::<String>(params.clone(), "conversation_id"))
        .map(|raw| raw.trim().to_string())
        .map_err(|_| "conversationId is required".to_string())
        .and_then(|trimmed| {
            if trimmed.is_empty() {
                Err("conversationId is required".to_string())
            } else {
                Ok(trimmed)
            }
        })?;
    ide_chat_serialize(schedule_event_list_runs_inner(state, &conversation_id)?)
}

async fn ide_chat_submit_delegate_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SubmitUserAsyncDelegateInput>(params, "input")?;
    ide_chat_serialize(submit_user_async_delegate_internal(input, state).await?)
}

async fn ide_chat_delete_unarchived_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<DeleteUnarchivedConversationInput>(params, "input")?;
    ide_chat_serialize(delete_unarchived_conversation_inner(input, state).await?)
}

fn ide_chat_export_conversation_share_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ExportConversationShareInput>(params, "input")?;
    ide_chat_serialize(export_conversation_share_json_inner(input, state)?)
}

fn ide_chat_import_archives_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ImportArchivesFromJsonInput>(params, "input")?;
    ide_chat_serialize(import_archives_from_json_inner(input, state)?)
}

fn ide_chat_import_agent_memories_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ImportAgentMemoriesInput>(params, "input")?;
    ide_chat_serialize(import_agent_memories_inner(input, state)?)
}

async fn ide_chat_remote_im_block_page_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactConversationBlockPageInput>(params, "input")?;
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        ide_chat_serialize(remote_im_get_contact_conversation_block_page_inner(input, &app_state)?)
    })
    .await
    .map_err(|err| format!("读取远程 IM 联系人会话块分页任务异常：{err}"))?
}

fn ide_chat_remote_im_clear_conversation_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RemoteImContactDeleteInput>(params, "input")?;
    ide_chat_serialize(remote_im_clear_contact_conversation_inner(input, state)?)
}

async fn ide_chat_frontend_ready_remote_im_command(app: &AppHandle) -> Result<Value, String> {
    ide_chat_serialize(frontend_ready_start_remote_im_services(app.clone()).await?)
}

async fn ide_chat_forward_selection_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ForwardUnarchivedConversationSelectionInput>(params, "input")?;
    ide_chat_serialize(forward_unarchived_conversation_selection_inner(input, state).await?)
}

fn ide_chat_forward_remote_contact_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ForwardSelectionToRemoteImContactInput>(params, "input")?;
    ide_chat_serialize(forward_selection_to_remote_im_contact_inner(input, state)?)
}

fn ide_chat_rename_conversation_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<RenameUnarchivedConversationInput>(params, "input")?;
    ide_chat_serialize(rename_unarchived_conversation_inner(input, state)?)
}

fn ide_chat_toggle_pin_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ToggleUnarchivedConversationPinInput>(params, "input")?;
    ide_chat_serialize(toggle_unarchived_conversation_pin_inner(input, state)?)
}

fn ide_chat_set_auto_push_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SetConversationAutoPushRemoteContactInput>(params, "input")?;
    ide_chat_serialize(set_conversation_auto_push_remote_contact_inner(input, state)?)
}

fn ide_chat_set_agent_primary_api_command(
    state: &AppState,
    app: &AppHandle,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SetAgentPrimaryApiConfigInput>(params, "input")?;
    ide_chat_serialize(set_agent_primary_api_config_inner(input, app, state)?)
}

fn ide_chat_set_ui_language_command(
    state: &AppState,
    app: &AppHandle,
    params: Value,
) -> Result<Value, String> {
    let ui_language = ide_chat_parse_param_field::<String>(params, "uiLanguage")?;
    ide_chat_serialize(set_ui_language_inner(ui_language, app, state)?)
}

fn ide_chat_dump_memory_cache_stats_command(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(dump_memory_cache_stats_inner(state)?)
}

async fn ide_chat_conversation_changed_since_command(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<ListUnarchivedConversationsChangedSinceInput>(params, "input")?;
    let app_state = state.clone();
    let output = tokio::task::spawn_blocking(move || {
        list_unarchived_conversations_changed_since_blocking(&app_state, &input)
    })
    .await
    .map_err(|err| format!("读取未归档会话列表差量任务异常：{err}"))??;
    ide_chat_serialize(output)
}

fn ide_chat_search_memories_recall_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SearchMemoriesRecallInput>(params, "input")?;
    ide_chat_serialize(search_memories_recall_inner(input, state)?)
}

fn ide_chat_conversation_runtime_snapshot(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatConversationInput>(params)?;
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required".to_string());
    }
    serde_json::to_value(read_conversation_runtime_snapshot(state, conversation_id)?)
        .map_err(|err| format!("Serialize conversation runtime snapshot failed: {err}"))
}

fn ide_chat_resume_sidebar_subscription(
    state: &AppState,
    params: Value,
    client_id: &str,
    opened_conversation_id: &mut Option<String>,
) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatConversationInput>(params)?;
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required".to_string());
    }
    let sidebar_label = ide_chat_sidebar_window_label(client_id);
    ide_chat_register_sidebar_conversation(
        state,
        conversation_id,
        &sidebar_label,
        opened_conversation_id,
    )?;
    let runtime = read_conversation_runtime_snapshot(state, conversation_id)?;
    Ok(serde_json::json!({
        "conversationId": conversation_id,
        "runtime": runtime,
    }))
}

fn ide_chat_stream_probe(
    params: Value,
    client_id: &str,
    opened_conversation_id: &Option<String>,
) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatStreamProbeInput>(params)?;
    let conversation_id = input.conversation_id.trim();
    let probe_id = input.probe_id.trim();
    if conversation_id.is_empty() || probe_id.is_empty() {
        return Err("conversationId and probeId are required".to_string());
    }
    if opened_conversation_id.as_deref() != Some(conversation_id) {
        return Ok(serde_json::json!({ "delivered": false }));
    }
    let client_registered = ide_context_chat_client_conversations()
        .lock()
        .ok()
        .and_then(|conversations| conversations.get(client_id).cloned())
        .is_some_and(|mapped_conversation_id| mapped_conversation_id.trim() == conversation_id);
    if !client_registered {
        return Ok(serde_json::json!({ "delivered": false }));
    }
    let delivered = ide_chat_emit_notification_to_sidebar_conversation(
        conversation_id,
        "chat.streamProbeAck",
        serde_json::json!({
            "conversationId": conversation_id,
            "probeId": probe_id,
        }),
    ) > 0;
    Ok(serde_json::json!({ "delivered": delivered }))
}

async fn ide_chat_conversation_freshness_snapshot(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ForegroundConversationFreshnessInput>(params)?;
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        serde_json::to_value(get_foreground_conversation_freshness_snapshot_blocking(input, &app_state)?)
            .map_err(|err| format!("Serialize conversation freshness snapshot failed: {err}"))
    })
    .await
    .map_err(|err| format!("读取前台 freshness 快照任务异常：{err}"))?
}

async fn ide_chat_mark_conversation_read(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<MarkConversationReadInput>(params)?;
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required".to_string());
    }
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        serde_json::to_value(
            conversation_service_v2()
                .mark_conversation_read(&app_state, &conversation_id)?
                .conversation
                .is_some(),
        )
        .map_err(|err| format!("Serialize mark conversation read result failed: {err}"))
    })
    .await
    .map_err(|err| format!("标记会话已读任务异常：{err}"))?
}

async fn ide_chat_create_conversation(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<CreateUnarchivedConversationInput>(params)?;
    ide_chat_serialize(create_unarchived_conversation_inner(input, state).await?)
}

async fn ide_chat_open_draft_conversation(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<Option<OpenDraftConversationInput>>(params, "input")?;
    ide_chat_serialize(open_draft_conversation_inner(input, state).await?)
}

async fn ide_chat_update_draft_conversation(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<UpdateDraftConversationInput>(params, "input")?;
    update_draft_conversation_inner(input, state).await?;
    Ok(serde_json::json!({ "ok": true }))
}

async fn ide_chat_create_side_chat_conversation(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<CreateSideChatConversationInput>(params, "input")?;
    let app_state = state.clone();
    let output = tokio::task::spawn_blocking(move || {
        create_side_chat_conversation_blocking(input, &app_state)
    })
    .await
    .map_err(|err| format!("创建追问会话任务异常：{err}"))??;
    ide_chat_serialize(output)
}

async fn ide_chat_delete_conversation(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<DeleteUnarchivedConversationInput>(params)?;
    ide_chat_serialize(delete_unarchived_conversation_inner(input, state).await?)
}

async fn ide_chat_batch_archive_conversations(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_params::<BatchArchiveConversationsInput>(params)?;
    let output = batch_archive_conversations_inner(state, input).await?;
    ide_chat_serialize(output)
}

async fn ide_chat_rebind_conversation_recipient(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<RebindUnarchivedConversationRecipientInput>(params)?;
    ide_chat_serialize(rebind_unarchived_conversation_recipient_inner(input, state).await?)
}

async fn ide_chat_queue_attachment(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<QueueInlineFileAttachmentInput>(params)?;
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        ide_chat_serialize(queue_inline_file_attachment_inner(input, &app_state)?)
    })
    .await
    .map_err(|err| format!("Web 内联附件兼容摄取任务异常：{err}"))?
}

async fn ide_chat_queue_inline_attachment(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<QueueInlineFileAttachmentInput>(params, "input")?;
    let app_state = state.clone();
    tokio::task::spawn_blocking(move || {
        ide_chat_serialize(queue_inline_file_attachment_inner(input, &app_state)?)
    })
    .await
    .map_err(|err| format!("Web 内联附件兼容摄取任务异常：{err}"))?
}

async fn ide_chat_submit_message_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<SendChatRequest>(params, "input")?;
    ide_chat_serialize(submit_chat_message_inner(input, state, None).await?)
}

fn ide_chat_stop_message_command(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_param_field::<StopChatRequest>(params, "input")?;
    ide_chat_serialize(stop_chat_message_inner(input, state)?)
}

async fn ide_chat_send_message(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<SendChatRequest>(params)?;
    let output = submit_chat_message_inner(input, state, None).await?;
    ide_chat_serialize(output)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatQueueEventInput {
    event_id: String,
}

fn ide_chat_queue_snapshot(state: &AppState) -> Result<Value, String> {
    let snapshot = get_queue_snapshot(state)?;
    serde_json::to_value(snapshot).map_err(|err| format!("serialize queue snapshot failed: {err}"))
}

fn ide_chat_session_state_snapshot(state: &AppState) -> Result<Value, String> {
    let snapshot = get_main_session_state(state)?;
    serde_json::to_value(snapshot).map_err(|err| format!("serialize session state failed: {err}"))
}

fn ide_chat_recall_queue_event(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatQueueEventInput>(params)?;
    let event_id = input.event_id.trim();
    if event_id.is_empty() {
        return Err("eventId is required".to_string());
    }
    ide_chat_serialize(recall_chat_queue_event_inner(event_id, state)?)
}

fn ide_chat_mark_queue_event_guided(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatQueueEventInput>(params)?;
    let event_id = input.event_id.trim();
    if event_id.is_empty() {
        return Err("eventId is required".to_string());
    }
    ide_chat_serialize(mark_chat_queue_event_guided_inner(event_id, state)?)
}

fn ide_chat_stop_conversation(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<StopChatRequest>(params)?;
    let stop_result = stop_chat_message_inner(input, state)?;
    let conversation_id = stop_result.conversation_id.clone().unwrap_or_default();
    ide_chat_broadcast_notification(
        "chat.roundFinished",
        serde_json::json!({
            "conversationId": conversation_id,
            "status": "stopped",
            "assistantText": stop_result.assistant_text,
            "assistantMessage": stop_result.assistant_message,
            "archivedBeforeSend": false,
        }),
    );
    Ok(serde_json::json!({
        "conversationId": conversation_id,
        "status": "stopped",
        "aborted": stop_result.aborted,
        "persisted": stop_result.persisted,
        "assistantText": stop_result.assistant_text,
        "assistantMessage": stop_result.assistant_message,
    }))
}

async fn ide_chat_rewind_conversation(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<RewindConversationInput>(params)?;
    if input.undo_apply_patch {
        return Err(ide_chat_web_native_only_error(
            "conversation.rewind.undoApplyPatch",
        ));
    }
    let result = rewind_conversation_from_message_inner(input, state).await?;
    if result.removed_count > 0 {
        ide_chat_emit_overview_updated(state)?;
    }
    ide_chat_serialize(result)
}

fn ide_chat_compact_preview(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ConversationIdOnlyInput>(params)?;
    ide_chat_serialize(compact_conversation_preview_inner(&input, state)?)
}

async fn ide_chat_compact_conversation(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ConversationIdOnlyInput>(params)?;
    ide_chat_serialize(compact_conversation_inner(input, state).await?)
}

fn ide_chat_model_list(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatConversationInput>(params)?;
    let conversation_meta =
        conversation_service_v2().get_conversation_meta(state, input.conversation_id.trim())?;
    let conversation = ide_chat_conversation_from_meta_view(&conversation_meta);
    ide_chat_model_payload_for_conversation(state, &conversation)
}

fn ide_chat_select_model(state: &AppState, _app: &AppHandle, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatSelectModelInput>(params)?;
    let conversation_id = input.conversation_id.trim().to_string();
    set_conversation_preferred_model_inner(
        SetConversationPreferredModelInput {
            conversation_id: conversation_id.clone(),
            preferred_api_config_id: (!input.api_config_id.trim().is_empty())
                .then(|| input.api_config_id.trim().to_string()),
        },
        state,
    )?;
    let updated_conversation = conversation_service_v2().get_conversation_meta(state, &conversation_id)?;
    let updated_conversation = ide_chat_conversation_from_meta_view(&updated_conversation);
    ide_chat_model_payload_for_conversation(state, &updated_conversation)
}

fn ide_chat_resolve_terminal_approval(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatResolveTerminalApprovalInput>(params)?;
    let resolved = resolve_terminal_approval_request(
        state,
        input.request_id.trim(),
        input.approved,
        input.reason.as_deref(),
    )?;
    Ok(serde_json::json!({ "resolved": resolved }))
}

fn ide_chat_approve_terminal_approval_for_session(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatTerminalApprovalRequestIdInput>(params)?;
    let approved = approve_terminal_approval_for_session_request(state, input.request_id.trim())?;
    Ok(serde_json::json!({ "approved": approved }))
}

fn ide_chat_approve_terminal_approval_for_workspace(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatTerminalApprovalRequestIdInput>(params)?;
    let approved =
        approve_terminal_approval_for_workspace_request(state, input.request_id.trim())?;
    Ok(serde_json::json!({ "approved": approved }))
}

fn ide_chat_set_conversation_plan_mode(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<SetConversationPlanModeInput>(params)?;
    ide_chat_serialize(set_conversation_plan_mode_inner(input, state)?)
}

async fn ide_chat_confirm_plan(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ConfirmPlanAndContinueInput>(params)?;
    let continued = confirm_plan_and_continue_inner(state, &input).await?;
    Ok(serde_json::json!({ "continued": continued }))
}

fn ide_chat_tool_review_reports(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ToolReviewConversationInput>(params)?;
    serde_json::to_value(list_tool_review_reports_internal(input, state)?)
        .map_err(|err| format!("Serialize tool review reports failed: {err}"))
}

fn ide_chat_tool_review_delete_report(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<DeleteToolReviewReportInput>(params)?;
    delete_tool_review_report_internal(input, state)?;
    Ok(serde_json::json!({ "deleted": true }))
}

async fn ide_chat_tool_review_commit_options(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ToolReviewCommitPageInput>(params)?;
    serde_json::to_value(list_tool_review_commit_options_internal_command(input, state).await?)
        .map_err(|err| format!("Serialize tool review commit options failed: {err}"))
}

async fn ide_chat_tool_review_submit_code(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ToolReviewCodeReviewInput>(params)?;
    serde_json::to_value(submit_tool_review_code_internal(input, state).await?)
        .map_err(|err| format!("Serialize tool review submit result failed: {err}"))
}

fn ide_chat_tool_review_batches(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ToolReviewConversationInput>(params)?;
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return serde_json::to_value(ListToolReviewBatchesOutput {
            batches: Vec::new(),
            current_batch_key: None,
        })
        .map_err(|err| format!("Serialize tool review batches failed: {err}"));
    }
    let (batches, current_batch_key) = with_tool_review_conversation(state, conversation_id, |conversation| {
        let batches = collect_tool_review_batches_internal(conversation);
        let current_batch_key = conversation
            .messages
            .iter()
            .rev()
            .find(|message| message.role.trim().eq_ignore_ascii_case("user"))
            .map(|message| message.id.clone());
        Ok((batches, current_batch_key))
    })?;
    serde_json::to_value(ListToolReviewBatchesOutput {
        current_batch_key,
        batches: batches
            .iter()
            .map(tool_review_batch_summary_from_collected)
            .collect(),
    })
    .map_err(|err| format!("Serialize tool review batches failed: {err}"))
}

fn ide_chat_tool_review_item_detail(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ToolReviewCallInput>(params)?;
    let conversation_id = input.conversation_id.trim();
    let call_id = input.call_id.trim();
    if conversation_id.is_empty() || call_id.is_empty() {
        return Err("conversationId 和 callId 不能为空。".to_string());
    }
    let detail = with_tool_review_conversation(state, conversation_id, |conversation| {
        let item = tool_review_find_item(conversation, call_id)?;
        Ok(tool_review_item_detail_from_collected(&item))
    })?;
    serde_json::to_value(detail)
        .map_err(|err| format!("Serialize tool review item detail failed: {err}"))
}

async fn ide_chat_tool_review_item_review(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ToolReviewCallInput>(params)?;
    let conversation_id = input.conversation_id.trim();
    let call_id = input.call_id.trim();
    if conversation_id.is_empty() || call_id.is_empty() {
        return Err("conversationId 和 callId 不能为空。".to_string());
    }
    serde_json::to_value(tool_review_run_for_call_internal(state, conversation_id, call_id).await?)
        .map_err(|err| format!("Serialize tool review item result failed: {err}"))
}

async fn ide_chat_tool_review_item_decision(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ToolReviewSetUserDecisionInput>(params)?;
    let conversation_id = input.conversation_id.trim().to_string();
    let call_id = input.call_id.trim().to_string();
    if conversation_id.is_empty() || call_id.is_empty() {
        return Err("conversationId 和 callId 不能为空。".to_string());
    }
    let opinion = input.opinion.trim().to_string();
    let user_decision_review = serde_json::json!({
        "kind": "user_decision",
        "allow": input.allow,
        "reviewOpinion": if opinion.is_empty() {
            if input.allow { "用户已批准本次工具执行" } else { "用户已否决本次工具执行" }
        } else {
            opinion.as_str()
        },
        "userOpinion": opinion,
    });
    let detail = conversation_service_v2()
        .update_unarchived_conversation_by_id(
            state,
            &conversation_id,
            move |conversation| {
                tool_review_write_call_review(conversation, &call_id, &user_decision_review)?;
                let refreshed = tool_review_find_item(conversation, &call_id)?;
                Ok(tool_review_item_detail_from_collected(&refreshed))
            },
        )
        .await?;
    serde_json::to_value(detail)
        .map_err(|err| format!("Serialize tool review decision result failed: {err}"))
}

async fn ide_chat_tool_review_batch_review(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ToolReviewBatchActionInput>(params)?;
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空。".to_string());
    }
    let conversation = with_tool_review_conversation(state, conversation_id, |conversation| {
        Ok(conversation.clone())
    })?;
    let (_batch_number, batch) = tool_review_find_batch_by_index(&conversation, input.batch_index)?;
    let reviewed_call_ids = tool_review_run_missing_reviews_for_batch(state, conversation_id, &batch).await?;
    serde_json::to_value(RunToolReviewBatchOutput {
        batch_key: batch.batch_key,
        reviewed_call_ids,
    })
    .map_err(|err| format!("Serialize tool review batch result failed: {err}"))
}

async fn ide_chat_branch_conversation(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<BranchUnarchivedConversationFromSelectionInput>(params)?;
    serde_json::to_value(branch_unarchived_conversation_from_selection_internal(input, state).await?)
        .map_err(|err| format!("Serialize branch conversation result failed: {err}"))
}

async fn ide_chat_branch_conversation_from_message(
    state: &AppState,
    params: Value,
) -> Result<Value, String> {
    let input = ide_chat_parse_params::<CreateConversationBranchFromMessageInput>(params)?;
    serde_json::to_value(create_conversation_branch_from_message_internal(input, state).await?)
        .map_err(|err| format!("Serialize branch conversation from message result failed: {err}"))
}

async fn ide_chat_submit_delegate(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<SubmitUserAsyncDelegateInput>(params)?;
    serde_json::to_value(submit_user_async_delegate_internal(input, state).await?)
        .map_err(|err| format!("Serialize delegate submit result failed: {err}"))
}

fn ide_chat_task_create(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<TaskCreateInput>(params)?;
    ide_chat_serialize(task_create_task_inner(input, state)?)
}

fn ide_chat_task_update(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<TaskUpdateInput>(params)?;
    ide_chat_serialize(task_update_task_inner(input, state)?)
}

fn ide_chat_task_delete(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<TaskDeleteInput>(params)?;
    ide_chat_serialize(task_delete_task_inner(input, state)?)
}

fn ide_chat_task_list(state: &AppState) -> Result<Value, String> {
    ide_chat_serialize(task_list_tasks_inner(state)?)
}

async fn ide_chat_task_optimize_draft(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<TaskOptimizeDraftInput>(params)?;
    ide_chat_serialize(task_optimize_draft_internal(input, state).await?)
}

async fn ide_chat_task_dispatch_now(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<TaskDispatchNowInput>(params)?;
    ide_chat_serialize(task_dispatch_task_now_inner(input, state).await?)
}

fn ide_chat_goal_current(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<GoalCancelInput>(params)?;
    serde_json::to_value(goal_get_current_inner(state, &input.conversation_id)?)
        .map_err(|err| format!("Serialize goal current result failed: {err}"))
}

fn ide_chat_goal_create(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<GoalCreateInput>(params)?;
    serde_json::to_value(goal_create_goal_inner(
        state,
        &input.conversation_id,
        &input.objective,
    )?)
    .map_err(|err| format!("Serialize goal create result failed: {err}"))
}

fn ide_chat_goal_cancel(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<GoalCancelInput>(params)?;
    serde_json::to_value(goal_cancel_goal_inner(state, &input.conversation_id)?)
        .map_err(|err| format!("Serialize goal cancel result failed: {err}"))
}
