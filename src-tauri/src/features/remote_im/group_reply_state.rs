#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteImGroupReplyTimerKind {
    NonMention,
    Mention,
}

/// 观察中的这一轮：**项存在 = 观察员醒着等这一次看群**；项消失 = 这一轮结束、观察员休眠。
///
/// 一轮的生命周期：入场排上 → sleep → 到点看群（读游标、读区间、结算、推进游标、交判断）→ 结束。
/// 结束就是这一项消失，没有「收尾」这个动作，也不留任何「上一轮跑着时来了消息」的标记：
/// 判断期间入库的新消息看到没有闹钟，自然起一个新闹钟，正确性由游标保证。
/// - `due_at`：这一次看群的时刻。
/// - `anchor_message_id`：游标还没有值时用它当区间起点。
#[derive(Clone)]
struct RemoteImInspectionSchedule {
    kind: RemoteImGroupReplyTimerKind,
    due_at: std::time::Instant,
    anchor_message_id: String,
    event: ChatPendingEvent,
}

#[derive(Default)]
struct RemoteImInspectionScheduleStore {
    next_inspection: std::collections::HashMap<String, RemoteImInspectionSchedule>,
}

#[derive(Debug, Clone, Copy)]
struct RemoteImGroupReplyDispatchPolicy {
    focus: bool,
    max_chars: u32,
}

#[derive(Clone)]
struct RemoteImGroupReplyTimerAction {
    state_key: String,
    contact_id: String,
    kind: RemoteImGroupReplyTimerKind,
    /// 排期到期时刻。定时器醒来时与当前排期比对：不一致说明已被更新的排期取代。
    due_at: std::time::Instant,
    delay: std::time::Duration,
}

fn remote_im_inspection_schedule_store(
) -> &'static std::sync::Mutex<RemoteImInspectionScheduleStore> {
    static STORE: std::sync::OnceLock<std::sync::Mutex<RemoteImInspectionScheduleStore>> =
        std::sync::OnceLock::new();
    STORE.get_or_init(|| std::sync::Mutex::new(RemoteImInspectionScheduleStore::default()))
}

fn lock_remote_im_inspection_schedule_store(
) -> std::sync::MutexGuard<'static, RemoteImInspectionScheduleStore> {
    match remote_im_inspection_schedule_store().lock() {
        Ok(store) => store,
        Err(poisoned) => {
            runtime_log_warn("[群聊巡检] 排期锁中毒，已恢复并保留现有排期".to_string());
            poisoned.into_inner()
        }
    }
}

fn remote_im_group_reply_state_key(state: &AppState, contact_id: &str) -> String {
    format!("{}::{}", state.data_path.to_string_lossy(), contact_id.trim())
}

fn remote_im_group_reply_inspection_delay(
    pacing: &RemoteImGroupReplyPacing,
    sample: f64,
) -> std::time::Duration {
    let sample = sample.clamp(0.0, 1.0);
    let centered = sample * 2.0 - 1.0;
    let seconds = pacing.secretary_inspection_seconds as f64
        * (1.0 + centered * pacing.inspection_jitter_ratio);
    std::time::Duration::from_secs_f64(seconds.max(1.0))
}

fn remote_im_group_reply_random_sample() -> f64 {
    let value = Uuid::new_v4().as_u128();
    (value as f64 / u128::MAX as f64).clamp(0.0, 1.0)
}

#[cfg(test)]
mod remote_im_group_reply_state_tests {
    use super::*;

    #[test]
    fn inspection_delay_should_stay_inside_jitter_bounds() {
        let pacing = RemoteImGroupReplyPacing {
            secretary_inspection_seconds: 10,
            inspection_jitter_ratio: 0.2,
            ..RemoteImGroupReplyPacing::default()
        };
        assert_eq!(remote_im_group_reply_inspection_delay(&pacing, 0.0).as_secs(), 8);
        assert_eq!(remote_im_group_reply_inspection_delay(&pacing, 0.5).as_secs(), 10);
        assert_eq!(remote_im_group_reply_inspection_delay(&pacing, 1.0).as_secs(), 12);
    }
}
