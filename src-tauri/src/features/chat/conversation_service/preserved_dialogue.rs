const PRESERVED_DIALOGUE_READ_PAGE_SIZE: usize = 32;
/// 保留对话单条消息的正文上限（字节）。行首说话人不计入这个预算。
const PRESERVED_MESSAGE_MAX_BYTES: usize = 1024;
/// 超限时留给前段的百分比，剩下全给后段。
const PRESERVED_HEAD_PERCENT: usize = 30;
const ACTIVE_COMPACTION_PRESERVED_DIALOGUE_BUDGET: PreservedDialogueBudget =
    PreservedDialogueBudget::Kib(26);
#[allow(dead_code)]
const EVALUATION_COMPACTION_PRESERVED_DIALOGUE_BUDGET: PreservedDialogueBudget =
    PreservedDialogueBudget::Tokens(10_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreservedDialogueBudget {
    Tokens(usize),
    Kib(usize),
}

impl PreservedDialogueBudget {
    fn label(self) -> &'static str {
        match self {
            Self::Tokens(_) => "tokens",
            Self::Kib(_) => "kib",
        }
    }

    fn limit(self) -> usize {
        match self {
            Self::Tokens(tokens) => tokens,
            Self::Kib(kib) => kib.saturating_mul(1024),
        }
    }

    fn measure(self, text: &str) -> usize {
        match self {
            Self::Tokens(_) => estimated_tokens_for_text(text).ceil() as usize,
            Self::Kib(_) => text.len(),
        }
    }

    fn truncate(self, text: &str, remaining: usize) -> String {
        match self {
            Self::Tokens(_) => truncate_text_to_token_limit(text, remaining),
            Self::Kib(_) => truncate_text_to_utf8_byte_limit(text, remaining),
        }
    }
}

fn truncate_text_to_utf8_byte_limit(text: &str, byte_limit: usize) -> String {
    if text.len() <= byte_limit {
        return text.to_string();
    }
    let mut end = byte_limit.min(text.len());
    while end > 0 && !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    text[..end].to_string()
}

struct PreservedDialogueAccumulator {
    budget: PreservedDialogueBudget,
    consumed: usize,
    newest_first: Vec<String>,
}

impl PreservedDialogueAccumulator {
    fn new(budget: PreservedDialogueBudget) -> Self {
        Self {
            budget,
            consumed: 0,
            newest_first: Vec::new(),
        }
    }

    fn is_full(&self) -> bool {
        self.consumed >= self.budget.limit()
    }

    /// 按“最新到更早”的顺序加入一项。返回 true 表示预算已满，不应继续向前读取。
    fn push(&mut self, text: &str) -> bool {
        let text = text.trim();
        if text.is_empty() || self.is_full() {
            return self.is_full();
        }
        let separator_cost = if self.newest_first.is_empty() {
            0
        } else {
            self.budget.measure("\n")
        };
        let remaining = self.budget.limit().saturating_sub(self.consumed);
        if remaining <= separator_cost {
            self.consumed = self.budget.limit();
            return true;
        }
        let text_cost = self.budget.measure(text);
        if text_cost <= remaining.saturating_sub(separator_cost) {
            self.consumed = self
                .consumed
                .saturating_add(separator_cost)
                .saturating_add(text_cost);
            self.newest_first.push(text.to_string());
            return self.is_full();
        }

        let truncated = self
            .budget
            .truncate(text, remaining.saturating_sub(separator_cost));
        if !truncated.trim().is_empty() {
            self.consumed = self
                .consumed
                .saturating_add(separator_cost)
                .saturating_add(self.budget.measure(&truncated));
            self.newest_first.push(truncated);
        }
        true
    }

    fn finish(mut self) -> String {
        self.newest_first.reverse();
        self.newest_first.join("\n")
    }
}

