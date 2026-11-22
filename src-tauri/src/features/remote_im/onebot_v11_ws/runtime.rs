impl OnebotV11WsManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connection_stop_senders: Arc::new(RwLock::new(HashMap::new())),
            channel_event_senders: Arc::new(RwLock::new(HashMap::new())),
            port_service: Arc::new(LocalPortServiceCore::new()),
            channel_tasks: Arc::new(RwLock::new(HashMap::new())),
            channel_runtimes: Arc::new(RwLock::new(HashMap::new())),
            event_consumer_stop_senders: Arc::new(RwLock::new(HashMap::new())),
            event_consumer_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加日志
    pub(crate) async fn add_log(&self, channel_id: &str, level: &str, message: &str) {
        self.port_service.add_log(channel_id, level, message).await;
    }

    #[cfg_attr(test, allow(dead_code))]
    // ==================== 停止逻辑 ====================

    /// 停止单个渠道的 WebSocket 服务器
    pub async fn stop_channel(&self, channel_id: &str) -> Result<(), String> {
        self.port_service
            .stop_serialized(channel_id, || self.stop_channel_inner(channel_id))
            .await
    }

    async fn stop_onebot_connection_inner(&self, channel_id: &str) -> Result<(), String> {
        let stop_sender = {
            self.connection_stop_senders
                .write()
                .await
                .remove(channel_id)
        };
        if let Some(tx) = stop_sender {
            let _ = tx.send(true);
        }
        let stopped = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let has_connection = self.connections.read().await.contains_key(channel_id);
                let has_stop_sender = self
                    .connection_stop_senders
                    .read()
                    .await
                    .contains_key(channel_id);
                if !has_connection && !has_stop_sender {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await;
        if stopped.is_err() {
            self.add_log(channel_id, "warn", "停止渠道连接超时，连接可能仍在退出中")
                .await;
            runtime_log_error(format!(
                "[远程IM][OneBot v11 WS] 停止渠道连接超时，连接可能仍在退出中: {}",
                channel_id
            ));
        }
        self.connections.write().await.remove(channel_id);
        Ok(())
    }

    async fn stop_channel_runtime_tasks_inner(&self, channel_id: &str) {
        let runtime = self.channel_runtimes.write().await.remove(channel_id);
        let Some(runtime) = runtime else {
            return;
        };
        self.add_log(channel_id, "info", "开始等待渠道连接任务组退出")
            .await;
        runtime_log_info(format!(
            "[远程IM][OneBot v11 WS] 开始等待渠道连接任务组退出: channel_id={}",
            channel_id
        ));
        runtime.cancel.cancel();
        runtime.tasks.close();
        runtime.tasks.wait().await;
        self.add_log(channel_id, "info", "渠道连接任务组已退出")
            .await;
        runtime_log_info(format!(
            "[远程IM][OneBot v11 WS] 渠道连接任务组已退出: channel_id={}",
            channel_id
        ));
    }

    async fn force_clear_channel_runtime_state(&self, channel_id: &str) {
        self.connections.write().await.remove(channel_id);
        self.connection_stop_senders.write().await.remove(channel_id);
        self.port_service.clear_runtime_state(channel_id).await;
        self.channel_event_senders.write().await.remove(channel_id);
        self.channel_tasks.write().await.remove(channel_id);
        self.channel_runtimes.write().await.remove(channel_id);
        self.event_consumer_stop_senders.write().await.remove(channel_id);
        self.event_consumer_tasks.write().await.remove(channel_id);
    }

    async fn channel_runtime_state_is_clear(&self, channel_id: &str) -> bool {
        !self.connections.read().await.contains_key(channel_id)
            && !self.connection_stop_senders.read().await.contains_key(channel_id)
            && !self.channel_event_senders.read().await.contains_key(channel_id)
            && self.port_service.runtime_state_is_clear(channel_id).await
            && !self.channel_tasks.read().await.contains_key(channel_id)
            && !self.channel_runtimes.read().await.contains_key(channel_id)
            && !self.event_consumer_stop_senders.read().await.contains_key(channel_id)
            && !self.event_consumer_tasks.read().await.contains_key(channel_id)
    }

    async fn stop_channel_inner(&self, channel_id: &str) -> Result<(), String> {
        let started_at = std::time::Instant::now();
        runtime_log_info(format!(
            "[远程IM][OneBot v11 WS] 开始停止渠道: channel_id={}",
            channel_id
        ));
        self.add_log(channel_id, "info", "开始停止渠道").await;

        // 1. 停止事件消费器
        if let Err(err) = self.stop_event_consumer_inner(channel_id).await {
            self.add_log(channel_id, "warn", &format!("停止事件消费器失败: {}", err))
                .await;
            runtime_log_error(format!(
                "[远程IM][OneBot v11 WS] 停止事件消费器失败: channel_id={}, error={}",
                channel_id, err
            ));
        }

        // 2. 停止活动 WebSocket 连接
        if let Err(err) = self.stop_onebot_connection_inner(channel_id).await {
            self.add_log(channel_id, "warn", &format!("停止活动连接失败: {}", err))
                .await;
            runtime_log_error(format!(
                "[远程IM][OneBot v11 WS] 停止活动连接失败: channel_id={}, error={}",
                channel_id, err
            ));
        }

        // 3. 取消 runtime（触发 axum graceful shutdown）并等待 serve 任务退出
        self.stop_channel_runtime_tasks_inner(channel_id).await;
        let handle = { self.channel_tasks.write().await.remove(channel_id) };
        if let Some(handle) = handle {
            let mut handle = handle;
            match tokio::time::timeout(Duration::from_secs(5), &mut handle).await {
                Ok(_) => {}
                Err(_) => {
                    handle.abort();
                    let _ = handle.await;
                    self.add_log(channel_id, "warn", "停止渠道超时，已强制中止 serve 任务")
                        .await;
                    runtime_log_error(format!(
                        "[远程IM][OneBot v11 WS] 停止渠道超时，已强制中止 serve 任务: {}",
                        channel_id
                    ));
                }
            }
        }

        // 4. 清理所有运行态
        self.force_clear_channel_runtime_state(channel_id).await;
        while !self.channel_runtime_state_is_clear(channel_id).await {
            self.force_clear_channel_runtime_state(channel_id).await;
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        self.add_log(channel_id, "info", "渠道服务器已停止").await;
        runtime_log_info(format!(
            "[远程IM][OneBot v11 WS] 渠道已停止: channel_id={}, duration_ms={}",
            channel_id,
            started_at.elapsed().as_millis()
        ));
        Ok(())
    }

    // ==================== 启动逻辑 ====================

    /// 依据渠道配置原子地收敛运行态：先停，再按 enabled/platform 决定是否启动
    pub(crate) async fn reconcile_channel_runtime(
        &self,
        channel: &RemoteImChannelConfig,
    ) -> Result<(), String> {
        self.port_service
            .reconcile_serialized(&channel.id, || async move {
                self.stop_channel_inner(&channel.id).await?;
                if !channel.enabled {
                    self.port_service
                        .set_status_text(&channel.id, Some("disabled".to_string()))
                        .await;
                    self.port_service.set_last_error(&channel.id, None).await;
                    self.add_log(&channel.id, "info", "渠道已禁用，跳过启动").await;
                    return Ok(());
                }
                if channel.platform != RemoteImPlatform::OnebotV11 {
                    self.port_service
                        .set_status_text(&channel.id, Some("unsupported_platform".to_string()))
                        .await;
                    self.add_log(&channel.id, "warn", "渠道不是 OneBot v11，跳过启动").await;
                    return Ok(());
                }
                self.add_log(&channel.id, "info", "渠道已启用，开始启动 OneBot v11 WS 服务")
                    .await;
                let credentials = OnebotV11WsCredentials::from_credentials(&channel.credentials);
                self.start_server(&channel.id, credentials).await
            })
            .await
    }

    /// 获取渠道连接状态
    pub async fn get_connection_status(&self, channel_id: &str) -> ChannelConnectionStatus {
        let connections = self.connections.read().await;
        let snapshot = self.port_service.status_snapshot(channel_id).await;

        if let Some(conn) = connections.get(channel_id) {
            ChannelConnectionStatus {
                channel_id: channel_id.to_string(),
                connected: true,
                peer_addr: conn.peer_addr.clone(),
                connected_at: conn.connected_at,
                listen_addr: snapshot.listen_addr.clone(),
                status_text: snapshot.status_text.clone(),
                last_error: snapshot.last_error.clone(),
                account_id: None,
                base_url: None,
                login_session_key: None,
                qrcode_url: None,
            }
        } else {
            ChannelConnectionStatus {
                channel_id: channel_id.to_string(),
                connected: false,
                peer_addr: None,
                connected_at: None,
                listen_addr: snapshot.listen_addr,
                status_text: snapshot.status_text,
                last_error: snapshot.last_error,
                account_id: None,
                base_url: None,
                login_session_key: None,
                qrcode_url: None,
            }
        }
    }

    /// 获取渠道日志
    pub async fn get_logs(&self, channel_id: &str) -> Vec<ChannelLogEntry> {
        self.port_service.get_logs(channel_id).await
    }

    /// 启动 WebSocket 服务器（公开接口）
    pub async fn start(
        &self,
        channel_id: String,
        credentials: OnebotV11WsCredentials,
    ) -> Result<(), String> {
        let _addr = onebot_listen_addr_from_credentials(&credentials)?;
        let outcome = self
            .port_service
            .start_serialized(&channel_id, self.channel_server_is_running(&channel_id), || async {
                self.stop_channel_inner(&channel_id).await?;
                self.start_server(&channel_id, credentials).await
            })
            .await?;
        if outcome == LocalPortServiceStartOutcome::SkippedAlreadyRunning {
            self.add_log(&channel_id, "info", "服务器已在运行，跳过重复启动").await;
        }
        Ok(())
    }

    async fn channel_server_is_running(&self, channel_id: &str) -> bool {
        self.channel_tasks
            .read()
            .await
            .get(channel_id)
            .map(|handle| !handle.is_finished())
            .unwrap_or(false)
    }

    /// 核心启动：绑定端口 → 启动 axum serve → 记录状态
    async fn start_server(
        &self,
        channel_id: &str,
        credentials: OnebotV11WsCredentials,
    ) -> Result<(), String> {
        let addr = onebot_listen_addr_from_credentials(&credentials)?;
        self.port_service
            .set_listen_addr(channel_id, Some(addr.to_string()))
            .await;
        self.port_service
            .set_status_text(channel_id, Some("binding".to_string()))
            .await;
        self.port_service.set_last_error(channel_id, None).await;
        self.add_log(channel_id, "info", &format!("开始绑定端口 {}", addr))
            .await;

        // 直接绑定，不做重试。如果端口被占用就报错，让用户处理。
        let listener = match TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(err) => {
                let message = format!("绑定端口失败: addr={}, error={}", addr, err);
                self.port_service
                    .set_status_text(channel_id, Some("bind_failed".to_string()))
                    .await;
                self.port_service
                    .set_last_error(channel_id, Some(message.clone()))
                    .await;
                self.add_log(channel_id, "error", &message).await;
                runtime_log_error(format!("[远程IM][OneBot v11 WS] 失败，error={}", message));
                return Err(message);
            }
        };
        let actual_addr = listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| addr.to_string());

        // 创建 runtime
        let channel_runtime = OnebotChannelRuntime {
            id: Uuid::new_v4().to_string(),
            cancel: CancellationToken::new(),
            tasks: TaskTracker::new(),
        };
        self.channel_runtimes
            .write()
            .await
            .insert(channel_id.to_string(), channel_runtime.clone());
        self.port_service
            .set_listen_addr(channel_id, Some(actual_addr.clone()))
            .await;
        self.port_service
            .set_status_text(channel_id, Some("listening".to_string()))
            .await;
        self.port_service.set_last_error(channel_id, None).await;

        // 创建连接通道
        let (conn_tx, _) = broadcast::channel::<String>(64);
        let (event_tx, _) = broadcast::channel::<Value>(256);
        self.channel_event_senders
            .write()
            .await
            .insert(channel_id.to_string(), event_tx.clone());
        let pending_responses =
            Arc::new(RwLock::new(HashMap::<String, oneshot::Sender<OneBotApiResponse>>::new()));

        let axum_state = OnebotAxumState {
            channel_id: channel_id.to_string(),
            expected_token: credentials.ws_token.clone(),
            conn_tx,
            pending_responses,
            event_tx,
            connections: self.connections.clone(),
            connection_stop_senders: self.connection_stop_senders.clone(),
            port_service: self.port_service.clone(),
            active_connection_gate: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            cancel: channel_runtime.cancel.clone(),
        };

        // 构建 axum 路由：所有路径都接受 WebSocket 升级
        let app = axum::Router::new()
            .fallback(onebot_ws_handler)
            .with_state(axum_state);

        let cancel_for_shutdown = channel_runtime.cancel.clone();
        let channel_id_owned = channel_id.to_string();
        let actual_addr_for_log = actual_addr.clone();
        let port_service_for_task = self.port_service.clone();

        let task_handle = tokio::spawn(async move {
            runtime_log_info(format!(
                "[远程IM][OneBot v11 WS] 渠道 {} axum serve 启动: {}",
                channel_id_owned, actual_addr
            ));
            port_service_for_task
                .add_log(&channel_id_owned, "info", &format!("axum serve 已启动: {}", actual_addr))
                .await;
            let serve_result = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(cancel_for_shutdown.cancelled_owned())
            .await;
            if let Err(err) = serve_result {
                port_service_for_task
                    .add_log(
                        &channel_id_owned,
                        "warn",
                        &format!("axum serve 退出异常: {}", err),
                    )
                    .await;
                runtime_log_error(format!(
                    "[远程IM][OneBot v11 WS] 渠道 {} axum serve 退出异常: {}",
                    channel_id_owned, err
                ));
            } else {
                port_service_for_task
                    .add_log(&channel_id_owned, "info", "axum serve 正常退出")
                    .await;
                runtime_log_info(format!(
                    "[远程IM][OneBot v11 WS] 渠道 {} axum serve 正常退出",
                    channel_id_owned
                ));
            }
            // serve 退出后 listener 自动 drop，端口立即释放
        });

        self.channel_tasks
            .write()
            .await
            .insert(channel_id.to_string(), task_handle);
        self.add_log(
            channel_id,
            "info",
            &format!("服务器启动，监听 {}", actual_addr_for_log),
        )
        .await;
        runtime_log_info(format!(
            "[远程IM][OneBot v11 WS] 渠道 {} 开始监听 {}",
            channel_id, actual_addr_for_log
        ));
        Ok(())
    }

    // ==================== prepare_start_after_stop_at (仅测试用) ====================

    #[allow(dead_code)]
    pub(crate) async fn prepare_start_after_stop_at(
        &self,
        channel_id: &str,
        addr: SocketAddr,
    ) -> OnebotChannelRuntime {
        let channel_runtime = OnebotChannelRuntime {
            id: Uuid::new_v4().to_string(),
            cancel: CancellationToken::new(),
            tasks: TaskTracker::new(),
        };
        self.channel_runtimes
            .write()
            .await
            .insert(channel_id.to_string(), channel_runtime.clone());
        self.port_service
            .set_listen_addr(channel_id, Some(addr.to_string()))
            .await;
        self.port_service
            .set_status_text(channel_id, Some("binding".to_string()))
            .await;
        self.port_service.set_last_error(channel_id, None).await;
        channel_runtime
    }
}

