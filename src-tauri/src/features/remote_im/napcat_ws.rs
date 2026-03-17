// NapCat 反向 WebSocket 服务器
// 实现 OneBot v11 协议的反向 WebSocket 连接

use std::net::SocketAddr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, oneshot, RwLock};
use tokio_tungstenite::accept_hdr_async;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::http::Response as HttpResponse;
use tokio_tungstenite::tungstenite::Message;

/// 渠道日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelLogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String, // "info", "warn", "error"
    pub message: String,
}

/// 渠道连接状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelConnectionStatus {
    pub channel_id: String,
    pub connected: bool,
    pub peer_addr: Option<String>,
    pub connected_at: Option<DateTime<Utc>>,
    pub listen_addr: String,
}

/// NapCat 凭证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NapcatCredentials {
    #[serde(default = "default_ws_host")]
    pub ws_host: String,
    #[serde(default = "default_ws_port")]
    pub ws_port: u16,
    #[serde(default)]
    pub ws_token: Option<String>,
}

fn default_ws_host() -> String {
    "0.0.0.0".to_string()
}

fn default_ws_port() -> u16 {
    6199
}

const NAPCAT_RECONNECT_INTERVAL_SECS: u64 = 30;

impl NapcatCredentials {
    pub fn from_credentials(credentials: &Value) -> Self {
        serde_json::from_value(credentials.clone()).unwrap_or_default()
    }
}

impl Default for NapcatCredentials {
    fn default() -> Self {
        Self {
            ws_host: default_ws_host(),
            ws_port: default_ws_port(),
            ws_token: None,
        }
    }
}

/// OneBot v11 API 请求
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OneBotApiRequest {
    action: String,
    params: Value,
    #[serde(default)]
    echo: Option<Value>,
}

/// OneBot v11 API 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OneBotApiResponse {
    status: String,
    retcode: i64,
    data: Value,
    #[serde(default)]
    echo: Option<Value>,
}

/// WebSocket 连接信息
struct WsConnection {
    /// 发送请求的通道
    tx: broadcast::Sender<String>,
    /// 等待响应的 oneshot 映射: echo -> sender
    pending_responses: Arc<RwLock<HashMap<String, oneshot::Sender<OneBotApiResponse>>>>,
    /// 事件上报通道
    event_tx: broadcast::Sender<Value>,
    /// 连接的对端地址
    peer_addr: Option<String>,
    /// 连接时间
    connected_at: Option<DateTime<Utc>>,
}

/// NapCat WebSocket 服务器管理器
pub struct NapcatWsManager {
    /// 活跃连接: channel_id -> 连接信息
    connections: Arc<RwLock<HashMap<String, WsConnection>>>,
    /// 每个渠道独立的关闭信号: channel_id -> shutdown sender
    channel_shutdowns: Arc<RwLock<HashMap<String, broadcast::Sender<()>>>>,
    /// 渠道日志: channel_id -> 日志条目列表
    channel_logs: Arc<RwLock<HashMap<String, Vec<ChannelLogEntry>>>>,
    /// 渠道监听地址: channel_id -> listen_addr
    listen_addrs: Arc<RwLock<HashMap<String, String>>>,
    /// 渠道 accept 循环的 JoinHandle，用于 stop 时等待旧服务器释放端口
    channel_tasks: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

type WsStream = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;
type WsSender = SplitSink<WsStream, Message>;
type WsReceiver = SplitStream<WsStream>;

fn build_reject_response(
    status: tokio_tungstenite::tungstenite::http::StatusCode,
) -> HttpResponse<Option<String>> {
    tokio_tungstenite::tungstenite::http::Response::builder()
        .status(status)
        .body(None)
        .unwrap_or_else(|e| {
            eprintln!("[NapCat WS] 构建拒绝响应失败: {}", e);
            tokio_tungstenite::tungstenite::http::Response::builder()
                .status(tokio_tungstenite::tungstenite::http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(None)
                .unwrap_or_else(|_| tokio_tungstenite::tungstenite::http::Response::new(None))
        })
}

fn validate_ws_token(req: &Request, expected: Option<&str>) -> Result<(), HttpResponse<Option<String>>> {
    let mut received_token: Option<String> = None;
    if let Some(query) = req.uri().query() {
        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                if key == "access_token" {
                    received_token = Some(value.to_string());
                }
            }
        }
    }
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                received_token = Some(token.to_string());
            }
        }
    }
    if let Some(expect) = expected {
        match received_token.as_deref() {
            Some(token) if token == expect => Ok(()),
            _ => Err(build_reject_response(
                tokio_tungstenite::tungstenite::http::StatusCode::FORBIDDEN,
            )),
        }
    } else {
        Ok(())
    }
}

