    #[test]
    fn remote_im_upsert_contact_for_inbound_should_keep_new_contact_communication_disabled() {
        let state = remote_im_test_state();
        let input = RemoteImEnqueueInput {
            channel_id: "c1".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            im_name: "qq".to_string(),
            remote_contact_type: "group".to_string(),
            remote_contact_id: "g1".to_string(),
            remote_contact_name: Some("测试群".to_string()),
            sender_id: "u1".to_string(),
            sender_name: "张三".to_string(),
            sender_avatar_url: None,
            platform_message_id: Some("m1".to_string()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            session: SessionSelector {
                api_config_id: None,
                agent_id: "agent".to_string(),
                conversation_id: Some("conv-1".to_string()),
            },
            payload: ChatInputPayload {
                text: Some("hello".to_string()),
                display_text: None,
                parts: None,
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: None,
            },
        };
        let now = now_iso();
        let contact_id = remote_im_upsert_contact_for_inbound(&state, &input, &now)
            .expect("upsert contact");
        assert_eq!(state_service_list_remote_im_contacts(&state, None)
            .expect("list contacts").len(), 1);
        let contact = state_service_get_remote_im_contact(&state, &contact_id)
            .expect("read contact")
            .expect("contact exists");
        assert!(!contact.allow_send);
        assert!(!contact.allow_receive);
        assert_eq!(contact.activation_mode, "never");
        assert!(contact.activation_keywords.is_empty());
        assert_eq!(contact.activation_cooldown_seconds, 0);
        assert_eq!(contact.response_strategy, "smart_judge");

        // 第二次入队应复用同一联系人
        let now2 = now_iso();
        let contact_id_2 = remote_im_upsert_contact_for_inbound(&state, &input, &now2)
            .expect("upsert contact again");
        assert_eq!(contact_id, contact_id_2);
        assert_eq!(state_service_list_remote_im_contacts(&state, None)
            .expect("list contacts").len(), 1);
    }

    #[test]
    fn remote_im_upsert_contact_for_inbound_weixin_defaults_allow_send_files() {
        // 微信渠道为私聊场景（bot 为本人扫码授权账号），新建联系人默认允许发送文件
        let state = remote_im_test_state();
        let input = RemoteImEnqueueInput {
            channel_id: "wx-1".to_string(),
            platform: RemoteImPlatform::WeixinOc,
            im_name: "weixin".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: "wxid_user".to_string(),
            remote_contact_name: Some("微信好友".to_string()),
            sender_id: "wxid_user".to_string(),
            sender_name: "微信好友".to_string(),
            sender_avatar_url: None,
            platform_message_id: Some("m1".to_string()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            session: SessionSelector {
                api_config_id: None,
                agent_id: "agent".to_string(),
                conversation_id: Some("conv-1".to_string()),
            },
            payload: ChatInputPayload {
                text: Some("hello".to_string()),
                display_text: None,
                parts: None,
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: None,
            },
        };
        let now = now_iso();
        let contact_id = remote_im_upsert_contact_for_inbound(&state, &input, &now)
            .expect("upsert weixin contact");
        let contact = state_service_get_remote_im_contact(&state, &contact_id)
            .expect("read contact")
            .expect("contact exists");
        assert!(contact.allow_send_files, "微信渠道新建联系人应默认允许发送文件");

        // 非微信渠道仍保持默认关闭
        let state_qq = remote_im_test_state();
        let input_qq = RemoteImEnqueueInput {
            platform: RemoteImPlatform::OnebotV11,
            ..input
        };
        let now_qq = now_iso();
        let contact_id_qq = remote_im_upsert_contact_for_inbound(&state_qq, &input_qq, &now_qq)
            .expect("upsert qq contact");
        let contact_qq = state_service_get_remote_im_contact(&state_qq, &contact_id_qq)
            .expect("read qq contact")
            .expect("contact exists");
        assert!(!contact_qq.allow_send_files, "非微信渠道应保持默认关闭文件发送");
    }

    #[test]
    fn validate_enqueue_input_should_accept_parts_with_attachment_when_receive_files_enabled() {
        let mut config = AppConfig::default();
        config.remote_im_channels.push(RemoteImChannelConfig {
            id: "wx-ch".to_string(),
            name: "微信渠道".to_string(),
            platform: RemoteImPlatform::WeixinOc,
            enabled: true,
            credentials: serde_json::json!({}),
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            filter_markdown: false,
            allow_send_files: false,
            behavior_settings: RemoteImChannelBehaviorSettings::default(),
        });
        let input = RemoteImEnqueueInput {
            channel_id: "wx-ch".to_string(),
            platform: RemoteImPlatform::WeixinOc,
            im_name: "weixin".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: "wxid_user".to_string(),
            remote_contact_name: Some("用户 (微信)".to_string()),
            sender_id: "wxid_user".to_string(),
            sender_name: "用户".to_string(),
            sender_avatar_url: None,
            platform_message_id: Some("m1".to_string()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            session: SessionSelector {
                api_config_id: None,
                agent_id: "agent".to_string(),
                conversation_id: None,
            },
            payload: ChatInputPayload {
                text: None,
                display_text: None,
                parts: Some(vec![ChatIngressPart::Attachment {
                    path: None,
                    bytes_base64: Some("aGVsbG8=".to_string()),
                    mime: "image/jpeg".to_string(),
                    name: "image.jpg".to_string(),
                }]),
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: None,
            },
        };
        let validated = validate_enqueue_input(&input, &config)
            .expect("纯图片消息在开启 receive_files 时必须验证通过");
        assert_eq!(validated.parts_image_count, 1);
        assert_eq!(validated.parts_audio_count, 0);
        assert_eq!(validated.parts_attachment_count, 0);
    }

    #[test]
    fn default_group_response_guidance_should_prefer_silence_and_direct_relation() {
        assert_eq!(default_remote_im_contact_response_strategy(), "smart_judge");

        let guidance = default_remote_im_contact_response_guidance();
        assert!(guidance.contains("默认保持沉默"));
        assert!(guidance.contains("默认应为 `false`"));
        assert!(guidance.contains("不让群友觉得助理话很多"));
        assert!(guidance.contains("新的必要价值"));
        assert!(guidance.contains("明确叫到助理的昵称"));
        assert!(guidance.contains("追问助理刚才的回答"));
        assert!(guidance.contains("不论首次还是后续，一律不回答"));
        assert!(guidance.contains("实质相近的问题"));
        assert!(!guidance.contains("有人首次点评、质疑、纠正、评价或反馈"));
        assert!(guidance.contains("除此以外一律不回答"));
    }

    #[test]
    fn assistant_work_ledger_should_only_project_active_remote_reply_delegates() {
        let state = remote_im_test_state();
        let message = |id: &str, text: &str| ChatMessage {
            id: id.to_string(),
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
        let active_trigger = message("trigger-active", "查看当前图片内容");
        let completed_trigger = message("trigger-completed", "历史报价是多少");
        let other_contact_trigger = message("trigger-other", "其他联系人任务");
        let mut runtimes = lock_remote_im_reply_delegate_runtimes(&state)
            .expect("lock delegate runtimes");
        for (delegate_id, contact_id, trigger, terminal) in [
            ("delegate-active", "contact-a", active_trigger, false),
            ("delegate-completed", "contact-a", completed_trigger, true),
            ("delegate-other", "contact-b", other_contact_trigger, false),
        ] {
            runtimes.insert(
                delegate_id.to_string(),
                RemoteImReplyDelegateRuntime {
                    delegate_id: delegate_id.to_string(),
                    contact_id: contact_id.to_string(),
                    conversation_id: "conversation-a".to_string(),
                    trigger_message_id: trigger.id.clone(),
                    started_at: now_iso(),
                    prompt_snapshot_messages: vec![trigger],
                    guidance_messages: std::collections::VecDeque::new(),
                    consumed_guidance_messages: Vec::new(),
                    cancelled: false,
                    terminal,
                    session_agent_id: "agent-a".to_string(),
                    inspection_generation: None,
                    group_reply_focus: false,
                    group_reply_max_chars: None,
                },
            );
        }
        drop(runtimes);

        let ledger = build_remote_im_assistant_work_ledger(
            &state,
            "contact-a",
            "conversation-a",
        )
        .expect("build ledger");
        assert!(ledger.contains("[运行中]"));
        assert!(ledger.contains("delegate-active"));
        assert!(ledger.contains("查看当前图片内容"));
        assert!(!ledger.contains("delegate-completed"));
        assert!(!ledger.contains("历史报价是多少"));
        assert!(!ledger.contains("delegate-other"));
    }

    #[test]
    fn create_pending_event_should_guide_only_activated_private_messages() {
        let sender = |remote_contact_type: &str| RemoteImMessageSource {
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            im_name: "QQ".to_string(),
            remote_contact_type: remote_contact_type.to_string(),
            remote_contact_id: "contact-a".to_string(),
            remote_contact_name: "联系人".to_string(),
            sender_id: "sender-a".to_string(),
            sender_name: "联系人".to_string(),
            sender_avatar_url: None,
            platform_message_id: None,
        };
        let session_info = || ChatSessionInfo {
            agent_id: DEFAULT_AGENT_ID.to_string(),
        };

        let private_event = create_pending_event(
            "event-private".to_string(),
            "conversation-a".to_string(),
            Vec::new(),
            true,
            session_info(),
            sender("private"),
        );
        let inactive_private_event = create_pending_event(
            "event-private-inactive".to_string(),
            "conversation-a".to_string(),
            Vec::new(),
            false,
            session_info(),
            sender("private"),
        );
        let group_event = create_pending_event(
            "event-group".to_string(),
            "conversation-a".to_string(),
            Vec::new(),
            true,
            session_info(),
            sender("group"),
        );

        assert_eq!(private_event.queue_mode, ChatQueueMode::Guided);
        assert_eq!(inactive_private_event.queue_mode, ChatQueueMode::Normal);
        assert_eq!(group_event.queue_mode, ChatQueueMode::Normal);
    }

    #[test]
    fn resolve_contact_agent_id_should_fall_back_to_assistant_persona_when_binding_is_gone() {
        let mut api = ApiConfig::default();
        api.id = "api-a".to_string();
        api.enable_text = true;
        api.model = "gpt-4o-mini".to_string();
        let config = AppConfig {
            api_configs: vec![api],
            expert_api_config_id: "api-a".to_string(),
            ..AppConfig::default()
        };

        let state = remote_im_test_state();
        write_config(&state.config_path, &config).expect("write config");
        state_write_agents_cached(
            &state,
            &[remote_im_test_agent(DEFAULT_AGENT_ID, "主助理"), default_user_persona()],
        )
        .expect("write agents");

        // 显式绑定的人格存在时原样使用
        let explicit = resolve_contact_agent_id(&state, Some(DEFAULT_AGENT_ID))
            .expect("explicit agent");
        assert_eq!(explicit, DEFAULT_AGENT_ID);

        // 绑定的人格已被删除时回落到助理人格，而不是让这条路由断掉
        let default_agent_id = state_service_get_assistant_agent_id(&state)
            .expect("runtime default persona");
        let fell_back = resolve_contact_agent_id(&state, Some("agent-removed"))
            .expect("missing agent should fall back");
        assert_eq!(fell_back, default_agent_id);

        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn remote_im_filter_channel_logs_for_contact_should_only_keep_matching_contact() {
        let logs = vec![
            ChannelLogEntry {
                timestamp: chrono::Utc::now(),
                level: "info".to_string(),
                message: "[联系人消息] 收到: contact=甲, preview=hello".to_string(),
                contact_record_id: Some("contact-a".to_string()),
            },
            ChannelLogEntry {
                timestamp: chrono::Utc::now(),
                level: "info".to_string(),
                message: "[联系人消息] 收到: contact=甲乙, preview=world".to_string(),
                contact_record_id: Some("contact-b".to_string()),
            },
            ChannelLogEntry {
                timestamp: chrono::Utc::now(),
                level: "info".to_string(),
                message: "事件消费器已启动".to_string(),
                contact_record_id: None,
            },
        ];

        let filtered = remote_im_filter_channel_logs_for_contact(logs, "contact-a");

        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].message.contains("contact=甲"));
    }

    #[test]
    fn resolve_conversation_id_should_route_remote_im_to_contact_conversation() {
        let state = remote_im_test_state();
        state_service_set_main_conversation_id(&state, Some("conversation-main"))
            .expect("write main conversation id");
        let conversations = vec![
            Conversation {
                id: "conversation-main".to_string(),
                title: "main".to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                bound_conversation_id: None,
                parent_conversation_id: None,
                child_conversation_ids: Vec::new(),
                fork_message_cursor: None,
                unread_count: 0,
                conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: now_iso(),
                updated_at: now_iso(),
                last_user_at: None,
                last_assistant_at: None,
                status: "inactive".to_string(),
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
            },
            Conversation {
                id: "conversation-sub".to_string(),
                title: "sub".to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                bound_conversation_id: None,
                parent_conversation_id: None,
                child_conversation_ids: Vec::new(),
                fork_message_cursor: None,
                unread_count: 0,
                conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: now_iso(),
                updated_at: now_iso(),
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
            },
        ];
        let input = RemoteImEnqueueInput {
            channel_id: "c1".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            im_name: "qq".to_string(),
            remote_contact_type: "group".to_string(),
            remote_contact_id: "g1".to_string(),
            remote_contact_name: Some("测试群".to_string()),
            sender_id: "u1".to_string(),
            sender_name: "张三".to_string(),
            sender_avatar_url: None,
            platform_message_id: Some("m1".to_string()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            session: SessionSelector {
                api_config_id: None,
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_id: Some("conversation-sub".to_string()),
            },
            payload: ChatInputPayload {
                text: Some("hello".to_string()),
                display_text: None,
                parts: None,
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: None,
            },
        };

        let mut contact = RemoteImContact {
            id: "contact-1".to_string(),
            channel_id: input.channel_id.clone(),
            platform: input.platform,
            remote_contact_type: input.remote_contact_type.clone(),
            remote_contact_id: input.remote_contact_id.clone(),
            remote_contact_name: input.remote_contact_name.clone().unwrap_or_default(),
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
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        };
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        for conversation in &conversations {
            state_write_conversation_cached(&state, conversation).expect("write conversation");
        }

        let (_, conversation_id) =
            resolve_contact_session_target(&state, &mut contact)
                .expect("resolve route");

        assert_ne!(conversation_id, "conversation-main");
        assert_eq!(contact.bound_conversation_id.as_deref(), Some(conversation_id.as_str()));
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn remote_im_should_still_route_to_contact_conversation_after_user_switches() {
        let state = remote_im_test_state();
        state_service_set_main_conversation_id(&state, Some("conversation-main"))
            .expect("write main conversation id");
        let conversations = vec![
            Conversation {
                id: "conversation-main".to_string(),
                title: "main".to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                bound_conversation_id: None,
                parent_conversation_id: None,
                child_conversation_ids: Vec::new(),
                fork_message_cursor: None,
                unread_count: 0,
                conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: now_iso(),
                updated_at: now_iso(),
                last_user_at: None,
                last_assistant_at: None,
                status: "inactive".to_string(),
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
            },
            Conversation {
                id: "conversation-sub".to_string(),
                title: "sub".to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                bound_conversation_id: None,
                parent_conversation_id: None,
                child_conversation_ids: Vec::new(),
                fork_message_cursor: None,
                unread_count: 0,
                conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: now_iso(),
                updated_at: now_iso(),
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
            },
        ];
        let input = RemoteImEnqueueInput {
            channel_id: "c1".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            im_name: "qq".to_string(),
            remote_contact_type: "group".to_string(),
            remote_contact_id: "g1".to_string(),
            remote_contact_name: Some("测试群".to_string()),
            sender_id: "u1".to_string(),
            sender_name: "张三".to_string(),
            sender_avatar_url: None,
            platform_message_id: Some("m1".to_string()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            session: SessionSelector {
                api_config_id: None,
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_id: Some("conversation-sub".to_string()),
            },
            payload: ChatInputPayload {
                text: Some("hello".to_string()),
                display_text: None,
                parts: None,
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: None,
            },
        };

        let mut contact = RemoteImContact {
            id: "contact-1".to_string(),
            channel_id: input.channel_id.clone(),
            platform: input.platform,
            remote_contact_type: input.remote_contact_type.clone(),
            remote_contact_id: input.remote_contact_id.clone(),
            remote_contact_name: input.remote_contact_name.clone().unwrap_or_default(),
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
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            onebot_group_members: Vec::new(),
            shell_workspaces: Vec::new(),
        };
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        for conversation in &conversations {
            state_write_conversation_cached(&state, conversation).expect("write conversation");
        }

        let (_, conversation_id) =
            resolve_contact_session_target(&state, &mut contact)
                .expect("resolve route");

        assert_ne!(conversation_id, "conversation-main");
        assert_eq!(contact.bound_conversation_id.as_deref(), Some(conversation_id.as_str()));
        assert_eq!(
            state_service_get_main_conversation_id(&state)
                .expect("read main conversation id")
                .as_deref(),
            Some("conversation-main")
        );
        assert_eq!(
            conversations
                .iter()
                .find(|item| item.id == "conversation-sub")
                .map(|item| item.status.as_str()),
            Some("active")
        );
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn conversation_has_remote_im_platform_message_should_match_snake_case_origin_meta() {
        let conversation = Conversation {
            id: "conv-1".to_string(),
            title: "联系人".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: "inactive".to_string(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: vec![ChatMessage {
                id: "msg-1".to_string(),
                role: "user".to_string(),
                created_at: now_iso(),
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "hello".to_string(),
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "origin": {
                        "kind": "remote_im",
                        "channel_id": "c1",
                        "contact_type": "private",
                        "contact_id": "u1",
                        "platform_message_id": "m1"
                    }
                })),
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
            }],
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

        assert!(conversation.messages.iter().any(|message| {
            message_has_remote_im_platform_message(message, "c1", "private", "u1", "m1")
        }));
        assert!(!conversation.messages.iter().any(|message| {
            message_has_remote_im_platform_message(message, "c1", "private", "u1", "m2")
        }));
    }

    #[test]
    fn conversation_has_remote_im_platform_message_should_ignore_legacy_origin_meta() {
        let conversation = Conversation {
            id: "conv-1".to_string(),
            title: "联系人".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: "inactive".to_string(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: vec![ChatMessage {
                id: "msg-1".to_string(),
                role: "user".to_string(),
                created_at: now_iso(),
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "hello".to_string(),
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: Some(serde_json::json!({
                    "origin": {
                        "kind": "remote_im",
                        "channelId": "c1",
                        "remoteContactType": "private",
                        "remoteContactId": "u1",
                        "platformMessageId": "m1"
                    }
                })),
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
            }],
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

        assert!(!conversation.messages.iter().any(|message| {
            message_has_remote_im_platform_message(message, "c1", "private", "u1", "m1")
        }));
    }

    #[test]
    fn remote_im_set_sender_origin_meta_should_write_snake_case_remote_identity() {
        let input = RemoteImEnqueueInput {
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            im_name: "qq".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: "remote-user-1".to_string(),
            remote_contact_name: Some("张三".to_string()),
            sender_id: "remote-user-1".to_string(),
            sender_name: "张三".to_string(),
            sender_avatar_url: Some("https://example.com/avatar.png".to_string()),
            platform_message_id: Some("msg-1".to_string()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            session: SessionSelector {
                api_config_id: None,
                agent_id: String::new(),
                conversation_id: None,
            },
            payload: ChatInputPayload {
                text: Some("hello".to_string()),
                display_text: None,
                parts: None,
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: None,
            },
        };

        let value = remote_im_set_sender_origin_meta(&input, "conversation-1", "record-1");
        let origin = value.get("origin").expect("origin");

        assert_eq!(origin.get("channel_id").and_then(Value::as_str), Some("channel-a"));
        assert_eq!(origin.get("contact_id").and_then(Value::as_str), Some("remote-user-1"));
        assert_eq!(origin.get("contact_record_id").and_then(Value::as_str), Some("record-1"));
        assert_eq!(origin.get("sender_name").and_then(Value::as_str), Some("张三"));
        assert_eq!(origin.get("platform_message_id").and_then(Value::as_str), Some("msg-1"));
        assert!(origin.get("channelId").is_none());
        assert!(origin.get("contactId").is_none());
    }

    #[test]
    fn weixin_oc_parse_media_aes_key_should_accept_base64_encoded_hex_text() {
        let encoded = B64.encode("00112233445566778899aabbccddeeff");
        let decoded = weixin_oc_parse_media_aes_key(&encoded).expect("decode aes key");
        assert_eq!(
            decoded,
            vec![
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb,
                0xcc, 0xdd, 0xee, 0xff,
            ]
        );
    }

    #[test]
    fn weixin_oc_decrypt_media_ecb_should_remove_pkcs7_padding() {
        use aes::cipher::{BlockCipherEncrypt, KeyInit};

        let key = vec![
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
            0xdd, 0xee, 0xff,
        ];
        let plain = b"wechat-image-bytes".to_vec();
        let pad_len = 16 - (plain.len() % 16);
        let mut padded = plain.clone();
        padded.extend(std::iter::repeat(pad_len as u8).take(pad_len));

        let cipher = aes::Aes128::new_from_slice(&key).expect("create cipher");
        let mut encrypted = padded.clone();
        for chunk in encrypted.chunks_exact_mut(16) {
            let block = <&mut aes::Block>::try_from(chunk).expect("16-byte AES block");
            cipher.encrypt_block(block);
        }

        let decrypted = weixin_oc_decrypt_media_ecb(&encrypted, &key).expect("decrypt");
        assert_eq!(decrypted, plain);
    }

    #[test]
    fn onebot_cq_string_should_extract_group_image_media_refs() {
        let (text, media_refs, embedded_refs) = parse_onebot_cq_string(
            "看看这个[CQ:image,file=https://example.com/a.png,file_id=img-1]图片",
        );
        assert_eq!(text, "看看这个图片");
        assert_eq!(media_refs.len(), 1);
        assert!(embedded_refs.is_empty());
        assert!(matches!(media_refs[0].kind, OnebotInboundMediaKind::Image));
        assert_eq!(media_refs[0].file_ref, "https://example.com/a.png");
        assert_eq!(media_refs[0].file_id.as_deref(), Some("img-1"));
    }

    #[test]
    fn onebot_ordered_segments_should_keep_text_media_text_interleaving() {
        let payload = serde_json::json!([
            { "type": "text", "data": { "text": "文字A" } },
            { "type": "image", "data": { "file": "base64://YWJj", "file_id": "img-1" } },
            { "type": "text", "data": { "text": "文字B" } }
        ]);

        let parsed = parse_onebot_message_array_detail(payload.as_array().expect("array"));

        assert_eq!(parsed.ordered_segments.len(), 3);
        assert!(matches!(&parsed.ordered_segments[0], OnebotParsedSegment::Text(text) if text == "文字A"));
        assert!(matches!(&parsed.ordered_segments[1], OnebotParsedSegment::Media(media)
            if media.file_id.as_deref() == Some("img-1")));
        assert!(matches!(&parsed.ordered_segments[2], OnebotParsedSegment::Text(text) if text == "文字B"));
    }

    #[test]
    fn extract_message_content_should_keep_media_when_message_is_cq_string() {
        let event = serde_json::json!({
            "message": "你好[CQ:image,file=base64://YWJj,file_id=image-2]"
        });
        let (text, media_refs, embedded_refs) = extract_message_content(&event);
        assert_eq!(text, "你好");
        assert_eq!(media_refs.len(), 1);
        assert!(embedded_refs.is_empty());
        assert!(matches!(media_refs[0].kind, OnebotInboundMediaKind::Image));
        assert_eq!(media_refs[0].file_ref, "base64://YWJj");
        assert_eq!(media_refs[0].file_id.as_deref(), Some("image-2"));
    }

    #[test]
    fn onebot_message_array_should_extract_forward_and_reply_refs() {
        let payload = serde_json::json!([
            { "type": "reply", "data": { "id": "123" } },
            { "type": "forward", "data": { "id": "456" } }
        ]);
        let (text, media_refs, embedded_refs) =
            parse_onebot_message_array(payload.as_array().expect("array"));
        assert!(text.is_empty());
        assert!(media_refs.is_empty());
        assert_eq!(embedded_refs.len(), 2);
        assert!(matches!(embedded_refs[0].kind, OnebotEmbeddedRefKind::Reply));
        assert_eq!(embedded_refs[0].id, "123");
        assert!(matches!(embedded_refs[1].kind, OnebotEmbeddedRefKind::Forward));
        assert_eq!(embedded_refs[1].id, "456");
    }

    #[test]
    fn onebot_message_array_should_extract_record_and_video_media_refs() {
        let payload = serde_json::json!([
            { "type": "record", "data": { "url": "https://example.com/a.silk", "file_id": "voice-1" } },
            { "type": "video", "data": { "file": "https://example.com/a.mp4", "file_id": "video-1" } }
        ]);
        let (text, media_refs, embedded_refs) =
            parse_onebot_message_array(payload.as_array().expect("array"));

        assert!(text.is_empty());
        assert!(embedded_refs.is_empty());
        assert_eq!(media_refs.len(), 2);
        assert!(matches!(media_refs[0].kind, OnebotInboundMediaKind::File));
        assert_eq!(media_refs[0].file_ref, "https://example.com/a.silk");
        assert_eq!(media_refs[0].file_id.as_deref(), Some("voice-1"));
        assert_eq!(media_refs[0].mime_hint.as_deref(), Some("audio/x-silk"));
        assert!(matches!(media_refs[1].kind, OnebotInboundMediaKind::File));
        assert_eq!(media_refs[1].file_ref, "https://example.com/a.mp4");
        assert_eq!(media_refs[1].file_id.as_deref(), Some("video-1"));
        assert_eq!(media_refs[1].mime_hint.as_deref(), Some("video/mp4"));
    }

    #[test]
    fn onebot_cq_info_segments_should_render_markdown_quote_blocks() {
        let (text, media_refs, embedded_refs) =
            parse_onebot_cq_string("[CQ:face,id=123][CQ:share,title=文档,url=https://example.com]");

        assert!(media_refs.is_empty());
        assert!(embedded_refs.is_empty());
        assert!(text.contains("> **QQ 表情**"));
        assert!(text.contains("> id: 123"));
        assert!(text.contains("> **链接分享**"));
        assert!(text.contains("> title: 文档"));
        assert!(text.contains("> url: https://example.com"));
    }

    #[test]
    fn onebot_reply_sender_should_use_card_then_nickname_then_id() {
        let with_card = serde_json::json!({
            "sender": {
                "user_id": "10000",
                "nickname": "昵称甲",
                "card": "群名片甲"
            }
        });
        let with_nickname = serde_json::json!({
            "sender": {
                "user_id": "10001",
                "nickname": "昵称乙"
            }
        });
        let with_id = serde_json::json!({
            "sender": {
                "user_id": "10002"
            }
        });

        assert_eq!(
            onebot_resolve_reply_sender_display_name(&with_card).as_deref(),
            Some("群名片甲")
        );
        assert_eq!(
            onebot_resolve_reply_sender_display_name(&with_nickname).as_deref(),
            Some("昵称乙")
        );
        assert_eq!(
            onebot_resolve_reply_sender_display_name(&with_id).as_deref(),
            Some("10002")
        );
    }

    #[test]
    fn onebot_forward_payload_should_prefer_sender_nickname_then_card_then_user_id() {
        let payload = serde_json::json!({
            "messages": [
                {
                    "sender": {
                        "nickname": "昵称甲",
                        "card": "群名片甲",
                        "user_id": "10001"
                    },
                    "message": [{ "type": "text", "data": { "text": "第一条" } }]
                },
                {
                    "sender": {
                        "card": "群名片乙",
                        "user_id": "10002"
                    },
                    "message": [{ "type": "text", "data": { "text": "第二条" } }]
                },
                {
                    "sender": {
                        "user_id": "10003"
                    },
                    "message": [{ "type": "text", "data": { "text": "第三条" } }]
                }
            ]
        });

        let (text, media_refs) = onebot_parse_forward_payload(&payload);

        assert!(media_refs.is_empty());
        assert_eq!(text, "昵称甲：第一条\n群名片乙：第二条\n10003：第三条");
    }

    #[test]
    fn onebot_forward_payload_should_fallback_to_node_name_when_sender_missing() {
        let payload = serde_json::json!({
            "messages": [
                {
                    "data": {
                        "name": "节点名称",
                        "content": [{ "type": "text", "data": { "text": "转发内容" } }]
                    }
                }
            ]
        });

        let (text, media_refs) = onebot_parse_forward_payload(&payload);

        assert!(media_refs.is_empty());
        assert_eq!(text, "节点名称：转发内容");
    }

    fn remote_im_test_state() -> AppState {
        let root = std::env::temp_dir().join(format!("eca-remote-im-test-{}", Uuid::new_v4()));
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
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            terminal_background_shell_tasks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
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
            provider_streaming_disabled_keys: Arc::new(Mutex::new(std::collections::HashMap::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(std::collections::HashSet::new())),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_runtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_semaphore: Arc::new(tokio::sync::Semaphore::new(8)),
            remote_im_channel_state_write_locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn remote_im_test_png(width: u32, height: u32) -> Vec<u8> {
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            width,
            height,
            image::Rgba([12, 34, 56, 255]),
        ));
        let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
        image
            .write_to(&mut cursor, image::ImageFormat::Png)
            .expect("encode png");
        cursor.into_inner()
    }

    #[tokio::test]
    async fn onebot_event_consumer_should_remain_singleton_per_channel() {
        let manager = OnebotV11WsManager::new();
        let state = remote_im_test_state();

        manager
            .start_event_consumer("channel-a".to_string(), state.clone())
            .await
            .expect("start consumer 1");
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(manager.event_consumer_tasks.read().await.len(), 1);
        assert_eq!(manager.event_consumer_stop_senders.read().await.len(), 1);

        manager
            .start_event_consumer("channel-a".to_string(), state)
            .await
            .expect("start consumer 2");
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(manager.event_consumer_tasks.read().await.len(), 1);
        assert_eq!(manager.event_consumer_stop_senders.read().await.len(), 1);

        manager
            .stop_channel("channel-a")
            .await
            .expect("stop channel");
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(manager.event_consumer_tasks.read().await.is_empty());
        assert!(manager.event_consumer_stop_senders.read().await.is_empty());
    }

    #[tokio::test]
    async fn onebot_channel_event_bus_should_exist_before_client_connection() {
        use futures_util::SinkExt as _;

        let manager = OnebotV11WsManager::new();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind temp listener");
        let port = listener.local_addr().expect("local addr").port();
        drop(listener);

        manager
            .start(
                "channel-a".to_string(),
                OnebotV11WsCredentials {
                    ws_host: "127.0.0.1".to_string(),
                    ws_port: port,
                    ws_token: None,
                },
            )
            .await
            .expect("start onebot channel");

        let mut event_rx = manager
            .subscribe_events("channel-a")
            .await
            .expect("event bus should be available before client connects");

        let url = format!("ws://127.0.0.1:{port}");
        let (mut client, _) = tokio_tungstenite::connect_async(url.as_str())
            .await
            .expect("connect client");
        let event = serde_json::json!({
            "post_type": "message",
            "message_type": "private",
            "user_id": 10001,
            "message_id": 42,
            "message": "hello"
        });
        client
            .send(tokio_tungstenite::tungstenite::Message::Text(
                event.to_string().into(),
            ))
            .await
            .expect("send event");

        let received = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("event should arrive")
            .expect("event bus open");
        assert_eq!(received.get("message_id").and_then(Value::as_i64), Some(42));

        manager
            .stop_channel("channel-a")
            .await
            .expect("stop onebot channel");
    }

    #[tokio::test]
    async fn onebot_channel_start_should_be_serialized_per_channel() {
        let manager = OnebotV11WsManager::new();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind temp listener");
        let port = listener.local_addr().expect("local addr").port();
        drop(listener);

        let credentials = OnebotV11WsCredentials {
            ws_host: "127.0.0.1".to_string(),
            ws_port: port,
            ws_token: None,
        };
        let first = manager.start("channel-a".to_string(), credentials.clone());
        let second = manager.start("channel-a".to_string(), credentials);
        let (first_result, second_result) = tokio::join!(first, second);

        first_result.expect("first start should succeed");
        second_result.expect("second start should wait and succeed");
        assert_eq!(manager.channel_tasks.read().await.len(), 1);
        assert_eq!(manager.channel_runtimes.read().await.len(), 1);
        let logs = manager.get_logs("channel-a").await;
        assert_eq!(
            logs.iter()
                .filter(|entry| entry.message.contains("服务器启动，监听"))
                .count(),
            1
        );
        assert!(
            logs.iter()
                .any(|entry| entry.message.contains("跳过重复启动")),
            "second start should reuse the existing listener"
        );

        manager
            .stop_channel("channel-a")
            .await
            .expect("stop onebot channel");
    }

    #[tokio::test]
    async fn onebot_stop_channel_should_cancel_bind_retry_without_waiting_for_lifecycle_lock() {
        let manager = OnebotV11WsManager::new();
        let addr = "127.0.0.1:6199".parse().expect("valid onebot addr");
        let runtime = {
            let _guard = manager.port_service.lifecycle_guard("channel-a").await;
            manager.prepare_start_after_stop_at("channel-a", addr).await
        };
        manager
            .port_service
            .set_status_text("channel-a", Some("binding_retry".to_string()))
            .await;

        tokio::time::timeout(Duration::from_secs(1), manager.stop_channel("channel-a"))
            .await
            .expect("stop should not wait for the bind retry timeout")
            .expect("stop channel");

        assert!(runtime.cancel.is_cancelled());
        assert!(manager.channel_runtimes.read().await.is_empty());
    }

    #[tokio::test]
    async fn onebot_channel_should_replace_existing_connection_on_second_handshake() {
        let manager = OnebotV11WsManager::new();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind temp listener");
        let port = listener.local_addr().expect("local addr").port();
        drop(listener);

        manager
            .start(
                "channel-a".to_string(),
                OnebotV11WsCredentials {
                    ws_host: "127.0.0.1".to_string(),
                    ws_port: port,
                    ws_token: None,
                },
            )
            .await
            .expect("start onebot channel");

        let url = format!("ws://127.0.0.1:{port}");
        let (mut first, _) = tokio_tungstenite::connect_async(url.as_str())
            .await
            .expect("first connection");
        tokio::time::sleep(Duration::from_millis(80)).await;

        let (mut second, _) = tokio_tungstenite::connect_async(url.as_str())
            .await
            .expect("second connection");
        tokio::time::sleep(Duration::from_millis(120)).await;

        let status = manager.get_connection_status("channel-a").await;
        assert!(status.connected);
        let logs = manager.get_logs("channel-a").await;
        assert!(
            logs.iter()
                .any(|entry| entry.message.contains("新连接已接管旧连接")),
            "second connection should replace the old one"
        );
        assert_eq!(manager.connections.read().await.len(), 1);
        assert_eq!(manager.connection_stop_senders.read().await.len(), 1);
        assert_eq!(manager.channel_runtimes.read().await.len(), 1);

        let _ = first.close(None).await;
        let _ = second.close(None).await;
        tokio::time::sleep(Duration::from_millis(80)).await;
        manager
            .stop_channel("channel-a")
            .await
            .expect("stop onebot channel");
        assert!(manager.channel_runtimes.read().await.is_empty());
    }

    #[tokio::test]
    async fn onebot_stop_channel_should_cancel_active_connection_task() {
        let manager = OnebotV11WsManager::new();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind temp listener");
        let port = listener.local_addr().expect("local addr").port();
        drop(listener);

        manager
            .start(
                "channel-a".to_string(),
                OnebotV11WsCredentials {
                    ws_host: "127.0.0.1".to_string(),
                    ws_port: port,
                    ws_token: None,
                },
            )
            .await
            .expect("start onebot channel");

        let url = format!("ws://127.0.0.1:{port}");
        let (_client, _) = tokio_tungstenite::connect_async(url.as_str())
            .await
            .expect("connect client");
        tokio::time::sleep(Duration::from_millis(80)).await;

        assert_eq!(manager.connections.read().await.len(), 1);
        assert_eq!(manager.channel_runtimes.read().await.len(), 1);

        manager
            .stop_channel("channel-a")
            .await
            .expect("stop onebot channel");
        tokio::time::sleep(Duration::from_millis(80)).await;

        assert!(manager.connections.read().await.is_empty());
        assert!(manager.connection_stop_senders.read().await.is_empty());
        assert!(manager.channel_runtimes.read().await.is_empty());
    }

    fn remote_im_test_contact(contact_id: &str, conversation_id: &str) -> RemoteImContact {
        RemoteImContact {
            id: contact_id.to_string(),
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "private".to_string(),
            remote_contact_id: "remote-a".to_string(),
            remote_contact_name: "张三".to_string(),
            avatar_url: String::new(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "keyword".to_string(),
            activation_keywords: vec!["派".to_string()],
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_agent_id: None,
            bound_conversation_id: Some(conversation_id.to_string()),
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
        }
    }

    fn remote_im_test_channel(
        id: &str,
        behavior_settings: RemoteImChannelBehaviorSettings,
    ) -> RemoteImChannelConfig {
        RemoteImChannelConfig {
            id: id.to_string(),
            name: id.to_string(),
            platform: RemoteImPlatform::OnebotV11,
            enabled: true,
            credentials: serde_json::json!({}),
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            filter_markdown: false,
            allow_send_files: false,
            behavior_settings,
        }
    }

    #[test]
    fn remote_image_ingress_should_persist_original_bytes_as_one_canonical_absolute_path() {
        let state = remote_im_test_state();
        let raw = remote_im_test_png(16, 12);
        let contact = remote_im_test_contact("contact-image", "conversation-image");
        let input = RemoteImEnqueueInput {
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            im_name: "QQ".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: "remote-a".to_string(),
            remote_contact_name: Some("张三".to_string()),
            sender_id: "remote-a".to_string(),
            sender_name: "张三".to_string(),
            sender_avatar_url: None,
            platform_message_id: Some("message-image".to_string()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            session: SessionSelector {
                api_config_id: None,
                agent_id: String::new(),
                conversation_id: Some("conversation-image".to_string()),
            },
            payload: ChatInputPayload {
                text: None,
                display_text: None,
                parts: None,
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: Some(serde_json::json!({
                    "attachments": [{ "relativePath": "legacy/duplicate.png" }]
                })),
            },
        };
        let images = vec![BinaryPart {
            mime: "image/png".to_string(),
            bytes_base64: B64.encode(&raw),
            saved_path: None,
        }];

        let message = build_chat_message_from_input(
            &state,
            &input,
            "conversation-image",
            &contact,
            &now_iso(),
            "",
            &images,
            &[],
            &[],
        );

        let MessagePart::Attachment { path, mime, .. } = message.parts.first().expect("attachment") else {
            panic!("expected canonical attachment");
        };
        assert_eq!(mime, "image/png");
        assert!(std::path::Path::new(path).is_absolute());
        assert_eq!(std::fs::read(path).expect("read persisted original"), raw);
        assert!(message
            .provider_meta
            .as_ref()
            .and_then(Value::as_object)
            .is_some_and(|meta| !meta.contains_key("attachments")));
        let wire = serde_json::to_string(&message).expect("serialize canonical remote message");
        assert!(!wire.contains("bytesBase64"));
        assert!(!wire.contains("bytes_base64"));
        assert!(!wire.contains("@download:"));
        assert!(!wire.contains("@media:"));
        let prompt_text = render_prompt_user_text_only(&message);
        assert!(prompt_text.contains("[图片#1]\npath: "));
        assert_eq!(prompt_text.matches("path: ").count(), 1);
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[tokio::test]
    async fn weixin_collector_should_keep_order_and_continue_after_bad_media_item() {
        let client = reqwest::Client::new();
        let credentials = WeixinOcCredentials::from_value(&serde_json::json!({}));
        let items = vec![
            WeixinOcMessageItem {
                item_type: Some(1),
                text_item: Some(WeixinOcTextItem {
                    text: Some("前文".to_string()),
                }),
                image_item: None,
                voice_item: None,
                file_item: None,
                video_item: None,
                ref_msg: None,
            },
            WeixinOcMessageItem {
                item_type: Some(2),
                text_item: None,
                image_item: None,
                voice_item: None,
                file_item: None,
                video_item: None,
                ref_msg: None,
            },
            WeixinOcMessageItem {
                item_type: Some(1),
                text_item: Some(WeixinOcTextItem {
                    text: Some("后文".to_string()),
                }),
                image_item: None,
                voice_item: None,
                file_item: None,
                video_item: None,
                ref_msg: None,
            },
        ];

        let collected = weixin_oc_collect_media(&client, &credentials, &items).await;
        let texts = collected
            .parts
            .iter()
            .filter_map(|part| match part {
                ChatIngressPart::Text { text } => Some(text.as_str()),
                ChatIngressPart::Attachment { .. } => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(texts.len(), 3);
        assert_eq!(texts[0], "前文");
        assert!(texts[1].contains("图片元数据缺失"));
        assert_eq!(texts[2], "后文");
    }

    fn remote_im_test_secretary_assistant_context() -> RemoteImConversationAssistantContext {
        RemoteImConversationAssistantContext {
            agent_id: "agent-sales".to_string(),
            agent_name: "售前助理".to_string(),
        }
    }

    fn remote_im_test_agent(agent_id: &str, agent_name: &str) -> AgentProfile {
        AgentProfile {
            id: agent_id.to_string(),
            name: agent_name.to_string(),
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
            source: "manual".to_string(),
            scope: "global".to_string(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        }
    }

    #[test]
    fn remote_im_secretary_current_assistant_context_should_read_from_runtime_slot() {
        let state = remote_im_test_state();
        let assistant = remote_im_test_secretary_assistant_context();

        set_conversation_remote_im_assistant_context(
            &state,
            "conversation-a",
            Some(assistant.clone()),
        )
        .expect("set runtime assistant");

        let resolved = remote_im_secretary_current_assistant_context(&state, "conversation-a")
            .expect("resolve runtime assistant");

        assert_eq!(resolved.agent_id, assistant.agent_id);
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn remote_im_resolve_contact_assistant_context_should_require_bound_agent() {
        let state = remote_im_test_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        state_write_agents_cached(
            &state,
            &[remote_im_test_agent(DEFAULT_AGENT_ID, "主助理"), default_user_persona()],
        )
        .expect("write agents");
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.bound_agent_id = None;

        let err = remote_im_resolve_contact_assistant_context(&state, &contact)
            .expect_err("missing agent should fail");

        assert!(err.contains("未设置应答人格"));
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn remote_im_resolve_contact_assistant_context_should_resolve_agent_name() {
        let state = remote_im_test_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        state_write_agents_cached(
            &state,
            &[remote_im_test_agent(DEFAULT_AGENT_ID, "主助理"), default_user_persona()],
        )
        .expect("write agents");
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.bound_agent_id = Some(DEFAULT_AGENT_ID.to_string());

        let resolved = remote_im_resolve_contact_assistant_context(&state, &contact)
            .expect("resolve assistant context");

        assert_eq!(resolved.agent_id, DEFAULT_AGENT_ID);
        assert_eq!(resolved.agent_name, "主助理");
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn remote_im_resolve_contact_assistant_context_should_reject_missing_agent_profile() {
        let state = remote_im_test_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        state_write_agents_cached(
            &state,
            &[remote_im_test_agent("agent-other", "其他助理"), default_user_persona()],
        )
        .expect("write agents");
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.bound_agent_id = Some(DEFAULT_AGENT_ID.to_string());

        let err = remote_im_resolve_contact_assistant_context(&state, &contact)
            .expect_err("missing agent profile should fail");

        assert!(err.contains("路由人格不存在"));
        assert!(err.contains(DEFAULT_AGENT_ID));
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn ensure_remote_im_contact_conversation_id_should_accept_private_bound_agent() {
        let state = remote_im_test_state();
        let mut api = ApiConfig::default();
        api.id = "api-a".to_string();
        api.enable_text = true;
        api.model = "gpt-4o-mini".to_string();
        let config = AppConfig {
            api_configs: vec![api],
            expert_api_config_id: "api-a".to_string(),
            ..AppConfig::default()
        };
        write_config(&state.config_path, &config).expect("write config");
        let private_personas_dir = app_root_from_data_path(&state.data_path)
            .join("llm-workspace")
            .join("private-organization")
            .join("personas");
        std::fs::create_dir_all(&private_personas_dir)
            .expect("create private personas dir");
        std::fs::write(
            private_personas_dir.join("private-agent.json"),
            r#"{
  "id": "private-agent",
  "name": "私域助理",
  "systemPrompt": "你是私域助理，负责接待私有渠道的联系人。"
}"#,
        )
        .expect("write private persona");
        state_write_agents_cached(&state, &[default_user_persona()]).expect("write agents");

        let mut contact = remote_im_test_contact("contact-private", "");
        contact.bound_agent_id = Some("private-agent".to_string());
        contact.bound_conversation_id = None;

        let conversation_id = ensure_remote_im_contact_conversation_id(&state, &mut contact)
            .expect("ensure contact conversation");
        let conversation =
            state_read_conversation_cached(&state, &conversation_id).expect("read conversation");

        assert_eq!(conversation.agent_id, "private-agent");
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn ensure_remote_im_contact_conversation_id_should_seed_initial_summary_context() {
        let state = remote_im_test_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let mut contact = remote_im_test_contact("contact-seed", "");
        contact.bound_conversation_id = None;

        let conversation_id = ensure_remote_im_contact_conversation_id(&state, &mut contact)
            .expect("ensure contact conversation");
        let conversation =
            state_read_conversation_cached(&state, &conversation_id).expect("read conversation");

        assert_eq!(conversation.messages.len(), 1);
        let seeded = &conversation.messages[0];
        assert_eq!(seeded.role, "user");
        let kind = seeded
            .provider_meta
            .as_ref()
            .and_then(|meta| meta.get("message_meta"))
            .and_then(|meta| meta.get("kind"))
            .and_then(Value::as_str);
        assert_eq!(kind, Some("summary_context_seed"));
        match &seeded.parts[0] {
            MessagePart::Text { text, .. } => assert!(text.contains("这是新会话的初始背景")),
            _ => panic!("expected text message"),
        }
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn remote_im_inbound_dedup_should_keep_recent_ten_platform_ids_per_channel() {
        let channel_a = format!("channel-a-{}", Uuid::new_v4());
        let channel_b = format!("channel-b-{}", Uuid::new_v4());

        assert!(!remote_im_remember_inbound_platform_message_id(
            &channel_a,
            Some("platform-1"),
        )
        .expect("remember first id"));
        assert!(remote_im_remember_inbound_platform_message_id(
            &channel_a,
            Some("platform-1"),
        )
        .expect("dedup repeated id"));
        assert!(!remote_im_remember_inbound_platform_message_id(
            &channel_b,
            Some("platform-1"),
        )
        .expect("same id in another channel should be accepted"));

        for index in 2..=11 {
            assert!(!remote_im_remember_inbound_platform_message_id(
                &channel_a,
                Some(&format!("platform-{index}")),
            )
            .expect("remember rolling id"));
        }
        assert!(!remote_im_remember_inbound_platform_message_id(
            &channel_a,
            Some("platform-1"),
        )
        .expect("old id should be evicted after ten newer ids"));
        assert!(!remote_im_remember_inbound_platform_message_id(&channel_a, None)
            .expect("missing platform id should not dedup"));
    }

    #[test]
    fn remote_im_collect_secretary_recent_messages_should_keep_last_seven_and_truncate_each_item() {
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.remote_contact_type = "private".to_string();
        contact.remote_contact_id = "contact-42".to_string();
        contact.remote_contact_name = "陈先生".to_string();
        let agents = vec![AgentProfile {
            id: "agent-sales".to_string(),
            name: "售前助理".to_string(),
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
            source: "manual".to_string(),
            scope: "global".to_string(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        }];
        let current_assistant = remote_im_test_secretary_assistant_context();
        let mut messages = Vec::<ChatMessage>::new();
        for idx in 0..8 {
            messages.push(ChatMessage {
                id: format!("msg-{idx}"),
                role: if idx % 2 == 0 { "user".to_string() } else { "assistant".to_string() },
                created_at: now_iso(),
                speaker_agent_id: if idx % 2 == 0 {
                    None
                } else {
                    Some("agent-sales".to_string())
                },
                parts: vec![MessagePart::Text {
                    text: format!("第{}条{}", idx, "很长的内容".repeat(30)),
                    reasoning_content: None,
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: if idx % 2 == 0 {
                    Some(serde_json::json!({
                        "origin": {
                            "kind": "remote_im",
                            "contact_type": "private",
                            "contact_id": "contact-42",
                            "contact_name": "陈先生",
                            "sender_id": "contact-42",
                            "sender_name": "陈先生"
                        }
                    }))
                } else {
                    None
                },
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
            });
        }

        let digests = remote_im_collect_secretary_recent_messages(
            &messages,
            7,
            &contact,
            &agents,
            &current_assistant,
        );

        assert_eq!(digests.len(), 7);
        assert_eq!(
            digests.first().map(|item| item.speaker.as_str()),
            Some("售前助理")
        );
        assert!(digests.iter().all(|item| item.text.chars().count() <= 100));
    }

    #[test]
    fn remote_reply_delegate_should_not_inject_busy_reminder_without_processing_messages() {
        let reminder = build_remote_im_reply_delegate_processing_reminder(&[]);

        assert!(reminder.is_none());
    }

    #[test]
    fn remote_reply_delegate_busy_reminder_should_keep_original_sender_name() {
        let message = ChatMessage {
            id: "message-route".to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "帮我规划从上海到杭州的路线".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "origin": {
                    "kind": "remote_im",
                    "contact_type": "group",
                    "contact_id": "group-42",
                    "contact_name": "项目群",
                    "sender_id": "user-7",
                    "sender_name": "用户甲"
                }
            })),
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        };
        let speaker = remote_im_reply_delegate_processing_message_speaker(&message, "contact-a");
        let line = format!(
            "- [{speaker}]：{}",
            remote_im_secretary_truncate_text(&render_message_content_for_model(&message), 100)
        );
        let reminder = build_remote_im_reply_delegate_processing_reminder(&[line])
            .expect("processing reminder should exist");

        assert!(reminder.contains("- [用户甲]：帮我规划从上海到杭州的路线"));
        assert!(!reminder.contains("- [contact-a]"));
        assert!(reminder.contains("请你假装你正在忙于工作，忙里偷闲回答，而不是暴露内部机制"));
    }

    #[test]
    fn remote_reply_delegate_second_round_should_prepend_reminder_before_metadata() {
        let mut trigger_message = ChatMessage {
            id: "message-weather".to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "今天天气怎么样？".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: vec!["[用户甲] 2026-07-17T10:01".to_string()],
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        };

        remote_im_reply_delegate_prepend_system_reminder(
            &mut trigger_message,
            "[以下消息已经在委托处理中，请不要重复处理]".to_string(),
        );

        assert_eq!(
            trigger_message.extra_text_blocks,
            vec![
                "[以下消息已经在委托处理中，请不要重复处理]".to_string(),
                "[用户甲] 2026-07-17T10:01".to_string(),
            ]
        );
        assert_eq!(
            render_message_content_for_model(&trigger_message),
            "今天天气怎么样？"
        );
    }

    #[test]
    fn departure_reflection_delegate_should_bind_contact_conversation_and_owner() {
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.remote_contact_type = "group".to_string();
        let conversation = Conversation {
            id: "conversation-a".to_string(),
            title: "群会话".to_string(),
            agent_id: "agent-a".to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
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
        let assistant = RemoteImConversationAssistantContext {
            agent_id: "agent-a".to_string(),
            agent_name: "客服".to_string(),
        };

        let input = remote_im_departure_reflection_delegate_input(
            &contact,
            &conversation,
            &assistant,
        );

        assert_eq!(input.kind, "remote_im_departure_reflection");
        assert_eq!(input.conversation_id, "conversation-a");
        assert_eq!(input.target_agent_id, "agent-a");
        assert!(!input.notify_assistant_when_done);
    }

    #[test]
    fn remote_im_secretary_message_digest_should_include_group_member_identity_and_media_placeholder() {
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.remote_contact_type = "group".to_string();
        contact.remote_contact_id = "group-88".to_string();
        contact.remote_contact_name = "项目群".to_string();
        let current_assistant = remote_im_test_secretary_assistant_context();
        let digest = remote_im_secretary_message_digest(&ChatMessage {
            id: "msg-image".to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Image {
                mime: "image/png".to_string(),
                bytes_base64: "abc".to_string(),
                name: None,
                compressed: false,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "origin": {
                    "kind": "remote_im",
                    "contact_type": "group",
                    "contact_id": "group-88",
                    "contact_name": "项目群",
                    "sender_id": "user-7",
                    "sender_name": "张三"
                }
            })),
            tool_call: None,
            mcp_call: None,
        meme_annotations: None,
        }, &contact, &Vec::new(), &current_assistant)
        .expect("digest");

        assert_eq!(digest.speaker, "群友 张三/user-7");
        assert_eq!(digest.text, "[图片]");
    }

    fn remote_im_test_group_user_message(sender_id: &str) -> ChatMessage {
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "群消息".to_string(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: Some(serde_json::json!({
                "origin": {
                    "kind": "remote_im",
                    "channel_id": "channel-a",
                    "contact_type": "group",
                    "contact_id": "group-88",
                    "contact_name": "项目群",
                    "sender_id": sender_id,
                    "sender_name": sender_id
                }
            })),
            tool_call: None,
            mcp_call: None,
            meme_annotations: None,
        }
    }

    #[test]
    fn remote_im_contact_activation_inner_should_preserve_private_response_strategy() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-private", "conversation-private");
        let expected_response_guidance = contact.response_guidance.clone();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        let updated = remote_im_update_contact_activation_inner(
            &state,
            RemoteImContactActivationUpdateInput {
                contact_id: "contact-private".to_string(),
                activation_mode: "always".to_string(),
                activation_keywords: Vec::new(),
                mute_keywords: default_remote_im_contact_mute_keywords(),
                unmute_keywords: default_remote_im_contact_unmute_keywords(),
                patience_seconds: default_remote_im_contact_patience_seconds(),
                mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
                activation_cooldown_seconds: 0,
                response_strategy: "smart_judge".to_string(),
                response_guidance: "测试指引".to_string(),
            },
        )
        .expect("update activation");

        assert_eq!(updated.activation_mode, "always");
        assert_eq!(updated.response_strategy, "always_reply");
        assert_eq!(updated.response_guidance, expected_response_guidance);
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn remote_im_blocked_message_prefixes_should_default_and_match_after_whitespace() {
        let defaults = default_remote_im_contact_blocked_message_prefixes();
        assert_eq!(defaults, vec!["#", "/", "%"]);
        assert_eq!(
            remote_im_blocked_inbound_message_prefix(" \n\t# Markdown 标题", &defaults),
            Some("#".to_string())
        );
        assert_eq!(
            remote_im_blocked_inbound_message_prefix("/help", &defaults),
            Some("/".to_string())
        );
        assert_eq!(remote_im_blocked_inbound_message_prefix("正文含 # 标记", &defaults), None);
        assert_eq!(
            remote_im_blocked_inbound_message_prefix("!quiet", &["!".to_string()]),
            Some("!".to_string())
        );
    }

    #[tokio::test]
    async fn remote_im_enqueue_should_discard_default_blocked_prefix_without_creating_contact() {
        let state = remote_im_test_state();
        let channel = RemoteImChannelConfig {
            id: "channel-a".to_string(),
            name: "QQ".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            enabled: true,
            credentials: serde_json::json!({}),
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            filter_markdown: false,
            allow_send_files: false,
            behavior_settings: RemoteImChannelBehaviorSettings::default(),
        };
        let config = AppConfig {
            remote_im_channels: vec![channel],
            ..AppConfig::default()
        };
        write_config(&state.config_path, &config).expect("write config");
        let input = RemoteImEnqueueInput {
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            im_name: "qq".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: "user-a".to_string(),
            remote_contact_name: Some("张三".to_string()),
            sender_id: "user-a".to_string(),
            sender_name: "张三".to_string(),
            sender_avatar_url: None,
            platform_message_id: Some("message-a".to_string()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            session: SessionSelector {
                api_config_id: None,
                agent_id: String::new(),
                conversation_id: None,
            },
            payload: ChatInputPayload {
                text: Some("  # 不接收的 Markdown 标题".to_string()),
                display_text: None,
                parts: None,
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: None,
            },
        };

        let result = remote_im_enqueue_message_internal(input, &state)
            .await
            .expect("enqueue result");
        assert!(result.event_id.is_empty());
        assert!(result.conversation_id.is_empty());
        assert!(result.contact_id.is_empty());
        assert!(
            state_service_list_remote_im_contacts(&state, None)
                .expect("list contacts")
                .is_empty()
        );
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[tokio::test]
    async fn remote_im_group_inbound_should_enter_contact_state_machine_after_persisting() {
        let state = remote_im_test_state();
        let config = AppConfig {
            remote_im_channels: vec![remote_im_test_channel(
                "channel-a",
                RemoteImChannelBehaviorSettings::default(),
            )],
            ..AppConfig::default()
        };
        write_config(&state.config_path, &config).expect("write config");

        let mut contact = remote_im_test_contact("contact-mention", "");
        contact.remote_contact_type = "group".to_string();
        contact.remote_contact_id = "group-mention".to_string();
        contact.remote_contact_name = "点名测试群".to_string();
        contact.activation_mode = "keyword".to_string();
        contact.activation_keywords = vec!["fairy".to_string()];
        contact.bound_conversation_id = None;
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        let result = remote_im_enqueue_message_internal(
            RemoteImEnqueueInput {
                channel_id: "channel-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                im_name: "QQ".to_string(),
                remote_contact_type: "group".to_string(),
                remote_contact_id: "group-mention".to_string(),
                remote_contact_name: Some("点名测试群".to_string()),
                sender_id: "member-a".to_string(),
                sender_name: "群友".to_string(),
                sender_avatar_url: None,
                platform_message_id: Some("message-mention".to_string()),
                dingtalk_session_webhook: None,
                dingtalk_session_webhook_expired_time: None,
                session: SessionSelector {
                    api_config_id: None,
                    agent_id: String::new(),
                    conversation_id: None,
                },
                payload: ChatInputPayload {
                    text: Some("fairy 在吗".to_string()),
                    display_text: None,
                    parts: None,
                    images: None,
                    audios: None,
                    attachments: None,
                    model: None,
                    extra_text_blocks: None,
                    mentions: None,
                    provider_meta: None,
                },
            },
            &state,
        )
        .await
        .expect("enqueue group mention");

        assert!(result.activate_assistant);
        let key = remote_im_group_reply_state_key(&state, &contact.id);
        let batch = lock_remote_im_group_reply_state_store()
            .by_contact
            .get(&key)
            .cloned()
            .expect("group inspection scheduled");
        assert_eq!(batch.phase, RemoteImGroupReplyPhase::MentionScheduled);
        assert_eq!(batch.event.conversation_id, result.conversation_id);
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[tokio::test]
    async fn remote_im_private_inbound_should_enqueue_guided_message_without_keyword() {
        let state = remote_im_test_state();
        let config = AppConfig {
            remote_im_channels: vec![remote_im_test_channel(
                "channel-a",
                RemoteImChannelBehaviorSettings::default(),
            )],
            ..AppConfig::default()
        };
        write_config(&state.config_path, &config).expect("write config");

        let mut contact = remote_im_test_contact("contact-private-guided", "");
        contact.remote_contact_type = "private".to_string();
        contact.remote_contact_id = "remote-private-guided".to_string();
        contact.remote_contact_name = "测试联系人".to_string();
        contact.activation_mode = "keyword".to_string();
        contact.activation_keywords = vec!["派".to_string()];
        contact.bound_conversation_id = None;
        let conversation_id = ensure_remote_im_contact_conversation_id(&state, &mut contact)
            .expect("ensure private conversation");
        flush_pending_persists_blocking(&state).expect("flush private conversation");
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        set_conversation_runtime_state_and_emit(
            &state,
            &conversation_id,
            MainSessionState::AssistantStreaming,
        )
        .expect("mark conversation busy");

        let result = remote_im_enqueue_message_internal(
            RemoteImEnqueueInput {
                channel_id: "channel-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                im_name: "QQ".to_string(),
                remote_contact_type: "private".to_string(),
                remote_contact_id: "remote-private-guided".to_string(),
                remote_contact_name: Some("测试联系人".to_string()),
                sender_id: "remote-private-guided".to_string(),
                sender_name: "测试联系人".to_string(),
                sender_avatar_url: None,
                platform_message_id: Some(format!("message-{}", Uuid::new_v4())),
                dingtalk_session_webhook: None,
                dingtalk_session_webhook_expired_time: None,
                session: SessionSelector {
                    api_config_id: None,
                    agent_id: String::new(),
                    conversation_id: None,
                },
                payload: ChatInputPayload {
                    text: Some("普通私聊消息".to_string()),
                    display_text: None,
                    parts: None,
                    images: None,
                    audios: None,
                    attachments: None,
                    model: None,
                    extra_text_blocks: None,
                    mentions: None,
                    provider_meta: None,
                },
            },
            &state,
        )
        .await
        .expect("enqueue private message");

        assert!(result.activate_assistant);
        assert_eq!(result.conversation_id, conversation_id);
        let queue = get_queue_snapshot(&state).expect("read queue");
        let queued = queue
            .iter()
            .find(|event| event.id == result.event_id)
            .expect("guided private event should be queued");
        assert_eq!(queued.queue_mode, ChatQueueMode::Guided);
        assert_eq!(queued.message_text, "普通私聊消息");
        let recent_messages = conversation_service_v2()
            .get_conversation_recent_messages(&state, &conversation_id, 8)
            .expect("read recent messages");
        assert!(!recent_messages.iter().any(|message| {
            message.parts.iter().any(|part| match part {
                MessagePart::Text { text, .. } => text == "普通私聊消息",
                _ => false,
            })
        }));
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn legacy_contact_prefix_update_should_not_override_channel_behavior() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-prefixes", "conversation-prefixes");
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        let updated = remote_im_update_contact_blocked_message_prefixes_inner(
            &state,
            RemoteImContactBlockedMessagePrefixesUpdateInput {
                contact_id: "contact-prefixes".to_string(),
                blocked_message_prefixes: vec!["! @".to_string(), " ! ".to_string(), "".to_string()],
            },
        )
        .expect("legacy prefix update should be tolerated");
        assert_eq!(
            updated.blocked_message_prefixes,
            default_remote_im_contact_blocked_message_prefixes()
        );

        let restored = state_service_get_remote_im_contact(&state, "contact-prefixes")
            .expect("read contact")
            .expect("contact exists");
        assert_eq!(
            restored.blocked_message_prefixes,
            default_remote_im_contact_blocked_message_prefixes()
        );

        let cleared = remote_im_update_contact_blocked_message_prefixes_inner(
            &state,
            RemoteImContactBlockedMessagePrefixesUpdateInput {
                contact_id: "contact-prefixes".to_string(),
                blocked_message_prefixes: Vec::new(),
            },
        )
        .expect("legacy prefix clear should be tolerated");
        assert_eq!(
            cleared.blocked_message_prefixes,
            default_remote_im_contact_blocked_message_prefixes()
        );
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn remote_im_contact_without_blocked_message_prefixes_should_use_defaults() {
        let contact = remote_im_test_contact("contact-legacy", "conversation-legacy");
        let mut value = serde_json::to_value(contact).expect("serialize contact");
        value
            .as_object_mut()
            .expect("contact json object")
            .remove("blockedMessagePrefixes");

        let restored: RemoteImContact = serde_json::from_value(value).expect("deserialize contact");
        assert_eq!(
            restored.blocked_message_prefixes,
            default_remote_im_contact_blocked_message_prefixes()
        );
    }

    #[test]
    fn remote_im_legacy_contact_and_checkpoint_should_default_group_reply_fields() {
        let contact = remote_im_test_contact("contact-legacy-pacing", "conversation-legacy-pacing");
        let mut value = serde_json::to_value(contact).expect("serialize contact");
        value
            .as_object_mut()
            .expect("contact json object")
            .remove("groupReplyPacing");
        let restored: RemoteImContact = serde_json::from_value(value).expect("deserialize contact");
        assert_eq!(restored.group_reply_pacing, RemoteImGroupReplyPacing::default());

        let checkpoint: RemoteImContactCheckpoint = serde_json::from_value(serde_json::json!({
            "contactId": "contact-legacy-pacing"
        }))
        .expect("deserialize checkpoint");
        assert_eq!(checkpoint.energy, None);
        assert_eq!(checkpoint.energy_updated_at, None);
        assert_eq!(checkpoint.last_success_reply_at, None);
        assert_eq!(checkpoint.atomic_revision, 0);
        assert!(checkpoint.group_reply_delivery.is_none());
    }

    #[test]
    fn legacy_contact_behavior_update_should_not_override_channel_behavior() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-behavior", "conversation-behavior");
        contact.remote_contact_type = "group".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        let mut pacing = RemoteImGroupReplyPacing::default();
        pacing.normal_reply_max_chars = 30;
        pacing.focus_reply_max_chars = 120;
        pacing.positive_energy_phrases = vec!["谢谢".to_string(), "谢谢".to_string()];
        let updated = remote_im_update_contact_behavior_inner(
            &state,
            RemoteImContactBehaviorUpdateInput {
                contact_id: "contact-behavior".to_string(),
                mute_keywords: vec!["安静".to_string()],
                unmute_keywords: vec!["继续".to_string()],
                patience_seconds: 90,
                mute_duration_seconds: 300,
                activation_cooldown_seconds: 8,
                blocked_message_prefixes: vec!["#".to_string(), "#".to_string()],
                group_reply_pacing: pacing,
            },
        )
        .expect("legacy behavior update should be tolerated");
        assert_eq!(updated.group_reply_pacing, RemoteImGroupReplyPacing::default());
        assert_eq!(
            updated.blocked_message_prefixes,
            default_remote_im_contact_blocked_message_prefixes()
        );

        let mut invalid = updated.group_reply_pacing.clone();
        invalid.focus_reply_max_chars = 10;
        remote_im_update_contact_behavior_inner(
            &state,
            RemoteImContactBehaviorUpdateInput {
                contact_id: "contact-behavior".to_string(),
                mute_keywords: Vec::new(),
                unmute_keywords: Vec::new(),
                patience_seconds: 0,
                mute_duration_seconds: 0,
                activation_cooldown_seconds: 0,
                blocked_message_prefixes: Vec::new(),
                group_reply_pacing: invalid,
            },
        )
        .expect("legacy invalid behavior update should be tolerated");
        let persisted = state_service_get_remote_im_contact(&state, "contact-behavior")
            .expect("read contact")
            .expect("contact exists");
        assert_eq!(persisted.group_reply_pacing, RemoteImGroupReplyPacing::default());
    }

    #[test]
    fn private_contact_behavior_save_should_ignore_invalid_group_only_pacing() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-private-behavior", "conversation-private-behavior");
        contact.group_reply_pacing.normal_reply_max_chars = 40;
        contact.group_reply_pacing.focus_reply_max_chars = 10;
        let original_pacing = contact.group_reply_pacing.clone();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        let updated = remote_im_update_contact_behavior_inner(
            &state,
            RemoteImContactBehaviorUpdateInput {
                contact_id: "contact-private-behavior".to_string(),
                mute_keywords: vec!["安静".to_string()],
                unmute_keywords: vec!["继续".to_string()],
                patience_seconds: 15,
                mute_duration_seconds: 20,
                activation_cooldown_seconds: 5,
                blocked_message_prefixes: vec!["#".to_string()],
                group_reply_pacing: RemoteImGroupReplyPacing {
                    assistant_debounce_seconds: 0,
                    ..RemoteImGroupReplyPacing::default()
                },
            },
        )
        .expect("private behavior save should ignore group-only pacing");
        assert_eq!(
            updated.blocked_message_prefixes,
            default_remote_im_contact_blocked_message_prefixes()
        );
        assert_eq!(updated.group_reply_pacing, original_pacing);
    }

    #[test]
    fn remote_im_patch_contact_settings_should_ignore_legacy_behavior_fields() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-patch", "conversation-patch");
        contact.remote_contact_type = "group".to_string();
        contact.processing_mode = "continuous".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        let mut invalid_pacing = RemoteImGroupReplyPacing::default();
        invalid_pacing.assistant_debounce_seconds = 0;
        let updated = remote_im_patch_contact_settings_inner(
            &state,
            RemoteImContactSettingsPatchInput {
                contact_id: "contact-patch".to_string(),
                agent_id: None,
                processing_mode: "qa".to_string(),
                blocked_message_prefixes: vec!["#".to_string()],
                activation_mode: "always".to_string(),
                activation_keywords: vec!["唤醒".to_string()],
                mute_keywords: vec!["安静".to_string()],
                unmute_keywords: vec!["继续".to_string()],
                patience_seconds: 10,
                mute_duration_seconds: 20,
                activation_cooldown_seconds: 30,
                group_reply_pacing: invalid_pacing,
                response_strategy: "always_reply".to_string(),
                response_guidance: "请回复".to_string(),
                allow_receive: false,
                allow_send: false,
                allow_send_files: true,
            },
        )
        .expect("legacy behavior fields should not block contact settings save");
        let persisted = state_service_get_remote_im_contact(&state, "contact-patch")
            .expect("read contact")
            .expect("contact exists");
        assert_eq!(updated.processing_mode, "qa");
        assert_eq!(persisted.processing_mode, "qa");
        assert!(!persisted.allow_receive);
        assert!(!persisted.allow_send);
        assert!(persisted.allow_send_files);
    }

    #[test]
    fn remote_im_patch_contact_settings_should_filter_group_only_fields_for_private_contact() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-private-patch", "conversation-private-patch");
        contact.group_reply_pacing.maximum_energy = 77.0;
        let expected_response_guidance = contact.response_guidance.clone();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        let mut incoming_pacing = RemoteImGroupReplyPacing::default();
        incoming_pacing.maximum_energy = 12.0;

        let updated = remote_im_patch_contact_settings_inner(
            &state,
            RemoteImContactSettingsPatchInput {
                contact_id: "contact-private-patch".to_string(),
                agent_id: None,
                processing_mode: "qa".to_string(),
                blocked_message_prefixes: vec!["[bot]".to_string()],
                activation_mode: "never".to_string(),
                activation_keywords: vec!["不应保留".to_string()],
                mute_keywords: vec!["安静".to_string()],
                unmute_keywords: vec!["继续".to_string()],
                patience_seconds: 10,
                mute_duration_seconds: 20,
                activation_cooldown_seconds: 30,
                group_reply_pacing: incoming_pacing,
                response_strategy: "smart_judge".to_string(),
                response_guidance: "不应保留".to_string(),
                allow_receive: true,
                allow_send: false,
                allow_send_files: true,
            },
        )
        .expect("patch private contact");
        assert_eq!(updated.activation_mode, "always");
        assert!(updated.activation_keywords.is_empty());
        assert_eq!(updated.response_strategy, "always_reply");
        assert_eq!(updated.response_guidance, expected_response_guidance);
        assert_eq!(updated.group_reply_pacing.maximum_energy, 77.0);
        assert!(updated.allow_receive && updated.allow_send);
    }

    #[test]
    fn remote_im_patch_contact_settings_should_save_when_organization_snapshot_is_unreadable() {
        let state = remote_im_test_state();
        std::fs::create_dir_all(&state.config_path).expect("make config path unreadable as file");
        let contact = remote_im_test_contact(
            "contact-full-patch-config-degraded",
            "conversation-full-patch-config-degraded",
        );
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");

        let updated = remote_im_patch_contact_settings_inner(
            &state,
            RemoteImContactSettingsPatchInput {
                contact_id: contact.id.clone(),
                agent_id: Some("agent-config-degraded".to_string()),
                processing_mode: "qa".to_string(),
                blocked_message_prefixes: vec!["[skip]".to_string()],
                activation_mode: "never".to_string(),
                activation_keywords: vec!["ignored-private".to_string()],
                mute_keywords: vec!["安静".to_string()],
                unmute_keywords: vec!["继续".to_string()],
                patience_seconds: 10,
                mute_duration_seconds: 20,
                activation_cooldown_seconds: 30,
                group_reply_pacing: RemoteImGroupReplyPacing::default(),
                response_strategy: "smart_judge".to_string(),
                response_guidance: "ignored-private".to_string(),
                allow_receive: true,
                allow_send: false,
                allow_send_files: true,
            },
        )
        .expect("full patch should save despite organization read failure");

        assert_eq!(
            updated.bound_agent_id.as_deref(),
            Some("agent-config-degraded")
        );
        assert_eq!(updated.processing_mode, "qa");
        assert_eq!(
            updated.blocked_message_prefixes,
            default_remote_im_contact_blocked_message_prefixes()
        );
        assert!(updated.allow_receive && updated.allow_send);
        assert!(updated.allow_send_files);
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn group_reply_settlement_should_be_idempotent_after_partial_write_result() {
        let state = remote_im_test_state();
        let mut behavior = RemoteImChannelBehaviorSettings::default();
        behavior.group_reply_pacing.base_reply_energy_cost = 100.0;
        behavior.group_reply_pacing.energy_cost_per_character = 1.0;
        state_write_config_cached(
            &state,
            &AppConfig {
                remote_im_channels: vec![remote_im_test_channel("channel-a", behavior)],
                ..AppConfig::default()
            },
        )
        .expect("write channel behavior config");
        let mut contact = remote_im_test_contact("contact-settlement", "conversation-settlement");
        contact.remote_contact_type = "group".to_string();
        let outbound_key = "group-reply::contact-settlement::7::message-9".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                energy: Some(1.0),
                energy_updated_at: Some(now_iso()),
                group_reply_delivery: Some(RemoteImGroupReplyDeliveryMarker {
                    generation: 7,
                    boundary_message_id: "message-9".to_string(),
                    outbound_key: outbound_key.clone(),
                    final_text: "你好".to_string(),
                    status: "dispatching".to_string(),
                    platform_message_id: None,
                    energy_applied: false,
                    updated_at: Some(now_iso()),
                }),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("write checkpoint");
        let settlement = RemoteImGroupReplySettlement {
            boundary_message_id: "message-9".to_string(),
            final_text: Some("你好".to_string()),
            outbound_key: Some(outbound_key),
            platform_message_id: Some("platform-1".to_string()),
            status: RemoteImGroupReplySettlementStatus::Delivered,
        };

        remote_im_persist_group_reply_settlement(&state, &contact, &settlement)
            .expect("first settlement");
        let first = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read first checkpoint")
            .expect("checkpoint exists");
        remote_im_persist_group_reply_settlement(&state, &contact, &settlement)
            .expect("retry settlement");
        let second = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read second checkpoint")
            .expect("checkpoint exists");
        assert_eq!(first.energy, Some(-100.0));
        assert_eq!(first.energy, second.energy);
        assert_eq!(second.last_boundary_covers_message_id.as_deref(), Some("message-9"));
        assert_eq!(
            second.group_reply_delivery.as_ref().map(|marker| marker.status.as_str()),
            Some("committed")
        );
    }

    #[test]
    fn uncertain_group_reply_should_charge_once_without_marking_success_then_allow_late_commit() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact(
            "contact-uncertain-settlement",
            "conversation-uncertain-settlement",
        );
        contact.remote_contact_type = "group".to_string();
        let outbound_key =
            "group-reply::contact-uncertain-settlement::8::message-10".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                energy: Some(100.0),
                energy_updated_at: Some(now_iso()),
                group_reply_delivery: Some(RemoteImGroupReplyDeliveryMarker {
                    generation: 8,
                    boundary_message_id: "message-10".to_string(),
                    outbound_key: outbound_key.clone(),
                    final_text: "可能已经送达".to_string(),
                    status: "dispatching".to_string(),
                    platform_message_id: None,
                    energy_applied: false,
                    updated_at: Some(now_iso()),
                }),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("write checkpoint");

        remote_im_persist_group_reply_settlement(
            &state,
            &contact,
            &RemoteImGroupReplySettlement {
                boundary_message_id: "message-10".to_string(),
                final_text: Some("可能已经送达".to_string()),
                outbound_key: Some(outbound_key.clone()),
                platform_message_id: None,
                status: RemoteImGroupReplySettlementStatus::Uncertain,
            },
        )
        .expect("persist uncertain settlement");
        let uncertain = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read uncertain checkpoint")
            .expect("checkpoint exists");
        assert!(uncertain.energy.unwrap_or_default() < 100.0);
        assert_eq!(uncertain.last_success_reply_at, None);
        assert_eq!(
            uncertain
                .group_reply_delivery
                .as_ref()
                .map(|marker| (marker.status.as_str(), marker.energy_applied)),
            Some(("uncertain", true))
        );
        let charged_energy = uncertain.energy;

        remote_im_persist_group_reply_settlement(
            &state,
            &contact,
            &RemoteImGroupReplySettlement {
                boundary_message_id: "message-10".to_string(),
                final_text: Some("可能已经送达".to_string()),
                outbound_key: Some(outbound_key),
                platform_message_id: Some("platform-late-commit".to_string()),
                status: RemoteImGroupReplySettlementStatus::Delivered,
            },
        )
        .expect("persist late delivered settlement");
        let delivered = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read delivered checkpoint")
            .expect("checkpoint exists");
        assert_eq!(delivered.energy, charged_energy);
        assert!(delivered.last_success_reply_at.is_some());
        assert_eq!(
            delivered
                .group_reply_delivery
                .as_ref()
                .map(|marker| (marker.status.as_str(), marker.energy_applied)),
            Some(("committed", true))
        );
    }

    #[test]
    fn atomic_runtime_mutation_should_preserve_parallel_group_delivery_markers() {
        let state = remote_im_test_state();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let mut workers = Vec::new();
        for index in 0..2_u64 {
            let state = state.clone();
            let barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                let contact_id = format!("contact-parallel-{index}");
                let outbound_key = format!("group-reply::{contact_id}::{index}");
                barrier.wait();
                state_service_set_remote_im_contact_checkpoint(
                    &state,
                    &RemoteImContactCheckpoint {
                        contact_id: contact_id.clone(),
                        group_reply_delivery: Some(RemoteImGroupReplyDeliveryMarker {
                            generation: index,
                            boundary_message_id: format!("message-{index}"),
                            outbound_key,
                            final_text: format!("reply-{index}"),
                            status: "dispatching".to_string(),
                            platform_message_id: None,
                            energy_applied: false,
                            updated_at: Some(now_iso()),
                        }),
                        ..RemoteImContactCheckpoint::default()
                    },
                )
                .expect("set checkpoint");
            }));
        }
        barrier.wait();
        for worker in workers {
            worker.join().expect("join mutation worker");
        }

        for index in 0..2_u64 {
            let contact_id = format!("contact-parallel-{index}");
            let expected_key = format!("group-reply::{contact_id}::{index}");
            let checkpoint = state_service_get_remote_im_contact_checkpoint(&state, &contact_id)
                .expect("read checkpoint")
                .expect("checkpoint exists");
            assert_eq!(
                checkpoint
                    .group_reply_delivery
                    .as_ref()
                    .map(|marker| marker.outbound_key.as_str()),
                Some(expected_key.as_str())
            );
        }
    }

    #[test]
    fn stale_full_runtime_write_should_preserve_atomic_group_checkpoint_fields() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact(
            "contact-stale-writer",
            "conversation-stale-writer",
        );
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                latest_seen_message_id: Some("message-new-inbound".to_string()),
                updated_at: Some(now_iso()),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("write latest seen checkpoint");

        // 模拟生产 get→mutate→set 模式：读取最新 checkpoint 后叠加字段再写回
        let mut merged = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read latest checkpoint")
            .expect("checkpoint exists");
        merged.energy = Some(82.0);
        merged.energy_updated_at = Some(now_iso());
        merged.last_boundary_message_id = Some("message-settled".to_string());
        merged.last_boundary_covers_message_id = Some("message-settled".to_string());
        merged.group_reply_delivery = Some(RemoteImGroupReplyDeliveryMarker {
            generation: 21,
            boundary_message_id: "message-settled".to_string(),
            outbound_key: "group-reply::atomic".to_string(),
            final_text: "已发送".to_string(),
            status: "committed".to_string(),
            platform_message_id: Some("platform-atomic".to_string()),
            energy_applied: true,
            updated_at: Some(now_iso()),
        });
        merged.updated_at = Some(now_iso());
        state_service_set_remote_im_contact_checkpoint(&state, &merged)
            .expect("write merged checkpoint");

        let checkpoint = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read merged checkpoint")
            .expect("checkpoint exists");
        assert_eq!(
            checkpoint.latest_seen_message_id.as_deref(),
            Some("message-new-inbound")
        );
        assert_eq!(checkpoint.energy, Some(82.0));
        assert_eq!(
            checkpoint.last_boundary_covers_message_id.as_deref(),
            Some("message-settled")
        );
        assert_eq!(
            checkpoint
                .group_reply_delivery
                .as_ref()
                .map(|marker| (marker.outbound_key.as_str(), marker.status.as_str())),
            Some(("group-reply::atomic", "committed"))
        );
    }

    #[test]
    fn atomic_checkpoint_reset_should_survive_stale_full_writer() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact(
            "contact-reset-checkpoint",
            "conversation-reset-checkpoint",
        );
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                atomic_revision: 1,
                energy: Some(55.0),
                group_reply_delivery: Some(RemoteImGroupReplyDeliveryMarker {
                    generation: 2,
                    boundary_message_id: "message-old".to_string(),
                    outbound_key: "group-reply::reset".to_string(),
                    final_text: "旧回复".to_string(),
                    status: "committed".to_string(),
                    platform_message_id: Some("platform-old".to_string()),
                    energy_applied: true,
                    updated_at: Some(now_iso()),
                }),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("seed checkpoint");
        let stale_revision = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read stale checkpoint")
            .expect("checkpoint exists")
            .atomic_revision;

        // 模拟生产 get→mutate→set 的原子重置：读取后清空字段、提升 revision 再写回
        let mut reset = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read latest checkpoint")
            .expect("checkpoint exists");
        reset.atomic_revision = reset.atomic_revision.saturating_add(1).max(1);
        reset.energy = None;
        reset.energy_updated_at = None;
        reset.last_boundary_message_id = None;
        reset.last_boundary_covers_message_id = None;
        reset.last_success_reply_at = None;
        reset.group_reply_delivery = None;
        reset.updated_at = Some(now_iso());
        state_service_set_remote_im_contact_checkpoint(&state, &reset)
            .expect("reset checkpoint");

        assert!(state_service_get_remote_im_contact(&state, &contact.id)
            .expect("read contact")
            .is_some());
        let checkpoint = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read reset checkpoint")
            .expect("reset tombstone checkpoint");
        assert!(checkpoint.atomic_revision > stale_revision);
        assert_eq!(checkpoint.energy, None);
        assert_eq!(checkpoint.last_boundary_message_id, None);
        assert_eq!(checkpoint.last_boundary_covers_message_id, None);
        assert_eq!(checkpoint.last_success_reply_at, None);
        assert!(checkpoint.group_reply_delivery.is_none());
    }

    #[test]
    fn stale_full_writer_should_not_restore_deleted_contact_or_rollback_contact_config() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact(
            "contact-runtime-revision",
            "conversation-runtime-revision",
        );
        contact.remark_name = "旧配置".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("seed contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                energy: Some(66.0),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("seed checkpoint");

        // 并发更新 contact 配置
        let mut updated = contact.clone();
        updated.remark_name = "新配置".to_string();
        state_service_upsert_remote_im_contact(&state, &updated).expect("update contact");
        let after_update = state_service_get_remote_im_contact(&state, &contact.id)
            .expect("read updated contact")
            .expect("contact exists");
        assert_eq!(after_update.remark_name, "新配置");
        // 无关字段（pinned）的并发写不影响 contact
        state_service_set_pinned_conversation_ids(&state, &["stale-writer-unrelated-change".to_string()])
            .expect("unrelated concurrent write");
        let after_unrelated = state_service_get_remote_im_contact(&state, &contact.id)
            .expect("read contact after unrelated write")
            .expect("contact exists");
        assert_eq!(after_unrelated.remark_name, "新配置");

        // 删除 contact 后 checkpoint 一并清理，且 stale 快照不会复活
        assert!(state_service_remove_remote_im_contact(&state, &contact.id).expect("delete contact"));
        assert!(state_service_get_remote_im_contact(&state, &contact.id)
            .expect("read deleted contact")
            .is_none());
        assert!(state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read deleted checkpoint")
            .is_none());
    }

    #[test]
    fn contact_command_should_persist_after_concurrent_checkpoint_revision() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact(
            "contact-command-atomic",
            "conversation-command-atomic",
        );
        contact.allow_send = false;
        contact.allow_receive = false;
        state_service_upsert_remote_im_contact(&state, &contact).expect("seed contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                energy: Some(75.0),
                energy_updated_at: Some(now_iso()),
                atomic_revision: 1,
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("concurrent checkpoint update");

        let updated = remote_im_update_contact_allow_send_inner(
            &state,
            RemoteImContactAllowSendUpdateInput {
                contact_id: contact.id.clone(),
                allow_send: true,
            },
        )
        .expect("atomic contact update");
        assert!(updated.allow_send && updated.allow_receive);
        let persisted = state_service_get_remote_im_contact(&state, &contact.id)
            .expect("read contact")
            .expect("contact exists");
        assert!(persisted.allow_send && persisted.allow_receive);
        assert_eq!(
            state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
                .expect("read checkpoint")
                .expect("checkpoint exists")
                .energy,
            Some(75.0)
        );
    }

    #[test]
    fn stale_binding_resolution_should_not_overwrite_concurrent_user_binding() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact(
            "contact-binding-cas",
            "conversation-binding-old",
        );
        contact.bound_agent_id = Some("agent-old".to_string());
        let baseline = remote_im_contact_binding_snapshot(&contact);
        let mut stale_resolved = baseline.clone();
        stale_resolved.bound_conversation_id = Some("conversation-resolved-old".to_string());
        state_service_upsert_remote_im_contact(&state, &contact).expect("seed contact");
        remote_im_mutate_contact(&state, &contact.id, |latest| {
            latest.bound_agent_id = Some("agent-new".to_string());
            latest.bound_conversation_id = Some("conversation-new".to_string());
            Ok(())
        })
        .expect("concurrent user binding");

        let applied = remote_im_mutate_contact(&state, &contact.id, |latest| {
            if !remote_im_contact_binding_matches(latest, &baseline) {
                return Ok(false);
            }
            remote_im_apply_contact_binding_snapshot(latest, &stale_resolved);
            Ok(true)
        })
        .expect("binding CAS");
        assert!(!applied);
        let persisted = state_service_get_remote_im_contact(&state, &contact.id)
            .expect("read contact")
            .expect("contact exists");
        assert_eq!(persisted.bound_agent_id.as_deref(), Some("agent-new"));
        assert_eq!(
            persisted.bound_conversation_id.as_deref(),
            Some("conversation-new")
        );
    }

    #[test]
    fn ensure_contact_conversation_should_not_sync_routing_before_authoritative_commit() {
        let state = remote_im_test_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        state_write_agents_cached(
            &state,
            &[
                remote_im_test_agent(DEFAULT_AGENT_ID, "主助理"),
                remote_im_test_agent("agent-new", "新助理"),
                default_user_persona(),
            ],
        )
        .expect("write agents");

        let mut authoritative = remote_im_test_contact("contact-sync-order", "");
        authoritative.bound_agent_id = Some(DEFAULT_AGENT_ID.to_string());
        authoritative.bound_conversation_id = None;
        let conversation_id = ensure_remote_im_contact_conversation_id(
            &state,
            &mut authoritative,
        )
        .expect("create original conversation");
        state_service_upsert_remote_im_contact(&state, &authoritative)
            .expect("seed authoritative contact");
        conversation_service_v2()
            .set_preferred_api_config_id(
                &state,
                &conversation_id,
                Some("legacy-provider".to_string()),
            )
            .expect("seed legacy preferred provider");

        let mut candidate = authoritative.clone();
        candidate.bound_agent_id = Some("agent-new".to_string());
        ensure_remote_im_contact_conversation_id(&state, &mut candidate)
            .expect("reuse conversation without side effect");
        let before_commit = conversation_service_v2()
            .get_conversation_meta(&state, &conversation_id)
            .expect("read conversation before commit");
        assert_eq!(before_commit.agent_id, DEFAULT_AGENT_ID);

        remote_im_mutate_contact(&state, &authoritative.id, |contact| {
            remote_im_apply_contact_binding_snapshot(
                contact,
                &remote_im_contact_binding_snapshot(&candidate),
            );
            Ok(())
        })
        .expect("commit authoritative binding");
        sync_remote_im_contact_conversation_binding(
            &state,
            &candidate,
            &conversation_id,
        )
        .expect("sync committed route");
        let after_commit = conversation_service_v2()
            .get_conversation_meta(&state, &conversation_id)
            .expect("read conversation after commit");
        assert_eq!(after_commit.agent_id, "agent-new");
        assert!(after_commit.preferred_api_config_id.is_none());
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn concurrent_agent_updates_should_leave_conversation_on_authoritative_route() {
        let state = remote_im_test_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        state_write_agents_cached(
            &state,
            &[
                remote_im_test_agent(DEFAULT_AGENT_ID, "主助理"),
                remote_im_test_agent("agent-concurrent-a", "并发助理甲"),
                remote_im_test_agent("agent-concurrent-b", "并发助理乙"),
                default_user_persona(),
            ],
        )
        .expect("write agents");
        let mut contact = remote_im_test_contact("contact-agent-concurrent", "");
        contact.bound_agent_id = Some(DEFAULT_AGENT_ID.to_string());
        contact.bound_conversation_id = None;
        let conversation_id = ensure_remote_im_contact_conversation_id(&state, &mut contact)
            .expect("create contact conversation");
        state_service_upsert_remote_im_contact(&state, &contact).expect("seed contact");

        let barrier = Arc::new(std::sync::Barrier::new(3));
        let mut handles = Vec::new();
        for agent_id in ["agent-concurrent-a", "agent-concurrent-b"] {
            let state = state.clone();
            let barrier = barrier.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                remote_im_update_contact_agent_binding_inner(
                    &state,
                    RemoteImContactAgentBindingUpdateInput {
                        contact_id: "contact-agent-concurrent".to_string(),
                        agent_id: Some(agent_id.to_string()),
                    },
                )
            }));
        }
        barrier.wait();
        for handle in handles {
            handle
                .join()
                .expect("join concurrent update")
                .expect("concurrent update should degrade instead of abort");
        }

        let persisted = state_service_get_remote_im_contact(&state, &contact.id)
            .expect("read contact")
            .expect("contact exists");
        let conversation = conversation_service_v2()
            .get_conversation_meta(&state, &conversation_id)
            .expect("read conversation");
        assert_eq!(
            conversation.agent_id,
            persisted.bound_agent_id.unwrap_or_default()
        );
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn binding_sync_rollback_should_restore_original_route_after_postwrite_failure() {
        let state = remote_im_test_state();
        write_config(&state.config_path, &AppConfig::default()).expect("write config");
        let mut contact = remote_im_test_contact("contact-sync-rollback", "");
        contact.bound_agent_id = Some(DEFAULT_AGENT_ID.to_string());
        contact.bound_conversation_id = None;
        let conversation_id = ensure_remote_im_contact_conversation_id(&state, &mut contact)
            .expect("create conversation");
        let original = conversation_service_v2()
            .get_conversation_meta(&state, &conversation_id)
            .expect("read original route");
        let mut written_contact = contact.clone();
        written_contact.bound_agent_id = Some("stale-agent".to_string());
        let stale_root = remote_im_contact_conversation_key(&written_contact);
        state_update_conversation_metadata_cached(&state, &conversation_id, |conversation| {
            conversation.agent_id = "stale-agent".to_string();
            conversation.root_conversation_id = Some(stale_root);
            Ok(())
        })
        .expect("inject stale route");

        restore_remote_im_contact_conversation_binding(
            &state,
            &conversation_id,
            &original,
            &written_contact,
            "stale-agent",
        )
        .expect("restore route");

        let restored = conversation_service_v2()
            .get_conversation_meta(&state, &conversation_id)
            .expect("read restored route");
        assert_eq!(restored.agent_id, original.agent_id);
        assert_eq!(restored.root_conversation_id, original.root_conversation_id);

        state_update_conversation_metadata_cached(&state, &conversation_id, |conversation| {
            conversation.agent_id = "new-authoritative-agent".to_string();
            conversation.root_conversation_id = Some("new-authoritative-root".to_string());
            Ok(())
        })
        .expect("inject concurrent authoritative route");
        restore_remote_im_contact_conversation_binding(
            &state,
            &conversation_id,
            &original,
            &written_contact,
            "stale-agent",
        )
        .expect("skip stale rollback");
        let preserved = conversation_service_v2()
            .get_conversation_meta(&state, &conversation_id)
            .expect("read preserved route");
        assert_eq!(preserved.agent_id, "new-authoritative-agent");
        assert_eq!(
            preserved.root_conversation_id.as_deref(),
            Some("new-authoritative-root")
        );
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[tokio::test]
    async fn agent_binding_should_persist_when_organization_snapshot_is_unreadable() {
        let state = remote_im_test_state();
        std::fs::create_dir_all(&state.config_path).expect("make config path unreadable as file");
        let contact = remote_im_test_contact("contact-config-degraded", "conversation-existing");
        state_service_upsert_remote_im_contact(&state, &contact).expect("seed contact");

        let updated = remote_im_update_contact_agent_binding_inner(
            &state,
            RemoteImContactAgentBindingUpdateInput {
                contact_id: contact.id.clone(),
                agent_id: Some("agent-offline".to_string()),
            },
        )
        .expect("save raw binding despite config read failure");

        assert_eq!(updated.bound_agent_id.as_deref(), Some("agent-offline"));
        let persisted = state_service_get_remote_im_contact(&state, &contact.id)
            .expect("read persisted contact")
            .expect("contact exists");
        assert_eq!(persisted.bound_agent_id.as_deref(), Some("agent-offline"));
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[tokio::test]
    async fn inbound_should_use_safe_fallback_when_config_is_unreadable() {
        let state = remote_im_test_state();
        let mut config = AppConfig::default();
        config.remote_im_channels.push(RemoteImChannelConfig {
            id: "channel-config-degraded".to_string(),
            name: "QQ".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            enabled: true,
            credentials: serde_json::json!({}),
            receive_files: false,
            streaming_send: false,
            show_tool_calls: false,
            filter_markdown: false,
            allow_send_files: false,
            behavior_settings: RemoteImChannelBehaviorSettings::default(),
        });
        state_write_config_cached(&state, &config).expect("seed trusted config cache");
        std::fs::remove_file(&state.config_path).expect("remove config file");
        std::fs::create_dir_all(&state.config_path).expect("make config path unreadable as file");
        *state
            .cached_config_mtime
            .lock()
            .expect("lock cached config mtime") = Some(std::time::SystemTime::UNIX_EPOCH);
        let input = RemoteImEnqueueInput {
            channel_id: "channel-config-degraded".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            im_name: "QQ".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: "remote-config-degraded".to_string(),
            remote_contact_name: Some("降级联系人".to_string()),
            sender_id: "sender-config-degraded".to_string(),
            sender_name: "联系人".to_string(),
            sender_avatar_url: None,
            platform_message_id: Some("message-config-degraded".to_string()),
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            session: SessionSelector {
                api_config_id: None,
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_id: None,
            },
            payload: ChatInputPayload {
                text: Some("配置暂时读不到也要接收".to_string()),
                display_text: None,
                parts: None,
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: None,
            },
        };

        let result = remote_im_enqueue_message_internal(input, &state)
            .await
            .expect("config read failure should degrade instead of aborting");

        assert!(!result.contact_id.is_empty());
        assert!(result.conversation_id.is_empty());
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[tokio::test]
    async fn inbound_should_fail_closed_without_trusted_channel_snapshot() {
        let state = remote_im_test_state();
        std::fs::create_dir_all(&state.config_path).expect("make config path unreadable as file");
        let input = RemoteImEnqueueInput {
            channel_id: "unknown-channel".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            im_name: "QQ".to_string(),
            remote_contact_type: "private".to_string(),
            remote_contact_id: "remote-unknown-channel".to_string(),
            remote_contact_name: Some("未知联系人".to_string()),
            sender_id: "sender-unknown-channel".to_string(),
            sender_name: "联系人".to_string(),
            sender_avatar_url: None,
            platform_message_id: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
            session: SessionSelector {
                api_config_id: None,
                agent_id: String::new(),
                conversation_id: None,
            },
            payload: ChatInputPayload {
                text: Some("不应绕过渠道白名单".to_string()),
                display_text: None,
                parts: None,
                images: None,
                audios: None,
                attachments: None,
                model: None,
                extra_text_blocks: None,
                mentions: None,
                provider_meta: None,
            },
        };

        let result = remote_im_enqueue_message_internal(input, &state)
            .await
            .expect("permission snapshot failure should skip without aborting caller");

        assert!(result.event_id.is_empty());
        assert!(result.conversation_id.is_empty());
        assert!(result.contact_id.is_empty());
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn stale_group_settlement_should_not_overwrite_newer_delivery_marker() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact(
            "contact-stale-settlement",
            "conversation-stale-settlement",
        );
        contact.remote_contact_type = "group".to_string();
        let newer_marker = RemoteImGroupReplyDeliveryMarker {
            generation: 12,
            boundary_message_id: "message-new".to_string(),
            outbound_key: "group-reply::new".to_string(),
            final_text: "新批次".to_string(),
            status: "dispatching".to_string(),
            platform_message_id: None,
            energy_applied: false,
            updated_at: Some(now_iso()),
        };
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                energy: Some(100.0),
                energy_updated_at: Some(now_iso()),
                group_reply_delivery: Some(newer_marker.clone()),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("write checkpoint");

        remote_im_persist_group_reply_settlement(
            &state,
            &contact,
            &RemoteImGroupReplySettlement {
                boundary_message_id: "message-old".to_string(),
                final_text: Some("旧批次迟到".to_string()),
                outbound_key: Some("group-reply::old".to_string()),
                platform_message_id: Some("platform-old".to_string()),
                status: RemoteImGroupReplySettlementStatus::Delivered,
            },
        )
        .expect("stale settlement should degrade without failure");

        let checkpoint = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read checkpoint")
            .expect("checkpoint exists");
        assert_eq!(checkpoint.energy, Some(100.0));
        assert_eq!(checkpoint.last_success_reply_at, None);
        let persisted_marker = checkpoint.group_reply_delivery.expect("newer marker");
        assert_eq!(persisted_marker.outbound_key, newer_marker.outbound_key);
        assert_eq!(persisted_marker.generation, newer_marker.generation);
        assert_eq!(persisted_marker.status, "dispatching");
    }

    #[test]
    fn active_group_delivery_should_not_be_recovered_as_uncertain_on_new_inbound() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-active-delivery", "conversation-active-delivery");
        contact.remote_contact_type = "group".to_string();
        let outbound_key =
            "group-reply::contact-active-delivery::9::message-active".to_string();
        let marker = RemoteImGroupReplyDeliveryMarker {
            generation: 9,
            boundary_message_id: "message-active".to_string(),
            outbound_key: outbound_key.clone(),
            final_text: "正在发送".to_string(),
            status: "dispatching".to_string(),
            platform_message_id: None,
            energy_applied: false,
            updated_at: Some(now_iso()),
        };
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                energy: Some(100.0),
                energy_updated_at: Some(now_iso()),
                group_reply_delivery: Some(marker.clone()),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("write checkpoint");
        let event = create_pending_event(
            "event-active-delivery".to_string(),
            "conversation-active-delivery".to_string(),
            vec![remote_im_test_group_user_message("user-a")],
            true,
            ChatSessionInfo {
                agent_id: "agent-a".to_string(),
            },
            RemoteImMessageSource {
                channel_id: "channel-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                im_name: "QQ".to_string(),
                remote_contact_type: "group".to_string(),
                remote_contact_id: "group-active".to_string(),
                remote_contact_name: "活跃发送群".to_string(),
                sender_id: "user-a".to_string(),
                sender_name: "user-a".to_string(),
                sender_avatar_url: None,
                platform_message_id: None,
            },
        );
        let state_key = remote_im_group_reply_state_key(&state, &contact.id);
        lock_remote_im_group_reply_state_store().by_contact.insert(
            state_key.clone(),
            RemoteImGroupReplyState {
                generation: 9,
                phase: RemoteImGroupReplyPhase::AssistantDispatching,
                start_message_id: "message-active".to_string(),
                decision_end_message_id: Some("message-active".to_string()),
                focus: false,
                energy_settled: false,
                next_round_mention: false,
                event,
                due_at: std::time::Instant::now(),
                inspection_kind: RemoteImGroupReplyTimerKind::Mention,
                pending_settlement: None,
            },
        );

        remote_im_recover_group_reply_delivery_marker(&state, &contact)
            .expect("active delivery should be ignored by recovery");
        let before_success = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read checkpoint")
            .expect("checkpoint exists");
        assert_eq!(before_success.energy, Some(100.0));
        assert_eq!(
            before_success.group_reply_delivery.as_ref().map(|item| item.status.as_str()),
            Some("dispatching")
        );

        remote_im_group_reply_complete_after_send(
            &state,
            &contact,
            9,
            marker,
            Some("platform-active".to_string()),
            RemoteImGroupReplySettlementStatus::Delivered,
        );
        let after_success = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read checkpoint")
            .expect("checkpoint exists");
        assert!(after_success.energy.unwrap_or_default() < 100.0);
        assert_eq!(
            after_success.group_reply_delivery.as_ref().map(|item| item.status.as_str()),
            Some("committed")
        );
        assert!(after_success
            .group_reply_delivery
            .as_ref()
            .map(|item| item.energy_applied)
            .unwrap_or(false));
        lock_remote_im_group_reply_state_store()
            .by_contact
            .remove(&state_key);
    }

    #[test]
    fn startup_group_delivery_recovery_should_not_wait_for_new_inbound() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-startup-recovery", "conversation-startup-recovery");
        contact.remote_contact_type = "group".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                energy: Some(100.0),
                energy_updated_at: Some(now_iso()),
                group_reply_delivery: Some(RemoteImGroupReplyDeliveryMarker {
                    generation: 19,
                    boundary_message_id: "message-startup".to_string(),
                    outbound_key: "group-reply::startup::19".to_string(),
                    final_text: "启动恢复".to_string(),
                    status: "dispatching".to_string(),
                    platform_message_id: None,
                    energy_applied: false,
                    updated_at: Some(now_iso()),
                }),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("write checkpoint");

        assert_eq!(
            remote_im_recover_all_group_reply_delivery_markers(&state)
                .expect("startup recovery"),
            (1, 0)
        );
        let checkpoint = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read checkpoint")
            .expect("checkpoint exists");
        assert_eq!(
            checkpoint.group_reply_delivery.as_ref().map(|marker| marker.status.as_str()),
            Some("uncertain")
        );
        assert_eq!(
            checkpoint.last_boundary_covers_message_id.as_deref(),
            Some("message-startup")
        );
    }

    #[test]
    fn contact_runtime_state_lock_should_recover_after_poison() {
        let state = remote_im_test_state();
        let shared = state.remote_im_contact_runtime_states.clone();
        let _ = std::thread::spawn(move || {
            let _guard = shared.lock().expect("lock before poison");
            panic!("poison contact runtime state lock");
        })
        .join();
        let mut recovered = lock_remote_im_contact_runtime_states(&state)
            .expect("poisoned lock should recover");
        recovered.insert("contact-recovered".to_string(), RemoteImContactRuntimeState::default());
        assert!(recovered.contains_key("contact-recovered"));
    }

    #[test]
    fn remote_im_list_contacts_inner_should_keep_canonical_sort_order() {
        let state = remote_im_test_state();
        let mut first = remote_im_test_contact("contact-b", "conversation-b");
        first.channel_id = "channel-b".to_string();
        first.last_message_at = Some("2026-07-14T08:00:00Z".to_string());
        let mut second = remote_im_test_contact("contact-c", "conversation-c");
        second.channel_id = "channel-a".to_string();
        second.last_message_at = Some("2026-07-14T08:00:00Z".to_string());
        let mut third = remote_im_test_contact("contact-a", "conversation-a");
        third.channel_id = "channel-a".to_string();
        third.last_message_at = Some("2026-07-14T09:00:00Z".to_string());
        state_service_upsert_remote_im_contact(&state, &first).expect("write first contact");
        state_service_upsert_remote_im_contact(&state, &second).expect("write second contact");
        state_service_upsert_remote_im_contact(&state, &third).expect("write third contact");

        let contacts = remote_im_list_contacts_inner(&state).expect("list contacts");

        assert_eq!(
            contacts.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(),
            vec!["contact-a", "contact-c", "contact-b"]
        );
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn build_remote_im_secretary_prepared_prompt_should_include_boundary_and_latest_marker() {
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.remote_contact_type = "group".to_string();
        contact.remote_contact_id = "group-88".to_string();
        contact.remote_contact_name = "项目群".to_string();
        let current_assistant = remote_im_test_secretary_assistant_context();
        let history_messages = vec![
            RemoteImSecretaryMessageDigest {
                time_text: "2026-06-28 10:00:00".to_string(),
                speaker: "群友 张三/user-7".to_string(),
                text: "这个报价我先看一下".to_string(),
            },
            RemoteImSecretaryMessageDigest {
                time_text: "2026-06-28 10:01:00".to_string(),
                speaker: "售前助理".to_string(),
                text: "好的，有问题随时提".to_string(),
            },
        ];
        let new_batch_messages = vec![
            RemoteImSecretaryMessageDigest {
                time_text: "2026-06-28 10:02:00".to_string(),
                speaker: "群友 李四/user-8".to_string(),
                text: "交期今天能不能定".to_string(),
            },
            RemoteImSecretaryMessageDigest {
                time_text: "2026-06-28 10:03:00".to_string(),
                speaker: "群友 张三/user-7".to_string(),
                text: "老板现在就等结论".to_string(),
            },
        ];

        let prompt = build_remote_im_secretary_prepared_prompt(
            "简体中文",
            &contact,
            &default_remote_im_contact_response_guidance(),
            &current_assistant,
            &history_messages,
            &new_batch_messages,
            "- [运行中] 委托 ID：delegate-a；任务：\"交期确认\"",
        );

        assert!(prompt.latest_user_text.contains("当前助理："));
        assert!(prompt.latest_user_text.contains("名称：售前助理"));
        assert!(prompt.latest_user_text.contains("当前联系人："));
        assert!(prompt.latest_user_text.contains("名称：项目群"));
        assert!(!prompt.latest_user_text.contains("当前人格："));
        assert!(prompt.latest_user_text.contains("最近 7 条已处理历史消息"));
        assert!(prompt
            .latest_user_text
            .contains("================ 未处理边界 ================"));
        assert!(prompt.latest_user_text.contains("最后一条是最新消息"));
        assert!(prompt.latest_user_text.contains("助理工作账本："));
        assert!(prompt.latest_user_text.contains("委托 ID：delegate-a"));
        assert!(prompt
            .latest_user_text
            .contains("[群友 张三/user-7](2026-06-28 10:03:00)（最新）：老板现在就等结论"));
    }


    #[test]
    fn remote_im_prepare_enqueue_runtime_state_should_mute_and_block_when_mute_keyword_matched() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.remote_contact_type = "group".to_string();
        contact.mute_keywords = vec!["闭嘴".to_string()];
        contact.unmute_keywords = vec!["张嘴".to_string()];
        contact.mute_duration_seconds = 600;

        let (activate_assistant, reason) =
            remote_im_prepare_enqueue_runtime_state(&state, &contact, "现在闭嘴")
                .expect("prepare runtime state");

        assert!(!activate_assistant);
        assert!(reason.contains("闭嘴词"));
        let runtime_states =
            lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
        let runtime = runtime_states.get("contact-a").expect("runtime exists");
        assert!(runtime.mute_until.is_some());
        assert_eq!(runtime.presence_state, RemoteImPresenceState::Away);
    }

    #[test]
    fn mute_keyword_should_abort_active_reply_delegates_for_contact() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.remote_contact_type = "group".to_string();
        contact.mute_keywords = vec!["闭嘴".to_string()];
        contact.mute_duration_seconds = 600;
        lock_remote_im_reply_delegate_runtimes(&state)
            .expect("lock delegate runtimes")
            .insert(
                "delegate-muted".to_string(),
                RemoteImReplyDelegateRuntime {
                    delegate_id: "delegate-muted".to_string(),
                    contact_id: contact.id.clone(),
                    conversation_id: "conversation-a".to_string(),
                    trigger_message_id: "trigger-a".to_string(),
                    started_at: now_iso(),
                    prompt_snapshot_messages: vec![remote_im_test_group_user_message("user-a")],
                    guidance_messages: std::collections::VecDeque::new(),
                    consumed_guidance_messages: Vec::new(),
                    cancelled: false,
                    terminal: false,
                    session_agent_id: "agent-a".to_string(),
                    inspection_generation: None,
                    group_reply_focus: false,
                    group_reply_max_chars: None,
                },
            );

        let (activate_assistant, reason) =
            remote_im_prepare_enqueue_runtime_state(&state, &contact, "现在闭嘴")
                .expect("prepare runtime state");

        assert!(!activate_assistant);
        assert!(reason.contains("闭嘴词"));
        assert!(!remote_im_reply_delegate_is_active(&state, "delegate-muted"));
        assert!(
            remote_im_reply_delegate_active_ids_for_contact(&state, &contact.id)
                .expect("list active delegates")
                .is_empty()
        );
    }

    #[test]
    fn remote_im_contact_is_muted_should_expire_and_clear_mute_until() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-a", "conversation-a");
        {
            let mut states = lock_remote_im_contact_runtime_states(&state).expect("lock states");
            remote_im_contact_runtime_state_mut(&mut states, &contact.id).mute_until =
                Some(remote_im_resolve_mute_until(now_utc() - time::Duration::seconds(5), 1));
        }

        assert!(!remote_im_contact_is_muted(&state, &contact.id).expect("check mute"));
        assert!(
            lock_remote_im_contact_runtime_states(&state)
                .expect("lock states")
                .get(&contact.id)
                .and_then(|runtime| runtime.mute_until.as_ref())
                .is_none()
        );
    }

    #[test]
    fn remote_im_prepare_enqueue_runtime_state_should_schedule_unmentioned_message_when_present() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.remote_contact_type = "group".to_string();
        contact.activation_mode = "keyword".to_string();
        contact.activation_keywords = vec!["@助理".to_string()];
        {
            let mut runtime_states =
                lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
            let runtime = remote_im_contact_runtime_state_mut(&mut runtime_states, &contact.id);
            runtime.presence_state = RemoteImPresenceState::Present;
            runtime.work_state = RemoteImWorkState::Idle;
            runtime.has_pending = false;
            runtime.last_success_reply_at = Some(now_iso());
            runtime.last_presence_at = Some("2000-01-01T00:00:00Z".to_string());
        }

        let (activate_assistant, reason) =
            remote_im_prepare_enqueue_runtime_state(&state, &contact, "普通新消息")
                .expect("prepare runtime state");

        assert!(activate_assistant);
        assert!(reason.contains("联系人仍在场"));
        let runtime_states =
            lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
        let runtime = runtime_states.get("contact-a").expect("runtime exists");
        assert_eq!(runtime.presence_state, RemoteImPresenceState::Present);
        assert_eq!(runtime.work_state, RemoteImWorkState::Idle);
        assert!(!runtime.has_pending);
        assert_ne!(
            runtime.last_presence_at.as_deref(),
            Some("2000-01-01T00:00:00Z")
        );
    }

    #[test]
    fn remote_im_prepare_enqueue_runtime_state_should_only_require_keyword_while_away() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-entry", "conversation-entry");
        contact.remote_contact_type = "group".to_string();
        contact.activation_mode = "keyword".to_string();
        contact.activation_keywords = vec!["fairy".to_string()];

        let (ordinary_activates, ordinary_reason) =
            remote_im_prepare_enqueue_runtime_state(&state, &contact, "普通新消息")
                .expect("prepare ordinary message while away");
        assert!(!ordinary_activates);
        assert!(ordinary_reason.contains("未命中点名词"));

        let (mention_activates, mention_reason) =
            remote_im_prepare_enqueue_runtime_state(&state, &contact, "fairy 看一下")
                .expect("prepare mentioned message while away");
        assert!(mention_activates);
        assert!(mention_reason.contains("命中点名词"));
        let runtime_states =
            lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
        let runtime = runtime_states
            .get(&contact.id)
            .expect("entry runtime exists");
        assert_eq!(runtime.presence_state, RemoteImPresenceState::Present);
        assert!(runtime.last_presence_at.is_some());
    }

    #[test]
    fn private_contact_should_bypass_presence_mute_and_secretary_state() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-private", "conversation-private");
        contact.remote_contact_type = "private".to_string();
        contact.activation_mode = "never".to_string();
        contact.mute_keywords = vec!["闭嘴".to_string()];

        let (activate_assistant, reason) =
            remote_im_prepare_enqueue_runtime_state(&state, &contact, "闭嘴")
                .expect("prepare private runtime state");

        assert!(activate_assistant);
        assert!(reason.contains("直接调度绑定会话"));
        assert!(!lock_remote_im_contact_runtime_states(&state)
            .expect("lock runtime states")
            .contains_key(&contact.id));
    }

    #[test]
    fn remote_reply_delegate_should_finish_atomically_with_empty_guidance_queue() {
        let state = remote_im_test_state();
        let register = |delegate_id: &str| {
            lock_remote_im_reply_delegate_runtimes(&state)
                .expect("lock delegate runtimes")
                .insert(
                    delegate_id.to_string(),
                    RemoteImReplyDelegateRuntime {
                        delegate_id: delegate_id.to_string(),
                        contact_id: "contact-a".to_string(),
                        conversation_id: "conversation-a".to_string(),
                        trigger_message_id: "trigger-a".to_string(),
                        started_at: now_iso(),
                        prompt_snapshot_messages: vec![remote_im_test_group_user_message("user-a")],
                        guidance_messages: std::collections::VecDeque::new(),
                        consumed_guidance_messages: Vec::new(),
                        cancelled: false,
                        terminal: false,
                        session_agent_id: "agent-a".to_string(),
                        inspection_generation: None,
                        group_reply_focus: false,
                        group_reply_max_chars: None,
                    },
                );
            delegate_id.to_string()
        };
        let delegate_id = register("delegate-a");
        assert_eq!(
            remote_im_reply_delegate_prompt_messages(&state, &delegate_id)
                .expect("read frozen prompt")
                .len(),
            1
        );

        let first_take = remote_im_reply_delegate_take_guidance_or_finish(&state, &delegate_id)
            .expect("take empty guidance");
        assert!(matches!(
            first_take,
            RemoteImReplyDelegateNext::Completed(runtime) if runtime.delegate_id == delegate_id
        ));
        assert!(!remote_im_reply_delegate_is_active(&state, &delegate_id));
        assert!(remote_im_reply_delegate_enqueue_guidance(
            &state,
            &delegate_id,
            remote_im_test_group_user_message("user-a"),
            None,
        )
        .is_err());

        let next_delegate_id = register("delegate-b");
        remote_im_reply_delegate_enqueue_guidance(
            &state,
            &next_delegate_id,
            remote_im_test_group_user_message("user-b"),
            Some(RemoteImGroupReplyDispatchPolicy {
                generation: 42,
                focus: false,
                max_chars: 33,
            }),
        )
        .expect("enqueue guidance before finish");
        let guidance = remote_im_reply_delegate_take_guidance_or_finish(&state, &next_delegate_id)
            .expect("take queued guidance");
        let guidance = match guidance {
            RemoteImReplyDelegateNext::Guidance(messages) => messages,
            _ => panic!("delegate should remain active for queued guidance"),
        };
        assert_eq!(guidance.len(), 1);
        assert!(guidance[0]
            .extra_text_blocks
            .first()
            .map(|block| block.contains("33"))
            .unwrap_or(false));
        assert!(guidance[0].extra_text_blocks.len() >= 2);
        let (_, policy_snapshot) = remote_im_reply_delegate_group_policy(&state, &next_delegate_id)
            .expect("group policy snapshot");
        assert_eq!(policy_snapshot.generation, 42);
        assert_eq!(policy_snapshot.max_chars, 33);
        assert_eq!(
            remote_im_reply_delegate_prompt_messages(&state, &next_delegate_id)
                .expect("read prompt with consumed guidance")
                .len(),
            2
        );
        assert!(matches!(
            remote_im_reply_delegate_take_guidance_or_finish(&state, &next_delegate_id)
                .expect("finish after guidance"),
            RemoteImReplyDelegateNext::Completed(runtime) if runtime.delegate_id == next_delegate_id
        ));
    }

    #[tokio::test]
    async fn presence_timeout_should_not_depart_while_reply_delegate_is_active() {
        let state = remote_im_test_state();
        let presence_at = now_iso();
        lock_remote_im_contact_runtime_states(&state)
            .expect("lock contact states")
            .insert(
                "contact-a".to_string(),
                RemoteImContactRuntimeState {
                    presence_state: RemoteImPresenceState::Present,
                    last_presence_at: Some(presence_at),
                    ..RemoteImContactRuntimeState::default()
                },
            );
        lock_remote_im_reply_delegate_runtimes(&state)
            .expect("lock delegate runtimes")
            .insert(
                "delegate-a".to_string(),
                RemoteImReplyDelegateRuntime {
                    delegate_id: "delegate-a".to_string(),
                    contact_id: "contact-a".to_string(),
                    conversation_id: "conversation-a".to_string(),
                    trigger_message_id: "trigger-a".to_string(),
                    started_at: now_iso(),
                    prompt_snapshot_messages: vec![remote_im_test_group_user_message("user-a")],
                    guidance_messages: std::collections::VecDeque::new(),
                    consumed_guidance_messages: Vec::new(),
                    cancelled: false,
                    terminal: false,
                    session_agent_id: "agent-a".to_string(),
                    inspection_generation: None,
                    group_reply_focus: false,
                    group_reply_max_chars: None,
                },
            );

        remote_im_schedule_presence_timeout(&state, "contact-a", 0)
            .expect("schedule timeout");
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        assert!(!remote_im_contact_is_away(&state, "contact-a").expect("read presence"));
    }

    #[test]
    fn group_reply_state_should_keep_entry_inspection_level_for_later_messages() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.remote_contact_type = "group".to_string();
        contact.activation_mode = "keyword".to_string();
        contact.activation_keywords = vec!["@助理".to_string()];
        contact.group_reply_pacing.assistant_debounce_seconds = 3600;
        contact.group_reply_pacing.secretary_inspection_seconds = 3600;
        let event = |id: &str, sender_id: &str, text: &str| {
            let mut message = remote_im_test_group_user_message(sender_id);
            message.id = format!("message-{id}");
            message.parts = vec![MessagePart::Text {
                text: text.to_string(),
                reasoning_content: None,
            }];
            create_pending_event(
                format!("event-{id}"),
                "conversation-a".to_string(),
                vec![message],
                true,
                ChatSessionInfo {
                    agent_id: "agent-a".to_string(),
                },
                RemoteImMessageSource {
                    channel_id: "channel-a".to_string(),
                    platform: RemoteImPlatform::OnebotV11,
                    im_name: "QQ".to_string(),
                    remote_contact_type: "group".to_string(),
                    remote_contact_id: "group-88".to_string(),
                    remote_contact_name: "项目群".to_string(),
                    sender_id: sender_id.to_string(),
                    sender_name: sender_id.to_string(),
                    sender_avatar_url: None,
                    platform_message_id: None,
                },
            )
        };

        observe_remote_im_persisted_event(&state, &contact, &event("normal", "user-a", "普通消息"));
        let key = remote_im_group_reply_state_key(&state, &contact.id);
        let (initial_generation, initial_due_at) = {
            let store = remote_im_group_reply_state_store().lock().expect("lock group state");
            let entry = store.by_contact.get(&key).expect("non mention scheduled");
            assert_eq!(entry.phase, RemoteImGroupReplyPhase::NonMentionScheduled);
            assert_eq!(entry.start_message_id, "message-normal");
            assert!(entry.event.messages.is_empty());
            (entry.generation, entry.due_at)
        };
        let (activate_assistant, reason) =
            remote_im_prepare_enqueue_runtime_state(&state, &contact, "未点名的后续消息")
                .expect("prepare active batch");
        assert!(activate_assistant);
        assert!(reason.contains("当前批次已入场"), "reason={reason}");
        observe_remote_im_persisted_event(&state, &contact, &event("normal-2", "user-b", "普通跟话"));
        {
            let store = remote_im_group_reply_state_store().lock().expect("lock group state");
            let entry = store.by_contact.get(&key).expect("same inspection");
            assert_eq!(entry.generation, initial_generation);
            assert_eq!(entry.due_at, initial_due_at);
            assert!(entry.decision_end_message_id.is_none());
        }
        observe_remote_im_persisted_event(&state, &contact, &event("wake", "user-b", "@助理 看这里"));
        {
            let store = remote_im_group_reply_state_store().lock().expect("lock group state");
            let entry = store.by_contact.get(&key).expect("same inspection");
            assert_eq!(entry.phase, RemoteImGroupReplyPhase::MentionScheduled);
            assert!(entry.generation > initial_generation);
            assert_ne!(entry.due_at, initial_due_at);
            assert_eq!(entry.start_message_id, "message-normal");
            assert!(entry.decision_end_message_id.is_none());
        }
        let current_generation = lock_remote_im_group_reply_state_store()
            .by_contact
            .get(&key)
            .map(|entry| entry.generation)
            .expect("current generation");
        assert!(remote_im_group_reply_generation_is_current(
            &state,
            &contact.id,
            current_generation,
        ));
    }

    #[test]
    fn group_reply_missing_contact_should_stop_batch_retry() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-retry", "conversation-retry");
        let mut message = remote_im_test_group_user_message("user-a");
        message.id = "message-retry-start".to_string();
        let event = create_pending_event(
            "event-retry".to_string(),
            "conversation-retry".to_string(),
            vec![message],
            true,
            ChatSessionInfo {
                agent_id: "agent-a".to_string(),
            },
            RemoteImMessageSource {
                channel_id: "channel-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                im_name: "QQ".to_string(),
                remote_contact_type: "group".to_string(),
                remote_contact_id: "group-retry".to_string(),
                remote_contact_name: "重试群".to_string(),
                sender_id: "user-a".to_string(),
                sender_name: "user-a".to_string(),
                sender_avatar_url: None,
                platform_message_id: None,
            },
        );
        let key = remote_im_group_reply_state_key(&state, &contact.id);
        let generation = {
            let mut store = lock_remote_im_group_reply_state_store();
            let generation = remote_im_group_reply_next_generation(&mut store);
            store.by_contact.insert(
                key.clone(),
                RemoteImGroupReplyState {
                generation,
                phase: RemoteImGroupReplyPhase::AssistantDispatching,
                start_message_id: "message-retry-start".to_string(),
                decision_end_message_id: Some("message-retry-start".to_string()),
                focus: false,
                energy_settled: false,
                next_round_mention: false,
                event,
                due_at: std::time::Instant::now(),
                inspection_kind: RemoteImGroupReplyTimerKind::Mention,
                pending_settlement: None,
                },
            );
            generation
        };

        // 联系人未写入 state 数据库（已删除）：重试应立即停止并清理批次
        remote_im_group_reply_retry_after_dispatch_failure(
            &state,
            &contact.id,
            generation,
            "联系人已删除",
        );
        let store = lock_remote_im_group_reply_state_store();
        assert!(
            store.by_contact.get(&key).is_none(),
            "batch should be cleared when contact is gone"
        );
    }

    #[test]
    fn mute_should_clear_pending_group_reply_state() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-a", "conversation-a");
        contact.activation_mode = "keyword".to_string();
        contact.activation_keywords = vec!["@助理".to_string()];
        let mut message = remote_im_test_group_user_message("user-a");
        message.parts = vec![MessagePart::Text {
            text: "@助理 回答".to_string(),
            reasoning_content: None,
        }];
        let event = create_pending_event(
            "event-wake".to_string(),
            "conversation-a".to_string(),
            vec![message],
            true,
            ChatSessionInfo {
                agent_id: "agent-a".to_string(),
            },
            RemoteImMessageSource {
                channel_id: "channel-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                im_name: "QQ".to_string(),
                remote_contact_type: "group".to_string(),
                remote_contact_id: "group-88".to_string(),
                remote_contact_name: "项目群".to_string(),
                sender_id: "user-a".to_string(),
                sender_name: "张三".to_string(),
                sender_avatar_url: None,
                platform_message_id: None,
            },
        );
        contact.group_reply_pacing.assistant_debounce_seconds = 3600;
        observe_remote_im_persisted_event(&state, &contact, &event);
        {
            let mut states = lock_remote_im_contact_runtime_states(&state).expect("lock states");
            remote_im_contact_runtime_state_mut(&mut states, &contact.id).mute_until =
                Some(remote_im_resolve_mute_until(now_utc(), 60));
        }
        observe_remote_im_persisted_event(&state, &contact, &event);
        let key = remote_im_group_reply_state_key(&state, &contact.id);
        let store = remote_im_group_reply_state_store().lock().expect("lock group state");
        assert!(!store.by_contact.contains_key(&key));
    }

    #[test]
    fn channel_behavior_should_be_shared_per_channel_and_ignore_legacy_contact_values() {
        let state = remote_im_test_state();
        let mut behavior_a = RemoteImChannelBehaviorSettings::default();
        behavior_a.response_guidance = "渠道 A 规则".to_string();
        behavior_a.group_reply_pacing.assistant_debounce_seconds = 31;
        behavior_a.group_reply_pacing.maximum_energy = 73.0;
        let mut behavior_b = RemoteImChannelBehaviorSettings::default();
        behavior_b.response_guidance = "渠道 B 规则".to_string();
        behavior_b.group_reply_pacing.assistant_debounce_seconds = 47;
        behavior_b.group_reply_pacing.maximum_energy = 29.0;
        state_write_config_cached(
            &state,
            &AppConfig {
                remote_im_channels: vec![
                    remote_im_test_channel("channel-a", behavior_a),
                    remote_im_test_channel("channel-b", behavior_b),
                ],
                ..AppConfig::default()
            },
        )
        .expect("write channel behavior config");

        let mut first = remote_im_test_contact("contact-a-1", "conversation-a-1");
        first.remote_contact_type = "group".to_string();
        first.response_guidance = "联系人旧规则 A".to_string();
        first.group_reply_pacing.assistant_debounce_seconds = 1;
        first.group_reply_pacing.maximum_energy = 1.0;
        let mut second = remote_im_test_contact("contact-a-2", "conversation-a-2");
        second.remote_contact_type = "group".to_string();
        second.response_guidance = "联系人旧规则 B".to_string();
        second.group_reply_pacing.assistant_debounce_seconds = 2;
        second.group_reply_pacing.maximum_energy = 2.0;
        let mut other_channel = remote_im_test_contact("contact-b-1", "conversation-b-1");
        other_channel.channel_id = "channel-b".to_string();
        other_channel.remote_contact_type = "group".to_string();
        other_channel.response_guidance = "联系人旧规则 C".to_string();

        let first_pacing = effective_remote_im_group_reply_pacing(&state, &first);
        let second_pacing = effective_remote_im_group_reply_pacing(&state, &second);
        let other_pacing = effective_remote_im_group_reply_pacing(&state, &other_channel);
        assert_eq!(first_pacing.assistant_debounce_seconds, 31);
        assert_eq!(second_pacing.assistant_debounce_seconds, 31);
        assert_eq!(first_pacing.maximum_energy, 73.0);
        assert_eq!(other_pacing.assistant_debounce_seconds, 47);
        assert_eq!(other_pacing.maximum_energy, 29.0);
        assert_eq!(
            effective_remote_im_channel_response_guidance(&state, &first),
            "渠道 A 规则"
        );
        assert_eq!(
            effective_remote_im_channel_response_guidance(&state, &second),
            "渠道 A 规则"
        );
        assert_eq!(
            effective_remote_im_channel_response_guidance(&state, &other_channel),
            "渠道 B 规则"
        );
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn channel_behavior_energy_ledger_should_remain_contact_scoped() {
        let state = remote_im_test_state();
        let mut behavior = RemoteImChannelBehaviorSettings::default();
        behavior.group_reply_pacing.positive_energy_phrases = vec!["谢谢".to_string()];
        behavior.group_reply_pacing.positive_energy_delta = 6.0;
        state_write_config_cached(
            &state,
            &AppConfig {
                remote_im_channels: vec![remote_im_test_channel("channel-a", behavior)],
                ..AppConfig::default()
            },
        )
        .expect("write channel behavior config");
        let mut first = remote_im_test_contact("contact-energy-a", "conversation-energy-a");
        first.remote_contact_type = "group".to_string();
        let mut second = remote_im_test_contact("contact-energy-b", "conversation-energy-b");
        second.remote_contact_type = "group".to_string();
        state_service_upsert_remote_im_contact(&state, &first).expect("write first contact");
        state_service_upsert_remote_im_contact(&state, &second).expect("write second contact");

        remote_im_apply_inbound_group_energy(&state, &first, "sender-a", "谢谢")
            .expect("settle first contact energy");
        let first_checkpoint = state_service_get_remote_im_contact_checkpoint(&state, &first.id)
            .expect("read first checkpoint")
            .expect("first checkpoint");
        assert!(first_checkpoint.energy.is_some());
        assert!(state_service_get_remote_im_contact_checkpoint(&state, &second.id)
            .expect("read second checkpoint")
            .is_none());
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn group_reply_gate_should_not_estimate_reply_cost_before_generation() {
        let state = remote_im_test_state();
        let mut behavior = RemoteImChannelBehaviorSettings::default();
        behavior.group_reply_pacing.base_reply_energy_cost = 100.0;
        behavior.group_reply_pacing.energy_cost_per_character = 100.0;
        behavior.group_reply_pacing.energy_recovery_per_second = 0.0;
        state_write_config_cached(
            &state,
            &AppConfig {
                remote_im_channels: vec![remote_im_test_channel("channel-a", behavior)],
                ..AppConfig::default()
            },
        )
        .expect("write channel behavior config");
        let mut contact = remote_im_test_contact("contact-energy-gate", "conversation-energy-gate");
        contact.remote_contact_type = "group".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                energy: Some(0.01),
                energy_updated_at: Some(now_iso()),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("write contact energy");

        let gate = remote_im_group_reply_gate(&state, &contact, false).expect("read reply gate");
        assert!(gate.allowed);
        assert_eq!(gate.max_chars, 20);
        assert!((gate.energy - 0.01).abs() < 0.01);
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn inspection_energy_should_count_same_positive_phrase_once_per_batch() {
        let state = remote_im_test_state();
        let mut behavior = RemoteImChannelBehaviorSettings::default();
        behavior.group_reply_pacing.positive_energy_phrases = vec!["谢谢".to_string()];
        behavior.group_reply_pacing.positive_energy_delta = 6.0;
        behavior.group_reply_pacing.energy_recovery_per_second = 0.0;
        state_write_config_cached(
            &state,
            &AppConfig {
                remote_im_channels: vec![remote_im_test_channel("channel-a", behavior)],
                ..AppConfig::default()
            },
        )
        .expect("write channel behavior config");
        let mut contact = remote_im_test_contact("contact-energy-batch", "conversation-energy-batch");
        contact.remote_contact_type = "group".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                energy: Some(0.0),
                energy_updated_at: Some(now_iso()),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("write contact");
        let mut first = remote_im_test_group_user_message("sender-a");
        first.id = "energy-batch-1".to_string();
        first.parts = vec![MessagePart::Text {
            text: "谢谢，收到".to_string(),
            reasoning_content: None,
        }];
        let mut second = remote_im_test_group_user_message("sender-b");
        second.id = "energy-batch-2".to_string();
        second.parts = vec![MessagePart::Text {
            text: "再次谢谢".to_string(),
            reasoning_content: None,
        }];
        remote_im_apply_group_energy_for_messages(&state, &contact, &[first, second])
            .expect("settle inspection batch energy");
        let checkpoint = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read batch checkpoint")
            .expect("checkpoint exists");
        assert_eq!(checkpoint.energy, Some(6.0));
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn inspection_negative_energy_should_stop_at_negative_maximum() {
        let state = remote_im_test_state();
        let mut behavior = RemoteImChannelBehaviorSettings::default();
        behavior.group_reply_pacing.negative_energy_phrases = vec!["烦".to_string()];
        behavior.group_reply_pacing.negative_energy_delta = -15.0;
        state_write_config_cached(
            &state,
            &AppConfig {
                remote_im_channels: vec![remote_im_test_channel("channel-a", behavior)],
                ..AppConfig::default()
            },
        )
        .expect("write channel behavior config");
        let mut contact = remote_im_test_contact(
            "contact-negative-energy-batch",
            "conversation-negative-energy-batch",
        );
        contact.remote_contact_type = "group".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                energy: Some(-95.0),
                energy_updated_at: Some(now_iso()),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("write contact");
        let mut message = remote_im_test_group_user_message("sender-a");
        message.id = "negative-energy-batch-1".to_string();
        message.parts = vec![MessagePart::Text {
            text: "太烦了".to_string(),
            reasoning_content: None,
        }];

        remote_im_apply_group_energy_for_messages(&state, &contact, &[message])
            .expect("settle negative inspection energy");
        let checkpoint = state_service_get_remote_im_contact_checkpoint(&state, &contact.id)
            .expect("read negative checkpoint")
            .expect("checkpoint exists");
        assert_eq!(checkpoint.energy, Some(-100.0));
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn contact_dashboard_snapshot_should_use_backend_energy_presence_and_watermark() {
        let state = remote_im_test_state();
        let mut behavior = RemoteImChannelBehaviorSettings::default();
        behavior.group_reply_pacing.maximum_energy = 100.0;
        behavior.group_reply_pacing.energy_recovery_per_second = 0.0;
        state_write_config_cached(
            &state,
            &AppConfig {
                remote_im_channels: vec![remote_im_test_channel("channel-a", behavior)],
                ..AppConfig::default()
            },
        )
        .expect("write channel behavior config");
        let mut contact = remote_im_test_contact("contact-dashboard", "conversation-dashboard");
        contact.remote_contact_type = "group".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        state_service_set_remote_im_contact_checkpoint(
            &state,
            &RemoteImContactCheckpoint {
                contact_id: contact.id.clone(),
                atomic_revision: 7,
                energy: Some(-25.0),
                energy_updated_at: Some(now_iso()),
                ..RemoteImContactCheckpoint::default()
            },
        )
        .expect("write dashboard checkpoint");
        lock_remote_im_contact_runtime_states(&state)
            .expect("lock dashboard runtime")
            .insert(
                contact.id.clone(),
                RemoteImContactRuntimeState {
                    presence_state: RemoteImPresenceState::Present,
                    last_presence_at: Some("2026-07-19T00:00:00Z".to_string()),
                    ..RemoteImContactRuntimeState::default()
                },
            );

        let snapshot = remote_im_contact_dashboard_snapshot_inner(&state, &contact.id)
            .expect("read dashboard snapshot");
        assert_eq!(snapshot.contact_id, contact.id);
        assert_eq!(snapshot.presence, "present");
        assert_eq!(snapshot.energy, -25.0);
        assert_eq!(snapshot.maximum_energy, 100.0);
        assert_eq!(snapshot.energy_percent, -25.0);
        assert!(snapshot.watermark.contains("checkpoint:7"));
        assert!(snapshot.watermark.contains("presence:present"));

        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn unreadable_channel_behavior_config_should_fall_back_without_interrupting_group_processing() {
        let state = remote_im_test_state();
        let mut contact = remote_im_test_contact("contact-fallback", "conversation-fallback");
        contact.remote_contact_type = "group".to_string();
        contact.group_reply_pacing.maximum_energy = 1.0;
        contact.response_guidance = "联系人旧规则".to_string();
        let pacing = effective_remote_im_group_reply_pacing(&state, &contact);
        assert_eq!(pacing, RemoteImGroupReplyPacing::default());
        assert_eq!(
            effective_remote_im_channel_response_guidance(&state, &contact),
            default_remote_im_contact_response_guidance()
        );
        assert!(!remote_im_group_reply_focus_matches(&state, &contact, "任意文本"));
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn remote_im_channel_behavior_save_should_invalidate_scheduled_group_generation() {
        let state = remote_im_test_state();
        let mut behavior = RemoteImChannelBehaviorSettings::default();
        behavior.group_reply_pacing.assistant_debounce_seconds = 3600;
        behavior.group_reply_pacing.secretary_inspection_seconds = 3600;
        state_write_config_cached(
            &state,
            &AppConfig {
                remote_im_channels: vec![remote_im_test_channel("channel-a", behavior)],
                ..AppConfig::default()
            },
        )
        .expect("write channel behavior config");
        let mut contact = remote_im_test_contact("contact-reconfigure", "conversation-reconfigure");
        contact.remote_contact_type = "group".to_string();
        state_service_upsert_remote_im_contact(&state, &contact).expect("write contact");
        let mut message = remote_im_test_group_user_message("user-a");
        message.id = "message-reconfigure".to_string();
        let event = create_pending_event(
            "event-reconfigure".to_string(),
            "conversation-reconfigure".to_string(),
            vec![message],
            true,
            ChatSessionInfo {
                agent_id: "agent-a".to_string(),
            },
            RemoteImMessageSource {
                channel_id: "channel-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                im_name: "QQ".to_string(),
                remote_contact_type: "group".to_string(),
                remote_contact_id: "group-reconfigure".to_string(),
                remote_contact_name: "重排群".to_string(),
                sender_id: "user-a".to_string(),
                sender_name: "用户A".to_string(),
                sender_avatar_url: None,
                platform_message_id: None,
            },
        );
        observe_remote_im_persisted_event(&state, &contact, &event);
        let key = remote_im_group_reply_state_key(&state, &contact.id);
        let previous_generation = lock_remote_im_group_reply_state_store()
            .by_contact
            .get(&key)
            .map(|entry| entry.generation)
            .expect("scheduled group state");

        let result = remote_im_reconfigure_channel_behavior_inner(&state, "channel-a");
        assert_eq!(result.reconfigured_contacts, 1);
        assert_eq!(result.skipped_contacts, 0);
        let mut store = lock_remote_im_group_reply_state_store();
        let reconfigured = store.by_contact.get(&key).expect("reconfigured group state");
        assert!(reconfigured.generation > previous_generation);
        assert_eq!(reconfigured.phase, RemoteImGroupReplyPhase::NonMentionScheduled);
        store.by_contact.remove(&key);
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }