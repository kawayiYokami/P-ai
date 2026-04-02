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

#[derive(Clone)]
struct ConversationLockOwnerSnapshot {
    task_name: String,
    acquired_at: std::time::Instant,
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
    fn lock(&self) -> std::sync::LockResult<TimedConversationLockGuard<'_>> {
        let location = std::panic::Location::caller();
        let task_name = format!("{}:{}", location.file(), location.line());
        self.lock_named(&task_name)
    }

    fn lock_named(&self, task_name: &str) -> std::sync::LockResult<TimedConversationLockGuard<'_>> {
        let wait_started_at = std::time::Instant::now();
        let owner_before_wait = match self.inner.try_lock() {
            Ok(guard) => {
                return Ok(self.build_guard(guard, task_name.to_string()));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                self.owner.lock().ok().and_then(|owner| owner.clone())
            }
            Err(std::sync::TryLockError::Poisoned(_)) => None,
        };
        let guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                return Err(std::sync::PoisonError::new(
                    self.build_guard(poisoned.into_inner(), task_name.to_string()),
                ));
            }
        };
        let waited_ms = wait_started_at.elapsed().as_millis();
        if waited_ms >= CONVERSATION_LOCK_SLOW_WAIT_MS {
            if let Some(owner) = owner_before_wait {
                let owner_held_ms = owner.acquired_at.elapsed().as_millis();
                runtime_log_debug(format!(
                    "[会话锁] 等待完成: task={}, waited_ms={}, owner={}, owner_held_ms={}",
                    task_name, waited_ms, owner.task_name, owner_held_ms
                ));
            } else {
                runtime_log_debug(format!(
                    "[会话锁] 等待完成: task={}, waited_ms={}",
                    task_name, waited_ms
                ));
            }
        }
        Ok(self.build_guard(guard, task_name.to_string()))
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
            });
        }
        TimedConversationLockGuard {
            task_name,
            acquired_at,
            lock: self,
            _guard: guard,
        }
    }
}

struct TimedConversationLockGuard<'a> {
    task_name: String,
    acquired_at: std::time::Instant,
    lock: &'a ConversationDomainLock,
    _guard: std::sync::MutexGuard<'a, ()>,
}

impl Drop for TimedConversationLockGuard<'_> {
    fn drop(&mut self) {
        let held_ms = self.acquired_at.elapsed().as_millis();
        if let Ok(mut owner) = self.lock.owner.lock() {
            owner.take();
        }
        if held_ms >= CONVERSATION_LOCK_SLOW_HOLD_MS {
            runtime_log_debug(format!(
                "[会话锁] 持有完成: task={}, held_ms={}",
                self.task_name, held_ms
            ));
        }
    }
}

fn lock_conversation_with_metrics<'a>(
    state: &'a AppState,
    task_name: &str,
) -> Result<TimedConversationLockGuard<'a>, String> {
    state
        .conversation_lock
        .lock_named(task_name)
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))
}