fn compaction_preserved_dialogue_section(message: &ChatMessage) -> Option<String> {
    if !is_context_compaction_message(message, message.role.trim()) {
        return None;
    }
    let text = render_prompt_message_text(message)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut lines = text.lines();
    for line in lines.by_ref() {
        if line.trim() != "## 保留对话" {
            continue;
        }
        let preserved = lines.collect::<Vec<_>>().join("\n");
        let preserved = preserved.trim();
        if preserved.is_empty() || preserved == "（暂无保留对话）" {
            return None;
        }
        return Some(preserved.to_string());
    }
    None
}

// 保留对话行首说话人按消息自身的 speaker_agent_id 解析，落空才用会话主人格名。
// 行首此前一律贴主人格，会把组里其他人格的发言记到主人格名下。
fn resolve_preserved_assistant_speaker<'a>(
    speaker_agent_id: Option<&str>,
    agent_names: &'a std::collections::HashMap<String, String>,
    fallback_assistant_name: &'a str,
) -> &'a str {
    speaker_agent_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .and_then(|id| agent_names.get(id))
        .map(String::as_str)
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(fallback_assistant_name)
}

/// 保留对话单条消息压缩：先按来源分派，再降级 markdown 结构，最后按预算头尾保留。
/// 纯函数：不读存储、不碰 AppState，字符串进、字符串出，便于单测。
///
/// 分工边界：
/// - `task_trigger` / `goal_continue` 是系统塞进来的调度与续跑提示，整条压成一句；
/// - 其余（普通用户与助手发言、团队汇报消息）一视同仁，只做结构降级与 1KiB 头尾保留，
///   团队汇报正文是队员真实汇报，不特殊处理、不压成一句。
fn compact_preserved_message(message: &ChatMessage, rendered: &str) -> String {
    match preserved_message_kind(message) {
        Some("task_trigger") => compact_task_trigger_message(rendered),
        Some("goal_continue") => compact_goal_continue_message(message),
        _ => truncate_preserved_message_head_tail(&downgrade_preserved_message_markdown(rendered)),
    }
}

fn preserved_message_kind(message: &ChatMessage) -> Option<&str> {
    message
        .provider_meta
        .as_ref()?
        .get("messageKind")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
}

/// `［定时任务触发］目标：<目标前 120 字符>`。
/// 目标取正文里 `目标：` 那一行；取不到就退回正文前 120 字符。
fn compact_task_trigger_message(rendered: &str) -> String {
    let goal = rendered
        .lines()
        .find_map(|line| line.trim().strip_prefix("目标："))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(take_chars)
        .unwrap_or_else(|| take_chars(rendered.trim()));
    format!("［定时任务触发］目标：{goal}")
}

/// `［目标续跑］目标：<objective 前 120 字符>（状态 <status>）`。
/// 字段取不到时整段不输出，不制造空括号。
fn compact_goal_continue_message(message: &ChatMessage) -> String {
    let meta = message.provider_meta.as_ref();
    let meta_field = |key: &str| {
        meta.and_then(|value| value.get(key))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let mut out = String::from("［目标续跑］");
    if let Some(objective) = meta_field("objective") {
        out.push_str("目标：");
        out.push_str(&take_chars(objective));
    }
    if let Some(status) = meta_field("status") {
        out.push_str("（状态 ");
        out.push_str(status);
        out.push('）');
    }
    out
}

fn take_chars(text: &str) -> String {
    text.chars().take(120).collect()
}

/// 结构降级：围栏代码块 → 表格 → 行内 code 之外的装饰标记 → 连续空行。
/// 标题 `#`、列表 `-` / `1.`、引用 `>`、行内 code 的反引号与内容一律保留。
fn downgrade_preserved_message_markdown(text: &str) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let mut out = Vec::<String>::with_capacity(lines.len());
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index];
        if let Some(language) = line.trim_start().strip_prefix("```") {
            let language = language.trim().to_ascii_lowercase();
            let mut body_lines = 0usize;
            let mut end = index + 1;
            while end < lines.len() && !lines[end].trim_start().starts_with("```") {
                body_lines += 1;
                end += 1;
            }
            if language.starts_with("mermaid") {
                out.push("［mermaid 图，已省略］".to_string());
            } else {
                out.push(format!("［代码块 {body_lines} 行，已省略］"));
            }
            // 未闭合的围栏吃到文末为止。
            index = if end < lines.len() { end + 1 } else { end };
            continue;
        }
        if is_preserved_table_row(line) {
            let mut end = index;
            while end < lines.len() && is_preserved_table_row(lines[end]) {
                end += 1;
            }
            if end - index >= 2 {
                out.push(format!("［表格 {} 行，已省略］", end - index));
                index = end;
                continue;
            }
        }
        out.push(strip_preserved_decoration(line));
        index += 1;
    }
    collapse_preserved_blank_lines(out).join("\n")
}

