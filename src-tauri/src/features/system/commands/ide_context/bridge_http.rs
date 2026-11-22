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
    let bridge_url = ide_context_bridge_url(port);
    port_service
        .set_listen_addr(WEB_ACCESS_SERVICE_ID, Some(format!("{}:{}", IDE_CONTEXT_BRIDGE_BIND_HOST, port)))
        .await;
    let remote_password = match ide_context_effective_remote_password(state, ide_context_runtime) {
        Ok(password) => password,
        Err(err) => {
            IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);
            ide_context_set_current_port(ide_context_runtime, None);
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
}

async fn ide_context_http_handle_connection(
    mut stream: tokio::net::TcpStream,
    app: AppHandle,
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
    let headers = String::from_utf8_lossy(&buffer[..count]);
    let (method, path) = ide_context_http_path_from_request(&headers);
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
