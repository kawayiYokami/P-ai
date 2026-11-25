fn ide_context_bridge_url_for_host(host: &str, port: u16) -> String {
    format!("ws://{}:{}{}", host, port, IDE_CONTEXT_BRIDGE_PATH)
}

fn ide_context_chat_bridge_url_for_host(host: &str, port: u16) -> String {
    format!("ws://{}:{}{}", host, port, IDE_CONTEXT_CHAT_BRIDGE_PATH)
}

fn ide_context_bridge_url(port: u16) -> String {
    ide_context_bridge_url_for_host(IDE_CONTEXT_BRIDGE_HOST, port)
}

fn ide_context_chat_bridge_url(port: u16) -> String {
    ide_context_chat_bridge_url_for_host(IDE_CONTEXT_BRIDGE_HOST, port)
}

fn ide_context_sidebar_url_for_host(host: &str, port: u16) -> String {
    format!("http://{}:{}/sidebar", host, port)
}

fn ide_context_bridge_discovery_path() -> std::path::PathBuf {
    std::env::temp_dir().join(IDE_CONTEXT_BRIDGE_DISCOVERY_FILE)
}

fn ide_context_bridge_shutdown_slot() -> Arc<Mutex<Option<tokio_util::sync::CancellationToken>>> {
    IDE_CONTEXT_BRIDGE_SHUTDOWN
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone()
}

fn ide_context_bridge_create_shutdown_token() -> tokio_util::sync::CancellationToken {
    let token = tokio_util::sync::CancellationToken::new();
    if let Ok(mut slot) = ide_context_bridge_shutdown_slot().lock() {
        *slot = Some(token.clone());
    }
    token
}

fn ide_context_bridge_server_task_slot() -> Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>> {
    IDE_CONTEXT_BRIDGE_SERVER_TASK
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone()
}

fn ide_context_bridge_set_server_task(handle: tauri::async_runtime::JoinHandle<()>) {
    if let Ok(mut slot) = ide_context_bridge_server_task_slot().lock() {
        *slot = Some(handle);
    }
}

fn ide_context_port_service_core() -> Arc<LocalPortServiceCore> {
    IDE_CONTEXT_PORT_SERVICE_CORE
        .get_or_init(|| Arc::new(LocalPortServiceCore::new()))
        .clone()
}

fn ide_context_bridge_server_task_is_running() -> bool {
    IDE_CONTEXT_BRIDGE_STARTED.load(Ordering::SeqCst)
}

fn ide_context_bridge_take_server_task() -> Option<tauri::async_runtime::JoinHandle<()>> {
    ide_context_bridge_server_task_slot()
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

fn publish_ide_context_bridge_discovery(port: u16, remote_password: &str) -> Result<(), String> {
    let url = ide_context_bridge_url(port);
    let chat_url = ide_context_chat_bridge_url(port);
    let payload = IdeContextBridgeDiscovery {
        url: url.clone(),
        bridge_url: url,
        chat_url,
        host: IDE_CONTEXT_BRIDGE_HOST.to_string(),
        bind_host: IDE_CONTEXT_BRIDGE_BIND_HOST.to_string(),
        port,
        path: IDE_CONTEXT_BRIDGE_PATH.to_string(),
        chat_path: IDE_CONTEXT_CHAT_BRIDGE_PATH.to_string(),
        pid: std::process::id(),
        updated_at: now_iso(),
        token: String::new(),
        remote_password: remote_password.to_string(),
    };
    let path = ide_context_bridge_discovery_path();
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("Serialize IDE context bridge discovery failed: {err}"))?;
    fs::write(&path, text).map_err(|err| {
        format!(
            "Write IDE context bridge discovery failed ({}): {err}",
            path.display()
        )
    })?;
    Ok(())
}

fn clear_ide_context_bridge_discovery() {
    let path = ide_context_bridge_discovery_path();
    if path.exists() {
        let _ = fs::remove_file(path);
    }
}

