fn expand(json: &str) -> Result<ParsedMcpDefinition, McpDefinitionValidationError> {
    parse_mcp_definition_servers(json)
}

fn expand_names(json: &str) -> Vec<String> {
    expand(json)
        .expect("expand ok")
        .servers
        .into_iter()
        .map(|(n, _)| n)
        .collect()
}

fn parse_single(json: &str) -> ParsedMcpServerDefinition {
    let (_, parsed) = parse_mcp_server_definition(json).expect("parse ok");
    parsed
}

#[test]
fn expand_mcp_servers_object_format() {
    let json = r#"{
        "mcpServers": {
            "context7": { "command": "npx", "args": ["-y", "@upstash/context7-mcp"] },
            "filesystem": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"] }
        }
    }"#;
    assert_eq!(expand_names(json), vec!["context7", "filesystem"]);
}

#[test]
fn expand_root_named_object_format() {
    let json = r#"{
        "context7": { "command": "npx", "args": ["-y", "@upstash/context7-mcp"] },
        "time": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-time"] }
    }"#;
    assert_eq!(expand_names(json), vec!["context7", "time"]);
}

#[test]
fn expand_mcp_servers_array_format() {
    let json = r#"{
        "mcpServers": [
            { "name": "context7", "command": "npx", "args": ["-y", "@upstash/context7-mcp"] },
            { "name": "fetch", "type": "http", "url": "https://mcp.example.com/fetch" }
        ]
    }"#;
    assert_eq!(expand_names(json), vec!["context7", "fetch"]);
}

#[test]
fn expand_root_array_format() {
    let json = r#"[
        { "name": "context7", "command": "npx", "args": ["-y", "@upstash/context7-mcp"] },
        { "name": "fetch", "url": "https://mcp.example.com/fetch" }
    ]"#;
    assert_eq!(expand_names(json), vec!["context7", "fetch"]);
}

#[test]
fn expand_single_server_direct_fields_backward_compat() {
    let json = r#"{ "command": "npx", "args": ["-y", "@upstash/context7-mcp"] }"#;
    assert_eq!(expand_names(json), vec!["mcp-server"]);
}

#[test]
fn expand_single_server_named_backward_compat() {
    let json = r#"{ "context7": { "command": "npx" } }"#;
    assert_eq!(expand_names(json), vec!["context7"]);
}

#[test]
fn headers_alias_maps_to_http_headers() {
    let json = r#"{
        "mcpServers": {
            "zhihu-search": {
                "headers": { "Authorization": "Bearer token123" },
                "transport": "sse",
                "url": "https://developer.zhihu.com/api/mcp/zhihu_search/v1/sse"
            }
        }
    }"#;
    let parsed = parse_single(json);
    assert_eq!(parsed.transport, McpTransportKind::Sse);
    assert_eq!(
        parsed.http_headers.get("Authorization").map(|s| s.as_str()),
        Some("Bearer token123")
    );
}

#[test]
fn headers_and_http_headers_merge() {
    let json = r#"{
        "headers": { "X-A": "1" },
        "httpHeaders": { "X-B": "2" },
        "url": "https://example.com/mcp"
    }"#;
    let parsed = parse_single(json);
    assert_eq!(parsed.http_headers.get("X-A").map(|s| s.as_str()), Some("1"));
    assert_eq!(parsed.http_headers.get("X-B").map(|s| s.as_str()), Some("2"));
}

