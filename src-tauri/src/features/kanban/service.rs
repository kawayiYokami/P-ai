// ==================== 队伍议题看板：业务编排 ====================
// 议题存在队长会话的元数据里，这里管「什么写入算合法」：
// 主会话身份、编号分配、状态值合法性、改状态与清空 note 都要求理由。
//
// 读改写都在一次会话元数据更新里完成：校验用副本，全部通过才写回。
// 因此不存在「改了一半失败」的中间态——失败时元数据一行都没动。

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KanbanUpsertResult {
    created: Vec<KanbanIssueView>,
    updated: Vec<KanbanIssueView>,
}

// ==================== 名册视角 ====================

/// 构造一块看板的成员视角：名册编号 ⇄ 角色名。
///
/// 议题里只存编号，收发边界上才翻译成名字，翻译必须带上名册。
/// 角色名取「队员会话当前绑定的人格名」，人格没了才退回名册里那份旧名字，
/// 所以这里读的是一次组织快照加每个队员的一次会话元数据。
/// 只给单条写入与看板页用；概览按编号直接比对，不走这条路径。
fn kanban_board_roster(
    state: &AppState,
    leader_conversation_id: &str,
) -> Result<KanbanBoardRoster, String> {
    let active = team_member_number_roles(state, leader_conversation_id, false)?;
    let all = team_member_number_roles(state, leader_conversation_id, true)?;
    Ok(KanbanBoardRoster::new(active, all))
}

// ==================== 读取 ====================

fn kanban_read_board(
    state: &AppState,
    leader_conversation_id: &str,
) -> Result<(Vec<KanbanIssueEntry>, u32), String> {
    let meta = conversation_service_v2().get_conversation_meta(state, leader_conversation_id)?;
    Ok((meta.issues.clone(), meta.issue_next_number))
}

/// 议题是否存在。只回布尔：给队伍派活那条硬约束用。
pub(crate) fn kanban_issue_exists(
    state: &AppState,
    leader_conversation_id: &str,
    number: u32,
) -> Result<bool, String> {
    let meta = conversation_service_v2().get_conversation_meta(state, leader_conversation_id)?;
    Ok(kanban_find_issue(meta.issues.as_slice(), number).is_some())
}

// ==================== 编号 ====================

fn kanban_find_issue(issues: &[KanbanIssueEntry], number: u32) -> Option<&KanbanIssueEntry> {
    issues.iter().find(|issue| issue.number == number)
}

fn kanban_max_number(issues: &[KanbanIssueEntry]) -> u32 {
    issues.iter().map(|issue| issue.number).max().unwrap_or(0)
}

/// 分配下一个编号。规则与问题共用 `conversation_allocate_number`：
/// 从游标取，游标没建过就以「当前最大 + 1」起步；分配即自增，删除不归还。
fn kanban_allocate_number(issues: &[KanbanIssueEntry], next_number: &mut u32) -> u32 {
    conversation_allocate_number(kanban_max_number(issues), next_number)
}

// ==================== 前台会话定位 ====================

/// 从工具 session_id 取 conversation_id。
/// 主会话的 session_id 也带 `agent::` 前缀，所以不能按「有没有 ::」判断归属，
/// 那会把队长自己一起拒掉；这里只负责取出会话 id，身份交给下面对会话本身判断。
fn kanban_session_conversation_id(session_id: &str) -> Result<String, String> {
    let trimmed = session_id.trim();
    if trimmed.is_empty() {
        return Err("kanban 工具缺少 conversation_id，无法定位当前会话".to_string());
    }
    match delegate_session_conversation_id(trimmed) {
        Some(conversation_id) => Ok(conversation_id),
        // 前端命令直接传的就是 conversation_id，没有 `agent::` 段
        None => Ok(trimmed.to_string()),
    }
}