/// 表格行：首尾都是 `|`。只认这种形状，避免误伤普通含管道符的句子。
fn is_preserved_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.len() >= 2 && trimmed.starts_with('|') && trimmed.ends_with('|')
}

/// 删除 `**` / `__` / `~~`，但反引号围起来的行内 code 原样保留。
fn strip_preserved_decoration(line: &str) -> String {
    let parts = line.split('`').collect::<Vec<_>>();
    let mut out = String::with_capacity(line.len());
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            out.push('`');
        }
        if index % 2 == 1 {
            out.push_str(part);
        } else {
            out.push_str(&part.replace("**", "").replace("__", "").replace("~~", ""));
        }
    }
    out
}

fn collapse_preserved_blank_lines(lines: Vec<String>) -> Vec<String> {
    let mut out = Vec::<String>::with_capacity(lines.len());
    for line in lines {
        let is_blank = line.trim().is_empty();
        if is_blank && out.last().map(|prev| prev.trim().is_empty()).unwrap_or(true) {
            continue;
        }
        out.push(line);
    }
    while out.last().map(|line| line.trim().is_empty()).unwrap_or(false) {
        out.pop();
    }
    out
}

/// 超 1KiB 的头尾保留：前段固定 `PRESERVED_HEAD_PERCENT`%，后段吃掉剩余预算，
/// 中间标记真实丢掉的字节数。切点收在 `char` 边界，且不让反引号被切一半。
fn truncate_preserved_message_head_tail(text: &str) -> String {
    if text.len() <= PRESERVED_MESSAGE_MAX_BYTES {
        return text.to_string();
    }
    let total = text.len();
    let head_limit = PRESERVED_MESSAGE_MAX_BYTES * PRESERVED_HEAD_PERCENT / 100;
    let head_end = retreat_past_unclosed_inline_code(
        text,
        retreat_to_char_boundary(text, head_limit.min(total)),
    );
    let head = &text[..head_end];

    // 标记里的字节数依赖后段起点，后段起点又依赖标记长度，迭代几次收敛。
    let mut dropped = total - head_end;
    for _ in 0..8 {
        let marker = format!("［省略 {dropped} 字节］");
        let tail_budget = PRESERVED_MESSAGE_MAX_BYTES.saturating_sub(head.len() + marker.len());
        let tail_start = advance_past_unclosed_inline_code(
            text,
            advance_to_char_boundary(text, total.saturating_sub(tail_budget)),
        );
        let next_dropped = tail_start.saturating_sub(head_end);
        if next_dropped == dropped {
            return format!("{head}{marker}{}", &text[tail_start..]);
        }
        dropped = next_dropped;
    }
    let marker = format!("［省略 {dropped} 字节］");
    let tail_budget = PRESERVED_MESSAGE_MAX_BYTES.saturating_sub(head.len() + marker.len());
    let tail_start = advance_past_unclosed_inline_code(
        text,
        advance_to_char_boundary(text, total.saturating_sub(tail_budget)),
    );
    format!("{head}{marker}{}", &text[tail_start..])
}

fn retreat_to_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn advance_to_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