#[test]
fn transport_type_alias_and_omit_inference() {
    let sse = parse_single(r#"{ "type": "sse", "url": "https://x/sse" }"#);
    assert_eq!(sse.transport, McpTransportKind::Sse);

    let stdio = parse_single(r#"{ "command": "npx", "args": ["-y", "pkg"] }"#);
    assert_eq!(stdio.transport, McpTransportKind::Stdio);

    let http = parse_single(r#"{ "url": "https://x/mcp" }"#);
    assert_eq!(http.transport, McpTransportKind::StreamableHttp);
}

#[test]
fn env_object_value_form_supported() {
    let json = r#"{
        "env": {
            "PLAIN": "value-a",
            "SECRET": { "value": "value-b", "secret": true }
        },
        "command": "npx"
    }"#;
    let parsed = parse_single(json);
    assert_eq!(parsed.env.get("PLAIN").map(|s| s.as_str()), Some("value-a"));
    assert_eq!(parsed.env.get("SECRET").map(|s| s.as_str()), Some("value-b"));
}

#[test]
fn missing_transport_reports_structured_issue() {
    let json = r#"{
        "mcpServers": {
            "broken": { "args": ["-y", "pkg"] }
        }
    }"#;
    let (servers, issues) = validate_mcp_definition_servers(json);
    assert_eq!(servers.len(), 1);
    assert_eq!(issues.len(), 1);
    let issue = &issues[0];
    assert_eq!(issue.code, "server_missing_transport");
    assert_eq!(issue.server_name.as_deref(), Some("broken"));
}

#[test]
fn array_missing_name_reports_issue() {
    let json = r#"{
        "mcpServers": [
            { "command": "npx" },
            { "name": "ok", "command": "npx" }
        ]
    }"#;
    let parsed = expand(json).expect("partial ok");
    assert_eq!(parsed.servers.len(), 1);
    assert_eq!(parsed.servers[0].0, "ok");
    assert!(parsed.issues.iter().any(|i| i.code == "server_missing_name"));
}

#[test]
fn array_duplicate_name_keeps_all_servers() {
    // 同名成员不去重：数组内两个同名 server 都保留，由用户自行安排顺序
    let json = r#"{
        "mcpServers": [
            { "name": "dup", "command": "npx" },
            { "name": "dup", "command": "npx" }
        ]
    }"#;
    let parsed = expand(json).expect("partial ok");
    assert_eq!(parsed.servers.len(), 2);
    assert_eq!(parsed.servers[0].0, "dup");
    assert_eq!(parsed.servers[1].0, "dup");
}

#[test]
fn invalid_json_reports_issue() {
    let err = expand("{ not json").expect_err("should fail");
    assert_eq!(err.issues[0].code, "invalid_json");
}

#[test]
fn args_type_error_reports_issue() {
    let json = r#"{
        "mcpServers": {
            "bad": { "command": "npx", "args": "not-array" }
        }
    }"#;
    let (_, issues) = validate_mcp_definition_servers(json);
    assert!(issues.iter().any(|i| i.code == "args_type_error"));
}

#[test]
fn multi_server_definition_parse_first_for_single_api() {
    let json = r#"{
        "mcpServers": {
            "a": { "command": "npx", "args": ["-y", "pkg-a"] },
            "b": { "command": "npx", "args": ["-y", "pkg-b"] }
        }
    }"#;
    let (name, parsed) = parse_mcp_server_definition(json).expect("parse ok");
    assert_eq!(name, "a");
    assert_eq!(parsed.args, vec!["-y", "pkg-a"]);
}

// ========== 一卡一组：成员解析、前缀与整组保存 ==========

fn test_server_with_definition(definition_json: &str) -> McpServerConfig {
    McpServerConfig {
        id: "mcp-test".to_string(),
        name: "测试组".to_string(),
        enabled: false,
        definition_json: definition_json.to_string(),
        oauth_capable: false,
        has_oauth_token: false,
        tool_policies: Vec::new(),
        cached_tools: Vec::new(),
        last_status: String::new(),
        last_error: String::new(),
        updated_at: String::new(),
    }
}

#[test]
fn tool_prefixed_name_normalizes_member_name_and_preserves_raw_tool_name() {
    assert_eq!(mcp_tool_prefixed_name("context7", "search"), "context7_search");
    assert_eq!(
        mcp_tool_prefixed_name("Akasha Terminal", "akasha_search"),
        "Akasha_Terminal_akasha_search"
    );
    assert_eq!(
        mcp_tool_prefixed_name("中文成员", "search"),
        "中文成员_search"
    );
    assert_eq!(
        mcp_member_name_compatibility_error("中文成员"),
        Some("MCP 组成员名规范化后没有可用字符，工具无法挂载".to_string())
    );
}