/// 定位当前会话所属的看板。
/// 看板挂在本地普通会话（kind == chat）上：追问会话、队员会话、委托会话、
/// 远程联系人一律不属于主会话，直接拒绝——它们没有独立看板，议题记在队长那块板上。
fn kanban_leader_conversation_id(state: &AppState, session_id: &str) -> Result<String, String> {
    let conversation_id = kanban_session_conversation_id(session_id)?;
    let meta = conversation_service_v2().get_conversation_meta(state, &conversation_id)?;
    if meta.conversation_kind.trim() != CONVERSATION_KIND_CHAT
        || meta.is_delegate
        || meta.is_remote_im_contact
        || meta.team.membership.is_some()
    {
        return Err("kanban 只能在主会话使用：队员会话与追问会话没有独立看板，请让队长维护议题"
            .to_string());
    }
    Ok(meta.id.trim().to_string())
}

// ==================== 校验 ====================

/// 改状态的前置校验：状态值有没有变、变了就得说明理由。
///
/// 判据是**这次请求带没带 note**，不是最终存着什么。否则模型把旧值原样带上来
/// 就能过闸，「每次变更都留理由」会退化成「这条议题以前写过理由」。
///
/// 同状态重复写入不算变更，不要求理由；只改标题、只改负责人也走不到这里。
fn kanban_next_state_checked(
    number: u32,
    current_status: &str,
    next_status: &str,
    requested_note: Option<&str>,
) -> Result<(), String> {
    let from = current_status.trim();
    let to = next_status.trim();
    if from == to {
        return Ok(());
    }
    let reason = requested_note.map(str::trim).unwrap_or("");
    if reason.is_empty() {
        return Err(format!(
            "议题 #{number} 要改状态（{from} → {to}），请在 note 里写清这次为什么改：\
             推翻方向、审查打回、用户确认过、卡在什么上都算理由"
        ));
    }
    Ok(())
}

/// note 校验：任何时候都不能被清空。
///
/// 红豆的口径是「不能为空，任何时候」，所以这里不看状态：只要这次传了 note，
/// 它就得有实际内容。空值与纯空白一律拒，避免模型用「顺手带个空串」把上一条
/// 变更理由静默抹掉。不传 note（None）不在校验范围——那表示这次不改 note。
fn kanban_updated_note_checked(number: u32, note: &str) -> Result<String, String> {
    let trimmed = note.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "议题 #{number} 的 note 不能清空：它是这条议题最近一次变更的理由，任何时候都不能为空"
        ));
    }
    Ok(trimmed.to_string())
}

/// 写之前核对：`member:<角色名>` 必须是名册里的在队队员。
///
/// 单独留一道预检而不是只在解析时判，是为了保住报错顺序：
/// 格式非法的那几条留给写入层统一报，这里只挑「已经能认出是队员指派」的条目，
/// 否则同一个入参会先报名册错、再报格式错。
fn kanban_validate_member_assignees(
    roster: &KanbanBoardRoster,
    items: &[KanbanIssueUpsertItem],
) -> Result<(), String> {
    let requested = items
        .iter()
        .filter_map(|item| item.assignee.as_deref())
        .filter_map(|raw| kanban_assignee_from_wire(raw).ok())
        .filter(|(kind, _)| kind == KANBAN_ASSIGNEE_MEMBER)
        .filter_map(|(_, role)| role)
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return Ok(());
    }
    let known = roster.active_roles();
    let missing = requested
        .iter()
        .filter(|role| !known.iter().any(|value| value.trim() == role.trim()))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    // 报错要带上可选角色：模型只看到「不存在」，下一轮还会随便猜一个名字。
    let available = if known.is_empty() {
        "（当前没有在队队员）".to_string()
    } else {
        known.join(" / ")
    };
    Err(format!(
        "议题负责人不能指派给不在队的角色：{}；当前在队角色：{available}。不分配请用 none",
        missing.join("、")
    ))
}

/// 本批写入后每条议题的最终状态与最终负责人。
/// 校验必须看最终态：本次没传 status / assignee 时，最终值来自现状，
/// 否则校验看到的就不是这条议题真正会变成的样子。
#[derive(Debug, Clone)]
struct KanbanFinalAssignee {
    /// 改的那条带编号；新建的没有，编号要等落盘时才分配。
    number: Option<u32>,
    status: String,
    assignee: KanbanAssigneeRef,
}

