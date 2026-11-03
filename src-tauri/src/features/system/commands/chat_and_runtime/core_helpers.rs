fn inflight_chat_key(
    agent_id: &str,
    conversation_id: Option<&str>,
) -> String {
    let agent_id = agent_id.trim();
    match conversation_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(conversation_id) if agent_id.is_empty() => conversation_id.to_string(),
        Some(conversation_id) => format!("{}::{}", agent_id, conversation_id),
        None => agent_id.to_string(),
    }
}

/// 停止请求要终止的那个调度的身份。
///
/// 三者都是「这一次调度」的标识，任一带上就能精确命中；全空说明前端也不知道在停谁，
/// 此时停止什么都不做——绝不按会话去猜。
#[derive(Debug, Default, Clone)]
struct StopDispatchIdentity {
    request_id: String,
    activation_id: String,
    assistant_message_id: String,
}

impl StopDispatchIdentity {
    /// 条目是否就是这次停止要终止的调度。
    ///
    /// 主流程里 `activation_id` 与 `request_id` 同值（调度器宣布轮次时直接用请求号做激活号），
    /// 所以命中比对只看请求号与助理消息 id：请求号可以来自 `request_id`，也可以来自前端
    /// 手上那个激活号，两者对同一个调度是同一个值。
    fn matches_running(&self, request_id: &str, assistant_message_id: &str) -> bool {
        let running_request = request_id.trim();
        let wanted_request = self.request_id.trim();
        let wanted_activation = self.activation_id.trim();
        let wanted_message = self.assistant_message_id.trim();
        let is_request = !wanted_request.is_empty() && wanted_request == running_request;
        let is_activation = !wanted_activation.is_empty() && wanted_activation == running_request;
        let is_message = !wanted_message.is_empty() && wanted_message == assistant_message_id.trim();
        is_request || is_activation || is_message
    }
}

/// 登记一个会话轮次的打断句柄。
///
/// 若同一请求号上留着「提交期取消」的标记，消费它并返回 `true`，表示这一轮不要开始生成。
/// 标记按请求号命中：别的调度、别的轮次的标记不会影响这一次。
fn register_inflight_chat_abort_handle(
    state: &AppState,
    chat_key: &str,
    handle: AbortHandle,
    request_id: &str,
    assistant_message_id: &str,
) -> Result<bool, String> {
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    if let Some(InflightChatAbortEntry::CancelRequested {
        request_id: cancelled_request_id,
    }) = inflight.get(chat_key)
    {
        let cancelled = !request_id.trim().is_empty()
            && cancelled_request_id.trim() == request_id.trim();
        inflight.remove(chat_key);
        if cancelled {
            return Ok(true);
        }
    }
    if let Some(previous) = inflight.insert(
        chat_key.to_string(),
        InflightChatAbortEntry::Running {
            handle,
            request_id: request_id.trim().to_string(),
            assistant_message_id: assistant_message_id.trim().to_string(),
        },
    ) {
        if let InflightChatAbortEntry::Running { handle, .. } = previous {
            handle.abort();
        }
    }
    Ok(false)
}

/// 对指定会话发起停止：只终止身份对得上的那个调度。
///
/// - 该 key 上正跑着的调度与身份匹配 → 当场 abort 并移除，返回 `true`；
/// - 该 key 上是别的调度 → 不动它，返回 `false`（含「另一个调度正在跑」的情况）；
/// - 该 key 上什么都没有、但这次停止带了请求号 → 记下「取消这一次提交」，
///   由这一次提交自己登记句柄时消费，返回 `true`；
/// - 连请求号都没有 → 空操作，什么都不写。
fn request_stop_inflight_chat(
    state: &AppState,
    chat_key: &str,
    identity: &StopDispatchIdentity,
) -> Result<bool, String> {
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    match inflight.remove(chat_key) {
        Some(InflightChatAbortEntry::Running {
            handle,
            request_id,
            assistant_message_id,
        }) => {
            if identity.matches_running(&request_id, &assistant_message_id) {
                handle.abort();
                Ok(true)
            } else {
                // 不是这一次的调度：原样放回，绝不误伤。
                inflight.insert(
                    chat_key.to_string(),
                    InflightChatAbortEntry::Running {
                        handle,
                        request_id,
                        assistant_message_id,
                    },
                );
                Ok(false)
            }
        }
        Some(InflightChatAbortEntry::CancelRequested {
            request_id: pending,
        }) => {
            let same_request = !identity.request_id.trim().is_empty()
                && pending.trim() == identity.request_id.trim();
            if !same_request {
                inflight.insert(
                    chat_key.to_string(),
                    InflightChatAbortEntry::CancelRequested { request_id: pending },
                );
            }
            Ok(same_request)
        }
        None => {
            let request_id = identity.request_id.trim();
            if request_id.is_empty() {
                // 没有可指认的对象，也没有请求号：停止就是空操作。
                return Ok(false);
            }
            inflight.insert(
                chat_key.to_string(),
                InflightChatAbortEntry::CancelRequested {
                    request_id: request_id.to_string(),
                },
            );
            Ok(true)
        }
    }
}

