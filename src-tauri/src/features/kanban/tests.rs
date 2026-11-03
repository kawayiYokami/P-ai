// 只覆盖本次改动的风险点：编号规则、状态机、序列化面、过滤、归属按编号解析。
#[cfg(test)]
mod kanban_tests {
    use super::*;

    // ---------- 夹具 ----------

    /// 在队名册：编号与角色名一一对应。
    fn test_roster(members: &[(u32, &str)]) -> KanbanBoardRoster {
        let entries = || {
            members
                .iter()
                .map(|(number, role)| (*number, role.to_string()))
                .collect::<Vec<_>>()
        };
        KanbanBoardRoster::new(entries(), entries())
    }

    /// 带已请离队员的名册：`active` 能接活，`dismissed` 只能被历史议题认出来。
    fn test_roster_with_dismissed(
        active: &[(u32, &str)],
        dismissed: &[(u32, &str)],
    ) -> KanbanBoardRoster {
        let active_entries = active
            .iter()
            .map(|(number, role)| (*number, role.to_string()))
            .collect::<Vec<_>>();
        let mut all = active_entries.clone();
        all.extend(
            dismissed
                .iter()
                .map(|(number, role)| (*number, role.to_string())),
        );
        KanbanBoardRoster::new(active_entries, all)
    }

    fn test_issue(number: u32, title: &str, status: &str) -> KanbanIssueEntry {
        let now = now_utc_rfc3339();
        KanbanIssueEntry {
            number,
            title: title.to_string(),
            status: status.to_string(),
            assignee: KanbanAssigneeRef {
                kind: KANBAN_ASSIGNEE_NONE.to_string(),
                member_number: None,
            },
            note: String::new(),
            plan_path: String::new(),
            created_at: now.clone(),
            updated_at: now,
            version: 0,
        }
    }

    fn issue_of_member(
        number: u32,
        title: &str,
        status: &str,
        member_number: u32,
    ) -> KanbanIssueEntry {
        let mut issue = test_issue(number, title, status);
        issue.assignee = KanbanAssigneeRef {
            kind: KANBAN_ASSIGNEE_MEMBER.to_string(),
            member_number: Some(member_number),
        };
        issue
    }

    fn issue_of_leader(number: u32, title: &str, status: &str) -> KanbanIssueEntry {
        let mut issue = test_issue(number, title, status);
        issue.assignee = KanbanAssigneeRef {
            kind: KANBAN_ASSIGNEE_LEADER.to_string(),
            member_number: None,
        };
        issue
    }

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

    // ---------- 编号：单调递增、不回收、按看板隔离 ----------

