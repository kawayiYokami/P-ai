    use super::*;
    use httpmock::{
        Method::{GET, POST},
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
            api_config_id: "api".to_string(),
            agent_id: "agent".to_string(),
            created_at: now.clone(),
            updated_at: now,
            last_user_at,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            status: "active".to_string(),
            messages,
        }
    }

    fn count_xml_tag(xml: &str, tag: &str) -> usize {
        let needle = format!("<{}>", tag);
        xml.match_indices(&needle).count()
    }

    include!("config/tests.rs");
    include!("chat/tests.rs");
    include!("system/tests.rs");
    include!("memory/tests.rs");
