    fn question_test_state() -> AppState {
        test_chat_runtime_state()
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

    /// 问题挂在会话自己的元数据里，所以每个用例先得有这样一条会话。
    fn question_test_seed_conversation(state: &AppState, id: &str) {
        let conversation = question_test_conversation(id, CONVERSATION_KIND_CHAT);
        state_write_conversation_cached(state, &conversation).expect("write conversation");
    }

    fn question_test_numbers(views: &[ConversationQuestionView]) -> Vec<u32> {
        views.iter().map(|view| view.number).collect()
    }

    // ==================== 编号 ====================

    /// 直接改题本身：本条与本次改动无关，用它删掉最高编号再建新题。
    fn question_test_delete_item(number: u32) -> QuestionUpdateItem {
        QuestionUpdateItem {
            number: Some(number),
            title: None,
            background: None,
            options: None,
        }
    }

    #[test]
    fn question_should_not_recycle_number_after_delete() {
        let state = question_test_state();
        question_test_seed_conversation(&state, "conv-a");
        for title in ["题一", "题二", "题三"] {
            question_apply_tool_mutations(
                &state,
                "conv-a",
                &[question_test_create_item(title, &["A"])],
            )
            .expect("create question");
        }
        question_apply_tool_mutations(&state, "conv-a", &[question_test_delete_item(3)])
            .expect("delete #3");

        let views = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("题四", &["A"])],
        )
        .expect("create after delete");
        assert_eq!(
            question_test_numbers(&views),
            vec![1, 2, 4],
            "删掉 #3 之后新题必须是 #4，编号不回收"
        );
    }

    #[test]
    fn question_number_should_be_unique_and_never_reused() {
        // 编号唯一性不再由数据库唯一索引保证，改由「从游标取值、分配即自增」保证。
        // 这里连续建满上限条，逐个断言既不重复也不跳号。
        let state = question_test_state();
        question_test_seed_conversation(&state, "conv-a");
        let creates = (1..=QUESTION_MAX_PER_CONVERSATION)
            .map(|index| question_test_create_item(&format!("题{index}"), &["A"]))
            .collect::<Vec<_>>();
        question_apply_tool_mutations(&state, "conv-a", &creates).expect("create all");
        let views = question_views_for_conversation(&state, "conv-a").expect("list");
        assert_eq!(
            question_test_numbers(&views),
            (1..=QUESTION_MAX_PER_CONVERSATION as u32).collect::<Vec<_>>(),
            "编号必须逐个 +1，不重复也不跳号"
        );
    }

    #[test]
    fn question_number_should_scope_per_conversation() {
        let state = question_test_state();
        question_test_seed_conversation(&state, "conv-a");
        question_test_seed_conversation(&state, "conv-b");
        let a_first = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("A 会话第一题", &["A"])],
        )
        .expect("create a");
        let b_first = question_apply_tool_mutations(
            &state,
            "conv-b",
            &[question_test_create_item("B 会话第一题", &["B"])],
        )
        .expect("create b");
        assert_eq!(question_test_numbers(&a_first), vec![1]);
        assert_eq!(
            question_test_numbers(&b_first),
            vec![1],
            "编号作用域按会话切分，两个会话各自从 1 开始"
        );

        let a_second = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("A 会话第二题", &["A"])],
        )
        .expect("create a again");
        assert_eq!(question_test_numbers(&a_second), vec![1, 2]);
        assert_eq!(
            question_test_numbers(&question_views_for_conversation(&state, "conv-b").expect("b")),
            vec![1],
            "A 会话新增不影响 B 会话"
        );
    }

    #[test]
    fn question_list_should_isolate_conversations() {
        let state = question_test_state();
        question_test_seed_conversation(&state, "conv-a");
        question_test_seed_conversation(&state, "conv-b");
        question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("A 会话的问题", &["A"])],
        )
        .expect("create a");

        assert!(
            question_views_for_conversation(&state, "conv-b")
                .expect("list b")
                .is_empty(),
            "A 会话的问题不能在 B 会话里看到"
        );
    }

    #[test]
    fn question_should_vanish_with_conversation() {
        // 问题跟着会话走：会话删了，它名下的问题跟着消失，不需要另行清理。
        let state = question_test_state();
        question_test_seed_conversation(&state, "conv-a");
        question_test_seed_conversation(&state, "conv-b");
        question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("题一", &["A"])],
        )
        .expect("create a");
        question_apply_tool_mutations(
            &state,
            "conv-b",
            &[question_test_create_item("题二", &["B"])],
        )
        .expect("create b");

        conversation_service_v2()
            .delete_conversation(&state, "conv-a")
            .expect("delete conversation");

        assert!(
            question_views_for_conversation(&state, "conv-a").is_err(),
            "会话没了，它的问题也读不出来"
        );
        assert_eq!(
            question_test_numbers(&question_views_for_conversation(&state, "conv-b").expect("b")),
            vec![1],
            "别的会话不受影响"
        );
    }

    #[test]
    fn question_should_survive_flush_to_disk() {
        // 写入只改缓存，落盘是后置的：把问题连同编号游标一起跑到盘上再读回来。
        let state = question_test_state();
        let conversation = question_test_conversation("conv-disk", CONVERSATION_KIND_CHAT);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation).expect("mark persisted");

        question_apply_tool_mutations(
            &state,
            "conv-disk",
            &[question_test_create_item("落盘的问题", &["A", "B"])],
        )
        .expect("create question");
        assert!(flush_pending_persists_blocking(&state).expect("flush"), "必须真的落盘");

        let restored = read_conversation_shard(&state.data_path, "conv-disk").expect("read back");
        assert_eq!(restored.questions.len(), 1);
        assert_eq!(restored.questions[0].number, 1);
        assert_eq!(restored.questions[0].title, "落盘的问题");
        assert_eq!(restored.questions[0].options.len(), 2);
        assert_eq!(restored.question_next_number, 2, "编号游标也要一起落盘");
    }

    // ==================== 工具：编号定位 ====================

    #[test]
    fn question_tool_update_without_number_should_create_next_number() {
        let state = question_test_state();
        question_test_seed_conversation(&state, "conv-a");
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
        question_test_seed_conversation(&state, "conv-a");
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
        question_test_seed_conversation(&state, "conv-a");
        question_apply_tool_mutations(
            &state,
            "conv-a",
            &[
                question_test_create_item("题一", &["A"]),
                question_test_create_item("题二", &["B"]),
            ],
        )
        .expect("create");
        let after_delete =
            question_apply_tool_mutations(&state, "conv-a", &[question_test_delete_item(1)])
                .expect("delete");
        assert_eq!(question_test_numbers(&after_delete), vec![2]);
    }

    #[test]
    fn question_tool_update_should_reject_missing_number_without_creating() {
        let state = question_test_state();
        question_test_seed_conversation(&state, "conv-a");
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
        question_test_seed_conversation(&state, "conv-a");
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
        question_test_seed_conversation(&state, "conv-a");
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
        assert_eq!(
            views.len(),
            QUESTION_MAX_PER_CONVERSATION,
            "被拒之后题目数量不变"
        );
    }

    #[test]
    fn question_tool_should_reject_oversized_title_and_option() {
        let state = question_test_state();
        question_test_seed_conversation(&state, "conv-a");
        let long_title = "题".repeat(QUESTION_TITLE_MAX_CHARS + 1);
        let err = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item(&long_title, &["A"])],
        )
        .expect_err("超长标题必须报错");
        assert!(
            err.contains(&QUESTION_TITLE_MAX_CHARS.to_string()),
            "报错要说明具体上限：{err}"
        );

        let long_option = "选".repeat(QUESTION_OPTION_MAX_CHARS + 1);
        let err = question_apply_tool_mutations(
            &state,
            "conv-a",
            &[question_test_create_item("题一", &[&long_option])],
        )
        .expect_err("超长选项必须报错");
        assert!(
            err.contains(&QUESTION_OPTION_MAX_CHARS.to_string()),
            "报错要说明具体上限：{err}"
        );

        let views = question_views_for_conversation(&state, "conv-a").expect("list");
        assert!(
            views.is_empty(),
            "两次都被拒，一条都不许落库，实际落了 {} 条",
            views.len()
        );
    }

    #[test]
    fn question_tool_should_reject_empty_and_unknown_action() {
        let state = question_test_state();
        question_test_seed_conversation(&state, "conv-a");
        let err = question_run_tool(
            &state,
            "conv-a",
            QuestionToolRequest {
                action: String::new(),
                items: Vec::new(),
            },
        )
        .expect_err("空 action 必须报错");
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
        // 回执不再回放提交的标题与选项，只报条数
        assert!(text.contains("updated, 1 question"), "{text}");

        let listed = builtin_question(
            &state,
            &format!("{DEFAULT_AGENT_ID}::conv-main"),
            QuestionToolRequest {
                action: "list".to_string(),
                items: Vec::new(),
            },
        )
        .expect("list in main chat");
        assert!(listed.contains("1 question"), "{listed}");
    }

    // ==================== 答题命令：回正文 + 删题 ====================

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

    /// 建立一个可答题的本地主会话。答题命令本身不入队任何消息，
    /// 这条会话只需要能通过「本地主会话」判定与元数据读写。
    fn question_test_answerable_conversation(state: &AppState, conversation_id: &str) {
        let conversation = question_test_conversation(conversation_id, CONVERSATION_KIND_CHAT);
        state_write_conversation_cached(state, &conversation).expect("write conversation");
        set_conversation_runtime_state(state, conversation_id, MainSessionState::AssistantStreaming)
            .expect("set streaming state");
    }

    /// 当前会话待消费队列的条数：答题不允许往里塞任何东西。
    fn question_test_pending_queue_len(state: &AppState, conversation_id: &str) -> usize {
        let slots = state
            .conversation_runtime_slots
            .lock()
            .expect("lock conversation slots");
        slots
            .get(conversation_id)
            .map(|slot| slot.pending_queue.len())
            .unwrap_or(0)
    }

    #[test]
    fn question_answer_should_remove_question_and_not_queue_any_event() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        question_apply_tool_mutations(
            &state,
            "conv-main",
            &[question_test_create_item("要不要现在发布？", &["现在发布", "再等等"])],
        )
        .expect("create question");

        let result =
            question_answer_inner(&state, "conv-main", vec![question_test_answer(1, 0)])
                .expect("answer question");
        assert_eq!(result.answered_numbers, vec![1]);
        assert!(result.remaining.is_empty(), "答完必须从列表消失");

        assert_eq!(
            question_test_pending_queue_len(&state, "conv-main"),
            0,
            "答题命令不允许自己造一条消息入队：发送必须由前端走唯一发送通道"
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
    fn question_answer_should_sort_answered_numbers_ascending() {
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
        assert!(result.remaining.is_empty());
        assert_eq!(
            question_test_pending_queue_len(&state, "conv-main"),
            0,
            "多题一次提交也不允许入队消息"
        );
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
            .expect_err("选项越界必须报错");
        assert!(err.contains("#1"), "{err}");
        assert!(err.contains("选项"), "{err}");
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
    fn question_answer_should_mark_only_submitted_numbers_per_submit() {
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

        let first = question_answer_inner(&state, "conv-main", vec![question_test_answer(1, 0)])
            .expect("answer first");
        let second = question_answer_inner(&state, "conv-main", vec![question_test_answer(2, 0)])
            .expect("answer second");

        // 逐次提交只标记自己那几题，剩下的仍留在面板上。
        assert_eq!(first.answered_numbers, vec![1]);
        assert_eq!(first.remaining.len(), 1, "答掉 #1 之后 #2 还在");
        assert_eq!(second.answered_numbers, vec![2]);
        assert!(second.remaining.is_empty());
        assert_eq!(
            question_test_pending_queue_len(&state, "conv-main"),
            0,
            "两次提交都不允许往队列里塞消息"
        );
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

        // 第二轮：把剩下的 #2 发出去。
        let second = question_answer_inner(&state, "conv-main", vec![question_test_answer(2, 0)])
            .expect("send remaining");
        assert_eq!(second.answered_numbers, vec![2]);
        assert!(second.remaining.is_empty());
        assert_eq!(
            question_test_pending_queue_len(&state, "conv-main"),
            0,
            "两轮提交都不允许往队列里塞消息"
        );
    }

    #[test]
    fn question_answer_should_reject_empty_answers() {
        let state = question_test_state();
        question_test_answerable_conversation(&state, "conv-main");
        let err =
            question_answer_inner(&state, "conv-main", Vec::new()).expect_err("空答案必须报错");
        assert!(err.contains("答案"), "{err}");
    }

    // ==================== 自定义答案：走同一条答题链路 ====================

    #[test]
    fn question_answer_should_accept_custom_text_and_remove_question() {
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
        assert_eq!(
            question_test_numbers(&result.remaining),
            vec![1],
            "答掉 #2 之后只剩 #1"
        );
        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(question_test_numbers(&views), vec![1], "被答的题必须从元数据里删掉");

        assert_eq!(
            question_test_pending_queue_len(&state, "conv-main"),
            0,
            "自定义答案与点选项同一条链路：命令本身不入队，发送由前端走唯一通道"
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

        let err =
            question_answer_inner(&state, "conv-main", vec![question_test_custom_answer(1, "   ")])
                .expect_err("空白自定义答案必须报错");
        assert!(err.contains("#1"), "{err}");

        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(
            question_test_numbers(&views),
            vec![1],
            "被拒之后题目不能被删掉"
        );
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
                custom_text: Some("自己写的".to_string()),
            }],
        )
        .expect_err("选项与自定义文本同时给必须报错");
        assert!(err.contains("二选一"), "{err}");

        let views = question_views_for_conversation(&state, "conv-main").expect("list");
        assert_eq!(
            question_test_numbers(&views),
            vec![1],
            "被拒之后题目不能被删掉"
        );
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
            "元数据里的题必须同步减少"
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