impl KanbanFinalAssignee {
    fn member_number(&self) -> Option<u32> {
        if self.assignee.kind.trim() != KANBAN_ASSIGNEE_MEMBER {
            return None;
        }
        self.assignee.member_number
    }

    fn has_assignee(&self) -> bool {
        self.assignee.kind.trim() != KANBAN_ASSIGNEE_NONE
    }

    /// 报错时指代这条议题：「议题 #7」或「这条新议题」。
    fn display(&self) -> String {
        match self.number {
            Some(number) => format!("议题 #{number}"),
            None => "这条新议题".to_string(),
        }
    }

    /// 报错时怎么称呼负责人。编号翻不出名字就退回编号本身——报错文案宁可生硬，
    /// 也不能说「不能指派给「」」。
    fn assignee_label(&self, roster: &KanbanBoardRoster) -> String {
        match self.assignee.kind.trim() {
            KANBAN_ASSIGNEE_LEADER => "队长".to_string(),
            KANBAN_ASSIGNEE_MEMBER => self
                .member_number()
                .map(|number| roster.member_label(number))
                .unwrap_or_default(),
            _ => String::new(),
        }
    }
}

/// 整批算出每条议题的最终状态与最终负责人。
/// 逐条边算边判会把「同一批里先改状态再改负责人」判成错，
/// 所以校验一律基于这份结果，不与落盘混在一起。
fn kanban_final_assignees(
    issues: &[KanbanIssueEntry],
    roster: &KanbanBoardRoster,
    items: &[KanbanIssueUpsertItem],
) -> Result<Vec<KanbanFinalAssignee>, String> {
    let mut finals = Vec::with_capacity(items.len());
    for item in items {
        // 没带编号就是新建，新建一律从待议开始。
        let mut status = KANBAN_STATUS_DISCUSSING.to_string();
        let mut assignee = KanbanAssigneeRef {
            kind: KANBAN_ASSIGNEE_NONE.to_string(),
            member_number: None,
        };
        if let Some(number) = item.issue_number {
            let issue = kanban_find_issue(issues, number)
                .ok_or_else(|| format!("议题 #{number} 不存在"))?;
            status = issue.status.clone();
            assignee = issue.assignee.clone();
            // 新建不收 status，那条会在落盘时直接报错；这里也不把它算进最终态，
            // 免得先报出「这下不能挂人」这种隔了一层的错。
            if let Some(raw) = item.status.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                if let Some(next) = kanban_status_normalized(raw) {
                    status = next.to_string();
                }
            }
        }
        if let Some(raw) = item.assignee.as_deref() {
            assignee = roster.resolve_assignee(raw)?;
        }
        finals.push(KanbanFinalAssignee {
            number: item.issue_number,
            status,
            assignee,
        });
    }
    Ok(finals)
}

/// 一条绑定约束：待议 / 已完成 / 已关闭三态不挂人。
/// 必须整批算完最终态再判——逐条判会漏掉同批内的互相影响，也会误伤「先解绑再绑新的」。
fn kanban_validate_assignee_constraints(
    issues: &[KanbanIssueEntry],
    roster: &KanbanBoardRoster,
    items: &[KanbanIssueUpsertItem],
) -> Result<(), String> {
    let finals = kanban_final_assignees(issues, roster, items)?;

    // 一、不归任何人办的两态。
    for final_state in &finals {
        if kanban_status_allows_assignee(&final_state.status) || !final_state.has_assignee() {
            continue;
        }
        if final_state.number.is_none() {
            return Err(format!(
                "新建议题一律从「{discussing}」开始，这个状态不挂负责人（收到 assignee）。\
                 先建这条议题，定了方向（改成 todo / in_progress）之后，再带编号把它派出去。",
                discussing = kanban_status_label(KANBAN_STATUS_DISCUSSING),
            ));
        }
        return Err(format!(
            "{} 不能停在「{label}」还挂着负责人「{who}」：这个状态不归任何人办，\
             先把 assignee 写成 none，再改状态。",
            final_state.display(),
            label = kanban_status_label(&final_state.status),
            who = final_state.assignee_label(roster),
        ));
    }

    Ok(())
}

