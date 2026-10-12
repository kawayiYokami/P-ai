// ==================== QQ 官方机器人：运行时管理 ====================

#[derive(Debug, Clone)]
struct QqOfficialTokenCache {
    access_token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Default)]
struct QqOfficialRuntimeState {
    connected: bool,
    connected_at: Option<chrono::DateTime<chrono::Utc>>,
    bot_id: Option<String>,
    last_error: Option<String>,
}

/// 网关会话恢复信息，断线重连时用于 Resume。
#[derive(Debug, Clone, Default)]
struct QqOfficialResumeState {
    session_id: Option<String>,
    last_seq: Option<i64>,
}

/// 被动回复上下文：记录某联系人最近一条入站消息的 `msg_id` 与已用回复次数。
#[derive(Debug, Clone)]
struct QqOfficialPassiveContext {
    msg_id: String,
    received_at: chrono::DateTime<chrono::Utc>,
    reply_count: u32,
}

pub struct QqOfficialManager {
    states: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, QqOfficialRuntimeState>>>,
    stop_senders: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>>,
    >,
    tasks: std::sync::Arc<
        tokio::sync::RwLock<std::collections::HashMap<String, tauri::async_runtime::JoinHandle<()>>>,
    >,
    tokens: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, QqOfficialTokenCache>>>,
    token_locks: std::sync::Arc<
        std::sync::Mutex<
            std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<()>>>,
        >,
    >,
    passive_contexts:
        std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, QqOfficialPassiveContext>>>,
    port_service: std::sync::Arc<LocalPortServiceCore>,
}

