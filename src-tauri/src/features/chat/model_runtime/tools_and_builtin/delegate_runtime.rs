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
        [api_config_id, agent_id, conversation_id, ..] => (
            (*api_config_id).to_string(),
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
        _ => (String::new(), DEFAULT_AGENT_ID.to_string(), None),
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
    let (department_id, agent_id, target_conversation_id) = {
        let guard = app_state
            .state_lock
            .lock()
            .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
        let config = read_config(&app_state.config_path)?;
        let mut data = state_read_app_data_cached(app_state)?;
        let before_len = data.conversations.len();
        let before_main_id = data.main_conversation_id.clone();
        let assistant_agent_id = assistant_department_agent_id(&config)
            .ok_or_else(|| "未找到助理部门委任人".to_string())?;
        let department_id = department_for_agent_id(&config, &assistant_agent_id)
            .map(|item| item.id.clone())
            .unwrap_or_else(|| ASSISTANT_DEPARTMENT_ID.to_string());
        let target_conversation_id = if data.conversations.iter().any(|item| {
            item.id == root_conversation_id
                && item.summary.trim().is_empty()
                && !conversation_is_delegate(item)
        }) {
            root_conversation_id.to_string()
        } else {
            let main_idx = ensure_main_conversation_index(&mut data, "", &assistant_agent_id);
            let conversation_id = data
                .conversations
                .get(main_idx)
                .map(|item| item.id.clone())
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "未找到可用主会话，无法回发委托结果".to_string())?;
            eprintln!(
                "[委托线程] 原始会话不可用，委托结果回退到主会话: requested_conversation_id={}, fallback_conversation_id={}",
                root_conversation_id,
                conversation_id
            );
            conversation_id
        };
        if data.conversations.len() != before_len || data.main_conversation_id != before_main_id {
            state_write_app_data_cached(app_state, &data)?;
        }
        drop(guard);
        (department_id, assistant_agent_id, target_conversation_id)
    };

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

    // 创建事件并入队
    let event_id = Uuid::new_v4().to_string();
    let mut runtime_context = runtime_context_new("delegate_result", "delegate_publish");
    runtime_context.request_id = Some(format!("delegate-result-{}", Uuid::new_v4()));
    runtime_context.dispatch_id = Some(event_id.clone());
    runtime_context.origin_conversation_id = Some(root_conversation_id.to_string());
    runtime_context.target_conversation_id = Some(target_conversation_id.clone());
    runtime_context.root_conversation_id = Some(root_conversation_id.to_string());
    runtime_context.executor_agent_id = Some(agent_id.clone());
    runtime_context.executor_department_id = Some(department_id.clone());
    let event = ChatPendingEvent {
        id: event_id,
        conversation_id: target_conversation_id,
        created_at: now_iso(),
        source: ChatEventSource::Delegate,
        messages: vec![delegate_message],
        activate_assistant: notify_assistant,
        session_info: ChatSessionInfo {
            department_id,
            agent_id,
        },
        runtime_context: Some(runtime_context),
        sender_info: None,
    };

    let ingress_started_at = std::time::Instant::now();
    let ingress = ingress_chat_event(app_state, event)?;
    let ingress_duration_ms = ingress_started_at
        .elapsed()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64;
    let (ingress_route, ingress_mode, ingress_key_count) = match &ingress {
        ChatEventIngress::Direct(event) => (
            "direct_write",
            "direct",
            usize::from(!event.id.trim().is_empty()),
        ),
        ChatEventIngress::Queued { event_id } => (
            "queue",
            "queued",
            usize::from(!event_id.trim().is_empty()),
        ),
    };
    eprintln!(
        "[聊天] 任务=chat_ingress 操作=delegate_publish 状态=完成 路由={} 模式={} 关键计数={} 耗时毫秒={}",
        ingress_route,
        ingress_mode,
        ingress_key_count,
        ingress_duration_ms
    );

    // 异步触发处理：直写或排队由 ingress 判定，排队仅在确实滞留时通知前端。
    trigger_chat_event_after_ingress(app_state, ingress);

    Ok(())
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
        }),
        trigger_only: false,
    };
    let noop_channel = tauri::ipc::Channel::new(|_| Ok(()));
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

