// ==================== 后台 shell 监控命令（监控面板 / Web 端共用） ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListConversationBackgroundShellTasksInput {
    conversation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminateConversationBackgroundShellTaskInput {
    conversation_id: String,
    task_id: String,
}

/// 列出当前会话的后台 shell 任务（含日志尾部，供监控面板直接展示）
#[tauri::command]
async fn list_conversation_background_shell_tasks(
    state: State<'_, AppState>,
    input: ListConversationBackgroundShellTasksInput,
) -> Result<Vec<Value>, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    Ok(terminal_background_shell_monitor_summaries(state.inner(), &conversation_id).await)
}

/// 终止当前会话的后台 shell 任务；已终态时幂等返回
#[tauri::command]
async fn terminate_conversation_background_shell_task(
    state: State<'_, AppState>,
    input: TerminateConversationBackgroundShellTaskInput,
) -> Result<Value, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    let task_id = input.task_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    if task_id.is_empty() {
        return Err("taskId 不能为空".to_string());
    }
    let (killed, confirmed, status, log) =
        terminal_background_shell_request_kill(state.inner(), &conversation_id, &task_id, "user").await?;
    Ok(serde_json::json!({
        "ok": true,
        "id": task_id,
        "killed": killed,
        "confirmed": confirmed,
        "status": status,
        "log": log,
    }))
}

// ==================== Web 端 jsonrpc 分发（与 Tauri command 同语义） ====================

async fn ide_chat_background_shell_list_command(state: &AppState, params: Value) -> Result<Value, String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Input {
        #[serde(default)]
        conversation_id: String,
    }
    let input = ide_chat_parse_params::<Input>(params)?;
    let tasks = terminal_background_shell_monitor_summaries(state, &input.conversation_id).await;
    ide_chat_serialize(tasks)
}

async fn ide_chat_background_shell_terminate_command(state: &AppState, params: Value) -> Result<Value, String> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Input {
        #[serde(default)]
        conversation_id: String,
        #[serde(default)]
        task_id: String,
    }
    let input = ide_chat_parse_params::<Input>(params)?;
    let (killed, confirmed, status, log) = terminal_background_shell_request_kill(
        state,
        &input.conversation_id,
        &input.task_id,
        "user",
    )
    .await?;
    ide_chat_serialize(serde_json::json!({
        "ok": true,
        "id": input.task_id,
        "killed": killed,
        "confirmed": confirmed,
        "status": status,
        "log": log,
    }))
}
