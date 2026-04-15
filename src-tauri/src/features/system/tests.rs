    #[test]
    fn fetch_models_openai_should_read_models_from_base_url() {
        let server = MockServer::start();
        let model_mock = server.mock(|when, then| {
            when.method(GET).path("/models");
            then.status(200).json_body(serde_json::json!({
              "data": [
                { "id": "gpt-4o-mini" },
                { "id": "gpt-4.1-mini" }
              ]
            }));
        });

        let input = RefreshModelsInput {
            base_url: server.base_url(),
            api_key: "test-key".to_string(),
            request_format: RequestFormat::OpenAI,
            provider_id: None,
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
        };

        let rt = test_runtime();
        let models = rt
            .block_on(fetch_models_openai(&input))
            .expect("fetch models from mock");

        model_mock.assert();
        assert_eq!(
            models,
            vec!["gpt-4.1-mini".to_string(), "gpt-4o-mini".to_string()]
        );
    }

    #[test]
    fn fetch_models_openai_should_fallback_to_v1_models() {
        let server = MockServer::start();
        let base_404_mock = server.mock(|when, then| {
            when.method(GET).path("/models");
            then.status(404).body("not found");
        });
        let v1_ok_mock = server.mock(|when, then| {
            when.method(GET).path("/v1/models");
            then.status(200).json_body(serde_json::json!({
              "data": [{ "id": "moonshot-v1-8k" }]
            }));
        });

        let input = RefreshModelsInput {
            base_url: server.base_url(),
            api_key: "test-key".to_string(),
            request_format: RequestFormat::OpenAI,
            provider_id: None,
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
        };

        let rt = test_runtime();
        let models = rt
            .block_on(fetch_models_openai(&input))
            .expect("fallback /v1/models should succeed");

        base_404_mock.assert();
        v1_ok_mock.assert();
        assert_eq!(models, vec!["moonshot-v1-8k".to_string()]);
    }

    #[test]
    fn verify_staging_files_should_accept_when_target_exe_present() {
        let temp_root = std::env::temp_dir().join(format!("easy-call-ai-updater-{}", Uuid::new_v4()));
        let staging_dir = temp_root.join("staging");
        std::fs::create_dir_all(staging_dir.join("config")).expect("create staging dir");
        std::fs::write(staging_dir.join("P-ai.exe"), b"exe").expect("write exe");
        std::fs::write(staging_dir.join("config").join("app.json"), b"{}")
            .expect("write config");

        let relative_files = vec![PathBuf::from("P-ai.exe"), PathBuf::from("config/app.json")];

        let result = verify_staging_files(&staging_dir, &relative_files, "P-ai.exe");

        let _ = std::fs::remove_dir_all(&temp_root);
        assert!(result.is_ok());
    }

    #[test]
    fn verify_staging_files_should_reject_missing_target_exe() {
        let temp_root = std::env::temp_dir().join(format!("easy-call-ai-updater-{}", Uuid::new_v4()));
        let staging_dir = temp_root.join("staging");
        std::fs::create_dir_all(&staging_dir).expect("create staging dir");
        std::fs::write(staging_dir.join("README.txt"), b"missing exe").expect("write readme");

        let relative_files = vec![PathBuf::from("README.txt")];

        let result = verify_staging_files(&staging_dir, &relative_files, "P-ai.exe");

        let _ = std::fs::remove_dir_all(&temp_root);
        assert_eq!(
            result.expect_err("missing target exe should fail"),
            "更新包缺少主程序文件：P-ai.exe"
        );
    }

    #[test]
    fn conversation_todo_replace_should_store_next_step_and_clear_when_done() {
        let state = test_chat_runtime_state();
        let conversation_id = "conversation-todo-a".to_string();
        let now = now_iso();
        let mut data = AppData::default();
        data.conversations.push(Conversation {
            id: conversation_id.clone(),
            title: "todo".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: String::new(),
            last_read_message_id: String::new(),
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now.clone(),
            updated_at: now,
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
        });
        state_write_app_data_cached(&state, &data).expect("write app data");

        let stored = conversation_todo_replace(
            &state,
            &conversation_id,
            vec![
                ConversationTodoItem {
                    content: "Add todo MCP server".to_string(),
                    status: "in_progress".to_string(),
                },
                ConversationTodoItem {
                    content: "Run cargo check".to_string(),
                    status: "pending".to_string(),
                },
            ],
        )
        .expect("store todos");

        assert_eq!(stored.len(), 2);
        assert_eq!(
            todo_response_text(&stored),
            "## Current Todo List\n\n→ Add todo MCP server\n○ Run cargo check"
        );
        assert_eq!(
            conversation_todo_list(&state, &conversation_id)
                .expect("read todos")
                .len(),
            2
        );

        let cleared = conversation_todo_replace(
            &state,
            &conversation_id,
            vec![
                ConversationTodoItem {
                    content: "Add todo MCP server".to_string(),
                    status: "completed".to_string(),
                },
                ConversationTodoItem {
                    content: "Run cargo check".to_string(),
                    status: "completed".to_string(),
                },
            ],
        )
        .expect("clear todos");

        assert!(cleared.is_empty());
        assert_eq!(
            todo_response_text(&[
                ConversationTodoItem {
                    content: "Add todo MCP server".to_string(),
                    status: "completed".to_string(),
                },
                ConversationTodoItem {
                    content: "Run cargo check".to_string(),
                    status: "completed".to_string(),
                },
            ]),
            "## Current Todo List\n\n✓ Add todo MCP server\n✓ Run cargo check\n\n已经完成了所有步骤，请向用户进行汇报"
        );
        assert!(
            conversation_todo_list(&state, &conversation_id)
                .expect("read cleared todos")
                .is_empty()
        );
    }

    #[test]
    fn todo_items_normalized_from_tool_args_should_trim_and_validate() {
        let items = todo_items_normalized_from_tool_args(
            r#"{
                "todos": [
                    { "content": "  第一步  ", "status": "IN_PROGRESS" },
                    { "content": " 第二步 ", "status": "pending" }
                ]
            }"#,
        )
        .expect("normalize todo args");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].content, "第一步");
        assert_eq!(items[0].status, "in_progress");
        assert_eq!(items[1].content, "第二步");
        assert_eq!(items[1].status, "pending");
    }

    #[test]
    fn todo_items_normalized_from_tool_args_should_reject_multiple_in_progress() {
        let err = todo_items_normalized_from_tool_args(
            r#"{
                "todos": [
                    { "content": "第一步", "status": "in_progress" },
                    { "content": "第二步", "status": "in_progress" }
                ]
            }"#,
        )
        .expect_err("multiple in_progress should fail");

        assert_eq!(err, "todo 同时只能有一个 in_progress");
    }

    #[test]
    fn build_compaction_message_should_append_current_todo_list_after_memory_snapshot() {
        let message = build_compaction_message(
            "这里是压缩摘要",
            "manual",
            Some("<user profile snapshot>\n记忆块\n</user profile snapshot>"),
            Some(&[
                ConversationTodoItem {
                    content: "Add todo MCP server".to_string(),
                    status: "in_progress".to_string(),
                },
                ConversationTodoItem {
                    content: "Run cargo check".to_string(),
                    status: "pending".to_string(),
                },
            ]),
            Some("用户：继续推进\n助手甲：我来接着处理"),
        );
        let text = message
            .parts
            .iter()
            .find_map(|part| match part {
                MessagePart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .expect("compaction text");

        assert!(text.contains("用户画像："));
        assert!(text.contains("摘要说明："));
        assert!(text.contains("摘要正文："));
        assert!(text.contains("保留对话："));
        assert!(text.contains("记忆块"));
        assert!(text.contains("## Current Todo List"));
        assert!(text.contains("- [in_progress] Add todo MCP server"));
        assert!(text.contains("- [pending] Run cargo check"));
        assert!(text.contains("用户：继续推进\n助手甲：我来接着处理"));
    }

    #[test]
    fn build_compaction_preserved_dialogue_block_should_use_token_budget_and_skip_compaction() {
        let now = now_iso();
        let long_middle = "中间消息".repeat(200);
        let latest_user = "最后一条用户消息";
        let latest_assistant = "最后一条助手消息";
        let budget = (estimated_tokens_for_text(latest_user) + estimated_tokens_for_text(latest_assistant)).ceil() as usize + 1;
        let conversation = Conversation {
            id: "conversation-token-budget".to_string(),
            title: "token budget".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: String::new(),
            last_read_message_id: String::new(),
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now.clone(),
            updated_at: now.clone(),
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
            messages: vec![
                ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "user".to_string(),
                    created_at: now.clone(),
                    speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
                    parts: vec![MessagePart::Text {
                        text: "[上下文整理]\n旧摘要".to_string(),
                    }],
                    extra_text_blocks: Vec::new(),
                    provider_meta: Some(serde_json::json!({
                        "message_meta": {
                            "kind": "context_compaction",
                            "scene": "compaction",
                            "reason": "manual"
                        }
                    })),
                    tool_call: None,
                    mcp_call: None,
                },
                ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "user".to_string(),
                    created_at: now.clone(),
                    speaker_agent_id: Some(USER_PERSONA_ID.to_string()),
                    parts: vec![MessagePart::Text {
                        text: long_middle.clone(),
                    }],
                    extra_text_blocks: Vec::new(),
                    provider_meta: None,
                    tool_call: None,
                    mcp_call: None,
                },
                ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "user".to_string(),
                    created_at: now.clone(),
                    speaker_agent_id: Some(USER_PERSONA_ID.to_string()),
                    parts: vec![MessagePart::Text {
                        text: latest_user.to_string(),
                    }],
                    extra_text_blocks: Vec::new(),
                    provider_meta: None,
                    tool_call: None,
                    mcp_call: None,
                },
                ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "assistant".to_string(),
                    created_at: now,
                    speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                    parts: vec![MessagePart::Text {
                        text: latest_assistant.to_string(),
                    }],
                    extra_text_blocks: Vec::new(),
                    provider_meta: None,
                    tool_call: None,
                    mcp_call: None,
                },
            ],
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        };

        let preserved =
            build_compaction_preserved_dialogue_block(&conversation, "用户", "助手", budget);

        assert!(preserved.contains("用户：最后一条用户消息"));
        assert!(preserved.contains("助手：最后一条助手消息"));
        assert!(!preserved.contains(&long_middle));
        assert!(!preserved.contains("旧摘要"));
    }
