    #[test]
    fn image_text_cache_upsert_and_find_should_work() {
        let mut data = AppData::default();
        upsert_image_text_cache(&mut data, "h1", "vision-a", "text-a");
        assert_eq!(
            find_image_text_cache(&data, "h1", "vision-a"),
            Some("text-a".to_string())
        );

        upsert_image_text_cache(&mut data, "h1", "vision-a", "text-b");
        assert_eq!(
            find_image_text_cache(&data, "h1", "vision-a"),
            Some("text-b".to_string())
        );
        assert_eq!(find_image_text_cache(&data, "h1", "vision-b"), None);
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
    fn normalize_app_config_should_fix_invalid_record_and_stt_fields() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            ui_language: default_ui_language(),
            ui_font: default_ui_font(),
            record_hotkey: "".to_string(),
            record_background_wake_enabled: false,
            min_record_seconds: 0,
            max_record_seconds: 0,
            tool_max_iterations: 0,
            llm_round_log_capacity: 9,
            selected_api_config_id: "a1".to_string(),
            assistant_department_api_config_id: "a1".to_string(),
            vision_api_config_id: None,
            stt_api_config_id: None,
            stt_auto_send: false,
            provider_non_stream_base_urls: Vec::new(),
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: Vec::new(),
            remote_im_channels: Vec::new(),
            departments: Vec::new(),
            api_configs: vec![
                ApiConfig {
                    id: "a1".to_string(),
                    name: "chat".to_string(),
                    request_format: RequestFormat::OpenAI,
                    allow_concurrent_requests: false,
                    enable_text: true,
                    enable_image: true,
                    enable_audio: false,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
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
                    enable_text: true,
                    enable_image: false,
                    enable_audio: true,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
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
        assert_eq!(cfg.record_hotkey, "Alt");
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
            record_hotkey: "Alt".to_string(),
            record_background_wake_enabled: false,
            min_record_seconds: 1,
            max_record_seconds: 60,
            tool_max_iterations: 10,
            llm_round_log_capacity: default_llm_round_log_capacity(),
            selected_api_config_id: "edit-b".to_string(),
            assistant_department_api_config_id: "chat-a".to_string(),
            vision_api_config_id: None,
            stt_api_config_id: None,
            stt_auto_send: false,
            provider_non_stream_base_urls: Vec::new(),
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: Vec::new(),
            remote_im_channels: Vec::new(),
            departments: Vec::new(),
            api_configs: vec![
                ApiConfig {
                    id: "chat-a".to_string(),
                    name: "chat-a".to_string(),
                    request_format: RequestFormat::OpenAI,
                    allow_concurrent_requests: false,
                    enable_text: true,
                    enable_image: true,
                    enable_audio: true,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
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
                    enable_text: true,
                    enable_image: false,
                    enable_audio: false,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
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
            cfg.assistant_department_api_config_id,
            "chat-a::chat-a-model-default".to_string()
        );
    }

    #[test]
    fn normalize_app_config_should_disable_audio_capability_globally() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            ui_language: default_ui_language(),
            ui_font: default_ui_font(),
            record_hotkey: "Alt".to_string(),
            record_background_wake_enabled: false,
            min_record_seconds: 1,
            max_record_seconds: 60,
            tool_max_iterations: 10,
            llm_round_log_capacity: default_llm_round_log_capacity(),
            selected_api_config_id: "tts-a".to_string(),
            assistant_department_api_config_id: "tts-a".to_string(),
            vision_api_config_id: Some("tts-a".to_string()),
            stt_api_config_id: Some("tts-a".to_string()),
            stt_auto_send: true,
            provider_non_stream_base_urls: Vec::new(),
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: Vec::new(),
            remote_im_channels: Vec::new(),
            departments: Vec::new(),
            api_configs: vec![ApiConfig {
                id: "tts-a".to_string(),
                name: "tts-a".to_string(),
                request_format: RequestFormat::OpenAITts,
                allow_concurrent_requests: false,
                enable_text: true,
                enable_image: false,
                enable_audio: true,
                enable_tools: true,
                tools: vec![],
                base_url: "https://api.siliconflow.cn/v1/audio/transcriptions".to_string(),
                api_key: "k".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                model: "m".to_string(),
                reasoning_effort: default_reasoning_effort(),
                temperature: 1.0,
                custom_temperature_enabled: false,
                context_window_tokens: 128_000,
                max_output_tokens: 4_096,
                custom_max_output_tokens_enabled: false,
                failure_retry_count: 0,
            }],
            api_providers: Vec::new(),
            tool_review_api_config_id: None,
        };
        normalize_app_config(&mut cfg);
        let api = &cfg.api_configs[0];
        assert!(api.enable_text);
        assert!(!api.enable_image);
        assert!(!api.enable_audio);
        assert!(api.enable_tools);
        assert_eq!(cfg.vision_api_config_id, None);
        assert!(cfg.stt_auto_send);
    }

    #[test]
    fn normalize_app_config_should_force_deputy_departments_to_deputy_only_agents() {
        let mut cfg = AppConfig::default();
        let mut primary = default_assistant_department("");
        primary.id = "department-primary".to_string();
        primary.name = "主部门".to_string();
        primary.is_built_in_assistant = false;
        primary.agent_ids = vec!["agent-a".to_string()];

        let mut deputy = default_deputy_department("");
        deputy.id = "department-deputy-custom".to_string();
        deputy.name = "临时副手".to_string();
        deputy.agent_ids = vec!["agent-a".to_string()];

        cfg.departments = vec![primary, deputy];

        normalize_app_config(&mut cfg);

        let primary = cfg
            .departments
            .iter()
            .find(|item| item.id == "department-primary")
            .expect("primary department");
        assert!(!primary.is_deputy);
        assert_eq!(primary.agent_ids, vec!["agent-a".to_string()]);

        let deputy = cfg
            .departments
            .iter()
            .find(|item| item.id == "department-deputy-custom")
            .expect("custom deputy department");
        assert!(deputy.is_deputy);
        assert_eq!(deputy.agent_ids, vec![DEPUTY_AGENT_ID.to_string()]);

        let builtin_deputy = cfg
            .departments
            .iter()
            .find(|item| item.id == DEPUTY_DEPARTMENT_ID)
            .expect("builtin deputy department");
        assert!(builtin_deputy.is_deputy);
        assert_eq!(builtin_deputy.agent_ids, vec![DEPUTY_AGENT_ID.to_string()]);
    }

    #[test]
    fn startup_self_check_should_fix_deputy_department_default_agent() {
        let mut cfg = AppConfig::default();
        let deputy = cfg
            .departments
            .iter_mut()
            .find(|item| item.id == DEPUTY_DEPARTMENT_ID)
            .expect("builtin deputy department");
        deputy.agent_ids = vec![DEFAULT_AGENT_ID.to_string()];
        let previous_updated_at = deputy.updated_at.clone();

        assert!(run_startup_self_checks(&mut cfg));

        let deputy = cfg
            .departments
            .iter()
            .find(|item| item.id == DEPUTY_DEPARTMENT_ID)
            .expect("builtin deputy department");
        assert_eq!(deputy.agent_ids, vec![DEPUTY_AGENT_ID.to_string()]);
        assert!(!deputy.updated_at.is_empty());
        assert_eq!(parse_rfc3339_time(&deputy.updated_at).is_some(), true);
        let _ = previous_updated_at;
    }

    #[test]
    fn normalize_app_config_should_drop_invalid_department_models_without_frontend_fallback() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            ui_language: default_ui_language(),
            ui_font: default_ui_font(),
            record_hotkey: "Alt".to_string(),
            record_background_wake_enabled: false,
            min_record_seconds: 1,
            max_record_seconds: 60,
            tool_max_iterations: 10,
            llm_round_log_capacity: default_llm_round_log_capacity(),
            selected_api_config_id: "embed-a".to_string(),
            assistant_department_api_config_id: "chat-a".to_string(),
            vision_api_config_id: None,
            stt_api_config_id: None,
            stt_auto_send: false,
            provider_non_stream_base_urls: Vec::new(),
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: Vec::new(),
            remote_im_channels: Vec::new(),
            departments: vec![
                DepartmentConfig {
                    id: ASSISTANT_DEPARTMENT_ID.to_string(),
                    name: "助理部门".to_string(),
                    summary: String::new(),
                    guide: String::new(),
                    api_config_ids: vec!["embed-a".to_string()],
                    api_config_id: "embed-a".to_string(),
                    agent_ids: vec![DEFAULT_AGENT_ID.to_string()],
                    created_at: "2026-03-10T00:00:00Z".to_string(),
                    updated_at: "2026-03-10T00:00:00Z".to_string(),
                    order_index: 1,
                    is_built_in_assistant: true,
                    is_deputy: false,
                    source: default_main_source(),
                    scope: default_global_scope(),
                    permission_control: DepartmentPermissionControl::default(),
                },
                DepartmentConfig {
                    id: "department-research".to_string(),
                    name: "资料部".to_string(),
                    summary: String::new(),
                    guide: String::new(),
                    api_config_ids: vec!["stt-a".to_string()],
                    api_config_id: "stt-a".to_string(),
                    agent_ids: vec![],
                    created_at: "2026-03-10T00:00:00Z".to_string(),
                    updated_at: "2026-03-10T00:00:00Z".to_string(),
                    order_index: 2,
                    is_built_in_assistant: false,
                    is_deputy: false,
                    source: default_main_source(),
                    scope: default_global_scope(),
                    permission_control: DepartmentPermissionControl::default(),
                },
            ],
            api_configs: vec![
                ApiConfig {
                    id: "embed-a".to_string(),
                    name: "embed-a".to_string(),
                    request_format: RequestFormat::OpenAIEmbedding,
                    allow_concurrent_requests: false,
                    enable_text: true,
                    enable_image: false,
                    enable_audio: false,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
                    model: "embed".to_string(),
                    reasoning_effort: default_reasoning_effort(),
                    temperature: 1.0,
                    custom_temperature_enabled: false,
                    context_window_tokens: 128_000,
                    max_output_tokens: 4_096,
                    custom_max_output_tokens_enabled: false,
                    failure_retry_count: 0,
                },
                ApiConfig {
                    id: "stt-a".to_string(),
                    name: "stt-a".to_string(),
                    request_format: RequestFormat::OpenAIStt,
                    allow_concurrent_requests: false,
                    enable_text: false,
                    enable_image: false,
                    enable_audio: false,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
                    model: "stt".to_string(),
                    reasoning_effort: default_reasoning_effort(),
                    temperature: 1.0,
                    custom_temperature_enabled: false,
                    context_window_tokens: 128_000,
                    max_output_tokens: 4_096,
                    custom_max_output_tokens_enabled: false,
                    failure_retry_count: 0,
                },
                ApiConfig {
                    id: "chat-a".to_string(),
                    name: "chat-a".to_string(),
                    request_format: RequestFormat::OpenAI,
                    allow_concurrent_requests: false,
                    enable_text: true,
                    enable_image: true,
                    enable_audio: false,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    codex_auth_mode: default_codex_auth_mode(),
                    codex_local_auth_path: default_codex_local_auth_path(),
                    model: "chat".to_string(),
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

        assert_eq!(cfg.assistant_department_api_config_id, "");
        let assistant = cfg
            .departments
            .iter()
            .find(|item| item.id == ASSISTANT_DEPARTMENT_ID)
            .expect("assistant department");
        assert_eq!(assistant.api_config_id, "");
        assert!(assistant.api_config_ids.is_empty());
        let research = cfg
            .departments
            .iter()
            .find(|item| item.id == "department-research")
            .expect("research department");
        assert_eq!(research.api_config_id, "");
        assert!(research.api_config_ids.is_empty());
    }

    #[test]
    fn normalize_app_config_should_preserve_empty_assistant_department_model() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            ui_language: default_ui_language(),
            ui_font: default_ui_font(),
            record_hotkey: "Alt".to_string(),
            record_background_wake_enabled: false,
            min_record_seconds: 1,
            max_record_seconds: 60,
            tool_max_iterations: 10,
            llm_round_log_capacity: default_llm_round_log_capacity(),
            selected_api_config_id: "chat-a".to_string(),
            assistant_department_api_config_id: String::new(),
            vision_api_config_id: None,
            stt_api_config_id: None,
            stt_auto_send: false,
            provider_non_stream_base_urls: Vec::new(),
            terminal_shell_kind: default_terminal_shell_kind(),
            shell_workspaces: Vec::new(),
            mcp_servers: Vec::new(),
            remote_im_channels: Vec::new(),
            departments: vec![DepartmentConfig {
                id: ASSISTANT_DEPARTMENT_ID.to_string(),
                name: "助理部门".to_string(),
                summary: String::new(),
                guide: String::new(),
                api_config_ids: Vec::new(),
                api_config_id: String::new(),
                agent_ids: vec![DEFAULT_AGENT_ID.to_string()],
                created_at: "2026-03-10T00:00:00Z".to_string(),
                updated_at: "2026-03-10T00:00:00Z".to_string(),
                order_index: 1,
                is_built_in_assistant: true,
                is_deputy: false,
                source: default_main_source(),
                scope: default_global_scope(),
                permission_control: DepartmentPermissionControl::default(),
            }],
            api_configs: vec![ApiConfig {
                id: "chat-a".to_string(),
                name: "chat-a".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                enable_text: true,
                enable_image: true,
                enable_audio: false,
                enable_tools: false,
                tools: vec![],
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "k".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                model: "chat".to_string(),
                reasoning_effort: default_reasoning_effort(),
                temperature: 1.0,
                custom_temperature_enabled: false,
                context_window_tokens: 128_000,
                max_output_tokens: 4_096,
                custom_max_output_tokens_enabled: false,
                failure_retry_count: 0,
            }],
            api_providers: Vec::new(),
            tool_review_api_config_id: None,
        };

        normalize_app_config(&mut cfg);

        assert_eq!(cfg.assistant_department_api_config_id, "");
        assert_eq!(cfg.departments[0].api_config_id, "");
        assert!(cfg.departments[0].api_config_ids.is_empty());
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
            assistant_department_api_config_id: "legacy-openai".to_string(),
            api_providers: Vec::new(),
            tool_review_api_config_id: None,
            api_configs: vec![ApiConfig {
                id: "legacy-openai".to_string(),
                name: "Legacy OpenAI".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_tools: true,
                tools: default_api_tools(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: "legacy-key".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
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
            assistant_department_api_config_id: api_endpoint_id(&provider_id, &model_a),
            api_providers: vec![ApiProviderConfig {
                id: provider_id.clone(),
                name: "OpenAI".to_string(),
                request_format: RequestFormat::OpenAI,
                allow_concurrent_requests: false,
                enable_text: true,
                enable_image: false,
                enable_audio: false,
                enable_tools: true,
                tools: default_api_tools(),
                base_url: "https://api.openai.com/v1".to_string(),
                codex_auth_mode: default_codex_auth_mode(),
                codex_local_auth_path: default_codex_local_auth_path(),
                api_keys: vec!["key-1".to_string(), "key-2".to_string()],
                key_cursor: 0,
                cached_model_options: vec!["gpt-4.1".to_string(), "gpt-4.1-mini".to_string()],
                models: vec![
                    ApiModelConfig {
                        id: model_a.clone(),
                        model: "gpt-4.1".to_string(),
                        enable_image: false,
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
                        enable_image: false,
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
    fn write_app_data_should_only_flush_changed_shards() {
        let root = std::env::temp_dir().join(format!("eca-app-data-layout-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create temp config dir");
        let data_path = root.join("config").join("app_data.json");

        let mut data = AppData::default();
        data.conversations = vec![
            build_test_conversation("conv-a", "Conversation A"),
            build_test_conversation("conv-b", "Conversation B"),
        ];

        let first = write_app_data_with_stats(&data_path, &data).expect("write first layout");
        assert!(first.agents_written);
        assert!(first.runtime_written);
        assert!(first.chat_index_written);
        assert_eq!(first.conversation_writes, 2);
        assert_eq!(first.conversation_deletes, 0);

        let second = write_app_data_with_stats(&data_path, &data).expect("write same layout");
        assert!(!second.agents_written);
        assert!(!second.runtime_written);
        assert!(!second.chat_index_written);
        assert_eq!(second.conversation_writes, 0);
        assert_eq!(second.conversation_deletes, 0);

        let mut runtime_only = data.clone();
        runtime_only.assistant_department_agent_id = "agent-runtime-only".to_string();
        let runtime_stats =
            write_app_data_with_stats(&data_path, &runtime_only).expect("write runtime-only diff");
        assert!(!runtime_stats.agents_written);
        assert!(runtime_stats.runtime_written);
        assert!(!runtime_stats.chat_index_written);
        assert_eq!(runtime_stats.conversation_writes, 0);
        assert_eq!(runtime_stats.conversation_deletes, 0);
    }

    #[test]
    fn write_agents_and_runtime_shards_should_not_touch_other_files() {
        let root = std::env::temp_dir().join(format!("eca-app-data-shards-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create temp config dir");
        let data_path = root.join("config").join("app_data.json");

        let mut data = AppData::default();
        data.assistant_department_agent_id = "assistant-before".to_string();
        data.conversations = vec![build_test_conversation("conv-a", "Conversation A")];
        write_app_data_with_stats(&data_path, &data).expect("seed layout");

        let agents_path = app_layout_agents_path(&data_path);
        let runtime_path = app_layout_runtime_state_path(&data_path);
        let conversation_paths =
            message_store::message_store_paths(&data_path, "conv-a").expect("conversation paths");

        let runtime_before = std::fs::read(&runtime_path).expect("read runtime before");
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
            source: default_main_source(),
            scope: default_global_scope(),
        });
        assert!(write_agents_shard(&data_path, &agents).expect("write agents shard"));
        assert_eq!(std::fs::read(&runtime_path).expect("read runtime after agents"), runtime_before);
        assert_eq!(
            message_store::message_store_shard_write_signature(&conversation_paths),
            conversation_before
        );

        let mut runtime = read_runtime_state_shard(&data_path).expect("read runtime shard");
        runtime.assistant_department_agent_id = "assistant-after".to_string();
        assert!(write_runtime_state_shard(&data_path, &runtime).expect("write runtime shard"));
        assert_ne!(
            std::fs::read(&runtime_path).expect("read runtime after runtime write"),
            runtime_before
        );
        assert_eq!(
            message_store::message_store_shard_write_signature(&conversation_paths),
            conversation_before
        );
        assert!(!std::fs::read(&agents_path).expect("read agents after runtime").is_empty());
    }

    #[test]
    fn write_conversation_shard_should_write_message_store_and_only_touch_target() {
        let root = std::env::temp_dir().join(format!("eca-conversation-shard-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create temp config dir");
        let data_path = root.join("config").join("app_data.json");

        let mut data = AppData::default();
        data.conversations = vec![
            build_test_conversation("conv-a", "Conversation A"),
            build_test_conversation("conv-b", "Conversation B"),
        ];
        write_app_data_with_stats(&data_path, &data).expect("seed layout");

        let legacy_conversation_a_path = app_layout_chat_conversation_path(&data_path, "conv-a");
        let legacy_conversation_b_path = app_layout_chat_conversation_path(&data_path, "conv-b");
        assert!(!legacy_conversation_a_path.exists());
        assert!(!legacy_conversation_b_path.exists());
        let conversation_a_paths =
            message_store::message_store_paths(&data_path, "conv-a").expect("conversation a paths");
        let conversation_b_paths =
            message_store::message_store_paths(&data_path, "conv-b").expect("conversation b paths");
        assert!(message_store::should_write_jsonl_snapshot_directory_shard(&conversation_a_paths)
            .expect("conversation a manifest ready"));
        assert!(message_store::should_write_jsonl_snapshot_directory_shard(&conversation_b_paths)
            .expect("conversation b manifest ready"));
        let conversation_a_before =
            message_store::message_store_shard_write_signature(&conversation_a_paths);
        let conversation_b_before =
            message_store::message_store_shard_write_signature(&conversation_b_paths);

        let mut conversation_a = read_conversation_shard(&data_path, "conv-a").expect("read conversation a");
        conversation_a.title = "Conversation A Updated".to_string();
        assert!(write_conversation_shard(&data_path, &conversation_a).expect("write conversation a"));

        assert_ne!(
            message_store::message_store_shard_write_signature(&conversation_a_paths),
            conversation_a_before
        );
        assert_eq!(
            message_store::message_store_shard_write_signature(&conversation_b_paths),
            conversation_b_before
        );
        assert!(!legacy_conversation_a_path.exists());
        assert!(!legacy_conversation_b_path.exists());
    }

    #[test]
    fn write_chat_index_shard_should_not_touch_conversation_files() {
        let root = std::env::temp_dir().join(format!("eca-chat-index-shard-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create temp config dir");
        let data_path = root.join("config").join("app_data.json");

        let mut data = AppData::default();
        data.conversations = vec![
            build_test_conversation("conv-a", "Conversation A"),
            build_test_conversation("conv-b", "Conversation B"),
        ];
        write_app_data_with_stats(&data_path, &data).expect("seed layout");

        let chat_index_path = app_layout_chat_index_path(&data_path);
        let conversation_a_paths =
            message_store::message_store_paths(&data_path, "conv-a").expect("conversation a paths");
        let conversation_b_paths =
            message_store::message_store_paths(&data_path, "conv-b").expect("conversation b paths");
        let conversation_a_before =
            message_store::message_store_shard_write_signature(&conversation_a_paths);
        let conversation_b_before =
            message_store::message_store_shard_write_signature(&conversation_b_paths);
        let chat_index_before = std::fs::read(&chat_index_path).expect("read chat index before");

        let mut index = read_chat_index_shard(&data_path).expect("read chat index shard");
        index.conversations.reverse();
        assert!(write_chat_index_shard(&data_path, &index).expect("write chat index shard"));

        assert_ne!(
            std::fs::read(&chat_index_path).expect("read chat index after"),
            chat_index_before
        );
        assert_eq!(
            message_store::message_store_shard_write_signature(&conversation_a_paths),
            conversation_a_before
        );
        assert_eq!(
            message_store::message_store_shard_write_signature(&conversation_b_paths),
            conversation_b_before
        );
    }

    #[test]
    fn upsert_chat_index_conversation_should_replace_existing_item_without_duplicates() {
        let mut conversation_a = build_test_conversation("conv-a", "Conversation A");
        let conversation_b = build_test_conversation("conv-b", "Conversation B");
        let mut index = build_chat_index_file(&[conversation_a.clone(), conversation_b.clone()]);

        conversation_a.updated_at = "2026-04-15T12:34:56Z".to_string();
        conversation_a.status = "archived".to_string();
        conversation_a.summary = "updated summary".to_string();
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
        assert_eq!(updated.summary, "updated summary");
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
    fn runtime_state_shard_should_preserve_pdf_caches() {
        let root = std::env::temp_dir().join(format!("eca-runtime-pdf-cache-{}", Uuid::new_v4()));
        std::fs::create_dir_all(root.join("config")).expect("create temp config dir");
        let data_path = root.join("config").join("app_data.json");

        let mut data = AppData::default();
        data.pdf_text_cache.push(PdfTextCacheEntry {
            file_hash: "file-hash-a".to_string(),
            file_path: "C:/tmp/a.pdf".to_string(),
            file_name: "a.pdf".to_string(),
            extracted_text: "pdf text".to_string(),
            total_pages: 8,
            extracted_pages: 3,
            is_truncated: true,
            conversation_ids: vec!["conv-a".to_string()],
            created_at: "2026-04-15T00:00:00Z".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
        });
        data.pdf_image_cache.push(PdfImageCacheEntry {
            file_hash: "file-hash-b".to_string(),
            file_path: "C:/tmp/b.pdf".to_string(),
            file_name: "b.pdf".to_string(),
            total_pages: 4,
            rendered_pages: 2,
            dpi: 144,
            images: vec![PdfRenderedImage {
                page_index: 0,
                width: 100,
                height: 200,
                bytes_base64: "Zm9v".to_string(),
                mime: "image/png".to_string(),
            }],
            conversation_ids: vec!["conv-b".to_string()],
            created_at: "2026-04-15T00:00:00Z".to_string(),
            updated_at: "2026-04-15T00:00:00Z".to_string(),
        });

        let runtime = build_runtime_state_file(&data);
        assert!(write_runtime_state_shard(&data_path, &runtime).expect("write runtime shard"));

        let restored = read_runtime_state_shard(&data_path).expect("read runtime shard");
        assert_eq!(restored.pdf_text_cache.len(), 1);
        assert_eq!(restored.pdf_text_cache[0].file_name, "a.pdf");
        assert_eq!(restored.pdf_image_cache.len(), 1);
        assert_eq!(restored.pdf_image_cache[0].file_name, "b.pdf");
        assert_eq!(restored.pdf_image_cache[0].images.len(), 1);
    }

    fn build_test_conversation(id: &str, title: &str) -> Conversation {
        Conversation {
            id: id.to_string(),
            title: title.to_string(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
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
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            archived_at: None,
            messages: vec![ChatMessage {
                id: format!("{id}-message-1"),
                role: "user".to_string(),
                created_at: "2026-04-15T00:00:00Z".to_string(),
                speaker_agent_id: None,
                parts: vec![MessagePart::Text {
                    text: "hello".to_string(),
                }],
                extra_text_blocks: Vec::new(),
                provider_meta: None,
                tool_call: None,
                mcp_call: None,
            }],
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        }
    }
