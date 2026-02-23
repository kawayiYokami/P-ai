type DynamicMcpClient = rmcp::service::RunningService<rmcp::RoleClient, ()>;

#[derive(Clone)]
struct CustomStreamableHttpClient {
    client: reqwest::Client,
}

impl rmcp::transport::streamable_http_client::StreamableHttpClient for CustomStreamableHttpClient {
    type Error = reqwest::Error;

    async fn get_stream(
        &self,
        uri: std::sync::Arc<str>,
        session_id: std::sync::Arc<str>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
    ) -> Result<
        futures_util::stream::BoxStream<'static, Result<sse_stream::Sse, sse_stream::Error>>,
        rmcp::transport::streamable_http_client::StreamableHttpError<Self::Error>,
    > {
        let mut request_builder = self
            .client
            .get(uri.as_ref())
            .header(
                reqwest::header::ACCEPT,
                [
                    rmcp::transport::common::http_header::EVENT_STREAM_MIME_TYPE,
                    rmcp::transport::common::http_header::JSON_MIME_TYPE,
                ]
                .join(", "),
            )
            .header(
                rmcp::transport::common::http_header::HEADER_SESSION_ID,
                session_id.as_ref(),
            );

        if let Some(last_event_id) = last_event_id {
            request_builder = request_builder.header(
                rmcp::transport::common::http_header::HEADER_LAST_EVENT_ID,
                last_event_id,
            );
        }
        if let Some(token) = auth_header {
            request_builder = request_builder.bearer_auth(token);
        }

        let response = request_builder
            .send()
            .await
            .map_err(rmcp::transport::streamable_http_client::StreamableHttpError::Client)?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Err(
                rmcp::transport::streamable_http_client::StreamableHttpError::ServerDoesNotSupportSse,
            );
        }
        let response = response
            .error_for_status()
            .map_err(rmcp::transport::streamable_http_client::StreamableHttpError::Client)?;
        match response.headers().get(reqwest::header::CONTENT_TYPE) {
            Some(ct) => {
                if !ct
                    .as_bytes()
                    .starts_with(
                        rmcp::transport::common::http_header::EVENT_STREAM_MIME_TYPE.as_bytes(),
                    )
                    && !ct
                        .as_bytes()
                        .starts_with(rmcp::transport::common::http_header::JSON_MIME_TYPE.as_bytes())
                {
                    return Err(
                        rmcp::transport::streamable_http_client::StreamableHttpError::UnexpectedContentType(
                            Some(String::from_utf8_lossy(ct.as_bytes()).to_string()),
                        ),
                    );
                }
            }
            None => {
                return Err(
                    rmcp::transport::streamable_http_client::StreamableHttpError::UnexpectedContentType(
                        None,
                    ),
                );
            }
        }

        let event_stream =
            sse_stream::SseStream::from_byte_stream(response.bytes_stream()).boxed();
        Ok(event_stream)
    }

    async fn delete_session(
        &self,
        uri: std::sync::Arc<str>,
        session_id: std::sync::Arc<str>,
        auth_header: Option<String>,
    ) -> Result<(), rmcp::transport::streamable_http_client::StreamableHttpError<Self::Error>>
    {
        let mut request_builder = self.client.delete(uri.as_ref()).header(
            rmcp::transport::common::http_header::HEADER_SESSION_ID,
            session_id.as_ref(),
        );
        if let Some(token) = auth_header {
            request_builder = request_builder.bearer_auth(token);
        }
        let response = request_builder
            .send()
            .await
            .map_err(rmcp::transport::streamable_http_client::StreamableHttpError::Client)?;
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            return Ok(());
        }
        let _ = response
            .error_for_status()
            .map_err(rmcp::transport::streamable_http_client::StreamableHttpError::Client)?;
        Ok(())
    }

    async fn post_message(
        &self,
        uri: std::sync::Arc<str>,
        message: rmcp::model::ClientJsonRpcMessage,
        session_id: Option<std::sync::Arc<str>>,
        auth_header: Option<String>,
    ) -> Result<
        rmcp::transport::streamable_http_client::StreamableHttpPostResponse,
        rmcp::transport::streamable_http_client::StreamableHttpError<Self::Error>,
    > {
        let mut request = self
            .client
            .post(uri.as_ref())
            .header(
                reqwest::header::ACCEPT,
                [
                    rmcp::transport::common::http_header::EVENT_STREAM_MIME_TYPE,
                    rmcp::transport::common::http_header::JSON_MIME_TYPE,
                ]
                .join(", "),
            );
        if let Some(token) = auth_header {
            request = request.bearer_auth(token);
        }
        if let Some(session_id) = session_id {
            request = request.header(
                rmcp::transport::common::http_header::HEADER_SESSION_ID,
                session_id.as_ref(),
            );
        }
        let response = request
            .json(&message)
            .send()
            .await
            .map_err(rmcp::transport::streamable_http_client::StreamableHttpError::Client)?;
        let status = response.status();
        let response = response
            .error_for_status()
            .map_err(rmcp::transport::streamable_http_client::StreamableHttpError::Client)?;
        if matches!(
            status,
            reqwest::StatusCode::ACCEPTED | reqwest::StatusCode::NO_CONTENT
        ) {
            return Ok(
                rmcp::transport::streamable_http_client::StreamableHttpPostResponse::Accepted,
            );
        }
        let content_type = response.headers().get(reqwest::header::CONTENT_TYPE);
        let session_id = response
            .headers()
            .get(rmcp::transport::common::http_header::HEADER_SESSION_ID)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        match content_type {
            Some(ct)
                if ct
                    .as_bytes()
                    .starts_with(
                        rmcp::transport::common::http_header::EVENT_STREAM_MIME_TYPE.as_bytes(),
                    ) =>
            {
                let stream =
                    sse_stream::SseStream::from_byte_stream(response.bytes_stream()).boxed();
                Ok(
                    rmcp::transport::streamable_http_client::StreamableHttpPostResponse::Sse(
                        stream, session_id,
                    ),
                )
            }
            Some(ct)
                if ct
                    .as_bytes()
                    .starts_with(rmcp::transport::common::http_header::JSON_MIME_TYPE.as_bytes()) =>
            {
                let message = response
                    .json::<rmcp::model::ServerJsonRpcMessage>()
                    .await
                    .map_err(rmcp::transport::streamable_http_client::StreamableHttpError::Client)?;
                Ok(
                    rmcp::transport::streamable_http_client::StreamableHttpPostResponse::Json(
                        message, session_id,
                    ),
                )
            }
            _ => Err(
                rmcp::transport::streamable_http_client::StreamableHttpError::UnexpectedContentType(
                    content_type.map(|ct| String::from_utf8_lossy(ct.as_bytes()).to_string()),
                ),
            ),
        }
    }
}

