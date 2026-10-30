fn json_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value.get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(ToOwned::to_owned)
    })
}

#[derive(Debug, Clone)]
struct TerminalToolResultMessage {
    assistant_text: String,
    provider_meta: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanToolResultState {
    action: String,
    path: String,
    stop_tool_loop: bool,
}

fn current_prompt_tokens_for_preserved_gate(
    context: Option<&ToolLoopAutoCompactionContext>,
    trusted_input_tokens: Option<u64>,
) -> u64 {
    if let Some(tokens) = trusted_input_tokens.filter(|value| *value > 0) {
        return tokens;
    }
    let Some(context) = context else {
        return 0;
    };
    context
        .trusted_prompt_usage
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|usage| usage.effective_prompt_tokens))
        .unwrap_or(0)
}

const CONTEXT_EXHAUSTED_CLOSE_PROMPT: &str =
    "上下文已经耗尽。请立刻汇报，并说明工作未能彻底完成以及原因。不要再调用任何工具。";
const CONTEXT_EXHAUSTED_NO_REPORT_PROMPT: &str =
    "上下文耗尽依然未能完成任务并且没有进行阶段性汇报";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolRoundCompactionGate {
    Continue,
    RestartAfterCompaction,
    ContextExhausted,
}

/// 只决定超限后走哪条路，不触发整理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolRoundGateIntent {
    WriteThrough,
    CompactThenRestart,
    DisableToolsAndClose,
}

fn classify_tool_round_gate(should_compact: bool, has_compaction_context: bool) -> ToolRoundGateIntent {
    if !should_compact {
        ToolRoundGateIntent::WriteThrough
    } else if has_compaction_context {
        ToolRoundGateIntent::CompactThenRestart
    } else {
        ToolRoundGateIntent::DisableToolsAndClose
    }
}

/// 上下文耗尽后的收口轮：有正文就结束，没正文就退回给模型。两种都不执行工具。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextExhaustedRoundAction {
    FinishWithReport,
    RequestReport,
}

fn decide_context_exhausted_round(turn_text: &str) -> ContextExhaustedRoundAction {
    if turn_text.trim().is_empty() {
        ContextExhaustedRoundAction::RequestReport
    } else {
        ContextExhaustedRoundAction::FinishWithReport
    }
}

