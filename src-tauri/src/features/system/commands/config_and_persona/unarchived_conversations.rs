mod git_ghost_snapshot;

use git_ghost_snapshot::read_git_snapshot_record_from_provider_meta;
use git_ghost_snapshot::restore_main_workspace_from_git_ghost_snapshot;

fn conversation_preferred_model_repair_candidate(
    config: &AppConfig,
    agents: &[AgentProfile],
    agent_id: &str,
    preferred_api_config_id: Option<&str>,
) -> Option<String> {
    let current = preferred_api_config_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(resolved_current) = current
        .and_then(|value| resolve_model_role_api_config_id(config, value))
        .filter(|resolved_id| config.api_configs.iter().any(|api| api.id == *resolved_id && is_text_chat_api(api)))
    {
        return Some(resolved_current);
    }

    // 会话未固化模型时回退到负责人格的主模型。
    let agent_id = agent_id.trim();
    let primary_id = agents
        .iter()
        .find(|agent| agent.id.trim() == agent_id)
        .map(agent_primary_api_config_id)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| config.expert_api_config_id.trim().to_string());
    resolve_model_role_api_config_id(config, &primary_id)
        .filter(|resolved_id| config.api_configs.iter().any(|api| api.id == *resolved_id && is_text_chat_api(api)))
}

fn repair_conversation_preferred_model_for_snapshot(
    state: &AppState,
    conversation: &Conversation,
) -> Result<Option<String>, String> {
    repair_conversation_preferred_model_for_snapshot_meta(
        state,
        &conversation.id,
        &conversation.agent_id,
        conversation.preferred_api_config_id.as_deref(),
    )
}

fn repair_conversation_preferred_model_for_snapshot_meta(
    state: &AppState,
    conversation_id: &str,
    agent_id: &str,
    preferred_api_config_id: Option<&str>,
) -> Result<Option<String>, String> {
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let repaired = conversation_preferred_model_repair_candidate(
        &runtime_snapshot.config,
        &runtime_snapshot.agents,
        agent_id,
        preferred_api_config_id,
    );
    let current = preferred_api_config_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if repaired.as_deref() != current {
        conversation_service_v2().set_preferred_api_config_id(
            state,
            conversation_id,
            repaired.clone(),
        )?;
        runtime_log_debug(format!(
            "[会话首选模型] 完成，任务=加载快照自动修复，conversation_id={}，旧值={}，新值={}",
            conversation_id,
            current.unwrap_or(""),
            repaired.as_deref().unwrap_or("")
        ));
    }
    Ok(repaired)
}

#[tauri::command]
async fn switch_active_conversation_snapshot(
    input: SwitchActiveConversationSnapshotInput,
    state: State<'_, AppState>,
) -> Result<SwitchActiveConversationSnapshotOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let started_at = std::time::Instant::now();
        let result =
            conversation_service_v2().switch_active_conversation_snapshot(&app_state, &input)?;
        let mut snapshot = result.snapshot;
        if let Ok(conversation_meta) = conversation_service_v2()
            .get_conversation_meta(&app_state, &snapshot.conversation_id)
        {
            snapshot.preferred_api_config_id =
                repair_conversation_preferred_model_for_snapshot_meta(
                    &app_state,
                    &conversation_meta.id,
                    &conversation_meta.agent_id,
                    conversation_meta.preferred_api_config_id.as_deref(),
                )?;
        }
        let unarchived_conversations = result.unarchived_conversations;
        runtime_log_debug(format!(
            "[前台重型快照] 完成，conversation_id={}，message_count={}，has_more_history={}，summary_count={}，duration_ms={}",
            snapshot.conversation_id,
            snapshot.messages.len(),
            snapshot.has_more_history,
            unarchived_conversations.len(),
            started_at.elapsed().as_millis()
        ));

        Ok(SwitchActiveConversationSnapshotOutput {
            conversation_id: snapshot.conversation_id,
            messages: snapshot.messages,
            has_more_history: snapshot.has_more_history,
            runtime_state: snapshot.runtime_state,
            current_todo: snapshot.current_todo,
            current_todos: snapshot.current_todos,
            preferred_api_config_id: snapshot.preferred_api_config_id,
            active_goal: snapshot.active_goal,
            unarchived_conversations,
        })
    })
    .await
    .map_err(|err| format!("切换会话快照任务异常：{err}"))?
}

#[tauri::command]
async fn get_foreground_conversation_light_snapshot(
    input: ForegroundConversationLightSnapshotInput,
    state: State<'_, AppState>,
) -> Result<ForegroundConversationLightSnapshotOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        get_foreground_conversation_light_snapshot_blocking(input, &app_state)
    })
    .await
    .map_err(|err| format!("读取前台轻量快照任务异常：{err}"))?
}

#[tauri::command]
async fn get_foreground_conversation_freshness_snapshot(
    input: ForegroundConversationFreshnessInput,
    state: State<'_, AppState>,
) -> Result<ForegroundConversationFreshnessOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        get_foreground_conversation_freshness_snapshot_blocking(input, &app_state)
    })
    .await
    .map_err(|err| format!("读取前台 freshness 快照任务异常：{err}"))?
}

fn get_foreground_conversation_freshness_snapshot_blocking(
    input: ForegroundConversationFreshnessInput,
    state: &AppState,
) -> Result<ForegroundConversationFreshnessOutput, String> {
    let conversation_meta = conversation_service_v2().get_foreground_conversation_meta_for_fast_path(
        state,
        input.conversation_id.as_deref(),
        input.agent_id.as_deref(),
    )?;
    Ok(if let Some(conversation_meta) = conversation_meta {
        let last_message_id = if conversation_meta.last_message_id.is_some() {
            conversation_meta.last_message_id
        } else {
            build_foreground_conversation_snapshot_from_meta_view(state, &conversation_meta, 1)?
                .last_message_id
        };
        ForegroundConversationFreshnessOutput {
            conversation_id: conversation_meta.id,
            last_message_id,
            updated_at: Some(conversation_meta.updated_at.clone()),
        }
    } else {
        ForegroundConversationFreshnessOutput {
            conversation_id: String::new(),
            last_message_id: None,
            updated_at: None,
        }
    })
}

fn get_foreground_conversation_light_snapshot_blocking(
    input: ForegroundConversationLightSnapshotInput,
    state: &AppState,
) -> Result<ForegroundConversationLightSnapshotOutput, String> {
    let started_at = std::time::Instant::now();
    let recent_limit = input
        .limit
        .unwrap_or(DEFAULT_FOREGROUND_SNAPSHOT_RECENT_LIMIT)
        .clamp(1, 50);
    let mut snapshot = conversation_service_v2().get_foreground_snapshot(
        state,
        input.conversation_id.as_deref(),
        input.agent_id.as_deref(),
        recent_limit,
    )?;
    if let Some(conversation) = conversation_service_v2()
        .mark_conversation_read(state, &snapshot.conversation_id)?
        .conversation
    {
        snapshot.preferred_api_config_id =
            repair_conversation_preferred_model_for_snapshot(state, &conversation)?;
        snapshot.runtime_state = unarchived_conversation_runtime_state(state, &conversation.id);
        snapshot.current_todo = conversation_current_todo_text(&conversation);
        snapshot.current_todos = conversation.current_todos.clone();
        snapshot.active_goal = goal_active_goal_from_conversation(&conversation);
    }
    let mut stream_cache = None;
    let mut should_bind_stream = false;
    let mut resume_projection_authoritative = false;
    if input.resume_projection {
        let runtime_snapshot = read_conversation_runtime_snapshot(state, &snapshot.conversation_id)?;
        should_bind_stream = foreground_runtime_snapshot_should_bind(&runtime_snapshot);
        resume_projection_authoritative = true;
        snapshot.runtime_state = Some(runtime_snapshot.runtime_state.clone());
        stream_cache = Some(runtime_snapshot.stream_cache);
    }
    runtime_log_debug(format!(
        "[前台轻量快照] 完成，conversation_id={}，message_count={}，has_more_history={}，duration_ms={}",
        snapshot.conversation_id,
        snapshot.messages.len(),
        snapshot.has_more_history,
        started_at.elapsed().as_millis()
    ));
    let conversation = conversation_service_v2()
        .read_unarchived_conversation_summary(state, &snapshot.conversation_id)?;

    Ok(ForegroundConversationLightSnapshotOutput {
        conversation_id: snapshot.conversation_id,
        messages: snapshot.messages,
        last_message_id: snapshot.last_message_id,
        has_more_history: snapshot.has_more_history,
        runtime_state: snapshot.runtime_state,
        current_todo: snapshot.current_todo,
        current_todos: snapshot.current_todos,
        preferred_api_config_id: snapshot.preferred_api_config_id,
        active_goal: snapshot.active_goal,
        conversation,
        stream_cache,
        should_bind_stream,
        resume_projection_authoritative,
    })
}

fn foreground_runtime_snapshot_should_bind(snapshot: &ConversationRuntimeSnapshot) -> bool {
    snapshot.runtime_state == MainSessionState::AssistantStreaming
        && !snapshot.stream_cache.persisted_assistant_message_id.trim().is_empty()
}

#[cfg(test)]
mod foreground_resume_projection_tests {
    use super::*;

    fn runtime_snapshot_with_state(
        runtime_state: MainSessionState,
        stream_cache: ConversationStreamRuntimeCacheSnapshot,
    ) -> ConversationRuntimeSnapshot {
        ConversationRuntimeSnapshot {
            conversation_id: "conversation-1".to_string(),
            runtime_state,
            is_processing: false,
            has_pending_queue: false,
            pending_queue_count: 0,
            stream_cache,
        }
    }

    #[test]
    fn idle_runtime_snapshot_with_stale_stream_cache_should_not_bind_stream() {
        let snapshot = runtime_snapshot_with_state(
            MainSessionState::Idle,
            ConversationStreamRuntimeCacheSnapshot {
                assistant_text: "stale partial reply".to_string(),
                has_visible_progress: true,
                persisted_assistant_message_id: "assistant-1".to_string(),
                ..Default::default()
            },
        );

        assert!(!foreground_runtime_snapshot_should_bind(&snapshot));
    }

    #[test]
    fn streaming_runtime_snapshot_should_bind_stream() {
        let snapshot = runtime_snapshot_with_state(
            MainSessionState::AssistantStreaming,
            ConversationStreamRuntimeCacheSnapshot {
                persisted_assistant_message_id: "assistant-1".to_string(),
                ..Default::default()
            },
        );

        assert!(foreground_runtime_snapshot_should_bind(&snapshot));
    }

    #[test]
    fn streaming_runtime_snapshot_without_formal_message_should_not_bind_stream() {
        let snapshot = runtime_snapshot_with_state(
            MainSessionState::AssistantStreaming,
            ConversationStreamRuntimeCacheSnapshot::default(),
        );

        assert!(!foreground_runtime_snapshot_should_bind(&snapshot));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MarkConversationReadInput {
    conversation_id: String,
}

#[tauri::command]
async fn mark_conversation_read(
    input: MarkConversationReadInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conversation_id = input.conversation_id.trim();
        if conversation_id.is_empty() {
            return Ok(false);
        }
        Ok(conversation_service_v2()
            .mark_conversation_read(&app_state, conversation_id)?
            .conversation
            .is_some())
    })
    .await
    .map_err(|err| format!("标记会话已读任务异常：{err}"))?
}

#[tauri::command]
fn set_conversation_plan_mode(
    input: SetConversationPlanModeInput,
    state: State<'_, AppState>,
) -> Result<SetConversationPlanModeOutput, String> {
    set_conversation_plan_mode_inner(input, state.inner())
}

fn set_conversation_plan_mode_inner(
    input: SetConversationPlanModeInput,
    state: &AppState,
) -> Result<SetConversationPlanModeOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }

    let current_enabled =
        get_conversation_plan_mode_enabled(state, conversation_id).unwrap_or(false);
    if current_enabled == input.plan_mode_enabled {
        return Ok(SetConversationPlanModeOutput {
            conversation_id: conversation_id.to_string(),
            plan_mode_enabled: input.plan_mode_enabled,
        });
    }

    set_conversation_plan_mode_enabled(state, conversation_id, input.plan_mode_enabled)?;
    emit_unarchived_conversation_overview_item_updated_from_state(state, conversation_id)?;
    runtime_log_info(format!(
        "[计划模式] 完成，任务=切换会话运行时计划模式，会话ID={}，状态={}",
        conversation_id,
        if input.plan_mode_enabled { "开启" } else { "关闭" }
    ));

    Ok(SetConversationPlanModeOutput {
        conversation_id: conversation_id.to_string(),
        plan_mode_enabled: input.plan_mode_enabled,
    })
}

#[tauri::command]
fn set_conversation_preferred_model(
    input: SetConversationPreferredModelInput,
    state: State<'_, AppState>,
) -> Result<SetConversationPreferredModelOutput, String> {
    set_conversation_preferred_model_inner(input, state.inner())
}

fn set_conversation_preferred_model_inner(
    input: SetConversationPreferredModelInput,
    state: &AppState,
) -> Result<SetConversationPreferredModelOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    let preferred_api_config_id = input
        .preferred_api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    runtime_log_info(format!(
        "[会话模型] 开始，任务=切换会话首选模型，入口=tauri，会话ID={}，api_config_id={}",
        conversation_id,
        preferred_api_config_id.as_deref().unwrap_or("默认模型")
    ));

    let resolved_preferred_api_config_id = if let Some(api_config_id) = preferred_api_config_id.as_deref() {
        let config = state_read_config_cached(state)?;
        let resolved_api_config_id = resolve_model_role_api_config_id(&config, api_config_id)
            .ok_or_else(|| format!("会话首选模型角色未配置：api_config_id={api_config_id}"))?;
        let Some(api_config) = config
            .api_configs
            .iter()
            .find(|api| api.id == resolved_api_config_id)
        else {
            return Err(format!("会话首选模型不存在：api_config_id={api_config_id}"));
        };
        if !api_config.enable_text || !api_config.request_format.is_chat_text() {
            return Err(format!(
                "会话首选模型不是聊天文本模型：api_config_id={}，request_format={:?}",
                api_config_id,
                api_config.request_format
            ));
        }
        Some(resolved_api_config_id)
    } else {
        None
    };

    conversation_service_v2().set_preferred_api_config_id(
        state,
        conversation_id,
        resolved_preferred_api_config_id.clone(),
    )?;

    runtime_log_info(format!(
        "[会话模型] 完成，任务=切换会话首选模型，会话ID={}，api_config_id={}",
        conversation_id,
        resolved_preferred_api_config_id.as_deref().unwrap_or("默认模型")
    ));

    Ok(SetConversationPreferredModelOutput {
        conversation_id: conversation_id.to_string(),
        preferred_api_config_id: resolved_preferred_api_config_id,
    })
}

#[tauri::command]
fn set_conversation_auto_push_remote_contact(
    input: SetConversationAutoPushRemoteContactInput,
    state: State<'_, AppState>,
) -> Result<SetConversationAutoPushRemoteContactOutput, String> {
    set_conversation_auto_push_remote_contact_inner(input, state.inner())
}

