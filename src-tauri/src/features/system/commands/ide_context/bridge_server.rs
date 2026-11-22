async fn start_ide_context_bridge_server_inner(
    app: AppHandle,
    state: AppState,
    ide_context_runtime: IdeContextRuntime,
) {
    let port_service = ide_context_port_service_core();
    if IDE_CONTEXT_BRIDGE_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    let shutdown_token = ide_context_bridge_create_shutdown_token();
    let Some((listener, port, bridge_url)) = prepare_ide_context_bridge_server_start(
        &state,
        &ide_context_runtime,
        &port_service,
    )
    .await
    else {
        return;
    };
    spawn_ide_context_bridge_server_task(
        app,
        state,
        ide_context_runtime,
        port_service,
        shutdown_token,
        listener,
        port,
        bridge_url,
    );
}

async fn start_web_access_server(
    app: AppHandle,
    state: AppState,
    ide_context_runtime: IdeContextRuntime,
) {
    let port_service = ide_context_port_service_core();
    let outcome = port_service
        .start_serialized(
            WEB_ACCESS_SERVICE_ID,
            async {
                IDE_CONTEXT_BRIDGE_STARTED.load(Ordering::SeqCst)
                    || ide_context_bridge_server_task_is_running()
            },
            || async {
                start_ide_context_bridge_server_inner(app, state, ide_context_runtime).await;
                Ok(())
            },
        )
        .await;
    match outcome {
        Ok(LocalPortServiceStartOutcome::SkippedAlreadyRunning) => {
            runtime_log_info(format!("[系统] 跳过重复启动：服务已在运行或正在启动"));
        }
        Ok(LocalPortServiceStartOutcome::Started) => {}
        Err(err) => {
            runtime_log_error(format!("[系统] 启动流程失败: {}", err));
        }
    }
}

async fn shutdown_ide_context_bridge_server_inner() {
    let port_service = ide_context_port_service_core();
    if !IDE_CONTEXT_BRIDGE_STARTED.load(Ordering::SeqCst) {
        ide_context_notify_chat_clients_shutdown("network_access_disabled");
        clear_ide_context_bridge_discovery();
        let _ = ide_context_bridge_take_server_task();
        port_service
            .set_listen_addr(WEB_ACCESS_SERVICE_ID, None)
            .await;
        port_service
            .set_status_text(WEB_ACCESS_SERVICE_ID, Some("stopped".to_string()))
            .await;
        port_service.set_last_error(WEB_ACCESS_SERVICE_ID, None).await;
        return;
    }
    if let Ok(slot) = ide_context_bridge_shutdown_slot().lock() {
        if let Some(token) = slot.as_ref() {
            token.cancel();
        }
    }
    ide_context_notify_chat_clients_shutdown("network_access_disabled");
    clear_ide_context_bridge_discovery();
    if let Ok(mut connections) = web_access_connections().lock() {
        connections.clear();
    }
    if let Some(clients) = IDE_CONTEXT_CHAT_CLIENTS.get() {
        if let Ok(mut clients) = clients.lock() {
            clients.clear();
        }
    }
    let task = ide_context_bridge_take_server_task();
    match task {
        Some(handle) => match tokio::time::timeout(std::time::Duration::from_secs(3), handle).await {
            Ok(Ok(())) => {
                port_service
                    .add_log(WEB_ACCESS_SERVICE_ID, "info", "服务已停止")
                    .await;
                runtime_log_info(format!("[系统] 已停止"));
            }
            Ok(Err(err)) => {
                IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);
                port_service
                    .set_last_error(WEB_ACCESS_SERVICE_ID, Some(err.to_string()))
                    .await;
                runtime_log_error(format!("[系统] 等待服务任务退出失败: {}", err));
            }
            Err(_) => {
                IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);
                port_service
                    .set_last_error(
                        WEB_ACCESS_SERVICE_ID,
                        Some("等待服务任务退出超时，已强制清理状态".to_string()),
                    )
                    .await;
                runtime_log_error(format!("[系统] 等待服务任务退出超时，已强制清理状态"));
            }
        },
        None => {
            IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);
            runtime_log_info(format!("[系统] 停机时未找到服务任务句柄，已清理状态"));
        }
    }
    if let Ok(mut slot) = ide_context_bridge_shutdown_slot().lock() {
        slot.take();
    }
    port_service
        .set_status_text(WEB_ACCESS_SERVICE_ID, Some("stopped".to_string()))
        .await;
    port_service
        .set_listen_addr(WEB_ACCESS_SERVICE_ID, None)
        .await;
}

