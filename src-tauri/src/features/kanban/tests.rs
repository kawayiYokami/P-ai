// ==================== 队伍议题看板：测试 ====================
// 只覆盖本次改动的风险点：编号规则、状态机、序列化面、过滤。
#[cfg(test)]
mod kanban_tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        kanban_store_init(&conn).expect("init schema");
        conn
    }

    fn test_issue(leader: &str, number: u32, title: &str, status: &str) -> KanbanIssue {
        let now = now_utc_rfc3339();
        KanbanIssue {
            id: Uuid::new_v4().to_string(),
            leader_conversation_id: leader.to_string(),
            number,
            title: title.to_string(),
            status: status.to_string(),
            assignee_kind: KANBAN_ASSIGNEE_NONE.to_string(),
            assignee_member_role: None,
            note: String::new(),
            plan_path: String::new(),
            created_at: now.clone(),
            updated_at: now,
            version: 0,
        }
    }

    // ---------- 编号：单调递增、不回收、按看板隔离 ----------

    #[test]
    fn kanban_number_should_not_be_reused_after_close() {
        // 议题只增不删：编号一旦分配就不再回收，关闭一条也不能把号还回去。
        let conn = test_conn();
        let leader = "conv-leader-a";
        for index in 1..=7u32 {
            let number = kanban_store_allocate_number(&conn, leader).expect("allocate");
            assert_eq!(number, index);
            kanban_store_insert(&conn, &test_issue(leader, number, "议题", KANBAN_STATUS_TODO))
                .expect("insert");
        }
        let next = kanban_store_allocate_number(&conn, leader).expect("allocate again");
        assert_eq!(next, 8, "编号一旦分配就不再回收");
    }

    #[test]
    fn kanban_number_should_be_scoped_per_board() {
        let conn = test_conn();
        let a = kanban_store_allocate_number(&conn, "conv-a").expect("a#1");
        let b = kanban_store_allocate_number(&conn, "conv-b").expect("b#1");
        assert_eq!(a, 1);
        assert_eq!(b, 1, "不同看板的编号互不相干");

        kanban_store_insert(&conn, &test_issue("conv-a", 1, "A 的议题", KANBAN_STATUS_TODO))
            .expect("insert a");
        assert!(kanban_store_find(&conn, "conv-a", 1).expect("find a").is_some());
        assert!(
            kanban_store_find(&conn, "conv-b", 1).expect("find b").is_none(),
            "另一个看板看不到这块板的 1 号"
        );
    }

    #[test]
    fn kanban_missing_numbers_should_be_reported_before_write() {
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_store_insert(&conn, &test_issue(leader, 1, "一", KANBAN_STATUS_TODO)).expect("insert");
        kanban_store_insert(&conn, &test_issue(leader, 2, "二", KANBAN_STATUS_TODO)).expect("insert");

        assert!(kanban_store_missing_numbers(&conn, leader, &[1, 2])
            .expect("check")
            .is_empty());
        assert_eq!(
            kanban_store_missing_numbers(&conn, leader, &[2, 9]).expect("check"),
            vec![9],
            "不存在的编号必须被点出来，不能静默新建"
        );
    }

    // ---------- 状态：任何状态都能切到任何状态 ----------

    #[test]
    fn kanban_any_status_should_switch_to_any_status() {
        // 两两互切全部放行：没有单向、没有跳级、没有终态锁。
        for from in KANBAN_STATUSES {
            for to in KANBAN_STATUSES {
                if from == to {
                    continue;
                }
                assert!(
                    kanban_next_state_checked(1, from, to, Some("用户改主意了")).is_ok(),
                    "{from} → {to} 必须放行"
                );
            }
        }
    }

    #[test]
    fn kanban_done_and_closed_should_both_be_reopenable() {
        // 红豆点名的两种重开：已完成改回解决中、已关闭重新打开。
        kanban_next_state_checked(
            1,
            KANBAN_STATUS_DONE,
            KANBAN_STATUS_IN_PROGRESS,
            Some("用户说还没做完"),
        )
        .expect("done → in_progress 必须放行");
        kanban_next_state_checked(
            1,
            KANBAN_STATUS_CLOSED,
            KANBAN_STATUS_TODO,
            Some("用户说这件事还要做"),
        )
        .expect("closed → todo 必须放行");
        kanban_next_state_checked(
            1,
            KANBAN_STATUS_DONE,
            KANBAN_STATUS_CLOSED,
            Some("确认作废"),
        )
        .expect("done → closed 必须放行");
    }

    #[test]
    fn kanban_status_change_should_require_a_reason_in_this_request() {
        let err = kanban_next_state_checked(3, KANBAN_STATUS_IN_REVIEW, KANBAN_STATUS_DONE, None)
            .expect_err("改状态必须带理由");
        assert!(err.contains("#3"), "要点出是哪一条：{err}");
        assert!(
            err.contains(KANBAN_STATUS_IN_REVIEW) && err.contains(KANBAN_STATUS_DONE),
            "要写清从什么改到什么：{err}"
        );
        assert!(err.contains("note"), "要告诉它理由写在哪：{err}");

        let err = kanban_next_state_checked(
            3,
            KANBAN_STATUS_TODO,
            KANBAN_STATUS_IN_PROGRESS,
            Some("   "),
        )
        .expect_err("纯空白理由等于没写");
        assert!(err.contains("为什么改"), "报错要说清要写什么：{err}");
    }

    #[test]
    fn kanban_same_status_should_not_require_a_reason() {
        // 同状态重复写入不是变更，不该逼模型现编一个理由。
        for status in KANBAN_STATUSES {
            assert!(
                kanban_next_state_checked(1, status, status, None).is_ok(),
                "{status} 同状态重复写入不该要求理由"
            );
        }
    }

    #[test]
    fn kanban_planning_should_no_longer_be_a_valid_status() {
        assert_eq!(KANBAN_STATUSES.len(), 7);
        assert!(!KANBAN_STATUSES.contains(&"planning"));
        assert_eq!(kanban_status_normalized("planning"), None);
        assert_eq!(kanban_status_normalized(" planners"), None);
    }

    #[test]
    fn kanban_closed_should_be_a_valid_status() {
        assert_eq!(
            kanban_status_normalized(" closed "),
            Some(KANBAN_STATUS_CLOSED)
        );
        assert!(KANBAN_STATUSES.contains(&KANBAN_STATUS_CLOSED));
    }

    #[test]
    fn kanban_approve_transition_should_respect_from_status() {
        // 看板点「通过」走条件更新：当前状态不符时必须命中 0 行，先到先得。
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_store_insert(&conn, &test_issue(leader, 1, "一", KANBAN_STATUS_IN_PROGRESS))
            .expect("insert");
        let now = now_utc_rfc3339();

        let applied = kanban_store_transition(
            &conn,
            leader,
            1,
            KANBAN_STATUS_IN_REVIEW,
            KANBAN_STATUS_DONE,
            "用户确认",
            false,
            &now,
        )
        .expect("transition");
        assert!(!applied);

        let applied = kanban_store_transition(
            &conn,
            leader,
            1,
            KANBAN_STATUS_IN_PROGRESS,
            KANBAN_STATUS_IN_REVIEW,
            "",
            false,
            &now,
        )
        .expect("transition");
        assert!(applied);
        let stored = kanban_store_find(&conn, leader, 1).expect("find").expect("exists");
        assert_eq!(stored.status, KANBAN_STATUS_IN_REVIEW);
        assert_eq!(stored.version, 1);
    }

    // ---------- 新建与修改的状态约束 ----------

    fn upsert_item(
        number: Option<u32>,
        title: Option<&str>,
        status: Option<&str>,
        note: Option<&str>,
    ) -> KanbanIssueUpsertItem {
        KanbanIssueUpsertItem {
            issue_number: number,
            title: title.map(str::to_string),
            status: status.map(str::to_string),
            assignee: None,
            note: note.map(str::to_string),
        }
    }

    fn stored_status(conn: &Connection, leader: &str, number: u32) -> String {
        kanban_store_find(conn, leader, number)
            .expect("find")
            .expect("exists")
            .status
    }

    #[test]
    fn kanban_create_should_reject_status_param() {
        // 新建是新建：不接受指定状态，一律从待议开始。
        let conn = test_conn();
        let err = kanban_apply_upsert_in(
            &conn,
            "conv-leader",
            &[upsert_item(
                None,
                Some("想直接建成已完成"),
                Some(KANBAN_STATUS_DONE),
                Some("用户同意了"),
            )],
        )
        .expect_err("新建不能指定状态");
        assert!(err.contains("不接受 status"), "实际：{err}");
        assert!(
            err.contains(KANBAN_STATUS_DISCUSSING),
            "要告诉它固定从哪开始：{err}"
        );
        assert!(
            kanban_store_list(&conn, "conv-leader", None, None, None)
                .expect("list")
                .is_empty(),
            "被拒之后不能留下半条议题"
        );
    }

    #[test]
    fn kanban_create_should_always_land_on_discussing() {
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(None, Some("议题"), None, Some("用户提了一句"))],
        )
        .expect("create");
        assert_eq!(
            stored_status(&conn, leader, 1),
            KANBAN_STATUS_DISCUSSING,
            "新建一律落在待议"
        );
    }

    #[test]
    fn kanban_create_should_require_a_reason() {
        let conn = test_conn();
        let leader = "conv-leader";
        let err = kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(None, Some("没写理由"), None, None)],
        )
        .expect_err("新建必须带理由");
        assert!(err.contains("note"), "要告诉它写在哪：{err}");
        assert!(
            kanban_store_list(&conn, leader, None, None, None)
                .expect("list")
                .is_empty(),
            "被拒之后不能留下半条议题"
        );

        // 纯空白同样不算理由。
        assert!(kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(None, Some("空白理由"), None, Some("   "))],
        )
        .is_err());
    }

    #[test]
    fn kanban_batch_should_write_nothing_when_one_item_is_rejected() {
        // 「整批不落库」必须是事务保证，不能靠循环恰好先炸在前一条。
        // 这里让第一条合法、第二条违规：若没有事务，第一条会留在库里。
        let conn = test_conn();
        let leader = "conv-leader";
        let err = kanban_apply_upsert_in(
            &conn,
            leader,
            &[
                upsert_item(None, Some("这条本来是合法的"), None, Some("用户说要记")),
                upsert_item(None, Some("这条没有理由"), Some(KANBAN_STATUS_TODO), None),
            ],
        )
        .expect_err("混入一条违规必须整批失败");
        assert!(!err.is_empty());
        assert!(
            kanban_store_list(&conn, leader, None, None, None)
                .expect("list")
                .is_empty(),
            "整批回滚后不能留下第一条"
        );
        // 编号也不能被白白消耗掉：事务回滚应把计数一起退回。
        let next = kanban_store_allocate_number(&conn, leader).expect("allocate");
        assert_eq!(next, 1, "失败的批量不该吃掉编号");
    }

    #[test]
    fn kanban_note_should_not_be_cleared_on_any_status() {
        // note 任何时候不得为空：任何状态的议题都不能被空值静默清掉上一条理由。
        for status in KANBAN_STATUSES {
            let conn = test_conn();
            let leader = "conv-leader";
            kanban_store_insert(
                &conn,
                &KanbanIssue {
                    note: "上一版理由".to_string(),
                    ..test_issue(leader, 1, "议题", status)
                },
            )
            .expect("insert");

            let err = kanban_apply_upsert_in(
                &conn,
                leader,
                &[upsert_item(Some(1), None, Some(status), Some(""))],
            )
            .expect_err(&format!("{status} 的 note 不能清空"));
            assert!(err.contains("note 不能清空"), "实际：{err}");
            let stored = kanban_store_find(&conn, leader, 1).expect("find").expect("exists");
            assert_eq!(stored.note, "上一版理由", "被拒后 note 必须原样保留");

            // 纯空白同样不行。
            assert!(kanban_apply_upsert_in(
                &conn,
                leader,
                &[upsert_item(Some(1), None, None, Some("  "))],
            )
            .is_err());
        }
    }

    #[test]
    fn kanban_title_only_update_should_keep_note() {
        // 不传 status 只改标题这条路径由 note 校验兜住，不能顺手把理由抹掉。
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_store_insert(
            &conn,
            &KanbanIssue {
                note: "上一版理由".to_string(),
                ..test_issue(leader, 1, "旧标题", KANBAN_STATUS_IN_PROGRESS)
            },
        )
        .expect("insert");

        kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(Some(1), Some("新标题"), None, Some(""))],
        )
        .expect_err("只改标题也不能把 note 清空");

        kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(Some(1), Some("新标题"), None, None)],
        )
        .expect("只改标题、不碰 note 应当成功");
        let stored = kanban_store_find(&conn, leader, 1).expect("find").expect("exists");
        assert_eq!(stored.title, "新标题");
        assert_eq!(stored.note, "上一版理由");
    }

    #[test]
    fn kanban_updated_note_should_reject_empty_value() {
        assert!(kanban_updated_note_checked(1, "").is_err());
        assert!(kanban_updated_note_checked(1, "  \n ").is_err());
        assert_eq!(
            kanban_updated_note_checked(1, " 用户确认 ").expect("ok"),
            "用户确认"
        );
    }

    #[test]
    fn kanban_same_status_update_should_pass_without_note() {
        // 同状态重复写入不算变更，不要求理由。
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_store_insert(&conn, &test_issue(leader, 1, "议题", KANBAN_STATUS_TODO)).expect("insert");
        kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(Some(1), None, Some(KANBAN_STATUS_TODO), None)],
        )
        .expect("同状态重复写入应当放行");
        assert_eq!(stored_status(&conn, leader, 1), KANBAN_STATUS_TODO);
    }

    // ---------- 对外序列化面 ----------

    #[test]
    fn kanban_tool_output_should_not_expose_storage_id() {
        let issue = test_issue("conv-leader", 3, "接看板磁贴", KANBAN_STATUS_IN_REVIEW);
        let storage_id = issue.id.clone();
        let value = kanban_tool_issue_value(&issue.to_view());
        let text = serde_json::to_string(&value).expect("serialize");

        assert!(!text.contains(&storage_id), "工具返回值里不能出现存储主键");
        assert!(text.contains("\"number\""), "对外只用 number 定位");
        assert!(
            value.get("number").and_then(Value::as_u64) == Some(3),
            "number 应直接出现在返回值里"
        );
        assert!(value.get("id").is_none(), "id 不能作为字段出现");
        assert!(
            value.get("version").is_none(),
            "version 是内部并发控制，返回值里出现会引诱模型去维护它"
        );
    }

    // ---------- 归属编解码 ----------

    #[test]
    fn kanban_assignee_should_round_trip() {
        for raw in ["leader", "member:李工", "member:审查者", "none"] {
            let (kind, role) = kanban_assignee_from_wire(raw).expect("parse");
            assert_eq!(kanban_assignee_to_wire(&kind, role.as_deref()), raw);
        }
        // 角色名带空格时 member: 之后必须整段取用，不能按空格切第一段
        let (kind, role) = kanban_assignee_from_wire("member: 李 工 ").expect("parse");
        assert_eq!(kind, KANBAN_ASSIGNEE_MEMBER);
        assert_eq!(role.as_deref(), Some("李 工"));
        assert_eq!(
            kanban_assignee_to_wire(&kind, role.as_deref()),
            "member:李 工"
        );
        // 空值与 unassigned 一并落到 none，避免模型换个说法就报错
        assert_eq!(
            kanban_assignee_from_wire("  ").expect("parse"),
            (KANBAN_ASSIGNEE_NONE.to_string(), None)
        );
        assert_eq!(
            kanban_assignee_from_wire("unassigned").expect("parse"),
            (KANBAN_ASSIGNEE_NONE.to_string(), None)
        );
        assert!(kanban_assignee_from_wire("member:").is_err());
        // 编号已不对模型暴露：模型还写编号时必须直接报错，不能当成角色名「3」收下
        let numeric = kanban_assignee_from_wire("member:3").expect_err("编号写法必须被拒");
        assert!(numeric.contains("角色"), "{numeric}");
        assert!(kanban_assignee_from_wire("李工").is_err());
    }

    // ---------- 状态与磁贴 ----------

    #[test]
    fn kanban_statuses_should_cover_seven_states() {
        assert_eq!(KANBAN_STATUSES.len(), 7);
        assert!(KANBAN_STATUSES.contains(&KANBAN_STATUS_DISCUSSING));
        assert!(KANBAN_STATUSES.contains(&KANBAN_STATUS_CLOSED));
        for key in [
            KANBAN_STATUS_TODO,
            KANBAN_STATUS_IN_PROGRESS,
            KANBAN_STATUS_IN_REVIEW,
            KANBAN_STATUS_BLOCKED,
            KANBAN_STATUS_DONE,
        ] {
            assert!(KANBAN_STATUSES.contains(&key), "{key} 应在 KANBAN_STATUSES 里");
        }
        for key in KANBAN_STATUSES {
            assert_eq!(kanban_status_normalized(key), Some(key));
        }
    }

    #[test]
    fn kanban_attention_should_only_contain_user_blocking_states() {
        assert_eq!(
            KANBAN_ATTENTION_STATUSES,
            [
                KANBAN_STATUS_IN_REVIEW,
                KANBAN_STATUS_TODO,
                KANBAN_STATUS_BLOCKED,
                KANBAN_STATUS_DISCUSSING
            ]
        );
        for key in [
            KANBAN_STATUS_IN_PROGRESS,
            KANBAN_STATUS_DONE,
            KANBAN_STATUS_CLOSED,
        ] {
            assert!(
                !KANBAN_ATTENTION_STATUSES.contains(&key),
                "{key} 不该上磁贴"
            );
        }
    }

    // ---------- 列表过滤 ----------

    #[test]
    fn kanban_list_should_filter_by_status_and_assignee() {
        let conn = test_conn();
        let leader = "conv-leader";
        let mut review = test_issue(leader, 1, "等通过", KANBAN_STATUS_IN_REVIEW);
        review.assignee_kind = KANBAN_ASSIGNEE_MEMBER.to_string();
        review.assignee_member_role = Some("李工".to_string());
        kanban_store_insert(&conn, &review).expect("insert");
        let mut todo = test_issue(leader, 2, "待办", KANBAN_STATUS_TODO);
        todo.assignee_kind = KANBAN_ASSIGNEE_LEADER.to_string();
        kanban_store_insert(&conn, &todo).expect("insert");

        assert_eq!(
            kanban_store_list(&conn, leader, None, None, None)
                .expect("all")
                .len(),
            2
        );
        assert_eq!(
            kanban_store_list(&conn, leader, Some(KANBAN_STATUS_IN_REVIEW), None, None)
                .expect("by status")
                .len(),
            1
        );
        // 只按归属过滤：参数位不能错位（这条分支曾经最容易写错）
        assert_eq!(
            kanban_store_list(
                &conn,
                leader,
                None,
                Some(KANBAN_ASSIGNEE_MEMBER),
                Some("李工")
            )
            .expect("by assignee")
            .len(),
            1
        );
        // 同 kind 不同角色不能串味：name 维度必须真的参与过滤
        assert_eq!(
            kanban_store_list(
                &conn,
                leader,
                None,
                Some(KANBAN_ASSIGNEE_MEMBER),
                Some("王工")
            )
            .expect("by other role")
            .len(),
            0
        );
        assert_eq!(
            kanban_store_list(
                &conn,
                leader,
                Some(KANBAN_STATUS_IN_REVIEW),
                Some(KANBAN_ASSIGNEE_MEMBER),
                Some("李工")
            )
            .expect("by both")
            .len(),
            1
        );
        assert_eq!(
            kanban_store_list(&conn, leader, Some(KANBAN_STATUS_DONE), None, None)
                .expect("empty")
                .len(),
            0
        );
    }

    // ---------- 设置计划文件 ----------

    /// 临时目录本身是绝对路径，各平台都用它拼，避免写死盘符。
    fn absolute_plan_path(name: &str) -> String {
        std::env::temp_dir()
            .join(name)
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn kanban_set_plan_should_write_path() {
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_store_insert(&conn, &test_issue(leader, 1, "议题", KANBAN_STATUS_TODO))
            .expect("insert");
        let raw = absolute_plan_path("kanban-set-plan-ok.md");

        let result = kanban_set_plan_in(&conn, leader, 1, &raw).expect("set plan");
        assert_eq!(result.number, 1);
        assert_eq!(result.plan_path, raw);

        let stored = kanban_store_find(&conn, leader, 1).expect("find").expect("exists");
        assert_eq!(stored.plan_path, raw, "路径要落到议题上");
    }

    #[test]
    fn kanban_set_plan_should_overwrite_previous() {
        // 没有卸载动作，换计划就是直接覆盖：第二次设置不是错误。
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_store_insert(&conn, &test_issue(leader, 1, "议题", KANBAN_STATUS_TODO))
            .expect("insert");
        let first = absolute_plan_path("first.md");
        let second = absolute_plan_path("second.md");

        kanban_set_plan_in(&conn, leader, 1, &first).expect("first");
        let result = kanban_set_plan_in(&conn, leader, 1, &second).expect("覆盖更新不该报错");
        assert_eq!(result.plan_path, second);

        let stored = kanban_store_find(&conn, leader, 1).expect("find").expect("exists");
        assert_eq!(stored.plan_path, second, "留下的必须是新路径");
    }

    #[test]
    fn kanban_set_plan_should_reject_empty_path() {
        // 不能卸载，所以空值等于要求清空，必须拒。
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_store_insert(&conn, &test_issue(leader, 1, "议题", KANBAN_STATUS_TODO))
            .expect("insert");
        let existing = absolute_plan_path("keep.md");
        kanban_set_plan_in(&conn, leader, 1, &existing).expect("set");

        for blank in ["", "   ", "\t\n"] {
            let err = kanban_set_plan_in(&conn, leader, 1, blank).expect_err("空路径要拒");
            assert!(err.contains("只能更新不能清空"), "要说清为什么不能空：{err}");
        }

        let stored = kanban_store_find(&conn, leader, 1).expect("find").expect("exists");
        assert_eq!(stored.plan_path, existing, "被拒的写入不能碰掉已有路径");
    }

    #[test]
    fn kanban_set_plan_should_not_require_file_exists() {
        // 计划可能还没落盘，设置只管记路径；文件在不在由模型自己去看。
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_store_insert(&conn, &test_issue(leader, 1, "议题", KANBAN_STATUS_TODO))
            .expect("insert");
        let missing = std::env::temp_dir().join("kanban-set-plan-not-there-9f3a.md");
        std::fs::remove_file(&missing).ok();
        let raw = missing.to_string_lossy().to_string();

        let result = kanban_set_plan_in(&conn, leader, 1, &raw).expect("不校验文件是否存在");
        assert_eq!(result.plan_path, raw);
    }

    #[test]
    fn kanban_plan_path_should_require_absolute() {
        assert!(kanban_set_plan_path_checked("plan/chat/x.md").is_err(), "相对路径没法定位");
        assert!(kanban_set_plan_path_checked("   ").is_err(), "空值等于要求清空");
        assert!(kanban_set_plan_path_checked(&absolute_plan_path("x.md")).is_ok());
    }

    #[test]
    fn kanban_unknown_status_should_be_rejected_on_update() {
        // 状态值必须在七态里：旧状态 planning 已经不在集合内，报错要列出可选值。
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_store_insert(&conn, &test_issue(leader, 1, "议题", KANBAN_STATUS_TODO))
            .expect("insert");
        let err = kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(
                Some(1),
                None,
                Some("planning"),
                Some("想回到旧状态"),
            )],
        )
        .expect_err("planning 已不是合法状态");
        assert!(err.contains("状态非法"), "要指明是状态值的问题：{err}");
        assert!(err.contains(KANBAN_STATUS_CLOSED), "报错要列出七态：{err}");
        assert_eq!(
            stored_status(&conn, leader, 1),
            KANBAN_STATUS_TODO,
            "被拒后状态必须原样不动"
        );
    }

    // ---------- 队员绑定：一人一条在办、三态不许有人 ----------

    fn upsert_item_with_assignee(
        number: Option<u32>,
        title: Option<&str>,
        status: Option<&str>,
        assignee: Option<&str>,
        note: Option<&str>,
    ) -> KanbanIssueUpsertItem {
        KanbanIssueUpsertItem {
            issue_number: number,
            title: title.map(str::to_string),
            status: status.map(str::to_string),
            assignee: assignee.map(str::to_string),
            note: note.map(str::to_string),
        }
    }

    /// 直接落一条带负责人的议题，绕过工具校验。
    /// 库里已经存在的旧数据就是这么来的：这次不做迁移，但新写入必须挡住。
    fn insert_issue_with_role(
        conn: &Connection,
        leader: &str,
        number: u32,
        title: &str,
        status: &str,
        role: Option<&str>,
    ) {
        let mut issue = test_issue(leader, number, title, status);
        if let Some(role) = role {
            issue.assignee_kind = KANBAN_ASSIGNEE_MEMBER.to_string();
            issue.assignee_member_role = Some(role.to_string());
        }
        kanban_store_insert(conn, &issue).expect("insert");
    }

    fn stored_assignee(conn: &Connection, leader: &str, number: u32) -> String {
        let issue = kanban_store_find(conn, leader, number)
            .expect("find")
            .expect("exists");
        kanban_assignee_to_wire(&issue.assignee_kind, issue.assignee_member_role.as_deref())
    }

    #[test]
    fn kanban_member_should_not_hold_two_active_issues() {
        let conn = test_conn();
        let leader = "conv-leader";
        insert_issue_with_role(
            &conn,
            leader,
            1,
            "修复登录超时",
            KANBAN_STATUS_IN_PROGRESS,
            Some("李工"),
        );
        insert_issue_with_role(&conn, leader, 2, "补文档", KANBAN_STATUS_TODO, None);

        let err = kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item_with_assignee(
                Some(2),
                None,
                None,
                Some("member:李工"),
                None,
            )],
        )
        .expect_err("一个人同时只能有一条在办议题");
        assert!(err.contains("#2"), "要点出这次这条：{err}");
        assert!(err.contains("李工"), "要点出是谁：{err}");
        assert!(err.contains("#1"), "要点出冲突的是哪一条：{err}");
        assert!(err.contains("修复登录超时"), "要带上冲突议题的标题：{err}");
        assert!(err.contains("解决中"), "要带上冲突议题的状态：{err}");
        assert!(err.contains("收口"), "要给一句能直接执行的动作：{err}");
        assert_eq!(
            stored_assignee(&conn, leader, 2),
            KANBAN_ASSIGNEE_NONE,
            "被拒之后不能留下半截指派"
        );
    }

    #[test]
    fn kanban_done_issue_should_not_occupy_its_member() {
        let conn = test_conn();
        let leader = "conv-leader";
        insert_issue_with_role(&conn, leader, 1, "已经做完了", KANBAN_STATUS_DONE, Some("李工"));
        insert_issue_with_role(&conn, leader, 2, "新活", KANBAN_STATUS_TODO, None);
        kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item_with_assignee(
                Some(2),
                None,
                None,
                Some("member:李工"),
                None,
            )],
        )
        .expect("已完成不占人，可以接新活");
    }

    #[test]
    fn kanban_closed_issue_should_not_occupy_its_member() {
        let conn = test_conn();
        let leader = "conv-leader";
        insert_issue_with_role(&conn, leader, 1, "不做了", KANBAN_STATUS_CLOSED, Some("李工"));
        insert_issue_with_role(&conn, leader, 2, "新活", KANBAN_STATUS_TODO, None);
        kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item_with_assignee(
                Some(2),
                None,
                None,
                Some("member:李工"),
                None,
            )],
        )
        .expect("已关闭不占人，可以接新活");
    }

    #[test]
    fn kanban_same_batch_should_not_bind_two_issues_to_one_member() {
        // 逐条判会两条都放行，所以校验必须在整批算完最终态之后做。
        let conn = test_conn();
        let leader = "conv-leader";
        insert_issue_with_role(&conn, leader, 1, "第一件", KANBAN_STATUS_TODO, None);
        insert_issue_with_role(&conn, leader, 2, "第二件", KANBAN_STATUS_TODO, None);
        let err = kanban_apply_upsert_in(
            &conn,
            leader,
            &[
                upsert_item_with_assignee(Some(1), None, None, Some("member:李工"), None),
                upsert_item_with_assignee(Some(2), None, None, Some("member:李工"), None),
            ],
        )
        .expect_err("同一批里两条绑给同一个人必须整批失败");
        assert!(err.contains("李工"), "要点出是谁：{err}");
        assert_eq!(
            stored_assignee(&conn, leader, 1),
            KANBAN_ASSIGNEE_NONE,
            "整批失败时第一条也不能落库"
        );
    }

    #[test]
    fn kanban_same_batch_should_allow_unbind_then_bind() {
        // 收口旧的、派新的常常写在同一批里，逐条判会把这种正常操作误判成冲突。
        let conn = test_conn();
        let leader = "conv-leader";
        insert_issue_with_role(&conn, leader, 1, "旧活", KANBAN_STATUS_TODO, Some("李工"));
        insert_issue_with_role(&conn, leader, 2, "新活", KANBAN_STATUS_TODO, None);
        kanban_apply_upsert_in(
            &conn,
            leader,
            &[
                upsert_item_with_assignee(Some(1), None, None, Some("none"), None),
                upsert_item_with_assignee(Some(2), None, None, Some("member:李工"), None),
            ],
        )
        .expect("同一批里先解绑再绑新的应当放行");
        assert_eq!(stored_assignee(&conn, leader, 2), "member:李工");
    }

    #[test]
    fn kanban_reopening_done_issue_should_still_respect_single_active() {
        let conn = test_conn();
        let leader = "conv-leader";
        insert_issue_with_role(&conn, leader, 1, "在办", KANBAN_STATUS_IN_PROGRESS, Some("李工"));
        insert_issue_with_role(&conn, leader, 2, "做完了", KANBAN_STATUS_DONE, Some("李工"));
        let err = kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(
                Some(2),
                None,
                Some(KANBAN_STATUS_IN_PROGRESS),
                Some("用户要求重开"),
            )],
        )
        .expect_err("重开后这个人就有两条在办了");
        assert!(err.contains("#1"), "要点出冲突的是哪一条：{err}");
        assert_eq!(
            stored_status(&conn, leader, 2),
            KANBAN_STATUS_DONE,
            "被拒后状态必须原样不动"
        );
    }

    #[test]
    fn kanban_finishing_issue_should_require_unbinding_first() {
        let conn = test_conn();
        let leader = "conv-leader";
        insert_issue_with_role(&conn, leader, 1, "做完的活", KANBAN_STATUS_IN_PROGRESS, Some("李工"));
        let err = kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(
                Some(1),
                None,
                Some(KANBAN_STATUS_DONE),
                Some("做完了"),
            )],
        )
        .expect_err("已完成不归任何人办，必须先摘人");
        assert!(err.contains("不归任何人办"), "要说清这个状态不该有人：{err}");
        assert!(err.contains("李工"), "要点出还挂着谁：{err}");
        assert!(err.contains("none"), "要告诉它改成什么：{err}");

        // 同一次里把 assignee 写成 none 就放行。
        kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item_with_assignee(
                Some(1),
                None,
                Some(KANBAN_STATUS_DONE),
                Some("none"),
                Some("做完了"),
            )],
        )
        .expect("先摘人再收口应当放行");
        assert_eq!(stored_assignee(&conn, leader, 1), KANBAN_ASSIGNEE_NONE);
    }

    #[test]
    fn kanban_reopening_finished_issue_should_pass_without_other_active() {
        let conn = test_conn();
        let leader = "conv-leader";
        insert_issue_with_role(&conn, leader, 1, "做完了", KANBAN_STATUS_DONE, Some("李工"));
        kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(
                Some(1),
                None,
                Some(KANBAN_STATUS_TODO),
                Some("用户要求重开"),
            )],
        )
        .expect("重开成待办后它重新占人，但此人没有别的在办");
        assert_eq!(stored_status(&conn, leader, 1), KANBAN_STATUS_TODO);
    }

    #[test]
    fn kanban_assignee_blacklist_should_reject_discussing_done_closed() {
        for status in [
            KANBAN_STATUS_DISCUSSING,
            KANBAN_STATUS_DONE,
            KANBAN_STATUS_CLOSED,
        ] {
            let conn = test_conn();
            let leader = "conv-leader";
            insert_issue_with_role(&conn, leader, 1, "议题", KANBAN_STATUS_TODO, Some("李工"));
            let err = kanban_apply_upsert_in(
                &conn,
                leader,
                &[upsert_item(Some(1), None, Some(status), Some("改状态"))],
            )
            .expect_err("这三态不允许挂负责人");
            assert!(err.contains("不归任何人办"), "{status} 的报错要说清原因：{err}");
            assert!(err.contains("李工"), "{status} 的报错要点出还挂着谁：{err}");
            assert_eq!(
                stored_status(&conn, leader, 1),
                KANBAN_STATUS_TODO,
                "{status} 被拒后状态原样不动"
            );
        }
    }

    #[test]
    fn kanban_assignee_should_stay_allowed_on_review_and_blocked() {
        // 待审是等用户点头，人还在名下；阻塞是卡住了，责任人更该留上面。
        for status in [KANBAN_STATUS_IN_REVIEW, KANBAN_STATUS_BLOCKED] {
            let conn = test_conn();
            let leader = "conv-leader";
            insert_issue_with_role(&conn, leader, 1, "议题", KANBAN_STATUS_IN_PROGRESS, Some("李工"));
            kanban_apply_upsert_in(
                &conn,
                leader,
                &[upsert_item(Some(1), None, Some(status), Some("推进"))],
            )
            .expect("待审与阻塞保留负责人");
            assert_eq!(stored_assignee(&conn, leader, 1), "member:李工", "{status} 不该被摘人");
        }
    }

    #[test]
    fn kanban_create_should_reject_assignee() {
        // 新建固定落待议，而待议不挂人：只能先建、下一次再改状态顺带派人。
        let conn = test_conn();
        let leader = "conv-leader";
        for assignee in ["member:李工", "leader"] {
            let err = kanban_apply_upsert_in(
                &conn,
                leader,
                &[upsert_item_with_assignee(
                    None,
                    Some("想建了就派人"),
                    None,
                    Some(assignee),
                    Some("用户提了一句"),
                )],
            )
            .expect_err("新建不接受负责人");
            assert!(err.contains("不挂负责人"), "{assignee} 的报错要说清原因：{err}");
            assert!(
                err.contains("再带编号"),
                "{assignee} 的报错要引导成两步：{err}"
            );
        }
        assert!(
            kanban_store_list(&conn, leader, None, None, None)
                .expect("list")
                .is_empty(),
            "被拒之后不能留下半条议题"
        );
    }

    #[test]
    fn kanban_approve_transition_should_clear_assignee() {
        // 界面上点「通过」是第二写入口：它把待审推成已完成，
        // 必须顺手把人摘掉，否则点一次就造出一条「已完成还挂着人」的违规数据。
        let conn = test_conn();
        let leader = "conv-leader";
        insert_issue_with_role(&conn, leader, 1, "议题", KANBAN_STATUS_IN_REVIEW, Some("李工"));
        let now = now_utc_rfc3339();
        let applied = kanban_store_transition(
            &conn,
            leader,
            1,
            KANBAN_STATUS_IN_REVIEW,
            KANBAN_STATUS_DONE,
            "用户确认",
            true,
            &now,
        )
        .expect("transition");
        assert!(applied);
        assert_eq!(stored_status(&conn, leader, 1), KANBAN_STATUS_DONE);
        assert_eq!(
            stored_assignee(&conn, leader, 1),
            KANBAN_ASSIGNEE_NONE,
            "点通过之后这名队员必须被摘下来"
        );

        // 摘干净了他就能接新活。
        insert_issue_with_role(&conn, leader, 2, "新活", KANBAN_STATUS_TODO, None);
        kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item_with_assignee(
                Some(2),
                None,
                None,
                Some("member:李工"),
                None,
            )],
        )
        .expect("上一条已经收口，可以派新活");
    }

    #[test]
    fn kanban_create_should_leave_plan_path_empty() {
        let conn = test_conn();
        let leader = "conv-leader";
        kanban_apply_upsert_in(
            &conn,
            leader,
            &[upsert_item(None, Some("议题"), None, Some("用户提了一句"))],
        )
        .expect("create");
        let stored = kanban_store_find(&conn, leader, 1).expect("find").expect("exists");
        assert!(stored.plan_path.is_empty(), "新建议题不带计划");
    }

    #[test]
    fn kanban_active_issues_by_role_should_include_leader_and_members() {
        let mut leader_issue = test_issue("conv-leader", 1, "重构会话状态", KANBAN_STATUS_IN_PROGRESS);
        leader_issue.assignee_kind = KANBAN_ASSIGNEE_LEADER.to_string();

        let mut member_issue = test_issue("conv-leader", 2, "前端样式适配", KANBAN_STATUS_IN_PROGRESS);
        member_issue.assignee_kind = KANBAN_ASSIGNEE_MEMBER.to_string();
        member_issue.assignee_member_role = Some("哥伦比娅".to_string());

        let mut done_issue = test_issue("conv-leader", 3, "已完成任务", KANBAN_STATUS_DONE);
        done_issue.assignee_kind = KANBAN_ASSIGNEE_LEADER.to_string();

        let issues = vec![leader_issue, member_issue, done_issue];
        let by_role = active_issues_by_role_from_issues(
            &issues,
            Some("纳西妲"),
            Some("agent-nahida"),
        );

        // 队长承接的议题：同时挂在 leader、队长、队长人格名、队长 agent_id 下
        assert_eq!(by_role.get("leader").map(|s| s.number), Some(1));
        assert_eq!(by_role.get("队长").map(|s| s.number), Some(1));
        assert_eq!(by_role.get("纳西妲").map(|s| s.number), Some(1));
        assert_eq!(by_role.get("agent-nahida").map(|s| s.number), Some(1));

        // 队员承接的议题：正常挂在队员角色下
        assert_eq!(by_role.get("哥伦比娅").map(|s| s.number), Some(2));

        // 已完成的议题不占人
        assert!(!by_role.values().any(|s| s.number == 3));
    }
}
