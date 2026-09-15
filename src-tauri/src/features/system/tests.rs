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
    fn model_refresh_strategies_should_prefer_native_provider_from_format() {
        let input = RefreshModelsInput {
            base_url: "https://generativelanguage.googleapis.com".to_string(),
            api_key: "test-key".to_string(),
            request_format: RequestFormat::Gemini,
            provider_id: None,
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
        };

        assert_eq!(
            model_refresh_strategies(&input),
            vec![
                ModelRefreshStrategy::GeminiNative,
                ModelRefreshStrategy::OpenAi,
                ModelRefreshStrategy::AnthropicNative,
                ModelRefreshStrategy::GenaiAdapter(genai::adapter::AdapterKind::Gemini),
            ]
        );
    }

    #[test]
    fn model_refresh_strategies_should_infer_native_provider_for_auto_base_url() {
        let input = RefreshModelsInput {
            base_url: "https://api.anthropic.com".to_string(),
            api_key: "test-key".to_string(),
            request_format: RequestFormat::Auto,
            provider_id: None,
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
        };

        assert_eq!(
            model_refresh_strategies(&input),
            vec![
                ModelRefreshStrategy::AnthropicNative,
                ModelRefreshStrategy::OpenAi,
                ModelRefreshStrategy::GeminiNative,
            ]
        );
    }

    #[test]
    fn model_refresh_strategies_should_only_use_codex_builtin_when_selected_or_inferred() {
        let codex_input = RefreshModelsInput {
            base_url: DEFAULT_CODEX_BASE_URL.to_string(),
            api_key: String::new(),
            request_format: RequestFormat::Auto,
            provider_id: None,
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
        };
        let openai_input = RefreshModelsInput {
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            request_format: RequestFormat::OpenAI,
            provider_id: None,
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
        };

        assert!(
            model_refresh_strategies(&codex_input).contains(&ModelRefreshStrategy::CodexBuiltin)
        );
        assert!(
            !model_refresh_strategies(&openai_input).contains(&ModelRefreshStrategy::CodexBuiltin)
        );
    }

    fn test_codex_jwt(payload: serde_json::Value) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let body = URL_SAFE_NO_PAD.encode(payload.to_string());
        format!("{header}.{body}.signature")
    }

    #[test]
    fn codex_parse_local_auth_file_should_read_nested_tokens() {
        let temp_root = std::env::temp_dir().join(format!("easy-call-ai-codex-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        let path = temp_root.join("auth.json");
        let access_token = test_codex_jwt(serde_json::json!({
            "exp": 2000000000,
            "email": "access@example.com",
            "chatgpt_account_id": "acc-from-access"
        }));
        let id_token = test_codex_jwt(serde_json::json!({
            "email": "id@example.com",
            "chatgpt_account_id": "acc-from-id"
        }));
        std::fs::write(
            &path,
            serde_json::json!({
                "tokens": {
                    "access_token": access_token,
                    "refresh_token": "refresh-nested",
                    "id_token": id_token
                }
            })
            .to_string(),
        )
        .expect("write auth file");

        let credential =
            codex_parse_local_auth_file(path.to_string_lossy().as_ref()).expect("parse nested auth");

        let _ = std::fs::remove_dir_all(&temp_root);
        assert_eq!(credential.refresh_token, "refresh-nested");
        assert_eq!(credential.account_id, "acc-from-id");
        assert_eq!(credential.email, "id@example.com");
        assert_eq!(credential.expires_at_ms, 2_000_000_000_000);
    }

    #[test]
    fn codex_parse_local_auth_file_should_read_flat_tokens() {
        let temp_root = std::env::temp_dir().join(format!("easy-call-ai-codex-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_root).expect("create temp dir");
        let path = temp_root.join("auth.json");
        let access_token = test_codex_jwt(serde_json::json!({
            "exp": 1990000000,
            "email": "access@example.com",
            "chatgpt_account_id": "acc-from-access"
        }));
        std::fs::write(
            &path,
            serde_json::json!({
                "access_token": access_token,
                "refresh_token": "refresh-flat",
                "account_id": "acc-flat",
                "email": "flat@example.com",
                "expired": "2033-05-18T03:33:20Z",
                "type": "codex"
            })
            .to_string(),
        )
        .expect("write auth file");

        let credential =
            codex_parse_local_auth_file(path.to_string_lossy().as_ref()).expect("parse flat auth");

        let _ = std::fs::remove_dir_all(&temp_root);
        assert_eq!(credential.refresh_token, "refresh-flat");
        assert_eq!(credential.account_id, "acc-flat");
        assert_eq!(credential.email, "flat@example.com");
        assert_eq!(credential.expires_at_ms, 2_000_000_000_000);
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
    fn cleanup_portable_update_temp_artifacts_should_keep_backups_and_log() {
        let temp_root = std::env::temp_dir().join(format!("easy-call-ai-updater-{}", Uuid::new_v4()));
        let backups_dir = temp_root.join("backups");
        let staging_dir = temp_root.join("staging-0.9.9");
        std::fs::create_dir_all(&backups_dir).expect("create backups dir");
        std::fs::create_dir_all(&staging_dir).expect("create staging dir");
        std::fs::write(temp_root.join("p-ai-portable-0.9.9.zip"), b"zip").expect("write zip");
        std::fs::write(temp_root.join("portable-helper-test.exe"), b"helper").expect("write helper");
        std::fs::write(temp_root.join("portable-plan-test.json"), b"{}").expect("write plan");
        std::fs::write(temp_root.join("portable-update.log"), b"log").expect("write log");
        std::fs::write(temp_root.join("other-file.tmp"), b"tmp").expect("write other file");

        cleanup_portable_update_temp_artifacts(&temp_root);

        assert!(backups_dir.exists());
        assert!(temp_root.join("portable-update.log").exists());
        assert!(temp_root.join("other-file.tmp").exists());
        assert!(!staging_dir.exists());
        assert!(!temp_root.join("p-ai-portable-0.9.9.zip").exists());
        assert!(!temp_root.join("portable-helper-test.exe").exists());
        assert!(!temp_root.join("portable-plan-test.json").exists());

        let _ = std::fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn github_auto_update_cooldown_active_should_only_block_within_window() {
        let now = now_utc();
        assert!(!github_auto_update_cooldown_active(None, now));
        assert!(github_auto_update_cooldown_active(
            Some(now - time::Duration::hours(GITHUB_AUTO_UPDATE_COOLDOWN_HOURS - 1)),
            now,
        ));
        assert!(!github_auto_update_cooldown_active(
            Some(now - time::Duration::hours(GITHUB_AUTO_UPDATE_COOLDOWN_HOURS)),
            now,
        ));
    }

    #[test]
    fn skipped_auto_update_result_should_use_current_version_and_report_no_update() {
        let result = build_skipped_auto_update_result(UpdateRuntimeKind::Portable);

        assert_eq!(result.current_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(result.latest_version, env!("CARGO_PKG_VERSION"));
        assert!(!result.has_update);
        assert_eq!(result.update_source, "cooldown");
        assert_eq!(result.runtime_kind, "portable");
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
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now.clone(),
            updated_at: now,
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: Vec::new(),
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
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
    fn build_compaction_message_should_not_include_todo_snapshot_or_user_profile_section() {
        let message = build_compaction_message(
            "这里是压缩摘要",
            Some("当前标题"),
            "manual",
            Some("用户：继续推进\n助手甲：我来接着处理"),
            &[],
        );
        let text = message
            .parts
            .iter()
            .find_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .expect("compaction text");

        assert!(!text.contains("当前会话标题："));
        assert!(!text.contains("当前标题"));
        assert!(!text.contains("## 用户画像"));
        assert!(text.contains("## 摘要说明"));
        assert!(text.contains("## 摘要正文"));
        assert!(text.contains("## 保留对话"));
        assert!(!text.contains("## Current Todo List"));
        assert!(text.contains("用户：继续推进\n助手甲：我来接着处理"));
    }

    #[test]
    fn build_compaction_message_should_keep_blank_line_before_active_plans() {
        let message = build_compaction_message(
            "这里是压缩摘要\n\n<active_plans>\n<active_plan index=\"1\">\n执行计划\n</active_plan>\n</active_plans>",
            Some("计划标题"),
            "manual",
            None,
            &[],
        );
        let text = message
            .parts
            .iter()
            .find_map(|part| match part {
                MessagePart::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .expect("compaction text");

        assert!(text.contains("这里是压缩摘要\n\n<active_plans>"));
        let provider_meta = message.provider_meta.expect("provider meta");
        let schema_version = provider_meta
            .get("message_meta")
            .and_then(|value| value.get("schemaVersion"))
            .and_then(Value::as_u64);
        assert_eq!(schema_version, Some(SUMMARY_CONTEXT_MESSAGE_SCHEMA_VERSION));
    }

    #[test]
    fn build_compaction_preserved_dialogue_block_should_use_token_budget_and_skip_compaction() {
        let now = now_iso();
        let long_middle = "中间消息".repeat(200);
        let latest_user = "最后一条用户消息";
        let latest_assistant = "最后一条助手消息";
        let latest_user_line = format!("用户：{latest_user}");
        let latest_assistant_line = format!("助手：{latest_assistant}");
        let budget = estimated_tokens_for_text(&latest_user_line).ceil() as usize
            + estimated_tokens_for_text(&latest_assistant_line).ceil() as usize
            + estimated_tokens_for_text("\n").ceil() as usize;
        let conversation = Conversation {
            id: "conversation-token-budget".to_string(),
            title: "token budget".to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            archived_at: None,
            messages: vec![
                ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "user".to_string(),
                    created_at: now.clone(),
                    speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
                    parts: vec![MessagePart::Text {
                        text: "[上下文整理]\n旧摘要".to_string(),
                reasoning_content: None,
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
                meme_annotations: None,
                },
                ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "user".to_string(),
                    created_at: now.clone(),
                    speaker_agent_id: Some(USER_PERSONA_ID.to_string()),
                    parts: vec![MessagePart::Text {
                        text: long_middle.clone(),
                reasoning_content: None,
            }],
                    extra_text_blocks: Vec::new(),
                    provider_meta: None,
                    tool_call: None,
                    mcp_call: None,
                meme_annotations: None,
                },
                ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "user".to_string(),
                    created_at: now.clone(),
                    speaker_agent_id: Some(USER_PERSONA_ID.to_string()),
                    parts: vec![MessagePart::Text {
                        text: latest_user.to_string(),
                reasoning_content: None,
            }],
                    extra_text_blocks: Vec::new(),
                    provider_meta: None,
                    tool_call: None,
                    mcp_call: None,
                meme_annotations: None,
                },
                ChatMessage {
                    id: Uuid::new_v4().to_string(),
                    role: "assistant".to_string(),
                    created_at: now,
                    speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                    parts: vec![MessagePart::Text {
                        text: latest_assistant.to_string(),
                reasoning_content: None,
            }],
                    extra_text_blocks: Vec::new(),
                    provider_meta: None,
                    tool_call: None,
                    mcp_call: None,
                meme_annotations: None,
                },
            ],
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        };

        let preserved = collect_block_preserved_dialogue(
            &conversation.messages,
            "用户",
            "助手",
            PreservedDialogueBudget::Tokens(budget),
        );

        assert!(preserved.contains("用户：最后一条用户消息"));
        assert!(preserved.contains("助手：最后一条助手消息"));
        assert!(!preserved.contains(&long_middle));
        assert!(!preserved.contains("旧摘要"));
    }

    #[test]
    fn compaction_message_plain_text_should_keep_assistant_tool_round_text_before_final_text() {
        let now = now_iso();
        let message = ChatMessage {
            id: "assistant-with-tool-round-text".to_string(),
            role: "assistant".to_string(),
            created_at: now,
            speaker_agent_id: Some(DEFAULT_AGENT_ID.to_string()),
            parts: vec![MessagePart::Text {
                text: "问题出在默认配置没有同步更新。".to_string(),
                reasoning_content: Some("这是最终答复思维内容，不应进入保留对话。".to_string()),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: Some(vec![
                serde_json::json!({
                    "role": "assistant",
                    "content": "我先读取配置文件。",
                    "reasoning_content": "先检查配置文件。",
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"config.toml\"}"
                        }
                    }]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call-1",
                    "content": "不应保留的配置文件工具结果"
                }),
                serde_json::json!({
                    "role": "assistant",
                    "content": "接着检查默认值。",
                    "reasoning_content": "继续搜索默认实现。",
                    "tool_calls": [{
                        "id": "call-2",
                        "type": "function",
                        "function": {
                            "name": "search",
                            "arguments": "{\"query\":\"Default\"}"
                        }
                    }]
                }),
                serde_json::json!({
                    "role": "tool",
                    "tool_call_id": "call-2",
                    "content": "不应保留的搜索工具结果"
                }),
            ]),
            mcp_call: None,
            meme_annotations: None,
        };

        let text = archive_pipeline_message_plain_text(&message);

        assert_eq!(
            text,
            "我先读取配置文件。 接着检查默认值。 问题出在默认配置没有同步更新。"
        );
        assert!(!text.contains("工具结果"));
        assert!(!text.contains("思维内容"));
        assert!(!text.contains("read_file"));
    }

    #[test]
    fn native_notification_text_excerpt_should_trim_blank_lines_and_limit_length() {
        let text = "\n  第一行  \n\n 第二行 \n";
        assert_eq!(native_notification_text_excerpt(text, 80), "第一行\n第二行");

        let truncated = native_notification_text_excerpt("123456", 4);
        assert_eq!(truncated, "1234...");
    }