// ==================== 写入 ====================

/// 批量写入。编号不存在一律报错，绝不静默新建——
/// 队长以为在改旧题、实际悄悄新建了一条，比报错严重得多。
///
/// 在副本上做完全部校验与修改，最后一次性写回：任何一步失败，元数据一行都没动。
fn kanban_apply_upsert_in(
    issues: &mut Vec<KanbanIssueEntry>,
    next_number: &mut u32,
    roster: &KanbanBoardRoster,
    items: &[KanbanIssueUpsertItem],
) -> Result<KanbanUpsertResult, String> {
    if items.is_empty() {
        return Err("kanban upsert 至少要提供一条议题".to_string());
    }

    let mut draft = issues.clone();
    let mut draft_next_number = *next_number;

    // 编号预检：整批里只要有一个编号不存在，整批失败，不做部分成功。
    let missing = items
        .iter()
        .filter_map(|item| item.issue_number)
        .filter(|number| kanban_find_issue(draft.as_slice(), *number).is_none())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "议题编号不存在：{}；编号不会自动新建，请先确认看板上现有的编号",
            missing
                .iter()
                .map(|number| format!("#{number}"))
                .collect::<Vec<_>>()
                .join("、")
        ));
    }

    // 绑定约束在改动之前判：整批算完最终态再判，失败时板上一行都没动。
    kanban_validate_assignee_constraints(draft.as_slice(), roster, items)?;

    let mut created = Vec::new();
    let mut updated = Vec::new();

    for item in items {
        let now = now_utc_rfc3339();
        match item.issue_number {
            Some(number) => {
                let index = draft
                    .iter()
                    .position(|issue| issue.number == number)
                    .ok_or_else(|| format!("议题 #{number} 不存在"))?;

                if let Some(title) = item.title.as_deref() {
                    let title = title.trim();
                    if title.is_empty() {
                        return Err(format!("议题 #{number} 的标题不能为空"));
                    }
                    draft[index].title = title.to_string();
                }
                if let Some(status) = item.status.as_deref() {
                    let next_status = kanban_status_normalized(status).ok_or_else(|| {
                        format!(
                            "议题 #{number} 的状态非法：{}；可选 {}",
                            status.trim(),
                            KANBAN_STATUSES.join(" / ")
                        )
                    })?;
                    kanban_next_state_checked(
                        number,
                        &draft[index].status,
                        next_status,
                        item.note.as_deref(),
                    )?;
                    draft[index].status = next_status.to_string();
                }
                if let Some(assignee) = item.assignee.as_deref() {
                    draft[index].assignee = roster.resolve_assignee(assignee)?;
                }
                // note 任何时候都不能被清空：判据与状态无关，
                // 即使这次同时把状态写成同一格（kanban_next_state_checked 直接放行），
                // 也不能顺手把 note 抹成空串。
                if let Some(note) = item.note.as_deref() {
                    draft[index].note = kanban_updated_note_checked(number, note)?;
                }
                draft[index].updated_at = now.clone();
                draft[index].version += 1;
                let view = kanban_issue_view(&draft[index], roster);
                updated.push(view);
            }
            None => {
                let title = item
                    .title
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "新建议题必须提供 title".to_string())?;
                // 新建不收 status：新建议题一律从待议开始。这里选择拒绝而不是静默忽略——
                // 忽略会让模型以为建成了它想要的那一格，下一轮才发现对不上；直接报错当轮就能改对。
                if let Some(raw) = item
                    .status
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    return Err(format!(
                        "新建议题不接受 status 参数（收到「{raw}」）：新建议题一律从 {discussing}\
                         （待议）开始，建好之后再带 number 改状态",
                        discussing = KANBAN_STATUS_DISCUSSING,
                    ));
                }
                // 理由必填同样适用于新建：建出来就落在待议这一格，为什么记它得说清楚。
                let note = item
                    .note
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        "新建议题必须在 note 里写明理由：为什么要把这件事记进看板".to_string()
                    })?
                    .to_string();
                let assignee = match item.assignee.as_deref() {
                    Some(raw) => roster.resolve_assignee(raw)?,
                    None => KanbanAssigneeRef {
                        kind: KANBAN_ASSIGNEE_NONE.to_string(),
                        member_number: None,
                    },
                };
                let number = kanban_allocate_number(draft.as_slice(), &mut draft_next_number);
                let issue = KanbanIssueEntry {
                    number,
                    title: title.to_string(),
                    status: KANBAN_STATUS_DISCUSSING.to_string(),
                    assignee,
                    note,
                    plan_path: String::new(),
                    created_at: now.clone(),
                    updated_at: now,
                    version: 0,
                };
                let view = kanban_issue_view(&issue, roster);
                draft.push(issue);
                created.push(view);
            }
        }
    }

    *issues = draft;
    *next_number = draft_next_number;
    Ok(KanbanUpsertResult { created, updated })
}