/// 前段若停在一段行内 code 中间（反引号个数为奇），整段回退到该反引号之前。
fn retreat_past_unclosed_inline_code(text: &str, end: usize) -> usize {
    let slice = &text[..end];
    if slice.matches('`').count() % 2 == 1 {
        if let Some(position) = slice.rfind('`') {
            return position;
        }
    }
    end
}

/// 后段若从一段行内 code 中间开始（反引号个数为奇），越过那个反引号。
fn advance_past_unclosed_inline_code(text: &str, start: usize) -> usize {
    let slice = &text[start..];
    if slice.matches('`').count() % 2 == 1 {
        if let Some(position) = slice.find('`') {
            return start + position + 1;
        }
    }
    start
}

fn preserved_dialogue_message_line(
    message: &ChatMessage,
    user_alias: &str,
    assistant_name: &str,
) -> Option<String> {
    let role = message.role.trim();
    if !matches!(role, "user" | "assistant") {
        return None;
    }
    let raw_text = preserved_conversation_message_backend_text(message);
    // 压缩必须在 `clean_text` 之前：它会把换行压成空格，围栏代码块与表格到那一步就认不出来了。
    // 行首说话人不进 1KiB 预算，所以这里只压正文，`speaker` 后面再拼。
    let compacted = compact_preserved_message(message, raw_text.trim());
    let text = clean_text(&compacted);
    if text.is_empty() {
        return None;
    }
    let speaker = if role.eq_ignore_ascii_case("assistant") {
        assistant_name.trim()
    } else if let Some(remote_speaker_label) = remote_im_sender_display_name(message)
        .map(|label| label.trim().to_string())
        .filter(|label| !label.is_empty())
    {
        return Some(format!("{remote_speaker_label}：{text}"));
    } else {
        user_alias.trim()
    };
    let speaker = if speaker.is_empty() {
        if role.eq_ignore_ascii_case("assistant") {
            "助手"
        } else {
            "用户"
        }
    } else {
        speaker
    };
    Some(format!("{speaker}：{text}"))
}

#[cfg(test)]
fn collect_block_preserved_dialogue(
    messages: &[ChatMessage],
    user_alias: &str,
    assistant_name: &str,
    budget: PreservedDialogueBudget,
) -> String {
    let mut accumulator = PreservedDialogueAccumulator::new(budget);
    for message in messages.iter().rev() {
        if is_context_compaction_message(message, message.role.trim()) {
            if let Some(previous) = compaction_preserved_dialogue_section(message) {
                for line in previous.lines().rev() {
                    if accumulator.push(line) {
                        break;
                    }
                }
            }
            break;
        }
        let Some(line) = preserved_dialogue_message_line(message, user_alias, assistant_name)
        else {
            continue;
        };
        if accumulator.push(&line) {
            break;
        }
    }
    accumulator.finish()
}

impl ConversationServiceV2 {
    fn read_block_preserved_dialogue(
        &self,
        state: &AppState,
        conversation_id: &str,
        requested_block_id: Option<u32>,
        end_message_id: Option<&str>,
        user_alias: &str,
        assistant_name: &str,
        budget: PreservedDialogueBudget,
    ) -> Result<String, String> {
        let conversation_id = conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("读取 block 保留对话失败：缺少会话 ID".to_string());
        }
        if budget.limit() == 0 {
            return Ok(String::new());
        }
        let store_paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
        ensure_chat_store_conversation_readable(state, conversation_id, &store_paths)?;
        let mut selected_block_id = requested_block_id;
        let mut before_message_id = end_message_id
            .map(str::trim)
            .filter(|message_id| !message_id.is_empty())
            .map(ToOwned::to_owned);
        let mut accumulator = PreservedDialogueAccumulator::new(budget);
        let mut read_message_count = 0usize;
        let mut reached_summary = false;
        // 人格 id→名映射：行首说话人按消息自身解析，读不到映射时后面按主人格名兜底。
        let assistant_names_by_id: std::collections::HashMap<String, String> =
            state_read_agents_cached(state)
                .map(|agents| {
                    agents
                        .into_iter()
                        .map(|agent| {
                            (
                                agent.id.trim().to_string(),
                                agent.name.trim().to_string(),
                            )
                        })
                        .filter(|(id, name)| !id.is_empty() && !name.is_empty())
                        .collect()
                })
                .unwrap_or_default();