fn mcp_policy_enabled_for_tool(server: &McpServerConfig, tool_name: &str) -> bool {
    server
        .tool_policies
        .iter()
        .find(|p| p.tool_name == tool_name)
        .map(|p| p.enabled)
        .unwrap_or(true)
}

fn mcp_definition_tool_filters(
    raw_definition_json: &str,
) -> (
    std::collections::HashSet<String>,
    std::collections::HashSet<String>,
) {
    let mut allow = std::collections::HashSet::<String>::new();
    let mut deny = std::collections::HashSet::<String>::new();
    if let Ok((_, root)) = parse_mcp_root_object(raw_definition_json) {
        for item in value_get_string_array(&root, "enabledTools") {
            allow.insert(item);
        }
        for item in value_get_string_array(&root, "disabledTools") {
            deny.insert(item);
        }
    }
    (allow, deny)
}

fn mcp_tool_allowed_by_definition(server: &McpServerConfig, tool_name: &str) -> bool {
    let (allow, deny) = mcp_definition_tool_filters(&server.definition_json);
    if deny.contains(tool_name) {
        return false;
    }
    if allow.is_empty() {
        return true;
    }
    allow.contains(tool_name)
}

fn mcp_connect_stdio_command(parsed: &ParsedMcpServerDefinition) -> Result<tokio::process::Command, String> {
    let command = parsed
        .command
        .as_deref()
        .ok_or_else(|| "stdio MCP command is missing".to_string())?;
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let trimmed = command.trim();
        let has_path_sep = trimmed.contains('\\') || trimmed.contains('/');
        let direct = std::path::PathBuf::from(trimmed);
        if has_path_sep || direct.extension().is_some() {
            let mut c = tokio::process::Command::new(trimmed);
            c.args(&parsed.args);
            c
        } else {
            let mut c = tokio::process::Command::new("cmd");
            c.arg("/C").arg(trimmed).args(&parsed.args);
            c
        }
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = tokio::process::Command::new(command);
        c.args(&parsed.args);
        c
    };

    if let Some(cwd) = &parsed.cwd {
        let path = std::path::PathBuf::from(cwd);
        if path.is_dir() {
            cmd.current_dir(path);
        }
    }
    if !parsed.env.is_empty() {
        cmd.envs(parsed.env.clone());
    }
    Ok(cmd)
}

