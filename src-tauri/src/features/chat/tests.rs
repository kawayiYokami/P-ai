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
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
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
    fn build_prompt_should_replay_final_parts_reasoning_after_tool_history() {
        let now = now_iso();
        let agent = default_agent();
        let mut assistant = test_text_message("assistant", "最终回答", &now);
        assistant.speaker_agent_id = Some(agent.id.clone());
        assistant.tool_call = Some(vec![serde_json::json!({
            "role": "assistant",
            "content": null,
            "reasoning_content": "工具思考",
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "read",
                    "arguments": "{}"
                }
            }]
        }), serde_json::json!({
            "role": "tool",
            "tool_call_id": "call_1",
            "content": "工具结果"
        })]);
        if let Some(MessagePart::Text {
            reasoning_content,
            ..
        }) = assistant.parts.first_mut()
        {
            *reasoning_content = Some("最终思考".to_string());
        }

        let messages = vec![
            test_text_message("user", "上一轮问题", &now),
            assistant,
            test_text_message("user", "继续", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
        );

        let final_assistant_messages = prepared
            .history_messages
            .iter()
            .filter(|message| message.role == "assistant" && message.text == "最终回答")
            .collect::<Vec<_>>();

        assert_eq!(final_assistant_messages.len(), 1);
        assert_eq!(
            final_assistant_messages[0].reasoning_content.as_deref(),
            Some("最终思考")
        );
        assert!(prepared.history_messages.iter().any(|message| {
            message.role == "assistant"
                && message.tool_calls.as_ref().is_some_and(|calls| !calls.is_empty())
                && message.reasoning_content.as_deref() == Some("工具思考")
        }));
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
            &[agent.clone()],
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
            &[agent.clone()],
            "zh-CN",
            None,
            &fixed_system_prompt,
            None,
            None,
            &[],
            None,
        );

        assert_eq!(first.revisions, second.revisions);
        assert_eq!(first.core_prompt, second.core_prompt);
        assert_eq!(first.environment_prompt, second.environment_prompt);
        assert_eq!(first.abstract_messages, second.abstract_messages);
    }

    #[test]
    fn conversation_prompt_service_should_omit_goal_rule_for_remote_group_origin() {
        let now = now_iso();
        let agent = default_agent();
        let conversation = test_active_conversation_with_messages(
            vec![test_text_message("user", "群聊消息", &now)],
            Some(now),
        );
        let group_source = RemoteImActivationSource {
            channel_id: "channel-group".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "group".to_string(),
            remote_contact_id: "group-1".to_string(),
            remote_contact_name: "测试群".to_string(),
        };

        let group_snapshot = conversation_prompt_service().build_prompt_snapshot(
            None,
            "chat",
            &conversation,
            &agent,
            &[agent.clone()],
            "zh-CN",
            None,
            "固定系统提示词",
            None,
            None,
            &[],
            Some(&ChatPromptOverrides {
                remote_im_activation_sources: vec![group_source],
                ..Default::default()
            }),
        );
        let local_snapshot = conversation_prompt_service().build_prompt_snapshot(
            None,
            "chat",
            &conversation,
            &agent,
            &[agent.clone()],
            "zh-CN",
            None,
            "固定系统提示词",
            None,
            None,
            &[],
            None,
        );

        assert!(!group_snapshot.core_prompt.contains("<goal tool rule>"));
        assert!(local_snapshot.core_prompt.contains("<goal tool rule>"));
    }

    #[test]
    fn conversation_prompt_service_should_align_task_and_plan_rules_with_conversation_scope() {
        let now = now_iso();
        let agent = default_agent();
        let local = test_active_conversation_with_messages(
            vec![test_text_message("user", "本地消息", &now)],
            Some(now.clone()),
        );
        let mut delegate = local.clone();
        delegate.conversation_kind = CONVERSATION_KIND_DELEGATE.to_string();
        let mut remote_contact = local.clone();
        remote_contact.conversation_kind = CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string();
        remote_contact.root_conversation_id = Some(
            "remote_im_contact:channel-group:group:group-1".to_string(),
        );
        let overrides = ChatPromptOverrides::default();

        let local_snapshot = conversation_prompt_service().build_prompt_snapshot(
            None,
            "chat",
            &local,
            &agent,
            &[agent.clone()],
            "zh-CN",
            None,
            "固定系统提示词",
            None,
            None,
            &[],
            Some(&overrides),
        );
        let delegate_snapshot = conversation_prompt_service().build_prompt_snapshot(
            None,
            "delegate",
            &delegate,
            &agent,
            &[agent.clone()],
            "zh-CN",
            None,
            "固定系统提示词",
            None,
            None,
            &[],
            Some(&overrides),
        );
        let remote_contact_snapshot = conversation_prompt_service().build_prompt_snapshot(
            None,
            "chat",
            &remote_contact,
            &agent,
            &[agent.clone()],
            "zh-CN",
            None,
            "固定系统提示词",
            None,
            None,
            &[],
            Some(&overrides),
        );

        assert!(local_snapshot.core_prompt.contains("<task tool rule>"));
        assert!(local_snapshot.core_prompt.contains("<plan tool rule>"));
        assert!(!delegate_snapshot.core_prompt.contains("<task tool rule>"));
        assert!(!delegate_snapshot.core_prompt.contains("<plan tool rule>"));
        assert!(remote_contact_snapshot
            .core_prompt
            .contains("<task tool rule>"));
        assert!(!remote_contact_snapshot
            .core_prompt
            .contains("<plan tool rule>"));
    }

    #[test]
    fn build_core_system_prompt_text_should_append_user_profile_snapshot_into_user_settings() {
        let now = now_iso();
        let agent = default_agent();
        let mut conv = test_active_conversation_with_messages(
            vec![test_text_message("user", "测试画像快照", &now)],
            Some(now),
        );
        conv.user_profile_snapshot =
            "[id:profile-1]\n偏好早晨回复\n> 近期交流稳定提到晨间安排"
                .to_string();

        let fixed_system_prompt = build_core_system_prompt_text(
            &conv,
            &agent,
            Some(("用户", "我是测试用户")),
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
        );

        assert!(fixed_system_prompt.contains("<admin user settings>"));
        assert!(fixed_system_prompt.contains("用户画像快照："));
        assert!(fixed_system_prompt.contains("[id:profile-1]"));
        assert!(fixed_system_prompt.contains("偏好早晨回复"));
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
            &[agent.clone()],
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
            &[agent.clone()],
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
    fn build_core_system_prompt_text_should_skip_conversation_style_block_when_none() {
        let now = now_iso();
        let agent = default_agent();
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", "测试无风格", &now)],
            Some(now),
        );

        let fixed_system_prompt = build_core_system_prompt_text(
            &conv,
            &agent,
            Some(("用户", "我是测试用户")),
            "none",
            "zh-CN",
            None,
        );

        assert!(!fixed_system_prompt.contains("<conversation style>"));
        assert!(!fixed_system_prompt.contains("当前风格："));
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
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
            },
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(agent.id.clone()),
                parts: vec![MessagePart::Text {
                    text: "我马上处理".to_string(),
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
            },
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(system_persona.id.clone()),
                parts: vec![MessagePart::Text {
                    text: "现在补发第二次提醒".to_string(),
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
            },
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona(), system_persona.clone()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
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
            "[id:m1]\n用户询问 codex 是什么\n> 无"
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
    fn build_prompt_user_meta_text_should_include_local_user_id() {
        let now = now_iso();
        let mut message = test_text_message("user", "继续", &now);
        message.speaker_agent_id = Some(USER_PERSONA_ID.to_string());

        let meta = build_prompt_user_meta_text(
            &message,
            &[default_agent(), default_user_persona()],
            "用户",
            "zh-CN",
            false,
        )
        .expect("meta text");

        assert!(meta.contains("user_id=user-persona"));
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
                "sender_id": "member-7",
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

        assert!(meta.contains("[张三/member-7]"));
        assert!(!meta.contains("测试群"));
        assert!(!meta.contains("channel_id=remote-im-1"));
        assert!(!meta.contains("contact_id=group-42"));
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
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "retrieved_memory_ids": [cat_memory_id]
                })),
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
            },
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(agent.id.clone()),
                parts: vec![MessagePart::Text {
                    text: "吐毛球可以先观察饮食和梳毛频率。".to_string(),
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
            },
            ChatMessage {
                id: Uuid::new_v4().to_string(),
                role: "user".to_string(),
                created_at: now.clone(),
                speaker_agent_id: Some(USER_PERSONA_ID.to_string()),
                parts: vec![MessagePart::Text {
                    text: "我想吃花生酱面包，可以吗？".to_string(),
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "retrieved_memory_ids": [saved[0].id.clone().expect("dup cat id"), peanut_memory_id.clone(), peanut_memory_id]
                })),
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
            },
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            Some(&state.data_path),
            None,
            None,
        );

        let history_extra = prepared.history_messages[0].extra_text_blocks.join("\n");
        assert_eq!(history_extra.matches("用户很喜欢猫咪").count(), 1);
        assert!(!history_extra.contains("因为用户妈妈从小养猫"));
        assert!(!history_extra.contains("用户对花生过敏"));
        assert!(prepared.latest_user_extra_text.contains("用户对花生过敏"));
        assert_eq!(prepared.latest_user_extra_text.matches("用户很喜欢猫咪").count(), 0);
        assert_eq!(prepared.latest_user_extra_text.matches("用户对花生过敏").count(), 1);
    }

    #[test]
    fn manual_memory_recall_mode_should_skip_rag_but_keep_builtin_recall() {
        let state = test_chat_runtime_state();
        let mut agent = default_agent();
        agent.memory_recall_mode = MEMORY_RECALL_MODE_MANUAL.to_string();
        state_write_agents_cached(&state, &[agent.clone(), default_user_persona()])
            .expect("seed agents");
        memory_store_upsert_drafts(
            &state.data_path,
            &[MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "用户喜欢猫咪".to_string(),
                reasoning: "回归测试".to_string(),
                tags: vec!["猫咪".to_string()],
                owner_agent_id: None,
            }],
        )
        .expect("seed memory");
        let user_message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: Some(USER_PERSONA_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "猫咪".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        };

        let recall_payload = collect_recall_payload_for_user_message(
            &state.data_path,
            &[agent.clone()],
            &agent.id,
            &user_message,
        )
        .expect("collect recall payload");
        assert!(recall_payload.stored_ids.is_empty());
        assert!(recall_payload.raw_ids.is_empty());

        let memory_context = memory_agent_context_from_agent(&agent)
            .expect("manual memory context");
        let recall_result = builtin_recall(&state, &memory_context, "猫咪", None, None, None)
            .expect("manual recall");
        let memory_board = recall_result
            .get("memoryBoard")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert_eq!(recall_result.get("count").and_then(Value::as_u64), Some(1));
        assert!(memory_board.contains("用户喜欢猫咪"));
    }

    #[test]
    fn off_memory_recall_mode_should_skip_rag_and_builtin_recall_results() {
        let state = test_chat_runtime_state();
        let mut agent = default_agent();
        agent.memory_recall_mode = MEMORY_RECALL_MODE_OFF.to_string();
        state_write_agents_cached(&state, &[agent.clone(), default_user_persona()])
            .expect("seed agents");
        memory_store_upsert_drafts(
            &state.data_path,
            &[MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "用户喜欢猫咪".to_string(),
                reasoning: "回归测试".to_string(),
                tags: vec!["猫咪".to_string()],
                owner_agent_id: None,
            }],
        )
        .expect("seed memory");
        let user_message = ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: Some(USER_PERSONA_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "猫咪".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        };

        let recall_payload = collect_recall_payload_for_user_message(
            &state.data_path,
            &[agent.clone()],
            &agent.id,
            &user_message,
        )
        .expect("collect recall payload");
        assert!(recall_payload.stored_ids.is_empty());
        assert!(recall_payload.raw_ids.is_empty());

        let memory_context = memory_agent_context_from_agent(&agent)
            .expect("off memory context");
        let recall_result = builtin_recall(&state, &memory_context, "猫咪", None, None, None)
            .expect("off recall");
        assert_eq!(recall_result.get("count").and_then(Value::as_u64), Some(0));
        assert_eq!(recall_result.get("total").and_then(Value::as_u64), Some(0));
        assert_eq!(
            recall_result.get("memoryBoard").and_then(Value::as_str),
            Some("")
        );
    }

    #[test]
    fn builtin_memory_save_should_use_current_tool_agent_as_owner() {
        let state = test_chat_runtime_state();
        let mut assistant = default_agent();
        assistant.private_memory_enabled = true;
        let mut worker = default_agent();
        worker.id = "worker-agent".to_string();
        worker.name = "执行者".to_string();
        worker.private_memory_enabled = true;
        let worker_memory_context =
            memory_agent_context_from_agent(&worker).expect("worker memory context");
        state_write_agents_cached(
            &state,
            &[assistant.clone(), worker.clone(), default_user_persona()],
        )
        .expect("seed agents");

        let result = builtin_memory_save(
            &state,
            &worker_memory_context,
            serde_json::json!({
                "action": "create",
                "memory": {
                    "memoryType": "knowledge",
                    "judgment": "当前任务由执行者人格负责",
                    "reasoning": "回归测试",
                    "tags": ["执行者", "回归"]
                }
            }),
        )
        .expect("save memory");

        assert_eq!(result.get("saved").and_then(Value::as_bool), Some(true));
        let memories = memory_store_list_memories(&state.data_path).expect("list memories");
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].owner_agent_id.as_deref(), Some(worker.id.as_str()));
        assert_eq!(memories[0].judgment, "当前任务由执行者人格负责");
    }

    #[test]
    fn builtin_recall_should_read_memories_for_current_tool_agent_only() {
        let state = test_chat_runtime_state();
        let mut assistant = default_agent();
        assistant.private_memory_enabled = true;
        let mut worker = default_agent();
        worker.id = "worker-agent".to_string();
        worker.name = "执行者".to_string();
        worker.private_memory_enabled = true;
        let worker_memory_context =
            memory_agent_context_from_agent(&worker).expect("worker memory context");
        state_write_agents_cached(
            &state,
            &[assistant.clone(), worker.clone(), default_user_persona()],
        )
        .expect("seed agents");
        memory_store_upsert_drafts(
            &state.data_path,
            &[
                MemoryDraftInput {
                    memory_type: "knowledge".to_string(),
                    judgment: "这是主助理的私有记忆".to_string(),
                    reasoning: "回归测试".to_string(),
                    tags: vec!["共享线索".to_string()],
                    owner_agent_id: Some(assistant.id.clone()),
                },
                MemoryDraftInput {
                    memory_type: "knowledge".to_string(),
                    judgment: "这是执行者的私有记忆".to_string(),
                    reasoning: "回归测试".to_string(),
                    tags: vec!["共享线索".to_string()],
                    owner_agent_id: Some(worker.id.clone()),
                },
            ],
        )
        .expect("seed memories");

        let result = builtin_recall(&state, &worker_memory_context, "共享线索", None, None, None)
            .expect("recall current agent memories");
        let memory_board = result
            .get("memoryBoard")
            .and_then(Value::as_str)
            .unwrap_or_default();

        assert_eq!(result.get("count").and_then(Value::as_u64), Some(1));
        assert!(memory_board.contains("这是执行者的私有记忆"));
        assert!(!memory_board.contains("这是主助理的私有记忆"));
    }

    #[test]
    fn builtin_memory_tools_should_accept_deputy_agent_as_current_persona() {
        let state = test_chat_runtime_state();
        let assistant = default_agent();
        let deputy = default_deputy_agent();
        let deputy_memory_context =
            memory_agent_context_from_agent(&deputy).expect("deputy memory context");
        state_write_agents_cached(
            &state,
            &[assistant, deputy.clone(), default_user_persona(), default_system_persona()],
        )
        .expect("seed agents");

        let save_result = builtin_memory_save(
            &state,
            &deputy_memory_context,
            serde_json::json!({
                "action": "create",
                "memory": {
                    "memoryType": "knowledge",
                    "judgment": "这是副手人格记录的共享记忆",
                    "reasoning": "回归测试",
                    "tags": ["副手回归", "共享"]
                }
            }),
        )
        .expect("save deputy memory");

        assert_eq!(save_result.get("saved").and_then(Value::as_bool), Some(true));

        let recall_result = builtin_recall(&state, &deputy_memory_context, "副手回归", None, None, None)
            .expect("recall deputy memory");
        let memory_board = recall_result
            .get("memoryBoard")
            .and_then(Value::as_str)
            .unwrap_or_default();

        assert_eq!(recall_result.get("count").and_then(Value::as_u64), Some(1));
        assert!(memory_board.contains("这是副手人格记录的共享记忆"));
    }

    #[test]
    fn builtin_memory_tools_should_accept_private_workspace_runtime_agent() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let private_worker = AgentProfile {
            id: "private-worker".to_string(),
            name: "私人执行者".to_string(),
            system_prompt: "你是谁：你是私人执行者。\n台词技巧：直接。\n性格画像：可靠。".to_string(),
            tools: default_agent_tools(),
            created_at: now.clone(),
            updated_at: now,
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: default_private_workspace_source(),
            scope: default_assistant_private_scope(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        };
        let private_worker_memory_context =
            memory_agent_context_from_agent(&private_worker).expect("private worker memory context");

        let save_result = builtin_memory_save(
            &state,
            &private_worker_memory_context,
            serde_json::json!({
                "action": "create",
                "memory": {
                    "memoryType": "knowledge",
                    "judgment": "这是私有工作区人格记录的共享记忆",
                    "reasoning": "回归测试",
                    "tags": ["私有回归", "共享"]
                }
            }),
        )
        .expect("save private workspace memory");

        assert_eq!(save_result.get("saved").and_then(Value::as_bool), Some(true));

        let recall_result = builtin_recall(&state, &private_worker_memory_context, "私有回归", None, None, None)
            .expect("recall private workspace memory");
        let memory_board = recall_result
            .get("memoryBoard")
            .and_then(Value::as_str)
            .unwrap_or_default();

        assert_eq!(recall_result.get("count").and_then(Value::as_u64), Some(1));
        assert!(memory_board.contains("这是私有工作区人格记录的共享记忆"));
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
                    user_time_text: Some("[测试用户] 2026-03-18T12:18".to_string()),
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
                            && arr[0].get("text").and_then(Value::as_str)
                                == Some("[测试用户] 2026-03-18T12:18")
                            && arr[1].get("type").and_then(Value::as_str) == Some("text")
                            && arr[1].get("text").and_then(Value::as_str) == Some("你好")
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
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
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
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
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
    fn prepared_prompt_to_messages_json_should_keep_reasoning_for_four_tool_rounds_in_order() {
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "user".to_string(),
                    text: "继续完成任务".to_string(),
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
                    text: String::new(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "read_file", "arguments": "{\"path\":\"a.md\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: Some("第1轮思考".to_string()),
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"ok\":true,\"step\":1}".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("call_1".to_string()),
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
                        "id": "call_2",
                        "type": "function",
                        "function": { "name": "grep", "arguments": "{\"pattern\":\"quest\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: Some("第2轮思考".to_string()),
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"ok\":true,\"step\":2}".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("call_2".to_string()),
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
                        "id": "call_3",
                        "type": "function",
                        "function": { "name": "http", "arguments": "{\"url\":\"/quests\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: Some("第3轮思考".to_string()),
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"ok\":true,\"step\":3}".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("call_3".to_string()),
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
                        "id": "call_4",
                        "type": "function",
                        "function": { "name": "write_file", "arguments": "{\"path\":\"out.md\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: Some("第4轮思考".to_string()),
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"ok\":true,\"step\":4}".to_string(),
                    extra_text_blocks: Vec::new(),
                    user_time_text: None,
                    images: Vec::new(),
                    audios: Vec::new(),
                    tool_calls: None,
                    tool_call_id: Some("call_4".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "继续下一步".to_string(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let messages = prepared_prompt_to_messages_json(&prepared);
        let assistant_tool_reasonings = messages
            .iter()
            .filter(|message| {
                message.get("role").and_then(Value::as_str) == Some("assistant")
                    && message.get("tool_calls").and_then(Value::as_array).is_some()
            })
            .map(|message| {
                message
                    .get("reasoning_content")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            assistant_tool_reasonings,
            vec![
                "第1轮思考".to_string(),
                "第2轮思考".to_string(),
                "第3轮思考".to_string(),
                "第4轮思考".to_string(),
            ]
        );
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
    fn build_prompt_should_replay_clean_tool_reasoning_chain_on_new_dispatch() {
        let now = now_iso();
        let agent = default_agent();
        let mut assistant = test_text_message("assistant", "我已经完成了工具阶段", &now);
        assistant.speaker_agent_id = Some(agent.id.clone());
        assistant.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第1次请求返回的思考",
                "tool_calls": [{
                    "id": "fc_1",
                    "type": "function",
                    "function": { "name": "read_file", "arguments": "{\"path\":\"a.md\"}" }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "fc_1",
                "content": "{\"ok\":true,\"step\":1}"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第2次请求返回的思考",
                "tool_calls": [{
                    "id": "fc_2",
                    "type": "function",
                    "function": { "name": "grep", "arguments": "{\"pattern\":\"quest\"}" }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "fc_2",
                "content": "{\"ok\":true,\"step\":2}"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第3次请求返回的思考",
                "tool_calls": [{
                    "id": "fc_3",
                    "type": "function",
                    "function": { "name": "http", "arguments": "{\"url\":\"/quests\"}" }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "fc_3",
                "content": "{\"ok\":true,\"step\":3}"
            }),
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "第4次请求返回的思考",
                "tool_calls": [{
                    "id": "fc_4",
                    "type": "function",
                    "function": { "name": "write_file", "arguments": "{\"path\":\"out.md\"}" }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "fc_4",
                "content": "{\"ok\":true,\"step\":4}"
            }),
        ]);
        let messages = vec![
            test_text_message("user", "先帮我查 quest API", &now),
            assistant,
            test_text_message("user", "现在继续下一次调度", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
        );

        let tool_reasonings = prepared
            .history_messages
            .iter()
            .filter(|message| {
                message.role == "assistant"
                    && message
                        .tool_calls
                        .as_ref()
                        .map(|calls| !calls.is_empty())
                        .unwrap_or(false)
            })
            .map(|message| message.reasoning_content.clone().unwrap_or_default())
            .collect::<Vec<_>>();

        assert_eq!(
            tool_reasonings,
            vec![
                "第1次请求返回的思考".to_string(),
                "第2次请求返回的思考".to_string(),
                "第3次请求返回的思考".to_string(),
                "第4次请求返回的思考".to_string(),
            ]
        );
        assert!(tool_reasonings.iter().all(|value| value != "整轮累计思考A。整轮累计思考A。整轮累计思考A。"));
        assert!(
            prepared.history_messages.iter().any(|message| {
                message.role == "assistant"
                    && message.text == "我已经完成了工具阶段"
                    && message.reasoning_content.is_none()
            })
        );
    }

    #[test]
    fn build_prompt_should_replay_persisted_real_tool_message_into_request_preview_shape() {
        let agent = default_agent();
        let raw: Value = serde_json::from_str(
            r#"{
                "kind":"message",
                "message":{
                    "id":"926b767d-c730-4e20-bfba-e57a84ee40b9",
                    "role":"assistant",
                    "createdAt":"2026-05-08T11:44:08Z",
                    "speakerAgentId":"persona-1776445616370",
                    "parts":[
                        {
                            "type":"text",
                            "text":"这是一张你发来的截图，显示的是我们的聊天界面和刚才查询终端版本的对话内容。看起来你想让我看什么？\n\n让我再仔细看看截图内容——它展示的就是我们刚刚这段对话：你问我终端版本，我执行了 `pwsh --version`，返回了 PowerShell 7.5.4 的信息。\n\n你想确认什么？还是截错了？"
                        }
                    ],
                    "extraTextBlocks":[],
                    "toolCall":[
                        {
                            "content":null,
                            "reasoning_content":"用户想让我调用一次终端命令，查看版本信息。这应该是指操作系统的终端信息，比如 PowerShell 版本之类的。",
                            "role":"assistant",
                            "tool_calls":[
                                {
                                    "call_id":"call_915742b663134f368df81c7d",
                                    "function":{
                                        "arguments":"{\"command\":\"pwsh --version; Write-Host \\\"---\\\"; $PSVersionTable; Write-Host \\\"---\\\"; [System.Environment]::OSVersion.VersionString\"}",
                                        "name":"exec"
                                    },
                                    "id":"call_915742b663134f368df81c7d",
                                    "type":"function"
                                }
                            ]
                        },
                        {
                            "content":"{\"durationMs\":764,\"exitCode\":0,\"ok\":true,\"stderr\":\"\",\"stderrTruncated\":false,\"stdout\":\"PowerShell 7.5.4\\r\\n---\\r\\nPSVersion 7.5.4\\r\\n---\\r\\nMicrosoft Windows NT 10.0.26200.0\\r\\n\",\"stdoutTruncated\":false,\"timedOut\":false,\"truncated\":false}",
                            "role":"tool",
                            "tool_call_id":"call_915742b663134f368df81c7d"
                        }
                    ],
                    "mcpCall":null
                }
            }"#,
        )
        .expect("real stored message json should parse");
        let mut assistant: ChatMessage = serde_json::from_value(raw["message"].clone())
            .expect("chat message should deserialize from stored json");
        assistant.speaker_agent_id = Some(agent.id.clone());

        let messages = vec![
            test_text_message("user", "你先调用一次工具，看看终端版本，然后告诉我是什么", "2026-05-08T11:43:52Z"),
            assistant,
            test_text_message("user", "继续", "2026-05-08T11:45:00Z"),
        ];
        let conv = test_active_conversation_with_messages(messages, Some("2026-05-08T11:45:00Z".to_string()));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
        );

        assert!(prepared.history_messages.iter().any(|message| {
            message.role == "assistant"
                && message
                    .tool_calls
                    .as_ref()
                    .map(|calls| calls.len() == 1)
                    .unwrap_or(false)
                && message.reasoning_content.as_deref() == Some("用户想让我调用一次终端命令，查看版本信息。这应该是指操作系统的终端信息，比如 PowerShell 版本之类的。")
        }));
        assert!(prepared.history_messages.iter().any(|message| {
            message.role == "tool"
                && message.tool_call_id.as_deref() == Some("call_915742b663134f368df81c7d")
                && message.text.contains("PowerShell 7.5.4")
        }));
        assert!(prepared.history_messages.iter().any(|message| {
            message.role == "assistant"
                && message.text.contains("这是一张你发来的截图")
                && message.reasoning_content.is_none()
        }));

        let request_messages = prepared_prompt_to_messages_json(&prepared);
        assert!(request_messages.iter().any(|message| {
            message.get("role").and_then(Value::as_str) == Some("assistant")
                && message
                    .get("tool_calls")
                    .and_then(Value::as_array)
                    .map(|calls| calls.len() == 1)
                    .unwrap_or(false)
                && message
                    .get("reasoning_content")
                    .and_then(Value::as_str)
                    == Some("用户想让我调用一次终端命令，查看版本信息。这应该是指操作系统的终端信息，比如 PowerShell 版本之类的。")
        }));
        assert!(request_messages.iter().any(|message| {
            message.get("role").and_then(Value::as_str) == Some("tool")
                && message.get("tool_call_id").and_then(Value::as_str)
                    == Some("call_915742b663134f368df81c7d")
        }));
        assert!(request_messages.iter().any(|message| {
            message.get("role").and_then(Value::as_str) == Some("assistant")
                && message.get("content").and_then(Value::as_str).is_some_and(|text| text.contains("这是一张你发来的截图"))
        }));
    }

    #[test]
    fn prepared_prompt_to_messages_json_should_skip_empty_latest_user_turn_when_no_media() {
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
        assert!(!messages.iter().any(|message| {
            message.get("role").and_then(Value::as_str) == Some("user")
                && message.get("content").and_then(Value::as_str) == Some(" ")
        }));
    }

    #[test]
    fn build_prompt_should_prefer_conversation_bound_agent_over_passed_agent() {
        let now = now_iso();
        let agent = default_agent();
        let mut wrong_agent = default_agent();
        wrong_agent.id = "another-agent".to_string();
        wrong_agent.name = "另一个人格".to_string();
        let mut assistant = test_text_message("assistant", "我本来是一条助手消息", &now);
        assistant.speaker_agent_id = Some(agent.id.clone());
        assistant.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "先调用工具",
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "exec",
                        "arguments": "{\"command\":\"pwsh --version\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "PowerShell 7.5.4"
            }),
        ]);
        let messages = vec![
            test_text_message("user", "上一轮", &now),
            assistant,
            test_text_message("user", "继续", &now),
        ];
        let mut conv = test_active_conversation_with_messages(messages, Some(now));
        conv.agent_id = agent.id.clone();

        let prepared = build_prompt(
            &conv,
            &wrong_agent,
            &[agent.clone(), wrong_agent.clone(), default_user_persona()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
        );

        assert!(prepared.history_messages.iter().any(|message| {
            message.role == "assistant"
                && message
                    .tool_calls
                    .as_ref()
                    .map(|calls| !calls.is_empty())
                    .unwrap_or(false)
                && message.reasoning_content.as_deref() == Some("先调用工具")
        }));
        assert!(prepared.history_messages.iter().all(|message| {
            !(message.role == "user" && message.text == "我本来是一条助手消息")
        }));
    }

    #[test]
    fn resolve_conversation_bound_agent_should_error_when_agent_missing() {
        let now = now_iso();
        let agent = default_agent();
        let mut conv = test_active_conversation_with_messages(Vec::new(), Some(now));
        conv.agent_id = "missing-agent".to_string();
        let agents = vec![agent.clone(), default_user_persona()];

        let err = resolve_conversation_bound_agent(&conv, &agents)
            .expect_err("missing bound agent should fail");

        assert!(err.contains("会话绑定人格不存在或不可用"));
    }

    #[test]
    fn resolve_conversation_bound_agent_should_error_when_agent_missing_without_department() {
        let now = now_iso();
        let agent = default_agent();
        let mut conv = test_active_conversation_with_messages(Vec::new(), Some(now));
        conv.agent_id = "missing-agent".to_string();

        let err = resolve_conversation_bound_agent(
            &conv,
            &[agent, default_user_persona()],
        )
        .expect_err("missing agent and department should fail");

        assert!(err.contains("会话绑定人格不存在或不可用"));
    }

    #[test]
    fn build_stop_chat_partial_assistant_message_should_keep_final_reasoning_separate_from_tool_round_reasoning(
    ) {
        let message = build_stop_chat_partial_assistant_message(
            "agent-a",
            "终端版本是 PowerShell 7.5.4。",
            "先调用终端工具查看 PowerShell 版本。我已经拿到工具结果，现在直接回答用户终端版本。",
            "",
            &[
                serde_json::json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "reasoning_content": "先调用终端工具查看 PowerShell 版本。",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "exec",
                            "arguments": "{\"command\":\"pwsh --version\"}"
                        }
                    }]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": "PowerShell 7.5.4"
                }),
            ],
        );

        assert!(message.provider_meta.is_none());
        assert_eq!(
            message
                .tool_call
                .as_ref()
                .and_then(|events| events.first())
                .and_then(|event| event.get("reasoning_content"))
                .and_then(Value::as_str),
            Some("先调用终端工具查看 PowerShell 版本。")
        );
    }

    #[test]
    fn build_stop_chat_partial_assistant_message_should_not_promote_tool_reasoning_when_final_text_missing(
    ) {
        let message = build_stop_chat_partial_assistant_message(
            "agent-a",
            "",
            "第1轮先调用工具",
            "",
            &[
                serde_json::json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "reasoning_content": "第1轮先调用工具",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"README.md\"}"
                        }
                    }]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": "{\"ok\":true}"
                }),
            ],
        );

        assert!(message.provider_meta.is_none());
        assert_eq!(
            message
                .tool_call
                .as_ref()
                .and_then(|events| events.first())
                .and_then(|event| event.get("reasoning_content"))
                .and_then(Value::as_str),
            Some("第1轮先调用工具")
        );
    }

    #[test]
    fn build_stop_chat_partial_assistant_message_should_keep_reasoning_only_final_turn_when_text_not_arrived_yet(
    ) {
        let message = build_stop_chat_partial_assistant_message(
            "agent-a",
            "",
            "第1轮先调用工具\n\n我已经拿到结果，准备组织最终答复。",
            "",
            &[
                serde_json::json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "reasoning_content": "第1轮先调用工具",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"README.md\"}"
                        }
                    }]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call_1",
                    "content": "{\"ok\":true}"
                }),
            ],
        );

        assert!(message.provider_meta.is_none());
        match message.parts.first() {
            Some(MessagePart::Text { text, .. }) => assert!(text.is_empty()),
            other => panic!("unexpected message part: {:?}", other),
        }
        assert_eq!(
            message
                .tool_call
                .as_ref()
                .and_then(|events| events.first())
                .and_then(|event| event.get("reasoning_content"))
                .and_then(Value::as_str),
            Some("第1轮先调用工具")
        );
    }

    #[test]
    fn stop_partial_message_should_merge_cached_tools_with_already_persisted_formal_message() {
        let existing_tool_history = vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "先读取配置文件。",
                "tool_calls": [{
                    "id": "call_existing",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\":\"app.toml\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_existing",
                "content": "已有工具结果"
            }),
        ];
        let cached_tool_history = vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "reasoning_content": "再检查运行日志。",
                "tool_calls": [{
                    "id": "call_cached",
                    "type": "function",
                    "function": {
                        "name": "read_file",
                        "arguments": "{\"path\":\"runtime.log\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_cached",
                "content": "缓存中的工具结果"
            }),
        ];

        let message = build_stop_chat_partial_assistant_message_for_id(
            "assistant-formal",
            "agent-a",
            "2026-07-28T10:00:00Z",
            Some("agent-a".to_string()),
            Some(existing_tool_history),
            None,
            "",
            "",
            &cached_tool_history,
        );
        let tool_history = message.tool_call.expect("merged tool history");

        assert!(tool_history.iter().any(|event| {
            event.get("tool_call_id").and_then(Value::as_str) == Some("call_existing")
        }));
        assert!(tool_history.iter().any(|event| {
            event.get("tool_call_id").and_then(Value::as_str) == Some("call_cached")
        }));
        assert_eq!(
            tool_history
                .iter()
                .filter(|event| event.get("tool_call_id").and_then(Value::as_str) == Some("call_existing"))
                .count(),
            1
        );
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
                        "agentName": "fairy"
                    },
                    {
                        "agentId": "agent-zhongli",
                        "agentName": "钟离"
                    }
                ]
            }
        }));
        let conv = test_active_conversation_with_messages(vec![user_message], Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
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
                reasoning_content: None,
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
            meme_annotations: None,
            },
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
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
                reasoning_content: None,
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
            meme_annotations: None,
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
                reasoning_content: None,
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
            meme_annotations: None,
            },
            trailing_assistant,
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
        );

        assert_eq!(prepared.history_messages.len(), 2);
        assert!(prepared.history_messages[0].text.contains("新摘要"));
        assert!(!prepared.history_messages[0].text.contains("旧摘要"));
        assert_eq!(prepared.history_messages[1].text, "摘要后的助手消息");
    }

    #[test]
    fn build_prompt_after_compaction_should_not_replay_pre_compaction_checkpoint_tool_history() {
        let now = now_iso();
        let agent = default_agent();
        let checkpoint = build_stop_chat_partial_assistant_message(
            &agent.id,
            "我已经读取完文件，准备继续处理。",
            "先读取文件，再继续总结。",
            "",
            &[
                serde_json::json!({
                    "role": "assistant",
                    "content": Value::Null,
                    "reasoning_content": "先读取文件",
                    "tool_calls": [{
                        "id": "call_read",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"README.md\"}"
                        }
                    }]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call_read",
                    "content": "README 内容"
                }),
            ],
        );
        let compaction = ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "[上下文整理]\n整理摘要：已读取 README，接下来继续处理。".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "message_meta": {
                    "kind": "context_compaction",
                    "scene": "compaction",
                    "reason": "organize_context"
                }
            })),
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        };
        let messages = vec![
            test_text_message("user", "请读取 README 并继续处理", &now),
            checkpoint,
            compaction,
            test_text_message("user", "继续", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
        );

        assert_eq!(prepared.latest_user_text, "继续");
        assert_eq!(prepared.history_messages.len(), 1);
        assert!(prepared.history_messages[0].text.contains("已读取 README"));
        assert!(
            prepared
                .history_messages
                .iter()
                .all(|message| message.tool_calls.is_none())
        );
        assert!(
            prepared
                .history_messages
                .iter()
                .all(|message| !message.text.contains("README 内容"))
        );
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
                reasoning_content: None,
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
            meme_annotations: None,
            },
            trailing_assistant,
            test_text_message("user", "这是压缩后的最新用户消息", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));

        let prepared = build_prompt(
            &conv,
            &agent,
            &[agent.clone(), default_user_persona()],
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
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
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
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
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
            None,
            None,
            None,
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
    fn resolve_remote_im_auto_send_target_should_only_auto_send_single_source() {
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
            true,
        )
        .expect("single target");
        assert_eq!(single_target, Some(single_source.clone()));
        assert_eq!(
            resolve_remote_im_auto_send_target(
                "你好，这里是最终回复。",
                &[single_source.clone()],
                false,
            )
            .expect("non delegate target"),
            None,
        );

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
            true,
        )
        .expect("multiple sources should skip auto send");
        assert!(multiple_sources.is_none());
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
                assistant_message_id: None,
                session_info: ChatSessionInfo {
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
                assistant_message_id: None,
                session_info: ChatSessionInfo {
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
                assistant_message_id: None,
                session_info: ChatSessionInfo {
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
                assistant_message_id: None,
                session_info: ChatSessionInfo {
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

    #[test]
    fn remote_im_event_requires_reply_delegate_should_only_match_group_messages() {
        let created_at = now_iso();
        let mut private_event = test_pending_event("conversation-a");
        private_event.source = ChatEventSource::RemoteIm;
        private_event.created_at = created_at.clone();
        private_event.sender_info = Some(RemoteImMessageSource {
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
        });
        let mut group_event = private_event.clone();
        group_event.sender_info.as_mut().expect("测试消息来源存在").remote_contact_type = "group".to_string();

        assert!(!remote_im_event_requires_reply_delegate(&private_event));
        assert!(remote_im_event_requires_reply_delegate(&group_event));
        assert!(!remote_im_event_should_observe_after_persistence(&group_event, false));
        assert!(remote_im_event_should_observe_after_persistence(&group_event, true));
    }

    #[test]
    fn filter_remote_im_follow_up_sources_should_wait_for_pending_queue_message() {
        let state = test_chat_runtime_state();
        let created_at = now_iso();
        let remote_sender = RemoteImMessageSource {
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
        let source = remote_im_activation_source_from_sender(&remote_sender);
        let event = ChatPendingEvent {
            id: Uuid::new_v4().to_string(),
            conversation_id: "conversation-a".to_string(),
            created_at: created_at.clone(),
            source: ChatEventSource::RemoteIm,
            queue_mode: ChatQueueMode::Normal,
            messages: vec![test_text_message("user", "忙碌期间来的新消息", &created_at)],
            activate_assistant: true,
            assistant_message_id: None,
            session_info: ChatSessionInfo {
                agent_id: DEFAULT_AGENT_ID.to_string(),
            },
            runtime_context: None,
            sender_info: Some(remote_sender),
        };
        {
            let mut slots = state
                .conversation_runtime_slots
                .lock()
                .expect("lock runtime slots");
            let slot = conversation_slot_mut(&mut slots, "conversation-a");
            slot.pending_queue.push_back(event);
        }

        assert!(remote_im_source_has_pending_queue_event(
            &state,
            "conversation-a",
            &source,
        ));
        let filtered = filter_remote_im_follow_up_sources_for_pending_queue(
            &state,
            "conversation-a",
            vec![source],
        );
        assert!(filtered.is_empty());
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
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            filter_markdown: false,
            allow_send_files: false,
            behavior_settings: RemoteImChannelBehaviorSettings::default(),
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
                reasoning_content: None,
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
        meme_annotations: None,
        });

        let mut data = AppData::default();
        data.conversations.push(conversation);
        let contact = RemoteImContact {
            id: "contact-record-a".to_string(),
            channel_id: "remote-im-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "private".to_string(),
            remote_contact_id: "contact-a".to_string(),
            remote_contact_name: "张三".to_string(),
            avatar_url: String::new(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "always".to_string(),
            activation_keywords: Vec::new(),
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_agent_id: None,
            bound_conversation_id: Some(conversation_id.clone()),
            processing_mode: "continuous".to_string(),
            response_strategy: default_remote_im_contact_response_strategy(),
            response_guidance: default_remote_im_contact_response_guidance(),
            blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
            group_reply_pacing: RemoteImGroupReplyPacing::default(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        };
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
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
        state_read_conversation_cached(state, conversation_id)
            .expect("read conversation")
            .messages
            .iter()
            .find(|message| message.id == assistant_message_id)
            .and_then(|message| message.provider_meta.as_ref())
            .and_then(|meta| meta.get("remoteImDecision"))
            .cloned()
            .expect("remoteImDecision")
    }

    #[test]
    fn contact_send_files_should_fall_back_to_persisted_contact_binding() {
        let (state, _source, conversation_id, _assistant_message_id, _assistant_text) =
            seed_remote_im_auto_send_test_state(serde_json::json!({ "mockSend": true }));
        let activation_sources =
            get_conversation_remote_im_activation_sources(&state, &conversation_id)
                .expect("read activation sources");
        assert!(activation_sources.is_empty());

        let session_id = format!("agent-a::{conversation_id}::remote_reply_delegate:delegate-a");
        let (_channel, contact) = remote_im_bound_contact_context_from_runtime(&state, &session_id)
            .expect("persisted contact binding should resolve contact file target");

        assert_eq!(contact.id, "contact-record-a");
        assert_eq!(contact.remote_contact_id, "contact-a");
    }

    #[test]
    fn remote_im_auto_send_and_record_decision_should_update_message_after_mock_send() {
        let (state, activation_source, conversation_id, assistant_message_id, assistant_text) =
            seed_remote_im_auto_send_test_state(serde_json::json!({
                "mockSend": true
            }));
        let assistant_message = state_read_conversation_cached(&state, &conversation_id)
            .expect("read conversation")
            .messages
            .iter()
            .find(|message| message.id == assistant_message_id)
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
                None,
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
            Some("remote_im_contact")
        );
        assert_eq!(
            decision.get("activationSourceCount").and_then(Value::as_u64),
            None
        );
        assert_eq!(decision.get("error").and_then(Value::as_str), Some(""));
    }

    #[test]
    fn remote_im_delegate_core_round_should_defer_auto_send_until_final_iteration() {
        let runtime_context = RuntimeContext {
            remote_im_reply_delegate_id: Some("delegate-a".to_string()),
            remote_im_defer_auto_send: true,
            ..RuntimeContext::default()
        };
        assert!(!remote_im_should_auto_send_after_core_round(&runtime_context));
        assert!(remote_im_should_auto_send_after_core_round(
            &RuntimeContext::default()
        ));
    }

    #[test]
    fn remote_im_auto_send_and_record_decision_should_mark_send_failed_after_mock_error() {
        let (state, activation_source, conversation_id, assistant_message_id, assistant_text) =
            seed_remote_im_auto_send_test_state(serde_json::json!({
                "mockSendError": "mock remote send failed"
            }));
        let assistant_message = state_read_conversation_cached(&state, &conversation_id)
            .expect("read conversation")
            .messages
            .iter()
            .find(|message| message.id == assistant_message_id)
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
                None,
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
            Some("remote_im_contact")
        );
        assert_eq!(
            decision.get("activationSourceCount").and_then(Value::as_u64),
            None
        );
        assert_eq!(
            decision.get("error").and_then(Value::as_str),
            Some("mock remote send failed")
        );
    }

    #[test]
    fn remote_im_group_auto_send_uncertain_result_should_not_schedule_body_retry() {
        let (state, mut activation_source, conversation_id, assistant_message_id, assistant_text) =
            seed_remote_im_auto_send_test_state(serde_json::json!({
                "mockSendError": "mock delivery timeout",
                "mockSendErrorKind": "uncertain"
            }));
        activation_source.remote_contact_type = "group".to_string();
        let mut contact = state_service_get_remote_im_contact(&state, "contact-record-a")
            .expect("read contact")
            .expect("contact");
        contact.remote_contact_type = "group".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        let mut trigger = remote_im_test_group_user_message("user-a");
        trigger.id = "group-trigger-a".to_string();
        let event = create_pending_event(
            "group-event-a".to_string(),
            conversation_id.clone(),
            vec![trigger],
            true,
            ChatSessionInfo {
                agent_id: "agent-a".to_string(),
            },
            RemoteImMessageSource {
                channel_id: activation_source.channel_id.clone(),
                platform: activation_source.platform.clone(),
                im_name: "QQ".to_string(),
                remote_contact_type: "group".to_string(),
                remote_contact_id: activation_source.remote_contact_id.clone(),
                remote_contact_name: activation_source.remote_contact_name.clone(),
                sender_id: "user-a".to_string(),
                sender_name: "user-a".to_string(),
                sender_avatar_url: None,
                platform_message_id: None,
            },
        );
        let generation = 7001;
        let state_key = remote_im_group_reply_state_key(&state, &contact.id);
        lock_remote_im_group_reply_state_store().by_contact.insert(
            state_key.clone(),
            RemoteImGroupReplyState {
                generation,
                phase: RemoteImGroupReplyPhase::AssistantDispatching,
                start_message_id: "group-trigger-a".to_string(),
                decision_end_message_id: Some("group-trigger-a".to_string()),
                focus: false,
                energy_settled: false,
                next_round_mention: false,
                event,
                due_at: std::time::Instant::now(),
                inspection_kind: RemoteImGroupReplyTimerKind::Mention,
                pending_settlement: None,
            },
        );
        let assistant_message = state_read_conversation_cached(&state, &conversation_id)
            .expect("read conversation")
            .messages
            .iter()
            .find(|message| message.id == assistant_message_id)
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
                Some(RemoteImGroupReplyDispatchPolicy {
                    generation,
                    focus: false,
                    max_chars: 200,
                }),
            ))
            .expect("uncertain result should be handled without body retry");
        assert!(matches!(
            outcome,
            RemoteImAutoSendExecutionOutcome::DeliveryUncertain { .. }
        ));
        assert!(!lock_remote_im_group_reply_state_store()
            .by_contact
            .contains_key(&state_key));
        let checkpoint = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read checkpoint")
            .expect("checkpoint");
        assert_eq!(
            checkpoint.group_reply_delivery.as_ref().map(|marker| marker.status.as_str()),
            Some("uncertain")
        );
        assert_eq!(
            checkpoint.last_boundary_covers_message_id.as_deref(),
            Some("group-trigger-a")
        );
        assert_eq!(checkpoint.last_success_reply_at, None);
        assert!(checkpoint
            .group_reply_delivery
            .as_ref()
            .map(|marker| marker.energy_applied)
            .unwrap_or(false));
        let decision =
            read_remote_im_decision_for_message(&state, &conversation_id, &assistant_message_id);
        assert_eq!(
            decision.get("action").and_then(Value::as_str),
            Some("delivery_uncertain")
        );
    }

    #[test]
    fn remote_im_group_auto_send_definite_preflight_failure_should_keep_batch_without_energy_cost() {
        let (state, mut source, conversation_id, assistant_message_id, assistant_text) =
            seed_remote_im_auto_send_test_state(serde_json::json!({
                "mockSendError": "mock request rejected before send"
            }));
        source.remote_contact_type = "group".to_string();
        let mut contact = state_service_get_remote_im_contact(&state, "contact-record-a")
            .expect("read contact")
            .expect("contact");
        contact.remote_contact_type = "group".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        let generation = {
            let mut store = lock_remote_im_group_reply_state_store();
            let generation = remote_im_group_reply_next_generation(&mut store);
            store.by_contact.insert(
                remote_im_group_reply_state_key(&state, &contact.id),
                RemoteImGroupReplyState {
                    generation,
                    phase: RemoteImGroupReplyPhase::AssistantDispatching,
                    start_message_id: "group-preflight-a".to_string(),
                    decision_end_message_id: Some("group-preflight-a".to_string()),
                    focus: false,
                    energy_settled: false,
                    next_round_mention: false,
                    event: create_pending_event(
                        "group-preflight-event".to_string(),
                        conversation_id.clone(),
                        vec![remote_im_test_group_user_message("user-a")],
                        true,
                        ChatSessionInfo {
                            agent_id: "agent-a".to_string(),
                        },
                        RemoteImMessageSource {
                            channel_id: source.channel_id.clone(),
                            platform: source.platform.clone(),
                            im_name: "QQ".to_string(),
                            remote_contact_type: "group".to_string(),
                            remote_contact_id: source.remote_contact_id.clone(),
                            remote_contact_name: source.remote_contact_name.clone(),
                            sender_id: "user-a".to_string(),
                            sender_name: "user-a".to_string(),
                            sender_avatar_url: None,
                            platform_message_id: None,
                        },
                    ),
                    due_at: std::time::Instant::now(),
                    inspection_kind: RemoteImGroupReplyTimerKind::Mention,
                    pending_settlement: None,
                },
            );
            generation
        };
        let assistant_message = state_read_conversation_cached(&state, &conversation_id)
            .expect("read conversation")
            .messages
            .iter()
            .find(|message| message.id == assistant_message_id)
            .cloned()
            .expect("assistant message");

        let outcome = test_runtime()
            .block_on(remote_im_auto_send_and_record_decision(
                &state,
                &source,
                &conversation_id,
                &assistant_text,
                Some(&assistant_message),
                Some(&assistant_message_id),
                Some(RemoteImGroupReplyDispatchPolicy {
                    generation,
                    focus: false,
                    max_chars: 200,
                }),
            ))
            .expect("definite preflight failure should be deferred");
        assert!(matches!(
            outcome,
            RemoteImAutoSendExecutionOutcome::PreflightDeferred { .. }
        ));
        let state_key = remote_im_group_reply_state_key(&state, &contact.id);
        let mut store = lock_remote_im_group_reply_state_store();
        let retry = store.by_contact.get(&state_key).expect("batch retained");
        assert!(retry.generation > generation);
        assert_eq!(retry.phase, RemoteImGroupReplyPhase::MentionScheduled);
        store.by_contact.remove(&state_key);
        drop(store);
        let checkpoint = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read checkpoint")
            .expect("checkpoint");
        assert_eq!(checkpoint.energy, None);
        assert_eq!(checkpoint.last_success_reply_at, None);
        assert_eq!(
            checkpoint.group_reply_delivery.as_ref().map(|marker| marker.status.as_str()),
            Some("preflight_failed")
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
    fn archive_decision_should_not_archive_after_idle_when_usage_below_force_threshold() {
        let now = now_utc();
        let old = (now - time::Duration::minutes(31))
            .format(&Rfc3339)
            .expect("format old time");
        let d = decide_archive_before_model_request(300, 1000, Some(&old), true);
        assert!(!d.should_archive);
        assert!(!d.forced);
        assert!(d.usage_ratio >= 0.30);
        assert_eq!(d.reason, "context_usage_below_force_threshold");
    }

    #[test]
    fn archive_decision_should_not_archive_when_usage_below_force_threshold() {
        let now = now_utc();
        let old = (now - time::Duration::minutes(31))
            .format(&Rfc3339)
            .expect("format old time");
        let d = decide_archive_before_model_request(299, 1000, Some(&old), true);
        assert!(!d.should_archive);
        assert!(!d.forced);
        assert!(d.usage_ratio < 0.30);
        assert_eq!(d.reason, "context_usage_below_force_threshold");
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
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "effectivePromptTokens": 640,
                "contextUsageRatio": 0.64,
                "contextUsagePercent": 64
            })),
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
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
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: "global".to_string(),
            scope: "global".to_string(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
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
    fn latest_real_prompt_usage_should_read_compatible_prompt_usage_fields() {
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-main", "active", &now);
        conversation.messages.push(ChatMessage {
            id: "assistant-1".to_string(),
            role: "assistant".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "兼容字段".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "providerPromptTokens": 250,
                "contextUsagePercent": 30
            })),
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        });
        let mut api = ApiConfig::default();
        api.context_window_tokens = 1000;

        let usage = conversation_prompt_service()
            .latest_real_prompt_usage(&conversation, &api)
            .expect("latest real prompt usage");

        assert_eq!(usage.source, "assistant_message_provider_prompt_tokens");
        assert_eq!(usage.effective_prompt_tokens, 250);
        assert!((usage.usage_ratio - 0.3).abs() < f64::EPSILON);
        assert!(usage.estimated_prompt_tokens.is_none());
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
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "effectivePromptTokens": 640,
                "contextUsageRatio": 0.64,
                "contextUsagePercent": 64
            })),
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        });
        conversation.messages.push(ChatMessage {
            id: "compaction-boundary".to_string(),
            role: "user".to_string(),
            created_at: now.clone(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "上下文整理".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "message_meta": {
                    "kind": "context_compaction"
                }
            })),
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        });
        conversation.messages.push(ChatMessage {
            id: "user-after-compaction".to_string(),
            role: "user".to_string(),
            created_at: now.clone(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "压缩后的用户消息".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
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
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
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
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: "global".to_string(),
            scope: "global".to_string(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
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
    fn latest_real_prompt_usage_should_read_last_valid_usage_from_tool_events() {
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-main", "active", &now);
        conversation.messages.push(ChatMessage {
            id: "assistant-1".to_string(),
            role: "assistant".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "最新一组消息".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: Some(vec![
                serde_json::json!({
                    "role": "assistant",
                    "tool_calls": [],
                    "usage": {"promptTokens": 1200, "contextWindowTokens": 10000},
                }),
                serde_json::json!({"role": "tool", "tool_call_id": "a"}),
                serde_json::json!({
                    "role": "assistant",
                    "tool_calls": [],
                    "usage": {"promptTokens": 3200, "contextWindowTokens": 10000},
                }),
            ]),
            mcp_call: None,
        meme_annotations: None,
        });
        let mut api = ApiConfig::default();
        api.context_window_tokens = 10000;

        let usage = conversation_prompt_service()
            .latest_real_prompt_usage(&conversation, &api)
            .expect("tool event usage");

        assert_eq!(usage.source, "assistant_tool_event_prompt_tokens");
        assert_eq!(usage.effective_prompt_tokens, 3200, "倒序取最后一个带真实用量的事件");
        assert!((usage.usage_ratio - 0.32).abs() < f64::EPSILON);
        assert!(usage.estimated_prompt_tokens.is_none());
    }

    #[test]
    fn latest_real_prompt_usage_should_not_fall_back_to_older_messages_when_latest_group_has_no_usage() {
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-main", "active", &now);
        conversation.messages.push(ChatMessage {
            id: "assistant-older-with-usage".to_string(),
            role: "assistant".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "更早的一条带真实用量消息".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "providerPromptTokens": 999,
                "contextUsagePercent": 50
            })),
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        });
        conversation.messages.push(ChatMessage {
            id: "assistant-latest-no-usage".to_string(),
            role: "assistant".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "最新一组整组没有真实用量".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        });

        let usage =
            conversation_prompt_service().latest_real_prompt_usage(&conversation, &ApiConfig::default());
        assert!(usage.is_none(), "最新一组没有用量时不往前翻更早的消息");
    }

    #[test]
    fn runtime_trusted_prompt_usage_should_be_reused_during_dispatch() {
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-main", "active", &now);
        conversation.messages.push(ChatMessage {
            id: "assistant-1".to_string(),
            role: "assistant".to_string(),
            created_at: now.clone(),
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "最近一次真实返回".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "effectivePromptTokens": 640,
                "contextUsageRatio": 0.64,
                "contextUsagePercent": 64
            })),
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
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
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: "global".to_string(),
            scope: "global".to_string(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
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
        let primed = conversation_prompt_service().prime_runtime_trusted_prompt_usage(
            &mut runtime_context,
            &conversation,
            &prepared,
            &ApiConfig::default(),
            &agent,
        );
        assert_eq!(primed.source, "assistant_message_effective_prompt_tokens");

        assert!(runtime_context.trusted_prompt_usage.is_some());

        let reused = conversation_prompt_service().prime_runtime_trusted_prompt_usage(
            &mut runtime_context,
            &conversation,
            &prepared,
            &ApiConfig::default(),
            &agent,
        );
        assert_eq!(reused.source, "trusted_prompt_usage");
        assert_eq!(reused.effective_prompt_tokens, 640);
        assert!(reused.estimated_prompt_tokens.is_none());
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
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: "global".to_string(),
            scope: "global".to_string(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
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
        let first = conversation_prompt_service().resolve_shared_trusted_prompt_usage_or_estimate(
            &trusted,
            &prepared,
            &ApiConfig::default(),
            &agent,
        );
        assert_eq!(first.source, "trusted_prompt_usage");
        assert_eq!(first.effective_prompt_tokens, 640);
        assert!(first.estimated_prompt_tokens.is_none());

        let second = conversation_prompt_service().resolve_shared_trusted_prompt_usage_or_estimate(
            &trusted,
            &prepared,
            &ApiConfig::default(),
            &agent,
        );
        assert_eq!(second.source, "trusted_prompt_usage");
        assert_eq!(second.effective_prompt_tokens, 640);
        assert!(second.estimated_prompt_tokens.is_none());

        conversation_prompt_service().refresh_shared_trusted_prompt_usage(
            &trusted,
            None,
            &ApiConfig::default(),
        );
        let after_missing_provider_usage = conversation_prompt_service()
            .resolve_shared_trusted_prompt_usage_or_estimate(
                &trusted,
                &prepared,
                &ApiConfig::default(),
                &agent,
            );
        assert_eq!(after_missing_provider_usage.source, "trusted_prompt_usage");
        assert_eq!(after_missing_provider_usage.effective_prompt_tokens, 640);
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
            false,
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
            data_path: root.join("config_mark"),
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
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_conversation_metadata: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_field_metadata_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_background_shell_tasks: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            schedule_events: Arc::new(Mutex::new(ScheduleEventStore::default())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            goal_continue_suppressed_conversation_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            accepted_submit_trace_ids: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_list_activity_marks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            task_scheduler_notify: Arc::new(tokio::sync::Notify::new()),
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
            remote_im_contact_runtime_states: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_reply_delegate_runtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_semaphore: Arc::new(tokio::sync::Semaphore::new(8)),
            remote_im_channel_state_write_locks: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
            assistant_message_id: None,
            session_info: ChatSessionInfo {
                agent_id: DEFAULT_AGENT_ID.to_string(),
            },
            runtime_context: None,
            sender_info: None,
        }
    }

    #[test]
    fn goal_continue_suppression_should_clear_on_next_non_goal_event() {
        let state = test_chat_runtime_state();
        let conversation_id = "conversation-goal-interrupted";
        mark_goal_continue_suppressed_by_user_interrupt(
            &state,
            conversation_id,
            "test_user_interrupt",
        )
        .expect("mark suppressed");
        assert!(goal_continue_is_suppressed(&state, conversation_id).expect("check suppressed"));

        let _ = ingress_chat_event(&state, test_pending_event(conversation_id))
            .expect("ingress event");
        assert!(!goal_continue_is_suppressed(&state, conversation_id).expect("check cleared"));
    }

    #[test]
    fn remote_group_active_goal_should_not_enqueue_goal_continuation() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-group-goal", "active", &now);
        conversation.conversation_kind = CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string();
        conversation.root_conversation_id = Some(
            "remote_im_contact:channel-group:group:group-1".to_string(),
        );
        conversation.active_goal = Some(ConversationGoalState {
            goal_id: "legacy-group-goal".to_string(),
            status: "active".to_string(),
            objective: "旧群聊目标不应续跑".to_string(),
            started_at: now.clone(),
            ended_at: None,
            usage_start: ConversationCumulativeUsage::default(),
            usage_end: None,
        });
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist remote group conversation");
        flush_pending_persists_blocking(&state).expect("flush remote group conversation");

        assert!(!maybe_enqueue_goal_continue_after_idle(&state, &conversation.id)
            .expect("group goal continuation should fail soft"));
        assert!(!conversation_has_pending_queue_events(&state, &conversation.id)
            .expect("read group queue"));
    }

    fn test_chat_conversation(conversation_id: &str, status: &str, updated_at: &str) -> Conversation {
        Conversation {
            id: conversation_id.to_string(),
            title: conversation_id.to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
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
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: Vec::new(),
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    #[test]
    fn list_archives_should_release_conversation_lock_before_slow_io() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut archive = test_chat_conversation("archive-lock-performance", "archived", &now);
        archive.title.clear();
        archive.archived_at = Some(now);
        write_conversation_shard(&state.data_path, &archive).expect("write archived conversation");
        state_read_chat_index_cached(&state).expect("warm chat index");

        let worker_state = state.clone();
        let metadata_state = worker_state.clone();
        let (stage_tx, stage_rx) = std::sync::mpsc::channel::<&'static str>();
        let metadata_stage_tx = stage_tx.clone();
        let title_stage_tx = stage_tx;
        let worker = std::thread::spawn(move || {
            conversation_service_v2().list_archives_with_resolvers(
                &worker_state,
                move |archive_id| {
                    metadata_stage_tx.send("metadata").expect("send metadata stage");
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    conversation_service_v2().get_conversation_meta(&metadata_state, archive_id)
                },
                move |_| {
                    title_stage_tx.send("title").expect("send title stage");
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    Some("归档标题".to_string())
                },
            )
        });

        for expected_stage in ["metadata", "title"] {
            let stage = stage_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("wait slow io stage");
            assert_eq!(stage, expected_stage);
            let lock_started_at = std::time::Instant::now();
            let guard = state
                .conversation_lock
                .lock_named("list_archives_performance_probe")
                .expect("acquire conversation lock during archive io");
            let waited = lock_started_at.elapsed();
            drop(guard);
            assert!(
                waited < std::time::Duration::from_millis(120),
                "stage={stage}, lock_wait_ms={}",
                waited.as_millis()
            );
        }

        let summaries = worker.join().expect("join archive list worker").expect("list archives");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].title, "归档标题");
    }

    fn test_user_switched_to_sub_conversation_data() -> AppData {
        let now = now_iso();
        let later = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format later");
        let mut data = AppData::default();
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
    fn remote_im_system_reminder_should_precede_meta_and_trigger_text() {
        let prepared = PreparedPrompt {
            preamble: String::new(),
            history_messages: Vec::new(),
            latest_user_text: "触发消息".to_string(),
            latest_user_meta_text: "meta".to_string(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: vec![
                build_remote_im_group_reply_length_reminder(false, 20),
                "[系统提醒]\n固定快照".to_string(),
                "普通附加块".to_string(),
            ],
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };

        let expected = vec![
            "[系统提醒]\n请在 20 个有效文本单位内进行回应。中文/日文/韩文按可见字形计 1，英语等按 Unicode 单词计 1，数字词和 Emoji 各计 1，标点与空白不计。",
            "[系统提醒]\n固定快照",
            "meta",
            "触发消息",
            "普通附加块",
        ];
        assert_eq!(prepared_prompt_latest_user_text_blocks(&prepared), expected);

        let first_messages = prepared_prompt_to_messages_json(&prepared);
        let second_messages = prepared_prompt_to_messages_json(&prepared);
        assert_eq!(first_messages, second_messages, "同一冻结上文重复组装必须完全一致");
        let content = first_messages
            .last()
            .and_then(|message| message.get("content"))
            .and_then(Value::as_array)
            .expect("latest user content array");
        let text_blocks = content
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(text_blocks, expected);
        assert!(!text_blocks.iter().any(|text| text.contains("user profile snapshot")));
    }

    #[test]
    fn state_read_app_data_cached_should_strip_runtime_conversations() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut data = AppData::default();
        data.conversations.push(test_chat_conversation("conversation-cached-app-data", "active", &now));
        state_write_app_data_cached(&state, &data).expect("write app data");

        let cached = state_read_app_data_cached(&state).expect("read app data");

        assert!(cached.conversations.is_empty());
    }

    #[test]
    fn state_read_chat_index_cached_should_rebuild_from_storage_without_disk_index() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-heal", "active", &now);

        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");

        let chat_index = state_read_chat_index_cached(&state).expect("read memory chat index");
        let item = chat_index
            .conversations
            .iter()
            .find(|item| item.id == conversation.id)
            .expect("healed chat index item");
        assert_eq!(item.updated_at, conversation.updated_at);
        assert_eq!(item.status, conversation.status);
        assert_eq!(item.archived_at, conversation.archived_at);
        assert!(!app_layout_chat_dir(&state.data_path).join("index.json").exists());
    }

    #[test]
    fn state_read_chat_index_cached_should_rebuild_archived_fields_from_storage() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-hidden-heal", "active", &now);
        conversation.archived_at = Some(now.clone());
        conversation.status = "archived".to_string();

        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");

        let chat_index = state_read_chat_index_cached(&state).expect("read memory chat index");
        let item = chat_index
            .conversations
            .iter()
            .find(|item| item.id == conversation.id)
            .expect("archived chat index item");
        assert_eq!(item.archived_at, conversation.archived_at);
        assert_eq!(item.status, conversation.status);
    }

    #[test]
    fn state_read_chat_index_cached_should_recover_archived_items_from_storage_snapshot() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-archived-recover", "active", &now);
        conversation.archived_at = Some(now.clone());
        conversation.status = "archived".to_string();

        write_conversation_shard(&state.data_path, &conversation).expect("write archived conversation");

        let chat_index = state_read_chat_index_cached(&state).expect("read rebuilt chat index");
        let item = chat_index
            .conversations
            .iter()
            .find(|item| item.id == conversation.id)
            .expect("recovered archived item");
        assert_eq!(item.archived_at, conversation.archived_at);
        assert_eq!(item.status, conversation.status);
    }

    #[test]
    fn unarchive_archive_should_restore_archived_chat_and_reject_active_chat() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut archived = test_chat_conversation("conversation-unarchive", "archived", &now);
        archived.archived_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &archived).expect("persist archived conversation");
        flush_pending_persists_blocking(&state).expect("flush archived conversation");

        conversation_service_v2()
            .unarchive_archive(&state, &archived.id)
            .expect("unarchive conversation");

        let restored = state_read_conversation_metadata_cached(&state, &archived.id)
            .expect("read restored conversation metadata");
        assert_eq!(restored.status(), "active");
        assert!(restored.archived_at().is_none());
        let chat_index = state_read_chat_index_cached(&state).expect("read chat index");
        let item = chat_index
            .conversations
            .iter()
            .find(|item| item.id == archived.id)
            .expect("restored chat index item");
        assert!(!chat_index_item_is_archived(item));

        let active = test_chat_conversation("conversation-unarchive-active", "active", &now);
        state_schedule_conversation_persist(&state, &active).expect("persist active conversation");
        flush_pending_persists_blocking(&state).expect("flush active conversation");
        let error = conversation_service_v2()
            .unarchive_archive(&state, &active.id)
            .expect_err("active conversation must not be unarchived");
        assert!(error.contains("无法恢复"));
    }

    #[test]
    fn read_app_bootstrap_snapshot_should_build_memory_chat_index_from_storage_snapshot() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-bootstrap-recover", "active", &now);
        conversation.archived_at = Some(now.clone());
        conversation.status = "archived".to_string();

        write_conversation_shard(&state.data_path, &conversation).expect("write archived conversation");
        state_service_set_message_store_migration_version(
            &state,
            MESSAGE_STORE_MIGRATION_CURRENT_VERSION,
        )
        .expect("mark message store migration complete");

        let _snapshot = read_app_bootstrap_snapshot(&state).expect("read bootstrap snapshot");

        let chat_index = state_read_chat_index_cached(&state).expect("read memory chat index");
        let archived_item = chat_index
            .conversations
            .iter()
            .find(|item| item.id == conversation.id)
            .expect("archived conversation should remain indexed");
        assert_eq!(archived_item.status, conversation.status);
        assert!(chat_index
            .conversations
            .iter()
            .any(|item| item.id == SYSTEM_NOTIFICATION_CONVERSATION_ID));
        assert!(!app_layout_chat_dir(&state.data_path).join("index.json").exists());
    }

    #[test]
    fn state_schedule_conversation_persist_should_update_memory_chat_index_only() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-memory-upsert", "active", &now);
        state_schedule_conversation_persist(&state, &conversation).expect("schedule persist");
        conversation.status = "archived".to_string();
        conversation.archived_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &conversation).expect("schedule updated persist");

        let chat_index = state_read_chat_index_cached(&state).expect("read memory chat index");
        let item = chat_index
            .conversations
            .iter()
            .find(|item| item.id == conversation.id)
            .expect("chat index item");
        assert_eq!(item.status, conversation.status);
        assert_eq!(item.archived_at, conversation.archived_at);
        assert!(!app_layout_chat_dir(&state.data_path).join("index.json").exists());
    }

    #[test]
    fn set_conversation_preferred_model_should_schedule_meta_only_persist() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-meta-only-model", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark persisted");

        let updated = conversation_service_v2()
            .set_conversation_preferred_api_config_id(
                &state,
                &conversation.id,
                Some("api-model-b".to_string()),
            )
            .expect("set preferred model");

        assert_eq!(
            updated.preferred_api_config_id.as_deref(),
            Some("api-model-b")
        );
        let pending = state
            .conversation_persist_pending
            .lock()
            .expect("lock pending");
        let pending = pending.as_ref().expect("pending meta persist");
        assert!(pending.conversations.is_empty());
        assert!(pending.metadata_conversation_ids.contains(&conversation.id));
        assert!(!pending.deleted_conversation_ids.contains(&conversation.id));
    }

    #[test]
    fn append_fast_request_turn_should_schedule_meta_only_persist() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-fast-request", "active", &now);
        conversation
            .messages
            .push(test_text_message("user", "hello", &now));
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark persisted");

        let turn = FastRequestTurn {
            id: "fast-request-a".to_string(),
            kind: "title_generation".to_string(),
            request_text: "request".to_string(),
            response_text: "response".to_string(),
            success: true,
            error: None,
            model_name: Some("quick-model".to_string()),
            duration_ms: Some(12),
            created_at: now.clone(),
        };
        let appended = conversation_service_v2()
            .append_fast_request_turn_if_unarchived_exists(&state, &conversation.id, turn.clone())
            .expect("append fast request turn");

        assert!(appended);
        assert_eq!(
            conversation_service_v2()
                .get_conversation_fast_request_turns(&state, &conversation.id)
                .expect("read fast request turns"),
            vec![turn]
        );
        {
            let pending = state
                .conversation_persist_pending
                .lock()
                .expect("lock pending");
            let pending = pending.as_ref().expect("pending meta persist");
            assert!(pending.conversations.is_empty());
            assert!(pending.metadata_conversation_ids.contains(&conversation.id));
            assert!(!pending.deleted_conversation_ids.contains(&conversation.id));
        }

        let wrote = flush_pending_persists_blocking(&state).expect("flush pending");
        let restored = read_conversation_shard(&state.data_path, &conversation.id)
            .expect("read restored conversation");

        assert!(wrote);
        assert_eq!(restored.messages.len(), 1);
        assert_eq!(restored.fast_request_turns.len(), 1);
        assert_eq!(restored.fast_request_turns[0].id, "fast-request-a");
    }

    #[test]
    fn set_conversation_preferred_model_should_update_existing_full_pending_snapshot() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-pending-model", "active", &now);
        state_schedule_conversation_persist(&state, &conversation).expect("schedule full persist");

        conversation_service_v2()
            .set_conversation_preferred_api_config_id(
                &state,
                &conversation.id,
                Some("api-model-c".to_string()),
            )
            .expect("set preferred model");

        let pending = state
            .conversation_persist_pending
            .lock()
            .expect("lock pending");
        let pending = pending.as_ref().expect("pending full persist");
        assert!(pending.metadata_conversation_ids.is_empty());
        assert_eq!(
            pending
                .conversations
                .get(&conversation.id)
                .and_then(|item| item.preferred_api_config_id.as_deref()),
            Some("api-model-c")
        );
    }

    #[test]
    fn set_conversation_auto_push_remote_contact_should_schedule_meta_only_persist() {
        let (state, source_id, _target_local_id, _remote_target_id) = seed_session_forward_test_state();
        flush_pending_persists_blocking(&state).expect("flush seeded full persists");

        let updated = conversation_service_v2()
            .set_conversation_auto_push_remote_contact_id(
                &state,
                &source_id,
                Some("contact-session-a".to_string()),
            )
            .expect("set auto push remote contact");

        assert_eq!(
            updated.auto_push_remote_contact_id.as_deref(),
            Some("contact-session-a")
        );
        let pending = state
            .conversation_persist_pending
            .lock()
            .expect("lock pending");
        let pending = pending.as_ref().expect("pending meta persist");
        assert!(pending.conversations.is_empty());
        assert!(pending.metadata_conversation_ids.contains(&source_id));
        assert!(!pending.deleted_conversation_ids.contains(&source_id));
    }

    #[test]
    fn set_conversation_auto_push_remote_contact_should_appear_in_overview_and_flush() {
        let (state, source_id, _target_local_id, _remote_target_id) = seed_session_forward_test_state();

        conversation_service_v2()
            .set_conversation_auto_push_remote_contact_id(
                &state,
                &source_id,
                Some("contact-session-a".to_string()),
            )
            .expect("set auto push remote contact");

        let summaries = conversation_service_v2()
            .list_unarchived_conversation_summaries(&state)
            .expect("list summaries")
            .summaries;
        let source_summary = summaries
            .iter()
            .find(|item| item.conversation_id == source_id)
            .expect("source summary");
        assert_eq!(
            source_summary.auto_push_remote_contact_id.as_deref(),
            Some("contact-session-a")
        );

        let wrote = flush_pending_persists_blocking(&state).expect("flush pending");
        let restored = read_conversation_shard(&state.data_path, &source_id)
            .expect("read restored conversation");
        assert!(wrote);
        assert_eq!(
            restored.auto_push_remote_contact_id.as_deref(),
            Some("contact-session-a")
        );
    }

    #[test]
    fn flush_pending_persists_should_write_meta_only_preferred_model() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-flush-meta-model", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark persisted");
        conversation_service_v2()
            .set_conversation_preferred_api_config_id(
                &state,
                &conversation.id,
                Some("api-model-flush".to_string()),
            )
            .expect("set preferred model");

        let wrote = flush_pending_persists_blocking(&state).expect("flush pending");
        let restored = read_conversation_shard(&state.data_path, &conversation.id)
            .expect("read restored conversation");

        assert!(wrote);
        assert_eq!(
            restored.preferred_api_config_id.as_deref(),
            Some("api-model-flush")
        );
        assert!(restored.messages.is_empty());
    }

    #[test]
    fn foreground_snapshot_should_include_cached_preferred_model() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-snapshot-model", "active", &now);
        conversation.messages.push(test_text_message("assistant", "最后一条消息", &now));
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark persisted");

        conversation_service_v2()
            .set_conversation_preferred_api_config_id(
                &state,
                &conversation.id,
                Some("api-model-snapshot".to_string()),
            )
            .expect("set preferred model");

        let snapshot = conversation_service_v2()
            .read_foreground_snapshot(&state, Some(&conversation.id), None, 4)
            .expect("read foreground snapshot");
        let summaries = conversation_service_v2()
            .list_unarchived_conversation_summaries(&state)
            .expect("list summaries")
            .summaries;
        let summaries_json = serde_json::to_string(&summaries).expect("serialize summaries");

        assert_eq!(
            snapshot.preferred_api_config_id.as_deref(),
            Some("api-model-snapshot")
        );
        assert_eq!(snapshot.last_message_id.as_deref(), Some(conversation.messages[0].id.as_str()));
        assert!(
            !summaries_json.contains("preferredApiConfigId"),
            "conversation overview must not carry model metadata"
        );
    }

    #[test]
    fn changed_since_should_read_changed_ids_directly_and_report_missing_as_deleted() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-changed-since-a", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation).expect("mark persisted");

        // 首调：无 since → 全量基线，包含已落盘的会话。
        let first = list_unarchived_conversations_changed_since_blocking(
            &state,
            &ListUnarchivedConversationsChangedSinceInput { since: None },
        )
        .expect("first sync");
        assert!(
            first
                .changed
                .iter()
                .any(|item| item.conversation_id == conversation.id),
            "首次差量应返回全量基线并包含已落盘会话"
        );
        assert!(first.deleted_ids.is_empty());
        assert!(!first.server_time.is_empty());

        // 基线之后注册单项 watermark → 差量按 id 直读返回该会话。
        let changed_at = overview_register_item_watermark(&conversation.id);
        assert!(!changed_at.is_empty());
        let second = list_unarchived_conversations_changed_since_blocking(
            &state,
            &ListUnarchivedConversationsChangedSinceInput {
                since: Some(first.server_time.clone()),
            },
        )
        .expect("second sync");
        assert!(
            second
                .changed
                .iter()
                .any(|item| item.conversation_id == conversation.id),
            "差量应包含 watermark 注册过的会话且不再全量枚举"
        );
        assert!(
            second
                .deleted_ids
                .iter()
                .all(|id| id != &conversation.id)
        );

        // watermark 声称变了但直读不到（未落盘的幽灵 id）→ 追加 deleted_ids 兜底。
        let ghost_id = "conversation-changed-since-ghost";
        let _ = overview_register_item_watermark(ghost_id);
        let third = list_unarchived_conversations_changed_since_blocking(
            &state,
            &ListUnarchivedConversationsChangedSinceInput {
                since: Some(second.server_time.clone()),
            },
        )
        .expect("third sync");
        assert!(
            third.deleted_ids.iter().any(|id| id == ghost_id),
            "直读不到的变更 id 应进入 deleted_ids 兜底"
        );
        assert!(
            !third
                .changed
                .iter()
                .any(|item| item.conversation_id == ghost_id)
        );

        // 注册删除语义后 → deleted_ids 收敛包含该会话。
        overview_register_missing_item(&conversation.id);
        let fourth = list_unarchived_conversations_changed_since_blocking(
            &state,
            &ListUnarchivedConversationsChangedSinceInput {
                since: Some(third.server_time.clone()),
            },
        )
        .expect("fourth sync");
        assert!(
            fourth.deleted_ids.iter().any(|id| id == &conversation.id),
            "注册 missing 后差量应返回 deleted_ids"
        );
    }

    #[test]
    fn foreground_snapshot_should_read_new_conversation_before_persist_worker_lands() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-foreground-before-persist", "active", &now);
        conversation
            .messages
            .push(test_text_message("assistant", "新建会话首条消息", &now));
        conversation.updated_at = now.clone();
        conversation.last_assistant_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &conversation).expect("schedule persist");

        let meta = conversation_service_v2()
            .get_conversation_meta(&state, &conversation.id)
            .expect("read meta from memory cache");
        let snapshot = build_foreground_conversation_snapshot_from_meta_view(&state, &meta, 4)
            .expect("新建会话尚未落盘时快照应可读，而不是报仓库不存在");
        assert_eq!(snapshot.conversation_id, conversation.id);
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(snapshot.messages[0].id, conversation.messages[0].id);
    }

    #[test]
    fn list_unarchived_conversation_summaries_should_fallback_to_recent_message_when_preview_missing() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-summary-preview-fallback", "active", &now);
        conversation.messages.push(test_text_message("assistant", "这是最新一条压缩后仍应显示的预览", &now));
        conversation.updated_at = now.clone();
        conversation.last_assistant_at = Some(now.clone());
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");

        let store_paths = message_store::message_store_paths(&state.data_path, &conversation.id)
            .expect("message store paths");
        let ready_meta = message_store::chat_store_read_meta(&store_paths)
            .expect("read ready meta")
            .expect("ready meta exists");
        let mut ready_meta_json = serde_json::to_value(&ready_meta).expect("serialize ready meta");
        ready_meta_json["previewMessages"] = serde_json::Value::Array(Vec::new());
        let cleared_meta: message_store::ConversationPersistMeta =
            serde_json::from_value(ready_meta_json).expect("deserialize cleared meta");
        message_store::chat_store_write_meta(&store_paths, &cleared_meta)
            .expect("write cleared meta");

        let summaries = conversation_service_v2()
            .list_unarchived_conversation_summaries(&state)
            .expect("list summaries")
            .summaries;
        let summary = summaries
            .iter()
            .find(|item| item.conversation_id == conversation.id)
            .expect("summary exists");

        assert_eq!(summary.preview_messages.len(), 1);
        assert_eq!(
            summary.preview_messages[0].text_preview,
            "这是最新一条压缩后仍应显示的预览"
        );
    }

    #[test]
    fn state_schedule_conversation_persist_should_preserve_field_level_metadata() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-preserve-model", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark persisted");
        conversation_service_v2()
            .set_conversation_preferred_api_config_id(
                &state,
                &conversation.id,
                Some("api-model-d".to_string()),
            )
            .expect("set preferred model");
        conversation_service_v2()
            .set_conversation_title_metadata(&state, &conversation.id, "字段级标题")
            .expect("set title");
        conversation_service_v2()
            .set_conversation_shell_workspace_metadata(
                &state,
                &conversation.id,
                Some(Some(state.llm_workspace_path.to_string_lossy().to_string())),
                None,
                Some(true),
                None,
            )
            .expect("set workspace metadata");
        conversation_service_v2()
            .set_conversation_lifecycle_metadata(
                &state,
                &conversation.id,
                Some("archived"),
                Some(Some(now.clone())),
                Some(now.clone()),
            )
            .expect("set lifecycle metadata");
        conversation_service_v2()
            .set_conversation_current_todos_metadata(
                &state,
                &conversation.id,
                vec![ConversationTodoItem {
                    content: "字段级 todo".to_string(),
                    status: "in_progress".to_string(),
                }],
            )
            .expect("set todos metadata");
        state_update_conversation_metadata_cached(
            &state,
            &conversation.id,
            |conversation| {
                conversation.agent_id = "字段级agent".to_string();
                conversation.root_conversation_id = Some("字段级root".to_string());
                conversation.conversation_kind = CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string();
                conversation.user_profile_snapshot = "字段级画像".to_string();
                conversation.memory_recall_table = vec!["memory-a".to_string()];
                Ok(())
            },
        )
        .expect("set runtime metadata");

        let mut stale_full_snapshot = conversation.clone();
        stale_full_snapshot.title = "过期标题".to_string();
        stale_full_snapshot.shell_workspace_path = None;
        stale_full_snapshot.shell_autonomous_mode = false;
        stale_full_snapshot.status = "active".to_string();
        stale_full_snapshot.archived_at = None;
        stale_full_snapshot.agent_id = DEFAULT_AGENT_ID.to_string();
        stale_full_snapshot.root_conversation_id = None;
        stale_full_snapshot.conversation_kind = CONVERSATION_KIND_CHAT.to_string();
        stale_full_snapshot.current_todos = Vec::new();
        stale_full_snapshot.user_profile_snapshot.clear();
        stale_full_snapshot.memory_recall_table.clear();
        stale_full_snapshot.messages.push(test_text_message("user", "hello", &now));
        stale_full_snapshot.updated_at = "2020-01-01T00:00:00Z".to_string();
        stale_full_snapshot.last_user_at = Some("2020-01-01T00:00:00Z".to_string());
        state_schedule_conversation_persist(&state, &stale_full_snapshot)
            .expect("schedule stale full persist");

        let cached = state_read_conversation_cached(&state, &conversation.id)
            .expect("read cached conversation");
        assert_eq!(
            cached.preferred_api_config_id.as_deref(),
            Some("api-model-d")
        );
        assert_eq!(cached.title, "字段级标题");
        assert_eq!(
            cached.shell_workspace_path.as_deref(),
            Some(state.llm_workspace_path.to_string_lossy().as_ref())
        );
        assert!(cached.shell_autonomous_mode);
        assert_eq!(cached.status, "archived");
        assert_eq!(cached.archived_at.as_deref(), Some(now.as_str()));
        assert_eq!(cached.current_todos.len(), 1);
        assert_eq!(cached.current_todos[0].content, "字段级 todo");
        assert_eq!(cached.user_profile_snapshot, "字段级画像");
        assert_eq!(cached.memory_recall_table, vec!["memory-a".to_string()]);
        assert_eq!(cached.agent_id, "字段级agent");
        assert_eq!(cached.root_conversation_id.as_deref(), Some("字段级root"));
        assert_eq!(
            cached.conversation_kind,
            CONVERSATION_KIND_REMOTE_IM_CONTACT
        );
        assert_eq!(cached.updated_at, now);
        assert_ne!(
            cached.last_user_at.as_deref(),
            Some("2020-01-01T00:00:00Z")
        );
        assert_eq!(cached.messages.len(), 1);
    }

    #[test]
    fn state_schedule_conversation_persist_should_preserve_metadata_newer_than_pending_full() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation =
            test_chat_conversation("conversation-pending-full-metadata", "active", &now);
        state_schedule_conversation_persist(&state, &conversation)
            .expect("schedule initial full persist");
        conversation_service_v2()
            .set_conversation_preferred_api_config_id(
                &state,
                &conversation.id,
                Some("api-after-full".to_string()),
            )
            .expect("set preferred model after pending full");
        let in_flight_batch = state
            .conversation_persist_pending
            .lock()
            .expect("lock pending persist")
            .take()
            .expect("take pending batch to simulate worker in-flight");
        assert!(in_flight_batch.conversations.contains_key(&conversation.id));

        let mut stale_full_snapshot = conversation.clone();
        stale_full_snapshot
            .messages
            .push(test_text_message("user", "new message", &now));
        state_schedule_conversation_persist(&state, &stale_full_snapshot)
            .expect("schedule stale full persist");

        let cached = state_read_conversation_cached(&state, &conversation.id)
            .expect("read cached conversation");
        assert_eq!(
            cached.preferred_api_config_id.as_deref(),
            Some("api-after-full")
        );
        assert_eq!(cached.messages.len(), 1);
    }

    #[test]
    fn state_schedule_conversation_persist_should_recover_poisoned_metadata_authority_lock() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation =
            test_chat_conversation("conversation-poisoned-metadata-authority", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark persisted");
        conversation_service_v2()
            .set_conversation_preferred_api_config_id(
                &state,
                &conversation.id,
                Some("api-before-poison".to_string()),
            )
            .expect("set preferred model");
        let authority_ids = state.cached_conversation_field_metadata_ids.clone();
        let _ = std::panic::catch_unwind(move || {
            let _guard = authority_ids.lock().expect("lock authority ids");
            panic!("poison metadata authority lock");
        });

        let mut stale_full_snapshot = conversation.clone();
        stale_full_snapshot
            .messages
            .push(test_text_message("user", "new message", &now));
        state_schedule_conversation_persist(&state, &stale_full_snapshot)
            .expect("poisoned authority lock should recover");

        let cached = state_read_conversation_cached(&state, &conversation.id)
            .expect("read cached conversation");
        assert_eq!(
            cached.preferred_api_config_id.as_deref(),
            Some("api-before-poison")
        );
        assert_eq!(cached.messages.len(), 1);
    }

    #[test]
    fn state_schedule_conversation_persist_should_not_decrease_cumulative_usage() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-usage-add-only", "active", &now);
        state_schedule_conversation_persist(&state, &conversation).expect("schedule persist");
        conversation_service()
            .add_conversation_cumulative_usage_delta(
                &state,
                &conversation.id,
                None,
                None,
                &serde_json::json!({
                    "completionTokens": 20,
                    "cachedTokens": 100,
                    "cacheCreationTokens": 5
                }),
            )
            .expect("add cumulative usage");

        let mut stale_full_snapshot = conversation.clone();
        stale_full_snapshot
            .messages
            .push(test_text_message("user", "hello", &now));
        state_schedule_conversation_persist(&state, &stale_full_snapshot)
            .expect("schedule stale full persist");

        let cached = state_read_conversation_cached(&state, &conversation.id)
            .expect("read cached conversation");
        assert_eq!(cached.cumulative_usage.output_tokens, 20);
        assert_eq!(cached.cumulative_usage.cache_read_tokens, 100);
        assert_eq!(cached.cumulative_usage.cache_write_tokens, 5);
        assert_eq!(
            conversation_cumulative_usage_weighted_tokens(&cached.cumulative_usage),
            47
        );
        assert_eq!(cached.messages.len(), 1);
    }

    #[test]
    fn state_mark_conversation_direct_persisted_should_update_memory_chat_index_only() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-direct-upsert", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        conversation.status = "archived".to_string();
        conversation.archived_at = Some(now.clone());
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        let chat_index = state_read_chat_index_cached(&state).expect("read memory chat index");
        assert_eq!(chat_index.conversations.len(), 1);
        assert_eq!(chat_index.conversations[0].id, conversation.id);
        assert!(!app_layout_chat_dir(&state.data_path).join("index.json").exists());
    }

    #[test]
    fn switch_active_conversation_snapshot_should_not_schedule_metadata_only_full_persist() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-switch-snapshot", "active", &now);
        conversation.messages.push(test_text_message("user", "第一条", &now));
        conversation
            .messages
            .push(test_text_message("assistant", "最新回复", &now));
        conversation.updated_at = now.clone();
        conversation.last_user_at = Some(now.clone());
        conversation.last_assistant_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        let result = conversation_service_v2()
            .switch_active_conversation_snapshot(
                &state,
                &SwitchActiveConversationSnapshotInput {
                    conversation_id: Some(conversation.id.clone()),
                    agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                },
            )
            .expect("switch active snapshot");
        assert_eq!(result.snapshot.messages.len(), 2);

        let persisted = state_read_conversation_cached(&state, &conversation.id)
            .expect("read persisted conversation");
        assert_eq!(persisted.messages.len(), 2);
        match &persisted.messages[1].parts[0] {
            MessagePart::Text { text, .. } => assert_eq!(text, "最新回复"),
            _ => panic!("expected assistant text message"),
        }
    }

    #[tokio::test]
    async fn append_message_to_unarchived_conversation_should_preserve_existing_shard_meta() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-append-meta", "active", &now);
        conversation
            .messages
            .push(test_text_message("user", "第一条", &now));
        conversation
            .messages
            .push(test_text_message("assistant", "第二条", &now));
        conversation.updated_at = now.clone();
        conversation.last_user_at = Some(now.clone());
        conversation.last_assistant_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        let appended = test_text_message("assistant", "第三条", &now);
        conversation_service_v2()
            .append_message_to_unarchived_conversation(&state, &conversation.id, &appended)
            .await
            .expect("append message");

        let meta = message_store::chat_store_read_meta(&store_paths)
            .expect("read store meta")
            .expect("store meta exists");
        assert_eq!(meta.message_count(), 3);
        assert_eq!(meta.body_message_count(), 3);
        assert_eq!(meta.last_message_id(), Some(appended.id.as_str()));
        assert!(meta.has_assistant_reply());

        let stored_messages = message_store::chat_store_read_all_messages(&store_paths)
            .expect("read stored messages")
            .expect("stored messages exist");
        assert_eq!(stored_messages.len(), 3);
        match &stored_messages[2].parts[0] {
            MessagePart::Text { text, .. } => assert_eq!(text, "第三条"),
            _ => panic!("expected assistant text message"),
        }
    }

    #[tokio::test]
    async fn append_message_should_not_overwrite_history_when_cached_meta_message_count_is_zero() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-append-no-full-rewrite", "active", &now);
        conversation
            .messages
            .push(test_text_message("user", "第一条历史", &now));
        conversation
            .messages
            .push(test_text_message("assistant", "第二条历史", &now));
        conversation.updated_at = now.clone();
        conversation.last_user_at = Some(now.clone());
        conversation.last_assistant_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        state_update_conversation_metadata_cached(&state, &conversation.id, |cached| {
            cached.updated_at = now.clone();
            Ok(())
        })
        .expect("prime metadata pending");
        {
            let mut cached = state
                .cached_conversation_metadata
                .lock()
                .expect("lock cached conversation metadata");
            let current = cached
                .get(&conversation.id)
                .cloned()
                .expect("cached meta exists");
            let broken_conversation = conversation_service_v2()
                .build_conversation_snapshot_from_meta(&current, Vec::new());
            let broken = message_store::ConversationShardMeta::from_conversation(
                &broken_conversation,
            );
            cached.insert(conversation.id.clone(), broken);
        }

        let appended = test_text_message("assistant", "第三条新消息", &now);
        conversation_service_v2()
            .append_message_to_unarchived_conversation(&state, &conversation.id, &appended)
            .await
            .expect("append message after broken cached meta");

        let stored_messages = message_store::chat_store_read_all_messages(&store_paths)
            .expect("read stored messages")
            .expect("stored messages exist");
        assert_eq!(stored_messages.len(), 3);
        match &stored_messages[0].parts[0] {
            MessagePart::Text { text, .. } => assert_eq!(text, "第一条历史"),
            _ => panic!("expected first historical text message"),
        }
        match &stored_messages[1].parts[0] {
            MessagePart::Text { text, .. } => assert_eq!(text, "第二条历史"),
            _ => panic!("expected second historical text message"),
        }
        match &stored_messages[2].parts[0] {
            MessagePart::Text { text, .. } => assert_eq!(text, "第三条新消息"),
            _ => panic!("expected appended text message"),
        }

        let ready_meta = message_store::chat_store_read_meta(&store_paths)
            .expect("read ready meta")
            .expect("ready meta exists");
        assert_eq!(ready_meta.message_count(), 3);
        assert_eq!(ready_meta.preview_messages().len(), 2);
        assert_eq!(
            ready_meta.preview_messages()[1].text_preview,
            "第三条新消息"
        );
    }

    #[test]
    fn conversation_service_v2_metadata_update_should_preserve_ready_store_message_stats() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-v2-meta-preserve", "active", &now);
        conversation
            .messages
            .push(test_text_message("user", "第一条", &now));
        conversation
            .messages
            .push(test_text_message("assistant", "第二条", &now));
        conversation.updated_at = now.clone();
        conversation.last_user_at = Some(now.clone());
        conversation.last_assistant_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        let updated = conversation_service_v2()
            .set_title(&state, &conversation.id, "V2字段级标题")
            .expect("set title through v2");
        assert_eq!(updated.title, "V2字段级标题");

        {
            let pending = state
                .conversation_persist_pending
                .lock()
                .expect("lock pending before flush");
            let pending = pending.as_ref().expect("pending metadata persist before flush");
            assert!(pending.conversations.is_empty());
            assert!(pending.metadata_conversation_ids.contains(&conversation.id));
        }

        flush_pending_persists_blocking(&state).expect("flush metadata persist");

        let meta = message_store::chat_store_read_meta(&store_paths)
            .expect("read ready store meta")
            .expect("ready store meta exists");
        assert_eq!(meta.message_count(), 2);
        assert_eq!(meta.body_message_count(), 2);
        assert_eq!(meta.title(), "V2字段级标题");
    }

    #[test]
    fn state_update_conversation_metadata_cached_should_preserve_cached_preview_messages() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-meta-preview-preserve", "active", &now);
        conversation
            .messages
            .push(test_text_message("user", "第一条用户消息", &now));
        conversation
            .messages
            .push(test_text_message("assistant", "第二条助手预览", &now));
        conversation.updated_at = now.clone();
        conversation.last_user_at = Some(now.clone());
        conversation.last_assistant_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");
        flush_pending_persists_blocking(&state).expect("flush full persist");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        state_update_conversation_metadata_cached(&state, &conversation.id, |cached| {
            cached.title = "只改标题".to_string();
            Ok(())
        })
        .expect("metadata update");

        let meta = state_read_conversation_metadata_cached(&state, &conversation.id)
            .expect("read cached meta");
        assert_eq!(meta.message_count(), 2);
        assert_eq!(meta.body_message_count(), 2);
        assert_eq!(meta.preview_messages().len(), 2);
        assert_eq!(
            meta.preview_messages()[1].text_preview,
            "第二条助手预览"
        );
    }

    #[test]
    fn state_read_conversation_metadata_cached_should_repair_empty_cached_preview_from_ready_store() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-meta-preview-repair", "active", &now);
        conversation
            .messages
            .push(test_text_message("assistant", "缓存坏了也要修回来", &now));
        conversation.updated_at = now.clone();
        conversation.last_assistant_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");
        flush_pending_persists_blocking(&state).expect("flush full persist");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        {
            let mut cached = state
                .cached_conversation_metadata
                .lock()
                .expect("lock cached conversation metadata");
            let current = cached
                .get(&conversation.id)
                .cloned()
                .expect("cached meta exists");
            let broken_conversation = conversation_service_v2()
                .build_conversation_snapshot_from_meta(&current, Vec::new());
            let broken = message_store::ConversationShardMeta::from_conversation(
                &broken_conversation,
            );
            cached.insert(conversation.id.clone(), broken);
        }

        let repaired = state_read_conversation_metadata_cached(&state, &conversation.id)
            .expect("read repaired meta");
        assert_eq!(repaired.message_count(), 1);
        assert_eq!(repaired.preview_messages().len(), 1);
        assert_eq!(
            repaired.preview_messages()[0].text_preview,
            "缓存坏了也要修回来"
        );
    }

    #[test]
    fn conversation_service_v2_should_read_conversation_meta_view() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-v2-meta-read", "active", &now);
        conversation.title = "Meta读取标题".to_string();
        conversation.agent_id = DEFAULT_AGENT_ID.to_string();
        conversation.current_todos = vec![ConversationTodoItem {
            content: "检查 meta view".to_string(),
            status: "pending".to_string(),
        }];
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

        let meta = conversation_service_v2()
            .get_conversation_meta(&state, &conversation.id)
            .expect("read v2 meta");

        assert_eq!(meta.id, conversation.id);
        assert_eq!(meta.title, "Meta读取标题");
        assert_eq!(meta.agent_id, DEFAULT_AGENT_ID);
        assert_eq!(meta.current_todos.len(), 1);
    }

    #[test]
    fn assistant_delta_broadcast_conversation_title_should_return_title_for_local_chat() {
        // 钉死：本地普通会话广播 assistantDelta 时附带会话标题（供远程前端通知对齐）。
        // 标题遵循用户配置的 ui_language：这里设为 en-US，构造无标题且时间解析失败的
        // 会话，标题应走 "Untitled conversation" 英文兜底，验证标题生成使用用户配置语言。
        let state = test_chat_runtime_state();
        let mut config = AppConfig::default();
        config.ui_language = "en-US".to_string();
        state_write_config_cached(&state, &config).expect("write config");

        let mut conversation = test_chat_conversation(
            "title-local-chat",
            "active",
            "not-a-valid-rfc3339-time",
        );
        conversation.title = String::new();
        conversation.agent_id = DEFAULT_AGENT_ID.to_string();
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

        let title = assistant_delta_broadcast_conversation_title(&state, &conversation.id);
        assert!(title.is_some(), "本地普通会话应返回会话标题");
        assert!(
            title.unwrap().contains("Untitled conversation"),
            "en-US 配置下无标题会话的标题应走英文兜底文案"
        );
    }

    #[test]
    fn assistant_delta_broadcast_conversation_title_should_return_none_for_non_local_chat() {
        // 钉死：delegate / remote-IM / system-notification 会话不参与本地通知标题，
        // 广播不应附带标题。逐一覆盖判定函数列出的全部过滤分支。
        let state = test_chat_runtime_state();
        let now = now_iso();
        for (conversation_id, kind) in [
            ("title-delegate", CONVERSATION_KIND_DELEGATE),
            ("title-remote-im", CONVERSATION_KIND_REMOTE_IM_CONTACT),
            ("title-system-notification", CONVERSATION_KIND_SYSTEM_NOTIFICATION),
        ] {
            let mut conversation = test_chat_conversation(conversation_id, "active", &now);
            conversation.title = format!("{conversation_id}-标题");
            conversation.conversation_kind = kind.to_string();
            state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

            let title = assistant_delta_broadcast_conversation_title(&state, &conversation.id);
            assert!(
                title.is_none(),
                "{kind} 会话不应返回本地通知标题，conversation_id={conversation_id}"
            );
        }
    }

    #[test]
    fn assistant_delta_broadcast_conversation_title_should_return_none_for_missing_conversation() {
        // 钉死：会话元数据读取失败时广播不附带标题，不能 panic。
        let state = test_chat_runtime_state();
        let title = assistant_delta_broadcast_conversation_title(
            &state,
            "nonexistent-conversation-id",
        );
        assert!(title.is_none(), "读取失败的会话不应返回标题");
    }

    #[test]
    fn conversation_service_v2_should_read_messages_before_and_after_anchor() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation("conversation-v2-page-read", "active", &now);
        let mut m1 = test_text_message("user", "第一条", &now);
        m1.id = "msg-1".to_string();
        let mut m2 = test_text_message("assistant", "第二条", &now);
        m2.id = "msg-2".to_string();
        let mut m3 = test_text_message("user", "第三条", &now);
        m3.id = "msg-3".to_string();
        let mut m4 = test_text_message("assistant", "第四条", &now);
        m4.id = "msg-4".to_string();
        conversation.messages = vec![m1, m2, m3, m4];
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");
        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        let before = conversation_service_v2()
            .get_messages_before(&state, &conversation.id, "msg-4", 2)
            .expect("read messages before");
        assert_eq!(before.messages.len(), 2);
        assert_eq!(before.messages[0].id, "msg-2");
        assert_eq!(before.messages[1].id, "msg-3");
        assert!(before.has_more);
        assert!(before.has_more_before);
        assert!(!before.has_more_after);
        assert_eq!(before.first_message_id.as_deref(), Some("msg-2"));
        assert_eq!(before.last_message_id.as_deref(), Some("msg-3"));

        let after = conversation_service_v2()
            .get_messages_after(&state, &conversation.id, "msg-1", 2)
            .expect("read messages after");
        assert_eq!(after.messages.len(), 2);
        assert_eq!(after.messages[0].id, "msg-2");
        assert_eq!(after.messages[1].id, "msg-3");
        assert!(after.has_more);
        assert!(!after.has_more_before);
        assert!(after.has_more_after);
        assert_eq!(after.first_message_id.as_deref(), Some("msg-2"));
        assert_eq!(after.last_message_id.as_deref(), Some("msg-3"));
    }

    #[tokio::test]
    async fn conversation_service_v2_should_create_and_delete_conversation() {
        let state = test_chat_runtime_state();
        let git_init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&state.llm_workspace_path)
            .output()
            .expect("initialize git workspace");
        assert!(git_init.status.success(), "git init should succeed");
        let created = conversation_service_v2()
            .create_conversation(
                &state,
                &CreateUnarchivedConversationInput {
                    api_config_id: None,
                    agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                    title: Some("V2创建会话".to_string()),
                    copy_source_conversation_id: None,
                    shell_workspaces: None,
                    shell_work_mode: Some("isolated_worktree".to_string()),
                    shell_work_branch: None,
                    shell_autonomous_mode: None,
                    is_draft: Some(false),
                },
            )
            .expect("create conversation through v2");

        let created_conversation = state_read_conversation_cached(&state, &created.conversation_id)
            .expect("created conversation should exist");
        assert_eq!(created_conversation.title, "V2创建会话");
        assert_eq!(created_conversation.agent_id, DEFAULT_AGENT_ID);
        assert_eq!(created_conversation.shell_work_mode, "worktree");
        assert_eq!(created_conversation.shell_workspace_path, None);
        assert_eq!(created_conversation.shell_workspaces.len(), 1);
        assert_eq!(
            created_conversation.shell_workspaces[0].level,
            SHELL_WORKSPACE_LEVEL_MAIN
        );
        assert_eq!(
            created_conversation.shell_workspaces[0].path,
            terminal_path_for_user(&state.llm_workspace_path)
        );
        assert_eq!(
            created_conversation.shell_workspaces[0].access,
            SHELL_WORKSPACE_ACCESS_FULL_ACCESS
        );
        let store_paths = message_store::message_store_paths(
            &state.data_path,
            &created.conversation_id,
        )
        .expect("created conversation store paths");
        let immediate_message = test_text_message("user", "创建后立即发送", &now_iso());
        conversation_service_v2()
            .append_message_to_unarchived_conversation(
                &state,
                &created.conversation_id,
                &immediate_message,
            )
            .await
            .expect("new conversation should accept an immediate first message");
        assert!(message_store::chat_store_read_message_by_id(
            &store_paths,
            &immediate_message.id,
        )
        .expect("read immediate first message")
        .is_some());
        assert!(!state
            .cached_conversation_dirty_ids
            .lock()
            .expect("read dirty ids after immediate append")
            .contains(&created.conversation_id));
        if let Some(pending) = state
            .conversation_persist_pending
            .lock()
            .expect("read pending persists after immediate append")
            .as_ref()
        {
            assert!(!pending.conversations.contains_key(&created.conversation_id));
            assert!(!pending
                .metadata_conversation_ids
                .contains(&created.conversation_id));
        }

        let legacy_collision_id = "conversation-v2-create-with-legacy-artifact";
        let mut legacy_collision = test_chat_conversation(legacy_collision_id, "active", &now_iso());        legacy_collision.title = "生产新建忽略旧 artifact".to_string();
        let legacy_path =
            app_layout_chat_conversation_path(&state.data_path, legacy_collision_id);
        let collision_paths = message_store::message_store_paths(
            &state.data_path,
            legacy_collision_id,
        )
        .expect("collision store paths");
        let collision_shard_dir =
            app_layout_chat_conversations_dir(&state.data_path).join(legacy_collision_id);
        let collision_manifest = collision_shard_dir
            .join(message_store::MESSAGE_STORE_MANIFEST_FILE_NAME);
        let collision_legacy_block = collision_shard_dir
            .join(message_store::MESSAGE_STORE_BLOCKS_DIR_NAME)
            .join("000000.jsonl");
        fs::create_dir_all(&collision_shard_dir).expect("create V2 artifact directory");
        fs::create_dir_all(
            collision_legacy_block
                .parent()
                .expect("legacy block parent"),
        )
        .expect("create V2 block directory");
        fs::write(&legacy_path, "{broken legacy source")
            .expect("write V1 artifact");
        fs::write(&collision_manifest, "{broken V2 manifest")
            .expect("write V2 artifact");
        fs::write(&collision_legacy_block, "legacy V2 block")
            .expect("write V2 block artifact");
        let legacy_before = fs::read(&legacy_path).expect("read V1 artifact before create");
        let manifest_before = fs::read(&collision_manifest).expect("read V2 artifact before create");
        let block_before =
            fs::read(&collision_legacy_block).expect("read V2 block before create");

        state_schedule_conversation_persist(&state, &legacy_collision)
            .expect("schedule new V3 conversation over ignored old artifacts");
        flush_pending_persists_blocking(&state)
            .expect("publish scheduled V3 conversation");

        assert!(message_store::chat_store_read_status(&collision_paths)
            .expect("read collision V3 status")
            .is_some());
        assert_eq!(
            fs::read(&legacy_path).expect("V1 artifact remains after create"),
            legacy_before
        );
        assert_eq!(
            fs::read(&collision_manifest).expect("V2 artifact remains after create"),
            manifest_before
        );
        assert_eq!(
            fs::read(&collision_legacy_block).expect("V2 block remains after create"),
            block_before
        );

        let deleted = conversation_service_v2()
            .delete_conversation(&state, &created.conversation_id)
            .expect("delete conversation through v2");
        assert_eq!(deleted.deleted_conversation_id, created.conversation_id);
    }

    #[test]
    fn conversation_service_v2_should_create_draft_with_fallback_defaults_and_find_it() {
        let state = test_chat_runtime_state();
        let git_init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&state.llm_workspace_path)
            .output()
            .expect("initialize git workspace");
        assert!(git_init.status.success(), "git init should succeed");

        let created = conversation_service_v2()
            .create_conversation(
                &state,
                &CreateUnarchivedConversationInput {
                    api_config_id: None,
                    agent_id: None,
                    title: None,
                    copy_source_conversation_id: None,
                    shell_workspaces: None,
                    shell_work_mode: None,
                    shell_work_branch: None,
                    shell_autonomous_mode: None,
                    is_draft: Some(true),
                },
            )
            .expect("create draft with fallback defaults");

        let draft = state_read_conversation_cached(&state, &created.conversation_id)
            .expect("draft conversation should exist");
        assert!(draft.is_draft, "draft flag should be set on creation");
        assert!(draft.title.is_empty(), "draft title should stay empty");
        assert!(!draft.agent_id.is_empty(), "draft agent should fall back to default");

        let meta = conversation_service_v2()
            .get_conversation_meta(&state, &created.conversation_id)
            .expect("read draft meta view");
        assert!(meta.is_draft, "is_draft must survive the shard meta roundtrip");

        let found =
            find_existing_draft_conversation_id(&state).expect("find existing draft");
        assert_eq!(
            found.as_deref(),
            Some(created.conversation_id.as_str()),
            "singleton query should locate the only unarchived draft"
        );
    }

    #[test]
    fn conversation_service_v2_should_create_draft_with_current_assistant_persona_outside_org() {
        let state = test_chat_runtime_state();
        let git_init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&state.llm_workspace_path)
            .output()
            .expect("initialize git workspace");
        assert!(git_init.status.success(), "git init should succeed");

        let config = AppConfig::default();
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(
            &state,
            &[
                {
                    let mut agent = default_agent();
                    agent.id = "persona-outside-department".to_string();
                    agent.name = "游离人格".to_string();
                    agent
                },
                default_agent(),
                default_user_persona(),
            ],
        )
        .expect("write agents");
        state_service_set_assistant_agent_id(&state, "persona-outside-department")
            .expect("write assistant department agent id");

        let created = conversation_service_v2()
            .create_conversation(
                &state,
                &CreateUnarchivedConversationInput {
                    api_config_id: None,
                    agent_id: None,
                    title: None,
                    copy_source_conversation_id: None,
                    shell_workspaces: None,
                    shell_work_mode: None,
                    shell_work_branch: None,
                    shell_autonomous_mode: None,
                    is_draft: Some(true),
                },
            )
            .expect("create draft with current assistant persona");

        let draft = state_read_conversation_cached(&state, &created.conversation_id)
            .expect("draft conversation should exist");
        assert_eq!(
            draft.agent_id, "persona-outside-department",
            "draft must use the current assistant persona even when it has no structural relationship to the assistant root"
        );
    }

    #[test]
    fn conversation_service_v2_should_keep_normal_create_non_draft() {
        let state = test_chat_runtime_state();
        let git_init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&state.llm_workspace_path)
            .output()
            .expect("initialize git workspace");
        assert!(git_init.status.success(), "git init should succeed");

        let created = conversation_service_v2()
            .create_conversation(
                &state,
                &CreateUnarchivedConversationInput {
                    api_config_id: None,
                    agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                    title: None,
                    copy_source_conversation_id: None,
                    shell_workspaces: None,
                    shell_work_mode: None,
                    shell_work_branch: None,
                    shell_autonomous_mode: None,
                    is_draft: None,
                },
            )
            .expect("create normal conversation");

        let conversation = state_read_conversation_cached(&state, &created.conversation_id)
            .expect("conversation should exist");
        assert!(!conversation.is_draft, "normal create must not set draft flag");
        let found =
            find_existing_draft_conversation_id(&state).expect("find draft after normal create");
        assert!(found.is_none(), "normal conversation must not be treated as draft");
    }

    #[tokio::test]
    async fn conversation_service_v2_should_promote_draft_on_first_user_message_and_create_next_draft()
    {
        let state = test_chat_runtime_state();
        let git_init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&state.llm_workspace_path)
            .output()
            .expect("initialize git workspace");
        assert!(git_init.status.success(), "git init should succeed");

        let created = conversation_service_v2()
            .create_conversation(
                &state,
                &CreateUnarchivedConversationInput {
                    api_config_id: None,
                    agent_id: None,
                    title: None,
                    copy_source_conversation_id: None,
                    shell_workspaces: None,
                    shell_work_mode: None,
                    shell_work_branch: None,
                    shell_autonomous_mode: None,
                    is_draft: Some(true),
                },
            )
            .expect("create draft with fallback defaults");

        let now = now_iso();
        conversation_service_v2()
            .append_user_message(
                &state,
                &UserMessageAppendInput {
                    conversation_id: created.conversation_id.clone(),
                    message: test_text_message("user", "第一条消息，转正", &now),
                    memory_recall_ids: Vec::new(),
                },
            )
            .await
            .expect("append user message should promote draft");

        let promoted = state_read_conversation_cached(&state, &created.conversation_id)
            .expect("read promoted conversation");
        assert!(
            !promoted.is_draft,
            "draft must be cleared after first user message is persisted"
        );
        let promoted_meta = conversation_service_v2()
            .get_conversation_meta(&state, &created.conversation_id)
            .expect("read promoted meta view");
        assert!(
            !promoted_meta.is_draft,
            "is_draft=false must be written back to storage meta"
        );

        let next_draft = find_existing_draft_conversation_id(&state)
            .expect("find next draft after promotion");
        assert!(
            next_draft.is_some() && next_draft.as_deref() != Some(created.conversation_id.as_str()),
            "a fresh backup draft should exist after promotion"
        );
        if let Some(next_draft_id) = next_draft {
            let next_meta = conversation_service_v2()
                .get_conversation_meta(&state, &next_draft_id)
                .expect("read next draft meta");
            assert!(next_meta.is_draft, "backup draft must keep is_draft=true");
            assert_eq!(
                next_meta.agent_id, promoted_meta.agent_id,
                "backup draft should inherit agent"
            );
        }
    }

    #[test]
    fn conversation_service_v2_should_reject_unknown_draft_agent() {
        let state = test_chat_runtime_state();
        let git_init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&state.llm_workspace_path)
            .output()
            .expect("initialize git workspace");
        assert!(git_init.status.success(), "git init should succeed");

        // 草稿人格必须存在于当前组织：不存在的人格必须拒绝
        let result = validate_draft_agent(&state, "agent-not-exists");
        assert!(result.is_err(), "unknown agent must be rejected");
    }

    #[test]
    fn conversation_service_v2_should_create_independent_worktree_conversation() {
        let state = test_chat_runtime_state();
        let git_init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&state.llm_workspace_path)
            .output()
            .expect("initialize git workspace");
        assert!(git_init.status.success(), "git init should succeed");

        let created = conversation_service_v2()
            .create_conversation(
                &state,
                &CreateUnarchivedConversationInput {
                    api_config_id: None,
                    agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                    title: Some("独立工作树会话".to_string()),
                    copy_source_conversation_id: None,
                    shell_workspaces: None,
                    shell_work_mode: Some(SHELL_WORK_MODE_INDEPENDENT_WORKTREE.to_string()),
                    shell_work_branch: None,
                    shell_autonomous_mode: None,
                    is_draft: Some(false),
                },
            )
            .expect("create independent worktree conversation");

        let conversation = state_read_conversation_cached(&state, &created.conversation_id)
            .expect("created conversation should exist");
        assert_eq!(
            conversation.shell_work_mode,
            SHELL_WORK_MODE_WORKTREE
        );
    }

    #[test]
    fn conversation_service_v2_should_reject_isolated_worktree_for_read_only_workspace() {
        let state = test_chat_runtime_state();
        let git_init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&state.llm_workspace_path)
            .output()
            .expect("initialize git workspace");
        assert!(git_init.status.success(), "git init should succeed");

        let result = conversation_service_v2().create_conversation(
            &state,
            &CreateUnarchivedConversationInput {
                api_config_id: None,
                agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                title: Some("只读隔离会话".to_string()),
                copy_source_conversation_id: None,
                shell_workspaces: Some(vec![ShellWorkspaceConfig {
                    id: "main-workspace".to_string(),
                    name: "测试 Git 根".to_string(),
                    path: terminal_path_for_user(&state.llm_workspace_path),
                    level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
                    access: SHELL_WORKSPACE_ACCESS_READ_ONLY.to_string(),
                    built_in: false,
                }]),
                shell_work_mode: Some(SHELL_WORK_MODE_ISOLATED_WORKTREE.to_string()),
                shell_work_branch: None,
                shell_autonomous_mode: None,
                is_draft: Some(false),
            },
        );

        match result {
            Ok(_) => panic!("read-only workspace must reject isolated worktree mode"),
            Err(error) => assert_eq!(error, "工作树至少需要审批权限。"),
        }
    }

    #[test]
    fn conversation_service_v2_should_reject_independent_worktree_for_read_only_workspace() {
        let state = test_chat_runtime_state();
        let git_init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&state.llm_workspace_path)
            .output()
            .expect("initialize git workspace");
        assert!(git_init.status.success(), "git init should succeed");

        let result = conversation_service_v2().create_conversation(
            &state,
            &CreateUnarchivedConversationInput {
                api_config_id: None,
                agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                title: Some("只读独立工作树会话".to_string()),
                copy_source_conversation_id: None,
                shell_workspaces: Some(vec![ShellWorkspaceConfig {
                    id: "main-workspace".to_string(),
                    name: "测试 Git 根".to_string(),
                    path: terminal_path_for_user(&state.llm_workspace_path),
                    level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
                    access: SHELL_WORKSPACE_ACCESS_READ_ONLY.to_string(),
                    built_in: false,
                }]),
                shell_work_mode: Some(SHELL_WORK_MODE_INDEPENDENT_WORKTREE.to_string()),
                shell_work_branch: None,
                shell_autonomous_mode: None,
                is_draft: Some(false),
            },
        );

        match result {
            Ok(_) => panic!("read-only workspace must reject independent worktree mode"),
            Err(error) => assert_eq!(error, "工作树至少需要审批权限。"),
        }
    }

    #[test]
    fn isolated_worktree_should_require_git_repository_root() {
        let temp_root = std::env::temp_dir().join(format!(
            "eca-isolated-worktree-root-test-{}",
            Uuid::new_v4()
        ));
        let git_root = temp_root.join("repo");
        let nested_path = git_root.join("nested");
        let plain_path = temp_root.join("plain");
        std::fs::create_dir_all(&nested_path).expect("create nested Git path");
        std::fs::create_dir_all(&plain_path).expect("create plain path");
        let git_init = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&git_root)
            .output()
            .expect("initialize Git repository");
        assert!(git_init.status.success(), "git init should succeed");

        assert!(validate_isolated_worktree_root(&git_root.to_string_lossy()).is_ok());
        let nested_error = validate_isolated_worktree_root(&nested_path.to_string_lossy())
            .expect_err("Git repository subdirectory must be rejected");
        assert!(nested_error.contains("必须选择 Git 仓库根目录"));
        let plain_error = validate_isolated_worktree_root(&plain_path.to_string_lossy())
            .expect_err("non-Git directory must be rejected");
        assert!(plain_error.contains("需要 Git 仓库根目录"));

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn build_unarchived_conversation_record_from_runtime_should_refresh_profile_snapshot() {
        let state = test_chat_runtime_state();
        memory_store_upsert_drafts(
            &state.data_path,
            &[MemoryDraftInput {
                memory_type: "knowledge".to_string(),
                judgment: "本地用户长期偏好结构化回复".to_string(),
                reasoning: "创建会话前已存在画像记忆".to_string(),
                tags: vec![
                    "本地用户".to_string(),
                    USER_PERSONA_ID.to_string(),
                    "用户要求".to_string(),
                ],
                owner_agent_id: None,
            }],
        )
        .expect("seed profile memory");
        let agents = state_read_agents_cached(&state).expect("read agents");
        let assistant_agent_id =
            state_service_get_assistant_agent_id(&state).expect("read agent id");

        let conversation = build_unarchived_conversation_record_from_runtime(
            &state.data_path,
            &agents,
            &assistant_agent_id,
            "api-1",
            DEFAULT_AGENT_ID,
            "新会话",
        );

        assert!(conversation.user_profile_snapshot.contains("[id:"));
        assert!(conversation.user_profile_snapshot.contains("本地用户长期偏好结构化回复"));
    }

    #[test]
    fn conversation_service_v2_should_allow_import_snapshot_via_privileged_method() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-v2-import-overwrite", "active", &now);
        conversation.title = "导入后的会话".to_string();
        conversation.current_todos = vec![ConversationTodoItem {
            content: "导入待办".to_string(),
            status: "in_progress".to_string(),
        }];
        conversation.messages.push(test_text_message("user", "第一条导入消息", &now));
        conversation
            .messages
            .push(test_text_message("assistant", "第二条导入消息", &now));
        conversation.updated_at = now.clone();
        conversation.last_user_at = Some(now.clone());
        conversation.last_assistant_at = Some(now.clone());

        conversation_service_v2()
            .import_conversation_snapshot(
                &state,
                "import-job-test",
                "test_import",
                "测试导入覆写",
                &conversation,
            )
            .expect("privileged import overwrite should succeed");

        let cached = state_read_conversation_cached(&state, &conversation.id)
            .expect("conversation should be cached after import overwrite");
        assert_eq!(cached.title, "导入后的会话");
        assert_eq!(cached.messages.len(), 2);
        assert_eq!(cached.current_todos.len(), 1);

        flush_pending_persists_blocking(&state).expect("flush imported conversation");
        let persisted = conversation_service_v2()
            .read_persisted_conversation(&state, &conversation.id)
            .expect("read persisted imported conversation");
        assert_eq!(persisted.title, "导入后的会话");
        assert_eq!(persisted.messages.len(), 2);
    }

    #[test]
    fn conversation_service_v2_should_allow_export_sync_snapshot_via_privileged_method() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-v2-export-sync-overwrite", "active", &now);
        conversation.title = "同步回灌后的会话".to_string();
        conversation.messages.push(test_text_message("user", "同步消息A", &now));
        conversation.messages.push(test_text_message("assistant", "同步消息B", &now));

        conversation_service_v2()
            .sync_replace_conversation_snapshot(
                &state,
                "sync-job-test",
                "test_sync",
                "测试导出同步回灌",
                &conversation,
            )
            .expect("privileged export sync overwrite should succeed");

        let cached = state_read_conversation_cached(&state, &conversation.id)
            .expect("conversation should be cached after sync overwrite");
        assert_eq!(cached.title, "同步回灌后的会话");
        assert_eq!(cached.messages.len(), 2);

        flush_pending_persists_blocking(&state).expect("flush sync conversation");
        let persisted = conversation_service_v2()
            .read_persisted_conversation(&state, &conversation.id)
            .expect("read persisted sync conversation");
        assert_eq!(persisted.title, "同步回灌后的会话");
        assert_eq!(persisted.messages.len(), 2);
    }

    #[test]
    fn conversation_service_v2_should_reject_privileged_overwrite_without_audit_fields() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation =
            test_chat_conversation("conversation-v2-invalid-audit", "active", &now);

        let err = conversation_service_v2()
            .import_conversation_snapshot(&state, "", "operator", "reason", &conversation)
            .expect_err("missing job id should be rejected");
        assert!(err.contains("jobId"));

        let err = conversation_service_v2()
            .sync_replace_conversation_snapshot(&state, "job", "", "reason", &conversation)
            .expect_err("missing operator should be rejected");
        assert!(err.contains("operator"));
    }

    fn test_v2_single_tool_group_result(call_id: &str, tool_name: &str) -> (Value, Value) {
        let assistant_tool_event = serde_json::json!({
            "role": "assistant",
            "reasoning_content": "先调用工具",
            "tool_calls": [{
                "id": call_id,
                "type": "function",
                "function": {
                    "name": tool_name,
                    "arguments": "{\"path\":\"README.md\"}"
                }
            }]
        });
        let tool_result_event = serde_json::json!({
            "role": "tool",
            "tool_call_id": call_id,
            "content": "tool result"
        });
        (assistant_tool_event, tool_result_event)
    }

    #[test]
    fn conversation_service_v2_should_forbid_tool_append_after_final_text_committed() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-v2-tool-append-closed", "active", &now);
        let mut assistant = test_text_message("assistant", "已经有最终正文", &now);
        assistant.id = "assistant-final".to_string();
        assistant.speaker_agent_id = Some(DEFAULT_AGENT_ID.to_string());
        conversation.messages.push(assistant);
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");
        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        let (assistant_tool_event, tool_result_event) =
            test_v2_single_tool_group_result("call-v2-1", "read_file");
        let err = conversation_service_v2()
            .append_tool_event_to_assistant_message(
                &state,
                &AssistantMessageToolAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-final".to_string(),
                    assistant_tool_event,
                    tool_result_event,
                },
            )
            .expect_err("final text should close tool append");

        assert!(err.contains("MSG_TOOL_APPEND_CLOSED"));
    }

    #[test]
    fn conversation_service_v2_should_append_tool_to_last_assistant_without_final_text() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-v2-tool-append-open", "active", &now);
        let mut assistant = test_text_message("assistant", "", &now);
        assistant.id = "assistant-open".to_string();
        assistant.speaker_agent_id = Some(DEFAULT_AGENT_ID.to_string());
        conversation.messages.push(assistant);
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");
        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        let (assistant_tool_event, tool_result_event) =
            test_v2_single_tool_group_result("call-v2-2", "read_file");
        let append = conversation_service_v2()
            .append_tool_event_to_assistant_message(
                &state,
                &AssistantMessageToolAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-open".to_string(),
                    assistant_tool_event,
                    tool_result_event,
                },
            )
            .expect("tool append should succeed");

        assert_eq!(append.assistant_message_id, "assistant-open");
        assert_eq!(append.tool_event_count, 2);
        assert!(!append.tool_append_closed);

        let stored = conversation_service_v2()
            .read_message_by_id(&state, &conversation.id, "assistant-open")
            .expect("read updated assistant message");
        assert_eq!(stored.tool_call.as_ref().map(Vec::len), Some(2));
        assert!(
            stored.parts.is_empty(),
            "开放组（未提交 final text）落盘为纯工具行组，读回是未闭合组语义：无正文 part"
        );
    }

    #[test]
    fn conversation_service_v2_should_keep_completed_tool_round_when_compaction_message_is_appended()
    {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-v2-compaction-keeps-tool-round", "active", &now);
        let mut user = test_text_message("user", "连续检查两个目标", &now);
        user.id = "user-before-tool-rounds".to_string();
        let mut assistant = test_text_message("assistant", "", &now);
        assistant.id = "assistant-tool-rounds".to_string();
        assistant.speaker_agent_id = Some(DEFAULT_AGENT_ID.to_string());
        conversation.messages = vec![user, assistant];
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");
        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        // 第一轮未超限，已经正式写入聚合 assistant 消息。
        let (mut first_assistant_event, first_tool_result) =
            test_v2_single_tool_group_result("call-round-1", "read_file");
        first_assistant_event["content"] = serde_json::json!("第一轮工具检查已经完成。");
        conversation_service_v2()
            .append_tool_event_to_assistant_message(
                &state,
                &AssistantMessageToolAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-tool-rounds".to_string(),
                    assistant_tool_event: first_assistant_event,
                    tool_result_event: first_tool_result,
                },
            )
            .expect("persist first completed tool round");

        // 第二轮命中写入前压缩闸门；此时压缩读取必须先看到已持久化的第一轮。
        let before_compaction = conversation_service_v2()
            .get_conversation_prompt_context(&state, &conversation.id)
            .expect("read prompt context before compaction");
        let before_assistant = before_compaction
            .messages
            .iter()
            .find(|message| message.id == "assistant-tool-rounds")
            .expect("completed tool round should be visible before compaction");
        let before_events = before_assistant
            .tool_call
            .as_ref()
            .expect("completed tool history before compaction");
        assert_eq!(before_events.len(), 2);
        assert_eq!(
            before_events[0]
                .get("tool_calls")
                .and_then(Value::as_array)
                .and_then(|calls| calls.first())
                .and_then(|call| call.get("id"))
                .and_then(Value::as_str),
            Some("call-round-1")
        );
        assert_eq!(
            before_events[1].get("tool_call_id").and_then(Value::as_str),
            Some("call-round-1")
        );
        assert_eq!(
            before_events[1].get("content").and_then(Value::as_str),
            Some("tool result")
        );

        let compaction_source = conversation_service_v2()
            .read_archive_pipeline_last_block_conversation(&state, &conversation.id)
            .expect("read compaction source after completed tool round");
        let compaction_assistant = compaction_source
            .messages
            .iter()
            .find(|message| message.id == "assistant-tool-rounds")
            .expect("tool-stage assistant message should be retained for compaction");
        assert_eq!(
            compaction_assistant
                .tool_call
                .as_ref()
                .and_then(|events| events.first())
                .and_then(|event| event.get("content"))
                .and_then(Value::as_str),
            Some("第一轮工具检查已经完成。")
        );
        let preserved_dialogue = collect_block_preserved_dialogue(
            &compaction_source.messages,
            "用户",
            "助手",
            PreservedDialogueBudget::Tokens(10_000),
        );
        assert!(preserved_dialogue.contains("助手：第一轮工具检查已经完成。"));
        assert!(!preserved_dialogue.contains("tool result"));

        let compression_message = build_compaction_message(
            "第一轮工具检查已经完成。",
            Some("工具调度压缩"),
            "force_context_usage_82",
            Some("用户：连续检查两个目标\n助手：第一轮工具检查已经完成。"),
            &[],
        );
        conversation_service_v2()
            .persist_compaction_message(&state, &conversation, &compression_message, None)
            .expect("append compaction message");

        let stored_assistant = conversation_service_v2()
            .get_raw_message_by_id(&state, &conversation.id, "assistant-tool-rounds")
            .expect("completed tool round should remain after compaction append");
        let stored_events = stored_assistant
            .tool_call
            .as_ref()
            .expect("completed tool history after compaction append");
        assert_eq!(stored_events, before_events);

        let snapshot = conversation_service_v2()
            .get_conversation_snapshot(&state, &conversation.id)
            .expect("read conversation after compaction append");
        assert_eq!(
            snapshot
                .messages
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "user-before-tool-rounds",
                "assistant-tool-rounds",
                compression_message.id.as_str(),
            ]
        );
    }

    #[test]
    fn conversation_service_v2_should_preserve_final_text_verbatim() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-v2-final-text-open", "active", &now);
        let mut assistant = test_text_message("assistant", "", &now);
        assistant.id = "assistant-final-open".to_string();
        assistant.speaker_agent_id = Some(DEFAULT_AGENT_ID.to_string());
        assistant.tool_call = Some(vec![serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "id": "call-final-open",
                "type": "function",
                "function": {
                    "name": "read_file",
                    "arguments": "{}"
                }
            }]
        }), serde_json::json!({
            "role": "tool",
            "tool_call_id": "call-final-open",
            "content": "tool result"
        })]);
        conversation.messages.push(assistant);
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");
        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        let final_text = "\n```python\ndef hello():\n    return True\n```\n";
        let append = conversation_service_v2()
            .append_final_text_to_assistant_message(
                &state,
                &AssistantMessageFinalTextAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-final-open".to_string(),
                    final_text: final_text.to_string(),
                    reasoning_text: Some("  这是最终思考  ".to_string()),
                    provider_meta_patch: Some(serde_json::json!({
                        "usage": { "outputTokens": 12 }
                    })),
                    meme_annotations: None,
                },
            )
            .expect("final text append should succeed");

        assert!(append.final_text_committed);
        assert!(append.tool_append_closed);

        let stored = conversation_service_v2()
            .read_message_by_id(&state, &conversation.id, "assistant-final-open")
            .expect("read updated assistant message");
        match &stored.parts[0] {
            MessagePart::Text {
                text,
                reasoning_content,
            } => {
                assert_eq!(text, final_text);
                assert_eq!(reasoning_content.as_deref(), Some("  这是最终思考  "));
            }
            _ => panic!("expected text part"),
        }
        assert_eq!(stored.tool_call.as_ref().map(Vec::len), Some(2));
        assert_eq!(
            stored
                .provider_meta
                .as_ref()
                .and_then(|value| value.get("usage"))
                .and_then(|value| value.get("outputTokens"))
                .and_then(Value::as_u64),
            Some(12)
        );
    }

    #[test]
    fn merge_last_tool_call_usage_should_derive_meta_from_tool_call_events() {
        let tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "tool_calls": [],
                "usage": {"promptTokens": 1000, "contextWindowTokens": 10000},
            }),
            serde_json::json!({"role": "tool", "tool_call_id": "a"}),
            serde_json::json!({
                "role": "assistant",
                "tool_calls": [],
                "usage": {"promptTokens": 2500, "contextWindowTokens": 10000},
            }),
        ]);
        let mut meta = Some(serde_json::json!({"dispatchElapsedMs": 12}));
        merge_last_tool_call_usage_into_provider_meta(&mut meta, &tool_call);
        let merged = meta.expect("meta present");
        assert_eq!(merged["providerPromptTokens"].as_u64(), Some(2500), "取最后一个带用量的事件");
        assert_eq!(merged["effectivePromptTokens"].as_u64(), Some(2500));
        assert_eq!(
            merged["effectivePromptSource"].as_str(),
            Some("provider_tool_round")
        );
        assert_eq!(merged["contextUsagePercent"].as_u64(), Some(25));
        assert_eq!(merged["contextWindowTokens"].as_u64(), Some(10000));

        // 已有真实用量（最终 call 写入）时不覆盖
        let mut meta = Some(serde_json::json!({"providerPromptTokens": 999}));
        merge_last_tool_call_usage_into_provider_meta(&mut meta, &tool_call);
        assert_eq!(
            meta.unwrap()["providerPromptTokens"].as_u64(),
            Some(999),
            "不覆盖已有真实用量"
        );

        // 旧数据/外部补丁写入 0 或 null 时不算有效用量，仍应从工具事件恢复
        for stale_meta in [
            serde_json::json!({"providerPromptTokens": 0}),
            serde_json::json!({"providerPromptTokens": null}),
            serde_json::json!({"providerPromptTokens": "not-a-number"}),
        ] {
            let mut meta = Some(stale_meta);
            merge_last_tool_call_usage_into_provider_meta(&mut meta, &tool_call);
            let merged = meta.expect("meta present");
            assert_eq!(
                merged["providerPromptTokens"].as_u64(),
                Some(2500),
                "0/null/非数字视为无有效用量，从工具事件恢复"
            );
            assert_eq!(
                merged["effectivePromptSource"].as_str(),
                Some("provider_tool_round")
            );
        }

        // 事件缺 contextWindowTokens：占用率无法正确计算，应跳过而不是退化为 prompt_tokens
        let tool_call_missing_window = Some(vec![serde_json::json!({
            "role": "assistant",
            "tool_calls": [],
            "usage": {"promptTokens": 1200},
        })]);
        let mut meta = Some(serde_json::json!({}));
        merge_last_tool_call_usage_into_provider_meta(&mut meta, &tool_call_missing_window);
        assert!(
            meta.as_ref().and_then(|m| m.get("contextUsageRatio")).is_none(),
            "缺 contextWindowTokens 时不写入 contextUsageRatio"
        );
        assert!(
            meta.as_ref().and_then(|m| m.get("providerPromptTokens")).is_none(),
            "缺 contextWindowTokens 时不写入 providerPromptTokens"
        );
    }

    #[test]
    fn conversation_service_v2_should_bootstrap_then_append_tool_and_final_text() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation =
            test_chat_conversation("conversation-v2-bootstrap-open", "active", &now);
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

        let bootstrap = conversation_service_v2()
            .bootstrap_streaming_assistant_message(
                &state,
                &AssistantMessageBootstrapInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-bootstrap".to_string(),
                    speaker_agent_id: DEFAULT_AGENT_ID.to_string(),
                    created_at: Some(now.clone()),
                    provider_meta_patch: None,
                    compaction_preserved_messages: None,
                },
            )
            .expect("bootstrap assistant should succeed");

        assert!(bootstrap.created);
        assert_eq!(bootstrap.assistant_message_id, "assistant-bootstrap");

        let (assistant_tool_event, tool_result_event) =
            test_v2_single_tool_group_result("call-v2-bootstrap", "read_file");
        let tool_append = conversation_service_v2()
            .append_tool_event_to_assistant_message(
                &state,
                &AssistantMessageToolAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-bootstrap".to_string(),
                    assistant_tool_event,
                    tool_result_event,
                },
            )
            .expect("tool append after bootstrap should succeed");
        assert_eq!(tool_append.tool_event_count, 2);

        let final_append = conversation_service_v2()
            .append_final_text_to_assistant_message(
                &state,
                &AssistantMessageFinalTextAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-bootstrap".to_string(),
                    final_text: "bootstrap 完成正文".to_string(),
                    reasoning_text: Some("bootstrap 完成思考".to_string()),
                    provider_meta_patch: None,
                    meme_annotations: Some(vec![MemeAnnotation {
                        meme: ":坏笑:".to_string(),
                        path: "E:/fake/坏笑.webp".to_string(),
                    }]),
                },
            )
            .expect("final append after bootstrap should succeed");
        assert!(final_append.final_text_committed);

        let stored = conversation_service_v2()
            .read_message_by_id(&state, &conversation.id, "assistant-bootstrap")
            .expect("read bootstrapped assistant");
        assert_eq!(stored.tool_call.as_ref().map(Vec::len), Some(2));
        match &stored.parts[0] {
            MessagePart::Text {
                text,
                reasoning_content,
            } => {
                assert_eq!(text, "bootstrap 完成正文");
                assert_eq!(reasoning_content.as_deref(), Some("bootstrap 完成思考"));
            }
            _ => panic!("expected text part"),
        }
        assert_eq!(stored.meme_annotations.as_ref().map(Vec::len), Some(1));
        assert_eq!(
            stored
                .meme_annotations
                .as_ref()
                .and_then(|items| items.first())
                .map(|item| item.meme.as_str()),
            Some(":坏笑:")
        );
    }

    #[test]
    fn conversation_service_v2_should_bootstrap_compaction_preserved_tools_without_final_text() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation =
            test_chat_conversation("conversation-v2-bootstrap-preserved-tools", "active", &now);
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");
        let preserved_assistant_text = "我先读取仓库状态，再根据结果继续。";
        let preserved_reasoning_text = "需要先调用 exec 获取 git status。";
        let preserved_events = vec![
            serde_json::json!({
                "role": "assistant",
                "content": preserved_assistant_text,
                "reasoning_content": preserved_reasoning_text,
                "tool_calls": [{
                    "id": "call-v2-preserved",
                    "type": "function",
                    "function": {
                        "name": "exec",
                        "arguments": "{\"command\":\"git status --short\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call-v2-preserved",
                "content": " M src-tauri/src/features/chat/tests.rs"
            }),
        ];

        let bootstrap = conversation_service_v2()
            .bootstrap_streaming_assistant_message(
                &state,
                &AssistantMessageBootstrapInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-bootstrap-preserved".to_string(),
                    speaker_agent_id: DEFAULT_AGENT_ID.to_string(),
                    created_at: Some(now.clone()),
                    provider_meta_patch: None,
                    compaction_preserved_messages: Some(CompactionPreservedMessages::new(
                        preserved_assistant_text,
                        preserved_reasoning_text,
                        preserved_events.clone(),
                    )),
                },
            )
            .expect("bootstrap preserved assistant should succeed");

        assert!(bootstrap.created);
        let stored_before_final = conversation_service_v2()
            .read_message_by_id(&state, &conversation.id, "assistant-bootstrap-preserved")
            .expect("read preserved bootstrapped assistant before final");
        let stored_before_tool_events = stored_before_final
            .tool_call
            .as_ref()
            .expect("preserved tool history should exist");
        assert_eq!(stored_before_tool_events.len(), 2);
        assert_eq!(
            stored_before_tool_events[0].get("content").and_then(Value::as_str),
            Some(preserved_assistant_text)
        );
        assert_eq!(
            stored_before_tool_events[0]
                .get("reasoning_content")
                .and_then(Value::as_str),
            Some(preserved_reasoning_text)
        );
        assert_eq!(
            stored_before_tool_events[0]
                .get("tool_calls")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            stored_before_tool_events[1]
                .get("tool_call_id")
                .and_then(Value::as_str),
            Some("call-v2-preserved")
        );
        assert_eq!(
            stored_before_tool_events[1].get("role").and_then(Value::as_str),
            Some("tool")
        );
        assert!(stored_before_final.extra_text_blocks.is_empty());
        assert!(
            stored_before_final.parts.is_empty(),
            "开放组（未提交 final text）落盘为纯工具行组，读回是未闭合组语义：无正文 part"
        );

        let final_append = conversation_service_v2()
            .append_final_text_to_assistant_message(
                &state,
                &AssistantMessageFinalTextAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-bootstrap-preserved".to_string(),
                    final_text: "压缩续调后的最终正文".to_string(),
                    reasoning_text: Some("压缩续调后的最终思考".to_string()),
                    provider_meta_patch: None,
                    meme_annotations: None,
                },
            )
            .expect("final append after preserved bootstrap should succeed");
        assert!(final_append.final_text_committed);

        let stored_after_final = conversation_service_v2()
            .read_message_by_id(&state, &conversation.id, "assistant-bootstrap-preserved")
            .expect("read preserved bootstrapped assistant after final");
        let stored_after_tool_events = stored_after_final
            .tool_call
            .as_ref()
            .expect("preserved tool history should remain after final");
        assert_eq!(stored_after_tool_events.len(), 2);
        assert_eq!(
            stored_after_tool_events[0].get("content").and_then(Value::as_str),
            Some(preserved_assistant_text)
        );
        assert_eq!(
            stored_after_tool_events[0]
                .get("reasoning_content")
                .and_then(Value::as_str),
            Some(preserved_reasoning_text)
        );
        match &stored_after_final.parts[0] {
            MessagePart::Text {
                text,
                reasoning_content,
            } => {
                assert_eq!(text, "压缩续调后的最终正文");
                assert_eq!(reasoning_content.as_deref(), Some("压缩续调后的最终思考"));
            }
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn conversation_service_v2_should_bootstrap_delegate_in_delegate_store() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let entry = delegate_store_create_delegate(
            &state.data_path,
            &DelegateCreateInput {
                kind: "delegate".to_string(),
                conversation_id: "root-conversation".to_string(),
                parent_delegate_id: None,
                source_agent_id: "source-agent".to_string(),
                target_agent_id: DEFAULT_AGENT_ID.to_string(),
                title: "委托启动测试".to_string(),
                why: "验证委托会话启动".to_string(),
                goal: "初始化助理消息".to_string(),
                todo: "写入会话".to_string(),
                notify_assistant_when_done: false,
                call_stack: Vec::new(),
            },
        )
        .expect("create delegate record");
        let mut conversation =
            test_chat_conversation(&entry.delegate_id, "active", &now);
        conversation.conversation_kind = CONVERSATION_KIND_DELEGATE.to_string();
        conversation.root_conversation_id = Some(entry.conversation_id.clone());
        conversation.delegate_id = Some(entry.delegate_id.clone());
        conversation
            .messages
            .push(test_text_message("user", "执行委托任务", &now));
        delegate_conversation_store_write(&state.data_path, &conversation)
            .expect("persist delegate conversation");

        let bootstrap = conversation_service_v2()
            .bootstrap_streaming_assistant_message(
                &state,
                &AssistantMessageBootstrapInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-delegate-bootstrap".to_string(),
                    speaker_agent_id: DEFAULT_AGENT_ID.to_string(),
                    created_at: Some(now),
                    provider_meta_patch: None,
                    compaction_preserved_messages: None,
                },
            )
            .expect("bootstrap delegate assistant should succeed");

        assert!(bootstrap.created);
        let stored = delegate_runtime_thread_conversation_get(&state, &conversation.id)
            .expect("read delegate conversation")
            .expect("delegate conversation should remain in delegate store");
        assert!(stored
            .messages
            .iter()
            .any(|message| message.id == "assistant-delegate-bootstrap"));
        assert!(conversation_service_v2()
            .get_conversation_meta(&state, &conversation.id)
            .is_err());
    }

    #[test]
    fn delegate_fast_request_should_append_atomically_and_be_readable_as_misc_work() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let root_conversation =
            test_chat_conversation("root-conversation", "active", &now);
        state_schedule_conversation_persist(&state, &root_conversation)
            .expect("persist root conversation");
        let delegate = delegate_store_create_delegate(
            &state.data_path,
            &DelegateCreateInput {
                kind: "remote_im_reply".to_string(),
                conversation_id: root_conversation.id.clone(),
                parent_delegate_id: None,
                source_agent_id: DEFAULT_AGENT_ID.to_string(),
                target_agent_id: DEFAULT_AGENT_ID.to_string(),
                title: "远程应答".to_string(),
                why: "验证应答委托杂务".to_string(),
                goal: "记录回复改写".to_string(),
                todo: "执行改写".to_string(),
                notify_assistant_when_done: false,
                call_stack: Vec::new(),
            },
        )
        .expect("create delegate record");
        delegate_runtime_thread_create(&state, &delegate, "", None, None)
            .expect("create delegate runtime thread");
        delegate_runtime_thread_conversation_append_if_absent(
            &state,
            &delegate.delegate_id,
            test_text_message("user", "执行远程应答", &now),
        )
        .expect("append delegate message");
        let turn = build_fast_request_turn(
            FAST_REQUEST_KIND_REMOTE_IM_REPLY_REWRITE,
            "压缩请求",
            "压缩结果",
            true,
            None,
            Some("quick-model".to_string()),
            Some(25),
        );

        assert!(delegate_runtime_thread_append_fast_request(
            &state,
            &delegate.delegate_id,
            turn,
        )
        .expect("append delegate fast request"));

        let turns = conversation_service_v2()
            .get_conversation_fast_request_turns(&state, &delegate.delegate_id)
            .expect("read delegate fast requests");
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].kind, FAST_REQUEST_KIND_REMOTE_IM_REPLY_REWRITE);
        assert_eq!(turns[0].response_text, "压缩结果");
    }

    #[tokio::test]
    async fn conversation_service_v2_should_refresh_preview_after_appending_final_text() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation =
            test_chat_conversation("conversation-v2-preview-refresh-final-text", "active", &now);
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

        conversation_service_v2()
            .append_user_message(
                &state,
                &UserMessageAppendInput {
                    conversation_id: conversation.id.clone(),
                    message: test_text_message("user", "你会rust语言吗", &now),
                    memory_recall_ids: Vec::new(),
                },
            )
            .await
            .expect("append user message");

        conversation_service_v2()
            .bootstrap_streaming_assistant_message(
                &state,
                &AssistantMessageBootstrapInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-preview-refresh".to_string(),
                    speaker_agent_id: DEFAULT_AGENT_ID.to_string(),
                    created_at: Some(now.clone()),
                    provider_meta_patch: None,
                    compaction_preserved_messages: None,
                },
            )
            .expect("bootstrap assistant should succeed");

        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        let meta_before = message_store::chat_store_read_meta(&store_paths)
            .expect("read ready meta before final text")
            .expect("ready meta exists before final text");
        assert_eq!(meta_before.preview_messages().len(), 1);
        assert_eq!(
            meta_before.preview_messages()[0].text_preview,
            "你会rust语言吗"
        );

        conversation_service_v2()
            .append_final_text_to_assistant_message(
                &state,
                &AssistantMessageFinalTextAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-preview-refresh".to_string(),
                    final_text: "会。Rust 是一门系统级编程语言。".to_string(),
                    reasoning_text: None,
                    provider_meta_patch: None,
                    meme_annotations: None,
                },
            )
            .expect("append final text");

        let meta_after = message_store::chat_store_read_meta(&store_paths)
            .expect("read ready meta after final text")
            .expect("ready meta exists after final text");
        assert_eq!(meta_after.preview_messages().len(), 2);
        assert_eq!(
            meta_after.preview_messages()[1].text_preview,
            "会。Rust 是一门系统级编程语言。"
        );
    }

    #[test]
    fn conversation_service_v2_should_close_tool_append_after_empty_final_commit() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation =
            test_chat_conversation("conversation-v2-empty-final-close", "active", &now);
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

        conversation_service_v2()
            .bootstrap_streaming_assistant_message(
                &state,
                &AssistantMessageBootstrapInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-empty-final".to_string(),
                    speaker_agent_id: DEFAULT_AGENT_ID.to_string(),
                    created_at: Some(now.clone()),
                    provider_meta_patch: None,
                    compaction_preserved_messages: None,
                },
            )
            .expect("bootstrap assistant should succeed");

        let final_append = conversation_service_v2()
            .append_final_text_to_assistant_message(
                &state,
                &AssistantMessageFinalTextAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-empty-final".to_string(),
                    final_text: String::new(),
                    reasoning_text: None,
                    provider_meta_patch: None,
                    meme_annotations: None,
                },
            )
            .expect("empty final text should still commit");
        assert!(final_append.final_text_committed);
        assert!(final_append.tool_append_closed);

        let stored = conversation_service_v2()
            .read_message_by_id(&state, &conversation.id, "assistant-empty-final")
            .expect("read assistant message");
        match &stored.parts[0] {
            MessagePart::Text {
                text,
                reasoning_content,
            } => {
                assert!(text.is_empty());
                assert_eq!(reasoning_content.as_deref(), None);
            }
            _ => panic!("expected text part"),
        }
        assert_eq!(
            stored
                .provider_meta
                .as_ref()
                .and_then(|value| value.get("streamFinalCommitted"))
                .and_then(Value::as_bool),
            Some(true)
        );

        let (assistant_tool_event, tool_result_event) =
            test_v2_single_tool_group_result("call-v2-empty-final", "read_file");
        let err = conversation_service_v2()
            .append_tool_event_to_assistant_message(
                &state,
                &AssistantMessageToolAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-empty-final".to_string(),
                    assistant_tool_event,
                    tool_result_event,
                },
            )
            .expect_err("empty final commit should close further tool append");

        assert!(err.contains("MSG_TOOL_APPEND_CLOSED"));
    }

    #[test]
    fn conversation_service_v2_should_patch_provider_meta_on_finalized_tail_assistant() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation =
            test_chat_conversation("conversation-v2-provider-meta-patch", "active", &now);
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");

        conversation_service_v2()
            .bootstrap_streaming_assistant_message(
                &state,
                &AssistantMessageBootstrapInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-provider-meta".to_string(),
                    speaker_agent_id: DEFAULT_AGENT_ID.to_string(),
                    created_at: Some(now.clone()),
                    provider_meta_patch: None,
                    compaction_preserved_messages: None,
                },
            )
            .expect("bootstrap assistant should succeed");
        conversation_service_v2()
            .append_final_text_to_assistant_message(
                &state,
                &AssistantMessageFinalTextAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-provider-meta".to_string(),
                    final_text: "已经结束".to_string(),
                    reasoning_text: None,
                    provider_meta_patch: Some(serde_json::json!({
                        "usage": { "outputTokens": 9 }
                    })),
                    meme_annotations: None,
                },
            )
            .expect("final append should succeed");

        let patch = conversation_service_v2()
            .patch_provider_meta_on_assistant_message(
                &state,
                &AssistantMessageProviderMetaPatchInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-provider-meta".to_string(),
                    provider_meta_patch: serde_json::json!({
                        "remoteImDecision": {
                            "action": "send_success",
                            "error": "",
                            "processingMode": "continuous",
                            "conversationKind": "remote_im_contact"
                        }
                    }),
                },
            )
            .expect("provider meta patch should succeed");
        assert_eq!(patch.assistant_message_id, "assistant-provider-meta");

        let stored = conversation_service_v2()
            .read_message_by_id(&state, &conversation.id, "assistant-provider-meta")
            .expect("read updated assistant message");
        match &stored.parts[0] {
            MessagePart::Text { text, .. } => assert_eq!(text, "已经结束"),
            _ => panic!("expected text part"),
        }
        assert_eq!(
            stored
                .provider_meta
                .as_ref()
                .and_then(|value| value.get("usage"))
                .and_then(|value| value.get("outputTokens"))
                .and_then(Value::as_u64),
            Some(9)
        );
        assert_eq!(
            stored
                .provider_meta
                .as_ref()
                .and_then(|value| value.get("remoteImDecision"))
                .and_then(|value| value.get("action"))
                .and_then(Value::as_str),
            Some("send_success")
        );
    }

    #[test]
    fn conversation_service_v2_should_allow_final_text_append_on_non_tail_assistant() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-v2-final-text-non-tail", "active", &now);
        let mut assistant = test_text_message("assistant", "", &now);
        assistant.id = "assistant-non-tail".to_string();
        assistant.speaker_agent_id = Some(DEFAULT_AGENT_ID.to_string());
        let user_tail = test_text_message("user", "后面还有用户消息", &now);
        conversation.messages.push(assistant);
        conversation.messages.push(user_tail);
        state_schedule_conversation_persist(&state, &conversation).expect("persist conversation");
        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        let append = conversation_service_v2()
            .append_final_text_to_assistant_message(
                &state,
                &AssistantMessageFinalTextAppendInput {
                    conversation_id: conversation.id.clone(),
                    assistant_message_id: "assistant-non-tail".to_string(),
                    final_text: "按 ID 写入成功".to_string(),
                    reasoning_text: None,
                    provider_meta_patch: None,
                    meme_annotations: None,
                },
            )
            .expect("non-tail assistant should still be writable by id");

        assert!(append.final_text_committed);
        assert_eq!(append.assistant_message_id, "assistant-non-tail");

        let stored = conversation_service_v2()
            .read_message_by_id(&state, &conversation.id, "assistant-non-tail")
            .expect("read updated assistant message");
        match &stored.parts[0] {
            MessagePart::Text { text, .. } => assert_eq!(text, "按 ID 写入成功"),
            _ => panic!("expected text part"),
        }
    }

    #[test]
    fn state_schedule_conversation_delete_should_remove_memory_chat_index_item() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-memory-delete", "active", &now);
        state_schedule_conversation_persist(&state, &conversation).expect("schedule persist");

        state_schedule_conversation_delete(&state, &conversation.id).expect("schedule delete");

        let chat_index = state_read_chat_index_cached(&state).expect("read memory chat index");
        assert!(chat_index.conversations.is_empty());
        assert!(!app_layout_chat_dir(&state.data_path).join("index.json").exists());
    }

    #[test]
    fn conversation_delete_should_not_be_overwritten_by_waiting_metadata_update() {
        let state = std::sync::Arc::new(test_chat_runtime_state());
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-delete-metadata-race", "active", &now);
        state_schedule_conversation_persist(&state, &conversation).expect("schedule persist");

        let mutation_gate = conversation_mutation_gate(&state.data_path, &conversation.id)
            .expect("mutation gate");
        let guard = mutation_gate.lock().expect("hold mutation gate");
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let state_for_update = std::sync::Arc::clone(&state);
        let conversation_id = conversation.id.clone();
        let metadata_update = std::thread::spawn(move || {
            started_tx.send(()).expect("notify metadata update start");
            let result = state_update_conversation_metadata_cached(
                &state_for_update,
                &conversation_id,
                |cached| {
                    cached.title = "must-not-resurrect".to_string();
                    Ok(())
                },
            );
            result_tx.send(result).expect("return metadata update result");
        });
        started_rx.recv().expect("wait metadata update start");
        assert!(matches!(result_rx.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));

        state_schedule_conversation_delete(&state, &conversation.id).expect("schedule delete while holding gate");
        drop(guard);
        let result = result_rx.recv().expect("wait metadata update result");
        metadata_update.join().expect("join metadata update");
        assert!(result.is_err());
        assert!(state_read_conversation_metadata_cached(&state, &conversation.id).is_err());
        assert!(state.cached_deleted_conversation_ids.lock().expect("read deleted ids").contains(&conversation.id));
        let pending = state.conversation_persist_pending.lock().expect("read pending delete");
        assert!(pending.as_ref().expect("pending exists").deleted_conversation_ids.contains(&conversation.id));
    }

    #[test]
    fn conversation_delete_should_not_be_overwritten_by_waiting_shell_workspace_direct_write() {
        let state = std::sync::Arc::new(test_chat_runtime_state());
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-delete-workspace-race", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark persisted");

        let mutation_gate = conversation_mutation_gate(&state.data_path, &conversation.id)
            .expect("mutation gate");
        let guard = mutation_gate.lock().expect("hold mutation gate");
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let state_for_write = std::sync::Arc::clone(&state);
        let conversation_id = conversation.id.clone();
        let workspace_path = state.llm_workspace_path.to_string_lossy().to_string();
        let direct_write = std::thread::spawn(move || {
            started_tx.send(()).expect("notify direct write start");
            let result = state_write_conversation_shell_workspace_metadata_direct(
                &state_for_write,
                &conversation_id,
                vec![ShellWorkspaceConfig {
                    id: "main".to_string(),
                    name: "main".to_string(),
                    path: workspace_path,
                    level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
                    access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
                    built_in: false,
                }],
            );
            result_tx.send(result).expect("return direct write result");
        });
        started_rx.recv().expect("wait direct write start");
        assert!(matches!(result_rx.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)));

        state_schedule_conversation_delete(&state, &conversation.id)
            .expect("schedule delete while holding gate");
        drop(guard);
        let result = result_rx.recv().expect("wait direct write result");
        direct_write.join().expect("join direct write");

        assert!(result.is_err());
        assert!(state_read_conversation_metadata_cached(&state, &conversation.id).is_err());
        assert!(state
            .cached_deleted_conversation_ids
            .lock()
            .expect("read deleted ids")
            .contains(&conversation.id));
        let pending = state
            .conversation_persist_pending
            .lock()
            .expect("read pending delete");
        assert!(pending
            .as_ref()
            .expect("pending exists")
            .deleted_conversation_ids
            .contains(&conversation.id));
    }

    #[tokio::test]
    async fn update_unarchived_conversation_by_id_should_publish_v3_message_replacements() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-v3-generic-message-update", "active", &now);
        let mut message = test_text_message("assistant", "待更新工具审查", &now);
        message.id = "tool-review-message".to_string();
        message.tool_call = Some(vec![serde_json::json!({
            "tool_call_id": "call-v3-review",
            "content": "原始结果"
        })]);
        conversation.messages.push(message);
        write_conversation_shard(&state.data_path, &conversation).expect("write v2 conversation");
        message_store::chat_metadata_store_run_v3_migration(&state.data_path)
            .expect("migrate conversation to v3");

        conversation_service_v2()
            .update_unarchived_conversation_by_id(&state, &conversation.id, |updated| {
                updated.messages[0].tool_call.as_mut().expect("tool call")[0]["content"] =
                    serde_json::Value::String("已审查结果".to_string());
                Ok(())
            })
            .await
            .expect("update v3 message");

        let paths = message_store::message_store_paths(&state.data_path, &conversation.id)
            .expect("message store paths");
        let stored = message_store::chat_store_read_message_by_id(
            &paths,
            "tool-review-message",
        )
        .expect("read stored message")
        .expect("stored message exists");
        assert_eq!(
            stored.tool_call.as_ref().expect("stored tool call")[0]["content"],
            serde_json::Value::String("已审查结果".to_string())
        );
    }

    #[tokio::test]
    async fn update_latest_summary_title_should_keep_summary_title_consistent_in_v3() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation(
            "conversation-summary-title-consistency",
            "active",
            &now,
        );
        let mut summary = test_text_message("assistant", "旧摘要正文", &now);
        summary.id = "summary-message".to_string();
        summary.provider_meta = Some(serde_json::json!({
            "message_meta": {
                "kind": "context_compaction",
                "title": "旧标题",
                "schemaVersion": 1,
            }
        }));
        let mut user = test_text_message("user", "你好", &now);
        user.id = "user-message".to_string();
        conversation.messages.push(summary);
        conversation.messages.push(user);
        write_conversation_shard(&state.data_path, &conversation).expect("write v2 conversation");
        message_store::chat_metadata_store_run_v3_migration(&state.data_path)
            .expect("migrate conversation to v3");

        let changed = conversation_service_v2()
            .update_latest_summary_title(&state, &conversation.id, "新标题")
            .await
            .expect("update summary title");
        assert!(changed);

        let paths = message_store::message_store_paths(&state.data_path, &conversation.id)
            .expect("message store paths");
        let stored =
            message_store::chat_store_read_message_by_id(&paths, "summary-message")
                .expect("read stored summary message")
                .expect("summary message exists");
        assert_eq!(
            stored.provider_meta.as_ref().expect("provider meta")["message_meta"]["title"],
            serde_json::Value::String("新标题".to_string())
        );
        let persisted = message_store::chat_store_read_meta(&paths)
            .expect("read persisted meta")
            .expect("persisted meta exists");
        assert_eq!(persisted.latest_summary_title().as_deref(), Some("新标题"));
        let cached = state
            .cached_conversation_metadata
            .lock()
            .expect("lock cached metadata")
            .get(&conversation.id)
            .cloned()
            .expect("cached meta exists");
        assert_eq!(cached.latest_summary_title().as_deref(), Some("新标题"));
        let meta_view = conversation_service_v2()
            .get_conversation_meta(&state, &conversation.id)
            .expect("read meta view");
        assert_eq!(meta_view.latest_summary_title.as_deref(), Some("新标题"));
    }

    #[tokio::test]
    async fn full_refresh_should_read_updated_summary_title() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation(
            "conversation-summary-title-full-refresh",
            "active",
            &now,
        );
        let mut summary = test_text_message("assistant", "摘要正文", &now);
        summary.id = "full-refresh-summary".to_string();
        summary.provider_meta = Some(serde_json::json!({
            "message_meta": {"kind": "context_compaction", "title": "刷新前标题", "schemaVersion": 1}
        }));
        conversation.messages.push(summary);
        write_conversation_shard(&state.data_path, &conversation).expect("write v2 conversation");
        message_store::chat_metadata_store_run_v3_migration(&state.data_path)
            .expect("migrate conversation to v3");

        conversation_service_v2()
            .update_latest_summary_title(&state, &conversation.id, "刷新后标题")
            .await
            .expect("update summary title");

        let summaries = conversation_service_v2()
            .list_unarchived_conversation_summaries(&state)
            .expect("list unarchived summaries")
            .summaries;
        let target = summaries
            .iter()
            .find(|item| item.conversation_id == conversation.id)
            .expect("conversation in full list");
        assert_eq!(target.summary_title.as_deref(), Some("刷新后标题"));
    }

    #[tokio::test]
    async fn replacing_non_latest_summary_message_should_not_override_latest_summary_title() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation(
            "conversation-summary-title-preserve",
            "active",
            &now,
        );
        let mut older = test_text_message("assistant", "旧摘要", &now);
        older.id = "older-summary".to_string();
        older.provider_meta = Some(serde_json::json!({
            "message_meta": {"kind": "context_compaction", "title": "旧标题", "schemaVersion": 1}
        }));
        let mut newer = test_text_message("assistant", "新摘要", &now);
        newer.id = "newer-summary".to_string();
        newer.provider_meta = Some(serde_json::json!({
            "message_meta": {"kind": "context_compaction", "title": "最新标题", "schemaVersion": 1}
        }));
        conversation.messages.push(older);
        conversation.messages.push(newer);
        write_conversation_shard(&state.data_path, &conversation).expect("write v2 conversation");
        message_store::chat_metadata_store_run_v3_migration(&state.data_path)
            .expect("migrate conversation to v3");

        conversation_service_v2()
            .update_unarchived_conversation_by_id(&state, &conversation.id, |updated| {
                let target = updated
                    .messages
                    .iter_mut()
                    .find(|message| message.id == "older-summary")
                    .expect("older summary exists");
                target.provider_meta = Some(serde_json::json!({
                    "message_meta": {"kind": "context_compaction", "title": "被改写", "schemaVersion": 1}
                }));
                Ok(())
            })
            .await
            .expect("replace older summary");

        let paths = message_store::message_store_paths(&state.data_path, &conversation.id)
            .expect("message store paths");
        let persisted = message_store::chat_store_read_meta(&paths)
            .expect("read persisted meta")
            .expect("persisted meta exists");
        assert_eq!(
            persisted.latest_summary_title().as_deref(),
            Some("最新标题")
        );
    }

    #[test]
    fn batch_provider_meta_patch_should_publish_final_summary_title() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation(
            "conversation-summary-title-batch",
            "active",
            &now,
        );
        let mut summary = test_text_message("assistant", "摘要正文", &now);
        summary.id = "batch-summary".to_string();
        summary.provider_meta = Some(serde_json::json!({
            "message_meta": {"kind": "context_compaction", "title": "原始标题", "schemaVersion": 1}
        }));
        let mut user = test_text_message("user", "你好", &now);
        user.id = "batch-user".to_string();
        conversation.messages.push(summary);
        conversation.messages.push(user);
        write_conversation_shard(&state.data_path, &conversation).expect("write v2 conversation");
        message_store::chat_metadata_store_run_v3_migration(&state.data_path)
            .expect("migrate conversation to v3");

        conversation_service_v2()
            .patch_message_provider_meta_batch(
                &state,
                &MessageProviderMetaBatchPatchInput {
                    conversation_id: conversation.id.clone(),
                    items: vec![
                        MessageProviderMetaPatchItem {
                            message_id: "batch-summary".to_string(),
                            provider_meta: Some(serde_json::json!({
                                "message_meta": {"kind": "context_compaction", "title": "批量新标题", "schemaVersion": 1}
                            })),
                        },
                        MessageProviderMetaPatchItem {
                            message_id: "batch-user".to_string(),
                            provider_meta: Some(serde_json::json!({"custom": true})),
                        },
                    ],
                },
            )
            .expect("batch patch provider meta");

        let paths = message_store::message_store_paths(&state.data_path, &conversation.id)
            .expect("message store paths");
        let persisted = message_store::chat_store_read_meta(&paths)
            .expect("read persisted meta")
            .expect("persisted meta exists");
        assert_eq!(
            persisted.latest_summary_title().as_deref(),
            Some("批量新标题")
        );
    }

    #[tokio::test]
    async fn replacing_plain_message_should_keep_summary_title_and_derived_fields() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation = test_chat_conversation(
            "conversation-summary-title-plain-replace",
            "active",
            &now,
        );
        let mut summary = test_text_message("assistant", "摘要正文", &now);
        summary.id = "plain-summary".to_string();
        summary.provider_meta = Some(serde_json::json!({
            "message_meta": {"kind": "context_compaction", "title": "原标题", "schemaVersion": 1}
        }));
        let mut user = test_text_message("user", "很短", &now);
        user.id = "plain-user".to_string();
        conversation.messages.push(summary);
        conversation.messages.push(user);
        write_conversation_shard(&state.data_path, &conversation).expect("write v2 conversation");
        message_store::chat_metadata_store_run_v3_migration(&state.data_path)
            .expect("migrate conversation to v3");

        let paths = message_store::message_store_paths(&state.data_path, &conversation.id)
            .expect("message store paths");
        let before = message_store::chat_store_read_meta(&paths)
            .expect("read persisted meta before")
            .expect("persisted meta exists before");

        conversation_service_v2()
            .update_unarchived_conversation_by_id(&state, &conversation.id, |updated| {
                let target = updated
                    .messages
                    .iter_mut()
                    .find(|message| message.id == "plain-user")
                    .expect("plain user message exists");
                target.parts = vec![MessagePart::Text {
                    text: "这是一条长很多的普通用户消息".to_string(),
                    reasoning_content: None,
                }];
                Ok(())
            })
            .await
            .expect("replace plain message");

        let after = message_store::chat_store_read_meta(&paths)
            .expect("read persisted meta after")
            .expect("persisted meta exists after");
        assert_eq!(
            after.latest_summary_title().as_deref(),
            before.latest_summary_title().as_deref()
        );
        assert_eq!(after.latest_summary_title().as_deref(), Some("原标题"));
        assert_eq!(after.body_text_length(), before.body_text_length() + 12);
        assert_eq!(after.last_message_id(), before.last_message_id());
        assert_eq!(after.message_count(), before.message_count());
    }

    #[test]
    fn pending_worker_snapshot_taken_before_delete_should_not_rewrite_conversation() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-delete-worker-full-race", "active", &now);
        start_conversation_persist_worker(&state).expect("start persist worker");

        let mutation_gate = conversation_mutation_gate(&state.data_path, &conversation.id)
            .expect("mutation gate");
        let guard = mutation_gate.lock().expect("hold mutation gate");
        state_schedule_conversation_persist(&state, &conversation).expect("schedule full persist");
        for _ in 0..100 {
            if state
                .conversation_persist_pending
                .lock()
                .expect("read pending")
                .is_none()
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(state
            .conversation_persist_pending
            .lock()
            .expect("pending snapshot taken")
            .is_none());

        state_schedule_conversation_delete(&state, &conversation.id)
            .expect("schedule delete after worker took snapshot");
        drop(guard);
        for _ in 0..100 {
            let pending_is_empty = state
                .conversation_persist_pending
                .lock()
                .expect("read pending")
                .is_none();
            let delete_flushed = !state
                .cached_deleted_conversation_ids
                .lock()
                .expect("read deleted ids")
                .contains(&conversation.id);
            if pending_is_empty && delete_flushed {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(state
            .conversation_persist_pending
            .lock()
            .expect("pending delete flushed")
            .is_none());
        assert!(!app_layout_chat_conversations_dir(&state.data_path)
            .join(&conversation.id)
            .exists());
        assert!(state_read_chat_index_cached(&state)
            .expect("read chat index")
            .conversations
            .iter()
            .all(|item| item.id != conversation.id));
    }

    #[test]
    fn pending_worker_metadata_taken_before_delete_should_not_rewrite_conversation() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-delete-worker-meta-race", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark persisted");
        start_conversation_persist_worker(&state).expect("start persist worker");

        let mutation_gate = conversation_mutation_gate(&state.data_path, &conversation.id)
            .expect("mutation gate");
        let guard = mutation_gate.lock().expect("hold mutation gate");
        state_update_conversation_metadata_cached(&state, &conversation.id, |cached| {
            cached.title = "must-not-write".to_string();
            Ok(())
        })
        .expect("schedule metadata update");
        for _ in 0..100 {
            if state
                .conversation_persist_pending
                .lock()
                .expect("read pending")
                .is_none()
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(state
            .conversation_persist_pending
            .lock()
            .expect("pending metadata taken")
            .is_none());

        state_schedule_conversation_delete(&state, &conversation.id)
            .expect("schedule delete after worker took metadata");
        drop(guard);
        for _ in 0..100 {
            let pending_is_empty = state
                .conversation_persist_pending
                .lock()
                .expect("read pending")
                .is_none();
            let delete_flushed = !state
                .cached_deleted_conversation_ids
                .lock()
                .expect("read deleted ids")
                .contains(&conversation.id);
            if pending_is_empty && delete_flushed {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(state
            .conversation_persist_pending
            .lock()
            .expect("pending delete flushed")
            .is_none());
        assert!(!app_layout_chat_conversations_dir(&state.data_path)
            .join(&conversation.id)
            .exists());
        assert!(state_read_chat_index_cached(&state)
            .expect("read chat index")
            .conversations
            .iter()
            .all(|item| item.id != conversation.id));
    }

    #[test]
    fn resolve_archive_request_by_id_should_allow_missing_department() {
        let state = test_chat_runtime_state();
        let mut config = AppConfig::default();
        if let Some(api_config) = config.api_configs.get_mut(0) {
            api_config.id = "api-archive".to_string();
            api_config.base_url = "https://api.openai.com/v1".to_string();
            api_config.api_key = "k".to_string();
            api_config.model = "gpt-4o-mini".to_string();
        }
        config.expert_api_config_id = "api-archive".to_string();
        let expected_api_id = api_endpoint_id("api-archive", "api-archive-model-default");
        write_config(&state.config_path, &config).expect("write config");
        let now = now_iso();
        let mut source = test_chat_conversation("conversation-archive-missing-dept", "active", &now);
        source.agent_id = String::new();
        source.messages = vec![
            test_text_message("user", "第一轮问题", &now),
            test_text_message("assistant", "第一轮回复", &now),
            test_text_message("user", "第二轮问题", &now),
            test_text_message("assistant", "第二轮回复", &now),
        ];
        write_conversation_shard(&state.data_path, &source).expect("write source conversation");

        let (selected_api, _resolved_api, resolved_source, effective_agent_id) =
            conversation_service_v2()
                .resolve_archive_request_conversation_by_id(&state, &source.id)
                .expect("resolve archive request without department");

        assert_eq!(resolved_source.id, source.id);
        assert_eq!(selected_api.id, expected_api_id);
        assert!(effective_agent_id.is_empty());
    }

    #[test]
    fn read_archive_block_page_should_reject_legacy_archive_without_v3_store() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut conversation =
            test_chat_conversation("conversation-archive-legacy-page", "active", &now);
        conversation.status = "archived".to_string();
        conversation.archived_at = Some(now.clone());
        conversation.messages = vec![
            test_text_message("user", "第一条", &now),
            test_text_message("assistant", "第二条", &now),
            test_text_message("user", "第三条", &now),
        ];
        let legacy_path =
            app_layout_chat_conversation_path(&state.data_path, &conversation.id);
        fs::create_dir_all(app_layout_chat_conversations_dir(&state.data_path))
            .expect("create conversations dir");
        fs::write(
            &legacy_path,
            serde_json::to_string_pretty(&conversation).expect("serialize legacy archive"),
        )
        .expect("write legacy archive");

        let paths = message_store::message_store_paths(&state.data_path, &conversation.id)
            .expect("message store paths");
        assert!(
            message_store::chat_store_read_status(&paths)
                .expect("read ready status before archive page")
                .is_none()
        );

        let err = match conversation_service_v2()
            .read_archive_block_page(&state, &conversation.id, None)
        {
            Ok(_) => panic!("legacy archive must not be migrated by production paging"),
            Err(err) => err,
        };

        assert!(err.contains("请先完成消息存储迁移"));
        assert!(message_store::chat_store_read_status(&paths)
            .expect("read status after rejected archive page")
            .is_none());
        assert!(legacy_path.exists());
    }

    #[test]
    fn list_remote_im_contact_conversations_should_skip_missing_conversation() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let now = now_iso();
        let contact = RemoteImContact {
            id: "contact-a".to_string(),
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "group".to_string(),
            remote_contact_id: "remote-a".to_string(),
            remote_contact_name: "测试群".to_string(),
            avatar_url: String::new(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "never".to_string(),
            activation_keywords: Vec::new(),
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_agent_id: None,
            bound_conversation_id: None,
            processing_mode: "continuous".to_string(),
            response_strategy: default_remote_im_contact_response_strategy(),
            response_guidance: default_remote_im_contact_response_guidance(),
            blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
            group_reply_pacing: RemoteImGroupReplyPacing::default(),
            last_activated_at: None,
            last_message_at: Some(now),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        };
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        let items = conversation_service_v2()
            .list_remote_im_contact_conversations(&state)
            .expect("list remote im contact conversations");

        assert!(items.is_empty());
        let persisted = state_service_get_remote_im_contact(&state, "contact-a")
            .expect("read contact")
            .expect("contact exists");
        assert!(persisted.bound_conversation_id.is_none());
    }

    fn seed_session_forward_test_state() -> (AppState, String, String, String) {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut config = AppConfig::default();
        config.remote_im_channels.push(RemoteImChannelConfig {
            id: "remote-channel-a".to_string(),
            name: "测试渠道".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            enabled: true,
            credentials: serde_json::json!({ "mockSend": true }),
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            filter_markdown: false,
            allow_send_files: false,
            behavior_settings: RemoteImChannelBehaviorSettings::default(),
        });
        write_config(&state.config_path, &config).expect("write config");

        let mut agent = default_agent();
        agent.id = DEFAULT_AGENT_ID.to_string();
        agent.name = "通知人格".to_string();
        state_write_agents_cached(&state, &[agent, default_user_persona()])
            .expect("write agents");

        let mut source = test_chat_conversation("source-session", "active", &now);
        source.title = "源会话".to_string();
        source.messages.push(test_text_message("user", "第一条原消息", &now));
        source.messages.push(test_text_message("assistant", "第二条原消息", &now));
        source.last_assistant_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &source).expect("persist source");

        let mut target_local = test_chat_conversation("target-local-session", "active", &now);
        target_local.title = "本地目标".to_string();
        state_schedule_conversation_persist(&state, &target_local).expect("persist local target");

        let contact = RemoteImContact {
            id: "contact-session-a".to_string(),
            channel_id: "remote-channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "private".to_string(),
            remote_contact_id: "remote-user-a".to_string(),
            remote_contact_name: "联系人乙".to_string(),
            avatar_url: String::new(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "never".to_string(),
            activation_keywords: Vec::new(),
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            bound_conversation_id: Some("target-remote-session".to_string()),
            processing_mode: "continuous".to_string(),
            response_strategy: default_remote_im_contact_response_strategy(),
            response_guidance: default_remote_im_contact_response_guidance(),
            blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
            group_reply_pacing: RemoteImGroupReplyPacing::default(),
            last_activated_at: None,
            last_message_at: Some(now.clone()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        };
        let mut target_remote = build_conversation_record(
            "",
            DEFAULT_AGENT_ID,
            "联系人会话",
            CONVERSATION_KIND_REMOTE_IM_CONTACT,
            Some(remote_im_contact_conversation_key(&contact)),
            None,
        );
        target_remote.id = "target-remote-session".to_string();
        target_remote.updated_at = now.clone();
        state_schedule_conversation_persist(&state, &target_remote).expect("persist remote target");
        flush_pending_persists_blocking(&state).expect("flush seeded sessions");

        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        (
            state,
            "source-session".to_string(),
            "target-local-session".to_string(),
            "target-remote-session".to_string(),
        )
    }

    fn wait_for_session_notification(
        state: &AppState,
        conversation_id: &str,
    ) -> Conversation {
        let mut last_error = None;
        for _ in 0..20 {
            match state_read_conversation_cached(state, conversation_id) {
                Ok(conversation) if !conversation.messages.is_empty() => return conversation,
                Ok(_) => last_error = None,
                Err(err) => last_error = Some(err),
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        match state_read_conversation_cached(state, conversation_id) {
            Ok(conversation) => conversation,
            Err(err) => panic!(
                "read notification target after timeout: {err}; previous_error={:?}",
                last_error
            ),
        }
    }

    #[test]
    fn list_tool_session_targets_should_include_local_and_remote_sessions() {
        let (state, _source_id, _local_target_id, remote_target_id) = seed_session_forward_test_state();

        let all_items = conversation_service_v2()
            .list_tool_session_targets(&state, None)
            .expect("list session targets");
        assert_eq!(all_items.len(), 3);
        assert!(all_items.iter().any(|item| item.session_id == remote_target_id && item.kind == "remote_im_contact"));
        assert!(all_items.iter().any(|item| item.title == "本地目标" && item.kind == "local_unarchived"));

        let remote_items = conversation_service_v2()
            .list_tool_session_targets(&state, Some("联系人乙"))
            .expect("filter remote session targets");
        assert_eq!(remote_items.len(), 1);
        assert_eq!(remote_items[0].kind, "remote_im_contact");
        assert_eq!(remote_items[0].remote_contact_name.as_deref(), Some("联系人乙"));
        assert_eq!(remote_items[0].channel_name.as_deref(), Some("测试渠道"));
    }

    #[test]
    fn inform_session_should_append_notification_to_local_conversation() {
        let (state, source_id, target_local_id, _remote_target_id) = seed_session_forward_test_state();

        let result = conversation_service_v2()
            .inform_session(&state, &source_id, &target_local_id, "请跟进")
            .expect("inform local session");

        assert_eq!(result.target_kind, "queued");
        assert!(!result.pushed_to_remote);
        // 本地会话走引导队列：空会话批次先补种初始摘要，通知消息随后写入并激活主助理。
        let target = {
            let mut found = None;
            for _ in 0..60 {
                if let Ok(conversation) = state_read_conversation_cached(&state, &target_local_id) {
                    if let Some(index) = conversation.messages.iter().position(|message| {
                        message
                            .provider_meta
                            .as_ref()
                            .and_then(|meta| meta.get("messageKind"))
                            .and_then(Value::as_str)
                            == Some("session_notification")
                    }) {
                        found = Some(conversation);
                        let _ = index;
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            found.expect("通知消息应写入本地目标会话历史")
        };
        let notification_index = target
            .messages
            .iter()
            .position(|message| {
                message
                    .provider_meta
                    .as_ref()
                    .and_then(|meta| meta.get("messageKind"))
                    .and_then(Value::as_str)
                    == Some("session_notification")
            })
            .expect("通知消息应写入本地目标会话历史");
        let notification = &target.messages[notification_index];
        assert_eq!(notification.role, "assistant");
        assert_eq!(
            notification.speaker_agent_id.as_deref(),
            Some(SYSTEM_PERSONA_ID)
        );
        match &notification.parts[0] {
            MessagePart::Text { text, .. } => {
                assert_eq!(text, "[源会话·通知人格]:请跟进");
            }
            _ => panic!("expected text notification"),
        }
    }

    #[test]
    fn inform_session_should_append_notification_to_remote_contact_conversation() {
        let (state, source_id, _target_local_id, remote_target_id) = seed_session_forward_test_state();

        let result = conversation_service_v2()
            .inform_session(&state, &source_id, &remote_target_id, "同步一下")
            .expect("inform remote session");

        assert_eq!(result.target_kind, "queued");
        assert!(!result.pushed_to_remote);
        let target = wait_for_session_notification(&state, &remote_target_id);
        assert_eq!(target.messages.len(), 1);
        match &target.messages[0].parts[0] {
            MessagePart::Text { text, .. } => {
                assert_eq!(text, "[源会话·通知人格]:同步一下");
            }
            _ => panic!("expected text notification"),
        }
    }

    #[test]
    fn auto_push_remote_contact_should_not_depend_on_contact_list_snapshot() {
        let (state, source_id, _target_local_id, remote_target_id) = seed_session_forward_test_state();
        let target =
            state_read_conversation_cached(&state, &remote_target_id).expect("read remote target");
        let store_paths = message_store::message_store_paths(&state.data_path, &remote_target_id)
            .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &target)
            .expect("write message store");

        conversation_service_v2()
            .enqueue_auto_push_remote_contact_message(
                &state,
                &source_id,
                "contact-session-a",
                "自动推送正文",
            )
            .expect("enqueue auto push remote contact");

        let target = wait_for_session_notification(&state, &remote_target_id);
        assert_eq!(target.messages.len(), 1);
        match &target.messages[0].parts[0] {
            MessagePart::Text { text, .. } => {
                assert_eq!(text, "[源会话·通知人格]:自动推送正文");
            }
            _ => panic!("expected text notification"),
        }
    }

    #[test]
    fn forward_selection_to_remote_im_contact_should_append_single_notification_message() {
        let (state, source_id, _target_local_id, remote_target_id) = seed_session_forward_test_state();
        let source = state_read_conversation_cached(&state, &source_id).expect("read source");
        let selected_message_ids = source
            .messages
            .iter()
            .map(|message| message.id.clone())
            .collect::<Vec<_>>();

        let result = conversation_service_v2()
            .forward_selection_to_remote_im_contact(
                &state,
                &source_id,
                &remote_target_id,
                "contact-session-a",
                &selected_message_ids,
            )
            .expect("forward selection to remote contact");

        assert_eq!(result.forwarded_count, 2);
        let target = wait_for_session_notification(&state, &remote_target_id);
        assert_eq!(target.messages.len(), 1);
        assert_eq!(target.messages[0].role, "assistant");
        assert_eq!(
            target.messages[0].speaker_agent_id.as_deref(),
            Some(SYSTEM_PERSONA_ID)
        );
        match &target.messages[0].parts[0] {
            MessagePart::Text { text, .. } => {
                assert_eq!(
                    text,
                    "[源会话·通知人格]:[用户]: 第一条原消息\n\n[助手]: 第二条原消息"
                );
            }
            _ => panic!("expected text notification"),
        }
    }

    #[test]
    fn list_remote_im_contact_conversations_should_reuse_existing_history_and_rebind_contact() {
        let state = test_chat_runtime_state();
        let mut config = AppConfig::default();
        config.remote_im_channels.push(RemoteImChannelConfig {
            id: "channel-a".to_string(),
            name: "测试渠道".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            enabled: true,
            credentials: serde_json::json!({}),
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            filter_markdown: false,
            allow_send_files: false,
            behavior_settings: RemoteImChannelBehaviorSettings::default(),
        });
        write_config(&state.config_path, &config).expect("write config");
        let now = now_iso();
        let contact = RemoteImContact {
            id: "contact-a".to_string(),
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "group".to_string(),
            remote_contact_id: "remote-a".to_string(),
            remote_contact_name: "测试群".to_string(),
            avatar_url: String::new(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "never".to_string(),
            activation_keywords: Vec::new(),
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_agent_id: None,
            bound_conversation_id: None,
            processing_mode: "continuous".to_string(),
            response_strategy: default_remote_im_contact_response_strategy(),
            response_guidance: default_remote_im_contact_response_guidance(),
            blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
            group_reply_pacing: RemoteImGroupReplyPacing::default(),
            last_activated_at: None,
            last_message_at: Some(now.clone()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        };
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        let mut conversation = build_conversation_record(
            "",
            "",
            "联系人 · 测试群",
            CONVERSATION_KIND_REMOTE_IM_CONTACT,
            Some(remote_im_contact_conversation_key(&contact)),
            None,
        );
        conversation.id = "conversation-contact-old".to_string();
        conversation.messages.push(test_text_message("user", "历史消息", &now));
        conversation.updated_at = now.clone();
        conversation.last_user_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");
        flush_pending_persists_blocking(&state).expect("flush contact conversation");

        let items = conversation_service_v2()
            .list_remote_im_contact_conversations(&state)
            .expect("list remote im contact conversations");

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].conversation_id, "conversation-contact-old");
        assert_eq!(items[0].message_count, 1);
        let updated_contact = state_service_get_remote_im_contact(&state, "contact-a")
            .expect("read contact")
            .expect("contact exists");
        assert_eq!(
            updated_contact.bound_conversation_id.as_deref(),
            Some("conversation-contact-old")
        );
        let updated_conversation =
            state_read_conversation_cached(&state, "conversation-contact-old")
                .expect("read rebound conversation");
        assert_eq!(updated_conversation.agent_id, DEFAULT_AGENT_ID);
    }

    #[tokio::test]
    async fn remote_im_contact_conversation_should_be_readable_and_writable_as_unarchived() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let now = now_iso();
        let contact = RemoteImContact {
            id: "contact-a".to_string(),
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "group".to_string(),
            remote_contact_id: "remote-a".to_string(),
            remote_contact_name: "测试群".to_string(),
            avatar_url: String::new(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "never".to_string(),
            activation_keywords: Vec::new(),
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_agent_id: None,
            bound_conversation_id: Some("conversation-contact-old".to_string()),
            processing_mode: "continuous".to_string(),
            response_strategy: default_remote_im_contact_response_strategy(),
            response_guidance: default_remote_im_contact_response_guidance(),
            blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
            group_reply_pacing: RemoteImGroupReplyPacing::default(),
            last_activated_at: None,
            last_message_at: Some(now.clone()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        };
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        let mut conversation = build_conversation_record(
            "",
            "",
            "联系人 · 测试群",
            CONVERSATION_KIND_REMOTE_IM_CONTACT,
            Some(remote_im_contact_conversation_key(&contact)),
            None,
        );
        conversation.id = "conversation-contact-old".to_string();
        conversation.messages.push(test_text_message("user", "历史消息", &now));
        conversation.updated_at = now.clone();
        conversation.last_user_at = Some(now.clone());
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");
        flush_pending_persists_blocking(&state).expect("flush contact conversation");

        let messages = conversation_service_v2()
            .read_unarchived_messages(&state, "conversation-contact-old")
            .expect("read remote im contact conversation as unarchived");
        assert_eq!(messages.len(), 1);
        match &messages[0].parts[0] {
            MessagePart::Text { text, .. } => assert_eq!(text, "历史消息"),
            _ => panic!("expected text message"),
        }

        conversation_service_v2()
            .update_unarchived_conversation_by_id(
                &state,
                "conversation-contact-old",
                |conversation| {
                    conversation.title = "联系人 · 已改名".to_string();
                    Ok(())
                },
            )
            .await
            .expect("update remote im contact conversation as unarchived");
        let updated = state_read_conversation_cached(&state, "conversation-contact-old")
            .expect("read updated conversation");
        assert_eq!(updated.title, "联系人 · 已改名");
    }

    #[test]
    fn rewind_remote_im_contact_conversation_should_hydrate_messages_from_store() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let now = now_iso();
        let mut first_user = test_text_message("user", "第一句", &now);
        first_user.id = "user-1".to_string();
        let mut first_assistant = test_text_message("assistant", "第一句回复", &now);
        first_assistant.id = "assistant-1".to_string();
        let mut recalled_user = test_text_message("user", "需要撤回", &now);
        recalled_user.id = "user-2".to_string();
        let mut trailing_assistant = test_text_message("assistant", "后续回复", &now);
        trailing_assistant.id = "assistant-2".to_string();
        let mut conversation = build_conversation_record(
            "",
            DEFAULT_AGENT_ID,
            "联系人 · 测试群",
            CONVERSATION_KIND_REMOTE_IM_CONTACT,
            Some("remote_im_contact:channel-a:group:remote-a".to_string()),
            None,
        );
        conversation.id = "conversation-contact-rewind".to_string();
        conversation.status = "inactive".to_string();
        conversation.messages = vec![
            first_user,
            first_assistant,
            recalled_user.clone(),
            trailing_assistant,
        ];
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist full conversation");
        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        conversation.messages = Vec::new();
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist slim conversation");

        let input = RewindConversationInput {
            session: SessionSelector {
                api_config_id: None,
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_id: Some(conversation.id.clone()),
            },
            message_id: recalled_user.id.clone(),
            undo_apply_patch: false,
        };
        let result = conversation_service_v2()
            .rewind_conversation_from_message(
                &state,
                &input,
                &recalled_user.id,
                &std::time::Instant::now(),
            )
            .expect("rewind remote im contact conversation");

        assert_eq!(result.removed_count, 2);
        assert_eq!(result.remaining_count, 2);
        assert_eq!(
            result
                .recalled_user_message
                .as_ref()
                .map(|message| message.id.as_str()),
            Some("user-2")
        );
        let stored = message_store::chat_store_read_all_messages(&store_paths)
            .expect("read truncated message store")
            .expect("message store exists");
        assert_eq!(
            stored
                .iter()
                .map(|message| message.id.as_str())
                .collect::<Vec<_>>(),
            vec!["user-1", "assistant-1"]
        );
    }

    #[test]
    fn rewind_conversation_should_rebuild_message_derived_meta_from_store() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let now = now_iso();
        let mut first_user = test_text_message("user", "第一句", &now);
        first_user.id = "user-1".to_string();
        let mut first_assistant = test_text_message("assistant", "第一句回复", &now);
        first_assistant.id = "assistant-1".to_string();
        let mut recalled_user = test_text_message("user", "需要撤回", &now);
        recalled_user.id = "user-2".to_string();
        let mut trailing_assistant = test_text_message("assistant", "后续回复", &now);
        trailing_assistant.id = "assistant-2".to_string();
        let mut conversation = build_conversation_record(
            "",
            DEFAULT_AGENT_ID,
            "撤回重建 metadata",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        conversation.id = "conversation-rewind-rebuild-meta".to_string();
        conversation.status = "active".to_string();
        conversation.messages = vec![
            first_user,
            first_assistant,
            recalled_user.clone(),
            trailing_assistant,
        ];
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");
        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        state_mark_conversation_direct_persisted(&state, &conversation)
            .expect("mark direct persisted");

        state_update_conversation_metadata_cached(&state, &conversation.id, |cached| {
            cached.updated_at = now.clone();
            Ok(())
        })
        .expect("prime metadata pending");
        {
            let mut cached = state
                .cached_conversation_metadata
                .lock()
                .expect("lock cached conversation metadata");
            let current = cached
                .get(&conversation.id)
                .cloned()
                .expect("cached meta exists");
            let mut broken_conversation =
                conversation_service_v2().build_conversation_snapshot_from_meta(&current, Vec::new());
            broken_conversation.messages = vec![
                test_text_message("user", "伪造一", &now),
                test_text_message("assistant", "伪造二", &now),
                test_text_message("user", "伪造三", &now),
                test_text_message("assistant", "伪造四", &now),
                test_text_message("user", "伪造五", &now),
            ];
            let broken = message_store::ConversationShardMeta::from_conversation(
                &broken_conversation,
            );
            cached.insert(conversation.id.clone(), broken);
        }

        let input = RewindConversationInput {
            session: SessionSelector {
                api_config_id: None,
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_id: Some(conversation.id.clone()),
            },
            message_id: recalled_user.id.clone(),
            undo_apply_patch: false,
        };
        let result = conversation_service_v2()
            .rewind_conversation_from_message(
                &state,
                &input,
                &recalled_user.id,
                &std::time::Instant::now(),
            )
            .expect("rewind conversation with stale cached meta");

        assert_eq!(result.removed_count, 2);
        assert_eq!(result.remaining_count, 2);
        let ready_meta = message_store::chat_store_read_meta(&store_paths)
            .expect("read ready meta after rewind")
            .expect("ready meta exists after rewind");
        assert_eq!(ready_meta.message_count(), 2);
        assert_eq!(ready_meta.body_message_count(), 2);
        assert_eq!(ready_meta.body_text_length(), 8);
        assert_eq!(ready_meta.preview_messages().len(), 2);
    }

    fn setup_rewind_busy_test_conversation(
        conversation_id: &str,
    ) -> (AppState, RewindConversationInput, String) {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let now = now_iso();
        let mut first_user = test_text_message("user", "第一句", &now);
        first_user.id = "user-1".to_string();
        let mut first_assistant = test_text_message("assistant", "第一句回复", &now);
        first_assistant.id = "assistant-1".to_string();
        let mut recalled_user = test_text_message("user", "需要撤回", &now);
        recalled_user.id = "user-2".to_string();
        let mut trailing_assistant = test_text_message("assistant", "后续回复", &now);
        trailing_assistant.id = "assistant-2".to_string();
        let mut conversation = build_conversation_record(
            "",
            DEFAULT_AGENT_ID,
            "撤回忙碌态测试",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        conversation.id = conversation_id.to_string();
        conversation.status = "active".to_string();
        conversation.messages = vec![
            first_user,
            first_assistant,
            recalled_user.clone(),
            trailing_assistant,
        ];
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");
        let store_paths =
            message_store::message_store_paths(&state.data_path, &conversation.id)
                .expect("message store paths");
        message_store::chat_store_write_snapshot(&store_paths, &conversation)
            .expect("write message store");
        let input = RewindConversationInput {
            session: SessionSelector {
                api_config_id: None,
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_id: Some(conversation.id.clone()),
            },
            message_id: recalled_user.id.clone(),
            undo_apply_patch: false,
        };
        (state, input, recalled_user.id)
    }

    #[test]
    fn rewind_conversation_should_fail_without_mutation_while_assistant_streaming() {
        let (state, input, recalled_user_id) =
            setup_rewind_busy_test_conversation("conversation-rewind-streaming");
        set_conversation_runtime_state(
            &state,
            "conversation-rewind-streaming",
            MainSessionState::AssistantStreaming,
        )
        .expect("set runtime state");

        let result = conversation_service_v2().rewind_conversation_from_message(
            &state,
            &input,
            &recalled_user_id,
            &std::time::Instant::now(),
        );

        assert!(result.is_err());
        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("当前会话正在运行或整理上下文")
        );
        let cached = state_read_conversation_cached(&state, "conversation-rewind-streaming")
            .expect("read cached conversation");
        assert_eq!(cached.messages.len(), 4);
        let store_paths =
            message_store::message_store_paths(&state.data_path, "conversation-rewind-streaming")
                .expect("message store paths");
        let stored = message_store::chat_store_read_all_messages(&store_paths)
            .expect("read message store")
            .expect("message store exists");
        assert_eq!(stored.len(), 4);
    }

    #[test]
    fn rewind_conversation_should_fail_without_mutation_while_organizing_context() {
        let (state, input, recalled_user_id) =
            setup_rewind_busy_test_conversation("conversation-rewind-organizing");
        set_conversation_runtime_state(
            &state,
            "conversation-rewind-organizing",
            MainSessionState::OrganizingContext,
        )
        .expect("set runtime state");

        let result = conversation_service_v2().rewind_conversation_from_message(
            &state,
            &input,
            &recalled_user_id,
            &std::time::Instant::now(),
        );

        assert!(result.is_err());
        let cached = state_read_conversation_cached(&state, "conversation-rewind-organizing")
            .expect("read cached conversation");
        assert_eq!(cached.messages.len(), 4);
        let store_paths =
            message_store::message_store_paths(&state.data_path, "conversation-rewind-organizing")
                .expect("message store paths");
        let stored = message_store::chat_store_read_all_messages(&store_paths)
            .expect("read message store")
            .expect("message store exists");
        assert_eq!(stored.len(), 4);
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
    fn mark_queue_event_guided_should_force_activation_and_guided_dispatch_reason() {
        let state = test_chat_runtime_state();
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::AssistantStreaming)
            .expect("set streaming state");
        let mut event = test_pending_event("conversation-a");
        event.activate_assistant = false;
        event.runtime_context = Some(runtime_context_new("user_message", "user_send"));

        let ingress = ingress_chat_event(&state, event).expect("queue event");
        let event_id = match ingress {
            ChatEventIngress::Queued { event_id } => event_id,
            _ => panic!("expected queued guided candidate"),
        };

        let updated_conversation_id =
            mark_queue_event_guided(&state, &event_id).expect("mark guided");
        assert_eq!(updated_conversation_id.as_deref(), Some("conversation-a"));

        let slots = state
            .conversation_runtime_slots
            .lock()
            .expect("lock slots");
        let slot = slots.get("conversation-a").expect("conversation slot");
        let queued = slot
            .pending_queue
            .iter()
            .find(|item| item.id == event_id)
            .expect("guided event still queued");
        assert_eq!(queued.queue_mode, ChatQueueMode::Guided);
        assert!(queued.activate_assistant);
        assert_eq!(
            queued
                .runtime_context
                .as_ref()
                .and_then(|ctx| ctx.dispatch_reason.as_deref()),
            Some("guided_queue")
        );
    }

    #[test]
    fn mark_queue_event_guided_should_allow_remote_im_queue_event() {
        let state = test_chat_runtime_state();
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::AssistantStreaming)
            .expect("set streaming state");
        let mut event = test_pending_event("conversation-a");
        event.source = ChatEventSource::RemoteIm;
        event.runtime_context = Some(runtime_context_new("remote_im", "remote_im_enqueue"));

        let ingress = ingress_chat_event(&state, event).expect("queue event");
        let event_id = match ingress {
            ChatEventIngress::Queued { event_id } => event_id,
            _ => panic!("expected queued remote im candidate"),
        };

        let updated_conversation_id =
            mark_queue_event_guided(&state, &event_id).expect("mark guided");
        assert_eq!(updated_conversation_id.as_deref(), Some("conversation-a"));

        let slots = state
            .conversation_runtime_slots
            .lock()
            .expect("lock slots");
        let slot = slots.get("conversation-a").expect("conversation slot");
        let queued = slot
            .pending_queue
            .iter()
            .find(|item| item.id == event_id)
            .expect("guided event still queued");
        assert_eq!(queued.queue_mode, ChatQueueMode::Guided);
        assert!(queued.activate_assistant);
        assert_eq!(
            queued
                .runtime_context
                .as_ref()
                .and_then(|ctx| ctx.dispatch_reason.as_deref()),
            Some("guided_queue")
        );
    }

    #[test]
    fn recall_chat_queue_event_inner_should_report_not_in_queue_on_miss() {
        let state = test_chat_runtime_state();
        let result =
            recall_chat_queue_event_inner("missing-event-id", &state).expect("recall miss");
        assert!(!result.removed);
        assert!(result.not_in_queue);
    }

    #[test]
    fn mark_chat_queue_event_guided_inner_should_report_not_in_queue_on_miss() {
        let state = test_chat_runtime_state();
        let result =
            mark_chat_queue_event_guided_inner("missing-event-id", &state).expect("mark miss");
        assert!(!result.updated);
        assert!(result.not_in_queue);
    }

    #[test]
    fn recall_chat_queue_event_inner_should_report_removed_on_hit() {
        let state = test_chat_runtime_state();
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::AssistantStreaming)
            .expect("set streaming state");
        let ingress =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("queue event");
        let event_id = match ingress {
            ChatEventIngress::Queued { event_id } => event_id,
            _ => panic!("expected queued recall candidate"),
        };

        let result = recall_chat_queue_event_inner(&event_id, &state).expect("recall hit");
        assert!(result.removed);
        assert!(!result.not_in_queue);
        assert_eq!(result.message_text, "hello");
    }

    #[test]
    fn claim_queued_conversation_batches_should_only_take_one_normal_event_per_round() {
        let state = test_chat_runtime_state();
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::AssistantStreaming)
            .expect("set streaming state");

        let ingress_first =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("queue first");
        let first_event_id = match ingress_first {
            ChatEventIngress::Queued { event_id } => event_id,
            _ => panic!("expected first event queued"),
        };
        let ingress_second =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("queue second");
        let second_event_id = match ingress_second {
            ChatEventIngress::Queued { event_id } => event_id,
            _ => panic!("expected second event queued"),
        };

        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::Idle)
            .expect("restore idle");

        let claimed_batches = claim_queued_conversation_batches(&state).expect("claim queued batches");
        assert_eq!(claimed_batches.len(), 1);
        assert_eq!(claimed_batches[0].0, "conversation-a");
        assert_eq!(claimed_batches[0].1.len(), 1);
        assert_eq!(claimed_batches[0].1[0].id, first_event_id);

        let slots = state
            .conversation_runtime_slots
            .lock()
            .expect("lock slots");
        let slot = slots.get("conversation-a").expect("conversation slot");
        assert_eq!(slot.pending_queue.len(), 1);
        assert_eq!(slot.pending_queue[0].id, second_event_id);
        drop(slots);

        let claims = state
            .conversation_processing_claims
            .lock()
            .expect("lock claims");
        assert!(claims.contains("conversation-a"));
    }

    #[test]
    fn claim_queued_conversation_batches_should_skip_conversation_when_guided_exists() {
        let state = test_chat_runtime_state();
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::AssistantStreaming)
            .expect("set streaming state");

        let guided_ingress =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("queue guided");
        let guided_event_id = match guided_ingress {
            ChatEventIngress::Queued { event_id } => event_id,
            _ => panic!("expected guided event queued"),
        };
        let normal_ingress =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("queue normal");
        let normal_event_id = match normal_ingress {
            ChatEventIngress::Queued { event_id } => event_id,
            _ => panic!("expected normal event queued"),
        };

        mark_queue_event_guided(&state, &guided_event_id).expect("mark guided");
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::Idle)
            .expect("restore idle");

        let claimed_batches = claim_queued_conversation_batches(&state).expect("claim queued batches");
        assert!(claimed_batches.is_empty());

        let slots = state
            .conversation_runtime_slots
            .lock()
            .expect("lock slots");
        let slot = slots.get("conversation-a").expect("conversation slot");
        assert_eq!(slot.pending_queue.len(), 2);
        assert_eq!(slot.pending_queue[0].id, guided_event_id);
        assert_eq!(slot.pending_queue[1].id, normal_event_id);
        drop(slots);

        let claims = state
            .conversation_processing_claims
            .lock()
            .expect("lock claims");
        assert!(!claims.contains("conversation-a"));
    }

    #[test]
    fn claim_guided_queue_events_for_conversation_should_remove_guided_and_keep_normal_events() {
        let state = test_chat_runtime_state();
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::AssistantStreaming)
            .expect("set streaming state");

        let guided_ingress =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("queue guided");
        let guided_event_id = match guided_ingress {
            ChatEventIngress::Queued { event_id } => event_id,
            _ => panic!("expected first event queued"),
        };
        let normal_ingress =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("queue normal");
        let normal_event_id = match normal_ingress {
            ChatEventIngress::Queued { event_id } => event_id,
            _ => panic!("expected second event queued"),
        };

        mark_queue_event_guided(&state, &guided_event_id).expect("mark guided");
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::Idle)
            .expect("restore idle");

        let guided_events =
            claim_guided_queue_events_for_conversation(&state, "conversation-a").expect("claim guided");

        assert_eq!(guided_events.len(), 1);
        assert_eq!(guided_events[0].id, guided_event_id);
        assert_eq!(guided_events[0].queue_mode, ChatQueueMode::Guided);
        assert!(guided_events[0].activate_assistant);
        assert_eq!(
            guided_events[0]
                .runtime_context
                .as_ref()
                .and_then(|ctx| ctx.dispatch_reason.as_deref()),
            Some("guided_queue")
        );

        let slots = state
            .conversation_runtime_slots
            .lock()
            .expect("lock slots");
        let slot = slots.get("conversation-a").expect("conversation slot");
        assert_eq!(slot.state, MainSessionState::Idle);
        assert_eq!(slot.pending_queue.len(), 1);
        assert_eq!(slot.pending_queue[0].id, normal_event_id);
        drop(slots);

        let claims = state
            .conversation_processing_claims
            .lock()
            .expect("lock claims");
        assert!(claims.contains("conversation-a"));
    }

    #[test]
    fn process_guided_queue_when_idle_should_remove_claimed_guided_event_after_failure() {
        let state = test_chat_runtime_state();
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::AssistantStreaming)
            .expect("set streaming state");
        let ingress =
            ingress_chat_event(&state, test_pending_event("conversation-a")).expect("queue event");
        let event_id = match ingress {
            ChatEventIngress::Queued { event_id } => event_id,
            _ => panic!("expected queued event"),
        };
        mark_queue_event_guided(&state, &event_id).expect("mark guided");
        set_conversation_runtime_state(&state, "conversation-a", MainSessionState::Idle)
            .expect("restore idle");

        let err = test_runtime()
            .block_on(process_guided_queue_when_idle(&state, "conversation-a"))
            .expect_err("guided processing should fail without conversation");
        assert!(err.contains("目标会话不存在"));
        assert_eq!(total_queue_len(&state).expect("queue len"), 0);
        assert_eq!(
            get_conversation_runtime_state(&state, "conversation-a")
                .expect("runtime state"),
            MainSessionState::Idle
        );
        let claims = state
            .conversation_processing_claims
            .lock()
            .expect("lock claims");
        assert!(!claims.contains("conversation-a"));
    }

    #[test]
    fn guided_batch_should_fail_when_history_flushed_without_activation() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-guided-no-activation", "active", &now);
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");

        let mut guided_event = test_pending_event("conversation-guided-no-activation");
        guided_event.queue_mode = ChatQueueMode::Guided;
        guided_event.activate_assistant = false;
        guided_event.runtime_context = Some(runtime_context_new("user_message", "guided_queue"));

        let err = test_runtime()
            .block_on(process_conversation_batch(
                &state,
                "conversation-guided-no-activation",
                vec![guided_event],
            ))
            .expect_err("guided batch should fail");
        assert_eq!(err, "引导消息未能触发助理回复");

        let updated = state_read_conversation_cached(&state, "conversation-guided-no-activation")
            .expect("read updated conversation");
        assert!(updated.messages.len() >= 1);
        assert_eq!(
            updated
                .messages
                .last()
                .map(|message| message.role.as_str()),
            Some("user")
        );
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
        let state = test_chat_runtime_state();
        let now = now_iso();
        let later = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format later");
        let mut data = AppData::default();
        data.conversations = vec![
            test_chat_conversation("conversation-main", "inactive", &now),
            test_chat_conversation("conversation-sub", "active", &later),
        ];

        let idx = ensure_main_conversation_index(&mut data, &state, "", DEFAULT_AGENT_ID).expect("ensure main conversation index");

        assert_eq!(data.conversations[idx].id, SYSTEM_NOTIFICATION_CONVERSATION_ID);
        assert_eq!(data.conversations[idx].title, "P-ai系统");
        assert_eq!(
            data.conversations[idx].conversation_kind,
            CONVERSATION_KIND_SYSTEM_NOTIFICATION
        );
        assert_eq!(
            state_service_get_main_conversation_id(&state)
                .expect("read main conversation id")
                .as_deref(),
            Some(SYSTEM_NOTIFICATION_CONVERSATION_ID)
        );
        let previous_main = data
            .conversations
            .iter()
            .find(|conversation| conversation.id == "conversation-main")
            .expect("previous main conversation should stay as a normal chat");
        assert!(conversation_is_local_normal_chat(previous_main));
    }

    #[test]
    fn normalize_system_notification_conversation_should_restore_fixed_title() {
        let mut conversation = build_system_notification_conversation_record();
        conversation.title = "用户改过的名字".to_string();

        let changed = normalize_system_notification_conversation(&mut conversation);

        assert!(changed);
        assert_eq!(conversation.title, "P-ai系统");
    }

    #[test]
    fn notification_conversation_display_title_should_prefer_title_then_summary_then_time() {
        let expected_time_title = chrono::DateTime::parse_from_rfc3339("2026-07-13T10:20:00+08:00")
            .expect("parse fallback time")
            .with_timezone(&chrono::Local)
            .format("%m/%d %H:%M")
            .to_string();
        assert_eq!(
            notification_conversation_display_title_from_parts(
                "conversation-a",
                "正式标题",
                Some("摘要标题"),
                Some("2026-07-13T10:20:00+08:00"),
                "2026-07-13T10:00:00+08:00",
                "zh-CN",
            ),
            "正式标题"
        );
        assert_eq!(
            notification_conversation_display_title_from_parts(
                "conversation-a",
                "conversation-a",
                Some("摘要标题"),
                Some("2026-07-13T10:20:00+08:00"),
                "2026-07-13T10:00:00+08:00",
                "zh-CN",
            ),
            "摘要标题"
        );
        assert_eq!(
            notification_conversation_display_title_from_parts(
                "conversation-a",
                "",
                None,
                Some("2026-07-13T10:20:00+08:00"),
                "2026-07-13T10:00:00+08:00",
                "zh-CN",
            ),
            expected_time_title
        );
    }

    #[test]
    fn notification_title_from_parts_should_append_failure_suffix() {
        assert_eq!(
            notification_title_from_parts("会话标题", "zh-CN", false),
            "会话标题"
        );
        assert_eq!(
            notification_title_from_parts("会话标题", "zh-CN", true),
            "会话标题 · 失败"
        );
        assert_eq!(
            notification_title_from_parts("Session", "en-US", true),
            "Session · Failed"
        );
    }

    #[test]
    fn normalize_single_active_main_conversation_should_keep_inactive_main_foreground_chat_active() {
        let now = now_iso();
        let later = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format later");
        let mut data = AppData::default();
        let main = test_chat_conversation("conversation-main", "inactive", &now);
        data.conversations = vec![main, test_chat_conversation("conversation-sub", "active", &later)];

        let changed = normalize_single_active_main_conversation(&mut data);

        assert!(changed);
        assert_eq!(data.conversations[0].status, "active");
        assert_eq!(data.conversations[1].status, "active");
    }

    #[test]
    fn conversation_is_archived_should_require_archive_fields() {
        let now = now_iso();
        let conversation = test_chat_conversation("conversation-summary-only", "active", &now);

        assert!(!conversation_is_archived(&conversation));

        let item = build_chat_index_item(&conversation);
        assert!(!chat_index_item_is_archived(&item));
    }

    #[test]
    fn normalize_single_active_main_conversation_should_keep_all_foreground_chats_active() {
        let now = now_iso();
        let later = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format later");
        let mut data = AppData::default();
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
            agent_id: Some(DEFAULT_AGENT_ID.to_string()),
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
                cron_expression: None,
                legacy_every_minutes: None,
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
    fn task_resolve_dispatch_session_should_use_conversation_owner_when_task_owner_missing() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");

        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");
        let task = TaskRecordStored {
            task_id: "task-missing-owner".to_string(),
            conversation_id: Some("conversation-sub".to_string()),
            agent_id: None,
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
                cron_expression: None,
                legacy_every_minutes: None,
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
        assert_eq!(session.agent_id, DEFAULT_AGENT_ID);
    }

    #[test]
    fn task_resolve_dispatch_session_should_return_none_when_bound_conversation_missing() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");
        let task = TaskRecordStored {
            task_id: "task-b".to_string(),
            conversation_id: Some("conversation-missing".to_string()),
            agent_id: None,
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
                cron_expression: None,
                legacy_every_minutes: None,
                end_at_utc: None,
                next_run_at_utc: None,
            },
            created_at_utc: now_utc_rfc3339(),
            updated_at_utc: now_utc_rfc3339(),
            last_triggered_at_utc: None,
            completed_at_utc: None,
        };

        let session = task_resolve_dispatch_session(&state, &task)
            .expect("resolve task session");
        let sub_conversation =
            state_read_conversation_cached(&state, "conversation-sub").expect("read sub conversation");

        assert!(session.is_none());
        assert_eq!(sub_conversation.status, "active");
    }

    #[test]
    fn task_resolve_dispatch_session_should_treat_system_conversation_as_system_task() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");
        let task = TaskRecordStored {
            task_id: "task-system".to_string(),
            conversation_id: Some(SYSTEM_NOTIFICATION_CONVERSATION_ID.to_string()),
            agent_id: None,
            target_scope: TASK_TARGET_SCOPE_DESKTOP.to_string(),
            order_index: 1,
            title: "系统任务".to_string(),
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
                cron_expression: None,
                legacy_every_minutes: None,
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

        assert_eq!(session.conversation_id, SYSTEM_NOTIFICATION_CONVERSATION_ID);
        assert_eq!(session.agent_id, DEFAULT_AGENT_ID);
        assert!(session.system_task);
    }

    #[test]
    fn task_resolve_dispatch_session_should_not_resolve_missing_contact_conversation() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");

        let data = test_user_switched_to_sub_conversation_data();
        let contact = RemoteImContact {
            id: "contact-a".to_string(),
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "group".to_string(),
            remote_contact_id: "remote-a".to_string(),
            remote_contact_name: "测试群".to_string(),
            avatar_url: String::new(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "never".to_string(),
            activation_keywords: Vec::new(),
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_agent_id: None,
            bound_conversation_id: Some("conversation-contact-missing".to_string()),
            processing_mode: "continuous".to_string(),
            response_strategy: default_remote_im_contact_response_strategy(),
            response_guidance: default_remote_im_contact_response_guidance(),
            blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
            group_reply_pacing: RemoteImGroupReplyPacing::default(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        };
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_write_app_data_cached(&state, &data).expect("write app data");
        let task = TaskRecordStored {
            task_id: "task-contact".to_string(),
            conversation_id: Some("conversation-contact-missing".to_string()),
            agent_id: None,
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
                cron_expression: None,
                legacy_every_minutes: None,
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
    fn task_resolve_dispatch_session_should_use_bound_agent() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        state_write_agents_cached(
            &state,
            &[{
                let mut agent = default_agent();
                agent.id = "private-agent".to_string();
                agent.name = "私域任务助理".to_string();
                agent
            }, default_user_persona()],
        )
        .expect("write agents");
        let task = TaskRecordStored {
            task_id: "task-private-agent".to_string(),
            conversation_id: None,
            agent_id: Some("private-agent".to_string()),
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
                cron_expression: None,
                legacy_every_minutes: None,
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
        let conversation =
            state_read_conversation_cached(&state, &session.conversation_id).expect("read conversation");

        assert_eq!(session.agent_id, "private-agent");
        assert_eq!(session.conversation_id, SYSTEM_NOTIFICATION_CONVERSATION_ID);
        assert!(session.system_task);
        assert!(conversation_is_system_notification(&conversation));
    }

    #[test]
    fn task_build_dispatch_candidates_should_allow_empty_input() {
        let state = test_chat_runtime_state();
        let candidates = task_build_dispatch_candidates(&state, Vec::new(), now_utc())
            .expect("build dispatch candidates");

        assert!(candidates.is_empty());
    }

    #[test]
    fn task_build_dispatch_candidates_should_limit_non_system_tasks_to_one_per_conversation_per_round() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");

        let create_task = |goal: &str, conversation_id: &str| {
            task_store_create_task(&state.data_path, &TaskCreateInput {
                goal: goal.to_string(),
                conversation_id: Some(conversation_id.to_string()),
                agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
                why: String::new(),
                todo: String::new(),
                trigger: TaskTriggerInputLocal {
                    run_at: Some("2026-04-10T10:00:00+08:00".to_string()),
                    cron_expression: Some("0,30 * * * *".to_string()),
                    end_at: Some("2099-04-10T12:00:00+08:00".to_string()),
                    legacy_every_minutes: None,
                },
            })
            .expect("create task")
        };
        let task_1 = create_task("t1", "conversation-main");
        let task_2 = create_task("t2", "conversation-main");
        let task_3 = create_task("t3", "conversation-sub");
        let tasks = vec![
            task_store_get_task_record(&state.data_path, &task_1.task_id).expect("get task 1"),
            task_store_get_task_record(&state.data_path, &task_2.task_id).expect("get task 2"),
            task_store_get_task_record(&state.data_path, &task_3.task_id).expect("get task 3"),
        ];

        let candidates =
            task_build_dispatch_candidates(&state, tasks, now_utc()).expect("build dispatch candidates");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].task.task_id, task_1.task_id);
        assert_eq!(candidates[0].session.conversation_id, "conversation-main");
        assert_eq!(candidates[1].task.task_id, task_3.task_id);
        assert_eq!(candidates[1].session.conversation_id, "conversation-sub");
    }

    #[test]
    fn task_build_dispatch_candidates_should_use_conversation_owner_for_legacy_task() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");

        let missing_owner = task_store_create_task(&state.data_path, &TaskCreateInput {
            goal: "missing owner".to_string(),
            conversation_id: Some("conversation-main".to_string()),
            agent_id: None,
            target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
            why: String::new(),
            todo: String::new(),
            trigger: TaskTriggerInputLocal {
                run_at: Some("2026-04-10T10:00:00+08:00".to_string()),
                cron_expression: Some("0,30 * * * *".to_string()),
                end_at: Some("2099-04-10T12:00:00+08:00".to_string()),
                legacy_every_minutes: None,
            },
        })
        .expect("create missing owner task");
        let valid = task_store_create_task(&state.data_path, &TaskCreateInput {
            goal: "valid".to_string(),
            conversation_id: Some("conversation-sub".to_string()),
            agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
            why: String::new(),
            todo: String::new(),
            trigger: TaskTriggerInputLocal {
                run_at: Some("2026-04-10T10:00:00+08:00".to_string()),
                cron_expression: Some("0,30 * * * *".to_string()),
                end_at: Some("2099-04-10T12:00:00+08:00".to_string()),
                legacy_every_minutes: None,
            },
        })
        .expect("create valid task");
        let tasks = vec![
            task_store_get_task_record(&state.data_path, &missing_owner.task_id)
                .expect("get missing owner task"),
            task_store_get_task_record(&state.data_path, &valid.task_id)
                .expect("get valid task"),
        ];

        let candidates =
            task_build_dispatch_candidates(&state, tasks, now_utc()).expect("build dispatch candidates");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].task.task_id, missing_owner.task_id);
        assert_eq!(candidates[0].session.agent_id, DEFAULT_AGENT_ID);
        assert_eq!(candidates[1].task.task_id, valid.task_id);
        let legacy_task = task_store_get_task_record(&state.data_path, &missing_owner.task_id)
            .expect("get legacy task");
        assert_eq!(legacy_task.completion_state, TASK_STATE_ACTIVE);
    }

    #[test]
    fn task_build_dispatch_candidates_should_allow_multiple_due_system_tasks() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");

        let create_system_task = |goal: &str| {
            task_store_create_task(&state.data_path, &TaskCreateInput {
                goal: goal.to_string(),
                conversation_id: Some(SYSTEM_NOTIFICATION_CONVERSATION_ID.to_string()),
                agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
                why: String::new(),
                todo: String::new(),
                trigger: TaskTriggerInputLocal {
                    run_at: Some("2026-04-10T10:00:00+08:00".to_string()),
                    cron_expression: Some("0,30 * * * *".to_string()),
                    end_at: Some("2099-04-10T12:00:00+08:00".to_string()),
                    legacy_every_minutes: None,
                },
            })
            .expect("create system task")
        };
        let task_1 = create_system_task("system task 1");
        let task_2 = create_system_task("system task 2");
        let tasks = vec![
            task_store_get_task_record(&state.data_path, &task_1.task_id)
                .expect("get system task 1"),
            task_store_get_task_record(&state.data_path, &task_2.task_id)
                .expect("get system task 2"),
        ];

        let candidates =
            task_build_dispatch_candidates(&state, tasks, now_utc()).expect("build dispatch candidates");

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].task.task_id, task_1.task_id);
        assert_eq!(candidates[1].task.task_id, task_2.task_id);
        assert!(candidates.iter().all(|item| item.session.system_task));
        assert!(candidates
            .iter()
            .all(|item| item.session.conversation_id == SYSTEM_NOTIFICATION_CONVERSATION_ID));
    }

    #[test]
    fn task_build_dispatch_candidates_should_skip_busy_conversation_and_wait_for_followup_check() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");
        set_conversation_runtime_state(&state, "conversation-main", MainSessionState::OrganizingContext)
            .expect("set busy");

        let created = task_store_create_task(&state.data_path, &TaskCreateInput {
            goal: "busy".to_string(),
            conversation_id: Some("conversation-main".to_string()),
            agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
            why: String::new(),
            todo: String::new(),
            trigger: TaskTriggerInputLocal {
                run_at: Some("2026-04-10T10:00:00+08:00".to_string()),
                cron_expression: Some("0,30 * * * *".to_string()),
                end_at: Some("2099-04-10T12:00:00+08:00".to_string()),
                legacy_every_minutes: None,
            },
        })
        .expect("create busy task");
        let tasks = vec![task_store_get_task_record(&state.data_path, &created.task_id)
            .expect("get busy task record")];

        let candidates =
            task_build_dispatch_candidates(&state, tasks, now_utc()).expect("build dispatch candidates");

        assert!(candidates.is_empty());
        let stored = task_store_get_task_record(&state.data_path, &created.task_id)
            .expect("read busy task after skip");
        assert_eq!(stored.completion_state, TASK_STATE_ACTIVE);
        assert!(stored.last_triggered_at_utc.is_none());
    }

    #[test]
    fn task_trigger_system_message_should_feed_model_as_user() {
        let task = TaskRecordStored {
            task_id: "task-trigger-shape".to_string(),
            conversation_id: Some("conversation-main".to_string()),
            agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            target_scope: TASK_TARGET_SCOPE_DESKTOP.to_string(),
            order_index: 1,
            title: "提醒跟进".to_string(),
            cause: String::new(),
            goal: "提醒跟进".to_string(),
            flow: String::new(),
            todos: vec!["检查结果".to_string()],
            status_summary: "检查结果".to_string(),
            completion_state: TASK_STATE_ACTIVE.to_string(),
            completion_conclusion: String::new(),
            progress_notes: Vec::new(),
            stage_key: String::new(),
            stage_updated_at_utc: None,
            trigger: TaskTriggerStored {
                run_at_utc: Some("2026-04-10T02:00:00Z".to_string()),
                cron_expression: None,
                legacy_every_minutes: None,
                end_at_utc: None,
                next_run_at_utc: Some("2026-04-10T02:00:00Z".to_string()),
            },
            created_at_utc: now_utc_rfc3339(),
            updated_at_utc: now_utc_rfc3339(),
            last_triggered_at_utc: None,
            completed_at_utc: None,
        };

        let message = build_task_trigger_message(&task);

        assert_eq!(message.role, "system");
        assert_eq!(message.speaker_agent_id.as_deref(), Some(SYSTEM_PERSONA_ID));
        assert_eq!(
            prompt_role_for_message(&message, DEFAULT_AGENT_ID).as_deref(),
            Some("user")
        );
        let meta = message.provider_meta.as_ref().expect("task provider meta");
        assert_eq!(meta.get("messageKind").and_then(Value::as_str), Some("task_trigger"));
    }

    #[test]
    fn maybe_enqueue_overdue_task_after_idle_should_dispatch_bound_conversation_task() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");

        let created = task_store_create_task(&state.data_path, &TaskCreateInput {
            goal: "idle overdue".to_string(),
            conversation_id: Some("conversation-main".to_string()),
            agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
            why: String::new(),
            todo: String::new(),
            trigger: TaskTriggerInputLocal {
                run_at: Some("2026-04-10T10:00:00+08:00".to_string()),
                cron_expression: Some("0,30 * * * *".to_string()),
                end_at: Some("2099-04-10T12:00:00+08:00".to_string()),
                legacy_every_minutes: None,
            },
        })
        .expect("create overdue task");

        let triggered =
            maybe_enqueue_overdue_task_after_idle(&state, "conversation-main").expect("enqueue overdue task");
        assert!(triggered);

        for _ in 0..10 {
            let stored = task_store_get_task_record(&state.data_path, &created.task_id)
                .expect("read overdue task");
            if stored.last_triggered_at_utc.is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        let stored = task_store_get_task_record(&state.data_path, &created.task_id)
            .expect("read overdue task after wait");
        assert!(stored.last_triggered_at_utc.is_some());
    }

    #[test]
    fn task_scheduler_next_wake_delay_should_pick_earliest_active_due_time() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");
        let late_utc = (now_utc() + time::Duration::minutes(10))
            .format(&time::format_description::well_known::Rfc3339)
            .expect("format late");
        let soon_utc = (now_utc() + time::Duration::minutes(2))
            .format(&time::format_description::well_known::Rfc3339)
            .expect("format soon");

        task_store_create_task(&state.data_path, &TaskCreateInput {
            goal: "late".to_string(),
            conversation_id: Some("conversation-main".to_string()),
            agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
            why: String::new(),
            todo: String::new(),
            trigger: TaskTriggerInputLocal {
                run_at: Some(format_utc_storage_time_to_local_rfc3339(&late_utc)),
                cron_expression: None,
                end_at: None,
                legacy_every_minutes: None,
            },
        })
        .expect("create late task");
        task_store_create_task(&state.data_path, &TaskCreateInput {
            goal: "soon".to_string(),
            conversation_id: Some("conversation-sub".to_string()),
            agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
            why: String::new(),
            todo: String::new(),
            trigger: TaskTriggerInputLocal {
                run_at: Some(format_utc_storage_time_to_local_rfc3339(&soon_utc)),
                cron_expression: None,
                end_at: None,
                legacy_every_minutes: None,
            },
        })
        .expect("create soon task");

        let delay = task_scheduler_next_wake_delay(&state)
            .expect("next wake delay")
            .expect("delay present");

        assert!(delay <= std::time::Duration::from_secs(2 * 60 + 2));
    }

    #[test]
    fn task_scheduler_next_wake_delay_should_not_spin_for_busy_overdue_conversation_task() {
        let state = test_chat_runtime_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");
        set_conversation_runtime_state(&state, "conversation-main", MainSessionState::OrganizingContext)
            .expect("set busy");

        task_store_create_task(&state.data_path, &TaskCreateInput {
            goal: "busy overdue".to_string(),
            conversation_id: Some("conversation-main".to_string()),
            agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
            why: String::new(),
            todo: String::new(),
            trigger: TaskTriggerInputLocal {
                run_at: Some("2026-04-10T10:00:00+08:00".to_string()),
                cron_expression: Some("0,30 * * * *".to_string()),
                end_at: Some("2099-04-10T12:00:00+08:00".to_string()),
                legacy_every_minutes: None,
            },
        })
        .expect("create busy overdue task");

        let delay = task_scheduler_next_wake_delay(&state).expect("next wake delay");

        assert_eq!(delay, None);
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
    fn delegate_parse_session_parts_should_accept_remote_reply_delegate_session() {
        let (_, agent_id, conversation_id) = delegate_parse_session_parts(
            "agent-a::conversation-sub::remote_reply_delegate:delegate-a",
        );
        assert_eq!(agent_id, "agent-a");
        assert_eq!(conversation_id.as_deref(), Some("conversation-sub"));
    }

    #[test]
    fn delegate_session_helpers_should_handle_remote_reply_delegate_session() {
        let session_id = "agent-a::conversation-sub::remote_reply_delegate:delegate-a";

        assert_eq!(delegate_session_agent_id(session_id), "agent-a");
        assert_eq!(
            delegate_session_conversation_id(session_id).as_deref(),
            Some("conversation-sub")
        );
        assert!(delegate_session_is_remote_reply_delegate(session_id));
    }

    #[test]
    fn inflight_chat_key_should_join_agent_and_conversation() {
        assert_eq!(
            inflight_chat_key("agent-a", Some("conversation-main")),
            "agent-a::conversation-main"
        );
        assert_eq!(
            inflight_chat_key("agent-b", Some("conversation-main")),
            "agent-b::conversation-main"
        );
        assert_ne!(
            inflight_chat_key("agent-a", Some("conversation-main")),
            inflight_chat_key("agent-b", Some("conversation-main"))
        );
    }

    #[test]
    fn delegate_thread_chat_key_should_use_thread_agent() {
        let mut conversation = build_conversation_record(
            "api-a",
            "agent-a",
            "委托线程",
            CONVERSATION_KIND_DELEGATE,
            Some("conversation-root".to_string()),
            Some("delegate-a".to_string()),
        );
        conversation.id = "delegate-a".to_string();
        let thread = DelegateRuntimeThread {
            delegate_id: "delegate-a".to_string(),
            root_conversation_id: "conversation-root".to_string(),
            target_agent_id: "agent-b".to_string(),
            title: "委托线程".to_string(),
            call_stack: vec!["agent-root".to_string(), "agent-a".to_string()],
            parent_chat_session_key: Some("agent-root::conversation-root".to_string()),
            archived_at: None,
            conversation,
        };

        assert_eq!(
            delegate_thread_chat_key(&thread),
            "agent-a::delegate-a"
        );
    }

    #[test]
    fn delegate_runtime_thread_build_should_prefer_parent_session_workspace_for_async_delegate() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-delegate-workspace-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("p-ai").join("llm-workspace");
        let root_workspace_path = temp_root.join("root-workspace");
        let current_workspace_path = temp_root.join("current-workspace");
        std::fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        std::fs::create_dir_all(&root_workspace_path).expect("create root workspace");
        std::fs::create_dir_all(&current_workspace_path).expect("create current workspace");

        let state = test_chat_runtime_state();
        std::fs::create_dir_all(&state.llm_workspace_path).expect("ensure llm workspace");
        let mut config = AppConfig::default();
        config.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "系统工作目录".to_string(),
            path: terminal_path_for_user(&llm_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_SYSTEM.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }];
        state_write_config_cached(&state, &config).expect("write config");

        let now = now_iso();
        let mut data = AppData::default();
        data.conversations.push(Conversation {
            id: "conversation-root".to_string(),
            title: "主会话".to_string(),
            agent_id: "agent-root".to_string(),
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
            user_profile_snapshot: String::new(),
            shell_workspace_path: Some(root_workspace_path.to_string_lossy().to_string()),
            shell_workspaces: vec![ShellWorkspaceConfig {
                id: "main-root".to_string(),
                name: "根会话目录".to_string(),
                path: terminal_path_for_user(&root_workspace_path),
                level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
                access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
                built_in: false,
            }],
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: Vec::new(),
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        });
        state_write_app_data_cached(&state, &data).expect("write app data");

        let parent_session_key = inflight_chat_key("dept-root", Some("conversation-root"));
        let mut session_conversation = build_conversation_record(
            "api-a",
            "agent-root",
            "当前运行会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        session_conversation.id = "conversation-root".to_string();
        session_conversation.shell_workspace_path =
            Some(current_workspace_path.to_string_lossy().to_string());
        session_conversation.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "main-current".to_string(),
            name: "当前工作目录".to_string(),
            path: terminal_path_for_user(&current_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
            built_in: false,
        }];
        state
            .terminal_session_roots
            .lock()
            .expect("terminal session roots")
            .insert(
                parent_session_key.clone(),
                current_workspace_path.to_string_lossy().to_string(),
            );
        state_schedule_conversation_persist(&state, &session_conversation)
            .expect("persist session conversation");

        let delegate = DelegateEntry {
            delegate_id: "delegate-async".to_string(),
            kind: DELEGATE_TOOL_KIND_USER_MENTION.to_string(),
            conversation_id: "conversation-root".to_string(),
            parent_delegate_id: None,
            source_agent_id: "agent-root".to_string(),
            target_agent_id: "agent-child".to_string(),
            title: "异步委托".to_string(),
            why: "背景".to_string(),
            goal: "目标".to_string(),
            todo: "待办".to_string(),
            notify_assistant_when_done: false,
            call_stack: vec!["agent-root".to_string(), "agent-child".to_string()],
            created_at: now.clone(),
            updated_at: now,
            status: "pending".to_string(),
            delivered_at: None,
            completed_at: None,
        };

        let workspace_snapshot = delegate_capture_workspace_snapshot(
            &state,
            &delegate.conversation_id,
            Some(parent_session_key.as_str()),
        );
        let thread = delegate_runtime_thread_build(
            &delegate,
            "api-a",
            workspace_snapshot,
            Some(parent_session_key),
        );

        assert_eq!(
            thread.conversation.shell_workspace_path.as_deref(),
            Some(current_workspace_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            thread.conversation.shell_workspaces.first().map(|item| item.path.as_str()),
            Some(terminal_path_for_user(&current_workspace_path).as_str())
        );

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn delegate_runtime_thread_build_should_inherit_session_workspaces_without_locked_root() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-delegate-workspaces-only-test-{}",
            uuid::Uuid::new_v4()
        ));
        let current_workspace_path = temp_root.join("current-workspace");
        std::fs::create_dir_all(&current_workspace_path).expect("create current workspace");

        let state = test_chat_runtime_state();
        let now = now_iso();
        let parent_session_key = inflight_chat_key("dept-root", Some("conversation-root"));
        let mut session_conversation = build_conversation_record(
            "api-a",
            "agent-root",
            "当前运行会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        session_conversation.id = "conversation-root".to_string();
        session_conversation.shell_workspace_path = None;
        session_conversation.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "main-current".to_string(),
            name: "当前工作目录".to_string(),
            path: terminal_path_for_user(&current_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_APPROVAL.to_string(),
            built_in: false,
        }];
        state
            .terminal_session_roots
            .lock()
            .expect("terminal session roots")
            .insert(
                parent_session_key.clone(),
                current_workspace_path.to_string_lossy().to_string(),
            );
        state_schedule_conversation_persist(&state, &session_conversation)
            .expect("persist session conversation");

        let delegate = DelegateEntry {
            delegate_id: "delegate-workspaces-only".to_string(),
            kind: DELEGATE_TOOL_KIND_USER_MENTION.to_string(),
            conversation_id: "conversation-root".to_string(),
            parent_delegate_id: None,
            source_agent_id: "agent-root".to_string(),
            target_agent_id: "agent-child".to_string(),
            title: "异步委托".to_string(),
            why: "背景".to_string(),
            goal: "目标".to_string(),
            todo: "待办".to_string(),
            notify_assistant_when_done: false,
            call_stack: vec!["agent-root".to_string(), "agent-child".to_string()],
            created_at: now.clone(),
            updated_at: now,
            status: "pending".to_string(),
            delivered_at: None,
            completed_at: None,
        };

        let workspace_snapshot = delegate_capture_workspace_snapshot(
            &state,
            &delegate.conversation_id,
            Some(parent_session_key.as_str()),
        );
        let thread = delegate_runtime_thread_build(
            &delegate,
            "api-a",
            workspace_snapshot,
            Some(parent_session_key),
        );

        assert_eq!(thread.conversation.shell_workspace_path, None);
        assert_eq!(
            thread.conversation.shell_workspaces.first().map(|item| item.path.as_str()),
            Some(terminal_path_for_user(&current_workspace_path).as_str())
        );

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn runtime_control_should_keep_conversation_bound_agent() {
        let state = test_chat_runtime_state();
        let mut old_agent = default_agent();
        old_agent.id = "old-agent".to_string();
        let mut new_agent = default_agent();
        new_agent.id = "new-agent".to_string();

        let config = AppConfig {
            ..AppConfig::default()
        };
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(
            &state,
            &[old_agent.clone(), new_agent.clone(), default_user_persona()],
        )
        .expect("write agents");

        let mut conversation = build_conversation_record(
            "api-a",
            &old_agent.id,
            "旧人格会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        conversation.id = "conversation-stop".to_string();
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");

        let agent_id = resolve_runtime_control_agent_id(
            &state,
            Some("new-agent"),
            Some("conversation-stop"),
        )
        .expect("resolve runtime control identity");

        assert_eq!(agent_id, "old-agent");
        assert_eq!(
            inflight_chat_key(&agent_id, Some("conversation-stop")),
            "old-agent::conversation-stop"
        );
        assert_ne!(
            inflight_chat_key(&agent_id, Some("conversation-stop")),
            inflight_chat_key("new-agent", Some("conversation-stop"))
        );
    }

    #[test]
    fn runtime_control_should_fallback_to_requested_agent_when_conversation_agent_empty() {
        let state = test_chat_runtime_state();
        let mut agent = default_agent();
        agent.id = "fallback-agent".to_string();
        let config = AppConfig {
            ..AppConfig::default()
        };
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(&state, &[agent.clone(), default_user_persona()])
            .expect("write agents");

        let mut conversation = build_conversation_record(
            "api-a",
            &agent.id,
            "缺少固化人格",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        conversation.id = "conversation-empty-agent".to_string();
        conversation.agent_id = String::new();
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");

        let agent_id = resolve_runtime_control_agent_id(
            &state,
            Some(&agent.id),
            Some("conversation-empty-agent"),
        )
        .expect("empty conversation agent should fall back to requested agent");

        assert_eq!(agent_id, "fallback-agent");
    }

    #[test]
    fn assemble_runtime_tools_should_gate_delegate_by_executor_agent() {
        let state = test_chat_runtime_state();
        let mut shared_agent = default_agent();
        shared_agent.id = "shared-agent".to_string();
        shared_agent.name = "共享人格".to_string();
        shared_agent.child_agent_ids = Vec::new();
        shared_agent.permission_control = AgentPermissionControl {
            enabled: true,
            mode: "whitelist".to_string(),
            builtin_tool_names: vec!["fetch".to_string()],
            skill_names: Vec::new(),
            mcp_tool_names: Vec::new(),
        };

        let mut selected_api = ApiConfig::default();
        selected_api.id = "api-a".to_string();
        selected_api.name = "测试模型".to_string();
        selected_api.request_format = RequestFormat::OpenAI;
        selected_api.enable_text = true;
        selected_api.enable_tools = true;
        selected_api.base_url = "https://api.openai.com/v1".to_string();
        selected_api.api_key = "k".to_string();
        selected_api.model = "gpt-4o-mini".to_string();

        let config = AppConfig {
            api_configs: vec![selected_api.clone()],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(&state, &[shared_agent.clone(), default_user_persona()])
            .expect("write agents");

        let conversation = build_conversation_record(
            &selected_api.id,
            &shared_agent.id,
            "叶子人格会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");
        refresh_global_tool_schema_cache(&state);

        let session_id = format!("{}::{}", shared_agent.id, conversation.id);
        let assembly = test_runtime()
            .block_on(assemble_runtime_tools(
                &config,
                &selected_api,
                &shared_agent,
                Some(&state),
                &session_id,
            ));

        assert!(assembly.tool_manifest.iter().any(|item| {
            item.get("source").and_then(Value::as_str) == Some("runtime_policy")
                && item.get("name").and_then(Value::as_str) == Some("delegate")
                && item.get("enabled").and_then(Value::as_bool) == Some(false)
                && item.get("reason").and_then(Value::as_str)
                    == Some("当前人格没有直接下级，无法使用委托")
        }));
        assert!(!assembly.tools.iter().any(|tool| tool.name() == "delegate"));
        assert!(assembly.tool_definitions.iter().any(|tool| tool.name == "fetch"));
        assert!(assembly.tools.iter().any(|tool| tool.name() == "fetch"));
        assert!(!assembly.tool_definitions.iter().any(|tool| tool.name == "config"));
        assert!(!assembly.tools.iter().any(|tool| tool.name() == "config"));
    }

    #[test]
    fn assemble_runtime_tools_should_keep_goal_tools_and_gate_contact_tools_by_conversation_state() {
        let state = test_chat_runtime_state();
        let mut agent = default_agent();
        agent.id = "state-agent".to_string();

        let mut selected_api = ApiConfig::default();
        selected_api.id = "api-a".to_string();
        selected_api.name = "测试模型".to_string();
        selected_api.request_format = RequestFormat::OpenAI;
        selected_api.enable_text = true;
        selected_api.enable_tools = true;
        selected_api.base_url = "https://api.openai.com/v1".to_string();
        selected_api.api_key = "k".to_string();
        selected_api.model = "gpt-4o-mini".to_string();

        let mut config = AppConfig {
            api_configs: vec![selected_api.clone()],
            api_providers: Vec::new(),
            ..AppConfig::default()
        };
        config.remote_im_channels.push(RemoteImChannelConfig {
            id: "channel-a".to_string(),
            name: "测试渠道".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            enabled: true,
            credentials: serde_json::json!({ "mockSend": true }),
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            filter_markdown: false,
            allow_send_files: true,
            behavior_settings: RemoteImChannelBehaviorSettings::default(),
        });
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(&state, &[agent.clone(), default_user_persona()])
            .expect("write agents");

        let local_without_goal = build_conversation_record(
            &selected_api.id,
            &agent.id,
            "普通会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        let mut local_with_goal = build_conversation_record(
            &selected_api.id,
            &agent.id,
            "目标会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        local_with_goal.id = "conversation-with-goal".to_string();
        local_with_goal.active_goal = Some(ConversationGoalState {
            goal_id: "goal-a".to_string(),
            status: "active".to_string(),
            objective: "继续推进".to_string(),
            started_at: now_iso(),
            ended_at: None,
            usage_start: ConversationCumulativeUsage::default(),
            usage_end: None,
        });
        let private_contact = RemoteImContact {
            id: "contact-private".to_string(),
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "private".to_string(),
            remote_contact_id: "private-a".to_string(),
            remote_contact_name: "私聊联系人".to_string(),
            avatar_url: String::new(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: true,
            allow_receive: true,
            activation_mode: "always".to_string(),
            activation_keywords: Vec::new(),
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_agent_id: Some(agent.id.clone()),
            bound_conversation_id: Some("conversation-remote-private".to_string()),
            processing_mode: "continuous".to_string(),
            response_strategy: default_remote_im_contact_response_strategy(),
            response_guidance: default_remote_im_contact_response_guidance(),
            blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
            group_reply_pacing: RemoteImGroupReplyPacing::default(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        };
        let group_contact = RemoteImContact {
            id: "contact-group".to_string(),
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "group".to_string(),
            remote_contact_id: "group-a".to_string(),
            remote_contact_name: "群聊联系人".to_string(),
            avatar_url: String::new(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: true,
            allow_receive: true,
            activation_mode: "always".to_string(),
            activation_keywords: Vec::new(),
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_agent_id: Some(agent.id.clone()),
            bound_conversation_id: Some("conversation-remote-group".to_string()),
            processing_mode: "continuous".to_string(),
            response_strategy: default_remote_im_contact_response_strategy(),
            response_guidance: default_remote_im_contact_response_guidance(),
            blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
            group_reply_pacing: RemoteImGroupReplyPacing::default(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        };
        state_service_upsert_remote_im_contact(&state, &private_contact).expect("write private contact");
        state_service_upsert_remote_im_contact(&state, &group_contact).expect("write group contact");

        let mut remote_private_contact = build_conversation_record(
            &selected_api.id,
            &agent.id,
            "私聊联系人会话",
            CONVERSATION_KIND_REMOTE_IM_CONTACT,
            Some(remote_im_contact_conversation_key(&private_contact)),
            None,
        );
        remote_private_contact.id = "conversation-remote-private".to_string();
        let mut remote_group_contact = build_conversation_record(
            &selected_api.id,
            &agent.id,
            "群聊联系人会话",
            CONVERSATION_KIND_REMOTE_IM_CONTACT,
            Some(remote_im_contact_conversation_key(&group_contact)),
            None,
        );
        remote_group_contact.id = "conversation-remote-group".to_string();
        let mut delegate_conversation = build_conversation_record(
            &selected_api.id,
            &agent.id,
            "委托会话",
            CONVERSATION_KIND_DELEGATE,
            Some(local_without_goal.id.clone()),
            Some("delegate-runtime".to_string()),
        );
        delegate_conversation.id = "conversation-delegate".to_string();
        let mut system_conversation = build_conversation_record(
            &selected_api.id,
            &agent.id,
            "系统通知会话",
            CONVERSATION_KIND_SYSTEM_NOTIFICATION,
            None,
            None,
        );
        system_conversation.id = "conversation-system-notification".to_string();

        for conversation in [
            &local_without_goal,
            &local_with_goal,
            &remote_private_contact,
            &remote_group_contact,
            &delegate_conversation,
            &system_conversation,
        ] {
            state_schedule_conversation_persist(&state, conversation)
                .expect("persist conversation");
        }
        refresh_global_tool_schema_cache(&state);

        let assemble_for = |conversation_id: &str| {
            let session_id = format!("{}::{}", agent.id, conversation_id);
            test_runtime()
                .block_on(assemble_runtime_tools(
                    &config,
                    &selected_api,
                    &agent,
                    Some(&state),
                    &session_id,
                ))
        };
        let has_attached_schema = |assembly: &RuntimeToolAssembly, name: &str| {
            assembly.tool_manifest.iter().any(|item| {
                item.get("name").and_then(Value::as_str) == Some(name)
                    && item.get("enabled").and_then(Value::as_bool) == Some(true)
                    && item.get("attached").and_then(Value::as_bool) == Some(true)
            })
        };
        let has_executor = |assembly: &RuntimeToolAssembly, name: &str| {
            assembly.tools.iter().any(|tool| tool.name() == name)
        };
        let has_definition = |assembly: &RuntimeToolAssembly, name: &str| {
            assembly.tool_definitions.iter().any(|def| def.name == name)
        };
        let assert_attached_sets_equal = |assembly: &RuntimeToolAssembly| {
            let mut definition_names = assembly
                .tool_definitions
                .iter()
                .map(|definition| definition.name.clone())
                .collect::<Vec<_>>();
            let mut executor_names = assembly
                .tools
                .iter()
                .map(|tool| tool.name())
                .collect::<Vec<_>>();
            let mut manifest_names = assembly
                .tool_manifest
                .iter()
                .filter(|item| {
                    item.get("enabled").and_then(Value::as_bool) == Some(true)
                        && item.get("attached").and_then(Value::as_bool) == Some(true)
                })
                .filter_map(|item| item.get("name").and_then(Value::as_str).map(str::to_string))
                .collect::<Vec<_>>();
            definition_names.sort();
            executor_names.sort();
            manifest_names.sort();
            assert_eq!(definition_names, executor_names);
            assert_eq!(definition_names, manifest_names);
        };

        let local_without_goal_assembly = assemble_for(&local_without_goal.id);
        assert!(has_attached_schema(&local_without_goal_assembly, "create_goal"));
        assert!(has_executor(&local_without_goal_assembly, "create_goal"));
        assert!(has_definition(&local_without_goal_assembly, "create_goal"));
        assert!(has_attached_schema(&local_without_goal_assembly, "update_goal"));
        assert!(has_executor(&local_without_goal_assembly, "update_goal"));
        assert!(!has_attached_schema(&local_without_goal_assembly, "contact_reply"));
        assert!(!has_executor(&local_without_goal_assembly, "contact_reply"));
        assert!(!has_definition(&local_without_goal_assembly, "contact_reply"));
        assert!(!has_definition(&local_without_goal_assembly, "contact_no_reply"));
        assert!(!has_definition(&local_without_goal_assembly, "contact_send_files"));
        assert_attached_sets_equal(&local_without_goal_assembly);

        let local_with_goal_assembly = assemble_for(&local_with_goal.id);
        assert!(has_attached_schema(&local_with_goal_assembly, "create_goal"));
        assert!(has_executor(&local_with_goal_assembly, "create_goal"));
        assert!(has_attached_schema(&local_with_goal_assembly, "update_goal"));
        assert!(has_executor(&local_with_goal_assembly, "update_goal"));
        assert!(!has_attached_schema(&local_with_goal_assembly, "contact_reply"));
        assert!(!has_executor(&local_with_goal_assembly, "contact_reply"));
        assert!(!has_definition(&local_with_goal_assembly, "contact_reply"));
        assert!(!has_definition(&local_with_goal_assembly, "contact_no_reply"));
        assert!(!has_definition(&local_with_goal_assembly, "contact_send_files"));
        assert_attached_sets_equal(&local_with_goal_assembly);

        let remote_private_contact_assembly = assemble_for(&remote_private_contact.id);
        assert!(has_attached_schema(&remote_private_contact_assembly, "create_goal"));
        assert!(has_executor(&remote_private_contact_assembly, "create_goal"));
        assert!(!has_attached_schema(&remote_private_contact_assembly, "contact_reply"));
        assert!(!has_executor(&remote_private_contact_assembly, "contact_reply"));
        assert!(!has_definition(&remote_private_contact_assembly, "contact_reply"));
        assert!(has_attached_schema(&remote_private_contact_assembly, "contact_send_files"));
        assert!(has_executor(&remote_private_contact_assembly, "contact_send_files"));
        assert!(!has_attached_schema(&remote_private_contact_assembly, "contact_no_reply"));
        assert!(!has_executor(&remote_private_contact_assembly, "contact_no_reply"));
        assert!(!has_definition(&remote_private_contact_assembly, "contact_no_reply"));
        assert_attached_sets_equal(&remote_private_contact_assembly);

        let remote_group_contact_assembly = assemble_for(&remote_group_contact.id);
        assert!(!has_attached_schema(&remote_group_contact_assembly, "create_goal"));
        assert!(!has_executor(&remote_group_contact_assembly, "create_goal"));
        assert!(!has_definition(&remote_group_contact_assembly, "create_goal"));
        assert!(!has_attached_schema(&remote_group_contact_assembly, "update_goal"));
        assert!(!has_executor(&remote_group_contact_assembly, "update_goal"));
        assert!(!has_definition(&remote_group_contact_assembly, "update_goal"));
        assert!(has_attached_schema(&remote_group_contact_assembly, "task"));
        assert!(has_executor(&remote_group_contact_assembly, "task"));
        assert!(!has_attached_schema(&remote_group_contact_assembly, "contact_reply"));
        assert!(!has_executor(&remote_group_contact_assembly, "contact_reply"));
        assert!(has_attached_schema(&remote_group_contact_assembly, "contact_send_files"));
        assert!(has_executor(&remote_group_contact_assembly, "contact_send_files"));
        assert!(!has_attached_schema(&remote_group_contact_assembly, "contact_no_reply"));
        assert!(!has_executor(&remote_group_contact_assembly, "contact_no_reply"));
        assert!(!has_definition(&remote_group_contact_assembly, "contact_no_reply"));
        assert_attached_sets_equal(&remote_group_contact_assembly);

        let remote_group_delegate_session = format!(
            "{}::{}::remote_reply_delegate:delegate-a",
            agent.id, remote_group_contact.id
        );
        let remote_group_delegate_assembly = test_runtime().block_on(assemble_runtime_tools(
            &config,
            &selected_api,
            &agent,
            Some(&state),
            &remote_group_delegate_session,
        ));
        assert!(!has_attached_schema(&remote_group_delegate_assembly, "create_goal"));
        assert!(!has_executor(&remote_group_delegate_assembly, "create_goal"));
        assert!(!has_attached_schema(&remote_group_delegate_assembly, "update_goal"));
        assert!(!has_executor(&remote_group_delegate_assembly, "update_goal"));
        assert!(has_attached_schema(&remote_group_delegate_assembly, "task"));
        assert!(has_executor(&remote_group_delegate_assembly, "task"));
        let created_contact_task = test_runtime()
            .block_on(builtin_task(
                &state,
                &remote_group_delegate_session,
                &selected_api.id,
                &agent.id,
                TaskToolArgsWire {
                    action: "create".to_string(),
                    task_id: None,
                    goal: Some("跟进群聊事项".to_string()),
                    todo: Some("等待下一次联系人会话调度".to_string()),
                    how: None,
                    why: Some("远程应答委托创建".to_string()),
                    title: None,
                    cause: None,
                    flow: None,
                    todos: None,
                    status_summary: None,
                    stage_key: None,
                    append_note: None,
                    completion_state: None,
                    completion_conclusion: None,
                    trigger: Some(TaskTriggerInputLocal {
                        run_at: Some(now_iso()),
                        cron_expression: None,
                        end_at: None,
                        legacy_every_minutes: None,
                    }),
                },
            ))
            .expect("remote reply delegate should create a contact task");
        let task_id = created_contact_task
            .get("taskId")
            .and_then(Value::as_str)
            .expect("created contact task id");
        let stored_contact_task = task_store_get_task_record(&state.data_path, task_id)
            .expect("read created contact task");
        assert_eq!(
            stored_contact_task.conversation_id.as_deref(),
            Some(remote_group_contact.id.as_str())
        );
        assert_eq!(stored_contact_task.target_scope, TASK_TARGET_SCOPE_CONTACT);
        assert_eq!(
            runtime_builtin_tool_authorization_error(
                &state,
                "create_goal",
                &remote_group_delegate_session,
                "assistant-department",
            )
            .as_deref(),
            Some("远程群聊及其来源委托禁止使用 Goal 工具")
        );
        assert_attached_sets_equal(&remote_group_delegate_assembly);

        let mut disabled_channel_config = config.clone();
        disabled_channel_config.remote_im_channels[0].enabled = false;
        state_write_config_cached(&state, &disabled_channel_config)
            .expect("disable remote channel in config cache");
        let disabled_channel_assembly = test_runtime().block_on(assemble_runtime_tools(
            &disabled_channel_config,
            &selected_api,
            &agent,
            Some(&state),
            &remote_group_delegate_session,
        ));
        assert!(!has_attached_schema(&disabled_channel_assembly, "create_goal"));
        assert!(!has_executor(&disabled_channel_assembly, "create_goal"));
        assert!(!has_attached_schema(&disabled_channel_assembly, "update_goal"));
        assert!(!has_executor(&disabled_channel_assembly, "update_goal"));
        assert_eq!(
            runtime_builtin_tool_authorization_error(
                &state,
                "create_goal",
                &remote_group_delegate_session,
                "assistant-department",
            )
            .as_deref(),
            Some("远程群聊及其来源委托禁止使用 Goal 工具")
        );
        assert_attached_sets_equal(&disabled_channel_assembly);

        let delegate_assembly = assemble_for(&delegate_conversation.id);
        assert!(!has_attached_schema(&delegate_assembly, "task"));
        assert!(!has_executor(&delegate_assembly, "task"));
        assert!(!has_definition(&delegate_assembly, "task"));
        assert!(!has_attached_schema(&delegate_assembly, "plan"));
        assert!(!has_executor(&delegate_assembly, "plan"));
        assert!(!has_definition(&delegate_assembly, "plan"));
        assert_attached_sets_equal(&delegate_assembly);

        let system_assembly = assemble_for(&system_conversation.id);
        assert!(!has_attached_schema(&system_assembly, "plan"));
        assert!(!has_executor(&system_assembly, "plan"));
        assert!(!has_definition(&system_assembly, "plan"));
        assert_attached_sets_equal(&system_assembly);
    }

    #[test]
    fn builtin_task_should_reject_delegate_conversation() {
        let state = test_chat_runtime_state();
        let mut agent = default_agent();
        agent.id = "delegate-agent".to_string();
        let now = now_iso();

        let mut delegate_conversation = build_conversation_record(
            "api-a",
            &agent.id,
            "委托会话",
            CONVERSATION_KIND_DELEGATE,
            Some("conversation-root".to_string()),
            Some("delegate-task-loop".to_string()),
        );
        delegate_conversation.id = "conversation-delegate-task".to_string();
        state_schedule_conversation_persist(&state, &delegate_conversation)
            .expect("persist delegate conversation");

        let session_id = format!("{}::{}", agent.id, delegate_conversation.id);
        let err = test_runtime()
            .block_on(builtin_task(
                &state,
                &session_id,
                "api-a",
                &agent.id,
                TaskToolArgsWire {
                    action: "create".to_string(),
                    task_id: None,
                    goal: Some("递归任务".to_string()),
                    todo: Some("不应创建".to_string()),
                    how: None,
                    why: Some("委托线程回归测试".to_string()),
                    title: None,
                    cause: None,
                    flow: None,
                    todos: None,
                    status_summary: None,
                    stage_key: None,
                    append_note: None,
                    completion_state: None,
                    completion_conclusion: None,
                    trigger: Some(TaskTriggerInputLocal {
                        run_at: Some(now),
                        cron_expression: None,
                        end_at: None,
                        legacy_every_minutes: None,
                    }),
                },
            ))
            .expect_err("delegate conversation should reject task tool");

        assert!(err.contains("委托线程中禁止使用 task 工具"));
    }

    #[test]
    fn common_delegate_preflight_should_resolve_source_agent_from_delegate_thread() {
        let state = test_chat_runtime_state();
        let mut shared_agent = default_agent();
        shared_agent.id = "shared-agent".to_string();
        shared_agent.name = "共享人格".to_string();

        let mut target_agent = default_agent();
        target_agent.id = "target-agent".to_string();
        target_agent.name = "目标人格".to_string();
        shared_agent.child_agent_ids = vec![target_agent.id.clone()];

        state_write_agents_cached(
            &state,
            &[
                shared_agent.clone(),
                target_agent.clone(),
                default_user_persona(),
            ],
        )
        .expect("write agents");

        let mut delegate_conversation = build_conversation_record(
            "api-a",
            &shared_agent.id,
            "委托线程",
            CONVERSATION_KIND_DELEGATE,
            Some("conversation-root".to_string()),
            Some("delegate-child".to_string()),
        );
        delegate_conversation.id = "delegate-child".to_string();
        let thread = DelegateRuntimeThread {
            delegate_id: "delegate-child".to_string(),
            root_conversation_id: "conversation-root".to_string(),
            target_agent_id: shared_agent.id.clone(),
            title: "委托线程".to_string(),
            call_stack: vec![shared_agent.id.clone()],
            parent_chat_session_key: Some(format!("{}::conversation-root", shared_agent.id)),
            archived_at: None,
            conversation: delegate_conversation,
        };
        state
            .delegate_runtime_threads
            .lock()
            .expect("delegate runtime threads")
            .insert(thread.delegate_id.clone(), thread);

        let preflight = common_delegate_preflight(
            &state,
            &shared_agent.id,
            Some("delegate-child"),
            &target_agent.id,
        )
        .expect("resolve nested sync delegate preflight");
        let call_stack = resolve_delegate_call_stack(
            preflight.current_thread.as_ref(),
            &preflight.source_agent,
            &preflight.target_agent,
        )
        .expect("resolve call stack");

        assert_eq!(preflight.source_agent.id, "shared-agent");
        assert_eq!(preflight.target_agent.id, "target-agent");
        assert_eq!(preflight.root_conversation_id, "conversation-root");
        assert_eq!(
            call_stack,
            vec!["shared-agent".to_string(), "target-agent".to_string()]
        );
    }

    #[test]
    fn common_delegate_preflight_should_resolve_source_agent_from_source_conversation() {
        let state = test_chat_runtime_state();
        let mut shared_agent = default_agent();
        shared_agent.id = "shared-agent-explicit-source".to_string();
        shared_agent.name = "共享人格".to_string();

        let mut other_agent = default_agent();
        other_agent.id = "other-agent".to_string();
        other_agent.name = "传入但非来源人格".to_string();

        let config = AppConfig::default();
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(
            &state,
            &[
                shared_agent.clone(),
                other_agent.clone(),
                default_user_persona(),
            ],
        )
        .expect("write agents");

        let conversation = build_conversation_record(
            "api-a",
            &shared_agent.id,
            "来源会话人格优先",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");

        // 传入的 source_agent_id 与来源会话不一致时，以来源会话上的人格为准。
        let preflight = common_delegate_preflight(
            &state,
            &other_agent.id,
            Some(&conversation.id),
            &shared_agent.id,
        )
        .expect("resolve delegate preflight");

        assert_eq!(preflight.source_agent.id, shared_agent.id);
        assert_eq!(preflight.target_agent.id, shared_agent.id);
        assert_eq!(preflight.root_conversation_id, conversation.id);
    }

    #[test]
    fn common_delegate_preflight_should_reject_unknown_target_agent() {
        let state = test_chat_runtime_state();
        let mut source_agent = default_agent();
        source_agent.id = "source-agent".to_string();
        source_agent.name = "源人格".to_string();

        let config = AppConfig::default();
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(&state, &[source_agent.clone(), default_user_persona()])
            .expect("write agents");

        let conversation = build_conversation_record(
            "api-a",
            &source_agent.id,
            "源人格会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");

        let result = common_delegate_preflight(
            &state,
            &source_agent.id,
            Some(&conversation.id),
            "missing-agent",
        );

        assert!(result.is_err());
    }

    #[test]
    fn common_delegate_preflight_should_reject_non_direct_child_target_in_validation() {
        let state = test_chat_runtime_state();
        let mut source_agent = default_agent();
        source_agent.id = "source-agent".to_string();
        source_agent.name = "源人格".to_string();
        source_agent.child_agent_ids = Vec::new();

        let mut target_agent = default_agent();
        target_agent.id = "target-agent".to_string();
        target_agent.name = "目标人格".to_string();

        let config = AppConfig::default();
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(
            &state,
            &[
                source_agent.clone(),
                target_agent.clone(),
                default_user_persona(),
            ],
        )
        .expect("write agents");

        let conversation = build_conversation_record(
            "api-a",
            &source_agent.id,
            "源人格会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");

        let preflight = common_delegate_preflight(
            &state,
            &source_agent.id,
            Some(&conversation.id),
            &target_agent.id,
        )
        .expect("delegate scheduling should allow any target agent");

        assert_eq!(preflight.source_agent.id, source_agent.id);
        assert_eq!(preflight.target_agent.id, target_agent.id);
        assert_eq!(preflight.root_conversation_id, conversation.id);
        assert!(validate_delegate_tool_direct_child_target(&preflight).is_err());
    }

    #[test]
    fn common_delegate_preflight_should_accept_direct_child_agent() {
        let state = test_chat_runtime_state();
        let mut parent_agent = default_agent();
        parent_agent.id = "parent-agent".to_string();
        parent_agent.name = "主负责人格".to_string();

        let mut private_agent = default_agent();
        private_agent.id = "private-agent".to_string();
        private_agent.name = "私域人格".to_string();

        parent_agent.child_agent_ids = vec![private_agent.id.clone()];

        let config = AppConfig::default();
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(
            &state,
            &[
                parent_agent.clone(),
                private_agent.clone(),
                default_user_persona(),
            ],
        )
        .expect("write agents");

        let conversation = build_conversation_record(
            "api-a",
            &parent_agent.id,
            "主负责人格会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");

        let preflight = common_delegate_preflight(
            &state,
            &parent_agent.id,
            Some(&conversation.id),
            &private_agent.id,
        )
        .expect("resolve private child delegate preflight");

        assert_eq!(preflight.source_agent.id, parent_agent.id);
        assert_eq!(preflight.target_agent.id, private_agent.id);
        assert_eq!(preflight.root_conversation_id, conversation.id);
        assert!(validate_delegate_tool_direct_child_target(&preflight).is_ok());
    }

    #[test]
    fn resolve_user_async_delegate_plan_should_accept_direct_child_agent() {
        let state = test_chat_runtime_state();
        let mut selected_api = ApiConfig::default();
        selected_api.id = "api-a".to_string();
        selected_api.name = "测试模型".to_string();
        selected_api.request_format = RequestFormat::OpenAI;
        selected_api.enable_text = true;
        selected_api.enable_tools = true;
        selected_api.base_url = "https://api.openai.com/v1".to_string();
        selected_api.api_key = "k".to_string();
        selected_api.model = "gpt-4o-mini".to_string();

        let mut parent_agent = default_agent();
        parent_agent.id = "parent-agent".to_string();
        parent_agent.name = "主负责人格".to_string();

        let mut private_agent = default_agent();
        private_agent.id = "private-agent".to_string();
        private_agent.name = "私域人格".to_string();
        private_agent.api_config_ids =
            vec![api_endpoint_id("api-a", "api-a-model-default")];
        private_agent.api_config_id = api_endpoint_id("api-a", "api-a-model-default");

        let config = AppConfig {
            api_configs: vec![selected_api.clone()],
            api_providers: Vec::new(),
            selected_api_config_id: selected_api.id.clone(),
            expert_api_config_id: selected_api.id.clone(),
            ..AppConfig::default()
        };
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(
            &state,
            &[
                parent_agent.clone(),
                private_agent.clone(),
                default_user_persona(),
            ],
        )
        .expect("write agents");

        let conversation = build_conversation_record(
            &selected_api.id,
            &parent_agent.id,
            "主负责人格会话",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        state_schedule_conversation_persist(&state, &conversation)
            .expect("persist conversation");

        let (plan, selected_count) = resolve_user_async_delegate_plan(
            &state,
            &SubmitUserAsyncDelegateInput {
                conversation_id: conversation.id.clone(),
                target_agent_id: Some(private_agent.id.clone()),
                preset_id: None,
                why: None,
                goal: Some("请调查这个问题".to_string()),
                todo: None,
                background: None,
                question: None,
                focus: None,
                selected_message_ids: Vec::new(),
            },
        )
        .expect("resolve async delegate plan");

        assert_eq!(selected_count, 0);
        assert_eq!(plan.root_conversation_id, conversation.id);
        assert_eq!(plan.source_agent_id, parent_agent.id);
        assert_eq!(plan.target_agent_id, private_agent.id);
        assert_eq!(plan.target_agent_name, "私域人格");
        assert_eq!(
            plan.target_api_config_ids,
            vec![api_endpoint_id("api-a", "api-a-model-default")]
        );
    }

    #[test]
    fn delegate_target_chat_api_config_ids_should_only_keep_current_department_models() {
        let app_config = AppConfig {
            api_configs: vec![ApiConfig {
                id: "provider-a::model-a".to_string(),
                name: "provider-a/model-a".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_video: false,
                enable_tools: true,
                tools: vec![],
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "k".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                codex_custom_url: None,
                codex_custom_api_key: None,
                codex_originator: default_codex_originator(),
                codex_residency_requirement: None,
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
        let mut agent = default_agent();
        agent.id = "agent-a".to_string();
        agent.name = "人格 A".to_string();
        agent.api_config_ids = vec!["provider-a::model-a".to_string(), "provider-a".to_string()];
        agent.api_config_id = "provider-a::model-a".to_string();
        agent.model_failure_fallback_enabled = true;

        let resolved = delegate_target_chat_api_config_ids(&app_config, &agent);

        assert_eq!(resolved, vec!["provider-a::model-a".to_string()]);
    }

    #[test]
    fn delegate_target_chat_api_config_ids_should_keep_only_primary_when_fallback_disabled() {
        let expert_id = "provider-a::expert";
        let quick_id = "provider-a::quick";
        let app_config = AppConfig {
            api_configs: vec![
                ApiConfig {
                    id: expert_id.to_string(),
                    name: "provider-a/expert".to_string(),
                    request_format: RequestFormat::OpenAI,
                    allow_concurrent_requests: false,
                    max_concurrent_requests: None,
                    enable_text: true,
                    enable_image: false,
                    enable_audio: false,
                    enable_video: false,
                    enable_tools: true,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
                    codex_custom_url: None,
                    codex_custom_api_key: None,
                    codex_originator: default_codex_originator(),
                    codex_residency_requirement: None,
                    model: "expert".to_string(),
                    reasoning_effort: default_reasoning_effort(),
                    temperature: 1.0,
                    custom_temperature_enabled: false,
                    context_window_tokens: 128_000,
                    max_output_tokens: 4_096,
                    custom_max_output_tokens_enabled: false,
                    failure_retry_count: 0,
                },
                ApiConfig {
                    id: quick_id.to_string(),
                    name: "provider-a/quick".to_string(),
                    request_format: RequestFormat::OpenAI,
                    allow_concurrent_requests: false,
                    max_concurrent_requests: None,
                    enable_text: true,
                    enable_image: false,
                    enable_audio: false,
                    enable_video: false,
                    enable_tools: true,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
                    codex_custom_url: None,
                    codex_custom_api_key: None,
                    codex_originator: default_codex_originator(),
                    codex_residency_requirement: None,
                    model: "quick".to_string(),
                    reasoning_effort: default_reasoning_effort(),
                    temperature: 1.0,
                    custom_temperature_enabled: false,
                    context_window_tokens: 128_000,
                    max_output_tokens: 4_096,
                    custom_max_output_tokens_enabled: false,
                    failure_retry_count: 0,
                },
            ],
            api_providers: Vec::new(),
            expert_api_config_id: expert_id.to_string(),
            tool_review_api_config_id: Some(quick_id.to_string()),
            ..AppConfig::default()
        };
        let mut agent = default_agent();
        agent.id = "agent-a".to_string();
        agent.name = "人格 A".to_string();
        agent.api_config_ids = vec![
            MODEL_ROLE_EXPERT_API_CONFIG_ID.to_string(),
            MODEL_ROLE_QUICK_API_CONFIG_ID.to_string(),
        ];
        agent.api_config_id = MODEL_ROLE_EXPERT_API_CONFIG_ID.to_string();
        agent.model_failure_fallback_enabled = false;

        let resolved = delegate_target_chat_api_config_ids(&app_config, &agent);

        assert_eq!(resolved, vec![expert_id.to_string()]);
    }

    #[test]
    fn build_organization_prompt_block_should_keep_same_persona_child_agents() {
        let mut parent = default_agent();
        parent.id = "agent-parent".to_string();
        parent.name = "父人格".to_string();
        parent.summary = "当任务需要总控时叫我".to_string();
        parent.child_agent_ids = vec!["agent-child".to_string()];

        let mut child = default_agent();
        child.id = "agent-child".to_string();
        child.name = "同人格下级".to_string();
        child.summary = "当任务需要专项摸底时叫我".to_string();

        let agents = vec![parent, child];
        let block = build_organization_prompt_block("agent-parent", &agents, "zh-CN");

        assert!(block.contains("人格：父人格"));
        assert!(block.contains("你的直属下级人格："));
        assert!(!block.contains("当任务需要总控时叫我"));
        assert!(block.contains("同人格下级"));
        assert!(block.contains("概述：当任务需要专项摸底时叫我"));
    }

    #[test]
    fn exec_tool_rule_should_include_rg_guidance_when_rg_installed() {
        let block = build_builtin_tool_rule_block("exec", true).expect("exec tool rule");

        assert!(block.contains("当前环境 `rg` 可用时，优先使用 `rg` 进行搜索"));
        assert!(block.contains("rg --files"));
    }

    #[test]
    fn exec_tool_rule_should_omit_rg_guidance_when_rg_unavailable() {
        let block = build_builtin_tool_rule_block("exec", false).expect("exec tool rule");

        assert!(!block.contains("`rg`"));
        assert!(!block.contains("rg --files"));
        assert!(!block.contains("rg -n"));
    }

    #[test]
    fn delegate_target_chat_api_config_ids_should_not_fallback_when_department_binding_invalid() {
        let app_config = AppConfig {
            api_configs: vec![ApiConfig {
                id: "provider-a::model-a".to_string(),
                name: "provider-a/model-a".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_video: false,
                enable_tools: true,
                tools: vec![],
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "k".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                codex_custom_url: None,
                codex_custom_api_key: None,
                codex_originator: default_codex_originator(),
                codex_residency_requirement: None,
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
        let mut agent = default_agent();
        agent.id = "agent-a".to_string();
        agent.name = "人格 A".to_string();
        agent.api_config_ids = vec!["provider-a".to_string()];
        agent.api_config_id = "provider-a".to_string();
        agent.model_failure_fallback_enabled = false;

        let resolved = delegate_target_chat_api_config_ids(&app_config, &agent);

        assert!(resolved.is_empty());
    }

    #[test]
    fn conversation_meta_is_unarchived_meta_view_should_follow_archived_at() {
        let state = test_chat_runtime_state();
        let now = now_utc_rfc3339();
        let mut data = AppData::default();
        let mut conversation = Conversation {
            id: "conversation-summary-only".to_string(),
            title: "摘要会话".to_string(),
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
            updated_at: now.clone(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: Vec::new(),
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        };
        let conversation_id = conversation.id.clone();
        data.conversations.push(conversation.clone());
        state_write_app_data_cached(&state, &data).expect("write app data");
        let meta = conversation_service_v2()
            .get_conversation_meta(&state, &conversation_id)
            .expect("get conversation meta");

        assert!(conversation_service_v2().conversation_meta_is_unarchived_meta_view(&meta));

        conversation.archived_at = Some(now_utc_rfc3339());
        let archived_meta = ConversationMetaView {
            archived_at: conversation.archived_at.clone(),
            ..meta
        };
        assert!(!conversation_service_v2().conversation_meta_is_unarchived_meta_view(&archived_meta));
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
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: Vec::new(),
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
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

        let conversation = state_read_conversation_cached(&state, "conversation-main")
            .expect("read conversation");
        assert_eq!(conversation.current_todos.len(), 2);
        assert_eq!(conversation.current_todos[0].content, "第一步");
        assert_eq!(conversation.current_todos[0].status, "completed");
        assert_eq!(conversation.current_todos[1].content, "第二步");
        assert_eq!(conversation.current_todos[1].status, "in_progress");
        assert_eq!(
            conversation_current_todo_text(&conversation).as_deref(),
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
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: Vec::new(),
            fast_request_turns: Vec::new(),
            current_todos: vec![ConversationTodoItem {
                content: "旧步骤".to_string(),
                status: "in_progress".to_string(),
            }],
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
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

        let conversation = state_read_conversation_cached(&state, "conversation-main")
            .expect("read conversation");
        assert!(conversation.current_todos.is_empty());
        assert_eq!(conversation_current_todo_text(&conversation), None);
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
        let state = test_chat_runtime_state();
        let mut data = test_user_switched_to_sub_conversation_data();
        let idx = resolve_unarchived_conversation_index_with_fallback(
            &mut data,
            &state,
            &AppConfig::default(),
            DEFAULT_AGENT_ID,
            Some("conversation-main"),
        )
        .expect("resolve requested conversation");

        assert_eq!(data.conversations[idx].id, "conversation-main");
    }

    #[test]
    fn resolve_unarchived_conversation_index_with_fallback_should_error_when_requested_missing() {
        let state = test_chat_runtime_state();
        let mut data = test_user_switched_to_sub_conversation_data();
        let err = resolve_unarchived_conversation_index_with_fallback(
            &mut data,
            &state,
            &AppConfig::default(),
            DEFAULT_AGENT_ID,
            Some("conversation-missing"),
        )
        .expect_err("missing requested conversation should fail");

        assert!(err.contains("Requested conversation not found"));
    }

    #[test]
    fn set_active_conversation_should_error_when_requested_missing() {
        let state = test_chat_runtime_state();
        let config = AppConfig::default();
        write_config(&state.config_path, &config).expect("write config");
        let data = test_user_switched_to_sub_conversation_data();
        state_write_app_data_cached(&state, &data).expect("write app data");

        let err = conversation_service_v2()
            .set_active_conversation(
                &state,
                &SetActiveUnarchivedConversationInput {
                    conversation_id: Some("conversation-missing".to_string()),
                    agent_id: None,
                },
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
        let mut source = test_chat_conversation("conversation-main", "active", &now);
        source.status = "archived".to_string();
        source.archived_at = Some(now.clone());
        state_service_set_main_conversation_id(&state, Some("conversation-main"))
            .expect("write main conversation id");
        let mut data = AppData::default();
        data.conversations = vec![
            source.clone(),
            test_chat_conversation("conversation-sub", "inactive", &later),
        ];
        state_write_app_data_cached(&state, &data).expect("write app data");

        let next_id = delete_main_conversation_and_activate_latest(&state, &selected_api, &source)
            .expect("delete main conversation");
        let system_notification = state_read_conversation_cached(
            &state,
            SYSTEM_NOTIFICATION_CONVERSATION_ID,
        )
        .expect("read system notification conversation");
        let promoted = state_read_conversation_cached(&state, &next_id)
            .expect("read promoted conversation");

        assert_eq!(next_id, "conversation-sub");
        assert_eq!(
            state_service_get_main_conversation_id(&state)
                .expect("read main conversation id")
                .as_deref(),
            Some(SYSTEM_NOTIFICATION_CONVERSATION_ID)
        );
        assert!(conversation_is_system_notification(&system_notification));
        assert_eq!(promoted.status, "inactive");
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
        let mut source = test_chat_conversation("conversation-main", "active", &now);
        source.status = "archived".to_string();
        source.archived_at = Some(now.clone());
        state_service_set_main_conversation_id(&state, Some("conversation-main"))
            .expect("write main conversation id");
        let mut data = AppData::default();
        data.conversations = vec![source.clone()];
        state_write_app_data_cached(&state, &data).expect("write app data");

        let next_id = delete_main_conversation_and_activate_latest(&state, &selected_api, &source)
            .expect("delete last main conversation");
        let system_notification = state_read_conversation_cached(
            &state,
            SYSTEM_NOTIFICATION_CONVERSATION_ID,
        )
        .expect("read system notification conversation");
        let replacement = state_read_conversation_cached(&state, &next_id)
            .expect("read replacement conversation");

        assert_ne!(next_id, "conversation-main");
        assert_ne!(next_id, SYSTEM_NOTIFICATION_CONVERSATION_ID);
        assert_eq!(
            state_service_get_main_conversation_id(&state)
                .expect("read main conversation id")
                .as_deref(),
            Some(SYSTEM_NOTIFICATION_CONVERSATION_ID)
        );
        assert!(conversation_is_system_notification(&system_notification));
        assert_eq!(replacement.status, "active");
    }

    #[test]
    fn archiving_main_conversation_should_promote_existing_sub_conversation() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let later = (now_utc() + time::Duration::minutes(1))
            .format(&Rfc3339)
            .expect("format later");
        let mut data = AppData::default();
        data.conversations = vec![
            test_chat_conversation("conversation-main", "active", &now),
            test_chat_conversation("conversation-sub", "inactive", &later),
        ];
        state_service_set_main_conversation_id(&state, Some("conversation-main"))
            .expect("write main conversation id");

        archive_conversation_now(&mut data, "conversation-main", "test")
            .expect("archive current main");
        let idx = ensure_main_conversation_index(&mut data, &state, "", DEFAULT_AGENT_ID).expect("ensure main conversation index");

        assert_eq!(data.conversations[idx].id, SYSTEM_NOTIFICATION_CONVERSATION_ID);
        assert_eq!(
            data.conversations[idx].conversation_kind,
            CONVERSATION_KIND_SYSTEM_NOTIFICATION
        );
        assert_eq!(
            state_service_get_main_conversation_id(&state)
                .expect("read main conversation id")
                .as_deref(),
            Some(SYSTEM_NOTIFICATION_CONVERSATION_ID)
        );
    }

    #[test]
    fn archiving_last_main_conversation_should_create_replacement_main_conversation() {
        let state = test_chat_runtime_state();
        let now = now_iso();
        let mut data = AppData::default();
        data.conversations = vec![test_chat_conversation("conversation-main", "active", &now)];
        state_service_set_main_conversation_id(&state, Some("conversation-main"))
            .expect("write main conversation id");

        archive_conversation_now(&mut data, "conversation-main", "test")
            .expect("archive last main");
        let idx = ensure_main_conversation_index(&mut data, &state, "api-default", DEFAULT_AGENT_ID).expect("ensure main conversation index");

        assert_eq!(data.conversations[idx].id, SYSTEM_NOTIFICATION_CONVERSATION_ID);
        assert_eq!(
            state_service_get_main_conversation_id(&state)
                .expect("read main conversation id")
                .as_deref(),
            Some(SYSTEM_NOTIFICATION_CONVERSATION_ID)
        );
        assert_eq!(data.conversations[idx].status, "active");
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

        runtime_log_info(format!(
            "[压测] prepared_prompt_to_messages_json 大上下文结果：messages={}，bytes={}，elapsed={}ms",
            messages.len(),
            json_bytes.len(),
            elapsed_ms
        ));

        assert!(json_bytes.len() > 1_500_000);
    }

    #[test]
    #[ignore = "压测探针：本地按需运行 cargo test llm_round_log_large_response_probe -- --ignored --nocapture"]
    fn llm_round_log_large_response_probe() {
        // 已切换至调度事件：通过真实调度事件路径写入大响应并校验序列化后体积
        let state = test_chat_runtime_state();
        let large_response = "响应片段。".repeat(220_000);
        let conversation_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let runtime_context = RuntimeContext::default();
        let started_at = now_iso();
        let dispatch_detail = serde_json::json!({
            "traceId": run_id,
            "provider": "probe",
            "model": "probe-model",
            "baseUrl": "https://example.test",
            "headers": [],
            "tools": []
        });
        schedule_event_run_start(&state, &runtime_context, &conversation_id, &run_id, &started_at, 0, dispatch_detail)
            .expect("schedule run start");
        let large_detail = serde_json::json!({
            "textPreview": large_response,
            "assistantTextLength": large_response.chars().count(),
            "usage": { "promptTokens": 1, "completionTokens": 1 }
        });
        schedule_event_push_inner(&state, &conversation_id, &run_id, "model_round_end", 1, Some(true), large_detail)
            .expect("push large scheduling event");
        let store = state.schedule_events.lock().expect("lock schedule events");
        let run = store
            .runs_by_conversation
            .get(&conversation_id)
            .and_then(|deque| deque.iter().find(|item| item.run_id == run_id))
            .expect("probe run should exist");
        let serialized = serde_json::to_vec(run).expect("serialize probe run");
        // 也可通过整库估算校验
        let estimated = schedule_event_estimated_json_bytes(&store);
        runtime_log_info(format!(
            "[压测] llm_round_log_large_response 调度事件序列化结果：run_bytes={}，store_estimated_bytes={}，events={}",
            serialized.len(),
            estimated,
            run.events.len()
        ));
        assert!(serialized.len() > 1_500_000, "serialized scheduling event should exceed threshold, got {}", serialized.len());
        assert!(estimated > 1_500_000, "estimated store bytes should exceed threshold, got {}", estimated);
    }

    #[test]
    fn model_reply_log_value_should_keep_activity_reasoning_text() {
        let reply = ModelReply {
            assistant_text: "最终答复".to_string(),
            final_response_text: "最终答复".to_string(),
            activity_reasoning_text: "完整思维链".to_string(),
            assistant_provider_meta: None,
            tool_history_events: Vec::new(),
            suppress_assistant_message: false,
            usage: None,
            trusted_input_tokens: None,
            round_logs_recorded_internally: false,
        };

        let value = model_reply_to_log_value(&reply);

        assert_eq!(value["assistantText"].as_str(), Some("最终答复"));
        assert_eq!(
            value["activityReasoningText"].as_str(),
            Some("完整思维链")
        );
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
                    reasoning_content: None,
                }],
                extra_text_blocks,
                provider_meta,
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
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
            )
            .expect("build perf probe prepared prompt");
            latest_extra_len = prepared.latest_user_extra_text.len();
            history_len = prepared.history_messages.len();
            assert!(!prepared.preamble.trim().is_empty());
        }
        let total_ms = started.elapsed().as_millis() as u64;
        let avg_ms = total_ms / u64::from(runs);
        runtime_log_info(format!(
            "[提示词性能探针] build_prepared_prompt_for_mode 平均耗时={}ms, total={}ms, runs={}, history_len={}, latest_extra_len={}",
            avg_ms,
            total_ms,
            runs,
            history_len,
            latest_extra_len
        ));
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
            image::guess_format(&normalized.bytes).expect("guess format"),
            image::ImageFormat::WebP
        );

        let decoded = image::load_from_memory(&normalized.bytes).expect("decode webp");
        assert_eq!(decoded.width(), normalized.output_width);
        assert_eq!(decoded.height(), normalized.output_height);
        assert!(
            u64::from(decoded.width()) * u64::from(decoded.height())
                <= IMAGE_NORMALIZE_FOR_LLM_REQUEST_DEFAULT_PIXEL_BUDGET
        );
    }

    #[test]
    fn build_user_parts_should_prefer_saved_path_and_ignore_duplicate_bad_base64() {
        let state = test_chat_runtime_state();
        let payload = ChatInputPayload {
            text: None,
            display_text: None,
            parts: None,
            images: Some(vec![BinaryPart {
                mime: "image/png".to_string(),
                bytes_base64: "%%%bad-base64%%%".to_string(),
                saved_path: Some("downloads/bad-image.png".to_string()),
            }]),
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };

        let api = ApiConfig {
            enable_image: true,
            ..ApiConfig::default()
        };
        let parts = build_user_parts(&state, &payload, &api).expect("build parts");

        assert_eq!(parts.len(), 1);
        match &parts[0] {
            MessagePart::Attachment { path, mime, name } => {
                assert!(std::path::Path::new(path).is_absolute());
                assert!(path.replace('\\', "/").ends_with("downloads/bad-image.png"));
                assert_eq!(mime, "image/png");
                assert_eq!(name, "bad-image.png");
            }
            other => panic!("expected canonical attachment, got {other:?}"),
        }
    }

    #[test]
    fn build_user_parts_should_keep_visible_notice_when_one_attachment_fails() {
        let state = test_chat_runtime_state();
        let payload = ChatInputPayload {
            text: None,
            display_text: None,
            parts: Some(vec![
                ChatIngressPart::Text {
                    text: "正常文字".to_string(),
                },
                ChatIngressPart::Attachment {
                    path: None,
                    bytes_base64: Some("%%%bad-base64%%%".to_string()),
                    mime: "image/png".to_string(),
                    name: "broken.png".to_string(),
                },
            ]),
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        };
        let api = ApiConfig {
            enable_image: true,
            ..ApiConfig::default()
        };

        let parts = build_user_parts(&state, &payload, &api).expect("build parts");

        assert_eq!(parts.len(), 2);
        assert!(matches!(&parts[0], MessagePart::Text { text, .. } if text == "正常文字"));
        assert!(matches!(&parts[1], MessagePart::Text { text, .. }
            if text.contains("broken.png") && text.contains("已跳过该附件并继续")));
    }

    #[test]
    fn remote_wake_compaction_should_skip_only_below_fourteen_block_messages() {
        assert!(remote_im_wake_compaction_should_skip_for_low_frequency(0));
        assert!(remote_im_wake_compaction_should_skip_for_low_frequency(13));
        assert!(!remote_im_wake_compaction_should_skip_for_low_frequency(14));
    }

    fn collect_rs_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(root) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_rs_files(&path));
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                out.push(path);
            }
        }
        out
    }

    #[test]
    fn assistant_message_mutations_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("assistant_message_mutations.rs");
        let content = std::fs::read_to_string(&file).expect("read assistant message mutations");

        assert!(
            !content.contains("conversation_mutation_gate("),
            "assistant message 写入链必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
        assert!(
            content.contains("with_conversation_mutation"),
            "assistant message 写入链应保留统一会话 mutation 入口"
        );
    }

    #[test]
    fn metadata_short_mutations_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("metadata_mutations.rs");
        let content = std::fs::read_to_string(&file).expect("read metadata mutations");
        let rename_start = content
            .find("fn rename_conversation")
            .expect("rename_conversation exists");
        let rename_end = content
            .find("fn update_latest_summary_title")
            .expect("update_latest_summary_title exists");
        let usage_start = content
            .find("fn add_conversation_cumulative_usage_delta")
            .expect("add_conversation_cumulative_usage_delta exists");
        let usage_section = &content[usage_start..];

        assert!(
            !content[rename_start..rename_end].contains("conversation_mutation_gate("),
            "rename_conversation 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
        assert!(
            usage_section.contains("with_conversation_mutation"),
            "add_conversation_cumulative_usage_delta 应保留统一会话 mutation 入口"
        );
        assert!(
            !usage_section.contains("conversation_mutation_gate("),
            "add_conversation_cumulative_usage_delta 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
    }

    #[test]
    fn scheduler_history_flush_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("scheduler_history_flush.rs");
        let content = std::fs::read_to_string(&file).expect("read scheduler history flush");

        assert!(
            !content.contains("conversation_mutation_gate("),
            "commit_scheduler_history_flush 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
        assert!(
            content.contains("with_conversation_mutation"),
            "commit_scheduler_history_flush 应保留统一会话 mutation 入口"
        );
    }

    #[test]
    fn rewind_conversation_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("history_mutations.rs");
        let content = std::fs::read_to_string(&file).expect("read history mutations");
        let rewind_start = content
            .find("fn rewind_conversation")
            .expect("rewind_conversation exists");
        let rewind_end = content
            .find("fn is_first_context_compaction_message_in_store")
            .expect("rewind helper exists");

        assert!(
            content[rewind_start..rewind_end].contains("with_conversation_mutation"),
            "rewind_conversation 应保留统一会话 mutation 入口"
        );
        assert!(
            !content[rewind_start..rewind_end].contains("conversation_mutation_gate("),
            "rewind_conversation 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
    }

    #[test]
    fn active_plan_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("message_store")
            .join("active_plan.rs");
        let content = std::fs::read_to_string(&file).expect("read active plan");

        assert!(
            !content.contains("conversation_mutation_gate("),
            "active_plan 写入链必须走 with_conversation_mutation_for_data_path，禁止重新裸用 conversation_mutation_gate"
        );
        assert!(
            content.contains("with_conversation_mutation_for_data_path"),
            "active_plan 写入链应保留统一会话 mutation 入口"
        );
    }

    #[test]
    fn remote_im_management_writes_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("remote_im_sessions.rs");
        let content = std::fs::read_to_string(&file).expect("read remote im sessions");
        let prune_start = content
            .find("fn prune_expired_remote_im_fast_request_turns")
            .expect("prune_expired_remote_im_fast_request_turns exists");
        let prune_end = content
            .find("fn get_active_goal")
            .expect("get_active_goal exists");
        let goal_start = content
            .find("fn update_goal_conversation")
            .expect("update_goal_conversation exists");
        let goal_end = content
            .find("fn remote_im_runtime_state_should_cache_blocks")
            .expect("remote_im_runtime_state_should_cache_blocks exists");
        let prune_section = &content[prune_start..prune_end];
        let goal_section = &content[goal_start..goal_end];

        assert!(
            prune_section.contains("with_conversation_mutation"),
            "prune_expired_remote_im_fast_request_turns 应保留统一会话 mutation 入口"
        );
        assert!(
            !prune_section.contains("conversation_mutation_gate("),
            "prune_expired_remote_im_fast_request_turns 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
        assert!(
            goal_section.contains("with_conversation_mutation"),
            "update_goal_conversation 应保留统一会话 mutation 入口"
        );
        assert!(
            goal_section.contains("lock_conversation_with_metrics"),
            "update_goal_conversation 的 delegate 兜底分支应继续保留带指标的会话锁"
        );
        assert!(
            !goal_section.contains("conversation_mutation_gate("),
            "update_goal_conversation 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
    }

    #[test]
    fn remote_im_short_writes_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("remote_im_sessions.rs");
        let content = std::fs::read_to_string(&file).expect("read remote im sessions");
        let update_start = content
            .find("fn update_unarchived_conversation_by_id")
            .expect("update_unarchived_conversation_by_id exists");
        let update_end = content
            .find("fn append_fast_request_turn_if_unarchived_exists")
            .expect("append_fast_request_turn_if_unarchived_exists exists");
        let fast_turn_start = update_end;
        let fast_turn_end = content
            .find("fn get_conversation_fast_request_turns")
            .expect("get_conversation_fast_request_turns exists");
        let update_section = &content[update_start..update_end];
        let fast_turn_section = &content[fast_turn_start..fast_turn_end];

        for (name, section) in [
            ("update_unarchived_conversation_by_id", update_section),
            ("append_fast_request_turn_if_unarchived_exists", fast_turn_section),
        ] {
            assert!(
                section.contains("with_conversation_mutation"),
                "{name} 应保留统一会话 mutation 入口"
            );
            assert!(
                !section.contains("conversation_mutation_gate("),
                "{name} 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
            );
        }
    }

    #[test]
    fn mark_conversation_read_entries_should_be_async_spawn_blocking() {
        // Tauri command：unarchived_conversations.rs 中 mark_conversation_read 必须是 async fn + spawn_blocking
        let command_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("system")
            .join("commands")
            .join("config_and_persona")
            .join("unarchived_conversations.rs");
        let command_content = std::fs::read_to_string(&command_file).expect("read unarchived conversations");
        let command_start = command_content
            .find("async fn mark_conversation_read")
            .expect("mark_conversation_read command should be async fn");
        let command_end = command_content[command_start..]
            .find("\n}\n\n#[tauri::command]")
            .map(|offset| command_start + offset + 3)
            .or_else(|| {
                command_content[command_start..]
                    .find("\n}\n\nasync fn ")
                    .map(|offset| command_start + offset + 3)
            })
            .unwrap_or((command_start + 600).min(command_content.len()));
        let mut end = command_end.min(command_content.len());
        while end > command_start && !command_content.is_char_boundary(end) {
            end -= 1;
        }
        let command_section = &command_content[command_start..end];
        assert!(
            command_section.contains("spawn_blocking"),
            "mark_conversation_read Tauri command 必须使用 spawn_blocking 移出主线程"
        );

        // IDE JSON-RPC：jsonrpc_dispatch.rs 两个分支都必须 .await
        let dispatch_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("system")
            .join("commands")
            .join("ide_context")
            .join("jsonrpc_dispatch.rs");
        let dispatch_content = std::fs::read_to_string(&dispatch_file).expect("read jsonrpc dispatch");
        assert!(
            dispatch_content.contains("conversation.markRead\" => ide_chat_mark_conversation_read(state, request.params).await"),
            "IDE conversation.markRead 分支必须 .await"
        );
        assert!(
            dispatch_content.contains("\"mark_conversation_read\" => ide_chat_mark_conversation_read_command(state, request.params).await"),
            "IDE mark_conversation_read 分支必须 .await"
        );

        // IDE handler 本体：chat_methods.rs 中 ide_chat_mark_conversation_read 必须是 async fn + spawn_blocking，
        // 防止回退为同步服务调用
        let methods_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("system")
            .join("commands")
            .join("ide_context")
            .join("chat_methods.rs");
        let methods_content = std::fs::read_to_string(&methods_file).expect("read IDE chat methods");
        let mark_read_start = methods_content
            .find("async fn ide_chat_mark_conversation_read(")
            .expect("IDE mark conversation read should be async fn");
        // 截取到下一个函数定义边界，避免硬截断落在多字节字符中间
        let next_fn_offset = methods_content[mark_read_start..]
            .find("\n}\n\nasync fn ")
            .map(|offset| mark_read_start + offset + 3)
            .unwrap_or(methods_content.len());
        let mark_read_section = &methods_content[mark_read_start..next_fn_offset];
        assert!(
            mark_read_section.contains("spawn_blocking"),
            "IDE mark conversation read 必须使用 spawn_blocking 移出主线程"
        );
    }

    #[test]
    fn switch_active_conversation_snapshot_should_be_async_spawn_blocking() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("system")
            .join("commands")
            .join("config_and_persona")
            .join("unarchived_conversations.rs");
        let content = std::fs::read_to_string(&file).expect("read unarchived conversations");
        let start = content
            .find("async fn switch_active_conversation_snapshot(")
            .expect("switch_active_conversation_snapshot should be async fn");
        let next_fn_offset = content[start..]
            .find("\n}\n\n#[tauri::command]")
            .map(|offset| start + offset + 3)
            .unwrap_or(content.len());
        let section = &content[start..next_fn_offset];
        assert!(
            section.contains("spawn_blocking"),
            "switch_active_conversation_snapshot 必须使用 spawn_blocking 移出主线程"
        );
    }

    #[test]
    fn message_read_command_family_should_be_async_spawn_blocking() {
        // 会话消息读取命令族（主会话 + 委托 + 归档 + 远程 IM）：unarchived_conversations.rs 10 个命令
        let unarchived_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("system")
            .join("commands")
            .join("config_and_persona")
            .join("unarchived_conversations.rs");
        let unarchived_content =
            std::fs::read_to_string(&unarchived_file).expect("read unarchived conversations");
        for name in [
            "get_unarchived_conversation_messages",
            "get_unarchived_conversation_recent_block_messages",
            "get_unarchived_conversation_block_page",
            "get_unarchived_conversation_recent_messages",
            "get_unarchived_conversation_message_by_id",
            "get_delegate_conversation_messages",
            "get_delegate_conversation_block_page",
            "get_active_conversation_messages",
            "get_active_conversation_messages_before",
            "get_active_conversation_messages_after",
        ] {
            let start = unarchived_content
                .find(&format!("async fn {name}("))
                .unwrap_or_else(|| panic!("{name} 应为 async fn"));
            let next_fn_offset = unarchived_content[start..]
                .find("\n}\n\n#[tauri::command]")
                .map(|offset| start + offset + 3)
                .unwrap_or(unarchived_content.len());
            let section = &unarchived_content[start..next_fn_offset];
            assert!(
                section.contains("spawn_blocking"),
                "{name} 必须使用 spawn_blocking 移出主线程"
            );
        }

        // 归档消息读取：archive_commands.rs 3 个命令
        let archive_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("system")
            .join("commands")
            .join("archive_commands.rs");
        let archive_content = std::fs::read_to_string(&archive_file).expect("read archive commands");
        for name in ["get_archive_messages", "get_archive_block_page"] {
            let start = archive_content
                .find(&format!("async fn {name}("))
                .unwrap_or_else(|| panic!("{name} 应为 async fn"));
            let next_fn_offset = archive_content[start..]
                .find("\n}\n\n#[tauri::command]")
                .map(|offset| start + offset + 3)
                .unwrap_or(archive_content.len());
            let section = &archive_content[start..next_fn_offset];
            assert!(
                section.contains("spawn_blocking"),
                "{name} 必须使用 spawn_blocking 移出主线程"
            );
        }

        // 远程 IM 联系人会话消息读取：contact_commands.rs 2 个命令
        let contact_file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("remote_im")
            .join("contact_commands.rs");
        let contact_content = std::fs::read_to_string(&contact_file).expect("read contact commands");
        for name in [
            "remote_im_get_contact_conversation_messages",
            "remote_im_get_contact_conversation_block_page",
        ] {
            let contact_start = contact_content
                .find(&format!("async fn {name}("))
                .unwrap_or_else(|| panic!("{name} 应为 async fn"));
            let contact_next_fn = contact_content[contact_start..]
                .find("\n}\n\n#[tauri::command]")
                .map(|offset| contact_start + offset + 3)
                .unwrap_or(contact_content.len());
            let contact_section = &contact_content[contact_start..contact_next_fn];
            assert!(
                contact_section.contains("spawn_blocking"),
                "{name} 必须使用 spawn_blocking 移出主线程"
            );
        }
    }

    #[test]
    fn foreground_mark_read_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("foreground_lifecycle.rs");
        let content = std::fs::read_to_string(&file).expect("read foreground lifecycle");
        let mark_read_start = content
            .find("fn mark_conversation_read")
            .expect("mark_conversation_read exists");
        let mark_read_end = content
            .rfind("}\n\n}")
            .expect("foreground lifecycle impl end exists");
        let mark_read_section = &content[mark_read_start..mark_read_end];

        assert!(
            mark_read_section.contains("with_conversation_mutation"),
            "mark_conversation_read 应保留统一会话 mutation 入口"
        );
        assert!(
            !mark_read_section.contains("conversation_mutation_gate("),
            "mark_conversation_read 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
    }

    #[test]
    fn conversation_service_v2_append_writes_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("conversation_service_v2.rs");
        let content = std::fs::read_to_string(&file).expect("read conversation service v2");
        let scopes = [
            (
                "append_message",
                "fn append_message",
                "fn append_message_locked",
            ),
            (
                "append_message_locked",
                "fn append_message_locked",
                "fn append_messages",
            ),
            (
                "append_messages",
                "fn append_messages",
                "fn build_forward_selection_notification_message",
            ),
            (
                "append_user_message",
                "fn append_user_message",
                "fn append_remote_im_user_message",
            ),
            (
                "append_remote_im_user_message",
                "fn append_remote_im_user_message",
                "fn increment_unread_count_if_background",
            ),
        ];

        for (name, start_marker, end_marker) in scopes {
            let start = content.find(start_marker).expect("append scope start exists");
            let end = content.find(end_marker).expect("append scope end exists");
            let section = &content[start..end];
            assert!(
                !section.contains("conversation_mutation_gate("),
                "{name} 必须走统一会话 mutation 入口，禁止重新裸用 conversation_mutation_gate"
            );
            if name != "append_message_locked" {
                assert!(
                    section.contains("with_conversation_mutation"),
                    "{name} 应保留统一会话 mutation 入口"
                );
            }
        }
    }

    #[test]
    fn conversation_service_v2_core_metadata_writes_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("conversation_service_v2.rs");
        let content = std::fs::read_to_string(&file).expect("read conversation service v2");
        let overwrite_start = content
            .find("fn apply_privileged_snapshot_overwrite")
            .expect("apply_privileged_snapshot_overwrite exists");
        let overwrite_end = content
            .find("fn apply_privileged_snapshot_overwrite_inner")
            .expect("apply_privileged_snapshot_overwrite_inner exists");
        let metadata_start = content
            .find("fn apply_external_metadata_patch")
            .expect("apply_external_metadata_patch exists");
        let metadata_end = content
            .find("fn get_conversation_meta")
            .expect("get_conversation_meta exists");
        let overwrite_section = &content[overwrite_start..overwrite_end];
        let metadata_section = &content[metadata_start..metadata_end];

        assert!(
            overwrite_section.contains("with_conversation_mutation"),
            "apply_privileged_snapshot_overwrite 应保留统一会话 mutation 入口"
        );
        assert!(
            !overwrite_section.contains("conversation_mutation_gate("),
            "apply_privileged_snapshot_overwrite 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
        assert!(
            metadata_section.contains("with_conversation_mutation"),
            "apply_external_metadata_patch 应保留统一会话 mutation 入口"
        );
        assert!(
            !metadata_section.contains("conversation_mutation_gate("),
            "apply_external_metadata_patch 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
    }

    #[test]
    fn metadata_todo_update_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("metadata_mutations.rs");
        let content = std::fs::read_to_string(&file).expect("read metadata mutations");
        let todos_start = content
            .find("fn update_conversation_todos")
            .expect("update_conversation_todos exists");
        let todos_end = content
            .find("fn read_unarchived_conversation_summary")
            .expect("read_unarchived_conversation_summary exists");
        let todos_section = &content[todos_start..todos_end];

        assert!(
            todos_section.contains("with_conversation_mutation"),
            "update_conversation_todos 应保留统一会话 mutation 入口"
        );
        assert!(
            !todos_section.contains("conversation_mutation_gate("),
            "update_conversation_todos 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
    }

    #[test]
    fn history_delete_and_preview_should_not_use_legacy_direct_locks() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("history_mutations.rs");
        let content = std::fs::read_to_string(&file).expect("read history mutations");
        let delete_start = content
            .find("fn delete_conversation")
            .expect("delete_conversation exists");
        let delete_end = content
            .find("fn rewind_conversation")
            .expect("rewind_conversation exists");
        let preview_start = content
            .find("fn preview_rewind_conversation")
            .expect("preview_rewind_conversation exists");
        let preview_end = content
            .find("fn branch_conversation_from_selection")
            .expect("branch_conversation_from_selection exists");
        let delete_section = &content[delete_start..delete_end];
        let preview_section = &content[preview_start..preview_end];

        assert!(
            delete_section.contains("with_conversation_mutation"),
            "delete_conversation 应保留统一会话 mutation 入口"
        );
        assert!(
            !delete_section.contains("conversation_mutation_gate("),
            "delete_conversation 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
        assert!(
            preview_section.contains("lock_conversation_with_metrics"),
            "preview_rewind_conversation 应使用带指标的 conversation lock 入口"
        );
        assert!(
            !preview_section.contains(".conversation_lock\n"),
            "preview_rewind_conversation 禁止直接访问 conversation_lock"
        );
    }

    #[test]
    fn archive_mutations_should_use_unified_conversation_mutation_entry() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("archive_lifecycle.rs");
        let content = std::fs::read_to_string(&file).expect("read archive lifecycle");
        let delete_start = content
            .find("fn delete_archive")
            .expect("delete_archive exists");
        let delete_end = content
            .find("fn unarchive_archive")
            .expect("unarchive_archive exists");
        let unarchive_start = delete_end;
        let unarchive_end = content
            .find("fn resolve_archive_request_conversation_by_id")
            .expect("resolve_archive_request_conversation_by_id exists");
        let delete_section = &content[delete_start..delete_end];
        let unarchive_section = &content[unarchive_start..unarchive_end];

        assert!(
            delete_section.contains("with_conversation_mutation"),
            "delete_archive 应保留统一会话 mutation 入口"
        );
        assert!(
            !delete_section.contains("conversation_mutation_gate("),
            "delete_archive 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
        assert!(
            unarchive_section.contains("with_conversation_mutation"),
            "unarchive_archive 应保留统一会话 mutation 入口"
        );
        assert!(
            !unarchive_section.contains("conversation_mutation_gate("),
            "unarchive_archive 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
        let wake_start = content
            .find("fn remote_im_apply_dynamic_wake_compaction")
            .expect("remote_im_apply_dynamic_wake_compaction exists");
        let wake_end = content
            .find("fn persist_compaction_message")
            .expect("persist_compaction_message exists");
        let compaction_start = wake_end;
        let compaction_end = content
            .find("fn import_archives")
            .expect("import_archives exists");
        let wake_section = &content[wake_start..wake_end];
        let compaction_section = &content[compaction_start..compaction_end];

        assert!(
            wake_section.contains("with_conversation_mutation"),
            "remote_im_apply_dynamic_wake_compaction 应保留统一会话 mutation 入口"
        );
        assert!(
            !wake_section.contains("conversation_mutation_gate("),
            "remote_im_apply_dynamic_wake_compaction 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
        assert!(
            compaction_section.contains("with_conversation_mutation"),
            "persist_compaction_message 应保留统一会话 mutation 入口"
        );
        assert!(
            !compaction_section.contains("conversation_mutation_gate("),
            "persist_compaction_message 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
        let archive_start = content
            .find("fn archive_conversation")
            .expect("archive_conversation exists");
        let archive_end = content
            .rfind("}\n\n}")
            .expect("archive lifecycle impl end exists");
        let archive_section = &content[archive_start..archive_end];

        assert!(
            archive_section.contains("with_conversation_mutation"),
            "archive_conversation 应保留统一会话 mutation 入口"
        );
        assert!(
            !archive_section.contains("conversation_mutation_gate("),
            "archive_conversation 必须走 with_conversation_mutation，禁止重新裸用 conversation_mutation_gate"
        );
    }

    #[test]
    fn archive_lifecycle_should_not_use_direct_conversation_lock() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("archive_lifecycle.rs");
        let content = std::fs::read_to_string(&file).expect("read archive lifecycle");

        assert!(
            content.contains("lock_conversation_with_metrics"),
            "archive_lifecycle legacy 只读/多会话路径应使用带指标的 conversation lock 入口"
        );
        assert!(
            !content.contains("conversation_lock.lock()"),
            "archive_lifecycle 禁止直接访问 conversation_lock.lock()"
        );
    }

    #[test]
    fn conversation_reads_snapshot_should_use_metric_lock() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("chat")
            .join("conversation_service")
            .join("conversation_reads.rs");
        let content = std::fs::read_to_string(&file).expect("read conversation reads");
        let snapshot_start = content
            .find("fn get_chat_snapshot")
            .expect("get_chat_snapshot exists");
        let snapshot_end = content
            .find("fn get_conversation_recent_messages")
            .expect("get_conversation_recent_messages exists");
        let snapshot_section = &content[snapshot_start..snapshot_end];

        assert!(
            snapshot_section.contains("lock_conversation_with_metrics"),
            "get_chat_snapshot 应使用带指标的 conversation lock 入口"
        );
        assert!(
            !snapshot_section.contains("conversation_lock.lock()"),
            "get_chat_snapshot 禁止直接访问 conversation_lock.lock()"
        );
    }

    #[test]
    fn conversation_mutation_gate_should_wait_without_hard_timeout() {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features")
            .join("core")
            .join("domain")
            .join("runtime_lock.rs");
        let content = std::fs::read_to_string(&file).expect("read runtime lock");
        let mutation_gate_start = content
            .find("impl ConversationMutationGate")
            .expect("ConversationMutationGate impl exists");
        let mutation_gate_end = content
            .find("struct TimedConversationMutationGuard")
            .expect("TimedConversationMutationGuard exists");
        let mutation_gate_section = &content[mutation_gate_start..mutation_gate_end];

        assert!(
            !mutation_gate_section.contains("CONVERSATION_LOCK_MAX_WAIT_MS"),
            "conversation mutation gate 必须保持串行等待语义，禁止固定等待超时失败"
        );
        assert!(
            !mutation_gate_section.contains("会话写入门等待超时"),
            "conversation mutation gate 禁止把等待变成用户可见超时错误"
        );
    }

    #[test]
    fn conversation_lock_should_only_be_used_by_service_or_explicit_legacy_exception() {
        let features_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("features");
        let allowed_main = features_root
            .join("chat")
            .join("conversation_service")
            .join("conversation_service_v2.rs");
        let allowed_remote_im_sessions = features_root
            .join("chat")
            .join("conversation_service")
            .join("remote_im_sessions.rs");
        let allowed_archive_lifecycle = features_root
            .join("chat")
            .join("conversation_service")
            .join("archive_lifecycle.rs");
        let allowed_history_mutations = features_root
            .join("chat")
            .join("conversation_service")
            .join("history_mutations.rs");
        let allowed_delegate_resolution = features_root
            .join("chat")
            .join("conversation_service")
            .join("delegate_resolution.rs");
        let allowed_conversation_reads = features_root
            .join("chat")
            .join("conversation_service")
            .join("conversation_reads.rs");
        let allowed_context_reads = features_root
            .join("chat")
            .join("conversation_service")
            .join("context_reads.rs");
        let allowed_foreground_lifecycle = features_root
            .join("chat")
            .join("conversation_service")
            .join("foreground_lifecycle.rs");
        let allowed_metadata_mutations = features_root
            .join("chat")
            .join("conversation_service")
            .join("metadata_mutations.rs");
        // 既有归档命令仍持有两处全局锁；保留精确例外，禁止扩散到其他命令文件。
        let allowed_legacy_conversation_archive = features_root
            .join("system")
            .join("commands")
            .join("conversation_archive.rs");
        let self_test_file = features_root.join("chat").join("tests.rs");
        let mut violations = Vec::<String>::new();

        for path in collect_rs_files(&features_root) {
            if path == allowed_main
                || path == allowed_remote_im_sessions
                || path == allowed_archive_lifecycle
                || path == allowed_history_mutations
                || path == allowed_delegate_resolution
                || path == allowed_conversation_reads
                || path == allowed_context_reads
                || path == allowed_foreground_lifecycle
                || path == allowed_metadata_mutations
                || path == allowed_legacy_conversation_archive
                || path == self_test_file
            {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            if content.contains("conversation_lock.lock(")
                || content.contains("conversation_lock\r\n            .lock(")
                || content.contains("conversation_lock\n            .lock(")
            {
                let relative = path
                    .strip_prefix(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
                    .unwrap_or(path.as_path());
                violations.push(relative.display().to_string());
            }
        }

        assert!(
            violations.is_empty(),
            "只有 conversation service 实现文件或显式遗留例外允许直接拿 conversation_lock，违规文件: {:?}",
            violations
        );
    }

    // ========== 计划模式内存态 ==========

    #[test]
    fn plan_mode_set_should_only_write_runtime_slot() {
        let state = test_chat_runtime_state();

        assert!(!get_conversation_plan_mode_enabled(&state, "conversation-plan-a").unwrap());
        set_conversation_plan_mode_enabled(&state, "conversation-plan-a", true).unwrap();
        assert!(get_conversation_plan_mode_enabled(&state, "conversation-plan-a").unwrap());
        set_conversation_plan_mode_enabled(&state, "conversation-plan-a", false).unwrap();
        assert!(!get_conversation_plan_mode_enabled(&state, "conversation-plan-a").unwrap());
    }

    #[test]
    fn plan_mode_without_slot_should_default_false_even_if_meta_has_old_value() {
        let state = test_chat_runtime_state();

        assert!(!get_conversation_plan_mode_enabled(&state, "conversation-plan-b").unwrap());
        set_conversation_plan_mode_enabled(&state, "conversation-plan-b", true).unwrap();
        assert!(get_conversation_plan_mode_enabled(&state, "conversation-plan-b").unwrap());

        // 模拟无 slot（新会话）时不再回退 meta，直接 false
        let other = "conversation-plan-c";
        assert!(!get_conversation_plan_mode_enabled(&state, other).unwrap());
        let slots = lock_conversation_runtime_slots(&state).unwrap();
        assert!(slots.get(other).is_none());
    }

    #[test]
    fn plan_mode_slot_is_independent_per_conversation() {
        let state = test_chat_runtime_state();

        set_conversation_plan_mode_enabled(&state, "conversation-plan-d", true).unwrap();
        set_conversation_plan_mode_enabled(&state, "conversation-plan-e", false).unwrap();

        assert!(get_conversation_plan_mode_enabled(&state, "conversation-plan-d").unwrap());
        assert!(!get_conversation_plan_mode_enabled(&state, "conversation-plan-e").unwrap());
        // 设置一个会话不影响其他会话
        set_conversation_plan_mode_enabled(&state, "conversation-plan-d", false).unwrap();
        assert!(!get_conversation_plan_mode_enabled(&state, "conversation-plan-d").unwrap());
        assert!(!get_conversation_plan_mode_enabled(&state, "conversation-plan-e").unwrap());
    }

    #[test]
    fn schedule_run_from_persisted_conversation_should_fold_tool_history_into_structural_run() {
        let mut conversation = test_chat_conversation("delegate-fold-test", "archived", "2026-08-28T13:00:00Z");
        conversation.conversation_kind = CONVERSATION_KIND_DELEGATE.to_string();
        conversation.root_conversation_id = Some("root-conversation".to_string());
        conversation.delegate_id = Some("delegate-fold-test".to_string());
        conversation.messages.push(test_text_message("user", "执行委托任务", "2026-08-28T13:00:00Z"));
        let mut assistant = test_text_message("assistant", "委托完成。", "2026-08-28T13:29:37Z");
        assistant.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": null,
                "reasoning_content": "先看目录",
                "tool_calls": [
                    { "id": "call-a", "call_id": "call-a", "type": "function", "function": { "name": "exec", "arguments": "{\"command\":\"ls\"}" } },
                    { "id": "call-b", "call_id": "call-b", "type": "function", "function": { "name": "read_file", "arguments": "{\"path\":\"a.rs\"}" } }
                ]
            }),
            serde_json::json!({ "role": "tool", "tool_call_id": "call-a", "content": "file-a" }),
            serde_json::json!({ "role": "tool", "tool_call_id": "call-b", "content": "file-b" }),
        ]);
        assistant.provider_meta = Some(serde_json::json!({ "providerPromptTokens": 1234_u64 }));
        conversation.messages.push(assistant);

        let run = schedule_run_from_persisted_conversation(&conversation).expect("structural run");

        assert_eq!(run.run_id, "persisted-delegate-fold-test");
        assert_eq!(run.delegate_id.as_deref(), Some("delegate-fold-test"));
        assert_eq!(run.status, "success");
        assert_eq!(run.request_count, 1);
        assert_eq!(run.tool_call_count, 2);
        assert_eq!(run.last_tool_name.as_deref(), Some("read_file"));
        // 消息级锚点差：13:00:00Z → 13:29:37Z = 1777 秒
        assert_eq!(run.elapsed_ms, 1_777_000);

        let phases: Vec<&str> = run.events.iter().map(|event| event.phase.as_str()).collect();
        assert_eq!(phases, vec!["dispatch_start", "model_round_start", "model_round_end", "tool_call", "tool_call", "tool_result", "tool_result", "dispatch_end"]);
        assert!(run.events.iter().all(|event| event.elapsed_ms == 0));
        assert_eq!(run.events[0].detail["textPreview"], "执行委托任务");
        assert_eq!(run.events[2].detail["toolCallCount"], 2);
        assert_eq!(run.events[2].detail["reasoningPreview"], "先看目录");
        assert_eq!(run.events[3].detail["toolName"], "exec");
        assert_eq!(run.events[3].detail["argPreview"], "{\"command\":\"ls\"}");
        // tool_result 按 tool_call_id 配对回工具名
        assert_eq!(run.events[5].detail["toolName"], "exec");
        assert_eq!(run.events[6].detail["toolName"], "read_file");
        assert_eq!(run.events[7].detail["textPreview"], "委托完成。");
        assert_eq!(run.events[7].detail["usage"]["providerPromptTokens"], 1234);
    }

    #[test]
    fn schedule_run_from_persisted_conversation_should_return_none_without_events() {
        let conversation = test_chat_conversation("delegate-empty-test", "archived", "2026-08-28T13:29:37Z");
        assert!(schedule_run_from_persisted_conversation(&conversation).is_none());
    }

    #[test]
    fn list_schedule_runs_should_stay_empty_when_memory_and_disk_have_no_delegate_events() {
        let state = test_chat_runtime_state();
        // delegate- 键：内存与磁盘皆无事件，返回空且不报错
        let runs = schedule_event_list_runs_inner(&state, "delegate-not-exist").expect("list runs");
        assert!(runs.is_empty());
        // 非 delegate- 前缀键不触发读盘回退
        let plain = schedule_event_list_runs_inner(&state, "plain-conversation").expect("list plain runs");
        assert!(plain.is_empty());
    }
    fn test_skill_summary(name: &str, description: &str, content: &str) -> SkillSummaryItem {
        SkillSummaryItem {
            name: name.to_string(),
            description: description.to_string(),
            content: content.to_string(),
            path: format!("/skills/{name}/SKILL.md"),
            additional_files: Vec::new(),
            is_builtin: true,
            enabled: true,
        }
    }

    fn test_agent_with_skill_lists(
        resident: Vec<&str>,
        optional: Vec<&str>,
        permission_mode: Option<&str>,
        permission_skills: Vec<&str>,
    ) -> AgentProfile {
        let mut agent = default_agent();
        agent.resident_skill_names = resident.into_iter().map(|value| value.to_string()).collect();
        agent.optional_skill_names = optional.into_iter().map(|value| value.to_string()).collect();
        if let Some(mode) = permission_mode {
            agent.permission_control = AgentPermissionControl {
                enabled: true,
                mode: mode.to_string(),
                builtin_tool_names: Vec::new(),
                skill_names: permission_skills
                    .into_iter()
                    .map(|value| value.to_string())
                    .collect(),
                mcp_tool_names: Vec::new(),
            };
        }
        agent
    }

    #[test]
    fn resident_skill_should_inject_fulltext_and_skip_unavailable() {
        let state = test_chat_runtime_state();
        let scope_key = hidden_skill_cache_scope_key(&state);
        hidden_skill_summaries_cache()
            .lock()
            .expect("lock skill cache")
            .insert(
                scope_key,
                vec![
                    test_skill_summary("leader", "统筹", "正文：先给结论，再给依据。"),
                    test_skill_summary(
                        "assistant-space-guide",
                        "助理空间",
                        "空间正文",
                    ),
                ],
            );

        // 常驻 skill 正文全文注入；清单里没有的名字直接跳过，不报错。
        let agent = test_agent_with_skill_lists(vec!["leader", "not-installed"], vec![], None, vec![]);
        let block = build_resident_skill_fulltext_block(&state, &agent);
        assert!(block.contains("正文：先给结论，再给依据。"), "got: {block}");
        assert!(block.contains("你的常驻技能"), "got: {block}");
        assert!(!block.contains("not-installed"), "got: {block}");

        // 可选 skill 只给引用：名字 + 描述 + 路径，不注入正文。
        let agent =
            test_agent_with_skill_lists(vec![], vec!["assistant-space-guide"], None, vec![]);
        let block = build_optional_skill_reference_block(&state, &agent);
        assert!(block.contains("/skills/assistant-space-guide/SKILL.md"), "got: {block}");
        assert!(!block.contains("空间正文"), "got: {block}");

        // 权限白名单未放行的 skill 不注入。
        let agent = test_agent_with_skill_lists(
            vec!["leader"],
            vec![],
            Some("whitelist"),
            vec!["other-skill"],
        );
        assert!(build_resident_skill_fulltext_block(&state, &agent).is_empty());
    }
