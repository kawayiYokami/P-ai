// ==================== 会话提问：答题命令 ====================

/// 单选答案：`optionIndex` 是 `ConversationQuestion.options` 的下标。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuestionAnswerInput {
    number: u32,
    #[serde(default)]
    option_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestionAnswerResult {
    answered_numbers: Vec<u32>,
    remaining: Vec<ConversationQuestionView>,
    message_text: String,
}

/// Web/IDE 侧转发用的参数形状（jsonrpc 侧与桌面端命令同构）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuestionListWebParams {
    #[serde(default)]
    conversation_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuestionAnswerWebParams {
    #[serde(default)]
    conversation_id: String,
    #[serde(default)]
    answers: Vec<QuestionAnswerInput>,
}

fn build_question_answer_message(text: &str, numbers: &[u32]) -> ChatMessage {
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_utc_rfc3339(),
        speaker_agent_id: None,
        parts: vec![MessagePart::Text {
            text: text.to_string(),
            reasoning_content: None,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "message_meta": {
                "kind": "question_answer",
                "question_number": numbers.first().copied(),
                "question_numbers": numbers,
            }
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

/// 答题作答：题目校验通过后删除题目并入队一条普通用户消息。
///
/// 删题与入队放在同一个 SQLite 事务里：入队失败时事务回滚，题目不会被吃掉；
/// 提交成功后才真正触发调度。
fn question_answer_inner(
    state: &AppState,
    conversation_id: &str,
    answers: Vec<QuestionAnswerInput>,
) -> Result<QuestionAnswerResult, String> {
    let conversation_id = conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("会话 ID 不能为空".to_string());
    }
    if answers.is_empty() {
        return Err("没有要提交的答案".to_string());
    }
    let meta = conversation_service_v2()
        .get_conversation_meta(state, &conversation_id)
        .map_err(|_| format!("会话不存在：{conversation_id}"))?;
    if !conversation_kind_is_local_main_chat(&meta.conversation_kind) {
        return Err("只有本地主会话可以回答问题".to_string());
    }

    let mut wanted: Vec<(u32, usize)> = Vec::new();
    for answer in &answers {
        if answer.number == 0 {
            return Err("问题编号从 1 开始，0 不是合法编号".to_string());
        }
        let option_index = answer
            .option_index
            .ok_or_else(|| format!("#{} 缺少选项：请选择一个选项后再提交", answer.number))?;
        if wanted.iter().any(|(number, _)| *number == answer.number) {
            return Err(format!("#{} 在同一次提交里出现了多次", answer.number));
        }
        wanted.push((answer.number, option_index));
    }
    wanted.sort_by_key(|(number, _)| *number);

    let mut conn = question_store_open(&state.data_path)?;
    let tx = conn
        .transaction()
        .map_err(|err| format!("开启答题事务失败：{err}"))?;

    let mut blocks = Vec::with_capacity(wanted.len());
    for (number, option_index) in &wanted {
        let question = question_store_get_by_number(&tx, &conversation_id, *number)?
            .ok_or_else(|| format!("#{number} 不存在（可能已被回答或清理）"))?;
        let label = question.options.get(*option_index).ok_or_else(|| {
            format!(
                "#{number} 没有第 {} 个选项（共 {} 个）",
                option_index + 1,
                question.options.len()
            )
        })?;
        blocks.push(format!(
            "【问题答复 #{number}】{}\n→ {}",
            question.title.trim(),
            label.trim()
        ));
    }
    let message_text = blocks.join("\n\n");
    let answered_numbers = wanted.iter().map(|(number, _)| *number).collect::<Vec<_>>();

    for number in &answered_numbers {
        let deleted = question_store_delete(&tx, &conversation_id, *number)?;
        if deleted == 0 {
            return Err(format!("#{number} 不存在（可能已被回答或清理）"));
        }
    }

    let agent_id = task_agent_from_conversation_for_write(state, &conversation_id)?
        .ok_or_else(|| format!("会话未绑定人格：{conversation_id}"))?;
    let event = ChatPendingEvent {
        id: format!("question-answer-{}", Uuid::new_v4()),
        conversation_id: conversation_id.clone(),
        created_at: now_utc_rfc3339(),
        source: ChatEventSource::User,
        queue_mode: ChatQueueMode::Normal,
        messages: vec![build_question_answer_message(&message_text, &answered_numbers)],
        activate_assistant: true,
        assistant_message_id: None,
        session_info: ChatSessionInfo { agent_id },
        runtime_context: None,
        sender_info: None,
    };
    let ingress = ingress_chat_event(state, event)?;
    tx.commit()
        .map_err(|err| format!("提交答题事务失败：{err}"))?;
    trigger_chat_event_after_ingress(state, ingress);

    let remaining_items = question_store_list(&conn, &conversation_id)?;
    let remaining = remaining_items
        .iter()
        .map(ConversationQuestion::to_view)
        .collect::<Vec<_>>();
    question_emit_items_updated(state, &conversation_id, &remaining);
    Ok(QuestionAnswerResult {
        answered_numbers,
        remaining,
        message_text,
    })
}

#[tauri::command]
fn question_answer(
    state: State<'_, AppState>,
    conversation_id: String,
    answers: Vec<QuestionAnswerInput>,
) -> Result<QuestionAnswerResult, String> {
    question_answer_inner(state.inner(), &conversation_id, answers)
}

#[tauri::command]
fn question_list(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<ConversationQuestionView>, String> {
    question_views_for_conversation(state.inner(), &conversation_id)
}
