// ==================== 定时器测试 ====================

fn timer_test_sandbox_data_path(tag: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let root = std::env::temp_dir().join(format!(
        "pai-timer-{tag}-{}-{unique}",
        std::process::id()
    ));
    // data_path 的父目录名必须是 config，`app_root_from_data_path` 才会抬到沙盒根。
    let config_dir = root.join("config");
    fs::create_dir_all(&config_dir).expect("创建沙盒 config 目录");
    config_dir.join("app_config.toml")
}

fn timer_test_stored(id: &str, run_at_utc: &str, cron: Option<&str>) -> TimerRecordStored {
    TimerRecordStored {
        timer_id: id.to_string(),
        conversation_id: None,
        message: format!("测试定时器 {id}"),
        dispatch_mode: TimerDispatchMode::Delegate,
        enabled: true,
        run_at_utc: run_at_utc.to_string(),
        cron_expression: cron.map(ToOwned::to_owned),
        end_at_utc: None,
        next_run_at_utc: Some(run_at_utc.to_string()),
        last_triggered_at_utc: None,
        created_at_utc: run_at_utc.to_string(),
    }
}

#[test]
fn timer_cron_should_advance_to_next_matching_local_time() {
    // 每天 09:00（本地）。起始 09:00 触发过一次后，下一次必须是「本地次日 09:00」。
    let run_at_utc = "2026-03-01T01:00:00Z";
    let first = timer_compute_next_run_at_utc(run_at_utc, Some("0 9 * * *"), None, None)
        .expect("首次应算出起始时间");
    assert_eq!(first, run_at_utc);

    let next = timer_compute_next_run_at_utc(run_at_utc, Some("0 9 * * *"), None, Some(run_at_utc))
        .expect("触发后应算出下一次");
    let next_local = to_local_datetime(parse_rfc3339_time(&next).expect("下一次可解析"));
    let run_at_local = to_local_datetime(parse_rfc3339_time(run_at_utc).expect("起始可解析"));
    assert_eq!(next_local.hour(), 9);
    assert_eq!(next_local.minute(), 0);
    assert_eq!(
        next_local.date(),
        run_at_local.date().next_day().expect("次日")
    );
    assert!(next_local > run_at_local);
}

#[test]
fn timer_single_shot_should_never_repeat() {
    let run_at_utc = "2026-03-01T01:00:00Z";
    assert_eq!(
        timer_compute_next_run_at_utc(run_at_utc, None, None, None).as_deref(),
        Some(run_at_utc)
    );
    assert_eq!(
        timer_compute_next_run_at_utc(run_at_utc, None, None, Some(run_at_utc)),
        None
    );
}

#[test]
fn timer_end_at_should_stop_schedule() {
    let run_at_utc = "2026-03-01T01:00:00Z";
    // 截止时间不晚于起始时间：从来就不该排。
    assert_eq!(
        timer_compute_next_run_at_utc(run_at_utc, Some("0 9 * * *"), Some(run_at_utc), None),
        None
    );
    // 截止时间卡在第一次之后：触发一次即收工。
    let end_at_utc = "2026-03-01T12:00:00Z";
    assert_eq!(
        timer_compute_next_run_at_utc(run_at_utc, Some("0 9 * * *"), Some(end_at_utc), Some(run_at_utc)),
        None
    );
}

#[test]
fn timer_cron_expression_should_reject_malformed_input() {
    assert!(timer_parse_cron_expression("0 9 * *").is_err());
    assert!(timer_parse_cron_expression("99 * * * *").is_err());
    let normalized = timer_normalize_cron_expression(" 0  9 * * 1-5 ").expect("合法 cron");
    assert_eq!(normalized, "0 9 * * 1-5");
}

#[test]
fn timer_global_timer_should_force_delegate_dispatch() {
    // 没有会话（或就是系统通知会话）＝全局定时器，只能委托：没有前台轮次可以激活。
    assert_eq!(
        timer_normalize_dispatch_mode(None, TimerDispatchMode::Direct),
        TimerDispatchMode::Delegate
    );
    assert_eq!(
        timer_normalize_dispatch_mode(Some(SYSTEM_NOTIFICATION_CONVERSATION_ID), TimerDispatchMode::Direct),
        TimerDispatchMode::Delegate
    );
    assert_eq!(
        timer_normalize_dispatch_mode(Some("conversation-1"), TimerDispatchMode::Direct),
        TimerDispatchMode::Direct
    );
    assert_eq!(timer_normalize_conversation_id(Some("   ")), None);
}

