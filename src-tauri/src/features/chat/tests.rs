    #[test]
    fn build_prompt_should_include_structured_tool_history_messages() {
        let now = now_iso();
        let mut assistant_with_tool = test_text_message("assistant", "我去查一下", &now);
        assistant_with_tool.tool_call = Some(vec![
            serde_json::json!({
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "bing_search",
                        "arguments": "{\"query\":\"rust\"}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_call_id": "call_1",
                "content": "{\"results\":[{\"title\":\"Rust\"}]}"
            }),
        ]);

        let messages = vec![
            test_text_message("user", "帮我查 Rust", &now),
            assistant_with_tool,
            test_text_message("user", "继续", &now),
        ];
        let conv = test_active_conversation_with_messages(messages, Some(now));
        let agent = default_agent();

        let prepared = build_prompt(
            &conv,
            &agent,
            "用户",
            "我是...",
            DEFAULT_RESPONSE_STYLE_ID,
            "zh-CN",
        );

        assert!(
            prepared
                .history_messages
                .iter()
                .any(|m| m.role == "assistant" && m.tool_calls.is_some())
        );
        assert!(
            prepared.history_messages.iter().any(|m| {
                m.role == "tool"
                    && m.tool_call_id.as_deref() == Some("call_1")
                    && m.text.contains("\"results\"")
            })
        );
    }

    #[test]
    fn request_preview_should_keep_structured_tool_history_messages() {
        let api = ApiConfig {
            id: "api-a".to_string(),
            name: "api-a".to_string(),
            request_format: "openai".to_string(),
            enable_text: true,
            enable_image: false,
            enable_audio: false,
            enable_tools: true,
            tools: default_api_tools(),
            base_url: "https://example.com/v1".to_string(),
            api_key: "k".to_string(),
            model: "gpt-x".to_string(),
            temperature: 0.7,
            context_window_tokens: 128_000,
        };
        let prepared = PreparedPrompt {
            preamble: "sys".to_string(),
            history_messages: vec![
                PreparedHistoryMessage {
                    role: "assistant".to_string(),
                    text: String::new(),
                    user_time_text: None,
                    tool_calls: Some(vec![serde_json::json!({
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "bing_search", "arguments": "{\"query\":\"rust\"}" }
                    })]),
                    tool_call_id: None,
                    reasoning_content: None,
                },
                PreparedHistoryMessage {
                    role: "tool".to_string(),
                    text: "{\"results\":[{\"title\":\"Rust\"}]}".to_string(),
                    user_time_text: None,
                    tool_calls: None,
                    tool_call_id: Some("call_1".to_string()),
                    reasoning_content: None,
                },
            ],
            latest_user_text: "继续".to_string(),
            latest_user_time_text: "2026-02-11 17:30:45".to_string(),
            latest_user_system_text: String::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };
        let preview = build_request_preview_value(
            &api,
            &prepared,
            vec![
                serde_json::json!({"type":"text","text":"继续"}),
                serde_json::json!({"type":"text","text":prepared.latest_user_time_text}),
            ],
        );
        let messages = preview
            .get("messages")
            .and_then(Value::as_array)
            .expect("messages array");
        assert!(messages.iter().any(|m| {
            m.get("role").and_then(Value::as_str) == Some("assistant")
                && m.get("tool_calls").and_then(Value::as_array).is_some()
        }));
        assert!(messages.iter().any(|m| {
            m.get("role").and_then(Value::as_str) == Some("tool")
                && m.get("tool_call_id").and_then(Value::as_str) == Some("call_1")
        }));
    }

    #[test]
    fn archive_decision_should_force_when_usage_reaches_82pct() {
        let now = now_iso();
        let huge = "中".repeat(2000);
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", &huge, &now)],
            Some(now),
        );
        let d = decide_archive_before_user_message(&conv, 1000);
        assert!(d.should_archive);
        assert!(d.forced);
        assert!(d.usage_ratio >= 0.82);
    }

    #[test]
    fn archive_decision_should_archive_after_30m_and_30pct() {
        let now = now_utc();
        let old = (now - time::Duration::minutes(31))
            .format(&Rfc3339)
            .expect("format old time");
        let text = "中".repeat(600);
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", &text, &old)],
            Some(old),
        );
        let d = decide_archive_before_user_message(&conv, 1000);
        assert!(d.should_archive);
        assert!(!d.forced);
        assert!(d.usage_ratio >= 0.30);
    }

    #[test]
    fn archive_decision_should_not_archive_when_usage_below_30pct() {
        let now = now_utc();
        let old = (now - time::Duration::minutes(31))
            .format(&Rfc3339)
            .expect("format old time");
        let conv = test_active_conversation_with_messages(
            vec![test_text_message("user", "hello", &old)],
            Some(old),
        );
        let d = decide_archive_before_user_message(&conv, 1000);
        assert!(!d.should_archive);
        assert!(!d.forced);
        assert!(d.usage_ratio < 0.30);
    }

