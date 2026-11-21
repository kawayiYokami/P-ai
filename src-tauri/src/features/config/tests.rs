    #[test]
    fn image_text_cache_upsert_and_find_should_work() {
        let state = storage_and_stt_test_state();
        state_service_upsert_image_text_cache(&state, "h1", "vision-a", "image", "", "text-a")
            .expect("upsert cache");
        assert_eq!(
            state_service_find_image_text_cache(&state, "h1", "vision-a", "image", "")
                .expect("find cache"),
            Some("text-a".to_string())
        );

        state_service_upsert_image_text_cache(&state, "h1", "vision-a", "image", "", "text-b")
            .expect("upsert cache");
        assert_eq!(
            state_service_find_image_text_cache(&state, "h1", "vision-a", "image", "")
                .expect("find cache"),
            Some("text-b".to_string())
        );
        assert_eq!(
            state_service_find_image_text_cache(&state, "h1", "vision-b", "image", "")
                .expect("find cache"),
            None
        );
    }

    #[test]
    fn image_text_cache_should_isolate_entries_by_image_type() {
        let state = storage_and_stt_test_state();
        state_service_upsert_image_text_cache(&state, "h1", "vision-a", "image", "", "text-image")
            .expect("upsert image type cache");
        state_service_upsert_image_text_cache(&state, "h1", "vision-a", "chart", "", "text-chart")
            .expect("upsert chart type cache");
        assert_eq!(
            state_service_find_image_text_cache(&state, "h1", "vision-a", "image", "")
                .expect("find image type cache"),
            Some("text-image".to_string())
        );
        assert_eq!(
            state_service_find_image_text_cache(&state, "h1", "vision-a", "chart", "")
                .expect("find chart type cache"),
            Some("text-chart".to_string())
        );
        assert_eq!(
            state_service_find_image_text_cache(&state, "h1", "vision-a", "screenshot", "")
                .expect("find other type cache"),
            None
        );
    }

    #[test]
    fn compute_image_hash_hex_should_be_stable() {
        let png_1x1_red = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO9WfXkAAAAASUVORK5CYII=";
        let part = BinaryPart {
            mime: "image/png".to_string(),
            bytes_base64: png_1x1_red.to_string(),
            saved_path: None,
        };
        let h1 = compute_image_hash_hex(&part).expect("hash1");
        let h2 = compute_image_hash_hex(&part).expect("hash2");
        assert_eq!(h1, h2);
        assert!(!h1.is_empty());
    }

    #[test]
    fn startup_window_label_should_open_main_without_usable_text_llm() {
        let mut cfg = AppConfig::default();
        normalize_app_config(&mut cfg);
        assert_eq!(startup_window_label_for_config(&cfg), "main");

        let api_id = cfg.expert_api_config_id.clone();
        let api = cfg
            .api_configs
            .iter_mut()
            .find(|item| item.id == api_id)
            .expect("default chat api exists");
        api.base_url = "https://api.deepseek.com/v1".to_string();
        api.model = "deepseek-chat".to_string();
        api.api_key = "sk-test".to_string();
        assert_eq!(startup_window_label_for_config(&cfg), "chat");
    }

    #[test]
    fn normalize_app_config_should_not_promote_legacy_video_capability_to_audio() {
        let mut cfg = AppConfig::default();
        let provider = cfg
            .api_providers
            .first_mut()
            .expect("default provider exists");
        provider.enable_audio = true;
        let model = provider.models.first_mut().expect("default model exists");
        model.enable_audio = false;
        model.enable_video = true;

        normalize_app_config(&mut cfg);

        let provider = cfg
            .api_providers
            .first()
            .expect("default provider exists after normalization");
        assert!(!provider.models[0].enable_audio);
        assert!(!provider.enable_audio);
        assert!(provider.models[0].enable_video);
    }

    #[test]
    fn legacy_api_config_video_capability_should_not_migrate_to_audio() {
        let mut cfg = AppConfig::default();
        cfg.api_providers.clear();
        let api = cfg.api_configs.first_mut().expect("default API config exists");
        api.enable_audio = true;
        api.enable_video = true;

        normalize_app_config(&mut cfg);

        let provider = cfg
            .api_providers
            .first()
            .expect("legacy API config should migrate to provider");
        assert!(!provider.models[0].enable_audio);
        assert!(provider.models[0].enable_video);
    }

    #[test]
    fn startup_window_label_should_allow_codex_local_auth_without_api_key() {
        let mut cfg = AppConfig::default();
        let api_id = cfg.expert_api_config_id.clone();
        let api = cfg
            .api_configs
            .iter_mut()
            .find(|item| item.id == api_id)
            .expect("default chat api exists");
        api.request_format = RequestFormat::Codex;
        api.base_url = DEFAULT_CODEX_BASE_URL.to_string();
        api.model = "gpt-5.4".to_string();
        api.api_key.clear();
        api.codex_auth_mode = CODEX_AUTH_MODE_READ_LOCAL.to_string();
        normalize_app_config(&mut cfg);
        assert_eq!(startup_window_label_for_config(&cfg), "chat");
    }

    #[test]
    fn startup_window_label_should_not_require_expert_model_binding() {
        // 专家模型已改为被动跟随的隐形兜底，为空不代表用户没配模型。
        // 拿它做启动判据会在「专家模型尚未确立」时把人踢回设置页。
        let mut cfg = AppConfig::default();
        let api_id = cfg.expert_api_config_id.clone();
        let api = cfg
            .api_configs
            .iter_mut()
            .find(|item| item.id == api_id)
            .expect("default chat api exists");
        api.api_key = "sk-test".to_string();
        cfg.expert_api_config_id.clear();
        assert_eq!(startup_window_label_for_config(&cfg), "chat");
    }

    #[test]
    fn startup_window_label_should_fall_back_when_no_text_model_is_usable() {
        let mut cfg = AppConfig::default();
        for api in cfg.api_configs.iter_mut() {
            api.api_key.clear();
        }
        cfg.expert_api_config_id.clear();
        assert_eq!(startup_window_label_for_config(&cfg), "main");
    }

    #[test]
    fn normalize_app_config_should_fix_invalid_record_and_stt_fields() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            ui_language: default_ui_language(),
            ui_font: default_ui_font(),
            code_font: default_code_font(),
            ui_size_scale: default_ui_size_scale(),
            web_access_port: default_web_access_port(),
            web_access_enabled: default_web_access_enabled(),
            web_access_password: default_web_access_password(),
            github_update_method: default_github_update_method(),
            skipped_github_update_version: String::new(),
            record_hotkey: "".to_string(),
            record_background_wake_enabled: false,
            min_record_seconds: 0,
            max_record_seconds: 0,
            tool_max_iterations: 0,
            llm_round_log_capacity: 9,
            message_notification_enabled: default_message_notification_enabled(),
            message_notification_sound_enabled: default_message_notification_sound_enabled(),
            desktop_operation_notice_enabled: default_desktop_operation_notice_enabled(),
            desktop_operate_enabled: default_desktop_operate_enabled(),
            expert_api_config_id: "a1".to_string(),
            simple_setup_mode: false,
            vision_api_config_id: None,
            image_generation_model_id: None,
            image_to_video_model_id: None,
            image_providers: Vec::new(),
            stt_api_config_id: None,
            stt_auto_send: false,
            provider_non_stream_base_urls: Vec::new(),
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: Vec::new(),
            remote_im_channels: Vec::new(),
            search_engines: default_search_engines(),
            search_mode: default_search_mode(),
            api_configs: vec![
                ApiConfig {
                    id: "a1".to_string(),
                    name: "chat".to_string(),
                    request_format: RequestFormat::OpenAI,
                    allow_concurrent_requests: false,
                    max_concurrent_requests: None,
                    enable_text: true,
                    enable_image: true,
                    enable_audio: false,
                    enable_video: false,
                    enable_tools: false,
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
                    codex_custom_url: None,
                    codex_custom_api_key: None,
                    codex_originator: default_codex_originator(),
                    codex_residency_requirement: None,
                    model: "m".to_string(),
                    reasoning_effort: default_reasoning_effort(),
                    temperature: 1.0,
                    custom_temperature_enabled: false,
                    context_window_tokens: 128_000,
                    max_output_tokens: 4_096,
                    custom_max_output_tokens_enabled: false,
                    failure_retry_count: 999,
                },
                ApiConfig {
                    id: "a2".to_string(),
                    name: "bad-stt".to_string(),
                    request_format: RequestFormat::OpenAI,
                    allow_concurrent_requests: false,
                    max_concurrent_requests: None,
                    enable_text: true,
                    enable_image: false,
                    enable_audio: true,
                    enable_video: false,
                    enable_tools: false,
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
                    codex_custom_url: None,
                    codex_custom_api_key: None,
                    codex_originator: default_codex_originator(),
                    codex_residency_requirement: None,
                    model: "m".to_string(),
                    reasoning_effort: default_reasoning_effort(),
                    temperature: 1.0,
                    custom_temperature_enabled: false,
                    context_window_tokens: 128_000,
                    max_output_tokens: 4_096,
                    custom_max_output_tokens_enabled: false,
                    failure_retry_count: 0,
                },
            ],
            api_providers: Vec::new(),
            tool_review_api_config_id: None,
        };
        normalize_app_config(&mut cfg);
        assert_eq!(cfg.record_hotkey, "");
        assert_eq!(cfg.min_record_seconds, 1);
        assert!(cfg.max_record_seconds >= cfg.min_record_seconds);
        assert_eq!(cfg.tool_max_iterations, 1);
        assert_eq!(cfg.llm_round_log_capacity, 3);
        assert_eq!(cfg.api_configs[0].failure_retry_count, 20);
        assert!(!cfg.stt_auto_send);
    }

    #[test]
    fn normalize_app_config_should_not_bind_chat_api_to_selected_api() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            ui_language: default_ui_language(),
            ui_font: default_ui_font(),
            code_font: default_code_font(),
            ui_size_scale: default_ui_size_scale(),
            web_access_port: default_web_access_port(),
            web_access_enabled: default_web_access_enabled(),
            web_access_password: default_web_access_password(),
            github_update_method: default_github_update_method(),
            skipped_github_update_version: String::new(),
            record_hotkey: "Alt".to_string(),
            record_background_wake_enabled: false,
            min_record_seconds: 1,
            max_record_seconds: 60,
            tool_max_iterations: 10,
            llm_round_log_capacity: default_llm_round_log_capacity(),
            message_notification_enabled: default_message_notification_enabled(),
            message_notification_sound_enabled: default_message_notification_sound_enabled(),
            desktop_operation_notice_enabled: default_desktop_operation_notice_enabled(),
            desktop_operate_enabled: default_desktop_operate_enabled(),
            expert_api_config_id: "chat-a".to_string(),
            vision_api_config_id: None,
            image_generation_model_id: None,
            image_to_video_model_id: None,
            image_providers: Vec::new(),
            stt_api_config_id: None,
            simple_setup_mode: false,
            stt_auto_send: false,
            provider_non_stream_base_urls: Vec::new(),
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: Vec::new(),
            remote_im_channels: Vec::new(),
            search_engines: default_search_engines(),
            search_mode: default_search_mode(),
            api_configs: vec![
                ApiConfig {
                    id: "chat-a".to_string(),
                    name: "chat-a".to_string(),
                    request_format: RequestFormat::OpenAI,
                    allow_concurrent_requests: false,
                    max_concurrent_requests: None,
                    enable_text: true,
                    enable_image: true,
                    enable_audio: true,
                    enable_video: false,
                    enable_tools: false,
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
                    codex_custom_url: None,
                    codex_custom_api_key: None,
                    codex_originator: default_codex_originator(),
                    codex_residency_requirement: None,
                    model: "m".to_string(),
                    reasoning_effort: default_reasoning_effort(),
                    temperature: 1.0,
                    custom_temperature_enabled: false,
                    context_window_tokens: 128_000,
                    max_output_tokens: 4_096,
                    custom_max_output_tokens_enabled: false,
                    failure_retry_count: 0,
                },
                ApiConfig {
                    id: "edit-b".to_string(),
                    name: "edit-b".to_string(),
                    request_format: RequestFormat::OpenAI,
                    allow_concurrent_requests: false,
                    max_concurrent_requests: None,
                    enable_text: true,
                    enable_image: false,
                    enable_audio: false,
                    enable_video: false,
                    enable_tools: false,
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
                    codex_custom_url: None,
                    codex_custom_api_key: None,
                    codex_originator: default_codex_originator(),
                    codex_residency_requirement: None,
                    model: "m".to_string(),
                    reasoning_effort: default_reasoning_effort(),
                    temperature: 1.0,
                    custom_temperature_enabled: false,
                    context_window_tokens: 128_000,
                    max_output_tokens: 4_096,
                    custom_max_output_tokens_enabled: false,
                    failure_retry_count: 0,
                },
            ],
            api_providers: Vec::new(),
            tool_review_api_config_id: None,
        };
        normalize_app_config(&mut cfg);
        assert_eq!(
            cfg.expert_api_config_id,
            "chat-a::chat-a-model-default".to_string()
        );
    }

    #[test]
    fn normalize_app_config_should_keep_provider_without_models() {
        let provider_id = "provider-without-models".to_string();
        let mut cfg = AppConfig {
            expert_api_config_id: String::new(),
            api_providers: vec![ApiProviderConfig {
                id: provider_id.clone(),
                name: "No Models".to_string(),
                deprecated: false,
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_video: false,
                enable_tools: true,
                base_url: "https://api.openai.com/v1".to_string(),
                login_provider: String::new(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                codex_custom_url: None,
                codex_custom_api_key: None,
                codex_originator: default_codex_originator(),
                codex_residency_requirement: None,
                api_keys: vec!["key-1".to_string()],
                key_cursor: 0,
                cached_model_options: Vec::new(),
                // 供应商可以没有模型
                models: Vec::new(),
                failure_retry_count: 0,
                client_user_agent: None,
            }],
            api_configs: Vec::new(),
            ..AppConfig::default()
        };

        normalize_app_config(&mut cfg);

        // 已有供应商（即使没有模型）必须原样保留，不得被默认供应商覆盖。
        assert_eq!(cfg.api_providers.len(), 1);
        assert_eq!(cfg.api_providers[0].id, provider_id);
    }

    #[test]
    fn startup_self_check_should_be_noop_after_deputy_semantics_removed() {
        let mut cfg = AppConfig::default();
        let snapshot = serde_json::to_string(&cfg).expect("config snapshot");
        assert!(!run_startup_self_checks(&mut cfg));
        assert_eq!(
            snapshot,
            serde_json::to_string(&cfg).expect("config snapshot after self check")
        );
    }

    #[test]
    fn app_data_default_should_include_deputy_agent() {
        let data = AppData::default();
        assert!(data.agents.iter().any(|agent| agent.id == DEPUTY_AGENT_ID));
    }

    #[test]
    fn resolve_api_config_should_preserve_openai_reasoning_none_for_runtime() {
        let mut cfg = AppConfig::default();
        let api = cfg
            .api_configs
            .iter_mut()
            .find(|item| item.id == cfg.expert_api_config_id)
            .expect("default api exists");
        api.request_format = RequestFormat::OpenAI;
        api.base_url = "https://api.deepseek.com/v1".to_string();
        api.api_key = "sk-test".to_string();
        api.model = "deepseek-v4-pro".to_string();
        api.reasoning_effort = "none".to_string();

        normalize_app_config(&mut cfg);

        let resolved = resolve_api_config(&cfg, Some(&cfg.expert_api_config_id))
            .expect("resolved api config");

        assert_eq!(resolved.reasoning_effort, Some("none".to_string()));
    }

    #[test]
    fn normalize_terminal_path_input_should_strip_wrapping_quotes() {
        let out = normalize_terminal_path_input_for_current_platform(r#""./repo""#);
        assert_eq!(out, "./repo".to_string());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn normalize_terminal_path_input_should_convert_git_bash_style_on_windows() {
        let out = normalize_terminal_path_input_for_current_platform("/e/work/repo");
        assert_eq!(out, r"E:\work\repo".to_string());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn normalize_shell_workspaces_should_convert_and_dedup_windows_paths() {
        let mut cfg = AppConfig::default();
        cfg.shell_workspaces = vec![
            ShellWorkspaceConfig {
                name: "A".to_string(),
                path: "/e/__easy_call_ai_path_norm_test__/repo".to_string(),
                built_in: false,
                ..Default::default()
            },
            ShellWorkspaceConfig {
                name: "a".to_string(),
                path: "E:/__easy_call_ai_path_norm_test__/repo".to_string(),
                built_in: false,
                ..Default::default()
            },
            ShellWorkspaceConfig {
                name: "B".to_string(),
                path: r#""E:\__easy_call_ai_path_norm_test__\repo""#.to_string(),
                built_in: false,
                ..Default::default()
            },
        ];
        normalize_shell_workspaces(&mut cfg);
        assert_eq!(cfg.shell_workspaces.len(), 1);
        assert_eq!(
            cfg.shell_workspaces[0].path,
            r"E:\__easy_call_ai_path_norm_test__\repo".to_string()
        );
    }

    #[test]
    fn normalize_app_config_should_migrate_legacy_api_configs_into_providers() {
        let mut cfg = AppConfig {
            expert_api_config_id: "legacy-openai".to_string(),
            api_providers: Vec::new(),
            tool_review_api_config_id: None,
            api_configs: vec![ApiConfig {
                id: "legacy-openai".to_string(),
                name: "Legacy OpenAI".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_video: false,
                enable_tools: true,
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "legacy-key".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                codex_custom_url: None,
                codex_custom_api_key: None,
                codex_originator: default_codex_originator(),
                codex_residency_requirement: None,
                model: "gpt-4.1".to_string(),
                reasoning_effort: default_reasoning_effort(),
                temperature: 0.7,
                custom_temperature_enabled: true,
                context_window_tokens: 256_000,
                max_output_tokens: 8_192,
                custom_max_output_tokens_enabled: true,
                failure_retry_count: 2,
            }],
            ..AppConfig::default()
        };

        normalize_app_config(&mut cfg);

        assert_eq!(cfg.api_providers.len(), 1);
        assert_eq!(cfg.api_providers[0].api_keys, vec!["legacy-key".to_string()]);
        assert_eq!(cfg.api_providers[0].models.len(), 1);
        assert_eq!(cfg.api_providers[0].models[0].model, "gpt-4.1".to_string());
        assert_eq!(cfg.api_configs.len(), 1);
        assert_eq!(cfg.api_configs[0].id, cfg.expert_api_config_id);
    }

    #[test]
    fn normalize_app_config_should_migrate_legacy_api_configs_when_serde_injected_default_provider() {
        let mut cfg: AppConfig = toml::from_str(
            r#"
hotkey = "Alt+·"
assistantDepartmentApiConfigId = "legacy-openai"

[[apiConfigs]]
id = "legacy-openai"
name = "Legacy OpenAI"
requestFormat = "openai"
enableText = true
enableImage = false
enableAudio = false
enableTools = true
baseUrl = "https://api.openai.com/v1"
apiKey = "legacy-key"
model = "gpt-4.1"
temperature = 0.7
contextWindowTokens = 256000
maxOutputTokens = 8192
"#,
        )
        .expect("legacy toml should deserialize");

        normalize_app_config(&mut cfg);

        assert_eq!(cfg.api_providers.len(), 1);
        assert_eq!(cfg.api_providers[0].id, "legacy-openai".to_string());
        assert_eq!(cfg.api_providers[0].api_keys, vec!["legacy-key".to_string()]);
        assert_eq!(cfg.api_providers[0].models.len(), 1);
        assert_eq!(cfg.api_providers[0].models[0].model, "gpt-4.1".to_string());
    }

    #[test]
    fn provider_without_concurrency_fields_defaults_to_unlimited() {
        let provider: ApiProviderConfig = serde_json::from_value(serde_json::json!({
            "id": "p",
            "name": "P",
            "baseUrl": "https://example.invalid/v1",
        }))
        .expect("provider json");
        assert!(provider.allow_concurrent_requests);
        assert_eq!(provider.max_concurrent_requests, None);
    }

    #[test]
    fn normalize_app_config_upgrades_legacy_serial_default_to_unlimited() {
        let mut provider = ApiProviderConfig::default();
        provider.id = "user-provider".to_string();
        provider.name = "User".to_string();
        provider.allow_concurrent_requests = false;
        provider.max_concurrent_requests = None;
        let mut cfg = AppConfig {
            api_providers: vec![provider],
            api_configs: Vec::new(),
            ..AppConfig::default()
        };

        normalize_app_config(&mut cfg);

        assert!(cfg.api_providers[0].allow_concurrent_requests);
        assert_eq!(cfg.api_providers[0].max_concurrent_requests, None);
        assert!(cfg.api_configs.iter().all(|api| api.allow_concurrent_requests && api.max_concurrent_requests.is_none()));
    }

    #[test]
    fn normalize_app_config_keeps_explicit_serial_limit() {
        let mut provider = ApiProviderConfig::default();
        provider.id = "user-provider".to_string();
        provider.name = "User".to_string();
        provider.allow_concurrent_requests = true;
        provider.max_concurrent_requests = Some(1);
        let mut cfg = AppConfig {
            api_providers: vec![provider],
            api_configs: Vec::new(),
            ..AppConfig::default()
        };

        normalize_app_config(&mut cfg);

        assert_eq!(cfg.api_providers[0].max_concurrent_requests, Some(1));
        assert_eq!(cfg.api_configs[0].max_concurrent_requests, Some(1));
    }

    #[test]
    fn app_config_should_ignore_legacy_selected_api_config_id_in_save_payload() {
        let cfg: AppConfig = serde_json::from_str(
            r#"{"hotkey":"Alt+·","selectedApiConfigId":"legacy-openai"}"#,
        )
        .expect("旧字段 selectedApiConfigId 应被忽略");
        assert_eq!(cfg.expert_api_config_id, "");
    }

    #[test]
    fn read_config_should_materialize_missing_model_enable_audio_as_false() {
        let root = std::env::temp_dir().join(format!("eca-config-enable-audio-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp config dir");
        let config_path = root.join("app_config.toml");
        std::fs::write(
            &config_path,
            r#"
hotkey = "Alt+·"
assistantDepartmentApiConfigId = "provider-a::model-a"

[[apiProviders]]
id = "provider-a"
name = "Provider A"
requestFormat = "openai"
enableText = true
enableImage = false
enableAudio = false
enableVideo = false
enableTools = true
baseUrl = "https://example.com/v1"
apiKeys = ["k"]
cachedModelOptions = ["mimo-v2.5"]

[[apiProviders.models]]
id = "model-a"
model = "mimo-v2.5"
enableImage = true
enableVideo = false
enableTools = true
"#,
        )
        .expect("write config");

        let cfg = read_config(&config_path).expect("read config");
        let provider = cfg
            .api_providers
            .iter()
            .find(|item| item.id == "provider-a")
            .expect("provider-a exists");
        let model = provider
            .models
            .iter()
            .find(|item| item.id == "model-a")
            .expect("model-a exists");
        assert!(!model.enable_audio);

        let persisted = std::fs::read_to_string(&config_path).expect("read persisted config");
        assert!(persisted.contains("enableAudio = false"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn app_config_should_map_legacy_ui_size_presets_to_scales() {
        for (preset, expected_scale) in [("small", 75), ("default", 100), ("large", 125), ("extraLarge", 150)] {
            let mut doc = toml::Value::try_from(AppConfig::default()).expect("serialize default config");
            let table = doc.as_table_mut().expect("config is a TOML table");
            table.remove("uiSizeScale");
            table.insert("uiSizePreset".to_string(), toml::Value::String(preset.to_string()));

            let config: AppConfig = doc.try_into().expect("legacy preset should deserialize");
            assert_eq!(config.ui_size_scale, expected_scale, "preset: {preset}");
        }
    }

    #[test]
    fn consume_api_key_for_request_should_rotate_provider_keys_across_same_provider_models() {
        let provider_id = format!(
            "provider-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or(0)
        );
        let model_a = "model-a".to_string();
        let model_b = "model-b".to_string();
        let mut cfg = AppConfig {
            expert_api_config_id: api_endpoint_id(&provider_id, &model_a),
            api_providers: vec![ApiProviderConfig {
                id: provider_id.clone(),
                name: "OpenAI".to_string(),
                deprecated: false,
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_video: false,
                enable_tools: true,
                base_url: "https://api.openai.com/v1".to_string(),
                login_provider: String::new(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                codex_custom_url: None,
                codex_custom_api_key: None,
                codex_originator: default_codex_originator(),
                codex_residency_requirement: None,
                api_keys: vec!["key-1".to_string(), "key-2".to_string()],
                key_cursor: 0,
                cached_model_options: vec!["gpt-4.1".to_string(), "gpt-4.1-mini".to_string()],
                models: vec![
                    ApiModelConfig {
                        id: model_a.clone(),
                        model: "gpt-4.1".to_string(),
                        display_name: String::new(),
                        deprecated: false,
                        enable_image: false,
                        enable_audio: false,
                        enable_video: false,
                        enable_tools: true,
                        reasoning_effort: default_reasoning_effort(),
                        temperature: 1.0,
                        custom_temperature_enabled: false,
                        context_window_tokens: 128_000,
                        max_output_tokens: 4_096,
                        custom_max_output_tokens_enabled: false,
                    },
                    ApiModelConfig {
                        id: model_b.clone(),
                        model: "gpt-4.1-mini".to_string(),
                        display_name: String::new(),
                        deprecated: false,
                        enable_image: false,
                        enable_audio: false,
                        enable_video: false,
                        enable_tools: true,
                        reasoning_effort: default_reasoning_effort(),
                        temperature: 1.0,
                        custom_temperature_enabled: false,
                        context_window_tokens: 128_000,
                        max_output_tokens: 4_096,
                        custom_max_output_tokens_enabled: false,
                    },
                ],
                failure_retry_count: 0,
                client_user_agent: None,
            }],
            api_configs: Vec::new(),
            ..AppConfig::default()
        };
        normalize_app_config(&mut cfg);

        let first = resolve_api_config(&cfg, Some(&api_endpoint_id(&provider_id, &model_a)))
            .expect("first resolve");
        let second = resolve_api_config(&cfg, Some(&api_endpoint_id(&provider_id, &model_b)))
            .expect("second resolve");
        let third = resolve_api_config(&cfg, Some(&api_endpoint_id(&provider_id, &model_a)))
            .expect("third resolve");

        assert_eq!(first.api_key, "key-1".to_string());
        assert_eq!(second.api_key, "key-1".to_string());
        assert_eq!(third.api_key, "key-1".to_string());

        let first_sent = consume_api_key_for_request(&first);
        let second_sent = consume_api_key_for_request(&second);
        let third_sent = consume_api_key_for_request(&third);

        assert_eq!(first_sent, "key-1".to_string());
        assert_eq!(second_sent, "key-2".to_string());
        assert_eq!(third_sent, "key-1".to_string());
    }

    #[test]
    fn resolve_api_config_should_use_codex_custom_api_key_for_custom_url_mode() {
        let provider_id = "codex-custom-provider".to_string();
        let model_id = "codex-model".to_string();
        let mut cfg = AppConfig {
            expert_api_config_id: api_endpoint_id(&provider_id, &model_id),
            api_providers: vec![ApiProviderConfig {
                id: provider_id.clone(),
                name: "SharedChat".to_string(),
                deprecated: false,
                request_format: RequestFormat::Codex,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_video: false,
                enable_tools: true,
                base_url: "https://new.sharedchat.cc/codex".to_string(),
                login_provider: String::new(),
                codex_auth_mode: CODEX_AUTH_MODE_CUSTOM_URL.to_string(),
                codex_local_auth_path: default_codex_local_auth_path(),
                codex_custom_url: Some("https://new.sharedchat.cc/codex".to_string()),
                codex_custom_api_key: Some("sharedchat-key".to_string()),
                codex_originator: default_codex_originator(),
                codex_residency_requirement: None,
                api_keys: Vec::new(),
                key_cursor: 0,
                cached_model_options: vec!["gpt-5.4".to_string()],
                models: vec![ApiModelConfig {
                    id: model_id.clone(),
                    model: "gpt-5.4".to_string(),
                    display_name: String::new(),
                    deprecated: false,
                    enable_image: false,
                    enable_audio: false,
                    enable_video: false,
                    enable_tools: true,
                    reasoning_effort: default_reasoning_effort(),
                    temperature: 1.0,
                    custom_temperature_enabled: false,
                    context_window_tokens: 128_000,
                    max_output_tokens: 4_096,
                    custom_max_output_tokens_enabled: false,
                }],
                failure_retry_count: 0,
                client_user_agent: None,
            }],
            api_configs: Vec::new(),
            ..AppConfig::default()
        };
        normalize_app_config(&mut cfg);

        let resolved = resolve_api_config(&cfg, Some(&api_endpoint_id(&provider_id, &model_id)))
            .expect("custom url codex resolve");

        assert_eq!(resolved.api_key, "sharedchat-key".to_string());
        assert!(resolved.codex_auth.is_none());
        assert!(resolved
            .extra_headers
            .iter()
            .any(|(key, value)| key == "Session-Id" && !value.trim().is_empty()));
        // custom_url 模式未显式配置 originator 时回落真实产品名 p_ai_desktop，
        // 不沿用 OAuth 模式伪装官方客户端的默认 codex-tui。
        assert_eq!(resolved.codex_originator.as_deref(), Some("p_ai_desktop"));
    }

    #[test]
    fn resolve_api_config_should_keep_explicit_originator_in_custom_url_mode() {
        let provider_id = "codex-custom-originator".to_string();
        let model_id = "codex-model".to_string();
        let mut cfg = AppConfig {
            api_providers: vec![ApiProviderConfig {
                id: provider_id.clone(),
                name: "SharedChat".to_string(),
                deprecated: false,
                request_format: RequestFormat::Codex,
                allow_concurrent_requests: false,
                max_concurrent_requests: None,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_video: false,
                enable_tools: true,
                base_url: "https://example.com/codex".to_string(),
                login_provider: String::new(),
                codex_auth_mode: CODEX_AUTH_MODE_CUSTOM_URL.to_string(),
                codex_local_auth_path: default_codex_local_auth_path(),
                codex_custom_url: Some("https://example.com/codex".to_string()),
                codex_custom_api_key: Some("key".to_string()),
                codex_originator: "my-custom-agent".to_string(),
                codex_residency_requirement: None,
                api_keys: Vec::new(),
                key_cursor: 0,
                cached_model_options: Vec::new(),
                models: vec![ApiModelConfig {
                    id: model_id.clone(),
                    model: "gpt-5.4".to_string(),
                    ..ApiModelConfig::default()
                }],
                failure_retry_count: 0,
                client_user_agent: None,
            }],
            ..AppConfig::default()
        };
        normalize_app_config(&mut cfg);
        let resolved = resolve_api_config(&cfg, Some(&api_endpoint_id(&provider_id, &model_id)))
            .expect("explicit originator resolve");
        assert_eq!(resolved.codex_originator.as_deref(), Some("my-custom-agent"));
    }

    #[test]
    fn write_agents_shard_should_not_touch_conversations() {
        let root = std::env::temp_dir().join(format!("eca-app-data-shards-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create temp config dir");
        let data_path = root.join("config").join("config_mark");

        let mut data = AppData::default();
        data.conversations = vec![build_test_conversation("conv-a", "Conversation A")];
        seed_app_data_shards(&data_path, &data).expect("seed layout");

        let agents_path = app_layout_agents_path(&data_path);
        let conversation_paths =
            message_store::message_store_paths(&data_path, "conv-a").expect("conversation paths");

        let conversation_before = message_store::message_store_shard_write_signature(&conversation_paths);

        let mut agents = data.agents.clone();
        agents.push(AgentProfile {
            id: "agent-added".to_string(),
            name: "Agent Added".to_string(),
            system_prompt: "test".to_string(),
            created_at: "2026-04-15T00:00:00Z".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            scope: default_global_scope(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            include_system_rules: true,
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        });
        assert!(write_agents_shard(&data_path, &agents).expect("write agents shard"));
        assert_eq!(
            message_store::message_store_shard_write_signature(&conversation_paths),
            conversation_before
        );
        assert!(!std::fs::read(&agents_path).expect("read agents after runtime").is_empty());
    }

    #[test]
    fn runtime_volatile_normalization_should_not_require_rewriting_after_migration_version_recorded() {
        let root = std::env::temp_dir().join(format!("eca-read-baseline-migration-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create temp config dir");
        let data_path = root.join("config").join("config_mark");
        let mut data = AppData::default();
        data.data_migration_version =
            DATA_MIGRATION_VERSION_V2_ASSISTANT_WORKSPACE_FOR_EMPTY_SHELL_WORKSPACES;
        data.conversations = vec![build_test_conversation("conv-baseline", "Baseline")];
        seed_app_data_shards(&data_path, &data).expect("seed layout");
        let paths = message_store::message_store_paths(&data_path, "conv-baseline")
            .expect("conversation paths");
        let before = message_store::message_store_shard_write_signature(&paths);

        let restored = read_layout_app_data(&data_path).expect("read app data");
        let after = message_store::message_store_shard_write_signature(&paths);

        assert_eq!(
            restored.data_migration_version,
            DATA_MIGRATION_VERSION_V2_ASSISTANT_WORKSPACE_FOR_EMPTY_SHELL_WORKSPACES
        );
        assert_eq!(after, before);
        let mut conversation = read_conversation_shard_raw(&data_path, "conv-baseline")
            .expect("read raw conversation shard");
        assert!(conversation.messages[0].speaker_agent_id.is_none());
        normalize_conversation_runtime_volatile_fields(&mut conversation);
        assert_eq!(
            conversation.messages[0].speaker_agent_id.as_deref(),
            Some(USER_PERSONA_ID)
        );
        // 迁移版本已记录时读取不重写 message store：上面 after == before 已钉死。
        // 不断言 restored.conversations[0].messages 为空——分片读取必然返回完整消息，
        // 该断言在旧布局删除后不可能成立（99f5b81d 合并测试时遗留的矛盾断言）。
    }

    #[test]
    fn write_conversation_shard_should_write_message_store_and_only_touch_target() {
        let root = std::env::temp_dir().join(format!("eca-conversation-shard-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create temp config dir");
        let data_path = root.join("config").join("config_mark");

        let mut data = AppData::default();
        data.conversations = vec![
            build_test_conversation("conv-a", "Conversation A"),
            build_test_conversation("conv-b", "Conversation B"),
        ];
        seed_app_data_shards(&data_path, &data).expect("seed layout");

        let legacy_conversation_a_path = app_layout_chat_conversation_path(&data_path, "conv-a");
        let legacy_conversation_b_path = app_layout_chat_conversation_path(&data_path, "conv-b");
        assert!(!legacy_conversation_a_path.exists());
        assert!(!legacy_conversation_b_path.exists());
        let conversation_a_paths =
            message_store::message_store_paths(&data_path, "conv-a").expect("conversation a paths");
        let conversation_b_paths =
            message_store::message_store_paths(&data_path, "conv-b").expect("conversation b paths");
        assert!(message_store::chat_store_read_status(&conversation_a_paths)
            .expect("conversation a sqlite status")
            .is_some());
        assert!(message_store::chat_store_read_status(&conversation_b_paths)
            .expect("conversation b sqlite status")
            .is_some());
        let mut conversation_a = read_conversation_shard(&data_path, "conv-a").expect("read conversation a");
        conversation_a.title = "Conversation A Updated".to_string();
        assert!(write_conversation_shard(&data_path, &conversation_a).expect("write conversation a"));

        let conversation_a_meta = message_store::chat_store_read_meta(&conversation_a_paths)
            .expect("read conversation a meta")
            .expect("conversation a meta exists");
        assert_eq!(conversation_a_meta.title(), "Conversation A Updated");
        let conversation_b_meta = message_store::chat_store_read_meta(&conversation_b_paths)
            .expect("read conversation b meta")
            .expect("conversation b meta exists");
        assert_eq!(conversation_b_meta.title(), "Conversation B");
        assert!(!legacy_conversation_a_path.exists());
        assert!(!legacy_conversation_b_path.exists());
    }

    #[test]
    fn upsert_chat_index_conversation_should_replace_existing_item_without_duplicates() {
        let mut conversation_a = build_test_conversation("conv-a", "Conversation A");
        let conversation_b = build_test_conversation("conv-b", "Conversation B");
        let mut index = build_chat_index_file(&[conversation_a.clone(), conversation_b.clone()]);

        conversation_a.updated_at = "2026-04-15T12:34:56Z".to_string();
        conversation_a.status = "archived".to_string();
        conversation_a.archived_at = Some("2026-04-15T12:34:56Z".to_string());

        upsert_chat_index_conversation(&mut index, &conversation_a);

        assert_eq!(index.conversations.len(), 2);
        let updated = index
            .conversations
            .iter()
            .find(|item| item.id == "conv-a")
            .expect("find updated chat index item");
        assert_eq!(updated.updated_at, "2026-04-15T12:34:56Z");
        assert_eq!(updated.status, "archived");
        assert_eq!(
            updated.archived_at.as_deref(),
            Some("2026-04-15T12:34:56Z")
        );
    }

    #[test]
    fn remove_chat_index_conversation_should_drop_matching_item_only() {
        let conversation_a = build_test_conversation("conv-a", "Conversation A");
        let conversation_b = build_test_conversation("conv-b", "Conversation B");
        let mut index = build_chat_index_file(&[conversation_a, conversation_b]);

        remove_chat_index_conversation(&mut index, "conv-a");

        assert_eq!(index.conversations.len(), 1);
        assert!(index.conversations.iter().all(|item| item.id != "conv-a"));
        assert!(index.conversations.iter().any(|item| item.id == "conv-b"));
    }

    #[test]
    fn migration_package_version_should_allow_importing_older_data_versions() {
        let manifest = MigrationManifest {
            schema_version: MIGRATION_SCHEMA_VERSION,
            migration_version:
                DATA_MIGRATION_VERSION_V2_ASSISTANT_WORKSPACE_FOR_EMPTY_SHELL_WORKSPACES,
            app_version: "0.18.8".to_string(),
            exported_at: "2026-07-07T00:00:00Z".to_string(),
        };
        let mut payload = MigrationPayload {
            config: AppConfig::default(),
            runtime_data: MigrationRuntimeData::default(),
            memories: Vec::new(),
            oauth_files: Vec::new(),
            avatar_files: Vec::new(),
        };
        payload.runtime_data.data_migration_version =
            DATA_MIGRATION_VERSION_V2_ASSISTANT_WORKSPACE_FOR_EMPTY_SHELL_WORKSPACES;

        let version = assert_manifest_version(&manifest, &payload).expect("allow older import");
        assert_eq!(
            version,
            DATA_MIGRATION_VERSION_V2_ASSISTANT_WORKSPACE_FOR_EMPTY_SHELL_WORKSPACES
        );
    }

    #[test]
    fn migration_package_version_should_reject_newer_data_versions() {
        let newer_version = DATA_MIGRATION_CURRENT_VERSION + 1;
        let manifest = MigrationManifest {
            schema_version: MIGRATION_SCHEMA_VERSION,
            migration_version: newer_version,
            app_version: "0.99.0".to_string(),
            exported_at: "2026-07-07T00:00:00Z".to_string(),
        };
        let mut payload = MigrationPayload {
            config: AppConfig::default(),
            runtime_data: MigrationRuntimeData::default(),
            memories: Vec::new(),
            oauth_files: Vec::new(),
            avatar_files: Vec::new(),
        };
        payload.runtime_data.data_migration_version = newer_version;

        let err = assert_manifest_version(&manifest, &payload).expect_err("reject newer import");
        assert!(err.contains("迁移版本不兼容"));
        assert!(err.contains(&format!("V{newer_version}")));
    }

    fn build_test_conversation(id: &str, title: &str) -> Conversation {
        Conversation {
            questions: Vec::new(),
            question_next_number: 0,
            issues: Vec::new(),
            issue_next_number: 0,
            loaded_mcp_tools: Vec::new(),
            id: id.to_string(),
            title: title.to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: "chat".to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: "2026-04-15T00:00:00Z".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            shell_worktree_path: String::new(),
            shell_recorded_branch: String::new(),
            archived_at: None,
            messages: vec![ChatMessage {
                id: format!("{id}-message-1"),
                role: "user".to_string(),
                created_at: "2026-04-15T00:00:00Z".to_string(),
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "hello".to_string(),
                reasoning_content: None,
            }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            meme_annotations: None,
            }],
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            auto_push_remote_contact_id: None,
            active_goal: None, team: TeamState::default(), last_error: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            is_draft: false,
        }
    }

    fn config_test_state() -> AppState {
        let root = std::env::temp_dir().join(format!("eca-config-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp test root");
        std::fs::create_dir_all(root.join("llm-workspace")).expect("create temp llm workspace");
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: root.join("app_config.toml"),
            data_path: root.join("config_mark"),
            llm_workspace_path: root.join("llm-workspace"),
            shared_http_client: reqwest::Client::new(),
            terminal_shell: detect_default_terminal_shell(),
            terminal_shell_candidates: detect_terminal_shell_candidates(),
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_agents: Arc::new(Mutex::new(None)),
            cached_agents_mtime: Arc::new(Mutex::new(None)),
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_conversation_metadata: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_field_metadata_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_background_shell_tasks: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            schedule_events: Arc::new(Mutex::new(ScheduleEventStore::default())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            goal_continue_suppressed_conversation_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            accepted_submit_trace_ids: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_list_activity_marks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            task_scheduler_notify: Arc::new(tokio::sync::Notify::new()),
            delegate_runtime_threads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            remote_im_reply_delegate_runtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_semaphore: Arc::new(tokio::sync::Semaphore::new(8)),
            remote_im_channel_state_write_locks: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            conversation_project_skills_cache: Arc::new(Mutex::new(
                std::collections::HashMap::new(),
            )),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn storage_and_stt_test_state() -> AppState {
        config_test_state()
    }

    /// 以真实本机快照数据独立驱动 V5 组织迁移（不依赖运行态、不依赖 AppConfig 解析结果）。
    /// 标记 `#[ignore]`：输入位于未被 git 跟踪的 `.pai/temp/`，CI 与全新克隆环境必然缺失，
    /// 不能进常驻测试集（否则整仓测试变红）。手动执行：
    #[test]
    fn migrate_v1_to_v2_should_fill_assistant_workspace_for_empty_conversations() {
        let state = config_test_state();
        let chat_dir = crate::app_layout_chat_dir(&state.data_path);
        std::fs::create_dir_all(&chat_dir).expect("create chat dir");
        let db_path = chat_dir.join("chat_metadata.sqlite");
        {
            let conn = rusqlite::Connection::open(&db_path).expect("open metadata db");
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS conversation_metadata (
                   conversation_id TEXT PRIMARY KEY,
                   metadata_json TEXT NOT NULL,
                   storage_revision INTEGER NOT NULL DEFAULT 0,
                   updated_at TEXT NOT NULL
                 );",
            )
            .expect("create metadata table");
            let insert = |id: &str, meta: serde_json::Value| {
                conn.execute(
                    "INSERT INTO conversation_metadata (conversation_id, metadata_json, updated_at) VALUES (?1, ?2, ?3)",
                    rusqlite::params![id, serde_json::to_string(&meta).expect("serialize meta"), "2026-01-01T00:00:00Z"],
                )
                .expect("insert metadata row");
            };
            // 空壳普通会话：该补工作区。
            insert(
                "c-empty",
                serde_json::json!({
                    "id": "c-empty",
                    "conversationKind": "chat",
                    "status": "active",
                    "shellWorkspaces": [],
                }),
            );
            // 归档会话：不补。
            insert(
                "c-archived",
                serde_json::json!({
                    "id": "c-archived",
                    "conversationKind": "chat",
                    "status": "archived",
                    "shellWorkspaces": [],
                }),
            );
            // 已有工作区的会话：不动。
            insert(
                "c-has-workspace",
                serde_json::json!({
                    "id": "c-has-workspace",
                    "conversationKind": "chat",
                    "status": "active",
                    "shellWorkspaces": [{
                        "id": "keep-me",
                        "name": "已有",
                        "path": "/tmp/keep",
                        "level": "main",
                        "access": "full_access",
                        "builtIn": false
                    }],
                }),
            );
        }

        let config = AppConfig::default();
        let context = crate::DataMigrationContext {
            state: &state,
            config: &config,
        };
        assert!(
            crate::migrate_v1_to_v2(&context),
            "空壳会话应被补上工作区"
        );

        let conn = rusqlite::Connection::open(&db_path).expect("reopen metadata db");
        let read_meta = |id: &str| -> serde_json::Value {
            let raw: String = conn
                .query_row(
                    "SELECT metadata_json FROM conversation_metadata WHERE conversation_id=?1",
                    [id],
                    |row| row.get(0),
                )
                .expect("read metadata row");
            serde_json::from_str(&raw).expect("parse metadata json")
        };

        let filled = read_meta("c-empty");
        let workspaces = filled["shellWorkspaces"]
            .as_array()
            .expect("shellWorkspaces array");
        assert_eq!(workspaces.len(), 1, "空壳会话应补一个主工作区");
        assert_eq!(workspaces[0]["level"], "main");
        assert!(
            !workspaces[0]["path"].as_str().unwrap_or("").trim().is_empty(),
            "补出来的工作区必须有路径"
        );
        assert!(
            filled["shellWorkspacePath"].is_null(),
            "补完之后旧字段要清空"
        );

        assert!(
            read_meta("c-archived")["shellWorkspaces"]
                .as_array()
                .expect("archived workspaces")
                .is_empty(),
            "归档会话不该被补工作区"
        );
        assert_eq!(
            read_meta("c-has-workspace")["shellWorkspaces"]
                .as_array()
                .expect("existing workspaces")[0]["id"],
            "keep-me",
            "已有工作区的会话不能被改动"
        );
    }

    /// `cargo test migration_should_convert_snapshot -- --ignored`
    #[test]
    #[ignore = "依赖本机未跟踪快照 .pai/temp/local-data-snapshot-20260915，需手动 --ignored 运行"]
    fn migration_should_convert_snapshot_departments_into_agent_organization() {
        // CARGO_MANIFEST_DIR = <repo>/.pai/.worktree/drop-department/src-tauri
        let snapshot = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../temp/local-data-snapshot-20260915");
        let snapshot_config = snapshot.join("config/app_config.toml");
        let snapshot_agents = snapshot.join("config/agents.json");
        assert!(
            snapshot_config.exists() && snapshot_agents.exists(),
            "本机快照缺失，无法验证迁移：{}",
            snapshot.display()
        );

        let state = config_test_state();
        let root = state
            .config_path
            .parent()
            .expect("config path parent")
            .to_path_buf();
        std::fs::create_dir_all(root.join("config")).expect("create config dir");
        std::fs::create_dir_all(root.join("llm-workspace/skills")).expect("create skills dir");
        std::fs::copy(&snapshot_config, &state.config_path).expect("copy snapshot config");
        std::fs::copy(&snapshot_agents, root.join("config/agents.json")).expect("copy snapshot agents");

        // 用默认配置作为迁移上下文：不含自定义工作区，skill 写出被隔离到临时目录。
        let config = AppConfig::default();
        let context = crate::DataMigrationContext {
            state: &state,
            config: &config,
        };
        let stats = crate::migrate_v2_to_v5(&context);
        assert!(stats, "迁移应产生人格数据变化");

        let agents = crate::read_agents_shard(&state.data_path).expect("read migrated agents");
        let by_id = |id: &str| {
            agents
                .iter()
                .find(|agent| agent.id == id)
                .unwrap_or_else(|| panic!("agent {id} 不存在"))
        };

        // 自定义部门「八重堂」溶解进 yae-miko。
        let yae = by_id("yae-miko");
        assert!(!yae.summary.trim().is_empty(), "人格简介应接替部门 summary");
        assert!(
            yae.resident_skill_names.iter().any(|name| name == "八重堂"),
            "八重堂应转为常驻 skill"
        );
        assert!(!yae.api_config_ids.is_empty(), "部门模型应并入人格");
        assert!(yae.permission_control.enabled, "部门权限应并入人格且启用");

        // 自定义部门「全栈工程师」溶解进 persona-1781792413924。
        let engineer = by_id("persona-1781792413924");
        assert!(
            engineer
                .resident_skill_names
                .iter()
                .any(|name| name == "全栈工程师"),
            "全栈工程师应转为常驻 skill"
        );
        assert!(engineer.permission_control.enabled, "全栈工程师权限应并入人格");

        // 主助理根是 default-agent，其下级应含内置人格与八重堂成员。
        // 内置 2 个人格（reviewer/support）由代码预设补齐，
        // 它们与根的层级来自代码预设而非迁移推导，迁移只补自定义部门相关的人-人边。
        let assistants = by_id(DEFAULT_AGENT_ID);
        let assistants_children = &assistants.child_agent_ids;
        assert!(assistants_children.iter().any(|id| id == DEPUTY_AGENT_ID));
        assert!(assistants_children.iter().any(|id| id == "yae-miko"));
        for node_id in ["reviewer", "support"] {
            assert!(
                assistants_children.iter().any(|id| id == node_id),
                "内置人格 {node_id} 应由代码预设挂到根"
            );
        }

        // 原部门层级保留：文本整理员成员挂在全栈工程师人格下。
        assert!(
            engineer
                .child_agent_ids
                .iter()
                .any(|id| id == "agent-1788547230"),
            "文本整理员成员应挂在全栈工程师人格下"
        );
        // 文本整理员不应越过全栈工程师直接挂到根。
        assert!(
            !assistants_children.iter().any(|id| id == "agent-1788547230"),
            "文本整理员成员不应直接挂根"
        );
        // 不应残留指向不存在人格的悬空边。
        for agent in &agents {
            for child in &agent.child_agent_ids {
                assert!(
                    agents.iter().any(|other| &other.id == child),
                    "悬空下级边: {} -> {}",
                    agent.id,
                    child
                );
            }
        }

        // 自定义部门 skill 已写出到工作区。
        let skills_root = state.llm_workspace_path.join("skills");
        assert!(skills_root.join("八重堂/SKILL.md").exists());
        assert!(skills_root.join("全栈工程师/SKILL.md").exists());
        assert!(skills_root.join("文本整理员/SKILL.md").exists());

        // 幂等：重复迁移不再产生变化。
        let second = crate::migrate_v2_to_v5(&context);
        assert!(!second, "重复迁移应幂等");
    }

    #[test]
    fn avatar_path_should_keep_absolute_and_resolve_relative_against_data_root() {
        let root = std::env::temp_dir().join(format!("eca-avatar-path-{}", Uuid::new_v4()));
        let data_path = root.join("config_mark");

        assert_eq!(avatar_relative_path("agent-x.webp"), "avatars/agent-x.webp");
        assert_eq!(
            avatar_path_to_absolute(&data_path, "avatars/agent-x.webp"),
            root.join("avatars").join("agent-x.webp")
        );

        let absolute = std::env::temp_dir().join("eca-elsewhere").join("agent-y.webp");
        assert_eq!(
            avatar_path_to_absolute(&data_path, &absolute.to_string_lossy()),
            absolute
        );
    }

    #[test]
    fn migration_should_rewrite_absolute_avatar_paths_and_stay_idempotent() {
        let state = config_test_state();
        let avatars_dir = avatar_storage_dir(&state).expect("avatar dir");
        std::fs::create_dir_all(&avatars_dir).expect("create avatars dir");
        std::fs::write(avatars_dir.join("agent-real.webp"), b"x").expect("write avatar file");

        // 存的是别的数据根下的绝对路径，但文件名在当前头像目录里存在 → 应归一为相对路径。
        let stale_absolute = std::env::temp_dir()
            .join("eca-other-root")
            .join("avatars")
            .join("agent-real.webp")
            .to_string_lossy()
            .to_string();
        // 当前头像目录里没有同名文件 → 保留原值，不做无声抹除。
        let missing_absolute = std::env::temp_dir()
            .join("eca-nowhere")
            .join("avatars")
            .join("agent-ghost.webp")
            .to_string_lossy()
            .to_string();

        let make_agent = |id: &str, avatar: Option<String>| AgentProfile {
            id: id.to_string(),
            name: id.to_string(),
            system_prompt: "test".to_string(),
            created_at: "2026-04-15T00:00:00Z".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
            avatar_path: avatar,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            scope: default_global_scope(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            include_system_rules: true,
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        };

        let mut agents = AppData::default().agents;
        agents.push(make_agent("agent-abs", Some(stale_absolute)));
        agents.push(make_agent("agent-rel", Some("avatars/agent-rel.webp".to_string())));
        agents.push(make_agent("agent-missing", Some(missing_absolute.clone())));
        write_agents_shard(&state.data_path, &agents).expect("write agents shard");

        let config = AppConfig::default();
        let context = DataMigrationContext {
            state: &state,
            config: &config,
        };
        let stats = migrate_v5_to_v6(&context);
        assert!(stats);

        let after = read_agents_shard(&state.data_path).expect("read agents shard");
        let avatar_of = |id: &str| {
            after
                .iter()
                .find(|agent| agent.id == id)
                .and_then(|agent| agent.avatar_path.clone())
        };
        assert_eq!(avatar_of("agent-abs").as_deref(), Some("avatars/agent-real.webp"));
        assert_eq!(avatar_of("agent-rel").as_deref(), Some("avatars/agent-rel.webp"));
        assert_eq!(avatar_of("agent-missing").as_deref(), Some(missing_absolute.as_str()));

        // 幂等：已是相对路径的不会被再次改写。
        let second = migrate_v5_to_v6(&context);
        assert!(!second, "重复迁移应幂等");
    }

    #[test]
    fn migration_should_remove_hr_persona_and_prune_child_references() {
        let state = config_test_state();
        let make_agent = |id: &str, child_agent_ids: Vec<String>| AgentProfile {
            id: id.to_string(),
            name: id.to_string(),
            system_prompt: "test".to_string(),
            created_at: "2026-04-15T00:00:00Z".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            scope: default_global_scope(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            include_system_rules: true,
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids,
        };

        // 历史数据：根人格的下级列表含 hr，且数据里已落下一个 hr 人格节点。
        let agents = vec![
            make_agent("default-agent", vec!["hr".to_string(), "support".to_string()]),
            make_agent("hr", Vec::new()),
            make_agent("support", Vec::new()),
        ];
        write_agents_shard(&state.data_path, &agents).expect("write agents shard");

        let config = AppConfig::default();
        let context = DataMigrationContext {
            state: &state,
            config: &config,
        };
        let stats = migrate_v6_to_v7(&context);
        assert!(stats);

        let after = read_agents_shard(&state.data_path).expect("read agents shard");
        assert!(
            !after.iter().any(|agent| agent.id == "hr"),
            "已废弃的 hr 人格节点应被移除"
        );
        let root = after
            .iter()
            .find(|agent| agent.id == "default-agent")
            .expect("root");
        assert!(
            !root.child_agent_ids.iter().any(|child| child == "hr"),
            "对 hr 的下级引用应被摘除"
        );
        assert!(
            root.child_agent_ids.iter().any(|child| child == "support"),
            "其他下级应保留"
        );

        // 幂等：再次执行不再产生变化。
        let second = migrate_v6_to_v7(&context);
        assert!(!second, "重复迁移应幂等");
    }

    #[test]
    fn migration_should_correct_deputy_system_flag() {
        let state = config_test_state();
        // 历史数据：副手被错标为系统人格。
        let mut agents = AppData::default().agents;
        for agent in agents.iter_mut() {
            if agent.id == DEPUTY_AGENT_ID {
                agent.is_built_in_system = true;
            }
        }
        write_agents_shard(&state.data_path, &agents).expect("write agents shard");

        let config = AppConfig::default();
        let context = DataMigrationContext {
            state: &state,
            config: &config,
        };
        let stats = migrate_v7_to_v8(&context);
        assert!(stats);

        let after = read_agents_shard(&state.data_path).expect("read agents shard");
        let deputy = after
            .iter()
            .find(|agent| agent.id == DEPUTY_AGENT_ID)
            .expect("deputy agent");
        assert!(!deputy.is_built_in_system, "副手不应再被标为系统人格");

        // 幂等：再次执行不再产生变化。
        let second = migrate_v7_to_v8(&context);
        assert!(!second, "重复迁移应幂等");
    }

    #[test]
    fn sync_block_bridge_inside_tokio_worker_should_not_panic() {
        // 复现 Grok 供应商发消息时崩溃的场景：resolve_api_config 会在 tokio 工作线程里
        // 同步驱动 Grok token 解析，直接 block_on 会 panic
        // （Cannot start a runtime from within a runtime）。
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build multi-thread tokio runtime");
        let value = runtime.block_on(async {
            tokio::spawn(async { block_on_async(async { Ok::<u32, String>(7) }) })
                .await
                .expect("join bridge task")
                .expect("bridge value")
        });
        assert_eq!(value, 7);
    }

    // ==================== 运行态与废弃字段不落盘 ====================

    fn test_provider_with_models(provider_id: &str, model_ids: &[&str]) -> ApiProviderConfig {
        ApiProviderConfig {
            id: provider_id.to_string(),
            name: "Provider A".to_string(),
            deprecated: false,
            request_format: RequestFormat::OpenAI,
            allow_concurrent_requests: true,
            max_concurrent_requests: None,
            enable_text: true,
            enable_image: false,
            enable_audio: false,
            enable_video: false,
            enable_tools: true,
            base_url: "https://api.example.com/v1".to_string(),
            login_provider: String::new(),
            codex_auth_mode: default_codex_auth_mode(),
            codex_local_auth_path: default_codex_local_auth_path(),
            codex_custom_url: None,
            codex_custom_api_key: None,
            codex_originator: default_codex_originator(),
            codex_residency_requirement: None,
            api_keys: Vec::new(),
            key_cursor: 0,
            cached_model_options: Vec::new(),
            models: model_ids
                .iter()
                .map(|model_id| ApiModelConfig {
                    id: (*model_id).to_string(),
                    model: (*model_id).to_string(),
                    display_name: String::new(),
                    deprecated: false,
                    enable_image: false,
                    enable_audio: false,
                    enable_video: false,
                    enable_tools: true,
                    reasoning_effort: default_reasoning_effort(),
                    temperature: default_api_temperature(),
                    custom_temperature_enabled: false,
                    context_window_tokens: default_context_window_tokens(),
                    max_output_tokens: default_max_output_tokens(),
                    custom_max_output_tokens_enabled: false,
                })
                .collect(),
            failure_retry_count: default_failure_retry_count(),
            client_user_agent: None,
        }
    }

    fn test_mcp_server_config(id: &str) -> McpServerConfig {
        McpServerConfig {
            id: id.to_string(),
            name: id.to_string(),
            enabled: true,
            definition_json: r#"{"command":"npx"}"#.to_string(),
            oauth_capable: false,
            has_oauth_token: false,
            tool_policies: Vec::new(),
            cached_tools: Vec::new(),
            instructions: String::new(),
            last_status: "ready".to_string(),
            last_error: String::new(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    /// 旧文件里同时带 `apiConfigs` / `tools` / `keyCursor` / `cachedModelOptions` / `mcpServers`。
    /// 读取必须成功：属于用户设定的照样读进来，磁盘上的派生 `apiConfigs` 被重算结果覆盖。
    #[test]
    fn legacy_config_with_removed_and_runtime_keys_should_still_load() {
        let state = config_test_state();
        let path = state.config_path.clone();
        std::fs::create_dir_all(path.parent().expect("config parent")).expect("create config dir");
        std::fs::write(
            &path,
            r#"hotkey = "Alt+·"

[[mcpServers]]
id = "embedded-mcp"
name = "Embedded"
definitionJson = "{\"command\":\"npx\"}"

[[apiConfigs]]
id = "legacy-config"
name = "Legacy"
requestFormat = "openai"
baseUrl = "https://legacy.example.com/v1"
apiKey = "legacy-key"
model = "legacy-model"
enableAudio = false
tools = [{ id = "exec", enabled = true }]

[[apiProviders]]
id = "provider-a"
name = "Provider A"
requestFormat = "openai"
baseUrl = "https://api.example.com/v1"
apiKeys = ["key-1", "key-2"]
keyCursor = 1
cachedModelOptions = ["model-a"]
enableAudio = false
tools = [{ id = "write", enabled = true }]

[[apiProviders.models]]
id = "model-a"
model = "gpt-4.1"
enableAudio = false
"#,
        )
        .expect("write legacy config");

        let config = read_config(&path).expect("旧文件应能正常读入");

        let provider = config.api_providers.first().expect("provider parsed");
        assert_eq!(provider.id, "provider-a");
        assert_eq!(provider.key_cursor, 1, "旧 keyCursor 仍读入结构体（只是不再写盘）");
        assert_eq!(provider.cached_model_options, vec!["model-a".to_string()]);
        assert_eq!(config.mcp_servers.len(), 1, "旧内嵌 mcpServers 仍读入结构体（只是不再写盘）");
        assert_eq!(config.api_configs.len(), 1, "apiConfigs 读后由 apiProviders 重算");
        assert_eq!(config.api_configs[0].id, "provider-a::model-a");
        assert!(
            !config.api_configs.iter().any(|api| api.id == "legacy-config"),
            "磁盘上的旧 apiConfigs 应被重算结果覆盖"
        );
    }

    /// 写盘：用户设定要留，运行态与派生数据一个键都不许出现。
    #[test]
    fn write_config_should_not_persist_runtime_or_derived_keys() {
        let state = config_test_state();
        let path = state.config_path.clone();
        let mut config = AppConfig::default();
        config.api_providers = vec![test_provider_with_models("provider-a", &["model-a", "model-b"])];
        config.api_providers[0].key_cursor = 3;
        config.api_providers[0].cached_model_options = vec!["model-a".to_string()];
        config.mcp_servers = vec![test_mcp_server_config("embedded-mcp")];
        expand_api_configs_from_providers(&mut config);
        assert_eq!(config.api_configs.len(), 2, "内存里仍保留派生 apiConfigs");

        write_config(&path, &config).expect("write config");
        let text = std::fs::read_to_string(&path).expect("read written config");

        assert!(text.contains("apiProviders"), "用户设定仍要落盘：{text}");
        assert!(!text.contains("apiConfigs"), "派生 apiConfigs 不落盘：{text}");
        assert!(!text.contains("mcpServers"), "运行态 mcpServers 不落盘：{text}");
        assert!(!text.contains("keyCursor"), "密钥轮换游标不落盘：{text}");
        assert!(!text.contains("cachedModelOptions"), "模型列表缓存不落盘：{text}");
    }

    /// 新格式文件（不带 apiConfigs）读回后，仍按 apiProviders 全量展开。
    #[test]
    fn read_config_should_rebuild_api_configs_from_providers() {
        let state = config_test_state();
        let path = state.config_path.clone();
        std::fs::create_dir_all(path.parent().expect("config parent")).expect("create config dir");
        std::fs::write(
            &path,
            r#"hotkey = "Alt+·"

[[apiProviders]]
id = "provider-a"
name = "Provider A"
baseUrl = "https://api.example.com/v1"
enableAudio = false

[[apiProviders.models]]
id = "model-a"
model = "gpt-4.1"
enableAudio = false

[[apiProviders.models]]
id = "model-b"
model = "gpt-4.1-mini"
enableAudio = false
"#,
        )
        .expect("write config without apiConfigs");

        let config = read_config(&path).expect("read config");
        let ids = config
            .api_configs
            .iter()
            .map(|api| api.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["provider-a::model-a", "provider-a::model-b"]);
        assert_eq!(config.api_configs[0].name, "Provider A/gpt-4.1");
    }

    /// MCP 定义的来源是 workspace 目录，app_config 里的内嵌 mcpServers 已不是来源。
    #[test]
    fn mcp_definitions_should_still_load_from_workspace_dir() {
        let state = config_test_state();
        std::fs::create_dir_all(state.config_path.parent().expect("config parent"))
            .expect("create config dir");
        std::fs::write(
            &state.config_path,
            r#"hotkey = "Alt+·"

[[mcpServers]]
id = "embedded-mcp"
name = "Embedded"
definitionJson = "{\"command\":\"npx\"}"
"#,
        )
        .expect("write config with embedded mcp servers");

        let servers_dir = state.llm_workspace_path.join("mcp").join("servers");
        std::fs::create_dir_all(&servers_dir).expect("create mcp servers dir");
        std::fs::write(
            servers_dir.join("demo-mcp.json"),
            r#"{"command":"npx","args":["-y","demo-mcp"]}"#,
        )
        .expect("write mcp server file");

        let servers = load_workspace_mcp_servers(&state).expect("load workspace mcp servers");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].id, "demo-mcp");
        assert!(
            !servers.iter().any(|server| server.id == "embedded-mcp"),
            "内嵌 mcpServers 不应再参与运行时装配"
        );
    }

    /// 游标不落盘，但进程内轮换照旧：一次写盘 + 读回之后仍能从第二个密钥继续。
    #[test]
    fn key_rotation_should_keep_working_without_persisting_cursor() {
        let state = config_test_state();
        let path = state.config_path.clone();
        let provider_id = format!("provider-rotate-{}", Uuid::new_v4());
        let mut config = AppConfig::default();
        config.api_providers = vec![test_provider_with_models(&provider_id, &["model-a"])];
        config.api_providers[0].api_keys = vec!["key-1".to_string(), "key-2".to_string()];
        expand_api_configs_from_providers(&mut config);

        let first = select_and_advance_provider_api_key(&config.api_providers[0]);
        assert_eq!(first, "key-1");

        write_config(&path, &config).expect("write config");
        let text = std::fs::read_to_string(&path).expect("read written config");
        assert!(!text.contains("keyCursor"), "游标不落盘：{text}");

        let reloaded = read_config(&path).expect("read config");
        let provider = reloaded.api_providers.first().expect("provider");
        assert_eq!(provider.key_cursor, 0, "磁盘不带游标，读回为 0");
        let second = select_and_advance_provider_api_key(provider);
        assert_eq!(second, "key-2", "进程内游标态仍在，轮换继续");
    }

    // ==================== 人格文件：内置兜底 / 覆盖 / 逐条容错 ====================

    fn agents_test_data_path() -> PathBuf {
        let root = std::env::temp_dir().join(format!("eca-agents-file-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create temp config dir");
        root.join("config").join("config_mark")
    }

    /// 往人格目录里写一个文件。`id` 决定文件名（与业务同一条安全化规则）。
    fn write_agent_file(data_path: &PathBuf, id: &str, content: &str) -> PathBuf {
        let path = app_layout_agent_file_path(data_path, id);
        std::fs::create_dir_all(path.parent().expect("agents dir")).expect("create agents dir");
        std::fs::write(&path, content).expect("write agent file");
        path
    }

    fn agent_file_json(id: &str, name: &str) -> String {
        format!(
            "{{\"id\":\"{id}\",\"name\":\"{name}\",\"systemPrompt\":\"p\",\"createdAt\":\"2026-01-01T00:00:00Z\",\"updatedAt\":\"2026-01-01T00:00:00Z\"}}"
        )
    }

    fn write_agents_json(data_path: &PathBuf, content: &str) {
        let path = app_layout_agents_path(data_path);
        std::fs::create_dir_all(path.parent().expect("agents parent")).expect("agents dir");
        std::fs::write(path, content).expect("write agents file");
    }

    fn built_in_ids(agents: &[AgentProfile]) -> Vec<String> {
        let mut ids = agents
            .iter()
            .map(|agent| agent.id.trim().to_string())
            .filter(|id| {
                built_in_agents()
                    .iter()
                    .any(|built_in| built_in.id.trim() == id)
            })
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    #[test]
    fn read_agents_shard_should_accept_trailing_comma_and_comments() {
        // 字段齐全的文件才会「只被读、不被改写」，因此这里都带上时间戳。
        let stamp = "\"createdAt\": \"2026-01-01T00:00:00Z\", \"updatedAt\": \"2026-01-01T00:00:00Z\"";
        // 尾逗号
        let trailing = agents_test_data_path();
        write_agents_json(
            &trailing,
            &format!(
                "{{\n  \"agents\": [\n    {{\"id\": \"agent-x\", \"name\": \"X\", \"systemPrompt\": \"p\", {stamp}}},\n  ]\n}}\n"
            ),
        );
        let agents = read_agents_shard(&trailing).expect("尾逗号应被接受");
        assert!(agents.iter().any(|agent| agent.id == "agent-x"));

        // 注释
        let commented = agents_test_data_path();
        write_agents_json(
            &commented,
            &format!(
                "{{\n  // 这是行注释\n  \"agents\": [\n    /* 块注释 */ {{\"id\": \"agent-y\", \"name\": \"Y\", \"systemPrompt\": \"p\", {stamp}}}\n  ]\n}}\n"
            ),
        );
        let agents = read_agents_shard(&commented).expect("注释应被接受");
        assert!(agents.iter().any(|agent| agent.id == "agent-y"));

        // 尾逗号 + 注释混合
        let mixed = agents_test_data_path();
        let mixed_content = format!(
            "{{\n  // 人格配置\n  \"agents\": [\n    {{\"id\": \"agent-z\", \"name\": \"Z\", \"systemPrompt\": \"p\", {stamp}}}, // 留着方便加\n  ],\n}}\n"
        );
        write_agents_json(&mixed, &mixed_content);
        let agents = read_agents_shard(&mixed).expect("尾逗号 + 注释混合也应被接受");
        assert!(agents.iter().any(|agent| agent.id == "agent-z"));

        // 合法 JSONC 不该被当成损坏：不备份、也不改写（保住用户写的注释）
        for path in [&trailing, &commented, &mixed] {
            assert!(
                agents_backup_files(path).is_empty(),
                "合法 JSONC 不该产生备份：{path:?}"
            );
        }
        assert_eq!(
            std::fs::read_to_string(app_layout_agents_path(&mixed)).expect("读文件"),
            mixed_content,
            "读得通、又没什么要补的文件不该被重写（注释是用户写的，不能抹掉）"
        );
    }

    #[test]
    fn read_agents_shard_should_override_built_in_by_id_and_append_custom() {
        let data_path = agents_test_data_path();
        let renamed = format!(
            "{{\"id\": \"{DEFAULT_AGENT_ID}\", \"name\": \"我改过的 Pai\", \"systemPrompt\": \"改过的提示词\"}}"
        );
        let custom = "{\"id\": \"agent-custom\", \"name\": \"自定义\", \"systemPrompt\": \"p\"}";
        write_agents_json(
            &data_path,
            &format!("{{\"agents\": [{renamed}, {custom}]}}"),
        );

        let agents = read_agents_shard(&data_path).expect("读取人格");
        let pai = agents
            .iter()
            .find(|agent| agent.id == DEFAULT_AGENT_ID)
            .expect("内置 Pai 必须在");
        assert_eq!(pai.name, "我改过的 Pai", "同 id 文件条目应覆盖内置");
        // 文件里没写的内置人格仍然存在
        assert!(agents.iter().any(|agent| agent.id == DEPUTY_AGENT_ID));
        assert!(agents.iter().any(|agent| agent.id == "reviewer"));
        assert!(
            agents.iter().any(|agent| agent.id == "agent-custom"),
            "非内置 id 应追加为自定义人格"
        );
    }

    #[test]
    fn read_agents_shard_should_fill_missing_created_at() {
        let data_path = agents_test_data_path();
        write_agents_json(
            &data_path,
            "{\"agents\": [{\"id\": \"agent-no-time\", \"name\": \"无时间\", \"systemPrompt\": \"p\"}]}",
        );
        let agents = read_agents_shard(&data_path).expect("缺时间戳不应被拒");
        let agent = agents
            .iter()
            .find(|agent| agent.id == "agent-no-time")
            .expect("缺 createdAt 的条目也要读进来");
        assert!(
            !agent.created_at.trim().is_empty(),
            "createdAt 应被补齐，不能留空串"
        );
        assert!(!agent.updated_at.trim().is_empty());
    }

    #[test]
    fn write_agents_dir_should_delete_files_of_removed_agents() {
        let state = config_test_state();
        let make_agent = |id: &str| AgentProfile {
            id: id.to_string(),
            name: id.to_string(),
            system_prompt: "test".to_string(),
            created_at: "2026-04-15T00:00:00Z".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            scope: default_global_scope(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            include_system_rules: true,
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        };
        let keep = make_agent("keep-me");
        let removed = make_agent("remove-me");

        crate::write_agents_dir(&state.data_path, &[keep.clone(), removed.clone()])
            .expect("首次写入两个人格");
        assert!(
            crate::app_layout_agent_file_path(&state.data_path, "remove-me").exists(),
            "两个人都该落盘"
        );

        crate::write_agents_dir(&state.data_path, &[keep.clone()]).expect("写入一个人格");
        assert!(
            crate::app_layout_agent_file_path(&state.data_path, "keep-me").exists(),
            "留下的人格必须还在"
        );
        assert!(
            !crate::app_layout_agent_file_path(&state.data_path, "remove-me").exists(),
            "删掉的人格文件必须一并删除，否则重启会复活"
        );

        let read_back = crate::read_agents_dir_outcome(&state.data_path).expect("读回人格");
        assert!(
            read_back.iter().any(|agent| agent.id == "keep-me"),
            "留下的人格必须读得回来"
        );
        assert!(
            !read_back.iter().any(|agent| agent.id == "remove-me"),
            "删掉的人格不能再被读回来"
        );
    }

    /// 机械验收：没有新数据时，旧单文件在不在，读取结果必须相同。
    #[test]
    fn agents_read_should_not_depend_on_legacy_single_file() {
        let state = config_test_state();
        let legacy = r#"{"agents":[{"id":"legacy-only","name":"旧单文件里的人格","systemPrompt":"p"}]}"#;
        // 只有旧单文件、没有目录：读出来必须等于纯内置，旧人格一条都不许进来。
        write_agents_json(&state.data_path, legacy);
        let with_legacy = crate::read_agents_dir_outcome(&state.data_path).expect("读取人格");
        assert!(
            !with_legacy.iter().any(|agent| agent.id == "legacy-only"),
            "旧单文件里的人格不该被读进来"
        );
        let ids_with_legacy = with_legacy
            .iter()
            .map(|agent| agent.id.clone())
            .collect::<Vec<_>>();

        // 删掉旧单文件：结果必须一字不差。
        std::fs::remove_file(crate::app_layout_agents_path(&state.data_path))
            .expect("删除旧单文件");
        let without_legacy = crate::read_agents_dir_outcome(&state.data_path).expect("读取人格");
        assert_eq!(
            without_legacy
                .iter()
                .map(|agent| agent.id.clone())
                .collect::<Vec<_>>(),
            ids_with_legacy,
            "旧单文件在不在，读取结果必须相同"
        );

        // 目录里有人格时，旧单文件同样不该被读进来。
        write_agent_file(
            &state.data_path,
            "dir-only",
            &agent_file_json("dir-only", "目录里的人格"),
        );
        write_agents_json(&state.data_path, legacy);
        let mixed = crate::read_agents_dir_outcome(&state.data_path).expect("读取人格");
        assert!(
            mixed.iter().any(|agent| agent.id == "dir-only"),
            "目录里的人格必须读到"
        );
        assert!(
            !mixed.iter().any(|agent| agent.id == "legacy-only"),
            "旧单文件里的人格不该被读进来"
        );
    }

    #[test]
    fn state_read_agents_cached_should_keep_other_agents_when_one_file_turns_corrupt() {
        let state = config_test_state();
        write_agent_file(
            &state.data_path,
            "agent-live",
            &agent_file_json("agent-live", "活着的自定义"),
        );
        write_agent_file(
            &state.data_path,
            "agent-keep",
            &agent_file_json("agent-keep", "旁边那条"),
        );
        let first = crate::state_read_agents_cached(&state).expect("首次读取人格");
        assert!(first.iter().any(|agent| agent.id == "agent-live"));

        // 把其中一个文件写成一堆解析不了的东西：只丢它自己，旁边那条照常。
        std::thread::sleep(std::time::Duration::from_millis(30));
        write_agent_file(&state.data_path, "agent-live", "{\"id\": \"agent-live\",");

        let second = crate::state_read_agents_cached(&state).expect("坏文件不应让读取失败");
        assert!(
            !second.iter().any(|agent| agent.id == "agent-live"),
            "坏掉的那个文件只能丢它自己"
        );
        assert!(
            second.iter().any(|agent| agent.id == "agent-keep"),
            "旁边的人格必须照常加载"
        );
        assert!(
            second.iter().any(|agent| agent.id == DEFAULT_AGENT_ID),
            "内置人格必须照常打底"
        );
        assert!(
            !agents_load_diagnostics(&state.data_path).is_empty(),
            "跳过坏文件要留下诊断"
        );
    }

    #[test]
    fn state_read_agents_cached_should_follow_deleted_file_instead_of_keeping_stale_cache() {
        let state = config_test_state();
        let file_path = write_agent_file(
            &state.data_path,
            "agent-stale",
            &agent_file_json("agent-stale", "要被删掉的"),
        );
        let first = crate::state_read_agents_cached(&state).expect("首次读取人格");
        assert!(first.iter().any(|agent| agent.id == "agent-stale"));

        // 删掉一个人格文件是正常状态（不是损坏），必须立刻反映到缓存，不能继续返回旧的。
        std::fs::remove_file(&file_path).expect("删除人格文件");
        let second = crate::state_read_agents_cached(&state).expect("删除后读取人格");
        assert!(
            !second.iter().any(|agent| agent.id == "agent-stale"),
            "删掉人格文件后不应继续返回旧的自定义人格"
        );
        assert!(
            second.iter().any(|agent| agent.id == DEFAULT_AGENT_ID),
            "删掉人格文件后应回到纯内置人格"
        );
        assert!(
            agents_load_diagnostics(&state.data_path).is_empty(),
            "文件不存在不是损坏，不该留诊断：{:?}",
            agents_load_diagnostics(&state.data_path)
        );
    }

    /// 列出某数据根下的人格文件备份。
    fn agents_backup_files(data_path: &PathBuf) -> Vec<PathBuf> {
        let dir = app_layout_config_dir(data_path);
        let mut files = std::fs::read_dir(&dir)
            .map(|iter| {
                iter.filter_map(std::result::Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.file_name()
                            .map(|name| name.to_string_lossy().starts_with("agents.json.corrupt-"))
                            .unwrap_or(false)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        files.sort();
        files
    }

    #[test]
    fn read_agents_shard_should_fill_every_missing_field() {
        // `{}` 作为一条人格：所有字段都缺，必须补齐成一条合法人格，而不是当坏条丢掉。
        let data_path = agents_test_data_path();
        write_agents_json(&data_path, "{\"agents\": [{}]}");
        let agents = read_agents_shard(&data_path).expect("读取人格");
        let agent = agents
            .iter()
            .find(|agent| agent.id.starts_with("agent-"))
            .expect("自动生成 id 的人格");
        assert!(!agent.name.trim().is_empty(), "名字要补齐");
        assert!(!agent.created_at.trim().is_empty(), "createdAt 要补齐");
        assert!(!agent.updated_at.trim().is_empty(), "updatedAt 要补齐");
        // systemPrompt 空串是合法状态（人格可以没有提示词），不编内容
        assert!(agent.system_prompt.is_empty());
        assert!(!agents_load_diagnostics(&data_path)
            .iter()
            .any(|item| item.contains("无法解析")));
    }

    #[test]
    fn merge_should_keep_built_in_text_when_file_overrides_with_empty_fields() {
        let data_path = agents_test_data_path();
        // 语法完全合法，只是把名字与提示词写成空串：不该把内置人格的作用清空。
        write_agents_json(
            &data_path,
            &format!("{{\"agents\": [{{\"id\": \"{DEFAULT_AGENT_ID}\", \"name\": \"\", \"systemPrompt\": \"\"}}]}}"),
        );
        let agents = read_agents_shard(&data_path).expect("读取人格");
        let built_in = built_in_agents();
        let default = built_in
            .iter()
            .find(|agent| agent.id == DEFAULT_AGENT_ID)
            .expect("built-in pai");
        let pai = agents
            .iter()
            .find(|agent| agent.id == DEFAULT_AGENT_ID)
            .expect("内置 Pai 必须在");
        assert_eq!(pai.name, default.name, "空名字要用代码默认值兜底");
        assert_eq!(
            pai.system_prompt, default.system_prompt,
            "空提示词要用代码默认值兜底"
        );
    }

    #[test]
    fn state_read_agents_cached_should_recover_after_file_fixed() {
        let state = config_test_state();
        let file_path = write_agent_file(
            &state.data_path,
            "agent-live",
            &agent_file_json("agent-live", "旧的"),
        );
        let first = crate::state_read_agents_cached(&state).expect("首次读取");
        assert!(first.iter().any(|agent| agent.id == "agent-live"));

        // 改坏：这个文件丢掉，读仍然成功
        std::thread::sleep(std::time::Duration::from_millis(30));
        write_agent_file(&state.data_path, "agent-live", "{\"id\": \"agent-live\",");
        let during = crate::state_read_agents_cached(&state).expect("坏文件期间读取");
        assert!(
            !during.iter().any(|agent| agent.id == "agent-live"),
            "坏掉的文件必须被跳过"
        );

        // 修好：mtime 变了就必须读到新内容，不能一直停在旧缓存
        std::thread::sleep(std::time::Duration::from_millis(30));
        std::fs::write(&file_path, agent_file_json("agent-live", "新的")).expect("写回人格文件");
        let after = crate::state_read_agents_cached(&state).expect("修好后读取");
        let live = after
            .iter()
            .find(|agent| agent.id == "agent-live")
            .expect("修好后必须能读到");
        assert_eq!(live.name, "新的", "修好后必须换成磁盘上的新内容");
        assert!(
            agents_load_diagnostics(&state.data_path).is_empty(),
            "修好之后不该再留诊断"
        );
    }

        #[test]
    fn app_data_migration_should_advance_version_even_when_agents_file_is_corrupt() {
        let state = config_test_state();
        state_service_set_message_store_migration_version(
            &state,
            MESSAGE_STORE_MIGRATION_CURRENT_VERSION,
        )
        .expect("mark message store migration complete");
        state_service_set_data_migration_version(
            &state,
            DATA_MIGRATION_VERSION_V2_ASSISTANT_WORKSPACE_FOR_EMPTY_SHELL_WORKSPACES,
        )
        .expect("set migration version");
        // 人格文件坏到读不出来：迁移不许因此失败，只跳过这一档、照常推进版本。
        let original = "{
  \"agents\": [ oops
}
";
        write_agents_json(&state.data_path, original);

        let config = AppConfig::default();
        run_app_data_migrations_with_state(&state, &config);

        assert_eq!(
            state_service_get_data_migration_version(&state).expect("读迁移版本"),
            DATA_MIGRATION_CURRENT_VERSION,
            "坏数据不许把用户卡在旧版本上，版本必须推进到当前"
        );
        assert_eq!(
            std::fs::read_to_string(app_layout_agents_path(&state.data_path)).expect("读人格文件"),
            original,
            "跳过时不许改写旧人格文件"
        );
    }


    #[test]
    fn load_agents_should_never_return_err_for_bad_agents_file() {
        let state = config_test_state();
        let cases = [
            ("尾逗号", "{\"agents\": [{\"id\": \"a\", \"name\": \"A\"},]}"),
            ("括号不闭合", "{\"agents\": [ oops\n}\n"),
            ("坏条", "{\"agents\": [{\"name\": 123}]}"),
            ("全是注释", "// 只有注释\n"),
        ];
        for (label, content) in cases {
            write_agents_json(&state.data_path, content);
            let agents = crate::load_agents_inner(&state)
                .unwrap_or_else(|err| panic!("{label} 不该让 load_agents 报错：{err}"));
            assert!(!agents.is_empty(), "{label} 之后也要有人格可用");
        }

        // 非 UTF-8 同样不报错
        let path = app_layout_agents_path(&state.data_path);
        std::fs::write(&path, [0xFFu8, 0xFE, 0x00, 0x41]).expect("write non utf8");
        let agents = crate::load_agents_inner(&state).expect("非 UTF-8 也不该报错");
        assert!(agents.iter().any(|agent| agent.id == DEFAULT_AGENT_ID));
    }

    #[test]
    fn write_agents_shard_should_persist_only_custom_and_modified_built_in() {
        let data_path = agents_test_data_path();
        let mut agents = built_in_agents();
        // 未改动的内置：不落盘
        // 被改过的内置：落盘
        let pai = agents
            .iter_mut()
            .find(|agent| agent.id == DEFAULT_AGENT_ID)
            .expect("built-in pai");
        pai.name = "改名后的 Pai".to_string();
        // 自定义：落盘
        agents.push(AgentProfile {
            id: "agent-custom".to_string(),
            name: "自定义".to_string(),
            system_prompt: "p".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            scope: default_global_scope(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            include_system_rules: true,
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        });

        write_agents_shard(&data_path, &agents).expect("写入人格");
        let written: Value = serde_json::from_str(
            &std::fs::read_to_string(app_layout_agents_path(&data_path)).expect("read agents file"),
        )
        .expect("parse agents file");
        let written_ids = written
            .get("agents")
            .and_then(Value::as_array)
            .expect("agents array")
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert!(written_ids.contains(&DEFAULT_AGENT_ID), "改过的内置要落盘");
        assert!(written_ids.contains(&"agent-custom"), "自定义要落盘");
        assert!(
            !written_ids.contains(&DEPUTY_AGENT_ID),
            "未改动的内置不该写进文件：{written_ids:?}"
        );
        assert!(
            !written_ids.contains(&"reviewer"),
            "内置组织人格没改就不该写进文件：{written_ids:?}"
        );

        // 读回仍然完整：内置由代码补齐
        let reloaded = read_agents_shard(&data_path).expect("读回人格");
        assert_eq!(reloaded.len(), agents.len(), "读回人格数应与写入前一致");
        let pai_reloaded = reloaded
            .iter()
            .find(|agent| agent.id == DEFAULT_AGENT_ID)
            .expect("pai");
        assert_eq!(pai_reloaded.name, "改名后的 Pai", "改过的内置要持久生效");
    }
