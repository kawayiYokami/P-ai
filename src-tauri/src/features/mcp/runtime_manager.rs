type DynamicMcpClient = rmcp::service::RunningService<rmcp::RoleClient, ()>;

include!("sse_client.rs");

const MCP_CONNECT_TIMEOUT_SECS: u64 = 30;
const MCP_REQUEST_TIMEOUT_SECS: u64 = 60;
const MCP_TOOL_CALL_TIMEOUT_SECS: u64 = 300;

struct McpConnectedClient {
    client: DynamicMcpClient,
    process_tree_guard: Option<McpProcessTreeGuard>,
}

struct CachedMcpClient {
    group_definition_json: String,
    definition_json: String,
    client: DynamicMcpClient,
    process_tree_guard: Option<McpProcessTreeGuard>,
}

// ========== 组内成员解析与工具名前缀 ==========

/// 解析卡片（组）内全部成员定义：返回 (成员名, 原始 JSON, 解析结果)
fn parse_mcp_group_definitions(
    server: &McpServerConfig,
) -> Result<Vec<(String, String, ParsedMcpServerDefinition)>, String> {
    let parsed = parse_mcp_definition_servers(&server.definition_json)
        .map_err(|err| err.message.clone())?;
    let mut out = Vec::<(String, String, ParsedMcpServerDefinition)>::new();
    for (name, obj) in parsed.servers {
        let raw = serde_json::to_string(&obj)
            .map_err(|err| format!("序列化 MCP 成员定义失败：{err}"))?;
        let parsed_def = parse_mcp_server_definition_from_value(&name, &obj)?;
        out.push((name, raw, parsed_def));
    }
    if out.is_empty() {
        return Err("MCP definition contains no servers".to_string());
    }
    Ok(out)
}

/// 组内成员工具名统一带规范化前缀：{成员名}_{工具名}
fn mcp_tool_prefixed_name(member_name: &str, tool_name: &str) -> String {
    format!("{}_{tool_name}", normalized_mcp_member_name_or_original(member_name))
}

fn mcp_member_name_compatibility_error(member_name: &str) -> Option<String> {
    normalize_mcp_member_name(member_name).is_none().then(|| {
        "MCP 组成员名规范化后没有可用字符，工具无法挂载".to_string()
    })
}

#[cfg(target_os = "windows")]
type McpProcessTreeGuard = WindowsJobGuard;

#[cfg(not(target_os = "windows"))]
struct McpProcessTreeGuard;

#[cfg(target_os = "windows")]
fn mcp_create_windows_job_kill_on_close(pid: u32) -> Result<McpProcessTreeGuard, String> {
    let job_guard = WindowsJobGuard::create_kill_on_close()?;
    job_guard.assign_process_id(pid)?;
    Ok(job_guard)
}

#[cfg(target_os = "windows")]
fn mcp_try_attach_windows_process_tree_guard(
    transport: &rmcp::transport::TokioChildProcess,
    parsed: &ParsedMcpServerDefinition,
) -> Option<McpProcessTreeGuard> {
    mcp_try_attach_windows_process_tree_guard_for_label(
        transport,
        parsed.command.as_deref().unwrap_or("<unknown>"),
    )
}

