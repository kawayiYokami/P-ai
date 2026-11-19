// ==================== 后台 shell 监控命令（监控面板 / Web 端共用） ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListConversationBackgroundShellTasksInput {
    conversation_id: String,
    /// 是否连名下的追问会话、队员会话与委托会话一起查（首页磁贴要看到「谁在跑」，
    /// 监控面板只看当前会话时保持默认 false）。
    #[serde(default)]
    include_sub_conversations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TerminateConversationBackgroundShellTaskInput {
    conversation_id: String,
    task_id: String,
}

/// 列出当前会话的后台 shell 任务（含日志尾部，供监控面板直接展示）。
///
/// `includeSubConversations=true` 时把该会话名下的追问会话、队员会话、委托会话一起查：
/// 首页磁贴要回答的是「这个主会话手底下现在谁在跑后台命令」，只看自己那一份会漏掉队员。
#[tauri::command]
async fn list_conversation_background_shell_tasks(
    state: State<'_, AppState>,
    input: ListConversationBackgroundShellTasksInput,
) -> Result<Vec<Value>, String> {
    let conversation_id = input.conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("conversationId 不能为空".to_string());
    }
    let conversation_ids = if input.include_sub_conversations {
        conversation_tree_background_shell_scope(state.inner(), &conversation_id)
    } else {
        vec![conversation_id]
    };
    let mut summaries = Vec::<Value>::new();
    for scoped_conversation_id in conversation_ids {
        let tasks =
            terminal_background_shell_monitor_summaries(state.inner(), &scoped_conversation_id).await;
        for mut task in tasks {
            if let Some(object) = task.as_object_mut() {
                object.insert(
                    "conversationId".to_string(),
                    Value::String(scoped_conversation_id.clone()),
                );
            }
            summaries.push(task);
        }
    }
    Ok(summaries)
}

/// 主会话 + 它的队员会话 + 它名下（含嵌套）的委托会话。
///
/// 只用于后台任务的作用域：这几类会话的后台命令都属于「这个主会话正在干的事」。
fn conversation_tree_background_shell_scope(state: &AppState, root_conversation_id: &str) -> Vec<String> {
    let mut scope = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    let mut push = |conversation_id: &str| {
        let value = conversation_id.trim().to_string();
        if !value.is_empty() && seen.insert(value.clone()) {
            scope.push(value);
        }
    };
    push(root_conversation_id);
    if let Ok(conversation) =
        conversation_service_v2().get_conversation_meta(state, root_conversation_id)
    {
        for member in conversation.team.members.iter() {
            push(&member.conversation_id);
        }
        for child_id in conversation.child_conversation_ids.iter() {
            push(child_id);
        }
    }
    if let Ok(threads) = delegate_runtime_thread_list(state) {
        for thread in threads {
            if thread.root_conversation_id.trim() == root_conversation_id.trim() {
                push(&thread.conversation.id);
            }
        }
    }
    scope
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
