// ==================== 会话提问：业务编排 ====================
// 问题只住会话元数据快照里的 `questions` 字段。
//
// 读走 `state_conversation_questions`，写走 `state_set_conversation_questions`：两个方法
// 都直接读写内存，立刻返回；写完之后由那个字段自己异步落到 SQL。这条链路上没有 SQL、
// 没有锁、没有事务、没有错误分支。

/// 问题列表变更广播：Web/IDE 走点号方法名，桌面窗口走 `easy-call:` 事件名，两条都要发。
fn question_emit_items_updated(
    state: &AppState,
    conversation_id: &str,
    views: &[ConversationQuestionView],
) {
    let payload = serde_json::json!({
        "conversationId": conversation_id,
        "items": views,
    });
    ide_chat_broadcast_notification("question.itemsUpdated", payload.clone());
    let app_handle = match state.app_handle.lock() {
        Ok(guard) => guard.as_ref().cloned(),
        Err(err) => {
            runtime_log_error(format!(
                "[聊天] 会话提问 获取窗口句柄失败：锁已损坏 错误={}",
                err
            ));
            None
        }
    };
    let Some(app_handle) = app_handle else {
        return;
    };
    if let Err(err) = app_handle.emit("easy-call:conversation-question-updated", payload) {
        runtime_log_error(format!("[聊天] 会话提问 推送失败 错误={}", err));
    }
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(state, conversation_id);
}

fn question_views(items: &[ConversationQuestionEntry]) -> Vec<ConversationQuestionView> {
    items.iter().map(question_view).collect()
}

/// 读：只读快照里的这一个字段。
fn question_views_for_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Result<Vec<ConversationQuestionView>, String> {
    Ok(question_views(
        &state_conversation_questions(state, conversation_id).items,
    ))
}

/// 工具 `update` 的写入：整批解释成写意图，一次改到快照上。
///
/// `new` 用快照里的 `nextNumber` 写一条并把游标加一；带编号就是对那一整条覆盖，
/// 不读旧值、不合并字段。
fn question_apply_tool_mutations(
    state: &AppState,
    conversation_id: &str,
    items: &[QuestionUpdateItem],
) -> Result<Vec<ConversationQuestionView>, String> {
    let normalized = conversation_id.trim();
    if normalized.is_empty() {
        return Err("会话 ID 不能为空".to_string());
    }
    if items.is_empty() {
        return Err("items 不能为空：需要提供要写入的条目".to_string());
    }
    let mut mutations = Vec::with_capacity(items.len());
    for item in items {
        mutations.push(question_mutation_from_item(item)?);
    }

    let mut column = state_conversation_questions(state, normalized);
    let create_count = mutations
        .iter()
        .filter(|mutation| matches!(mutation, QuestionMutation::Write { number: None, .. }))
        .count();
    if column.items.len() + create_count > QUESTION_MAX_PER_CONVERSATION {
        return Err(format!(
            "当前会话已有 {} 条问题，一次最多保留 {} 条：请先处理已答的问题再新建",
            column.items.len(),
            QUESTION_MAX_PER_CONVERSATION
        ));
    }

    for mutation in mutations {
        match mutation {
            QuestionMutation::Write {
                number: None,
                title,
                background,
                options,
            } => {
                let now = now_utc_rfc3339();
                let number = column.next_number;
                column.next_number = number.saturating_add(1);
                column.items.push(ConversationQuestionEntry {
                    number,
                    title,
                    background,
                    options,
                    created_at: now.clone(),
                    updated_at: now,
                });
            }
            QuestionMutation::Write {
                number: Some(number),
                title,
                background,
                options,
            } => {
                let entry = column
                    .items
                    .iter_mut()
                    .find(|entry| entry.number == number)
                    .ok_or_else(|| {
                        format!("#{number} 不存在（可能已被回答或清理），请先 list 查看当前问题")
                    })?;
                entry.title = title;
                entry.background = background;
                entry.options = options;
                entry.updated_at = now_utc_rfc3339();
            }
        }
    }

    let views = question_views(&column.items);
    state_set_conversation_questions(state, normalized, column);
    question_emit_items_updated(state, normalized, &views);
    Ok(views)
}

/// 答题与放弃都只做这一件事：按编号从快照里去掉，然后广播。
///
/// 不读题目内容、不判断编号是否存在——编号是人点出来的。
fn question_remove_numbers(
    state: &AppState,
    conversation_id: &str,
    numbers: &[u32],
) -> Result<Vec<ConversationQuestionView>, String> {
    let normalized = conversation_id.trim();
    let mut column = state_conversation_questions(state, normalized);
    column
        .items
        .retain(|entry| !numbers.contains(&entry.number));
    let views = question_views(&column.items);
    state_set_conversation_questions(state, normalized, column);
    question_emit_items_updated(state, normalized, &views);
    Ok(views)
}

/// 会话提问工具的总入口：`list` 读取，`update` 写入。
fn question_run_tool(
    state: &AppState,
    conversation_id: &str,
    request: QuestionToolRequest,
) -> Result<Vec<ConversationQuestionView>, String> {
    match request.action.trim().to_ascii_lowercase().as_str() {
        "list" => question_views_for_conversation(state, conversation_id),
        "update" => question_apply_tool_mutations(state, conversation_id, &request.items),
        "" => Err("缺少 action：请使用 list 或 update".to_string()),
        other => Err(format!("未知 action：{other}，仅支持 list 与 update")),
    }
}
