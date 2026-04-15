fn archive_message_agent_hint(message: &ChatMessage) -> Option<String> {
    let meta = message.provider_meta.as_ref()?;
    let obj = meta.as_object()?;
    for key in ["agentId", "agent_id", "speakerAgentId", "speaker_agent_id"] {
        let value = obj.get(key).and_then(Value::as_str).unwrap_or("").trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn choose_archive_host_agent_id(data: &AppData, source: &Conversation, fallback_agent_id: &str) -> String {
    let fallback = fallback_agent_id.trim();
    if !fallback.is_empty()
        && data
            .agents
            .iter()
            .any(|a| !a.is_built_in_user && a.id == fallback)
    {
        return fallback.to_string();
    }

    let mut count_map = HashMap::<String, usize>::new();
    let mut last_idx_map = HashMap::<String, usize>::new();
    for (idx, message) in source.messages.iter().enumerate() {
        let hint = archive_message_agent_hint(message).or_else(|| {
            if message.role == "assistant" {
                Some(source.agent_id.clone())
            } else {
                None
            }
        });
        let Some(agent_id) = hint else {
            continue;
        };
        *count_map.entry(agent_id.clone()).or_insert(0) += 1;
        last_idx_map.insert(agent_id, idx);
    }

    let public_agents = data
        .agents
        .iter()
        .filter(|a| !a.is_built_in_user && !a.private_memory_enabled)
        .map(|a| a.id.clone())
        .collect::<Vec<_>>();
    if !public_agents.is_empty() {
        return public_agents
            .into_iter()
            .max_by(|a, b| {
                let ac = count_map.get(a).copied().unwrap_or(0);
                let bc = count_map.get(b).copied().unwrap_or(0);
                ac.cmp(&bc)
                    .then_with(|| {
                        let ai = last_idx_map.get(a).copied().unwrap_or(0);
                        let bi = last_idx_map.get(b).copied().unwrap_or(0);
                        ai.cmp(&bi)
                    })
                    .then_with(|| b.cmp(a))
            })
            .unwrap_or_else(|| source.agent_id.clone());
    }

    source.agent_id.clone()
}

#[cfg(test)]
mod archive_host_selection_tests {
    use super::*;

    fn mk_agent(id: &str, private_memory_enabled: bool) -> AgentProfile {
        AgentProfile {
            id: id.to_string(),
            name: id.to_string(),
            system_prompt: String::new(),
            tools: default_agent_tools(),
            created_at: now_iso(),
            updated_at: now_iso(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled,
            source: default_main_source(),
            scope: default_global_scope(),
        }
    }

    fn mk_msg(role: &str) -> ChatMessage {
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: role.to_string(),
            created_at: now_iso(),
            speaker_agent_id: Some("test-agent".to_string()),
            parts: vec![MessagePart::Text {
                text: "x".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    #[test]
    fn host_should_prefer_fallback_agent_when_valid() {
        let data = AppData {
            agents: vec![mk_agent("pub-a", false), mk_agent("pub-b", false)],
            assistant_department_agent_id: "pub-b".to_string(),
            user_alias: "u".to_string(),
            response_style_id: "concise".to_string(),
            main_conversation_id: None,
            ..AppData::default()
        };
        let source = Conversation {
            id: "c1".to_string(),
            title: "t".to_string(),
            agent_id: "pub-a".to_string(),
            department_id: String::new(),
            last_read_message_id: String::new(),
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: vec![mk_msg("assistant"), mk_msg("assistant"), mk_msg("assistant")],
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        };
        let host = choose_archive_host_agent_id(&data, &source, "pub-b");
        assert_eq!(host, "pub-b");
    }

    #[test]
    fn host_should_prefer_fallback_even_when_private() {
        let data = AppData {
            agents: vec![mk_agent("p1", true), mk_agent("p2", true)],
            assistant_department_agent_id: "p2".to_string(),
            user_alias: "u".to_string(),
            response_style_id: "concise".to_string(),
            main_conversation_id: None,
            ..AppData::default()
        };
        let source = Conversation {
            id: "c1".to_string(),
            title: "t".to_string(),
            agent_id: "p1".to_string(),
            department_id: String::new(),
            last_read_message_id: String::new(),
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: vec![mk_msg("assistant")],
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        };
        let host = choose_archive_host_agent_id(&data, &source, "p2");
        assert_eq!(host, "p2");
    }

}