    #[test]
    fn kanban_number_should_not_be_reused_after_removal() {
        // 编号只走游标，不从「现有最大编号」推：板上哪条没了都不影响下一个号。
        let roster = test_roster(&[]);
        let mut issues: Vec<KanbanIssueEntry> = Vec::new();
        let mut next_number = 0u32;
        for index in 1..=3u32 {
            kanban_apply_upsert_in(
                &mut issues,
                &mut next_number,
                &roster,
                &[upsert_item(
                    None,
                    Some("议题"),
                    None,
                    Some("用户提了一句"),
                )],
            )
            .expect("create");
            assert_eq!(issues.last().map(|issue| issue.number), Some(index));
        }

        // 假设某条被清理掉，游标不能跟着回退。
        issues.retain(|issue| issue.number != 3);
        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(None, Some("新议题"), None, Some("用户又提了一句"))],
        )
        .expect("create after removal");
        assert_eq!(
            issues.last().map(|issue| issue.number),
            Some(4),
            "编号一旦分配就不再回收"
        );
    }

    #[test]
    fn kanban_number_should_be_scoped_per_board() {
        // 两块板各有各的游标，互不相干。
        let roster = test_roster(&[]);
        let mut a: Vec<KanbanIssueEntry> = Vec::new();
        let mut a_next = 0u32;
        let mut b: Vec<KanbanIssueEntry> = Vec::new();
        let mut b_next = 0u32;
        kanban_apply_upsert_in(
            &mut a,
            &mut a_next,
            &roster,
            &[upsert_item(None, Some("A 的议题"), None, Some("理由"))],
        )
        .expect("a");
        kanban_apply_upsert_in(
            &mut b,
            &mut b_next,
            &roster,
            &[upsert_item(None, Some("B 的议题"), None, Some("理由"))],
        )
        .expect("b");
        assert_eq!(a[0].number, 1);
        assert_eq!(b[0].number, 1, "不同看板的编号互不相干");
        assert_eq!(kanban_find_issue(&a, 1).map(|issue| issue.title.as_str()), Some("A 的议题"));
        assert_eq!(kanban_find_issue(&b, 1).map(|issue| issue.title.as_str()), Some("B 的议题"));
    }

    #[test]
    fn kanban_missing_numbers_should_be_reported_before_write() {
        let roster = test_roster(&[]);
        let mut issues = vec![
            test_issue(1, "一", KANBAN_STATUS_TODO),
            test_issue(2, "二", KANBAN_STATUS_TODO),
        ];
        let mut next_number = 3u32;
        let err = kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(Some(9), Some("九"), None, Some("理由"))],
        )
        .expect_err("不存在的编号必须报错");
        assert!(err.contains("#9"), "要点出是哪个编号：{err}");
        assert!(
            err.contains("不会自动新建"),
            "要说清为什么不能写：{err}"
        );
        assert_eq!(issues.len(), 2, "被拒之后不能多出一条");
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
    fn kanban_done_should_be_reopenable() {
        // 已完成不是终态：用户说还没做完，就得能改回解决中。
        kanban_next_state_checked(
            1,
            KANBAN_STATUS_DONE,
            KANBAN_STATUS_IN_PROGRESS,
            Some("用户说还没做完"),
        )
        .expect("done → in_progress 必须放行");
        kanban_next_state_checked(
            1,
            KANBAN_STATUS_DONE,
            KANBAN_STATUS_TODO,
            Some("用户说这件事还要做"),
        )
        .expect("done → todo 必须放行");
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
    fn kanban_statuses_should_cover_six_states() {
        assert_eq!(KANBAN_STATUSES.len(), 6);
        assert!(KANBAN_STATUSES.contains(&KANBAN_STATUS_DISCUSSING));
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
        for key in [KANBAN_STATUS_IN_PROGRESS, KANBAN_STATUS_DONE] {
            assert!(
                !KANBAN_ATTENTION_STATUSES.contains(&key),
                "{key} 不该上磁贴"
            );
        }
    }

    #[test]
    fn kanban_planning_and_closed_should_not_be_valid_statuses() {
        // 六态白名单之外的值一律读不进来、也写不进去。
        for raw in ["planning", "closed", " planners", " closed "] {
            assert_eq!(kanban_status_normalized(raw), None, "{raw} 不是合法状态");
        }
    }

    #[test]
    fn kanban_unknown_status_should_be_rejected_on_update() {
        let roster = test_roster(&[]);
        let mut issues = vec![test_issue(1, "议题", KANBAN_STATUS_TODO)];
        let mut next_number = 2u32;
        let err = kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(Some(1), None, Some("planning"), Some("想回到旧状态"))],
        )
        .expect_err("planning 已不是合法状态");
        assert!(err.contains("状态非法"), "要指明是状态值的问题：{err}");
        assert!(err.contains(KANBAN_STATUS_DONE), "报错要列出六态：{err}");
        assert_eq!(issues[0].status, KANBAN_STATUS_TODO, "被拒后状态必须原样不动");
    }

    // ---------- 新建与修改的状态约束 ----------

    #[test]
    fn kanban_create_should_reject_status_param() {
        // 新建是新建：不接受指定状态，一律从待议开始。
        let roster = test_roster(&[]);
        let mut issues: Vec<KanbanIssueEntry> = Vec::new();
        let mut next_number = 0u32;
        let err = kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
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
        assert!(issues.is_empty(), "被拒之后不能留下半条议题");
        assert_eq!(next_number, 0, "被拒之后不能消耗编号");
    }

    #[test]
    fn kanban_create_should_always_land_on_discussing() {
        let roster = test_roster(&[]);
        let mut issues: Vec<KanbanIssueEntry> = Vec::new();
        let mut next_number = 0u32;
        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(None, Some("议题"), None, Some("用户提了一句"))],
        )
        .expect("create");
        assert_eq!(issues[0].status, KANBAN_STATUS_DISCUSSING, "新建一律落在待议");
    }

    #[test]
    fn kanban_create_should_require_a_reason() {
        let roster = test_roster(&[]);
        let mut issues: Vec<KanbanIssueEntry> = Vec::new();
        let mut next_number = 0u32;
        let err = kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(None, Some("没写理由"), None, None)],
        )
        .expect_err("新建必须带理由");
        assert!(err.contains("note"), "要告诉它写在哪：{err}");
        assert!(issues.is_empty(), "被拒之后不能留下半条议题");

        // 纯空白同样不算理由。
        assert!(kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(None, Some("空白理由"), None, Some("   "))],
        )
        .is_err());
    }

    #[test]
    fn kanban_batch_should_write_nothing_when_one_item_is_rejected() {
        // 「整批不落库」现在靠「副本上校验 + 最后一次写回」保证，
        // 不能靠循环恰好先炸在前一条。这里让第一条合法、第二条违规。
        let roster = test_roster(&[]);
        let mut issues: Vec<KanbanIssueEntry> = Vec::new();
        let mut next_number = 0u32;
        let err = kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[
                upsert_item(None, Some("这条本来是合法的"), None, Some("用户说要记")),
                upsert_item(None, Some("这条没有理由"), Some(KANBAN_STATUS_TODO), None),
            ],
        )
        .expect_err("混入一条违规必须整批失败");
        assert!(!err.is_empty());
        assert!(issues.is_empty(), "整批失败后不能留下第一条");
        assert_eq!(next_number, 0, "失败的批量不该吃掉编号");
    }

    #[test]
    fn kanban_note_should_not_be_cleared_on_any_status() {
        // note 任何时候不得为空：任何状态的议题都不能被空值静默清掉上一条理由。
        for status in KANBAN_STATUSES {
            let roster = test_roster(&[]);
            let mut issue = test_issue(1, "议题", status);
            issue.note = "上一版理由".to_string();
            let mut issues = vec![issue];
            let mut next_number = 2u32;

            let err = kanban_apply_upsert_in(
                &mut issues,
                &mut next_number,
                &roster,
                &[upsert_item(Some(1), None, Some(status), Some(""))],
            )
            .expect_err(&format!("{status} 的 note 不能清空"));
            assert!(err.contains("note 不能清空"), "实际：{err}");
            assert_eq!(issues[0].note, "上一版理由", "被拒后 note 必须原样保留");

            // 纯空白同样不行。
            assert!(kanban_apply_upsert_in(
                &mut issues,
                &mut next_number,
                &roster,
                &[upsert_item(Some(1), None, None, Some("  "))],
            )
            .is_err());
        }
    }

    #[test]
    fn kanban_title_only_update_should_keep_note() {
        // 不传 status 只改标题这条路径由 note 校验兜住，不能顺手把理由抹掉。
        let roster = test_roster(&[]);
        let mut issue = test_issue(1, "旧标题", KANBAN_STATUS_IN_PROGRESS);
        issue.note = "上一版理由".to_string();
        let mut issues = vec![issue];
        let mut next_number = 2u32;

        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(Some(1), Some("新标题"), None, Some(""))],
        )
        .expect_err("只改标题也不能把 note 清空");

        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(Some(1), Some("新标题"), None, None)],
        )
        .expect("只改标题、不碰 note 应当成功");
        assert_eq!(issues[0].title, "新标题");
        assert_eq!(issues[0].note, "上一版理由");
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
        let roster = test_roster(&[]);
        let mut issues = vec![test_issue(1, "议题", KANBAN_STATUS_TODO)];
        let mut next_number = 2u32;
        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(Some(1), None, Some(KANBAN_STATUS_TODO), None)],
        )
        .expect("同状态重复写入应当放行");
        assert_eq!(issues[0].status, KANBAN_STATUS_TODO);
    }

    #[test]
    fn kanban_create_should_leave_plan_path_empty() {
        let roster = test_roster(&[]);
        let mut issues: Vec<KanbanIssueEntry> = Vec::new();
        let mut next_number = 0u32;
        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(None, Some("议题"), None, Some("用户提了一句"))],
        )
        .expect("create");
        assert!(issues[0].plan_path.is_empty(), "新建议题不带计划");
    }

    // ---------- 对外序列化面 ----------

    #[test]
    fn kanban_tool_output_should_not_expose_storage_id() {
        let roster = test_roster(&[]);
        let issue = test_issue(3, "接看板磁贴", KANBAN_STATUS_IN_REVIEW);
        let value = kanban_tool_issue_value(&kanban_issue_view(&issue, &roster));
        let text = serde_json::to_string(&value).expect("serialize");

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

    // ---------- 归属：对外写名字，对内认编号 ----------

    #[test]
    fn kanban_assignee_should_round_trip_through_roster() {
        let roster = test_roster(&[(1, "李工"), (2, "王工")]);
        for raw in ["leader", "member:李工", "member:王工", "none"] {
            let assignee = roster.resolve_assignee(raw).expect("parse");
            assert_eq!(roster.wire_assignee(&assignee), raw, "{raw} 往返不该变");
        }
        // 空值与 unassigned 一并落到 none，避免模型换个说法就报错
        for raw in ["  ", "unassigned"] {
            let assignee = roster.resolve_assignee(raw).expect("parse");
            assert_eq!(roster.wire_assignee(&assignee), KANBAN_ASSIGNEE_NONE);
        }
        assert!(roster.resolve_assignee("member:").is_err());
        // 编号不对模型暴露：模型还写编号时必须直接报错，不能当成角色名「3」收下
        let numeric = roster
            .resolve_assignee("member:3")
            .expect_err("编号写法必须被拒");
        assert!(numeric.contains("角色"), "{numeric}");
        assert!(roster.resolve_assignee("李工").is_err());
        // 不在队的人不能接活，报错要列出可选角色
        let missing = roster
            .resolve_assignee("member:张三")
            .expect_err("不在队要拒");
        assert!(missing.contains("李工"), "要列出可选角色：{missing}");
    }

    #[test]
    fn kanban_dismissed_member_issue_should_still_resolve() {
        // 请离只影响「能不能再接活」，不影响历史议题认人。
        let roster = test_roster_with_dismissed(&[(1, "李工")], &[(2, "已请离的王工")]);
        let issue = issue_of_member(1, "当年的事", KANBAN_STATUS_IN_PROGRESS, 2);
        let view = kanban_issue_view(&issue, &roster);
        assert_eq!(
            view.assignee, "member:已请离的王工",
            "请离的人名还翻得出来，不能变成 none"
        );
        assert_eq!(roster.member_label(2), "已请离的王工");
        assert!(
            roster.resolve_assignee("member:已请离的王工").is_err(),
            "请离的人不能再被指派"
        );
    }

    #[test]
    fn kanban_member_should_not_hold_two_active_issues() {
        let roster = test_roster(&[(1, "李工")]);
        let mut issues = vec![issue_of_member(
            1,
            "修复登录超时",
            KANBAN_STATUS_IN_PROGRESS,
            1,
        )];
        issues.push(test_issue(2, "补文档", KANBAN_STATUS_TODO));
        let mut next_number = 3u32;

        let err = kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
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
            issues[1].assignee.kind, KANBAN_ASSIGNEE_NONE,
            "被拒之后不能留下半截指派"
        );
    }

    #[test]
    fn kanban_single_active_should_be_judged_by_number_not_role() {
        // 判定键是名册编号：同名不同号互不影响，改过名的人还是同一个人。
        let roster = test_roster(&[(1, "李工"), (2, "李工")]);
        let mut issues = vec![issue_of_member(1, "一号的活", KANBAN_STATUS_IN_PROGRESS, 1)];
        issues.push(test_issue(2, "二号的活", KANBAN_STATUS_TODO));
        let mut next_number = 3u32;
        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item_with_assignee(
                Some(2),
                None,
                None,
                Some("member:李工"),
                None,
            )],
        )
        .expect("同名的另一个人不该被拦");
        let resolved = issues[1]
            .assignee
            .member_number
            .expect("派出去的人必须落到名册编号上");
        assert_ne!(
            resolved, 1,
            "解析到的是另一个编号的人，所以不该被 1 号的在办议题拦住"
        );
        assert_eq!(
            issues[0].assignee.member_number,
            Some(1),
            "已经在办的那条不能被顺手改掉"
        );

        // 同一编号换个名字派第二条，必须照样拦住。
        let renamed = test_roster(&[(2, "李明")]);
        let mut issues = vec![issue_of_member(1, "改名前埋的活", KANBAN_STATUS_IN_PROGRESS, 2)];
        issues.push(test_issue(2, "改名后派的新活", KANBAN_STATUS_TODO));
        let mut next_number = 3u32;
        let err = kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &renamed,
            &[upsert_item_with_assignee(
                Some(2),
                None,
                None,
                Some("member:李明"),
                None,
            )],
        )
        .expect_err("同一个人改了名，名下在办议题照样占着名额");
        assert!(err.contains("李明"), "报错要用名册里的当前名字：{err}");
    }

    #[test]
    fn kanban_done_issue_should_not_occupy_its_member() {
        let roster = test_roster(&[(1, "李工")]);
        let mut issues = vec![
            issue_of_member(1, "已经做完了", KANBAN_STATUS_DONE, 1),
            test_issue(2, "新活", KANBAN_STATUS_TODO),
        ];
        let mut next_number = 3u32;
        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
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
    fn kanban_same_batch_should_not_bind_two_issues_to_one_member() {
        // 逐条判会两条都放行，所以校验必须在整批算完最终态之后做。
        let roster = test_roster(&[(1, "李工")]);
        let mut issues = vec![
            test_issue(1, "第一件", KANBAN_STATUS_TODO),
            test_issue(2, "第二件", KANBAN_STATUS_TODO),
        ];
        let mut next_number = 3u32;
        let err = kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[
                upsert_item_with_assignee(Some(1), None, None, Some("member:李工"), None),
                upsert_item_with_assignee(Some(2), None, None, Some("member:李工"), None),
            ],
        )
        .expect_err("同一批里两条绑给同一个人必须整批失败");
        assert!(err.contains("李工"), "要点出是谁：{err}");
        assert_eq!(
            issues[0].assignee.kind, KANBAN_ASSIGNEE_NONE,
            "整批失败时第一条也不能落库"
        );
    }

    #[test]
    fn kanban_same_batch_should_allow_unbind_then_bind() {
        // 收口旧的、派新的常常写在同一批里，逐条判会把这种正常操作误判成冲突。
        let roster = test_roster(&[(1, "李工")]);
        let mut issues = vec![
            issue_of_member(1, "旧活", KANBAN_STATUS_TODO, 1),
            test_issue(2, "新活", KANBAN_STATUS_TODO),
        ];
        let mut next_number = 3u32;
        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[
                upsert_item_with_assignee(Some(1), None, None, Some("none"), None),
                upsert_item_with_assignee(Some(2), None, None, Some("member:李工"), None),
            ],
        )
        .expect("同一批里先解绑再绑新的应当放行");
        assert_eq!(issues[1].assignee.member_number, Some(1));
    }

    #[test]
    fn kanban_reopening_done_issue_should_still_respect_single_active() {
        let roster = test_roster(&[(1, "李工")]);
        let mut issues = vec![
            issue_of_member(1, "在办", KANBAN_STATUS_IN_PROGRESS, 1),
            issue_of_member(2, "做完了", KANBAN_STATUS_DONE, 1),
        ];
        let mut next_number = 3u32;
        let err = kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(
                Some(2),
                None,
                Some(KANBAN_STATUS_IN_PROGRESS),
                Some("用户要求重开"),
            )],
        )
        .expect_err("重开后这个人就有两条在办了");
        assert!(err.contains("#1"), "要点出冲突的是哪一条：{err}");
        assert_eq!(issues[1].status, KANBAN_STATUS_DONE, "被拒后状态必须原样不动");
    }

    #[test]
    fn kanban_finishing_issue_should_require_unbinding_first() {
        let roster = test_roster(&[(1, "李工")]);
        let mut issues = vec![issue_of_member(
            1,
            "做完的活",
            KANBAN_STATUS_IN_PROGRESS,
            1,
        )];
        let mut next_number = 2u32;
        let err = kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
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
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item_with_assignee(
                Some(1),
                None,
                Some(KANBAN_STATUS_DONE),
                Some("none"),
                Some("做完了"),
            )],
        )
        .expect("先摘人再收口应当放行");
        assert_eq!(issues[0].assignee.kind, KANBAN_ASSIGNEE_NONE);
    }

    #[test]
    fn kanban_reopening_finished_issue_should_pass_without_other_active() {
        let roster = test_roster(&[(1, "李工")]);
        let mut issues = vec![issue_of_member(1, "做完了", KANBAN_STATUS_DONE, 1)];
        let mut next_number = 2u32;
        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
            &[upsert_item(
                Some(1),
                None,
                Some(KANBAN_STATUS_TODO),
                Some("用户要求重开"),
            )],
        )
        .expect("重开成待办后它重新占人，但此人没有别的在办");
        assert_eq!(issues[0].status, KANBAN_STATUS_TODO);
    }

    #[test]
    fn kanban_assignee_blacklist_should_reject_discussing_done() {
        for status in [KANBAN_STATUS_DISCUSSING, KANBAN_STATUS_DONE] {
            let roster = test_roster(&[(1, "李工")]);
            let mut issues = vec![issue_of_member(1, "议题", KANBAN_STATUS_TODO, 1)];
            let mut next_number = 2u32;
            let err = kanban_apply_upsert_in(
                &mut issues,
                &mut next_number,
                &roster,
                &[upsert_item(Some(1), None, Some(status), Some("改状态"))],
            )
            .expect_err("这两态不允许挂负责人");
            assert!(
                err.contains("不归任何人办"),
                "{status} 的报错要说清原因：{err}"
            );
            assert!(err.contains("李工"), "{status} 的报错要点出还挂着谁：{err}");
            assert_eq!(
                issues[0].status, KANBAN_STATUS_TODO,
                "{status} 被拒后状态原样不动"
            );
        }
    }

    #[test]
    fn kanban_assignee_should_stay_allowed_on_review_and_blocked() {
        // 待审是等用户点头，人还在名下；阻塞是卡住了，责任人更该留上面。
        for status in [KANBAN_STATUS_IN_REVIEW, KANBAN_STATUS_BLOCKED] {
            let roster = test_roster(&[(1, "李工")]);
            let mut issues = vec![issue_of_member(1, "议题", KANBAN_STATUS_IN_PROGRESS, 1)];
            let mut next_number = 2u32;
            kanban_apply_upsert_in(
                &mut issues,
                &mut next_number,
                &roster,
                &[upsert_item(Some(1), None, Some(status), Some("推进"))],
            )
            .expect("待审与阻塞保留负责人");
            assert_eq!(issues[0].assignee.member_number, Some(1), "{status} 不该被摘人");
        }
    }

    #[test]
    fn kanban_create_should_reject_assignee() {
        // 新建固定落待议，而待议不挂人：只能先建、下一次再改状态顺带派人。
        let roster = test_roster(&[(1, "李工")]);
        for assignee in ["member:李工", "leader"] {
            let mut issues: Vec<KanbanIssueEntry> = Vec::new();
            let mut next_number = 0u32;
            let err = kanban_apply_upsert_in(
                &mut issues,
                &mut next_number,
                &roster,
                &[upsert_item_with_assignee(
                    None,
                    Some("想建了就派人"),
                    None,
                    Some(assignee),
                    Some("用户提了一句"),
                )],
            )
            .expect_err("新建不接受负责人");
            assert!(
                err.contains("不挂负责人"),
                "{assignee} 的报错要说清原因：{err}"
            );
            assert!(
                err.contains("再带编号"),
                "{assignee} 的报错要引导成两步：{err}"
            );
            assert!(issues.is_empty(), "被拒之后不能留下半条议题");
        }
    }

    // ---------- 看板页点「通过」 ----------

    #[test]
    fn kanban_approve_transition_should_respect_from_status() {
        // 点「通过」只在待审时推进，先到先得：状态不符时不能覆盖别人的改动。
        let roster = test_roster(&[]);
        let mut issues = vec![test_issue(1, "一", KANBAN_STATUS_IN_PROGRESS)];
        let err = kanban_approve_in(&mut issues, &roster, 1, "用户确认")
            .expect_err("当前不是待审");
        assert!(err.contains(KANBAN_STATUS_IN_PROGRESS), "要说清当前是什么状态：{err}");
        assert_eq!(issues[0].status, KANBAN_STATUS_IN_PROGRESS, "被拒后状态原样不动");

        issues[0].status = KANBAN_STATUS_IN_REVIEW.to_string();
        let view = kanban_approve_in(&mut issues, &roster, 1, "用户确认").expect("approve");
        assert_eq!(view.status, KANBAN_STATUS_DONE);
        assert_eq!(issues[0].version, 1, "通过也是一次写入，版本要加");
    }

    #[test]
    fn kanban_approve_transition_should_clear_assignee() {
        // 界面上点「通过」是第二写入口：它把待审推成已完成，
        // 必须顺手把人摘掉，否则点一次就造出一条「已完成还挂着人」的违规数据。
        let roster = test_roster(&[(1, "李工")]);
        let mut issues = vec![
            issue_of_member(1, "议题", KANBAN_STATUS_IN_REVIEW, 1),
            test_issue(2, "新活", KANBAN_STATUS_TODO),
        ];
        let mut next_number = 3u32;
        kanban_approve_in(&mut issues, &roster, 1, "用户确认").expect("approve");
        assert_eq!(issues[0].status, KANBAN_STATUS_DONE);
        assert_eq!(
            issues[0].assignee.kind, KANBAN_ASSIGNEE_NONE,
            "点通过之后这名队员必须被摘下来"
        );

        // 摘干净了他就能接新活。
        kanban_apply_upsert_in(
            &mut issues,
            &mut next_number,
            &roster,
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

    // ---------- 列表过滤 ----------

    #[test]
    fn kanban_list_should_filter_by_status_and_assignee() {
        let roster = test_roster(&[(1, "李工"), (2, "王工")]);
        let issues = vec![
            issue_of_member(1, "等通过", KANBAN_STATUS_IN_REVIEW, 1),
            issue_of_leader(2, "待办", KANBAN_STATUS_TODO),
        ];

        assert_eq!(
            kanban_board_list_in(&issues, &roster, None, None, None).len(),
            2
        );
        assert_eq!(
            kanban_board_list_in(&issues, &roster, Some(KANBAN_STATUS_IN_REVIEW), None, None).len(),
            1
        );
        // 只按归属过滤：参数位不能错位，写反了就会串味
        assert_eq!(
            kanban_board_list_in(
                &issues,
                &roster,
                None,
                Some(KANBAN_ASSIGNEE_MEMBER),
                Some("李工")
            )
            .len(),
            1
        );
        // 同 kind 不同角色不能串味：name 维度必须真的参与过滤
        assert_eq!(
            kanban_board_list_in(
                &issues,
                &roster,
                None,
                Some(KANBAN_ASSIGNEE_MEMBER),
                Some("王工")
            )
            .len(),
            0
        );
        assert_eq!(
            kanban_board_list_in(
                &issues,
                &roster,
                Some(KANBAN_STATUS_IN_REVIEW),
                Some(KANBAN_ASSIGNEE_MEMBER),
                Some("李工")
            )
            .len(),
            1
        );
        assert_eq!(
            kanban_board_list_in(&issues, &roster, Some(KANBAN_STATUS_DONE), None, None).len(),
            0
        );
    }

    /// 名册里查不到的角色不能退化成「不过滤」：给了角色，过滤维度就必须生效。
    #[test]
    fn kanban_list_should_return_empty_for_role_outside_roster() {
        let roster = test_roster(&[(1, "李工")]);
        let issues = vec![
            issue_of_member(1, "李工的活", KANBAN_STATUS_TODO, 1),
            issue_of_leader(2, "队长的活", KANBAN_STATUS_TODO),
        ];

        assert!(
            kanban_board_list_in(
                &issues,
                &roster,
                None,
                Some(KANBAN_ASSIGNEE_MEMBER),
                Some("查无此人")
            )
            .is_empty(),
            "名册外的角色必须过滤成空列表，不能把别人的议题当成他的返回"
        );
        assert!(
            kanban_board_list_in(&issues, &roster, None, None, Some("查无此人")).is_empty(),
            "只给角色不给 kind 时，同样不能漏出整块板"
        );
        assert_eq!(
            kanban_board_list_in(&issues, &roster, None, None, Some("李工")).len(),
            1,
            "名册内的人照旧查得到，别把过滤条件写没了"
        );
    }

    // ---------- 设置计划文件 ----------

    /// 临时目录本身是绝对路径，各平台都用它拼，避免写死盘符。
    fn absolute_plan_path(name: &str) -> String {
        std::env::temp_dir().join(name).to_string_lossy().to_string()
    }

    #[test]
    fn kanban_set_plan_should_write_path() {
        let mut issues = vec![test_issue(1, "议题", KANBAN_STATUS_TODO)];
        let version_before = issues[0].version;
        let raw = absolute_plan_path("kanban-set-plan-ok.md");
        let result = kanban_set_plan_in(&mut issues, 1, &raw).expect("set plan");
        assert_eq!(result.number, 1);
        assert_eq!(result.plan_path, raw);
        assert_eq!(issues[0].plan_path, raw, "路径要落到议题上");
        assert_eq!(
            issues[0].version,
            version_before + 1,
            "设置计划也是一次写入，版本必须 +1"
        );
    }

    #[test]
    fn kanban_set_plan_should_overwrite_previous() {
        // 没有卸载动作，换计划就是直接覆盖：第二次设置不是错误。
        let mut issues = vec![test_issue(1, "议题", KANBAN_STATUS_TODO)];
        let first = absolute_plan_path("first.md");
        let second = absolute_plan_path("second.md");
        kanban_set_plan_in(&mut issues, 1, &first).expect("first");
        let result = kanban_set_plan_in(&mut issues, 1, &second).expect("覆盖更新不该报错");
        assert_eq!(result.plan_path, second);
        assert_eq!(issues[0].plan_path, second, "留下的必须是新路径");
    }

    #[test]
    fn kanban_set_plan_should_reject_empty_path() {
        // 不能卸载，所以空值等于要求清空，必须拒。
        let mut issues = vec![test_issue(1, "议题", KANBAN_STATUS_TODO)];
        let existing = absolute_plan_path("keep.md");
        kanban_set_plan_in(&mut issues, 1, &existing).expect("set");

        for blank in ["", "   ", "\t\n"] {
            let err = kanban_set_plan_in(&mut issues, 1, blank).expect_err("空路径要拒");
            assert!(err.contains("只能更新不能清空"), "要说清为什么不能空：{err}");
        }
        assert_eq!(issues[0].plan_path, existing, "被拒的写入不能碰掉已有路径");
    }

    #[test]
    fn kanban_set_plan_should_not_require_file_exists() {
        // 计划可能还没落盘，设置只管记路径；文件在不在由模型自己去看。
        let mut issues = vec![test_issue(1, "议题", KANBAN_STATUS_TODO)];
        let missing = std::env::temp_dir().join("kanban-set-plan-not-there-9f3a.md");
        std::fs::remove_file(&missing).ok();
        let raw = missing.to_string_lossy().to_string();
        let result = kanban_set_plan_in(&mut issues, 1, &raw).expect("不校验文件是否存在");
        assert_eq!(result.plan_path, raw);
    }

    #[test]
    fn kanban_set_plan_should_reject_unknown_issue() {
        let mut issues = vec![test_issue(1, "议题", KANBAN_STATUS_TODO)];
        let err = kanban_set_plan_in(&mut issues, 7, &absolute_plan_path("x.md"))
            .expect_err("没有这条议题");
        assert!(err.contains("#7"), "要指出是哪一条：{err}");
    }

    #[test]
    fn kanban_plan_path_should_require_absolute() {
        assert!(
            kanban_set_plan_path_checked("plan/chat/x.md").is_err(),
            "相对路径没法定位"
        );
        assert!(kanban_set_plan_path_checked("   ").is_err(), "空值等于要求清空");
        assert!(kanban_set_plan_path_checked(&absolute_plan_path("x.md")).is_ok());
    }

    // ---------- 与会话同生共死：读写都走会话自己的元数据 ----------

    fn test_leader_conversation(conversation_id: &str, issues: Vec<KanbanIssueEntry>) -> Conversation {
        let now = now_iso();
        let mut conversation = crate::tests::test_chat_conversation(conversation_id, "active", &now);
        conversation.issues = issues;
        conversation.issue_next_number = conversation.issues.iter().map(|issue| issue.number).max().unwrap_or(0) + 1;
        conversation
    }

    #[test]
    fn kanban_issue_should_survive_flush_to_disk() {
        // 写入只改缓存，落盘是后置的：这里把整条链路跑到盘上再读回来。
        let state = crate::tests::test_chat_runtime_state();
        let leader = "conv-kanban-flush";
        let conversation = test_leader_conversation(leader, Vec::new());
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation).expect("mark persisted");

        kanban_apply_upsert(
            &state,
            leader,
            &[upsert_item(None, Some("落盘的议题"), None, Some("用户提了一句"))],
        )
        .expect("create issue");
        let wrote = flush_pending_persists_blocking(&state).expect("flush pending");
        assert!(wrote, "元数据变更必须真的落盘");

        let restored = read_conversation_shard(&state.data_path, leader).expect("read back");
        assert_eq!(restored.issues.len(), 1);
        assert_eq!(restored.issues[0].title, "落盘的议题");
        assert_eq!(restored.issues[0].number, 1);
        assert_eq!(restored.issue_next_number, 2, "编号游标也要一起落盘");
    }

    #[test]
    fn kanban_issues_should_vanish_with_leader_conversation() {
        // 红豆的验收项：会话删了，它名下的议题必须跟着走，不许留孤儿。
        let state = crate::tests::test_chat_runtime_state();
        let leader = "conv-kanban-cascade";
        let conversation = test_leader_conversation(
            leader,
            vec![
                issue_of_leader(1, "删会话也要没的议题", KANBAN_STATUS_IN_PROGRESS),
                test_issue(2, "另一条", KANBAN_STATUS_TODO),
            ],
        );
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation).expect("mark persisted");
        assert!(
            kanban_issue_exists(&state, leader, 1).expect("exists before delete"),
            "删之前先确认议题确实在"
        );

        conversation_service_v2()
            .delete_conversation(&state, leader)
            .expect("delete leader conversation");
        flush_pending_persists_blocking(&state).expect("flush pending");

        assert!(
            kanban_read_board(&state, leader).is_err(),
            "会话没了，它那块板也读不出来了"
        );
        assert!(
            kanban_issue_exists(&state, leader, 1).is_err(),
            "查议题必须跟着会话一起失败，不能回一个空列表"
        );
        assert!(
            read_conversation_shard(&state.data_path, leader).is_err(),
            "盘上也不该再留着这条会话的元数据"
        );
    }

    #[test]
    fn kanban_active_issue_for_member_should_pick_single_issue_by_number() {
        // 概览按编号直接比对，不走角色名映射。
        let state = crate::tests::test_chat_runtime_state();
        let leader = "conv-kanban-active";
        let conversation = test_leader_conversation(
            leader,
            vec![
                issue_of_member(5, "后派的活", KANBAN_STATUS_TODO, 1),
                issue_of_member(2, "更早的活", KANBAN_STATUS_IN_PROGRESS, 1),
                issue_of_member(3, "别人的活", KANBAN_STATUS_IN_PROGRESS, 2),
                issue_of_leader(4, "队长自己的活", KANBAN_STATUS_IN_PROGRESS),
            ],
        );
        write_conversation_shard(&state.data_path, &conversation).expect("write conversation");
        state_mark_conversation_direct_persisted(&state, &conversation).expect("mark persisted");

        let mut session = KanbanSession::new();
        let issue = session
            .active_issue_for_member(&state, leader, 1)
            .expect("一号队员有一条在办");
        assert_eq!(issue.number, 2, "同名下多条时取编号最小那条");
        assert_eq!(
            session
                .active_issue_for_member(&state, leader, 3)
                .map(|issue| issue.number),
            None,
            "没有议题的人不能借到别人的"
        );
    }
}
