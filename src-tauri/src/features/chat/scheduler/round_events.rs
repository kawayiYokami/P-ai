fn latest_user_text_from_events(events: &[ChatPendingEvent]) -> String {
    events
        .iter()
        .flat_map(|event| event.messages.iter())
        .rev()
        .find_map(|message| {
            if message.role.trim() != "user" {
                return None;
            }
            message.parts.iter().find_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
        })
        .unwrap_or_default()
}

fn emit_history_flushed_event(
    state: &AppState,
    payload: &serde_json::Value,
    _conversation_id: &str,
    event_ids: &[String],
) {
    ide_chat_broadcast_notification("chat.historyFlushed", payload.clone());
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        runtime_log_debug(format!(
            "[聊天] history_flushed emit 失败: app_handle unavailable 事件编号={}",
            event_ids.join("、")
        ));
        return;
    };
    match app_handle.emit(CHAT_HISTORY_FLUSHED_EVENT, payload) {
        Ok(_) => {}
        Err(err) => {
            runtime_log_error(format!(
                "[聊天] history_flushed emit 失败 事件编号={} 原因={}",
                event_ids.join("、"),
                err
            ));
        }
    }
}

fn emit_round_started_event(
    state: &AppState,
    conversation_id: &str,
    activation_id: &str,
    request_id: &str,
    assistant_message_id: &str,
    reason: &str,
    agent_id: &str,
    started_at: &str,
    started_at_ms: u64,
) {
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        runtime_log_debug(format!(
            "[聊天] emit round_started 失败: app_handle unavailable"
        ));
        return;
    };
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "activationId": activation_id,
        "requestId": request_id,
        "assistantMessageId": assistant_message_id,
        "reason": reason,
        "agentId": agent_id,
        "startedAt": started_at,
        "startedAtMs": started_at_ms,
    });
    ide_chat_broadcast_notification("chat.roundStarted", payload.clone());
    match app_handle.emit(CHAT_ROUND_STARTED_EVENT, payload) {
        Ok(_) => {}
        Err(err) => runtime_log_error(format!(
            "[聊天] emit round_started 失败 原因={}",
            err
        )),
    }
}

fn emit_round_completed_event(
    state: &AppState,
    conversation_id: &str,
    result: &SendChatResult,
    activation_id: Option<&str>,
    request_id: Option<&str>,
) {
    notify_local_chat_round_completed(state, conversation_id, &result.assistant_text);
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        runtime_log_debug(format!(
            "[聊天] emit round_completed 失败: app_handle unavailable"
        ));
        return;
    };
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "activationId": activation_id.map(str::trim).filter(|value| !value.is_empty()),
        "requestId": request_id.map(str::trim).filter(|value| !value.is_empty()),
        "status": "completed",
        "assistantText": result.assistant_text,
        "archivedBeforeSend": result.archived_before_send,
        "assistantMessage": result
            .assistant_message
            .clone()
            .map(project_message_for_frontend_display_only),
    });
    ide_chat_broadcast_notification("chat.roundFinished", payload.clone());
    match app_handle.emit(CHAT_ROUND_COMPLETED_EVENT, payload) {
        Ok(_) => {}
        Err(err) => runtime_log_error(format!(
            "[聊天] emit round_completed 失败 原因={}",
            err
        )),
    }
}

/// 会话相关的活（队员 / 委托）是否都收摊了。
///
/// 一个活可能连着响好几次：队员完成一次、委托回报一次，用户看到的就是
/// 「主会话明明还在处理，提示却响个不停」。所以除了本会话的队列，还要看这两类。
/// 返回第一个命中的原因用于日志；都没在跑时返回 None。
///
/// 刻意不看后台 shell：它是可以常驻的（dev server、监听进程），按「有 shell 在跑」
/// 拦通知会让用户只要开着常驻 shell 就永远收不到提醒；它完成时会自己写回会话、
/// 触发新一轮调度，那一轮收口自然会通知。
fn conversation_related_work_busy_reason(
    state: &AppState,
    conversation_meta: &ConversationMetaView,
) -> Option<&'static str> {
    if conversation_has_busy_team_member(state, &conversation_meta.team.members) {
        return Some("team_busy");
    }
    if delegate_store_conversation_has_unfinished(&state.data_path, &conversation_meta.id)
        .unwrap_or(false)
    {
        return Some("delegate_running");
    }
    None
}