/// 清除指定会话的提交期取消标记；已登记的运行句柄不受影响。
fn clear_inflight_chat_cancel_request(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    if matches!(
        inflight.get(chat_key),
        Some(InflightChatAbortEntry::CancelRequested { .. })
    ) {
        inflight.remove(chat_key);
    }
    Ok(())
}

/// 清除某个会话上所有残留的提交期取消标记（按会话 id 匹配 key 后缀）。
///
/// 会话这一批已经收尾：还挂着的取消标记说明它要取消的那次提交没有等到自己的轮次。
/// 必须清掉，否则它会一直躺在共享表里，成为没人认领的残留。
fn clear_conversation_cancel_requests(
    state: &AppState,
    conversation_id: &str,
) -> Result<usize, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(0);
    }
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    let suffix = format!("::{conversation_id}");
    let mut stale_keys: Vec<String> = Vec::new();
    for (key, entry) in inflight.iter() {
        if !matches!(entry, InflightChatAbortEntry::CancelRequested { .. }) {
            continue;
        }
        if key.as_str() == conversation_id || key.ends_with(&suffix) {
            stale_keys.push(key.clone());
        }
    }
    let cleared = stale_keys.len();
    for key in stale_keys {
        inflight.remove(&key);
    }
    Ok(cleared)
}

/// 移除并中止指定调度键上正在运行的轮次句柄；命中返回 `true`。
/// 该键上若只有提交期取消标记，则原样保留（它针对的是尚未登记句柄的那次提交），返回 `false`。
/// 这是「按会话键」的粗粒度打断，供委托/子调度等内部收尾使用；用户停止走 [`request_stop_inflight_chat`]。
fn abort_running_inflight_chat(state: &AppState, chat_key: &str) -> Result<bool, String> {
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    if matches!(
        inflight.get(chat_key),
        Some(InflightChatAbortEntry::Running { .. })
    ) {
        if let Some(InflightChatAbortEntry::Running { handle, .. }) = inflight.remove(chat_key) {
            handle.abort();
        }
        return Ok(true);
    }
    Ok(false)
}

fn register_inflight_tool_abort_handle(
    state: &AppState,
    chat_key: &str,
    handle: AbortHandle,
) -> Result<(), String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    if let Some(previous) = inflight.insert(chat_key.to_string(), handle) {
        previous.abort();
    }
    Ok(())
}

fn reset_inflight_completed_tool_history(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    inflight.insert(chat_key.to_string(), Vec::new());
    Ok(())
}

fn replace_inflight_completed_tool_history(
    state: &AppState,
    chat_key: &str,
    events: &[Value],
) -> Result<(), String> {
    let mut inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    inflight.insert(chat_key.to_string(), events.to_vec());
    Ok(())
}

fn inflight_completed_tool_history(
    state: &AppState,
    chat_key: &str,
) -> Result<Vec<Value>, String> {
    let inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    Ok(inflight.get(chat_key).cloned().unwrap_or_default())
}

fn clear_inflight_completed_tool_history(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_completed_tool_history
        .lock()
        .map_err(|_| "Failed to lock inflight completed tool history".to_string())?;
    inflight.remove(chat_key);
    Ok(())
}

fn clear_inflight_tool_abort_handle(state: &AppState, chat_key: &str) -> Result<(), String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    inflight.remove(chat_key);
    Ok(())
}