#[test]
fn parse_group_definitions_multi_members() {
    let server = test_server_with_definition(
        r#"{
            "mcpServers": {
                "a": { "command": "npx", "args": ["-y", "pkg-a"] },
                "b": { "url": "https://x/sse", "transport": "sse" }
            }
        }"#,
    );
    let members = parse_mcp_group_definitions(&server).expect("parse group");
    assert_eq!(members.len(), 2);
    assert_eq!(members[0].0, "a");
    assert_eq!(members[0].2.transport, McpTransportKind::Stdio);
    assert_eq!(members[1].0, "b");
    assert_eq!(members[1].2.transport, McpTransportKind::Sse);
}

#[test]
fn normalized_member_name_collision_across_servers_is_allowed() {
    // 跨卡片成员名重复不再拦截：同名覆盖由用户自行安排顺序
    assert_eq!(
        normalized_mcp_member_name_or_original("Akasha Terminal"),
        "Akasha_Terminal"
    );
    assert_eq!(
        normalized_mcp_member_name_or_original("Akasha_Terminal"),
        "Akasha_Terminal"
    );
}

#[test]
fn normalize_mcp_server_input_allows_multi_server_group() {
    let input = McpServerInput {
        id: "mcp-group".to_string(),
        name: "知乎组".to_string(),
        enabled: false,
        definition_json: r#"{
            "mcpServers": {
                "Akasha Terminal": { "command": "npx", "args": ["-y", "pkg"] },
                "global_search": { "url": "https://x/sse", "transport": "sse" }
            }
        }"#
        .to_string(),
    };
    let config = normalize_mcp_server_input(input).expect("整组保存不应报错");
    assert_eq!(config.name, "知乎组");
    assert!(config.definition_json.contains("Akasha_Terminal"));
    assert!(config.definition_json.contains("global_search"));
}

#[test]
fn definition_tool_filters_are_member_prefixed() {
    let server = test_server_with_definition(
        r#"{
            "mcpServers": {
                "ctx": {
                    "command": "npx",
                    "enabledTools": ["search"],
                    "disabledTools": ["trending"]
                }
            }
        }"#,
    );
    let (allow, deny) = mcp_definition_tool_filters(&server.definition_json);
    assert!(allow.contains("ctx_search"));
    assert!(deny.contains("ctx_trending"));
    assert!(mcp_tool_allowed_by_definition(&server, "ctx_search"));
    assert!(!mcp_tool_allowed_by_definition(&server, "ctx_trending"));
    assert!(!mcp_tool_allowed_by_definition(&server, "search"));
}

#[test]
fn definition_tool_filters_keep_already_prefixed_names() {
    // 与探测别名规则一致：已带成员前缀的工具名保持原样，不重复加前缀
    let server = test_server_with_definition(
        r#"{
            "mcpServers": {
                "akasha": {
                    "url": "https://mcp.example.com/akasha",
                    "transport": "sse",
                    "enabledTools": ["akasha_search"],
                    "disabledTools": ["akasha_skill"]
                }
            }
        }"#,
    );
    let (allow, deny) = mcp_definition_tool_filters(&server.definition_json);
    assert!(allow.contains("akasha_search"), "已带前缀应保持，got {allow:?}");
    assert!(deny.contains("akasha_skill"), "已带前缀应保持，got {deny:?}");
    assert!(mcp_tool_allowed_by_definition(&server, "akasha_search"));
    assert!(!mcp_tool_allowed_by_definition(&server, "akasha_skill"));
}

// ========== SSE transport 集成测试（本地 mock 服务端） ==========