#[cfg(target_os = "windows")]
fn mcp_try_attach_windows_process_tree_guard_for_label(
    transport: &rmcp::transport::TokioChildProcess,
    command_label: &str,
) -> Option<McpProcessTreeGuard> {
    let Some(pid) = transport.id() else {
        runtime_log_warn(format!(
            "[MCP] Windows 进程树托管跳过：未取得子进程 pid，command={}",
            command_label
        ));
        return None;
    };

    match mcp_create_windows_job_kill_on_close(pid) {
        Ok(guard) => Some(guard),
        Err(err) => {
            runtime_log_error(format!(
                "[MCP] Windows 进程树托管失败：pid={}，command={}，error={}",
                pid, command_label, err
            ));
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn mcp_try_attach_windows_process_tree_guard(
    _transport: &rmcp::transport::TokioChildProcess,
    _parsed: &ParsedMcpServerDefinition,
) -> Option<McpProcessTreeGuard> {
    None
}

#[cfg(not(target_os = "windows"))]
fn mcp_try_attach_windows_process_tree_guard_for_label(
    _transport: &rmcp::transport::TokioChildProcess,
    _command_label: &str,
) -> Option<McpProcessTreeGuard> {
    None
}

#[derive(Clone)]
struct McpRuntimeTool {
    definition: rmcp::model::Tool,
    raw_tool_name: String,
    client: rmcp::service::Peer<rmcp::RoleClient>,
}

#[derive(Clone)]
struct CachedMcpRuntimeTool {
    app_state: AppState,
    server_id: String,
    executor_agent_id: String,
    runtime_tool_name: String,
    definition: rmcp::model::Tool,
}

fn provider_tool_result_from_mcp_call(
    tool_name: &str,
    result: rmcp::model::CallToolResult,
) -> ProviderToolResult {
    let mut parts = Vec::<ProviderToolResultPart>::new();
    let mut display_lines = Vec::<String>::new();
    let mut metadata = ProviderToolMetadata::default();

    if let Some(structured) = result.structured_content.as_ref() {
        let mut structured = structured.clone();
        if metadata.backup_record_id.is_none() {
            metadata.backup_record_id = value_string(&structured, "backupRecordId");
        }
        let payload = structured.get("data").unwrap_or(&structured);
        for image in extract_forward_images_from_value(payload) {
            parts.push(ProviderToolResultPart::Image {
                mime: image.mime,
                data_base64: image.base64,
                width: image.width,
                height: image.height,
            });
        }
        remove_inline_media_from_tool_value(&mut structured);
        display_lines.push(tool_value_readable_text(&structured));
    }

    for content in result.content {
        match content {
            rmcp::model::ContentBlock::Text(raw) => {
                if let Ok(mut value) = serde_json::from_str::<Value>(&raw.text) {
                    if metadata.backup_record_id.is_none() {
                        metadata.backup_record_id = value_string(&value, "backupRecordId");
                    }
                    let payload = value.get("data").unwrap_or(&value);
                    for image in extract_forward_images_from_value(payload) {
                        parts.push(ProviderToolResultPart::Image {
                            mime: image.mime,
                            data_base64: image.base64,
                            width: image.width,
                            height: image.height,
                        });
                    }
                    remove_inline_media_from_tool_value(&mut value);
                    let text = tool_value_readable_text(&value);
                    if !text.trim().is_empty() {
                        display_lines.push(text.clone());
                        parts.push(ProviderToolResultPart::Text { text });
                    }
                } else {
                    if !raw.text.trim().is_empty() {
                        display_lines.push(raw.text.clone());
                    }
                    parts.push(ProviderToolResultPart::Text { text: raw.text });
                }
            }
            rmcp::model::ContentBlock::Image(raw) => {
                display_lines.push(format!("[image:{}]", raw.mime_type));
                parts.push(ProviderToolResultPart::Image {
                    mime: raw.mime_type,
                    data_base64: raw.data,
                    width: 0,
                    height: 0,
                });
            }
            rmcp::model::ContentBlock::Audio(raw) => {
                display_lines.push(format!("[audio:{}]", raw.mime_type));
                parts.push(ProviderToolResultPart::Audio {
                    mime: raw.mime_type,
                    data_base64: raw.data,
                });
            }
            rmcp::model::ContentBlock::Resource(raw) => match raw.resource {
                rmcp::model::ResourceContents::TextResourceContents {
                    uri,
                    mime_type,
                    text,
                    ..
                } => {
                    if !text.trim().is_empty() {
                        display_lines.push(text.clone());
                    } else {
                        display_lines.push(format!("[resource:{uri}]"));
                    }
                    parts.push(ProviderToolResultPart::Resource {
                        mime: mime_type,
                        uri: Some(uri),
                        text,
                    });
                }
                rmcp::model::ResourceContents::BlobResourceContents {
                    uri,
                    mime_type,
                    blob,
                    ..
                } => {
                    display_lines.push(format!("[resource:{uri}]"));
                    parts.push(ProviderToolResultPart::Resource {
                        mime: mime_type,
                        uri: Some(uri),
                        text: blob,
                    });
                }
                _ => {
                    display_lines.push("[resource]".to_string());
                }
            },
            rmcp::model::ContentBlock::ResourceLink(raw) => {
                let text = raw
                    .description
                    .clone()
                    .or(raw.title.clone())
                    .unwrap_or_else(|| raw.name.clone());
                if !text.trim().is_empty() {
                    display_lines.push(text.clone());
                } else {
                    display_lines.push(format!("[resource_link:{}]", raw.uri));
                }
                parts.push(ProviderToolResultPart::Resource {
                    mime: raw.mime_type,
                    uri: Some(raw.uri),
                    text,
                });
            }
            _ => {}
        }
    }

    let output = if display_lines.is_empty() {
        format!("工具 `{tool_name}` 返回空结果。")
    } else {
        display_lines.join("\n")
    };

    ProviderToolResult {
        output,
        metadata,
        parts,
        is_error: result.is_error.unwrap_or(false),
    }
}

fn remove_inline_media_from_tool_value(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                remove_inline_media_from_tool_value(item);
            }
        }
        Value::Object(map) => {
            map.remove("imageBase64");
            map.remove("image_base64");
            if map.get("type").and_then(Value::as_str).is_some_and(|kind| kind.eq_ignore_ascii_case("image")) {
                map.remove("data");
            }
            for item in map.values_mut() {
                remove_inline_media_from_tool_value(item);
            }
        }
        _ => {}
    }
}

