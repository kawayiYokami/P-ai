// ==================== 平台同步调用限时执行 ====================
//
// operate 执行路径上的 UIA 调用（元素树扫描、元素名寻址、pattern 调用）是跨进程同步 COM 调用。
// 目标进程无响应时（例如线程被调试器断点挂起），这类调用会长时间不返回，带来两个后果：
// 1. 直接在当前 async 任务上调用会占死 Tokio 工作线程，且 future 停在同步代码里，
//    工具组中断（Abortable）没有 await 点可观察中止信号，停止按钮形同空操作；
// 2. 没有超时边界，桌面脚本会永久挂起。
// 统一改为「阻塞线程池执行 + 超时」：超时按读取失败降级返回，同时把主流程保持在 await 点，
// 用户中断得以在任何 await 处生效。

/// UIA 调用单次等待上限：正常扫描数百毫秒，取 3 秒既覆盖元素较多的大窗口，也不让打断延迟过长。
const UI_CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// 在阻塞线程池执行同步平台调用并施加统一超时。
/// 返回 None 表示超时或阻塞任务异常退出（两种情况都已记日志），调用方按各自的「读取失败」语义降级。
/// 超时后阻塞线程可能仍堵在目标进程上无法回收，这是本方案的固有代价。
async fn run_ui_call_timed<T, F>(label: &str, f: F) -> Option<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    run_blocking_with_timeout(label, UI_CALL_TIMEOUT, f).await
}

/// 限时执行的内核：独立出 `limit` 参数，方便针对超时行为做单元测试。
async fn run_blocking_with_timeout<T, F>(label: &str, limit: std::time::Duration, f: F) -> Option<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    match tokio::time::timeout(limit, tokio::task::spawn_blocking(f)).await {
        Ok(Ok(value)) => Some(value),
        Ok(Err(err)) => {
            runtime_log_warn(format!(
                "[桌面脚本] 平台调用任务异常，任务=run_ui_call_timed，label={label}，原因={err}"
            ));
            None
        }
        Err(_) => {
            runtime_log_warn(format!(
                "[桌面脚本] 平台调用超时，任务=run_ui_call_timed，label={label}，超时毫秒={}；目标窗口可能无响应，本次按读取失败处理",
                limit.as_millis()
            ));
            None
        }
    }
}

#[cfg(test)]
mod operate_blocking_tests {
    use super::*;

    #[tokio::test]
    async fn blocking_call_returns_value_within_limit() {
        let value = run_blocking_with_timeout("test.fast", std::time::Duration::from_secs(1), || 7u32).await;
        assert_eq!(value, Some(7));
    }

    #[tokio::test]
    async fn blocking_call_degrades_to_none_on_timeout() {
        // 模拟目标进程无响应：闭包长时间不返回，调用方应在超时后拿到「无结果」而不是一直等
        let value = run_blocking_with_timeout("test.slow", std::time::Duration::from_millis(50), || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            1u32
        })
        .await;
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn abort_takes_effect_while_blocking_call_in_flight() {
        // 回归：同步阻塞调用被包进阻塞线程池后，就有了可观察的 await 点，
        // 工具组中断（Abortable）必须能在阻塞期间生效，而不是等阻塞结束或等到超时。
        let (abort_handle, abort_registration) = futures_util::future::AbortHandle::new_pair();
        let blocked = run_blocking_with_timeout("test.blocked", std::time::Duration::from_secs(30), || {
            std::thread::sleep(std::time::Duration::from_millis(400));
        });
        let future = futures_util::future::Abortable::new(blocked, abort_registration);
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            abort_handle.abort();
        });
        let started = std::time::Instant::now();
        let result = future.await;
        assert!(result.is_err(), "阻塞期间发出的中断必须被观察到");
        assert!(
            started.elapsed() < std::time::Duration::from_millis(300),
            "中断应立即生效，不应等到超时或阻塞结束"
        );
    }
}