/// 队员各自是独立会话：逐个看那个会话的运行态。
///
/// 不能拿 `team_has_active_member` 代替——它判断的是「这个人在不在队里」，
/// 一个常驻但闲着的队员会让它永远为真，通知就永久静音了。
fn conversation_has_busy_team_member(
    state: &AppState,
    members: &[TeamMemberEntry],
) -> bool {
    members
        .iter()
        .filter(|member| member.status.trim() == TEAM_MEMBER_STATUS_ACTIVE)
        .map(|member| member.conversation_id.trim())
        .filter(|conversation_id| !conversation_id.is_empty())
        .any(|conversation_id| {
            read_conversation_runtime_snapshot(state, conversation_id)
                .map(|snapshot| {
                    snapshot.is_processing
                        || snapshot.has_pending_queue
                        || snapshot.runtime_state != MainSessionState::Idle
                })
                .unwrap_or(false)
        })
}

fn notify_local_chat_round_completed(
    state: &AppState,
    conversation_id: &str,
    assistant_text: &str,
) {
    let conversation_meta = match conversation_service_v2().get_conversation_meta(state, conversation_id) {
        Ok(conversation_meta) => conversation_meta,
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天] 跳过 任务=读取本地会话完成通知上下文 原因={}",
                err
            ));
            return;
        }
    };
    if !conversation_meta_is_local_normal_chat_for_notification(&conversation_meta) {
        return;
    }
    if conversation_has_focused_chat_view(state, conversation_id) {
        runtime_log_debug(format!(
            "[聊天] 跳过 任务=本地会话完成通知 原因=chat_view_focused"
        ));
        return;
    }
    // 队列里还压着待处理消息（排队的或引导的）时不提醒：这一轮不是用户真正在等的结果，
    // 引导消息更是会把当前轮次在工具切点截断后另起一轮。等这些消息都处理完、会话真正
    // 空下来那一轮再发，一次交互只提醒一次。
    if conversation_has_pending_queue_events(state, conversation_id).unwrap_or(false) {
        runtime_log_debug(format!(
            "[聊天] 跳过 任务=本地会话完成通知 原因=pending_queue"
        ));
        return;
    }
    // 队员与委托都收摊了才算「整个会话真的空下来」：否则一个活会连响几次。
    if let Some(reason) = conversation_related_work_busy_reason(state, &conversation_meta) {
        runtime_log_debug(format!(
            "[聊天] 跳过 任务=本地会话完成通知 原因={}",
            reason
        ));
        return;
    }
    let notification_settings = local_chat_notification_settings(state, conversation_id);
    if !notification_settings.enabled {
        runtime_log_debug(format!(
            "[聊天] 跳过 任务=本地会话完成通知 原因=notification_disabled"
        ));
        return;
    }
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        runtime_log_debug(format!(
            "[聊天] 跳过 任务=发送本地会话完成通知 原因=app_handle_unavailable"
        ));
        return;
    };
    let speaker_name = notification_speaker_name_for_conversation_meta(
        state,
        &conversation_meta,
        notification_settings.ui_language,
    );
    let title = notification_title_for_conversation_meta(
        &conversation_meta,
        notification_settings.ui_language,
        false,
    );
    let body = native_notification_text_excerpt(
        assistant_text,
        NATIVE_NOTIFICATION_BODY_MAX_CHARS,
    );
    let final_body = notification_body_with_speaker(
        &speaker_name,
        if body.trim().is_empty() {
            local_chat_notification_text(
                notification_settings.ui_language,
                "已完成本轮回复。",
                "已完成本輪回覆。",
                "Finished this reply.",
            )
        } else {
            body
        },
        notification_settings.ui_language,
    );
    if let Err(err) = send_native_notification(
        &app_handle,
        &title,
        &final_body,
        notification_settings.sound_enabled,
    ) {
        runtime_log_warn(format!(
            "[聊天] 失败 任务=发送本地会话完成通知 原因={}",
            err
        ));
    }
}

fn notification_body_with_speaker(speaker_name: &str, body: String, ui_language: &str) -> String {
    let body = body.trim();
    let speaker_name = speaker_name.trim();
    if speaker_name.is_empty() {
        return body.to_string();
    }
    match ui_language.trim() {
        "en-US" => format!("{speaker_name}: {body}"),
        _ => format!("{speaker_name}：{body}"),
    }
}

