// ==================== 队伍议题看板工具 ====================
// 队长专属。工具对外只用 number 定位，id 不出现在入参、返回值与错误文案里。
// 查与改拆成两个工具：看板查询与议题写入是两种不同的动作，各自只带自己的参数，
// 模型不必再先选 action 再记哪些字段属于哪一支。
pub(crate) const KANBAN_LIST_TOOL_NAME: &str = "kanban_list";
pub(crate) const KANBAN_UPSERT_TOOL_NAME: &str = "kanban_upsert";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanListRequest {
    /// 必填：按状态过滤，可多选。
    #[serde(default)]
    pub(crate) status: Vec<String>,
    /// 可选：按谁在做过滤。
    #[serde(default)]
    pub(crate) assignee: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanUpsertRequest {
    /// 必填：带 number 是改，不带是新建。
    #[serde(default)]
    pub(crate) issues: Vec<KanbanIssueUpsertItem>,
}

pub(crate) fn kanban_list_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        KANBAN_LIST_TOOL_NAME,
        "查队伍议题看板。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": KANBAN_STATUSES
                    },
                    "description": "按状态过滤，可多选"
                },
                "assignee": {
                    "type": "string",
                    "description": "按归属过滤：leader / member:<角色名> / none"
                }
            },
            "required": ["status"]
        }),
    )
}

pub(crate) fn kanban_upsert_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        KANBAN_UPSERT_TOOL_NAME,
        "新建或修改队伍议题。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "issues": {
                    "type": "array",
                    "description": "要新建或修改的议题",
                    "items": {
                        "type": "object",
                        "properties": {
                            "number": {
                                "type": "integer",
                                "description": "改哪一条议题。留空表示新建"
                            },
                            "title": {
                                "type": "string",
                                "description": "议题标题。新建时必填；修改时留空表示不改"
                            },
                            "status": {
                                "type": "string",
                                "enum": KANBAN_STATUSES,
                                "description": "目标状态。新建时不传就从 discussing 开始"
                            },
                            "assignee": {
                                "type": "string",
                                "description": "谁在做：leader / member:<角色名> / none。新建时不传就是 leader"
                            },
                            "note": {
                                "type": "string",
                                "description": "本次变更的理由"
                            }
                        }
                    }
                }
            },
            "required": ["issues"]
        }),
    )
}

/// 一条议题排成一行纯文本：`#12 [review] 标题 · 木偶 · plan:/path — 上一条理由`。
///
/// 结果区是纯文本渲染，所以回执直接给能读的行，而不是留给前端拼表格。
/// 刻意不含 id / 创建时间 / 版本号：模型不需要它们，版本号出现在回执里还会引诱模型去维护它。
/// `note` 必须留：它是库里存下来的状态理由，不是入参回显，list 时模型靠它才看得到为什么阻塞、为什么待审。
fn kanban_tool_issue_line(view: &KanbanIssueView) -> String {
    let mut line = format!("#{} [{}] {}", view.number, view.status.trim(), view.title.trim());
    let assignee = view.assignee.trim();
    if !assignee.is_empty() {
        line.push_str(" · ");
        line.push_str(assignee);
    }
    let plan = view.plan_path.trim();
    if !plan.is_empty() {
        line.push_str(" · plan:");
        line.push_str(plan);
    }
    let note = view.note.trim();
    if !note.is_empty() {
        line.push_str(" — ");
        line.push_str(note);
    }
    line
}

fn kanban_tool_issue_lines(views: &[KanbanIssueView]) -> String {
    views
        .iter()
        .map(|view| format!("- {}", kanban_tool_issue_line(view)))
        .collect::<Vec<_>>()
        .join("\n")
}

// ==================== 待审反思提示 ====================
// 议题推进到待审时，模型正好停在「活干完了、还没收口」这个断点上。
// 这里是提炼经验唯一自然的时机：再往后用户点了头，注意力就散了；再往前活还没成形。
// 判据取本次入参的 status，而不是改完之后的库内状态——把一条本来就停在待审的议题
// 再设一次待审，同样算「把议题设置为待审」这个动作，提示照给。

fn kanban_pending_review_numbers(items: &[KanbanIssueUpsertItem]) -> Vec<u32> {
    items
        .iter()
        .filter(|item| {
            item.status.as_deref().and_then(kanban_status_normalized) == Some(KANBAN_STATUS_IN_REVIEW)
        })
        .filter_map(|item| item.issue_number)
        .collect()
}

