    // ==================== 调度运行态与意外重启恢复 ====================

    fn active_dispatch_test_conversation(state: &AppState) -> String {
        let input = CreateUnarchivedConversationInput {
            api_config_id: None,
            agent_id: None,
            title: Some("调度运行态测试会话".to_string()),
            copy_source_conversation_id: None,
            shell_workspaces: None,
            shell_work_mode: None,
            shell_work_branch: None,
            shell_autonomous_mode: None,
            conversation_kind: None,
            is_draft: Some(true),
        };
        let conversation_id = conversation_service_v2()
            .create_conversation(state, &input)
            .expect("create conversation")
            .conversation_id;
        // 运行态表带外键，先把元数据行落进同一个库，测试才有可指向的会话。
        let conn = message_store::chat_metadata_store_open(&state.data_path).expect("open metadata");
        conn.execute(
            "INSERT OR IGNORE INTO conversation_metadata(conversation_id, metadata_json, storage_revision, updated_at) VALUES(?1, ?2, ?3, ?4)",
            rusqlite::params![conversation_id, "{}", 0, now_iso()],
        )
        .expect("insert conversation metadata");
        conversation_id
    }

    fn active_dispatch_records(state: &AppState) -> Vec<message_store::ActiveDispatchRecord> {
        message_store::active_dispatch_list_all(&state.data_path).expect("list active dispatches")
    }

    /// 写 → 改 → 删：一行记录的生命周期就这三步。
    #[test]
    fn active_dispatch_should_upsert_update_and_remove_one_row() {
        let state = test_chat_runtime_state();
        let conversation_id = active_dispatch_test_conversation(&state);

        message_store::active_dispatch_upsert(&state.data_path, &conversation_id, "[]")
            .expect("upsert empty");
        let records = active_dispatch_records(&state);
        assert_eq!(records.len(), 1, "写一次就该有一条");
        assert_eq!(records[0].conversation_id, conversation_id);
        assert_eq!(records[0].pending_queue_json, "[]");

        message_store::active_dispatch_upsert(&state.data_path, &conversation_id, "[1,2]")
            .expect("upsert again");
        let records = active_dispatch_records(&state);
        assert_eq!(records.len(), 1, "同一个会话重复写仍然只有一行");
        assert_eq!(records[0].pending_queue_json, "[1,2]", "后写的内容覆盖前一次");

        message_store::active_dispatch_remove(&state.data_path, &conversation_id).expect("remove");
        assert!(
            active_dispatch_records(&state).is_empty(),
            "删掉之后表里不该还剩这一行"
        );
    }

    /// 队列能被原样写出去、再原样读回来。
    #[test]
    fn active_dispatch_queue_json_should_round_trip() {
        let state = test_chat_runtime_state();
        let conversation_id = active_dispatch_test_conversation(&state);
        let message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "排队中的一条".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        };
        let event = ChatPendingEvent {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.clone(),
            created_at: now_iso(),
            created_at_ms: now_unix_ms(),
            source: ChatEventSource::System,
            queue_mode: ChatQueueMode::Normal,
            messages: vec![message],
            activate_assistant: false,
            assistant_message_id: None,
            session_info: ChatSessionInfo {
                agent_id: DEFAULT_AGENT_ID.to_string(),
            },
            runtime_context: None,
            sender_info: None,
        };
        let queue = std::collections::VecDeque::from(vec![event.clone()]);
        let json = serde_json::to_string(&queue).expect("serialize queue");
        message_store::active_dispatch_upsert(&state.data_path, &conversation_id, &json)
            .expect("upsert queue");

        let stored = active_dispatch_records(&state);
        assert_eq!(stored.len(), 1);
        let decoded: Vec<ChatPendingEvent> =
            serde_json::from_str(&stored[0].pending_queue_json).expect("deserialize queue");
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].id, event.id, "事件 id 必须原样回来");
        assert_eq!(
            decoded[0].conversation_id, conversation_id,
            "事件仍然指向同一个会话"
        );
        assert_eq!(
            decoded[0].messages.len(),
            1,
            "消息内容不能被序列化吃掉"
        );
    }

    /// 恢复用的系统引导消息：系统身份、引导模式、写完历史就激活助理、英文原文。
    #[test]
    fn recovery_guided_event_should_be_system_guided_and_english() {
        let state = test_chat_runtime_state();
        let conversation_id = active_dispatch_test_conversation(&state);
        let event = build_recovery_system_guided_event(&state, &conversation_id)
            .expect("build recovery event");

        assert!(
            matches!(event.source, ChatEventSource::System),
            "必须是系统身份"
        );
        assert_eq!(event.queue_mode, ChatQueueMode::Guided, "必须是引导模式");
        assert!(event.activate_assistant, "必须激活主助理");
        assert_eq!(event.messages.len(), 1);
        let text = match &event.messages[0].parts[0] {
            MessagePart::Text { text, .. } => text.as_str(),
            other => panic!("恢复消息必须是文本：{other:?}"),
        };
        assert_eq!(text, "System exited unexpectedly. Please continue.");
    }

    /// 收口时队列已空：运行态记录必须被清掉，不能留到下次启动误报。
    #[test]
    fn release_claim_should_clear_active_dispatch_when_queue_empty() {
        let state = test_chat_runtime_state();
        let conversation_id = active_dispatch_test_conversation(&state);
        message_store::active_dispatch_upsert(&state.data_path, &conversation_id, "[]")
            .expect("upsert");
        assert_eq!(active_dispatch_records(&state).len(), 1);

        release_conversation_processing_claim(&state, &conversation_id).expect("release claim");
        flush_active_dispatches_blocking().expect("flush active dispatches");
        assert!(
            active_dispatch_records(&state).is_empty(),
            "队列空了就该把运行态记录删掉"
        );
    }

    fn active_dispatch_test_event(conversation_id: &str, text: &str) -> ChatPendingEvent {
        let message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: text.to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        };
        ChatPendingEvent {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            created_at: now_iso(),
            created_at_ms: now_unix_ms(),
            source: ChatEventSource::System,
            queue_mode: ChatQueueMode::Normal,
            messages: vec![message],
            activate_assistant: false,
            assistant_message_id: None,
            session_info: ChatSessionInfo {
                agent_id: DEFAULT_AGENT_ID.to_string(),
            },
            runtime_context: None,
            sender_info: None,
        }
    }

    /// 恢复的顺序：先发起调度，再把崩溃前排着的话入队。
    /// 轮次在跑的时候队里的话不许被凭空出队；收口之后才轮到它们。
    #[test]
    fn recovery_should_start_dispatch_before_restoring_queue() {
        let state = test_chat_runtime_state();
        let conversation_id = active_dispatch_test_conversation(&state);

        let first = active_dispatch_test_event(&conversation_id, "崩溃前排队第一句");
        let second = active_dispatch_test_event(&conversation_id, "崩溃前排队第二句");
        let first_id = first.id.clone();

        // 第一步：发起调度。这时候内存队列还是空的，引导必须直接开跑。
        let event = build_recovery_system_guided_event(&state, &conversation_id)
            .expect("build recovery event");
        let ingress = ingress_chat_event(&state, event).expect("ingress");
        assert!(
            matches!(ingress, ChatEventIngress::Direct(_)),
            "队列空的时候，激活消息必须直接开跑，不能排进队列"
        );

        // 第二步：把崩溃前还排着的话放回队列。
        {
            let mut slots = lock_conversation_runtime_slots(&state).expect("lock slots");
            let slot = conversation_slot_mut(&mut slots, &conversation_id);
            slot.pending_queue.push_back(first);
            slot.pending_queue.push_back(second);
        }

        // 这一轮在跑：队列原样扣着，不许被出队器认领。
        let claimed = claim_queued_conversation_batches(&state).expect("claim while running");
        assert!(claimed.is_empty(), "轮次进行中不许从队列里取消息");

        // 收口之后才轮到队列，而且一次只出一条。
        release_conversation_processing_claim(&state, &conversation_id).expect("release");
        let claimed = claim_queued_conversation_batches(&state).expect("claim after release");
        assert_eq!(claimed.len(), 1, "收口之后这个会话才被认领");
        assert_eq!(claimed[0].1.len(), 1, "普通队列一次只出一条");
        assert_eq!(claimed[0].1[0].id, first_id, "先出崩溃前排队的第一句");
    }

    /// 队列只在内存里改，写盘由运行态自己跟着做：进队就有一行记录，清空就删掉。
    /// 调用方不许自己拼 JSON、自己碰表。
    #[test]
    fn queue_mutation_should_persist_itself_without_caller_writing_rows() {
        let state = test_chat_runtime_state();
        let conversation_id = active_dispatch_test_conversation(&state);

        {
            let mut slots = lock_conversation_runtime_slots(&state).expect("lock slots");
            let slot = conversation_slot_mut(&mut slots, &conversation_id);
            slot.pending_queue
                .push_back(active_dispatch_test_event(&conversation_id, "排队一句"));
        }
        sync_active_dispatch(&state, &conversation_id);
        flush_active_dispatches_blocking().expect("flush active dispatches");
        let records = active_dispatch_records(&state);
        assert_eq!(records.len(), 1, "队列里有话就必须留下一行记录");
        let decoded: Vec<ChatPendingEvent> =
            serde_json::from_str(&records[0].pending_queue_json).expect("decode queue");
        assert_eq!(decoded.len(), 1, "记下来的就是内存里的队列本身");

        {
            let mut slots = lock_conversation_runtime_slots(&state).expect("lock slots");
            let slot = conversation_slot_mut(&mut slots, &conversation_id);
            slot.pending_queue.clear();
        }
        sync_active_dispatch(&state, &conversation_id);
        flush_active_dispatches_blocking().expect("flush active dispatches");
        assert!(
            active_dispatch_records(&state).is_empty(),
            "队列空了、也没在跑，记录就该跟着删掉"
        );
    }

    /// 连改多次只落一次盘：写盘次数与操作次数脱钩，落地的是最后那一刻的完整队列。
    #[test]
    fn repeated_queue_mutation_should_merge_into_one_write() {
        let state = test_chat_runtime_state();
        let conversation_id = active_dispatch_test_conversation(&state);
        let _ = flush_active_dispatches_blocking();

        for index in 0..5usize {
            {
                let mut slots = lock_conversation_runtime_slots(&state).expect("lock slots");
                let slot = conversation_slot_mut(&mut slots, &conversation_id);
                slot.pending_queue.clear();
                for sentence in 0..=index {
                    slot.pending_queue.push_back(active_dispatch_test_event(
                        &conversation_id,
                        &format!("第{sentence}句"),
                    ));
                }
            }
            sync_active_dispatch(&state, &conversation_id);
        }

        reset_active_dispatch_write_count(&state.data_path);
        flush_active_dispatches_blocking().expect("flush active dispatches");
        assert_eq!(
            active_dispatch_write_count(&state.data_path),
            1,
            "五次改动必须合并成一次写盘"
        );

        let records = active_dispatch_records(&state);
        assert_eq!(records.len(), 1, "同一个会话只有一行");
        let decoded: Vec<ChatPendingEvent> =
            serde_json::from_str(&records[0].pending_queue_json).expect("decode queue");
        assert_eq!(decoded.len(), 5, "落盘的是最后一次那一刻的完整队列");
    }
