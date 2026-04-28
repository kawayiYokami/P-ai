    #[test]
    fn build_prompt_should_include_structured_tool_history_messages() {
        let now = now_iso();
        let mut assistant_with_tool = test_text_message("assistant", "我去查一下", &now);
        assistant_with_tool.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "bing_search",
                        "arguments": "{\"query\":\"rust\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"results\":[{\"title\":\"Rust\"}]}"
            }),
        ]);
        let agent = default_agent();
        assistant_with_tool.speaker_agent_id = Some(agent.id.clone());

        let messages = vec![
            test_text_message("user", "帮我查 Rust", &now),
            assistant_with_tool,
            test_text_message("user", "继续", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert!(
            prepared
                .history_messages
                .iter()
                .any(|m| m.role == "assistant" && m.tool_calls.is_some())
        );
        assert!(
            prepared.history_messages.iter().any(|m| {
                m.role == "tool"
                    && m.tool_call_id.as_deref() == Some("call_1")
                    && m.text.contains("\"results\"")
            })
        );
    }

    #[test]
    fn conversation_prompt_service_snapshot_should_keep_cache_hits_stable() {
        let now = now_iso();
        let agent = default_agent();
        let messages = vec![
            test_text_message("user", "帮我看一下会话缓存", &now),
            test_text_message("assistant", "我先整理一下", &now),
            test_text_message("user", "继续", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now.clone()));
        let fixed_system_prompt = build_core_system_prompt_text(
            &conv,
            &agent,
            &[],
            Some(("用户", "我是测试用户")),
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
        );

        let first = conversation_prompt_service().build_prompt_snapshot(
            None,
            "chat",
            &conv,
            &agent,
            &[],
            "zh-CN",
            None,
            &fixed_system_prompt,
            None,
            None,
            &[],
            None,
        );
        let second = conversation_prompt_service().build_prompt_snapshot(
            None,
            "chat",
            &conv,
            &agent,
            &[],
            "zh-CN",
            None,
            &fixed_system_prompt,
            None,
            None,
            &[],
            None,
        );

        assert_eq!(first.revisions, second.revisions);
        assert_eq!(first.department_prompt, second.department_prompt);
        assert_eq!(first.environment_prompt, second.environment_prompt);
        assert_eq!(first.abstract_messages, second.abstract_messages);
    }

    #[test]
    fn conversation_prompt_service_prompt_revision_should_ignore_todos_and_memory_recall() {
        let now = now_iso();
        let agent = default_agent();
        let messages = vec![
            test_text_message("user", "检查 prompt revision", &now),
            test_text_message("assistant", "收到", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now.clone()));
        let fixed_system_prompt = build_core_system_prompt_text(
            &conv,
            &agent,
            &[],
            Some(("用户", "我是测试用户")),
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
        );
        let baseline = conversation_prompt_service().build_prompt_snapshot(
            None,
            "chat",
            &conv,
            &agent,
            &[],
            "zh-CN",
            None,
            &fixed_system_prompt,
            None,
            None,
            &[],
            None,
        );

        let mut with_conversation_side_blocks = conv.clone();
        with_conversation_side_blocks.current_todos.push(ConversationTodoItem {
            content: "第一步".to_string(),
            status: "in_progress".to_string(),
        });
        with_conversation_side_blocks
            .memory_recall_table
            .push("memory-1".to_string());
        let fixed_after = build_core_system_prompt_text(
            &with_conversation_side_blocks,
            &agent,
            &[],
            Some(("用户", "我是测试用户")),
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
        );
        let mutated = conversation_prompt_service().build_prompt_snapshot(
            None,
            "chat",
            &with_conversation_side_blocks,
            &agent,
            &[],
            "zh-CN",
            None,
            &fixed_after,
            None,
            None,
            &[],
            None,
        );

        assert_eq!(baseline.revisions.prompt_revision, mutated.revisions.prompt_revision);
    }

    #[test]
    fn build_prompt_should_map_non_self_personas_to_user_with_speaker_block() {
        let now = now_iso();
        let agent = default_agent();
        let system_persona = default_system_persona();
        let messages = vec![
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(system_persona.id.clone()),
                parts: vec![MessagePart::Text {
                    text: "请检查今天的任务触发情况".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            },
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(agent.id.clone()),
                parts: vec![MessagePart::Text {
                    text: "我马上处理".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            },
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(system_persona.id.clone()),
                parts: vec![MessagePart::Text {
                    text: "现在补发第二次提醒".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            },
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona(), system_persona.clone()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert_eq!(prepared.history_messages.len(), 2);
        assert_eq!(prepared.history_messages[0].role, "user");
        assert!(
            prepared.history_messages[0]
                .user_time_text
                .as_deref()
                .unwrap_or_default()
                .contains("pai system")
        );
        assert_eq!(prepared.history_messages[1].role, "assistant");
        assert!(prepared.latest_user_meta_text.contains("pai system"));
        assert!(prepared.latest_user_text.contains("现在补发第二次提醒"));
    }

    #[test]
    fn build_prompt_user_meta_text_should_not_append_memory_injected_tag() {
        let now = now_iso();
        let mut message = test_text_message("user", "继续", &now);
        message.extra_text_blocks.push(
            "<memory_context>\n<id:m1>\n用户询问 codex 是什么\n> 无\n</id:m1>\n</memory_context>"
                .to_string(),
        );

        let meta = build_prompt_user_meta_text(
            &message,
            &[default_agent(), default_user_persona()],
            "用户",
            "zh-CN",
            false,
        )
        .expect("meta text");

        assert!(!meta.contains("memory=已注入"));
        assert!(meta.contains("T"));
    }

    #[test]
    fn build_prompt_user_meta_text_should_use_snake_case_remote_identity_tags() {
        let now = now_iso();
        let mut message = test_text_message("user", "你好", &now);
        message.provider_meta = Some(serde_json::json!({
            "origin": {
                "kind": "remote_im",
                "channel_id": "remote-im-1",
                "contact_type": "group",
                "contact_id": "group-42",
                "contact_name": "测试群",
                "sender_name": "张三"
            }
        }));

        let meta = build_prompt_user_meta_text(
            &message,
            &[default_agent(), default_user_persona()],
            "用户",
            "zh-CN",
            true,
        )
        .expect("meta text");

        assert!(meta.contains("张三 (测试群)"));
        assert!(meta.contains("channel_id=remote-im-1"));
        assert!(meta.contains("contact_id=group-42"));
        assert!(!meta.contains("channelId="));
        assert!(!meta.contains("contactId="));
    }

    #[test]
    fn build_prompt_user_meta_text_should_ignore_legacy_remote_identity_keys() {
        let now = now_iso();
        let mut message = test_text_message("user", "你好", &now);
        message.provider_meta = Some(serde_json::json!({
            "origin": {
                "kind": "remote_im",
                "channelId": "legacy-channel",
                "remoteContactType": "private",
                "remoteContactId": "legacy-contact",
                "remoteContactName": "旧联系人",
                "senderName": "旧联系人"
            }
        }));

        let meta = build_prompt_user_meta_text(
            &message,
            &[default_agent(), default_user_persona()],
            "用户",
            "zh-CN",
            true,
        )
        .expect("meta text");

        assert!(!meta.contains("旧联系人"));
        assert!(!meta.contains("channel_id=legacy-channel"));
        assert!(!meta.contains("contact_id=legacy-contact"));
    }

    #[test]
    fn build_prompt_should_delay_inject_retrieved_memories_with_request_local_dedupe() {
        let state = test_chat_runtime_state();
        let drafts = vec![
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "用户很喜欢猫咪".to_string(),
                reasoning: "因为用户妈妈从小养猫".to_string(),
                tags: vec!["猫".to_string()],
                owner_agent_id: None,
            },
            MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "用户对花生过敏".to_string(),
                reasoning: "因为用户小时候吃花生酱休克过".to_string(),
                tags: vec!["花生".to_string(), "过敏".to_string()],
                owner_agent_id: None,
            },
        ];
        let (saved, _) = memory_store_upsert_drafts(&state.data_path, &drafts).expect("seed memories");
        let cat_memory_id = saved[0].id.clone().expect("cat memory id");
        let peanut_memory_id = saved[1].id.clone().expect("peanut memory id");
        let now = now_iso();
        let agent = default_agent();
        let messages = vec![
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(USER_PERSONA_ID.to_string()),
                parts: vec![MessagePart::Text {
                    text: "我家猫吐毛球怎么办？".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "retrieved_memory_ids": [cat_memory_id]
                })),
                tool_call: None,
                mcp_call: None,
            },
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(agent.id.clone()),
                parts: vec![MessagePart::Text {
                    text: "吐毛球可以先观察饮食和梳毛频率。".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            },
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(USER_PERSONA_ID.to_string()),
                parts: vec![MessagePart::Text {
                    text: "我想吃花生酱面包，可以吗？".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "retrieved_memory_ids": [saved[0].id.clone().expect("dup cat id"), peanut_memory_id.clone(), peanut_memory_id]
                })),
                tool_call: None,
                mcp_call: None,
            },
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            Some(&state.data_path),
            None,
            None,
            false,
        );

        let history_extra = prepared.history_messages[0].extra_text_blocks.join("\n");
        assert_eq!(history_extra.matches("用户很喜欢猫咪").count(), 1);
        assert!(history_extra.contains("因为用户妈妈从小养猫"));
        assert!(!history_extra.contains("用户对花生过敏"));
        assert!(prepared.latest_user_extra_text.contains("用户对花生过敏"));
        assert_eq!(prepared.latest_user_extra_text.matches("用户很喜欢猫咪").count(), 0);
        assert_eq!(prepared.latest_user_extra_text.matches("用户对花生过敏").count(), 1);
    }

    #[test]
    fn build_prompt_user_meta_text_should_skip_compaction_message_metadata() {
        let now = now_iso();
        let mut message = test_text_message(
            "user",
            "[上下文整理]\n触发原因：manual\n整理摘要：\n用户刚刚确认继续推进。",
            &now,
        );
        message.provider_meta = Some(serde_json::json!({
            "message_meta": {
                "kind": "context_compaction"
            },
            "origin": {
                "kind": "remote_im",
                "channel_id": "remote-im-1",
                "contact_type": "private",
                "contact_id": "contact-42",
                "contact_name": "测试联系人",
                "sender_name": "张三"
            }
        }));

        let meta = build_prompt_user_meta_text(
            &message,
            &[default_agent(), default_user_persona()],
            "用户",
            "zh-CN",
            true,
        );

        assert!(meta.is_none());
    }

    #[test]
    fn prepared_prompt_to_messages_json_should_keep_structured_tool_history_messages() {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "user".to_string(),
                    text: "你好".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: Some("[遥酱] 2026-03-18T12:18".to_string()),
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: String::new(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "bing_search", "arguments": "{\"query\":\"rust\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"results\":[{\"title\":\"Rust\"}]}".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("call_1".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "继续".to_string(),
            latest_user_meta_text: "2026-02-11 17:30:45".to_string(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };
        let messages = prepared_prompt_to_messages_json(&prepared);
        assert!(messages.iter().any(|m| {
            m.get("role").and_then(Value::as_str) == Some("assistant")
                && m.get("tool_calls").and_then(Value::as_array).is_some()
        }));
        assert!(messages.iter().any(|m| {
            m.get("role").and_then(Value::as_str) == Some("tool")
                && m.get("tool_call_id").and_then(Value::as_str) == Some("call_1")
        }));
        assert!(messages.iter().any(|m| {
            m.get("role").and_then(Value::as_str) == Some("user")
                && m.get("content")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.len() == 2
                            && arr[0].get("type").and_then(Value::as_str) == Some("text")
                            && arr[0].get("text").and_then(Value::as_str) == Some("你好")
                            && arr[1].get("type").and_then(Value::as_str) == Some("text")
                            && arr[1].get("text").and_then(Value::as_str)
                                == Some("[遥酱] 2026-03-18T12:18")
                    })
                    .unwrap_or(false)
        }));
    }

    #[test]
    fn build_prompt_should_not_extract_latest_user_when_tail_is_assistant() {
        let now = now_iso();
        let agent = default_agent();
        let mut user_message = test_text_message("user", "现在时间是多少？", &now);
        user_message.speaker_agent_id = None;
        let mut assistant_message = test_text_message("assistant", "2026-03-30 00:26（+08:00）", &now);
        assistant_message.speaker_agent_id = Some(agent.id.clone());
        let messages = vec![user_message, assistant_message];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert!(prepared.latest_user_text.trim().is_empty());
        assert_eq!(prepared.history_messages.len(), 2);
        assert_eq!(prepared.history_messages[0].role, "user");
        assert_eq!(prepared.history_messages[1].role, "assistant");
    }

    #[test]
    fn build_prompt_should_merge_adjacent_plain_assistant_history_messages() {
        let now = now_iso();
        let agent = default_agent();
        let mut user_message = test_text_message("user", "先听我说", &now);
        user_message.speaker_agent_id = None;
        let mut assistant_message_1 = test_text_message("assistant", "第一段回复", &now);
        assistant_message_1.speaker_agent_id = Some(agent.id.clone());
        let mut assistant_message_2 = test_text_message("assistant", "第二段补充", &now);
        assistant_message_2.speaker_agent_id = Some(agent.id.clone());
        let messages = vec![user_message, assistant_message_1, assistant_message_2];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert_eq!(prepared.history_messages.len(), 2);
        assert_eq!(prepared.history_messages[0].role, "user");
        assert_eq!(prepared.history_messages[1].role, "assistant");
        assert_eq!(prepared.history_messages[1].text, "第一段回复\n\n第二段补充");
    }

    #[test]
    fn prepared_prompt_to_messages_json_should_keep_tool_call_reasoning_per_assistant_message(
    ) {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "user".to_string(),
                    text: "查一下结果".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: "我先调用工具".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "search_docs",
                            "arguments": "{\"q\":\"结果\"}"
                        }
                    })]),
                    tool_call_id: None,
                    reasoning_content: Some("先查资料".to_string()),
                },
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: "工具结果我看完了".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "call_2",
                        "type": "function",
                        "function": {
                            "name": "resolve_link",
                            "arguments": "{\"query\":\"结果详情\"}"
                        }
                    })]),
                    tool_call_id: None,
                    reasoning_content: Some("再补一轮定位".to_string()),
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"ok\":true}".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("call_2".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: String::new(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let messages = prepared_prompt_to_messages_json(&prepared);

        assert_eq!(messages.len(), 5);
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(
            messages[2]["content"].as_str(),
            Some("我先调用工具")
        );
        assert_eq!(
            messages[2]["reasoning_content"].as_str(),
            Some("先查资料")
        );
        assert_eq!(
            messages[2]["tool_calls"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(messages[3]["role"], "assistant");
        assert_eq!(
            messages[3]["content"].as_str(),
            Some("工具结果我看完了")
        );
        assert_eq!(
            messages[3]["reasoning_content"].as_str(),
            Some("再补一轮定位")
        );
        assert_eq!(
            messages[3]["tool_calls"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(messages[4]["role"], "tool");
    }

    #[test]
    fn prepared_prompt_to_messages_json_should_keep_reasoning_for_plain_assistant_messages() {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![PreparedHistoryMessage {
                role: "assistant".to_string(),
                text: "这是结论".to_string(),
                extra_text_blocks: Vec::new(),
                user_time_text: None,
                images: Vec::new(),
                audios: Vec::new(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: Some("这是思考过程".to_string()),
            }],
            latest_user_text: String::new(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let messages = prepared_prompt_to_messages_json(&prepared);

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["content"].as_str(), Some("这是结论"));
        assert_eq!(
            messages[1]["reasoning_content"].as_str(),
            Some("这是思考过程")
        );
    }

    #[test]
    fn build_prompt_should_preserve_assistant_reasoning_from_provider_meta() {
        let now = now_iso();
        let agent = default_agent();
        let mut assistant = test_text_message("assistant", "这是最终回复", &now);
        assistant.speaker_agent_id = Some(agent.id.clone());
        assistant.provider_meta = Some(serde_json::json!({
            "reasoningStandard": "先分析用户问题，再整理答案"
        }));
        let messages = vec![
            test_text_message("user", "帮我总结一下", &now),
            assistant,
            test_text_message("user", "继续", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert!(
            prepared.history_messages.iter().any(|message| {
                message.role == "assistant"
                    && message.text == "这是最终回复"
                    && message.reasoning_content.as_deref() == Some("先分析用户问题，再整理答案")
            })
        );
    }

    #[test]
    fn prepared_prompt_to_messages_json_should_omit_empty_latest_user_turn() {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "user".to_string(),
                    text: "现在时间是多少？".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: "2026-03-30 00:26（+08:00）".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning_content: None,
                },
            ],
            latest_user_text: String::new(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let messages = prepared_prompt_to_messages_json(&prepared);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].get("role").and_then(Value::as_str), Some("user"));
        assert_eq!(messages[2].get("role").and_then(Value::as_str), Some("assistant"));
    }

    #[test]
    fn build_prompt_should_prefix_latest_user_text_with_mentions() {
        let now = now_iso();
        let agent = default_agent();
        let mut user_message = test_text_message("user", "请你看看这个方案", &now);
        user_message.provider_meta = Some(serde_json::json!({
            "message_meta": {
                "kind": "user_message",
                "mentions": [
                    {
                        "agentId": "agent-fairy",
                        "agentName": "fairy",
                        "departmentId": "department-fairy",
                        "departmentName": "测试部门"
                    },
                    {
                        "agentId": "agent-zhongli",
                        "agentName": "钟离",
                        "departmentId": "department-zhongli",
                        "departmentName": "璃月顾问组"
                    }
                ]
            }
        }));
        let conv = test_active_conversation_with_messages(vec![user_message], Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert_eq!(prepared.latest_user_text, "@fairy,@钟离\n请你看看这个方案");
    }

    #[test]
    fn prepared_prompt_to_messages_json_should_keep_mention_prefix_for_latest_user() {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![PreparedHistoryMessage {
                role: "assistant".to_string(),
                text: "收到".to_string(),
                extra_text_blocks: Vec::new(),
                user_time_text: None,
                images: Vec::new(),
                audios: Vec::new(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            latest_user_text: "@fairy,@钟离\n请你看看这个方案".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let messages = prepared_prompt_to_messages_json(&prepared);
        let latest_user = messages
            .last()
            .and_then(Value::as_object)
            .cloned()
            .expect("latest user message should exist");
        assert_eq!(latest_user.get("role").and_then(Value::as_str), Some("user"));
        let content = latest_user
            .get("content")
            .and_then(Value::as_str)
            .or_else(|| {
                latest_user
                    .get("content")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first())
                    .and_then(|item| item.get("text"))
                    .and_then(Value::as_str)
            });
        assert_eq!(content, Some("@fairy,@钟离\n请你看看这个方案"));
    }

    #[test]
    fn build_prompt_should_not_duplicate_compaction_message_into_latest_user_text() {
        let now = now_iso();
        let agent = default_agent();
        let messages = vec![
            test_text_message("user", "第一轮用户原始消息", &now),
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
                parts: vec![MessagePart::Text {
                    text: "[上下文整理]\n触发原因：force_context_usage_82_after_reply\n整理摘要：\n保留关键上下文。".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "message_meta": {
                        "kind": "context_compaction",
                        "scene": "compaction",
                        "reason": "force_context_usage_82_after_reply"
                    }
                })),
                tool_call: None,
                mcp_call: None,
            },
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert_eq!(prepared.history_messages.len(), 1);
        assert!(prepared.history_messages[0].text.contains("[上下文整理]"));
        assert!(prepared.latest_user_text.trim().is_empty());
        assert!(prepared.latest_user_meta_text.trim().is_empty());
        assert_eq!(prepared.history_messages[0].role, "user");
    }

    #[test]
    fn build_prompt_should_only_keep_last_compaction_message_as_boundary() {
        let now = now_iso();
        let agent = default_agent();
        let mut trailing_assistant = test_text_message("assistant", "摘要后的助手消息", &now);
        trailing_assistant.speaker_agent_id = Some(agent.id.clone());
        let messages = vec![
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
                parts: vec![MessagePart::Text {
                    text: "[上下文整理]\n旧摘要".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "message_meta": {
                        "kind": "summary_context_seed",
                        "scene": "seed",
                    }
                })),
                tool_call: None,
                mcp_call: None,
            },
            test_text_message("user", "中间用户消息", &now),
            test_text_message("assistant", "中间助手消息", &now),
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
                parts: vec![MessagePart::Text {
                    text: "[上下文整理]\n新摘要".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "message_meta": {
                        "kind": "context_compaction",
                        "scene": "compaction",
                        "reason": "manual"
                    }
                })),
                tool_call: None,
                mcp_call: None,
            },
            trailing_assistant,
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert_eq!(prepared.history_messages.len(), 2);
        assert!(prepared.history_messages[0].text.contains("新摘要"));
        assert!(!prepared.history_messages[0].text.contains("旧摘要"));
        assert_eq!(prepared.history_messages[1].text, "摘要后的助手消息");
    }

    #[test]
    fn build_prompt_should_resolve_latest_user_from_trimmed_context_window() {
        let now = now_iso();
        let agent = default_agent();
        let mut trailing_assistant = test_text_message("assistant", "收到，我继续处理", &now);
        trailing_assistant.speaker_agent_id = Some(agent.id.clone());
        let messages = vec![
            test_text_message("user", "这是很久之前的超长历史消息，不应再参与本轮提示词", &now),
            test_text_message("assistant", "这是旧助手回复", &now),
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
                parts: vec![MessagePart::Text {
                    text: "[上下文整理]\n只保留最近有效上下文".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "message_meta": {
                        "kind": "context_compaction",
                        "scene": "compaction",
                        "reason": "manual"
                    }
                })),
                tool_call: None,
                mcp_call: None,
            },
            trailing_assistant,
            test_text_message("user", "这是压缩后的最新用户消息", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert_eq!(prepared.latest_user_text, "这是压缩后的最新用户消息");
        assert_eq!(prepared.history_messages.len(), 2);
        assert!(
            prepared.history_messages[0]
                .text
                .contains("只保留最近有效上下文")
        );
        assert_eq!(prepared.history_messages[1].text, "收到，我继续处理");
        assert!(
            prepared
                .history_messages
                .iter()
                .all(|message| !message.text.contains("很久之前的超长历史消息"))
        );
    }

    #[test]
    fn build_prompt_should_not_treat_normal_message_with_compaction_phrase_as_compaction_boundary() {
        let now = now_iso();
        let agent = default_agent();
        let messages = vec![
            test_text_message("user", "第一轮用户原始消息", &now),
            test_text_message("assistant", "第一轮助手回复", &now),
            test_text_message(
                "user",
                "plan 写入 markdown，是为了防止上下文压缩之后，计划被压缩掉了的设计。",
                &now,
            ),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert_eq!(prepared.history_messages.len(), 2);
        assert_eq!(prepared.history_messages[0].text, "第一轮用户原始消息");
        assert_eq!(prepared.history_messages[1].text, "第一轮助手回复");
        assert!(prepared
            .latest_user_text
            .contains("防止上下文压缩之后"));
    }

    #[test]
    fn build_prompt_should_not_treat_prefix_only_message_without_meta_as_compaction_boundary() {
        let now = now_iso();
        let agent = default_agent();
        let messages = vec![
            test_text_message("user", "第一轮用户原始消息", &now),
            test_text_message("assistant", "第一轮助手回复", &now),
            test_text_message("user", "[上下文整理]\n这只是普通文本，不是系统压缩消息。", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            &[],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
            false,
        );

        assert_eq!(prepared.history_messages.len(), 2);
        assert!(prepared.latest_user_text.contains("这只是普通文本"));
    }

    #[test]
    fn build_remote_im_activation_runtime_block_should_warn_multiple_sources_no_auto_send() {
        let sources = vec![
            RemoteImActivationSource {
                channel_id: "remote-im-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                remote_contact_type: "private".to_string(),
                remote_contact_id: "contact-a".to_string(),
                remote_contact_name: "张三".to_string(),
            },
            RemoteImActivationSource {
                channel_id: "remote-im-b".to_string(),
                platform: RemoteImPlatform::Dingtalk,
                remote_contact_type: "private".to_string(),
                remote_contact_id: "contact-b".to_string(),
                remote_contact_name: "李四".to_string(),
            },
        ];

        let block =
            build_remote_im_activation_runtime_block(&sources, "zh-CN").expect("runtime block");

        assert!(block.contains("多个远程 IM 来源共同激活"));
        assert!(block.contains("系统不会自动外发本轮最终回复"));
        assert!(block.contains("channel_id=remote-im-a"));
        assert!(block.contains("channel_id=remote-im-b"));
    }

    #[test]
    fn resolve_remote_im_auto_send_target_should_only_auto_send_single_source_without_no_reply() {
        let single_source = RemoteImActivationSource {
            channel_id: "remote-im-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "private".to_string(),
            remote_contact_id: "contact-a".to_string(),
            remote_contact_name: "张三".to_string(),
        };
        let single_target = resolve_remote_im_auto_send_target(
            "你好，这里是最终回复。",
            &[single_source.clone()],
            None,
        )
        .expect("single target");
        assert_eq!(single_target, Some(single_source.clone()));

        let multiple_sources = resolve_remote_im_auto_send_target(
            "你好，这里是最终回复。",
            &[
                single_source,
                RemoteImActivationSource {
                    channel_id: "remote-im-b".to_string(),
                    platform: RemoteImPlatform::Dingtalk,
                    remote_contact_type: "private".to_string(),
                    remote_contact_id: "contact-b".to_string(),
                    remote_contact_name: "李四".to_string(),
                },
            ],
            None,
        )
        .expect("multiple sources should skip auto send");
        assert!(multiple_sources.is_none());

        let no_reply_decision = RemoteImReplyDecisionSummary {
            action: "no_reply".to_string(),
            target: None,
        };
        let no_reply_target = resolve_remote_im_auto_send_target(
            "你好，这里是最终回复。",
            &[RemoteImActivationSource {
                channel_id: "remote-im-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                remote_contact_type: "private".to_string(),
                remote_contact_id: "contact-a".to_string(),
                remote_contact_name: "张三".to_string(),
            }],
            Some(&no_reply_decision),
        )
        .expect("no_reply should suppress auto send");
        assert!(no_reply_target.is_none());
    }

    #[test]
    fn resolve_bound_remote_im_activation_source_should_only_bind_single_source() {
        let single_source = RemoteImActivationSource {
            channel_id: "remote-im-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "private".to_string(),
            remote_contact_id: "contact-a".to_string(),
            remote_contact_name: "张三".to_string(),
        };

        assert_eq!(
            resolve_bound_remote_im_activation_source(std::slice::from_ref(&single_source)),
            Some(single_source.clone())
        );
        assert!(resolve_bound_remote_im_activation_source(&[]).is_none());
        assert!(resolve_bound_remote_im_activation_source(&[
            single_source,
            RemoteImActivationSource {
                channel_id: "remote-im-b".to_string(),
                platform: RemoteImPlatform::Dingtalk,
                remote_contact_type: "private".to_string(),
                remote_contact_id: "contact-b".to_string(),
                remote_contact_name: "李四".to_string(),
            }
        ])
        .is_none());
    }

    #[test]
    fn collect_activated_remote_im_sources_should_dedup_same_contact_and_ignore_inactive_events() {
        let created_at = now_iso();
        let remote_sender_a = RemoteImMessageSource {
            channel_id: "remote-im-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            im_name: "QQ".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: "contact-a".to_string(),
            remote_contact_name: "张三".to_string(),
            sender_id: "contact-a".to_string(),
            sender_name: "张三".to_string(),
            sender_avatar_url: None,
            platform_message_id: None,
        };
        let remote_sender_b = RemoteImMessageSource {
            channel_id: "remote-im-b".to_string(),
            platform: RemoteImPlatform::Dingtalk,
            im_name: "钉钉".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: "contact-b".to_string(),
            remote_contact_name: "李四".to_string(),
            sender_id: "contact-b".to_string(),
            sender_name: "李四".to_string(),
            sender_avatar_url: None,
            platform_message_id: None,
        };
        let events = vec![
            ChatPendingEvent {
                id: Uuid::new_v4().to_string(),
                conversation_id: "conversation-a".to_string(),
                created_at: created_at.clone(),
                source: ChatEventSource::RemoteIm,
                queue_mode: ChatQueueMode::Normal,
                messages: vec![test_text_message("user", "来自张三的第一条消息", &created_at)],
                activate_assistant: true,
                session_info: ChatSessionInfo {
                    department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                },
                runtime_context: None,
                sender_info: Some(remote_sender_a.clone()),
            },
            ChatPendingEvent {
                id: Uuid::new_v4().to_string(),
                conversation_id: "conversation-a".to_string(),
                created_at: created_at.clone(),
                source: ChatEventSource::RemoteIm,
                queue_mode: ChatQueueMode::Normal,
                messages: vec![test_text_message("user", "来自张三的第二条消息", &created_at)],
                activate_assistant: true,
                session_info: ChatSessionInfo {
                    department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                },
                runtime_context: None,
                sender_info: Some(remote_sender_a),
            },
            ChatPendingEvent {
                id: Uuid::new_v4().to_string(),
                conversation_id: "conversation-a".to_string(),
                created_at: created_at.clone(),
                source: ChatEventSource::RemoteIm,
                queue_mode: ChatQueueMode::Normal,
                messages: vec![test_text_message("user", "来自李四的消息", &created_at)],
                activate_assistant: true,
                session_info: ChatSessionInfo {
                    department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                },
                runtime_context: None,
                sender_info: Some(remote_sender_b),
            },
            ChatPendingEvent {
                id: Uuid::new_v4().to_string(),
                conversation_id: "conversation-a".to_string(),
                created_at,
                source: ChatEventSource::User,
                queue_mode: ChatQueueMode::Normal,
                messages: vec![test_text_message("user", "普通用户消息", &now_iso())],
                activate_assistant: true,
                session_info: ChatSessionInfo {
                    department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
                    agent_id: DEFAULT_AGENT_ID.to_string(),
                },
                runtime_context: None,
                sender_info: None,
            },
        ];

        let sources =
            collect_activated_remote_im_sources(&events, &[true, true, false, true]);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].channel_id, "remote-im-a");
        assert_eq!(sources[0].remote_contact_id, "contact-a");

        let all_sources =
            collect_activated_remote_im_sources(&events, &[true, true, true, true]);
        assert_eq!(all_sources.len(), 2);
        assert_eq!(all_sources[0].channel_id, "remote-im-a");
        assert_eq!(all_sources[1].channel_id, "remote-im-b");
    }

    fn seed_remote_im_auto_send_test_state(
        channel_credentials: Value,
    ) -> (AppState, RemoteImActivationSource, String, String, String) {
        let state = test_chat_runtime_state();
        let mut config = AppConfig::default();
        config.remote_im_channels.push(RemoteImChannelConfig {
            id: "remote-im-a".to_string(),
            name: "测试渠道".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            enabled: true,
            credentials: channel_credentials,
            activate_assistant: true,
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            allow_send_files: false,
        });
        write_config(&state.config_path, &config).expect("write config");

        let conversation_id = "conversation-a".to_string();
        let assistant_message_id = Uuid::new_v4().to_string();
        let assistant_text = "这里是自动发送回复".to_string();
        let created_at = now_iso();

        let mut conversation = test_chat_conversation(&conversation_id, "active", &created_at);
        conversation.messages.push(ChatMessage {
            id: assistant_message_id.clone(),
            role: "assistant".to_string(),
            created_at: created_at.clone(),
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: assistant_text.clone(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "remoteImDecision": {
                    "action": "send_async",
                    "processingMode": "continuous",
                    "conversationKind": "standard_conversation",
                    "activationSourceCount": 1,
                    "error": ""
                }
            })),
            tool_call: None,
            mcp_call: None,
        });

        let mut data = AppData::default();
        data.conversations.push(conversation);
        data.remote_im_contacts.push(RemoteImContact {
            id: "contact-record-a".to_string(),
            channel_id: "remote-im-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "private".to_string(),
            remote_contact_id: "contact-a".to_string(),
            remote_contact_name: "张三".to_string(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "always".to_string(),
            activation_keywords: Vec::new(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "main_session".to_string(),
            bound_department_id: None,
            bound_conversation_id: Some(conversation_id.clone()),
            processing_mode: "continuous".to_string(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
        });
        state_write_app_data_cached(&state, &data).expect("write app data");

        (
            state,
            RemoteImActivationSource {
                channel_id: "remote-im-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                remote_contact_type: "private".to_string(),
                remote_contact_id: "contact-a".to_string(),
                remote_contact_name: "张三".to_string(),
            },
            conversation_id,
            assistant_message_id,
            assistant_text,
        )
    }

    fn read_remote_im_decision_for_message(
        state: &AppState,
        conversation_id: &str,
        assistant_message_id: &str,
    ) -> Value {
        let data = state_read_app_data_cached(state).expect("read app data");
        data.conversations
            .iter()
            .find(|conversation| conversation.id == conversation_id)
            .and_then(|conversation| {
                conversation
                    .messages
                    .iter()
                    .find(|message| message.id == assistant_message_id)
            })
            .and_then(|message| message.provider_meta.as_ref())
            .and_then(|meta| meta.get("remoteImDecision"))
            .cloned()
            .expect("remoteImDecision")
    }

    #[test]
    fn remote_im_auto_send_and_record_decision_should_update_message_after_mock_send() {
        let (state, activation_source, conversation_id, assistant_message_id, assistant_text) =
            seed_remote_im_auto_send_test_state(serde_json::json!({
                "mockSend": true
            }));
        let assistant_message = state_read_app_data_cached(&state)
            .expect("read app data")
            .conversations
            .iter()
            .find(|conversation| conversation.id == conversation_id)
            .and_then(|conversation| {
                conversation
                    .messages
                    .iter()
                    .find(|message| message.id == assistant_message_id)
            })
            .cloned()
            .expect("assistant message");

        let outcome = test_runtime()
            .block_on(remote_im_auto_send_and_record_decision(
                &state,
                &activation_source,
                &conversation_id,
                &assistant_text,
                Some(&assistant_message),
                Some(&assistant_message_id),
            ))
            .expect("auto send should succeed");

        assert_eq!(
            outcome,
            RemoteImAutoSendExecutionOutcome::Sent {
                action: "reply_async".to_string()
            }
        );

        let decision =
            read_remote_im_decision_for_message(&state, &conversation_id, &assistant_message_id);
        assert_eq!(
            decision.get("action").and_then(Value::as_str),
            Some("reply_async")
        );
        assert_eq!(
            decision.get("processingMode").and_then(Value::as_str),
            Some("continuous")
        );
        assert_eq!(
            decision.get("conversationKind").and_then(Value::as_str),
            Some("standard_conversation")
        );
        assert_eq!(
            decision.get("activationSourceCount").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(decision.get("error").and_then(Value::as_str), Some(""));
    }

    #[test]
    fn remote_im_auto_send_and_record_decision_should_mark_send_failed_after_mock_error() {
        let (state, activation_source, conversation_id, assistant_message_id, assistant_text) =
            seed_remote_im_auto_send_test_state(serde_json::json!({
                "mockSendError": "mock remote send failed"
            }));
        let assistant_message = state_read_app_data_cached(&state)
            .expect("read app data")
            .conversations
            .iter()
            .find(|conversation| conversation.id == conversation_id)
            .and_then(|conversation| {
                conversation
                    .messages
                    .iter()
                    .find(|message| message.id == assistant_message_id)
            })
            .cloned()
            .expect("assistant message");

        let err = test_runtime()
            .block_on(remote_im_auto_send_and_record_decision(
                &state,
                &activation_source,
                &conversation_id,
                &assistant_text,
                Some(&assistant_message),
                Some(&assistant_message_id),
            ))
            .expect_err("auto send should fail");

        assert!(err.contains("mock remote send failed"));

        let decision =
            read_remote_im_decision_for_message(&state, &conversation_id, &assistant_message_id);
        assert_eq!(
            decision.get("action").and_then(Value::as_str),
            Some("send_failed")
        );
        assert_eq!(
            decision.get("processingMode").and_then(Value::as_str),
            Some("continuous")
        );
        assert_eq!(
            decision.get("conversationKind").and_then(Value::as_str),
            Some("standard_conversation")
        );
        assert_eq!(
            decision.get("activationSourceCount").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            decision.get("error").and_then(Value::as_str),
            Some("mock remote send failed")
        );
    }

    #[test]
    fn archive_decision_should_force_when_usage_reaches_82pct() {
        let now = now_iso();
        let d = decide_archive_before_model_request(820, 1000, Some(&now), true);
        assert!(d.should_archive);
        assert!(d.forced);
        assert!(d.usage_ratio >= 0.82);
    }

    #[test]
    fn archive_decision_should_archive_after_30m_and_30pct() {
        let now = now_utc();
        let old = (now - time::Duration::minutes(31))
            .format(&Rfc3339)
            .expect("format old time");
        let d = decide_archive_before_model_request(300, 1000, Some(&old), true);
        assert!(d.should_archive);
        assert!(!d.forced);
        assert!(d.usage_ratio >= 0.30);
    }

    #[test]
    fn archive_decision_should_not_archive_when_usage_below_30pct() {
        let now = now_utc();
        let old = (now - time::Duration::minutes(31))
            .format(&Rfc3339)
            .expect("format old time");
        let d = decide_archive_before_model_request(299, 1000, Some(&old), true);
        assert!(!d.should_archive);
        assert!(!d.forced);
        assert!(d.usage_ratio < 0.30);
    }

    #[test]
    fn archive_decision_should_use_prepared_prompt_usage_before_model_request() {
        let now = now_iso();
        let d = decide_archive_before_model_request(166_000, 200_000, Some(&now), true);
        assert!(d.should_archive);
        assert!(d.forced);
        assert!(d.usage_ratio >= 0.82);
    }

    #[test]
    fn archive_decision_should_prefer_cached_effective_prompt_tokens() {
        let now = now_iso();
        let (decision, source) =
            decide_archive_before_send_with_fallback(820, 0.10, Some(100), 1000, Some(&now), true);
        assert_eq!(source, "cached_effective_prompt_tokens");
        assert!(decision.should_archive);
        assert!(decision.forced);
        assert!(decision.usage_ratio >= 0.82);
    }

    #[test]
    fn archive_decision_should_fallback_to_estimate_only_when_cache_missing() {
        let now = now_iso();
        let (decision, source) =
            decide_archive_before_send_with_fallback(0, 0.0, Some(820), 1000, Some(&now), true);
        assert_eq!(source, "estimated_prompt_tokens");
        assert!(!decision.should_archive);
        assert!(!decision.forced);
        assert!(decision.usage_ratio >= 0.82);
    }

    #[test]
    fn archive_decision_should_force_only_at_95pct_when_estimate_is_used() {
        let now = now_iso();
        let (decision, source) =
            decide_archive_before_send_with_fallback(0, 0.0, Some(950), 1000, Some(&now), true);
        assert_eq!(source, "estimated_prompt_tokens");
        assert!(decision.should_archive);
        assert!(decision.forced);
        assert_eq!(decision.reason, "force_estimated_context_usage_95");
    }

    #[test]
    fn latest_real_prompt_usage_should_prefer_latest_assistant_message_provider_meta() {
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-main", "active", &now);
        conversation.messages.push(ChatMessage {
            id: "assistant-1".to_string(),
            role: "assistant".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "这是最近一条助手消息".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "effectivePromptTokens": 640,
                "contextUsageRatio": 0.64,
                "contextUsagePercent": 64
            })),
            tool_call: None,
            mcp_call: None,
        });
        let agent = AgentProfile {
            id: DEFAULT_AGENT_ID.to_string(),
            name: "默认助手".to_string(),
            system_prompt: String::new(),
            tools: Vec::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            source: "global".to_string(),
            scope: "global".to_string(),
        };
        let usage = conversation_prompt_service()
            .latest_real_prompt_usage(&conversation, &ApiConfig::default())
            .expect("latest real prompt usage");

        assert_eq!(usage.source, "assistant_message_effective_prompt_tokens");
        assert_eq!(usage.effective_prompt_tokens, 640);
        assert!((usage.usage_ratio - 0.64).abs() < f64::EPSILON);
        assert!(usage.estimated_prompt_tokens.is_none());

        let prepared = PreparedPrompt {
            preamble: String::new(),
            history_messages: Vec::new(),
            latest_user_text: String::new(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };
        let resolved = conversation_prompt_service().resolve_prompt_usage(
            &prepared,
            &ApiConfig::default(),
            &agent,
            &conversation,
        );
        assert_eq!(resolved, usage);
    }

    #[test]
    fn latest_real_prompt_usage_should_not_cross_context_compaction_boundary() {
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-main", "active", &now);
        conversation.messages.push(ChatMessage {
            id: "assistant-before-compaction".to_string(),
            role: "assistant".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "压缩前的一条助手消息".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "effectivePromptTokens": 640,
                "contextUsageRatio": 0.64,
                "contextUsagePercent": 64
            })),
            tool_call: None,
            mcp_call: None,
        });
        conversation.messages.push(ChatMessage {
            id: "compaction-boundary".to_string(),
            role: "user".to_string(),
            created_at: now.clone(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "上下文整理".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "message_meta": {
                    "kind": "context_compaction"
                }
            })),
            tool_call: None,
            mcp_call: None,
        });
        conversation.messages.push(ChatMessage {
            id: "user-after-compaction".to_string(),
            role: "user".to_string(),
            created_at: now.clone(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "压缩后的用户消息".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        });

        let usage =
            conversation_prompt_service().latest_real_prompt_usage(&conversation, &ApiConfig::default());
        assert!(usage.is_none());
    }

    #[test]
    fn resolve_prompt_usage_should_ignore_conversation_last_fields_without_assistant_meta() {
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-main", "active", &now);
        conversation.messages.push(ChatMessage {
            id: "assistant-1".to_string(),
            role: "assistant".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "这是最近一条没有 provider meta 的助手消息".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        });
        let agent = AgentProfile {
            id: DEFAULT_AGENT_ID.to_string(),
            name: "默认助手".to_string(),
            system_prompt: String::new(),
            tools: Vec::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            source: "global".to_string(),
            scope: "global".to_string(),
        };
        let prepared = PreparedPrompt {
            preamble: "系统提示词".to_string(),
            history_messages: Vec::new(),
            latest_user_text: "用户消息".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let usage = conversation_prompt_service().resolve_prompt_usage(
            &prepared,
            &ApiConfig::default(),
            &agent,
            &conversation,
        );

        assert_eq!(usage.source, "estimated_prompt_tokens");
        assert!(usage.estimated_prompt_tokens.is_some());
        assert!(usage.effective_prompt_tokens > 0);
        assert!(usage.usage_ratio > 0.0);
    }

    #[test]
    fn runtime_trusted_prompt_usage_should_be_consumed_once_then_estimate() {
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-main", "active", &now);
        conversation.messages.push(ChatMessage {
            id: "assistant-1".to_string(),
            role: "assistant".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "最近一次真实返回".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "effectivePromptTokens": 640,
                "contextUsageRatio": 0.64,
                "contextUsagePercent": 64
            })),
            tool_call: None,
            mcp_call: None,
        });
        let agent = AgentProfile {
            id: DEFAULT_AGENT_ID.to_string(),
            name: "默认助手".to_string(),
            system_prompt: String::new(),
            tools: Vec::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            source: "global".to_string(),
            scope: "global".to_string(),
        };
        let prepared = PreparedPrompt {
            preamble: "系统提示词".to_string(),
            history_messages: Vec::new(),
            latest_user_text: "用户消息".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };
        let mut runtime_context = RuntimeContext::default();
        conversation_prompt_service().prime_runtime_trusted_prompt_usage(
            &mut runtime_context,
            &conversation,
            &ApiConfig::default(),
        );

        let first = conversation_prompt_service().consume_runtime_trusted_prompt_usage_or_estimate(
            &mut runtime_context,
            &prepared,
            &ApiConfig::default(),
            &agent,
        );
        assert_eq!(first.source, "trusted_prompt_usage");
        assert_eq!(first.effective_prompt_tokens, 640);
        assert!(first.estimated_prompt_tokens.is_none());
        assert!(runtime_context.trusted_prompt_usage.is_none());

        let second = conversation_prompt_service().consume_runtime_trusted_prompt_usage_or_estimate(
            &mut runtime_context,
            &prepared,
            &ApiConfig::default(),
            &agent,
        );
        assert_eq!(second.source, "estimated_prompt_tokens");
        assert!(second.estimated_prompt_tokens.is_some());
    }

    #[test]
    fn shared_trusted_prompt_usage_should_refresh_after_provider_response() {
        let now = now_iso();
        let agent = AgentProfile {
            id: DEFAULT_AGENT_ID.to_string(),
            name: "默认助手".to_string(),
            system_prompt: String::new(),
            tools: Vec::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            source: "global".to_string(),
            scope: "global".to_string(),
        };
        let prepared = PreparedPrompt {
            preamble: "系统提示词".to_string(),
            history_messages: Vec::new(),
            latest_user_text: "用户消息".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };
        let trusted = std::sync::Mutex::new(None::<TrustedPromptUsage>);

        conversation_prompt_service().refresh_shared_trusted_prompt_usage(
            &trusted,
            Some(640),
            &ApiConfig::default(),
        );
        let first = conversation_prompt_service().consume_shared_trusted_prompt_usage_or_estimate(
            &trusted,
            &prepared,
            &ApiConfig::default(),
            &agent,
        );
        assert_eq!(first.source, "trusted_prompt_usage");
        assert_eq!(first.effective_prompt_tokens, 640);
        assert!(first.estimated_prompt_tokens.is_none());

        let second = conversation_prompt_service().consume_shared_trusted_prompt_usage_or_estimate(
            &trusted,
            &prepared,
            &ApiConfig::default(),
            &agent,
        );
        assert_eq!(second.source, "estimated_prompt_tokens");
        assert!(second.estimated_prompt_tokens.is_some());
    }

    #[test]
    fn decide_archive_before_send_from_trusted_usage_should_use_real_threshold_branch() {
        let usage = PromptUsageResolution {
            effective_prompt_tokens: 240_845,
            usage_ratio: 0.8854595588235294,
            estimated_prompt_tokens: None,
            source: "trusted_prompt_usage",
        };

        let (decision, source) = decide_archive_before_send_from_usage(
            &usage,
            Some(&now_iso()),
            true,
        );

        assert_eq!(source, "trusted_prompt_usage");
        assert!(decision.should_archive);
        assert!(decision.forced);
        assert_eq!(decision.reason, "force_context_usage_82");
    }

    fn test_chat_runtime_state() -> AppState {
        let root = std::env::temp_dir().join(format!("eca-chat-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp test root");
        std::fs::create_dir_all(root.join("llm-workspace")).expect("create temp llm workspace");
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: root.join("app_config.toml"),
            data_path: root.join("app_data.json"),
            llm_workspace_path: root.join("llm-workspace"),
            shared_http_client: reqwest::Client::new(),
            terminal_shell: detect_default_terminal_shell(),
            terminal_shell_candidates: detect_terminal_shell_candidates(),
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_agents: Arc::new(Mutex::new(None)),
            cached_agents_mtime: Arc::new(Mutex::new(None)),
            cached_runtime_state: Arc::new(Mutex::new(None)),
            cached_runtime_state_mtime: Arc::new(Mutex::new(None)),
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_chat_index_mtime: Arc::new(Mutex::new(None)),
            cached_conversations: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_pending: Arc::new(Mutex::new(None)),
            app_data_persist_notify: Arc::new(tokio::sync::Notify::new()),
            app_data_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_chat_index_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            llm_round_logs: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            delegate_runtime_threads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            conversation_index_repair_gates: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        }
    }

    fn test_pending_event(conversation_id: &str) -> ChatPendingEvent {
        let created_at = now_iso();
        ChatPendingEvent {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            created_at: created_at.clone(),
            source: ChatEventSource::User,
            queue_mode: ChatQueueMode::Normal,
            messages: vec![test_text_message("user", "hello", &created_at)],
            activate_assistant: true,
            session_info: ChatSessionInfo {
                department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
            },
            runtime_context: None,
            sender_info: None,
        }
    }

    fn test_chat_conversation(conversation_id: &str, status: &str, updated_at: &str) -> Conversation {
        Conversation {
            id: conversation_id.to_string(),
            title: conversation_id.to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: String::new(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
            last_user_at: None,
            last_assistant_at: None,
            status: status.to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        }
    }

    fn test_user_switched_to_sub_conversation_data() -> AppData {
        let now = now_iso();
        let later = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format later");
        let mut data = AppData::default();
        data.main_conversation_id = Some("conversation-main".to_string());
        data.conversations = vec![
            test_chat_conversation("conversation-main", "inactive", &now),
            test_chat_conversation("conversation-sub", "active", &later),
        ];
        data
    }

    fn total_queue_len(state: &AppState) -> Result<usize, String> {
        let slots = state
            .conversation_runtime_slots
            .lock()
            .map_err(|err| format!("lock conversation_runtime_slots failed: {err}"))?;
        Ok(slots.values().map(|slot| slot.pending_queue.len()).sum())
    }

    #[test]
    fn state_read_conversation_cached_should_self_heal_missing_chat_index_item() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-heal", "active", &now);

        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        write_chat_index_shard(&state.data_path, &ChatIndexFile::default())
            .expect("write empty chat index");

        let loaded = state_read_conversation_cached(&state, &conversation.id)
            .expect("read conversation with self heal");
        assert_eq!(loaded.id, conversation.id);

        let chat_index = read_chat_index_shard(&state.data_path).expect("read chat index");
        let item = chat_index
            .conversations
            .iter()
            .find(|item| item.id == conversation.id)
            .expect("healed chat index item");
        assert_eq!(item.updated_at, conversation.updated_at);
        assert_eq!(item.status, conversation.status);
        assert_eq!(item.summary, conversation.summary);
        assert_eq!(item.archived_at, conversation.archived_at);
    }

    #[test]
    fn state_read_conversation_cached_should_self_heal_stale_hidden_chat_index_item() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-hidden-heal", "active", &now);
        conversation.summary = "测试摘要".to_string();

        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        write_chat_index_shard(
            &state.data_path,
            &ChatIndexFile {
                conversations: vec![ChatIndexConversationItem {
                    id: conversation.id.clone(),
                    updated_at: conversation.updated_at.clone(),
                    status: conversation.status.clone(),
                    summary: String::new(),
                    archived_at: conversation.archived_at.clone(),
                }],
            },
        )
        .expect("write stale chat index");

        let loaded = state_read_conversation_cached(&state, &conversation.id)
            .expect("read conversation with stale index self heal");
        assert_eq!(loaded.id, conversation.id);

        let chat_index = read_chat_index_shard(&state.data_path).expect("read chat index");
        assert!(chat_index
            .conversations
            .iter()
            .all(|item| item.id != conversation.id));
    }

    #[test]
    fn collect_unarchived_conversation_summaries_cached_should_remove_orphan_index_items() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-existing", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        write_chat_index_shard(
            &state.data_path,
            &ChatIndexFile {
                conversations: vec![
                    build_chat_index_item(&conversation),
                    ChatIndexConversationItem {
                        id: "conversation-orphan".to_string(),
                        updated_at: now.clone(),
                        status: "active".to_string(),
                        summary: String::new(),
                        archived_at: None,
                    },
                ],
            },
        )
        .expect("write stale chat index");

        let summaries = conversation_service()
            .collect_unarchived_conversation_summaries_cached(&state, &AppConfig::default())
            .expect("collect summaries");

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].conversation_id, conversation.id);
        let chat_index = read_chat_index_shard(&state.data_path).expect("read chat index");
        assert_eq!(chat_index.conversations.len(), 1);
        assert_eq!(chat_index.conversations[0].id, conversation.id);
    }

    #[test]
    fn collect_unarchived_conversation_summaries_cached_should_repair_visible_summary_mismatch() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-visible-summary-mismatch", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        write_chat_index_shard(
            &state.data_path,
            &ChatIndexFile {
                conversations: vec![ChatIndexConversationItem {
                    id: conversation.id.clone(),
                    updated_at: conversation.updated_at.clone(),
                    status: conversation.status.clone(),
                    summary: "旧摘要".to_string(),
                    archived_at: conversation.archived_at.clone(),
                }],
            },
        )
        .expect("write mismatched chat index");

        let summaries = conversation_service()
            .collect_unarchived_conversation_summaries_cached(&state, &AppConfig::default())
            .expect("collect summaries");

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].conversation_id, conversation.id);
        let chat_index = read_chat_index_shard(&state.data_path).expect("read chat index");
        assert_eq!(chat_index.conversations.len(), 1);
        assert_eq!(chat_index.conversations[0].id, conversation.id);
        assert_eq!(chat_index.conversations[0].summary, conversation.summary);
    }

    #[test]
    fn scheduler_should_allow_two_conversations_to_run_in_parallel() {
        let state = test_chat_runtime_state();
        let ingress_a =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("ingress a");
        let ingress_b =
            ingress_chat_event(&state, test_pending_event("conversation-b")).expect("ingress b");

        assert!(matches!(ingress_a, ChatEventIngress::Direct(_)));
        assert!(matches!(ingress_b, ChatEventIngress::Direct(_)));
        assert_eq!(total_queue_len(&state).expect("queue len"), 0);

        let claims = state
            .conversation_processing_claims
            .lock()
            .expect("lock claims");
        assert!(claims.contains("conversation-a"));
        assert!(claims.contains("conversation-b"));
        assert_eq!(claims.len(), 2);
    }

    #[test]
    fn scheduler_should_keep_same_conversation_serial() {
        let state = test_chat_runtime_state();
        let ingress_a1 =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("ingress a1");
        let ingress_a2 =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("ingress a2");

        assert!(matches!(ingress_a1, ChatEventIngress::Direct(_)));
        assert!(matches!(ingress_a2, ChatEventIngress::Queued { .. }));
        assert_eq!(total_queue_len(&state).expect("queue len"), 1);
    }

    #[test]
    fn scheduler_should_ignore_duplicate_user_event_after_message_persisted() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-a", "active", &now);
        let mut persisted = test_text_message("user", "hello", &now);
        persisted.provider_meta = build_user_message_provider_meta(
            None,
            &[],
            &[],
            Some("chat-same-request"),
        );
        conversation.messages.push(persisted);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        write_chat_index_shard(
            &state.data_path,
            &ChatIndexFile {
                conversations: vec![build_chat_index_item(&conversation)],
            },
        )
        .expect("write chat index");
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::AssistantStreaming)
            .expect("set streaming state");

        let mut duplicate = test_pending_event("conversation-a");
        duplicate.runtime_context = Some(RuntimeContext {
            request_id: Some("chat-same-request".to_string()),
            ..RuntimeContext::default()
        });

        let ingress = ingress_chat_event(&state, duplicate).expect("ingress duplicate");

        assert!(matches!(ingress, ChatEventIngress::Duplicate { .. }));
        assert_eq!(total_queue_len(&state).expect("queue len"), 0);
    }

    #[test]
    fn queue_snapshot_should_keep_full_message_text_for_recall() {
        let state = test_chat_runtime_state();
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::AssistantStreaming)
            .expect("set streaming state");
        let long_text = "0123456789".repeat(8);
        let created_at = now_iso();
        let mut event = test_pending_event("conversation-a");
        event.messages = vec![test_text_message("user", &long_text, &created_at)];

        let ingress = ingress_chat_event(&state, event).expect("queue event");
        assert!(matches!(ingress, ChatEventIngress::Queued { .. }));

        let snapshot = get_queue_snapshot(&state).expect("queue snapshot");
        assert_eq!(snapshot.len(), 1);
        assert!(snapshot[0].message_preview.ends_with("..."));
        assert_eq!(snapshot[0].message_text, long_text);
    }

    #[test]
    fn scheduler_should_allow_eight_conversations_and_queue_the_ninth() {
        let state = test_chat_runtime_state();
        for idx in 0..8 {
            let conversation_id = format!("conversation-{idx}");
            let ingress = ingress_chat_event(&state, test_pending_event(&conversation_id))
                .unwrap_or_else(|_| panic!("ingress {conversation_id}"));
            assert!(
                matches!(ingress, ChatEventIngress::Direct(_)),
                "expected direct ingress for {conversation_id}"
            );
        }

        let ninth = ingress_chat_event(&state, test_pending_event("conversation-8"))
            .expect("ingress ninth");
        assert!(matches!(ninth, ChatEventIngress::Queued { .. }));
        assert_eq!(total_queue_len(&state).expect("queue len"), 1);

        let claims = state
            .conversation_processing_claims
            .lock()
            .expect("lock claims");
        assert_eq!(claims.len(), 8);
        assert!(!claims.contains("conversation-8"));
    }

    #[test]
    fn compaction_state_should_only_block_its_own_conversation() {
        let state = test_chat_runtime_state();
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::OrganizingContext)
            .expect("set conversation state");

        let ingress_same =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("same ingress");
        let ingress_other =
            ingress_chat_event(&state, test_pending_event("conversation-b")).expect("other ingress");

        assert!(matches!(ingress_same, ChatEventIngress::Queued { .. }));
        assert!(matches!(ingress_other, ChatEventIngress::Direct(_)));
    }

    #[test]
    fn ensure_main_conversation_index_should_keep_notification_home_stable() {
        let now = now_iso();
        let later = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format later");
        let mut data = AppData::default();
        data.main_conversation_id = Some("conversation-main".to_string());
        data.conversations = vec![
            test_chat_conversation("conversation-main", "inactive", &now),
            test_chat_conversation("conversation-sub", "active", &later),
        ];

        let idx = ensure_main_conversation_index(&mut data, "", DEFAULT_AGENT_ID);

        assert_eq!(data.conversations[idx].id, "conversation-main");
        assert_eq!(
            data.main_conversation_id.as_deref(),
            Some("conversation-main")
        );
    }

    #[test]
    fn normalize_single_active_main_conversation_should_keep_all_foreground_chats_active() {
        let now = now_iso();
        let later = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format later");
        let mut data = AppData::default();
        data.main_conversation_id = Some("conversation-main".to_string());
        data.conversations = vec![
            test_chat_conversation("conversation-main", "inactive", &now),
            test_chat_conversation("conversation-sub", "active", &later),
        ];

        let changed = normalize_single_active_main_conversation(&mut data);

        assert!(changed);
        assert_eq!(data.conversations[0].status, "active");
        assert_eq!(data.conversations[1].status, "active");
    }

    #[test]
    fn collect_unarchived_conversation_summaries_should_include_last_two_preview_messages() {
        let state = test_chat_runtime_state();
        let first = now_iso();
        let second = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format second");
        let third = (now_utc() + time::Duration::minutes(2))
            .format(&Rfc3339)
            .expect("format third");
        let mut data = AppData::default();
        data.main_conversation_id = Some("conversation-main".to_string());
        let mut conversation = test_chat_conversation("conversation-main", "active", &third);
        conversation.messages = vec![
            ChatMessage {
                id: "msg-1".to_string(),
                role: "user".to_string(),
                created_at: first.clone(),
                speaker_agent_id: Some("user-persona".to_string()),
                parts: vec![MessagePart::Text {
                    text: "第一条".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            },
            ChatMessage {
                id: "msg-2".to_string(),
                role: "assistant".to_string(),
                created_at: second.clone(),
                speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                parts: vec![MessagePart::Text {
                    text: "第二条".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            },
            ChatMessage {
                id: "msg-3".to_string(),
                role: "user".to_string(),
                created_at: third.clone(),
                speaker_agent_id: Some("user-persona".to_string()),
                parts: vec![MessagePart::Text {
                    text: "第三条".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "attachments": [
                        {
                            "fileName": "notes.txt",
                            "relativePath": "downloads/notes.txt"
                        }
                    ]
                })),
                tool_call: None,
                mcp_call: None,
            },
        ];
        data.conversations = vec![conversation];

        let summaries = collect_unarchived_conversation_summaries(&state, &AppConfig::default(), &data);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].workspace_label, "");
        assert!(!summaries[0].detached_window_open);
        assert_eq!(summaries[0].detached_window_label, None);
        assert_eq!(summaries[0].preview_messages.len(), 2);
        assert_eq!(summaries[0].preview_messages[0].message_id, "msg-2");
        assert_eq!(summaries[0].preview_messages[0].text_preview, "第二条");
        assert_eq!(summaries[0].preview_messages[1].message_id, "msg-3");
        assert_eq!(summaries[0].preview_messages[1].text_preview, "第三条");
        assert!(summaries[0].preview_messages[1].has_attachment);
    }

    #[test]
    fn task_resolve_dispatch_session_should_prefer_task_bound_conversation() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");

        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");
        let task = TaskRecordStored {
            task_id: "task-a".to_string(),
            conversation_id: Some("conversation-sub".to_string()),
            target_scope: TASK_TARGET_SCOPE_DESKTOP.to_string(),
            order_index: 1,
            title: "t".to_string(),
            cause: String::new(),
            goal: String::new(),
            flow: String::new(),
            todos: Vec::new(),
            status_summary: String::new(),
            completion_state: TASK_STATE_ACTIVE.to_string(),
            completion_conclusion: String::new(),
            progress_notes: Vec::new(),
            stage_key: String::new(),
            stage_updated_at_utc: None,
            trigger: TaskTriggerStored {
                run_at_utc: None,
                every_minutes: None,
                end_at_utc: None,
                next_run_at_utc: None,
            },
            created_at_utc: now_utc_rfc3339(),
            updated_at_utc: now_utc_rfc3339(),
            last_triggered_at_utc: None,
            completed_at_utc: None,
        };

        let session = task_resolve_dispatch_session(&state, &task)
            .expect("resolve task session")
            .expect("dispatch session");

        assert_eq!(session.conversation_id, "conversation-sub");
        assert_eq!(session.target_scope, TASK_TARGET_SCOPE_DESKTOP);
    }

    #[test]
    fn task_resolve_dispatch_session_should_fallback_to_main_when_bound_conversation_missing() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");
        let task = TaskRecordStored {
            task_id: "task-b".to_string(),
            conversation_id: Some("conversation-missing".to_string()),
            target_scope: TASK_TARGET_SCOPE_DESKTOP.to_string(),
            order_index: 1,
            title: "t".to_string(),
            cause: String::new(),
            goal: String::new(),
            flow: String::new(),
            todos: Vec::new(),
            status_summary: String::new(),
            completion_state: TASK_STATE_ACTIVE.to_string(),
            completion_conclusion: String::new(),
            progress_notes: Vec::new(),
            stage_key: String::new(),
            stage_updated_at_utc: None,
            trigger: TaskTriggerStored {
                run_at_utc: None,
                every_minutes: None,
                end_at_utc: None,
                next_run_at_utc: None,
            },
            created_at_utc: now_utc_rfc3339(),
            updated_at_utc: now_utc_rfc3339(),
            last_triggered_at_utc: None,
            completed_at_utc: None,
        };

        let session = task_resolve_dispatch_session(&state, &task)
            .expect("resolve task session")
            .expect("dispatch session");
        let updated = state_read_app_data_cached(&state).expect("read app data");

        assert_eq!(session.conversation_id, "conversation-main");
        assert!(session.fallback_to_main);
        assert_eq!(updated.main_conversation_id.as_deref(), Some("conversation-main"));
        assert_eq!(
            updated
                .conversations
                .iter()
                .find(|item| item.id == "conversation-sub")
                .map(|item| item.status.as_str()),
            Some("active")
        );
    }

    #[test]
    fn task_resolve_dispatch_session_should_skip_missing_contact_conversation() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");

        let mut data = test_user_switched_to_sub_conversation_data();
        data.remote_im_contacts.push(RemoteImContact {
            id: "contact-a".to_string(),
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "group".to_string(),
            remote_contact_id: "remote-a".to_string(),
            remote_contact_name: "测试群".to_string(),
            remark_name: String::new(),
            allow_send: false,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "never".to_string(),
            activation_keywords: Vec::new(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_department_id: Some(REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID.to_string()),
            bound_conversation_id: Some("conversation-contact-missing".to_string()),
            processing_mode: "continuous".to_string(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
        });
        state_write_app_data_cached(&state, &data).expect("write app data");
        let task = TaskRecordStored {
            task_id: "task-contact".to_string(),
            conversation_id: Some("conversation-contact-missing".to_string()),
            target_scope: TASK_TARGET_SCOPE_CONTACT.to_string(),
            order_index: 1,
            title: "t".to_string(),
            cause: String::new(),
            goal: String::new(),
            flow: String::new(),
            todos: Vec::new(),
            status_summary: String::new(),
            completion_state: TASK_STATE_ACTIVE.to_string(),
            completion_conclusion: String::new(),
            progress_notes: Vec::new(),
            stage_key: String::new(),
            stage_updated_at_utc: None,
            trigger: TaskTriggerStored {
                run_at_utc: None,
                every_minutes: None,
                end_at_utc: None,
                next_run_at_utc: None,
            },
            created_at_utc: now_utc_rfc3339(),
            updated_at_utc: now_utc_rfc3339(),
            last_triggered_at_utc: None,
            completed_at_utc: None,
        };

        let session = task_resolve_dispatch_session(&state, &task).expect("resolve task session");

        assert!(session.is_none());
    }

    #[test]
    fn task_conversation_last_message_is_system_persona_should_detect_system_message() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut data = AppData::default();
        data.main_conversation_id = Some("conversation-main".to_string());
        data.conversations = vec![Conversation {
            id: "conversation-main".to_string(),
            title: "main".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: String::new(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            last_user_at: None,
            last_assistant_at: Some(now.clone()),
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: vec![ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
                parts: vec![MessagePart::Text {
                    text: "任务提醒".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            }],
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        }];
        state_write_app_data_cached(&state, &data).expect("write app data");

        let blocked = task_conversation_last_message_is_system_persona(&state, "conversation-main")
            .expect("check last system message");

        assert!(blocked);
    }

    #[test]
    fn task_build_dispatch_candidates_should_keep_oldest_due_task_per_conversation() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");

        let tasks = vec![
            TaskRecordStored {
                task_id: "task-1".to_string(),
                conversation_id: Some("conversation-main".to_string()),
                target_scope: TASK_TARGET_SCOPE_DESKTOP.to_string(),
                order_index: 1,
                title: "t1".to_string(),
                cause: String::new(),
                goal: String::new(),
                flow: String::new(),
                todos: Vec::new(),
                status_summary: String::new(),
                completion_state: TASK_STATE_ACTIVE.to_string(),
                completion_conclusion: String::new(),
                progress_notes: Vec::new(),
                stage_key: String::new(),
                stage_updated_at_utc: None,
                trigger: TaskTriggerStored {
                    run_at_utc: None,
                    every_minutes: None,
                    end_at_utc: None,
                    next_run_at_utc: None,
                },
                created_at_utc: now_utc_rfc3339(),
                updated_at_utc: now_utc_rfc3339(),
                last_triggered_at_utc: None,
                completed_at_utc: None,
            },
            TaskRecordStored {
                task_id: "task-2".to_string(),
                conversation_id: Some("conversation-main".to_string()),
                target_scope: TASK_TARGET_SCOPE_DESKTOP.to_string(),
                order_index: 2,
                title: "t2".to_string(),
                cause: String::new(),
                goal: String::new(),
                flow: String::new(),
                todos: Vec::new(),
                status_summary: String::new(),
                completion_state: TASK_STATE_ACTIVE.to_string(),
                completion_conclusion: String::new(),
                progress_notes: Vec::new(),
                stage_key: String::new(),
                stage_updated_at_utc: None,
                trigger: TaskTriggerStored {
                    run_at_utc: None,
                    every_minutes: None,
                    end_at_utc: None,
                    next_run_at_utc: None,
                },
                created_at_utc: now_utc_rfc3339(),
                updated_at_utc: now_utc_rfc3339(),
                last_triggered_at_utc: None,
                completed_at_utc: None,
            },
            TaskRecordStored {
                task_id: "task-3".to_string(),
                conversation_id: Some("conversation-sub".to_string()),
                target_scope: TASK_TARGET_SCOPE_DESKTOP.to_string(),
                order_index: 3,
                title: "t3".to_string(),
                cause: String::new(),
                goal: String::new(),
                flow: String::new(),
                todos: Vec::new(),
                status_summary: String::new(),
                completion_state: TASK_STATE_ACTIVE.to_string(),
                completion_conclusion: String::new(),
                progress_notes: Vec::new(),
                stage_key: String::new(),
                stage_updated_at_utc: None,
                trigger: TaskTriggerStored {
                    run_at_utc: None,
                    every_minutes: None,
                    end_at_utc: None,
                    next_run_at_utc: None,
                },
                created_at_utc: now_utc_rfc3339(),
                updated_at_utc: now_utc_rfc3339(),
                last_triggered_at_utc: None,
                completed_at_utc: None,
            },
        ];

        let candidates =
            task_build_dispatch_candidates(&state, tasks, now_utc()).expect("build dispatch candidates");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].task.task_id, "task-1");
        assert_eq!(candidates[0].session.conversation_id, "conversation-main");
        assert_eq!(candidates[1].task.task_id, "task-3");
        assert_eq!(candidates[1].session.conversation_id, "conversation-sub");
    }

    #[test]
    fn task_build_dispatch_candidates_should_skip_busy_conversation() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");
        set_conversation_runtime_state(&state, "conversation-main", MainSessionState::OrganizingContext)
            .expect("set busy");

        let created = task_store_create_task(&state.data_path, &TaskCreateInput {
            goal: "busy".to_string(),
            conversation_id: Some("conversation-main".to_string()),
            target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
            why: String::new(),
            todo: String::new(),
            trigger: TaskTriggerInputLocal {
                run_at_local: Some("2026-04-10T10:00:00+08:00".to_string()),
                every_minutes: Some(30.0),
                end_at_local: Some("2099-04-10T12:00:00+08:00".to_string()),
            },
        })
        .expect("create busy task");
        let tasks = vec![task_store_get_task_record(&state.data_path, &created.task_id)
            .expect("get busy task record")];

        let candidates =
            task_build_dispatch_candidates(&state, tasks, now_utc()).expect("build dispatch candidates");

        assert!(candidates.is_empty());
        let skipped = task_store_get_task_record(&state.data_path, &created.task_id)
            .expect("get skipped task record");
        assert!(skipped.last_triggered_at_utc.is_some());
    }

    #[test]
    fn task_try_ingress_chat_event_direct_should_skip_when_queue_not_empty() {
        let state = test_chat_runtime_state();
        let existing = ingress_chat_event(&state, test_pending_event("conversation-main"))
            .expect("existing ingress");
        assert!(matches!(existing, ChatEventIngress::Direct(_)));
        let queued = ingress_chat_event(&state, test_pending_event("conversation-main"))
            .expect("queued ingress");
        assert!(matches!(queued, ChatEventIngress::Queued { .. }));

        let result = task_try_ingress_chat_event_direct(&state, test_pending_event("conversation-main"))
            .expect("task direct ingress");

        assert!(matches!(result, Err("conversation_queue_not_empty")));
    }

    #[test]
    fn delegate_parse_session_parts_should_preserve_conversation_in_two_segment_session() {
        let (api_config_id, agent_id, conversation_id) =
            delegate_parse_session_parts("default-agent::conversation-sub");

        assert_eq!(api_config_id, "");
        assert_eq!(agent_id, "default-agent");
        assert_eq!(conversation_id.as_deref(), Some("conversation-sub"));
    }

    #[test]
    fn delegate_parse_session_parts_should_reject_legacy_three_segment_session() {
        let (api_config_id, agent_id, conversation_id) =
            delegate_parse_session_parts("api-config-a::default-agent::conversation-sub");

        assert_eq!(api_config_id, "");
        assert_eq!(agent_id, "");
        assert_eq!(conversation_id, None);
    }

    #[test]
    fn delegate_target_chat_api_config_ids_should_only_keep_current_department_models() {
        let app_config = AppConfig {
            api_configs: vec![ApiConfig {
                id: "provider-a::model-a".to_string(),
                name: "provider-a/model-a".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_tools: true,
                tools: vec![],
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "k".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                model: "gpt-4o-mini".to_string(),
                reasoning_effort: default_reasoning_effort(),
                temperature: 1.0,
                custom_temperature_enabled: false,
                context_window_tokens: 128_000,
                max_output_tokens: 4_096,
                custom_max_output_tokens_enabled: false,
                failure_retry_count: 0,
            }],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let department = DepartmentConfig {
            id: "dept-a".to_string(),
            name: "部门 A".to_string(),
            summary: String::new(),
            guide: String::new(),
            api_config_ids: vec!["provider-a".to_string(), "provider-a::model-a".to_string()],
            api_config_id: "provider-a".to_string(),
            agent_ids: vec![DEFAULT_AGENT_ID.to_string()],
            created_at: now_utc_rfc3339(),
            updated_at: now_utc_rfc3339(),
            order_index: 1,
            is_built_in_assistant: false,
            is_deputy: false,
            source: "main_config".to_string(),
            scope: "global".to_string(),
            permission_control: DepartmentPermissionControl::default(),
        };

        let resolved = delegate_target_chat_api_config_ids(&app_config, &department);

        assert_eq!(resolved, vec!["provider-a::model-a".to_string()]);
    }

    #[test]
    fn delegate_target_chat_api_config_ids_should_not_fallback_when_department_binding_invalid() {
        let app_config = AppConfig {
            api_configs: vec![ApiConfig {
                id: "provider-a::model-a".to_string(),
                name: "provider-a/model-a".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_tools: true,
                tools: vec![],
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "k".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                model: "gpt-4o-mini".to_string(),
                reasoning_effort: default_reasoning_effort(),
                temperature: 1.0,
                custom_temperature_enabled: false,
                context_window_tokens: 128_000,
                max_output_tokens: 4_096,
                custom_max_output_tokens_enabled: false,
                failure_retry_count: 0,
            }],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        let department = DepartmentConfig {
            id: "dept-a".to_string(),
            name: "部门 A".to_string(),
            summary: String::new(),
            guide: String::new(),
            api_config_ids: vec!["provider-a".to_string()],
            api_config_id: "provider-a".to_string(),
            agent_ids: vec![DEFAULT_AGENT_ID.to_string()],
            created_at: now_utc_rfc3339(),
            updated_at: now_utc_rfc3339(),
            order_index: 1,
            is_built_in_assistant: false,
            is_deputy: false,
            source: "main_config".to_string(),
            scope: "global".to_string(),
            permission_control: DepartmentPermissionControl::default(),
        };

        let resolved = delegate_target_chat_api_config_ids(&app_config, &department);

        assert!(resolved.is_empty());
    }

    #[test]
    fn update_conversation_todos_and_emit_should_persist_conversation_todos() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let now = now_utc_rfc3339();
        let mut data = AppData::default();
        data.conversations.push(Conversation {
            id: "conversation-main".to_string(),
            title: "主会话".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: String::new(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        });
        state_write_app_data_cached(&state, &data).expect("write app data");

        update_conversation_todos_and_emit(
            &state,
            "conversation-main",
            vec![
                ConversationTodoItem {
                    content: "第一步".to_string(),
                    status: "completed".to_string(),
                },
                ConversationTodoItem {
                    content: "第二步".to_string(),
                    status: "in_progress".to_string(),
                },
            ],
        )
        .expect("update conversation todos");

        let data = state_read_app_data_cached(&state).expect("read app data");
        let conversation = data
            .conversations
            .iter()
            .find(|item| item.id == "conversation-main")
            .expect("conversation exists");
        assert_eq!(conversation.current_todos.len(), 2);
        assert_eq!(conversation.current_todos[0].content, "第一步");
        assert_eq!(conversation.current_todos[0].status, "completed");
        assert_eq!(conversation.current_todos[1].content, "第二步");
        assert_eq!(conversation.current_todos[1].status, "in_progress");
        assert_eq!(
            conversation_current_todo_text(conversation).as_deref(),
            Some("第二步")
        );
    }

    #[test]
    fn update_conversation_todos_and_emit_should_clear_todos_when_all_completed() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut data = AppData::default();
        data.conversations.push(Conversation {
            id: "conversation-main".to_string(),
            title: "主会话".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: String::new(),
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
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: Vec::new(),
            current_todos: vec![ConversationTodoItem {
                content: "旧步骤".to_string(),
                status: "in_progress".to_string(),
            }],
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        });
        state_write_app_data_cached(&state, &data).expect("write app data");

        update_conversation_todos_and_emit(
            &state,
            "conversation-main",
            vec![
                ConversationTodoItem {
                    content: "第一步".to_string(),
                    status: "completed".to_string(),
                },
                ConversationTodoItem {
                    content: "第二步".to_string(),
                    status: "completed".to_string(),
                },
            ],
        )
        .expect("update completed conversation todos");

        let data = state_read_app_data_cached(&state).expect("read app data");
        let conversation = data
            .conversations
            .iter()
            .find(|item| item.id == "conversation-main")
            .expect("conversation exists");
        assert!(conversation.current_todos.is_empty());
        assert_eq!(conversation_current_todo_text(conversation), None);
    }

    #[test]
    fn runtime_context_request_id_or_new_should_prefer_runtime_context() {
        let runtime_context = RuntimeContext {
            request_id: Some("request-from-context".to_string()),
            ..RuntimeContext::default()
        };

        let request_id = runtime_context_request_id_or_new(
            Some(&runtime_context),
            Some("trace-from-input"),
            "chat",
        );

        assert_eq!(request_id, "request-from-context");
    }

    #[test]
    fn runtime_context_new_should_seed_event_source_and_dispatch_reason() {
        let runtime_context = runtime_context_new("task_trigger", "task_due");

        assert_eq!(runtime_context.event_source.as_deref(), Some("task_trigger"));
        assert_eq!(runtime_context.dispatch_reason.as_deref(), Some("task_due"));
    }

    #[test]
    fn resolve_unarchived_conversation_index_with_fallback_should_use_requested_conversation_when_available() {
        let mut data = test_user_switched_to_sub_conversation_data();
        let idx = resolve_unarchived_conversation_index_with_fallback(
            &mut data,
            &AppConfig::default(),
            DEFAULT_AGENT_ID,
            Some("conversation-main"),
        )
        .expect("resolve requested conversation");

        assert_eq!(data.conversations[idx].id, "conversation-main");
    }

    #[test]
    fn resolve_unarchived_conversation_index_with_fallback_should_error_when_requested_missing() {
        let mut data = test_user_switched_to_sub_conversation_data();
        let err = resolve_unarchived_conversation_index_with_fallback(
            &mut data,
            &AppConfig::default(),
            DEFAULT_AGENT_ID,
            Some("conversation-missing"),
        )
        .expect_err("missing requested conversation should fail");

        assert!(err.contains("Requested conversation not found"));
    }

    #[test]
    fn delete_main_conversation_should_promote_existing_sub_conversation() {
        let state = test_chat_runtime_state();
        let config = AppConfig::default();
        write_config(&state.config_path, &config).expect("write config");
        let selected_api = resolve_selected_api_config(&config, None)
            .expect("selected api")
            .clone();

        let now = now_iso();
        let later = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format later");
        let source = test_chat_conversation("conversation-main", "active", &now);
        let mut data = AppData::default();
        data.main_conversation_id = Some(source.id.clone());
        data.conversations = vec![
            source.clone(),
            test_chat_conversation("conversation-sub", "inactive", &later),
        ];
        state_write_app_data_cached(&state, &data).expect("write app data");

        let next_id = delete_main_conversation_and_activate_latest(&state, &selected_api, &source)
            .expect("delete main conversation");
        let updated = state_read_app_data_cached(&state).expect("read app data");

        assert_ne!(next_id, "conversation-main");
        assert_ne!(next_id, "conversation-sub");
        assert_eq!(updated.main_conversation_id.as_deref(), Some(next_id.as_str()));
        assert_eq!(updated.conversations.len(), 2);
        assert!(updated.conversations.iter().any(|item| item.id == next_id && item.status == "active"));
        assert!(!updated.conversations.iter().any(|item| item.id == "conversation-main" && item.summary.is_empty()));
    }

    #[test]
    fn delete_last_main_conversation_should_create_replacement_main_conversation() {
        let state = test_chat_runtime_state();
        let config = AppConfig::default();
        write_config(&state.config_path, &config).expect("write config");
        let selected_api = resolve_selected_api_config(&config, None)
            .expect("selected api")
            .clone();

        let now = now_iso();
        let source = test_chat_conversation("conversation-main", "active", &now);
        let mut data = AppData::default();
        data.main_conversation_id = Some(source.id.clone());
        data.conversations = vec![source.clone()];
        state_write_app_data_cached(&state, &data).expect("write app data");

        let next_id = delete_main_conversation_and_activate_latest(&state, &selected_api, &source)
            .expect("delete last main conversation");
        let updated = state_read_app_data_cached(&state).expect("read app data");

        assert_ne!(next_id, "conversation-main");
        assert_eq!(updated.main_conversation_id.as_deref(), Some(next_id.as_str()));
        assert_eq!(updated.conversations.len(), 1);
        assert_eq!(updated.conversations[0].id, next_id);
        assert_eq!(updated.conversations[0].status, "active");
        assert!(updated.conversations[0].summary.is_empty());
    }

    #[test]
    fn archiving_main_conversation_should_promote_existing_sub_conversation() {
        let now = now_iso();
        let later = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format later");
        let mut data = AppData::default();
        data.main_conversation_id = Some("conversation-main".to_string());
        data.conversations = vec![
            test_chat_conversation("conversation-main", "active", &now),
            test_chat_conversation("conversation-sub", "inactive", &later),
        ];

        archive_conversation_now(&mut data, "conversation-main", "test", "archived summary")
            .expect("archive current main");
        let idx = ensure_main_conversation_index(&mut data, "", DEFAULT_AGENT_ID);

        assert_ne!(data.conversations[idx].id, "conversation-main");
        assert_ne!(data.conversations[idx].id, "conversation-sub");
        assert_eq!(
            data.main_conversation_id.as_deref(),
            Some(data.conversations[idx].id.as_str())
        );
    }

    #[test]
    fn archiving_last_main_conversation_should_create_replacement_main_conversation() {
        let now = now_iso();
        let mut data = AppData::default();
        data.main_conversation_id = Some("conversation-main".to_string());
        data.conversations = vec![test_chat_conversation("conversation-main", "active", &now)];

        archive_conversation_now(&mut data, "conversation-main", "test", "archived summary")
            .expect("archive last main");
        let idx = ensure_main_conversation_index(&mut data, "api-default", DEFAULT_AGENT_ID);

        assert_ne!(data.conversations[idx].id, "conversation-main");
        assert_eq!(
            data.main_conversation_id.as_deref(),
            Some(data.conversations[idx].id.as_str())
        );
        assert_eq!(data.conversations[idx].status, "active");
        assert!(data.conversations[idx].summary.is_empty());
    }

    #[test]
    #[ignore = "压测探针：本地按需运行 cargo test prepared_prompt_to_messages_json_large_context_probe -- --ignored --nocapture"]
    fn prepared_prompt_to_messages_json_large_context_probe() {
        let large_text = "上下文片段。".repeat(220_000);
        let prepared = PreparedPrompt {
            preamble: "系统提示词".to_string(),
            history_messages: vec![PreparedHistoryMessage {
                role: "user".to_string(),
                text: large_text.clone(),
                extra_text_blocks: Vec::new(),
                user_time_text: None,
                images: Vec::new(),
                audios: Vec::new(),
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            latest_user_text: large_text,
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let started = std::time::Instant::now();
        let messages = prepared_prompt_to_messages_json(&prepared);
        let json_bytes = serde_json::to_vec(&messages).expect("serialize large prepared messages");
        let elapsed_ms = started.elapsed().as_millis();

        eprintln!(
            "[压测] prepared_prompt_to_messages_json 大上下文结果：messages={}，bytes={}，elapsed={}ms",
            messages.len(),
            json_bytes.len(),
            elapsed_ms
        );

        assert!(json_bytes.len() > 1_500_000);
    }

    #[test]
    #[ignore = "压测探针：本地按需运行 cargo test llm_round_log_large_response_probe -- --ignored --nocapture"]
    fn llm_round_log_large_response_probe() {
        let state = test_chat_runtime_state();
        let large_response = "响应片段。".repeat(220_000);
        let response = serde_json::json!({
            "assistantText": large_response,
            "reasoningStandard": "",
            "reasoningInline": "",
            "toolHistoryEvents": []
        });

        let started = std::time::Instant::now();
        push_llm_round_log(
            Some(&state),
            Some("trace-large-response".to_string()),
            None,
            "Archive summary",
            RequestFormat::OpenAI,
            "archive-summary",
            "deepseek-chat",
            "http://localhost:5001/v1",
            masked_auth_headers("sk-test"),
            None,
            Some(response),
            None,
            1234,
            None,
        );
        let elapsed_ms = started.elapsed().as_millis();
        let logs = state.llm_round_logs.lock().expect("llm round logs");
        let stored = logs.back().expect("stored log entry");
        let response_bytes = serde_json::to_vec(&stored.response).expect("serialize stored response");

        eprintln!(
            "[压测] llm_round_log_large_response 大响应结果：stored_logs={}，response_bytes={}，elapsed={}ms",
            logs.len(),
            response_bytes.len(),
            elapsed_ms
        );

        assert_eq!(logs.len(), 1);
        assert!(response_bytes.len() > 1_500_000);
    }

    #[test]
    #[ignore = "性能探针：本地按需运行 cargo test build_prepared_prompt_for_mode_perf_probe -- --ignored --nocapture"]
    fn build_prepared_prompt_for_mode_perf_probe() {
        let state = test_chat_runtime_state();
        let agent = default_agent();
        let user = default_user_persona();
        let drafts = (0..12)
            .map(|idx| MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: format!("用户偏好样本{}", idx),
                reasoning: format!("这是第{}条用于提示词性能探针的记忆。", idx),
                tags: vec!["性能".to_string(), format!("tag{}", idx)],
                owner_agent_id: None,
            })
            .collect::<Vec<_>>();
        let (saved, _) =
            memory_store_upsert_drafts(&state.data_path, &drafts).expect("seed perf probe memories");
        let memory_ids = saved
            .iter()
            .filter_map(|item| item.id.clone())
            .collect::<Vec<_>>();

        let base_time = now_utc();
        let mut messages = Vec::<ChatMessage>::new();
        for idx in 0..80 {
            let created_at = (base_time + time::Duration::seconds(idx as i64))
                .format(&Rfc3339)
                .expect("format probe message time");
            let is_user = idx % 2 == 0;
            let role = if is_user { "user" } else { "assistant" };
            let speaker_agent_id = if is_user {
                Some(USER_PERSONA_ID.to_string())
            } else {
                Some(agent.id.clone())
            };
            let mut provider_meta = None;
            let mut extra_text_blocks = Vec::<String>::new();
            if is_user && idx >= 60 {
                let picked = memory_ids
                    .iter()
                    .skip((idx / 2) % 4)
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>();
                provider_meta = Some(serde_json::json!({
                    "retrieved_memory_ids": picked
                }));
                extra_text_blocks.push(format!("补充上下文块{}", idx));
            }
            messages.push(ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: role.to_string(),
                created_at,
                speaker_agent_id,
                parts: vec![MessagePart::Text {
                    text: format!("这是第{}条{}消息，用于测量提示词主结构构建速度。", idx, role),
                }],
                extra_text_blocks,
                provider_meta,
                tool_call: None,
                mcp_call: None,
            });
        }

        let last_user_at = messages
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .map(|message| message.created_at.clone());
        let conversation = test_active_conversation_with_messages(messages, last_user_at);
        let overrides = ChatPromptOverrides {
            latest_user_intent: Some(LatestUserPayloadIntent::Explicit {
                text: String::new(),
                meta_text: String::new(),
                extra_blocks: vec![
                    "这是一个额外的任务板块。".to_string(),
                    "这是一个额外的前台工具提示块。".to_string(),
                ],
            }),
            ..Default::default()
        };

        let runs = 20u32;
        let started = std::time::Instant::now();
        let mut latest_extra_len = 0usize;
        let mut history_len = 0usize;
        for _ in 0..runs {
            let prepared = build_prepared_prompt_for_mode(
                PromptBuildMode::Chat,
                &conversation,
                &agent,
                &[agent.clone(), user.clone()],
                &[],
                "用户",
                "我是性能探针里的用户。",
                DEFAULT_RESPONSE_STYLE_ID,
                "zh-CN",
                Some(&state.data_path),
                None,
                None,
                Some(overrides.clone()),
                Some(&state),
                Some(&ApiConfig::default()),
                None,
                Some(false),
            );
            latest_extra_len = prepared.latest_user_extra_text.len();
            history_len = prepared.history_messages.len();
            assert!(!prepared.preamble.trim().is_empty());
        }
        let total_ms = started.elapsed().as_millis() as u64;
        let avg_ms = total_ms / u64::from(runs);
        eprintln!(
            "[提示词性能探针] build_prepared_prompt_for_mode 平均耗时={}ms, total={}ms, runs={}, history_len={}, latest_extra_len={}",
            avg_ms,
            total_ms,
            runs,
            history_len,
            latest_extra_len
        );
    }

    #[test]
    fn normalize_image_for_chat_upload_should_resize_large_png_and_encode_webp() {
        let source = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            4000,
            2000,
            image::Rgb([12, 34, 56]),
        ));
        let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
        source
            .write_to(&mut cursor, image::ImageFormat::Png)
            .expect("encode png");

        let normalized =
            normalize_image_for_chat_upload(&cursor.into_inner()).expect("normalize image");

        assert_eq!(
            image::guess_format(&normalized).expect("guess format"),
            image::ImageFormat::WebP
        );

        let decoded = image::load_from_memory(&normalized).expect("decode webp");
        assert_eq!(decoded.width(), CHAT_UPLOAD_IMAGE_MAX_EDGE);
        assert_eq!(decoded.height(), 640);
    }
