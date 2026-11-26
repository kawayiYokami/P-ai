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
            short_id: None,
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
    include!("timer/tests.rs");
    include!("question/tests.rs");
    include!("remote_im/tests.rs");
    include!("system/tests.rs");
    include!("memory/tests.rs");
    include!("mcp/tests.rs");

    // ==================== 停止：按调度身份中止，不留跨轮次残留 ====================

    /// 停止命令不允许等待收尾。
    ///
    /// 状态组合是本轮已在跑（登记了运行中的轮次）+ 工具组正在执行 + 会话流式缓存里已有正文，
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
                        request_id: "req-stop-latency".to_string(),
                        assistant_message_id: "assistant-stop-latency".to_string(),
                        retired_request_ids: Vec::new(),
                    },
                );

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

    /// 停止**不落盘**：它只写水位、翻信号，然后立刻返回。
    ///
    /// 构造的是「这一轮正在流式输出、容器里已有正文」的状态。停止命令返回时：
    /// 会话必须还在跑、所有权还在 worker 手上、库里没有多出半截消息——
    /// 落盘与收尾全是 worker 读到信号后自己走的完结路线。
    #[test]
    fn stop_chat_message_should_not_persist_and_leave_finishing_to_worker() {
        let runtime = test_runtime();
        runtime.block_on(async {
            let state = test_chat_runtime_state();
            let now = now_iso();
            let conversation = test_chat_conversation("conversation-stop-persist", "active", &now);
            state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");
            conversation_service_v2()
                .bootstrap_streaming_assistant_message(
                    &state,
                    &AssistantMessageBootstrapInput {
                        conversation_id: conversation.id.clone(),
                        assistant_message_id: "assistant-stop-persist".to_string(),
                        speaker_agent_id: DEFAULT_AGENT_ID.to_string(),
                        created_at: Some(now.clone()),
                        provider_meta_patch: None,
                        compaction_preserved_messages: None,
                    },
                )
                .expect("bootstrap assistant message");

            let chat_key = inflight_chat_key(DEFAULT_AGENT_ID, Some(conversation.id.as_str()));
            state
                .inflight_chat_abort_handles
                .lock()
                .expect("lock inflight chat abort handles")
                .insert(
                    chat_key.clone(),
                    InflightChatAbortEntry::Running {
                        request_id: "req-stop-persist".to_string(),
                        assistant_message_id: "assistant-stop-persist".to_string(),
                        retired_request_ids: Vec::new(),
                    },
                );
            {
                let mut slots =
                    lock_conversation_runtime_slots(&state).expect("lock runtime slots");
                let slot = slots.entry(conversation.id.clone()).or_default();
                slot.state = MainSessionState::AssistantStreaming;
                slot.round_owner = "owner-stop-persist".to_string();
                slot.stream_cache.activation_id = "req-stop-persist".to_string();
                slot.stream_cache.request_id = "req-stop-persist".to_string();
                slot.stream_cache.agent_id = DEFAULT_AGENT_ID.to_string();
                slot.stream_cache.persisted_assistant_message_id =
                    "assistant-stop-persist".to_string();
                slot.stream_cache.assistant_text = "流到一半的正文".to_string();
            }

            let input = StopChatRequest {
                session: SessionSelector {
                    api_config_id: None,
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                    conversation_id: Some(conversation.id.clone()),
                },
                request_id: Some("req-stop-persist".to_string()),
                activation_id: Some("req-stop-persist".to_string()),
                assistant_message_id: Some("assistant-stop-persist".to_string()),
            };
            let result = stop_chat_message_inner(input, &state)
                .await
                .expect("停止不应失败");

            assert!(result.aborted, "应当中止到这一轮");
            assert!(!result.persisted, "停止方不许代 worker 落盘");
            assert!(
                result.assistant_text.is_empty(),
                "停止方不返回正文，正文随 worker 的完结结果回前台"
            );

            // 库里那条助理消息上不该多出半截正文：落盘仍是 worker 的活。
            let stored = conversation_service_v2()
                .read_message_by_id(&state, &conversation.id, "assistant-stop-persist")
                .expect("read stored message");
            let stored_text = stored.parts.iter().find_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.as_str()),
                _ => None,
            });
            assert_ne!(
                stored_text,
                Some("流到一半的正文"),
                "停止方不许把流式容器里的正文写进库"
            );

            // 会话仍在跑、所有权仍在 worker 手上：它要自己走完结路线。
            assert_eq!(
                get_conversation_runtime_state(&state, &conversation.id).expect("read state"),
                MainSessionState::AssistantStreaming,
                "停止方不许把会话置空闲"
            );
            assert!(
                finish_conversation_round(&state, &conversation.id, "owner-stop-persist")
                    .expect("finish round"),
                "所有权必须还在 worker 手上，由它自己交还"
            );
            assert!(
                test_round_requested(&state, &conversation.id),
                "必须把这一轮的调度号记进待停列表，让 worker 读到并自己收尾"
            );
            assert!(
                inflight_has_running(&state, &chat_key),
                "停止方不许摘掉 worker 的登记"
            );
        });
    }

    /// 停止只做一件事：翻转本会话的结束信号，交给已经起来的 worker 自己收尾。
    /// **不中止任何协程、不作废队列**——worker 必须活着走完自己的收尾。
    #[test]
    fn stop_chat_message_without_identity_should_signal_worker() {
        let runtime = test_runtime();
        runtime.block_on(async {
            let state = test_chat_runtime_state();
            let conversation_id = "conversation-stop-no-identity";
            let chat_key = inflight_chat_key(DEFAULT_AGENT_ID, Some(conversation_id));
            state
                .inflight_chat_abort_handles
                .lock()
                .expect("lock inflight chat abort handles")
                .insert(
                    chat_key.clone(),
                    InflightChatAbortEntry::Running {
                        request_id: "req-running".to_string(),
                        assistant_message_id: "assistant-running".to_string(),
                        retired_request_ids: Vec::new(),
                    },
                );
            {
                let mut slots =
                    lock_conversation_runtime_slots(&state).expect("lock runtime slots");
                let slot = slots.entry(conversation_id.to_string()).or_default();
                slot.state = MainSessionState::AssistantStreaming;
                slot.round_owner = "owner-running".to_string();
                slot.stream_cache.activation_id = "req-running".to_string();
                slot.stream_cache.request_id = "req-running".to_string();
                slot.stream_cache.persisted_assistant_message_id = "assistant-running".to_string();
            }

            let input = StopChatRequest {
                session: SessionSelector {
                    api_config_id: None,
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                    conversation_id: Some(conversation_id.to_string()),
                },
                request_id: None,
                activation_id: None,
                assistant_message_id: None,
            };
            let result = stop_chat_message_inner(input, &state)
                .await
                .expect("无身份的停止不应失败");

            assert!(result.aborted, "无身份的停止也算一次成立的停止");
            assert!(
                inflight_has_running(&state, &chat_key),
                "不许把在跑的轮次登记摘掉：worker 要活着走完自己的收尾"
            );
            assert!(
                test_round_requested(&state, conversation_id),
                "必须把这一轮的调度号记进待停列表，让已经起来的 worker 自己停"
            );
        });
    }

    /// 确权之后、登记之前被打断：表里没有这条记录，也不许停止方替它收尾。
    ///
    /// 这一段空隙里轮次已在装配上下文但还没登记。停止只写水位、翻信号：
    /// 所有权仍归 worker，落盘凭据不许由停止方写——它自己会在下一个检查点收尾。
    #[test]
    fn stop_before_handle_registered_should_signal_without_claiming_round() {
        let runtime = test_runtime();
        runtime.block_on(async {
            let state = test_chat_runtime_state();
            let conversation_id = "conversation-stop-before-handle";
            {
                let mut slots =
                    lock_conversation_runtime_slots(&state).expect("lock runtime slots");
                let slot = slots.entry(conversation_id.to_string()).or_default();
                slot.state = MainSessionState::AssistantStreaming;
                slot.round_owner = "owner-before-handle".to_string();
            }

            let input = StopChatRequest {
                session: SessionSelector {
                    api_config_id: None,
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                    conversation_id: Some(conversation_id.to_string()),
                },
                request_id: None,
                activation_id: None,
                assistant_message_id: None,
            };
            let result = stop_chat_message_inner(input, &state)
                .await
                .expect("停止不应失败");

            assert!(result.aborted, "表里没有登记时也必须把这次停止算成立");
            // 这一轮还没登记、停止也没带任何身份号：没有可点名的对象，
            // 停止不在会话上造任何「已停止」状态，所有权仍归 worker。
            let slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.get(conversation_id).expect("slot");
            assert_eq!(
                slot.round_owner, "owner-before-handle",
                "所有权必须留在 worker 手上，由它自己收尾"
            );
        });
    }

    /// worker 起来之后只认待停列表里的调度号：不看所有权令牌。
    #[test]
    fn round_stop_requested_reads_only_the_stop_list() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-stop-signal-only";

        // 没有登记：没有可等待的号，worker 不许自行判死。
        assert!(
            !round_stop_requested(&state, &[]),
            "没有本轮信号时不许自行判死"
        );

        // 本轮登记之后：没被要求停止就放行。
        register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
            .expect("register");
        let round_ids = inflight_round_request_ids(&state, chat_key);
        assert!(!round_stop_requested(&state, &round_ids), "没被点名时必须放行");

        // 点名到本轮自己的号就必须承认，与时间、令牌无关。
        mark_round_stop_requested(&state, &["req-1".to_string()]).expect("mark stop");
        assert!(
            round_stop_requested(&state, &round_ids),
            "点名到本轮，worker 必须立刻认到"
        );

        // 别的轮次的号留在列表里也伤不到这一轮。
        clear_round_stop_requested(&state, &["req-1".to_string()]);
        mark_round_stop_requested(&state, &["req-other".to_string()]).expect("mark other");
        assert!(
            !round_stop_requested(&state, &round_ids),
            "别人的号不许命中这一轮"
        );
    }

    /// 端到端时序：主助理确权（T1）→ 中途被打断 → 旧协程继续走进 send_chat_message_inner，
    /// 必须在发请求前看到结束信号，带着空结果走完结路线——不是报错。
    #[test]
    fn interrupted_round_should_not_be_resurrected_by_send_entry() {
        let runtime = test_runtime();
        runtime.block_on(async {
            let state = test_chat_runtime_state();
            let now = now_iso();
            let conversation = test_chat_conversation("conversation-send-gate", "active", &now);
            state_schedule_conversation_persist(&state, &conversation)
                .expect("persist conversation");
            conversation_service_v2()
                .bootstrap_streaming_assistant_message(
                    &state,
                    &AssistantMessageBootstrapInput {
                        conversation_id: conversation.id.clone(),
                        assistant_message_id: "assistant-send-gate".to_string(),
                        speaker_agent_id: DEFAULT_AGENT_ID.to_string(),
                        created_at: Some(now.clone()),
                        provider_meta_patch: None,
                        compaction_preserved_messages: None,
                    },
                )
                .expect("bootstrap assistant message");

            let confirmed_started_at_ms = 1_700_000_000_000u64;
            {
                let mut slots =
                    lock_conversation_runtime_slots(&state).expect("lock runtime slots");
                let slot = slots.entry(conversation.id.clone()).or_default();
                slot.state = MainSessionState::AssistantStreaming;
                slot.round_owner = "owner-send-gate".to_string();
                slot.stream_cache.activation_id = "req-send-gate".to_string();
                slot.stream_cache.request_id = "req-send-gate".to_string();
                slot.stream_cache.persisted_assistant_message_id =
                    "assistant-send-gate".to_string();
                slot.stream_cache.started_at_ms = confirmed_started_at_ms;
            }
            let chat_key = inflight_chat_key(DEFAULT_AGENT_ID, Some(conversation.id.as_str()));
            state
                .inflight_chat_abort_handles
                .lock()
                .expect("lock inflight chat abort handles")
                .insert(
                    chat_key,
                    InflightChatAbortEntry::Running {
                        request_id: "req-send-gate".to_string(),
                        assistant_message_id: "assistant-send-gate".to_string(),
                        retired_request_ids: Vec::new(),
                    },
                );

            // 打断：只写水位、翻信号。收尾留给这一轮的 worker。
            let stop_input = StopChatRequest {
                session: SessionSelector {
                    api_config_id: None,
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                    conversation_id: Some(conversation.id.clone()),
                },
                request_id: Some("req-send-gate".to_string()),
                activation_id: Some("req-send-gate".to_string()),
                assistant_message_id: Some("assistant-send-gate".to_string()),
            };
            let stop_result = stop_chat_message_inner(stop_input, &state)
                .await
                .expect("停止不应失败");
            assert!(stop_result.aborted, "应当停住这一轮");
            assert!(!stop_result.persisted, "停止方不许代 worker 落盘");

            // 旧协程继续走到发送入口（带着本轮确权时的令牌）。
            let channel = tauri::ipc::Channel::new(|_| Ok(()));
            let request = SendChatRequest {
                trigger_only: false,
                session: Some(SessionSelector {
                    api_config_id: None,
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                    conversation_id: Some(conversation.id.clone()),
                }),
                payload: ChatInputPayload {
                    text: None,
                    display_text: None,
                    parts: None,
                    images: None,
                    audios: None,
                    attachments: None,
                    model: None,
                    extra_text_blocks: None,
                    mentions: None,
                    provider_meta: None,
                },
                speaker_agent_id: None,
                trace_id: Some("req-send-gate".to_string()),
                assistant_message_id: Some("assistant-send-gate".to_string()),
                oldest_queue_created_at: None,
                remote_im_activation_sources: Vec::new(),
                runtime_context: Some(RuntimeContext {
                    request_id: Some("req-send-gate".to_string()),
                    round_owner: Some("owner-send-gate".to_string()),
                    ..RuntimeContext::default()
                }),
            };
            let send_result = send_chat_message_inner(request, &state, &channel)
                .await
                .expect("打断不是错误：本轮必须正常收口，不许抛错");
            assert!(
                send_result.assistant_text.is_empty(),
                "已收到结束信号的轮次不许再发请求、不许产出正文"
            );

            let slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.get(&conversation.id).expect("slot");
            assert_eq!(
                slot.stream_cache.started_at_ms, confirmed_started_at_ms,
                "停止方不碰流式缓存，发送入口也不许改写它"
            );
        });
    }

    /// 停止带上不匹配的身份，仍然是一次成立的停止：打断不认身份。
    ///
    /// 断言的重点从「算不算停住了」变成「停完之后谁做收尾」：别人的轮次登记不许被摘、
    /// 所有权不许被收走、不许留落盘凭据——收尾只能由那一轮的 worker 自己做。
    #[test]
    fn stop_with_mismatched_identity_still_reports_stopped_without_taking_over_round() {
        let runtime = test_runtime();
        runtime.block_on(async {
            let state = test_chat_runtime_state();
            let conversation_id = "conversation-stop-mismatch";
            let chat_key = inflight_chat_key(DEFAULT_AGENT_ID, Some(conversation_id));
            state
                .inflight_chat_abort_handles
                .lock()
                .expect("lock inflight chat abort handles")
                .insert(
                    chat_key.clone(),
                    InflightChatAbortEntry::Running {
                        request_id: "req-actually-running".to_string(),
                        assistant_message_id: "assistant-actually-running".to_string(),
                        retired_request_ids: Vec::new(),
                    },
                );
            {
                let mut slots =
                    lock_conversation_runtime_slots(&state).expect("lock runtime slots");
                let slot = slots.entry(conversation_id.to_string()).or_default();
                slot.state = MainSessionState::AssistantStreaming;
                slot.round_owner = "owner-actually-running".to_string();
                slot.stream_cache.activation_id = "req-actually-running".to_string();
                slot.stream_cache.request_id = "req-actually-running".to_string();
                slot.stream_cache.persisted_assistant_message_id =
                    "assistant-actually-running".to_string();
                slot.stream_cache.started_at_ms = 1_000;
            }

            let input = StopChatRequest {
                session: SessionSelector {
                    api_config_id: None,
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                    conversation_id: Some(conversation_id.to_string()),
                },
                // 身份对不上任何在跑的调度：但停止针对的是「这个会话这一轮」。
                request_id: Some("req-stale".to_string()),
                activation_id: Some("req-stale".to_string()),
                assistant_message_id: Some("assistant-stale".to_string()),
            };
            let result = stop_chat_message_inner(input, &state)
                .await
                .expect("停止不应失败");

            assert!(result.aborted, "指定了会话就是一次成立的停止");
            assert!(!result.persisted, "停止方不许代 worker 落盘");
            assert!(
                test_round_requested(&state, conversation_id),
                "必须把这一轮的调度号记进待停列表，由这一轮的 worker 自己停"
            );
            assert!(
                inflight_has_running(&state, &chat_key),
                "别人那一轮的登记不许被摘掉"
            );
            let slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.get(conversation_id).expect("slot");
            assert_eq!(
                slot.round_owner, "owner-actually-running",
                "别人那一轮的所有权不许被收走"
            );
        });
    }

    /// 测试用：这一轮是否已被要求停止。
    fn test_round_requested(state: &AppState, conversation_id: &str) -> bool {
        round_stop_requested(
            state,
            &inflight_round_request_ids_by_conversation(state, conversation_id),
        )
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
        // 提交窗口：消息已发出、登记还没写上，用户此刻按下停止。
        // 停止只能落在「这一次提交」的请求号上，由这次提交登记时消费一次。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";

        assert!(
            request_stop_inflight_chat(&state, chat_key, "req-1")
                .expect("request stop"),
            "提交期停止要给这次提交留下取消标记"
        );
        let stopped_before_start = register_inflight_chat_abort_handle(
            &state,
            chat_key,
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

        request_stop_inflight_chat(&state, chat_key, "req-1")
            .expect("request stop");
        let stopped_before_start = register_inflight_chat_abort_handle(
            &state,
            chat_key,
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
    fn register_inflight_chat_abort_handle_takes_over_previous_record() {
        // 同一键上还挂着上一轮的记录：后登记的直接把记录换成本轮的，不中止任何协程。
        // 上一轮该自己走完结路线——它读到的是自己那一份信号，不是这一轮的登记动作。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
            .expect("register first");

        let stopped_before_start =
            register_inflight_chat_abort_handle(&state, chat_key, "req-2", "assistant-2")
                .expect("register second");
        assert!(!stopped_before_start, "没有取消标记时新一轮照常开跑");
        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(
            matches!(
                inflight.get(chat_key),
                Some(InflightChatAbortEntry::Running { request_id, .. }) if request_id == "req-2"
            ),
            "表里只留新一轮自己"
        );
    }

    #[test]
    fn register_same_request_id_again_should_refresh_without_dropping_retired_ids() {
        // 同一轮会被登记两次：批次入口先按本轮定死的号登，发送入口再登一次补消息 id。
        // 号相同就是同一次调度的刷新，不是被新一轮接管——已退役的旧号必须留着，
        // 否则压缩重启前那个号丢了，那一轮就再也停不住。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        register_inflight_chat_abort_handle(&state, chat_key, "req-old", "assistant-old")
            .expect("register first");
        refresh_inflight_chat_dispatch_identity(
            &state,
            chat_key,
            "req-old",
            "req-new",
            "assistant-old",
        )
        .expect("refresh round id");

        let stopped_before_start =
            register_inflight_chat_abort_handle(&state, chat_key, "req-new", "assistant-new")
                .expect("register same round again");
        assert!(!stopped_before_start, "重复登记不是取消标记");
        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(
            matches!(
                inflight.get(chat_key),
                Some(InflightChatAbortEntry::Running {
                    request_id,
                    assistant_message_id,
                    retired_request_ids,
                }) if request_id == "req-new"
                    && assistant_message_id == "assistant-new"
                    && retired_request_ids == &vec!["req-old".to_string()]
            ),
            "同号重复登记只补消息 id，保留已退役的旧号"
        );
    }

    #[test]
    fn clear_conversation_cancel_requests_removes_stale_requests_only() {
        let state = test_chat_runtime_state();
        let target_key = "agent-1::conversation-1";
        let other_key = "agent-1::conversation-2";
        let running_key = "agent-1::conversation-1::delegate";
        request_stop_inflight_chat(&state, target_key, "req-1")
            .expect("request stop target");
        request_stop_inflight_chat(&state, other_key, "req-2")
            .expect("request stop other");
        register_inflight_chat_abort_handle(&state, running_key, "req-3", "assistant-3")
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
        assert!(
            matches!(
                inflight.get(running_key),
                Some(InflightChatAbortEntry::Running { .. })
            ),
            "清理取消标记不应动运行中的轮次"
        );
    }

    #[test]
    fn request_stop_inflight_chat_reports_hit_without_aborting_worker() {
        // 停止一个正在跑的轮次：如实报告命中，但**不中止**任何协程，也不摘表项。
        // 收尾（落盘、广播、置空闲、释放占位）全是 worker 自己的活。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
            .expect("register");

        assert!(
            request_stop_inflight_chat(&state, chat_key, "req-1")
                .expect("request stop")
        );
        assert!(inflight_has_running(&state, chat_key), "表项必须留着给 worker 自己摘");
    }

    #[test]
    fn request_stop_inflight_chat_hits_after_compaction_restart() {
        // 压缩重启会换用新的请求号与助理消息 id，但继续跑在同一次 run 里、同一条登记。
        // 停止不看身份，命中即报告；这一轮照样由 worker 自己收尾。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
            .expect("register");

        refresh_inflight_chat_dispatch_identity(&state, chat_key, "req-1", "req-2", "assistant-2")
            .expect("refresh identity");

        assert!(
            request_stop_inflight_chat(&state, chat_key, "req-2")
                .expect("stop with new identity"),
            "压缩重启后的新号同样命中同一轮"
        );
        assert!(inflight_has_running(&state, chat_key));
    }

    #[test]
    fn request_stop_inflight_chat_still_hits_with_retired_identity() {
        // 压缩边界的完成事件只走单发通道，Web / VS Code 收不到，那一端会丢弃新的轮次开始
        // 事件、一直持有压缩前的旧身份。旧号也必须能停住同一轮，否则 Web 端会从
        // 「能停」退化成「不能停」。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
            .expect("register");
        refresh_inflight_chat_dispatch_identity(&state, chat_key, "req-1", "req-2", "assistant-2")
            .expect("refresh identity");

        assert!(
            request_stop_inflight_chat(&state, chat_key, "req-1")
                .expect("stop with retired identity"),
            "压缩前的旧号仍代表同一轮，必须能停住"
        );
    }

    #[test]
    fn refresh_inflight_chat_dispatch_identity_skips_entry_taken_over() {
        // 同一 key 上可能已被更晚一次调度接手：它不是我们的条目，不得改写其身份，
        // 否则那个调度按自己的号就停不掉了。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        register_inflight_chat_abort_handle(&state, chat_key, "req-2", "assistant-2")
            .expect("register");

        // 本轮以为表内还是 req-1，实际已是 req-2：应跳过改写。
        refresh_inflight_chat_dispatch_identity(&state, chat_key, "req-1", "req-3", "assistant-3")
            .expect("refresh identity");

        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(matches!(
            inflight.get(chat_key),
            Some(InflightChatAbortEntry::Running { request_id, retired_request_ids, .. })
                if request_id == "req-2" && retired_request_ids.is_empty()
        ));
    }

    #[test]
    fn inflight_entry_is_ours_accepts_retired_identity() {
        // 收尾清理按本轮用过的任一身份认领自己；只比最初那个 trace_id 会留下残留表项，
        // 而残留的 Running 会让下一轮提交期的停止撞上它并对不上。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
            .expect("register");
        refresh_inflight_chat_dispatch_identity(&state, chat_key, "req-1", "req-2", "assistant-2")
            .expect("refresh identity");

        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(
            inflight_entry_is_ours(inflight.get(chat_key), "req-1"),
            "压缩重启前的旧号必须仍能把这一轮认成自己的"
        );
        assert!(
            inflight_entry_is_ours(inflight.get(chat_key), "req-2"),
            "新号同样认得出来"
        );
        assert!(
            !inflight_entry_is_ours(inflight.get(chat_key), "req-other"),
            "别的轮次不得被认成自己"
        );
    }

    #[test]
    fn refresh_inflight_chat_dispatch_identity_leaves_cancel_request_untouched() {
        // 刷新只针对已登记的运行轮次；挂着的「提交期取消」标记不属于任何一轮，不能被它改掉。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        request_stop_inflight_chat(&state, chat_key, "req-pending").expect("record cancel request");

        refresh_inflight_chat_dispatch_identity(&state, chat_key, "req-new", "req-newer", "assistant-newer")
            .expect("refresh identity");

        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(matches!(
            inflight.get(chat_key),
            Some(InflightChatAbortEntry::CancelRequested { request_id }) if request_id == "req-pending"
        ));
    }

    #[test]
    fn request_stop_inflight_chat_does_not_compare_identity() {
        // 停止不按身份比对：键上正跑着轮次就如实报告「停到了」，但绝不从外面把它中止，
        // 也绝不把表项摘掉——worker 必须活着走完自己的收尾。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
            .expect("register");

        assert!(
            request_stop_inflight_chat(&state, chat_key, "req-other").expect("request stop"),
            "键上有在跑的轮次，停止就必须如实报告命中"
        );
        assert!(inflight_has_running(&state, chat_key), "表项必须留着给 worker 自己摘");
    }

    #[test]
    fn request_stop_inflight_chat_records_cancel_for_pending_commit() {
        // 提交窗口：登记还没写上，停止按请求号记下取消；这一条只能被消费一次。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";

        assert!(
            request_stop_inflight_chat(&state, chat_key, "req-1")
                .expect("request stop"),
            "带请求号的停止要给这次提交留下取消标记"
        );
        let stopped_before_start = register_inflight_chat_abort_handle(
            &state,
            chat_key,
            "req-1",
            "assistant-1",
        )
        .expect("register");
        assert!(stopped_before_start, "同请求号的提交必须被中止");
        assert!(
            !inflight_has_running(&state, chat_key),
            "取消标记只能被消费一次，且消费后不留槽位"
        );
    }

    #[test]
    fn request_stop_inflight_chat_after_round_finished_does_not_affect_next_round() {
        // 现场：点停止时上一轮刚好收尾，记录已经撤掉；用户紧接着发新消息，
        // 新一轮必须照常登记、照常进入激活，不能被上一轮的停止残留吞掉。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";

        register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
            .expect("register previous round");
        // 上一轮收尾：记录被摘掉，之后停止才姗姗来迟。
        state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles")
            .remove(chat_key);
        // 表里已经没有可中止的对象，此刻它与「提交窗口里轮次尚未登记」在形态上完全一致，
        // 后端无法也不该去猜。停止最多在 req-1 这个请求号上留一枚只能被同一个请求号
        // 消费一次的取消标记——它指不到任何别的调度，也不会中止任何轮次。
        request_stop_inflight_chat(&state, chat_key, "req-1")
            .expect("late stop");

        // 新一轮：请求号不同，必须照常登记、照常进入激活。
        let stopped_before_start =
            register_inflight_chat_abort_handle(&state, chat_key, "req-2", "assistant-2")
                .expect("register next round");
        assert!(!stopped_before_start, "新一轮不得被上一轮的停止中止");
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
            !request_stop_inflight_chat(&state, chat_key, "")
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
    fn clear_inflight_chat_cancel_request_keeps_running_round() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
            .expect("register");

        clear_inflight_chat_cancel_request(&state, chat_key).expect("clear cancel request");
        assert!(inflight_has_running(&state, chat_key), "清理取消标记不应影响运行中的轮次");
    }

    /// 只有提交期取消标记、没有在跑轮次时，停止不许凭空认领一轮。
    #[test]
    fn stop_with_only_cancel_request_keeps_it_for_that_commit() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        request_stop_inflight_chat(&state, chat_key, "req-1")
            .expect("request stop");

        assert!(
            !request_stop_inflight_chat(&state, chat_key, "req-other")
                .expect("request stop other"),
            "号对不上的停止不许认领这一枚标记"
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
            "针对尚未登记的那次提交的取消标记必须保留"
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

        // 上一轮：登记过、已经收尾，停止姗姗来迟（就发生在同一秒内）。
        register_inflight_chat_abort_handle(&state, &chat_key, "req-1", "assistant-1")
            .expect("register previous round");
        state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles")
            .remove(&chat_key);
        let _ = request_stop_inflight_chat(&state, &chat_key, "req-1")
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

    // ==================== 打断：各阶段的毫秒级验收 ====================

    /// 阶段一（worker 还没起来）：打断不会作废队列里已经排着的消息。
    ///
    /// 出队发生在打断之后，说明用户仍然要这条消息跑；出队处只认队列，不按发送时间判死。
    #[test]
    fn stop_before_worker_starts_should_still_claim_queued_events() {
        let state = test_chat_runtime_state();
        let conversation_id = "conversation-stop-before-spawn";
        let mut event = test_pending_event(conversation_id);
        // 事件的产生时间早于打断时刻：按旧规则会被丢掉。
        event.created_at_ms = 1_000;
        let event_id = event.id.clone();
        {
            let mut slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.entry(conversation_id.to_string()).or_default();
            slot.pending_queue.push_back(event);
        }

        // 首次打开聊天元数据库要建表建 WAL，那是冷启动成本，不属于出队成本。
        let _ = message_store::active_dispatch_upsert(&state.data_path, "__warmup__", "[]");

        let started = std::time::Instant::now();
        let batches = claim_queued_conversation_batches(&state).expect("claim batches");
        let elapsed = started.elapsed();

        assert_eq!(batches.len(), 1, "排队的事件必须照常出队");
        assert!(
            batches[0].1.iter().any(|claimed| claimed.id == event_id),
            "出队的就是刚才排进去的那条消息"
        );
        assert!(
            elapsed < std::time::Duration::from_millis(50),
            "出队只读内存，必须远快于 50ms，实测 {elapsed:?}"
        );
        assert!(
            lock_conversation_runtime_slots(&state)
                .expect("lock runtime slots")
                .get(conversation_id)
                .map(|slot| slot.pending_queue.is_empty())
                .unwrap_or(true),
            "认领过的事件不许留在队列里反复重试"
        );
    }

    /// 阶段二（worker 第一步：写历史）：信号已到 → 跳过召回、照常写盘、不激活任何轮次。
    #[test]
    fn stop_during_write_history_skips_recall_and_activates_nothing() {
        let state = test_chat_runtime_state();
        let conversation_id = "conversation-stop-write-history";
        let now = now_iso();
        let conversation = test_chat_conversation(conversation_id, "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");

        // 停止先到：这一轮登记时用的就是同一个号，worker 起来读到的是同一张待停列表。
        let chat_key = inflight_chat_key(DEFAULT_AGENT_ID, Some(conversation_id));
        register_inflight_chat_abort_handle(&state, &chat_key, "req-write-history", "")
            .expect("登记本轮");
        assert!(
            request_stop_inflight_chat_round(&state, &chat_key, conversation_id)
                .expect("点名本轮"),
            "该键上有在跑的轮次，点名必须命中"
        );
        assert!(
            test_round_requested(&state, conversation_id),
            "记下号之后这一轮必须认得出自己"
        );

        let mut event = test_pending_event(conversation_id);
        let mut runtime_context = runtime_context_new("user_message", "user_send");
        runtime_context.request_id = Some("req-write-history".to_string());
        event.runtime_context = Some(runtime_context);

        let started = std::time::Instant::now();
        let outcome = test_runtime().block_on(process_conversation_batch(
            &state,
            conversation_id,
            vec![event],
        ));
        let elapsed = started.elapsed();

        if let Err(err) = &outcome {
            assert_ne!(
                err, CHAT_ABORTED_BY_USER_ERROR,
                "打断不是错误：写历史阶段不允许返回用户中止错误"
            );
        }
        assert!(
            elapsed < std::time::Duration::from_secs(3),
            "写历史阶段不许挂住（这一段含 SQLite 写，不钉毫秒，钉的是不等到 provider 超时），实测 {elapsed:?}"
        );

        // 用户发的字必须留在历史里：丢的只是这一轮的回复。
        let updated =
            state_read_conversation_cached(&state, conversation_id).expect("read conversation");
        assert!(
            updated
                .messages
                .iter()
                .any(|message| message.role.trim() == "user"),
            "被打断也要把用户消息写进去，实测 {:?}",
            updated.messages.len()
        );
        assert!(
            !updated
                .messages
                .iter()
                .any(|message| message.role.trim() == "assistant"),
            "不许建轮次：信号已到时一个助理消息都不该出现"
        );
        assert_eq!(
            get_conversation_runtime_state(&state, conversation_id).expect("read state"),
            MainSessionState::Idle,
            "收尾之后会话必须回到空闲"
        );
        assert!(
            lock_conversation_processing_claims(&state)
                .expect("lock claims")
                .is_empty(),
            "收尾必须释放会话名额"
        );
    }

    /// 事故钉子：一轮被点名收尾之后，下一条消息必须照常激活。
    ///
    /// 线上事故是「一次停止永久停止」——旧实现把会话级的「已停止」布尔置 true 之后没有人复位，
    /// 于是之后每条消息都被拦在激活之前。这里跑完整两轮：第一轮被点名收尾，
    /// 第二轮必须真的激活出助理轮次。只要会话上还留着「已停止」这种东西，这条就会变红。
    #[test]
    fn stopped_round_must_not_block_the_next_message() {
        let state = test_chat_runtime_state();
        let conversation_id = "conversation-stop-then-next";
        let now = now_iso();
        let conversation = test_chat_conversation(conversation_id, "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        let chat_key = inflight_chat_key(DEFAULT_AGENT_ID, Some(conversation_id));

        // 第一轮：登记、被点名、跑完不激活。
        register_inflight_chat_abort_handle(&state, &chat_key, "req-stop-first", "")
            .expect("登记第一轮");
        assert!(
            request_stop_inflight_chat_round(&state, &chat_key, conversation_id)
                .expect("点名第一轮"),
            "点名必须命中在跑的轮次"
        );
        let mut stopped_event = test_pending_event(conversation_id);
        let mut first_context = runtime_context_new("user_message", "user_send");
        first_context.request_id = Some("req-stop-first".to_string());
        stopped_event.runtime_context = Some(first_context);
        let first_outcome = test_runtime().block_on(process_conversation_batch(
            &state,
            conversation_id,
            vec![stopped_event],
        ));
        if let Err(err) = &first_outcome {
            assert_ne!(err, CHAT_ABORTED_BY_USER_ERROR, "打断不是错误");
        }
        let after_first =
            state_read_conversation_cached(&state, conversation_id).expect("read conversation");
        assert!(
            !after_first
                .messages
                .iter()
                .any(|message| message.role.trim() == "assistant"),
            "被点名的那一轮不许激活"
        );

        // 第二轮：新号、新事件，必须照常激活。
        let mut next_event = test_pending_event(conversation_id);
        let mut next_context = runtime_context_new("user_message", "user_send");
        next_context.request_id = Some("req-after-stop".to_string());
        next_event.runtime_context = Some(next_context);
        let next_outcome = test_runtime().block_on(process_conversation_batch(
            &state,
            conversation_id,
            vec![next_event],
        ));
        if let Err(err) = &next_outcome {
            assert_ne!(
                err, CHAT_ABORTED_BY_USER_ERROR,
                "上一轮的停止不许波及这一轮"
            );
        }
        let after_next =
            state_read_conversation_cached(&state, conversation_id).expect("read conversation");
        assert!(
            after_next
                .messages
                .iter()
                .any(|message| message.role.trim() == "assistant"),
            "被停止之后的下一条消息必须真的激活出助理轮次"
        );
    }

    /// 阶段四（请求建连）：provider 不接、不回时，结束信号必须当场放弃等待。
    ///
    /// 这是最常被卡住的一处：裸 await 会一直挂到 provider 自己的超时，
    /// 用户按了停止毫无反应。这里用「挂 30 秒的 future」代替真实建连。
    #[test]
    fn stop_during_connect_returns_within_100ms() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-stop-connect";
        test_runtime().block_on(async {
            register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
                .expect("register");
            let round_ids = inflight_round_request_ids(&state, chat_key);
            let stopper = {
                let stopper_state = state.clone();
                let stopper_ids = round_ids.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    mark_round_stop_requested(&stopper_state, &stopper_ids).expect("mark stop");
                })
            };

            let started = std::time::Instant::now();
            let outcome = race_with_chat_stop(&state, &round_ids, async {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                "connected"
            })
            .await;
            let elapsed = started.elapsed();
            stopper.await.expect("stopper");

            assert!(
                outcome.is_none(),
                "信号先到时必须放弃等待，不许等建连自己回来"
            );
            assert!(
                elapsed < std::time::Duration::from_millis(100),
                "建连阶段必须能被打断，实测 {elapsed:?}"
            );
        });
    }

    /// 建连先成功时照常拿到结果：赛跑不许把正常路径也判成「已结束」。
    #[test]
    fn race_with_chat_stop_keeps_work_that_finishes_first() {
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-race-normal";
        test_runtime().block_on(async {
            register_inflight_chat_abort_handle(&state, chat_key, "req-1", "assistant-1")
                .expect("register");
            let round_ids = inflight_round_request_ids(&state, chat_key);
            let outcome = race_with_chat_stop(&state, &round_ids, async {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                "connected"
            })
            .await;
            assert_eq!(outcome, Some("connected"), "没有信号时工作结果必须原样返回");
        });
    }

    /// 阶段八（空闲时被停止）：没有任何活可停，也不许写库、不许拖时间。
    #[test]
    fn stop_after_idle_is_noop() {
        let runtime = test_runtime();
        runtime.block_on(async {
            let state = test_chat_runtime_state();
            let conversation_id = "conversation-stop-idle";
            let input = StopChatRequest {
                session: SessionSelector {
                    api_config_id: None,
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                    conversation_id: Some(conversation_id.to_string()),
                },
                request_id: None,
                activation_id: None,
                assistant_message_id: None,
            };
            let started = std::time::Instant::now();
            let result = stop_chat_message_inner(input, &state)
                .await
                .expect("空闲时停止不应失败");
            let elapsed = started.elapsed();

            assert!(!result.persisted, "空闲时停止不许写库");
            assert!(
                result.assistant_message.is_none(),
                "空闲时停止不许造出任何消息"
            );
            assert!(
                elapsed < std::time::Duration::from_secs(3),
                "空闲时停止只有内存动作加一次目标压制落账，不许挂住，实测 {elapsed:?}"
            );
        });
    }


    /// 各域独立单调递增，号只增不减：删除/跳过都不会把号重新发出去。
    #[test]
    fn short_id_sequences_should_increment_per_domain_and_never_recycle() {
        let state = test_chat_runtime_state();
        let data_path = state.data_path.clone();

        assert_eq!(
            allocate_next_short_id(&data_path, SHORT_ID_DOMAIN_CONVERSATION).expect("alloc"),
            0
        );
        assert_eq!(
            allocate_next_short_id(&data_path, SHORT_ID_DOMAIN_CONVERSATION).expect("alloc"),
            1
        );
        // 其它域从 0 开始，互不影响。
        assert_eq!(
            allocate_next_short_id(&data_path, SHORT_ID_DOMAIN_AGENT).expect("alloc"),
            0
        );
        // 回到会话域继续向前，不回收。
        assert_eq!(
            allocate_next_short_id(&data_path, SHORT_ID_DOMAIN_CONVERSATION).expect("alloc"),
            2
        );
        assert_eq!(
            peek_next_short_id(&data_path, SHORT_ID_DOMAIN_CONVERSATION).expect("peek"),
            3
        );
        assert_eq!(
            peek_next_short_id(&data_path, SHORT_ID_DOMAIN_AGENT).expect("peek"),
            1
        );
    }

    /// 历史会话迁移：按 createdAt 升序补号，重跑幂等。
    #[test]
    fn backfill_conversation_short_ids_should_order_by_created_at_and_be_idempotent() {
        let state = test_chat_runtime_state();
        insert_test_conversation_metadata(&state, "conv-late", "2024-03-01T00:00:00Z", None);
        insert_test_conversation_metadata(&state, "conv-early", "2024-01-01T00:00:00Z", None);
        insert_test_conversation_metadata(&state, "conv-mid", "2024-02-01T00:00:00Z", None);

        let assigned = backfill_conversation_short_ids(&state).expect("backfill");
        assert_eq!(assigned, 3);
        assert_eq!(conversation_short_id_of(&state, "conv-early"), Some(0));
        assert_eq!(conversation_short_id_of(&state, "conv-mid"), Some(1));
        assert_eq!(conversation_short_id_of(&state, "conv-late"), Some(2));
        assert_eq!(
            peek_next_short_id(&state.data_path, SHORT_ID_DOMAIN_CONVERSATION).expect("peek"),
            3
        );

        // 再跑一次：没有待分配的，号一个都不变。
        let again = backfill_conversation_short_ids(&state).expect("backfill again");
        assert_eq!(again, 0);
        assert_eq!(conversation_short_id_of(&state, "conv-early"), Some(0));
        assert_eq!(conversation_short_id_of(&state, "conv-late"), Some(2));
    }

    /// P1 回归：V5 瘦身后 JSON 里已经没有 createdAt，补号必须按 `created_at` 列升序，
    /// 不能退化成「按主键字典序」。
    #[test]
    fn backfill_conversation_short_ids_should_order_by_created_at_column_after_v5_slim() {
        let state = test_chat_runtime_state();
        // 主键字典序 a < b < c，创建时间序 b < c < a：两种排法结果不同，能钉住依据。
        insert_test_conversation_metadata(&state, "conv-a", "2024-03-01T00:00:00Z", None);
        insert_test_conversation_metadata(&state, "conv-b", "2024-01-01T00:00:00Z", None);
        insert_test_conversation_metadata(&state, "conv-c", "2024-02-01T00:00:00Z", None);

        let raw = conversation_metadata_json_of(&state, "conv-a");
        let value: serde_json::Value = serde_json::from_str(&raw).expect("parse metadata json");
        assert!(
            value.get("createdAt").is_none(),
            "V5 瘦身后 JSON 不该再有 createdAt，时间只住在列里：{raw}"
        );

        let assigned = backfill_conversation_short_ids(&state).expect("backfill");
        assert_eq!(assigned, 3);
        assert_eq!(conversation_short_id_of(&state, "conv-b"), Some(0));
        assert_eq!(conversation_short_id_of(&state, "conv-c"), Some(1));
        assert_eq!(conversation_short_id_of(&state, "conv-a"), Some(2));
    }

    /// 已有短 ID 的会话在迁移中不得被覆盖：模拟迁移中断后重启继续。
    #[test]
    fn backfill_should_keep_existing_short_ids() {        let state = test_chat_runtime_state();
        insert_test_conversation_metadata(&state, "conv-has-id", "2024-01-01T00:00:00Z", Some(5));
        insert_test_conversation_metadata(&state, "conv-no-id", "2024-01-02T00:00:00Z", None);

        let assigned = backfill_conversation_short_ids(&state).expect("backfill");
        assert_eq!(assigned, 1);
        assert_eq!(
            conversation_short_id_of(&state, "conv-has-id"),
            Some(5),
            "已分配的号必须原样保留"
        );
        assert_eq!(
            conversation_short_id_of(&state, "conv-no-id"),
            Some(0),
            "空缺号从 0 起补"
        );
        assert_eq!(
            peek_next_short_id(&state.data_path, SHORT_ID_DOMAIN_CONVERSATION).expect("peek"),
            6
        );
    }

    /// 双解析：纯数字按短 ID 寻址，UUID / 文本按主键原样返回。
    #[test]
    fn resolve_conversation_reference_should_accept_short_id_and_primary_key() {
        let state = test_chat_runtime_state();
        insert_test_conversation_metadata(&state, "conv-resolve", "2024-05-01T00:00:00Z", Some(42));

        assert_eq!(
            resolve_conversation_id_reference(&state, "42").expect("resolve short id"),
            Some("conv-resolve".to_string())
        );
        assert_eq!(
            resolve_conversation_id_reference(&state, "#42").expect("resolve short id with prefix"),
            Some("conv-resolve".to_string())
        );
        assert_eq!(
            resolve_conversation_id_reference(&state, "conv-resolve").expect("resolve primary key"),
            Some("conv-resolve".to_string())
        );
        assert_eq!(
            resolve_conversation_id_reference(&state, "9999").expect("resolve missing"),
            None
        );
        assert_eq!(
            resolve_conversation_id_reference(&state, "   ").expect("resolve blank"),
            None
        );
    }

    /// 手工往会话库写一条元数据：迁移与解析只依赖库里的行，不依赖内存缓存。
    ///
    /// 写入形态与 V5 之后的库一致：短 ID 与创建时间只在各自列里，metadata_json 里
    /// 没有 `shortId` / `createdAt`。补号读的是 `created_at` 列，不是 JSON。
    fn insert_test_conversation_metadata(
        state: &AppState,
        conversation_id: &str,
        created_at: &str,
        short_id: Option<u64>,
    ) {
        let metadata = serde_json::json!({
            "id": conversation_id,
            "title": conversation_id,
        });
        let conn = crate::message_store::chat_metadata_store_open(&state.data_path)
            .expect("open chat metadata");
        conn.execute(
            "INSERT INTO conversation_metadata(conversation_id, metadata_json, storage_revision, updated_at, short_id, created_at)
             VALUES(?1, ?2, 0, ?3, ?4, ?5)
             ON CONFLICT(conversation_id) DO UPDATE SET metadata_json=excluded.metadata_json, updated_at=excluded.updated_at",
            rusqlite::params![
                conversation_id,
                serde_json::to_string(&metadata).expect("serialize metadata"),
                created_at,
                short_id.map(|value| value as i64),
                created_at
            ],
        )
        .expect("insert conversation metadata");
    }

    fn conversation_short_id_of(state: &AppState, conversation_id: &str) -> Option<u64> {        use rusqlite::OptionalExtension as _;
        let conn = crate::message_store::chat_metadata_store_open(&state.data_path)
            .expect("open chat metadata");
        let value: Option<i64> = conn
            .query_row(
                "SELECT short_id FROM conversation_metadata WHERE conversation_id=?1",
                rusqlite::params![conversation_id],
                |row| row.get(0),
            )
            .optional()
            .expect("query short id");
        value.map(|item| item as u64)
    }

    /// 读出会话库里某条行的 metadata_json 原文（用来断言短 ID 没被塞进 JSON）。
    fn conversation_metadata_json_of(state: &AppState, conversation_id: &str) -> String {
        let conn = crate::message_store::chat_metadata_store_open(&state.data_path)
            .expect("open chat metadata");
        conn.query_row(
            "SELECT metadata_json FROM conversation_metadata WHERE conversation_id=?1",
            rusqlite::params![conversation_id],
            |row| row.get::<_, String>(0),
        )
        .expect("query metadata json")
    }

    /// 端到端：分配 -> 真实落盘 -> 从存储读回一致 -> 二次写盘不抹列 -> 重跑 bootstrap 号不漂移。
    ///
    /// 短 ID 只以 `conversation_metadata.short_id` 列为权威，metadata_json 里不许出现它。
    #[test]
    fn conversation_short_id_should_persist_in_column_and_survive_bootstrap() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conv-short-id-e2e", "active", &now);
        conversation.short_id = None;
        let assigned = assign_conversation_short_id(&state, &mut conversation).expect("assign");
        state_write_conversation_cached(&state, &conversation).expect("write conversation");

        let raw = conversation_metadata_json_of(&state, "conv-short-id-e2e");
        assert!(
            !raw.contains("shortId"),
            "metadata_json 里不许出现 shortId，实际={raw}"
        );
        assert_eq!(
            conversation_short_id_of(&state, "conv-short-id-e2e"),
            Some(assigned),
            "落盘后列上必须有号"
        );

        let read_back =
            state_read_conversation_cached(&state, "conv-short-id-e2e").expect("read back");
        assert_eq!(
            read_back.short_id,
            Some(assigned),
            "读回的会话必须带上列里的号（投影不能掉件）"
        );

        // 模拟重启：内存缓存与待落盘队列全部作废，只剩磁盘 shard + SQLite 列。
        {
            state
                .cached_conversation_metadata
                .lock()
                .expect("lock metadata cache")
                .clear();
            state
                .cached_conversation_mtimes
                .lock()
                .expect("lock mtime cache")
                .clear();
            state
                .cached_conversation_dirty_ids
                .lock()
                .expect("lock dirty ids")
                .clear();
            state
                .cached_deleted_conversation_ids
                .lock()
                .expect("lock deleted ids")
                .clear();
            *state
                .conversation_persist_pending
                .lock()
                .expect("lock pending persist") = None;
        }
        let restarted = state_read_conversation_cached(&state, "conv-short-id-e2e")
            .expect("read back after restart");
        assert_eq!(
            restarted.short_id,
            Some(assigned),
            "重启后必须从 short_id 列把号还原回来"
        );

        // 模拟后续保存（改标题、追加消息）：UPDATE 绝不改列。
        let mut updated = read_back.clone();
        updated.title = "改过标题".to_string();
        state_write_conversation_cached(&state, &updated).expect("write again");
        assert_eq!(
            conversation_short_id_of(&state, "conv-short-id-e2e"),
            Some(assigned),
            "再次写入不得改动已锁定的号"
        );

        // 模拟重启：号不重排、游标不跳。
        run_global_short_id_bootstrap(&state).expect("bootstrap");
        assert_eq!(
            conversation_short_id_of(&state, "conv-short-id-e2e"),
            Some(assigned),
            "启动自检不得覆盖已分配的号"
        );
        assert_eq!(
            peek_next_short_id(&state.data_path, SHORT_ID_DOMAIN_CONVERSATION).expect("peek"),
            assigned + 1,
            "游标应停在已发号的下一个"
        );

        // 新会话继续向前取号，绝不回收。
        let mut another = test_chat_conversation("conv-short-id-e2e-2", "active", &now);
        another.short_id = None;
        assert_eq!(
            assign_conversation_short_id(&state, &mut another).expect("assign second"),
            assigned + 1,
            "新会话的号必须单调前进"
        );
    }

    /// 人格补号：全部未分配时按序补满，号唯一，重跑幂等。
    #[test]
    fn backfill_agent_short_ids_should_assign_and_stay_idempotent() {
        let state = test_chat_runtime_state();
        let mut agents = state_read_agents_cached(&state).expect("read agents");
        for agent in &mut agents {
            agent.short_id = None;
        }
        state_write_agents_cached(&state, &agents).expect("write agents");

        let assigned = backfill_agent_short_ids(&state).expect("backfill agents");
        assert_eq!(assigned, agents.len());
        let after = state_read_agents_cached(&state).expect("read agents again");
        assert!(
            after.iter().all(|agent| agent.short_id.is_some()),
            "补号之后每个人都该有号"
        );
        let numbers: std::collections::BTreeSet<u64> =
            after.iter().filter_map(|agent| agent.short_id).collect();
        assert_eq!(numbers.len(), after.len(), "号不得重复");
        assert_eq!(
            backfill_agent_short_ids(&state).expect("again"),
            0,
            "重跑不得再分配"
        );
        let again = state_read_agents_cached(&state).expect("read agents third time");
        assert_eq!(
            again
                .iter()
                .map(|agent| (agent.id.clone(), agent.short_id))
                .collect::<Vec<_>>(),
            after
                .iter()
                .map(|agent| (agent.id.clone(), agent.short_id))
                .collect::<Vec<_>>(),
            "重跑不得改动已分配的号"
        );
    }

    /// 保存人格时同 id 必须沿用旧号：前端回传丢了 shortId 也不能重发。
    #[test]
    fn inherit_agent_short_ids_should_keep_previous_number_for_same_agent() {
        let state = test_chat_runtime_state();
        let mut previous = state_read_agents_cached(&state).expect("read agents");
        assert!(!previous.is_empty(), "默认状态应至少内置一个人格");
        for (index, agent) in previous.iter_mut().enumerate() {
            agent.short_id = Some(100 + index as u64);
        }
        let expected = previous
            .iter()
            .map(|agent| (agent.id.clone(), agent.short_id))
            .collect::<Vec<_>>();
        let mut incoming = previous.clone();
        for agent in &mut incoming {
            // 模拟前端回传：结构里没有 shortId 字段。
            agent.short_id = None;
        }
        let assigned =
            inherit_or_assign_agent_short_ids(&state, &previous, &mut incoming).expect("inherit");
        assert_eq!(assigned, 0, "同 id 不该重新分配");
        assert_eq!(
            incoming
                .iter()
                .map(|agent| (agent.id.clone(), agent.short_id))
                .collect::<Vec<_>>(),
            expected,
            "每个人都必须沿用旧号"
        );
    }

    /// 配置补号：模型配置与远程渠道各自按现有顺序补号，重跑幂等。
    #[test]
    fn backfill_app_config_short_ids_should_assign_in_order_and_be_idempotent() {
        let state = test_chat_runtime_state();
        let mut config = state_read_config_cached(&state).expect("read config");
        assert!(
            !config.api_configs.is_empty(),
            "默认配置应至少有一条模型配置"
        );
        for api in &mut config.api_configs {
            api.short_id = None;
        }
        for channel in &mut config.remote_im_channels {
            channel.short_id = None;
        }
        let expected = config.api_configs.len() + config.remote_im_channels.len();
        state_write_config_cached(&state, &config).expect("write config");

        assert_eq!(
            backfill_app_config_short_ids(&state).expect("backfill config"),
            expected
        );
        let after = state_read_config_cached(&state).expect("read config again");
        for (index, api) in after.api_configs.iter().enumerate() {
            assert_eq!(api.short_id, Some(index as u64), "模型配置按顺序补号");
        }
        assert!(
            after
                .remote_im_channels
                .iter()
                .all(|channel| channel.short_id.is_some()),
            "远程渠道也要补上号"
        );
        assert_eq!(
            backfill_app_config_short_ids(&state).expect("backfill again"),
            0,
            "重跑不得再分配"
        );
    }

    /// MCP 新建才分配：已有号原样保留，新服务按序取号。
    #[test]
    fn resolve_mcp_server_short_id_should_only_allocate_for_new_server() {
        let state = test_chat_runtime_state();
        assert_eq!(
            resolve_mcp_server_short_id(&state, "mcp-test-new-a", None).expect("resolve"),
            Some(0)
        );
        assert_eq!(
            resolve_mcp_server_short_id(&state, "mcp-test-new-b", None).expect("resolve"),
            Some(1)
        );
        // 已经带号（编辑既有服务）时原样沿用，不再取号。
        assert_eq!(
            resolve_mcp_server_short_id(&state, "mcp-test-new-a", Some(9)).expect("resolve"),
            Some(9)
        );
    }