fn notification_title_for_conversation_meta(
    conversation_meta: &ConversationMetaView,
    ui_language: &str,
    failed: bool,
) -> String {
    let base_title = notification_conversation_display_title(conversation_meta, ui_language);
    notification_title_from_parts(&base_title, ui_language, failed)
}

fn notification_title_from_parts(
    base_title: &str,
    ui_language: &str,
    failed: bool,
) -> String {
    let mut parts = vec![base_title.trim().to_string()];
    if failed {
        parts.push(local_chat_notification_text(
            ui_language,
            "失败",
            "失敗",
            "Failed",
        ));
    }
    parts.join(" · ")
}

fn notification_conversation_display_title(
    conversation_meta: &ConversationMetaView,
    ui_language: &str,
) -> String {
    notification_conversation_display_title_from_parts(
        &conversation_meta.id,
        &conversation_meta.title,
        conversation_meta.latest_summary_title.as_deref(),
        conversation_meta.last_message_at.as_deref(),
        &conversation_meta.updated_at,
        ui_language,
    )
}

fn notification_conversation_display_title_from_parts(
    conversation_id: &str,
    title: &str,
    summary_title: Option<&str>,
    last_message_at: Option<&str>,
    updated_at: &str,
    ui_language: &str,
) -> String {
    normalized_notification_title_part(title, conversation_id)
        .or_else(|| {
            summary_title
                .and_then(|value| normalized_notification_title_part(value, conversation_id))
        })
        .or_else(|| {
            last_message_at
                .or(Some(updated_at))
                .and_then(|value| notification_fallback_title_from_time(value, ui_language))
        })
        .unwrap_or_else(|| {
            local_chat_notification_text(
                ui_language,
                "未命名会话",
                "未命名會話",
                "Untitled conversation",
            )
        })
}

fn normalized_notification_title_part(value: &str, conversation_id: &str) -> Option<String> {
    let title = value.trim();
    if title.is_empty() || title == conversation_id.trim() {
        return None;
    }
    Some(title.to_string())
}

fn notification_fallback_title_from_time(value: &str, _ui_language: &str) -> Option<String> {
    let raw_value = value.trim();
    if raw_value.is_empty() {
        return None;
    }
    let parsed = chrono::DateTime::parse_from_rfc3339(raw_value).ok()?;
    let local = parsed.with_timezone(&chrono::Local);
    Some(local.format("%m/%d %H:%M").to_string())
}

fn local_chat_notification_text(
    ui_language: &str,
    zh_cn: &str,
    zh_tw: &str,
    en_us: &str,
) -> String {
    match ui_language.trim() {
        "en-US" => en_us.to_string(),
        "zh-TW" => zh_tw.to_string(),
        _ => zh_cn.to_string(),
    }
}

fn conversation_meta_is_local_normal_chat_for_notification(
    conversation_meta: &ConversationMetaView,
) -> bool {
    matches!(
        conversation_meta.conversation_kind.trim(),
        CONVERSATION_KIND_CHAT | CONVERSATION_KIND_SIDE_CHAT
    )
        && conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_SYSTEM_NOTIFICATION
        && conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_DELEGATE
        && conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_REMOTE_IM_CONTACT
}

fn notification_speaker_name_for_conversation_meta(
    state: &AppState,
    conversation_meta: &ConversationMetaView,
    ui_language: &str,
) -> String {
    let agent_id = conversation_meta.agent_id.trim();
    if agent_id.is_empty() {
        return local_chat_notification_text(
            ui_language,
            "当前人格",
            "當前人格",
            "Current persona",
        );
    }
    match state_read_agents_cached(state) {
        Ok(agents) => agents
            .iter()
            .find(|agent| agent.id.trim() == agent_id)
            .map(|agent| agent.name.trim())
            .filter(|name| !name.is_empty())
            .unwrap_or(agent_id)
            .to_string(),
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天] 跳过 任务=读取人格名称失败后回退ID 原因={}",
                err
            ));
            agent_id.to_string()
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LocalChatNotificationSettings {
    enabled: bool,
    sound_enabled: bool,
    ui_language: &'static str,
}

impl Default for LocalChatNotificationSettings {
    fn default() -> Self {
        Self {
            enabled: default_message_notification_enabled(),
            sound_enabled: default_message_notification_sound_enabled(),
            ui_language: "zh-CN",
        }
    }
}

