const DINGTALK_STREAM_TOPIC: &str = "/v1.0/im/bot/messages/get";
const DINGTALK_DOWNLOAD_API: &str = "https://api.dingtalk.com/v1.0/robot/messageFiles/download";
const DINGTALK_RECONNECT_INTERVAL_SECS: u64 = 30;
const DINGTALK_MAX_DOWNLOAD_SIZE_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone)]
struct DingtalkRuntimeState {
    connected: bool,
    connected_at: Option<chrono::DateTime<chrono::Utc>>,
    endpoint: Option<String>,
    last_error: Option<String>,
}

impl Default for DingtalkRuntimeState {
    fn default() -> Self {
        Self {
            connected: false,
            connected_at: None,
            endpoint: None,
            last_error: None,
        }
    }
}

pub struct DingtalkStreamManager {
    states: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, DingtalkRuntimeState>>>,
    stop_senders: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>>,
    >,
    tasks: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tauri::async_runtime::JoinHandle<()>>>,
    >,
    port_service: std::sync::Arc<LocalPortServiceCore>,
}

impl DingtalkStreamManager {
    pub fn new() -> Self {
        Self {
            states: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            stop_senders: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            tasks: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            port_service: std::sync::Arc::new(LocalPortServiceCore::new()),
        }
    }

    async fn set_state(
        &self,
        channel_id: &str,
        connected: bool,
        endpoint: Option<String>,
        last_error: Option<String>,
    ) {
        let normalized_error = last_error
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        {
            let mut states = self.states.write().await;
            let state = states
                .entry(channel_id.to_string())
                .or_insert_with(DingtalkRuntimeState::default);
            let was_connected = state.connected;
            state.connected = connected;
            state.endpoint = endpoint;
            state.last_error = normalized_error.clone();
            state.connected_at = if connected {
                if was_connected {
                    state.connected_at
                } else {
                    Some(chrono::Utc::now())
                }
            } else {
                None
            };
        }
        self.port_service
            .set_status_text(
                channel_id,
                Some(
                    if connected {
                        "connected".to_string()
                    } else if normalized_error.is_some() {
                        "error".to_string()
                    } else {
                        "stopped".to_string()
                    },
                ),
            )
            .await;
        self.port_service
            .set_last_error(channel_id, normalized_error)
            .await;
    }

    pub(crate) async fn add_log(&self, channel_id: &str, level: &str, message: &str) {
        self.port_service.add_log(channel_id, level, message).await;
    }

