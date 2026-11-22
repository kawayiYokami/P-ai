fn validate_ws_token_from_query(query: Option<&str>, headers: &axum::http::HeaderMap, expected: Option<&str>) -> bool {
    let Some(expect) = expected else {
        return true; // 无 token 要求，直接通过
    };
    // 从 query string 提取 access_token
    if let Some(query) = query {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                if key == "access_token" && value == expect {
                    return true;
                }
            }
        }
    }
    // 从 Authorization header 提取 Bearer token
    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if token == expect {
                    return true;
                }
            }
        }
    }
    false
}

async fn append_channel_log(
    port_service: &Arc<LocalPortServiceCore>,
    channel_id: &str,
    level: &str,
    message: String,
) {
    port_service.add_log(channel_id, level, &message).await;
}

async fn route_onebot_ws_payload(
    payload: &str,
    pending_responses: &Arc<RwLock<HashMap<String, oneshot::Sender<OneBotApiResponse>>>>,
    event_tx: &broadcast::Sender<Value>,
) {
    if let Ok(value) = serde_json::from_str::<Value>(payload) {
        if let Some(echo) = value.get("echo").and_then(Value::as_str) {
            if let Ok(response) = serde_json::from_value::<OneBotApiResponse>(value.clone()) {
                if let Some(tx) = pending_responses.write().await.remove(echo) {
                    let _ = tx.send(response);
                }
            }
        } else if value.get("post_type").is_some() {
            let _ = event_tx.send(value);
        }
    }
}

async fn run_message_loop(
    mut ws_sender: AxumWsSender,
    mut ws_receiver: AxumWsReceiver,
    mut cmd_rx: broadcast::Receiver<String>,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
    pending_responses: Arc<RwLock<HashMap<String, oneshot::Sender<OneBotApiResponse>>>>,
    event_tx: broadcast::Sender<Value>,
    connections: Arc<RwLock<HashMap<String, WsConnection>>>,
    port_service: Arc<LocalPortServiceCore>,
    channel_id: String,
    peer_addr_str: String,
    cancel: CancellationToken,
) {
    use futures_util::StreamExt;

    let mut disconnect_level = "info".to_string();
    let mut disconnect_message = format!("客户端断开: {}", peer_addr_str);
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_millis(500),
                    ws_sender.send(AxumWsMessage::Close(None)),
                )
                .await;
                disconnect_level = "info".to_string();
                disconnect_message = format!("收到渠道取消信号: {}", peer_addr_str);
                break;
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Ok(payload) => {
                        let send_result = tokio::time::timeout(
                            std::time::Duration::from_millis(500),
                            ws_sender.send(AxumWsMessage::Text(payload.into())),
                        )
                        .await;
                        if !matches!(send_result, Ok(Ok(()))) {
                            disconnect_level = "warn".to_string();
                            disconnect_message = format!("向客户端发送消息失败: {}", peer_addr_str);
                            break;
                        }
                    }
                    Err(_) => {
                        disconnect_level = "warn".to_string();
                        disconnect_message = "命令广播通道已关闭".to_string();
                        break;
                    }
                }
            }
            changed = stop_rx.changed() => {
                match changed {
                    Ok(()) => {
                        if *stop_rx.borrow() {
                            let _ = tokio::time::timeout(
                                std::time::Duration::from_millis(500),
                                ws_sender.send(AxumWsMessage::Close(None)),
                            )
                            .await;
                            disconnect_level = "info".to_string();
                            disconnect_message = format!("收到连接停止信号: {}", peer_addr_str);
                            break;
                        }
                    }
                    Err(_) => {
                        disconnect_level = "warn".to_string();
                        disconnect_message = "连接停止通道已关闭".to_string();
                        break;
                    }
                }
            }
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(AxumWsMessage::Text(text))) => {
                        route_onebot_ws_payload(&text, &pending_responses, &event_tx).await;
                    }
                    Some(Ok(AxumWsMessage::Binary(data))) => {
                        if let Ok(text) = std::str::from_utf8(&data) {
                            route_onebot_ws_payload(text, &pending_responses, &event_tx).await;
                        }
                    }
                    Some(Ok(AxumWsMessage::Ping(data))) => {
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_millis(500),
                            ws_sender.send(AxumWsMessage::Pong(data)),
                        )
                        .await;
                    }
                    Some(Ok(AxumWsMessage::Close(_))) | None => {
                        break;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        runtime_log_error(format!("[远程IM] 接收错误 原因={}", e));
                        disconnect_level = "error".to_string();
                        disconnect_message = format!("接收错误: {}", e);
                        break;
                    }
                }
            }
        }
    }

    runtime_log_info(format!("[远程IM] 渠道 {} 客户端断开 来源={}", channel_id, peer_addr_str));
    append_channel_log(&port_service, &channel_id, &disconnect_level, disconnect_message).await;
    connections.write().await.remove(&channel_id);
}
