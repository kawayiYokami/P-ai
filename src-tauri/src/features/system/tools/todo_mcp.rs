const TODO_TOOL_NAME: &str = "todo";

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TodoWriteRequest {
    todos: Vec<TodoWireItem>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TodoWireItem {
    content: String,
    status: String,
}

fn todo_status_normalized(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pending" => Some("pending"),
        "in_progress" => Some("in_progress"),
        "completed" => Some("completed"),
        _ => None,
    }
}

fn todo_items_normalized(items: &[TodoWireItem]) -> Result<Vec<ConversationTodoItem>, String> {
    let mut normalized = Vec::<ConversationTodoItem>::new();
    let mut in_progress_count = 0usize;
    for item in items {
        let content = item.content.trim();
        if content.is_empty() {
            return Err("todo.content 不能为空".to_string());
        }
        let status = todo_status_normalized(&item.status)
            .ok_or_else(|| format!("todo.status 非法：{}", item.status.trim()))?;
        if status == "in_progress" {
            in_progress_count += 1;
        }
        normalized.push(ConversationTodoItem {
            content: content.to_string(),
            status: status.to_string(),
        });
    }
    if in_progress_count > 1 {
        return Err("todo 同时只能有一个 in_progress".to_string());
    }
    Ok(normalized)
}

#[cfg(test)]
fn todo_items_normalized_from_tool_args(
    tool_args: &str,
) -> Result<Vec<ConversationTodoItem>, String> {
    let request = serde_json::from_str::<TodoWriteRequest>(tool_args)
        .map_err(|err| format!("todo 参数不是合法 JSON：{err}"))?;
    todo_items_normalized(&request.todos)
}

fn todo_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        TODO_TOOL_NAME,
        "会话内 Todo 步骤追踪工具。入参为完整 todos 列表，每次调用都会全量覆盖当前会话的 Todo。仅在复杂任务或多要求任务时使用；步骤数优先保持在 3~7 步；同一时刻只允许一个 in_progress；全部完成后应直接向用户汇报。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "当前会话的完整 Todo 列表。每次调用都会全量覆盖旧列表。",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": { "type": "string", "description": "步骤内容" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "步骤状态"
                            }
                        },
                        "required": ["content", "status"]
                    }
                }
            },
            "required": ["todos"]
        }),
    )
}

fn todo_items_all_completed(items: &[ConversationTodoItem]) -> bool {
    !items.is_empty() && items.iter().all(|item| item.status == "completed")
}

fn todo_status_marker(status: &str) -> &'static str {
    match status.trim() {
        "completed" => "✓",
        "in_progress" => "→",
        _ => "○",
    }
}

fn todo_response_text(items: &[ConversationTodoItem]) -> String {
    if items.is_empty() {
        return "已经完成了所有步骤，请向用户进行汇报".to_string();
    }
    let mut lines = vec!["## Current Todo List".to_string(), String::new()];
    for item in items {
        lines.push(format!(
            "{} {}",
            todo_status_marker(&item.status),
            item.content.trim()
        ));
    }
    if todo_items_all_completed(items) {
        lines.push(String::new());
        lines.push("已经完成了所有步骤，请向用户进行汇报".to_string());
    }
    lines.join("\n")
}

fn todo_markdown_block(items: &[ConversationTodoItem]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let mut lines = vec!["## Current Todo List".to_string(), String::new()];
    for item in items {
        lines.push(format!(
            "- [{}] {}",
            item.status.trim(),
            item.content.trim()
        ));
    }
    Some(lines.join("\n"))
}

fn build_conversation_todo_board_block(conversation: &Conversation) -> Option<String> {
    if conversation.current_todos.is_empty() {
        return None;
    }
    let mut lines = Vec::<String>::new();
    lines.push(format!("activeTodoCount: {}", conversation.current_todos.len()));
    lines.push("todoCountSuggestion: 优先保持 3~7 步；过少容易失去追踪意义，过多容易失控".to_string());
    for (idx, item) in conversation.current_todos.iter().enumerate() {
        let todo_no = idx + 1;
        lines.push(format!("todo[{todo_no}].content: {}", item.content.trim()));
        lines.push(format!("todo[{todo_no}].status: {}", item.status.trim()));
    }
    Some(prompt_xml_block("todo board", lines.join("\n")))
}

fn build_todo_guide_block() -> String {
    prompt_xml_block(
        "todo guide",
        "todo 是当前复杂任务的步骤追踪器，不是长期任务系统。\n\
         仅在复杂任务、多要求任务、或用户明确要求时使用 todo。\n\
         如果使用 todo，步骤数优先保持在 3~7 步。\n\
         开始执行某一步之前，先把该步标记为 in_progress。\n\
         任一时刻只允许一个 in_progress。\n\
         完成某一步后立即更新状态。\n\
         如果步骤已经无关、走偏、或不再需要，及时删除或改写，不要堆积垃圾步骤。\n\
         当所有步骤都完成后，不要继续制造新步骤，应直接向用户汇报结果。\
        ",
    )
}

fn build_summary_context_todo_block(conversation: &Conversation) -> Option<String> {
    todo_markdown_block(&conversation.current_todos)
        .map(|block| prompt_xml_block("current todo list", block))
}

fn todo_target_conversation_id(session_id: &str) -> Result<String, String> {
    let (_, _, conversation_id) = delegate_parse_session_parts(session_id);
    conversation_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "todo 工具缺少 conversation_id，无法定位当前会话".to_string())
}

fn builtin_todo(
    app_state: &AppState,
    session_id: &str,
    args: TodoWriteRequest,
) -> Result<String, String> {
    let conversation_id = todo_target_conversation_id(session_id)?;
    let normalized = todo_items_normalized(&args.todos)?;
    let response_text = todo_response_text(&normalized);
    update_conversation_todos_and_emit(app_state, &conversation_id, normalized)?;
    Ok(response_text)
}

#[cfg(test)]
fn conversation_todo_list(state: &AppState, conversation_id: &str) -> Result<Vec<ConversationTodoItem>, String> {
    if let Some(conversation) = delegate_runtime_thread_conversation_get(state, conversation_id)? {
        return Ok(conversation.current_todos);
    }
    let data = state_read_app_data_cached(state)?;
    data.conversations
        .iter()
        .find(|conversation| conversation.id == conversation_id.trim())
        .map(|conversation| conversation.current_todos.clone())
        .ok_or_else(|| format!("未找到会话，conversation_id={conversation_id}"))
}

#[cfg(test)]
fn conversation_todo_replace(
    state: &AppState,
    conversation_id: &str,
    todos: Vec<ConversationTodoItem>,
) -> Result<Vec<ConversationTodoItem>, String> {
    let stored = if todo_items_all_completed(&todos) {
        Vec::new()
    } else {
        todos
    };

    if let Some(mut conversation) = delegate_runtime_thread_conversation_get(state, conversation_id)? {
        conversation.current_todos = stored.clone();
        conversation.updated_at = now_iso();
        delegate_runtime_thread_conversation_update(state, conversation_id, conversation)?;
        return Ok(stored);
    }

    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut data = state_read_app_data_cached(state)?;
    let conversation = data
        .conversations
        .iter_mut()
        .find(|conversation| conversation.id == conversation_id.trim())
        .ok_or_else(|| format!("未找到会话，conversation_id={conversation_id}"))?;
    conversation.current_todos = stored.clone();
    conversation.updated_at = now_iso();
    state_write_app_data_cached(state, &data)?;
    drop(guard);
    Ok(stored)
}


