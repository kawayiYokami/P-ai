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

/// 登记一个会话轮次的运行记录。
///
/// 若同一请求号上留着「提交期取消」的标记，消费它并返回 `true`，表示这一轮不要开始生成。
/// 标记按请求号命中：别的调度、别的轮次的标记不会影响这一次。
///
/// **这里不中止任何协程。** 同一键上原本若还挂着上一轮的记录，只把记录换成本轮的：
/// 上一轮该自己走完结路线，谁也不许从外面掐它。
fn register_inflight_chat_abort_handle(
    state: &AppState,
    chat_key: &str,
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
            // 这次提交不生成：调用方随后就走完结路线。
            return Ok(true);
        }
    }
    // 同一轮会被登记两次：批次入口先按本轮定死的号登，发送入口再登一次补
    // assistant_message_id。号相同就是同一次调度的刷新，不是被新一轮接管——
    // 已退役的旧号必须留着，否则压缩重启前后的号会丢，那一轮就再也停不住。
    if let Some(InflightChatAbortEntry::Running {
        request_id: registered_request_id,
        assistant_message_id: registered_message_id,
        ..
    }) = inflight.get_mut(chat_key)
    {
        if registered_request_id.trim() == request_id.trim() {
            if !assistant_message_id.trim().is_empty() {
                *registered_message_id = assistant_message_id.trim().to_string();
            }
            runtime_log_debug(format!(
                "[聊天] 同一轮重复登记已并入现有记录 键={} 请求号={}",
                chat_key,
                request_id.trim()
            ));
            return Ok(false);
        }
    }
    let previous = inflight.insert(
        chat_key.to_string(),
        InflightChatAbortEntry::Running {
            request_id: request_id.trim().to_string(),
            assistant_message_id: assistant_message_id.trim().to_string(),
            // 新一轮从干净号开始：上一轮用过的号不继承，避免认错自己的条目。
            retired_request_ids: Vec::new(),
        },
    );
    if let Some(InflightChatAbortEntry::Running { request_id, .. }) = previous {
        runtime_log_warn(format!(
            "[聊天] 同一键上已有在跑的轮次，记录被新一轮接管（旧轮次自己收尾） 键={} 旧请求号={}",
            chat_key, request_id
        ));
    }
    Ok(false)
}

/// 这一轮收尾时摘掉自己的登记（按本轮用过的任一请求号认领）。
///
/// 摘掉之后停止方就找不到这条通道了——这一轮已经不需要被停止。
fn unregister_inflight_chat_round(state: &AppState, chat_key: &str, request_ids: &[String]) {
    let removed = {
        let Ok(mut inflight) = state.inflight_chat_abort_handles.lock() else {
            return;
        };
        let ours = request_ids
            .iter()
            .any(|id| inflight_entry_is_ours(inflight.get(chat_key), id));
        if ours {
            inflight.remove(chat_key);
            true
        } else {
            false
        }
    };
    if removed {
        // 收口顺便把本轮自己的号从待停列表里清掉。漏清也无害，这里只是不留垃圾。
        clear_round_stop_requested(state, request_ids);
    }
}