async fn prepare_ide_context_bridge_server_start(
    state: &AppState,
    ide_context_runtime: &IdeContextRuntime,
    port_service: &Arc<LocalPortServiceCore>,
) -> Option<(tokio::net::TcpListener, u16, String)> {
    let config = match state_read_config_cached(state) {
        Ok(config) => config,
        Err(err) => {
            runtime_log_error(format!(
                "[系统] 读取配置失败，使用默认端口: {}",
                err
            ));
            AppConfig::default()
        }
    };
    if !config.web_access_enabled {
        IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);
        ide_context_set_current_port(ide_context_runtime, None);
        activation_api_clear_port();
        clear_ide_context_bridge_discovery();
        port_service
            .set_status_text(WEB_ACCESS_SERVICE_ID, Some("disabled".to_string()))
            .await;
        port_service
            .set_listen_addr(WEB_ACCESS_SERVICE_ID, None)
            .await;
        runtime_log_info(format!("[系统] 跳过启动：网络访问已关闭"));
        return None;
    }
    port_service
        .set_status_text(WEB_ACCESS_SERVICE_ID, Some("binding".to_string()))
        .await;
    port_service.set_last_error(WEB_ACCESS_SERVICE_ID, None).await;
    let preferred_port = normalize_web_access_port(config.web_access_port);
    let (listener, port) = match bind_ide_context_bridge_listener(preferred_port).await {
        Ok(result) => result,
        Err(err) => {
            IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);
            ide_context_set_current_port(ide_context_runtime, None);
            activation_api_clear_port();
            // 端口被占用说明可能有另一个实例正在监听并维护 discovery 文件，
            // 此处不能清掉它，否则 VSCode 侧边栏会丢失后端发现信息。
            port_service
                .set_status_text(WEB_ACCESS_SERVICE_ID, Some("bind_failed".to_string()))
                .await;
            port_service
                .set_last_error(WEB_ACCESS_SERVICE_ID, Some(err.clone()))
                .await;
            runtime_log_error(format!("[系统] 监听失败: {}", err));
            return None;
        }
    };
    ide_context_set_current_port(ide_context_runtime, Some(port));
    activation_api_set_port(port);
    let bridge_url = ide_context_bridge_url(port);
    port_service
        .set_listen_addr(WEB_ACCESS_SERVICE_ID, Some(format!("{}:{}", IDE_CONTEXT_BRIDGE_BIND_HOST, port)))
        .await;
    let remote_password = match ide_context_effective_remote_password(state, ide_context_runtime) {
        Ok(password) => password,
        Err(err) => {
            IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);
            ide_context_set_current_port(ide_context_runtime, None);
            activation_api_clear_port();
            clear_ide_context_bridge_discovery();
            port_service
                .set_listen_addr(WEB_ACCESS_SERVICE_ID, None)
                .await;
            port_service
                .set_status_text(WEB_ACCESS_SERVICE_ID, Some("error".to_string()))
                .await;
            port_service
                .set_last_error(WEB_ACCESS_SERVICE_ID, Some(err.clone()))
                .await;
            runtime_log_error(format!("[系统] 初始化远程访问密码失败 错误={}", err));
            return None;
        }
    };
    if let Err(err) = publish_ide_context_bridge_discovery(port, &remote_password) {
        IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);
        ide_context_set_current_port(ide_context_runtime, None);
        activation_api_clear_port();
        clear_ide_context_bridge_discovery();
        port_service
            .set_listen_addr(WEB_ACCESS_SERVICE_ID, None)
            .await;
        port_service
            .set_status_text(WEB_ACCESS_SERVICE_ID, Some("error".to_string()))
            .await;
        port_service
            .set_last_error(WEB_ACCESS_SERVICE_ID, Some(err.clone()))
            .await;
        runtime_log_error(format!("[系统] 写入发现文件失败 错误={}", err));
        return None;
    }
    port_service
        .set_status_text(WEB_ACCESS_SERVICE_ID, Some("listening".to_string()))
        .await;
    port_service.set_last_error(WEB_ACCESS_SERVICE_ID, None).await;
    port_service
        .add_log(
            WEB_ACCESS_SERVICE_ID,
            "info",
            &format!("服务启动，监听 {}", bridge_url),
        )
        .await;
    runtime_log_info(format!("[系统] 已监听 {}", bridge_url));
    Some((listener, port, bridge_url))
}

