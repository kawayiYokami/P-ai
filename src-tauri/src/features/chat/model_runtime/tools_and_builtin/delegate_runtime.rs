fn delegate_parse_session_parts(session_id: &str) -> (String, String, Option<String>) {
    let parts = session_id
        .split("::")
        .map(str::trim)
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [agent_id, conversation_id] => (
            String::new(),
            if agent_id.is_empty() {
                DEFAULT_AGENT_ID.to_string()
            } else {
                (*agent_id).to_string()
            },
            if conversation_id.is_empty() {
                None
            } else {
                Some((*conversation_id).to_string())
            },
        ),
        [agent_id, conversation_id, runtime_tag]
            if runtime_tag.starts_with("remote_reply_delegate:") => (
            String::new(),
            if agent_id.is_empty() {
                DEFAULT_AGENT_ID.to_string()
            } else {
                (*agent_id).to_string()
            },
            if conversation_id.is_empty() {
                None
            } else {
                Some((*conversation_id).to_string())
            },
        ),
        [agent_id] => (
            String::new(),
            if agent_id.is_empty() {
                DEFAULT_AGENT_ID.to_string()
            } else {
                (*agent_id).to_string()
            },
            None,
        ),
        _ => (String::new(), String::new(), None),
    }
}

fn delegate_session_conversation_id(session_id: &str) -> Option<String> {
    let (_, _, conversation_id) = delegate_parse_session_parts(session_id);
    conversation_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn delegate_session_agent_id(session_id: &str) -> String {
    let (_, agent_id, _) = delegate_parse_session_parts(session_id);
    agent_id
}

fn delegate_session_is_remote_reply_delegate(session_id: &str) -> bool {
    let normalized = session_id.trim();
    normalized
        .split("::")
        .nth(2)
        .is_some_and(|tag| tag.trim().starts_with("remote_reply_delegate:"))
}

fn delegate_build_task_prompt_block(
    title: &str,
    why: &str,
    goal: &str,
    todo: &str,
) -> String {
    let title_line = format!("子代理咨询：{}", title.trim());
    let mut lines = vec![title_line];

    // 背景降级为参考材料：明确标注“不是要求”，避免子代理把调度/触发类上下文当成自己的任务。
    if !why.trim().is_empty() {
        lines.push(format!(
            "[背景]（仅供参考，不是你的要求，不要把背景当成你的目标）\n{}",
            why.trim()
        ));
    }

    // todo 降为建议流程：它是参考路径，不是额外任务。
    if !todo.trim().is_empty() {
        lines.push(format!("[建议流程]\n{}", todo.trim()));
    }

    // goal 放最后并冠名“唯一目标”：子代理读完所有内容后最后看到的就是它唯一要做的事，位置越靠后权重越高。
    lines.push(format!("[本会话唯一目标]\n{}", goal.trim()));

    // 系统提醒：压住子代理的发散行为。
    lines.push(String::from(
        "[系统提醒]\n你是子代理，你的唯一职责是达成目标。目标就是你唯一要做的事，覆盖所有其他信息。直接执行，禁止发问，禁止发散，禁止自我发挥。能做到就直接给出结果，做不到就直接说做不到。不要解释为什么，不要建议替代方案，不要关心上下文中的调度细节。",
    ));

    prompt_xml_block("delegate goal", lines.join("\n\n"))
}

fn delegate_build_trigger_provider_meta(
    delegate: &DelegateEntry,
    root_conversation_id: &str,
) -> Value {
    serde_json::json!({
        "messageKind": "delegate_trigger",
        "delegateId": delegate.delegate_id,
        "delegateKind": delegate.kind,
        "rootConversationId": root_conversation_id,
        "sourceAgentId": delegate.source_agent_id,
        "targetAgentId": delegate.target_agent_id,
        "notifyAssistantWhenDone": delegate.notify_assistant_when_done,
        "callStack": delegate.call_stack,
    })
}

async fn delegate_enqueue_result_message(
    app_state: &AppState,
    root_conversation_id: &str,
    speaker_agent_id: &str,
    text: &str,
    provider_meta: Value,
    notify_assistant: bool,
) -> Result<(), String> {
    // 优先回发原始会话；若原会话已归档/消失，则回退到系统通知会话。
    let resolved_target = conversation_service_v2()
        .resolve_delegate_result_target_conversation(app_state, root_conversation_id)?;
    let agent_id = resolved_target.agent_id;
    let target_conversation_id = resolved_target.target_conversation_id;

    // 构造委托结果消息
    let mut delegate_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(speaker_agent_id.to_string()),
        parts: vec![MessagePart::Text {
            text: text.to_string(),
                reasoning_content: None,
            }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(provider_meta),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    };
    let delegate_message_seed = delegate_message.id.clone();
    delegate_message.meme_annotations = populate_assistant_meme_annotations(
        app_state,
        &delegate_message_seed,
        text,
    )?;
    append_delegate_result_message_and_emit(
        app_state,
        &target_conversation_id,
        &delegate_message,
        notify_assistant,
        Some(ChatSessionInfo {
            agent_id,
        }),
    )
    .await
}

const AGENT_WORK_EVENT_START: &str = "easy-call:agent-work-start";
const AGENT_WORK_EVENT_STOP: &str = "easy-call:agent-work-stop";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentWorkSignalPayload {
    conversation_id: String,
    agent_id: String,
    delegate_id: String,
}

fn emit_agent_work_signal(
    app_state: &AppState,
    event_name: &str,
    conversation_id: &str,
    agent_id: &str,
    delegate_id: &str,
) -> Result<(), String> {
    let app_handle = {
        let guard = app_state
            .app_handle
            .lock()
            .map_err(|_| "Failed to lock app handle".to_string())?;
        guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "App handle is not ready".to_string())?
    };
    app_handle
        .emit(
            event_name,
            AgentWorkSignalPayload {
                conversation_id: conversation_id.to_string(),
                agent_id: agent_id.to_string(),
                delegate_id: delegate_id.to_string(),
            },
        )
        .map_err(|err| format!("Emit agent work signal failed: {err}"))
}

