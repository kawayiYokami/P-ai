    use super::*;
    use httpmock::{
        Method::GET,
        MockServer,
    };

    fn test_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime")
    }


    fn test_text_message(role: &str, text: &str, created_at: &str) -> ChatMessage {
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: role.to_string(),
            created_at: created_at.to_string(),
            speaker_agent_id: Some("agent".to_string()),
            parts: vec![MessagePart::Text {
                text: text.to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    fn test_active_conversation_with_messages(
        messages: Vec<ChatMessage>,
        last_user_at: Option<String>,
    ) -> Conversation {
        let now = now_iso();
        Conversation {
            id: Uuid::new_v4().to_string(),
            title: "t".to_string(),
            agent_id: "agent".to_string(),
            department_id: String::new(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now.clone(),
            updated_at: now,
            last_user_at,
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

    include!("config/tests.rs");
    include!("chat/tests.rs");
    include!("task/tests.rs");
    include!("remote_im/tests.rs");
    include!("system/tests.rs");
    include!("memory/tests.rs");