impl RuntimeToolDyn for McpRuntimeTool {
    fn name(&self) -> String {
        self.definition.name.to_string()
    }

    fn is_mcp_tool(&self) -> bool {
        true
    }

    fn call_json(&self, args_json: String) -> RuntimeToolCallFuture<'_> {
        let name = self.definition.name.clone();
        let raw_tool_name = self.raw_tool_name.clone();
        Box::pin(async move {
            let arguments = if args_json.trim().is_empty() {
                serde_json::Map::new()
            } else {
                serde_json::from_str::<serde_json::Map<String, Value>>(&args_json)
                    .map_err(|err| format!("Parse MCP tool args failed: {err}"))?
            };
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(MCP_TOOL_CALL_TIMEOUT_SECS),
                self.client
                    .call_tool(rmcp::model::CallToolRequestParams::new(raw_tool_name.clone()).with_arguments(arguments)),
            )
            .await
            .map_err(|_| {
                format!(
                    "Call MCP tool '{}' timed out after {}s",
                    name.as_ref(),
                    MCP_TOOL_CALL_TIMEOUT_SECS
                )
            })?
            .map_err(|err| format!("Call MCP tool '{}' failed: {err}", name.as_ref()))?;
            Ok(provider_tool_result_from_mcp_call(name.as_ref(), result))
        })
    }
}

impl RuntimeToolDyn for CachedMcpRuntimeTool {
    fn name(&self) -> String {
        self.definition.name.to_string()
    }

    fn is_mcp_tool(&self) -> bool {
        true
    }