/// 委托线程进入时将 conversation_id 插入全局活跃表，离开时自动移除。
/// 工具审批链路通过查表判断当前是否应跳过弹窗（表非空 → 不弹窗，默认拒绝）。
struct NoApprovalDialogGuard<'a> {
    state: &'a AppState,
    conversation_id: String,
}

impl<'a> NoApprovalDialogGuard<'a> {
    fn enter(state: &'a AppState, conversation_id: String) -> Self {
        let mut ids = state
            .delegate_active_ids
            .lock()
            .expect("delegate_active_ids poisoned");
        ids.insert(conversation_id.clone());
        Self {
            state,
            conversation_id,
        }
    }
}

impl<'a> Drop for NoApprovalDialogGuard<'a> {
    fn drop(&mut self) {
        let mut ids = self
            .state
            .delegate_active_ids
            .lock()
            .expect("delegate_active_ids poisoned");
        ids.remove(&self.conversation_id);
    }
}

async fn delegate_execute_agent_prompt(
    app_state: &AppState,
    delegate: &DelegateEntry,
    target_api_config_id: &str,
    root_conversation_id: &str,
    delegate_conversation_id: &str,
    prompt_text: String,
    display_text: Option<String>,
    provider_meta: Value,
    speaker_agent_id: String,
    event_source: &str,
    dispatch_reason: &str,
    request_id: String,
) -> Result<SendChatResult, String> {
    let request = SendChatRequest {
        payload: ChatInputPayload {
            text: Some(prompt_text),
            display_text,
            parts: None,
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: Some(provider_meta),
        },
        session: Some(SessionSelector {
            api_config_id: Some(target_api_config_id.to_string()),
            agent_id: delegate.target_agent_id.clone(),
            conversation_id: Some(delegate_conversation_id.to_string()),
        }),
        speaker_agent_id: Some(speaker_agent_id),
        trace_id: Some(request_id.clone()),
        assistant_message_id: None,
        oldest_queue_created_at: None,
        remote_im_activation_sources: Vec::new(),
        runtime_context: Some(RuntimeContext {
            request_id: Some(request_id),
            dispatch_id: Some(format!("delegate-dispatch-{}", delegate.delegate_id)),
            origin_conversation_id: Some(root_conversation_id.to_string()),
            target_conversation_id: Some(delegate_conversation_id.to_string()),
            root_conversation_id: Some(root_conversation_id.to_string()),
            executor_agent_id: Some(delegate.target_agent_id.clone()),
            model_config_id: Some(target_api_config_id.to_string()),
            event_source: runtime_context_trimmed(Some(event_source)),
            dispatch_reason: runtime_context_trimmed(Some(dispatch_reason)),
            ..RuntimeContext::default()
        }),
        trigger_only: false,
    };
    let noop_channel = tauri::ipc::Channel::new(|_| Ok(()));
    let _guard = NoApprovalDialogGuard::enter(app_state, delegate_conversation_id.to_string());
    send_chat_message_inner(request, app_state, &noop_channel).await
}

