#[cfg(test)]
mod qq_official_tests {
    use super::*;
    use serde_json::json;

    fn test_channel(credentials: Value) -> RemoteImChannelConfig {
        serde_json::from_value(json!({
            "id": "qq-official-test",
            "name": "QQ 官方机器人",
            "platform": "qq_official",
            "enabled": true,
            "credentials": credentials,
        }))
        .expect("构造 QQ 官方机器人渠道失败")
    }

    #[test]
    fn credentials_resolve_api_base_url_by_sandbox_flag() {
        let official = QqOfficialCredentials::from_value(&json!({ "appId": "1", "appSecret": "s" }));
        assert_eq!(official.api_base_url(), QQ_OFFICIAL_API_BASE_URL);
        assert_eq!(official.normalized_api_timeout_ms(), QQ_OFFICIAL_DEFAULT_HTTP_TIMEOUT_MS);

        let sandbox = QqOfficialCredentials::from_value(&json!({
            "appId": "1",
            "appSecret": "s",
            "sandbox": true,
            "apiTimeoutMs": 3000,
        }));
        assert_eq!(sandbox.api_base_url(), QQ_OFFICIAL_SANDBOX_API_BASE_URL);
        assert_eq!(sandbox.normalized_api_timeout_ms(), 3000);
    }

    #[test]
    fn json_i64_accepts_number_and_string() {
        assert_eq!(qq_official_json_i64(&json!(7200)), Some(7200));
        assert_eq!(qq_official_json_i64(&json!("7200")), Some(7200));
        assert_eq!(qq_official_json_i64(&json!("  ")), None);
        assert_eq!(qq_official_json_i64(&json!(null)), None);
    }

    #[test]
    fn upload_type_maps_supported_media_extensions() {
        assert_eq!(
            qq_official_upload_type_for_item("photo.png", "image/png"),
            QQ_OFFICIAL_UPLOAD_TYPE_IMAGE
        );
        assert_eq!(
            qq_official_upload_type_for_item("photo", "image/jpeg"),
            QQ_OFFICIAL_UPLOAD_TYPE_IMAGE
        );
        assert_eq!(
            qq_official_upload_type_for_item("clip.mp4", "application/octet-stream"),
            QQ_OFFICIAL_UPLOAD_TYPE_VIDEO
        );
        assert_eq!(
            qq_official_upload_type_for_item("voice.silk", "audio/x-silk"),
            QQ_OFFICIAL_UPLOAD_TYPE_VOICE
        );
        // QQ 官方图片仅支持 png/jpg，gif 降级为文件发送
        assert_eq!(
            qq_official_upload_type_for_item("anim.gif", "image/gif"),
            QQ_OFFICIAL_UPLOAD_TYPE_FILE
        );
        assert_eq!(
            qq_official_upload_type_for_item("report.pdf", "application/pdf"),
            QQ_OFFICIAL_UPLOAD_TYPE_FILE
        );
    }

    #[test]
    fn contact_scope_requires_known_contact_type() {
        assert_eq!(
            qq_official_scope_for_contact_type("group"),
            Some(QqOfficialSendScope::Group)
        );
        assert_eq!(
            qq_official_scope_for_contact_type("private"),
            Some(QqOfficialSendScope::C2c)
        );
        assert_eq!(qq_official_scope_for_contact_type(""), None);
        assert_eq!(qq_official_scope_for_contact_type("channel"), None);
    }

    #[test]
    fn passive_slot_expires_outside_window() {
        let fresh = QqOfficialPassiveContext {
            msg_id: "msg-1".to_string(),
            received_at: chrono::Utc::now(),
            reply_count: 0,
        };
        let slot = qq_official_passive_slot(QqOfficialSendScope::Group, Some(&fresh));
        assert!(slot.is_some());

        let stale = QqOfficialPassiveContext {
            msg_id: "msg-1".to_string(),
            received_at: chrono::Utc::now()
                - chrono::Duration::seconds(QQ_OFFICIAL_GROUP_PASSIVE_WINDOW_SECS + 1),
            reply_count: 0,
        };
        assert!(qq_official_passive_slot(QqOfficialSendScope::Group, Some(&stale)).is_none());
        // 单聊窗口更长，同一时间点仍然可用
        assert!(qq_official_passive_slot(QqOfficialSendScope::C2c, Some(&stale)).is_some());
        assert!(qq_official_passive_slot(QqOfficialSendScope::Group, None).is_none());
    }