    fn call_json(&self, args_json: String) -> RuntimeToolCallFuture<'_> {
        let app_state = self.app_state.clone();
        let server_id = self.server_id.clone();
        let executor_agent_id = self.executor_agent_id.clone();
        let runtime_tool_name = self.runtime_tool_name.clone();
        let definition = self.definition.clone();
        Box::pin(async move {
            let server = match load_server_by_id(&app_state, &server_id) {
                Ok(server) if server.enabled => server,
                Ok(_) => {
                    return Ok(ProviderToolResult::error(format!(
                        "MCP 工具 `{}` 当前不可用：服务器已停用",
                        definition.name.as_ref()
                    )))
                }
                Err(err) => {
                    return Ok(ProviderToolResult::error(format!(
                        "MCP 工具 `{}` 当前不可用：{err}",
                        definition.name.as_ref()
                    )))
                }
            };
            let tool_name = definition.name.to_string();
            let current_tool = list_tools_from_runtime(&server)
                .into_iter()
                .find(|tool| tool.tool_name == runtime_tool_name && tool.enabled);
            let Some(current_tool) = current_tool else {
                return Ok(ProviderToolResult::error(format!(
                    "MCP 工具 `{tool_name}` 当前不可用：工具已停用或运行态已失效"
                )));
            };
            let raw_tool_name = if current_tool.raw_tool_name.is_empty() {
                tool_name.clone()
            } else {
                current_tool.raw_tool_name.clone()
            };
            let agents = match state_read_agents_cached(&app_state) {
                Ok(agents) => agents,
                Err(err) => {
                    return Ok(ProviderToolResult::error(format!(
                        "MCP 工具 `{tool_name}` 当前不可用：读取最新人格权限失败，已安全跳过：{err}"
                    )))
                }
            };
            let Some(agent) = agents
                .iter()
                .find(|agent| agent.id.trim() == executor_agent_id.trim())
            else {
                return Ok(ProviderToolResult::error(format!(
                    "MCP 工具 `{tool_name}` 当前不可用：执行人格已不存在"
                )));
            };
            let qualified_by_name = format!("{}::{runtime_tool_name}", server.name);
            let qualified_by_id = format!("{}::{runtime_tool_name}", server.id);
            if !agent_permission_allows_any_name(
                Some(agent),
                AgentPermissionCategory::McpTool,
                &[
                    qualified_by_name.as_str(),
                    qualified_by_id.as_str(),
                    runtime_tool_name.as_str(),
                    tool_name.as_str(),
                ],
            ) {
                return Ok(ProviderToolResult::error(format!(
                    "MCP 工具 `{qualified_by_name}` 当前不可用：人格权限已撤销"
                )));
            }
            let (peer, _) = match mcp_get_or_connect_peer_for_tool(
                &server,
                &runtime_tool_name,
                &current_tool.member_name,
            )
            .await
            {
                Ok(result) => result,
                Err(err) => {
                    return Ok(ProviderToolResult::error(format!(
                        "MCP 工具 `{tool_name}` 暂时不可用，已跳过本次调用：{err}"
                    )))
                }
            };
            let tool = McpRuntimeTool {
                definition,
                raw_tool_name,
                client: peer,
            };
            match tool.call_json(args_json).await {
                Ok(result) => Ok(result),
                Err(err) => Ok(ProviderToolResult::error(format!(
                    "MCP 工具 `{tool_name}` 执行失败，聊天将继续：{err}"
                ))),
            }
        })
    }
}

fn mcp_client_cache(
) -> &'static tokio::sync::Mutex<
    std::collections::HashMap<String, std::collections::HashMap<String, CachedMcpClient>>,
> {
    static CACHE: OnceLock<
        tokio::sync::Mutex<
            std::collections::HashMap<
                String,
                std::collections::HashMap<String, CachedMcpClient>,
            >,
        >,
    > = OnceLock::new();
    CACHE.get_or_init(|| {
        tokio::sync::Mutex::new(std::collections::HashMap::new())
    })
}

fn mcp_client_connect_locks(
) -> &'static tokio::sync::Mutex<
    std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>,
> {
    static LOCKS: OnceLock<
        tokio::sync::Mutex<
            std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>,
        >,
    > = OnceLock::new();
    LOCKS.get_or_init(|| tokio::sync::Mutex::new(std::collections::HashMap::new()))
}

async fn mcp_client_connect_lock_for_server(
    server_id: &str,
) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    let mut locks = mcp_client_connect_locks().lock().await;
    locks
        .entry(server_id.to_string())
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

#[derive(Debug, Clone)]
struct McpRuntimeState {
    deployed: bool,
    last_status: String,
    last_error: String,
    updated_at: String,
    tools: Vec<McpToolDescriptor>,
}

fn mcp_runtime_state_store() -> &'static Mutex<std::collections::HashMap<String, McpRuntimeState>> {
    static STORE: OnceLock<Mutex<std::collections::HashMap<String, McpRuntimeState>>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn mcp_runtime_state_get(server_id: &str) -> Option<McpRuntimeState> {
    let Ok(guard) = mcp_runtime_state_store().lock() else {
        return None;
    };
    guard.get(server_id).cloned()
}