        loop {
            let Some(page) = message_store::chat_store_read_block_messages_before(
                &store_paths,
                selected_block_id,
                before_message_id.as_deref(),
                PRESERVED_DIALOGUE_READ_PAGE_SIZE,
            )?
            else {
                break;
            };
            selected_block_id = Some(page.selected_block_id);
            if page.messages.is_empty() {
                break;
            }
            read_message_count = read_message_count.saturating_add(page.messages.len());
            let next_before_message_id = page.messages.first().map(|message| message.id.clone());
            for message in page.messages.iter().rev() {
                if is_context_compaction_message(message, message.role.trim()) {
                    reached_summary = true;
                    if let Some(previous) = compaction_preserved_dialogue_section(message) {
                        for line in previous.lines().rev() {
                            if accumulator.push(line) {
                                break;
                            }
                        }
                    }
                    break;
                }
                let line_assistant_name = if message.role.trim().eq_ignore_ascii_case("assistant")
                {
                    resolve_preserved_assistant_speaker(
                        message.speaker_agent_id.as_deref(),
                        &assistant_names_by_id,
                        assistant_name,
                    )
                    .to_string()
                } else {
                    assistant_name.to_string()
                };
                let Some(line) =
                    preserved_dialogue_message_line(message, user_alias, &line_assistant_name)
                else {
                    continue;
                };
                if accumulator.push(&line) {
                    break;
                }
            }
            if accumulator.is_full() || reached_summary || !page.has_more {
                break;
            }
            let Some(next_before_message_id) = next_before_message_id else {
                break;
            };
            before_message_id = Some(next_before_message_id);
        }

        let preserved = accumulator.finish();
        runtime_log_debug(format!(
            "[上下文整理] 完成，任务=读取block保留对话，conversation_id={}，block_id={}，budget_mode={}，budget_limit={}，result_tokens={}，result_utf8_bytes={}，read_message_count={}，reached_summary={}",
            conversation_id,
            selected_block_id.map(|block_id| block_id.to_string()).unwrap_or_default(),
            budget.label(),
            budget.limit(),
            estimated_tokens_for_text(&preserved).ceil() as usize,
            preserved.len(),
            read_message_count,
            reached_summary
        ));
        Ok(preserved)
    }
}

#[cfg(test)]
mod preserved_dialogue_tests {
    use super::*;

