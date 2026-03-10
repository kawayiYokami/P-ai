#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptBuildMode {
    Chat,
    Archive,
}

#[derive(Debug, Clone, Default)]
struct ChatPromptOverrides {
    latest_user_text: Option<String>,
    latest_user_time_iso: Option<String>,
    latest_user_system_blocks: Vec<String>,
    latest_images: Vec<(String, String)>,
    latest_audios: Vec<(String, String)>,
}

fn build_prepared_prompt_for_mode(
    mode: PromptBuildMode,
    conversation: &Conversation,
    agent: &AgentProfile,
    agents: &[AgentProfile],
    departments: &[DepartmentConfig],
    user_name: &str,
    user_intro: &str,
    response_style_id: &str,
    ui_language: &str,
    data_path: Option<&PathBuf>,
    last_archive_summary: Option<&str>,
    terminal_block: Option<String>,
    chat_overrides: Option<ChatPromptOverrides>,
) -> PreparedPrompt {
    match mode {
        PromptBuildMode::Chat => {
            let temp_config = AppConfig {
                hotkey: String::new(),
                ui_language: String::new(),
                ui_font: String::new(),
                record_hotkey: String::new(),
                record_background_wake_enabled: false,
                min_record_seconds: 0,
                max_record_seconds: 0,
                tool_max_iterations: 0,
                selected_api_config_id: String::new(),
                assistant_department_api_config_id: String::new(),
                vision_api_config_id: None,
                stt_api_config_id: None,
                stt_auto_send: false,
                shell_workspaces: Vec::new(),
                mcp_servers: Vec::new(),
                departments: departments.to_vec(),
                api_configs: Vec::new(),
            };
            let include_archive_recap = department_for_agent_id(&temp_config, &agent.id)
            .map(|department| department.id == ASSISTANT_DEPARTMENT_ID || department.is_built_in_assistant)
            .unwrap_or(false);
            let mut prepared = build_prompt(
                conversation,
                agent,
                agents,
                departments,
                user_name,
                user_intro,
                response_style_id,
                ui_language,
                data_path,
            );
            prepared = enrich_prepared_prompt_with_common_preamble(
                prepared,
                if include_archive_recap {
                    last_archive_summary
                } else {
                    None
                },
                terminal_block,
            );
            if let Some(overrides) = chat_overrides {
                let latest_user_text = overrides
                    .latest_user_text
                    .unwrap_or_else(|| prepared.latest_user_text.clone());
                let latest_user_time_iso = overrides.latest_user_time_iso.unwrap_or_default();
                apply_chat_latest_user_payload(
                    &mut prepared,
                    latest_user_text,
                    &latest_user_time_iso,
                    &overrides.latest_user_system_blocks,
                    overrides.latest_images,
                    overrides.latest_audios,
                );
            }
            prepared
        }
        PromptBuildMode::Archive => PreparedPrompt {
            // Keep system/preamble stable for archive mode.
            preamble: "你是一个严格遵循用户指令的助手。".to_string(),
            history_messages: build_archive_history_messages(conversation),
            latest_user_text: String::new(),
            latest_user_time_text: String::new(),
            latest_user_system_text: String::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        },
    }
}

fn build_archive_history_messages(source_conversation: &Conversation) -> Vec<PreparedHistoryMessage> {
    source_conversation
        .messages
        .iter()
        .map(|msg| {
            let mut text = archive_message_plain_text(msg);
            if text.trim().is_empty() {
                text = render_message_for_context(msg);
            }
            if msg.role == "user" {
                text = strip_archive_role_prefix(&text, "USER:");
            } else if msg.role == "assistant" {
                text = strip_archive_role_prefix(&text, "ASSISTANT:");
            }
            let reasoning_content = msg
                .provider_meta
                .as_ref()
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("reasoningStandard").and_then(Value::as_str))
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned);
            PreparedHistoryMessage {
                role: msg.role.clone(),
                text,
                user_time_text: if msg.role == "user" {
                    Some(format_message_time_text(&msg.created_at))
                } else {
                    None
                },
                tool_calls: msg
                    .tool_call
                    .as_ref()
                    .map(|events| sanitize_tool_history_events(events)),
                tool_call_id: None,
                reasoning_content,
            }
        })
        .collect::<Vec<_>>()
}

