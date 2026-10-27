const PRESERVED_DIALOGUE_READ_PAGE_SIZE: usize = 32;
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

fn preserved_dialogue_message_line(
    message: &ChatMessage,
    user_alias: &str,
    assistant_name: &str,
) -> Option<String> {
    let role = message.role.trim();
    if !matches!(role, "user" | "assistant") {
        return None;
    }
    let text = preserved_conversation_message_backend_text(message);
    let text = clean_text(text.trim());
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