/// 上下文压缩后调度换新请求号时，把旧号收进历史、换成新号。
///
/// 压缩重启在同一次 run 内继续跑，请求号换成了新的。
/// 收尾认领要按「用过的任一请求号」才认得出自己，所以旧号累积进
/// `retired_request_ids` 而不是被覆盖——只比最初那个号会把重启过的这一轮认成别人的。
///
/// 这里不碰任何中止手段：这一轮读到结束信号会自己停。
/// `expected_request_id` 是调用方认为的当前号，只有表内仍是它时才改写，
/// 避免覆盖掉已经被更晚一次调度接手的条目。
fn refresh_inflight_chat_dispatch_identity(
    state: &AppState,
    chat_key: &str,
    expected_request_id: &str,
    request_id: &str,
    assistant_message_id: &str,
) -> Result<(), String> {
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    let Some(InflightChatAbortEntry::Running {
        request_id: registered_request_id,
        assistant_message_id: registered_message_id,
        retired_request_ids,
        ..
    }) = inflight.get_mut(chat_key)
    else {
        // 压缩重启途中这一轮已被停掉（表项没了或只剩取消标记）：没有可更新的记录。
        runtime_log_warn(format!(
            "[聊天] 压缩重启后刷新调度身份落空 期望请求={} 原因=表内无运行中轮次",
            expected_request_id.trim()
        ));
        return Ok(());
    };
    if !expected_request_id.trim().is_empty()
        && registered_request_id.trim() != expected_request_id.trim()
    {
        // 已被更晚的一次调度接手：不是我们的条目，不碰。
        runtime_log_warn(format!(
            "[聊天] 压缩重启后刷新调度身份跳过 原因=条目已被其他调度接手 期望={} 实际={}",
            expected_request_id.trim(),
            registered_request_id.trim()
        ));
        return Ok(());
    }
    // 调用方没带号（这一轮的身份是登记时自发的）：保留原号，不把身份刷成空。
    let next_request_id = if request_id.trim().is_empty() {
        registered_request_id.trim().to_string()
    } else {
        request_id.trim().to_string()
    };
    if next_request_id != registered_request_id.trim() {
        retired_request_ids.push(registered_request_id.trim().to_string());
        *registered_request_id = next_request_id.clone();
    }
    *registered_message_id = assistant_message_id.trim().to_string();
    Ok(())
}

/// 表里这一条是否属于本轮。
///
/// 按本轮用过的**任一**请求号认领：压缩重启会换号，旧号收在 `retired_request_ids` 里。
/// 只比最初那个 trace_id 会把重启过的这一轮认成别人，条目就永远摘不掉——
/// 残留的 `Running` 会让下一次提交期的停止撞上它并对不上号。
fn inflight_entry_is_ours(entry: Option<&InflightChatAbortEntry>, trace_id: &str) -> bool {
    let Some(InflightChatAbortEntry::Running {
        request_id,
        retired_request_ids,
        ..
    }) = entry
    else {
        return false;
    };
    request_id.trim() == trace_id.trim()
        || retired_request_ids
            .iter()
            .any(|retired| retired.trim() == trace_id.trim())
}

// ==================== 停止：一张待停号列表 ====================

/// 这一轮用过的全部请求号：登记时的号，加上上下文压缩重启换掉的旧号。
///
/// 找不到在跑的轮次就返回空：调用方按「这一轮不会被打断」处理。
fn inflight_round_request_ids(state: &AppState, chat_key: &str) -> Vec<String> {
    let Ok(inflight) = state.inflight_chat_abort_handles.lock() else {
        return Vec::new();
    };
    match inflight.get(chat_key) {
        Some(InflightChatAbortEntry::Running {
            request_id,
            retired_request_ids,
            ..
        }) => {
            let mut ids = retired_request_ids.clone();
            ids.push(request_id.clone());
            ids
        }
        _ => Vec::new(),
    }
}

/// 按会话 id 取那一轮用过的号（给不在 worker 调用链上的路径用，例如离场反思）。
fn inflight_round_request_ids_by_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Vec<String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Vec::new();
    }
    let Ok(inflight) = state.inflight_chat_abort_handles.lock() else {
        return Vec::new();
    };
    let suffix = format!("::{conversation_id}");
    let mut fallback: Option<Vec<String>> = None;
    for (key, entry) in inflight.iter() {
        let InflightChatAbortEntry::Running {
            request_id,
            retired_request_ids,
            ..
        } = entry
        else {
            continue;
        };
        let mut ids = retired_request_ids.clone();
        ids.push(request_id.clone());
        if key.trim() == conversation_id {
            return ids;
        }
        if key.ends_with(&suffix) && fallback.is_none() {
            fallback = Some(ids);
        }
    }
    fallback.unwrap_or_default()
}

/// 本轮是否已被要求停止：这一轮用过的任一请求号出现在待停列表里。
fn round_stop_requested(state: &AppState, request_ids: &[String]) -> bool {
    if request_ids.is_empty() {
        return false;
    }
    let Ok(stopped) = state.stopped_request_ids.lock() else {
        return false;
    };
    request_ids
        .iter()
        .any(|id| !id.trim().is_empty() && stopped.contains(id.trim()))
}