fn set_conversation_auto_push_remote_contact_inner(
    input: SetConversationAutoPushRemoteContactInput,
    state: &AppState,
) -> Result<SetConversationAutoPushRemoteContactOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    let remote_contact_id = input
        .remote_contact_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let conversation_meta = conversation_service_v2()
        .get_conversation_meta(state, conversation_id)
        .map_err(|_| "会话不存在".to_string())?;
    if conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_CHAT
        || matches!(
            conversation_meta.conversation_kind.trim(),
            CONVERSATION_KIND_DELEGATE | CONVERSATION_KIND_REMOTE_IM_CONTACT | CONVERSATION_KIND_SYSTEM_NOTIFICATION
        )
        || conversation_meta.status.trim() == "archived"
        || conversation_meta
            .archived_at
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some()
    {
        return Err("仅普通本地会话支持自动推送".to_string());
    }

    if let Some(target_contact_id) = remote_contact_id.as_deref() {
        let has_target = conversation_service_v2()
            .list_remote_im_contact_conversations(state)?
            .iter()
            .any(|item| item.contact_id.trim() == target_contact_id);
        if !has_target {
            return Err(format!("未找到远程联系人：{target_contact_id}"));
        }
    }

    conversation_service_v2().set_auto_push_remote_contact_id(
        state,
        conversation_id,
        remote_contact_id.clone(),
    )?;
    emit_unarchived_conversation_overview_item_updated_from_state(state, conversation_id)?;

    runtime_log_info(format!(
        "[自动推送] 完成，任务=更新会话自动推送目标，会话ID={}，remote_contact_id={}",
        conversation_id,
        remote_contact_id.as_deref().unwrap_or("关闭")
    ));

    Ok(SetConversationAutoPushRemoteContactOutput {
        conversation_id: conversation_id.to_string(),
        remote_contact_id,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUnarchivedConversationInput {
    #[serde(default)]
    api_config_id: Option<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    copy_source_conversation_id: Option<String>,
    #[serde(default)]
    shell_workspaces: Option<Vec<ShellWorkspaceConfig>>,
    #[serde(default)]
    shell_work_mode: Option<String>,
    #[serde(default)]
    shell_work_branch: Option<String>,
    #[serde(default)]
    shell_autonomous_mode: Option<bool>,
    /// true 时创建会话草稿：允许缺省人格，走系统默认；草稿不进入常规新建流程
    #[serde(default)]
    is_draft: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateUnarchivedConversationOutput {
    conversation_id: String,
    unarchived_conversations: Vec<UnarchivedConversationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSideChatConversationInput {
    parent_conversation_id: String,
    /// false 时新建空上文追问（不复制父会话消息），默认 true 保持现有行为
    #[serde(default)]
    with_context: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSideChatConversationOutput {
    conversation_id: String,
    parent_conversation_id: String,
    conversation_kind: String,
    title: String,
}

#[tauri::command]
async fn create_side_chat_conversation(
    input: CreateSideChatConversationInput,
    state: State<'_, AppState>,
) -> Result<CreateSideChatConversationOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        create_side_chat_conversation_blocking(input, &app_state)
    })
    .await
    .map_err(|err| format!("创建追问会话任务异常：{err}"))?
}

fn create_side_chat_conversation_blocking(
    input: CreateSideChatConversationInput,
    state: &AppState,
) -> Result<CreateSideChatConversationOutput, String> {
    let parent_id = input.parent_conversation_id.trim();
    if parent_id.is_empty() {
        return Err("parentConversationId 不能为空".to_string());
    }
    let parent = conversation_service_v2()
        .get_conversation_meta(state, parent_id)
        .map_err(|_| "父会话不存在或已归档".to_string())?;
    if parent.status.trim() == "archived"
        || parent.conversation_kind.trim() != CONVERSATION_KIND_CHAT
    {
        return Err("只能从普通会话创建追问会话".to_string());
    }

    let with_context = input.with_context.unwrap_or(true);
    let copied_messages = if with_context {
        let store_paths = message_store::message_store_paths(&state.data_path, parent_id)?;
        ensure_chat_store_conversation_readable(state, parent_id, &store_paths)?;
        let latest_block = message_store::chat_store_read_block_page(&store_paths, None)?
            .ok_or_else(|| "父会话消息尚未就绪".to_string())?;
        latest_block
            .messages
            .iter()
            .map(clone_chat_message_for_copied_conversation)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let title = parent
        .latest_summary_title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| parent.title.clone());
    let mut side_chat = build_conversation_record(
        parent.preferred_api_config_id.as_deref().unwrap_or_default(),
        &parent.agent_id,
        &title,
        CONVERSATION_KIND_SIDE_CHAT,
        parent.root_conversation_id.clone(),
        None,
    );
    side_chat.parent_conversation_id = Some(parent_id.to_string());
    side_chat.shell_workspace_path = parent.shell_workspace_path.clone();
    side_chat.shell_workspaces = parent.shell_workspaces.clone();
    side_chat.shell_autonomous_mode = parent.shell_autonomous_mode;
    side_chat.shell_work_mode = normalize_shell_work_mode_text(&parent.shell_work_mode);
    side_chat.current_todos = parent.current_todos.clone();
    side_chat.user_profile_snapshot = parent.user_profile_snapshot.clone();
    side_chat.preferred_api_config_id = parent.preferred_api_config_id.clone();
    side_chat.messages = copied_messages;
    if let Some(last_message) = side_chat.messages.last() {
        side_chat.updated_at = last_message.created_at.clone();
        side_chat.last_user_at = side_chat
            .messages
            .iter()
            .rev()
            .find(|message| message.role.trim().eq_ignore_ascii_case("user"))
            .map(|message| message.created_at.clone());
        side_chat.last_assistant_at = side_chat
            .messages
            .iter()
            .rev()
            .find(|message| message.role.trim().eq_ignore_ascii_case("assistant"))
            .map(|message| message.created_at.clone());
        side_chat.fork_message_cursor = Some(last_message.id.clone());
    }
    let side_chat_id = side_chat.id.clone();
    state_schedule_conversation_persist(state, &side_chat)?;
    state_update_conversation_metadata_cached(state, parent_id, |conversation| {
        if !conversation.child_conversation_ids.iter().any(|id| id == &side_chat_id) {
            conversation.child_conversation_ids.push(side_chat_id.clone());
        }
        Ok(())
    })?;
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(state, parent_id);
    runtime_log_info(format!(
        "[追问会话] 完成，任务=创建真实会话，parent_conversation_id={}，conversation_id={}，message_count={}，with_context={}",
        parent_id,
        side_chat_id,
        side_chat.messages.len(),
        with_context
    ));
    Ok(CreateSideChatConversationOutput {
        conversation_id: side_chat_id,
        parent_conversation_id: parent_id.to_string(),
        conversation_kind: CONVERSATION_KIND_SIDE_CHAT.to_string(),
        title,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportConversationShareInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportConversationShareOutput {
    file_name: String,
    payload_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportConversationShareFromFileInput {
    path: String,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    shell_workspaces: Option<Vec<ShellWorkspaceConfig>>,
    #[serde(default)]
    shell_work_mode: Option<String>,
    #[serde(default)]
    shell_autonomous_mode: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationShareSource {
    #[serde(default)]
    conversation_id: String,
    #[serde(default)]
    agent_id: String,
    #[serde(default)]
    user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationSharePayload {
    #[serde(rename = "type")]
    payload_type: String,
    version: u32,
    exported_at: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    source: Option<ConversationShareSource>,
    #[serde(default)]
    messages: Vec<ChatMessage>,
    #[serde(default)]
    current_todos: Vec<ConversationTodoItem>,
    #[serde(default)]
    plan_mode_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BranchUnarchivedConversationFromSelectionInput {
    source_conversation_id: String,
    selected_message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateConversationBranchFromMessageInput {
    source_conversation_id: String,
    turn_message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BranchUnarchivedConversationFromCurrentInput {
    source_conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BranchUnarchivedConversationFromSelectionOutput {
    conversation_id: String,
    title: String,
    #[serde(default)]
    warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForwardUnarchivedConversationSelectionInput {
    source_conversation_id: String,
    target_conversation_id: String,
    selected_message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForwardUnarchivedConversationSelectionOutput {
    target_conversation_id: String,
    forwarded_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForwardSelectionToRemoteImContactInput {
    source_conversation_id: String,
    target_conversation_id: String,
    remote_contact_id: String,
    selected_message_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ForwardSelectionToRemoteImContactOutput {
    target_conversation_id: String,
    remote_contact_id: String,
    forwarded_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameUnarchivedConversationInput {
    conversation_id: String,
    title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameUnarchivedConversationOutput {
    conversation_id: String,
    title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RebindUnarchivedConversationRecipientInput {
    conversation_id: String,
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RebindUnarchivedConversationRecipientOutput {
    conversation_id: String,
    agent_id: String,
    preferred_api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToggleUnarchivedConversationPinInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToggleUnarchivedConversationPinOutput {
    conversation_id: String,
    is_pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveConversationSectionOrderInput {
    tab: String,
    ordered_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveConversationSectionOrderOutput {
    tab: String,
    ordered_keys: Vec<String>,
}

fn normalize_conversation_section_order_keys(keys: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::<String>::new();
    keys.iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .filter(|item| seen.insert((*item).to_string()))
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_conversation_section_order_tab(tab: &str) -> Result<&'static str, String> {
    match tab.trim() {
        "local" => Ok("local"),
        "contact" => Ok("contact"),
        _ => Err("tab 只支持 local 或 contact".to_string()),
    }
}

fn trimmed_option(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn clone_chat_message_for_copied_conversation(message: &ChatMessage) -> ChatMessage {
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: message.role.clone(),
        created_at: message.created_at.clone(),
        speaker_agent_id: message.speaker_agent_id.clone(),
        parts: message.parts.clone(),
        extra_text_blocks: message.extra_text_blocks.clone(),
        provider_meta: message.provider_meta.clone(),
        tool_call: message.tool_call.clone(),
        mcp_call: message.mcp_call.clone(),
        meme_annotations: None,
    }
}

fn sanitize_conversation_share_file_name(title: &str, conversation_id: &str) -> String {
    let base = title
        .trim()
        .chars()
        .map(|ch| match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string();
    let fallback = conversation_id.trim();
    let name = if base.is_empty() {
        if fallback.is_empty() {
            "conversation".to_string()
        } else {
            format!("conversation-{fallback}")
        }
    } else {
        base.chars().take(80).collect::<String>()
    };
    format!("{name}.json")
}

fn normalize_imported_conversation_share_message(
    message: &ChatMessage,
    target_agent_id: &str,
) -> ChatMessage {
    let normalized_role = match message.role.trim().to_ascii_lowercase().as_str() {
        "user" => "user",
        "assistant" => "assistant",
        "tool" => "tool",
        "system" => "system",
        _ => "assistant",
    };
    let speaker_agent_id = match normalized_role {
        "user" => Some(USER_PERSONA_ID.to_string()),
        "assistant" => Some(target_agent_id.trim().to_string()),
        "system" => Some(SYSTEM_PERSONA_ID.to_string()),
        _ => None,
    };
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: normalized_role.to_string(),
        created_at: if message.created_at.trim().is_empty() {
            now_iso()
        } else {
            message.created_at.clone()
        },
        speaker_agent_id,
        parts: message.parts.clone(),
        extra_text_blocks: message.extra_text_blocks.clone(),
        provider_meta: message.provider_meta.clone(),
        tool_call: message.tool_call.clone(),
        mcp_call: message.mcp_call.clone(),
        meme_annotations: None,
    }
}

fn clone_foreground_conversation_for_copy(
    source: &Conversation,
    agent_id: &str,
    title: &str,
) -> Conversation {
    let now = now_iso();
    let mut conversation = source.clone();
    conversation.id = Uuid::new_v4().to_string();
    conversation.title = if title.trim().is_empty() {
        source.title.clone()
    } else {
        title.trim().to_string()
    };
    conversation.agent_id = agent_id.trim().to_string();
    conversation.bound_conversation_id = None;
    conversation.parent_conversation_id = Some(source.id.clone());
    conversation.child_conversation_ids = Vec::new();
    conversation.unread_count = 0;
    conversation.conversation_kind = CONVERSATION_KIND_CHAT.to_string();
    conversation.root_conversation_id = None;
    conversation.delegate_id = None;
    conversation.status = "active".to_string();
    conversation.archived_at = None;
    // 工作树路径是会话专属目录：复制出的会话必须重新解析，不能复用源会话的工作树
    conversation.shell_worktree_path = String::new();
    conversation.created_at = now.clone();
    conversation.updated_at = now.clone();
    conversation.messages = source
        .messages
        .iter()
        .map(clone_chat_message_for_copied_conversation)
        .collect::<Vec<_>>();
    conversation.fork_message_cursor = conversation
        .messages
        .last()
        .map(|message| message.id.clone());
    conversation
}

fn build_branch_conversation_summary_title(
    source_title: &str,
    source_summary_title: Option<&str>,
    first_selected_ordinal: Option<usize>,
    source_is_main_conversation: bool,
) -> String {
    let base_title = source_title.trim();
    let base_summary_title = source_summary_title.map(str::trim).unwrap_or_default();
    let prefix = if source_is_main_conversation {
        "P-ai系统"
    } else if !base_title.is_empty() {
        base_title
    } else if !base_summary_title.is_empty() {
        base_summary_title
    } else {
        "未命名会话"
    };
    match first_selected_ordinal {
        Some(ordinal) => format!("{prefix}[会话分支自第{ordinal}条对话]"),
        None => format!("{prefix}[会话分支]"),
    }
}

fn resolve_branch_from_message_target_index(
    messages: &[ChatMessage],
    turn_message_id: &str,
) -> Option<usize> {
    let normalized_turn_message_id = turn_message_id.trim();
    if normalized_turn_message_id.is_empty() {
        return None;
    }
    messages
        .iter()
        .position(|message| message.id.trim() == normalized_turn_message_id)
}

fn visible_message_ordinal_for_index(messages: &[ChatMessage], target_index: usize) -> usize {
    let mut visible_ordinal = 0usize;
    for (index, message) in messages.iter().enumerate() {
        if archive_pipeline_is_context_compaction_message(message) {
            continue;
        }
        visible_ordinal += 1;
        if index == target_index {
            return visible_ordinal;
        }
    }
    0
}

fn collect_selected_messages_for_branch(
    source: &Conversation,
    selected_message_ids: &[String],
) -> (Vec<ChatMessage>, usize) {
    let selected_ids = selected_message_ids
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let mut selected_messages = Vec::new();
    let mut visible_ordinal = 0usize;
    let mut first_selected_ordinal = 0usize;
    for message in &source.messages {
        if archive_pipeline_is_context_compaction_message(message) {
            continue;
        }
        visible_ordinal += 1;
        if !selected_ids.contains(message.id.trim()) {
            continue;
        }
        if first_selected_ordinal == 0 {
            first_selected_ordinal = visible_ordinal;
        }
        selected_messages.push(message.clone());
    }
    (selected_messages, first_selected_ordinal)
}

#[cfg(test)]
fn branch_conversation_settings_agent_id(
    agents: &[AgentProfile],
    requested_agent_id: &str,
) -> Result<String, String> {
    let normalized_requested_agent_id = requested_agent_id.trim();
    if normalized_requested_agent_id.is_empty() {
        return Err("源会话缺少绑定人格，无法创建分支。".to_string());
    }
    if available_non_user_agent(agents, normalized_requested_agent_id).is_some() {
        return Ok(normalized_requested_agent_id.to_string());
    }
    Err(format!(
        "源会话绑定人格不存在或不可用，无法创建分支: agent_id={normalized_requested_agent_id}"
    ))
}

#[cfg(test)]
fn build_branch_conversation_record_from_selection(
    data_path: &PathBuf,
    data: &AppData,
    source: &Conversation,
    branch_summary_title: &str,
    latest_compaction_message: Option<&ChatMessage>,
    selected_messages: &[ChatMessage],
) -> Result<Conversation, String> {
    let agent_id = branch_conversation_settings_agent_id(&data.agents, &source.agent_id)?;
    let mut conversation = build_conversation_record(
        "",
        &agent_id,
        "",
        CONVERSATION_KIND_CHAT,
        None,
        None,
    );
    conversation.parent_conversation_id = Some(source.id.clone());
    conversation.plan_mode_enabled = source.plan_mode_enabled;
    conversation.shell_workspace_path = source.shell_workspace_path.clone();
    conversation.shell_workspaces = source.shell_workspaces.clone();
    conversation.shell_autonomous_mode = source.shell_autonomous_mode;
    conversation.shell_work_mode = normalize_shell_work_mode_text(&source.shell_work_mode);
    conversation.current_todos = source.current_todos.clone();
    let user_profile_snapshot = data
        .agents
        .iter()
        .find(|item| item.id == agent_id)
        .and_then(|agent| match build_user_profile_snapshot_block(data_path, agent, 12) {
            Ok(snapshot) => snapshot,
            Err(err) => {
                runtime_log_warn(format!(
                    "[会话分支] 跳过，任务=构建用户画像快照，agent_id={}，error={}",
                    agent.id, err
                ));
                None
            }
        })
        .or_else(|| {
            let snapshot = source.user_profile_snapshot.trim();
            if snapshot.is_empty() {
                None
            } else {
                Some(snapshot.to_string())
            }
        });
    if let Some(snapshot) = user_profile_snapshot {
        conversation.user_profile_snapshot = snapshot;
    }
    if let Some(message) = latest_compaction_message {
        conversation
            .messages
            .push(clone_chat_message_for_copied_conversation(message));
    } else {
        conversation
            .messages
            .push(build_initial_summary_context_message(Some(&conversation.current_todos), None));
    }
    conversation_update_latest_summary_title_with_source(
        &mut conversation,
        Some(branch_summary_title),
        Some(SUMMARY_CONTEXT_TITLE_SOURCE_BRANCH),
    );
    conversation.messages.extend(
        selected_messages
            .iter()
            .map(clone_chat_message_for_copied_conversation),
    );
    if let Some(last_message) = conversation.messages.last() {
        conversation.unread_count = 0;
        conversation.updated_at = last_message.created_at.clone();
        conversation.last_user_at = Some(last_message.created_at.clone());
    }
    Ok(conversation)
}

#[cfg(test)]
fn latest_compaction_message_for_branch(source: &Conversation) -> Option<ChatMessage> {
    source
        .messages
        .iter()
        .rev()
        .find(|message| archive_pipeline_is_context_compaction_message(message))
        .cloned()
}

#[tauri::command]
async fn create_unarchived_conversation(
    input: CreateUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<CreateUnarchivedConversationOutput, String> {
    create_unarchived_conversation_inner(input, state.inner()).await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenDraftConversationInput {
    /// 打开草稿时立即写入的工作区；None 表示不修改工作区
    #[serde(default)]
    shell_workspaces: Option<Vec<ShellWorkspaceConfig>>,
    #[serde(default)]
    shell_work_mode: Option<String>,
    #[serde(default)]
    shell_work_branch: Option<String>,
    #[serde(default)]
    shell_autonomous_mode: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenDraftConversationOutput {
    conversation_id: String,
    created: bool,
}

/// 打开会话草稿：存在未归档草稿则直接返回；否则创建一个新草稿。
/// 传入工作区时，无论新建还是复用已有草稿，都会在返回前写入该草稿的工作区。
#[tauri::command]
async fn open_draft_conversation(
    input: Option<OpenDraftConversationInput>,
    state: State<'_, AppState>,
) -> Result<OpenDraftConversationOutput, String> {
    open_draft_conversation_inner(input, state.inner()).await
}

async fn open_draft_conversation_inner(
    input: Option<OpenDraftConversationInput>,
    state: &AppState,
) -> Result<OpenDraftConversationOutput, String> {
    let app_state = state.clone();
    let shell_workspaces = input.as_ref().and_then(|item| item.shell_workspaces.clone());
    let shell_work_mode = input.as_ref().and_then(|item| item.shell_work_mode.clone());
    let shell_autonomous_mode = input.as_ref().and_then(|item| item.shell_autonomous_mode);
    let output = tokio::task::spawn_blocking(
        move || -> Result<OpenDraftConversationOutput, String> {
            if let Some(conversation_id) = find_existing_draft_conversation_id(&app_state)? {
                if shell_workspaces.is_some() || shell_work_mode.is_some() || shell_autonomous_mode.is_some() {
                    conversation_service_v2().apply_external_metadata_patch(
                        &app_state,
                        &conversation_id,
                        "conversation_v2_open_draft_with_workspace",
                        ConversationExternalMetadataPatch {
                            shell_workspaces: shell_workspaces.clone(),
                            shell_work_mode: shell_work_mode.clone(),
                            shell_autonomous_mode,
                            ..Default::default()
                        },
                    )?;
                }
                return Ok(OpenDraftConversationOutput {
                    conversation_id,
                    created: false,
                });
            }
            let input = CreateUnarchivedConversationInput {
                api_config_id: None,
                agent_id: None,
                title: None,
                copy_source_conversation_id: None,
                shell_workspaces,
                shell_work_mode,
                shell_work_branch: None,
                shell_autonomous_mode,
                is_draft: Some(true),
            };
            let result = conversation_service_v2().create_conversation(&app_state, &input)?;
            runtime_log_info(format!(
                "[会话草稿] 完成，任务=创建备用草稿，conversation_id={}",
                result.conversation_id
            ));
            Ok(OpenDraftConversationOutput {
                conversation_id: result.conversation_id,
                created: true,
            })
        },
    )
    .await
    .map_err(|err| format!("打开会话草稿任务异常：{err}"))??;
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &output.conversation_id,
    );
    Ok(output)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateDraftConversationInput {
    conversation_id: String,
    #[serde(default)]
    agent_id: Option<String>,
    /// None=不修改；Some(None)=清空回负责人格默认模型；Some(Some(id))=指定偏好模型
    #[serde(default)]
    preferred_api_config_id: Option<Option<String>>,
    /// None=不修改；Some(Some(title))=自定义会话标题；Some(None)=清空标题
    #[serde(default)]
    title: Option<Option<String>>,
}

/// 在草稿历史区切换人格/模型：直接改写草稿会话字段，作为下次新建的默认值。
#[tauri::command]
async fn update_draft_conversation(
    input: UpdateDraftConversationInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    update_draft_conversation_inner(input, state.inner()).await
}

async fn update_draft_conversation_inner(
    input: UpdateDraftConversationInput,
    state: &AppState,
) -> Result<(), String> {
    let app_state = state.clone();
    let conversation_id = input.conversation_id.trim().to_string();
    let conversation_id_for_emit = conversation_id.clone();
    tokio::task::spawn_blocking(move || {
        if conversation_id.is_empty() {
            return Err("更新会话草稿失败：conversationId 为空。".to_string());
        }
        let conversation_meta = conversation_service_v2()
            .get_conversation_meta(&app_state, &conversation_id)?;
        if !conversation_meta.is_draft {
            return Err(format!(
                "仅会话草稿支持直接改写设置，conversation_id={conversation_id}"
            ));
        }
        let requested_agent_id = input
            .agent_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        if let Some(agent_id) = requested_agent_id.as_deref() {
            validate_draft_agent(&app_state, agent_id)?;
        }
        conversation_service_v2().apply_external_metadata_patch(
            &app_state,
            &conversation_id,
            "conversation_v2_update_draft_conversation",
            ConversationExternalMetadataPatch {
                routing_agent_id: requested_agent_id,
                preferred_api_config_id: input.preferred_api_config_id,
                title: input.title.flatten().map(|value| {
                    let trimmed = value.trim().to_string();
                    if trimmed.is_empty() { "".to_string() } else { trimmed }
                }),
                ..Default::default()
            },
        )?;
        Ok(())
    })
    .await
    .map_err(|err| format!("更新会话草稿任务异常：{err}"))??;
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &conversation_id_for_emit,
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClearChatErrorInput {
    conversation_id: String,
}

#[tauri::command]
async fn clear_chat_error(
    input: ClearChatErrorInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    clear_chat_error_inner(input, state.inner()).await
}

async fn clear_chat_error_inner(input: ClearChatErrorInput, state: &AppState) -> Result<(), String> {
    let app_state = state.clone();
    let conversation_id = input.conversation_id.trim().to_string();
    let conversation_id_for_emit = conversation_id.clone();
    tokio::task::spawn_blocking(move || {
        if conversation_id.is_empty() {
            return Err("清除会话错误失败：conversationId 为空。".to_string());
        }
        state_update_conversation_metadata_cached(&app_state, &conversation_id, |conversation| {
            conversation.last_error = None;
            Ok(())
        })?;
        Ok(())
    })
    .await
    .map_err(|err| format!("清除会话错误任务异常：{err}"))??;
    emit_chat_error_cleared_event(state, &conversation_id_for_emit);
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &conversation_id_for_emit,
    );
    Ok(())
}

fn emit_chat_error_cleared_event(state: &AppState, conversation_id: &str) {
    let payload = serde_json::json!({ "conversationId": conversation_id });
    ide_chat_broadcast_notification("conversation.chatErrorCleared", payload.clone());
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        runtime_log_warn(format!(
            "[聊天推送] chatErrorCleared emit 跳过: app_handle unavailable, conversation_id={}",
            conversation_id
        ));
        return;
    };
    match app_handle.emit("easy-call:chat-error-cleared", payload) {
        Ok(_) => {}
        Err(err) => runtime_log_error(format!(
            "[聊天推送] chatErrorCleared emit 失败: conversation_id={}, error={}",
            conversation_id, err
        )),
    }
}

fn validate_draft_agent(state: &AppState, agent_id: &str) -> Result<(), String> {
    let agents = state_read_agents_cached(state)?;
    let agent_exists = agents
        .iter()
        .any(|agent| agent.id == agent_id.trim() && !agent.is_built_in_user);
    if !agent_exists {
        return Err(format!(
            "会话草稿的人格不存在或不可用: agent_id={agent_id}"
        ));
    }
    Ok(())
}

async fn create_unarchived_conversation_inner(
    input: CreateUnarchivedConversationInput,
    state: &AppState,
) -> Result<CreateUnarchivedConversationOutput, String> {
    let app_state = state.clone();
    let (output, _overview_payload) = tokio::task::spawn_blocking(move || {
        create_unarchived_conversation_blocking(input, &app_state)
    })
    .await
    .map_err(|err| format!("新建未归档会话任务异常：{err}"))??;
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &output.conversation_id,
    );
    Ok(output)
}

fn create_unarchived_conversation_blocking(
    input: CreateUnarchivedConversationInput,
    state: &AppState,
) -> Result<(CreateUnarchivedConversationOutput, UnarchivedConversationOverviewUpdatedPayload), String> {
    let started_at = std::time::Instant::now();
    runtime_log_info(format!(
        "[会话] 开始，任务=新建未归档会话，agent_id={}，api_config_id={}，title_len={}，copy_source_conversation_id={}",
        input.agent_id.as_deref().unwrap_or(""),
        input.api_config_id.as_deref().unwrap_or(""),
        input.title.as_deref().unwrap_or("").chars().count(),
        input.copy_source_conversation_id.as_deref().unwrap_or("")
    ));
    let result = conversation_service_v2().create_conversation(state, &input)?;
    let conversation_id = result.conversation_id.clone();
    let overview_count = result.overview_payload.unarchived_conversations.len();
    let preferred_conversation_id = result
        .overview_payload
        .preferred_conversation_id
        .as_deref()
        .unwrap_or("")
        .to_string();
    runtime_log_info(format!(
        "[会话] 完成，任务=新建未归档会话，阶段=创建并更新索引，conversation_id={}，preferred_conversation_id={}，overview_count={}，duration_ms={}",
        conversation_id,
        preferred_conversation_id,
        overview_count,
        started_at.elapsed().as_millis()
    ));
    runtime_log_info(format!(
        "[会话] 完成，任务=新建未归档会话，阶段=返回前端，conversation_id={}，overview_count={}，duration_ms={}",
        conversation_id,
        overview_count,
        started_at.elapsed().as_millis()
    ));
    let overview_payload = result.overview_payload;
    Ok((
        CreateUnarchivedConversationOutput {
            conversation_id,
            unarchived_conversations: overview_payload.unarchived_conversations.clone(),
        },
        overview_payload,
    ))
}

#[tauri::command]
fn export_conversation_share_json(
    input: ExportConversationShareInput,
    state: State<'_, AppState>,
) -> Result<ExportConversationShareOutput, String> {
    export_conversation_share_json_inner(input, state.inner())
}

fn export_conversation_share_json_inner(
    input: ExportConversationShareInput,
    state: &AppState,
) -> Result<ExportConversationShareOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    let conversation = conversation_service_v2()
        .get_conversation_snapshot(state, conversation_id)
        .map_err(|_| "会话不存在或已归档".to_string())?;
    if conversation_is_archived(&conversation)
        || !conversation_visible_in_foreground_lists(&conversation)
    {
        return Err("只能导出未归档会话".to_string());
    }
    let payload = ConversationSharePayload {
        payload_type: "easy_call_conversation_share".to_string(),
        version: 1,
        exported_at: now_iso(),
        title: conversation.title.clone(),
        source: Some(ConversationShareSource {
            conversation_id: conversation.id.clone(),
            agent_id: conversation.agent_id.clone(),
            user_id: USER_PERSONA_ID.to_string(),
        }),
        messages: conversation.messages.clone(),
        current_todos: conversation.current_todos.clone(),
        plan_mode_enabled: conversation.plan_mode_enabled,
    };
    let payload_json = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("生成会话分享 JSON 失败: {err}"))?;
    runtime_log_info(format!(
        "[会话分享] 完成，任务=导出会话，conversation_id={}，message_count={}",
        conversation.id,
        conversation.messages.len()
    ));
    Ok(ExportConversationShareOutput {
        file_name: sanitize_conversation_share_file_name(&conversation.title, &conversation.id),
        payload_json,
    })
}

#[tauri::command]
fn import_conversation_share_from_file(
    input: ImportConversationShareFromFileInput,
    state: State<'_, AppState>,
) -> Result<CreateUnarchivedConversationOutput, String> {
    let path = input.path.trim();
    if path.is_empty() {
        return Err("path 不能为空".to_string());
    }
    let payload_json = fs::read_to_string(PathBuf::from(path))
        .map_err(|err| format!("读取会话分享文件失败: {err}"))?;
    let payload: ConversationSharePayload = serde_json::from_str(&payload_json)
        .map_err(|err| format!("解析会话分享 JSON 失败: {err}"))?;
    if payload.payload_type.trim() != "easy_call_conversation_share" {
        return Err("不是有效的 PAI 会话分享文件".to_string());
    }
    if payload.messages.is_empty() {
        return Err("会话分享文件没有可导入的消息".to_string());
    }

    let requested_title = input
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            let title = payload.title.trim();
            if title.is_empty() {
                None
            } else {
                Some(title.to_string())
            }
        });
    let create_input = CreateUnarchivedConversationInput {
        api_config_id: None,
        agent_id: input.agent_id.clone(),
        title: requested_title,
        copy_source_conversation_id: None,
        shell_workspaces: input.shell_workspaces.clone(),
        shell_work_mode: input.shell_work_mode.clone(),
        shell_work_branch: None,
        shell_autonomous_mode: input.shell_autonomous_mode,
        is_draft: None,
    };
    let result = conversation_service_v2().create_conversation(state.inner(), &create_input)?;
    let conversation_id = result.conversation_id.clone();
    let mut conversation = conversation_service_v2()
        .get_conversation_snapshot(state.inner(), &conversation_id)
        .map_err(|_| "新建导入会话后读取失败".to_string())?;
    let target_agent_id = conversation.agent_id.clone();
    conversation.messages = payload
        .messages
        .iter()
        .map(|message| normalize_imported_conversation_share_message(message, &target_agent_id))
        .collect();
    conversation.current_todos = payload.current_todos;
    conversation.plan_mode_enabled = payload.plan_mode_enabled;
    conversation.fork_message_cursor = conversation.messages.last().map(|message| message.id.clone());
    if let Some(last_message) = conversation.messages.last() {
        conversation.updated_at = last_message.created_at.clone();
    }
    conversation.last_user_at = conversation
        .messages
        .iter()
        .rev()
        .find(|message| message.role.trim() == "user")
        .map(|message| message.created_at.clone());
    conversation.last_assistant_at = conversation
        .messages
        .iter()
        .rev()
        .find(|message| message.role.trim() == "assistant")
        .map(|message| message.created_at.clone());
    let import_job_id = format!("conversation-share-import-{conversation_id}");
    let import_reason = format!("从会话分享文件导入，path={path}");
    conversation_service_v2().import_conversation_snapshot(
        state.inner(),
        &import_job_id,
        "conversation_share_import",
        &import_reason,
        &conversation,
    )?;

    let overview_payload = UnarchivedConversationOverviewUpdatedPayload {
        preferred_conversation_id: Some(conversation_id.clone()),
        unarchived_conversations: conversation_service_v2()
            .read_unarchived_conversation_summary(state.inner(), &conversation_id)?
            .map(|conversation| vec![conversation])
            .unwrap_or_default(),
    };
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state.inner(),
        &conversation_id,
    );
    runtime_log_info(format!(
        "[会话分享] 完成，任务=导入会话，conversation_id={}，agent_id={}，message_count={}",
        conversation.id,
        conversation.agent_id,
        conversation.messages.len()
    ));
    Ok(CreateUnarchivedConversationOutput {
        conversation_id,
        unarchived_conversations: overview_payload.unarchived_conversations,
    })
}

#[tauri::command]
async fn branch_unarchived_conversation_from_selection(
    input: BranchUnarchivedConversationFromSelectionInput,
    state: State<'_, AppState>,
) -> Result<BranchUnarchivedConversationFromSelectionOutput, String> {
    branch_unarchived_conversation_from_selection_internal(input, state.inner()).await
}

#[tauri::command]
async fn create_conversation_branch_from_message(
    input: CreateConversationBranchFromMessageInput,
    state: State<'_, AppState>,
) -> Result<BranchUnarchivedConversationFromSelectionOutput, String> {
    create_conversation_branch_from_message_internal(input, state.inner()).await
}

#[tauri::command]
async fn branch_unarchived_conversation_from_current(
    input: BranchUnarchivedConversationFromCurrentInput,
    state: State<'_, AppState>,
) -> Result<BranchUnarchivedConversationFromSelectionOutput, String> {
    branch_unarchived_conversation_from_current_internal(input, state.inner()).await
}

/// 从当前会话创建分支：全量复制会话快照，不选消息、不截断、不算序号，忙碌中照常执行。
async fn branch_unarchived_conversation_from_current_internal(
    input: BranchUnarchivedConversationFromCurrentInput,
    state: &AppState,
) -> Result<BranchUnarchivedConversationFromSelectionOutput, String> {
    let started_at = std::time::Instant::now();
    let source_conversation_id = input.source_conversation_id.trim();
    if source_conversation_id.is_empty() {
        return Err("sourceConversationId 不能为空".to_string());
    }

    let source_conversation = conversation_service_v2()
        .try_get_conversation_snapshot(state, source_conversation_id)?
        .filter(|conversation| {
            !conversation_is_archived(conversation)
                && conversation_visible_in_foreground_lists(conversation)
                && conversation_is_local_normal_chat(conversation)
        })
        .ok_or_else(|| "源会话不存在或已归档，无法创建会话分支".to_string())?;
    let branch_summary_title = build_branch_conversation_summary_title(
        &source_conversation.title,
        conversation_latest_summary_title(&source_conversation).as_deref(),
        None,
        source_conversation.id.trim() == SYSTEM_NOTIFICATION_CONVERSATION_ID,
    );
    let create_input = CreateUnarchivedConversationInput {
        api_config_id: source_conversation.preferred_api_config_id.clone(),
        agent_id: Some(source_conversation.agent_id.clone()),
        title: None,
        copy_source_conversation_id: Some(source_conversation.id.clone()),
        shell_workspaces: None,
        shell_work_mode: None,
        shell_work_branch: None,
        shell_autonomous_mode: None,
        is_draft: None,
    };
    let create_result = conversation_service_v2().create_conversation(state, &create_input)?;
    let conversation_id = create_result.conversation_id.clone();
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &conversation_id,
    );
    if conversation_service_v2()
        .update_latest_summary_title_with_source(
            state,
            &conversation_id,
            &branch_summary_title,
            SUMMARY_CONTEXT_TITLE_SOURCE_BRANCH,
        )
        .await
        .unwrap_or(false)
    {
        let _ = emit_unarchived_conversation_overview_item_updated_from_state(
            state,
            &conversation_id,
        );
    }
    runtime_log_info(format!(
        "[会话分支] 完成，任务=从当前会话创建分支，source_conversation_id={}，conversation_id={}，duration_ms={}",
        source_conversation_id,
        conversation_id,
        started_at.elapsed().as_millis()
    ));
    Ok(BranchUnarchivedConversationFromSelectionOutput {
        conversation_id,
        title: branch_summary_title,
        warning: None,
    })
}

async fn create_conversation_branch_from_message_internal(
    input: CreateConversationBranchFromMessageInput,
    state: &AppState,
) -> Result<BranchUnarchivedConversationFromSelectionOutput, String> {
    let started_at = std::time::Instant::now();
    let source_conversation_id = input.source_conversation_id.trim();
    let turn_message_id = input.turn_message_id.trim();
    if source_conversation_id.is_empty() {
        return Err("sourceConversationId 不能为空".to_string());
    }
    if turn_message_id.is_empty() {
        return Err("turnMessageId 不能为空".to_string());
    }

    let source_conversation = conversation_service_v2()
        .try_get_conversation_snapshot(state, source_conversation_id)?
        .filter(|conversation| {
            !conversation_is_archived(conversation)
                && conversation_visible_in_foreground_lists(conversation)
                && conversation_is_local_normal_chat(conversation)
        })
        .ok_or_else(|| "源会话不存在或已归档，无法创建会话分支".to_string())?;
    let target_message_index = resolve_branch_from_message_target_index(
        &source_conversation.messages,
        turn_message_id,
    )
    .ok_or_else(|| "未找到可用于创建会话分支的起始消息".to_string())?;
    let first_selected_ordinal =
        visible_message_ordinal_for_index(&source_conversation.messages, target_message_index);
    let rewind_anchor_index = target_message_index.saturating_add(1);
    let branch_summary_title = build_branch_conversation_summary_title(
        &source_conversation.title,
        conversation_latest_summary_title(&source_conversation).as_deref(),
        Some(first_selected_ordinal).filter(|ordinal| *ordinal > 0),
        source_conversation.id.trim() == SYSTEM_NOTIFICATION_CONVERSATION_ID,
    );
    let create_input = CreateUnarchivedConversationInput {
        api_config_id: source_conversation.preferred_api_config_id.clone(),
        agent_id: Some(source_conversation.agent_id.clone()),
        title: None,
        copy_source_conversation_id: Some(source_conversation.id.clone()),
        shell_workspaces: None,
        shell_work_mode: None,
        shell_work_branch: None,
        shell_autonomous_mode: None,
        is_draft: None,
    };
    let create_result = conversation_service_v2().create_conversation(state, &create_input)?;
    let conversation_id = create_result.conversation_id.clone();
    let branched_conversation = conversation_service_v2()
        .get_conversation_snapshot(state, &conversation_id)
        .map_err(|_| "新建会话分支后读取失败".to_string())?;
    if let Some(rewind_anchor_message_id) = branched_conversation
        .messages
        .get(rewind_anchor_index)
        .map(|message| message.id.clone())
    {
        let rewind_input = RewindConversationInput {
            session: SessionSelector {
                api_config_id: None,
                agent_id: String::new(),
                conversation_id: Some(conversation_id.clone()),
            },
            message_id: rewind_anchor_message_id.clone(),
            undo_apply_patch: false,
        };
        let rewind_started_at = std::time::Instant::now();
        let rewind_result = conversation_service_v2().rewind_conversation(
            state,
            &rewind_input,
            &rewind_anchor_message_id,
            &rewind_started_at,
        )?;
        if rewind_result.removed_count > 0 {
            emit_conversation_todos_updated_payload(
                state,
                &ConversationTodosUpdatedPayload {
                    conversation_id: conversation_id.clone(),
                    current_todo: rewind_result.current_todo,
                    current_todos: rewind_result.current_todos,
                },
            );
        }
    }
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &conversation_id,
    );
    if conversation_service_v2()
        .update_latest_summary_title_with_source(
            state,
            &conversation_id,
            &branch_summary_title,
            SUMMARY_CONTEXT_TITLE_SOURCE_BRANCH,
        )
        .await
        .unwrap_or(false)
    {
        let _ = emit_unarchived_conversation_overview_item_updated_from_state(
            state,
            &conversation_id,
        );
    }
    runtime_log_info(format!(
        "[会话分支] 完成，任务=从此消息开始创建会话分支，source_conversation_id={}，conversation_id={}，turn_message_id={}，target_message_index={}，has_rewind_anchor={}，duration_ms={}",
        source_conversation_id,
        conversation_id,
        turn_message_id,
        target_message_index,
        rewind_anchor_index < branched_conversation.messages.len(),
        started_at.elapsed().as_millis()
    ));
    Ok(BranchUnarchivedConversationFromSelectionOutput {
        conversation_id,
        title: branch_summary_title,
        warning: None,
    })
}

async fn branch_unarchived_conversation_from_selection_internal(
    input: BranchUnarchivedConversationFromSelectionInput,
    state: &AppState,
) -> Result<BranchUnarchivedConversationFromSelectionOutput, String> {
    let source_conversation_id = input.source_conversation_id.trim();
    if source_conversation_id.is_empty() {
        return Err("sourceConversationId 不能为空".to_string());
    }
    let normalized_selected_message_ids = input
        .selected_message_ids
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if normalized_selected_message_ids.is_empty() {
        return Err("selectedMessageIds 不能为空".to_string());
    }

    let result = conversation_service_v2().branch_conversation_from_selection(
        state,
        source_conversation_id,
        &normalized_selected_message_ids,
    )?;
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &result.conversation_id,
    );
    runtime_log_info(format!(
        "[会话分支] 完成，任务=按已选消息创建会话分支，source_conversation_id={}，conversation_id={}，selected_count={}，has_compaction_seed={}",
        source_conversation_id,
        result.conversation_id,
        result.selected_count,
        result.has_compaction_seed
    ));

    Ok(BranchUnarchivedConversationFromSelectionOutput {
        conversation_id: result.conversation_id,
        title: result.title,
        warning: None,
    })
}

#[tauri::command]
async fn forward_unarchived_conversation_selection(
    input: ForwardUnarchivedConversationSelectionInput,
    state: State<'_, AppState>,
) -> Result<ForwardUnarchivedConversationSelectionOutput, String> {
    forward_unarchived_conversation_selection_inner(input, state.inner()).await
}

async fn forward_unarchived_conversation_selection_inner(
    input: ForwardUnarchivedConversationSelectionInput,
    state: &AppState,
) -> Result<ForwardUnarchivedConversationSelectionOutput, String> {
    let source_conversation_id = input.source_conversation_id.trim();
    let target_conversation_id = input.target_conversation_id.trim();
    if source_conversation_id.is_empty() {
        return Err("sourceConversationId 不能为空".to_string());
    }
    if target_conversation_id.is_empty() {
        return Err("targetConversationId 不能为空".to_string());
    }
    if source_conversation_id == target_conversation_id {
        return Err("目标会话不能是当前会话".to_string());
    }
    let normalized_selected_message_ids = input
        .selected_message_ids
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if normalized_selected_message_ids.is_empty() {
        return Err("selectedMessageIds 不能为空".to_string());
    }

    let result = conversation_service_v2()
        .forward_conversation_selection(
            state,
            source_conversation_id,
            target_conversation_id,
            &normalized_selected_message_ids,
        )
        .await?;
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &result.target_conversation_id,
    );
    runtime_log_info(format!(
        "[转发到会话] 完成，任务=转发已选消息到目标会话，source_conversation_id={}，target_conversation_id={}，message_count={}",
        source_conversation_id,
        result.target_conversation_id,
        result.forwarded_count
    ));

    Ok(ForwardUnarchivedConversationSelectionOutput {
        target_conversation_id: result.target_conversation_id,
        forwarded_count: result.forwarded_count,
    })
}

#[tauri::command]
fn forward_selection_to_remote_im_contact(
    input: ForwardSelectionToRemoteImContactInput,
    state: State<'_, AppState>,
) -> Result<ForwardSelectionToRemoteImContactOutput, String> {
    forward_selection_to_remote_im_contact_inner(input, state.inner())
}

fn forward_selection_to_remote_im_contact_inner(
    input: ForwardSelectionToRemoteImContactInput,
    state: &AppState,
) -> Result<ForwardSelectionToRemoteImContactOutput, String> {
    let source_conversation_id = input.source_conversation_id.trim();
    let target_conversation_id = input.target_conversation_id.trim();
    let remote_contact_id = input.remote_contact_id.trim();
    if source_conversation_id.is_empty() {
        return Err("sourceConversationId 不能为空".to_string());
    }
    if target_conversation_id.is_empty() {
        return Err("targetConversationId 不能为空".to_string());
    }
    if remote_contact_id.is_empty() {
        return Err("remoteContactId 不能为空".to_string());
    }
    if source_conversation_id == target_conversation_id {
        return Err("目标会话不能是当前会话".to_string());
    }
    let normalized_selected_message_ids = input
        .selected_message_ids
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if normalized_selected_message_ids.is_empty() {
        return Err("selectedMessageIds 不能为空".to_string());
    }

    let result = conversation_service_v2().forward_selection_to_remote_im_contact(
        state,
        source_conversation_id,
        target_conversation_id,
        remote_contact_id,
        &normalized_selected_message_ids,
    )?;
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &result.target_conversation_id,
    );
    runtime_log_info(format!(
        "[转发到远程联系人] 完成，任务=转发已选消息到远程联系人会话，source_conversation_id={}，target_conversation_id={}，remote_contact_id={}，message_count={}",
        source_conversation_id,
        result.target_conversation_id,
        result.remote_contact_id,
        result.forwarded_count
    ));

    Ok(ForwardSelectionToRemoteImContactOutput {
        target_conversation_id: result.target_conversation_id,
        remote_contact_id: result.remote_contact_id,
        forwarded_count: result.forwarded_count,
    })
}

#[tauri::command]
fn rename_unarchived_conversation(
    input: RenameUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<RenameUnarchivedConversationOutput, String> {
    rename_unarchived_conversation_inner(input, state.inner())
}

fn rename_unarchived_conversation_inner(
    input: RenameUnarchivedConversationInput,
    state: &AppState,
) -> Result<RenameUnarchivedConversationOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    let next_title = clean_text(input.title.trim());
    let next_title = conversation_service_v2().rename_conversation(
        state,
        conversation_id,
        &next_title,
    )?;

    runtime_log_info(format!(
        "[会话] 完成，任务=重命名会话，conversation_id={}，title={}",
        conversation_id, next_title
    ));
    emit_unarchived_conversation_overview_item_updated_from_state(state, conversation_id)?;

    Ok(RenameUnarchivedConversationOutput {
        conversation_id: conversation_id.to_string(),
        title: next_title,
    })
}

async fn rebind_unarchived_conversation_recipient_inner(
    input: RebindUnarchivedConversationRecipientInput,
    state: &AppState,
) -> Result<RebindUnarchivedConversationRecipientOutput, String> {
    let conversation_id = input.conversation_id.trim();
    let agent_id = input.agent_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    if agent_id.is_empty() {
        return Err("新的接收人不能为空".to_string());
    }

    let runtime_org = load_runtime_organization_snapshot(state)?;
    let agent = runtime_org
        .agents
        .iter()
        .find(|item| item.id.trim() == agent_id)
        .ok_or_else(|| format!("目标人格不存在：{agent_id}"))?;
    if agent.is_built_in_user
        || agent.is_built_in_system
        || agent.id.trim() == USER_PERSONA_ID
        || agent.id.trim() == SYSTEM_PERSONA_ID
    {
        return Err("目标接收人不能是用户或系统人格".to_string());
    }

    let agent_id_for_mutation = agent_id.to_string();
    let runtime_org_config_for_mutation = runtime_org.config.clone();
    let runtime_org_agents_for_mutation = runtime_org.agents.clone();
    let preferred_api_config_id = conversation_service_v2()
        .update_unarchived_conversation_by_id(
            state,
            conversation_id,
            move |conversation| {
                if conversation_is_system_notification(conversation) {
                    return Err("系统通知会话不能手动修改接收人".to_string());
                }
                if conversation.conversation_kind.trim() == CONVERSATION_KIND_REMOTE_IM_CONTACT {
                    return Err("远程联系人会话不能手动修改接收人".to_string());
                }
                conversation.agent_id = agent_id_for_mutation.clone();
                conversation.updated_at = now_iso();
                conversation.preferred_api_config_id = conversation_preferred_model_repair_candidate(
                    &runtime_org_config_for_mutation,
                    &runtime_org_agents_for_mutation,
                    &agent_id_for_mutation,
                    conversation.preferred_api_config_id.as_deref(),
                );
                Ok(conversation.preferred_api_config_id.clone())
            },
        )
        .await?;
    emit_unarchived_conversation_overview_item_updated_from_state(state, conversation_id)?;
    runtime_log_info(format!(
        "[会话] 完成，任务=修复会话接收人，conversation_id={}，agent_id={}，preferred_api_config_id={}",
        conversation_id,
        agent_id,
        preferred_api_config_id.as_deref().unwrap_or("")
    ));
    Ok(RebindUnarchivedConversationRecipientOutput {
        conversation_id: conversation_id.to_string(),
        agent_id: agent_id.to_string(),
        preferred_api_config_id,
    })
}

#[tauri::command]
async fn rebind_unarchived_conversation_recipient(
    input: RebindUnarchivedConversationRecipientInput,
    state: State<'_, AppState>,
) -> Result<RebindUnarchivedConversationRecipientOutput, String> {
    rebind_unarchived_conversation_recipient_inner(input, state.inner()).await
}

#[tauri::command]
fn toggle_unarchived_conversation_pin(
    input: ToggleUnarchivedConversationPinInput,
    state: State<'_, AppState>,
) -> Result<ToggleUnarchivedConversationPinOutput, String> {
    toggle_unarchived_conversation_pin_inner(input, state.inner())
}

fn toggle_unarchived_conversation_pin_inner(
    input: ToggleUnarchivedConversationPinInput,
    state: &AppState,
) -> Result<ToggleUnarchivedConversationPinOutput, String> {
    let result = conversation_service_v2().toggle_conversation_pin(
        state,
        &input.conversation_id,
    )?;
    runtime_log_info(format!(
        "[会话] 完成，任务=切换会话置顶，conversation_id={}，is_pinned={}",
        result.conversation_id, result.is_pinned
    ));
    emit_conversation_pin_updated_payload(
        state,
        &ConversationPinUpdatedPayload {
            conversation_id: result.conversation_id.clone(),
            is_pinned: result.is_pinned,
            pin_index: result.pin_index,
        },
    );

    Ok(ToggleUnarchivedConversationPinOutput {
        conversation_id: result.conversation_id,
        is_pinned: result.is_pinned,
    })
}

#[tauri::command]
fn get_conversation_section_orders(
    state: State<'_, AppState>,
) -> Result<ConversationSectionOrders, String> {
    get_conversation_section_orders_inner(state.inner())
}

fn get_conversation_section_orders_inner(
    state: &AppState,
) -> Result<ConversationSectionOrders, String> {
    state_service_get_conversation_section_orders(state)
}

#[tauri::command]
fn save_conversation_section_order(
    input: SaveConversationSectionOrderInput,
    state: State<'_, AppState>,
) -> Result<SaveConversationSectionOrderOutput, String> {
    save_conversation_section_order_inner(input, state.inner())
}

fn save_conversation_section_order_inner(
    input: SaveConversationSectionOrderInput,
    state: &AppState,
) -> Result<SaveConversationSectionOrderOutput, String> {
    let tab = normalize_conversation_section_order_tab(&input.tab)?;
    let ordered_keys = normalize_conversation_section_order_keys(&input.ordered_keys);
    let mut orders = state_service_get_conversation_section_orders(state)?;
    match tab {
        "local" => orders.local = ordered_keys.clone(),
        "contact" => orders.contact = ordered_keys.clone(),
        _ => {}
    }
    state_service_set_conversation_section_orders(state, &orders)?;
    runtime_log_info(format!(
        "[会话分组排序] 完成，任务=保存会话分组顺序，tab={}，group_count={}",
        tab,
        ordered_keys.len()
    ));
    Ok(SaveConversationSectionOrderOutput {
        tab: tab.to_string(),
        ordered_keys,
    })
}

#[tauri::command]
fn list_delegate_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<DelegateConversationSummary>, String> {
    list_delegate_conversations_inner(state.inner())
}

fn list_delegate_conversations_inner(
    state: &AppState,
) -> Result<Vec<DelegateConversationSummary>, String> {
    let mut threads = delegate_runtime_thread_list(state)?;
    threads.extend(delegate_recent_thread_list(state)?);
    let mut seen_ids = std::collections::HashSet::<String>::new();
    let mut summaries = threads
        .iter()
        .filter_map(|thread| {
            seen_ids.insert(thread.delegate_id.clone()).then(|| {
                let mut summary = delegate_conversation_summary_from_runtime_thread(thread);
                summary.title = delegate_display_title_from_id(
                    state,
                    &thread.delegate_id,
                    Some(&thread.conversation),
                    Some(&thread.title),
                );
                summary
            })
        })
        .collect::<Vec<_>>();
    for snapshot in delegate_persisted_conversation_summary_list(state)? {
        let delegate_id = snapshot.delegate_id.clone();
        if !seen_ids.insert(delegate_id.clone()) {
            continue;
        }
        summaries.push(DelegateConversationSummary {
            conversation_id: snapshot.conversation_id.clone(),
            title: delegate_display_title_from_id(
                state,
                &delegate_id,
                None,
                Some(&snapshot.title),
            ),
            updated_at: snapshot.updated_at.clone(),
            last_message_at: snapshot.last_message_at.clone(),
            message_count: snapshot.message_count,
            agent_id: snapshot.target_agent_id.clone(),
            delegate_id: Some(delegate_id),
            root_conversation_id: Some(snapshot.root_conversation_id.clone()),
            archived_at: snapshot.archived_at.clone(),
        });
    }
    summaries.sort_by(|a, b| {
        let bk = b
            .archived_at
            .as_deref()
            .or(b.last_message_at.as_deref())
            .unwrap_or(b.updated_at.as_str());
        let ak = a
            .archived_at
            .as_deref()
            .or(a.last_message_at.as_deref())
            .unwrap_or(a.updated_at.as_str());
        bk.cmp(ak).then_with(|| b.updated_at.cmp(&a.updated_at))
    });
    Ok(summaries)
}

fn clean_delegate_display_title(value: &str) -> String {
    let collapsed = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '“' | '”' | '‘' | '’' | '`' | ':' | '：' | '-' | '。' | '.' | '，' | ','
            )
        })
        .to_string();
    if collapsed.chars().count() > 36 {
        format!("{}...", collapsed.chars().take(36).collect::<String>())
    } else {
        collapsed
    }
}

fn delegate_title_is_generic(value: &str) -> bool {
    matches!(
        value.trim(),
        "" | "未命名委托" | "委托" | "委托任务" | "新委托" | "任务"
    )
}

fn delegate_display_title_parts(title: &str, goal: &str, todo: &str, why: &str) -> String {
    let explicit_title = clean_delegate_display_title(title);
    if !delegate_title_is_generic(&explicit_title) {
        return explicit_title;
    }
    [goal, todo, why]
    .iter()
    .map(|value| clean_delegate_display_title(value))
    .find(|value| !value.is_empty())
    .unwrap_or_else(|| {
        if explicit_title.is_empty() {
            "未命名委托".to_string()
        } else {
            explicit_title
        }
    })
}

fn delegate_display_title_from_snapshot(snapshot: &DelegateConversationSnapshot) -> String {
    delegate_display_title_parts(
        &snapshot.title,
        &snapshot.goal,
        &snapshot.todo,
        &snapshot.why,
    )
}

fn delegate_display_title_from_id(
    app_state: &AppState,
    delegate_id: &str,
    conversation: Option<&Conversation>,
    fallback_title: Option<&str>,
) -> String {
    if let Ok(Some(snapshot)) = delegate_snapshot_cache_get(&app_state.data_path, delegate_id) {
        let title = delegate_display_title_from_snapshot(&snapshot);
        if !title.trim().is_empty() {
            return title;
        }
    }
    let fallback = fallback_title
        .map(clean_delegate_display_title)
        .filter(|value| !delegate_title_is_generic(value))
        .unwrap_or_default();
    if !fallback.is_empty() {
        return fallback;
    }
    conversation
        .map(conversation_preview_title)
        .map(|value| clean_delegate_display_title(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "未命名委托".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListConversationDelegateStatusesInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationDelegateStatusSummary {
    delegate_id: String,
    kind: String,
    conversation_id: String,
    root_conversation_id: String,
    title: String,
    status: String,
    active: bool,
    started_at: String,
    updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived_at: Option<String>,
    elapsed_ms: u64,
    request_count: usize,
    tool_call_count: usize,
    last_tool_name: String,
    token_count: u64,
    input_token_count: u64,
    output_token_count: u64,
    cache_read_token_count: u64,
    cache_write_token_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_agent_id: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct DelegateConversationStats {
    request_count: usize,
    tool_call_count: usize,
    last_tool_name: String,
    token_count: u64,
    cumulative_usage: ConversationCumulativeUsage,
}

fn conversation_delegate_effective_prompt_tokens(message: &ChatMessage) -> Option<u64> {
    message
        .provider_meta
        .as_ref()?
        .get("effectivePromptTokens")?
        .as_u64()
}

fn conversation_delegate_compaction_kind(message: &ChatMessage) -> Option<String> {
    let kind = message
        .provider_meta
        .as_ref()?
        .get("message_meta")
        .or_else(|| message.provider_meta.as_ref()?.get("messageMeta"))?
        .get("kind")?
        .as_str()?
        .trim();
    match kind {
        "context_compaction" => Some("context_compaction".to_string()),
        "summary_context_seed" => Some("summary_context_seed".to_string()),
        _ => None,
    }
}

fn conversation_delegate_token_count(messages: &[ChatMessage]) -> u64 {
    let mut total = 0u64;
    let mut latest_segment_usage = None::<u64>;
    for message in messages {
        if let Some(value) = conversation_delegate_effective_prompt_tokens(message) {
            latest_segment_usage = Some(value);
        }
        if conversation_delegate_compaction_kind(message).is_some() {
            if let Some(value) = latest_segment_usage.take() {
                total = total.saturating_add(value);
            }
        }
    }
    if let Some(value) = latest_segment_usage {
        total = total.saturating_add(value);
    }
    total
}

fn conversation_delegate_text_message_has_content(message: &ChatMessage) -> bool {
    !render_prompt_message_text(message).trim().is_empty()
        || message.extra_text_blocks.iter().any(|item| !item.trim().is_empty())
}

fn conversation_delegate_stats_from_conversation(
    conversation: &Conversation,
    inflight_tool_history: &[Value],
) -> DelegateConversationStats {
    let cumulative_usage = if conversation.cumulative_usage.is_empty() {
        ConversationCumulativeUsage {
            input_tokens: conversation_delegate_token_count(&conversation.messages),
            ..ConversationCumulativeUsage::default()
        }
    } else {
        conversation.cumulative_usage.clone()
    };
    let mut stats = DelegateConversationStats {
        token_count: conversation_cumulative_usage_weighted_tokens(&cumulative_usage),
        cumulative_usage,
        ..DelegateConversationStats::default()
    };
    for message in &conversation.messages {
        if message.role != "assistant" || conversation_delegate_compaction_kind(message).is_some() {
            continue;
        }
        let events = normalize_message_tool_history_events(message, MessageToolHistoryView::Display);
        let assistant_tool_request_count = events
            .iter()
            .filter(|event| event.role == "assistant")
            .count();
        let mut tool_call_count = 0usize;
        let mut last_tool_name = String::new();
        for event in &events {
            if event.role != "assistant" {
                continue;
            }
            for call in &event.tool_calls {
                tool_call_count = tool_call_count.saturating_add(1);
                if let Some(name) = call
                    .tool_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    last_tool_name = name.to_string();
                }
            }
        }
        let has_final_text = conversation_delegate_text_message_has_content(message);
        if assistant_tool_request_count == 0 || has_final_text {
            stats.request_count = stats.request_count.saturating_add(1);
        }
        stats.request_count = stats
            .request_count
            .saturating_add(assistant_tool_request_count);
        stats.tool_call_count = stats.tool_call_count.saturating_add(tool_call_count);
        if !last_tool_name.is_empty() {
            stats.last_tool_name = last_tool_name;
        }
    }
    if !inflight_tool_history.is_empty() {
        let transient = ChatMessage {
            id: "delegate_inflight_tool_history".to_string(),
            role: "assistant".to_string(),
            created_at: String::new(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: String::new(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: Some(inflight_tool_history.to_vec()),
            mcp_call: None,
            meme_annotations: None,
        };
        for event in normalize_message_tool_history_events(&transient, MessageToolHistoryView::Display) {
            if event.role != "assistant" {
                continue;
            }
            stats.request_count = stats.request_count.saturating_add(1);
            for call in event.tool_calls {
                stats.tool_call_count = stats.tool_call_count.saturating_add(1);
                if let Some(name) = call
                    .tool_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    stats.last_tool_name = name.to_string();
                }
            }
        }
    }
    stats
}

fn conversation_delegate_status_from_entry(
    app_state: &AppState,
    delegate_id: &str,
    active: bool,
) -> (String, String, Option<String>) {
    match delegate_snapshot_cache_get(&app_state.data_path, delegate_id) {
        Ok(Some(snapshot)) => {
            let status = if active && snapshot.status == DELEGATE_STATUS_DELIVERED {
                "running".to_string()
            } else {
                snapshot.status
            };
            (status, snapshot.created_at, snapshot.completed_at)
        }
        _ => {
            let status = if active {
                "running".to_string()
            } else {
                "unknown".to_string()
            };
            (status, String::new(), None)
        }
    }
}

fn conversation_delegate_summary_from_thread(
    app_state: &AppState,
    thread: &DelegateRuntimeThread,
    active: bool,
) -> Result<ConversationDelegateStatusSummary, String> {
    let delegate_id = thread.delegate_id.clone();
    let chat_key = delegate_thread_chat_key(thread);
    let inflight_tool_history = if active {
        inflight_completed_tool_history(app_state, &chat_key)?
    } else {
        Vec::new()
    };
    let stats =
        conversation_delegate_stats_from_conversation(&thread.conversation, &inflight_tool_history);
    let (status, stored_started_at, stored_completed_at) =
        conversation_delegate_status_from_entry(app_state, &delegate_id, active);
    let started_at = if stored_started_at.trim().is_empty() {
        thread.conversation.created_at.clone()
    } else {
        stored_started_at
    };
    let snapshot = delegate_snapshot_cache_get(&app_state.data_path, &delegate_id)?
        .unwrap_or_else(|| DelegateConversationSnapshot {
            delegate_id: delegate_id.clone(),
            kind: "normal".to_string(),
            conversation_id: thread.conversation.id.clone(),
            root_conversation_id: thread.root_conversation_id.clone(),
            title: thread.title.clone(),
            why: String::new(),
            goal: String::new(),
            todo: String::new(),
            target_agent_id: thread.target_agent_id.clone(),
            status: status.clone(),
            created_at: started_at.clone(),
            updated_at: thread.conversation.updated_at.clone(),
            completed_at: stored_completed_at.clone(),
            archived_at: thread.archived_at.clone().or_else(|| thread.conversation.archived_at.clone()),
            last_message_at: thread.conversation.messages.last().map(|message| message.created_at.clone()),
            message_count: thread.conversation.messages.len(),
            step_count: stats.request_count,
            tool_call_count: stats.tool_call_count,
            last_tool_name: stats.last_tool_name.clone(),
            cumulative_usage: stats.cumulative_usage.clone(),
        });
    let completed_at = stored_completed_at
        .or_else(|| thread.archived_at.clone())
        .or_else(|| thread.conversation.archived_at.clone());
    Ok(ConversationDelegateStatusSummary {
        delegate_id: delegate_id.clone(),
        kind: snapshot.kind.clone(),
        conversation_id: thread.conversation.id.clone(),
        root_conversation_id: thread.root_conversation_id.clone(),
        title: delegate_display_title_from_id(
            app_state,
            &delegate_id,
            Some(&thread.conversation),
            Some(&thread.title),
        ),
        status,
        active,
        started_at: started_at.clone(),
        updated_at: thread.conversation.updated_at.clone(),
        completed_at: completed_at.clone(),
        archived_at: thread.archived_at.clone().or_else(|| thread.conversation.archived_at.clone()),
        elapsed_ms: 0,
        request_count: stats.request_count,
        tool_call_count: stats.tool_call_count,
        last_tool_name: stats.last_tool_name,
        token_count: stats.token_count,
        input_token_count: stats.cumulative_usage.input_tokens,
        output_token_count: stats.cumulative_usage.output_tokens,
        cache_read_token_count: stats.cumulative_usage.cache_read_tokens,
        cache_write_token_count: stats.cumulative_usage.cache_write_tokens,
        target_agent_id: Some(snapshot.target_agent_id.clone()),
    })
}

fn conversation_delegate_summary_from_snapshot(
    app_state: &AppState,
    snapshot: &DelegateConversationSnapshot,
) -> Result<ConversationDelegateStatusSummary, String> {
    let (status, stored_started_at, stored_completed_at) =
        conversation_delegate_status_from_entry(app_state, &snapshot.delegate_id, false);
    let started_at = if stored_started_at.trim().is_empty() {
        snapshot.created_at.clone()
    } else {
        stored_started_at
    };
    let completed_at = stored_completed_at.or_else(|| snapshot.archived_at.clone());
    Ok(ConversationDelegateStatusSummary {
        delegate_id: snapshot.delegate_id.clone(),
        kind: snapshot.kind.clone(),
        conversation_id: snapshot.conversation_id.clone(),
        root_conversation_id: snapshot.root_conversation_id.clone(),
        title: delegate_display_title_from_snapshot(snapshot),
        status,
        active: false,
        started_at,
        updated_at: snapshot.updated_at.clone(),
        completed_at,
        archived_at: snapshot.archived_at.clone(),
        elapsed_ms: 0,
        request_count: snapshot.step_count,
        tool_call_count: snapshot.tool_call_count,
        last_tool_name: snapshot.last_tool_name.clone(),
        token_count: conversation_cumulative_usage_weighted_tokens(&snapshot.cumulative_usage),
        input_token_count: snapshot.cumulative_usage.input_tokens,
        output_token_count: snapshot.cumulative_usage.output_tokens,
        cache_read_token_count: snapshot.cumulative_usage.cache_read_tokens,
        cache_write_token_count: snapshot.cumulative_usage.cache_write_tokens,
        target_agent_id: Some(snapshot.target_agent_id.clone()),
    })
}

#[tauri::command]
fn list_conversation_delegate_statuses(
    input: ListConversationDelegateStatusesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ConversationDelegateStatusSummary>, String> {
    list_conversation_delegate_statuses_inner(input, state.inner())
}

fn list_conversation_delegate_statuses_inner(
    input: ListConversationDelegateStatusesInput,
    state: &AppState,
) -> Result<Vec<ConversationDelegateStatusSummary>, String> {
    let root_conversation_id = input.conversation_id.trim();
    if root_conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    runtime_log_debug(format!(
        "[委托状态] 开始，任务=list_conversation_delegate_statuses，stage=active_threads，root_conversation_id={}",
        root_conversation_id
    ));
    let active_threads = delegate_runtime_thread_list(state)?;
    runtime_log_debug(format!(
        "[委托状态] 完成，任务=list_conversation_delegate_statuses，stage=active_threads，root_conversation_id={}，thread_count={}",
        root_conversation_id,
        active_threads.len()
    ));
    let active_ids = active_threads
        .iter()
        .map(|thread| thread.delegate_id.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut seen_ids = std::collections::HashSet::<String>::new();
    let mut summaries = Vec::<ConversationDelegateStatusSummary>::new();
    for thread in active_threads {
        if thread.root_conversation_id.trim() != root_conversation_id {
            continue;
        }
        if !seen_ids.insert(thread.delegate_id.clone()) {
            continue;
        }
        summaries.push(conversation_delegate_summary_from_thread(
            state,
            &thread,
            true,
        )?);
    }
    runtime_log_debug(format!(
        "[委托状态] 开始，任务=list_conversation_delegate_statuses，stage=recent_threads，root_conversation_id={}",
        root_conversation_id
    ));
    for thread in delegate_recent_thread_list(state)? {
        if thread.root_conversation_id.trim() != root_conversation_id {
            continue;
        }
        if !seen_ids.insert(thread.delegate_id.clone()) {
            continue;
        }
        summaries.push(conversation_delegate_summary_from_thread(
            state,
            &thread,
            active_ids.contains(&thread.delegate_id),
        )?);
    }
    runtime_log_debug(format!(
        "[委托状态] 完成，任务=list_conversation_delegate_statuses，stage=recent_threads，root_conversation_id={}，summary_count={}",
        root_conversation_id,
        summaries.len()
    ));
    runtime_log_debug(format!(
        "[委托状态] 开始，任务=list_conversation_delegate_statuses，stage=persisted_snapshots，root_conversation_id={}",
        root_conversation_id
    ));
    for snapshot in delegate_persisted_snapshot_list_by_root(state, root_conversation_id)? {
        if !seen_ids.insert(snapshot.delegate_id.clone()) {
            continue;
        }
        summaries.push(conversation_delegate_summary_from_snapshot(
            state,
            &snapshot,
        )?);
    }
    runtime_log_debug(format!(
        "[委托状态] 完成，任务=list_conversation_delegate_statuses，stage=persisted_snapshots，root_conversation_id={}，summary_count={}",
        root_conversation_id,
        summaries.len()
    ));
    summaries.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| b.started_at.cmp(&a.started_at))
    });
    runtime_log_debug(format!(
        "[委托状态] 完成，任务=list_conversation_delegate_statuses，stage=return，root_conversation_id={}，summary_count={}",
        root_conversation_id,
        summaries.len()
    ));
    Ok(summaries)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AbortDelegateConversationInput {
    delegate_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AbortDelegateConversationResult {
    aborted: bool,
}

#[tauri::command]
fn abort_delegate_conversation(
    input: AbortDelegateConversationInput,
    state: State<'_, AppState>,
) -> Result<AbortDelegateConversationResult, String> {
    abort_delegate_conversation_inner(input, state.inner())
}

fn abort_delegate_conversation_inner(
    input: AbortDelegateConversationInput,
    state: &AppState,
) -> Result<AbortDelegateConversationResult, String> {
    let entry = delegate_store_get_delegate(&state.data_path, &input.delegate_id)?;
    let aborted = if entry.kind == "remote_im_reply" {
        abort_remote_im_reply_delegate(state, &input.delegate_id, "用户从委托状态卡片打断")?
    } else {
        abort_delegate_runtime_thread(state, &input.delegate_id, "用户从委托状态卡片打断")?
    };
    Ok(AbortDelegateConversationResult { aborted })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetUnarchivedConversationMessagesInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetUnarchivedConversationRecentMessagesInput {
    conversation_id: String,
    #[serde(default = "default_recent_unarchived_message_limit")]
    limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetUnarchivedConversationMessageByIdInput {
    conversation_id: String,
    message_id: String,
}

fn default_recent_unarchived_message_limit() -> usize {
    5
}

#[tauri::command]
async fn get_unarchived_conversation_messages(
    input: GetUnarchivedConversationMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        conversation_service_v2().get_all_messages(&app_state, &conversation_id)
    })
    .await
    .map_err(|err| format!("读取会话全部消息任务异常：{err}"))?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetUnarchivedConversationRecentBlockMessagesInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetConversationBlockPageInput {
    conversation_id: String,
    #[serde(default)]
    block_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationBlockSummaryOutput {
    block_id: u32,
    message_count: usize,
    first_message_id: String,
    last_message_id: String,
    first_created_at: Option<String>,
    last_created_at: Option<String>,
    is_latest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationBlockPageOutput {
    blocks: Vec<ConversationBlockSummaryOutput>,
    selected_block_id: u32,
    messages: Vec<ChatMessage>,
    has_prev_block: bool,
    has_next_block: bool,
}

fn conversation_block_page_output_from_message_store_page(
    page: message_store::MessageStoreBlockPage,
) -> ConversationBlockPageOutput {
    ConversationBlockPageOutput {
        blocks: page
            .blocks
            .into_iter()
            .map(|item| ConversationBlockSummaryOutput {
                block_id: item.block_id,
                message_count: item.message_count,
                first_message_id: item.first_message_id,
                last_message_id: item.last_message_id,
                first_created_at: item.first_created_at,
                last_created_at: item.last_created_at,
                is_latest: item.is_latest,
            })
            .collect(),
        selected_block_id: page.selected_block_id,
        messages: page.messages,
        has_prev_block: page.has_prev_block,
        has_next_block: page.has_next_block,
    }
}

#[tauri::command]
async fn get_unarchived_conversation_recent_block_messages(
    input: GetUnarchivedConversationRecentBlockMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        conversation_service_v2().get_recent_block_messages(&app_state, &conversation_id)
    })
    .await
    .map_err(|err| format!("读取会话最近块消息任务异常：{err}"))?
}

#[tauri::command]
async fn get_unarchived_conversation_block_page(
    input: GetConversationBlockPageInput,
    state: State<'_, AppState>,
) -> Result<ConversationBlockPageOutput, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let app_state = state.inner().clone();
    let block_id = input.block_id;
    tokio::task::spawn_blocking(move || {
        let page = if let Some(block_id) = block_id {
            conversation_service_v2().get_conversation_block(&app_state, &conversation_id, Some(block_id))?
        } else {
            conversation_service_v2().get_conversation_last_block(&app_state, &conversation_id)?
        };
        Ok(ConversationBlockPageOutput {
            blocks: page
                .blocks
                .into_iter()
                .map(|item| ConversationBlockSummaryOutput {
                    block_id: item.block_id,
                    message_count: item.message_count,
                    first_message_id: item.first_message_id,
                    last_message_id: item.last_message_id,
                    first_created_at: item.first_created_at,
                    last_created_at: item.last_created_at,
                    is_latest: item.is_latest,
                })
                .collect(),
            selected_block_id: page.selected_block_id,
            messages: page.messages,
            has_prev_block: page.has_prev_block,
            has_next_block: page.has_next_block,
        })
    })
    .await
    .map_err(|err| format!("读取会话块分页任务异常：{err}"))?
}

#[tauri::command]
async fn get_unarchived_conversation_recent_messages(
    input: GetUnarchivedConversationRecentMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let app_state = state.inner().clone();
    let limit = input.limit;
    tokio::task::spawn_blocking(move || {
        conversation_service_v2().get_recent_messages_for_frontend_display_only(
            &app_state,
            &conversation_id,
            limit,
        )
    })
    .await
    .map_err(|err| format!("读取会话最近消息任务异常：{err}"))?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetToolResultContentInput {
    conversation_id: String,
    message_id: String,
    tool_call_id: String,
}

#[tauri::command]
async fn get_tool_result_content(
    input: GetToolResultContentInput,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let message_id = input.message_id.trim().to_string();
    if message_id.is_empty() {
        return Err("messageId is required.".to_string());
    }
    let tool_call_id = input.tool_call_id.trim().to_string();
    if tool_call_id.is_empty() {
        return Err("toolCallId is required.".to_string());
    }
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        conversation_service_v2().get_tool_result_content_by_call_id(
            &app_state,
            &conversation_id,
            &message_id,
            &tool_call_id,
        )
    })
    .await
    .map_err(|err| format!("读取工具结果任务异常：{err}"))?
}

#[tauri::command]
async fn get_unarchived_conversation_message_by_id(
    input: GetUnarchivedConversationMessageByIdInput,
    state: State<'_, AppState>,
) -> Result<ChatMessage, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let message_id = input.message_id.trim().to_string();
    if message_id.is_empty() {
        return Err("messageId is required.".to_string());
    }
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        conversation_service_v2().get_message_by_id_for_frontend_display_only(
            &app_state,
            &conversation_id,
            &message_id,
        )
    })
    .await
    .map_err(|err| format!("读取会话单条消息任务异常：{err}"))?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetDelegateConversationMessagesInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteDelegateConversationInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteDelegateConversationOutput {
    deleted_conversation_id: String,
    deleted: bool,
}

#[tauri::command]
async fn get_delegate_conversation_messages(
    input: GetDelegateConversationMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let mut messages = delegate_runtime_thread_conversation_get_any(&app_state, &conversation_id)?
            .map(|conversation| conversation.messages.clone())
            .ok_or_else(|| "Delegate conversation not found.".to_string())?;
        materialize_chat_message_parts_from_media_refs(&mut messages, &app_state.data_path);
        messages.retain(|message| !remote_im_delegate_message_is_internal(message));
        Ok(project_messages_for_frontend_display_only(messages))
    })
    .await
    .map_err(|err| format!("读取委托会话消息任务异常：{err}"))?
}

fn remote_im_delegate_message_is_internal(message: &ChatMessage) -> bool {
    message
        .provider_meta
        .as_ref()
        .and_then(|meta| meta.get("remote_im_delegate_internal"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

#[tauri::command]
async fn get_delegate_conversation_block_page(
    input: GetConversationBlockPageInput,
    state: State<'_, AppState>,
) -> Result<ConversationBlockPageOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        get_delegate_conversation_block_page_inner(input, &app_state)
    })
    .await
    .map_err(|err| format!("读取委托会话块分页任务异常：{err}"))?
}

fn get_delegate_conversation_block_page_inner(
    input: GetConversationBlockPageInput,
    state: &AppState,
) -> Result<ConversationBlockPageOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let mut page = delegate_conversation_store_read_block_page(
        &state.data_path,
        conversation_id,
        input.block_id,
    )?
    .ok_or_else(|| "Delegate conversation not found.".to_string())?;
    materialize_chat_message_parts_from_media_refs(&mut page.messages, &state.data_path);
    page.messages.retain(|message| !remote_im_delegate_message_is_internal(message));
    page.messages = project_messages_for_frontend_display_only(page.messages);
    Ok(conversation_block_page_output_from_message_store_page(page))
}

#[tauri::command]
fn delete_delegate_conversation(
    input: DeleteDelegateConversationInput,
    state: State<'_, AppState>,
) -> Result<DeleteDelegateConversationOutput, String> {
    delete_delegate_conversation_inner(input, state.inner())
}

fn delete_delegate_conversation_inner(
    input: DeleteDelegateConversationInput,
    state: &AppState,
) -> Result<DeleteDelegateConversationOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let deleted = delegate_runtime_thread_conversation_delete(state, conversation_id)?;
    runtime_log_info(format!(
        "[委托会话] 完成，任务=删除委托会话，conversation_id={}，deleted={}",
        conversation_id, deleted
    ));
    Ok(DeleteDelegateConversationOutput {
        deleted_conversation_id: conversation_id.to_string(),
        deleted,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteUnarchivedConversationInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteUnarchivedConversationOutput {
    deleted_conversation_id: String,
    active_conversation_id: String,
    unarchived_conversations: Vec<UnarchivedConversationSummary>,
}

#[tauri::command]
async fn delete_unarchived_conversation(
    input: DeleteUnarchivedConversationInput,
    state: State<'_, AppState>,
) -> Result<DeleteUnarchivedConversationOutput, String> {
    delete_unarchived_conversation_inner(input, state.inner()).await
}

async fn delete_unarchived_conversation_inner(
    input: DeleteUnarchivedConversationInput,
    state: &AppState,
) -> Result<DeleteUnarchivedConversationOutput, String> {
    let app_state = state.clone();
    let output = tokio::task::spawn_blocking(move || {
        delete_unarchived_conversation_blocking(input, &app_state)
    })
    .await
    .map_err(|err| format!("删除未归档会话任务异常：{err}"))??;
    // 删除语义：注册 watermark 删除，前端差量同步收敛；不再全量广播列表。
    overview_register_missing_item(&output.deleted_conversation_id);
    Ok(output)
}

fn delete_unarchived_conversation_blocking(
    input: DeleteUnarchivedConversationInput,
    state: &AppState,
) -> Result<DeleteUnarchivedConversationOutput, String> {
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let started_at = std::time::Instant::now();
    runtime_log_info(format!(
        "[会话] 开始，任务=delete_unarchived_conversation，action=delete_unarchived_convo，convo_id={}",
        conversation_id
    ));
    let result = match conversation_service_v2().delete_conversation(
        state,
        conversation_id,
    ) {
        Ok(result) => result,
        Err(err) => {
            let reason = if err.contains("系统通知会话暂不支持删除") {
                "main_conversation_locked"
            } else if err.contains("Unarchived conversation not found") {
                "not_found"
            } else if err.contains("删除后未找到可用会话") {
                "no_active_conversation_after_delete"
            } else {
                "delete_failed"
            };
            runtime_log_info(format!(
                "[会话] 失败，任务=delete_unarchived_conversation，action=delete_unarchived_convo，convo_id={}，reason={}，duration_ms={}",
                conversation_id,
                reason,
                started_at.elapsed().as_millis()
            ));
            return Err(err);
        }
    };
    runtime_log_info(format!(
        "[会话] 完成，任务=delete_unarchived_conversation，action=delete_unarchived_convo，convo_id={}，duration_ms={}",
        conversation_id,
        started_at.elapsed().as_millis()
    ));
    match delegate_runtime_thread_conversation_delete_by_root(state, conversation_id) {
        Ok(deleted_count) => runtime_log_info(format!(
            "[委托会话] 完成，任务=随会话删除级联清理，root_conversation_id={}，deleted_count={}",
            conversation_id, deleted_count
        )),
        Err(err) => runtime_log_warn(format!(
            "[委托会话] 失败，任务=随会话删除级联清理，root_conversation_id={}，error={}",
            conversation_id, err
        )),
    }
    cleanup_pdf_session_memory_cache_for_conversation(conversation_id);
    // 会话删除后按会话清空截图目录。
    match clear_operate_screenshots_temp(&state.data_path, conversation_id) {
        Ok((file_count, dir_count)) => {
            runtime_log_info(format!(
                "[operate截图缓存] 完成，任务=clear_temp_on_delete，conversation_id={}，截图文件数={}，子目录数={}",
                conversation_id, file_count, dir_count
            ));
        }
        Err(err) => {
            runtime_log_error(format!(
                "[operate截图缓存] 失败，任务=clear_temp_on_delete，conversation_id={}，error={}",
                conversation_id, err
            ));
        }
    }
    Ok(DeleteUnarchivedConversationOutput {
        deleted_conversation_id: result.deleted_conversation_id,
        active_conversation_id: result.active_conversation_id,
        unarchived_conversations: Vec::new(),
    })
}

#[tauri::command]
async fn get_active_conversation_messages(
    input: SessionSelector,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        conversation_service_v2().get_active_conversation_messages(&app_state, &input)
    })
    .await
    .map_err(|err| format!("读取活动会话消息任务异常：{err}"))?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesBeforeInput {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    session: Option<SessionSelector>,
    before_message_id: String,
    limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesBeforeOutput {
    messages: Vec<ChatMessage>,
    has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesAfterInput {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    session: Option<SessionSelector>,
    after_message_id: String,
    #[serde(default = "default_message_page_limit")]
    limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationMessagesAfterOutput {
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestConversationMessagesAfterAsyncInput {
    conversation_id: String,
    #[serde(default)]
    after_message_id: Option<String>,
    #[serde(default = "default_recent_unarchived_message_limit")]
    fallback_limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestConversationMessagesAfterAsyncOutput {
    accepted: bool,
    request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationMessagesAfterAsyncPayload {
    request_id: String,
    conversation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    after_message_id: Option<String>,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn default_message_page_limit() -> usize {
    100
}

fn clone_messages_after_page(
    messages: &[ChatMessage],
    after_message_id: &str,
    limit: usize,
) -> Result<Vec<ChatMessage>, String> {
    let after_idx = messages
        .iter()
        .position(|item| item.id == after_message_id)
        .ok_or_else(|| format!("afterMessageId not found: {after_message_id}"))?;
    let end = (after_idx + 1 + limit).min(messages.len());
    Ok(messages[(after_idx + 1)..end].to_vec())
}

fn clone_messages_before_page(
    messages: &[ChatMessage],
    before_message_id: &str,
    limit: usize,
) -> Result<(Vec<ChatMessage>, bool), String> {
    let before_idx = messages
        .iter()
        .position(|item| item.id == before_message_id)
        .ok_or_else(|| format!("beforeMessageId not found: {before_message_id}"))?;
    let start = before_idx.saturating_sub(limit);
    let has_more = start > 0;
    Ok((messages[start..before_idx].to_vec(), has_more))
}

fn resolve_unarchived_conversation_messages_after(
    state: &AppState,
    conversation_id: &str,
    after_message_id: Option<&str>,
    fallback_limit: usize,
) -> Result<(Vec<ChatMessage>, Option<String>), String> {
    conversation_service_v2().get_messages_after_with_fallback(
        state,
        conversation_id,
        after_message_id,
        fallback_limit,
    )
}

#[tauri::command]
async fn get_active_conversation_messages_before(
    input: GetActiveConversationMessagesBeforeInput,
    state: State<'_, AppState>,
) -> Result<GetActiveConversationMessagesBeforeOutput, String> {
    let before_message_id = input.before_message_id.trim().to_string();
    if before_message_id.is_empty() {
        return Err("beforeMessageId is required.".to_string());
    }
    let limit = input.limit.clamp(1, 100);
    let conversation_id = input
        .conversation_id
        .as_deref()
        .or_else(|| {
            input
                .session
                .as_ref()
                .and_then(|session| session.conversation_id.as_deref())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "conversationId is required.".to_string())?;
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let page = conversation_service_v2().get_messages_before(
            &app_state,
            &conversation_id,
            &before_message_id,
            limit,
        )?;
        Ok(GetActiveConversationMessagesBeforeOutput {
            messages: page.messages,
            has_more: page.has_more,
        })
    })
    .await
    .map_err(|err| format!("读取活动会话历史消息任务异常：{err}"))?
}

#[tauri::command]
async fn get_active_conversation_messages_after(
    input: GetActiveConversationMessagesAfterInput,
    state: State<'_, AppState>,
) -> Result<GetActiveConversationMessagesAfterOutput, String> {
    let after_message_id = input.after_message_id.trim().to_string();
    if after_message_id.is_empty() {
        return Err("afterMessageId is required.".to_string());
    }
    let limit = input.limit.clamp(1, 200);
    let conversation_id = input
        .conversation_id
        .as_deref()
        .or_else(|| {
            input
                .session
                .as_ref()
                .and_then(|session| session.conversation_id.as_deref())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "conversationId is required.".to_string())?;
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let page = conversation_service_v2().get_messages_after(
            &app_state,
            &conversation_id,
            &after_message_id,
            limit,
        )?;
        Ok(GetActiveConversationMessagesAfterOutput {
            messages: page.messages,
        })
    })
    .await
    .map_err(|err| format!("读取活动会话后续消息任务异常：{err}"))?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationCompactionSegmentBeforeInput {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    session: Option<SessionSelector>,
    anchor_message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetActiveConversationCompactionSegmentBeforeOutput {
    messages: Vec<ChatMessage>,
    has_more: bool,
}

#[tauri::command]
async fn get_active_conversation_compaction_segment_before(
    input: GetActiveConversationCompactionSegmentBeforeInput,
    state: State<'_, AppState>,
) -> Result<GetActiveConversationCompactionSegmentBeforeOutput, String> {
    let anchor_message_id = input.anchor_message_id.trim().to_string();
    if anchor_message_id.is_empty() {
        return Err("anchorMessageId is required.".to_string());
    }
    let conversation_id = input
        .conversation_id
        .as_deref()
        .or_else(|| {
            input
                .session
                .as_ref()
                .and_then(|session| session.conversation_id.as_deref())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "conversationId is required.".to_string())?;
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let page = conversation_service_v2().get_compaction_segment_before_anchor(
            &app_state,
            &conversation_id,
            &anchor_message_id,
        )?;
        Ok(GetActiveConversationCompactionSegmentBeforeOutput {
            messages: page.messages,
            has_more: page.has_more,
        })
    })
    .await
    .map_err(|err| format!("读取压缩段分页任务异常：{err}"))?
}


fn request_conversation_messages_after_async_inner(
    input: RequestConversationMessagesAfterAsyncInput,
    state: &AppState,
) -> Result<RequestConversationMessagesAfterAsyncOutput, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let request_id = Uuid::new_v4().to_string();
    let after_message_id = input
        .after_message_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let fallback_limit = input.fallback_limit.clamp(1, 50);
    let state_clone = state.clone();
    let request_id_clone = request_id.clone();
    runtime_log_warn(format!(
        "[聊天推送] 收到异步补消息请求: request_id={}, conversation_id={}, after_message_id={}, fallback_limit={}",
        request_id,
        conversation_id,
        after_message_id.as_deref().unwrap_or(""),
        fallback_limit
    ));
    tauri::async_runtime::spawn(async move {
        let payload = match resolve_unarchived_conversation_messages_after(
            &state_clone,
            &conversation_id,
            after_message_id.as_deref(),
            fallback_limit,
        ) {
            Ok((messages, fallback_mode)) => {
                runtime_log_warn(format!(
                    "[聊天推送] 异步补消息完成: request_id={}, conversation_id={}, message_count={}, fallback_mode={}",
                    request_id_clone,
                    conversation_id,
                    messages.len(),
                    fallback_mode.as_deref().unwrap_or("")
                ));
                ConversationMessagesAfterAsyncPayload {
                    request_id: request_id_clone.clone(),
                    conversation_id: conversation_id.clone(),
                    after_message_id: after_message_id.clone(),
                    messages,
                    fallback_mode,
                    error: None,
                }
            }
            Err(error) => {
                runtime_log_error(format!(
                    "[聊天推送] 异步补消息失败: request_id={}, conversation_id={}, error={}",
                    request_id_clone, conversation_id, error
                ));
                ConversationMessagesAfterAsyncPayload {
                    request_id: request_id_clone.clone(),
                    conversation_id: conversation_id.clone(),
                    after_message_id: after_message_id.clone(),
                    messages: Vec::new(),
                    fallback_mode: None,
                    error: Some(error),
                }
            }
        };
        ide_chat_broadcast_notification(
            "conversation.messagesAfterSynced",
            serde_json::json!(&payload),
        );
        let app_handle = match state_clone.app_handle.lock() {
            Ok(guard) => guard.as_ref().cloned(),
            Err(_) => None,
        };
        let Some(app_handle) = app_handle else {
            runtime_log_warn(format!(
                "[聊天推送] 异步补消息 emit 跳过: app_handle unavailable, request_id={}, conversation_id={}",
                request_id_clone, conversation_id
            ));
            return;
        };
        match app_handle.emit("easy-call:conversation-messages-after-synced", &payload) {
            Ok(_) => runtime_log_info(format!(
                "[聊天推送] 异步补消息 emit 成功: request_id={}, conversation_id={}",
                request_id_clone, conversation_id
            )),
            Err(err) => runtime_log_error(format!(
                "[聊天推送] 异步补消息 emit 失败: request_id={}, conversation_id={}, error={}",
                request_id_clone, conversation_id, err
            )),
        }
    });

    Ok(RequestConversationMessagesAfterAsyncOutput {
        accepted: true,
        request_id,
    })
}

#[tauri::command]
fn request_conversation_messages_after_async(
    input: RequestConversationMessagesAfterAsyncInput,
    state: State<'_, AppState>,
) -> Result<RequestConversationMessagesAfterAsyncOutput, String> {
    request_conversation_messages_after_async_inner(input, state.inner())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewindConversationInput {
    session: SessionSelector,
    message_id: String,
    #[serde(default)]
    undo_apply_patch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewindConversationResult {
    removed_count: usize,
    remaining_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    recalled_user_message: Option<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RewindConversationPreviewResultPayload {
    conversation_id: String,
    can_undo_patch: bool,
    hint: String,
}

fn validate_rewind_input(
    input: &RewindConversationInput,
    started_at: &std::time::Instant,
) -> Result<String, String> {
    let message_id = input.message_id.trim().to_string();
    if message_id.is_empty() {
        let elapsed_ms = started_at.elapsed().as_millis();
        runtime_log_error(format!(
            "[会话撤回] 失败，任务=validate_rewind_input，reason=message_id_empty，duration_ms={}",
            elapsed_ms
        ));
        return Err("messageId is required.".to_string());
    }

    let requested_conversation_id = trimmed_option(input.session.conversation_id.as_deref());
    if requested_conversation_id.is_none() {
        let elapsed_ms = started_at.elapsed().as_millis();
        runtime_log_error(format!(
            "[会话撤回] 失败，任务=validate_rewind_input，reason=conversation_id_empty，duration_ms={}",
            elapsed_ms
        ));
        return Err("conversationId is required.".to_string());
    }

    Ok(message_id)
}

#[cfg(test)]
fn persist_rewind_conversation_state(
    conversation: &mut Conversation,
    remove_from: usize,
) -> Result<(usize, usize, Option<String>, Vec<ConversationTodoItem>), String> {
    let removed_count = conversation.messages.len().saturating_sub(remove_from);
    conversation.messages.truncate(remove_from);
    restore_conversation_todos_after_rewind(conversation)?;
    conversation.updated_at = now_iso();
    conversation.last_user_at = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.created_at.clone());
    conversation.last_assistant_at = conversation
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "assistant")
        .map(|m| m.created_at.clone());
    Ok((
        removed_count,
        remove_from,
        conversation_current_todo_text(conversation),
        conversation.current_todos.clone(),
    ))
}

fn latest_todos_from_message_tool_history(
    message: &ChatMessage,
) -> Result<Option<Vec<ConversationTodoItem>>, String> {
    for event in normalize_message_tool_history_events(message, MessageToolHistoryView::Display)
        .into_iter()
        .rev()
    {
        if event.role != "assistant" {
            continue;
        }
        for call in event.tool_calls.into_iter().rev() {
            if call.tool_name.as_deref().map(str::trim) != Some("todo") {
                continue;
            }
            let raw_arguments = match &call.raw_arguments {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            let request = serde_json::from_str::<TodoWriteRequest>(&raw_arguments)
                .map_err(|err| format!("todo 参数不是合法 JSON：{err}"))?;
            let normalized = todo_items_normalized(&request.todos)?;
            let stored = if !normalized.is_empty()
                && normalized.iter().all(|item| item.status == "completed")
            {
                Vec::new()
            } else {
                normalized
            };
            return Ok(Some(stored));
        }
    }
    Ok(None)
}

#[cfg(test)]
fn restore_conversation_todos_after_rewind(conversation: &mut Conversation) -> Result<(), String> {
    let mut restored = None::<Vec<ConversationTodoItem>>;
    for message in conversation.messages.iter().rev() {
        if let Some(todos) = latest_todos_from_message_tool_history(message)? {
            restored = Some(todos);
            break;
        }
    }
    conversation.current_todos = restored.unwrap_or_default();
    Ok(())
}

#[tauri::command]
async fn preview_rewind_conversation_from_message(
    input: RewindConversationInput,
    state: State<'_, AppState>,
) -> Result<RewindConversationPreviewResultPayload, String> {
    preview_rewind_conversation_from_message_inner(input, state.inner()).await
}

async fn preview_rewind_conversation_from_message_inner(
    input: RewindConversationInput,
    state: &AppState,
) -> Result<RewindConversationPreviewResultPayload, String> {
    let started_at = std::time::Instant::now();
    let message_id = validate_rewind_input(&input, &started_at)?;
    runtime_log_info(format!(
        "[会话撤回] 开始，任务=preview_rewind_conversation_from_message，message_id={}",
        message_id
    ));
    let result = conversation_service_v2().preview_rewind_conversation(
        state,
        &input,
        &message_id,
    )?;
    runtime_log_info(format!(
        "[会话撤回] 完成，任务=preview_rewind_conversation_from_message，conversation_id={}，can_undo_patch={}，duration_ms={}",
        result.conversation_id,
        result.can_undo_patch,
        started_at.elapsed().as_millis()
    ));
    Ok(RewindConversationPreviewResultPayload {
        conversation_id: result.conversation_id,
        can_undo_patch: result.can_undo_patch,
        hint: result.hint,
    })
}

#[tauri::command]
async fn rewind_conversation_from_message(
    input: RewindConversationInput,
    state: State<'_, AppState>,
) -> Result<RewindConversationResult, String> {
    rewind_conversation_from_message_inner(input, state.inner()).await
}

async fn rewind_conversation_from_message_inner(
    input: RewindConversationInput,
    state: &AppState,
) -> Result<RewindConversationResult, String> {
    let started_at = std::time::Instant::now();
    let message_id = validate_rewind_input(&input, &started_at)?;
    runtime_log_info(format!(
        "[会话撤回] 开始，任务=rewind_conversation_from_message，message_id={}，undo_apply_patch={}",
        message_id, input.undo_apply_patch
    ));

    let result = conversation_service_v2().rewind_conversation(
        state,
        &input,
        &message_id,
        &started_at,
    )?;
    let conversation_id = result.conversation_id;
    let removed_count = result.removed_count;
    let remaining_count = result.remaining_count;
    let current_todo = result.current_todo;
    let current_todos = result.current_todos;
    let mut recalled_user_message = result.recalled_user_message;
    let git_snapshot = result.git_snapshot;

    if removed_count > 0 {
        emit_conversation_todos_updated_payload(
            state,
            &ConversationTodosUpdatedPayload {
                conversation_id: conversation_id.clone(),
                current_todo,
                current_todos,
            },
        );
    }

    if let Some(snapshot) = git_snapshot.as_ref() {
        if snapshot.status.trim() == "created"
            && snapshot
                .ghost_commit_id
                .as_deref()
                .map(|value: &str| value.trim())
                .filter(|value: &&str| !value.is_empty())
                .is_some()
        {
            runtime_log_info(format!(
                "[会话撤回] 开始 Git 幽灵快照恢复，任务=rewind_conversation_from_message，conversation_id={}，message_id={}，workspace={}",
                conversation_id,
                message_id,
                snapshot.main_workspace_path
            ));
            match restore_main_workspace_from_git_ghost_snapshot(snapshot).await {
                Ok(()) => runtime_log_info(format!(
                    "[会话撤回] Git 幽灵快照恢复完成，任务=rewind_conversation_from_message，conversation_id={}，message_id={}，commit_id={}",
                    conversation_id,
                    message_id,
                    snapshot.ghost_commit_id.as_deref().unwrap_or_default()
                )),
                Err(err) => runtime_log_error(format!(
                    "[会话撤回] Git 幽灵快照恢复失败，任务=rewind_conversation_from_message，conversation_id={}，message_id={}，error={}",
                    conversation_id, message_id, err
                )),
            }
        } else {
            runtime_log_info(format!(
                "[会话撤回] 跳过 Git 幽灵快照恢复，任务=rewind_conversation_from_message，conversation_id={}，message_id={}，status={}",
                conversation_id, message_id, snapshot.status
            ));
        }
    }

    if let Some(message) = recalled_user_message.as_mut() {
        materialize_message_parts_from_media_refs(&mut message.parts, &state.data_path);
    }
    let elapsed_ms = started_at.elapsed().as_millis();
    runtime_log_info(format!(
        "[会话撤回] 完成，任务=rewind_conversation_from_message，removed_count={}，remaining_count={}，duration_ms={}",
        removed_count, remaining_count, elapsed_ms
    ));

    // 撤回按消息撤：该消息及其后消息全部删除，按会话清空截图目录语义正确。
    match clear_operate_screenshots_temp(&state.data_path, &conversation_id) {
        Ok((file_count, dir_count)) => {
            runtime_log_info(format!(
                "[operate截图缓存] 完成，任务=clear_temp_on_rewind，conversation_id={}，截图文件数={}，子目录数={}",
                conversation_id, file_count, dir_count
            ));
        }
        Err(err) => {
            runtime_log_error(format!(
                "[operate截图缓存] 失败，任务=clear_temp_on_rewind，conversation_id={}，error={}",
                conversation_id, err
            ));
        }
    }

    Ok(RewindConversationResult {
        removed_count,
        remaining_count,
        recalled_user_message,
    })
}

#[cfg(test)]
mod unarchived_conversations_tests {
    use super::*;

    fn build_test_message(id: &str, text: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: "assistant".to_string(),
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

    #[test]
    fn clone_foreground_conversation_for_copy_should_reset_worktree_path() {
        let mut source = build_test_conversation();
        source.shell_work_mode = SHELL_WORK_MODE_WORKTREE.to_string();
        source.shell_worktree_path = "D:/work/demo/.pai/.worktree/20260921-a1b2c3d4".to_string();

        let copied = clone_foreground_conversation_for_copy(&source, "agent-a", "副本");

        assert!(
            copied.shell_worktree_path.is_empty(),
            "复制出的会话必须重新解析自己的工作树，不能复用源会话目录"
        );
        assert_ne!(copied.id, source.id, "复制出的会话应使用新 id");
    }

    fn build_test_conversation() -> Conversation {
        Conversation {
            id: "source-conversation".to_string(),
            title: "原会话".to_string(),
            agent_id: "agent-a".to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: "2026-04-18T10:00:00Z".to_string(),
            updated_at: "2026-04-18T10:01:00Z".to_string(),
            last_user_at: None,
            last_assistant_at: Some("2026-04-18T10:01:00Z".to_string()),
            status: "active".to_string(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            shell_worktree_path: String::new(),
            shell_recorded_branch: String::new(),
            archived_at: None,
            messages: vec![build_test_message("m1", "hello"), build_test_message("m2", "world")],
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: true,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    fn build_test_compaction_message(id: &str) -> ChatMessage {
        let mut message = build_test_message(id, "[上下文整理]");
        message.role = "user".to_string();
        message.speaker_agent_id = Some(SYSTEM_PERSONA_ID.to_string());
        message.provider_meta = Some(serde_json::json!({
            "message_meta": {
                "kind": "context_compaction",
                "scene": "compaction",
            }
        }));
        message
    }

    fn build_test_usage_message(id: &str, effective_prompt_tokens: u64) -> ChatMessage {
        let mut message = build_test_message(id, "assistant");
        message.provider_meta = Some(serde_json::json!({
            "effectivePromptTokens": effective_prompt_tokens,
        }));
        message
    }

    fn build_test_todo_tool_message(
        id: &str,
        todos: serde_json::Value,
        tool_result: &str,
    ) -> ChatMessage {
        let mut message = build_test_message(id, "");
        message.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "id": format!("call_{id}"),
                    "type": "function",
                    "function": {
                        "name": "todo",
                        "arguments": serde_json::json!({ "todos": todos }),
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": format!("call_{id}"),
                "content": tool_result,
            }),
        ]);
        message
    }

    #[test]
    fn conversation_delegate_token_count_should_sum_latest_usage_per_compaction_segment() {
        let messages = vec![
            build_test_usage_message("a1", 100),
            build_test_usage_message("a2", 140),
            build_test_compaction_message("c1"),
            build_test_usage_message("a3", 25),
            build_test_usage_message("a4", 40),
        ];

        assert_eq!(conversation_delegate_token_count(&messages), 180);
    }

    #[test]
    fn conversation_delegate_stats_should_use_weighted_cumulative_usage() {
        let mut conversation = build_test_conversation();
        conversation.cumulative_usage = ConversationCumulativeUsage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: 1000,
            cache_write_tokens: 30,
            ..ConversationCumulativeUsage::default()
        };

        let stats = conversation_delegate_stats_from_conversation(&conversation, &[]);

        assert_eq!(stats.token_count, 150);
        assert_eq!(stats.cumulative_usage.input_tokens, 100);
        assert_eq!(stats.cumulative_usage.output_tokens, 50);
        assert_eq!(stats.cumulative_usage.cache_read_tokens, 1000);
        assert_eq!(stats.cumulative_usage.cache_write_tokens, 30);
    }

    #[test]
    fn conversation_delegate_stats_should_count_tool_rounds_and_final_reply() {
        let mut message = build_test_message("assistant-1", "最终答复");
        message.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\":\"README.md\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "ok",
            }),
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_2",
                    "type": "function",
                    "function": {
                        "name": "apply_patch",
                        "arguments": "{\"path\":\"src/main.rs\"}"
                    }
                }]
            }),
        ]);

        let mut conversation = build_test_conversation();
        conversation.messages = vec![message];
        let stats = conversation_delegate_stats_from_conversation(&conversation, &[]);

        assert_eq!(stats.request_count, 3);
        assert_eq!(stats.tool_call_count, 2);
        assert_eq!(stats.last_tool_name, "apply_patch");
    }

    #[test]
    fn persist_rewind_conversation_state_should_restore_previous_todos_from_remaining_messages() {
        let mut conversation = build_test_conversation();
        conversation.messages = vec![
            ChatMessage {
                id: "user-1".to_string(),
                role: "user".to_string(),
                created_at: "2026-04-18T10:00:00Z".to_string(),
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "先做任务".to_string(),
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
                meme_annotations: None,
            },
            build_test_todo_tool_message(
                "assistant-1",
                serde_json::json!([
                    { "content": "第一步", "status": "completed" },
                    { "content": "第二步", "status": "in_progress" }
                ]),
                "## Current Todo List\n\n✓ 第一步\n→ 第二步",
            ),
            ChatMessage {
                id: "user-2".to_string(),
                role: "user".to_string(),
                created_at: "2026-04-18T10:02:00Z".to_string(),
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "再做一轮".to_string(),
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
                meme_annotations: None,
            },
            build_test_todo_tool_message(
                "assistant-2",
                serde_json::json!([{ "content": "新步骤", "status": "in_progress" }]),
                "## Current Todo List\n\n→ 新步骤",
            ),
        ];
        conversation.current_todos = vec![ConversationTodoItem {
            content: "新步骤".to_string(),
            status: "in_progress".to_string(),
        }];

        let (removed_count, remaining_count, current_todo, current_todos) =
            persist_rewind_conversation_state(&mut conversation, 2).expect("rewind state");

        assert_eq!(removed_count, 2);
        assert_eq!(remaining_count, 2);
        assert_eq!(current_todo.as_deref(), Some("第二步"));
        assert_eq!(current_todos.len(), 2);
        assert_eq!(conversation.current_todos.len(), 2);
        assert_eq!(conversation.current_todos[0].content, "第一步");
        assert_eq!(conversation.current_todos[0].status, "completed");
        assert_eq!(conversation.current_todos[1].content, "第二步");
        assert_eq!(conversation.current_todos[1].status, "in_progress");
    }

    #[test]
    fn persist_rewind_conversation_state_should_clear_todos_when_no_history_found() {
        let mut conversation = build_test_conversation();
        conversation.current_todos = vec![ConversationTodoItem {
            content: "残留步骤".to_string(),
            status: "in_progress".to_string(),
        }];

        let (removed_count, remaining_count, current_todo, current_todos) =
            persist_rewind_conversation_state(&mut conversation, 1).expect("rewind state");

        assert_eq!(removed_count, 1);
        assert_eq!(remaining_count, 1);
        assert_eq!(current_todo, None);
        assert!(current_todos.is_empty());
        assert!(conversation.current_todos.is_empty());
    }

    #[test]
    fn clone_foreground_conversation_for_copy_should_record_parent_and_fork_cursor() {
        let source = build_test_conversation();
        let cloned = clone_foreground_conversation_for_copy(&source, "agent-b", "");

        assert_ne!(cloned.id, source.id);
        assert_eq!(cloned.title, source.title);
        assert_eq!(cloned.parent_conversation_id.as_deref(), Some(source.id.as_str()));
        assert!(cloned.bound_conversation_id.is_none());
        assert_eq!(cloned.agent_id, "agent-b");
        assert_eq!(cloned.messages.len(), source.messages.len());
        assert_ne!(cloned.messages[0].id, source.messages[0].id);
        assert_eq!(
            cloned.fork_message_cursor.as_deref(),
            cloned.messages.last().map(|message| message.id.as_str())
        );
    }

    #[test]
    fn collect_selected_messages_for_branch_should_keep_source_order_and_visible_ordinal() {
        let mut source = build_test_conversation();
        source.messages.insert(
            0,
            build_initial_summary_context_message(None, None),
        );
        let (selected, first_selected_ordinal) = collect_selected_messages_for_branch(
            &source,
            &["m2".to_string(), "m1".to_string()],
        );

        assert_eq!(first_selected_ordinal, 1);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].id, "m1");
        assert_eq!(selected[1].id, "m2");
    }

    #[test]
    fn resolve_branch_from_message_target_index_should_use_exact_message() {
        let mut source = build_test_conversation();
        source.messages = vec![
            {
                let mut message = build_test_message("u1", "user");
                message.role = "user".to_string();
                message
            },
            build_test_message("a1", "assistant"),
            {
                let mut message = build_test_message("u2", "user");
                message.role = "user".to_string();
                message
            },
        ];

        let target_index =
            resolve_branch_from_message_target_index(&source.messages, "a1").expect("target index");

        assert_eq!(target_index, 1);
        assert_eq!(target_index.saturating_add(1), 2);
    }

    #[test]
    fn branch_from_message_next_index_should_point_past_tail_for_last_message() {
        let source = build_test_conversation();
        let target_index =
            resolve_branch_from_message_target_index(&source.messages, "m2").expect("target index");

        assert_eq!(target_index, 1);
        assert_eq!(target_index.saturating_add(1), source.messages.len());
    }

    #[test]
    fn build_branch_conversation_summary_title_should_include_source_title_and_ordinal() {
        assert_eq!(
            build_branch_conversation_summary_title("原会话", None, Some(7), false),
            "原会话[会话分支自第7条对话]"
        );
        assert_eq!(
            build_branch_conversation_summary_title("Chat 2026-04-18T10:00", Some("摘要标题"), Some(3), true),
            "P-ai系统[会话分支自第3条对话]"
        );
        assert_eq!(
            build_branch_conversation_summary_title("", Some("摘要标题"), Some(2), false),
            "摘要标题[会话分支自第2条对话]"
        );
        assert_eq!(
            build_branch_conversation_summary_title("原会话", None, None, false),
            "原会话[会话分支]"
        );
        assert_eq!(
            build_branch_conversation_summary_title("", None, None, true),
            "P-ai系统[会话分支]"
        );
    }

    #[test]
    fn build_branch_conversation_record_should_copy_latest_compaction_and_selected_messages() {
        let mut data = AppData::default();
        data.agents.push(AgentProfile {
            id: "agent-a".to_string(),
            name: "助手".to_string(),
            system_prompt: String::new(),
            tools: Vec::new(),
            created_at: "2026-04-18T10:00:00Z".to_string(),
            updated_at: "2026-04-18T10:00:00Z".to_string(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: "manual".to_string(),
            scope: "global".to_string(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            include_system_rules: true,
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        });
        let source = Conversation {
            messages: vec![
                build_test_compaction_message("seed-1"),
                build_test_message("m1", "hello"),
                build_test_compaction_message("seed-2"),
                build_test_message("m2", "world"),
            ],
            fast_request_turns: Vec::new(),
            ..build_test_conversation()
        };
        let branched = build_branch_conversation_record_from_selection(
            &PathBuf::from("."),
            &data,
            &source,
            "会话分支标题",
            latest_compaction_message_for_branch(&source).as_ref(),
            &[source.messages[1].clone(), source.messages[3].clone()],
        )
        .expect("build branch conversation");

        assert_eq!(branched.messages.len(), 3);
        assert_eq!(
            render_prompt_message_text(&branched.messages[0]),
            render_prompt_message_text(&source.messages[2])
        );
        assert_eq!(
            render_prompt_message_text(&branched.messages[1]),
            render_prompt_message_text(&source.messages[1])
        );
        assert_eq!(
            render_prompt_message_text(&branched.messages[2]),
            render_prompt_message_text(&source.messages[3])
        );
        assert!(branched.title.is_empty());
        assert_eq!(
            conversation_latest_summary_title(&branched).as_deref(),
            Some("会话分支标题")
        );
        assert_eq!(
            branched
                .messages
                .first()
                .and_then(summary_context_message_title_source),
            Some(SUMMARY_CONTEXT_TITLE_SOURCE_BRANCH)
        );
        assert_ne!(branched.messages[0].id, source.messages[2].id);
    }
}