/// 工具整轮执行完立刻判定。本轮结果无条件先写进当前消息，压缩只决定要不要重启。
async fn apply_compaction_preserved_gate_after_tool_round(
    state: Option<&AppState>,
    context: Option<&ToolLoopAutoCompactionContext>,
    selected_api: &ApiConfig,
    resolved_api: &ResolvedApiConfig,
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    chat_session_key: &str,
    pending_tool_group_result_persists: &mut Vec<tauri::async_runtime::JoinHandle<Result<(), String>>>,
    trusted_input_tokens: Option<u64>,
    turn_text: &str,
    turn_reasoning: &str,
    assistant_tool_group_history_event: &Value,
    round_history_events: &[Value],
    completed_tool_result_events: &[Value],
) -> Result<ToolRoundCompactionGate, String> {
    if completed_tool_result_events.is_empty() {
        return Ok(ToolRoundCompactionGate::Continue);
    }
    // 本轮完整事件切片：assistant tool_calls、tool results、以及本轮旁路注入事件。
    let preserved = CompactionPreservedMessages::new(
        turn_text,
        turn_reasoning,
        round_history_events.to_vec(),
    );
    let current_tokens = current_prompt_tokens_for_preserved_gate(context, trusted_input_tokens);
    let group_tokens = preserved.token_usage();
    let context_window = selected_api.context_window_tokens.max(1);
    let should_compact =
        (current_tokens as f64 + group_tokens as f64) / f64::from(context_window) >= 0.82;
    runtime_log_info(format!(
        "[聊天] 工具执行完即时闸门 session={} should_compact={} current_tokens={} group_tokens={} context_window={}",
        chat_session_key,
        should_compact,
        current_tokens,
        group_tokens,
        selected_api.context_window_tokens
    ));

    // 压缩前后是两个独立会话：压缩前把结果写进当前消息，压缩后新消息再从
    // CompactionPreservedMessages 继承同一轮工具事件，两边的工具链各自完整。
    for tool_result_event in completed_tool_result_events {
        persist_completed_tool_group_result(
            state,
            context,
            selected_api,
            trusted_input_tokens,
            chat_session_key,
            assistant_tool_group_history_event.clone(),
            tool_result_event.clone(),
            ToolResultPersistKind::Completed,
        )?;
    }

    match classify_tool_round_gate(should_compact, context.is_some()) {
        ToolRoundGateIntent::WriteThrough => {
            return Ok(ToolRoundCompactionGate::Continue);
        }
        ToolRoundGateIntent::DisableToolsAndClose => {
            // 委托等没有调度上下文的路径无法整理。本轮工具结果留在内存里交给模型，不报错。
            runtime_log_info(format!(
                "[聊天] 上下文耗尽且无法整理，禁用后续工具 session={} current_tokens={} group_tokens={} context_window={}",
                chat_session_key,
                current_tokens,
                group_tokens,
                selected_api.context_window_tokens
            ));
            return Ok(ToolRoundCompactionGate::ContextExhausted);
        }
        ToolRoundGateIntent::CompactThenRestart => {}
    }

    // 超限：本轮结果已经写进当前消息，再把这组工具事件交给压缩后的新调度继承。
    let state = state.ok_or_else(|| "缺少应用状态，无法整理上下文。".to_string())?;
    let context = context.ok_or_else(|| "缺少当前调度上下文，无法整理上下文。".to_string())?;
    if context.remote_im_reply_delegate_id.is_some() {
        return Err("远程应答委托冻结快照期间不允许自动压缩重启。".to_string());
    }
    if let Ok(mut guard) = context.compaction_preserved_messages.lock() {
        *guard = Some(preserved);
    }
    let reason = "preserve_tool_group_before_persist";
    await_pending_tool_group_result_persists(
        pending_tool_group_result_persists,
        chat_session_key,
        reason,
    )
    .await?;
    let checkpoint = persist_tool_loop_compaction_checkpoint(
        state,
        context,
        on_delta,
        &[],
        "",
        "",
        chat_session_key,
        reason,
    )?;
    let archive_res = run_context_compaction_pipeline(
        state,
        selected_api,
        resolved_api,
        &checkpoint.refreshed_source,
        &context.agent.id,
        reason,
        "COMPACTION-BEFORE-TOOL-CONTINUE",
        &checkpoint.boundary_messages,
        false,
    )
    .await;
    match archive_res {
        Ok(result) => {
            if let Some(warning) = result
                .warning
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                runtime_log_warn(format!(
                    "[聊天] 工具写入前上下文整理 完成 conversation_id={} warning={}",
                    context.conversation_id, warning
                ));
            } else {
                runtime_log_info(format!(
                    "[聊天] 工具写入前上下文整理 完成 conversation_id={} reason={}",
                    context.conversation_id, reason
                ));
            }
            Ok(ToolRoundCompactionGate::RestartAfterCompaction)
        }
        Err(err) => Err(format!("自动整理失败：{err}")),
    }
}

fn plan_tool_result_state(
    tool_name: &str,
    tool_args: &str,
    tool_result: &ProviderToolResult,
) -> Option<PlanToolResultState> {
    if tool_name != "plan" || tool_result.is_error {
        return None;
    }

    let args_value = serde_json::from_str::<Value>(tool_args).ok();
    let result_control = match &tool_result.metadata.control {
        ProviderToolControl::Plan { action, path, stop } => Some((action, path, *stop)),
        _ => None,
    };
    let action = args_value
        .as_ref()
        .and_then(|value| json_string_field(value, &["action"]))
        .or_else(|| result_control.map(|value| value.0.clone()))?;
    let normalized_action = action.to_ascii_lowercase();
    let path = args_value
        .as_ref()
        .and_then(|value| json_string_field(value, &["path"]))
        .or_else(|| result_control.map(|value| value.1.clone()))?;
    let stop_tool_loop = result_control
        .map(|value| value.2)
        .unwrap_or(normalized_action == "present");

    Some(PlanToolResultState {
        action,
        path,
        stop_tool_loop,
    })
}

fn terminal_plan_present_result(
    tool_name: &str,
    tool_args: &str,
    tool_result: &ProviderToolResult,
) -> Option<TerminalToolResultMessage> {
    let plan_state = plan_tool_result_state(tool_name, tool_args, tool_result)?;
    if !plan_state.action.eq_ignore_ascii_case("present") || !plan_state.stop_tool_loop {
        return None;
    }
    Some(TerminalToolResultMessage {
        assistant_text: String::new(),
        provider_meta: Some(serde_json::json!({
            "messageKind": "plan_present",
            "planCard": {
                "action": plan_state.action,
                "path": plan_state.path,
            },
            "message_meta": {
                "kind": "plan_present",
            }
        })),
    })
}