async fn append_channel_log(
    channel_logs: &Arc<RwLock<HashMap<String, Vec<ChannelLogEntry>>>>,
    channel_id: &str,
    level: &str,
    message: String,
) {
    let mut logs = channel_logs.write().await;
    if let Some(entries) = logs.get_mut(channel_id) {
        entries.push(ChannelLogEntry {
            timestamp: Utc::now(),
            level: level.to_string(),
            message,
        });
    }
}

async fn run_message_loop(
    mut ws_sender: WsSender,
    mut ws_receiver: WsReceiver,
    mut cmd_rx: broadcast::Receiver<String>,
    pending_responses: Arc<RwLock<HashMap<String, oneshot::Sender<OneBotApiResponse>>>>,
    event_tx: broadcast::Sender<Value>,
    connections: Arc<RwLock<HashMap<String, WsConnection>>>,
    channel_logs: Arc<RwLock<HashMap<String, Vec<ChannelLogEntry>>>>,
    channel_id: String,
    peer_addr_str: String,
) {
    let mut disconnect_level = "info".to_string();
    let mut disconnect_message = format!("客户端断开: {}", peer_addr_str);
    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    Ok(payload) => {
                        if ws_sender.send(Message::Text(payload.into())).await.is_err() {
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
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(value) = serde_json::from_str::<Value>(&text) {
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
                    Some(Ok(Message::Ping(data))) => {
                        let _ = ws_sender.send(Message::Pong(data)).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        eprintln!("[NapCat WS] 接收错误: {}", e);
                        disconnect_level = "error".to_string();
                        disconnect_message = format!("接收错误: {}", e);
                        break;
                    }
                }
            }
        }
    }

    eprintln!("[NapCat WS] 渠道 {} 客户端断开: {}", channel_id, peer_addr_str);
    append_channel_log(&channel_logs, &channel_id, &disconnect_level, disconnect_message).await;
    connections.write().await.remove(&channel_id);
}