fn mcp_runtime_state_set(
    server_id: &str,
    deployed: bool,
    last_status: &str,
    last_error: &str,
    tools: Vec<McpToolDescriptor>,
) {
    let mut guard = match mcp_runtime_state_store().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            runtime_log_info(format!(
                "[MCP] 设置 MCP 运行状态时锁中毒 for server_id={}: {}",
                server_id, poisoned
            ));
            poisoned.into_inner()
        }
    };
    guard.insert(
        server_id.to_string(),
        McpRuntimeState {
            deployed,
            last_status: last_status.to_string(),
            last_error: last_error.to_string(),
            updated_at: now_iso(),
            tools,
        },
    );
}

fn mcp_runtime_state_remove(server_id: &str) {
    let mut guard = match mcp_runtime_state_store().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            runtime_log_info(format!(
                "[MCP] 移除 MCP 运行状态时锁中毒 for server_id={}: {}",
                server_id, poisoned
            ));
            poisoned.into_inner()
        }
    };
    guard.remove(server_id);
}

fn mcp_runtime_state_update<F>(server_id: &str, update: F)
where
    F: FnOnce(&mut McpRuntimeState),
{
    let mut guard = match mcp_runtime_state_store().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            runtime_log_info(format!(
                "[MCP] 更新 MCP 运行状态时锁中毒 for server_id={}: {}",
                server_id, poisoned
            ));
            poisoned.into_inner()
        }
    };
    if let Some(runtime) = guard.get_mut(server_id) {
        update(runtime);
        runtime.updated_at = now_iso();
    }
}

fn mcp_runtime_state_set_tool_enabled(server_id: &str, tool_name: &str, enabled: bool) {
    mcp_runtime_state_update(server_id, |runtime| {
        for tool in &mut runtime.tools {
            if tool.tool_name == tool_name {
                tool.enabled = enabled;
            }
        }
    });
}

#[derive(Clone)]
struct CustomStreamableHttpClient {
    client: reqwest::Client,
}

fn custom_streamable_http_apply_headers(
    mut request_builder: reqwest::RequestBuilder,
    custom_headers: std::collections::HashMap<tauri::http::HeaderName, tauri::http::HeaderValue>,
) -> reqwest::RequestBuilder {
    for (name, value) in custom_headers {
        request_builder = request_builder.header(name, value);
    }
    request_builder
}

impl rmcp::transport::streamable_http_client::StreamableHttpClient for CustomStreamableHttpClient {
    type Error = reqwest::Error;

    async fn get_stream(
        &self,
        uri: std::sync::Arc<str>,
        session_id: Option<std::sync::Arc<str>>,
        last_event_id: Option<String>,
        auth_header: Option<String>,
        custom_headers: std::collections::HashMap<tauri::http::HeaderName, tauri::http::HeaderValue>,
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
            );
        if let Some(session_id) = session_id {
            request_builder = request_builder.header(
                rmcp::transport::common::http_header::HEADER_SESSION_ID,
                session_id.as_ref(),
            );
        }

