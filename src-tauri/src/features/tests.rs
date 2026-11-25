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
                        retired_identities: Vec::new(),
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

    /// 打断必须当场把半成品收走落盘，不等生成协程收口。
    ///
    /// 构造的是「这一轮正在流式输出、容器里已有正文」的状态。停止命令返回时：
    /// 内容必须已经写进会话（persisted = true 并带回助理消息）、会话当场回到空闲，
    /// 并且这一轮的所有权已经被停止方收走——旧协程退出时认不上号，不会再二次落盘复位。
    #[test]
    fn stop_chat_message_should_persist_partial_content_immediately() {
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
                    chat_key,
                    InflightChatAbortEntry::Running {
                        handle: new_inflight_abort_handle(),
                        request_id: "req-stop-persist".to_string(),
                        assistant_message_id: "assistant-stop-persist".to_string(),
                        retired_identities: Vec::new(),
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
            assert!(result.persisted, "打断必须当场落盘，而不是等协程收口");
            assert_eq!(result.assistant_text, "流到一半的正文");
            assert_eq!(
                result
                    .assistant_message
                    .as_ref()
                    .map(|message| message.id.as_str()),
                Some("assistant-stop-persist"),
                "落盘写在本轮那条助理消息上"
            );

            // 内容真的进了会话存储：同一 id 上读回来就是这段半成品正文。
            let stored = conversation_service_v2()
                .read_message_by_id(&state, &conversation.id, "assistant-stop-persist")
                .expect("read stored message");
            let stored_text = stored.parts.iter().find_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.as_str()),
                _ => None,
            });
            assert_eq!(stored_text, Some("流到一半的正文"));

            // 会话当场就绪，所有权已交还：旧协程退出时不得再复位这一轮。
            assert_eq!(
                get_conversation_runtime_state(&state, &conversation.id).expect("read state"),
                MainSessionState::Idle
            );
            assert!(
                conversation_interrupt_persisted(&state, &conversation.id)
                    .expect("read interrupt record"),
                "应当留下「已就地落盘」的凭据"
            );
            assert!(
                !finish_conversation_round(&state, &conversation.id, "owner-stop-persist")
                    .expect("finish round"),
                "旧协程的收尾必须短路，不能再复位会话"
            );
        });
    }

    /// 认领只允许发生在「槽位持有运行中所有权」且「身份对得上」同时成立时。
    ///
    /// 两条边都不能松：所有权在、身份不对，认领就是误夺别人的轮次；
    /// 所有权不在、只是残留缓存身份恰好命中，认领就是把早就收完的旧内容再写一遍。
    #[test]
    fn claim_conversation_round_should_require_both_ownership_and_identity() {
        let state = test_chat_runtime_state();
        let conversation_id = "conversation-claim-guard";

        // 情况一：这一轮在跑（所有权在），但停止身份对不上。
        {
            let mut slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.entry(conversation_id.to_string()).or_default();
            slot.round_owner = "owner-running".to_string();
            slot.stream_cache.activation_id = "req-running".to_string();
            slot.stream_cache.request_id = "req-running".to_string();
            slot.stream_cache.persisted_assistant_message_id = "assistant-running".to_string();
        }
        let claimed = claim_conversation_round_for_interrupt(
            &state,
            conversation_id,
            &stop_identity("req-other", ""),
        )
        .expect("claim round");
        assert!(!claimed, "身份对不上时不许认领");
        {
            let slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.get(conversation_id).expect("slot");
            assert_eq!(slot.round_owner, "owner-running", "所有权不得被动");
            assert!(slot.stop_persisted_at_ms.is_none(), "不得留下落盘凭据");
        }

        // 情况二：这一轮已经收完（所有权已交还），残留缓存身份恰好命中。
        {
            let mut slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            slots
                .get_mut(conversation_id)
                .expect("slot")
                .round_owner
                .clear();
        }
        let claimed = claim_conversation_round_for_interrupt(
            &state,
            conversation_id,
            &stop_identity("req-running", "assistant-running"),
        )
        .expect("claim round");
        assert!(!claimed, "没有运行中的所有权时不许认领");
        assert!(
            lock_conversation_runtime_slots(&state)
                .expect("lock runtime slots")
                .get(conversation_id)
                .expect("slot")
                .stop_persisted_at_ms
                .is_none(),
            "不得留下落盘凭据"
        );
    }

    /// 前端乐观打断一个 id 都拿不到时，停止仍必须立下截止时间、掐断在跑的轮次并认领这一轮。
    ///
    /// 这是「不再靠调度身份判生死」的那条路：身份全空时，正确性由截止时间 + 会话所有权承担。
    #[test]
    fn stop_chat_message_without_identity_should_set_cutoff_and_abort_running_round() {
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
                        handle: new_inflight_abort_handle(),
                        request_id: "req-running".to_string(),
                        assistant_message_id: "assistant-running".to_string(),
                        retired_identities: Vec::new(),
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

            let before = now_unix_ms();
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

            assert!(result.aborted, "无身份的停止也必须掐断在跑的轮次");
            assert!(
                !inflight_has_running(&state, &chat_key),
                "在跑句柄必须被摘掉"
            );
            let cutoff = conversation_interrupt_cutoff_ms(&state, conversation_id)
                .expect("read cutoff")
                .expect("无身份的停止也必须立下截止时间");
            assert!(
                cutoff >= before,
                "截止时间必须由后端 now_unix_ms() 立下，实测 {cutoff} < {before}"
            );
            let slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.get(conversation_id).expect("slot");
            assert!(
                slot.round_owner.is_empty(),
                "无身份时以所有权为准认领，认领后必须交还所有权"
            );
            assert!(
                slot.stop_persisted_at_ms.is_some(),
                "认领后必须留下打断落盘凭据"
            );
        });
    }

    /// 被打断那一刻之前就在等的轮次，在激活门前就该被判死：不写空助理消息、不切流式。
    #[test]
    fn activate_main_assistant_should_block_round_older_than_interrupt_cutoff() {
        let runtime = test_runtime();
        runtime.block_on(async {
            let state = test_chat_runtime_state();
            let now = now_iso();
            let conversation = test_chat_conversation("conversation-cutoff-block", "active", &now);
            state_schedule_conversation_persist(&state, &conversation)
                .expect("persist conversation");

            let cutoff = now_unix_ms();
            mark_conversation_interrupt_cutoff(&state, &conversation.id, cutoff)
                .expect("mark cutoff");

            let err = activate_main_assistant(
                &state,
                &ChatSessionInfo {
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                },
                &conversation.id,
                None,
                None,
                None,
                None,
                Vec::new(),
                "",
                cutoff - 1,
            )
            .await
            .expect_err("旧于截止时间的轮次必须被门前拦下");
            assert_eq!(err, CHAT_ABORTED_BY_USER_ERROR, "拦截要走用户中止这条既有收尾");

            let messages = conversation_service_v2()
                .get_conversation_recent_messages(&state, &conversation.id, 5)
                .expect("read recent messages");
            assert!(
                messages.is_empty(),
                "门前拦截不得写入任何消息（含空助理消息），实测 {messages:?}"
            );
            assert_eq!(
                get_conversation_runtime_state(&state, &conversation.id).expect("read state"),
                MainSessionState::Idle,
                "门前拦截不得把会话切成流式"
            );
            let cache_activation_id = lock_conversation_runtime_slots(&state)
                .expect("lock runtime slots")
                .get(&conversation.id)
                .map(|slot| slot.stream_cache.activation_id.clone())
                .unwrap_or_default();
            assert!(
                cache_activation_id.is_empty(),
                "门前拦截不得建立流式缓存，实测 {cache_activation_id}"
            );
        });
    }

    /// 截止时间之后产生的轮次必须放行；同一毫秒算旧；没有截止时间或事件没带时间时不拦。
    /// 新轮次越过门卫、确权后截止时间作废。
    #[test]
    fn interrupt_cutoff_should_let_newer_round_through_and_clear_on_new_round() {
        let cutoff = 1_700_000_000_000u64;
        assert!(
            round_blocked_by_interrupt_cutoff(Some(cutoff), cutoff - 1),
            "更早的轮次必须拦"
        );
        assert!(
            round_blocked_by_interrupt_cutoff(Some(cutoff), cutoff),
            "同一毫秒也算旧，区间是闭的"
        );
        assert!(
            !round_blocked_by_interrupt_cutoff(Some(cutoff), cutoff + 1),
            "晚于截止时间的新轮次必须放行"
        );
        assert!(
            !round_blocked_by_interrupt_cutoff(None, cutoff),
            "没有截止时间时不拦"
        );
        assert!(
            !round_blocked_by_interrupt_cutoff(Some(cutoff), 0),
            "事件没带时间（旧数据缺字段）时不拦"
        );

        let state = test_chat_runtime_state();
        let conversation_id = "conversation-cutoff-reset";
        mark_conversation_interrupt_cutoff(&state, conversation_id, cutoff).expect("mark cutoff");
        assert_eq!(
            conversation_interrupt_cutoff_ms(&state, conversation_id)
                .expect("read cutoff")
                .expect("cutoff present"),
            cutoff
        );
        // 新轮次越过门卫、建立所有权后，这条水位作废。
        begin_conversation_round_owner(&state, conversation_id).expect("begin round");
        assert_eq!(
            conversation_interrupt_cutoff_ms(&state, conversation_id).expect("read cutoff"),
            None,
            "新轮次确权后截止时间必须复位"
        );
    }

    /// 确权之后、句柄登记之前被打断：表里没有句柄可中止，也必须把这次打断算成立。
    ///
    /// 这一段空隙里轮次已经在装配上下文但还没登记句柄，按句柄找必然落空；
    /// 若不看会话所有权，打断会被判成「什么也没停」，那一轮随后就把请求发给模型了。
    #[test]
    fn stop_chat_message_should_claim_round_before_handle_registered() {
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

            assert!(result.aborted, "没有句柄时也必须把这次打断算成立");
            let slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.get(conversation_id).expect("slot");
            assert!(slot.round_owner.is_empty(), "所有权必须当场收走");
            assert!(
                slot.stop_persisted_at_ms.is_some(),
                "必须留下打断落盘凭据，发请求前的自检才拦得住"
            );
            assert!(
                slot.interrupt_cutoff_ms.is_some(),
                "无身份的打断必须立下会话截止时间"
            );
        });
    }

    /// 发请求前的自检必须用会话槽里这一轮的真实启动时刻与所有权令牌，不能用发送入口的就地采样。
    ///
    /// 关键场景：本轮 `started_at_ms` 早于 `cutoff`，而发送入口的采样晚于 `cutoff`
    /// （确权 → 广播 → 装配上下文这段空隙里被打断）。就地采样会把它判成「停完之后的新轮次」
    /// 放行，槽内时基才拦得住。
    #[test]
    fn dispatch_should_short_circuit_when_round_already_interrupted() {
        let state = test_chat_runtime_state();
        let conversation_id = "conversation-dispatch-guard";

        // 没有任何打断：放行。
        assert!(
            !conversation_round_interrupted_since(&state, conversation_id, None)
                .expect("check interrupted"),
            "没有打断时不许短路"
        );

        {
            let mut slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.entry(conversation_id.to_string()).or_default();
            slot.round_owner = "owner-current".to_string();
            slot.stream_cache.started_at_ms = 1_000;
        }

        // 打断落在本轮启动之后（1000 <= 1100）：必须短路。
        // 注意这里没有传任何「入口时间」——判据只取槽里的启动时刻。
        mark_conversation_interrupt_cutoff(&state, conversation_id, 1_100).expect("mark cutoff");
        assert!(
            conversation_round_interrupted_since(&state, conversation_id, Some("owner-current"))
                .expect("check interrupted"),
            "打断不早于本轮启动时刻时必须短路"
        );

        // 打断早于本轮启动（停完才起的新轮次）：放行。
        mark_conversation_interrupt_cutoff(&state, conversation_id, 900).expect("mark cutoff");
        assert!(
            !conversation_round_interrupted_since(&state, conversation_id, Some("owner-current"))
                .expect("check interrupted"),
            "晚于截止时间启动的新轮次必须放行"
        );

        // 同一毫秒也算旧（区间闭）。
        mark_conversation_interrupt_cutoff(&state, conversation_id, 1_000).expect("mark cutoff");
        assert!(
            conversation_round_interrupted_since(&state, conversation_id, Some("owner-current"))
                .expect("check interrupted"),
            "启动时刻与截止时间同一毫秒也必须拦"
        );

        // 令牌已被剥离（停止认领会清空槽上的令牌）：即使时间对不上也必须拦。
        {
            let mut slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots
                .entry(conversation_id.to_string())
                .or_default();
            slot.round_owner.clear();
            slot.interrupt_cutoff_ms = None;
        }
        assert!(
            conversation_round_interrupted_since(&state, conversation_id, Some("owner-current"))
                .expect("check interrupted"),
            "令牌被剥离后必须短路，不许靠时间侥幸放行"
        );

        // 本轮已被停止收走（留下落盘凭据）：与时间、令牌无关，一律短路。
        {
            let mut slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            slots
                .entry(conversation_id.to_string())
                .or_default()
                .stop_persisted_at_ms = Some(1_300);
        }
        mark_conversation_interrupt_cutoff(&state, conversation_id, 0).expect("mark cutoff");
        assert!(
            conversation_round_interrupted_since(&state, conversation_id, None)
                .expect("check interrupted"),
            "已被停止收走的轮次不论时间一律短路"
        );
    }

    /// 端到端时序：主助理确权（T1）→ 中途被打断（认领、落盘、清缓存、留凭据）→
    /// 旧协程继续走进 send_chat_message_inner，必须在发请求前被拦下，
    /// 且不得把确权时的时基与凭据二次改写。
    ///
    /// 这条钉的是历史缺陷：发送入口曾经无条件重置流式缓存（把 started_at_ms 覆盖成更晚的
    /// 时间、并把打断凭据抹成 None），导致两道门栅同时失效、请求照样发给模型。
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
                        handle: new_inflight_abort_handle(),
                        request_id: "req-send-gate".to_string(),
                        assistant_message_id: "assistant-send-gate".to_string(),
                        retired_identities: Vec::new(),
                    },
                );

            // 打断：认领成功、落盘、会话置就绪、缓存被清、凭据留下。
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
            assert!(stop_result.aborted, "应当认领并停住这一轮");

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
            let send_err = send_chat_message_inner(request, &state, &channel)
                .await
                .expect_err("被打断的轮次不许走到模型请求");
            assert_eq!(send_err, CHAT_ABORTED_BY_USER_ERROR);

            let slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.get(&conversation.id).expect("slot");
            assert_eq!(
                slot.stream_cache.started_at_ms, 0,
                "停止已清空缓存，发送入口不得把它重写成更晚的时间（重写会让 cutoff 判定失效）"
            );
            assert!(
                slot.stop_persisted_at_ms.is_some(),
                "打断落盘凭据不许被发送入口抹掉"
            );
        });
    }

    /// 停止带了不匹配的身份时不许把「别人的轮次」当成停住了。
    ///
    /// 这一条钉的是身份隔离：句柄没命中、只是槽上有别的轮次在跑，`aborted` 必须为 false，
    /// 既不能报给前端「已中止」，也不能顺手压目标、清队员待办，更不能把那一轮的所有权收走。
    #[test]
    fn stop_with_mismatched_identity_should_not_report_aborted() {
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
                        handle: new_inflight_abort_handle(),
                        request_id: "req-actually-running".to_string(),
                        assistant_message_id: "assistant-actually-running".to_string(),
                        retired_identities: Vec::new(),
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
                // 身份对不上任何在跑的调度：这次停止没有对象。
                request_id: Some("req-stale".to_string()),
                activation_id: Some("req-stale".to_string()),
                assistant_message_id: Some("assistant-stale".to_string()),
            };
            let result = stop_chat_message_inner(input, &state)
                .await
                .expect("停止不应失败");

            assert!(!result.aborted, "身份对不上时不许报「已中止」");
            assert!(
                inflight_has_running(&state, &chat_key),
                "别人那一轮的句柄不许被摘掉"
            );
            assert!(
                !state
                    .goal_continue_suppressed_conversation_ids
                    .lock()
                    .expect("lock suppressed ids")
                    .contains(conversation_id),
                "没停住任何东西时不许压目标继续"
            );
            let slots = lock_conversation_runtime_slots(&state).expect("lock runtime slots");
            let slot = slots.get(conversation_id).expect("slot");
            assert_eq!(
                slot.round_owner, "owner-actually-running",
                "别人那一轮的所有权不许被收走"
            );
            assert!(
                slot.stop_persisted_at_ms.is_none(),
                "没认领成功就不许留下打断落盘凭据"
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
    fn request_stop_inflight_chat_hits_after_compaction_restart() {
        // 压缩重启会换用新的请求号与助理消息 id，但继续跑在同一次 run 里、句柄不变。
        // 前端采纳新身份后点停止，必须命中同一个句柄。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, handle.clone(), "req-1", "assistant-1")
            .expect("register");

        refresh_inflight_chat_dispatch_identity(&state, chat_key, "req-1", "req-2", "assistant-2")
            .expect("refresh identity");

        assert!(
            request_stop_inflight_chat(&state, chat_key, &stop_identity("req-2", "assistant-2"))
                .expect("stop with new identity"),
            "压缩重启后的新身份必须能停住这一轮"
        );
        assert!(handle.is_aborted(), "新身份命中后应立即中止");
        let inflight = state
            .inflight_chat_abort_handles
            .lock()
            .expect("lock inflight chat abort handles");
        assert!(inflight.get(chat_key).is_none(), "停止之后不得留下任何残留");
    }

    #[test]
    fn request_stop_inflight_chat_still_hits_with_retired_identity() {
        // 压缩边界的完成事件只走单发通道，Web / VS Code 收不到，那一端会丢弃新的轮次开始
        // 事件、一直持有压缩前的旧身份。旧号必须继续能停住同一轮，否则 Web 端会从
        // 「能停」退化成「不能停」。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, handle.clone(), "req-1", "assistant-1")
            .expect("register");
        refresh_inflight_chat_dispatch_identity(&state, chat_key, "req-1", "req-2", "assistant-2")
            .expect("refresh identity");

        assert!(
            request_stop_inflight_chat(&state, chat_key, &stop_identity("req-1", "assistant-1"))
                .expect("stop with retired identity"),
            "压缩前的旧身份仍代表同一轮，必须能停住"
        );
        assert!(handle.is_aborted(), "旧身份命中后也应立即中止");
    }

    #[test]
    fn refresh_inflight_chat_dispatch_identity_skips_entry_taken_over() {
        // 同一 key 上可能已被更晚一次调度接手：它不是我们的条目，不得改写其身份，
        // 否则那个调度按自己的号就停不掉了。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, handle.clone(), "req-2", "assistant-2")
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
            Some(InflightChatAbortEntry::Running { request_id, retired_identities, .. })
                if request_id == "req-2" && retired_identities.is_empty()
        ));
    }

    #[test]
    fn inflight_entry_is_ours_accepts_retired_identity() {
        // 收尾清理按本轮用过的任一身份认领自己；只比最初那个 trace_id 会留下残留表项，
        // 而残留的 Running 会让下一轮提交期的停止撞上它并对不上。
        let state = test_chat_runtime_state();
        let chat_key = "agent-1::conversation-1";
        let handle = new_inflight_abort_handle();
        register_inflight_chat_abort_handle(&state, chat_key, handle.clone(), "req-1", "assistant-1")
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
        let mut identity = stop_identity("req-pending", "");
        identity.assistant_message_id.clear();
        request_stop_inflight_chat(&state, chat_key, &identity).expect("record cancel request");

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

    // ==================== 全局短 ID 基建 ====================

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