impl NapcatWsManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            channel_shutdowns: Arc::new(RwLock::new(HashMap::new())),
            channel_logs: Arc::new(RwLock::new(HashMap::new())),
            listen_addrs: Arc::new(RwLock::new(HashMap::new())),
            channel_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 添加日志
    pub(crate) async fn add_log(&self, channel_id: &str, level: &str, message: &str) {
        let entry = ChannelLogEntry {
            timestamp: Utc::now(),
            level: level.to_string(),
            message: message.to_string(),
        };
        let mut logs = self.channel_logs.write().await;
        let channel_logs = logs.entry(channel_id.to_string()).or_insert_with(Vec::new);
        channel_logs.push(entry);
        // 只保留最近 100 条日志
        if channel_logs.len() > 100 {
            let start = channel_logs.len() - 100;
            channel_logs.drain(0..start);
        }
    }

    /// 停止单个渠道的 WebSocket 服务器
    pub async fn stop_channel(&self, channel_id: &str) -> Result<(), String> {
        // 发送关闭信号给该渠道的 accept 循环
        if let Some(tx) = self.channel_shutdowns.write().await.remove(channel_id) {
            let _ = tx.send(());
        }
        // 等待旧的 accept 循环任务退出，确保端口被释放
        if let Some(handle) = self.channel_tasks.write().await.remove(channel_id) {
            let mut handle = handle;
            match tokio::time::timeout(Duration::from_secs(8), &mut handle).await {
                Ok(join_result) => {
                    if let Err(err) = join_result {
                        return Err(format!("停止渠道任务失败: {}", err));
                    }
                }
                Err(_) => {
                    handle.abort();
                    let _ = handle.await;
                    return Err(format!("停止渠道超时: {}", channel_id));
                }
            }
        }
        // 清除连接和监听地址
        self.connections.write().await.remove(channel_id);
        self.listen_addrs.write().await.remove(channel_id);
        self.add_log(channel_id, "info", "渠道服务器已停止").await;
        Ok(())
    }

    /// 依据渠道配置原子地收敛运行态：先停，再按 enabled/platform 决定是否启动
    pub(crate) async fn reconcile_channel_runtime(
        &self,
        channel: &RemoteImChannelConfig,
    ) -> Result<(), String> {
        self.stop_channel(&channel.id).await?;
        if channel.enabled && channel.platform == RemoteImPlatform::OnebotV11 {
            let credentials = NapcatCredentials::from_credentials(&channel.credentials);
            self.start(channel.id.clone(), credentials).await?;
        }
        Ok(())
    }

    /// 获取渠道连接状态
    pub async fn get_connection_status(&self, channel_id: &str) -> ChannelConnectionStatus {
        let connections = self.connections.read().await;
        let listen_addrs = self.listen_addrs.read().await;

        if let Some(conn) = connections.get(channel_id) {
            ChannelConnectionStatus {
                channel_id: channel_id.to_string(),
                connected: true,
                peer_addr: conn.peer_addr.clone(),
                connected_at: conn.connected_at,
                listen_addr: listen_addrs.get(channel_id).cloned().unwrap_or_default(),
            }
        } else {
            ChannelConnectionStatus {
                channel_id: channel_id.to_string(),
                connected: false,
                peer_addr: None,
                connected_at: None,
                listen_addr: listen_addrs.get(channel_id).cloned().unwrap_or_default(),
            }
        }
    }

    /// 获取渠道日志
    pub async fn get_logs(&self, channel_id: &str) -> Vec<ChannelLogEntry> {
        let logs = self.channel_logs.read().await;
        logs.get(channel_id).cloned().unwrap_or_default()
    }

    /// 启动 WebSocket 服务器
    pub async fn start(
        &self,
        channel_id: String,
        credentials: NapcatCredentials,
    ) -> Result<(), String> {
        let addr: SocketAddr = format!("{}:{}", credentials.ws_host, credentials.ws_port)
            .parse()
            .map_err(|e| format!("无效地址: {}", e))?;

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("绑定端口失败: {}", e))?;

        let listen_addr = addr.to_string();
        eprintln!("[NapCat WS] 渠道 {} 监听 {}", channel_id, listen_addr);

        // 记录监听地址
        self.listen_addrs.write().await.insert(channel_id.clone(), listen_addr.clone());
        self.add_log(&channel_id, "info", &format!("服务器启动，监听 {}", listen_addr)).await;

        let connections = self.connections.clone();
        let channel_logs = self.channel_logs.clone();

        // 创建 per-channel 的关闭信号
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        self.channel_shutdowns.write().await.insert(channel_id.clone(), shutdown_tx.clone());
        let mut shutdown_rx = shutdown_tx.subscribe();

        // 创建连接通道
        let (conn_tx, _) = broadcast::channel::<String>(64);
        let (event_tx, _) = broadcast::channel::<Value>(256);
        let pending_responses = Arc::new(RwLock::new(HashMap::<String, oneshot::Sender<OneBotApiResponse>>::new()));

        let saved_channel_id = channel_id.clone();
        let task_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        eprintln!("[NapCat WS] 渠道 {} 收到关闭信号", channel_id);
                        let mut logs = channel_logs.write().await;
                        if let Some(l) = logs.get_mut(&channel_id) {
                            l.push(ChannelLogEntry {
                                timestamp: Utc::now(),
                                level: "info".to_string(),
                                message: "服务器关闭".to_string(),
                            });
                        }
                        break;
                    }
                    accept = listener.accept() => {
                        match accept {
                            Ok((stream, peer_addr)) => {
                                let channel_id = channel_id.clone();
                                let expected_token = credentials.ws_token.clone();
                                let conn_tx = conn_tx.clone();
                                let pending_responses = pending_responses.clone();
                                let event_tx = event_tx.clone();
                                let connections = connections.clone();
                                let channel_logs = channel_logs.clone();

                                tokio::spawn(async move {
                                    Self::handle_connection(
                                        stream,
                                        peer_addr,
                                        channel_id.clone(),
                                        expected_token,
                                        conn_tx,
                                        pending_responses,
                                        event_tx,
                                        connections,
                                        channel_logs,
                                    ).await;
                                });
                            }
                            Err(e) => {
                                eprintln!("[NapCat WS] 接受连接失败: {}", e);
                                let mut logs = channel_logs.write().await;
                                if let Some(l) = logs.get_mut(&channel_id) {
                                    l.push(ChannelLogEntry {
                                        timestamp: Utc::now(),
                                        level: "error".to_string(),
                                        message: format!("接受连接失败: {}", e),
                                    });
                                }
                            }
                        }
                    }
                }
            }
            // listener 在此处 drop，释放端口绑定
        });
        self.channel_tasks.write().await.insert(saved_channel_id, task_handle);

        Ok(())
    }

    async fn handle_connection(
        stream: tokio::net::TcpStream,
        peer_addr: SocketAddr,
        channel_id: String,
        expected_token: Option<String>,
        conn_tx: broadcast::Sender<String>,
        pending_responses: Arc<RwLock<HashMap<String, oneshot::Sender<OneBotApiResponse>>>>,
        event_tx: broadcast::Sender<Value>,
        connections: Arc<RwLock<HashMap<String, WsConnection>>>,
        channel_logs: Arc<RwLock<HashMap<String, Vec<ChannelLogEntry>>>>,
    ) {
        let expected_for_closure = expected_token.clone();
        let ws_result = accept_hdr_async(stream, move |req: &Request, response: Response| {
            if let Err(mut err_response) = validate_ws_token(req, expected_for_closure.as_deref()) {
                *err_response.headers_mut() = response.headers().clone();
                return Err(err_response);
            }
            Ok(response)
        })
        .await;

        let ws_stream = match ws_result {
            Ok(ws) => ws,
            Err(e) => {
                eprintln!("[NapCat WS] WebSocket 握手失败 {}: {}", peer_addr, e);
                append_channel_log(
                    &channel_logs,
                    &channel_id,
                    "error",
                    format!("WebSocket 握手失败 {}: {}", peer_addr, e),
                )
                .await;
                return;
            }
        };

        let peer_addr_str = peer_addr.to_string();
        eprintln!("[NapCat WS] 渠道 {} 客户端已连接: {}", channel_id, peer_addr_str);

        append_channel_log(
            &channel_logs,
            &channel_id,
            "info",
            format!("客户端已连接: {}", peer_addr_str),
        )
        .await;

        // 更新连接状态
        {
            let conn = WsConnection {
                tx: conn_tx.clone(),
                pending_responses: pending_responses.clone(),
                event_tx: event_tx.clone(),
                peer_addr: Some(peer_addr_str.clone()),
                connected_at: Some(Utc::now()),
            };
            connections.write().await.insert(channel_id.clone(), conn);
        }

        let (ws_sender, ws_receiver) = ws_stream.split();
        let cmd_rx = conn_tx.subscribe();
        run_message_loop(
            ws_sender,
            ws_receiver,
            cmd_rx,
            pending_responses,
            event_tx,
            connections,
            channel_logs,
            channel_id,
            peer_addr_str,
        )
        .await;
    }

    /// 调用 OneBot API 并等待响应
    pub async fn call_api(
        &self,
        channel_id: &str,
        action: &str,
        params: Value,
        timeout_ms: u64,
    ) -> Result<Value, String> {
        let connections = self.connections.read().await;
        let conn = connections
            .get(channel_id)
            .ok_or_else(|| format!("渠道 {} 未连接", channel_id))?;
        let pending_responses = conn.pending_responses.clone();

        // 生成唯一 echo
        let echo = uuid::Uuid::new_v4().to_string();

        // 创建响应等待通道
        let (tx, rx) = oneshot::channel();
        pending_responses.write().await.insert(echo.clone(), tx);

        // 构建请求
        let request = OneBotApiRequest {
            action: action.to_string(),
            params,
            echo: Some(serde_json::json!(echo.clone())),
        };

        let payload = serde_json::to_string(&request)
            .map_err(|e| format!("序列化请求失败: {}", e))?;

        // 发送请求
        conn.tx
            .send(payload)
            .map_err(|e| format!("发送失败: {}", e))?;

        // 释放连接锁，等待响应
        drop(connections);

        // 等待响应或超时
        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            rx
        ).await;

        match result {
            Ok(Ok(response)) => {
                if response.status == "ok" {
                    Ok(response.data)
                } else {
                    Err(format!("API 调用失败: status={}, retcode={}", response.status, response.retcode))
                }
            }
            Ok(Err(_)) => Err("响应通道已关闭".to_string()),
            Err(_) => {
                // 超时，清理 pending
                pending_responses.write().await.remove(&echo);
                Err(format!("API 调用超时 ({}ms)", timeout_ms))
            }
        }
    }

    /// 订阅事件流
    pub async fn subscribe_events(&self, channel_id: &str) -> Option<broadcast::Receiver<Value>> {
        let connections = self.connections.read().await;
        connections.get(channel_id).map(|c| c.event_tx.subscribe())
    }

    /// 关闭所有服务器
    pub async fn shutdown(&self) {
        let shutdowns = self.channel_shutdowns.write().await.drain().collect::<Vec<_>>();
        for (_, tx) in shutdowns {
            let _ = tx.send(());
        }
    }

    /// 检查渠道是否已连接
    pub async fn is_connected(&self, channel_id: &str) -> bool {
        self.connections.read().await.contains_key(channel_id)
    }

    /// 订阅渠道的关闭信号
    pub(crate) async fn subscribe_shutdown(&self, channel_id: &str) -> Option<broadcast::Receiver<()>> {
        let shutdowns = self.channel_shutdowns.read().await;
        shutdowns.get(channel_id).map(|tx| tx.subscribe())
    }
}