        if let Some(last_event_id) = last_event_id {
            request_builder = request_builder.header(
                rmcp::transport::common::http_header::HEADER_LAST_EVENT_ID,
                last_event_id,
            );
        }
        if let Some(token) = auth_header {
            request_builder = request_builder.bearer_auth(token);
        }
        request_builder = custom_streamable_http_apply_headers(request_builder, custom_headers);

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
            sse_stream::SseStream::from_bytes_stream(response.bytes_stream()).boxed();
        Ok(event_stream)
    }

    async fn delete_session(
        &self,
        uri: std::sync::Arc<str>,
        session_id: std::sync::Arc<str>,
        auth_header: Option<String>,
        custom_headers: std::collections::HashMap<tauri::http::HeaderName, tauri::http::HeaderValue>,
    ) -> Result<(), rmcp::transport::streamable_http_client::StreamableHttpError<Self::Error>>
    {
        let mut request_builder = self.client.delete(uri.as_ref()).header(
            rmcp::transport::common::http_header::HEADER_SESSION_ID,
            session_id.as_ref(),
        );
        if let Some(token) = auth_header {
            request_builder = request_builder.bearer_auth(token);
        }
        request_builder = custom_streamable_http_apply_headers(request_builder, custom_headers);
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
        headers: std::collections::HashMap<tauri::http::HeaderName, tauri::http::HeaderValue>,
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
        request = custom_streamable_http_apply_headers(request, headers);
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
                    sse_stream::SseStream::from_bytes_stream(response.bytes_stream()).boxed();
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
    if let Ok(parsed) = parse_mcp_definition_servers(raw_definition_json) {
        for (member_name, obj) in parsed.servers {
            let member_prefix = format!("{}_", normalized_mcp_member_name_or_original(&member_name));
            for item in value_get_string_array(&obj, "enabledTools") {
                // 与探测别名规则一致：已带成员前缀保持，否则补前缀
                allow.insert(if item.starts_with(&member_prefix) {
                    item
                } else {
                    mcp_tool_prefixed_name(&member_name, &item)
                });
            }
            for item in value_get_string_array(&obj, "disabledTools") {
                deny.insert(if item.starts_with(&member_prefix) {
                    item
                } else {
                    mcp_tool_prefixed_name(&member_name, &item)
                });
            }
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
            c.arg("/D")
                .arg("/S")
                .arg("/C")
                .arg(format!("chcp 65001 >nul 2>&1 & {trimmed}"))
                .args(&parsed.args);
            c
        }
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = tokio::process::Command::new(command);
        c.args(&parsed.args);
        c
    };
    #[cfg(target_os = "windows")]
    {
        // 0x08000000 = CREATE_NO_WINDOW, keep MCP child processes headless.
        cmd.creation_flags(0x08000000);
    }

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

async fn mcp_connect_client(parsed: &ParsedMcpServerDefinition) -> Result<McpConnectedClient, String> {
    match parsed.transport {
        McpTransportKind::Stdio => {
            let cmd = mcp_connect_stdio_command(parsed)?;
            let (transport, stderr_opt) = rmcp::transport::TokioChildProcess::builder(cmd)
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|err| format!("Start MCP stdio child process failed: {err}"))?;
            let process_tree_guard = mcp_try_attach_windows_process_tree_guard(&transport, parsed);

            let stderr_cache = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::<u8>::new()));
            if let Some(mut stderr_pipe) = stderr_opt {
                let cache = stderr_cache.clone();
                tokio::spawn(async move {
                    const STDERR_MAX_BYTES: usize = 4096;
                    let mut chunk = [0u8; 1024];
                    loop {
                        let read = tokio::io::AsyncReadExt::read(&mut stderr_pipe, &mut chunk).await;
                        let Ok(n) = read else {
                            break;
                        };
                        if n == 0 {
                            break;
                        }
                        let mut guard = cache.lock().await;
                        guard.extend_from_slice(&chunk[..n]);
                        if guard.len() > STDERR_MAX_BYTES {
                            let drain = guard.len().saturating_sub(STDERR_MAX_BYTES);
                            guard.drain(0..drain);
                        }
                    }
                });
            }

            match ().serve(transport).await {
                Ok(client) => Ok(McpConnectedClient {
                    client,
                    process_tree_guard,
                }),
                Err(err) => {
                    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                    let stderr_text = {
                        let guard = stderr_cache.lock().await;
                        let text = String::from_utf8_lossy(&guard).into_owned();
                        text.trim().replace('\r', "")
                    };
                    if stderr_text.is_empty() {
                        Err(format!("Connect MCP stdio server failed: {err}"))
                    } else {
                        Err(format!(
                            "Connect MCP stdio server failed: {err} | child stderr: {}",
                            stderr_text
                        ))
                    }
                }
            }
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
            let mut client_builder =
                reqwest::Client::builder().timeout(std::time::Duration::from_secs(MCP_REQUEST_TIMEOUT_SECS));
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
                .map(|client| McpConnectedClient {
                    client,
                    process_tree_guard: None,
                })
                .map_err(|err| format!("Connect MCP streamable HTTP server failed: {err}"))
        }
        McpTransportKind::Sse => {
            let (sink, stream) = connect_sse_transport(parsed).await?;
            ().serve((sink, stream))
                .await
                .map(|client| McpConnectedClient {
                    client,
                    process_tree_guard: None,
                })
                .map_err(|err| format!("Connect MCP SSE server failed: {err}"))
        }
    }
}