fn sync_completed_tool_history_cache(
    state: Option<&AppState>,
    chat_session_key: &str,
    events: &[Value],
) {
    let Some(state) = state else {
        return;
    };
    if let Err(err) = replace_inflight_completed_tool_history(state, chat_session_key, events) {
        runtime_log_error(format!(
            "[聊天] 同步已完成工具历史缓存失败 (session={}): {}",
            chat_session_key, err
        ));
        return;
    }
    if let Ok(Some(thread)) = delegate_runtime_thread_list(state).map(|threads| {
        threads
            .into_iter()
            .find(|thread| delegate_thread_chat_key(thread) == chat_session_key)
    }) {
        let _ = emit_conversation_delegate_status_updated(
            state,
            &thread.root_conversation_id,
            &thread.delegate_id,
            DELEGATE_STATUS_RUNNING,
        );
    }
}

/// 执行前预写占位结果。历史里先出现这次调用与「尚未返回」，进程中断后仍能看出调过什么；
/// 工具完成后追加真实结果，消费端取最新一条，占位本身保留不动。
fn persist_pending_tool_group_result(
    state: Option<&AppState>,
    context: Option<&ToolLoopAutoCompactionContext>,
    selected_api: &ApiConfig,
    chat_session_key: &str,
    assistant_tool_group_history_event: &Value,
    tool_call_id: &str,
    tool_name: &str,
) -> Result<(), String> {
    let tool_result_event = tool_result_history_event(
        tool_call_id,
        format!("工具 {tool_name} 已发起调用，尚未返回结果。"),
        &ProviderToolMetadata::default(),
    );
    persist_completed_tool_group_result(
        state,
        context,
        selected_api,
        None,
        chat_session_key,
        assistant_tool_group_history_event.clone(),
        tool_result_event,
        ToolResultPersistKind::Pending,
    )
}

/// 用户打断工具执行时补一条真实结果。占位保留，消费端取这条最新的。
fn persist_user_aborted_tool_group_result(
    state: Option<&AppState>,
    context: Option<&ToolLoopAutoCompactionContext>,
    selected_api: &ApiConfig,
    chat_session_key: &str,
    assistant_tool_group_history_event: &Value,
    tool_call_id: &str,
) -> Result<(), String> {
    let tool_result_event = tool_result_history_event(
        tool_call_id,
        "用户打断了工具执行。".to_string(),
        &ProviderToolMetadata::default(),
    );
    persist_completed_tool_group_result(
        state,
        context,
        selected_api,
        None,
        chat_session_key,
        assistant_tool_group_history_event.clone(),
        tool_result_event,
        ToolResultPersistKind::Aborted,
    )
}

/// 这次落盘写的是什么结果。预写占位和打断结果都只落历史，不触发对外发送。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolResultPersistKind {
    Pending,
    Completed,
    Aborted,
}