impl Default for NapcatWsManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局 NapCat WebSocket 管理器
static NAPCAT_WS_MANAGER: once_cell::sync::Lazy<Arc<NapcatWsManager>> =
    once_cell::sync::Lazy::new(|| Arc::new(NapcatWsManager::new()));

pub fn napcat_ws_manager() -> Arc<NapcatWsManager> {
    NAPCAT_WS_MANAGER.clone()
}

/// 启动 NapCat WebSocket 服务器（同步版本，用于应用启动）
pub(crate) fn napcat_ws_server_start(
    channel: RemoteImChannelConfig,
    _app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let credentials = NapcatCredentials::from_credentials(&channel.credentials);
    let manager = napcat_ws_manager();
    let channel_id = channel.id.clone();

    // 通过 oneshot 等待异步启动结果，确保错误能传递给调用方
    let (tx, rx) = std::sync::mpsc::channel();
    tauri::async_runtime::spawn(async move {
        let result = manager.start(channel_id, credentials).await;
        if tx.send(result).is_err() {
            eprintln!("[NapCat WS] 启动结果回传失败：接收端已关闭");
        }
    });

    match rx.recv_timeout(Duration::from_secs(8)) {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            Err("启动 NapCat WS 服务超时，异步任务未及时返回".to_string())
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err("启动任务通道关闭".to_string())
        }
    }
}

