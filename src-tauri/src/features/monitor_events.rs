// ==================== 监控事件统一发布 ====================
// 事件仅作脏标记：业务只更新运行时，写路径变更即发布；前端收到后按 domain 拉快照，不信任 payload 内容。

const MONITOR_CHANGED_EVENT: &str = "easy-call:monitor-changed";
const MONITOR_CHANGED_NOTIFICATION_METHOD: &str = "monitor.changed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonitorDomain {
    Task,
    BackgroundShell,
    Delegate,
}

impl MonitorDomain {
    fn as_str(self) -> &'static str {
        match self {
            MonitorDomain::Task => "task",
            MonitorDomain::BackgroundShell => "backgroundShell",
            MonitorDomain::Delegate => "delegate",
        }
    }
}

fn monitor_changed_payload(
    domain: MonitorDomain,
    kind: &str,
    conversation_id: &str,
    entity_id: &str,
) -> serde_json::Value {
    serde_json::json!({
        "domain": domain.as_str(),
        "kind": kind,
        "conversationId": conversation_id,
        "entityId": entity_id,
    })
}

fn monitor_publish_changed(
    state: &AppState,
    domain: MonitorDomain,
    kind: &str,
    conversation_id: &str,
    entity_id: &str,
) {
    let payload = monitor_changed_payload(domain, kind, conversation_id, entity_id);
    if let Ok(guard) = state.app_handle.lock() {
        if let Some(app_handle) = guard.as_ref() {
            if let Err(err) = app_handle.emit(MONITOR_CHANGED_EVENT, &payload) {
                runtime_log_warn(format!(
                    "[系统] 监控 事件推送失败 领域={} 类型={} 实体编号={} 错误={err}",
                    domain.as_str(),
                    kind,
                    entity_id
                ));
            }
        }
    }
    ide_chat_broadcast_notification(MONITOR_CHANGED_NOTIFICATION_METHOD, payload);
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return;
    }
    if let Err(err) = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        conversation_id,
    ) {
        runtime_log_debug(format!(
            "[聊天] 会话概览 跳过 任务=后台账本变动后推送单会话 领域={} 错误={}",
            domain.as_str(),
            err
        ));
    }
}

#[cfg(test)]
mod monitor_events_tests {
    use super::*;

    #[test]
    fn monitor_changed_payload_keeps_dirty_marker_fields_only() {
        let payload = monitor_changed_payload(
            MonitorDomain::BackgroundShell,
            "started",
            "conv-1",
            "bg-shell-1",
        );
        assert_eq!(payload["domain"], "backgroundShell");
        assert_eq!(payload["kind"], "started");
        assert_eq!(payload["conversationId"], "conv-1");
        assert_eq!(payload["entityId"], "bg-shell-1");
        // 事件只允许这四个字段，禁止夹带业务快照
        assert_eq!(
            payload.as_object().map(|obj| obj.len()),
            Some(4),
            "监控事件禁止携带快照内容"
        );
    }

    #[test]
    fn monitor_domain_labels_are_stable() {
        assert_eq!(MonitorDomain::Task.as_str(), "task");
        assert_eq!(MonitorDomain::BackgroundShell.as_str(), "backgroundShell");
        assert_eq!(MonitorDomain::Delegate.as_str(), "delegate");
    }
}
