// ==================== 会话提问：答题命令 ====================

/// 单选答案：`optionIndex` 是 `ConversationQuestion.options` 的下标。
///
/// `optionIndex` 与 `customText` 二选一：前者是点给定选项，后者是用户自己写一段。
/// 两个都给视为歧义直接报错，都不给视为空答案。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuestionAnswerInput {
    number: u32,
    #[serde(default)]
    option_index: Option<usize>,
    #[serde(default)]
    custom_text: Option<String>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuestionRejectWebParams {
    #[serde(default)]
    conversation_id: String,
    #[serde(default)]
    number: u32,
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

/// 一道题的答案来源：点给定选项换成下标，自己写就是一段文本。
enum QuestionAnswerChoice {
    Option(usize),
    Custom(String),
}

/// 把一条入参解释成答案来源。判定写死：给了选项 + 给了文本 → 歧义报错；
/// 只给选项 → 选项；只给文本 → 自定义（空文本与超长都算无效）；都不给 → 空答案。
fn question_answer_choice_from_input(
    answer: &QuestionAnswerInput,
) -> Result<QuestionAnswerChoice, String> {
    let custom = answer.custom_text.as_deref().map(str::trim).unwrap_or("");
    let has_custom = !custom.is_empty();
    match (answer.option_index, has_custom) {
        (Some(_), true) => Err(format!(
            "#{} 同时提供了选项和自己写的答案：只能二选一",
            answer.number
        )),
        (Some(index), false) => Ok(QuestionAnswerChoice::Option(index)),
        (None, false) => Err(format!(
            "#{} 缺少答案：请选择一个选项或自己写一个",
            answer.number
        )),
        (None, true) => {
            if question_char_count(custom) > QUESTION_CUSTOM_ANSWER_MAX_CHARS {
                return Err(format!(
                    "#{} 自己写的答案超过 {} 字上限，请精简后重试",
                    answer.number, QUESTION_CUSTOM_ANSWER_MAX_CHARS
                ));
            }
            Ok(QuestionAnswerChoice::Custom(custom.to_string()))
        }
    }
}

/// 答题作答：题目校验通过后入队一条普通用户消息，再把已答的题从会话元数据里去掉。
///
/// 顺序是「先入队、后删题」：入队失败时题目还在，用户可以再答一次；
/// 反过来先删题，一旦入队失败，题目没了、消息也没发出去，用户无从重试。
///
/// 整段在会话写入门里跑：校验看到的题、入队的那条消息、删题的写入要落在同一个临界区，
/// 否则两次并发提交都拿着同一份快照通过校验，同一道题会被答两遍。
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
    with_conversation_mutation(state, &conversation_id, "question.answer", || {
        question_answer_locked(state, &conversation_id, answers)
    })
}

/// 写入门内的作答主体；调到这里时这条会话的写入已被独占。
fn question_answer_locked(
    state: &AppState,
    conversation_id: &str,
    answers: Vec<QuestionAnswerInput>,
) -> Result<QuestionAnswerResult, String> {
    let meta = conversation_service_v2()
        .get_conversation_meta(state, conversation_id)
        .map_err(|_| format!("会话不存在：{conversation_id}"))?;
    if !conversation_kind_is_local_main_chat(&meta.conversation_kind) {
        return Err("只有本地主会话可以回答问题".to_string());
    }

    let mut wanted: Vec<(u32, QuestionAnswerChoice)> = Vec::new();
    for answer in &answers {
        if answer.number == 0 {
            return Err("问题编号从 1 开始，0 不是合法编号".to_string());
        }
        let choice = question_answer_choice_from_input(answer)?;
        if wanted.iter().any(|(number, _)| *number == answer.number) {
            return Err(format!("#{} 在同一次提交里出现了多次", answer.number));
        }
        wanted.push((answer.number, choice));
    }
    wanted.sort_by_key(|(number, _)| *number);

    let mut blocks = Vec::with_capacity(wanted.len());
    for (number, choice) in &wanted {
        let question = meta
            .questions
            .iter()
            .find(|entry| entry.number == *number)
            .ok_or_else(|| format!("#{number} 不存在（可能已被回答或清理）"))?;
        let answer_text = match choice {
            QuestionAnswerChoice::Option(option_index) => {
                let label = question.options.get(*option_index).ok_or_else(|| {
                    format!(
                        "#{number} 没有第 {} 个选项（共 {} 个）",
                        option_index + 1,
                        question.options.len()
                    )
                })?;
                label.trim().to_string()
            }
            QuestionAnswerChoice::Custom(text) => text.clone(),
        };
        blocks.push(format!("【{}】\n→ {}", question.title.trim(), answer_text));
    }
    let message_text = blocks.join("\n\n");
    let answered_numbers = wanted.iter().map(|(number, _)| *number).collect::<Vec<_>>();

    let agent_id = task_agent_from_conversation_for_write(state, conversation_id)?
        .ok_or_else(|| format!("会话未绑定人格：{conversation_id}"))?;
    let event = ChatPendingEvent {
        id: format!("question-answer-{}", Uuid::new_v4()),
        conversation_id: conversation_id.to_string(),
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

    let mut remaining: Vec<ConversationQuestionView> = Vec::new();
    state_update_conversation_metadata_cached(state, &conversation_id, |conversation| {
        conversation
            .questions
            .retain(|entry| !answered_numbers.contains(&entry.number));
        remaining = conversation.questions.iter().map(question_view).collect();
        Ok(())
    })?;
    trigger_chat_event_after_ingress(state, ingress);

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

/// 拒绝回答问题：只删题，不产生任何会话消息。
///
/// 与答题的边界在这里：答题要构造一条用户消息入队并触发调度，所以先入队再删题；
/// 拒绝不碰入队链路，只要把题从会话元数据里去掉。
fn question_reject_inner(
    state: &AppState,
    conversation_id: &str,
    number: u32,
) -> Result<Vec<ConversationQuestionView>, String> {
    let conversation_id = conversation_id.trim().to_string();
    if conversation_id.is_empty() {
        return Err("会话 ID 不能为空".to_string());
    }
    if number == 0 {
        return Err("问题编号从 1 开始，0 不是合法编号".to_string());
    }
    let meta = conversation_service_v2()
        .get_conversation_meta(state, &conversation_id)
        .map_err(|_| format!("会话不存在：{conversation_id}"))?;
    if !conversation_kind_is_local_main_chat(&meta.conversation_kind) {
        return Err("只有本地主会话可以拒绝回答问题".to_string());
    }

    let mut remaining: Vec<ConversationQuestionView> = Vec::new();
    state_update_conversation_metadata_cached(state, &conversation_id, |conversation| {
        let before = conversation.questions.len();
        conversation.questions.retain(|entry| entry.number != number);
        if conversation.questions.len() == before {
            return Err(format!("#{number} 不存在（可能已被回答或清理）"));
        }
        remaining = conversation.questions.iter().map(question_view).collect();
        Ok(())
    })?;

    question_emit_items_updated(state, &conversation_id, &remaining);
    Ok(remaining)
}

#[tauri::command]
fn question_reject(
    state: State<'_, AppState>,
    conversation_id: String,
    number: u32,
) -> Result<Vec<ConversationQuestionView>, String> {
    question_reject_inner(state.inner(), &conversation_id, number)
}

#[tauri::command]
fn question_list(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<Vec<ConversationQuestionView>, String> {
    question_views_for_conversation(state.inner(), &conversation_id)
}