/// 获取渠道连接状态
pub async fn get_channel_connection_status(channel_id: String) -> Result<ChannelConnectionStatus, String> {
    let manager = napcat_ws_manager();
    Ok(manager.get_connection_status(&channel_id).await)
}

/// 获取渠道日志
pub async fn get_channel_logs(channel_id: String) -> Result<Vec<ChannelLogEntry>, String> {
    let manager = napcat_ws_manager();
    Ok(manager.get_logs(&channel_id).await)
}

// ==================== OneBot v11 事件消费 ====================

/// 从 OneBot v11 message 数组格式中提取文本和图片 URL
fn parse_onebot_message_array(segments: &[Value]) -> (String, Vec<String>) {
    let mut texts = Vec::new();
    let mut image_urls = Vec::new();

    for seg in segments {
        let seg_type = seg.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let data = seg.get("data");
        match seg_type {
            "text" => {
                if let Some(text) = data.and_then(|d| d.get("text")).and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        texts.push(text.to_string());
                    }
                }
            }
            "image" => {
                if let Some(url) = data.and_then(|d| d.get("url")).and_then(|v| v.as_str()) {
                    if !url.trim().is_empty() {
                        image_urls.push(url.to_string());
                    }
                }
            }
            "at" => {
                let qq = data
                    .and_then(|d| d.get("qq"))
                    .and_then(|v| v.as_str().map(String::from).or_else(|| v.as_u64().map(|n| n.to_string())));
                if let Some(qq) = qq {
                    texts.push(format!("@{}", qq));
                }
            }
            "face" => {
                let face_id = data
                    .and_then(|d| d.get("id"))
                    .and_then(|v| v.as_str().map(String::from).or_else(|| v.as_u64().map(|n| n.to_string())));
                if let Some(id) = face_id {
                    texts.push(format!("[表情:{}]", id));
                }
            }
            "reply" | "forward" | "record" | "video" | "file" | "poke" => {
                // 暂时以标签占位
                texts.push(format!("[{}]", seg_type));
            }
            _ => {}
        }
    }
    (texts.join(""), image_urls)
}

