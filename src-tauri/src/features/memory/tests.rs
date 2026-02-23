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
        assert!(xml.contains("user-hit"));
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
            memory_type: "knowledge".to_string(),
            judgment: "用户偏好简洁回答".to_string(),
            reasoning: "用户多次要求简短".to_string(),
            tags: vec!["偏好".to_string(), "简洁".to_string()],
            created_at: now_iso(),
            owner_agent_id: None,
            updated_at: now_iso(),
        }];

        let xml = build_memory_board_xml(&memories, &search_text, "简洁").expect("should produce board");
        assert!(xml.contains("用户偏好简洁回答"));
        assert!(xml.contains("> 用户多次要求简短"));
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
            memory_type: "knowledge".to_string(),
            judgment: "用户提到记忆".to_string(),
            reasoning: "".to_string(),
            tags: vec!["记忆".to_string()],
            created_at: now_iso(),
            owner_agent_id: None,
            updated_at: now_iso(),
        }];

        let xml = build_memory_board_xml(&memories, &search_text, "记忆").expect("should produce board");
        assert!(xml.contains("> 无"));
    }

    #[test]
    fn memory_recall_query_should_mix_latest_assistant_and_current_user() {
        let now = now_iso();
        let conv = test_active_conversation_with_messages(
            vec![
                test_text_message("user", "old user text", &now),
                test_text_message("assistant", "assistant latest context", &now),
            ],
            Some(now),
        );

        let query = memory_recall_query_text(&conv, "new user request");
        assert!(query.contains("assistant latest context"));
        assert!(query.contains("new user request"));
        assert!(!query.contains("old user text"));
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