/// 把「要停的调度号」记进列表，并拍一下叫醒正在 `await` 的轮次。
///
/// 这不是开关：列表里留的是某个调度号，只有那一轮用过的号会命中它。下一轮的号天生不同，
/// 所以不需要任何复位动作，也不存在「谁该在什么时候把它清掉」这个问题。
fn mark_round_stop_requested(state: &AppState, request_ids: &[String]) -> Result<usize, String> {
    let mut marked = 0usize;
    {
        let mut stopped = state
            .stopped_request_ids
            .lock()
            .map_err(|_| "Failed to lock stopped request ids".to_string())?;
        for id in request_ids {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            if stopped.insert(id.to_string()) {
                marked += 1;
            }
        }
    }
    if marked > 0 {
        // 只拍一下：判据在列表里，这个通知不承载任何信息，拍空也没有副作用。
        state.chat_stop_notify.notify_waiters();
    }
    Ok(marked)
}

/// 收尾时把本轮用过的号从待停列表里删掉。
///
/// 漏清也无害：留下的是死号，新一轮的号天生不同，匹配不到它。清理只是卫生。
fn clear_round_stop_requested(state: &AppState, request_ids: &[String]) {
    let Ok(mut stopped) = state.stopped_request_ids.lock() else {
        return;
    };
    for id in request_ids {
        stopped.remove(id.trim());
    }
}


/// 让一次长等待与「本轮被要求停止」赛跑：本轮被点名就返回 `None`，工作就地放弃。
///
/// 建连、压缩、上下文装配这类「等外部东西」的步骤都该走它。裸 await 会让用户按了停止
/// 也毫无反应，一直挂到对方自己的超时——这是最常被卡住的一处。
///
/// 被拍醒不等于被停：唤醒是广播的（同时在跑的轮次个位数，多拍几下无所谓），醒来先查列表，
/// 不是自己这一轮的号就继续等工作，工作本身不会因为被拍而丢失。
async fn race_with_chat_stop<T>(
    state: &AppState,
    request_ids: &[String],
    work: impl std::future::Future<Output = T>,
) -> Option<T> {
    if request_ids.is_empty() {
        return Some(work.await);
    }
    let mut work = std::pin::pin!(work);
    loop {
        if round_stop_requested(state, request_ids) {
            return None;
        }
        let notified = state.chat_stop_notify.notified();
        tokio::select! {
            biased;
            _ = notified => {}
            value = &mut work => return Some(value),
        }
    }
}

