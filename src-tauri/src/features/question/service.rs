// ==================== 会话提问：业务编排 ====================

/// 会话列表行用：某会话下待回答的问题条数。
/// 查询失败按 0 处理——不能让问题库异常挡掉整个会话列表下发。
fn question_count_for_conversation(state: &AppState, conversation_id: &str) -> usize {
    let normalized = conversation_id.trim();
    if normalized.is_empty() {
        return 0;
    }
    match question_store_open(&state.data_path)
        .and_then(|conn| question_store_count(&conn, normalized))
    {
        Ok(count) => count,
        Err(err) => {
            runtime_log_debug(format!(
                "[会话问题] 读取待答问题数失败，按 0 处理，conversation_id={}，error={}",
                normalized, err
            ));
            0
        }
    }
}

/// 会话列表批量路径用：一次查询拿到所有会话的待答问题数。
fn question_counts_by_conversation(
    state: &AppState,
) -> std::collections::HashMap<String, usize> {
    match question_store_open(&state.data_path)
        .and_then(|conn| question_store_counts_by_conversation(&conn))
    {
        Ok(counts) => counts,
        Err(err) => {
            runtime_log_debug(format!(
                "[会话问题] 批量读取待答问题数失败，全部按 0 处理，error={}",
                err
            ));
            std::collections::HashMap::new()
        }
    }
}

/// 会话删除时连带清理问题记录与编号游标。
/// 清理失败只记 warn，不阻断会话删除本身——问题库异常不应让会话删不掉。
fn question_clear_conversation(state: &AppState, conversation_id: &str) {
    let normalized = conversation_id.trim();
    if normalized.is_empty() {
        return;
    }
    let result = question_store_open(&state.data_path)
        .and_then(|conn| question_store_delete_conversation(&conn, normalized));
    if let Err(err) = result {
        runtime_log_warn(format!(
            "[会话问题] 清理失败，任务=会话删除连带清理，conversation_id={}，error={}",
            normalized, err
        ));
    }
}

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
                "[会话问题] 获取 app_handle 失败：锁已损坏，error={:?}",
                err
            ));
            None
        }
    };
    let Some(app_handle) = app_handle else {
        return;
    };
    if let Err(err) = app_handle.emit("easy-call:conversation-question-updated", payload) {
        runtime_log_error(format!("[会话问题] 推送失败：错误={}", err));
    }
}

fn question_views_for_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Result<Vec<ConversationQuestionView>, String> {
    let normalized = conversation_id.trim();
    if normalized.is_empty() {
        return Err("会话 ID 不能为空".to_string());
    }
    let conn = question_store_open(&state.data_path)?;
    let items = question_store_list(&conn, normalized)?;
    Ok(items.iter().map(ConversationQuestion::to_view).collect())
}

/// 执行一次工具 `update` 调用的全部条目。
///
/// 先整批解释与校验，参数错误一条都不落；再逐条落地，避免「前几条写进去、后面报错」的结果漂移。
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
        return Err("items 不能为空：需要提供要新建、更新或清理的条目".to_string());
    }
    let mut mutations = Vec::with_capacity(items.len());
    for item in items {
        mutations.push(question_mutation_from_item(item)?);
    }

    let mut conn = question_store_open(&state.data_path)?;
    let current_count = question_store_count(&conn, normalized)?;
    let create_count = mutations
        .iter()
        .filter(|mutation| matches!(mutation, QuestionMutation::Create { .. }))
        .count();
    if current_count + create_count > QUESTION_MAX_PER_CONVERSATION {
        return Err(format!(
            "当前会话已有 {} 条问题，一次最多保留 {} 条：请先清理已回答的问题再新建",
            current_count, QUESTION_MAX_PER_CONVERSATION
        ));
    }

    for mutation in &mutations {
        match mutation {
            QuestionMutation::Create {
                title,
                background,
                options,
            } => {
                question_store_insert(&mut conn, normalized, title, background, options)?;
            }
            QuestionMutation::Update {
                number,
                title,
                background,
                options,
            } => {
                let updated = question_store_update(
                    &conn,
                    normalized,
                    *number,
                    title,
                    background,
                    options,
                )?;
                if updated == 0 {
                    return Err(format!(
                        "#{number} 不存在（可能已被回答或清理），请先 list 查看当前问题"
                    ));
                }
            }
            QuestionMutation::Delete { number } => {
                let deleted = question_store_delete(&conn, normalized, *number)?;
                if deleted == 0 {
                    return Err(format!(
                        "#{number} 不存在（可能已被回答或清理），请先 list 查看当前问题"
                    ));
                }
            }
        }
    }

    let items = question_store_list(&conn, normalized)?;
    let views = items
        .iter()
        .map(ConversationQuestion::to_view)
        .collect::<Vec<_>>();
    question_emit_items_updated(state, normalized, &views);
    Ok(views)
}

/// 会话提问工具的总入口：`list` 读取，`update` 增删改。
fn question_run_tool(
    state: &AppState,
    conversation_id: &str,
    request: QuestionToolRequest,
) -> Result<Vec<ConversationQuestionView>, String> {
    match request.action.trim().to_ascii_lowercase().as_str() {
        "list" => question_views_for_conversation(state, conversation_id),
        "update" => {
            question_apply_tool_mutations(state, conversation_id, &request.items)
        }
        "" => Err("缺少 action：请使用 list 或 update".to_string()),
        other => Err(format!("未知 action：{other}，仅支持 list 与 update")),
    }
}