fn kanban_apply_upsert(
    state: &AppState,
    leader_conversation_id: &str,
    items: &[KanbanIssueUpsertItem],
) -> Result<KanbanUpsertResult, String> {
    let roster = kanban_board_roster(state, leader_conversation_id)?;
    kanban_validate_member_assignees(&roster, items)?;
    let mut outcome: Option<KanbanUpsertResult> = None;
    state_update_conversation_metadata_cached(state, leader_conversation_id, |conversation| {
        let mut issues = conversation.issues.clone();
        let mut next_number = conversation.issue_next_number;
        let result = kanban_apply_upsert_in(&mut issues, &mut next_number, &roster, items)?;
        conversation.issues = issues;
        conversation.issue_next_number = next_number;
        outcome = Some(result);
        Ok(())
    })?;
    outcome.ok_or_else(|| "写入议题后未能取得结果".to_string())
}

/// 用户在看板上点「通过」：把待审查推进到已完成。
/// 只在当前状态是待审时才推进，和队长工具调用撞车时先到先得；
/// 后到的一侧拿到「状态不符」而不是覆盖。
fn kanban_apply_approve(
    state: &AppState,
    leader_conversation_id: &str,
    number: u32,
    note: &str,
) -> Result<KanbanIssueView, String> {
    let roster = kanban_board_roster(state, leader_conversation_id)?;
    let mut outcome: Option<KanbanIssueView> = None;
    state_update_conversation_metadata_cached(state, leader_conversation_id, |conversation| {
        outcome = Some(kanban_approve_in(
            &mut conversation.issues,
            &roster,
            number,
            note,
        )?);
        Ok(())
    })?;
    outcome.ok_or_else(|| format!("议题 #{number} 在通过后读不回来了"))
}

fn kanban_approve_in(
    issues: &mut [KanbanIssueEntry],
    roster: &KanbanBoardRoster,
    number: u32,
    note: &str,
) -> Result<KanbanIssueView, String> {
    let now = now_utc_rfc3339();
    let note = note.trim();
    let note = if note.is_empty() {
        "用户在队伍议题看板上确认通过"
    } else {
        note
    };
    let issue = issues
        .iter_mut()
        .find(|issue| issue.number == number)
        .ok_or_else(|| format!("议题 #{number} 不存在"))?;
    if issue.status.trim() != KANBAN_STATUS_IN_REVIEW {
        let current = issue.status.trim().to_string();
        return Err(format!(
            "议题 #{number} 当前状态是「{current}」，只有「待审查」才能点通过"
        ));
    }
    issue.status = KANBAN_STATUS_DONE.to_string();
    issue.note = note.to_string();
    issue.updated_at = now;
    issue.version += 1;
    // 已完成不归任何人办：用户点通过也要把人摘下来，否则这条议题会一直占着那名队员。
    issue.assignee = KanbanAssigneeRef {
        kind: KANBAN_ASSIGNEE_NONE.to_string(),
        member_number: None,
    };
    Ok(kanban_issue_view(issue, roster))
}