fn abort_inflight_tool_abort_handle(state: &AppState, chat_key: &str) -> Result<bool, String> {
    let mut inflight = state
        .inflight_tool_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight tool abort handles".to_string())?;
    if let Some(handle) = inflight.remove(chat_key) {
        handle.abort();
        Ok(true)
    } else {
        Ok(false)
    }
}

fn delegate_thread_chat_key(thread: &DelegateRuntimeThread) -> String {
    inflight_chat_key(
        &thread.conversation.agent_id,
        Some(&thread.conversation.id),
    )
}

fn abort_delegate_runtime_descendant_threads(
    state: &AppState,
    parent_chat_key: &str,
    children: Vec<DelegateRuntimeThread>,
) -> Result<usize, String> {
    let mut aborted_count = 0usize;
    for thread in children {
        let child_chat_key = delegate_thread_chat_key(&thread);
        let aborted_chat = abort_running_inflight_chat(state, &child_chat_key)?;
        let aborted_tool = abort_inflight_tool_abort_handle(state, &child_chat_key)?;
        if aborted_chat || aborted_tool {
            aborted_count += 1;
            runtime_log_info(format!(
                "[聊天] 已中止同步委托子会话: parent_session={}, child_session={}, delegate_id={}",
                parent_chat_key,
                child_chat_key,
                thread.delegate_id
            ));
        }
        aborted_count += abort_delegate_runtime_descendants_by_parent_session(state, &child_chat_key)?;
    }
    Ok(aborted_count)
}

fn abort_delegate_runtime_descendants_by_parent_session(
    state: &AppState,
    parent_chat_key: &str,
) -> Result<usize, String> {
    let children = delegate_runtime_thread_list(state)?
        .into_iter()
        .filter(|thread| thread.parent_chat_session_key.as_deref() == Some(parent_chat_key))
        .collect::<Vec<_>>();
    abort_delegate_runtime_descendant_threads(state, parent_chat_key, children)
}

fn abort_delegate_runtime_descendants_by_parent_context(
    state: &AppState,
    parent_chat_key: &str,
    root_conversation_id: Option<&str>,
) -> Result<usize, String> {
    let root_conversation_id = root_conversation_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(root_conversation_id) = root_conversation_id else {
        return abort_delegate_runtime_descendants_by_parent_session(state, parent_chat_key);
    };

    let mut seen_delegate_ids = std::collections::HashSet::new();
    let children = delegate_runtime_thread_list(state)?
        .into_iter()
        .filter(|thread| {
            let exact_child = thread.parent_chat_session_key.as_deref() == Some(parent_chat_key);
            let same_root_sync_child = thread.root_conversation_id == root_conversation_id
                && thread.parent_chat_session_key.is_some();
            exact_child || same_root_sync_child
        })
        .filter(|thread| seen_delegate_ids.insert(thread.delegate_id.clone()))
        .collect::<Vec<_>>();
    abort_delegate_runtime_descendant_threads(state, parent_chat_key, children)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelReplyContentState {
    Visible,
    ReasoningOnly,
    Empty,
}

fn model_reply_content_state(reply: &ModelReply) -> ModelReplyContentState {
    if !reply.assistant_text.trim().is_empty()
        || !reply.final_response_text.trim().is_empty()
        || reply.assistant_provider_meta.is_some()
        || !reply.tool_history_events.is_empty()
        || reply.suppress_assistant_message
    {
        return ModelReplyContentState::Visible;
    }
    if !reply.activity_reasoning_text.trim().is_empty() {
        return ModelReplyContentState::ReasoningOnly;
    }
    ModelReplyContentState::Empty
}

fn effective_prompt_tokens_from_provider(
    estimated_prompt_tokens: u64,
    trusted_input_tokens: Option<u64>,
) -> (u64, &'static str) {
    let estimated = estimated_prompt_tokens.max(1);
    let Some(provider) = trusted_input_tokens.filter(|value| *value > 0) else {
        return (estimated_prompt_tokens, "estimate_no_provider");
    };
    let gap = provider.abs_diff(estimated) as f64 / estimated as f64;
    if gap > 0.5 {
        return (provider.max(estimated_prompt_tokens), "max_large_gap");
    }
    (provider, "provider")
}