    fn text_message(id: &str, role: &str, text: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: text.to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        }
    }

    fn compaction_message(preserved: &str) -> ChatMessage {
        let mut message = text_message(
            "summary",
            "user",
            format!(
                "## 摘要说明\n\n旧说明\n\n## 摘要正文\n\n绝不能继承\n\n## 保留对话\n\n{preserved}"
            )
            .as_str(),
        );
        message.provider_meta = Some(serde_json::json!({
            "message_meta": { "kind": "context_compaction" }
        }));
        message
    }

    fn assistant_message_with_tool_events(
        id: &str,
        speaker_agent_id: Option<&str>,
        parts_text: &str,
        events: Vec<serde_json::Value>,
    ) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: "assistant".to_string(),
            created_at: now_iso(),
            speaker_agent_id: speaker_agent_id.map(str::to_string),
            parts: vec![MessagePart::Text {
                text: parts_text.to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: if events.is_empty() {
                None
            } else {
                Some(events)
            },
            mcp_call: None,
            meme_annotations: None,
        }
    }

    #[test]
    fn preserved_line_should_strip_toolcall_markers_without_duplicating_tool_event_text() {
        // 2026-10-27 摘要事故形态：中断落盘的 parts 是带标记的展示串，
        // 与 tool_call 事件正文是同一句话。行里只留一句干净正文。
        let message = assistant_message_with_tool_events(
            "assistant-dirty",
            Some("persona-owner"),
            "[toolcall:call_00_aaa]现在把其余位置的「期待」统一换成「意料中」。 [toolcall:call_01_bbb]",
            vec![serde_json::json!({
                "role": "assistant",
                "content": "现在把其余位置的「期待」统一换成「意料中」。",
                "tool_calls": [{"id": "call_00_aaa"}]
            })],
        );

        let line = preserved_dialogue_message_line(&message, "用户", "纳西妲")
            .expect("dirty line");
        assert_eq!(
            line,
            "纳西妲：现在把其余位置的「期待」统一换成「意料中」。"
        );
        assert!(!line.contains("[toolcall:"));
    }

    #[test]
    fn preserved_line_should_keep_final_text_beyond_tool_event_text() {
        // 正常落盘：工具事件是中间说明，parts 是最终结论，两句都要进摘要。
        let message = assistant_message_with_tool_events(
            "assistant-normal",
            Some("persona-owner"),
            "最后汇总",
            vec![
                serde_json::json!({
                    "role": "assistant",
                    "content": "先说明第一步",
                    "tool_calls": [{"id": "call_1"}]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": "文件内容"
                }),
            ],
        );

        let line = preserved_dialogue_message_line(&message, "用户", "纳西妲")
            .expect("normal line");
        assert!(line.contains("先说明第一步"));
        assert!(line.contains("最后汇总"));
        assert!(!line.contains("[toolcall:"));
    }

    #[test]
    fn preserved_line_should_drop_marker_only_parts_text() {
        let message = assistant_message_with_tool_events(
            "assistant-marker-only",
            Some("persona-owner"),
            "[toolcall:call_1] [toolcall:call_2]",
            vec![serde_json::json!({
                "role": "assistant",
                "content": "先说明第一步",
                "tool_calls": [{"id": "call_1"}]
            })],
        );

        let line = preserved_dialogue_message_line(&message, "用户", "纳西妲")
            .expect("marker only line");
        assert_eq!(line, "纳西妲：先说明第一步");
    }

    #[test]
    fn resolve_preserved_assistant_speaker_should_prefer_message_speaker() {
        let mut names = std::collections::HashMap::new();
        names.insert("persona-owner".to_string(), "纳西妲".to_string());
        names.insert("persona-member".to_string(), "队员甲".to_string());

        assert_eq!(
            resolve_preserved_assistant_speaker(
                Some("persona-member"),
                &names,
                "纳西妲"
            ),
            "队员甲"
        );
        assert_eq!(
            resolve_preserved_assistant_speaker(Some("persona-unknown"), &names, "纳西妲"),
            "纳西妲"
        );
        assert_eq!(
            resolve_preserved_assistant_speaker(None, &names, "纳西妲"),
            "纳西妲"
        );
    }

    #[test]
    fn block_preserved_dialogue_should_inherit_only_preserved_section() {
        let messages = vec![
            text_message("old", "user", "不应跨块读取"),
            compaction_message("用户：上一轮保留\n助手：上一轮回答"),
            text_message("current", "user", "当前问题"),
        ];

        let preserved = collect_block_preserved_dialogue(
            &messages,
            "用户",
            "助手",
            PreservedDialogueBudget::Tokens(10_000),
        );

        assert_eq!(
            preserved,
            "用户：上一轮保留\n助手：上一轮回答\n用户：当前问题"
        );
        assert!(!preserved.contains("绝不能继承"));
        assert!(!preserved.contains("不应跨块读取"));
    }

    #[test]
    fn kib_budget_should_truncate_at_utf8_boundary() {
        let mut accumulator = PreservedDialogueAccumulator::new(PreservedDialogueBudget::Kib(1));
        let input = "中".repeat(400);

        assert!(accumulator.push(&input));
        let preserved = accumulator.finish();

        assert!(preserved.len() <= 1024);
        assert_eq!(preserved.chars().count(), 341);
    }

    #[test]
    fn token_budget_should_stop_before_older_messages() {
        let latest = text_message("latest", "assistant", "最新回答");
        let latest_line = preserved_dialogue_message_line(&latest, "用户", "助手")
            .expect("latest line");
        let budget = PreservedDialogueBudget::Tokens(
            estimated_tokens_for_text(&latest_line).ceil() as usize,
        );
        let messages = vec![
            text_message("older", "user", "更早问题"),
            latest,
        ];

        let preserved = collect_block_preserved_dialogue(&messages, "用户", "助手", budget);

        assert_eq!(preserved, latest_line);
        assert!(!preserved.contains("更早问题"));
    }

    #[test]
    fn active_budget_should_use_26_kib() {
        assert_eq!(
            ACTIVE_COMPACTION_PRESERVED_DIALOGUE_BUDGET,
            PreservedDialogueBudget::Kib(26)
        );
        assert_eq!(ACTIVE_COMPACTION_PRESERVED_DIALOGUE_BUDGET.limit(), 26 * 1024);
    }
}