fn spawn_ide_context_bridge_server_task(
    app: AppHandle,
    state: AppState,
    ide_context_runtime: IdeContextRuntime,
    port_service: Arc<LocalPortServiceCore>,
    shutdown_token: tokio_util::sync::CancellationToken,
    listener: tokio::net::TcpListener,
    port: u16,
    bridge_url: String,
) {
    let server_task = tauri::async_runtime::spawn(async move {
        loop {
            let (stream, peer_addr) = tokio::select! {
                _ = shutdown_token.cancelled() => {
                    clear_ide_context_bridge_discovery();
                    IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);
                    ide_context_set_current_port(&ide_context_runtime, None);
                    activation_api_clear_port();
                    port_service
                        .set_status_text(WEB_ACCESS_SERVICE_ID, Some("stopped".to_string()))
                        .await;
                    port_service
                        .set_listen_addr(WEB_ACCESS_SERVICE_ID, None)
                        .await;
                    if let Ok(mut slot) = ide_context_bridge_shutdown_slot().lock() {
                        slot.take();
                    }
                    runtime_log_info(format!("[系统] 收到停机信号，停止监听 {}", bridge_url));
                    break;
                }
                result = listener.accept() => match result {
                    Ok(result) => result,
                    Err(err) => {
                        runtime_log_error(format!("[系统] 接收连接失败: {}", err));
                        continue;
                    }
                },
            };
            let state_clone = state.clone();
            let app_clone = app.clone();
            let ide_context_runtime_clone = ide_context_runtime.clone();
            tauri::async_runtime::spawn(async move {
                ide_context_ws_handle_connection(
                    stream,
                    peer_addr,
                    port,
                    app_clone,
                    state_clone,
                    ide_context_runtime_clone,
                )
                .await;
            });
        }
    });
    ide_context_bridge_set_server_task(server_task);
}

async fn bind_ide_context_bridge_listener(
    preferred_port: u16,
) -> Result<(tokio::net::TcpListener, u16), String> {
    let port = normalize_web_access_port(preferred_port);
    let addr = format!("{}:{}", IDE_CONTEXT_BRIDGE_BIND_HOST, port);
    match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => Ok((listener, port)),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::AddrInUse {
                runtime_log_info(format!("[系统] 固定端口已占用，无法启动: {}", addr));
                Err(format!("固定端口 {} 已被占用，请释放后重试", port))
            } else {
                runtime_log_error(format!("[系统] 固定端口监听失败 {}: {}", addr, err));
                Err(format!("固定端口 {} 监听失败: {}", port, err))
            }
        }
    }
}

async fn ide_context_stream_is_websocket(stream: &tokio::net::TcpStream) -> bool {
    let mut buffer = [0_u8; 1024];
    match tokio::time::timeout(std::time::Duration::from_millis(500), stream.peek(&mut buffer)).await
    {
        Ok(Ok(count)) if count > 0 => {
            String::from_utf8_lossy(&buffer[..count])
                .to_ascii_lowercase()
                .contains("upgrade: websocket")
        }
        _ => false,
    }
}

fn ide_context_http_status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        426 => "Upgrade Required",
        _ => "Internal Server Error",
    }
}

fn ide_context_http_header_value<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{}:", name.to_ascii_lowercase());
    headers.lines().skip(1).find_map(|line| {
        let trimmed = line.trim();
        if trimmed.to_ascii_lowercase().starts_with(&prefix) {
            trimmed.split_once(':').map(|(_, value)| value.trim())
        } else {
            None
        }
    })
}

fn ide_context_http_path_from_request(headers: &str) -> (&str, &str) {
    let first_line = headers.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let uri = parts.next().unwrap_or("/");
    let path = uri.split('?').next().unwrap_or("/");
    (method, path)
}

fn ide_context_web_asset_path(path: &str) -> Option<String> {
    let path = path.trim();
    match path {
        "/" | "/sidebar" | "/sidebar.html" => Some("sidebar.html".to_string()),
        "/settings" | "/settings.html" => Some("settings.html".to_string()),
        _ if path.starts_with("/assets/") && !path.contains("..") => {
            Some(path.trim_start_matches('/').to_string())
        }
        _ => None,
    }
}

