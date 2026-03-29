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
            title: "跟进并发会话".to_string(),
            conversation_id: Some("conversation-a".to_string()),
            cause: String::new(),
            goal: String::new(),
            flow: String::new(),
            todos: vec!["检查调度".to_string()],
            status_summary: "待处理".to_string(),
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

        let (preferred_id, fallback_used) = task_resolve_dispatch_conversation_id(
            &mut data,
            api_id,
            agent_id,
            Some(side.id.as_str()),
        )
        .expect("resolve bound conversation");
        assert_eq!(preferred_id, side.id);
        assert!(!fallback_used);

        let (fallback_id, fallback_used) = task_resolve_dispatch_conversation_id(
            &mut data,
            api_id,
            agent_id,
            Some("missing-conversation"),
        )
        .expect("fallback to main conversation");
        assert_eq!(fallback_id, main.id);
        assert!(fallback_used);
    }
