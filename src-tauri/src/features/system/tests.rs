    #[test]
    fn candidate_openai_chat_urls_should_handle_common_forms() {
        assert_eq!(
            candidate_openai_chat_urls("https://api.openai.com/v1"),
            vec!["https://api.openai.com/v1/chat/completions".to_string()]
        );
        assert_eq!(
            candidate_openai_chat_urls("https://gateway.example.com/chat/completions"),
            vec!["https://gateway.example.com/chat/completions".to_string()]
        );
        assert_eq!(
            candidate_openai_chat_urls("https://gateway.example.com"),
            vec![
                "https://gateway.example.com/chat/completions".to_string(),
                "https://gateway.example.com/v1/chat/completions".to_string()
            ]
        );
        assert!(candidate_openai_chat_urls("  ").is_empty());
    }


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
            request_format: "openai".to_string(),
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
            request_format: "openai".to_string(),
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
    fn openai_stream_request_with_sink_should_emit_incremental_deltas() {
        let server = MockServer::start();
        let sse_body = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n",
            "\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n",
            "\n",
            "data: [DONE]\n",
            "\n"
        );
        let sse_mock = server.mock(|when, then| {
            when.method(POST).path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(sse_body);
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("build reqwest client");
        let body = serde_json::json!({
          "model": "gpt-4o-mini",
          "messages": [{ "role": "user", "content": "hello" }],
          "stream": true
        });

        let mut deltas = Vec::<String>::new();
        let rt = test_runtime();
        let (full_text, reasoning_standard, reasoning_inline, tool_calls) = rt
            .block_on(openai_stream_request_with_sink(
                &client,
                &format!("{}/v1/chat/completions", server.base_url()),
                body,
                |kind, delta| {
                    if kind == "text" {
                        deltas.push(delta.to_string());
                    }
                },
            ))
            .expect("stream request should parse");

        sse_mock.assert();
        assert_eq!(deltas, vec!["你".to_string(), "好".to_string()]);
        assert_eq!(full_text, "你好".to_string());
        assert!(reasoning_standard.is_empty());
        assert!(reasoning_inline.is_empty());
        assert!(tool_calls.is_empty());
    }

    #[test]
    fn openai_stream_request_with_sink_should_assemble_tool_calls_from_fragments() {
        let server = MockServer::start();
        let sse_body = concat!(
      "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"bing_\",\"arguments\":\"{\\\"query\\\":\\\"\"}}]}}]}\n",
      "\n",
      "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"name\":\"search\",\"arguments\":\"rust\\\"}\"}}]}}]}\n",
      "\n",
      "data: [DONE]\n",
      "\n"
    );
        let sse_mock = server.mock(|when, then| {
            when.method(POST).path("/v1/chat/completions");
            then.status(200)
                .header("content-type", "text/event-stream")
                .body(sse_body);
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("build reqwest client");
        let body = serde_json::json!({
          "model": "gpt-4o-mini",
          "messages": [{ "role": "user", "content": "hello" }],
          "stream": true
        });

        let rt = test_runtime();
        let (_full_text, _reasoning_standard, _reasoning_inline, tool_calls) = rt
            .block_on(openai_stream_request_with_sink(
                &client,
                &format!("{}/v1/chat/completions", server.base_url()),
                body,
                |_kind, _delta| {},
            ))
            .expect("stream tool call should parse");

        sse_mock.assert();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1".to_string());
        assert_eq!(tool_calls[0].function.name, "bing_search".to_string());
        assert_eq!(
            tool_calls[0].function.arguments,
            "{\"query\":\"rust\"}".to_string()
        );
    }