mod sse_transport_tests {
    use super::*;
    use std::convert::Infallible;
    use std::sync::Arc;

    use axum::http::{HeaderMap, StatusCode};
    use axum::response::sse::{Event, Sse};
    use axum::response::IntoResponse;
    use axum::routing::{get, post};
    use axum::Router;
    use futures_util::stream::unfold;
    use tokio::sync::Mutex;

    #[derive(Clone, Default)]
    struct SseMockState {
        sse_tx: Arc<Mutex<Option<tokio::sync::mpsc::Sender<Event>>>>,
        post_headers: Arc<Mutex<Vec<HeaderMap>>>,
        tool_names: Arc<Vec<String>>,
    }

    impl SseMockState {
        fn with_tools(tools: &[&str]) -> Self {
            Self {
                sse_tx: Arc::new(Mutex::new(None)),
                post_headers: Arc::new(Mutex::new(Vec::new())),
                tool_names: Arc::new(tools.iter().map(|s| s.to_string()).collect()),
            }
        }
    }

    fn sse_mock_router(state: SseMockState) -> Router {
        Router::new()
            .route("/sse", get({
                let state = state.clone();
                move || sse_mock_handler(state.clone())
            }))
            .route("/message", post({
                let state = state.clone();
                move |headers: HeaderMap, body: String| {
                    sse_mock_message(state.clone(), headers, body)
                }
            }))
            .with_state(state)
    }