fn persist_completed_tool_group_result(
    state: Option<&AppState>,
    context: Option<&ToolLoopAutoCompactionContext>,
    selected_api: &ApiConfig,
    trusted_input_tokens: Option<u64>,
    chat_session_key: &str,
    assistant_tool_call_event: Value,
    tool_result_event: Value,
    kind: ToolResultPersistKind,
) -> Result<(), String> {
    let Some(state) = state else {
        // 正式调度的唯一入口始终持有 AppState，这里缺失属于异常；
        // 但拿不到状态也无从判断该不该落库，维持不写、不阻断。
        runtime_log_error(format!(
            "[聊天] 跳过工具结果写历史，任务=append_tool_group_result，reason=state_missing，session={}",
            chat_session_key
        ));
        return Ok(());
    };
    let Some(context) = context else {
        // 运行时会话与远程 IM 动态边界不落库工具历史：这类会话本就没有持久化载体，
        // 预写与结果写入一起跳过，属于既有设计，不是写入失败。
        runtime_log_info(format!(
            "[聊天] 跳过工具结果写历史，任务=append_tool_group_result，reason=context_missing，session={}",
            chat_session_key
        ));
        return Ok(());
    };
    let tool_name = assistant_tool_call_event
        .get("tool_calls")
        .and_then(Value::as_array)
        .and_then(|calls| calls.first())
        .and_then(|call| call.get("function"))
        .and_then(|func| func.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown")
        .to_string();
    let backup_record_id = tool_result_event
        .get("metadata")
        .and_then(|metadata| metadata.get("backup_record_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .map(str::to_string)
        .filter(|value| !value.is_empty());
    let has_backup_record_id = backup_record_id.is_some();
    runtime_log_info(format!(
        "[聊天] 准备写入工具结果历史，任务=append_tool_group_result，session={}，conversation_id={}，tool_name={}，has_backup_record_id={}，backup_record_id={}",
        chat_session_key,
        context.conversation_id,
        tool_name,
        has_backup_record_id,
        backup_record_id.as_deref().unwrap_or("(none)")
    ));
    // 只认当前调度上下文中的 assistant_message_id，禁止回读会话级缓存或补生成。
    // 注意：这里刻意不改 provider_meta——token 用量已在 core_send_inner 的 final text
    // 落盘时统一写入，工具追加时改 meta 会让 D14 组内追加全部回退整块重写。
    let assistant_message_id = context
        .assistant_message_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            format!(
                "当前调度缺少 assistant_message_id，无法写入工具结果，conversation_id={}",
                context.conversation_id
            )
        })?;
    let append_result = conversation_service_v2()
        .append_tool_event_to_assistant_message(
            state,
            &AssistantMessageToolAppendInput {
                conversation_id: context.conversation_id.clone(),
                assistant_message_id: assistant_message_id.clone(),
                assistant_tool_event: assistant_tool_call_event.clone(),
                tool_result_event: tool_result_event.clone(),
            },
        )
        .map(|result| (result.assistant_message_id, result.tool_event_count));

    match append_result {
        Ok(result) => {
            if context.remote_im_reply_delegate_id.is_none() {
                set_stream_cache_persisted_assistant_message_id(
                    state,
                    &context.conversation_id,
                    &result.0,
                );
            }
            // 工具结果落盘时把上下文用量写进流式缓存：前端随后续 delta 的
            // stream_cache 拿到最新占用率，切屏恢复也直接来自缓存，无需旁路广播。
            if let Some(tokens) = trusted_input_tokens.filter(|value| *value > 0) {
                set_stream_cache_context_usage(
                    state,
                    &context.conversation_id,
                    tokens,
                    selected_api.context_window_tokens,
                );
            }
            if kind == ToolResultPersistKind::Completed {
                maybe_spawn_remote_im_tool_persist_auto_send(
                    state,
                    context,
                    &result.0,
                    &assistant_tool_call_event,
                    &tool_result_event,
                );
            }
            runtime_log_info(format!(
                "[聊天] 完成，任务=append_tool_group_result，session={}，conversation_id={}，assistant_message_id={}，tool_event_count={}，tool_name={}，has_backup_record_id={}",
                chat_session_key,
                context.conversation_id,
                result.0,
                result.1,
                tool_name,
                has_backup_record_id
            ));
            Ok(())
        }
        Err(err) => {
            runtime_log_error(format!(
                "[聊天] 失败，任务=append_tool_group_result，session={}，conversation_id={}，error={}",
                chat_session_key, context.conversation_id, err
            ));
            Err(format!("{TOOL_HISTORY_PERSIST_FAILED_PREFIX}：{err}"))
        }
    }
}

async fn await_pending_tool_group_result_persists(
    pending_tool_group_result_persists: &mut Vec<tauri::async_runtime::JoinHandle<Result<(), String>>>,
    chat_session_key: &str,
    reason: &str,
) -> Result<(), String> {
    if pending_tool_group_result_persists.is_empty() {
        return Ok(());
    }
    runtime_log_info(format!(
        "[聊天] 等待工具结果落盘，任务=drain_tool_group_result_persist，session={}，reason={}，pending_count={}",
        chat_session_key,
        reason,
        pending_tool_group_result_persists.len()
    ));
    for handle in pending_tool_group_result_persists.drain(..) {
        handle
            .await
            .map_err(|err| format!("等待工具结果落盘任务失败：{err}"))?
            .map_err(|err| format!("工具结果落盘失败：{err}"))?;
    }
    Ok(())
}
