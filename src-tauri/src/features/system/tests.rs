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
        };

        let rt = test_runtime();
        let models = rt
            .block_on(fetch_models_openai(&input))
            .expect("fallback /v1/models should succeed");

        base_404_mock.assert();
        v1_ok_mock.assert();
        assert_eq!(models, vec!["moonshot-v1-8k".to_string()]);
    }