fn strip_archive_role_prefix(text: &str, prefix: &str) -> String {
    let trimmed = text.trim_start();
    let lower_text = trimmed.to_ascii_lowercase();
    let lower_prefix = prefix.to_ascii_lowercase();
    if lower_text.starts_with(&lower_prefix) {
        let rest = &trimmed[prefix.len()..];
        return rest.trim_start().to_string();
    }
    text.to_string()
}

fn archive_used_memories_block(memories: &[MemoryEntry], recall_table: &[String]) -> String {
    if recall_table.is_empty() {
        return "（无）".to_string();
    }
    let mut seen = HashSet::<String>::new();
    let memory_map = memories
        .iter()
        .map(|m| (m.id.clone(), m))
        .collect::<HashMap<String, &MemoryEntry>>();
    let mut lines = Vec::<String>::new();
    for memory_id in recall_table
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
    {
        if !seen.insert(memory_id.to_string()) {
            continue;
        }
        if let Some(memory) = memory_map.get(memory_id) {
            lines.push(format!(
                "<{}>\n判断：{}\n理由：{}\n</{}>",
                memory_id,
                clean_text(memory.judgment.trim()),
                clean_text(memory.reasoning.trim()),
                memory_id
            ));
        } else {
            lines.push(format!("<{}>\n判断：\n理由：\n</{}>", memory_id, memory_id));
        }
    }
    if lines.is_empty() {
        "（无）".to_string()
    } else {
        lines.join("\n")
    }
}

fn archive_example_output_block() -> &'static str {
    r###"{
  "summary": "## Current Progress\n- 已完成事项（仅已落地）\n\n## Current State\n- 当前代码/工作区状态（是否已提交、是否有未提交改动）\n\n## User Decisions\n- 用户已明确同意/否决的决策\n\n## Open Issue (Root Cause)\n- 现存问题（含根因）\n\n## What Changed\n- 本轮实际改动（无则写“无”）\n\n## What Remains\n1. 下一步动作1\n2. 下一步动作2\n\n## Constraints / Preferences\n- 用户偏好与约束\n\n## Quick References\n- 关键文件路径或关键对象",
  "usefulMemoryIds": ["string"],
  "newMemories": [
    {
      "memoryType": "knowledge|skill|emotion|event",
      "judgment": "string",
      "reasoning": "string",
      "tags": ["string"]
    }
  ],
  "mergeGroups": [
    {
      "sourceIds": ["string", "string"],
      "target": {
        "memoryType": "knowledge|skill|emotion|event",
        "judgment": "string",
        "reasoning": "string",
        "tags": ["string"]
      }
    }
  ]
}"###
}

fn enrich_prepared_prompt_with_common_preamble(
    mut prepared: PreparedPrompt,
    last_archive_summary: Option<&str>,
    terminal_block: Option<String>,
) -> PreparedPrompt {
    if let Some(summary) = last_archive_summary {
        let summary = summary.trim();
        if !summary.is_empty() {
            prepared.preamble.push_str(
                "\n[HIDDEN ARCHIVE RECAP]\nUSER: 上次我们聊到哪里？\nASSISTANT: ",
            );
            prepared.preamble.push_str(summary);
            prepared.preamble.push('\n');
        }
    }
    if let Some(block) = terminal_block {
        if !block.trim().is_empty() {
            prepared.preamble.push('\n');
            prepared.preamble.push_str(&block);
            prepared.preamble.push('\n');
        }
    }
    prepared
}

fn apply_chat_latest_user_payload(
    prepared: &mut PreparedPrompt,
    latest_user_text: String,
    latest_user_time_iso: &str,
    latest_user_system_blocks: &[String],
    latest_images: Vec<(String, String)>,
    latest_audios: Vec<(String, String)>,
) {
    prepared.latest_user_text = latest_user_text;
    prepared.latest_user_time_text = format_message_time_rfc3339_local(latest_user_time_iso);
    prepared.latest_user_system_text = latest_user_system_blocks.join("\n\n");
    prepared.latest_images = latest_images;
    prepared.latest_audios = latest_audios;
}