pub(crate) async fn shutdown_web_access_server() {
    if let Err(err) = ide_context_port_service_core()
        .stop_serialized(WEB_ACCESS_SERVICE_ID, || async {
            shutdown_ide_context_bridge_server_inner().await;
            Ok(())
        })
        .await
    {
        runtime_log_error(format!("[系统] 停止流程失败: {}", err));
    }
}

async fn restart_web_access_server(
    app: AppHandle,
    state: AppState,
    ide_context_runtime: IdeContextRuntime,
) {
    let port_service = ide_context_port_service_core();
    if let Err(err) = port_service
        .restart_serialized(WEB_ACCESS_SERVICE_ID, || async {
            shutdown_ide_context_bridge_server_inner().await;
            start_ide_context_bridge_server_inner(app, state, ide_context_runtime).await;
            Ok(())
        })
        .await
    {
        runtime_log_error(format!("[系统] 重启流程失败: {}", err));
    }
}

async fn ide_context_ws_handle_connection(
    stream: tokio::net::TcpStream,
    peer_addr: std::net::SocketAddr,
    port: u16,
    app: AppHandle,
    state: AppState,
    ide_context_runtime: IdeContextRuntime,
) {
    if !ide_context_stream_is_websocket(&stream).await {
        ide_context_http_handle_connection(stream, app).await;
        return;
    }
    let path_holder = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let path_holder_clone = path_holder.clone();
    let ws_stream = match accept_hdr_async(stream, move |request: &Request, response: Response| {
        if let Ok(mut slot) = path_holder_clone.lock() {
            *slot = request.uri().path().to_string();
        }
        if !ide_context_ws_origin_allowed(request, port) {
            return Err(ide_context_ws_forbidden_response("Forbidden origin"));
        }
        Ok(response)
    })
    .await
    {
        Ok(ws_stream) => ws_stream,
        Err(err) => {
            runtime_log_error(format!("[系统] WebSocket 握手失败 {}: {}", peer_addr, err));
            return;
        }
    };
    let path = path_holder.lock().map(|value| value.clone()).unwrap_or_default();
    if path == IDE_CONTEXT_CHAT_BRIDGE_PATH {
        ide_context_chat_ws_handle_connection(
            ws_stream,
            peer_addr,
            app,
            state,
            ide_context_runtime,
        )
        .await;
        return;
    }
    if path != IDE_CONTEXT_BRIDGE_PATH {
        runtime_log_info(format!("[系统] 非法路径 {} from {}", path, peer_addr));
        return;
    }
    runtime_log_info(format!("[系统] 客户端已连接: {}", peer_addr));
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut connected_client_id = String::new();
    let mut authenticated = ide_context_peer_is_local(&peer_addr);
    let connection_id = web_access_register_connection(
        IDE_CONTEXT_BRIDGE_PATH,
        &peer_addr,
        ide_context_peer_is_local(&peer_addr),
        authenticated,
        "",
    );
    let _ = ws_sender
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "ready",
                "path": IDE_CONTEXT_BRIDGE_PATH,
                "authRequired": !authenticated,
            })
                .to_string()
                .into(),
        ))
        .await;
    while let Some(message) = ws_receiver.next().await {
        match message {
            Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                if !ide_context_web_access_enabled(&state) {
                    let _ = ws_sender
                        .send(tokio_tungstenite::tungstenite::Message::Text(
                            serde_json::json!({
                                "type": "ack",
                                "ok": false,
                                "error": "网络访问已关闭",
                            })
                            .to_string()
                            .into(),
                        ))
                        .await;
                    break;
                }
                match serde_json::from_str::<UpsertIdeContextSnapshotInput>(&text) {
                    Ok(input) => {
                        if !authenticated {
                            match ide_context_consume_bridge_token_with_state(
                                &ide_context_runtime,
                                Some(&state),
                                input.auth_token.as_deref().unwrap_or(""),
                            ) {
                                Ok(_token) => {
                                    authenticated = true;
                                    web_access_update_connection_auth(&connection_id, true, None);
                                }
                                Err((err, refreshed_token)) => {
                                    if let Some(_refreshed_token) = refreshed_token.as_deref() {
                                        if let Ok(remote_password) = ide_context_effective_remote_password(&state, &ide_context_runtime) {
                                            if let Err(publish_err) =
                                                publish_ide_context_bridge_discovery(port, &remote_password)
                                            {
                                                runtime_log_error(format!(
                                                    "[系统] 过期后重写发现文件失败: {}",
                                                    publish_err
                                                ));
                                            }
                                        }
                                    }
                                    let _ = ws_sender
                                        .send(tokio_tungstenite::tungstenite::Message::Text(
                                            serde_json::json!({"type": "ack", "ok": false, "error": err})
                                                .to_string()
                                                .into(),
                                        ))
                                        .await;
                                    break;
                                }
                            }
                        }
                        match upsert_ide_context_snapshot_internal(input, &ide_context_runtime) {
                            Ok((client_id, updated_at)) => {
                                connected_client_id = client_id.clone();
                                web_access_update_connection_auth(
                                    &connection_id,
                                    authenticated,
                                    Some(&client_id),
                                );
                                emit_ide_context_updated(&state, &client_id, &updated_at);
                                let _ = ws_sender
                                    .send(tokio_tungstenite::tungstenite::Message::Text(
                                        serde_json::json!({"type": "ack", "ok": true}).to_string().into(),
                                    ))
                                    .await;
                            }
                            Err(err) => {
                                let _ = ws_sender
                                    .send(tokio_tungstenite::tungstenite::Message::Text(
                                        serde_json::json!({"type": "ack", "ok": false, "error": err})
                                            .to_string()
                                            .into(),
                                    ))
                                    .await;
                            }
                        }
                    }
                    Err(err) => {
                        let _ = ws_sender
                            .send(tokio_tungstenite::tungstenite::Message::Text(
                                serde_json::json!({"type": "ack", "ok": false, "error": format!("invalid json: {err}")}).to_string().into(),
                            ))
                            .await;
                    }
                }
            }
            Ok(tokio_tungstenite::tungstenite::Message::Ping(payload)) => {
                let _ = ws_sender.send(tokio_tungstenite::tungstenite::Message::Pong(payload)).await;
            }
            Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
            Ok(_) => {}
            Err(err) => {
                runtime_log_error(format!("[系统] 客户端消息错误 {}: {}", peer_addr, err));
                break;
            }
        }
    }
    web_access_remove_connection(&connection_id);
    if !connected_client_id.is_empty() {
        match ide_context_runtime.snapshots.lock() {
            Ok(mut snapshots) => {
                snapshots.remove(&connected_client_id);
            }
            Err(_) => {
                runtime_log_error(format!("[系统] 清理客户端缓存失败: {}", connected_client_id));
            }
        }
    }
    runtime_log_info(format!("[系统] 客户端已断开: {}", peer_addr));
}