fn local_chat_notification_settings(
    state: &AppState,
    _conversation_id: &str,
) -> LocalChatNotificationSettings {
    match state_read_config_cached(state) {
        Ok(config) => LocalChatNotificationSettings {
            enabled: config.message_notification_enabled,
            sound_enabled: config.message_notification_sound_enabled,
            ui_language: match config.ui_language.trim() {
                "en-US" => "en-US",
                "zh-TW" => "zh-TW",
                _ => "zh-CN",
            },
        },
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天] 跳过 任务=读取通知设置失败后回退默认值 原因={}",
                err
            ));
            LocalChatNotificationSettings::default()
        }
    }
}

fn persist_conversation_round_error(state: &AppState, conversation_id: &str, error_text: &str) {
    let normalized_error = error_text.trim().to_string();
    if normalized_error.is_empty() {
        return;
    }
    match state_update_conversation_metadata_cached(state, conversation_id, |conversation| {
        conversation.last_error = Some(normalized_error.clone());
        Ok(())
    }) {
        Ok(_) => {}
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天] 持久化轮次失败信息失败 原因={}",
                err
            ));
            return;
        }
    }
    let is_side_chat = conversation_service_v2()
        .get_conversation_meta(state, conversation_id)
        .ok()
        .map(|meta| meta.conversation_kind.trim() == CONVERSATION_KIND_SIDE_CHAT)
        .unwrap_or(false);
    if is_side_chat {
        return;
    }
    if let Err(err) = emit_unarchived_conversation_overview_item_updated_from_state(state, conversation_id) {
        runtime_log_debug(format!(
            "[聊天] 轮次失败后 overview 广播失败 原因={}",
            err
        ));
    }
}

fn emit_round_failed_event(
    state: &AppState,
    conversation_id: &str,
    error_text: &str,
    activation_id: Option<&str>,
    request_id: Option<&str>,
) {
    persist_conversation_round_error(state, conversation_id, error_text);
    notify_local_chat_round_failed(state, conversation_id, error_text);
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        runtime_log_debug(format!(
            "[聊天] emit round_failed 失败: app_handle unavailable"
        ));
        return;
    };
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "activationId": activation_id.map(str::trim).filter(|value| !value.is_empty()),
        "requestId": request_id.map(str::trim).filter(|value| !value.is_empty()),
        "status": "failed",
        "error": error_text,
    });
    ide_chat_broadcast_notification("chat.roundFinished", payload.clone());
    match app_handle.emit(CHAT_ROUND_FAILED_EVENT, payload) {
        Ok(_) => {}
        Err(err) => runtime_log_error(format!(
            "[聊天] emit round_failed 失败 原因={}",
            err
        )),
    }
}

fn notify_local_chat_round_failed(state: &AppState, conversation_id: &str, error_text: &str) {
    let conversation_meta = match conversation_service_v2().get_conversation_meta(state, conversation_id) {
        Ok(conversation_meta) => conversation_meta,
        Err(err) => {
            runtime_log_warn(format!(
                "[聊天] 跳过 任务=读取本地会话失败通知上下文 原因={}",
                err
            ));
            return;
        }
    };
    if !conversation_meta_is_local_normal_chat_for_notification(&conversation_meta) {
        return;
    }
    if conversation_has_focused_chat_view(state, conversation_id) {
        runtime_log_debug(format!(
            "[聊天] 跳过 任务=本地会话失败通知 原因=chat_view_focused"
        ));
        return;
    }
    let notification_settings = local_chat_notification_settings(state, conversation_id);
    if !notification_settings.enabled {
        runtime_log_debug(format!(
            "[聊天] 跳过 任务=本地会话失败通知 原因=notification_disabled"
        ));
        return;
    }
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        runtime_log_debug(format!(
            "[聊天] 跳过 任务=发送本地会话失败通知 原因=app_handle_unavailable"
        ));
        return;
    };
    let speaker_name = notification_speaker_name_for_conversation_meta(
        state,
        &conversation_meta,
        notification_settings.ui_language,
    );
    let title = notification_title_for_conversation_meta(
        &conversation_meta,
        notification_settings.ui_language,
        true,
    );
    let body = native_notification_text_excerpt(
        error_text,
        NATIVE_NOTIFICATION_BODY_MAX_CHARS,
    );
    let final_body = notification_body_with_speaker(
        &speaker_name,
        if body.trim().is_empty() {
            local_chat_notification_text(
                notification_settings.ui_language,
                "本轮调度失败。",
                "本輪調度失敗。",
                "This round failed.",
            )
        } else {
            body
        },
        notification_settings.ui_language,
    );
    if let Err(err) = send_native_notification(
        &app_handle,
        &title,
        &final_body,
        notification_settings.sound_enabled,
    ) {
        runtime_log_warn(format!(
            "[聊天] 失败 任务=发送本地会话失败通知 原因={}",
            err
        ));
    }
}

