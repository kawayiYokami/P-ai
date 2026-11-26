// ==================== 调度运行态：内存快照 → 单写者顺序落盘 ====================
//
// 队列与调度状态只活在内存里。谁改了它们，只报一个会话 id；写盘由这里唯一的
// 写者按快照顺序补上，调用方投完就走，不等磁盘。
//
// 快照是按会话整体覆盖的：同一会话连改一百次，落地只写最后一次，写盘次数与
// 操作次数脱钩。写者只有一个，天然串行，不需要靠锁来排次序。
//
// 用独立线程而不是 async 任务：退出链路要能同步排空它，不能依赖运行时还活着。

/// 一批待写的运行态：会话 id → Some(队列 JSON) 写这一行，None 删这一行。
struct PendingActiveDispatch {
    data_path: std::path::PathBuf,
    rows: std::collections::HashMap<String, Option<String>>,
}

struct ActiveDispatchPersistState {
    pending: Option<PendingActiveDispatch>,
    notified: bool,
}

struct ActiveDispatchPersistRuntime {
    state: std::sync::Mutex<ActiveDispatchPersistState>,
    notify: std::sync::Condvar,
    /// 后台写者与退出排空互斥，保证同一时刻只有一个在碰库。
    write_lock: std::sync::Mutex<()>,
    started: std::sync::atomic::AtomicBool,
}

fn active_dispatch_persist_runtime() -> &'static ActiveDispatchPersistRuntime {
    static RUNTIME: std::sync::OnceLock<ActiveDispatchPersistRuntime> =
        std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| ActiveDispatchPersistRuntime {
        state: std::sync::Mutex::new(ActiveDispatchPersistState {
            pending: None,
            notified: false,
        }),
        notify: std::sync::Condvar::new(),
        write_lock: std::sync::Mutex::new(()),
        started: std::sync::atomic::AtomicBool::new(false),
    })
}

/// 投一份快照：立即返回，不碰磁盘。
fn schedule_active_dispatch_persist(
    state: &AppState,
    conversation_id: &str,
    snapshot: Option<String>,
) {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return;
    }
    let data_path = state.data_path.clone();
    let runtime = active_dispatch_persist_runtime();
    match runtime.state.lock() {
        Ok(mut guard) => {
            let batch = guard
                .pending
                .get_or_insert_with(|| PendingActiveDispatch {
                    data_path,
                    rows: std::collections::HashMap::new(),
                });
            batch.data_path = state.data_path.clone();
            batch.rows.insert(conversation_id.to_string(), snapshot);
            guard.notified = true;
        }
        Err(_) => {
            runtime_log_warn(format!(
                "[聊天调度] 记录调度运行态失败 会话={conversation_id} 原因=待写队列不可用"
            ));
            return;
        }
    }
    runtime.notify.notify_one();
    ensure_active_dispatch_persist_worker();
}

/// 起唯一的写者。只起一次。
fn ensure_active_dispatch_persist_worker() {
    let runtime = active_dispatch_persist_runtime();
    if runtime
        .started
        .compare_exchange(
            false,
            true,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
        )
        .is_err()
    {
        return;
    }
    std::thread::Builder::new()
        .name("active-dispatch-persist".to_string())
        .spawn(move || loop {
            // 等第一次通知。
            match runtime.state.lock() {
                Ok(mut guard) => {
                    while !guard.notified {
                        guard = match runtime.notify.wait(guard) {
                            Ok(guard) => guard,
                            Err(_) => return,
                        };
                    }
                    guard.notified = false;
                }
                Err(_) => return,
            }
            // 去抖：把这一阵突发收进同一批。
            std::thread::sleep(std::time::Duration::from_millis(120));
            let batch = match runtime.state.lock() {
                Ok(mut guard) => guard.pending.take(),
                Err(_) => return,
            };
            let Some(batch) = batch else {
                continue;
            };
            write_active_dispatch_batch(&runtime, batch);
        })
        .map(|_| ())
        .unwrap_or_else(|err| {
            runtime_log_warn(format!("[聊天调度] 启动运行态后台写者失败 错误={err}"));
        });
}

fn write_active_dispatch_batch(
    runtime: &ActiveDispatchPersistRuntime,
    batch: PendingActiveDispatch,
) {
    #[cfg(test)]
    active_dispatch_write_counters_add(&batch.data_path);
    let _io_guard = runtime.write_lock.lock();
    for (conversation_id, snapshot) in batch.rows {
        let result = match snapshot {
            Some(json) => {
                message_store::active_dispatch_upsert(&batch.data_path, &conversation_id, &json)
            }
            None => message_store::active_dispatch_remove(&batch.data_path, &conversation_id),
        };
        if let Err(err) = result {
            runtime_log_warn(format!(
                "[聊天调度] 记录调度运行态失败 会话={conversation_id} 错误={err}"
            ));
        }
    }
}

/// 同步写完全部待写快照。退出前调用一次，测试里也用它把异步收敛成同步。
fn flush_active_dispatches_blocking() -> Result<bool, String> {
    let runtime = active_dispatch_persist_runtime();
    let mut wrote_anything = false;
    loop {
        let batch = {
            let mut guard = runtime
                .state
                .lock()
                .map_err(|_| "Failed to lock active dispatch persist".to_string())?;
            guard.pending.take()
        };
        let Some(batch) = batch else {
            break;
        };
        write_active_dispatch_batch(runtime, batch);
        wrote_anything = true;
    }
    Ok(wrote_anything)
}

#[cfg(test)]
fn active_dispatch_write_counters(
) -> &'static std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, usize>> {
    static COUNTERS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, usize>>,
    > = std::sync::OnceLock::new();
    COUNTERS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

#[cfg(test)]
fn active_dispatch_write_counters_add(data_path: &std::path::PathBuf) {
    if let Ok(mut counters) = active_dispatch_write_counters().lock() {
        *counters.entry(data_path.clone()).or_insert(0) += 1;
    }
}

#[cfg(test)]
fn reset_active_dispatch_write_count(data_path: &std::path::PathBuf) {
    if let Ok(mut counters) = active_dispatch_write_counters().lock() {
        counters.insert(data_path.clone(), 0);
    }
}

#[cfg(test)]
fn active_dispatch_write_count(data_path: &std::path::PathBuf) -> usize {
    active_dispatch_write_counters()
        .lock()
        .ok()
        .and_then(|counters| counters.get(data_path).copied())
        .unwrap_or(0)
}
