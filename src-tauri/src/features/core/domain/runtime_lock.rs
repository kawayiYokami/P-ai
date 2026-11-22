static LAST_PANIC_SNAPSHOT_SLOT: OnceLock<Arc<Mutex<Option<String>>>> = OnceLock::new();

fn init_last_panic_snapshot_slot(slot: Arc<Mutex<Option<String>>>) {
    let _ = LAST_PANIC_SNAPSHOT_SLOT.set(slot);
}

fn last_panic_snapshot_text() -> String {
    LAST_PANIC_SNAPSHOT_SLOT
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|v| v.clone()))
        .unwrap_or_default()
}

fn state_lock_error_with_panic(
    file: &str,
    line: u32,
    module_path: &str,
    err: &dyn std::fmt::Display,
) -> String {
    let panic_snapshot = last_panic_snapshot_text();
    if panic_snapshot.trim().is_empty() {
        return format!(
            "无法获取状态锁：{}（位置：{}:{} 模块：{}）",
            err, file, line, module_path
        );
    }
    format!(
        "无法获取状态锁：{}（位置：{}:{} 模块：{}；最近 panic：{}）",
        err, file, line, module_path, panic_snapshot
    )
}

fn named_lock_error(
    lock_name: &str,
    file: &str,
    line: u32,
    module_path: &str,
    err: &dyn std::fmt::Display,
) -> String {
    format!(
        "无法获取 {} 锁：{}（位置：{}:{} 模块：{}）",
        lock_name, err, file, line, module_path
    )
}

const CONVERSATION_LOCK_SLOW_WAIT_MS: u128 = 20;
const CONVERSATION_LOCK_SLOW_HOLD_MS: u128 = 20;
const CONVERSATION_LOCK_WAIT_LOG_FIRST_MS: u128 = 200;
const CONVERSATION_LOCK_WAIT_LOG_REPEAT_MS: u128 = 1000;
const CONVERSATION_LOCK_MAX_WAIT_MS: u128 = 3000;

#[derive(Clone)]
struct ConversationLockOwnerSnapshot {
    task_name: String,
    acquired_at: std::time::Instant,
    thread: String,
}

struct ConversationDomainLock {
    inner: Mutex<()>,
    owner: Mutex<Option<ConversationLockOwnerSnapshot>>,
}

impl ConversationDomainLock {
    fn new() -> Self {
        Self {
            inner: Mutex::new(()),
            owner: Mutex::new(None),
        }
    }

