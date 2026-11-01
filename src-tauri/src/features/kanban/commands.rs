// ==================== 队伍议题看板：Tauri 命令 ====================
// 与 task 命令同形：outer 命令只做 State 拆包，逻辑在 _inner，
// 这样 Web 端（jsonrpc dispatch）能复用同一份实现。

const KANBAN_UPDATED_EVENT: &str = "easy-call:kanban-updated";

/// 用户点通过后那条通知消息的 kind，落进 provider_meta，
/// 让这条用户消息在日志与后续处理里能与手打的发言区分开。
const KANBAN_APPROVE_MESSAGE_KIND: &str = "kanban_issue_approved";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanBoardRequest {
    conversation_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanApproveIssueRequest {
    conversation_id: String,
    number: u32,
    #[serde(default)]
    note: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanBoardView {
    conversation_id: String,
    issues: Vec<KanbanIssueView>,
}

/// 广播看板变更。磁贴与看板页都订阅它，不需要各自轮询。
fn kanban_emit_updated(state: &AppState, leader_conversation_id: &str) {
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(_) => None,
    };
    let Some(app_handle) = app_handle else {
        runtime_log_warn(format!(
            "[议题看板] 广播失败：app_handle 不可用，conversation_id={leader_conversation_id}"
        ));
        return;
    };
    let payload = serde_json::json!({ "conversationId": leader_conversation_id });
    if let Err(err) = app_handle.emit(KANBAN_UPDATED_EVENT, payload) {
        runtime_log_error(format!(
            "[议题看板] 广播失败：conversation_id={leader_conversation_id}，error={err}"
        ));
    }
}

#[tauri::command]
pub(crate) fn kanban_get_board(
    input: KanbanBoardRequest,
    state: State<'_, AppState>,
) -> Result<KanbanBoardView, String> {
    kanban_get_board_inner(input, state.inner())
}

pub(crate) fn kanban_get_board_inner(
    input: KanbanBoardRequest,
    state: &AppState,
) -> Result<KanbanBoardView, String> {
    let conversation_id = kanban_leader_conversation_id(state, &input.conversation_id)?;
    let issues = kanban_board_list(state, &conversation_id, None, None, None)?;
    Ok(KanbanBoardView {
        conversation_id,
        issues,
    })
}

/// 通过通知的正文：队长不回读看板就能接上下文。
/// 措辞按「用户自己说的一句话」写，不是系统告警；没填备注时不追加空段落。
fn kanban_approve_notification_text(view: &KanbanIssueView, note: &str) -> String {
    let mut text = format!(
        "议题 #{}「{}」我点了通过，这条就算定了。",
        view.number,
        view.title.trim()
    );
    let note = note.trim();
    if !note.is_empty() {
        text.push_str(&format!("\n\n我的备注：{note}"));
    }
    text
}

fn kanban_approve_notification_message(view: &KanbanIssueView, note: &str) -> ChatMessage {
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: None,
        parts: vec![MessagePart::Text {
            text: kanban_approve_notification_text(view, note),
            reasoning_content: None,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "messageKind": KANBAN_APPROVE_MESSAGE_KIND,
            "message_meta": {
                "kind": KANBAN_APPROVE_MESSAGE_KIND
            }
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

/// 用户点通过的那一刻要以**用户消息**的形态送达队长：
/// 所以 source=User、queue_mode=Normal——和真人在输入框敲一句走同一条路。
/// 不用 Guided：引导会把队长正在跑的轮次切断，用户点个通过不该有这种副作用；
/// Normal 让它在队长空闲时立刻起一轮，正忙时排在当前轮次之后。
/// 目标会话由调用方用 kanban_leader_conversation_id 过滤过，队员会话与追问会话到不了这里。
fn kanban_approve_notification_event(
    leader_conversation_id: &str,
    agent_id: &str,
    root_conversation_id: Option<String>,
    view: &KanbanIssueView,
    note: &str,
) -> ChatPendingEvent {
    let mut runtime_context = runtime_context_new("kanban_approve", KANBAN_APPROVE_MESSAGE_KIND);
    runtime_context.request_id = Some(format!("kanban-approve-request-{}", Uuid::new_v4()));
    runtime_context.dispatch_id = Some(format!("kanban-approve-dispatch-{}", Uuid::new_v4()));
    runtime_context.target_conversation_id = Some(leader_conversation_id.to_string());
    runtime_context.root_conversation_id =
        root_conversation_id.or_else(|| Some(leader_conversation_id.to_string()));
    runtime_context.executor_agent_id = Some(agent_id.to_string());
    ChatPendingEvent {
        id: format!("kanban-approve-{}", Uuid::new_v4()),
        conversation_id: leader_conversation_id.to_string(),
        created_at: now_iso(),
        source: ChatEventSource::User,
        queue_mode: ChatQueueMode::Normal,
        messages: vec![kanban_approve_notification_message(view, note)],
        activate_assistant: true,
        assistant_message_id: None,
        session_info: ChatSessionInfo {
            agent_id: agent_id.to_string(),
        },
        runtime_context: Some(runtime_context),
        sender_info: None,
    }
}

/// 通知是「点通过」的附带动作：入队失败不回滚状态、也不让命令失败，只记 warn。
/// 重复点击走不到这里——kanban_apply_approve 在状态不是 in_review 时直接返回 Err。
fn kanban_notify_leader_approved(
    state: &AppState,
    leader_conversation_id: &str,
    view: &KanbanIssueView,
    note: &str,
) {
    let meta = match conversation_service_v2()
        .get_conversation_meta(state, leader_conversation_id)
    {
        Ok(meta) => meta,
        Err(err) => {
            runtime_log_warn(format!(
                "[议题看板] 通过通知跳过，任务=读取队长会话，conversation_id={leader_conversation_id}，issue=#{}，error={err}",
                view.number
            ));
            return;
        }
    };
    let event = kanban_approve_notification_event(
        leader_conversation_id,
        meta.agent_id.trim(),
        meta.root_conversation_id.clone(),
        view,
        note,
    );
    match ingress_chat_event(state, event) {
        Ok(ingress) => trigger_chat_event_after_ingress(state, ingress),
        Err(err) => runtime_log_warn(format!(
            "[议题看板] 通过通知入队失败，conversation_id={leader_conversation_id}，issue=#{}，error={err}",
            view.number
        )),
    }
}

#[tauri::command]
pub(crate) fn kanban_approve_issue(
    input: KanbanApproveIssueRequest,
    state: State<'_, AppState>,
) -> Result<KanbanIssueView, String> {
    kanban_approve_issue_inner(input, state.inner())
}

pub(crate) fn kanban_approve_issue_inner(
    input: KanbanApproveIssueRequest,
    state: &AppState,
) -> Result<KanbanIssueView, String> {
    let conversation_id = kanban_leader_conversation_id(state, &input.conversation_id)?;
    let view = kanban_apply_approve(state, &conversation_id, input.number, &input.note)?;
    kanban_emit_updated(state, &conversation_id);
    // 状态已经落库，通知才发：顺序反过来的话，入队成功而状态没改会留下一条假通知。
    kanban_notify_leader_approved(state, &conversation_id, &view, &input.note);
    Ok(view)
}
