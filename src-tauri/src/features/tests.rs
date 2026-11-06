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

    pub(crate) fn test_active_conversation_with_messages(
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
            questions: Vec::new(),
            question_next_number: 0,
            issues: Vec::new(),
            issue_next_number: 0,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    include!("config/tests.rs");
    include!("chat/tests.rs");
    include!("task/tests.rs");
    include!("question/tests.rs");
    include!("remote_im/tests.rs");
    include!("system/tests.rs");
    include!("memory/tests.rs");
    include!("mcp/tests.rs");

    // ==================== 停止：按调度身份中止，不留跨轮次残留 ====================

    fn new_inflight_abort_handle() -> futures_util::future::AbortHandle {
        let (abortable, handle) = futures_util::future::abortable(async {});
        drop(abortable);
        handle
    }

    /// 停止命令不允许等待收尾。
    ///
    /// 状态组合是本轮已在跑（登记了 Running 句柄）+ 工具组正在执行 + 会话流式缓存里已有正文，
    /// 这正是旧实现走 `for _ in 0..25 { sleep(100ms) }` 轮询的入口。
    ///
    /// 为什么不允许等：abort 之后协程的收口必然会发生（落盘 partial → 置 Idle → 清流式缓存），
    /// 等它只是在用户按下停止之后白加延迟，还会把队列里的下一条一起拖住。停止只需要中止，
    /// 不需要确认中止已经完成。
    ///
    /// 阈值取 50ms：正常路径实测个位数毫秒，50ms 留足 CI 抖动；旧实现第一轮 sleep 就 100ms，
    /// 一开始就超。
    #[test]
    fn stop_chat_message_should_not_block_on_graceful_exit() {
        let runtime = test_runtime();
        runtime.block_on(async {
            let state = test_chat_runtime_state();
            let conversation_id = "conversation-stop-latency";
            // 用与实现同一个解析入口算 key，避免测试自己猜 agent_id 而错过真正的在途轮次。
            let agent_id = resolve_runtime_control_agent_id(
                &state,
                Some(""),
                Some(conversation_id),
            )
            .unwrap_or_default();
            let chat_key = inflight_chat_key(&agent_id, Some(conversation_id));

            state
                .inflight_chat_abort_handles
                .lock()
                .expect("lock inflight chat abort handles")
                .insert(
                    chat_key.clone(),
                    InflightChatAbortEntry::Running {
                        handle: new_inflight_abort_handle(),
                        request_id: "req-stop-latency".to_string(),
                        assistant_message_id: "assistant-stop-latency".to_string(),
                    },
                );
            state
                .inflight_tool_abort_handles
                .lock()
                .expect("lock inflight tool abort handles")
                .insert(chat_key.clone(), new_inflight_abort_handle());

            // 「有流式内容」这个条件也要成立：旧实现除了看请求字段，还会读该会话的流式缓存。
            // 少这一条，守卫只守得住「有工具在跑」，守不住「有内容可等」——将来有人把等待
            // 加回来、并改按缓存判空，这个测试仍会绿。
            lock_conversation_runtime_slots(&state)
                .expect("lock runtime slots")
                .entry(conversation_id.to_string())
                .or_default()
                .stream_cache
                .assistant_text = "已经流出来的正文".to_string();

            let input = StopChatRequest {
                session: SessionSelector {
                    api_config_id: None,
                    agent_id: agent_id.clone(),
                    conversation_id: Some(conversation_id.to_string()),
                },
                // 停止不再依赖调用方回传的部分正文：收尾由后端自己读流式缓存完成。
                request_id: Some("req-stop-latency".to_string()),
                activation_id: None,
                assistant_message_id: Some("assistant-stop-latency".to_string()),
            };

            let started = std::time::Instant::now();
            let result = stop_chat_message_inner(input, &state)
                .await
                .expect("停止不应失败");
            let elapsed = started.elapsed();
            println!("[stop-latency] stop_chat_message_inner 耗时 = {elapsed:?}");

            assert!(result.aborted, "应当中止到这一轮");
            assert!(
                elapsed < std::time::Duration::from_millis(50),
                "停止命令不允许等待收尾，实测 {elapsed:?}"
            );
        });
    }

    /// 构造一次停止请求要终止的调度身份。
    fn stop_identity(request_id: &str, assistant_message_id: &str) -> StopDispatchIdentity {
        StopDispatchIdentity {
            request_id: request_id.to_string(),
            activation_id: request_id.to_string(),
            assistant_message_id: assistant_message_id.to_string(),
        }
    }

    fn inflight_has_running(state: &AppState, chat_key: &str) -> bool {
        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        matches!(
            inflight.get(chat_key),
            Some(InflightChatAbortEntry::Running { .. })
        )
    }

    #[test]
    fn register_inflight_chat_abort_handle_consumes_matching_cancel_request() {
        // 提交窗口：消息已发出、句柄还没登记，用户此刻按下停止。
        // 停止只能落在「这一次提交」的请求号上，由这次提交登记时消费一次。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";

        assert!(
            request_stop_inflight_chat(&state, chat_key, &stop_identity("req-1", ""))
                .expect("request stop"),
            "提交期停止要给这次提交留下取消标记"
        );
        let stopped_before_start = register_inflight_chat_abort_handle(
            &state,
            chat_key,
            new_inflight_abort_handle(),
            "req-1",
            "assistant-1",
        )
        .expect("register inflight chat abort handle");
        assert!(
            stopped_before_start,
            "同请求号的提交必须被自己那次停止中止"
        );
        assert!(!inflight_has_running(&state, chat_key), "被中止的提交不应留下槽位");
    }

    #[test]
    fn register_inflight_chat_abort_handle_ignores_cancel_request_of_other_request() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";

        request_stop_inflight_chat(&state, chat_key, &stop_identity("req-1", ""))
            .expect("request stop");
        let stopped_before_start = register_inflight_chat_abort_handle(
            &state,
            chat_key,
            new_inflight_abort_handle(),
            "req-2",
            "assistant-2",
        )
        .expect("register inflight chat abort handle");
        assert!(
            !stopped_before_start,
            "别人的提交不能被这次提交的停止牵连"
        );
        assert!(
            inflight_has_running(&state, chat_key),
            "req-2 应当正常登记为运行中"
        );
    }

    #[test]
    fn register_inflight_chat_abort_handle_aborts_previous_running_handle() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let first = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, first.clone(), "req-1", "assistant-1")
            .expect("register first");

        let second = new_inflight_abort_handle();
        let stopped_before_start =
            register_inflight_chat_abort_handle(&state, chat_key, second.clone(), "req-2", "assistant-2")
                .expect("register second");
        assert!(!stopped_before_start);
        assert!(first.is_aborted(), "后登记的句柄必须中止前一个");
        assert!(!second.is_aborted());
    }

    #[test]
    fn clear_conversation_cancel_requests_removes_stale_requests_only() {
        let state = test_chat_runtime_state();
        let target_key = "agent-1::conversation-1";
        let other_key = "agent-1::conversation-2";
        let running_key = "agent-1::conversation-1::delegate";
        request_stop_inflight_chat(&state, target_key, &stop_identity("req-1", ""))
            .expect("request stop target");
        request_stop_inflight_chat(&state, other_key, &stop_identity("req-2", ""))
            .expect("request stop other");
        let running_handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(
            &state,
            running_key,
            running_handle.clone(),
            "req-3",
            "assistant-3",
        )
        .expect("register running");

        let cleared = clear_conversation_cancel_requests(&state, "conversation-1")
            .expect("clear cancel requests");
        assert_eq!(cleared, 1, "只应清掉目标会话上残留的提交期取消标记");

        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(inflight.get(target_key).is_none());
        assert!(matches!(
            inflight.get(other_key),
            Some(InflightChatAbortEntry::CancelRequested { .. })
        ));
        assert!(matches!(
            inflight.get(running_key),
            Some(InflightChatAbortEntry::Running { .. })
        ));
        assert!(!running_handle.is_aborted(), "清理取消标记不应动运行中的句柄");
    }

    #[test]
    fn request_stop_inflight_chat_aborts_registered_round() {
        // 停止一个正在跑的轮次：当场 abort，事后共享状态里不留任何残留。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, handle.clone(), "req-1", "assistant-1")
            .expect("register");

        assert!(
            request_stop_inflight_chat(&state, chat_key, &stop_identity("req-1", "assistant-1"))
                .expect("request stop")
        );
        assert!(handle.is_aborted(), "已登记的轮次必须被立即中止");
        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(inflight.get(chat_key).is_none(), "停止之后不得留下任何残留");
    }

    #[test]
    fn request_stop_inflight_chat_matches_by_assistant_message_id() {
        // 前端手上未必有请求号，但一定有本轮助理消息 id：只给消息 id 也要能精确命中。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, handle.clone(), "req-1", "assistant-1")
            .expect("register");

        let mut identity = stop_identity("", "");
        identity.assistant_message_id = "assistant-1".to_string();
        assert!(request_stop_inflight_chat(&state, chat_key, &identity).expect("request stop"));
        assert!(handle.is_aborted());
    }

    #[test]
    fn request_stop_inflight_chat_leaves_other_dispatch_untouched() {
        // 不猜：带 A 身份停止，只中止 A；表里 B 那一份必须原样留着。
        let state = test_chat_runtime_state();
        let chat_key_b = "agent-1::conversation-1";
        let handle_b = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(
            &state,
            chat_key_b,
            handle_b.clone(),
            "req-b",
            "assistant-b",
        )
        .expect("register b");

        assert!(
            !request_stop_inflight_chat(&state, chat_key_b, &stop_identity("req-a", "assistant-a"))
                .expect("request stop a"),
            "身份对不上时停止必须落空"
        );
        assert!(!handle_b.is_aborted(), "另一个调度不得被误伤");
        assert!(inflight_has_running(&state, chat_key_b), "B 必须仍在表里");
    }

    #[test]
    fn request_stop_inflight_chat_records_cancel_for_pending_commit() {
        // 提交窗口：句柄还没登记，停止按请求号记下取消；这一条只能被消费一次。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";

        assert!(
            request_stop_inflight_chat(&state, chat_key, &stop_identity("req-1", ""))
                .expect("request stop"),
            "带请求号的停止要给这次提交留下取消标记"
        );
        assert!(
            register_inflight_chat_abort_handle(
                &state,
                chat_key,
                new_inflight_abort_handle(),
                "req-1",
                "assistant-1",
            )
            .expect("register"),
            "同请求号的提交必须被中止"
        );
        assert!(
            !inflight_has_running(&state, chat_key),
            "取消标记只能被消费一次，且消费后不留槽位"
        );
    }

    #[test]
    fn request_stop_inflight_chat_after_round_finished_does_not_affect_next_round() {
        // 现场：点停止时上一轮刚好收尾，句柄已经撤掉；用户紧接着发新消息，
        // 新一轮必须照常登记、照常进入激活，不能被上一轮的停止残留吞掉。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";

        let finished = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, finished.clone(), "req-1", "assistant-1")
            .expect("register previous round");
        // 上一轮收尾：句柄被摘掉，之后停止才姗姗来迟。
        state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles")
            .remove(chat_key);
        // 表里已经没有可中止的对象，此刻它与「提交窗口里句柄尚未登记」在形态上完全一致，
        // 后端无法也不该去猜。停止最多在 req-1 这个请求号上留一枚只能被同一个请求号
        // 消费一次的取消标记——它指不到任何别的调度，也不会中止任何轮次。
        request_stop_inflight_chat(&state, chat_key, &stop_identity("req-1", "assistant-1"))
            .expect("late stop");

        // 新一轮：请求号不同，必须照常登记、照常进入激活。
        let next = new_inflight_abort_handle();
        let stopped_before_start =
            register_inflight_chat_abort_handle(&state, chat_key, next.clone(), "req-2", "assistant-2")
                .expect("register next round");
        assert!(!stopped_before_start, "新一轮不得被上一轮的停止中止");
        assert!(!next.is_aborted(), "新一轮必须真正进入激活");
        assert!(inflight_has_running(&state, chat_key), "新一轮必须正常登记");
        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(
            matches!(
                inflight.get(chat_key),
                Some(InflightChatAbortEntry::Running { request_id, .. }) if request_id == "req-2"
            ),
            "表里该键只能是新一轮自己，req-1 的标记不得残留"
        );
    }

    #[test]
    fn request_stop_inflight_chat_without_any_identity_touches_nothing() {
        // 身份全空：前端都不知道在停谁，后端不许按会话猜，一个字节都不许写。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";

        assert!(
            !request_stop_inflight_chat(&state, chat_key, &StopDispatchIdentity::default())
                .expect("request stop"),
            "没有身份的停止必须落空"
        );
        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(inflight.get(chat_key).is_none());
    }

    #[test]
    fn clear_inflight_chat_cancel_request_keeps_running_handle() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, handle.clone(), "req-1", "assistant-1")
            .expect("register");

        clear_inflight_chat_cancel_request(&state, chat_key).expect("clear cancel request");
        assert!(!handle.is_aborted(), "清理取消标记不应影响运行中的句柄");
        assert!(inflight_has_running(&state, chat_key));
    }

    #[test]
    fn abort_running_inflight_chat_preserves_cancel_request() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        request_stop_inflight_chat(&state, chat_key, &stop_identity("req-1", ""))
            .expect("request stop");

        assert!(
            !abort_running_inflight_chat(&state, chat_key).expect("abort running"),
            "只有提交期取消标记时不应视为中止到运行中的轮次"
        );
        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(
            matches!(
                inflight.get(chat_key),
                Some(InflightChatAbortEntry::CancelRequested { .. })
            ),
            "针对尚未登记句柄的那次提交的取消标记必须保留"
        );
    }

    #[test]
    fn sending_after_previous_round_stopped_must_activate_new_round() {
        // 红豆报的原始时序：正在回复时点停止，紧接着发一条新消息。
        // 断言的不是「某个函数被调用」，而是新消息真的走进了激活：本批跑完之后，
        // 会话里出现了助理轮次（激活会先 bootstrap 助理消息），且结果不是「用户中止」。
        let state = test_chat_runtime_state();
        let conversation_id = "conversation-stop-then-send";
        let now = now_iso();
        let conversation = test_chat_conversation(conversation_id, "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        let chat_key = inflight_chat_key(DEFAULT_AGENT_ID, Some(conversation_id));

        // 上一轮：登记过句柄、已经收尾，停止姗姗来迟（就发生在同一秒内）。
        let previous = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(
            &state,
            &chat_key,
            previous.clone(),
            "req-1",
            "assistant-1",
        )
        .expect("register previous round");
        state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles")
            .remove(&chat_key);
        let _ = request_stop_inflight_chat(&state, &chat_key, &stop_identity("req-1", "assistant-1"))
            .expect("late stop");

        // 紧接着发出的新消息：它有自己的请求号，必须真的被激活。
        let mut event = test_pending_event(conversation_id);
        let mut runtime_context = runtime_context_new("user_message", "user_send");
        runtime_context.request_id = Some("req-2".to_string());
        event.runtime_context = Some(runtime_context);

        let outcome = test_runtime().block_on(process_conversation_batch(
            &state,
            conversation_id,
            vec![event],
        ));
        if let Err(err) = &outcome {
            assert_ne!(
                err, CHAT_ABORTED_BY_USER_ERROR,
                "已经发出的用户消息不得被上一轮的停止吞掉"
            );
        }

        let updated =
            state_read_conversation_cached(&state, conversation_id).expect("read conversation");
        assert!(
            updated
                .messages
                .iter()
                .any(|message| message.role.trim() == "assistant"),
            "新消息必须真的激活出助理轮次；库里只有用户消息说明整轮没被激活"
        );
    }