fn ide_context_web_icon_bytes(path: &str) -> Option<&'static [u8]> {
    match path.trim() {
        "/favicon.ico" | "/favicon.png" => {
            Some(include_bytes!("../../../../../icons/32x32.png").as_slice())
        }
        _ => None,
    }
}

fn ide_context_web_html_with_bridge(asset_bytes: &[u8], host: &str) -> Vec<u8> {
    let chat_url = format!("ws://{}{}", host, IDE_CONTEXT_CHAT_BRIDGE_PATH);
    let injected = serde_json::json!({
        "chatUrl": chat_url,
        "workspaceRoots": [],
    });
    let script = format!(
        "<script>window.__PAI_SIDEBAR_BRIDGE__ = {}; window.__PAI_SETTINGS_BRIDGE__ = window.__PAI_SIDEBAR_BRIDGE__;</script>",
        injected
    );
    let icon_links = r#"<link rel="icon" type="image/png" href="/favicon.png">
  <link rel="shortcut icon" type="image/png" href="/favicon.png">"#;
    let injection = format!("{}\n  {}", icon_links, script);
    let html = String::from_utf8_lossy(asset_bytes);
    if html.contains("</head>") {
        html.replacen("</head>", &format!("  {}\n  </head>", injection), 1)
            .into_bytes()
    } else {
        format!("{}\n{}", injection, html).into_bytes()
    }
}

async fn ide_context_http_write_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    content_type: &str,
    body: Vec<u8>,
) {
    use tokio::io::AsyncWriteExt;

    let headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n",
        status,
        ide_context_http_status_text(status),
        content_type,
        body.len()
    );
    let _ = stream.write_all(headers.as_bytes()).await;
    let _ = stream.write_all(&body).await;
    let _ = stream.shutdown().await;
    // 关连接前把对端还没读走的数据排空：接收缓冲里留着未读字节时直接 close
    // 会发 RST，调用方看到的是「连接被中止」而不是我们刚写出去的响应。
    use tokio::io::AsyncReadExt;
    let mut drain = [0_u8; 4096];
    let mut drained = 0_usize;
    while drained < 64 * 1024 {
        match tokio::time::timeout(std::time::Duration::from_millis(200), stream.read(&mut drain)).await
        {
            Ok(Ok(0)) | Ok(Err(_)) | Err(_) => break,
            Ok(Ok(count)) => drained += count,
        }
    }
}

async fn ide_context_http_handle_connection(
    mut stream: tokio::net::TcpStream,
    app: AppHandle,
    state: AppState,
) {
    use tokio::io::AsyncReadExt;

    let mut buffer = vec![0_u8; 8192];
    let count = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        stream.read(&mut buffer),
    )
    .await
    {
        Ok(Ok(count)) => count,
        _ => 0,
    };
    // 请求行与请求头都可能跨 TCP 分段到达：先读满头再分流，否则分段来的激活请求会被判成 405。
    let (headers, rest) = match ide_context_http_read_headers(&mut stream, buffer[..count].to_vec())
        .await
    {
        Ok(result) => result,
        Err(err) => {
            ide_context_http_write_response(
                &mut stream,
                400,
                "text/plain; charset=utf-8",
                err.into_bytes(),
            )
            .await;
            return;
        }
    };
    let (method, path) = ide_context_http_path_from_request(&headers);
    let activation_request = path == ACTIVATION_API_PATH;
    let activation_method = method.to_string();
    if activation_request {
        // 激活端点是本服务唯一的写入口：方法、鉴权、请求体都在这里收口。
        activation_http_handle_request(stream, state, &activation_method, headers, rest).await;
        return;
    }
    if method != "GET" {
        ide_context_http_write_response(
            &mut stream,
            405,
            "text/plain; charset=utf-8",
            b"Method Not Allowed".to_vec(),
        )
        .await;
        return;
    }
    if path == IDE_CONTEXT_BRIDGE_PATH || path == IDE_CONTEXT_CHAT_BRIDGE_PATH {
        ide_context_http_write_response(
            &mut stream,
            426,
            "text/plain; charset=utf-8",
            b"WebSocket upgrade required".to_vec(),
        )
        .await;
        return;
    }
    if let Some(icon) = ide_context_web_icon_bytes(path) {
        ide_context_http_write_response(
            &mut stream,
            200,
            "image/png",
            icon.to_vec(),
        )
        .await;
        return;
    }
    let Some(asset_path) = ide_context_web_asset_path(path) else {
        ide_context_http_write_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"Not Found".to_vec(),
        )
        .await;
        return;
    };
    let Some(asset) = app.asset_resolver().get(asset_path.clone()) else {
        ide_context_http_write_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"Asset Not Found".to_vec(),
        )
        .await;
        return;
    };
    let host = ide_context_http_header_value(&headers, "host")
        .filter(|value| !value.is_empty())
        .unwrap_or("127.0.0.1");
    let body = if asset_path == "sidebar.html" || asset_path == "settings.html" {
        ide_context_web_html_with_bridge(asset.bytes(), host)
    } else {
        asset.bytes().to_vec()
    };
    ide_context_http_write_response(
        &mut stream,
        200,
        asset.mime_type(),
        body,
    )
    .await;
}