    #[test]
    fn passive_slot_respects_reply_quota() {
        let exhausted = QqOfficialPassiveContext {
            msg_id: "msg-1".to_string(),
            received_at: chrono::Utc::now(),
            reply_count: QQ_OFFICIAL_GROUP_PASSIVE_MAX_REPLIES,
        };
        assert!(qq_official_passive_slot(QqOfficialSendScope::Group, Some(&exhausted)).is_none());
        // 同样的次数在单聊上限（4 次）下已超限
        assert!(qq_official_passive_slot(QqOfficialSendScope::C2c, Some(&exhausted)).is_none());

        let once = QqOfficialPassiveContext {
            msg_id: "msg-1".to_string(),
            received_at: chrono::Utc::now(),
            reply_count: 1,
        };
        let slot = qq_official_passive_slot(QqOfficialSendScope::Group, Some(&once))
            .expect("窗口内且未超次数时应可用");
        assert_eq!(slot.msg_id, "msg-1");
        assert_eq!(slot.next_seq, 2);
        assert_eq!(slot.remaining, QQ_OFFICIAL_GROUP_PASSIVE_MAX_REPLIES - 1);
    }

    #[tokio::test]
    async fn passive_context_key_uses_platform_openid() {
        let manager = qq_official_manager();
        let channel_id = "qq-official-passive-key-test";
        let openid = "OPENID_TEST_1";
        manager
            .record_passive_context(channel_id, openid, "msg-1")
            .await;
        let context = manager
            .passive_context(channel_id, openid)
            .await
            .expect("按平台 openid 记录后应能查到");
        assert_eq!(context.msg_id, "msg-1");
        assert_eq!(context.reply_count, 0);
        manager.mark_passive_reply_sent(channel_id, openid).await;
        assert_eq!(
            manager
                .passive_context(channel_id, openid)
                .await
                .expect("应能查到")
                .reply_count,
            1
        );
        // 空标识不写入脏键
        manager.record_passive_context(channel_id, "", "msg-2").await;
        assert!(manager.passive_context(channel_id, "").await.is_none());
        manager.stop_channel(channel_id).await;
        assert!(manager.passive_context(channel_id, openid).await.is_none());
    }

    #[test]
    fn peek_and_commit_passive_quota() {
        let mut slot = Some(QqOfficialPassiveSlot {
            msg_id: "msg-1".to_string(),
            next_seq: 1,
            remaining: 2,
        });
        // 预览不消耗配额
        assert_eq!(qq_official_peek_passive(&slot), Some(("msg-1".to_string(), 1)));
        assert_eq!(qq_official_peek_passive(&slot), Some(("msg-1".to_string(), 1)));
        qq_official_commit_passive(&mut slot);
        assert_eq!(qq_official_peek_passive(&slot), Some(("msg-1".to_string(), 2)));
        qq_official_commit_passive(&mut slot);
        assert_eq!(qq_official_peek_passive(&slot), None);
        qq_official_commit_passive(&mut slot);
        assert_eq!(qq_official_peek_passive(&slot), None);

        let empty: Option<QqOfficialPassiveSlot> = None;
        assert_eq!(qq_official_peek_passive(&empty), None);
        let mut empty = empty;
        qq_official_commit_passive(&mut empty);
        assert!(empty.is_none());
    }

