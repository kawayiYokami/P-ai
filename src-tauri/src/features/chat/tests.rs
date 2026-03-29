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
                .contains("凯瑟琳")
        );
        assert_eq!(prepared.history_messages[1].role, "assistant");
        assert!(prepared.latest_user_meta_text.contains("凯瑟琳"));
        assert!(prepared.latest_user_text.contains("现在补发第二次提醒"));
    }

    #[test]
    fn build_prompt_user_meta_text_should_not_append_memory_injected_tag() {
        let now = now_iso();
        let mut message = test_text_message("user", "继续", &now);
        message.extra_text_blocks.push(
            "<system-reminder>\n[MemoryBoard]\n\n以下为相关记忆，仅作背景参考，并非用户当前发言。请勿直接针对记忆内容作答，仅在确有帮助时自然使用。\n\n用户询问 codex 是什么\n> 无\n</system-reminder>"
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
    fn build_prompt_user_meta_text_should_skip_compaction_message_metadata() {
        let now = now_iso();
        let mut message = test_text_message(
            "user",
            "[上下文整理]\n触发原因：manual\n整理摘要：\n用户刚刚确认继续推进。",
            &now,
        );
        message.provider_meta = Some(serde_json::json!({
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
    fn prepared_prompt_to_messages_json_should_omit_empty_latest_user_turn() {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "user".to_string(),
                    text: "现在时间是多少？".to_string(),
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
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let messages = prepared_prompt_to_messages_json(&prepared);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].get("role").and_then(Value::as_str), Some("user"));
        assert_eq!(messages[2].get("role").and_then(Value::as_str), Some("assistant"));
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
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "[上下文整理]\n触发原因：force_context_usage_82_after_reply\n整理摘要：\n保留关键上下文。".to_string(),
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
    }

    #[test]
    fn build_archive_history_messages_should_expand_tool_history_events_instead_of_nesting_them() {
        let now = now_iso();
        let mut assistant = test_text_message("assistant", "我查好了", &now);
        assistant.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "remote_im_send",
                        "arguments": "{\"action\":\"send\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"ok\":true}"
            }),
        ]);
        let conv = test_active_conversation_with_messages(vec![assistant], Some(now));

        let history = build_archive_history_messages(&conv);

        assert_eq!(history.len(), 3);
        assert_eq!(history[0].role, "assistant");
        assert!(history[0].tool_calls.is_some());
        assert_eq!(
            history[0]
                .tool_calls
                .as_ref()
                .and_then(|calls| calls.first())
                .and_then(|call| call.get("id"))
                .and_then(Value::as_str),
            Some("call_1")
        );
        assert_eq!(
            history[0]
                .tool_calls
                .as_ref()
                .and_then(|calls| calls.first())
                .and_then(|call| call.get("type"))
                .and_then(Value::as_str),
            Some("function")
        );
        assert_eq!(history[1].role, "tool");
        assert_eq!(history[1].tool_call_id.as_deref(), Some("call_1"));
        assert_eq!(history[2].role, "assistant");
        assert!(history[2].tool_calls.is_none());
        assert_eq!(history[2].text, "我查好了");
    }

    #[test]
    fn build_remote_im_activation_runtime_block_should_require_explicit_send_for_multiple_sources() {
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
        assert!(block.contains("必须显式调用 `remote_im_send`"));
        assert!(block.contains("channel_id=remote-im-a"));
        assert!(block.contains("channel_id=remote-im-b"));
    }

    #[test]
    fn resolve_remote_im_auto_send_target_should_only_allow_single_source() {
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
            false,
        )
        .expect("single target");
        assert_eq!(single_target, Some(single_source.clone()));

        let err = resolve_remote_im_auto_send_target(
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
            false,
        )
        .expect_err("multiple sources should require explicit tool");
        assert!(err.contains("多个远程IM来源"));

        let explicit_decision = resolve_remote_im_auto_send_target(
            "你好，这里是最终回复。",
            &[RemoteImActivationSource {
                channel_id: "remote-im-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                remote_contact_type: "private".to_string(),
                remote_contact_id: "contact-a".to_string(),
                remote_contact_name: "张三".to_string(),
            }],
            true,
        )
        .expect("explicit decision bypass");
        assert!(explicit_decision.is_none());
    }

    #[test]
    fn runtime_remote_im_activation_sources_should_force_send_tool() {
        let state = test_chat_runtime_state();
        set_conversation_remote_im_activation_sources(
            &state,
            "conversation-a",
            vec![RemoteImActivationSource {
                channel_id: "remote-im-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                remote_contact_type: "private".to_string(),
                remote_contact_id: "contact-a".to_string(),
                remote_contact_name: "张三".to_string(),
            }],
        )
        .expect("set activation sources");

        assert!(remote_im_activation_runtime_forces_send_tool(
            Some(&state),
            "chat::conversation-a"
        ));
        assert!(!remote_im_activation_runtime_forces_send_tool(
            Some(&state),
            "chat::conversation-b"
        ));
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

        let outcome = test_runtime()
            .block_on(remote_im_auto_send_and_record_decision(
                &state,
                &activation_source,
                &conversation_id,
                &assistant_text,
                Some(&assistant_message_id),
            ))
            .expect("auto send should succeed");

        assert_eq!(
            outcome,
            RemoteImAutoSendExecutionOutcome::Sent {
                action: "send".to_string()
            }
        );

        let decision =
            read_remote_im_decision_for_message(&state, &conversation_id, &assistant_message_id);
        assert_eq!(decision.get("action").and_then(Value::as_str), Some("send"));
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

        let err = test_runtime()
            .block_on(remote_im_auto_send_and_record_decision(
                &state,
                &activation_source,
                &conversation_id,
                &assistant_text,
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
        let huge = "中".repeat(2000);
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", &huge, &now)],
            Some(now),
        );
        let d = decide_archive_before_user_message(&conv, 1000);
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
        let text = "中".repeat(600);
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", &text, &old)],
            Some(old),
        );
        let d = decide_archive_before_user_message(&conv, 1000);
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
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", "hello", &old)],
            Some(old),
        );
        let d = decide_archive_before_user_message(&conv, 1000);
        assert!(!d.should_archive);
        assert!(!d.forced);
        assert!(d.usage_ratio < 0.30);
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
            terminal_shell: detect_default_terminal_shell(),
            terminal_shell_candidates: detect_terminal_shell_candidates(),
            state_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_mtime: Arc::new(Mutex::new(None)),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
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
                std::collections::HashSet::new(),
            )),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
        }
    }

    fn test_pending_event(conversation_id: &str) -> ChatPendingEvent {
        let created_at = now_iso();
        ChatPendingEvent {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_string(),
            created_at: created_at.clone(),
            source: ChatEventSource::User,
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
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
            status: status.to_string(),
            summary: String::new(),
            archived_at: None,
            messages: Vec::new(),
            memory_recall_table: Vec::new(),
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
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_department_id: Some(FRONT_DESK_DEPARTMENT_ID.to_string()),
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
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            last_user_at: None,
            last_assistant_at: Some(now.clone()),
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
            status: "active".to_string(),
            summary: String::new(),
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
            memory_recall_table: Vec::new(),
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
    fn delegate_parse_session_parts_should_preserve_conversation_in_two_segment_session() {
        let (api_config_id, agent_id, conversation_id) =
            delegate_parse_session_parts("default-agent::conversation-sub");

        assert_eq!(api_config_id, "");
        assert_eq!(agent_id, "default-agent");
        assert_eq!(conversation_id.as_deref(), Some("conversation-sub"));
    }

    #[test]
    fn delegate_parse_session_parts_should_preserve_conversation_in_three_segment_session() {
        let (api_config_id, agent_id, conversation_id) =
            delegate_parse_session_parts("api-config-a::default-agent::conversation-sub");

        assert_eq!(api_config_id, "api-config-a");
        assert_eq!(agent_id, "default-agent");
        assert_eq!(conversation_id.as_deref(), Some("conversation-sub"));
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

        assert_eq!(next_id, "conversation-sub");
        assert_eq!(updated.main_conversation_id.as_deref(), Some("conversation-sub"));
        assert_eq!(updated.conversations.len(), 1);
        assert_eq!(updated.conversations[0].id, "conversation-sub");
        assert_eq!(updated.conversations[0].status, "active");
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

        assert_eq!(data.conversations[idx].id, "conversation-sub");
        assert_eq!(data.main_conversation_id.as_deref(), Some("conversation-sub"));
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
