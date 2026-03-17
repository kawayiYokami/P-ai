use super::*;

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