    #[test]
    fn md5_and_sha1_match_known_vectors() {
        assert_eq!(qq_official_md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            qq_official_sha1_hex(b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn inbound_identity_parses_c2c_event() {
        let data = json!({
            "id": "ROBOT1.0_msg",
            "author": { "user_openid": "USER_OPENID_1", "username": "红豆" },
            "content": "你好",
            "message_type": 0,
        });
        let identity = qq_official_parse_inbound_identity(&data, false).expect("解析单聊事件");
        assert_eq!(identity.message_id, "ROBOT1.0_msg");
        assert_eq!(identity.remote_contact_type, "private");
        assert_eq!(identity.remote_contact_id, "USER_OPENID_1");
        assert_eq!(identity.sender_id, "USER_OPENID_1");
        assert_eq!(identity.sender_name, "红豆");
        assert_eq!(identity.remote_contact_name.as_deref(), Some("红豆"));
        assert_eq!(identity.content, "你好");
    }

    #[test]
    fn inbound_identity_parses_group_event() {
        let data = json!({
            "id": "ROBOT1.0_group_msg",
            "author": { "member_openid": "MEMBER_OPENID_1", "username": "小红" },
            "group_openid": "GROUP_OPENID_1",
            "content": "在吗",
            "message_type": 0,
        });
        let identity = qq_official_parse_inbound_identity(&data, true).expect("解析群聊事件");
        assert_eq!(identity.remote_contact_type, "group");
        assert_eq!(identity.remote_contact_id, "GROUP_OPENID_1");
        assert_eq!(identity.sender_id, "MEMBER_OPENID_1");
        assert_eq!(identity.sender_name, "小红");
        assert_eq!(identity.remote_contact_name, None);
    }

    #[test]
    fn inbound_identity_rejects_missing_openid() {
        let missing_user = json!({ "id": "m1", "author": { "username": "x" }, "content": "hi" });
        assert!(qq_official_parse_inbound_identity(&missing_user, false).is_err());

        let missing_group = json!({ "id": "m2", "author": { "member_openid": "m" }, "content": "hi" });
        assert!(qq_official_parse_inbound_identity(&missing_group, true).is_err());

        assert!(qq_official_parse_inbound_identity(&json!("not-object"), false).is_err());
    }

    #[test]
    fn enqueue_input_carries_message_id_and_provider_meta() {
        let channel = test_channel(json!({ "appId": "1", "appSecret": "s" }));
        let data = json!({
            "id": "ROBOT1.0_msg",
            "author": { "user_openid": "USER_1", "username": "红豆" },
            "content": "你好",
            "message_type": 0,
        });
        let identity = qq_official_parse_inbound_identity(&data, false).expect("解析单聊事件");
        let parts = vec![ChatIngressPart::Text {
            text: "你好".to_string(),
        }];
        let input = qq_official_build_enqueue_input(&channel, &identity, &data, parts, false);
        assert_eq!(input.platform, RemoteImPlatform::QqOfficial);
        assert_eq!(input.platform_message_id.as_deref(), Some("ROBOT1.0_msg"));
        assert_eq!(input.remote_contact_type, "private");
        assert_eq!(input.payload.text.as_deref(), Some("你好"));
        let meta = input.payload.provider_meta.as_ref().expect("provider_meta");
        assert_eq!(meta.get("messageId").and_then(Value::as_str), Some("ROBOT1.0_msg"));
        assert_eq!(meta.get("userOpenId").and_then(Value::as_str), Some("USER_1"));
        assert!(meta.get("groupId").map(Value::is_null).unwrap_or(false));
    }

    #[test]
    fn enqueue_input_flags_group_scope_in_provider_meta() {
        let channel = test_channel(json!({ "appId": "1", "appSecret": "s" }));
        let data = json!({
            "id": "ROBOT1.0_group_msg",
            "author": { "member_openid": "MEMBER_1", "username": "小红" },
            "group_openid": "GROUP_1",
            "content": "在吗",
        });
        let identity = qq_official_parse_inbound_identity(&data, true).expect("解析群聊事件");
        let parts = vec![ChatIngressPart::Text {
            text: "在吗".to_string(),
        }];
        let input = qq_official_build_enqueue_input(&channel, &identity, &data, parts, true);
        assert_eq!(input.remote_contact_type, "group");
        let meta = input.payload.provider_meta.as_ref().expect("provider_meta");
        assert_eq!(meta.get("groupId").and_then(Value::as_str), Some("GROUP_1"));
        assert_eq!(meta.get("memberOpenId").and_then(Value::as_str), Some("MEMBER_1"));
    }
}
