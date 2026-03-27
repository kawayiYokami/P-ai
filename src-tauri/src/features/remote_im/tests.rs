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
                archived_at: None,
                messages: Vec::new(),
                memory_recall_table: Vec::new(),
            },
            Conversation {
                id: "conversation-sub".to_string(),
                title: "sub".to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
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
                archived_at: None,
                messages: Vec::new(),
                memory_recall_table: Vec::new(),
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
                archived_at: None,
                messages: Vec::new(),
                memory_recall_table: Vec::new(),
            },
            Conversation {
                id: "conversation-sub".to_string(),
                title: "sub".to_string(),
                agent_id: DEFAULT_AGENT_ID.to_string(),
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
                archived_at: None,
                messages: Vec::new(),
                memory_recall_table: Vec::new(),
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
    fn conversation_has_remote_im_platform_message_should_match_existing_origin_meta() {
        let conversation = Conversation {
            id: "conv-1".to_string(),
            title: "联系人".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
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
            memory_recall_table: Vec::new(),
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
