    fn test_task_data_path(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "easy_call_ai_task_test_{}_{}",
            label,
            Uuid::new_v4()
        ));
        let _ = fs::remove_dir_all(&path);
        path
    }

    fn task_test_state(label: &str) -> AppState {
        let data_path = test_task_data_path(label).join("app_data.json");
        let state = AppState::new().expect("create test app state");
        AppState { data_path, ..state }
    }

    fn write_task_test_snapshot(
        state: &AppState,
        runtime: &mut RuntimeStateFile,
        conversations: &[Conversation],
    ) {
        state_write_runtime_state_cached(state, runtime).expect("write runtime state");
        for conversation in conversations {
            state_write_conversation_cached(state, conversation).expect("write conversation");
        }
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
                run_at_local: Some("2026-04-10T10:00:00+08:00".to_string()),
                every_minutes: Some(30.0),
                end_at_local: Some("2026-04-10T12:00:00+08:00".to_string()),
            },
        };

        let created = task_store_create_task(&data_path, &input).expect("create task");
        assert_eq!(created.conversation_id.as_deref(), Some("conversation-a"));

        let fetched = task_store_get_task(&data_path, &created.task_id).expect("get task");
        assert_eq!(fetched.conversation_id.as_deref(), Some("conversation-a"));

        let _ = fs::remove_dir_all(app_root_from_data_path(&data_path));
    }

    #[test]
    fn task_store_mark_skipped_should_advance_next_run_atomically() {
        let data_path = test_task_data_path("mark_skipped_advances_next_run");
        let input = TaskCreateInput {
            goal: "跳过后重试".to_string(),
            conversation_id: Some("conversation-a".to_string()),
            target_scope: Some(TASK_TARGET_SCOPE_DESKTOP.to_string()),
            why: String::new(),
            todo: "等待空闲后继续".to_string(),
            trigger: TaskTriggerInputLocal {
                run_at_local: Some("2026-04-10T10:00:00+08:00".to_string()),
                every_minutes: Some(30.0),
                end_at_local: Some("2099-04-10T12:00:00+08:00".to_string()),
            },
        };
        let created = task_store_create_task(&data_path, &input).expect("create task");
        let before = task_store_get_task_record(&data_path, &created.task_id).expect("get before");
        assert!(before.last_triggered_at_utc.is_none());
        let before_run = before
            .trigger
            .run_at_utc
            .as_deref()
            .and_then(parse_rfc3339_time)
            .expect("before run");
        let before_next = before
            .trigger
            .next_run_at_utc
            .as_deref()
            .and_then(parse_rfc3339_time)
            .expect("before next run");
        assert_eq!(before_next, before_run + time::Duration::minutes(30));

        task_store_mark_skipped(&data_path, &created.task_id, "skipped", "busy skip")
            .expect("mark skipped");

        let after = task_store_get_task_record(&data_path, &created.task_id).expect("get after");
        let last = after
            .last_triggered_at_utc
            .as_deref()
            .and_then(parse_rfc3339_time)
            .expect("last triggered");
        let next = after
            .trigger
            .next_run_at_utc
            .as_deref()
            .and_then(parse_rfc3339_time)
            .expect("next run");
        assert_eq!(next, last + time::Duration::minutes(30));
        let logs = task_store_list_run_log_records(&data_path, Some(&created.task_id), 10)
            .expect("list logs");
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].outcome, "skipped");

        let _ = fs::remove_dir_all(app_root_from_data_path(&data_path));
    }

    #[test]
    fn task_dispatch_conversation_should_prefer_bound_then_fallback_to_main() {
        let state = task_test_state("dispatch_prefer_bound");
        let mut runtime = RuntimeStateFile::default();
        let api_id = "api-a";
        let agent_id = DEFAULT_AGENT_ID;

        let mut main = build_conversation_record(
            api_id,
            agent_id,
            ASSISTANT_DEPARTMENT_ID,
            "main",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        main.id = "main-conversation".to_string();
        let mut side = build_conversation_record(
            api_id,
            agent_id,
            REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID,
            "side",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        side.id = "side-conversation".to_string();
        runtime.main_conversation_id = Some(main.id.clone());
        write_task_test_snapshot(&state, &mut runtime, &[main.clone(), side.clone()]);

        let preferred = task_resolve_dispatch_conversation(
            &state,
            &mut runtime,
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
            &state,
            &mut runtime,
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

        let _ = fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }

    #[test]
    fn task_dispatch_conversation_should_skip_missing_contact_conversation() {
        let state = task_test_state("dispatch_skip_missing_contact");
        let mut runtime = RuntimeStateFile::default();
        let api_id = "api-a";
        let agent_id = DEFAULT_AGENT_ID;

        let mut main = build_conversation_record(
            api_id,
            agent_id,
            ASSISTANT_DEPARTMENT_ID,
            "main",
            CONVERSATION_KIND_CHAT,
            None,
            None,
        );
        main.id = "main-conversation".to_string();
        runtime.main_conversation_id = Some(main.id.clone());
        runtime.remote_im_contacts.push(RemoteImContact {
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
            patience_seconds: default_remote_im_contact_patience_seconds(),
            activation_cooldown_seconds: 0,
            route_mode: "dedicated_contact_conversation".to_string(),
            bound_department_id: Some(REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID.to_string()),
            bound_conversation_id: Some("missing-contact-conversation".to_string()),
            processing_mode: "continuous".to_string(),
            last_activated_at: None,
            last_message_at: None,
            dingtalk_session_webhook: None,
            dingtalk_session_webhook_expired_time: None,
        });
        write_task_test_snapshot(&state, &mut runtime, &[main]);

        let resolved = task_resolve_dispatch_conversation(
            &state,
            &mut runtime,
            api_id,
            agent_id,
            Some("missing-contact-conversation"),
            TASK_TARGET_SCOPE_CONTACT,
        )
        .expect("resolve missing contact conversation");

        assert!(resolved.is_none());

        let _ = fs::remove_dir_all(app_root_from_data_path(&state.data_path));
    }