#[test]
fn timer_is_due_should_respect_enabled_and_next_run() {
    let now = parse_rfc3339_time("2026-03-01T02:00:00Z").expect("当前时间");
    let mut record = timer_test_stored("1", "2026-03-01T01:00:00Z", Some("0 9 * * *"));
    assert!(timer_is_due(&record, now));

    record.enabled = false;
    assert!(!timer_is_due(&record, now));

    record.enabled = true;
    record.next_run_at_utc = Some("2026-03-01T03:00:00Z".to_string());
    assert!(!timer_is_due(&record, now));

    record.next_run_at_utc = None;
    assert!(!timer_is_due(&record, now));
}

#[test]
fn timer_store_should_roundtrip_and_delete() {
    let data_path = timer_test_sandbox_data_path("store");
    let record = timer_test_stored("7", "2026-03-01T01:00:00Z", Some("0 9 * * *"));

    timer_store_write_records(&data_path, &record).expect("写入定时器");
    let listed = timer_store_list_records(&data_path).expect("读取定时器列表");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].timer_id, "7");
    assert_eq!(listed[0].message, record.message);
    assert_eq!(listed[0].cron_expression.as_deref(), Some("0 9 * * *"));

    let fetched = timer_store_get_record(&data_path, "7").expect("按 id 读取定时器");
    assert_eq!(fetched.timer_id, "7");
    assert!(timer_store_get_record(&data_path, "9999").is_err());

    timer_store_delete(&data_path, "7").expect("删除定时器");
    assert!(timer_store_list_records(&data_path).expect("读取空列表").is_empty());

    let _ = fs::remove_dir_all(data_path.parent().and_then(|path| path.parent()).unwrap_or(&data_path));
}

#[test]
fn timer_recompute_should_fill_missing_next_run_for_migrated_row() {
    // 迁移来的行没有 next_run_at：补算时必须填上，否则永远算不到点。
    // 起始时间取远未来，补算结果就是它本身（过期行的口径由下一条用例单独钉住）。
    let data_path = timer_test_sandbox_data_path("recompute");
    let mut record = timer_test_stored("9", "2036-03-01T01:00:00Z", Some("0 9 * * *"));
    record.next_run_at_utc = None;
    timer_store_write_records(&data_path, &record).expect("写入迁移形态的定时器");

    let recomputed = timer_store_recompute_next_run(&data_path, &record).expect("补算下一次");
    assert_eq!(
        recomputed.next_run_at_utc.as_deref(),
        Some("2036-03-01T01:00:00Z")
    );
    let fetched = timer_store_get_record(&data_path, "9").expect("读取补算后的定时器");
    assert_eq!(
        fetched.next_run_at_utc.as_deref(),
        Some("2036-03-01T01:00:00Z")
    );

    let _ = fs::remove_dir_all(data_path.parent().and_then(|path| path.parent()).unwrap_or(&data_path));
}

#[test]
fn timer_recompute_should_skip_stale_history_instead_of_firing_all_at_once() {
    // 迁移来的历史行落在过去时，补算不能按老时间补课：
    // cron 行从当前时刻往后取下一个周期，一次性任务直接停用。
    let data_path = timer_test_sandbox_data_path("recompute-stale");
    let now = parse_rfc3339_time(
        &normalize_time_for_utc_storage(now_utc()).expect("当前时间"),
    )
    .expect("当前时间可解析");

    let mut cron_record = timer_test_stored("20", "2026-03-01T01:00:00Z", Some("0 9 * * *"));
    cron_record.next_run_at_utc = None;
    timer_store_write_records(&data_path, &cron_record).expect("写入过期 cron 行");
    let recomputed = timer_store_recompute_next_run(&data_path, &cron_record).expect("补算 cron 行");
    let next_run = parse_rfc3339_time(
        recomputed
            .next_run_at_utc
            .as_deref()
            .expect("过期 cron 行要补出未来的下一次"),
    )
    .expect("下一次可解析");
    assert!(next_run > now, "过期 cron 行的下一次必须落在当前时刻之后");
    assert!(recomputed.enabled, "过期 cron 行补出未来时间后保持启用");

    let mut single_record = timer_test_stored("21", "2026-03-01T01:00:00Z", None);
    single_record.next_run_at_utc = None;
    timer_store_write_records(&data_path, &single_record).expect("写入过期单次行");
    let recomputed = timer_store_recompute_next_run(&data_path, &single_record).expect("补算单次行");
    assert_eq!(recomputed.next_run_at_utc, None, "过期单次行不该补出到点时间");
    assert!(!recomputed.enabled, "过期单次行必须停用");
    assert!(!timer_is_due(&recomputed, now), "停用后不能仍判成到点");

    let _ = fs::remove_dir_all(data_path.parent().and_then(|path| path.parent()).unwrap_or(&data_path));
}

