// ==================== 队伍议题看板工具 ====================
// 队长专属。工具对外只用 number 定位，id 不出现在入参、返回值与错误文案里。
pub(crate) const KANBAN_TOOL_NAME: &str = "kanban";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanToolRequest {
    /// list / upsert。
    pub(crate) action: String,
    /// list 时可选：按状态过滤。
    #[serde(default)]
    pub(crate) status: Option<String>,
    /// list 时可选：按归属过滤（leader / member:<角色名> / none）。
    #[serde(default)]
    pub(crate) assignee: Option<String>,
    /// upsert 时必填：带 number 是改，不带是新建。
    #[serde(default)]
    pub(crate) issues: Vec<KanbanIssueUpsertItem>,
}

pub(crate) fn kanban_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        KANBAN_TOOL_NAME,
        "队伍议题看板工具（队长专属）。用编号维护队伍级议题：list 查、upsert 增改。\
         议题只增不删：没有删除动作，一条议题被记下来就永久留在板上。\
         状态六态：discussing（待议）/ todo（待办）/ in_progress（解决中）/ in_review（待审）/ \
         blocked（阻塞）/ done（已完成）。不做了、作废了就是 blocked，没有单独的放弃态。\
         任何状态都可以切到任何状态，没有顺序限制：已完成能改回解决中。\
         每次改状态都必须在 note 里写清这次为什么改；note 任何时候都不能为空。\
         同一个人可以同时挂多条在办议题（未完成的都算在办），不必等旧的收口；\
         队员概览只显示他名下编号最小的那条。\
         待议、已完成这两态不挂负责人，改成它们前先把 assignee 写成 none。\
         新建的议题一律从 discussing（待议）开始，不接受指定状态，也不能顺手挂负责人。\
         编号单调递增且不回收，编号不存在会报错、不会自动新建。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "upsert"],
                    "description": "list=查询看板；upsert=新建或修改议题"
                },
                "status": {
                    "type": "string",
                    "description": "仅 list 使用：按状态过滤，留空返回全部",
                    "enum": KANBAN_STATUSES
                },
                "assignee": {
                    "type": "string",
                    "description": "仅 list 使用：按归属过滤，写法同 issues[].assignee"
                },
                "issues": {
                    "type": "array",
                    "description": "仅 upsert 使用：要新建或修改的议题列表",
                    "items": {
                        "type": "object",
                        "properties": {
                            "number": {
                                "type": "integer",
                                "description": "改哪一条议题。留空表示新建一条（新议题的编号由系统分配）"
                            },
                            "title": {
                                "type": "string",
                                "description": "议题标题。新建时必填；修改时留空表示不改"
                            },
                            "status": {
                                "type": "string",
                                "enum": KANBAN_STATUSES,
                                "description": "目标状态。仅修改已有议题时使用：新建不接受 status，一律从 discussing（待议）开始"
                            },
                            "assignee": {
                                "type": "string",
                                "description": "谁在做：leader=队长自己；member:<队员角色>=某个队员（角色就是人格名，与 team_info 里的 role 一致）；none=未分配。留空表示不改"
                            },
                            "note": {
                                "type": "string",
                                "description": "本次变更的理由。改状态时必填；新建时必填；任何时候都不能为空，也不能用空值把已有的理由清掉"
                            }
                        }
                    }
                }
            },
            "required": ["action"]
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

fn kanban_tool_action_normalized(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "list" => Some("list"),
        "upsert" => Some("upsert"),
        _ => None,
    }
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
        "{target}已进入待审。趁收口之前回看一遍这次执行：流程是怎么走的、哪里卡住过、最后靠什么验明的。\
         有值得沉淀成 skill 的，就趁现在整理出来——多数议题至少能挖出一小条，哪怕很小。\
         空手而归不算中性结果，是一次被漏掉的学习机会。"
    )
}

pub(crate) fn builtin_kanban(
    app_state: &AppState,
    session_id: &str,
    args: KanbanToolRequest,
) -> Result<String, String> {
    let leader_conversation_id = kanban_leader_conversation_id(app_state, session_id)?;
    let action = kanban_tool_action_normalized(&args.action).ok_or_else(|| {
        format!(
            "kanban.action 非法：{}；可选 list / upsert",
            args.action.trim()
        )
    })?;

    let text = match action {
        "list" => {
            let status = match args.status.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                Some(raw) => {
                    let normalized = kanban_status_normalized(raw).ok_or_else(|| {
                        format!(
                            "kanban.status 非法：{raw}；可选 {}",
                            KANBAN_STATUSES.join(" / ")
                        )
                    })?;
                    Some(normalized)
                }
                None => None,
            };
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
                status,
                assignee.as_ref().map(|(kind, _)| kind.as_str()),
                assignee.as_ref().and_then(|(_, role)| role.as_deref()),
            )?;
            if issues.is_empty() {
                "no issues".to_string()
            } else {
                format!(
                    "{} issue{}\n{}",
                    issues.len(),
                    if issues.len() == 1 { "" } else { "s" },
                    kanban_tool_issue_lines(&issues)
                )
            }
        }
        "upsert" => {
            let review_numbers = kanban_pending_review_numbers(&args.issues);
            let result = kanban_apply_upsert(app_state, &leader_conversation_id, &args.issues)?;
            kanban_emit_updated(app_state, &leader_conversation_id);
            // 只回这次的变更：整张看板要看得用 action=list。
            let mut lines = Vec::<String>::new();
            for issue in &result.created {
                lines.push(format!("created {}", kanban_tool_issue_line(issue)));
            }
            for issue in &result.updated {
                lines.push(format!("updated {}", kanban_tool_issue_line(issue)));
            }
            if lines.is_empty() {
                lines.push("nothing changed".to_string());
            }
            if !review_numbers.is_empty() {
                lines.push(String::new());
                lines.push(kanban_review_reflection_hint(&review_numbers));
            }
            lines.join("\n")
        }
        _ => return Err(format!("kanban.action 不支持：{action}")),
    };

    Ok(text)
}