impl QqOfficialManager {
    pub fn new() -> Self {
        Self {
            states: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            stop_senders: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            tasks: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            tokens: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            token_locks: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            passive_contexts: std::sync::Arc::new(tokio::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            port_service: std::sync::Arc::new(LocalPortServiceCore::new()),
        }
    }

    async fn set_state(
        &self,
        channel_id: &str,
        connected: bool,
        bot_id: Option<String>,
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
                .or_insert_with(QqOfficialRuntimeState::default);
            let was_connected = state.connected;
            state.connected = connected;
            if bot_id.is_some() {
                state.bot_id = bot_id;
            }
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
    pub(crate) async fn add_contact_log(
        &self,
        channel_id: &str,
        level: &str,
        message: &str,
        contact_record_id: &str,
    ) {
        self.port_service
            .add_log_for_contact(channel_id, level, message, Some(contact_record_id))
            .await;
    }

    async fn cached_access_token(&self, cache_key: &str) -> Option<String> {
        let tokens = self.tokens.read().await;
        let cache = tokens.get(cache_key)?;
        let refresh_at = cache.expires_at
            - chrono::Duration::seconds(QQ_OFFICIAL_TOKEN_REFRESH_AHEAD_SECS);
        if refresh_at > chrono::Utc::now() {
            Some(cache.access_token.clone())
        } else {
            None
        }
    }

    /// 取调用凭证（带缓存；同一渠道并发刷新时串行化）。
    pub(crate) async fn access_token(
        &self,
        channel: &RemoteImChannelConfig,
    ) -> Result<String, String> {
        let cache_key = channel.id.trim().to_string();
        if let Some(token) = self.cached_access_token(&cache_key).await {
            return Ok(token);
        }
        let refresh_lock = {
            let mut locks = self
                .token_locks
                .lock()
                .map_err(|_| "锁定 QQ 调用凭证刷新锁失败".to_string())?;
            locks
                .entry(cache_key.clone())
                .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        let _guard = refresh_lock.lock().await;
        if let Some(token) = self.cached_access_token(&cache_key).await {
            return Ok(token);
        }
        let credentials = QqOfficialCredentials::from_value(&channel.credentials);
        let (token, expires_in) = qq_official_request_access_token(&credentials).await?;
        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in.max(60));
        self.tokens.write().await.insert(
            cache_key,
            QqOfficialTokenCache {
                access_token: token.clone(),
                expires_at,
            },
        );
        Ok(token)
    }

    fn passive_context_key(channel_id: &str, contact_id: &str) -> String {        format!("{}:{}", channel_id.trim(), contact_id.trim())
    }

    /// 记录最近一条入站消息，供出站时作为被动回复目标。
    pub(crate) async fn record_passive_context(
        &self,
        channel_id: &str,
        contact_id: &str,
        msg_id: &str,
    ) {
        let msg_id = msg_id.trim();
        let contact_id = contact_id.trim();
        if channel_id.trim().is_empty() || contact_id.is_empty() || msg_id.is_empty() {
            return;
        }
        self.passive_contexts.write().await.insert(
            Self::passive_context_key(channel_id, contact_id),
            QqOfficialPassiveContext {
                msg_id: msg_id.to_string(),
                received_at: chrono::Utc::now(),
                reply_count: 0,
            },
        );
    }

    pub(crate) async fn passive_context(
        &self,
        channel_id: &str,
        contact_id: &str,
    ) -> Option<QqOfficialPassiveContext> {
        self.passive_contexts
            .read()
            .await
            .get(&Self::passive_context_key(channel_id, contact_id))
            .cloned()
    }

    /// 被动回复发送成功后累加已用次数。
    pub(crate) async fn mark_passive_reply_sent(&self, channel_id: &str, contact_id: &str) {
        let key = Self::passive_context_key(channel_id, contact_id);
        if let Some(context) = self.passive_contexts.write().await.get_mut(&key) {
            context.reply_count = context.reply_count.saturating_add(1);
        }
    }

    async fn stop_channel_inner(&self, channel_id: &str) {
        self.add_log(channel_id, "info", "[QQ官方机器人] 正在停止渠道连接")
            .await;
        if let Some(tx) = self.stop_senders.write().await.remove(channel_id) {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.tasks.write().await.remove(channel_id) {
            if let Err(err) = handle.await {
                self.add_log(
                    channel_id,
                    "warn",
                    &format!("[QQ官方机器人] 停止渠道任务失败: error={err}"),
                )
                .await;
            }
        }
        self.tokens.write().await.remove(channel_id);
        {
            let prefix = format!("{}:", channel_id.trim());
            self.passive_contexts
                .write()
                .await
                .retain(|key, _| !key.starts_with(&prefix));
        }
        self.set_state(channel_id, false, None, None).await;
        self.add_log(channel_id, "info", "[QQ官方机器人] 渠道已停止")
            .await;
    }

    pub(crate) async fn stop_channel(&self, channel_id: &str) {
        let _ = self
            .port_service
            .stop_serialized(channel_id, || async {
                self.stop_channel_inner(channel_id).await;
                Ok(())
            })
            .await;
    }

    pub(crate) async fn reconcile_channel_runtime(
        &self,
        channel: &RemoteImChannelConfig,
        state: AppState,
    ) -> Result<(), String> {
        self.port_service
            .reconcile_serialized(&channel.id, || async move {
                self.stop_channel_inner(&channel.id).await;
                if channel.enabled && channel.platform == RemoteImPlatform::QqOfficial {
                    self.add_log(&channel.id, "info", "[QQ官方机器人] 渠道已启用，开始建立网关连接")
                        .await;
                    self.start_channel_inner(channel.clone(), state).await?;
                } else {
                    self.port_service
                        .set_status_text(&channel.id, Some("disabled".to_string()))
                        .await;
                    self.port_service.set_last_error(&channel.id, None).await;
                    self.add_log(&channel.id, "info", "[QQ官方机器人] 渠道已停用，跳过启动")
                        .await;
                }
                Ok(())
            })
            .await
    }

    async fn start_channel_inner(
        &self,
        channel: RemoteImChannelConfig,
        state: AppState,
    ) -> Result<(), String> {
        let channel_id = channel.id.clone();
        self.stop_channel_inner(&channel_id).await;
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        self.stop_senders
            .write()
            .await
            .insert(channel_id.clone(), stop_tx);
        let task_channel_id = channel_id.clone();
        let manager = qq_official_manager();
        let handle = tauri::async_runtime::spawn(async move {
            let mut stop_rx = stop_rx;
            let mut stop_watch = stop_rx.clone();
            let mut resume = QqOfficialResumeState::default();
            let mut backoff_secs = QQ_OFFICIAL_RECONNECT_MIN_SECS;
            manager
                .add_log(
                    &task_channel_id,
                    "info",
                    "[QQ官方机器人] 渠道任务已启动",
                )
                .await;
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
                    ret = run_qq_official_ws_once(
                        &channel,
                        &state,
                        &manager,
                        &mut resume,
                        &mut stop_watch,
                        &mut backoff_secs,
                    ) => ret,
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
                                    "[QQ官方机器人] 网关会话中断，{} 秒后重连: error={}",
                                    backoff_secs, err
                                ),
                            )
                            .await;
                    }
                }
                backoff_secs = (backoff_secs * 2).min(QQ_OFFICIAL_RECONNECT_MAX_SECS);
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
                    _ = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {}
                }
            }
            manager
                .set_state(&task_channel_id, false, None, None)
                .await;
            manager
                .add_log(&task_channel_id, "info", "[QQ官方机器人] 渠道任务已结束")
                .await;
        });
        self.tasks.write().await.insert(channel_id, handle);
        Ok(())
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
            peer_addr: None,
            connected_at: state.connected_at,
            listen_addr: String::new(),
            status_text: self.port_service.get_status_text(channel_id).await,
            last_error: self
                .port_service
                .get_last_error(channel_id)
                .await
                .or(state.last_error.clone()),
            account_id: state.bot_id.clone(),
            base_url: None,
            login_session_key: None,
            qrcode_url: None,
        }
    }

    pub(crate) async fn get_logs(&self, channel_id: &str) -> Vec<ChannelLogEntry> {
        self.port_service.get_logs(channel_id).await
    }

    /// 发送「输入中」状态；失败只记录日志，不影响主流程。
    pub(crate) async fn send_input_notify(
        &self,
        channel: &RemoteImChannelConfig,
        contact_type: &str,
        contact_id: &str,
    ) {
        if let Err(err) = qq_official_send_input_notify(channel, contact_type, contact_id).await {
            runtime_log_debug(format!(
                "[远程IM][QQ官方机器人] 发送输入中状态失败: channel_id={}, contact_id={}, error={}",
                channel.id, contact_id, err
            ));
        }
    }
}

