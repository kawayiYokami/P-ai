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
    /// 按题目拼好的答复正文。前端拿它走正常发送通道发出去；
    /// 后端不自己入队，避免绕开用户发送的那条唯一路径。
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

/// 答题作答：校验题目、拼出答复正文，再把已答的题从会话元数据里去掉。
///
/// 这里**不**入队任何消息：答复必须走用户发送的那一条唯一通道（前端 `sendChat`），
/// 由它统一生成消息、乐观插入、入队、激活轮次。后端自己造一条 `ChatPendingEvent`
/// 等于绕开那条通道另开一条旁路，前端还得靠 id 回灌草稿去对齐，属于生造通道。
///
/// 顺序是「先删题、后交回正文」：删题与校验落在同一个写入门里，两次并发提交不会
/// 拿同一份快照把同一道题答两遍；回给前端的正文由前端立刻发出。
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

    let mut remaining: Vec<ConversationQuestionView> = Vec::new();
    state_update_conversation_metadata_cached(state, &conversation_id, |conversation| {
        conversation
            .questions
            .retain(|entry| !answered_numbers.contains(&entry.number));
        remaining = conversation.questions.iter().map(question_view).collect();
        Ok(())
    })?;

    question_emit_items_updated(state, &conversation_id, &remaining);
    Ok(QuestionAnswerResult {
        answered_numbers,
        remaining,
        message_text,
    })
}

#[tauri::command]
async fn question_answer(
    state: State<'_, AppState>,
    conversation_id: String,
    answers: Vec<QuestionAnswerInput>,
) -> Result<QuestionAnswerResult, String> {
    // 里面做会话写锁、ingress 三把锁与元数据落盘，同步跑会阻塞主线程冻结界面。
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        question_answer_inner(&app_state, &conversation_id, answers)
    })
    .await
    .map_err(|error| format!("答题提交任务异常：{error}"))?
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
