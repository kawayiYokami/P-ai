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
}

/// Web/IDE 侧转发用的参数形状（jsonrpc 侧与桌面端命令同构）。
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

/// 校验一条入参的形状：给了选项 + 给了文本 → 歧义；只给文本 → 长度上限；都不给 → 空答案。
///
/// 这里只看入参本身：选项下标是否有这道题里存在，不查——答题就是按编号删题。
fn question_validate_answer_input(answer: &QuestionAnswerInput) -> Result<(), String> {
    let custom = answer.custom_text.as_deref().map(str::trim).unwrap_or("");
    let has_custom = !custom.is_empty();
    match (answer.option_index, has_custom) {
        (Some(_), true) => Err(format!(
            "#{} 同时提供了选项和自己写的答案：只能二选一",
            answer.number
        )),
        (Some(_), false) => Ok(()),
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
            Ok(())
        }
    }
}

/// 答题作答：只校验答案形状，再把已答的题从快照里去掉。
///
/// 这里**不**产生消息：既不入队，也不拼答复正文。答复是一条用户消息，
/// 它的唯一产生路径是前端 `sendChat`（生成消息、乐观插入、入队、激活轮次）；
/// 后端自己造一条 `ChatPendingEvent` 就是绕开那条通道另开旁路，
/// 而替前端拼正文则会把发送卡在这次写操作的往返之后，两样都不做。
fn question_answer_inner(
    state: &AppState,
    conversation_id: &str,
    answers: Vec<QuestionAnswerInput>,
) -> Result<QuestionAnswerResult, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("会话 ID 不能为空".to_string());
    }
    if answers.is_empty() {
        return Err("没有要提交的答案".to_string());
    }

    let mut answered_numbers: Vec<u32> = Vec::new();
    for answer in &answers {
        if answer.number == 0 {
            return Err("问题编号从 1 开始，0 不是合法编号".to_string());
        }
        question_validate_answer_input(answer)?;
        if answered_numbers.contains(&answer.number) {
            return Err(format!("#{} 在同一次提交里出现了多次", answer.number));
        }
        answered_numbers.push(answer.number);
    }
    answered_numbers.sort_unstable();

    let remaining = question_remove_numbers(state, conversation_id, &answered_numbers)?;
    Ok(QuestionAnswerResult {
        answered_numbers,
        remaining,
    })
}

#[tauri::command]
async fn question_answer(
    state: State<'_, AppState>,
    conversation_id: String,
    answers: Vec<QuestionAnswerInput>,
) -> Result<QuestionAnswerResult, String> {
    question_answer_inner(state.inner(), &conversation_id, answers)
}

/// 拒绝回答问题：只删题，不产生任何会话消息。
///
/// 与答题的边界在这里：答题要由前端把答复当一条普通用户消息发出去，命令本身同样
/// 只删题、不碰发送链路；拒绝连待发正文都没有，区别仅在用户侧有没有那次发送。
fn question_reject_inner(
    state: &AppState,
    conversation_id: &str,
    number: u32,
) -> Result<Vec<ConversationQuestionView>, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("会话 ID 不能为空".to_string());
    }
    if number == 0 {
        return Err("问题编号从 1 开始，0 不是合法编号".to_string());
    }
    question_remove_numbers(state, conversation_id, &[number])
}

#[tauri::command]
async fn question_reject(
    state: State<'_, AppState>,
    conversation_id: String,
    number: u32,
) -> Result<Vec<ConversationQuestionView>, String> {
    question_reject_inner(state.inner(), &conversation_id, number)
}