impl Default for QqOfficialManager {
    fn default() -> Self {
        Self::new()
    }
}

static QQ_OFFICIAL_MANAGER: once_cell::sync::Lazy<std::sync::Arc<QqOfficialManager>> =
    once_cell::sync::Lazy::new(|| std::sync::Arc::new(QqOfficialManager::new()));

pub fn qq_official_manager() -> std::sync::Arc<QqOfficialManager> {
    QQ_OFFICIAL_MANAGER.clone()
}

/// 建立一次网关会话；正常停止或被服务端断开时返回，由调用方决定是否重连。
async fn run_qq_official_ws_once(
    channel: &RemoteImChannelConfig,
    state: &AppState,
    manager: &std::sync::Arc<QqOfficialManager>,
    resume: &mut QqOfficialResumeState,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
    backoff_secs: &mut u64,
) -> Result<(), String> {
    use futures_util::{SinkExt as _, StreamExt as _};

    let channel_id = channel.id.as_str();
    let credentials = QqOfficialCredentials::from_value(&channel.credentials);
    let access_token = manager.access_token(channel).await?;
    let gateway_url = qq_official_request_gateway_url(&credentials, &access_token).await?;
    let identify_token = format!("QQBot {access_token}");

    let (mut ws, _response) = tokio_tungstenite::connect_async(gateway_url.as_str())
        .await
        .map_err(|err| format!("连接 QQ 网关失败: {err}"))?;
    manager
        .add_log(channel_id, "info", "[QQ官方机器人] 网关连接已建立")
        .await;

    let mut identified = false;
    let mut heartbeat_timer = tokio::time::interval(std::time::Duration::from_secs(45));
    heartbeat_timer.tick().await;

    loop {
        tokio::select! {
            changed = stop_rx.changed() => {
                let _ = ws.close(None).await;
                match changed {
                    Ok(()) => {
                        if *stop_rx.borrow() {
                            return Ok(());
                        }
                    }
                    Err(_) => return Ok(()),
                }
            }
            _ = heartbeat_timer.tick(), if identified => {
                let payload = serde_json::json!({
                    "op": QQ_OFFICIAL_OP_HEARTBEAT,
                    "d": resume.last_seq,
                });
                ws.send(tokio_tungstenite::tungstenite::Message::Text(
                    payload.to_string().into(),
                ))
                .await
                .map_err(|err| format!("发送 QQ 网关心跳失败: {err}"))?;
            }
            message = ws.next() => {
                let message = match message {
                    Some(Ok(message)) => message,
                    Some(Err(err)) => return Err(format!("QQ 网关连接异常: {err}")),
                    None => return Err("QQ 网关连接已关闭".to_string()),
                };
                match message {
                    tokio_tungstenite::tungstenite::Message::Text(text) => {
                        let payload: Value = serde_json::from_str(text.as_str())
                            .map_err(|err| format!("解析 QQ 网关下行消息失败: {err}"))?;
                        let op = payload.get("op").and_then(qq_official_json_i64).unwrap_or(-1);
                        let data = payload.get("d").cloned().unwrap_or(Value::Null);
                        if let Some(seq) = payload.get("s").and_then(qq_official_json_i64) {
                            resume.last_seq = Some(seq);
                        }
                        match op {
                            QQ_OFFICIAL_OP_HELLO => {
                                let interval_ms = data
                                    .get("heartbeat_interval")
                                    .and_then(qq_official_json_i64)
                                    .filter(|value| *value > 0)
                                    .unwrap_or(45_000) as u64;
                                heartbeat_timer =
                                    tokio::time::interval(std::time::Duration::from_millis(
                                        interval_ms.max(1_000),
                                    ));
                                heartbeat_timer.tick().await;
                                let frame = match resume.session_id.clone() {
                                    Some(session_id) => serde_json::json!({
                                        "op": QQ_OFFICIAL_OP_RESUME,
                                        "d": {
                                            "token": identify_token,
                                            "session_id": session_id,
                                            "seq": resume.last_seq,
                                        }
                                    }),
                                    None => serde_json::json!({
                                        "op": QQ_OFFICIAL_OP_IDENTIFY,
                                        "d": {
                                            "token": identify_token,
                                            "intents": QQ_OFFICIAL_INTENT_GROUP_AND_C2C,
                                            "shard": [0, 1],
                                            "properties": {
                                                "$os": std::env::consts::OS,
                                                "$browser": "pai",
                                                "$device": "pai",
                                            }
                                        }
                                    }),
                                };
                                ws.send(tokio_tungstenite::tungstenite::Message::Text(
                                    frame.to_string().into(),
                                ))
                                .await
                                .map_err(|err| format!("发送 QQ 网关鉴权消息失败: {err}"))?;
                                manager
                                    .add_log(channel_id, "debug", "[QQ官方机器人] 已发送网关鉴权消息")
                                    .await;
                            }
                            QQ_OFFICIAL_OP_DISPATCH => {
                                let event_name =
                                    payload.get("t").and_then(Value::as_str).unwrap_or("");
                                if event_name == QQ_OFFICIAL_EVENT_READY || event_name == "RESUMED" {
                                    identified = true;
                                    // 连接恢复后重置退避，避免后续偶发断线仍受历史退避影响。
                                    *backoff_secs = QQ_OFFICIAL_RECONNECT_MIN_SECS;
                                    if let Some(session_id) =
                                        data.get("session_id").and_then(Value::as_str)
                                    {
                                        resume.session_id = Some(session_id.to_string());
                                    }
                                    let bot_id = data
                                        .pointer("/user/id")
                                        .and_then(Value::as_str)
                                        .map(str::to_string);
                                    manager
                                        .set_state(channel_id, true, bot_id, None)
                                        .await;
                                    manager
                                        .add_log(channel_id, "info", "[QQ官方机器人] 网关鉴权完成")
                                        .await;
                                } else {
                                    qq_official_dispatch_event(channel, state, event_name, &data)
                                        .await;
                                }
                            }
                            QQ_OFFICIAL_OP_HEARTBEAT_ACK => {}
                            QQ_OFFICIAL_OP_RECONNECT => {
                                return Err("QQ 网关要求重新连接".to_string());
                            }
                            QQ_OFFICIAL_OP_INVALID_SESSION => {
                                *resume = QqOfficialResumeState::default();
                                return Err("QQ 网关会话失效，需要重新鉴权".to_string());
                            }
                            _ => {}
                        }
                    }
                    tokio_tungstenite::tungstenite::Message::Close(_) => {
                        return Err("QQ 网关主动关闭连接".to_string());
                    }
                    _ => {}
                }
            }
        }
    }
}
