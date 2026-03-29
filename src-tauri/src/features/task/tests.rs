    fn test_task_data_path(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "easy_call_ai_task_test_{}_{}",
            label,
            Uuid::new_v4()
        ));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn task_store_should_persist_conversation_id() {
        let data_path = test_task_data_path("persist_conversation_id");
        let input = TaskCreateInput {
            goal: "跟进并发会话".to_string(),
            conversation_id: Some("conversation-a".to_string()),
            target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
            why: String::new(),
            todo: "检查调度".to_string(),
            trigger: TaskTriggerInputLocal {
                run_at_local: None,
                every_minutes: None,
                end_at_local: None,
            },
        };

        let created = task_store_create_task(&data_path, &input).expect("create task");
        assert_eq!(created.conversation_id.as_deref(), Some("conversation-a"));

        let fetched = task_store_get_task(&data_path, &created.task_id).expect("get task");
        assert_eq!(fetched.conversation_id.as_deref(), Some("conversation-a"));

        let _ = fs::remove_dir_all(app_root_from_data_path(&data_path));
    }

    #[test]
    fn task_dispatch_conversation_should_prefer_bound_then_fallback_to_main() {
        let mut data = AppData::default();
        let api_id = "api-a";
        let agent_id = DEFAULT_AGENT_ID;

        let mut main = build_conversation_record(api_id, agent_id, "main", CONVERSATION_KIND_CHAT, None, None);
        main.id = "main-conversation".to_string();
        let mut side = build_conversation_record(api_id, agent_id, "side", CONVERSATION_KIND_CHAT, None, None);
        side.id = "side-conversation".to_string();
        data.main_conversation_id = Some(main.id.clone());
        data.conversations = vec![main.clone(), side.clone()];

        let preferred = task_resolve_dispatch_conversation(
            &mut data,
            api_id,
            agent_id,
            Some(side.id.as_str()),
            TASK_TARGET_SCOPE_DESKTOP,
        )
        .expect("resolve bound conversation")
        .expect("preferred conversation");
        assert_eq!(preferred.conversation_id, side.id);
        assert_eq!(preferred.target_scope, TASK_TARGET_SCOPE_DESKTOP);
        assert!(!preferred.fallback_to_main);

        let fallback = task_resolve_dispatch_conversation(
            &mut data,
            api_id,
            agent_id,
            Some("missing-conversation"),
            TASK_TARGET_SCOPE_DESKTOP,
        )
        .expect("fallback to main conversation")
        .expect("fallback conversation");
        assert_eq!(fallback.conversation_id, main.id);
        assert_eq!(fallback.target_scope, TASK_TARGET_SCOPE_DESKTOP);
        assert!(fallback.fallback_to_main);
    }

    #[test]
    fn task_dispatch_conversation_should_skip_missing_contact_conversation() {
        let mut data = AppData::default();
        let api_id = "api-a";
        let agent_id = DEFAULT_AGENT_ID;

        let mut main = build_conversation_record(api_id, agent_id, "main", CONVERSATION_KIND_CHAT, None, None);
        main.id = "main-conversation".to_string();
        data.main_conversation_id = Some(main.id.clone());
        data.conversations = vec![main];
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
            bound_conversation_id: Some("missing-contact-conversation".to_string()),
            processing_mode: "continuous".to_string(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
        });

        let resolved = task_resolve_dispatch_conversation(
            &mut data,
            api_id,
            agent_id,
            Some("missing-contact-conversation"),
            TASK_TARGET_SCOPE_CONTACT,
        )
        .expect("resolve missing contact conversation");

        assert!(resolved.is_none());
    }
