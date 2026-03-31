    #[test]
    fn memory_board_should_match_user_text_only_and_require_hit() {
        let now = now_iso();
        let conv = test_active_conversation_with_messages(
            vec![
                test_text_message("user", "hello world", &now),
                test_text_message("assistant", "k99 only assistant side", &now),
            ],
            Some(now),
        );
        let search_text = conversation_search_text(&conv);
        assert!(search_text.contains("hello world"));
        assert!(!search_text.contains("k99 only assistant side"));

        let memories = vec![
            MemoryEntry {
                id: "m-user".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "user-hit".to_string(),
                reasoning: "".to_string(),
                tags: vec!["hello".to_string()],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m-assistant-only".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "assistant-only-hit".to_string(),
                reasoning: "".to_string(),
                tags: vec!["k99".to_string()],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
        ];

        let xml =
            build_memory_board_xml(&memories, &search_text, "").expect("should have one hit");
        assert!(xml.starts_with("<memory_context>\n<id:m-user>\n"));
        assert!(xml.ends_with("\n</memory_context>"));
        assert!(xml.contains("user-hit"));
        assert!(xml.contains("</id:m-user>"));
        assert!(!xml.contains("assistant-only-hit"));
    }

    #[test]
    fn memory_board_should_sort_by_hit_count_and_cap_at_seven() {
        let now = now_iso();
        let user_text =
            "k01 k02 k03 k04 k05 k06 k07 k08 k09 k10 k11 k12 k13 k14 k15 k16".to_string();
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", &user_text, &now)],
            Some(now),
        );
        let search_text = conversation_search_text(&conv);

        let memories = vec![
            MemoryEntry {
                id: "m1".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "rank-8".to_string(),
                reasoning: "".to_string(),
                tags: vec![
                    "k01".to_string(),
                    "k02".to_string(),
                    "k03".to_string(),
                    "k04".to_string(),
                    "k05".to_string(),
                    "k06".to_string(),
                    "k07".to_string(),
                    "k08".to_string(),
                ],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m2".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "rank-7".to_string(),
                reasoning: "".to_string(),
                tags: vec![
                    "k01".to_string(),
                    "k02".to_string(),
                    "k03".to_string(),
                    "k04".to_string(),
                    "k05".to_string(),
                    "k06".to_string(),
                    "k07".to_string(),
                ],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m3".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "rank-6".to_string(),
                reasoning: "".to_string(),
                tags: vec![
                    "k01".to_string(),
                    "k02".to_string(),
                    "k03".to_string(),
                    "k04".to_string(),
                    "k05".to_string(),
                    "k06".to_string(),
                ],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m4".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "rank-5".to_string(),
                reasoning: "".to_string(),
                tags: vec![
                    "k01".to_string(),
                    "k02".to_string(),
                    "k03".to_string(),
                    "k04".to_string(),
                    "k05".to_string(),
                ],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m5".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "rank-4".to_string(),
                reasoning: "".to_string(),
                tags: vec![
                    "k01".to_string(),
                    "k02".to_string(),
                    "k03".to_string(),
                    "k04".to_string(),
                ],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m6".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "rank-3".to_string(),
                reasoning: "".to_string(),
                tags: vec!["k01".to_string(), "k02".to_string(), "k03".to_string()],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m7".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "rank-2".to_string(),
                reasoning: "".to_string(),
                tags: vec!["k01".to_string(), "k02".to_string()],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m8".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "rank-1".to_string(),
                reasoning: "".to_string(),
                tags: vec!["k01".to_string()],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
        ];

        let xml =
            build_memory_board_xml(&memories, &search_text, "").expect("should produce board");

        assert!(xml.starts_with("<memory_context>\n<id:m1>\n"));
        assert_eq!(xml.matches("\n> ").count(), 7);
        assert!(xml.contains("rank-8"));
        assert!(xml.contains("rank-2"));
        assert!(!xml.contains("rank-1"));

        let idx_rank_8 = xml.find("rank-8").expect("rank-8 index");
        let idx_rank_2 = xml.find("rank-2").expect("rank-2 index");
        assert!(idx_rank_8 < idx_rank_2);
    }

    #[test]
    fn memory_board_should_include_reasoning_when_present() {
        let now = now_iso();
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", "prefers concise answers", &now)],
            Some(now),
        );
        let search_text = conversation_search_text(&conv);

        let memories = vec![MemoryEntry {
            id: "m-reasoning".to_string(),
            memory_no: None,
            memory_type: "knowledge".to_string(),
            judgment: "用户偏好简洁回答".to_string(),
            reasoning: "用户多次要求简短".to_string(),
            tags: vec!["偏好".to_string(), "简洁".to_string()],
            created_at: now_iso(),
            owner_agent_id: None,
            updated_at: now_iso(),
        }];

        let xml = build_memory_board_xml(&memories, &search_text, "简洁").expect("should produce board");
        assert!(xml.contains("<id:m-reasoning>"));
        assert!(xml.contains("用户偏好简洁回答"));
        assert!(xml.contains("> 用户多次要求简短"));
        assert!(xml.contains("</id:m-reasoning>"));
    }

    #[test]
    fn memory_board_should_show_reasoning_none_when_empty() {
        let now = now_iso();
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", "hello memory", &now)],
            Some(now),
        );
        let search_text = conversation_search_text(&conv);

        let memories = vec![MemoryEntry {
            id: "m-empty-reasoning".to_string(),
            memory_no: None,
            memory_type: "knowledge".to_string(),
            judgment: "用户提到记忆".to_string(),
            reasoning: "".to_string(),
            tags: vec!["记忆".to_string()],
            created_at: now_iso(),
            owner_agent_id: None,
            updated_at: now_iso(),
        }];

        let xml = build_memory_board_xml(&memories, &search_text, "记忆").expect("should produce board");
        assert!(xml.contains("<id:m-empty-reasoning>"));
        assert!(xml.contains("> 无"));
    }

    #[test]
    fn memory_search_query_should_use_matched_tags_only_for_long_query_text() {
        let memories = vec![
            MemoryEntry {
                id: "m1".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "用户关注极简风格".to_string(),
                reasoning: "".to_string(),
                tags: vec!["极简风格".to_string(), "界面".to_string()],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m2".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "用户关注项目A".to_string(),
                reasoning: "".to_string(),
                tags: vec!["项目A".to_string()],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
        ];
        let long_query_text =
            "这次我想认真聊一下项目A目前的界面问题，我还是更偏向极简风格，不希望页面里出现太多装饰性信息，同时也想减少噪声内容，让信息层级更清楚一些。除此之外，我还希望后续讨论能尽量围绕核心问题，不要被太多支线细节带偏，因为现在最重要的还是先把整体风格和主信息路径稳定下来。";

        let query = memory_search_query_text(&memories, long_query_text);

        assert!(query.contains("项目A"));
        assert!(query.contains("极简风格"));
        assert!(!query.contains("这次我想认真聊一下"));
    }

    #[test]
    fn memory_search_query_should_dedup_tags_case_insensitively() {
        let memories = vec![
            MemoryEntry {
                id: "m1".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "用户提到 Apple".to_string(),
                reasoning: "".to_string(),
                tags: vec!["Apple".to_string()],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m2".to_string(),
                memory_no: None,
                memory_type: "knowledge".to_string(),
                judgment: "用户提到 apple".to_string(),
                reasoning: "".to_string(),
                tags: vec!["apple".to_string()],
                created_at: now_iso(),
                owner_agent_id: None,
                updated_at: now_iso(),
            },
        ];
        let long_query_text =
            "这里先铺一些上下文，确保查询长度超过一百字。用户一直在聊 Apple 的设备生态、apple 相关使用体验，以及后续可能继续扩展到更多兼容问题，所以这轮检索应该只保留一个大小写去重后的标签，而不是重复返回两个等价词元。";

        let query = memory_search_query_text(&memories, long_query_text);
        let lines = query.lines().collect::<Vec<_>>();

        assert_eq!(lines.len(), 1);
        assert!(lines[0].eq_ignore_ascii_case("apple"));
    }

    #[test]
    fn latest_recall_memory_ids_should_return_latest_seven() {
        let mut rows = Vec::<String>::new();
        for idx in 1..=9 {
            rows.push(format!("m{idx}"));
        }

        let ids = latest_recall_memory_ids(&rows, 7);
        assert_eq!(
            ids,
            vec![
                "m9".to_string(),
                "m8".to_string(),
                "m7".to_string(),
                "m6".to_string(),
                "m5".to_string(),
                "m4".to_string(),
                "m3".to_string(),
            ]
        );
    }