    #[cfg_attr(test, allow(dead_code))]
    async fn stop_channel_inner(&self, channel_id: &str) {
        self.add_log(channel_id, "info", "[钉钉生命周期] 开始停止渠道").await;
        if let Some(tx) = self.stop_senders.write().await.remove(channel_id) {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.tasks.write().await.remove(channel_id) {
            if let Err(err) = handle.await {
                self.add_log(
                    channel_id,
                    "warn",
                    &format!("[钉钉生命周期] task={} status=失败 trigger=stop_channel error={err}", channel_id),
                )
                .await;
            }
        }
        self.set_state(channel_id, false, None, None).await;
        self.add_log(channel_id, "info", "[钉钉生命周期] 渠道已停止").await;
    }

    #[allow(dead_code)]
    pub(crate) async fn stop_channel(&self, channel_id: &str) {
        let _ = self
            .port_service
            .stop_serialized(channel_id, || async {
                self.stop_channel_inner(channel_id).await;
                Ok(())
            })
            .await;
    }

    pub(crate) async fn reconcile_channel_runtime(&self, channel: &RemoteImChannelConfig, state: AppState) -> Result<(), String> {
        self.port_service
            .reconcile_serialized(&channel.id, || async move {
                self.stop_channel_inner(&channel.id).await;
                if channel.enabled && channel.platform == RemoteImPlatform::Dingtalk {
                    self.add_log(&channel.id, "info", "[钉钉生命周期] 渠道已启用，开始启动 Stream")
                        .await;
                    self.start_channel_inner(channel.clone(), state).await?;
                } else {
                    self.port_service
                        .set_status_text(&channel.id, Some("disabled".to_string()))
                        .await;
                    self.port_service.set_last_error(&channel.id, None).await;
                    self.add_log(&channel.id, "info", "[钉钉生命周期] 渠道已停用，跳过启动")
                        .await;
                }
                Ok(())
            })
            .await
    }

    async fn start_channel_inner(&self, channel: RemoteImChannelConfig, state: AppState) -> Result<(), String> {
        let channel_id = channel.id.clone();
        self.stop_channel_inner(&channel_id).await;
        self.add_log(&channel_id, "info", "[钉钉生命周期] 正在创建 Stream 任务")
            .await;
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        self.stop_senders
            .write()
            .await
            .insert(channel_id.clone(), stop_tx);

        let task_channel_id = channel_id.clone();
        let manager = dingtalk_stream_manager();
        let handle = tauri::async_runtime::spawn(async move {
            manager
                .add_log(
                    &task_channel_id,
                    "info",
                    &format!(
                        "[钉钉生命周期] task={} status=开始 trigger=start_channel key_counts=0 duration_ms=0",
                        task_channel_id
                    ),
                )
                .await;
            let mut stop_rx = stop_rx;
            loop {
                if *stop_rx.borrow() {
                    break;
                }
                let result = tokio::select! {
                    changed = stop_rx.changed() => {
                        match changed {
                            Ok(()) => {
                                if *stop_rx.borrow() {
                                    break;
                                }
                                continue;
                            }
                            Err(_) => break,
                        }
                    }
                    ret = run_single_dingtalk_stream_session(&channel, &state) => ret,
                };
                match result {
                    Ok(()) => break,
                    Err(err) => {
                        manager
                            .set_state(&task_channel_id, false, None, Some(err.clone()))
                            .await;
                        manager
                            .add_log(
                                &task_channel_id,
                                "warn",
                                &format!(
                                    "[钉钉生命周期] task={} status=失败 trigger=run_session backoff_secs={} error={}",
                                    task_channel_id,
                                    DINGTALK_RECONNECT_INTERVAL_SECS,
                                    err
                                ),
                            )
                            .await;
                    }
                }
                tokio::select! {
                    changed = stop_rx.changed() => {
                        match changed {
                            Ok(()) => {
                                if *stop_rx.borrow() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(DINGTALK_RECONNECT_INTERVAL_SECS)) => {}
                }
            }
            manager
                .set_state(&task_channel_id, false, None, None)
                .await;
            manager
                .add_log(
                    &task_channel_id,
                    "info",
                    &format!(
                        "[钉钉生命周期] task={} status=完成 trigger=stop_channel key_counts=0 duration_ms=0",
                        task_channel_id
                    ),
                )
                .await;
        });
        self.tasks.write().await.insert(channel_id, handle);
        Ok(())
    }

    pub(crate) async fn start_channel(&self, channel: RemoteImChannelConfig, state: AppState) -> Result<(), String> {
        let service_id = channel.id.clone();
        self.port_service
            .restart_serialized(&service_id, || async move { self.start_channel_inner(channel, state).await })
            .await
    }

    pub(crate) async fn get_channel_status(&self, channel_id: &str) -> ChannelConnectionStatus {
        let state = self
            .states
            .read()
            .await
            .get(channel_id)
            .cloned()
            .unwrap_or_default();
        ChannelConnectionStatus {
            channel_id: channel_id.to_string(),
            connected: state.connected,
            peer_addr: state.endpoint.clone(),
            connected_at: state.connected_at,
            listen_addr: String::new(),
            status_text: self.port_service.get_status_text(channel_id).await,
            last_error: self
                .port_service
                .get_last_error(channel_id)
                .await
                .or(state.last_error.clone()),
            account_id: None,
            base_url: None,
            login_session_key: None,
            qrcode_url: None,
        }
    }

    pub(crate) async fn get_logs(&self, channel_id: &str) -> Vec<ChannelLogEntry> {
        self.port_service.get_logs(channel_id).await
    }
}

impl Default for DingtalkStreamManager {
    fn default() -> Self {
        Self::new()
    }
}

static DINGTALK_STREAM_MANAGER: once_cell::sync::Lazy<std::sync::Arc<DingtalkStreamManager>> =
    once_cell::sync::Lazy::new(|| std::sync::Arc::new(DingtalkStreamManager::new()));

pub fn dingtalk_stream_manager() -> std::sync::Arc<DingtalkStreamManager> {
    DINGTALK_STREAM_MANAGER.clone()
}