#[test]
fn timer_dispatch_failure_should_push_next_run_into_backoff_window() {
    // 派发失败的行必须离开「到点」集合：`next_run_at` 推到退避窗口之后并落库，
    // 否则下一拍又会把它判成到点，失败一次就变成零间隔重试。
    let data_path = timer_test_sandbox_data_path("dispatch-backoff");
    let mut record = timer_test_stored("30", "2026-03-01T01:00:00Z", Some("0 9 * * *"));
    record.next_run_at_utc = Some("2026-03-01T01:00:00Z".to_string());
    timer_store_write_records(&data_path, &record).expect("写入到点的定时器");
    let now = parse_rfc3339_time(
        &normalize_time_for_utc_storage(now_utc()).expect("当前时间"),
    )
    .expect("当前时间可解析");
    assert!(timer_is_due(&record, now), "前置条件：这条必须已经到点");

    let updated = timer_store_mark_dispatch_failure(&data_path, &record).expect("退避下一次触发");
    let next_run = parse_rfc3339_time(
        updated
            .next_run_at_utc
            .as_deref()
            .expect("退避后必须写回下一次触发"),
    )
    .expect("退避时间可解析");
    assert!(next_run > now, "退避时间必须在当前时刻之后");
    assert!(
        next_run <= now + time::Duration::seconds(TIMER_DISPATCH_RETRY_BACKOFF_SECONDS),
        "退避窗口不得超过一个退避时长"
    );
    assert!(updated.enabled, "退避不改启用状态");

    let fetched = timer_store_get_record(&data_path, "30").expect("读取退避后的定时器");
    assert_eq!(
        fetched.next_run_at_utc, updated.next_run_at_utc,
        "退避结果必须落库"
    );
    assert!(!timer_is_due(&fetched, now), "退避后这一拍不能再判成到点");

    let _ = fs::remove_dir_all(data_path.parent().and_then(|path| path.parent()).unwrap_or(&data_path));
}

#[test]
fn timer_scheduler_wait_should_never_be_zero() {
    // 到点集合里残留刚失败的行时，等待时长会算成 0；必须被抬到硬下限，杜绝单核自旋。
    let zero = timer_scheduler_clamp_wait(Some(std::time::Duration::ZERO));
    assert!(
        zero >= std::time::Duration::from_millis(TIMER_SCHEDULER_MIN_WAIT_MILLIS),
        "零等待必须被抬到硬下限，实际={zero:?}"
    );

    let no_due = timer_scheduler_clamp_wait(None);
    assert_eq!(
        no_due,
        std::time::Duration::from_secs(TIMER_SCHEDULER_FALLBACK_SECONDS),
        "没有更近的到点时用兜底复查间隔"
    );

    let far = timer_scheduler_clamp_wait(Some(std::time::Duration::from_secs(3600)));
    assert_eq!(
        far,
        std::time::Duration::from_secs(TIMER_SCHEDULER_FALLBACK_SECONDS),
        "远到点也要按兜底间隔复查"
    );

    let near = timer_scheduler_clamp_wait(Some(std::time::Duration::from_secs(3)));
    assert_eq!(near, std::time::Duration::from_secs(3), "近到点按实际时长等");
}

#[test]
fn timer_tool_should_only_touch_current_conversation() {
    // 模型工具只能把定时器挂在自己的会话或全局：指定别的会话一律拒绝。
    let session_id = "agent-1::conversation-1";
    assert!(timer_tool_assert_conversation_in_scope(session_id, Some("conversation-1")).is_ok());
    assert!(timer_tool_assert_conversation_in_scope(session_id, Some("  conversation-1  ")).is_ok());
    // 不传或空串＝全局定时器，不受限制。
    assert!(timer_tool_assert_conversation_in_scope(session_id, None).is_ok());
    assert!(timer_tool_assert_conversation_in_scope(session_id, Some("")).is_ok());
    assert!(timer_tool_assert_conversation_in_scope(session_id, Some("   ")).is_ok());
    // 别的会话必须被拒。
    let err = timer_tool_assert_conversation_in_scope(session_id, Some("conversation-2"))
        .expect_err("跨会话建定时器必须失败");
    assert!(err.contains("conversation-2"), "错误里要说清被拒的会话：{err}");
}