/// 对指定调度键发起停止通知：该键上有没有活都如实回报。
///
/// **这里不中止任何协程。** worker 必须活着走完自己的收尾——落盘、广播、置空闲、释放占位、续跑
/// 都是它的活。把 worker 从外面弄死，收尾就没人做了。
///
/// 真正的「停」由结束信号完成（`request_graceful_chat_stop`）：worker 在各个等待点读到它，
/// 自己结束这一轮。本函数只负责两件事：
///
/// - 该 key 上正跑着轮次 → 如实返回 `true`，让调用方知道这次停止命中了活；
/// - 该 key 上什么都没有、但这次停止带了请求号 → 记下「取消这一次提交」，返回 `true`。
///   这只是一条欠账：提交还没登记，等它提交时按同一个号认领并直接放弃。
///   按号消费不是鉴权，是为了不把下一个刚提交的轮次一起误伤。
fn request_stop_inflight_chat(
    state: &AppState,
    chat_key: &str,
    request_id: &str,
) -> Result<bool, String> {
    let request_id = request_id.trim();
    let mut inflight = state
        .inflight_chat_abort_handles
        .lock()
        .map_err(|_| "Failed to lock inflight chat abort handles".to_string())?;
    if matches!(
        inflight.get(chat_key),
        Some(InflightChatAbortEntry::Running { .. })
    ) {
        runtime_log_debug(format!(
            "[聊天] 打断命中在跑轮次，交给结束信号收尾 键={}",
            chat_key
        ));
        return Ok(true);
    }
    match inflight.remove(chat_key) {
        Some(InflightChatAbortEntry::CancelRequested {
            request_id: pending,
        }) => {
            let same_request = !request_id.is_empty() && pending.trim() == request_id;
            if !same_request {
                inflight.insert(
                    chat_key.to_string(),
                    InflightChatAbortEntry::CancelRequested { request_id: pending },
                );
            }
            Ok(same_request)
        }
        _ => {
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

/// 清除指定会话的提交期取消标记；已登记的运行轮次不受影响。
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

/// 让指定调度键上的轮次自己停下来：**只记号、不中止任何协程**。
///
/// 记的是这一轮自己用过的请求号（当前号，加上上下文压缩重启换掉的旧号）。worker 在检查点
/// 问「我的号在不在待停列表里」，命中就自己收尾。返回该键上是否有在跑的轮次。
/// 这是全项目唯一的「打断一个在跑的轮次」入口——用户停止、会话中断、委托中止、
/// 远程应答取消都走它。
fn request_stop_inflight_chat_round(
    state: &AppState,
    chat_key: &str,
    conversation_id: &str,
) -> Result<bool, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(false);
    }
    let request_ids = inflight_round_request_ids(state, chat_key);
    if request_ids.is_empty() {
        return Ok(false);
    }
    let marked = mark_round_stop_requested(state, &request_ids)?;
    runtime_log_info(format!(
        "[聊天] 已记下要停的调度号 键={} 会话={} 新记号和数={}",
        chat_key, conversation_id, marked
    ));
    Ok(true)
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
    visited: &mut std::collections::HashSet<String>,
) -> Result<usize, String> {
    let mut aborted_count = 0usize;
    for thread in children {
        let child_chat_key = delegate_thread_chat_key(&thread);
        // 委托树可能出现自引用或环，同一个 chat_key 只处理一次，避免无限递归爆栈
        if !visited.insert(child_chat_key.clone()) {
            continue;
        }
        let stopped_chat =
            request_stop_inflight_chat_round(state, &child_chat_key, &thread.conversation.id)?;
        if stopped_chat {
            aborted_count += 1;
            runtime_log_info(format!(
                "[聊天] 已请求同步委托子会话收尾 父会话={} 子会话={}",
                parent_chat_key,
                child_chat_key
            ));
        }
        aborted_count +=
            abort_delegate_runtime_descendants_inner(state, &child_chat_key, visited)?;
    }
    Ok(aborted_count)
}

fn abort_delegate_runtime_descendants_by_parent_session(
    state: &AppState,
    parent_chat_key: &str,
) -> Result<usize, String> {
    let mut visited = std::collections::HashSet::<String>::new();
    visited.insert(parent_chat_key.to_string());
    abort_delegate_runtime_descendants_inner(state, parent_chat_key, &mut visited)
}

fn abort_delegate_runtime_descendants_inner(
    state: &AppState,
    parent_chat_key: &str,
    visited: &mut std::collections::HashSet<String>,
) -> Result<usize, String> {
    let children = delegate_runtime_thread_list(state)?
        .into_iter()
        .filter(|thread| thread.parent_chat_session_key.as_deref() == Some(parent_chat_key))
        .collect::<Vec<_>>();
    abort_delegate_runtime_descendant_threads(state, parent_chat_key, children, visited)
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
    let mut visited = std::collections::HashSet::<String>::new();
    visited.insert(parent_chat_key.to_string());
    abort_delegate_runtime_descendant_threads(state, parent_chat_key, children, &mut visited)
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

// ========== 请求层重试（重试下沉） ==========
// 把模型请求的重试从「重跑整个 attempt」下沉为「重发同一次请求」：请求体已构造好，
// 失败就原样再发一次，不重新组装上下文、不重跑工具。
// 口径见计划 20261121_模型请求重试下沉：
// - 零 delta 门禁：调用方在每次发送时通过 `pushed_any` 上报是否向前端推送过任何
//   delta（文本与 reasoning 都计入）。推过就不再重试，原样上抛。
// - 空回（Ok 但内容为空）可重试、仅思维链不重试，由调用方的 `should_retry_ok`
//   判定，本函数只执行判定结果。
// - 并发闸门由调用方在外层持有，重试退避期间保持占用、不释放：一次模型调用
//   （含重试等待）占用一个名额。供应商刚失败时正不健康，此时放别的请求进去只会更糟。
// - 每次尝试重取 API key 由调用方在闭包内完成（resolve + consume），429 换到
//   下一个 key 正是想要的。
// - 退避沿用 core_send_inner 的 retry_delay（含 Retry-After 与抖动），调用方
//   只传 max_attempts，不反向依赖退避常量。
async fn retry_request_once<T, F, Fut>(
    request_max_attempts: u32,
    should_retry_ok: impl Fn(&T) -> bool,
    mut send_once: F,
) -> Result<T, String>
where
    // 闭包返回 (结果, 本次是否推过 delta)：门禁状态随值返回，不借用外部，
    // 避免 future 逃逸出 FnMut 闭包时的借用冲突。
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = (Result<T, String>, bool)>,
{
    let max_attempts = request_max_attempts.max(1);
    let mut attempt: u32 = 0;
    loop {
        attempt += 1;
        let (outcome, pushed_any) = send_once().await;
        let retryable = match &outcome {
            Ok(value) => should_retry_ok(value) && !pushed_any,
            Err(err) => {
                !pushed_any && retry_kind_is_retryable(classify_retry_kind_for_error_text(err))
            }
        };
        if !retryable || attempt >= max_attempts {
            return outcome;
        }
        let retry_after_ms = match &outcome {
            Err(err) => parse_retry_after_ms(err),
            Ok(_) => None,
        };
        let wait = retry_delay(attempt, retry_after_ms);
        runtime_log_warn(format!(
            "[聊天] 请求层重试 任务=request_retry 尝试={attempt}/{max_attempts} 延迟={} 已推送={pushed_any}",
            wait.as_millis()
        ));
        tokio::time::sleep(wait).await;
    }
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

#[cfg(test)]
mod request_retry_tests {
    use super::*;

    fn empty_reply() -> ModelReply {
        ModelReply {
            assistant_text: String::new(),
            final_response_text: String::new(),
            activity_reasoning_text: String::new(),
            assistant_provider_meta: None,
            tool_history_events: Vec::new(),
            suppress_assistant_message: false,
            trusted_input_tokens: None,
            usage: None,
            round_logs_recorded_internally: false,
        }
    }

    fn is_empty_retryable(reply: &ModelReply) -> bool {
        matches!(
            model_reply_content_state(reply),
            ModelReplyContentState::Empty
        )
    }

    // 429 后第二次成功：同一闭包重发，调用 2 次（验收 1 的 helper 层面）。
    #[tokio::test]
    async fn retryable_error_resends_until_success() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(0_usize));
        let bodies = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let calls_for_send = calls.clone();
        let bodies_for_send = bodies.clone();
        let outcome = retry_request_once(
            3,
            is_empty_retryable,
            move || {
                let calls = calls_for_send.clone();
                let bodies = bodies_for_send.clone();
                async move {
                    let mut locked = calls.lock().expect("calls lock");
                    *locked += 1;
                    let attempt = *locked;
                    drop(locked);
                    bodies
                        .lock()
                        .expect("bodies lock")
                        .push("same-request-body".to_string());
                    if attempt == 1 {
                        (Err("upstream 429 Too Many Requests".to_string()), false)
                    } else {
                        (
                            Ok(ModelReply {
                                assistant_text: "pong".to_string(),
                                ..empty_reply()
                            }),
                            false,
                        )
                    }
                }
            },
        )
        .await;
        assert_eq!(outcome.unwrap().assistant_text, "pong");
        assert_eq!(*calls.lock().expect("calls lock"), 2);
        let bodies = bodies.lock().expect("bodies lock");
        assert_eq!(bodies.len(), 2);
        assert_eq!(bodies[0], bodies[1]);
    }

    // 推过 delta 就不再发第二次（验收 3 的 helper 层面：后端断言请求次数）。
    #[tokio::test]
    async fn pushed_delta_blocks_second_request() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(0_usize));
        let calls_for_send = calls.clone();
        let outcome: Result<String, String> = retry_request_once(
            5,
            |_: &String| true,
            move || {
                let calls = calls_for_send.clone();
                async move {
                    *calls.lock().expect("calls lock") += 1;
                    (Err("connection reset by peer".to_string()), true)
                }
            },
        )
        .await;
        assert!(outcome.is_err());
        assert_eq!(*calls.lock().expect("calls lock"), 1);
    }

    // 空回可重试、仅思维链不重试（验收 4 的 helper 层面）。
    #[tokio::test]
    async fn empty_ok_retries_but_reasoning_only_does_not() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(0_usize));
        let calls_for_send = calls.clone();
        let outcome = retry_request_once(
            3,
            is_empty_retryable,
            move || {
                let calls = calls_for_send.clone();
                async move {
                    *calls.lock().expect("calls lock") += 1;
                    if *calls.lock().expect("calls lock") == 1 {
                        (Ok(empty_reply()), false)
                    } else {
                        (
                            Ok(ModelReply {
                                assistant_text: "recovered".to_string(),
                                ..empty_reply()
                            }),
                            false,
                        )
                    }
                }
            },
        )
        .await;
        assert_eq!(outcome.unwrap().assistant_text, "recovered");
        assert_eq!(*calls.lock().expect("calls lock"), 2);

        let reasoning_only = ModelReply {
            activity_reasoning_text: "先想一步".to_string(),
            ..empty_reply()
        };
        let outcome = retry_request_once(3, is_empty_retryable, || async {
            (Ok(reasoning_only.clone()), false)
        })
        .await;
        assert!(outcome.unwrap().assistant_text.is_empty());
    }

    // 工具执行史不参与判定：调用方见证了工具执行，重试照样发生（验收 2 的结构保证）。
    #[tokio::test]
    async fn tools_executed_does_not_block_request_retry() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(0_usize));
        let calls_for_send = calls.clone();
        let outcome: Result<String, String> = retry_request_once(
            2,
            |_: &String| false,
            move || {
                let calls = calls_for_send.clone();
                async move {
                    // 模拟“已经执行过 3 次工具的一轮”：该事实只存在于调用方，
                    // helper 的签名里根本没有工具次数这一项。
                    let _tools_executed_in_attempt = 3_usize;
                    *calls.lock().expect("calls lock") += 1;
                    if *calls.lock().expect("calls lock") == 1 {
                        (Err("upstream 503 Service Unavailable".to_string()), false)
                    } else {
                        (Ok("recovered".to_string()), false)
                    }
                }
            },
        )
        .await;
        assert_eq!(outcome.unwrap(), "recovered");
        assert_eq!(*calls.lock().expect("calls lock"), 2);
    }

    // 不可重试错误与预算耗尽：一次都不多发。
    #[tokio::test]
    async fn non_retryable_or_exhausted_sends_once() {
        let calls = std::sync::Arc::new(std::sync::Mutex::new(0_usize));
        let calls_for_send = calls.clone();
        let outcome: Result<String, String> = retry_request_once(
            5,
            |_: &String| false,
            move || {
                let calls = calls_for_send.clone();
                async move {
                    *calls.lock().expect("calls lock") += 1;
                    (Err("invalid api key".to_string()), false)
                }
            },
        )
        .await;
        assert!(outcome.is_err());
        assert_eq!(*calls.lock().expect("calls lock"), 1);

        let calls_for_send = calls.clone();
        *calls.lock().expect("calls lock") = 0;
        let outcome: Result<String, String> = retry_request_once(
            1,
            |_: &String| false,
            move || {
                let calls = calls_for_send.clone();
                async move {
                    *calls.lock().expect("calls lock") += 1;
                    (Err("upstream 429 Too Many Requests".to_string()), false)
                }
            },
        )
        .await;
        assert!(outcome.is_err());
        assert_eq!(*calls.lock().expect("calls lock"), 1);
    }
}
