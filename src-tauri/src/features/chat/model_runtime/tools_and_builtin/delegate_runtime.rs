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

fn delegate_build_task_prompt_block(
    title: &str,
    instruction: &str,
    background: &str,
    specific_goal: &str,
    deliverable_requirement: &str,
) -> String {
    let mut lines = vec![format!("委托任务：{}", title.trim())];
    lines.push(format!("核心指令：{}", instruction.trim()));
    if !background.trim().is_empty() {
        lines.push(format!("背景：{}", background.trim()));
    }
    if !specific_goal.trim().is_empty() {
        lines.push(format!("具体目标：{}", specific_goal.trim()));
    }
    if !deliverable_requirement.trim().is_empty() {
        lines.push(format!("交付要求：{}", deliverable_requirement.trim()));
    }
    prompt_xml_block("delegate task", lines.join("\n"))
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
        "sourceDepartmentId": delegate.source_department_id,
        "targetDepartmentId": delegate.target_department_id,
        "sourceAgentId": delegate.source_agent_id,
        "targetAgentId": delegate.target_agent_id,
        "notifyAssistantWhenDone": delegate.notify_assistant_when_done,
        "callStack": delegate.call_stack,
    })
}

fn delegate_enqueue_result_message(
    app_state: &AppState,
    root_conversation_id: &str,
    speaker_agent_id: &str,
    text: &str,
    provider_meta: Value,
    notify_assistant: bool,
) -> Result<(), String> {
    // 优先回发原始会话；若原会话已归档/消失，则回退到主会话。
    let resolved_target = conversation_service()
        .resolve_delegate_result_target_conversation(app_state, root_conversation_id)?;
    let department_id = resolved_target.department_id;
    let agent_id = resolved_target.agent_id;
    let target_conversation_id = resolved_target.target_conversation_id;

    // 构造委托结果消息
    let delegate_message = ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(speaker_agent_id.to_string()),
        parts: vec![MessagePart::Text {
            text: text.to_string(),
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(provider_meta),
        tool_call: None,
        mcp_call: None,
    };
    append_delegate_result_message_and_emit(
        app_state,
        &target_conversation_id,
        &delegate_message,
        notify_assistant,
        Some(ChatSessionInfo {
            department_id,
            agent_id,
        }),
    )
}

const AGENT_WORK_EVENT_START: &str = "easy-call:agent-work-start";
const AGENT_WORK_EVENT_STOP: &str = "easy-call:agent-work-stop";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentWorkSignalPayload {
    agent_id: String,
    delegate_id: String,
}

fn emit_agent_work_signal(
    app_state: &AppState,
    event_name: &str,
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

async fn delegate_execute_agent_run(
    app_state: &AppState,
    delegate: &DelegateEntry,
    target_api_config_id: &str,
    root_conversation_id: &str,
    delegate_conversation_id: &str,
) -> Result<SendChatResult, String> {
    let hidden_prompt = delegate_build_task_prompt_block(
        &delegate.title,
        &delegate.instruction,
        &delegate.background,
        &delegate.specific_goal,
        &delegate.deliverable_requirement,
    );
    let request = SendChatRequest {
        payload: ChatInputPayload {
            text: Some(hidden_prompt),
            display_text: None,
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: Some(delegate_build_trigger_provider_meta(delegate, root_conversation_id)),
        },
        session: Some(SessionSelector {
            api_config_id: Some(target_api_config_id.to_string()),
            department_id: Some(delegate.target_department_id.clone()),
            agent_id: delegate.target_agent_id.clone(),
            conversation_id: Some(delegate_conversation_id.to_string()),
        }),
        speaker_agent_id: Some(delegate.source_agent_id.clone()),
        trace_id: Some(format!("delegate-{}", delegate.delegate_id)),
        oldest_queue_created_at: None,
        remote_im_activation_sources: Vec::new(),
        runtime_context: Some(RuntimeContext {
            request_id: Some(format!("delegate-request-{}", delegate.delegate_id)),
            dispatch_id: Some(format!("delegate-dispatch-{}", delegate.delegate_id)),
            origin_conversation_id: Some(root_conversation_id.to_string()),
            target_conversation_id: Some(delegate_conversation_id.to_string()),
            root_conversation_id: Some(root_conversation_id.to_string()),
            executor_agent_id: Some(delegate.target_agent_id.clone()),
            executor_department_id: Some(delegate.target_department_id.clone()),
            model_config_id: Some(target_api_config_id.to_string()),
            event_source: runtime_context_trimmed(Some("delegate_trigger")),
            dispatch_reason: runtime_context_trimmed(Some("delegate_send")),
            ..RuntimeContext::default()
        }),
        trigger_only: false,
    };
    let noop_channel = tauri::ipc::Channel::new(|_| Ok(()));
    let _guard = NoApprovalDialogGuard::enter(app_state, delegate_conversation_id.to_string());
    send_chat_message_inner(request, app_state, &noop_channel).await
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
        .ok_or_else(|| format!("部门没有可用模型，departmentId={}", delegate.target_department_id))?;
    let delegate_thread_id = delegate_runtime_thread_create(
        &app_state,
        &delegate,
        &primary_api_config_id,
        parent_chat_session_key,
    )?;
    if let Err(err) = emit_agent_work_signal(
        &app_state,
        AGENT_WORK_EVENT_START,
        &delegate.target_agent_id,
        &delegate.delegate_id,
    ) {
        eprintln!(
            "[委托线程] 推送开工信号失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, target_agent_id={}, error={}",
            delegate_thread_id,
            delegate.delegate_id,
            delegate.target_agent_id,
            err
        );
    }
    let mut run_result = Err("未尝试任何候选模型".to_string());
    let mut errors = Vec::<String>::new();
    for api_config_id in target_api_config_ids {
        if let Err(err) = delegate_runtime_thread_touch(&app_state, &delegate_thread_id) {
            eprintln!(
                "[委托线程] 更新运行模型失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, api_config_id={}, error={}",
                delegate_thread_id,
                delegate.delegate_id,
                api_config_id,
                err
            );
        }
        match delegate_execute_agent_run(
            &app_state,
            &delegate,
            &api_config_id,
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
                run_result = Err(format!("部门所有候选模型均失败：{}", errors.join(" | ")));
            }
        }
    }
    match run_result {
        Ok(result) => {
            let completed_at = now_iso();
            if let Err(err) =
                delegate_runtime_thread_archive(&app_state, &delegate_thread_id, &completed_at)
            {
                eprintln!(
                    "[委托线程] 归档运行线程失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, archived_at={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_COMPLETED,
                    completed_at,
                    err
                );
            }
            if let Err(err) = delegate_store_update_status(
                &app_state.data_path,
                &delegate.delegate_id,
                DELEGATE_STATUS_COMPLETED,
            ) {
                eprintln!(
                    "[委托线程] 更新委托状态失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_COMPLETED,
                    err
                );
            }
            if let Err(err) = emit_agent_work_signal(
                &app_state,
                AGENT_WORK_EVENT_STOP,
                &delegate.target_agent_id,
                &delegate.delegate_id,
            ) {
                eprintln!(
                    "[委托线程] 推送停工信号失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, target_agent_id={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    delegate.target_agent_id,
                    err
                );
            }
            Ok(result)
        }
        Err(err) => {
            let archived_at = now_iso();
            if let Err(remove_err) =
                delegate_runtime_thread_archive(&app_state, &delegate_thread_id, &archived_at)
            {
                eprintln!(
                    "[委托线程] 归档运行线程失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, archived_at={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_FAILED,
                    archived_at,
                    remove_err
                );
            }
            if let Err(status_err) = delegate_store_update_status(
                &app_state.data_path,
                &delegate.delegate_id,
                DELEGATE_STATUS_FAILED,
            ) {
                eprintln!(
                    "[委托线程] 更新委托状态失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, status={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    DELEGATE_STATUS_FAILED,
                    status_err
                );
            }
            if let Err(stop_err) = emit_agent_work_signal(
                &app_state,
                AGENT_WORK_EVENT_STOP,
                &delegate.target_agent_id,
                &delegate.delegate_id,
            ) {
                eprintln!(
                    "[委托线程] 推送停工信号失败: function=delegate_run_thread_to_completion, delegate_thread_id={}, delegate_id={}, target_agent_id={}, error={}",
                    delegate_thread_id,
                    delegate.delegate_id,
                    delegate.target_agent_id,
                    stop_err
                );
            }
            Err(err)
        }
    }
}
