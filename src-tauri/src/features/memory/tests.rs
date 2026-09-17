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
        assert!(xml.starts_with("[id:m-user]\n"));
        assert!(xml.contains("user-hit"));
        assert!(!xml.contains("<memory_context>"));
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

        assert!(xml.starts_with("[id:m1]\n"));
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
        assert!(xml.contains("[id:m-reasoning]"));
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
        assert!(xml.contains("[id:m-empty-reasoning]"));
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

    fn test_memory_entry(id: &str, judgment: &str, tags: Vec<&str>) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            memory_no: None,
            memory_type: "knowledge".to_string(),
            judgment: judgment.to_string(),
            reasoning: "".to_string(),
            tags: tags.into_iter().map(ToOwned::to_owned).collect(),
            created_at: now_iso(),
            owner_agent_id: None,
            updated_at: now_iso(),
        }
    }

    fn temp_memory_data_path(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("easy_call_ai_tests")
            .join(format!("{}_{}", name, Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp dir");
        root.join("config_mark")
    }

    #[test]
    fn memory_tokenizer_should_emit_cjk_unigrams_and_bigrams() {
        let tokens = memory_tokenize_terms("测试角色 Rust", true);
        assert!(tokens.contains(&"测".to_string()));
        assert!(tokens.contains(&"测试".to_string()));
        assert!(tokens.contains(&"试角".to_string()));
        assert!(tokens.contains(&"角色".to_string()));
        assert!(tokens.contains(&"rust".to_string()));
    }

    #[test]
    fn memory_tantivy_search_should_prioritize_multi_term_hits() {
        let memories = vec![
            test_memory_entry("both", "用户偏好深色主题，也喜欢简洁代码", vec!["主题", "代码"]),
            test_memory_entry("theme", "用户偏好深色主题", vec!["主题"]),
            test_memory_entry("code", "用户喜欢简洁代码", vec!["代码"]),
        ];

        let hits = memory_tantivy_bm25_scores(&memories, "深色主题|简洁代码", 10).expect("bm25");
        assert_eq!(
            hits.first().map(|hit| hit.memory_id.as_str()),
            Some("both")
        );
    }

    #[test]
    fn memory_multi_term_count_should_not_be_lost_by_per_term_truncation() {
        let mut memories = Vec::<MemoryEntry>::new();
        for idx in 0..60 {
            memories.push(test_memory_entry(
                &format!("theme-{idx}"),
                &format!("深色主题 深色主题 深色主题 样本 {idx}"),
                vec!["主题"],
            ));
        }
        memories.push(test_memory_entry(
            "both-low-frequency",
            "深色主题与简洁代码都重要",
            vec!["主题", "代码"],
        ));
        memories.push(test_memory_entry(
            "code-only",
            "简洁代码也重要",
            vec!["代码"],
        ));

        let hits = memory_tantivy_bm25_scores(&memories, "深色主题|简洁代码", 3).expect("bm25");
        assert_eq!(
            hits.first().map(|hit| hit.memory_id.as_str()),
            Some("both-low-frequency")
        );
    }

    #[test]
    fn memory_multi_term_count_should_ignore_loose_fragment_hits() {
        let memories = vec![
            test_memory_entry(
                "fragment-noise",
                "深 简 深 简 深 简",
                vec!["碎片"],
            ),
            test_memory_entry(
                "complete-one-term",
                "深色主题是用户明确偏好",
                vec!["主题"],
            ),
        ];

        let hits = memory_tantivy_bm25_scores(&memories, "深色主题|简洁代码", 2).expect("bm25");
        assert_eq!(
            hits.first().map(|hit| hit.memory_id.as_str()),
            Some("complete-one-term")
        );
    }

    #[test]
    fn memory_rrf_should_fuse_bm25_and_vector_ranks() {
        let bm25_rank_map = HashMap::from([
            ("a".to_string(), 1usize),
            ("b".to_string(), 2usize),
        ]);
        let vector_rank_map = HashMap::from([
            ("b".to_string(), 1usize),
            ("a".to_string(), 3usize),
        ]);

        let a_score = memory_rrf_score_for_id("a", &bm25_rank_map, Some(&vector_rank_map));
        let b_score = memory_rrf_score_for_id("b", &bm25_rank_map, Some(&vector_rank_map));
        let vector_only = memory_rrf_score_for_id("c", &bm25_rank_map, Some(&HashMap::from([
            ("c".to_string(), 1usize),
        ])));

        assert!(b_score > a_score, "vector rank should affect fused order");
        assert!(vector_only > 0.0 && vector_only < b_score);
        assert!(a_score > 0.0 && a_score < 0.5);
    }

    #[test]
    fn memory_bm25_only_should_keep_normalized_scores_for_filtering() {
        let data_path = temp_memory_data_path("rrf_relative_threshold");
        let memories = vec![
            test_memory_entry("hit", "用户最喜欢测试角色", vec!["测试角色"]),
            test_memory_entry("miss", "今天讨论数据库迁移", vec!["数据库"]),
        ];

        let ranked = memory_mixed_ranked_items(&data_path, &memories, "测试角色", 7, 0.0);
        assert_eq!(
            ranked.first().map(|item| item.memory_id.as_str()),
            Some("hit")
        );
        assert!(
            (ranked[0].final_score - ranked[0].bm25_score).abs() < f64::EPSILON,
            "BM25-only path should keep normalized BM25 as final_score for thresholding"
        );

        let ids = memory_recall_hit_ids(
            &data_path,
            &memories,
            "测试角色",
            MEMORY_RERANK_MIN_SCORE_RAG,
        );
        assert_eq!(ids, vec!["hit".to_string()]);
    }

    #[test]
    fn memory_recall_half_top_threshold_should_keep_single_route_rank_one() {
        let dual_route_top = 2.0 / (MEMORY_RRF_K + 1.0);
        let single_route_rank_one = 1.0 / (MEMORY_RRF_K + 1.0);
        let below_half = single_route_rank_one - 0.0001;
        let ranked = vec![
            MemoryMixedRankItem {
                memory_id: "dual".to_string(),
                bm25_score: 1.0,
                bm25_raw_score: 10.0,
                vector_score: 1.0,
                rerank_score: 0.0,
                final_score: dual_route_top,
            },
            MemoryMixedRankItem {
                memory_id: "bm25-rank1".to_string(),
                bm25_score: 1.0,
                bm25_raw_score: 9.0,
                vector_score: 0.0,
                rerank_score: 0.0,
                final_score: single_route_rank_one,
            },
            MemoryMixedRankItem {
                memory_id: "weak".to_string(),
                bm25_score: 0.0,
                bm25_raw_score: 0.0,
                vector_score: 0.0,
                rerank_score: 0.0,
                final_score: below_half,
            },
        ];

        let ids = memory_recall_ids_from_ranked_items(ranked);
        assert_eq!(ids, vec!["dual".to_string(), "bm25-rank1".to_string()]);
    }

    fn test_chat_message(id: &str, role: &str, speaker_agent_id: Option<&str>, text: &str) -> ChatMessage {
        ChatMessage {
            id: id.to_string(),
            role: role.to_string(),
            created_at: format!("2026-05-30T00:00:0{}Z", id.chars().last().unwrap_or('0')),
            speaker_agent_id: speaker_agent_id.map(ToOwned::to_owned),
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

    fn test_chat_agent(id: &str, name: &str) -> AgentProfile {
        AgentProfile {
            id: id.to_string(),
            name: name.to_string(),
            system_prompt: String::new(),
            tools: Vec::new(),
            created_at: now_iso(),
            updated_at: now_iso(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: "main".to_string(),
            scope: "global".to_string(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            include_system_rules: true,
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        }
    }

    #[test]
    fn chat_history_slices_should_keep_short_message_boundaries_without_overlap() {
        let segment = ChatHistorySegment {
            source_kind: "localConversation".to_string(),
            source_id: "c1".to_string(),
            source_title: "测试会话".to_string(),
            segment_id: "block-1".to_string(),
            messages: vec![
                test_chat_message("m1", "user", None, "我喜欢深色主题"),
                test_chat_message("m2", "assistant", Some("agent-a"), "我记住了"),
            ],
        };
        let slices = chat_history_slices_from_segment(
            &segment,
            &[test_chat_agent("agent-a", "小夏")],
            "用户",
            "agent-a",
        );

        assert_eq!(slices.len(), 1);
        assert!(slices[0].content.contains("[用户]: 我喜欢深色主题"));
        assert!(slices[0].content.contains("[小夏]: 我记住了"));
        assert_eq!(slices[0].owner_agent_id, "agent-a");
    }

    #[test]
    fn chat_history_slices_should_overlap_only_when_single_message_is_split() {
        let long_text = "甲".repeat(CHAT_HISTORY_SLICE_TARGET_CHARS + 40);
        let segment = ChatHistorySegment {
            source_kind: "localConversation".to_string(),
            source_id: "c2".to_string(),
            source_title: "长消息".to_string(),
            segment_id: "block-2".to_string(),
            messages: vec![test_chat_message("m1", "assistant", Some("agent-a"), &long_text)],
        };
        let slices = chat_history_slices_from_segment(
            &segment,
            &[test_chat_agent("agent-a", "小夏")],
            "用户",
            "agent-a",
        );

        assert!(slices.len() >= 2);
        let first_tail = slices[0]
            .content
            .chars()
            .rev()
            .take(CHAT_HISTORY_LONG_MESSAGE_OVERLAP_CHARS)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<String>();
        assert!(slices[1].content.starts_with(&first_tail));
        assert_eq!(slices[0].message_start_id, "m1");
        assert_eq!(slices[1].message_start_id, "m1");
    }

    #[test]
    fn chat_history_tantivy_search_should_filter_by_owner_agent() {
        let slices = vec![
            ChatHistorySlice {
                id: "s1".to_string(),
                source_kind: "localConversation".to_string(),
                source_id: "c1".to_string(),
                source_title: "A".to_string(),
                segment_id: "b1".to_string(),
                slice_index: 0,
                content: "[小夏]: 用户喜欢深色主题".to_string(),
                speakers: vec!["小夏".to_string()],
                owner_agent_id: "agent-a".to_string(),
                time_start: now_iso(),
                time_end: now_iso(),
                message_start_id: "m1".to_string(),
                message_end_id: "m1".to_string(),
            },
            ChatHistorySlice {
                id: "s2".to_string(),
                source_kind: "localConversation".to_string(),
                source_id: "c2".to_string(),
                source_title: "B".to_string(),
                segment_id: "b1".to_string(),
                slice_index: 0,
                content: "[小秋]: 用户喜欢深色主题".to_string(),
                speakers: vec!["小秋".to_string()],
                owner_agent_id: "agent-b".to_string(),
                time_start: now_iso(),
                time_end: now_iso(),
                message_start_id: "m2".to_string(),
                message_end_id: "m2".to_string(),
            },
        ];
        let (index, reader, fields) =
            chat_history_build_tantivy_index(&slices).expect("build index");
        let cached = CachedChatHistoryIndex {
            signature: "test".to_string(),
            slices,
            stats: ChatHistorySearchStats::default(),
            index,
            reader,
            fields,
        };
        let filtered = chat_history_tantivy_search(&cached, Some("agent-a"), "深色主题", 10)
            .expect("search");

        assert_eq!(filtered.len(), 1);
        assert_eq!(cached.slices[filtered[0].0].id, "s1");

        let unfiltered =
            chat_history_tantivy_search(&cached, None, "深色主题", 10).expect("search");

        assert_eq!(unfiltered.len(), 2);
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