#[test]
fn timer_list_should_drop_timers_whose_conversation_is_gone() {
    let data_path = timer_test_sandbox_data_path("filter-unknown-conversation");

    // 全局定时器不绑会话，永远保留。
    let global = timer_test_stored("1", "2026-03-01T01:00:00Z", Some("0 9 * * *"));
    // 目标会话在会话库里不存在：它已经没有地方可投递。
    let mut orphan = timer_test_stored("2", "2026-03-01T01:00:00Z", Some("0 9 * * *"));
    orphan.conversation_id = Some("conversation-that-does-not-exist".to_string());
    // 目标会话已归档：同样不投递，也不该在列表里露头。
    let mut archived = timer_test_stored("3", "2026-03-01T01:00:00Z", Some("0 9 * * *"));
    archived.conversation_id = Some("conversation-archived".to_string());
    // 目标会话还在用：必须保留。
    let mut active = timer_test_stored("4", "2026-03-01T01:00:00Z", Some("0 9 * * *"));
    active.conversation_id = Some("conversation-active".to_string());

    timer_store_write_records(&data_path, &global).expect("写入全局定时器");
    timer_store_write_records(&data_path, &orphan).expect("写入孤儿定时器");
    timer_store_write_records(&data_path, &archived).expect("写入归档会话定时器");
    timer_store_write_records(&data_path, &active).expect("写入有效会话定时器");

    let conn = crate::message_store::chat_metadata_store_open(&data_path).expect("打开会话元数据库");
    for (conversation_id, status) in [
        ("conversation-archived", "archived"),
        ("conversation-active", "active"),
    ] {
        conn.execute(
            "INSERT INTO conversation_metadata (conversation_id, metadata_json, updated_at, status) \
             VALUES (?1, '{}', '2026-03-01T00:00:00Z', ?2)",
            rusqlite::params![conversation_id, status],
        )
        .expect("写入会话元数据");
    }
    drop(conn);

    assert_eq!(
        timer_store_list(&data_path).expect("读取定时器列表").len(),
        4,
        "过滤前四条都在库里"
    );

    let visible = timer_store_filter_unknown_conversations(
        &data_path,
        timer_store_list(&data_path).expect("读取定时器列表"),
    );

    let mut ids: Vec<&str> = visible.iter().map(|timer| timer.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["1", "4"], "只保留全局与仍在使用中的会话");

    let _ = fs::remove_dir_all(
        data_path
            .parent()
            .and_then(|path| path.parent())
            .unwrap_or(&data_path),
    );
}

#[test]
fn timer_list_should_only_return_requested_conversation() {
    // 会话侧栏的定时器页只该看到本会话的定时器：
    // 全局定时器没有会话归属，不属于任何会话侧栏。
    let data_path = timer_test_sandbox_data_path("filter-by-conversation");

    let global = timer_test_stored("1", "2026-03-01T01:00:00Z", Some("0 9 * * *"));
    let mut mine = timer_test_stored("2", "2026-03-01T01:00:00Z", Some("0 9 * * *"));
    mine.conversation_id = Some("conversation-a".to_string());
    let mut other = timer_test_stored("3", "2026-03-01T01:00:00Z", Some("0 9 * * *"));
    other.conversation_id = Some("conversation-b".to_string());

    timer_store_write_records(&data_path, &global).expect("写入全局定时器");
    timer_store_write_records(&data_path, &mine).expect("写入本会话定时器");
    timer_store_write_records(&data_path, &other).expect("写入其他会话定时器");

    let all = timer_store_list(&data_path).expect("读取定时器列表");
    assert_eq!(all.len(), 3, "库里有三条");

    let scoped = timer_store_filter_by_conversation(all.clone(), Some("conversation-a"));
    assert_eq!(
        scoped.iter().map(|timer| timer.id.as_str()).collect::<Vec<_>>(),
        vec!["2"],
        "只返回本会话那一条"
    );
    assert_eq!(
        timer_store_filter_by_conversation(all.clone(), Some("  conversation-a  ")).len(),
        1,
        "会话 id 两端空白不影响匹配"
    );
    assert_eq!(
        timer_store_filter_by_conversation(all.clone(), None).len(),
        3,
        "不传会话即全量，设置页与左栏总览依赖这个口径"
    );
    assert_eq!(
        timer_store_filter_by_conversation(all, Some("")).len(),
        3,
        "空串按未指定处理"
    );

    let _ = fs::remove_dir_all(
        data_path
            .parent()
            .and_then(|path| path.parent())
            .unwrap_or(&data_path),
    );
}
