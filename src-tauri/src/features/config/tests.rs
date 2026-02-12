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
            record_hotkey: "".to_string(),
            min_record_seconds: 0,
            max_record_seconds: 0,
            tool_max_iterations: 0,
            selected_api_config_id: "a1".to_string(),
            chat_api_config_id: "a1".to_string(),
            vision_api_config_id: None,
            api_configs: vec![
                ApiConfig {
                    id: "a1".to_string(),
                    name: "chat".to_string(),
                    request_format: "openai".to_string(),
                    enable_text: true,
                    enable_image: true,
                    enable_audio: false,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    model: "m".to_string(),
                    temperature: 1.0,
                    context_window_tokens: 128_000,
                },
                ApiConfig {
                    id: "a2".to_string(),
                    name: "bad-stt".to_string(),
                    request_format: "openai".to_string(),
                    enable_text: true,
                    enable_image: false,
                    enable_audio: true,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    model: "m".to_string(),
                    temperature: 1.0,
                    context_window_tokens: 128_000,
                },
            ],
        };
        normalize_app_config(&mut cfg);
        assert_eq!(cfg.record_hotkey, "Alt");
        assert_eq!(cfg.min_record_seconds, 1);
        assert!(cfg.max_record_seconds >= cfg.min_record_seconds);
        assert_eq!(cfg.tool_max_iterations, 1);
    }

    #[test]
    fn normalize_app_config_should_not_bind_chat_api_to_selected_api() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            ui_language: default_ui_language(),
            record_hotkey: "Alt".to_string(),
            min_record_seconds: 1,
            max_record_seconds: 60,
            tool_max_iterations: 10,
            selected_api_config_id: "edit-b".to_string(),
            chat_api_config_id: "chat-a".to_string(),
            vision_api_config_id: None,
            api_configs: vec![
                ApiConfig {
                    id: "chat-a".to_string(),
                    name: "chat-a".to_string(),
                    request_format: "openai".to_string(),
                    enable_text: true,
                    enable_image: true,
                    enable_audio: true,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    model: "m".to_string(),
                    temperature: 1.0,
                    context_window_tokens: 128_000,
                },
                ApiConfig {
                    id: "edit-b".to_string(),
                    name: "edit-b".to_string(),
                    request_format: "openai".to_string(),
                    enable_text: true,
                    enable_image: false,
                    enable_audio: false,
                    enable_tools: false,
                    tools: vec![],
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key: "k".to_string(),
                    model: "m".to_string(),
                    temperature: 1.0,
                    context_window_tokens: 128_000,
                },
            ],
        };
        normalize_app_config(&mut cfg);
        assert_eq!(cfg.selected_api_config_id, "edit-b".to_string());
        assert_eq!(cfg.chat_api_config_id, "chat-a".to_string());
    }

    #[test]
    fn normalize_app_config_should_disable_audio_capability_globally() {
        let mut cfg = AppConfig {
            hotkey: "Alt+·".to_string(),
            ui_language: default_ui_language(),
            record_hotkey: "Alt".to_string(),
            min_record_seconds: 1,
            max_record_seconds: 60,
            tool_max_iterations: 10,
            selected_api_config_id: "tts-a".to_string(),
            chat_api_config_id: "tts-a".to_string(),
            vision_api_config_id: Some("tts-a".to_string()),
            api_configs: vec![ApiConfig {
                id: "tts-a".to_string(),
                name: "tts-a".to_string(),
                request_format: "openai_tts".to_string(),
                enable_text: true,
                enable_image: false,
                enable_audio: true,
                enable_tools: true,
                tools: vec![],
                base_url: "https://api.siliconflow.cn/v1/audio/transcriptions".to_string(),
                api_key: "k".to_string(),
                model: "m".to_string(),
                temperature: 1.0,
                context_window_tokens: 128_000,
            }],
        };
        normalize_app_config(&mut cfg);
        let api = &cfg.api_configs[0];
        assert!(api.enable_text);
        assert!(!api.enable_image);
        assert!(!api.enable_audio);
        assert!(api.enable_tools);
        assert_eq!(cfg.vision_api_config_id, None);
    }

