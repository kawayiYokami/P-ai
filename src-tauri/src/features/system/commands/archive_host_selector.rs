fn resolve_archive_owner_agent_id(
    config: &AppConfig,
    agents: &[AgentProfile],
    source: &Conversation,
) -> Result<String, String> {
    let department_id = source.department_id.trim();
    if department_id.is_empty() {
        return Err(format!(
            "会话缺少归属部门，无法确定归档记忆归属人格: conversation_id={}",
            source.id
        ));
    }

    let department = department_by_id(config, department_id).ok_or_else(|| {
        format!(
            "会话归属部门不存在，无法确定归档记忆归属人格: conversation_id={}, department_id={}",
            source.id, department_id
        )
    })?;
    let owner_agent_ids = department
        .agent_ids
        .iter()
        .map(|id| id.trim())
        .filter(|id| !id.is_empty())
        .collect::<Vec<_>>();
    if owner_agent_ids.len() != 1 {
        return Err(format!(
            "归档记忆归属部门必须且只能绑定一个人格: conversation_id={}, department_id={}, agent_count={}",
            source.id,
            department_id,
            owner_agent_ids.len()
        ));
    }

    let owner_agent_id = owner_agent_ids[0];
    if !agents
        .iter()
        .any(|agent| !agent.is_built_in_user && agent.id == owner_agent_id)
    {
        return Err(format!(
            "归档记忆归属人格不存在: conversation_id={}, department_id={}, agent_id={}",
            source.id, department_id, owner_agent_id
        ));
    }

    Ok(owner_agent_id.to_string())
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
            source: default_main_source(),
            scope: default_global_scope(),
        }
    }

    fn mk_department(id: &str, agent_ids: Vec<&str>) -> DepartmentConfig {
        DepartmentConfig {
            id: id.to_string(),
            name: id.to_string(),
            summary: String::new(),
            guide: String::new(),
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            agent_ids: agent_ids.into_iter().map(ToOwned::to_owned).collect(),
            created_at: now_iso(),
            updated_at: now_iso(),
            order_index: 0,
            is_built_in_assistant: false,
            is_deputy: false,
            source: default_main_source(),
            scope: default_global_scope(),
            permission_control: DepartmentPermissionControl::default(),
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
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "agentId": agent_id,
            })),
            tool_call: None,
            mcp_call: None,
        }
    }

    fn mk_source(department_id: &str, agent_id: &str, messages: Vec<ChatMessage>) -> Conversation {
        Conversation {
            id: "c1".to_string(),
            title: "t".to_string(),
            agent_id: agent_id.to_string(),
            department_id: department_id.to_string(),
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
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            archived_at: None,
            messages,
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        }
    }

    #[test]
    fn archive_owner_should_come_only_from_conversation_department() {
        let config = AppConfig {
            departments: vec![mk_department("dept-main", vec!["owner-agent"])],
            ..AppConfig::default()
        };
        let agents = vec![mk_agent("owner-agent"), mk_agent("message-agent")];
        let source = mk_source(
            "dept-main",
            "message-agent",
            vec![
                mk_msg_with_agent_hint("message-agent"),
                mk_msg_with_agent_hint("message-agent"),
            ],
        );

        let owner = resolve_archive_owner_agent_id(&config, &agents, &source).unwrap();

        assert_eq!(owner, "owner-agent");
    }

    #[test]
    fn archive_owner_should_reject_missing_department() {
        let config = AppConfig::default();
        let agents = vec![mk_agent("owner-agent")];
        let source = mk_source("missing-dept", "owner-agent", Vec::new());

        let err = resolve_archive_owner_agent_id(&config, &agents, &source).unwrap_err();

        assert!(err.contains("会话归属部门不存在"));
    }

    #[test]
    fn archive_owner_should_reject_department_without_single_agent() {
        let agents = vec![mk_agent("a1"), mk_agent("a2")];
        for agent_ids in [Vec::<&str>::new(), vec!["a1", "a2"]] {
            let config = AppConfig {
                departments: vec![mk_department("dept-main", agent_ids)],
                ..AppConfig::default()
            };
            let source = mk_source("dept-main", "a1", Vec::new());

            let err = resolve_archive_owner_agent_id(&config, &agents, &source).unwrap_err();

            assert!(err.contains("必须且只能绑定一个人格"));
        }
    }

    #[test]
    fn archive_owner_should_reject_missing_agent() {
        let config = AppConfig {
            departments: vec![mk_department("dept-main", vec!["owner-agent"])],
            ..AppConfig::default()
        };
        let source = mk_source("dept-main", "owner-agent", Vec::new());

        let err = resolve_archive_owner_agent_id(&config, &[], &source).unwrap_err();

        assert!(err.contains("归档记忆归属人格不存在"));
    }
}
