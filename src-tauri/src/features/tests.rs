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
        let speaker_agent_id = if role.eq_ignore_ascii_case("assistant") {
            Some(DEFAULT_AGENT_ID.to_string())
        } else if role.eq_ignore_ascii_case("user") {
            Some(USER_PERSONA_ID.to_string())
        } else {
            None
        };
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: role.to_string(),
            created_at: created_at.to_string(),
            speaker_agent_id,
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

    fn test_active_conversation_with_messages(
        messages: Vec<ChatMessage>,
        last_user_at: Option<String>,
    ) -> Conversation {
        let now = now_iso();
        Conversation {
            loaded_mcp_tools: Vec::new(),
            id: Uuid::new_v4().to_string(),
            title: "t".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
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
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            shell_worktree_path: String::new(),
            shell_recorded_branch: String::new(),
            archived_at: None,
            messages,
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, team: TeamState::default(), last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    include!("config/tests.rs");
    include!("chat/tests.rs");
    include!("task/tests.rs");
    include!("remote_im/tests.rs");
    include!("system/tests.rs");
    include!("memory/tests.rs");
    include!("mcp/tests.rs");

    // ==================== 打断入口：登记 / 停止意图互斥 ====================

    fn insert_inflight_stop_intent(state: &AppState, chat_key: &str) {
        let mut inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        inflight.insert(chat_key.to_string(), InflightChatAbortEntry::StopRequested);
    }

    fn new_inflight_abort_handle() -> futures_util::future::AbortHandle {
        let (abortable, handle) = futures_util::future::abortable(async {});
        drop(abortable);
        handle
    }

    #[test]
    fn register_inflight_chat_abort_handle_consumes_fresh_stop_intent() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        insert_inflight_stop_intent(&state, chat_key);

        let stopped_before_start =
            register_inflight_chat_abort_handle(&state, chat_key, new_inflight_abort_handle())
                .expect("register inflight chat abort handle");
        assert!(stopped_before_start, "停止意图必须让这一轮在开始前就中止");

        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(
            inflight.get(chat_key).is_none(),
            "被消费的停止意图不应留下槽位"
        );
    }

    #[test]
    fn register_inflight_chat_abort_handle_consumes_stop_intent_regardless_of_age() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        insert_inflight_stop_intent(&state, chat_key);

        // 停止意图没有过期语义：只要已经落下，就必须被接下来的这一轮消费。
        let stopped_before_start =
            register_inflight_chat_abort_handle(&state, chat_key, new_inflight_abort_handle())
                .expect("register inflight chat abort handle");
        assert!(
            stopped_before_start,
            "停止意图不允许因为「时间久了」而失效"
        );
    }

    #[test]
    fn register_inflight_chat_abort_handle_aborts_previous_running_handle() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let first = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, first.clone()).expect("register first");

        let second = new_inflight_abort_handle();
        let stopped_before_start =
            register_inflight_chat_abort_handle(&state, chat_key, second.clone())
                .expect("register second");
        assert!(!stopped_before_start);
        assert!(first.is_aborted(), "后登记的句柄必须中止前一个");
        assert!(!second.is_aborted());
    }

    #[test]
    fn clear_conversation_stop_intents_removes_stale_intents_only() {
        let state = test_chat_runtime_state();
        let target_key = "agent-1::conversation-1";
        let other_key = "agent-1::conversation-2";
        let running_key = "agent-1::conversation-1::delegate";
        insert_inflight_stop_intent(&state, target_key);
        insert_inflight_stop_intent(&state, other_key);
        let running_handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, running_key, running_handle.clone())
            .expect("register running");

        let cleared =
            clear_conversation_stop_intents(&state, "conversation-1").expect("clear stop intents");
        assert_eq!(cleared, 1, "只应清掉目标会话上残留的停止意图");

        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(inflight.get(target_key).is_none());
        assert!(matches!(
            inflight.get(other_key),
            Some(InflightChatAbortEntry::StopRequested)
        ));
        assert!(matches!(
            inflight.get(running_key),
            Some(InflightChatAbortEntry::Running(_))
        ));
        assert!(!running_handle.is_aborted(), "清理意图不应动运行中的句柄");
    }

    #[test]
    fn request_stop_inflight_chat_aborts_registered_round() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, handle.clone()).expect("register");

        assert!(request_stop_inflight_chat(&state, chat_key).expect("request stop"));
        assert!(handle.is_aborted(), "已登记的轮次必须被立即中止");
        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(inflight.get(chat_key).is_none());
    }

    #[test]
    fn request_stop_inflight_chat_leaves_and_consumes_stop_intent() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";

        assert!(request_stop_inflight_chat(&state, chat_key).expect("request stop"));
        assert!(
            take_inflight_chat_stop_intent(&state, chat_key).expect("take stop intent"),
            "尚无句柄时停止应留下意图"
        );
        assert!(
            !take_inflight_chat_stop_intent(&state, chat_key).expect("take stop intent again"),
            "停止意图只能被消费一次"
        );
    }

    #[test]
    fn clear_inflight_chat_stop_intent_keeps_running_handle() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, handle.clone()).expect("register");

        clear_inflight_chat_stop_intent(&state, chat_key).expect("clear stop intent");
        assert!(!handle.is_aborted(), "清理停止意图不应影响运行中的句柄");
        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(matches!(
            inflight.get(chat_key),
            Some(InflightChatAbortEntry::Running(_))
        ));
    }

    #[test]
    fn abort_running_inflight_chat_preserves_stop_intent() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        request_stop_inflight_chat(&state, chat_key).expect("request stop");

        assert!(
            !abort_running_inflight_chat(&state, chat_key).expect("abort running"),
            "只有停止意图时不应视为中止到运行中的轮次"
        );
        assert!(
            take_inflight_chat_stop_intent(&state, chat_key).expect("take stop intent"),
            "针对尚未登记轮次的停止意图必须保留"
        );
    }