#[cfg(test)]
mod compact_preserved_message_tests {
    use super::*;

    fn message_with_meta(role: &str, text: &str, meta: Option<serde_json::Value>) -> ChatMessage {
        ChatMessage {
            id: "compact-case".to_string(),
            role: role.to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: text.to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: meta,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        }
    }

    fn plain_message(role: &str, text: &str) -> ChatMessage {
        message_with_meta(role, text, None)
    }

    #[test]
    fn short_message_should_only_drop_decoration_markers() {
        let message = plain_message("user", "普通一句**加粗**内容");
        let out = compact_preserved_message(&message, "普通一句**加粗**内容");
        assert_eq!(out, "普通一句加粗内容");
    }

    #[test]
    fn fenced_code_block_should_collapse_with_line_count() {
        let text = "前置\n```rust\nfn main() {}\nlet a = 1;\n```\n后续";
        let message = plain_message("assistant", text);
        let out = compact_preserved_message(&message, text);
        assert!(out.contains("［代码块 2 行，已省略］"), "实际: {out}");
        assert!(!out.contains("fn main"), "代码正文应被折叠");
        assert!(out.contains("前置") && out.contains("后续"));
    }

    #[test]
    fn mermaid_fence_should_collapse_to_diagram_placeholder() {
        let text = "```mermaid\ngraph TD;\nA-->B;\n```";
        let message = plain_message("assistant", text);
        let out = compact_preserved_message(&message, text);
        assert_eq!(out, "［mermaid 图，已省略］");
    }

    #[test]
    fn table_should_collapse_but_plain_pipe_sentence_should_stay() {
        let text = "| 名字 | 值 |\n| --- | --- |\n| 甲 | 1 |\n普通一句含 | 管道符。";
        let message = plain_message("assistant", text);
        let out = compact_preserved_message(&message, text);
        assert!(out.contains("［表格 3 行，已省略］"), "实际: {out}");
        assert!(out.contains("普通一句含 | 管道符。"), "不该误伤: {out}");
    }

    #[test]
    fn decoration_should_be_stripped_outside_inline_code_only() {
        let text = "`a**b` 和 **加粗**";
        let message = plain_message("user", text);
        let out = compact_preserved_message(&message, text);
        assert_eq!(out, "`a**b` 和 加粗");
    }

    #[test]
    fn oversize_message_should_keep_head_and_tail_with_marker() {
        let text = format!("开头内容{}结尾内容", "啊".repeat(600));
        let message = plain_message("user", &text);
        let out = compact_preserved_message(&message, &text);
        assert!(out.contains("开头内容"), "首段必须保留");
        assert!(out.contains("结尾内容"), "尾段必须保留");
        assert!(out.contains("［省略 "), "必须有省略标记: {out}");
        assert!(out.contains(" 字节］"));
        assert!(out.len() <= PRESERVED_MESSAGE_MAX_BYTES, "不得超过上限: {}", out.len());
        assert!(out.len() > 900, "应尽量吃满预算: {}", out.len());
    }