async fn mcp_connect_client(parsed: &ParsedMcpServerDefinition) -> Result<DynamicMcpClient, String> {
    match parsed.transport {
        McpTransportKind::Stdio => {
            let cmd = mcp_connect_stdio_command(parsed)?;
            let transport = rmcp::transport::TokioChildProcess::new(cmd)
                .map_err(|err| format!("Start MCP stdio child process failed: {err}"))?;
            ().serve(transport)
                .await
                .map_err(|err| format!("Connect MCP stdio server failed: {err}"))
        }
        McpTransportKind::StreamableHttp => {
            let url = parsed
                .url
                .as_deref()
                .ok_or_else(|| "streamable HTTP MCP url is missing".to_string())?;
            let mut headers = reqwest::header::HeaderMap::new();
            for (k, v) in &parsed.http_headers {
                let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
                    .map_err(|err| format!("Invalid MCP http header name '{k}': {err}"))?;
                let value = reqwest::header::HeaderValue::from_str(v)
                    .map_err(|err| format!("Invalid MCP http header value for '{k}': {err}"))?;
                headers.insert(name, value);
            }
            for (k, env_var) in &parsed.env_http_headers {
                let env_name = env_var.trim();
                if env_name.is_empty() {
                    continue;
                }
                if let Ok(value_text) = std::env::var(env_name) {
                    let value_text = value_text.trim();
                    if value_text.is_empty() {
                        continue;
                    }
                    let name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
                        .map_err(|err| format!("Invalid MCP env_http_headers name '{k}': {err}"))?;
                    let value = reqwest::header::HeaderValue::from_str(value_text).map_err(|err| {
                        format!("Invalid MCP env_http_headers value for '{k}': {err}")
                    })?;
                    headers.insert(name, value);
                }
            }
            let mut client_builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));
            if !headers.is_empty() {
                client_builder = client_builder.default_headers(headers);
            }
            let custom_client = CustomStreamableHttpClient {
                client: client_builder
                    .build()
                    .map_err(|err| format!("Build MCP HTTP client failed: {err}"))?,
            };

            let mut config = rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::with_uri(url.to_string());
            if let Some(token_env) = &parsed.bearer_token_env_var {
                let env_name = token_env.trim();
                if !env_name.is_empty() {
                    if let Ok(token_value) = std::env::var(env_name) {
                        let token = token_value.trim();
                        if !token.is_empty() {
                            config = config.auth_header(token.to_string());
                        }
                    }
                }
            }

            let transport =
                rmcp::transport::StreamableHttpClientTransport::with_client(custom_client, config);
            ().serve(transport)
                .await
                .map_err(|err| format!("Connect MCP streamable HTTP server failed: {err}"))
        }
    }
}

async fn mcp_list_tools_with_client(
    parsed: &ParsedMcpServerDefinition,
) -> Result<(DynamicMcpClient, Vec<rmcp::model::Tool>), String> {
    let client = mcp_connect_client(parsed).await?;
    let tools = client
        .list_all_tools()
        .await
        .map_err(|err| format!("List MCP tools failed: {err}"))?;
    Ok((client, tools))
}

async fn mcp_list_server_tools_runtime(server: &McpServerConfig) -> Result<Vec<McpToolDescriptor>, String> {
    let parsed = parse_mcp_server_definition_from_config(server)?;
    let (_client, tools) = mcp_list_tools_with_client(&parsed).await?;

    let mut out = Vec::<McpToolDescriptor>::new();
    for def in tools {
        let name = def.name.to_string();
        let description = def.description.clone().unwrap_or_default().to_string();
        out.push(McpToolDescriptor {
            enabled: mcp_policy_enabled_for_tool(server, &name) && mcp_tool_allowed_by_definition(server, &name),
            tool_name: name,
            description,
        });
    }
    Ok(out)
}

async fn attach_enabled_mcp_tools_for_runtime(
    tools: &mut Vec<Box<dyn ToolDyn>>,
    app_state: Option<&AppState>,
) -> Result<Vec<DynamicMcpClient>, String> {
    let Some(state) = app_state else {
        return Ok(Vec::new());
    };
    let config = {
        let guard = state
            .state_lock
            .lock()
            .map_err(|err| format!("Failed to lock state mutex at {}:{} {}: {err}", file!(), line!(), module_path!()))?;
        let cfg = read_config(&state.config_path)?;
        drop(guard);
        cfg
    };

    let mut clients = Vec::<DynamicMcpClient>::new();
    for server in &config.mcp_servers {
        if !server.enabled {
            continue;
        }
        let parsed = match parse_mcp_server_definition_from_config(server) {
            Ok(v) => v,
            Err(err) => {
                eprintln!("[MCP] skip server={} parse failed: {err}", server.id);
                continue;
            }
        };

        let (client, defs) = match mcp_list_tools_with_client(&parsed).await {
            Ok(v) => v,
            Err(err) => {
                eprintln!("[MCP] skip server={} connect/list failed: {}", server.id, err);
                continue;
            }
        };
        let sink = client.peer().clone();
        for def in defs {
            let tool_name = def.name.to_string();
            if !mcp_policy_enabled_for_tool(server, &tool_name) {
                continue;
            }
            if !mcp_tool_allowed_by_definition(server, &tool_name) {
                continue;
            }
            tools.push(Box::new(rig::tool::rmcp::McpTool::from_mcp_server(
                def,
                sink.clone(),
            )));
        }
        clients.push(client);
    }

    Ok(clients)
}