// ==================== axum WebSocket handler ====================

async fn onebot_ws_handler(
    ws: axum::extract::WebSocketUpgrade,
    axum::extract::ConnectInfo(peer_addr): axum::extract::ConnectInfo<SocketAddr>,
    axum::extract::State(state): axum::extract::State<OnebotAxumState>,
    uri: axum::http::Uri,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    // Token 验证
    if !validate_ws_token_from_query(uri.query(), &headers, state.expected_token.as_deref()) {
        append_channel_log(
            &state.port_service,
            &state.channel_id,
            "warn",
            format!("拒绝连接（token 无效）: {}", peer_addr),
        )
        .await;
        runtime_log_error(format!(
            "[远程IM][OneBot v11 WS] 渠道 {} 拒绝连接（token 无效）: {}",
            state.channel_id, peer_addr
        ));
        return axum::response::Response::builder()
            .status(axum::http::StatusCode::FORBIDDEN)
            .body(axum::body::Body::empty())
            .unwrap_or_default();
    }

    ws.on_upgrade(move |socket| handle_ws_connection(socket, peer_addr, state))
}

async fn handle_ws_connection(socket: WebSocket, peer_addr: SocketAddr, state: OnebotAxumState) {
    let channel_id = state.channel_id.clone();
    let peer_addr_str = peer_addr.to_string();

    // 连接替换逻辑：同一渠道只允许一个活跃连接
    if state
        .active_connection_gate
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
        )
        .is_err()
    {
        append_channel_log(
            &state.port_service,
            &channel_id,
            "info",
            format!("检测到已有连接，正在尝试替换: {}", peer_addr),
        )
        .await;
        runtime_log_info(format!(
            "[远程IM][OneBot v11 WS] 渠道 {} 检测到已有连接，正在尝试替换: {}",
            channel_id, peer_addr
        ));
        let existing_sender = {
            state
                .connection_stop_senders
                .read()
                .await
                .get(&channel_id)
                .cloned()
        };
        if let Some(sender) = existing_sender {
            let _ = sender.send(true);
        }
        let replaced = tokio::time::timeout(
            Duration::from_millis(NAPCAT_ACTIVE_CONNECTION_REPLACE_TIMEOUT_MS),
            async {
                loop {
                    if state
                        .active_connection_gate
                        .compare_exchange(
                            false,
                            true,
                            std::sync::atomic::Ordering::SeqCst,
                            std::sync::atomic::Ordering::SeqCst,
                        )
                        .is_ok()
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
            },
        )
        .await;
        if replaced.is_err() {
            runtime_log_error(format!(
                "[远程IM][OneBot v11 WS] 渠道 {} 替换旧连接超时，拒绝新连接: {}",
                channel_id, peer_addr
            ));
            append_channel_log(
                &state.port_service,
                &channel_id,
                "warn",
                format!("替换旧连接超时，拒绝新连接: {}", peer_addr),
            )
            .await;
            return;
        }
        append_channel_log(
            &state.port_service,
            &channel_id,
            "info",
            format!("新连接已接管旧连接: {}", peer_addr),
        )
        .await;
    }

    let (connection_stop_tx, connection_stop_rx) = tokio::sync::watch::channel(false);
    state
        .connection_stop_senders
        .write()
        .await
        .insert(channel_id.clone(), connection_stop_tx);

    runtime_log_info(format!(
        "[远程IM][OneBot v11 WS] 渠道 {} 客户端已连接: {}",
        channel_id, peer_addr_str
    ));
    append_channel_log(
        &state.port_service,
        &channel_id,
        "info",
        format!("客户端已连接: {}", peer_addr_str),
    )
    .await;

    // 更新连接状态
    {
        let conn = WsConnection {
            tx: state.conn_tx.clone(),
            pending_responses: state.pending_responses.clone(),
            peer_addr: Some(peer_addr_str.clone()),
            connected_at: Some(Utc::now()),
        };
        state
            .connections
            .write()
            .await
            .insert(channel_id.clone(), conn);
    }

    let (ws_sender, ws_receiver) = socket.split();
    let cmd_rx = state.conn_tx.subscribe();
    run_message_loop(
        ws_sender,
        ws_receiver,
        cmd_rx,
        connection_stop_rx,
        state.pending_responses,
        state.event_tx,
        state.connections.clone(),
        state.port_service.clone(),
        channel_id.clone(),
        peer_addr_str,
        state.cancel,
    )
    .await;
    state
        .connection_stop_senders
        .write()
        .await
        .remove(&channel_id);
    state
        .active_connection_gate
        .store(false, std::sync::atomic::Ordering::SeqCst);
}

fn onebot_listen_addr_from_credentials(
    credentials: &OnebotV11WsCredentials,
) -> Result<SocketAddr, String> {
    format!("{}:{}", credentials.ws_host, credentials.ws_port)
        .parse()
        .map_err(|err| format!("无效地址: {}", err))
}