/// 从 CQ 码字符串中提取纯文本（剥离 [CQ:...] 标签）
fn parse_onebot_cq_string(raw: &str) -> String {
    // 简单正则方式：移除所有 [CQ:xxx] 标签
    let mut result = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch == '[' {
            // 检查是否是 CQ 码
            let rest: String = chars.clone().collect();
            if rest.starts_with("[CQ:") {
                // 跳过到 ]
                while let Some(c) = chars.next() {
                    if c == ']' {
                        break;
                    }
                }
                continue;
            }
        }
        result.push(ch);
        chars.next();
    }
    result
}

fn extract_message_content(event: &Value) -> (String, Vec<String>) {
    let message_field = event.get("message");
    if let Some(arr) = message_field.and_then(|v| v.as_array()) {
        let result = parse_onebot_message_array(arr);
        eprintln!(
            "[NapCat Event] 解析数组格式 message: text_len={}, images={}",
            result.0.len(),
            result.1.len()
        );
        return result;
    }
    if let Some(msg_str) = message_field.and_then(|v| v.as_str()) {
        let parsed = parse_onebot_cq_string(msg_str);
        eprintln!("[NapCat Event] 解析字符串格式 message: text=\"{}\"", parsed);
        return (parsed, Vec::new());
    }
    if let Some(raw) = event.get("raw_message").and_then(|v| v.as_str()) {
        let parsed = parse_onebot_cq_string(raw);
        eprintln!("[NapCat Event] 解析 raw_message: text=\"{}\"", parsed);
        return (parsed, Vec::new());
    }
    eprintln!(
        "[NapCat Event] message 字段类型未识别: {:?}",
        message_field.map(|v| v.to_string())
    );
    (String::new(), Vec::new())
}