    #[track_caller]
    fn lock(&self) -> Result<TimedConversationLockGuard<'_>, String> {
        let location = std::panic::Location::caller();
        let task_name = format!("{}:{}", location.file(), location.line());
        self.lock_named(&task_name)
    }

    fn lock_named(&self, task_name: &str) -> Result<TimedConversationLockGuard<'_>, String> {
        let wait_started_at = std::time::Instant::now();
        let waiter_thread = current_thread_descriptor();
        let mut next_wait_log_ms = CONVERSATION_LOCK_WAIT_LOG_FIRST_MS;
        let owner_before_wait = match self.inner.try_lock() {
            Ok(guard) => {
                return Ok(self.build_guard(guard, task_name.to_string()));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                self.owner.lock().ok().and_then(|owner| owner.clone())
            }
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err("conversation lock poisoned".to_string());
            }
        };

        loop {
            std::thread::sleep(std::time::Duration::from_millis(10));
            match self.inner.try_lock() {
                Ok(guard) => {
                    let waited_ms = wait_started_at.elapsed().as_millis();
                    if waited_ms >= CONVERSATION_LOCK_SLOW_WAIT_MS {
                        if let Some(owner) = owner_before_wait {
                            let owner_held_ms = owner.acquired_at.elapsed().as_millis();
                            runtime_log_debug(format!(
                                "[聊天] 会话锁 等待完成 等待任务={} 等待线程={} 等待毫秒={} 占用任务={} 占用线程={} 占用毫秒={}",
                                task_name,
                                waiter_thread,
                                waited_ms,
                                owner.task_name,
                                owner.thread,
                                owner_held_ms
                            ));
                        } else {
                            runtime_log_debug(format!(
                                "[聊天] 会话锁 等待完成 等待任务={} 等待线程={} 等待毫秒={}",
                                task_name, waiter_thread, waited_ms
                            ));
                        }
                    }
                    return Ok(self.build_guard(guard, task_name.to_string()));
                }
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    return Err("conversation lock poisoned".to_string());
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    let waited_ms = wait_started_at.elapsed().as_millis();
                    if waited_ms >= CONVERSATION_LOCK_MAX_WAIT_MS {
                        let owner = self.owner.lock().ok().and_then(|owner| owner.clone());
                        let message = if let Some(owner) = owner {
                            let owner_held_ms = owner.acquired_at.elapsed().as_millis();
                            format!(
                                "会话锁等待超时，等待任务={}，等待线程={}，等待毫秒={}，占用任务={}，占用线程={}，占用毫秒={}",
                                task_name,
                                waiter_thread,
                                waited_ms,
                                owner.task_name,
                                owner.thread,
                                owner_held_ms
                            )
                        } else {
                            format!(
                                "会话锁等待超时，等待任务={}，等待线程={}，等待毫秒={}",
                                task_name, waiter_thread, waited_ms
                            )
                        };
                        runtime_log_error(format!("[聊天] 会话锁 失败 任务=等待会话锁 错误={message}"));
                        return Err(message);
                    }
                    if waited_ms >= next_wait_log_ms {
                        let owner = self.owner.lock().ok().and_then(|owner| owner.clone());
                        if let Some(owner) = owner {
                            let owner_held_ms = owner.acquired_at.elapsed().as_millis();
                            runtime_log_warn(format!(
                                "[聊天] 会话锁 等待中 等待任务={} 等待线程={} 等待毫秒={} 占用任务={} 占用线程={} 占用毫秒={}",
                                task_name,
                                waiter_thread,
                                waited_ms,
                                owner.task_name,
                                owner.thread,
                                owner_held_ms
                            ));
                        } else {
                            runtime_log_warn(format!(
                                "[聊天] 会话锁 等待中 等待任务={} 等待线程={} 等待毫秒={}",
                                task_name, waiter_thread, waited_ms
                            ));
                        }
                        next_wait_log_ms += CONVERSATION_LOCK_WAIT_LOG_REPEAT_MS;
                    }
                }
            }
        }
    }

    fn build_guard<'a>(
        &'a self,
        guard: std::sync::MutexGuard<'a, ()>,
        task_name: String,
    ) -> TimedConversationLockGuard<'a> {
        let acquired_at = std::time::Instant::now();
        if let Ok(mut owner) = self.owner.lock() {
            *owner = Some(ConversationLockOwnerSnapshot {
                task_name: task_name.clone(),
                acquired_at,
                thread: current_thread_descriptor(),
            });
        }
        TimedConversationLockGuard {
            task_name,
            acquired_at,
            thread: current_thread_descriptor(),
            lock: self,
            _guard: guard,
        }
    }
}

struct TimedConversationLockGuard<'a> {
    task_name: String,
    acquired_at: std::time::Instant,
    thread: String,
    lock: &'a ConversationDomainLock,
    _guard: std::sync::MutexGuard<'a, ()>,
}

static CONVERSATION_MUTATION_GATES: OnceLock<Mutex<std::collections::HashMap<String, std::sync::Weak<ConversationMutationGate>>>> = OnceLock::new();

struct ConversationMutationGate {
    inner: parking_lot::ReentrantMutex<()>,
}

impl ConversationMutationGate {
    fn lock(&self) -> Result<parking_lot::ReentrantMutexGuard<'_, ()>, String> {
        Ok(self.inner.lock())
    }

    fn lock_named<'a>(
        &'a self,
        conversation_id: &str,
        task_name: &str,
    ) -> Result<TimedConversationMutationGuard<'a>, String> {
        let started_at = std::time::Instant::now();
        let waiter_thread = current_thread_descriptor();
        let mut next_wait_log_ms = CONVERSATION_LOCK_WAIT_LOG_FIRST_MS;
        loop {
            if let Some(guard) = self.inner.try_lock() {
                let waited_ms = started_at.elapsed().as_millis();
                if waited_ms >= CONVERSATION_LOCK_SLOW_WAIT_MS {
                    runtime_log_debug(format!(
                        "[聊天] 会话写入门 等待完成 任务={} 线程={} 等待毫秒={}",
                        task_name, waiter_thread, waited_ms
                    ));
                }
                return Ok(TimedConversationMutationGuard {
                    task_name: task_name.to_string(),
                    conversation_id: conversation_id.to_string(),
                    acquired_at: std::time::Instant::now(),
                    thread: waiter_thread,
                    _guard: guard,
                });
            }
            let waited_ms = started_at.elapsed().as_millis();
            if waited_ms >= next_wait_log_ms {
                runtime_log_warn(format!(
                    "[聊天] 会话写入门 等待中 任务={} 线程={} 等待毫秒={}",
                    task_name, waiter_thread, waited_ms
                ));
                next_wait_log_ms += CONVERSATION_LOCK_WAIT_LOG_REPEAT_MS;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

struct TimedConversationMutationGuard<'a> {
    task_name: String,
    conversation_id: String,
    acquired_at: std::time::Instant,
    thread: String,
    _guard: parking_lot::ReentrantMutexGuard<'a, ()>,
}

impl Drop for TimedConversationMutationGuard<'_> {
    fn drop(&mut self) {
        let held_ms = self.acquired_at.elapsed().as_millis();
        if held_ms >= CONVERSATION_LOCK_SLOW_HOLD_MS {
            runtime_log_debug(format!(
                "[聊天] 会话写入门 持有完成 任务={} 线程={} 持有毫秒={}",
                self.task_name, self.thread, held_ms
            ));
        }
    }
}

