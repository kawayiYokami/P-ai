fn background_tool_conversation_id(session_id: &str) -> Result<String, String> {
    goal_tool_conversation_id(session_id)
}

fn background_clip_text(text: String, limit: Option<usize>) -> String {
    let Some(limit) = limit else {
        return text;
    };
    if limit == 0 {
        return String::new();
    }
    text.chars().take(limit).collect()
}

async fn background_shell_status_inner(
    state: &AppState,
    session_id: &str,
    task_id: &str,
    limit: Option<usize>,
) -> Result<Value, String> {
    let conversation_id = background_tool_conversation_id(session_id)?;
    let maybe_task = terminal_background_shell_find(state, &conversation_id, task_id).await;
    let Some(task) = maybe_task else {
        return Err(format!("background id not found: {}", task_id.trim()));
    };
    let detail = background_clip_text(terminal_background_shell_status_text(&task), limit);
    Ok(serde_json::json!({
        "id": task.id,
        "status": format!("{:?}", *task.status.lock().expect("terminal background status poisoned")),
        "detail": detail,
    }))
}

async fn background_shell_kill_inner(
    state: &AppState,
    session_id: &str,
    task_id: &str,
) -> Result<Value, String> {
    let conversation_id = background_tool_conversation_id(session_id)?;
    let (killed, confirmed, status, log) =
        terminal_background_shell_request_kill(state, &conversation_id, task_id.trim(), "assistant").await?;
    // 只回「停掉了没有」和尾巴日志：killed 与 alreadyTerminal 互为反，
    // confirmed 是内部确认位，kind 恒为 shell。
    let mut output = serde_json::json!({
        "id": task_id.trim(),
        "status": status,
        "log": log,
    });
    if !killed {
        output["alreadyEnded"] = serde_json::json!(true);
    }
    let _ = confirmed;
    Ok(output)
}

async fn background_list_inner(state: &AppState, session_id: &str, limit: Option<usize>) -> Result<Value, String> {
    let conversation_id = background_tool_conversation_id(session_id)?;
    let mut items = Vec::<Value>::new();
    for item in terminal_background_shell_list(state, &conversation_id).await {
        items.push(item);
    }
    let summaries = list_conversation_delegate_statuses_inner(
        ListConversationDelegateStatusesInput {
            conversation_id: conversation_id.clone(),
        },
        state,
    )?;
    for summary in summaries {
        // 条目只留能被下一步用到的：id 定位、kind 决定续查走哪条、status 核心状态、
        // title 唯一的人话、startedAt 判断跑了多久。其余都是内部进度账本。
        let item = serde_json::json!({
            "id": summary.delegate_id,
            "kind": summary.kind,
            "status": summary.status,
            "title": summary.title,
            "startedAt": summary.started_at,
        });
        items.push(item);
    }
    let mut clipped = Vec::<Value>::new();
    for item in items {
        let mut trial = clipped.clone();
        trial.push(item.clone());
        let rendered = serde_json::to_string(&trial)
            .map_err(|err| format!("Serialize background list failed: {err}"))?;
        if let Some(limit) = limit {
            if rendered.chars().count() > limit {
                break;
            }
        }
        clipped.push(item);
    }
    Ok(serde_json::json!({
        "ok": true,
        "action": "list",
        "conversationId": conversation_id,
        "count": clipped.len(),
        "items": clipped,
    }))
}

fn background_delegate_latest_result_text(state: &AppState, delegate_id: &str) -> Option<String> {
    let conversation = delegate_runtime_thread_conversation_get_any(state, delegate_id).ok()?;
    let conversation = conversation?;
    conversation.messages.iter().rev().find_map(|message| {
        if message.role.trim().eq_ignore_ascii_case("assistant") {
            let text = message
                .parts
                .iter()
                .filter_map(|part| match part {
                    MessagePart::Text { text, .. } => Some(text.trim()),
                    _ => None,
                })
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if text.is_empty() { None } else { Some(text) }
        } else {
            None
        }
    })
}

fn background_delegate_status_inner(
    state: &AppState,
    session_id: &str,
    delegate_id: &str,
    limit: Option<usize>,
) -> Result<Value, String> {
    let conversation_id = background_tool_conversation_id(session_id)?;
    let summaries = list_conversation_delegate_statuses_inner(
        ListConversationDelegateStatusesInput {
            conversation_id: conversation_id.clone(),
        },
        state,
    )?;
    let Some(summary) = summaries
        .into_iter()
        .find(|item| item.delegate_id == delegate_id.trim())
    else {
        return Err(format!("background id not found: {}", delegate_id.trim()));
    };
    if summary.active {
        let detail = background_clip_text(
            format!(
                "id={}\nkind={}\nstatus={}\ntitle={}\nstartedAt={}\nupdatedAt={}",
                summary.delegate_id,
                summary.kind,
                summary.status,
                summary.title,
                summary.started_at,
                summary.updated_at,
            ),
            limit,
        );
        return Ok(serde_json::json!({
            "id": summary.delegate_id,
            "status": summary.status,
            "detail": detail,
        }));
    }
    let result_text = background_delegate_latest_result_text(state, &summary.delegate_id)
        .unwrap_or_else(|| "结果已写回会话".to_string());
    let result_text = background_clip_text(result_text, limit);
    Ok(serde_json::json!({
        "id": summary.delegate_id,
        "status": summary.status,
        "detail": result_text,
    }))
}

fn background_delegate_kill_inner(
    state: &AppState,
    session_id: &str,
    delegate_id: &str,
) -> Result<Value, String> {
    let conversation_id = background_tool_conversation_id(session_id)?;
    let summaries = list_conversation_delegate_statuses_inner(
        ListConversationDelegateStatusesInput {
            conversation_id: conversation_id.clone(),
        },
        state,
    )?;
    if !summaries.iter().any(|item| item.delegate_id == delegate_id.trim()) {
        return Err(format!("background id not found: {}", delegate_id.trim()));
    }
    let result = abort_delegate_conversation_inner(
        AbortDelegateConversationInput {
            delegate_id: delegate_id.trim().to_string(),
        },
        state,
    )?;
    Ok(serde_json::json!({
        "id": delegate_id.trim(),
        "status": if result.aborted { "killed" } else { "already ended" },
    }))
}

async fn builtin_background(
    state: &AppState,
    session_id: &str,
    args: BackgroundToolArgs,
) -> Result<Value, String> {
    let action = args.action.trim().to_ascii_lowercase();
    match action.as_str() {
        "list" => background_list_inner(state, session_id, args.limit).await,
        "status" => {
            let id = args
                .id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "background.id is required for status".to_string())?;
            match background_shell_status_inner(state, session_id, id, args.limit).await {
                Ok(value) => Ok(value),
                Err(err) if err.starts_with("background id not found:") => {
                    background_delegate_status_inner(state, session_id, id, args.limit)
                }
                Err(err) => Err(err),
            }
        }
        "kill" => {
            let id = args
                .id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "background.id is required for kill".to_string())?;
            match background_shell_kill_inner(state, session_id, id).await {
                Ok(value) => Ok(value),
                Err(err) if err.starts_with("background id not found:") => {
                    background_delegate_kill_inner(state, session_id, id)
                }
                Err(err) => Err(err),
            }
        }
        other => Err(format!("background.action 必须是 list|status|kill，当前收到：{other}")),
    }
}