/// 检查卡片（组）所有成员是否都已连接且 definition 未变化
fn mcp_group_cache_fully_hit(
    cache_guard: &std::collections::HashMap<String, std::collections::HashMap<String, CachedMcpClient>>,
    server_id: &str,
    members: &[(String, String, ParsedMcpServerDefinition)],
) -> bool {
    let Some(member_cache) = cache_guard.get(server_id) else {
        return false;
    };
    if member_cache.len() != members.len() {
        return false;
    }
    members.iter().all(|(name, raw, _)| {
        member_cache
            .get(name)
            .map(|c| c.definition_json == *raw)
            .unwrap_or(false)
    })
}

async fn mcp_connect_single_member(
    server: &McpServerConfig,
    member_name: &str,
    parsed: &ParsedMcpServerDefinition,
    raw_definition: &str,
) -> Result<(), String> {
    let connected = tokio::time::timeout(
        std::time::Duration::from_secs(MCP_CONNECT_TIMEOUT_SECS),
        mcp_connect_client(parsed),
    )
    .await
    .map_err(|_| {
        format!(
            "Connect MCP member '{member_name}' timed out after {}s",
            MCP_CONNECT_TIMEOUT_SECS
        )
    })??;
    let mut old_cached: Option<CachedMcpClient> = None;

    let cache = mcp_client_cache();
    let mut guard = cache.lock().await;
    let member_map = guard
        .entry(server.id.clone())
        .or_insert_with(std::collections::HashMap::new);
    if let Some(old) = member_map.insert(
        member_name.to_string(),
        CachedMcpClient {
            group_definition_json: server.definition_json.clone(),
            definition_json: raw_definition.to_string(),
            client: connected.client,
            process_tree_guard: connected.process_tree_guard,
        },
    ) {
        old_cached = Some(old);
    }
    drop(guard);
    if let Some(old) = old_cached {
        let CachedMcpClient {
            client,
            process_tree_guard,
            ..
        } = old;
        let _ = client.cancel().await;
        drop(process_tree_guard);
    }
    Ok(())
}

