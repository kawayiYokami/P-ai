    // ==================== 会话提问：存储 / 工具 / 权限 / 答题 ====================

    fn question_test_state() -> AppState {
        test_chat_runtime_state()
    }

    fn question_test_conn(state: &AppState) -> rusqlite::Connection {
        question_store_open(&state.data_path).expect("open question store")
    }

    fn question_test_options(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn question_test_create_item(title: &str, options: &[&str]) -> QuestionUpdateItem {
        QuestionUpdateItem {
            number: None,
            title: Some(title.to_string()),
            background: None,
            options: Some(question_test_options(options)),
        }
    }

    fn question_test_conversation(id: &str, kind: &str) -> Conversation {
        let mut conversation = test_active_conversation_with_messages(Vec::new(), None);
        conversation.id = id.to_string();
        conversation.conversation_kind = kind.to_string();
        conversation
    }

    fn question_test_numbers(views: &[ConversationQuestionView]) -> Vec<u32> {
        views.iter().map(|view| view.number).collect()
    }

    // ==================== 存储 ====================

    #[test]
    fn question_store_should_not_recycle_number_after_delete() {
        let state = question_test_state();
        let mut conn = question_test_conn(&state);
        for title in ["题一", "题二", "题三"] {
            question_store_insert(&mut conn, "conv-a", title, "", &question_test_options(&["A"]))
                .expect("insert question");
        }
        let deleted = question_store_delete(&conn, "conv-a", 3).expect("delete #3");
        assert_eq!(deleted, 1);

        let created =
            question_store_insert(&mut conn, "conv-a", "题四", "", &question_test_options(&["A"]))
                .expect("insert after delete");
        assert_eq!(created.number, 4, "删掉 #3 之后新题必须是 #4，编号不回收");

        let numbers: Vec<u32> = question_store_list(&conn, "conv-a")
            .expect("list questions")
            .into_iter()
            .map(|item| item.number)
            .collect();
        assert_eq!(numbers, vec![1, 2, 4]);
    }

    #[test]
    fn question_store_unique_index_should_reject_duplicate_number() {
        let state = question_test_state();
        {
            let mut conn = question_test_conn(&state);
            question_store_insert(&mut conn, "conv-a", "题一", "", &question_test_options(&["A"]))
                .expect("insert question");
        }
        let conn = question_test_conn(&state);
        let err = conn
            .execute(
                "INSERT INTO conversation_question
                    (id, conversation_id, number, title, background, options_json, created_at, updated_at)
                 VALUES ('dup-id', 'conv-a', 1, '重复', '', '[]', 'now', 'now')",
                [],
            )
            .expect_err("同会话内编号冲突必须被唯一索引拒绝");
        assert!(
            err.to_string().to_uppercase().contains("UNIQUE"),
            "唯一索引未生效：{err}"
        );
    }

    #[test]
    fn question_store_number_should_scope_per_conversation() {
        let state = question_test_state();
        let mut conn = question_test_conn(&state);
        let first = question_store_insert(
            &mut conn,
            "conv-a",
            "A 会话第一题",
            "",
            &question_test_options(&["A"]),
        )
        .expect("insert a");
        let second = question_store_insert(
            &mut conn,
            "conv-b",
            "B 会话第一题",
            "",
            &question_test_options(&["B"]),
        )
        .expect("insert b");
        assert_eq!(first.number, 1);
        assert_eq!(second.number, 1, "编号作用域按会话切分，两个会话各自从 1 开始");

        let a_third = question_store_insert(
            &mut conn,
            "conv-a",
            "A 会话第二题",
            "",
            &question_test_options(&["A"]),
        )
        .expect("insert a again");
        assert_eq!(a_third.number, 2);
        assert_eq!(
            question_store_count(&conn, "conv-b").expect("count b"),
            1,
            "A 会话新增不影响 B 会话"
        );
    }

    #[test]
    fn question_store_list_should_isolate_conversations() {
        let state = question_test_state();
        let mut conn = question_test_conn(&state);
        question_store_insert(
            &mut conn,
            "conv-a",
            "A 会话的问题",
            "",
            &question_test_options(&["A"]),
        )
        .expect("insert a");

        let b_items = question_store_list(&conn, "conv-b").expect("list b");
        assert!(b_items.is_empty(), "A 会话的问题不能在 B 会话里看到");
        assert_eq!(question_store_count(&conn, "conv-b").expect("count b"), 0);
    }

    #[test]
    fn question_clear_conversation_should_remove_records_and_cursor() {
        let state = question_test_state();
        {
            let mut conn = question_test_conn(&state);
            question_store_insert(
                &mut conn,
                "conv-a",
                "题一",
                "",
                &question_test_options(&["A"]),
            )
            .expect("insert a");
            question_store_insert(
                &mut conn,
                "conv-b",
                "题二",
                "",
                &question_test_options(&["B"]),
            )
            .expect("insert b");
        }
        question_clear_conversation(&state, "conv-a");

        let conn = question_test_conn(&state);
        assert_eq!(question_store_count(&conn, "conv-a").expect("count a"), 0);
        let cursor: Option<i64> = conn
            .query_row(
                "SELECT next_number FROM question_next_number WHERE conversation_id = 'conv-a'",
                [],
                |row| row.get(0),
            )
            .optional()
            .expect("read cursor");
        assert!(cursor.is_none(), "会话删除后编号游标一并清掉");
        assert_eq!(
            question_store_count(&conn, "conv-b").expect("count b"),
            1,
            "清理只作用于被删除会话"
        );
    }

    // ==================== 工具：编号定位 ====================

    #[test]
    fn question_tool_update_without_number_should_create_next_number() {
        let state = question_test_state();
        let first = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("题一", &["A", "B"])],
        )
        .expect("create first");
        assert_eq!(question_test_numbers(&first), vec![1]);

        let second = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("题二", &["A"])],
        )
        .expect("create second");
        assert_eq!(question_test_numbers(&second), vec![1, 2]);
        assert_eq!(second[1].title, "题二");
    }

    #[test]
    fn question_tool_update_should_overwrite_existing_number() {
        let state = question_test_state();
        question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create");
        let updated = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[QuestionUpdateItem {
                number: Some(1),
                title: Some("改后的题一".to_string()),
                background: Some("背景".to_string()),
                options: Some(question_test_options(&["是", "否"])),
            }],
        )
        .expect("update");
        assert_eq!(updated.len(), 1);
        assert_eq!(updated[0].number, 1);
        assert_eq!(updated[0].title, "改后的题一");
        assert_eq!(updated[0].background, "背景");
        assert_eq!(updated[0].options, vec!["是".to_string(), "否".to_string()]);
    }

    #[test]
    fn question_tool_update_with_blank_fields_should_delete() {
        let state = question_test_state();
        question_apply_tool_mutations(
            &state,
            "conv-a",
            &[
                question_test_create_item("题一", &["A"]),
                question_test_create_item("题二", &["B"]),
            ],
        )
        .expect("create");
        let after_delete = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[QuestionUpdateItem {
                number: Some(1),
                title: None,
                background: None,
                options: None,
            }],
        )
        .expect("delete");
        assert_eq!(question_test_numbers(&after_delete), vec![2]);
    }

    #[test]
    fn question_tool_update_should_reject_missing_number_without_creating() {
        let state = question_test_state();
        question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create");

        let err = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[QuestionUpdateItem {
                number: Some(5),
                title: Some("不存在的编号".to_string()),
                background: None,
                options: Some(question_test_options(&["A"])),
            }],
        )
        .expect_err("编号不存在必须报错");
        assert!(err.contains("#5"), "报错要指明编号：{err}");

        let views = question_views_for_conversation(&state, "conv-a").expect("list");
        assert_eq!(
            question_test_numbers(&views),
            vec![1],
            "编号不存在时绝不允许顺手新建"
        );
    }

    #[test]
    fn question_tool_view_should_not_expose_internal_id() {
        let state = question_test_state();
        let views = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create");
        let value = serde_json::to_value(&views[0]).expect("serialize view");
        let object = value.as_object().expect("view is object");
        assert!(object.contains_key("number"), "必须带编号：{value}");
        assert!(
            !object.contains_key("id"),
            "对外视图不允许出现内部 id：{value}"
        );
        assert!(
            !object.contains_key("conversationId"),
            "对外视图不允许出现会话 id：{value}"
        );
    }

    #[test]
    fn question_tool_should_reject_more_than_max_questions() {
        let state = question_test_state();
        let creates = (1..=QUESTION_MAX_PER_CONVERSATION)
            .map(|index| question_test_create_item(&format!("题{index}"), &["A"]))
            .collect::<Vec<_>>();
        question_apply_tool_mutations(&state, "conv-a", &creates).expect("create up to max");

        let err = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("超出的题", &["A"])],
        )
        .expect_err("超过上限必须报错");
        assert!(
            err.contains(&QUESTION_MAX_PER_CONVERSATION.to_string()),
            "报错要说明上限：{err}"
        );
        let views = question_views_for_conversation(&state, "conv-a").expect("list");
        assert_eq!(views.len(), QUESTION_MAX_PER_CONVERSATION);
    }

    #[test]
    fn question_tool_should_reject_oversized_title_and_option() {
        let state = question_test_state();
        let long_title = "题".repeat(QUESTION_TITLE_MAX_CHARS + 1);
        let err = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item(&long_title, &["A"])],
        )
        .expect_err("超长标题必须报错");
        assert!(err.contains(&QUESTION_TITLE_MAX_CHARS.to_string()), "{err}");

        let long_option = "选".repeat(QUESTION_OPTION_MAX_CHARS + 1);
        let err = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("正常标题", &[long_option.as_str()])],
        )
        .expect_err("超长选项必须报错");
        assert!(err.contains(&QUESTION_OPTION_MAX_CHARS.to_string()), "{err}");

        let views = question_views_for_conversation(&state, "conv-a").expect("list");
        assert!(views.is_empty(), "校验失败时一条都不许落库");
    }

    #[test]
    fn question_tool_should_reject_empty_and_unknown_action() {
        let state = question_test_state();
        let err = question_run_tool(
            &state,
            "conv-a",
            QuestionToolRequest {
                action: String::new(),
                items: Vec::new(),
            },
        )
        .expect_err("缺少 action 必须报错");
        assert!(err.contains("action"), "{err}");

        let err = question_run_tool(
            &state,
            "conv-a",
            QuestionToolRequest {
                action: "append".to_string(),
                items: Vec::new(),
            },
        )
        .expect_err("未知 action 必须报错");
        assert!(err.contains("append"), "{err}");
    }

    // ==================== 权限：只有本地主会话 ====================

    #[test]
    fn question_tool_kind_check_should_only_allow_local_main_chat() {
        assert!(conversation_kind_is_local_main_chat(CONVERSATION_KIND_CHAT));
        assert!(!conversation_kind_is_local_main_chat(
            CONVERSATION_KIND_SIDE_CHAT
        ));
        assert!(!conversation_kind_is_local_main_chat(
            CONVERSATION_KIND_DELEGATE
        ));
        assert!(!conversation_kind_is_local_main_chat(
            CONVERSATION_KIND_REMOTE_IM_CONTACT
        ));
        assert!(!conversation_kind_is_local_main_chat(
            CONVERSATION_KIND_SYSTEM_NOTIFICATION
        ));
        assert!(
            !conversation_kind_is_local_main_chat(CONVERSATION_KIND_SIDE_CHAT)
                && conversation_kind_is_local_chat(CONVERSATION_KIND_SIDE_CHAT),
            "side_chat 属于本地普通会话，但不是主会话：判据必须是两套而不是一套"
        );
    }

    #[test]
    fn question_tool_should_reject_side_chat_conversation() {
        let state = question_test_state();
        let conversation = question_test_conversation("conv-side", CONVERSATION_KIND_SIDE_CHAT);
        state_write_conversation_cached(&state, &conversation).expect("write conversation");

        let err = builtin_question(
            &state,
            &format!("{DEFAULT_AGENT_ID}::conv-side"),
            QuestionToolRequest {
                action: "list".to_string(),
                items: Vec::new(),
            },
        )
        .expect_err("追问会话必须被拒");
        assert!(err.contains(CONVERSATION_KIND_SIDE_CHAT), "{err}");
    }

    #[test]
    fn question_tool_should_reject_delegate_conversation() {
        let state = question_test_state();
        let conversation = question_test_conversation("conv-delegate", CONVERSATION_KIND_DELEGATE);
        state_write_conversation_cached(&state, &conversation).expect("write conversation");

        let err = builtin_question(
            &state,
            &format!("{DEFAULT_AGENT_ID}::conv-delegate"),
            QuestionToolRequest {
                action: "list".to_string(),
                items: Vec::new(),
            },
        )
        .expect_err("队员会话必须被拒");
        assert!(err.contains(CONVERSATION_KIND_DELEGATE), "{err}");
    }

    #[test]
    fn question_tool_should_reject_remote_contact_conversation() {
        let state = question_test_state();
        let conversation =
            question_test_conversation("conv-remote", CONVERSATION_KIND_REMOTE_IM_CONTACT);
        state_write_conversation_cached(&state, &conversation).expect("write conversation");

        let err = builtin_question(
            &state,
            &format!("{DEFAULT_AGENT_ID}::conv-remote"),
            QuestionToolRequest {
                action: "list".to_string(),
                items: Vec::new(),
            },
        )
        .expect_err("远程联系人会话必须被拒");
        assert!(err.contains(CONVERSATION_KIND_REMOTE_IM_CONTACT), "{err}");
    }

    #[test]
    fn question_tool_should_allow_local_main_chat_conversation() {
        let state = question_test_state();
        let conversation = question_test_conversation("conv-main", CONVERSATION_KIND_CHAT);
        state_write_conversation_cached(&state, &conversation).expect("write conversation");

        let text = builtin_question(
            &state,
            &format!("{DEFAULT_AGENT_ID}::conv-main"),
            QuestionToolRequest {
                action: "update".to_string(),
                items: vec![question_test_create_item("要不要现在发布？", &["现在发布", "再等等"])],
            },
        )
        .expect("主会话必须可用");
        assert!(text.contains("#1 要不要现在发布？"), "{text}");

        let listed = builtin_question(
            &state,
            &format!("{DEFAULT_AGENT_ID}::conv-main"),
            QuestionToolRequest {
                action: "list".to_string(),
                items: Vec::new(),
            },
        )
        .expect("list in main chat");
        assert!(listed.contains("共 1 条"), "{listed}");
    }

    // ==================== 答题消息构造 ====================

    #[test]
    fn question_answer_message_should_carry_number_and_original_title() {
        let message = build_question_answer_message(
            "【问题答复 #2】要不要现在发布？\n→ 现在发布",
            &[2],
        );
        assert_eq!(message.role, "user");
        let text = message
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
        assert!(text.contains("【问题答复 #2】"), "{text}");
        assert!(text.contains("要不要现在发布？"), "正文必须带问题原文：{text}");
        assert!(text.contains("→ 现在发布"), "{text}");
    }

    #[test]
    fn question_answer_message_should_mark_kind_and_numbers_in_provider_meta() {
        let message = build_question_answer_message("正文", &[3, 5]);
        let meta = message.provider_meta.expect("provider_meta");
        let message_meta = meta.get("message_meta").expect("message_meta");
        assert_eq!(
            message_meta.get("kind").and_then(|value| value.as_str()),
            Some("question_answer")
        );
        assert_eq!(
            message_meta
                .get("question_numbers")
                .and_then(|value| value.as_array())
                .map(|items| items
                    .iter()
                    .filter_map(|item| item.as_u64())
                    .collect::<Vec<_>>()),
            Some(vec![3, 5])
        );
    }

    // ==================== 答题命令：删题 + 入队 ====================

    fn question_test_answer(number: u32, option_index: usize) -> QuestionAnswerInput {
        QuestionAnswerInput {
            number,
            option_index: Some(option_index),
            custom_text: None,
        }
    }

    fn question_test_custom_answer(number: u32, text: &str) -> QuestionAnswerInput {
        QuestionAnswerInput {
            number,
            option_index: None,
            custom_text: Some(text.to_string()),
        }
    }

    /// 建立一个可答题的本地主会话，并把它按在流式中：答题事件会进队列而不是立刻被消费，
    /// 这样测试能稳定读到事件本身（source / queue_mode / messages）。
    fn question_test_answerable_conversation(state: &AppState, conversation_id: &str) {
        let conversation = question_test_conversation(conversation_id, CONVERSATION_KIND_CHAT);
        state_write_conversation_cached(state, &conversation).expect("write conversation");
        set_conversation_runtime_state(state, conversation_id, MainSessionState::AssistantStreaming)
            .expect("set streaming state");
    }

    fn question_test_last_queued_event(
        state: &AppState,
        conversation_id: &str,
    ) -> ChatPendingEvent {
        let slots = state
            .conversation_runtime_slots
            .lock()
            .expect("lock conversation slots");
        let slot = slots.get(conversation_id).expect("conversation slot");
        slot.pending_queue
            .back()
            .cloned()
            .expect("queued answer event")
    }

    #[test]
    fn question_answer_should_remove_question_and_queue_normal_user_event() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("要不要现在发布？", &["现在发布", "再等等"])],
        )
        .expect("create question");

        let result = question_answer_inner(
            &state,
            "conv-main",
            vec![question_test_answer(1, 0)],
        )
        .expect("answer question");
        assert_eq!(result.answered_numbers, vec![1]);
        assert!(result.remaining.is_empty(), "答完必须从列表消失");
        assert!(
            result.message_text.contains("【问题答复 #1】要不要现在发布？"),
            "{}",
            result.message_text
        );
        assert!(result.message_text.contains("→ 现在发布"), "{}", result.message_text);

        let event = question_test_last_queued_event(&state, "conv-main");
        assert!(
            matches!(event.source, ChatEventSource::User),
            "答题消息必须以用户来源入队，实际来源={:?}",
            event.source
        );
        assert_eq!(
            event.queue_mode,
            ChatQueueMode::Normal,
            "答题走普通队列，不允许用引导截断当前轮次"
        );
        assert!(event.activate_assistant);
        assert_eq!(event.messages.len(), 1);
        let message = &event.messages[0];
        assert_eq!(message.role, "user");
        let message_meta = message
            .provider_meta
            .as_ref()
            .and_then(|meta| meta.get("message_meta"))
            .expect("message_meta");
        assert_eq!(
            message_meta.get("kind").and_then(|value| value.as_str()),
            Some("question_answer")
        );
        assert_eq!(
            message_meta
                .get("question_number")
                .and_then(|value| value.as_u64()),
            Some(1)
        );

        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert!(views.is_empty(), "答完列表必须为空");
    }

    #[test]
    fn question_answer_should_not_recycle_number_after_answering() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create question");
        question_answer_inner(&state, "conv-main", vec![question_test_answer(1, 0)])
            .expect("answer question");

        let views = question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题二", &["B"])],
        )
        .expect("create after answer");
        assert_eq!(
            question_test_numbers(&views),
            vec![2],
            "答掉 #1 之后新题必须是 #2，编号不回收"
        );
    }

    #[test]
    fn question_answer_should_reject_unknown_number_and_keep_question() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create question");

        let err = question_answer_inner(&state, "conv-main", vec![question_test_answer(5, 0)])
            .expect_err("答不存在的编号必须报错");
        assert!(err.contains("#5"), "{err}");

        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(question_test_numbers(&views), vec![1], "报错时题目必须还在");
    }

    #[test]
    fn question_answer_should_build_blocks_in_ascending_number_order() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[
                question_test_create_item("题一", &["A1", "A2"]),
                question_test_create_item("题二", &["B1", "B2"]),
            ],
        )
        .expect("create questions");

        let result = question_answer_inner(
            &state,
            "conv-main",
            vec![question_test_answer(2, 1), question_test_answer(1, 0)],
        )
        .expect("answer both");
        assert_eq!(result.answered_numbers, vec![1, 2], "多题按编号升序答复");

        let first = result
            .message_text
            .find("【问题答复 #1】")
            .expect("block 1");
        let second = result
            .message_text
            .find("【问题答复 #2】")
            .expect("block 2");
        assert!(first < second, "正文顺序必须是 #1 在 #2 之前");
        assert!(result.message_text.contains("→ A1"), "{}", result.message_text);
        assert!(result.message_text.contains("→ B2"), "{}", result.message_text);
        assert!(result.remaining.is_empty());
    }

    #[test]
    fn question_answer_should_reject_duplicate_number_in_one_submit() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create question");

        let err = question_answer_inner(
            &state,
            "conv-main",
            vec![question_test_answer(1, 0), question_test_answer(1, 0)],
        )
        .expect_err("同一编号重复提交必须报错");
        assert!(err.contains("#1"), "{err}");
        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(views.len(), 1);
    }

    #[test]
    fn question_answer_should_reject_answer_without_option_and_text() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create question");

        let err = question_answer_inner(
            &state,
            "conv-main",
            vec![QuestionAnswerInput {
                number: 1,
                option_index: None,
                custom_text: None,
            }],
        )
        .expect_err("既没选项也没自己写的答案必须报错");
        assert!(err.contains("#1"), "{err}");
    }

    #[test]
    fn question_answer_should_reject_out_of_range_option_index() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create question");

        let err = question_answer_inner(&state, "conv-main", vec![question_test_answer(1, 3)])
            .expect_err("选项下标越界必须报错");
        assert!(err.contains("#1"), "{err}");
        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(views.len(), 1, "越界时题目不能被删掉");
    }

    #[test]
    fn question_answer_should_reject_side_chat_conversation() {
        let state = question_test_state();
        let conversation = question_test_conversation("conv-side", CONVERSATION_KIND_SIDE_CHAT);
        state_write_conversation_cached(&state, &conversation).expect("write conversation");

        let err = question_answer_inner(&state, "conv-side", vec![question_test_answer(1, 0)])
            .expect_err("追问会话不能答题");
        assert!(err.contains("本地主会话"), "{err}");
    }

    #[test]
    fn question_answer_should_queue_one_message_per_answered_question() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[
                question_test_create_item("题一", &["A1"]),
                question_test_create_item("题二", &["B1"]),
            ],
        )
        .expect("create questions");

        question_answer_inner(&state, "conv-main", vec![question_test_answer(1, 0)])
            .expect("answer first");
        question_answer_inner(&state, "conv-main", vec![question_test_answer(2, 0)])
            .expect("answer second");

        let slots = state
            .conversation_runtime_slots
            .lock()
            .expect("lock conversation slots");
        let slot = slots.get("conv-main").expect("conversation slot");
        assert_eq!(
            slot.pending_queue.len(),
            2,
            "逐题提交必须一题一条消息，不是攒成一包"
        );
        let texts = slot
            .pending_queue
            .iter()
            .map(|event| {
                assert_eq!(event.messages.len(), 1, "每条事件只带一道题的答复");
                assert_eq!(event.queue_mode, ChatQueueMode::Normal);
                event
                    .messages
                    .iter()
                    .filter_map(|message| match &message.parts[0] {
                        MessagePart::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert!(texts[0].contains("【问题答复 #1】题一"), "{:?}", texts[0]);
        assert!(texts[1].contains("【问题答复 #2】题二"), "{:?}", texts[1]);
    }

    #[test]
    fn question_answer_should_keep_unselected_questions_when_sending_subset() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[
                question_test_create_item("题一", &["A1"]),
                question_test_create_item("题二", &["B1"]),
                question_test_create_item("题三", &["C1"]),
            ],
        )
        .expect("create questions");

        // 第一轮：只发 #1 和 #3，#2 没选，必须留在面板上。
        let first = question_answer_inner(
            &state,
            "conv-main",
            vec![question_test_answer(1, 0), question_test_answer(3, 0)],
        )
        .expect("send subset");
        assert_eq!(first.answered_numbers, vec![1, 3]);
        assert_eq!(
            question_test_numbers(&first.remaining),
            vec![2],
            "没选的题不能被一起删掉"
        );
        assert!(
            first.message_text.contains("【问题答复 #1】") && first.message_text.contains("【问题答复 #3】"),
            "{}",
            first.message_text
        );

        // 第二轮：把剩下的 #2 发出去。
        let second = question_answer_inner(&state, "conv-main", vec![question_test_answer(2, 0)])
            .expect("send remaining");
        assert!(second.remaining.is_empty());

        let slots = state
            .conversation_runtime_slots
            .lock()
            .expect("lock conversation slots");
        let slot = slots.get("conv-main").expect("conversation slot");
        assert_eq!(slot.pending_queue.len(), 2, "两轮发送是两条独立事件");
        let texts = slot
            .pending_queue
            .iter()
            .map(|event| {
                event
                    .messages
                    .iter()
                    .filter_map(|message| match &message.parts[0] {
                        MessagePart::Text { text, .. } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        assert!(texts[0].contains("【问题答复 #1】") && texts[0].contains("【问题答复 #3】"), "{:?}", texts[0]);
        assert!(!texts[0].contains("【问题答复 #2】"), "第一轮不该带上没选的 #2");
        assert!(texts[1].contains("【问题答复 #2】"), "{:?}", texts[1]);
    }

    #[test]
    fn question_answer_should_reject_empty_answers() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        let err = question_answer_inner(&state, "conv-main", Vec::new())
            .expect_err("空答案必须报错");
        assert!(err.contains("答案"), "{err}");
    }

    // ==================== 自定义答案：走同一条答题链路 ====================

    #[test]
    fn question_answer_should_carry_custom_text_into_message() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[
                question_test_create_item("要不要现在发布？", &["现在发布", "再等等"]),
                question_test_create_item("用哪个分支？", &["main", "release"]),
            ],
        )
        .expect("create questions");

        let result = question_answer_inner(
            &state,
            "conv-main",
            vec![question_test_custom_answer(2, "  两个都不对，改成灰度发布  ")],
        )
        .expect("answer with custom text");

        assert_eq!(result.answered_numbers, vec![2]);
        assert!(
            result.message_text.contains("【问题答复 #2】用哪个分支？"),
            "{}",
            result.message_text
        );
        assert!(
            result.message_text.contains("→ 两个都不对，改成灰度发布"),
            "正文必须带用户自己写的那段，且去掉首尾空白：{}",
            result.message_text
        );
        assert!(
            !result.message_text.contains("→ main"),
            "自己写答案不能混进给定选项：{}",
            result.message_text
        );

        assert_eq!(
            question_test_numbers(&result.remaining),
            vec![1],
            "答掉 #2 之后只剩 #1"
        );
        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(question_test_numbers(&views), vec![1], "被答的题必须从库里删掉");

        let event = question_test_last_queued_event(&state, "conv-main");
        assert!(matches!(event.source, ChatEventSource::User));
        assert_eq!(event.queue_mode, ChatQueueMode::Normal);
        assert_eq!(event.messages.len(), 1);
        let message = &event.messages[0];
        assert_eq!(
            message
                .provider_meta
                .as_ref()
                .and_then(|meta| meta.get("message_meta"))
                .and_then(|meta| meta.get("kind"))
                .and_then(|value| value.as_str()),
            Some("question_answer"),
            "自定义答案与点选项必须是同一条消息通道、同一个 kind"
        );
    }

    #[test]
    fn question_answer_should_reject_blank_custom_text() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create question");

        let err = question_answer_inner(&state, "conv-main", vec![question_test_custom_answer(1, "   ")])
            .expect_err("空内容不许提交");
        assert!(err.contains("#1"), "{err}");
        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(views.len(), 1, "被挡住时题目不能被删掉");
    }

    #[test]
    fn question_answer_should_reject_option_and_custom_text_together() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create question");

        let err = question_answer_inner(
            &state,
            "conv-main",
            vec![QuestionAnswerInput {
                number: 1,
                option_index: Some(0),
                custom_text: Some("我自己写".to_string()),
            }],
        )
        .expect_err("选项与自己写同时给必须报错");
        assert!(err.contains("#1"), "{err}");
        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(views.len(), 1, "歧义输入不能被当成答案吃掉题目");
    }

    #[test]
    fn question_answer_should_reject_too_long_custom_text() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create question");

        let long_text = "答".repeat(QUESTION_CUSTOM_ANSWER_MAX_CHARS + 1);
        let err = question_answer_inner(
            &state,
            "conv-main",
            vec![question_test_custom_answer(1, &long_text)],
        )
        .expect_err("超长自定义答案必须报错");
        assert!(err.contains("上限"), "{err}");
    }

    // ==================== 拒绝回答：只删题，不产生消息 ====================

    fn question_test_pending_queue_is_empty(state: &AppState, conversation_id: &str) -> bool {
        let slots = state
            .conversation_runtime_slots
            .lock()
            .expect("lock conversation slots");
        match slots.get(conversation_id) {
            Some(slot) => slot.pending_queue.is_empty(),
            None => true,
        }
    }

    #[test]
    fn question_reject_should_remove_only_that_question() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[
                question_test_create_item("题一", &["A"]),
                question_test_create_item("题二", &["B"]),
                question_test_create_item("题三", &["C"]),
            ],
        )
        .expect("create questions");

        let remaining = question_reject_inner(&state, "conv-main", 2).expect("reject question");
        assert_eq!(
            question_test_numbers(&remaining),
            vec![1, 3],
            "只删被拒的那一题，其余原样返回"
        );
        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(
            question_test_numbers(&views),
            vec![1, 3],
            "库里的题必须同步减少"
        );
    }

    #[test]
    fn question_reject_should_not_write_any_message() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create question");

        let before = state_read_conversation_cached(&state, "conv-main")
            .expect("read conversation")
            .messages
            .len();
        question_reject_inner(&state, "conv-main", 1).expect("reject question");
        let after = state_read_conversation_cached(&state, "conv-main")
            .expect("read conversation")
            .messages
            .len();

        assert_eq!(before, after, "拒绝不许往会话历史里写任何消息");
        assert!(
            question_test_pending_queue_is_empty(&state, "conv-main"),
            "拒绝不许入队任何事件"
        );
    }

    #[test]
    fn question_reject_should_return_empty_when_every_question_rejected() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[
                question_test_create_item("题一", &["A"]),
                question_test_create_item("题二", &["B"]),
            ],
        )
        .expect("create questions");

        let first = question_reject_inner(&state, "conv-main", 1).expect("reject #1");
        assert_eq!(question_test_numbers(&first), vec![2]);
        let second = question_reject_inner(&state, "conv-main", 2).expect("reject #2");
        assert!(second.is_empty(), "全部拒绝后剩余必须为空，面板自行消失");
    }

    #[test]
    fn question_reject_should_reject_unknown_number_and_keep_question() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create question");

        let err = question_reject_inner(&state, "conv-main", 5).expect_err("未知编号必须报错");
        assert!(err.contains("#5 不存在"), "{err}");
        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(
            question_test_numbers(&views),
            vec![1],
            "拒绝失败不许吃掉既有题目"
        );
    }

    #[test]
    fn question_reject_should_reject_zero_number() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        let err = question_reject_inner(&state, "conv-main", 0).expect_err("0 不是合法编号");
        assert!(err.contains("0 不是合法编号"), "{err}");
    }

    #[test]
    fn question_reject_should_reject_side_chat_conversation() {
        let state = question_test_state();
        let conversation = question_test_conversation("conv-side", CONVERSATION_KIND_SIDE_CHAT);
        state_write_conversation_cached(&state, &conversation).expect("write conversation");

        let err = question_reject_inner(&state, "conv-side", 1).expect_err("追问会话不能拒绝");
        assert!(err.contains("本地主会话"), "{err}");
    }

