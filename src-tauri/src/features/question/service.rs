// ==================== 会话提问：业务编排 ====================
// 问题存在会话自己的元数据里，与会话同生共死：会话删了，它名下的问题跟着消失。

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
    let meta = conversation_service_v2().get_conversation_meta(state, normalized)?;
    Ok(meta
        .questions
        .iter()
        .map(question_view)
        .collect::<Vec<_>>())
}

/// 执行一次工具 `update` 调用的全部条目。
///
/// 先整批解释与校验，参数错误一条都不落；再在副本上逐条落地，最后一次性写回。
/// 任何一条失败，元数据一行都没动，不会留下「前几条写进去、后面报错」的结果漂移。
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

    let mut outcome: Option<Vec<ConversationQuestionView>> = None;
    state_update_conversation_metadata_cached(state, normalized, |conversation| {
        let mut draft = conversation.questions.clone();
        let mut next_number = conversation.question_next_number;

        let create_count = mutations
            .iter()
            .filter(|mutation| matches!(mutation, QuestionMutation::Create { .. }))
            .count();
        if draft.len() + create_count > QUESTION_MAX_PER_CONVERSATION {
            return Err(format!(
                "当前会话已有 {} 条问题，一次最多保留 {} 条：请先清理已回答的问题再新建",
                draft.len(),
                QUESTION_MAX_PER_CONVERSATION
            ));
        }

        for mutation in &mutations {
            match mutation {
                QuestionMutation::Create {
                    title,
                    background,
                    options,
                } => {
                    let now = now_utc_rfc3339();
                    let current_max = draft.iter().map(|entry| entry.number).max().unwrap_or(0);
                    let number = conversation_allocate_number(current_max, &mut next_number);
                    draft.push(ConversationQuestionEntry {
                        number,
                        title: title.clone(),
                        background: background.clone(),
                        options: options.clone(),
                        created_at: now.clone(),
                        updated_at: now,
                    });
                }
                QuestionMutation::Update {
                    number,
                    title,
                    background,
                    options,
                } => {
                    let entry = draft
                        .iter_mut()
                        .find(|entry| entry.number == *number)
                        .ok_or_else(|| {
                            format!(
                                "#{number} 不存在（可能已被回答或清理），请先 list 查看当前问题"
                            )
                        })?;
                    entry.title = title.clone();
                    entry.background = background.clone();
                    entry.options = options.clone();
                    entry.updated_at = now_utc_rfc3339();
                }
                QuestionMutation::Delete { number } => {
                    let before = draft.len();
                    draft.retain(|entry| entry.number != *number);
                    if draft.len() == before {
                        return Err(format!(
                            "#{number} 不存在（可能已被回答或清理），请先 list 查看当前问题"
                        ));
                    }
                }
            }
        }

        conversation.questions = draft;
        conversation.question_next_number = next_number;
        outcome = Some(
            conversation
                .questions
                .iter()
                .map(question_view)
                .collect::<Vec<_>>(),
        );
        Ok(())
    })?;

    let views = outcome.ok_or_else(|| "更新问题后未能取得结果".to_string())?;
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
        "update" => question_apply_tool_mutations(state, conversation_id, &request.items),
        "" => Err("缺少 action：请使用 list 或 update".to_string()),
        other => Err(format!("未知 action：{other}，仅支持 list 与 update")),
    }
}