async fn mcp_get_or_connect_client(server: &McpServerConfig) -> Result<(), String> {
    let members = parse_mcp_group_definitions(server)?;
    {
        let cache = mcp_client_cache();
        let guard = cache.lock().await;
        if mcp_group_cache_fully_hit(&guard, &server.id, &members) {
            return Ok(());
        }
    }

    let connect_lock = mcp_client_connect_lock_for_server(&server.id).await;
    let _connect_guard = connect_lock.lock().await;

    {
        let cache = mcp_client_cache();
        let guard = cache.lock().await;
        if mcp_group_cache_fully_hit(&guard, &server.id, &members) {
            return Ok(());
        }
    }

    let mut failures = Vec::<String>::new();
    for (member_name, raw, parsed) in &members {
        let cached_ok = {
            let cache = mcp_client_cache();
            let guard = cache.lock().await;
            guard
                .get(&server.id)
                .and_then(|member_map| member_map.get(member_name))
                .map(|c| c.definition_json == *raw)
                .unwrap_or(false)
        };
        if cached_ok {
            continue;
        }
        if let Err(err) = mcp_connect_single_member(server, member_name, parsed, raw).await {
            failures.push(format!("{member_name}: {err}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "MCP 组内 {} 个成员连接失败: {}",
            failures.len(),
            failures.join(" | ")
        ))
    }
}

async fn mcp_disconnect_cached_client(server_id: &str) {
    let mut old_cached = Vec::<CachedMcpClient>::new();
    let cache = mcp_client_cache();
    let mut guard = cache.lock().await;
    if let Some(member_map) = guard.remove(server_id) {
        old_cached = member_map.into_values().collect();
    }
    drop(guard);
    for old in old_cached {
        let CachedMcpClient {
            client,
            process_tree_guard,
            ..
        } = old;
        let _ = client.cancel().await;
        drop(process_tree_guard);
    }
}

async fn mcp_disconnect_cached_client_if_definition(server_id: &str, definition_json: &str) {
    let mut old_cached = Vec::<CachedMcpClient>::new();
    let cache = mcp_client_cache();
    let mut guard = cache.lock().await;
    let definition_changed = guard
        .get(server_id)
        .map(|members| {
            members
                .values()
                .any(|c| c.group_definition_json != definition_json)
        })
        .unwrap_or(false);
    // 整组 definition 变化时全部断开重连
    if definition_changed {
        if let Some(members) = guard.remove(server_id) {
            old_cached = members.into_values().collect();
        }
    }
    drop(guard);
    for old in old_cached {
        let CachedMcpClient {
            client,
            process_tree_guard,
            ..
        } = old;
        let _ = client.cancel().await;
        drop(process_tree_guard);
    }
}

async fn mcp_list_tools_with_peer(
    server: &McpServerConfig,
    member_name: &str,
) -> Result<(rmcp::service::Peer<rmcp::RoleClient>, Vec<rmcp::model::Tool>), String> {
    mcp_get_or_connect_client(server).await?;
    let peer = {
        let cache = mcp_client_cache();
        let guard = cache.lock().await;
        let cached = guard
            .get(&server.id)
            .and_then(|member_map| member_map.get(member_name))
            .ok_or_else(|| {
                format!(
                    "MCP runtime cache missing member '{member_name}' of server '{}'",
                    server.id
                )
            })?;
        cached.client.peer().clone()
    };
    let tools = tokio::time::timeout(
        std::time::Duration::from_secs(MCP_REQUEST_TIMEOUT_SECS),
        peer.list_all_tools(),
    )
    .await
    .map_err(|_| {
        format!(
            "List MCP tools timed out after {}s for member '{member_name}' of server '{}'",
            MCP_REQUEST_TIMEOUT_SECS, server.id
        )
    })?
    .map_err(|err| format!("List MCP tools failed: {err}"))?;
    Ok((peer, tools))
}

async fn mcp_get_or_connect_peer_for_tool(
    server: &McpServerConfig,
    tool_name: &str,
    member_name: &str,
) -> Result<(rmcp::service::Peer<rmcp::RoleClient>, String), String> {
    if !mcp_policy_enabled_for_tool(server, tool_name) || !mcp_tool_allowed_by_definition(server, tool_name) {
        return Err(format!(
            "MCP tool '{}' is disabled by policy for server '{}'",
            tool_name, server.id
        ));
    }
    // 成员归属来自运行时实例的 member_name（探测时记录），不再靠工具名前缀反查
    mcp_get_or_connect_client(server).await?;
    let cache = mcp_client_cache();
    let guard = cache.lock().await;
    let cached = guard
        .get(&server.id)
        .and_then(|member_map| member_map.get(member_name))
        .ok_or_else(|| {
            format!(
                "MCP runtime cache missing member '{member_name}' of server '{}'",
                server.id
            )
        })?;
    Ok((cached.client.peer().clone(), member_name.to_string()))
}

async fn mcp_list_server_tools_runtime(server: &McpServerConfig) -> Result<Vec<McpToolDescriptor>, String> {
    let members = parse_mcp_group_definitions(server)?;
    let mut out = Vec::<McpToolDescriptor>::new();
    for (member_name, _, _) in &members {
        let member_compatibility_error = mcp_member_name_compatibility_error(member_name);
        let (_peer, tools) = mcp_list_tools_with_peer(server, member_name).await?;
        for def in tools {
            let raw_name = def.name.to_string();
            // 原始名已带本成员规范化前缀（如 akasha_skill）则保持，避免 akasha_akasha_skill；
            // 不带（如裸名 search）才补 {成员}_{工具} 前缀
            let member_prefix = format!("{}_", normalized_mcp_member_name_or_original(member_name));
            let tool_name = if raw_name.starts_with(&member_prefix) {
                raw_name.clone()
            } else {
                mcp_tool_prefixed_name(member_name, &raw_name)
            };
            let description = def.description.clone().unwrap_or_default().to_string();
            out.push(McpToolDescriptor {
                enabled: mcp_policy_enabled_for_tool(server, &tool_name)
                    && mcp_tool_allowed_by_definition(server, &tool_name),
                tool_name,
                description,
                compatibility_error: member_compatibility_error.clone(),
                member_name: member_name.clone(),
                raw_tool_name: raw_name,
                parameters: serde_json::Value::Object(def.input_schema.as_ref().clone()),
            });
        }
    }
    Ok(out)
}
