    #[test]
    fn remote_im_upsert_contact_for_inbound_should_create_with_send_false_and_receive_follow_channel_activation() {
        let channel = RemoteImChannelConfig {
            id: "c1".to_string(),
            name: "qq".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            enabled: true,
            credentials: serde_json::json!({}),
            activate_assistant: true,
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            allow_send_files: false,
        };
        let mut runtime = RuntimeStateFile::default();
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
            activate_assistant: Some(true),
            session: SessionSelector {
                api_config_id: None,
                department_id: None,
                agent_id: "agent".to_string(),
                conversation_id: Some("conv-1".to_string()),
            },
            payload: ChatInputPayload {
                text: Some("hello".to_string()),
                display_text: None,
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
        let contact_id = remote_im_upsert_contact_for_inbound(&mut runtime, &channel, &input, &now);
        assert_eq!(runtime.remote_im_contacts.len(), 1);
        let contact = runtime
            .remote_im_contacts
            .iter()
            .find(|item| item.id == contact_id)
            .expect("contact exists");
        assert!(!contact.allow_send);
        assert!(contact.allow_receive);
        assert_eq!(contact.activation_mode, "never");
        assert!(contact.activation_keywords.is_empty());
        assert_eq!(contact.activation_cooldown_seconds, 0);

        // 第二次入队应复用同一联系人
        let now2 = now_iso();
        let contact_id_2 = remote_im_upsert_contact_for_inbound(&mut runtime, &channel, &input, &now2);
        assert_eq!(contact_id, contact_id_2);
        assert_eq!(runtime.remote_im_contacts.len(), 1);
    }

    #[test]
    fn remote_im_resolve_inbound_activate_should_prefer_message_level_flag() {
        let channel = RemoteImChannelConfig {
            id: "c1".to_string(),
            name: "qq".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            enabled: true,
            credentials: serde_json::json!({}),
            activate_assistant: false,
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            allow_send_files: false,
        };
        assert!(!remote_im_resolve_inbound_activate(&channel, None));
        assert!(remote_im_resolve_inbound_activate(&channel, Some(true)));
        assert!(!remote_im_resolve_inbound_activate(&channel, Some(false)));
    }

    #[test]
    fn resolve_conversation_id_should_route_remote_im_to_main_conversation() {
        let state = remote_im_test_state();
        let mut runtime = RuntimeStateFile::default();
        runtime.main_conversation_id = Some("conversation-main".to_string());
        let conversations = vec![
            Conversation {
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
                created_at: now_iso(),
                updated_at: now_iso(),
                last_user_at: None,
                last_assistant_at: None,
                status: "inactive".to_string(),
                summary: String::new(),
                user_profile_snapshot: String::new(),
                shell_workspace_path: None,
                shell_workspaces: Vec::new(),
                archived_at: None,
                messages: Vec::new(),
                current_todos: Vec::new(),
                memory_recall_table: Vec::new(),
                plan_mode_enabled: false,
            },
            Conversation {
                id: "conversation-sub".to_string(),
                title: "sub".to_string(),
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
                created_at: now_iso(),
                updated_at: now_iso(),
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
            activate_assistant: Some(true),
            session: SessionSelector {
                api_config_id: None,
                department_id: None,
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_id: Some("conversation-sub".to_string()),
            },
            payload: ChatInputPayload {
                text: Some("hello".to_string()),
                display_text: None,
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
            remark_name: String::new(),
            allow_send: false,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "never".to_string(),
            activation_keywords: Vec::new(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "main_session".to_string(),
            bound_department_id: None,
            bound_conversation_id: None,
            processing_mode: "continuous".to_string(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
        };
        state_write_runtime_state_cached(&state, &runtime).expect("write runtime state");
        for conversation in &conversations {
            state_write_conversation_cached(&state, conversation).expect("write conversation");
        }

        let config = AppConfig::default();
        let (_, _, conversation_id) =
            resolve_contact_session_target(&state, &config, &mut runtime, &mut contact)
                .expect("resolve route");

        assert_eq!(conversation_id, "conversation-main");
        let _ = std::fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn remote_im_should_still_route_to_main_after_user_switches_to_sub_conversation() {
        let state = remote_im_test_state();
        let mut runtime = RuntimeStateFile::default();
        runtime.main_conversation_id = Some("conversation-main".to_string());
        let conversations = vec![
            Conversation {
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
                created_at: now_iso(),
                updated_at: now_iso(),
                last_user_at: None,
                last_assistant_at: None,
                status: "inactive".to_string(),
                summary: String::new(),
                user_profile_snapshot: String::new(),
                shell_workspace_path: None,
                shell_workspaces: Vec::new(),
                archived_at: None,
                messages: Vec::new(),
                current_todos: Vec::new(),
                memory_recall_table: Vec::new(),
                plan_mode_enabled: false,
            },
            Conversation {
                id: "conversation-sub".to_string(),
                title: "sub".to_string(),
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
                created_at: now_iso(),
                updated_at: now_iso(),
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
            activate_assistant: Some(true),
            session: SessionSelector {
                api_config_id: None,
                department_id: None,
                agent_id: DEFAULT_AGENT_ID.to_string(),
                conversation_id: Some("conversation-sub".to_string()),
            },
            payload: ChatInputPayload {
                text: Some("hello".to_string()),
                display_text: None,
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
            remark_name: String::new(),
            allow_send: false,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "never".to_string(),
            activation_keywords: Vec::new(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "main_session".to_string(),
            bound_department_id: None,
            bound_conversation_id: None,
            processing_mode: "continuous".to_string(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
        };
        state_write_runtime_state_cached(&state, &runtime).expect("write runtime state");
        for conversation in &conversations {
            state_write_conversation_cached(&state, conversation).expect("write conversation");
        }

        let config = AppConfig::default();
        let (_, _, conversation_id) =
            resolve_contact_session_target(&state, &config, &mut runtime, &mut contact)
                .expect("resolve route");

        assert_eq!(conversation_id, "conversation-main");
        assert_eq!(runtime.main_conversation_id.as_deref(), Some("conversation-main"));
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
            department_id: String::new(),
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
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: vec![ChatMessage {
                id: "msg-1".to_string(),
                role: "user".to_string(),
                created_at: now_iso(),
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "hello".to_string(),
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
            }],
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        };

        assert!(conversation_has_remote_im_platform_message(
            &conversation,
            "c1",
            "private",
            "u1",
            "m1",
        ));
        assert!(!conversation_has_remote_im_platform_message(
            &conversation,
            "c1",
            "private",
            "u1",
            "m2",
        ));
    }

    #[test]
    fn conversation_has_remote_im_platform_message_should_ignore_legacy_origin_meta() {
        let conversation = Conversation {
            id: "conv-1".to_string(),
            title: "联系人".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: String::new(),
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
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: vec![ChatMessage {
                id: "msg-1".to_string(),
                role: "user".to_string(),
                created_at: now_iso(),
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "hello".to_string(),
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
            }],
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        };

        assert!(!conversation_has_remote_im_platform_message(
            &conversation,
            "c1",
            "private",
            "u1",
            "m1",
        ));
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
            activate_assistant: Some(true),
            session: SessionSelector {
                api_config_id: None,
                department_id: None,
                agent_id: String::new(),
                conversation_id: None,
            },
            payload: ChatInputPayload {
                text: Some("hello".to_string()),
                display_text: None,
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
        use aes::cipher::{generic_array::GenericArray, BlockEncrypt, KeyInit};

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
            let block = GenericArray::from_mut_slice(chunk);
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
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
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
            provider_streaming_disabled_keys: Arc::new(Mutex::new(std::collections::HashMap::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(std::collections::HashSet::new())),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            conversation_index_repair_gates: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(std::collections::HashMap::new())),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        }
    }

    fn remote_im_test_contact(contact_id: &str, conversation_id: &str) -> RemoteImContact {
        RemoteImContact {
            id: contact_id.to_string(),
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "private".to_string(),
            remote_contact_id: "remote-a".to_string(),
            remote_contact_name: "张三".to_string(),
            remark_name: String::new(),
            allow_send: true,
            allow_send_files: false,
            allow_receive: true,
            activation_mode: "keyword".to_string(),
            activation_keywords: vec!["派".to_string()],
            patience_seconds: default_remote_im_contact_patience_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_department_id: Some(REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID.to_string()),
            bound_conversation_id: Some(conversation_id.to_string()),
            processing_mode: "continuous".to_string(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
        }
    }

    fn remote_im_test_conversation(conversation_id: &str) -> Conversation {
        Conversation {
            id: conversation_id.to_string(),
            title: "联系人".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID.to_string(),
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

    #[test]
    fn remote_im_prepare_enqueue_runtime_state_should_activate_away_keyword_contact() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-a", "conversation-a");

        let (activate_assistant, reason) =
            remote_im_prepare_enqueue_runtime_state(&state, &contact, "派师傅帮我看看")
                .expect("prepare runtime state");

        assert!(activate_assistant);
        assert!(reason.contains("keyword"));
        let runtime_states = lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
        let runtime = runtime_states.get("contact-a").expect("runtime exists");
        assert_eq!(runtime.presence_state, RemoteImPresenceState::Present);
        assert_eq!(runtime.work_state, RemoteImWorkState::Idle);
        assert!(runtime.needs_boundary);
    }

    #[test]
    fn remote_im_prepare_enqueue_runtime_state_should_leave_present_when_patience_exhausted_and_keyword_not_matched(
    ) {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-a", "conversation-a");

        {
            let mut runtime_states =
                lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
            runtime_states.insert(
                "contact-a".to_string(),
                RemoteImContactRuntimeState {
                    presence_state: RemoteImPresenceState::Present,
                    work_state: RemoteImWorkState::Idle,
                    has_pending: false,
                    last_success_reply_at: Some(
                        (time::OffsetDateTime::now_utc() - time::Duration::seconds(600))
                            .format(&time::format_description::well_known::Rfc3339)
                            .expect("format old time"),
                    ),
                    needs_boundary: false,
                    consecutive_no_reply_count: 0,
                },
            );
        }

        let (activate_assistant, reason) =
            remote_im_prepare_enqueue_runtime_state(&state, &contact, "这是一条普通消息")
                .expect("prepare runtime state");

        assert!(!activate_assistant);
        assert!(reason.contains("耐心耗尽"));
        let runtime_states =
            lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
        let runtime = runtime_states.get("contact-a").expect("runtime exists");
        assert_eq!(runtime.presence_state, RemoteImPresenceState::Away);
        assert_eq!(runtime.work_state, RemoteImWorkState::Idle);
        assert!(!runtime.has_pending);
    }

    #[test]
    fn remote_im_prepare_enqueue_runtime_state_should_mark_pending_without_activation_when_busy() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-a", "conversation-a");

        {
            let mut runtime_states =
                lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
            runtime_states.insert(
                "contact-a".to_string(),
                RemoteImContactRuntimeState {
                    presence_state: RemoteImPresenceState::Present,
                    work_state: RemoteImWorkState::Busy,
                    has_pending: false,
                    last_success_reply_at: Some(now_iso()),
                    needs_boundary: false,
                    consecutive_no_reply_count: 0,
                },
            );
        }

        let (activate_assistant, reason) =
            remote_im_prepare_enqueue_runtime_state(&state, &contact, "请补充这条信息")
                .expect("prepare runtime state");

        assert!(!activate_assistant);
        assert!(reason.contains("待办"));
        let runtime_states =
            lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
        let runtime = runtime_states.get("contact-a").expect("runtime exists");
        assert_eq!(runtime.presence_state, RemoteImPresenceState::Present);
        assert_eq!(runtime.work_state, RemoteImWorkState::Busy);
        assert!(runtime.has_pending);
    }

    #[test]
    fn remote_im_finalize_round_completion_should_leave_after_patience_exhausted() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-a", "conversation-a");
        let mut data = AppData::default();
        data.remote_im_contacts.push(contact);
        state_write_app_data_cached(&state, &data).expect("write app data");

        {
            let mut runtime_states =
                lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
            runtime_states.insert(
                "contact-a".to_string(),
                RemoteImContactRuntimeState {
                    presence_state: RemoteImPresenceState::Present,
                    work_state: RemoteImWorkState::Busy,
                    has_pending: false,
                    last_success_reply_at: Some(
                        (time::OffsetDateTime::now_utc() - time::Duration::seconds(600))
                            .format(&time::format_description::well_known::Rfc3339)
                            .expect("format old time"),
                    ),
                    needs_boundary: false,
                    consecutive_no_reply_count: 0,
                },
            );
        }

        remote_im_finalize_round_completion(
            &state,
            &[RemoteImActivationSource {
                channel_id: "channel-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                remote_contact_type: "private".to_string(),
                remote_contact_id: "remote-a".to_string(),
                remote_contact_name: "张三".to_string(),
            }],
            Some("no_reply"),
            None,
            None,
            &now_iso(),
        )
        .expect("finalize round");

        let runtime_states = lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
        let runtime = runtime_states.get("contact-a").expect("runtime exists");
        assert_eq!(runtime.work_state, RemoteImWorkState::Idle);
        assert_eq!(runtime.presence_state, RemoteImPresenceState::Away);
    }

    #[test]
    fn remote_im_finalize_round_completion_should_consume_pending_into_follow_up() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-a", "conversation-a");
        let mut data = AppData::default();
        data.remote_im_contacts.push(contact);
        state_write_app_data_cached(&state, &data).expect("write app data");

        {
            let mut runtime_states =
                lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
            runtime_states.insert(
                "contact-a".to_string(),
                RemoteImContactRuntimeState {
                    presence_state: RemoteImPresenceState::Present,
                    work_state: RemoteImWorkState::Busy,
                    has_pending: true,
                    last_success_reply_at: Some(now_iso()),
                    needs_boundary: false,
                    consecutive_no_reply_count: 0,
                },
            );
        }

        let follow_up_sources = remote_im_finalize_round_completion(
            &state,
            &[RemoteImActivationSource {
                channel_id: "channel-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                remote_contact_type: "private".to_string(),
                remote_contact_id: "remote-a".to_string(),
                remote_contact_name: "张三".to_string(),
            }],
            Some("send"),
            Some(&RemoteImReplyTarget {
                channel_id: "channel-a".to_string(),
                contact_id: "remote-a".to_string(),
            }),
            None,
            &now_iso(),
        )
        .expect("finalize round");

        assert_eq!(follow_up_sources.len(), 1);
        let runtime_states = lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
        let runtime = runtime_states.get("contact-a").expect("runtime exists");
        assert_eq!(runtime.work_state, RemoteImWorkState::Idle);
        assert_eq!(runtime.presence_state, RemoteImPresenceState::Present);
        assert!(!runtime.has_pending);
    }

    #[test]
    fn remote_im_finalize_round_completion_should_not_refresh_last_success_on_no_reply() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-a", "conversation-a");
        let mut data = AppData::default();
        data.remote_im_contacts.push(contact);
        state_write_app_data_cached(&state, &data).expect("write app data");

        let previous_success_at = (time::OffsetDateTime::now_utc() - time::Duration::seconds(123))
            .format(&time::format_description::well_known::Rfc3339)
            .expect("format old time");

        {
            let mut runtime_states =
                lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
            runtime_states.insert(
                "contact-a".to_string(),
                RemoteImContactRuntimeState {
                    presence_state: RemoteImPresenceState::Present,
                    work_state: RemoteImWorkState::Busy,
                    has_pending: false,
                    last_success_reply_at: Some(previous_success_at.clone()),
                    needs_boundary: false,
                    consecutive_no_reply_count: 0,
                },
            );
        }

        let follow_up_sources = remote_im_finalize_round_completion(
            &state,
            &[RemoteImActivationSource {
                channel_id: "channel-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                remote_contact_type: "private".to_string(),
                remote_contact_id: "remote-a".to_string(),
                remote_contact_name: "张三".to_string(),
            }],
            Some("no_reply"),
            None,
            None,
            &now_iso(),
        )
        .expect("finalize round");

        assert!(follow_up_sources.is_empty());
        let runtime_states = lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
        let runtime = runtime_states.get("contact-a").expect("runtime exists");
        assert_eq!(
            runtime.last_success_reply_at.as_deref(),
            Some(previous_success_at.as_str())
        );
    }

    #[test]
    fn remote_im_finalize_round_completion_should_leave_after_two_consecutive_no_reply() {
        let state = remote_im_test_state();
        let contact = remote_im_test_contact("contact-a", "conversation-a");
        let mut data = AppData::default();
        data.remote_im_contacts.push(contact);
        state_write_app_data_cached(&state, &data).expect("write app data");

        {
            let mut runtime_states =
                lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
            runtime_states.insert(
                "contact-a".to_string(),
                RemoteImContactRuntimeState {
                    presence_state: RemoteImPresenceState::Present,
                    work_state: RemoteImWorkState::Busy,
                    has_pending: false,
                    last_success_reply_at: Some(now_iso()),
                    needs_boundary: false,
                    consecutive_no_reply_count: 0,
                },
            );
        }

        let source = RemoteImActivationSource {
            channel_id: "channel-a".to_string(),
            platform: RemoteImPlatform::OnebotV11,
            remote_contact_type: "private".to_string(),
            remote_contact_id: "remote-a".to_string(),
            remote_contact_name: "张三".to_string(),
        };

        remote_im_finalize_round_completion(
            &state,
            &[source.clone()],
            Some("no_reply"),
            None,
            None,
            &now_iso(),
        )
        .expect("finalize first no_reply");
        {
            let runtime_states =
                lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
            let runtime = runtime_states.get("contact-a").expect("runtime exists");
            assert_eq!(runtime.presence_state, RemoteImPresenceState::Present);
            assert_eq!(runtime.consecutive_no_reply_count, 1);
        }

        remote_im_finalize_round_completion(
            &state,
            &[source],
            Some("no_reply"),
            None,
            None,
            &now_iso(),
        )
        .expect("finalize second no_reply");
        {
            let runtime_states =
                lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
            let runtime = runtime_states.get("contact-a").expect("runtime exists");
            assert_eq!(runtime.presence_state, RemoteImPresenceState::Away);
            assert_eq!(runtime.consecutive_no_reply_count, 2);
        }
    }

    #[test]
    fn remote_im_handle_persisted_event_after_history_flush_should_insert_presence_boundary() {
        let state = remote_im_test_state();
        let mut data = AppData::default();
        data.remote_im_contacts
            .push(remote_im_test_contact("contact-a", "conversation-a"));
        let mut conversation = remote_im_test_conversation("conversation-a");
        conversation.messages.push(ChatMessage {
            id: "old-user".to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "之前的上下文".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        });
        data.conversations.push(conversation);
        {
            let mut runtime_states =
                lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
            runtime_states.insert(
                "contact-a".to_string(),
                RemoteImContactRuntimeState {
                    presence_state: RemoteImPresenceState::Present,
                    work_state: RemoteImWorkState::Idle,
                    has_pending: false,
                    last_success_reply_at: None,
                    needs_boundary: true,
                    consecutive_no_reply_count: 0,
                },
            );
        }

        let event_message = ChatMessage {
            id: "incoming-1".to_string(),
            role: "user".to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: "新的联系人消息".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        };
        let event = ChatPendingEvent {
            id: "event-1".to_string(),
            conversation_id: "conversation-a".to_string(),
            created_at: now_iso(),
            source: ChatEventSource::RemoteIm,
            queue_mode: ChatQueueMode::Normal,
            messages: vec![event_message.clone()],
            activate_assistant: true,
            session_info: ChatSessionInfo {
                department_id: REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID.to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
            },
            runtime_context: None,
            sender_info: Some(RemoteImMessageSource {
                channel_id: "channel-a".to_string(),
                platform: RemoteImPlatform::OnebotV11,
                im_name: "qq".to_string(),
                remote_contact_type: "private".to_string(),
                remote_contact_id: "remote-a".to_string(),
                remote_contact_name: "张三".to_string(),
                sender_id: "remote-a".to_string(),
                sender_name: "张三".to_string(),
                sender_avatar_url: None,
                platform_message_id: Some("msg-1".to_string()),
            }),
        };
        data.conversations[0].messages.push(event_message);

        let mut activated_contacts = std::collections::HashSet::new();
        let should_activate = remote_im_handle_persisted_event_after_history_flush(
            &state,
            &mut data,
            "conversation-a",
            &event,
            &now_iso(),
            &mut activated_contacts,
        )
        .expect("handle persisted event");

        assert!(should_activate);
        assert_eq!(
            provider_meta_message_kind(&data.conversations[0].messages[0]).as_deref(),
            Some("context_compaction")
        );
        assert_eq!(
            data.remote_im_contact_checkpoints[0]
                .latest_seen_message_id
                .as_deref(),
            Some("incoming-1")
        );
        let runtime_states = lock_remote_im_contact_runtime_states(&state).expect("lock runtime states");
        let runtime = runtime_states.get("contact-a").expect("runtime exists");
        assert_eq!(runtime.work_state, RemoteImWorkState::Busy);
        assert!(!runtime.needs_boundary);
    }
