#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPortServiceLogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub message: String,
    /// 业务角色（库房/前台/秘书/专家）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// 仅用于联系人日志的内部筛选，不展示给用户。
    #[serde(default, skip_serializing)]
    pub contact_record_id: Option<String>,
}

pub type ChannelLogEntry = LocalPortServiceLogEntry;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPortServiceStatusSnapshot {
    pub listen_addr: String,
    #[serde(default)]
    pub status_text: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalPortServiceStartOutcome {
    Started,
    SkippedAlreadyRunning,
}

#[derive(Clone)]
pub struct LocalPortServiceCore {
    listen_addrs: Arc<RwLock<HashMap<String, String>>>,
    status_texts: Arc<RwLock<HashMap<String, String>>>,
    last_errors: Arc<RwLock<HashMap<String, String>>>,
    lifecycle_locks: Arc<tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

impl LocalPortServiceCore {
    pub fn new() -> Self {
        Self {
            listen_addrs: Arc::new(RwLock::new(HashMap::new())),
            status_texts: Arc::new(RwLock::new(HashMap::new())),
            last_errors: Arc::new(RwLock::new(HashMap::new())),
            lifecycle_locks: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    pub async fn lifecycle_guard(
        &self,
        service_id: &str,
    ) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = {
            let mut locks = self.lifecycle_locks.lock().await;
            locks
                .entry(service_id.to_string())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .clone()
        };
        lock.lock_owned().await
    }

    pub async fn start_serialized<IsRunningFuture, StartFn, StartFuture>(
        &self,
        service_id: &str,
        is_running: IsRunningFuture,
        start_fn: StartFn,
    ) -> Result<LocalPortServiceStartOutcome, String>
    where
        IsRunningFuture: std::future::Future<Output = bool>,
        StartFn: FnOnce() -> StartFuture,
        StartFuture: std::future::Future<Output = Result<(), String>>,
    {
        let _guard = self.lifecycle_guard(service_id).await;
        if is_running.await {
            return Ok(LocalPortServiceStartOutcome::SkippedAlreadyRunning);
        }
        start_fn().await?;
        Ok(LocalPortServiceStartOutcome::Started)
    }

    pub async fn stop_serialized<StopFn, StopFuture>(
        &self,
        service_id: &str,
        stop_fn: StopFn,
    ) -> Result<(), String>
    where
        StopFn: FnOnce() -> StopFuture,
        StopFuture: std::future::Future<Output = Result<(), String>>,
    {
        let _guard = self.lifecycle_guard(service_id).await;
        stop_fn().await
    }

    pub async fn restart_serialized<RestartFn, RestartFuture>(
        &self,
        service_id: &str,
        restart_fn: RestartFn,
    ) -> Result<(), String>
    where
        RestartFn: FnOnce() -> RestartFuture,
        RestartFuture: std::future::Future<Output = Result<(), String>>,
    {
        let _guard = self.lifecycle_guard(service_id).await;
        restart_fn().await
    }

    pub async fn reconcile_serialized<ReconcileFn, ReconcileFuture>(
        &self,
        service_id: &str,
        reconcile_fn: ReconcileFn,
    ) -> Result<(), String>
    where
        ReconcileFn: FnOnce() -> ReconcileFuture,
        ReconcileFuture: std::future::Future<Output = Result<(), String>>,
    {
        let _guard = self.lifecycle_guard(service_id).await;
        reconcile_fn().await
    }

    pub async fn add_log(&self, service_id: &str, level: &str, message: &str) {
        self.add_log_for_contact(service_id, level, message, None).await;
    }

    /// 渠道日志也只有一套：格式化后写唯一日志，渠道日志窗按 channel_id 过滤。
    pub async fn add_log_for_contact(
        &self,
        service_id: &str,
        level: &str,
        message: &str,
        contact_record_id: Option<&str>,
    ) {
        let channel_id = service_id.trim();
        let level = level.trim();
        let message = message.trim();
        if channel_id.is_empty() || level.is_empty() || message.is_empty() {
            return;
        }
        let contact_text = contact_record_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| format!("，contact_id={value}"))
            .unwrap_or_default();
        let line =
            format!("[渠道日志][{level}] channel_id={channel_id}{contact_text}，{message}");
        match level {
            "warn" => runtime_log_warn(line),
            "error" => {
                runtime_log_error(line)
            }
            _ => runtime_log_info(line),
        }
    }

    pub async fn get_logs(&self, service_id: &str) -> Vec<LocalPortServiceLogEntry> {
        // 从唯一日志按 channel_id 过滤：不再为每个渠道单独存一份缓冲。
        let channel_id = service_id.trim();
        if channel_id.is_empty() {
            return Vec::new();
        }
        // 用裸 channel_id 过滤，不挑字段名：日志里它可能出现在 channel_id=…、
        // 也可能出现在「联系人=名字(id@会话)」这类文本里。
        runtime_log_entries_containing(channel_id)
            .into_iter()
        .map(|(created_at, level, message)| LocalPortServiceLogEntry {
            timestamp: chrono::DateTime::parse_from_rfc3339(&created_at)
                .map(|value| value.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            level,
            message,
            role: None,
            contact_record_id: None,
        })
        .collect()
    }

    pub async fn set_listen_addr(&self, service_id: &str, listen_addr: Option<String>) {
        let mut listen_addrs = self.listen_addrs.write().await;
        match listen_addr.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) {
            Some(value) => {
                listen_addrs.insert(service_id.to_string(), value);
            }
            None => {
                listen_addrs.remove(service_id);
            }
        }
    }

    pub async fn get_listen_addr(&self, service_id: &str) -> Option<String> {
        self.listen_addrs.read().await.get(service_id).cloned()
    }

    pub async fn set_status_text(&self, service_id: &str, status_text: Option<String>) {
        let mut status_texts = self.status_texts.write().await;
        match status_text.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) {
            Some(value) => {
                status_texts.insert(service_id.to_string(), value);
            }
            None => {
                status_texts.remove(service_id);
            }
        }
    }

    pub async fn get_status_text(&self, service_id: &str) -> Option<String> {
        self.status_texts.read().await.get(service_id).cloned()
    }

    pub async fn set_last_error(&self, service_id: &str, last_error: Option<String>) {
        let mut last_errors = self.last_errors.write().await;
        match last_error.map(|value| value.trim().to_string()).filter(|value| !value.is_empty()) {
            Some(value) => {
                last_errors.insert(service_id.to_string(), value);
            }
            None => {
                last_errors.remove(service_id);
            }
        }
    }

    pub async fn get_last_error(&self, service_id: &str) -> Option<String> {
        self.last_errors.read().await.get(service_id).cloned()
    }

    pub async fn status_snapshot(&self, service_id: &str) -> LocalPortServiceStatusSnapshot {
        LocalPortServiceStatusSnapshot {
            listen_addr: self.get_listen_addr(service_id).await.unwrap_or_default(),
            status_text: self.get_status_text(service_id).await,
            last_error: self.get_last_error(service_id).await,
        }
    }

    pub async fn clear_runtime_state(&self, service_id: &str) {
        self.listen_addrs.write().await.remove(service_id);
        self.status_texts.write().await.remove(service_id);
        self.last_errors.write().await.remove(service_id);
    }

    pub async fn runtime_state_is_clear(&self, service_id: &str) -> bool {
        !self.listen_addrs.read().await.contains_key(service_id)
            && !self.status_texts.read().await.contains_key(service_id)
            && !self.last_errors.read().await.contains_key(service_id)
    }
}

impl Default for LocalPortServiceCore {
    fn default() -> Self {
        Self::new()
    }
}