fn kanban_review_reflection_hint(numbers: &[u32]) -> String {
    let target = numbers
        .iter()
        .map(|number| format!("{number} 号议题"))
        .collect::<Vec<_>>()
        .join("、");
    format!(
        "{target}已进入待审。收口之前先确认审查结论到了没有——没派就现在派，\
         汇报里写清是已通过、审查中、已打回还是尚未派出，自己核验不算审查。\
         然后回看这次执行：流程是怎么走的、哪里卡住过、最后靠什么验明的。\
         有值得沉淀成 skill 的，就趁现在整理出来——多数议题至少能挖出一小条，哪怕很小。"
    )
}

// 改成解决中就该派活了，而派活前计划得先挂上：没挂，队员拿到的就是一句没有落点的活。
// 判据取改完之后的库内状态与 plan_path，不取入参——重设一次已经是解决中的议题同样算数。

fn kanban_in_progress_without_plan(issue: &KanbanIssueView) -> bool {
    issue.status == KANBAN_STATUS_IN_PROGRESS && issue.plan_path.trim().is_empty()
}

fn kanban_in_progress_plan_hint() -> String {
    "如果有计划，请确保挂上对应计划之后再派活".to_string()
}

pub(crate) fn builtin_kanban_list(
    app_state: &AppState,
    session_id: &str,
    args: KanbanListRequest,
) -> Result<String, String> {
    let leader_conversation_id = kanban_leader_conversation_id(app_state, session_id)?;
    let mut statuses = Vec::<&str>::new();
    for raw in &args.status {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let normalized = kanban_status_normalized(raw).ok_or_else(|| {
            format!(
                "kanban_list.status 非法：{raw}；可选 {}",
                KANBAN_STATUSES.join(" / ")
            )
        })?;
        if !statuses.contains(&normalized) {
            statuses.push(normalized);
        }
    }
    if statuses.is_empty() {
        return Err(format!(
            "kanban_list.status 必填；可选 {}",
            KANBAN_STATUSES.join(" / ")
        ));
    }
    let assignee = match args
        .assignee
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(raw) => Some(kanban_assignee_from_wire(raw)?),
        None => None,
    };
    let issues = kanban_board_list(
        app_state,
        &leader_conversation_id,
        &statuses,
        assignee.as_ref().map(|(kind, _)| kind.as_str()),
        assignee.as_ref().and_then(|(_, role)| role.as_deref()),
    )?;
    Ok(if issues.is_empty() {
        "no issues".to_string()
    } else {
        format!(
            "{} issue{}\n{}",
            issues.len(),
            if issues.len() == 1 { "" } else { "s" },
            kanban_tool_issue_lines(&issues)
        )
    })
}

pub(crate) fn builtin_kanban_upsert(
    app_state: &AppState,
    session_id: &str,
    args: KanbanUpsertRequest,
) -> Result<String, String> {
    let leader_conversation_id = kanban_leader_conversation_id(app_state, session_id)?;
    let review_numbers = kanban_pending_review_numbers(&args.issues);
    let result = kanban_apply_upsert(app_state, &leader_conversation_id, &args.issues)?;
    kanban_emit_updated(app_state, &leader_conversation_id);
    // 只回这次的变更：整张看板要看得用 kanban_list。
    let mut lines = Vec::<String>::new();
    let mut unplanned_in_progress = Vec::<u32>::new();
    for issue in &result.created {
        lines.push(format!("created {}", kanban_tool_issue_line(issue)));
        if kanban_in_progress_without_plan(issue) {
            unplanned_in_progress.push(issue.number);
        }
    }
    for issue in &result.updated {
        lines.push(format!("updated {}", kanban_tool_issue_line(issue)));
        if kanban_in_progress_without_plan(issue) {
            unplanned_in_progress.push(issue.number);
        }
    }
    if lines.is_empty() {
        lines.push("nothing changed".to_string());
    }
    if !unplanned_in_progress.is_empty() {
        lines.push(String::new());
        lines.push(kanban_in_progress_plan_hint());
    }
    if !review_numbers.is_empty() {
        lines.push(String::new());
        lines.push(kanban_review_reflection_hint(&review_numbers));
    }
    Ok(lines.join("\n"))
}