    #[test]
    fn cjk_and_emoji_truncation_should_not_split_characters() {
        let text = format!("表情开头{}{}", "😀".repeat(300), "尾部标记");
        let message = plain_message("user", &text);
        let out = compact_preserved_message(&message, &text);
        assert!(!out.contains('\u{FFFD}'), "不能出现替换字符");
        assert!(out.chars().count() > 0);
        assert!(out.len() <= PRESERVED_MESSAGE_MAX_BYTES);
        // 逐字符能走完就说明没有落在字符中间（&str 本身保证，但这里显式过一遍）。
        let rebuilt = out.chars().collect::<String>();
        assert_eq!(rebuilt, out);
    }

    #[test]
    fn task_trigger_should_collapse_to_one_sentence_with_goal() {
        let rendered = "<task_remind>\n背景：例行巡检\n目标：修复登录问题\n要求：跑测试\n</task_remind>";
        let message = message_with_meta(
            "system",
            rendered,
            Some(serde_json::json!({ "messageKind": "task_trigger" })),
        );
        let out = compact_preserved_message(&message, rendered);
        assert_eq!(out, "［定时任务触发］目标：修复登录问题");
        assert!(out.len() < rendered.len());
    }

    #[test]
    fn goal_continue_should_collapse_to_one_sentence_with_status() {
        let rendered = "继续完成目标";
        let message = message_with_meta(
            "assistant",
            rendered,
            Some(serde_json::json!({
                "messageKind": "goal_continue",
                "objective": "把测试跑通",
                "status": "active"
            })),
        );
        let out = compact_preserved_message(&message, rendered);
        assert!(out.contains("［目标续跑］"));
        assert!(out.contains("目标：把测试跑通"));
        assert!(out.contains("（状态 active）"));

        let missing = message_with_meta(
            "assistant",
            rendered,
            Some(serde_json::json!({ "messageKind": "goal_continue" })),
        );
        let out = compact_preserved_message(&missing, rendered);
        assert_eq!(out, "［目标续跑］", "字段缺失时不能留空括号: {out}");
        assert!(!out.contains("（）"));
    }

    #[test]
    fn team_report_message_should_follow_structural_path_not_sentence_collapse() {
        let text = "**汇报**\n\n```\ncargo check\n```\n\n| 结论 | 值 |\n| --- | --- |\n完成并通过。";
        let message = message_with_meta(
            "user",
            text,
            Some(serde_json::json!({ "messageKind": "team_message" })),
        );
        let out = compact_preserved_message(&message, text);
        assert!(!out.starts_with("［定时任务触发］"));
        assert!(!out.starts_with("［目标续跑］"));
        assert!(out.contains("［代码块 1 行，已省略］"), "实际: {out}");
        assert!(out.contains("［表格 2 行，已省略］"), "实际: {out}");
        assert!(out.contains("汇报"), "装饰标记去掉但正文要留");
        assert!(out.contains("完成并通过。"));
    }

    #[test]
    fn speaker_name_should_not_count_toward_message_budget() {
        let body = "字".repeat(500);
        let message = plain_message("assistant", &body);
        let line = preserved_dialogue_message_line(&message, "用户", "纳西妲").expect("line");
        assert!(line.starts_with("纳西妲："), "说话人必须完整保留: {line}");
        let body_part = line.trim_start_matches("纳西妲：");
        assert!(body_part.len() <= PRESERVED_MESSAGE_MAX_BYTES);
        assert!(body_part.contains("［省略 "));
        assert!(body_part.len() > 900, "正文预算应被吃满: {}", body_part.len());
    }
}