fn with_conversation_mutation_for_data_path<T, F>(
    data_path: &std::path::PathBuf,
    conversation_id: &str,
    task_name: &str,
    f: F,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    let normalized_conversation_id = conversation_id.trim();
    let mutation_gate = conversation_mutation_gate(data_path, normalized_conversation_id)?;
    let _guard = mutation_gate.lock_named(normalized_conversation_id, task_name)?;
    f()
}

fn with_conversation_mutation<T, F>(
    state: &AppState,
    conversation_id: &str,
    task_name: &str,
    f: F,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String>,
{
    with_conversation_mutation_for_data_path(&state.data_path, conversation_id, task_name, f)
}

/// 异步版会话写入门：将「获取会话锁 + 执行同步 mutation 闭包」整体放入阻塞线程池，
/// 避免 sqlite/文件 I/O 阻塞 async runtime worker；同一 conversation_id 的写入顺序
/// 仍由会话锁在线程池内串行保证。join 错误会传播为可读错误。
async fn with_conversation_mutation_async<T, F>(
    state: AppState,
    conversation_id: String,
    task_name: String,
    f: F,
) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        with_conversation_mutation(&state, &conversation_id, &task_name, f)
    })
    .await
    .map_err(|err| format!("会话写入门任务失败：{err}"))?
}

fn conversation_mutation_gate(
    data_path: &std::path::PathBuf,
    conversation_id: &str,
) -> Result<std::sync::Arc<ConversationMutationGate>, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Err("conversationId is required.".to_string());
    }
    let key = format!("{}:{}", data_path.display(), conversation_id);
    let gates = CONVERSATION_MUTATION_GATES
        .get_or_init(|| Mutex::new(std::collections::HashMap::new()));
    let mut gates = gates
        .lock()
        .map_err(|err| named_lock_error("conversation_mutation_gates", file!(), line!(), module_path!(), &err))?;
    gates.retain(|_, gate| gate.strong_count() > 0);
    if let Some(gate) = gates.get(&key).and_then(std::sync::Weak::upgrade) {
        return Ok(gate);
    }
    let gate = std::sync::Arc::new(ConversationMutationGate {
        inner: parking_lot::ReentrantMutex::new(()),
    });
    gates.insert(key, std::sync::Arc::downgrade(&gate));
    Ok(gate)
}

impl Drop for TimedConversationLockGuard<'_> {
    fn drop(&mut self) {
        let held_ms = self.acquired_at.elapsed().as_millis();
        if let Ok(mut owner) = self.lock.owner.lock() {
            owner.take();
        }
        if held_ms >= CONVERSATION_LOCK_SLOW_HOLD_MS {
            runtime_log_debug(format!(
                "[聊天] 会话锁 持有完成 任务={} 线程={} 持有毫秒={}",
                self.task_name, self.thread, held_ms
            ));
        }
    }
}

fn current_thread_descriptor() -> String {
    // 只取线程名。原先拼了 `:{:?}` 的 ThreadId，输出成 `tokio-rt-worker:ThreadId(4698)`，
    // 那是调试器里的东西，写进 INFO 日志没人看得懂。
    std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_string()
}

#[track_caller]
fn lock_conversation_with_metrics<'a>(
    state: &'a AppState,
    task_name: &str,
) -> Result<TimedConversationLockGuard<'a>, String> {
    let location = std::panic::Location::caller();
    let task_name = format!("{} @ {}:{}", task_name, location.file(), location.line());
    state
        .conversation_lock
        .lock_named(&task_name)
        .map_err(|err| {
            state_lock_error_with_panic(location.file(), location.line(), module_path!(), &err)
        })
}