// ==================== 外部激活端点 ====================
//
// `POST /api/v1/sessions/activate`，鉴权头 `Authorization: Bearer <token>`。
// 令牌绑定的会话就是投递目标，请求体只带一句话：
//   {"test": true}          只验令牌、回目标会话短号，不投递、不唤醒
//   {"message": "..."}      投一条系统引导消息并唤醒该会话
// 不检查会话是否存在、是否归档；投不进去就把原因回给调用方。

const ACTIVATION_API_PATH: &str = "/api/v1/sessions/activate";
/// 请求体上限：激活消息是给主助理看的文本，不需要更大的壳。
const ACTIVATION_API_MAX_BODY_BYTES: usize = 256 * 1024;

#[derive(Debug, Deserialize)]
struct ActivationApiRequest {
    #[serde(default)]
    test: bool,
    #[serde(default)]
    message: String,
}

async fn activation_http_json_response(
    stream: &mut tokio::net::TcpStream,
    status: u16,
    payload: serde_json::Value,
) {
    let body = serde_json::to_vec(&payload).unwrap_or_else(|_| b"{}".to_vec());
    ide_context_http_write_response(stream, status, "application/json; charset=utf-8", body).await;
}

fn activation_http_find_header_end(raw: &[u8]) -> Option<usize> {
    raw.windows(4).position(|window| window == b"\r\n\r\n")
}

/// 读到请求头结束为止，返回（头文本，已经多读到的请求体字节）。
/// 请求行与头都可能跨 TCP 分段到达，一次 read 拿到的字节不能当完整请求用。
async fn ide_context_http_read_headers(
    stream: &mut tokio::net::TcpStream,
    already_read: Vec<u8>,
) -> Result<(String, Vec<u8>), String> {
    use tokio::io::AsyncReadExt;

    let mut raw = already_read;
    let mut chunk = vec![0_u8; 8192];
    let header_end = loop {
        if let Some(position) = activation_http_find_header_end(&raw) {
            break position;
        }
        if raw.len() > 64 * 1024 {
            return Err("请求头过大".to_string());
        }
        let count = match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            stream.read(&mut chunk),
        )
        .await
        {
            Ok(Ok(count)) => count,
            Ok(Err(err)) => return Err(format!("读取请求失败: {err}")),
            Err(_) => return Err("读取请求超时".to_string()),
        };
        if count == 0 {
            return Err("请求不完整".to_string());
        }
        raw.extend_from_slice(&chunk[..count]);
    };
    let header_text = String::from_utf8_lossy(&raw[..header_end]).to_string();
    Ok((header_text, raw[header_end + 4..].to_vec()))
}

