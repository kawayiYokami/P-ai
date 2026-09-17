fn resolve_archive_owner_agent_id(
    _config: &AppConfig,
    agents: &[AgentProfile],
    source: &Conversation,
) -> Result<String, String> {
    let owner_agent_id = source.agent_id.trim();
    if owner_agent_id.is_empty() {
        return Err(format!(
            "会话缺少归属人格，无法确定归档记忆归属人格: conversation_id={}",
            source.id
        ));
    }
    if available_non_user_agent(agents, owner_agent_id).is_none() {
        return Err(format!(
            "归档记忆归属人格不存在: conversation_id={}, agent_id={}",
            source.id, owner_agent_id
        ));
    }
    let owner_agent_id = owner_agent_id.to_string();

    Ok(owner_agent_id)
}

#[cfg(test)]
mod archive_host_selection_tests {
    use super::*;

    fn mk_agent(id: &str) -> AgentProfile {
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
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: default_main_source(),
            scope: default_global_scope(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            include_system_rules: true,
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        }
    }

    fn mk_msg_with_agent_hint(agent_id: &str) -> ChatMessage {
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "assistant".to_string(),
            created_at: now_iso(),
            speaker_agent_id: Some(agent_id.to_string()),
            parts: vec![MessagePart::Text {
                text: "x".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "agentId": agent_id,
            })),
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        }
    }

    fn mk_source(agent_id: &str, messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: "c1".to_string(),
            title: "t".to_string(),
            agent_id: agent_id.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages,
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None,
            last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    #[test]
    fn archive_owner_should_come_from_conversation_agent() {
        let agents = vec![mk_agent("owner-agent"), mk_agent("message-agent")];
        let source = mk_source(
            "message-agent",
            vec![
                mk_msg_with_agent_hint("message-agent"),
                mk_msg_with_agent_hint("message-agent"),
            ],
        );

        let owner =
            resolve_archive_owner_agent_id(&AppConfig::default(), &agents, &source).unwrap();

        assert_eq!(owner, "message-agent");
    }

    #[test]
    fn archive_owner_should_reject_missing_conversation_agent() {
        let agents = vec![mk_agent("owner-agent")];
        let source = mk_source("", Vec::new());

        let err = resolve_archive_owner_agent_id(&AppConfig::default(), &agents, &source).unwrap_err();

        assert!(err.contains("会话缺少归属人格"));
    }

    #[test]
    fn archive_owner_should_reject_missing_agent() {
        let source = mk_source("owner-agent", Vec::new());

        let err = resolve_archive_owner_agent_id(&AppConfig::default(), &[], &source).unwrap_err();

        assert!(err.contains("归档记忆归属人格不存在"));
    }

    #[test]
    fn archive_owner_should_accept_private_runtime_agent() {
        let root = std::env::temp_dir().join(format!(
            "eca-archive-owner-private-{}",
            Uuid::new_v4()
        ));
        let data_path = root.join("data").join("config_mark");
        let personas_dir = app_root_from_data_path(&data_path)
            .join("llm-workspace")
            .join("private-organization")
            .join("personas");
        std::fs::create_dir_all(&personas_dir).expect("create private personas dir");
        std::fs::write(
            personas_dir.join("private-owner.json"),
            r#"{
  "id": "private-owner",
  "name": "私域归档人格",
  "prompt": "x"
}"#,
        )
        .expect("write private persona");

        let snapshot = build_runtime_organization_snapshot_from_parts(
            &data_path,
            &AppConfig::default(),
            &[default_user_persona()],
        )
        .expect("build runtime snapshot");
        let source = mk_source("private-owner", Vec::new());

        let owner =
            resolve_archive_owner_agent_id(&snapshot.config, &snapshot.agents, &source).unwrap();

        assert_eq!(owner, "private-owner");
        let _ = std::fs::remove_dir_all(root);
    }
}