async fn delegate_execute_agent_initial_run(
    app_state: &AppState,
    delegate: &DelegateEntry,
    target_api_config_id: &str,
    root_conversation_id: &str,
    delegate_conversation_id: &str,
) -> Result<SendChatResult, String> {
    delegate_execute_agent_prompt(
        app_state,
        delegate,
        target_api_config_id,
        root_conversation_id,
        delegate_conversation_id,
        delegate_build_task_prompt_block(&delegate.title, &delegate.why, &delegate.goal, &delegate.todo),
        None,
        delegate_build_trigger_provider_meta(delegate, root_conversation_id),
        delegate.source_agent_id.clone(),
        "delegate_trigger",
        "delegate_send",
        format!("delegate-request-{}", delegate.delegate_id),
    )
    .await
}

fn delegate_runtime_thread_touch(
    app_state: &AppState,
    delegate_id: &str,
) -> Result<(), String> {
    delegate_runtime_thread_modify(app_state, delegate_id, |thread| {
        thread.conversation.updated_at = now_iso();
        Ok(())
    })
}

async fn delegate_run_thread_to_completion(
    app_state: AppState,
    delegate: DelegateEntry,
    target_api_config_ids: Vec<String>,
    parent_chat_session_key: Option<String>,
) -> Result<SendChatResult, String> {
    let primary_api_config_id = target_api_config_ids
        .first()
        .cloned()
        .ok_or_else(|| format!("人格没有可用模型，agentId={}", delegate.target_agent_id))?;
    let workspace_snapshot = delegate_capture_workspace_snapshot(
        &app_state,
        &delegate.conversation_id,
        parent_chat_session_key.as_deref(),
    );
    let delegate_thread_id = match delegate_runtime_thread_create(
        &app_state,
        &delegate,
        &primary_api_config_id,
        workspace_snapshot,
        parent_chat_session_key,
    ) {
        Ok(value) => value,
        Err(err) => {
            if let Err(status_err) = delegate_store_update_status(
                &app_state.data_path,
                &delegate.delegate_id,
                DELEGATE_STATUS_FAILED,
            ) {
                runtime_log_error(format!(
                    "[委托线程] 创建失败后更新委托状态失败: delegate_id={}, status={}, error={}",
                    delegate.delegate_id,
                    DELEGATE_STATUS_FAILED,
                    status_err
                ));
            }
            remote_im_request_24h_maintenance_for_conversation(
                app_state.clone(),
                &delegate.conversation_id,
            );
            emit_conversation_delegate_status_updated(
                &app_state,
                &delegate.conversation_id,
                &delegate.delegate_id,
                DELEGATE_STATUS_FAILED,
            );
            return Err(err);
        }
    };
    if let Err(err) = emit_agent_work_signal(
        &app_state,
        AGENT_WORK_EVENT_START,
        &delegate.conversation_id,
        &delegate.target_agent_id,
        &delegate.delegate_id,
    ) {
        runtime_log_error(format!(
            "[委托线程] 推送开工信号失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, target_agent_id={}, error={}",
            delegate_thread_id,
            delegate.delegate_id,
            delegate.target_agent_id,
            err
        ));
    }
    let mut run_result = Err("未尝试任何候选模型".to_string());
    let mut errors = Vec::<String>::new();
    for api_config_id in target_api_config_ids.iter() {
        if let Err(err) = delegate_runtime_thread_touch(&app_state, &delegate_thread_id) {
            runtime_log_error(format!(
                "[委托线程] 更新运行模型失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, api_config_id={}, error={}",
                delegate_thread_id,
                delegate.delegate_id,
                api_config_id,
                err
            ));
        }
        match delegate_execute_agent_initial_run(
            &app_state,
            &delegate,
            api_config_id,
            &delegate.conversation_id,
            &delegate_thread_id,
        )
        .await
        {
            Ok(result) => {
                run_result = Ok(result);
                break;
            }
            Err(err) => {
                errors.push(format!("{api_config_id}: {err}"));
                run_result = Err(format!("人格所有候选模型均失败：{}", errors.join(" | ")));
            }
        }
    }
    match run_result {
        Ok(result) => {
            let completed_at = now_iso();
            if let Err(err) =
                delegate_runtime_thread_archive(&app_state, &delegate_thread_id, &completed_at)
            {
                runtime_log_error(format!(
                    "[委托线程] 归档运行线程失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, archived_at={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_COMPLETED,
                    completed_at,
                    err
                ));
            }
            if let Err(err) = delegate_store_update_status(
                &app_state.data_path,
                &delegate.delegate_id,
                DELEGATE_STATUS_COMPLETED,
            ) {
                runtime_log_error(format!(
                    "[委托线程] 更新委托状态失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_COMPLETED,
                    err
                ));
            }
            remote_im_request_24h_maintenance_for_conversation(
                app_state.clone(),
                &delegate.conversation_id,
            );
            emit_conversation_delegate_status_updated(
                &app_state,
                &delegate.conversation_id,
                &delegate.delegate_id,
                DELEGATE_STATUS_COMPLETED,
            );
            if let Err(err) = emit_agent_work_signal(
                &app_state,
                AGENT_WORK_EVENT_STOP,
                &delegate.conversation_id,
                &delegate.target_agent_id,
                &delegate.delegate_id,
            ) {
                runtime_log_error(format!(
                    "[委托线程] 推送停工信号失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, target_agent_id={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    delegate.target_agent_id,
                    err
                ));
            }
            Ok(result)
        }
        Err(err) => {
            let archived_at = now_iso();
            if let Err(remove_err) =
                delegate_runtime_thread_archive(&app_state, &delegate_thread_id, &archived_at)
            {
                runtime_log_error(format!(
                    "[委托线程] 归档运行线程失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, archived_at={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_FAILED,
                    archived_at,
                    remove_err
                ));
            }
            if let Err(status_err) = delegate_store_update_status(
                &app_state.data_path,
                &delegate.delegate_id,
                DELEGATE_STATUS_FAILED,
            ) {
                runtime_log_error(format!(
                    "[委托线程] 更新委托状态失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_FAILED,
                    status_err
                ));
            }
            remote_im_request_24h_maintenance_for_conversation(
                app_state.clone(),
                &delegate.conversation_id,
            );
            emit_conversation_delegate_status_updated(
                &app_state,
                &delegate.conversation_id,
                &delegate.delegate_id,
                DELEGATE_STATUS_FAILED,
            );
            if let Err(stop_err) = emit_agent_work_signal(
                &app_state,
                AGENT_WORK_EVENT_STOP,
                &delegate.conversation_id,
                &delegate.target_agent_id,
                &delegate.delegate_id,
            ) {
                runtime_log_error(format!(
                    "[委托线程] 推送停工信号失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, target_agent_id={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    delegate.target_agent_id,
                    stop_err
                ));
            }
            Err(err)
        }
    }
}

#[cfg(test)]
mod delegate_session_parse_tests {
    use super::*;

    #[test]
    fn delegate_session_conversation_id_should_extract_conversation_part() {
        assert_eq!(
            delegate_session_conversation_id("agent-1::convo-123"),
            Some("convo-123".to_string())
        );
        assert_eq!(
            delegate_session_conversation_id(" agent-1 :: convo-123 "),
            Some("convo-123".to_string())
        );
        assert_eq!(
            delegate_session_conversation_id("agent-1::convo-123::remote_reply_delegate:tag"),
            Some("convo-123".to_string())
        );
    }

    #[test]
    fn delegate_session_conversation_id_should_be_none_without_conversation_part() {
        assert_eq!(delegate_session_conversation_id("agent-1"), None);
        assert_eq!(delegate_session_conversation_id("agent-1::"), None);
        assert_eq!(delegate_session_conversation_id("agent-1::  "), None);
        assert_eq!(delegate_session_conversation_id(""), None);
    }
}