    async fn sse_mock_handler(
        state: SseMockState,
    ) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
        let (tx, rx) = tokio::sync::mpsc::channel::<Event>(16);
        let mut guard = state.sse_tx.lock().await;
        *guard = Some(tx.clone());
        drop(guard);
        let _ = tx
            .send(Event::default().event("endpoint").data("/message?sessionId=test123"))
            .await;
        let stream = unfold(rx, |mut rx| async move {
            rx.recv()
                .await
                .map(|event| (Ok::<Event, Infallible>(event), rx))
        });
        Sse::new(stream)
    }

    async fn sse_mock_message(
        state: SseMockState,
        headers: HeaderMap,
        body: String,
    ) -> impl IntoResponse {
        state.post_headers.lock().await.push(headers);
        let msg: serde_json::Value =
            serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
        // 通知（无 id）不回复，仅 202
        let Some(id) = msg.get("id").cloned() else {
            return StatusCode::ACCEPTED;
        };
        let method = msg
            .get("method")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let result = match method {
            "initialize" => serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "serverInfo": { "name": "mock-sse-server", "version": "1.0.0" }
            }),
            "tools/list" => serde_json::json!({
                "tools": state.tool_names.iter().map(|name| serde_json::json!({
                    "name": name,
                    "description": format!("tool {name}"),
                    "inputSchema": { "type": "object", "properties": {} }
                })).collect::<Vec<_>>()
            }),
            _ => serde_json::json!({}),
        };
        let response = serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result });
        let tx = state.sse_tx.lock().await.clone();
        if let Some(tx) = tx {
            let _ = tx
                .send(Event::default().event("message").data(response.to_string()))
                .await;
        }
        StatusCode::ACCEPTED
    }

    #[tokio::test]
    async fn sse_transport_full_handshake_and_headers() {
        let state = SseMockState::with_tools(&["echo"]);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock sse server");
        let addr = listener.local_addr().expect("mock addr");
        let server_state = state.clone();
        tokio::spawn(async move {
            let app = sse_mock_router(server_state);
            axum::serve(listener, app).await.expect("serve mock");
        });

        let sse_url = format!("http://{addr}/sse");
        let mut http_headers = std::collections::HashMap::<String, String>::new();
        http_headers.insert("Authorization".to_string(), "Bearer mock-token-123".to_string());
        let parsed = ParsedMcpServerDefinition {
            transport: McpTransportKind::Sse,
            command: None,
            args: Vec::new(),
            env: std::collections::HashMap::new(),
            cwd: None,
            url: Some(sse_url),
            bearer_token_env_var: None,
            http_headers,
            env_http_headers: std::collections::HashMap::new(),
        };

        let (sink, stream) = connect_sse_transport(&parsed)
            .await
            .expect("connect sse transport");
        let client = ().serve((sink, stream)).await.expect("serve sse client");
        let tools = client
            .peer()
            .list_tools(Default::default())
            .await
            .expect("list tools over sse");
        assert!(
            tools.tools.iter().any(|t| t.name == "echo"),
            "tools should contain echo, got {:?}",
            tools.tools
        );
        let _ = client.cancel();

        // 断言鉴权头确实传到了 message POST
        let headers = state.post_headers.lock().await.clone();
        assert!(!headers.is_empty(), "message POST 应至少收到一次");
        let has_auth = headers.iter().any(|h| {
            h.get("authorization")
                .map(|v| v.to_str().unwrap_or("") == "Bearer mock-token-123")
                .unwrap_or(false)
        });
        assert!(has_auth, "message POST 必须携带 Authorization 头");
    }

    async fn spawn_sse_mock(tools: &[&str]) -> String {
        let state = SseMockState::with_tools(tools);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock sse server");
        let addr = listener.local_addr().expect("mock addr");
        let server_state = state.clone();
        tokio::spawn(async move {
            let app = sse_mock_router(server_state);
            axum::serve(listener, app).await.expect("serve mock");
        });
        format!("http://{addr}/sse")
    }

    fn group_server_with_urls(members: &[(&str, &str)]) -> super::McpServerConfig {
        let mut map = serde_json::Map::new();
        for (name, url) in members {
            map.insert(
                name.to_string(),
                serde_json::json!({ "url": url, "transport": "sse" }),
            );
        }
        super::test_server_with_definition(
            &serde_json::json!({ "mcpServers": map }).to_string(),
        )
    }

    #[tokio::test]
    async fn sse_group_multi_members_merge_tools_with_prefix() {
        let url_a = spawn_sse_mock(&["echo"]).await;
        let url_b = spawn_sse_mock(&["fetch"]).await;
        let server = group_server_with_urls(&[("alpha", &url_a), ("beta", &url_b)]);

        let tools = mcp_list_server_tools_runtime(&server)
            .await
            .expect("list group tools");
        let names = tools
            .iter()
            .map(|t| t.tool_name.as_str())
            .collect::<Vec<_>>();
        assert!(
            names.contains(&"alpha_echo"),
            "应包含前缀工具名 alpha_echo, got {names:?}"
        );
        assert!(
            names.contains(&"beta_fetch"),
            "应包含前缀工具名 beta_fetch, got {names:?}"
        );
    }

    #[tokio::test]
    async fn sse_group_tools_already_prefixed_kept_unchanged() {
        // 成员返回的工具名已带成员前缀（如 akasha_skill）时保持原样，不重复加前缀
        let url_a = spawn_sse_mock(&["akasha_catalog", "akasha_read"]).await;
        let server = group_server_with_urls(&[("akasha", &url_a)]);

        let tools = mcp_list_server_tools_runtime(&server)
            .await
            .expect("list group tools");
        let names = tools
            .iter()
            .map(|t| t.tool_name.as_str())
            .collect::<Vec<_>>();
        assert!(
            names.contains(&"akasha_catalog"),
            "已带前缀应保持, got {names:?}"
        );
        assert!(
            names.contains(&"akasha_read"),
            "已带前缀应保持, got {names:?}"
        );
        assert!(
            !names.contains(&"akasha_akasha_catalog"),
            "不应重复加前缀, got {names:?}"
        );
        let catalog = tools
            .iter()
            .find(|t| t.tool_name == "akasha_catalog")
            .expect("akasha_catalog 存在");
        assert_eq!(catalog.raw_tool_name, "akasha_catalog");
    }

    #[tokio::test]
    async fn sse_group_ambiguous_prefixed_name_kept_side_by_side() {
        // 成员名/工具名含下划线导致前缀歧义：a_b 成员的 c 工具 vs a 成员的 b_c 工具 → a_b_c 同名共存
        let url_a = spawn_sse_mock(&["c"]).await;
        let url_b = spawn_sse_mock(&["b_c"]).await;
        let server = group_server_with_urls(&[("a_b", &url_a), ("a", &url_b)]);

        let tools = mcp_list_server_tools_runtime(&server)
            .await
            .expect("同名共存不应报错");
        let a_b_c_count = tools
            .iter()
            .filter(|t| t.tool_name == "a_b_c")
            .count();
        assert_eq!(a_b_c_count, 2, "同名工具应共存, got {tools:?}");
    }

    #[test]
    fn validate_allows_cross_group_member_duplicate() {
        let result = mcp_validate_definition_inner(McpDefinitionValidateInput {
            definition_json: r#"{
                "mcpServers": {
                    "context7": { "command": "npx", "args": ["-y", "@upstash/context7-mcp"] }
                }
            }"#
            .to_string(),
            existing_member_names: vec!["context7".to_string(), "filesystem".to_string()],
        })
        .expect("validate ok");
        assert!(
            !result.issues.iter().any(|i| i.code == "duplicate_member_name"),
            "跨卡片成员重名不应再报 issue, got {:?}",
            result.issues
        );
    }
}

