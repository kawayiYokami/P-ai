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
                content: "user-hit".to_string(),
                keywords: vec!["hello".to_string()],
                created_at: now_iso(),
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m-assistant-only".to_string(),
                content: "assistant-only-hit".to_string(),
                keywords: vec!["k99".to_string()],
                created_at: now_iso(),
                updated_at: now_iso(),
            },
        ];

        let xml =
            build_memory_board_xml(&memories, &search_text, "").expect("should have one hit");
        assert!(xml.contains("<content>user-hit</content>"));
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
                content: "rank-8".to_string(),
                keywords: vec![
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
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m2".to_string(),
                content: "rank-7".to_string(),
                keywords: vec![
                    "k01".to_string(),
                    "k02".to_string(),
                    "k03".to_string(),
                    "k04".to_string(),
                    "k05".to_string(),
                    "k06".to_string(),
                    "k07".to_string(),
                ],
                created_at: now_iso(),
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m3".to_string(),
                content: "rank-6".to_string(),
                keywords: vec![
                    "k01".to_string(),
                    "k02".to_string(),
                    "k03".to_string(),
                    "k04".to_string(),
                    "k05".to_string(),
                    "k06".to_string(),
                ],
                created_at: now_iso(),
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m4".to_string(),
                content: "rank-5".to_string(),
                keywords: vec![
                    "k01".to_string(),
                    "k02".to_string(),
                    "k03".to_string(),
                    "k04".to_string(),
                    "k05".to_string(),
                ],
                created_at: now_iso(),
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m5".to_string(),
                content: "rank-4".to_string(),
                keywords: vec![
                    "k01".to_string(),
                    "k02".to_string(),
                    "k03".to_string(),
                    "k04".to_string(),
                ],
                created_at: now_iso(),
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m6".to_string(),
                content: "rank-3".to_string(),
                keywords: vec!["k01".to_string(), "k02".to_string(), "k03".to_string()],
                created_at: now_iso(),
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m7".to_string(),
                content: "rank-2".to_string(),
                keywords: vec!["k01".to_string(), "k02".to_string()],
                created_at: now_iso(),
                updated_at: now_iso(),
            },
            MemoryEntry {
                id: "m8".to_string(),
                content: "rank-1".to_string(),
                keywords: vec!["k01".to_string()],
                created_at: now_iso(),
                updated_at: now_iso(),
            },
        ];

        let xml =
            build_memory_board_xml(&memories, &search_text, "").expect("should produce board");

        assert_eq!(count_xml_tag(&xml, "memory"), 7);
        assert!(xml.contains("<content>rank-8</content>"));
        assert!(xml.contains("<content>rank-2</content>"));
        assert!(!xml.contains("rank-1"));

        let idx_rank_8 = xml.find("rank-8").expect("rank-8 index");
        let idx_rank_2 = xml.find("rank-2").expect("rank-2 index");
        assert!(idx_rank_8 < idx_rank_2);
    }
