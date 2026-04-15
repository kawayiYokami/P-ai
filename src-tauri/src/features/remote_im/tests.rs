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
        let mut data = AppData::default();
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
                provider_meta: None,
            },
        };
        let now = now_iso();
        let contact_id = remote_im_upsert_contact_for_inbound(&mut data, &channel, &input, &now);
        assert_eq!(data.remote_im_contacts.len(), 1);
        let contact = data
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
        let contact_id_2 = remote_im_upsert_contact_for_inbound(&mut data, &channel, &input, &now2);
        assert_eq!(contact_id, contact_id_2);
        assert_eq!(data.remote_im_contacts.len(), 1);
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
        let mut data = AppData::default();
        data.main_conversation_id = Some("conversation-main".to_string());
        data.conversations = vec![
            Conversation {
                id: "conversation-main".to_string(),
                title: "main".to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                department_id: String::new(),
                last_read_message_id: String::new(),
                conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: now_iso(),
                updated_at: now_iso(),
                last_user_at: None,
                last_assistant_at: None,
                last_context_usage_ratio: 0.0,
                last_effective_prompt_tokens: 0,
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
                last_read_message_id: String::new(),
                conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: now_iso(),
                updated_at: now_iso(),
                last_user_at: None,
                last_assistant_at: None,
                last_context_usage_ratio: 0.0,
                last_effective_prompt_tokens: 0,
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
        let config = AppConfig::default();
        let (_, _, conversation_id) =
            resolve_contact_session_target(&config, &mut data, &mut contact).expect("resolve route");

        assert_eq!(conversation_id, "conversation-main");
    }

    #[test]
    fn remote_im_should_still_route_to_main_after_user_switches_to_sub_conversation() {
        let mut data = AppData::default();
        data.main_conversation_id = Some("conversation-main".to_string());
        data.conversations = vec![
            Conversation {
                id: "conversation-main".to_string(),
                title: "main".to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
                department_id: String::new(),
                last_read_message_id: String::new(),
                conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: now_iso(),
                updated_at: now_iso(),
                last_user_at: None,
                last_assistant_at: None,
                last_context_usage_ratio: 0.0,
                last_effective_prompt_tokens: 0,
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
                last_read_message_id: String::new(),
                conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
                root_conversation_id: None,
                delegate_id: None,
                created_at: now_iso(),
                updated_at: now_iso(),
                last_user_at: None,
                last_assistant_at: None,
                last_context_usage_ratio: 0.0,
                last_effective_prompt_tokens: 0,
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
        let config = AppConfig::default();
        let (_, _, conversation_id) =
            resolve_contact_session_target(&config, &mut data, &mut contact).expect("resolve route");

        assert_eq!(conversation_id, "conversation-main");
        assert_eq!(data.main_conversation_id.as_deref(), Some("conversation-main"));
        assert_eq!(
            data.conversations
                .iter()
                .find(|item| item.id == "conversation-sub")
                .map(|item| item.status.as_str()),
            Some("active")
        );
    }

    #[test]
    fn conversation_has_remote_im_platform_message_should_match_snake_case_origin_meta() {
        let conversation = Conversation {
            id: "conv-1".to_string(),
            title: "联系人".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: String::new(),
            last_read_message_id: String::new(),
            conversation_kind: CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
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
            last_read_message_id: String::new(),
            conversation_kind: CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
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