#[test]
fn test_mcp_oauth_parse_callback_query() {
    let req = "GET /callback?code=test_auth_code_123&state=xyz_state_456 HTTP/1.1";
    let Some(OAuthCallbackQuery::Code { code, state, issuer }) = parse_oauth_callback_query(req) else {
        panic!("should parse query");
    };
    assert_eq!(code, "test_auth_code_123");
    assert_eq!(state, "xyz_state_456");
    assert_eq!(issuer, None);

    let req_with_iss = "GET /callback?code=abc&state=def&iss=https%3A%2F%2Fclerk.context7.com HTTP/1.1";
    let Some(OAuthCallbackQuery::Code { code: code_iss, state: state_iss, issuer: issuer_iss }) = parse_oauth_callback_query(req_with_iss) else {
        panic!("should parse query with iss");
    };
    assert_eq!(code_iss, "abc");
    assert_eq!(state_iss, "def");
    assert_eq!(issuer_iss, Some("https://clerk.context7.com".to_string()));

    let req_encoded = "GET /callback?code=hello%20world&state=foo%2Bbar HTTP/1.1";
    let Some(OAuthCallbackQuery::Code { code: code2, state: state2, issuer: issuer2 }) = parse_oauth_callback_query(req_encoded) else {
        panic!("should parse encoded query");
    };
    assert_eq!(code2, "hello world");
    assert_eq!(state2, "foo+bar");
    assert_eq!(issuer2, None);

    let req_error = "GET /callback?error=access_denied&error_description=User%20denied&state=xyz HTTP/1.1";
    let Some(OAuthCallbackQuery::Error { error, error_description }) = parse_oauth_callback_query(req_error) else {
        panic!("should parse error query");
    };
    assert_eq!(error, "access_denied");
    assert_eq!(error_description, Some("User denied".to_string()));

    let req_no_code = "GET /callback?state=xyz HTTP/1.1";
    assert!(parse_oauth_callback_query(req_no_code).is_none());

    let req_favicon = "GET /favicon.ico HTTP/1.1";
    assert!(parse_oauth_callback_query(req_favicon).is_none());
}

#[test]
fn test_mcp_extract_scope_from_header() {
    let header = r#"Bearer error="insufficient_scope", scope="read write email""#;
    assert_eq!(mcp_extract_scope_from_header(header), Some("read write email".to_string()));

    let header_single = r#"Bearer scope="profile""#;
    assert_eq!(mcp_extract_scope_from_header(header_single), Some("profile".to_string()));

    let header_none = r#"Bearer error="invalid_token""#;
    assert_eq!(mcp_extract_scope_from_header(header_none), None);
}

