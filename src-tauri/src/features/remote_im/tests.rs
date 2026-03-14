    #[test]
    fn remote_im_should_forward_matrix_should_match_policy() {
        assert!(!remote_im_should_forward(
            &RemoteImReplyMode::None,
            true,
            false
        ));
        assert!(remote_im_should_forward(
            &RemoteImReplyMode::Always,
            false,
            true
        ));
        assert!(remote_im_should_forward(
            &RemoteImReplyMode::ReplyOnce,
            true,
            false
        ));
        assert!(!remote_im_should_forward(
            &RemoteImReplyMode::ReplyOnce,
            false,
            false
        ));
        assert!(!remote_im_should_forward(
            &RemoteImReplyMode::ReplyOnce,
            true,
            true
        ));
    }

    #[test]
    fn remote_im_upsert_contact_for_inbound_should_reset_once_flag() {
        let channel = RemoteImChannelConfig {
            id: "c1".to_string(),
            name: "qq".to_string(),
            platform: RemoteImPlatform::Napcat,
            enabled: true,
            credentials: serde_json::json!({}),
            activate_assistant: true,
            default_reply_mode: RemoteImReplyMode::ReplyOnce,
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            allow_proactive_send: false,
            allow_send_files: false,
        };
        let mut data = AppData::default();
        let input = RemoteImEnqueueInput {
            channel_id: "c1".to_string(),
            platform: RemoteImPlatform::Napcat,
            im_name: "qq".to_string(),
            remote_contact_type: "group".to_string(),
            remote_contact_id: "g1".to_string(),
            remote_contact_name: Some("测试群".to_string()),
            sender_id: "u1".to_string(),
            sender_name: "张三".to_string(),
            sender_avatar_url: None,
            platform_message_id: Some("m1".to_string()),
            activate_assistant: Some(true),
            session: SessionSelector {
                api_config_id: Some("api".to_string()),
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
        assert!(contact.has_new_message);
        assert!(!contact.forwarded_once_since_last_inbound);
        assert_eq!(contact.reply_mode, RemoteImReplyMode::None);

        if let Some(contact) = data.remote_im_contacts.first_mut() {
            contact.forwarded_once_since_last_inbound = true;
            contact.has_new_message = false;
        }
        let now2 = now_iso();
        let contact_id_2 = remote_im_upsert_contact_for_inbound(&mut data, &channel, &input, &now2);
        assert_eq!(contact_id, contact_id_2);
        let contact = data.remote_im_contacts.first().expect("contact exists");
        assert!(contact.has_new_message);
        assert!(!contact.forwarded_once_since_last_inbound);
    }

    #[test]
    fn remote_im_collect_outbound_should_collect_and_update_contact_state() {
        let mut config = AppConfig::default();
        config.remote_im_channels = vec![
            RemoteImChannelConfig {
                id: "c-none".to_string(),
                name: "none".to_string(),
                platform: RemoteImPlatform::Napcat,
                enabled: true,
                credentials: serde_json::json!({}),
                activate_assistant: true,
                default_reply_mode: RemoteImReplyMode::None,
                receive_files: true,
                streaming_send: false,
                show_tool_calls: false,
                allow_proactive_send: false,
                allow_send_files: false,
            },
            RemoteImChannelConfig {
                id: "c-always".to_string(),
                name: "always".to_string(),
                platform: RemoteImPlatform::Feishu,
                enabled: true,
                credentials: serde_json::json!({}),
                activate_assistant: true,
                default_reply_mode: RemoteImReplyMode::Always,
                receive_files: true,
                streaming_send: false,
                show_tool_calls: false,
                allow_proactive_send: false,
                allow_send_files: false,
            },
            RemoteImChannelConfig {
                id: "c-once".to_string(),
                name: "once".to_string(),
                platform: RemoteImPlatform::Dingtalk,
                enabled: true,
                credentials: serde_json::json!({}),
                activate_assistant: true,
                default_reply_mode: RemoteImReplyMode::ReplyOnce,
                receive_files: true,
                streaming_send: false,
                show_tool_calls: false,
                allow_proactive_send: false,
                allow_send_files: false,
            },
        ];

        let mut data = AppData::default();
        data.remote_im_contacts = vec![
            RemoteImContact {
                id: "k-none".to_string(),
                channel_id: "c-none".to_string(),
                platform: RemoteImPlatform::Napcat,
                remote_contact_type: "group".to_string(),
                remote_contact_id: "1".to_string(),
                remote_contact_name: String::new(),
                remark_name: String::new(),
                reply_mode: RemoteImReplyMode::None,
                has_new_message: true,
                forwarded_once_since_last_inbound: false,
                last_message_at: None,
                last_forwarded_at: None,
            },
            RemoteImContact {
                id: "k-always".to_string(),
                channel_id: "c-always".to_string(),
                platform: RemoteImPlatform::Feishu,
                remote_contact_type: "group".to_string(),
                remote_contact_id: "2".to_string(),
                remote_contact_name: String::new(),
                remark_name: String::new(),
                reply_mode: RemoteImReplyMode::Always,
                has_new_message: false,
                forwarded_once_since_last_inbound: true,
                last_message_at: None,
                last_forwarded_at: None,
            },
            RemoteImContact {
                id: "k-once".to_string(),
                channel_id: "c-once".to_string(),
                platform: RemoteImPlatform::Dingtalk,
                remote_contact_type: "group".to_string(),
                remote_contact_id: "3".to_string(),
                remote_contact_name: String::new(),
                remark_name: String::new(),
                reply_mode: RemoteImReplyMode::ReplyOnce,
                has_new_message: true,
                forwarded_once_since_last_inbound: false,
                last_message_at: None,
                last_forwarded_at: None,
            },
        ];

        let assistant_message = ChatMessage {
            id: "msg-a".to_string(),
            role: "assistant".to_string(),
            created_at: now_iso(),
            speaker_agent_id: Some("agent".to_string()),
            parts: vec![MessagePart::Text {
                text: "assistant reply".to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        };

        let targets =
            remote_im_collect_outbound_after_assistant_message(&config, &mut data, &assistant_message);
        assert_eq!(targets.len(), 2);
        let once_contact = data
            .remote_im_contacts
            .iter()
            .find(|item| item.id == "k-once")
            .expect("once contact");
        assert!(!once_contact.has_new_message);
        assert!(once_contact.forwarded_once_since_last_inbound);
    }

    #[test]
    fn remote_im_resolve_inbound_activate_should_prefer_message_level_flag() {
        let channel = RemoteImChannelConfig {
            id: "c1".to_string(),
            name: "qq".to_string(),
            platform: RemoteImPlatform::Napcat,
            enabled: true,
            credentials: serde_json::json!({}),
            activate_assistant: false,
            default_reply_mode: RemoteImReplyMode::ReplyOnce,
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            allow_proactive_send: false,
            allow_send_files: false,
        };
        assert!(!remote_im_resolve_inbound_activate(&channel, None));
        assert!(remote_im_resolve_inbound_activate(&channel, Some(true)));
        assert!(!remote_im_resolve_inbound_activate(&channel, Some(false)));
    }