#[cfg(test)]
mod round_notification_busy_tests {
    use super::*;

    fn test_state() -> AppState {
        crate::tests::test_chat_runtime_state()
    }

    fn member(conversation_id: &str, status: &str) -> TeamMemberEntry {
        TeamMemberEntry {
            conversation_id: conversation_id.to_string(),
            status: status.to_string(),
            ..Default::default()
        }
    }

    fn delegate_input(conversation_id: &str) -> DelegateCreateInput {
        DelegateCreateInput {
            kind: DELEGATE_TOOL_KIND_DELEGATE.to_string(),
            conversation_id: conversation_id.to_string(),
            parent_delegate_id: None,
            source_agent_id: "agent-root".to_string(),
            target_agent_id: DEFAULT_AGENT_ID.to_string(),
            title: "通知判断测试".to_string(),
            why: "验证委托运行态".to_string(),
            goal: "验证委托运行态".to_string(),
            todo: String::new(),
            prompt: "验证委托运行态".to_string(),
            notify_assistant_when_done: false,
            call_stack: Vec::new(),
        }
    }

    /// 在队但空闲的队员不算「还在处理」：通知必须照常发，不能误杀。
    #[test]
    fn busy_team_member_should_ignore_active_but_idle_member() {
        let state = test_state();
        let members = vec![member("member-conversation-idle", TEAM_MEMBER_STATUS_ACTIVE)];

        assert!(!conversation_has_busy_team_member(&state, &members));
    }

    /// 队员会话非空闲时，主会话这次不算完成，通知跳过。
    #[test]
    fn busy_team_member_should_detect_running_member_conversation() {
        let state = test_state();
        let members = vec![member(
            "member-conversation-streaming",
            TEAM_MEMBER_STATUS_ACTIVE,
        )];
        set_conversation_runtime_state(
            &state,
            "member-conversation-streaming",
            MainSessionState::AssistantStreaming,
        )
        .expect("set member conversation state");

        assert!(conversation_has_busy_team_member(&state, &members));
    }

    /// 已请离的队员哪怕会话还在跑，也不该拦住队长的通知。
    #[test]
    fn busy_team_member_should_ignore_dismissed_member() {
        let state = test_state();
        let members = vec![member(
            "member-conversation-dismissed",
            TEAM_MEMBER_STATUS_DISMISSED,
        )];
        set_conversation_runtime_state(
            &state,
            "member-conversation-dismissed",
            MainSessionState::AssistantStreaming,
        )
        .expect("set dismissed member conversation state");

        assert!(!conversation_has_busy_team_member(&state, &members));
    }

    /// 有未到终态的委托时跳过通知；委托终态后恢复。
    #[test]
    fn unfinished_delegate_should_block_notification_until_terminal() {
        let state = test_state();
        let conversation_id = "root-conversation-delegate";
        let delegate = delegate_store_create_delegate(&state.data_path, &delegate_input(conversation_id))
            .expect("create delegate record");

        assert!(
            delegate_store_conversation_has_unfinished(&state.data_path, conversation_id)
                .expect("query unfinished delegates")
        );

        delegate_store_update_status(&state.data_path, &delegate.delegate_id, DELEGATE_STATUS_COMPLETED)
            .expect("complete delegate");

        assert!(
            !delegate_store_conversation_has_unfinished(&state.data_path, conversation_id)
                .expect("query unfinished delegates after completion")
        );
    }

    /// 没有委托库文件时不能算「有委托在跑」（首次启动/老库场景）。
    #[test]
    fn unfinished_delegate_should_be_false_without_store_file() {
        let state = test_state();
        assert!(
            !delegate_store_conversation_has_unfinished(&state.data_path, "conversation-without-store")
                .expect("query without store file")
        );
    }
}