#[test]
fn test_mcp_oauth_challenge_cache() {
    let test_server = "test_challenge_server_123";
    assert_eq!(mcp_oauth_get_cached_challenge(test_server), None);

    mcp_oauth_set_cached_challenge(test_server, "test_challenge_data");
    assert_eq!(
        mcp_oauth_get_cached_challenge(test_server),
        Some("test_challenge_data".to_string())
    );

    mcp_oauth_clear_cached_challenge(test_server);
    assert_eq!(mcp_oauth_get_cached_challenge(test_server), None);
}

#[tokio::test]
async fn test_mcp_file_credential_store() {
    let temp_dir = std::env::temp_dir().join(format!("pai_mcp_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let cred_path = temp_dir.join("test_cred.json");
    let store = McpFileCredentialStore::new(cred_path.clone());

    let loaded = store.load().await.expect("load ok");
    assert!(loaded.is_none());

    let test_creds = rmcp::transport::auth::StoredCredentials::new(
        "test_client_id".to_string(),
        None,
        vec!["read".to_string(), "write".to_string()],
        Some(1234567890),
    ).with_issuer(Some("https://auth.example.com".to_string()));
    store.save(test_creds).await.expect("save ok");
    assert!(cred_path.exists());

    let loaded2 = store.load().await.expect("load ok").expect("some creds");
    assert_eq!(loaded2.client_id, "test_client_id");
    assert_eq!(loaded2.granted_scopes, vec!["read".to_string(), "write".to_string()]);
    assert_eq!(loaded2.issuer, Some("https://auth.example.com".to_string()));

    store.clear().await.expect("clear ok");
    assert!(!cred_path.exists());
    let loaded3 = store.load().await.expect("load ok");
    assert!(loaded3.is_none());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_mcp_check_and_extract_auth_challenge() {
    let single_member_err = "MCP 组内 1 个成员连接失败: context7: auth_required: Bearer resource_metadata=\"https://mcp.context7.com/.well-known/oauth-protected-resource\"";
    assert_eq!(
        mcp_check_and_extract_auth_challenge(single_member_err),
        Some("Bearer resource_metadata=\"https://mcp.context7.com/.well-known/oauth-protected-resource\"".to_string())
    );
    assert_eq!(mcp_status_from_runtime_error(single_member_err), "auth_required");

    let direct_err = "auth_required: Bearer realm=\"pai\"";
    assert_eq!(
        mcp_check_and_extract_auth_challenge(direct_err),
        Some("Bearer realm=\"pai\"".to_string())
    );
    assert_eq!(mcp_status_from_runtime_error(direct_err), "auth_required");

    let multi_member_err = "MCP 组内 2 个成员连接失败: s1: auth_required: Bearer challenge_1 | s2: connect error";
    assert_eq!(
        mcp_check_and_extract_auth_challenge(multi_member_err),
        Some("Bearer challenge_1".to_string())
    );
    assert_eq!(mcp_status_from_runtime_error(multi_member_err), "auth_required");

    // 普通 401 文本没有 auth_required: 前缀（SSE transport、token 过期等），
    // 不含 challenge 参数，不应被识别为 OAuth 挑战。
    let status_401_err = "HTTP request failed with 401 Unauthorized";
    assert_eq!(mcp_check_and_extract_auth_challenge(status_401_err), None);
    assert_eq!(mcp_status_from_runtime_error(status_401_err), "failed");

    let timeout_err = "Connection timed out after 30 seconds";
    assert_eq!(mcp_check_and_extract_auth_challenge(timeout_err), None);
    assert_eq!(mcp_status_from_runtime_error(timeout_err), "timeout");

    let other_err = "failed to spawn process: file not found";
    assert_eq!(mcp_check_and_extract_auth_challenge(other_err), None);
    assert_eq!(mcp_status_from_runtime_error(other_err), "failed");
}


