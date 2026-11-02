// ==================== 队伍议题看板：Tauri 命令 ====================
// 与 task 命令同形：outer 命令只做 State 拆包，逻辑在 _inner，
// 这样 Web 端（jsonrpc dispatch）能复用同一份实现。

const KANBAN_UPDATED_EVENT: &str = "easy-call:kanban-updated";

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
    Ok(view)
}