// ==================== 对外查询 ====================

/// 看板页与工具 list 用。
/// `assignee_role` 按角色名过滤，内部翻成名册编号再比——外部认名字，内部认编号。
fn kanban_board_list(
    state: &AppState,
    leader_conversation_id: &str,
    status: Option<&str>,
    assignee_kind: Option<&str>,
    assignee_role: Option<&str>,
) -> Result<Vec<KanbanIssueView>, String> {
    let (issues, _) = kanban_read_board(state, leader_conversation_id)?;
    let roster = kanban_board_roster(state, leader_conversation_id)?;
    Ok(kanban_board_list_in(
        &issues,
        &roster,
        status,
        assignee_kind,
        assignee_role,
    ))
}

fn kanban_board_list_in(
    issues: &[KanbanIssueEntry],
    roster: &KanbanBoardRoster,
    status: Option<&str>,
    assignee_kind: Option<&str>,
    assignee_role: Option<&str>,
) -> Vec<KanbanIssueView> {
    let requested_role = assignee_role
        .map(str::trim)
        .filter(|role| !role.is_empty());
    let wanted_number = match requested_role {
        Some(role) => match roster.member_number_of_role(role) {
            Some(number) => Some(number),
            // 给了角色却查不到编号，说明这个人不在在队名册里：过滤成空，
            // 不能退化成「这个维度不生效」把整块板漏出去。
            None => return Vec::new(),
        },
        None => None,
    };
    let mut views = Vec::new();
    for issue in issues {
        if let Some(wanted) = status {
            if issue.status.trim() != wanted.trim() {
                continue;
            }
        }
        if let Some(kind) = assignee_kind {
            if issue.assignee.kind.trim() != kind.trim() {
                continue;
            }
        }
        if let Some(number) = wanted_number {
            if issue.assignee.member_number != Some(number) {
                continue;
            }
        }
        views.push(kanban_issue_view(issue, roster));
    }
    views
}

/// 一块看板的读取缓存：整批概览里同一块板只读一次会话元数据。
///
/// 概览按名册编号直接比对，不构造角色名——那需要为每名队员各读一次会话，
/// 而首页是高频路径，不该为显示一个议题标题付出这个代价。
pub(crate) struct KanbanSession {
    boards: std::collections::HashMap<String, Vec<KanbanIssueEntry>>,
}

impl KanbanSession {
    pub(crate) fn new() -> Self {
        Self {
            boards: std::collections::HashMap::new(),
        }
    }

    fn board(&mut self, state: &AppState, leader_conversation_id: &str) -> &[KanbanIssueEntry] {
        let leader = leader_conversation_id.trim();
        if !self.boards.contains_key(leader) {
            let issues = match kanban_read_board(state, leader) {
                Ok((issues, _)) => issues,
                Err(err) => {
                    runtime_log_warn(format!(
                        "[看板] 跳过，任务=读取在办议题，leader_conversation_id={leader}，error={err}"
                    ));
                    Vec::new()
                }
            };
            self.boards.insert(leader.to_string(), issues);
        }
        self.boards
            .get(leader)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// 某名队员名下的在办议题。一个人可以同时挂多条在办，
    /// 这里取编号最小那条：选择要确定，不随读顺序变，也不擅自改板。
    pub(crate) fn active_issue_for_member(
        &mut self,
        state: &AppState,
        leader_conversation_id: &str,
        member_number: u32,
    ) -> Option<KanbanActiveIssueSummary> {
        let issues = self.board(state, leader_conversation_id);
        issues
            .iter()
            .filter(|issue| {
                issue.assignee.kind.trim() == KANBAN_ASSIGNEE_MEMBER
                    && issue.assignee.member_number == Some(member_number)
                    && kanban_status_occupies_member(&issue.status)
            })
            .min_by_key(|issue| issue.number)
            .map(|issue| KanbanActiveIssueSummary {
                number: issue.number,
                title: issue.title.clone(),
                status: issue.status.clone(),
            })
    }
}