async fn resolve_contact_info(
    event: &Value,
    manager: &NapcatWsManager,
    channel_id: &str,
) -> Result<(String, String, Option<String>), String> {
    let message_type = event
        .get("message_type")
        .and_then(|v| v.as_str())
        .unwrap_or("private");
    let user_id = event
        .get("user_id")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let group_id = event.get("group_id").and_then(|v| v.as_u64());
    if message_type == "group" {
        let gid = group_id.ok_or("群消息缺少 group_id")?;
        let group_name = match manager
            .call_api(channel_id, "get_group_info", serde_json::json!({"group_id": gid}), 5000)
            .await
        {
            Ok(info) => info
                .get("group_name")
                .and_then(|n| n.as_str())
                .map(String::from),
            Err(_) => None,
        };
        Ok(("group".to_string(), gid.to_string(), group_name))
    } else {
        Ok(("private".to_string(), user_id.to_string(), None))
    }
}

fn read_channel_config(
    state: &AppState,
    channel_id: &str,
) -> Result<Option<RemoteImChannelConfig>, String> {
    let config = state_read_config_cached(state)?;
    let channel_config = remote_im_channel_by_id(&config, channel_id).cloned();
    Ok(channel_config)
}

fn resolve_sender_name(event: &Value) -> String {
    let sender = event.get("sender");
    let sender_nickname = sender
        .and_then(|s| s.get("nickname"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let sender_card = sender
        .and_then(|s| s.get("card"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    sender_card.unwrap_or(sender_nickname).to_string()
}

fn message_field_kind(message_field: Option<&Value>) -> &'static str {
    message_field
        .map(|v| match v {
            Value::Array(_) => "array",
            Value::String(_) => "string",
            Value::Null => "null",
            Value::Object(_) => "object",
            Value::Number(_) => "number",
            Value::Bool(_) => "bool",
        })
        .unwrap_or("missing")
}

fn build_remote_im_enqueue_input(
    channel_id: &str,
    sender_name: String,
    user_id: u64,
    im_name: String,
    activate_assistant: Option<bool>,
    remote_contact_type: String,
    remote_contact_id: String,
    remote_contact_name: Option<String>,
    platform_message_id: Option<String>,
    final_text: String,
) -> RemoteImEnqueueInput {
    RemoteImEnqueueInput {
        channel_id: channel_id.to_string(),
        platform: RemoteImPlatform::OnebotV11,
        im_name,
        remote_contact_type,
        remote_contact_id,
        remote_contact_name,
        sender_id: user_id.to_string(),
        sender_name,
        sender_avatar_url: None,
        platform_message_id,
        dingtalk_session_webhook: None,
        dingtalk_session_webhook_expired_time: None,
        activate_assistant,
        session: SessionSelector {
            api_config_id: None,
            department_id: None,
            agent_id: String::new(),
            conversation_id: None,
        },
        payload: ChatInputPayload {
            text: Some(final_text),
            display_text: None,
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            provider_meta: None,
        },
    }
}

/// 解析 OneBot v11 message 事件并入队
async fn parse_and_enqueue_onebot_event(
    channel_id: &str,
    event: &Value,
    state: &AppState,
    manager: &NapcatWsManager,
) -> Result<RemoteImEnqueueResult, String> {
    eprintln!(
        "[NapCat Event][trace] channel_id={}, message_type={}, user_id={}, message_id={}",
        channel_id,
        event
            .get("message_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown"),
        event
            .get("user_id")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        event
            .get("message_id")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    let user_id = event.get("user_id").and_then(|v| v.as_u64()).unwrap_or(0);
    let sender_name = resolve_sender_name(event);
    let message_field = event.get("message");
    let (text, image_urls) = extract_message_content(event);
    if text.trim().is_empty() && image_urls.is_empty() {
        return Err(format!(
            "消息内容为空，跳过 (message_type={}, user_id={}, message_field_type={})",
            event
                .get("message_type")
                .and_then(|v| v.as_str())
                .unwrap_or("private"),
            user_id,
            message_field_kind(message_field)
        ));
    }

    let (remote_contact_type, remote_contact_id, mut remote_contact_name) =
        resolve_contact_info(event, manager, channel_id).await?;
    if remote_contact_type != "group" {
        remote_contact_name = Some(sender_name.clone());
    }
    let platform_message_id = event
        .get("message_id")
        .and_then(|v| v.as_u64())
        .map(|id| id.to_string())
        .or_else(|| {
            event
                .get("message_id")
                .and_then(|v| v.as_i64())
                .map(|id| id.to_string())
        });
    let channel_config = read_channel_config(state, channel_id)?;
    let im_name = channel_config
        .as_ref()
        .map(|ch| ch.name.clone())
        .unwrap_or_else(|| "NapCat".to_string());
    let activate_assistant = channel_config.as_ref().map(|ch| ch.activate_assistant);
    let final_text = if text.trim().is_empty() && !image_urls.is_empty() {
        format!("[图片x{}]", image_urls.len())
    } else {
        text
    };
    let input = build_remote_im_enqueue_input(
        channel_id,
        sender_name,
        user_id,
        im_name,
        activate_assistant,
        remote_contact_type,
        remote_contact_id,
        remote_contact_name,
        platform_message_id,
        final_text,
    );
    remote_im_enqueue_message_internal(input, state)
}

/// 启动 OneBot v11 事件消费循环
pub(crate) async fn napcat_start_event_consumer(
    channel_id: String,
    state: AppState,
) {
    let manager = napcat_ws_manager();

    loop {
        // 等待连接建立后才能订阅事件
        let (mut event_rx, mut shutdown_rx) = loop {
            if let Some(rx) = manager.subscribe_events(&channel_id).await {
                if let Some(srx) = manager.subscribe_shutdown(&channel_id).await {
                    break (rx, srx);
                }
            }
            // 连接尚未建立或渠道已停止，按节流间隔重试
            tokio::time::sleep(Duration::from_secs(NAPCAT_RECONNECT_INTERVAL_SECS)).await;
        };

        eprintln!("[NapCat Event] 渠道 {} 开始消费事件", channel_id);
        manager.add_log(&channel_id, "info", "事件消费器已启动").await;

        loop {
            tokio::select! {
                event_result = event_rx.recv() => {
                    match event_result {
                        Ok(event) => {
                            // 只处理 message 事件
                            if event.get("post_type").and_then(|v| v.as_str()) != Some("message") {
                                continue;
                            }

                            match parse_and_enqueue_onebot_event(&channel_id, &event, &state, &manager).await {
                                Ok(result) => {
                                    eprintln!("[NapCat Event] 渠道 {} 入队成功: event_id={}", channel_id, result.event_id);
                                }
                                Err(err) if err.contains("跳过") => {
                                    // 正常跳过（联系人未开启、内容为空等），仅输出调试日志，不写渠道日志
                                    eprintln!("[NapCat Event] 渠道 {} {}", channel_id, err);
                                }
                                Err(err) => {
                                    eprintln!("[NapCat Event] 渠道 {} 入队失败: {}", channel_id, err);
                                    manager.add_log(&channel_id, "warn", &format!("消息入队失败: {}", err)).await;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            eprintln!("[NapCat Event] 渠道 {} 落后 {} 条事件", channel_id, n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            eprintln!("[NapCat Event] 渠道 {} 事件通道关闭", channel_id);
                            break;
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    eprintln!("[NapCat Event] 渠道 {} 收到关闭信号，停止事件消费", channel_id);
                    manager.add_log(&channel_id, "info", "事件消费器已停止").await;
                    return; // 渠道已停止，完全退出消费循环
                }
            }
        }

        // 事件通道关闭（客户端断开），按节流间隔等待重连
        eprintln!("[NapCat Event] 渠道 {} 等待重新连接...", channel_id);
        tokio::time::sleep(Duration::from_secs(NAPCAT_RECONNECT_INTERVAL_SECS)).await;
    }
}
