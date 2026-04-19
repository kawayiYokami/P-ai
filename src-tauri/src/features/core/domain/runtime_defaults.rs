fn format_message_time_rfc3339_local(raw: &str) -> String {
    format_utc_storage_time_to_local_rfc3339(raw)
}

fn format_message_time_text(raw: &str) -> String {
    format_utc_storage_time_to_local_text(raw)
}

fn default_agent() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: DEFAULT_AGENT_ID.to_string(),
        name: "助理".to_string(),
        system_prompt: "你是一个耐心、友善的助理。请用短信聊天的口吻与用户交流，优先自然、简短、有人味的表达。除非用户明确要求，否则不要使用结构化输出（如分点、表格、章节）和过度正式语气。面对截图相关问题时，先结合用户上下文给出直接可执行的建议，再补充必要说明。".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: false,
        private_memory_enabled: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn default_user_persona() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: USER_PERSONA_ID.to_string(),
        name: "用户".to_string(),
        system_prompt: "我是...".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: true,
        is_built_in_system: false,
        private_memory_enabled: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn default_system_persona() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: SYSTEM_PERSONA_ID.to_string(),
        name: "pai system".to_string(),
        system_prompt: "我是系统人格，负责代表任务中心与系统调度向当前助手传达信息。".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: true,
        private_memory_enabled: false,
        source: default_main_source(),
        scope: default_global_scope(),
    }
}

fn normalize_agent_tools(agent: &mut AgentProfile) -> bool {
    let defaults = default_agent_tools();
    let mut next = Vec::<ApiToolConfig>::new();
    for default in defaults {
        if let Some(found) = agent.tools.iter().find(|tool| tool.id == default.id) {
            next.push(ApiToolConfig {
                id: default.id.clone(),
                command: if found.command.trim().is_empty() {
                    default.command.clone()
                } else {
                    found.command.clone()
                },
                args: if found.args.is_empty() {
                    default.args.clone()
                } else {
                    found.args.clone()
                },
                enabled: found.enabled,
                values: found.values.clone(),
            });
        } else {
            next.push(default);
        }
    }
    let changed = agent.tools.len() != next.len()
        || agent.tools.iter().zip(next.iter()).any(|(left, right)| {
            left.id != right.id
                || left.enabled != right.enabled
                || left.command != right.command
                || left.args != right.args
                || left.values != right.values
        });
    if changed {
        agent.tools = next;
    }
    changed
}

fn fill_missing_message_speaker_agent_ids(data: &mut AppData) -> bool {
    fn provider_meta_speaker_agent_id(message: &ChatMessage) -> Option<String> {
        let meta = message.provider_meta.as_ref()?;
        let object = meta.as_object()?;
        for key in [
            "speakerAgentId",
            "speaker_agent_id",
            "targetAgentId",
            "target_agent_id",
            "agentId",
            "agent_id",
            "sourceAgentId",
            "source_agent_id",
        ] {
            let value = object
                .get(key)
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        None
    }

    let mut changed = false;
    for conversation in &mut data.conversations {
        let host_agent_id = conversation.agent_id.trim().to_string();
        if host_agent_id.is_empty() {
            continue;
        }
        for message in &mut conversation.messages {
            let current = message
                .speaker_agent_id
                .as_deref()
                .map(str::trim)
                .unwrap_or("");
            if current.is_empty() {
                message.speaker_agent_id =
                    Some(provider_meta_speaker_agent_id(message).unwrap_or_else(|| {
                        if message.role == "user" {
                            USER_PERSONA_ID.to_string()
                        } else {
                            host_agent_id.clone()
                        }
                    }));
                changed = true;
            }
        }
    }
    changed
}

fn fill_missing_conversation_metadata(data: &mut AppData) -> bool {
    let mut changed = false;
    for conversation in &mut data.conversations {
        if conversation.conversation_kind.trim().is_empty() {
            conversation.conversation_kind = CONVERSATION_KIND_CHAT.to_string();
            changed = true;
        }
    }
    for archive in &mut data.archived_conversations {
        if archive
            .source_conversation
            .conversation_kind
            .trim()
            .is_empty()
        {
            archive.source_conversation.conversation_kind = CONVERSATION_KIND_CHAT.to_string();
            changed = true;
        }
    }
    changed
}
