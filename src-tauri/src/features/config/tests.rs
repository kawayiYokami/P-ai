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
    fn startup_window_label_should_require_expert_model_binding() {
        let mut cfg = AppConfig::default();
        let api_id = cfg.expert_api_config_id.clone();
        let api = cfg
            .api_configs
            .iter_mut()
            .find(|item| item.id == api_id)
            .expect("default chat api exists");
        api.api_key = "sk-test".to_string();
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
            selected_api_config_id: "a1".to_string(),
            expert_api_config_id: "a1".to_string(),
            simple_setup_mode: false,
            vision_api_config_id: None,
            image_generation_model_id: None,
            image_providers: Vec::new(),
            stt_api_config_id: None,
            stt_auto_send: false,
            provider_non_stream_base_urls: Vec::new(),
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: Vec::new(),
            remote_im_channels: Vec::new(),
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
                    tools: vec![],
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
                    tools: vec![],
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
            selected_api_config_id: "edit-b".to_string(),
            expert_api_config_id: "chat-a".to_string(),
            vision_api_config_id: None,
            image_generation_model_id: None,
            image_providers: Vec::new(),
            stt_api_config_id: None,
            simple_setup_mode: false,
            stt_auto_send: false,
            provider_non_stream_base_urls: Vec::new(),
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: Vec::new(),
            remote_im_channels: Vec::new(),
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
                    tools: vec![],
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
                    tools: vec![],
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
        assert_eq!(cfg.selected_api_config_id, "edit-b::edit-b-model-default".to_string());
        assert_eq!(
            cfg.expert_api_config_id,
            "chat-a::chat-a-model-default".to_string()
        );
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
            .find(|item| item.id == cfg.selected_api_config_id)
            .expect("default api exists");
        api.request_format = RequestFormat::OpenAI;
        api.base_url = "https://api.deepseek.com/v1".to_string();
        api.api_key = "sk-test".to_string();
        api.model = "deepseek-v4-pro".to_string();
        api.reasoning_effort = "none".to_string();

        normalize_app_config(&mut cfg);

        let resolved = resolve_api_config(&cfg, Some(&cfg.selected_api_config_id))
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
            selected_api_config_id: "legacy-openai".to_string(),
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
                tools: default_api_tools(),
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
        assert_eq!(
            cfg.selected_api_config_id,
            "legacy-openai::legacy-openai-model-default".to_string()
        );
        assert_eq!(cfg.api_configs.len(), 1);
        assert_eq!(cfg.api_configs[0].id, cfg.selected_api_config_id);
    }

    #[test]
    fn normalize_app_config_should_migrate_legacy_api_configs_when_serde_injected_default_provider() {
        let mut cfg: AppConfig = toml::from_str(
            r#"
hotkey = "Alt+·"
selectedApiConfigId = "legacy-openai"
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
        assert_eq!(
            cfg.selected_api_config_id,
            "legacy-openai::legacy-openai-model-default".to_string()
        );
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
selectedApiConfigId = "provider-a::model-a"
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
            selected_api_config_id: api_endpoint_id(&provider_id, &model_a),
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
                tools: default_api_tools(),
                base_url: "https://api.openai.com/v1".to_string(),
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
            selected_api_config_id: api_endpoint_id(&provider_id, &model_id),
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
                tools: default_api_tools(),
                base_url: "https://new.sharedchat.cc/codex".to_string(),
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
            tools: default_agent_tools(),
            created_at: "2026-04-15T00:00:00Z".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: default_main_source(),
            scope: default_global_scope(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
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
            active_goal: None, last_error: None,
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
        let stats = crate::migrate_departments_into_agent_organization(&context)
            .expect("run v5 organization migration");
        assert!(stats.data_changed, "迁移应产生人格数据变化");

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
        // 内置 3 个人格（reviewer/saddler/support）由代码预设补齐，
        // 它们与根的层级来自代码预设而非迁移推导，迁移只补自定义部门相关的人-人边。
        let assistants = by_id(DEFAULT_AGENT_ID);
        let assistants_children = &assistants.child_agent_ids;
        assert!(assistants_children.iter().any(|id| id == DEPUTY_AGENT_ID));
        assert!(assistants_children.iter().any(|id| id == "yae-miko"));
        for node_id in ["reviewer", "saddler", "support"] {
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
        let second = crate::migrate_departments_into_agent_organization(&context)
            .expect("rerun v5 organization migration");
        assert!(!second.data_changed, "重复迁移应幂等");
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
            tools: default_agent_tools(),
            created_at: "2026-04-15T00:00:00Z".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
            avatar_path: avatar,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: default_main_source(),
            scope: default_global_scope(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
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
        let stats = migrate_avatar_paths_to_relative(&context).expect("run v6 avatar migration");
        assert!(stats.data_changed);

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
        let second = migrate_avatar_paths_to_relative(&context).expect("rerun v6 migration");
        assert!(!second.data_changed, "重复迁移应幂等");
    }

    #[test]
    fn migration_should_remove_hr_persona_and_prune_child_references() {
        let state = config_test_state();
        let make_agent = |id: &str, child_agent_ids: Vec<String>| AgentProfile {
            id: id.to_string(),
            name: id.to_string(),
            system_prompt: "test".to_string(),
            tools: default_agent_tools(),
            created_at: "2026-04-15T00:00:00Z".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: default_main_source(),
            scope: default_global_scope(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
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
        let stats = migrate_remove_hr_persona(&context).expect("run v7 hr migration");
        assert!(stats.data_changed);

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
        let second = migrate_remove_hr_persona(&context).expect("rerun v7 migration");
        assert!(!second.data_changed, "重复迁移应幂等");
    }