async fn ide_context_chat_ws_handle_connection(
    ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    peer_addr: std::net::SocketAddr,
    app: AppHandle,
    state: AppState,
    ide_context_runtime: IdeContextRuntime,
) {
    runtime_log_info(format!("[系统] 客户端已连接: {}", peer_addr));
    let client_id = Uuid::new_v4().to_string();
    let mut authenticated = ide_context_peer_is_local(&peer_addr);
    let connection_id = web_access_register_connection(
        IDE_CONTEXT_CHAT_BRIDGE_PATH,
        &peer_addr,
        ide_context_peer_is_local(&peer_addr),
        authenticated,
        &client_id,
    );
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::unbounded_channel::<serde_json::Value>();
    let writer_client_id = client_id.clone();
    let writer = tauri::async_runtime::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            if ws_sender
                .send(tokio_tungstenite::tungstenite::Message::Text(message.to_string().into()))
                .await
                .is_err()
            {
                break;
            }
        }
        if let Ok(mut clients) = ide_context_chat_clients().lock() {
            clients.remove(&writer_client_id);
        }
        if let Ok(mut conversations) = ide_context_chat_client_conversations().lock() {
            conversations.remove(&writer_client_id);
        }
    });
    let _ = outbound_tx.send(serde_json::json!({
        "jsonrpc": "2.0",
        "method": "bridge.ready",
        "params": {
            "path": IDE_CONTEXT_CHAT_BRIDGE_PATH,
            "authRequired": !authenticated,
            "authMode": if authenticated { "none" } else { "password" },
            "attachmentTransfer": {
                "version": 1,
                "chunkSize": ATTACHMENT_TRANSFER_CHUNK_BYTES,
                "maxBytes": ATTACHMENT_TRANSFER_WEB_MAX_BYTES,
            },
        },
    }));
    let mut registered_client = false;
    if authenticated {
        if let Ok(mut clients) = ide_context_chat_clients().lock() {
            clients.insert(client_id.clone(), outbound_tx.clone());
            registered_client = true;
        }
    }
    let mut opened_conversation_id: Option<String> = None;
    while let Some(message) = ws_receiver.next().await {
        match message {
            Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                if !ide_context_web_access_enabled(&state) {
                    let _ = outbound_tx.send(ide_context_bridge_shutdown_notification(
                        "network_access_disabled",
                    ));
                    let _ = outbound_tx.send(ide_chat_jsonrpc_error(
                        None,
                        -32002,
                        "网络访问已关闭",
                    ));
                    break;
                }
                let response = match serde_json::from_str::<IdeChatJsonRpcRequest>(&text) {
                    Ok(request) => {
                        if !authenticated {
                            if request.jsonrpc.trim() != "2.0" {
                                ide_chat_jsonrpc_error(request.id, -32600, "jsonrpc must be 2.0")
                            } else if request.method.as_str() == "auth.login" {
                                match ide_chat_parse_params::<IdeChatAuthLoginInput>(request.params) {
                                    Ok(input) => {
                                        match ide_context_verify_remote_password(
                                            &ide_context_runtime,
                                            Some(&state),
                                            &input.password,
                                        ) {
                                        Ok(true) => match ide_context_issue_bridge_token_with_state(
                                            &ide_context_runtime,
                                            Some(&state),
                                        ) {
                                            Ok(auth_token) => {
                                                authenticated = true;
                                                web_access_update_connection_auth(
                                                    &connection_id,
                                                    true,
                                                    Some(&client_id),
                                                );
                                                if !registered_client {
                                                    if let Ok(mut clients) = ide_context_chat_clients().lock() {
                                                        clients.insert(client_id.clone(), outbound_tx.clone());
                                                        registered_client = true;
                                                    }
                                                }
                                                ide_chat_jsonrpc_success(request.id, serde_json::json!({
                                                    "authenticated": true,
                                                    "authToken": auth_token,
                                                }))
                                            }
                                            Err(err) => ide_chat_jsonrpc_error(request.id, -32000, err),
                                        },
                                        Ok(false) => ide_chat_jsonrpc_error(request.id, -32001, "远程访问密码错误"),
                                        Err(err) => ide_chat_jsonrpc_error(request.id, -32000, err),
                                        }
                                    }
                                    Err(err) => ide_chat_jsonrpc_error(request.id, -32602, err),
                                }
                            } else {
                                let provided_auth_token = request
                                    .params
                                    .as_object()
                                    .and_then(|params| params.get("authToken"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                match ide_context_consume_bridge_token_with_state(
                                    &ide_context_runtime,
                                    Some(&state),
                                    provided_auth_token,
                                ) {
                                    Ok(_token) => {
                                        authenticated = true;
                                        web_access_update_connection_auth(
                                            &connection_id,
                                            true,
                                            Some(&client_id),
                                        );
                                        if !registered_client {
                                            if let Ok(mut clients) = ide_context_chat_clients().lock() {
                                                clients.insert(client_id.clone(), outbound_tx.clone());
                                                registered_client = true;
                                            }
                                        }
                                        ide_chat_handle_jsonrpc_request(
                                            request,
                                            &state,
                                            &app,
                                            &ide_context_runtime,
                                            &client_id,
                                            &mut opened_conversation_id,
                                        )
                                        .await
                                    }
                                    Err((err, refreshed_token)) => {
                                        if let Some(_refreshed_token) = refreshed_token.as_deref() {
                                            if let Some(current_port) = ide_context_current_port(&ide_context_runtime) {
                                                if let Ok(remote_password) =
                                                    ide_context_effective_remote_password(&state, &ide_context_runtime)
                                                {
                                                    if let Err(publish_err) =
                                                        publish_ide_context_bridge_discovery(current_port, &remote_password)
                                                    {
                                                        runtime_log_error(format!(
                                                            "[系统] 过期后重写发现文件失败: {}",
                                                            publish_err
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                        ide_chat_jsonrpc_error(request.id, -32001, err)
                                    }
                                }
                            }
                        } else {
                            ide_chat_handle_jsonrpc_request(
                                request,
                                &state,
                                &app,
                                &ide_context_runtime,
                                &client_id,
                                &mut opened_conversation_id,
                            )
                            .await
                        }
                    }
                    Err(err) => ide_chat_jsonrpc_error(None, -32700, format!("invalid json: {err}")),
                };
                let _ = outbound_tx.send(response);
            }
            Ok(tokio_tungstenite::tungstenite::Message::Ping(payload)) => {
                let _ = outbound_tx.send(serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "bridge.ping",
                    "params": { "bytes": payload.len() },
                }));
            }
            Ok(tokio_tungstenite::tungstenite::Message::Binary(data)) => {
                if !ide_context_web_access_enabled(&state) {
                    let _ = outbound_tx.send(ide_chat_jsonrpc_error(
                        None,
                        -32002,
                        "网络访问已关闭",
                    ));
                    break;
                }
                if !authenticated {
                    let _ = outbound_tx.send(ide_chat_jsonrpc_error(
                        None,
                        -32001,
                        "远程访问需要先输入密码",
                    ));
                } else {
                    let response = match ide_attachment_transfer_binary_chunk(&client_id, &data).await {
                        Ok(notification) => notification,
                        Err(err) => ide_chat_jsonrpc_error(None, -32020, err),
                    };
                    let _ = outbound_tx.send(response);
                }
            }
            Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
            Ok(_) => {}
            Err(err) => {
                runtime_log_error(format!("[系统] 客户端消息错误 {}: {}", peer_addr, err));
                break;
            }
        }
    }
    web_access_remove_connection(&connection_id);
    if let Ok(mut clients) = ide_context_chat_clients().lock() {
        clients.remove(&client_id);
    }
    if let Ok(mut conversations) = ide_context_chat_client_conversations().lock() {
        conversations.remove(&client_id);
    }
    if opened_conversation_id.is_some() {
        let sidebar_label = ide_chat_sidebar_window_label(&client_id);
        if let Err(err) = ide_chat_release_sidebar_conversation(&state, &sidebar_label) {
            runtime_log_error(format!("[系统] 释放会话占用失败: {}", err));
        }
    }
    attachment_transfer_abort_owner(&client_id).await;
    writer.abort();
    runtime_log_info(format!("[系统] 客户端已断开: {}", peer_addr));
}