/// 按 Content-Length 补齐请求体。
async fn activation_http_read_body(
    stream: &mut tokio::net::TcpStream,
    header_text: &str,
    mut body: Vec<u8>,
) -> Result<Vec<u8>, String> {
    use tokio::io::AsyncReadExt;

    let content_length = ide_context_http_header_value(header_text, "content-length")
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if content_length > ACTIVATION_API_MAX_BODY_BYTES {
        return Err("请求体过大".to_string());
    }
    let mut chunk = vec![0_u8; 8192];
    while body.len() < content_length {
        let count = match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            stream.read(&mut chunk),
        )
        .await
        {
            Ok(Ok(count)) => count,
            Ok(Err(err)) => return Err(format!("读取请求体失败: {err}")),
            Err(_) => return Err("读取请求体超时".to_string()),
        };
        if count == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..count]);
    }
    body.truncate(content_length.min(body.len()));
    Ok(body)
}

/// 从 `Authorization: Bearer <token>` 取出令牌明文。
fn activation_http_bearer_token(headers: &str) -> Option<String> {
    let value = ide_context_http_header_value(headers, "authorization")?;
    let value = value.trim();
    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))?
        .trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

async fn activation_http_handle_request(
    mut stream: tokio::net::TcpStream,
    state: AppState,
    method: &str,
    header_text: String,
    already_read: Vec<u8>,
) {
    if method != "POST" {
        activation_http_json_response(
            &mut stream,
            405,
            serde_json::json!({ "ok": false, "error": "Method Not Allowed" }),
        )
        .await;
        return;
    }
    let Some(token) = activation_http_bearer_token(&header_text) else {
        activation_http_json_response(
            &mut stream,
            401,
            serde_json::json!({ "ok": false, "error": "missing bearer token" }),
        )
        .await;
        return;
    };
    let record = match activation_token_verify(&state, &token) {
        Ok(record) => record,
        Err(err) => {
            runtime_log_warn(format!("[自动化] 跳过 任务=激活令牌鉴权 原因={err}"));
            activation_http_json_response(
                &mut stream,
                401,
                serde_json::json!({ "ok": false, "error": "invalid token" }),
            )
            .await;
            return;
        }
    };
    let body = match activation_http_read_body(&mut stream, &header_text, already_read).await {
        Ok(body) => body,
        Err(err) => {
            activation_http_json_response(
                &mut stream,
                400,
                serde_json::json!({ "ok": false, "error": err }),
            )
            .await;
            return;
        }
    };
    let parsed = match serde_json::from_slice::<ActivationApiRequest>(&body) {
        Ok(parsed) => parsed,
        Err(err) => {
            activation_http_json_response(
                &mut stream,
                400,
                serde_json::json!({ "ok": false, "error": format!("请求体不是合法 JSON: {err}") }),
            )
            .await;
            return;
        }
    };
    let conversation_id = record.conversation_id.clone();
    // 对外一律回短号：调用方与模型看到的是同一个标识，不回传长 UUID。
    let short_id = conversation_short_id_for(&state, &conversation_id).map(|id| id.to_string());
    let message = parsed.message.trim();
    // 没消息就没有要发生的事：验令牌的、空请求体，都在这里收口，不投递、不唤醒。
    if parsed.test || message.is_empty() {
        activation_http_json_response(
            &mut stream,
            200,
            serde_json::json!({ "ok": true, "conversationId": short_id }),
        )
        .await;
        return;
    }
    match activation_deliver_event(&state, &conversation_id, message) {
        Ok(event_id) => {
            runtime_log_info(format!(
                "[自动化] 完成 任务=外部激活投递 令牌号={} 会话号={:?} 事件={}",
                record.token_id, short_id, event_id
            ));
            activation_http_json_response(
                &mut stream,
                200,
                serde_json::json!({
                    "ok": true,
                    "conversationId": short_id,
                    "eventId": event_id,
                }),
            )
            .await;
        }
        Err(err) => {
            runtime_log_error(format!(
                "[自动化] 失败 任务=外部激活投递 令牌号={} 原因={}",
                record.token_id, err
            ));
            activation_http_json_response(
                &mut stream,
                400,
                serde_json::json!({ "ok": false, "error": err }),
            )
            .await;
        }
    }
}
