// ==================== 队伍议题看板：业务编排 ====================
// store.rs 只管 SQL，这里管「什么写入算合法」：
// 主会话身份、编号预检、状态值合法性、改状态与清空 note 都要求理由、事务边界。

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KanbanUpsertResult {
    created: Vec<KanbanIssueView>,
    updated: Vec<KanbanIssueView>,
}

fn kanban_conn(state: &AppState) -> Result<Connection, String> {
    let conn = kanban_store_open(&state.data_path)?;
    kanban_migrate_legacy_member_assignees(state, &conn)?;
    Ok(conn)
}

/// 一次性迁移：编号时代的 `member:<编号>` 归属换成角色。
/// 反查不到就落 none——那个编号当时也指不到人，落 none 是把无效引用显式化，不是丢数据。
/// 幂等：成功的行角色列有值、失败的行 kind 已是 none，第二遍一条都不会再命中。
fn kanban_migrate_legacy_member_assignees(
    state: &AppState,
    conn: &Connection,
) -> Result<(), String> {
    let pending = kanban_store_legacy_member_assignees(conn)?;
    if pending.is_empty() {
        return Ok(());
    }
    let mut roster_cache: std::collections::HashMap<String, Vec<TeamMemberEntry>> =
        std::collections::HashMap::new();
    let mut migrated = 0usize;
    let mut cleared = 0usize;
    for row in pending {
        let role = kanban_legacy_member_role(
            state,
            &mut roster_cache,
            &row.leader_conversation_id,
            row.member_number,
        );
        match role {
            Some(role) => {
                kanban_store_set_assignee_member_role(
                    conn,
                    &row.leader_conversation_id,
                    row.number,
                    &role,
                )?;
                migrated += 1;
            }
            None => {
                kanban_store_clear_legacy_member_assignee(
                    conn,
                    &row.leader_conversation_id,
                    row.number,
                )?;
                cleared += 1;
            }
        }
    }
    runtime_log_info(format!(
        "[看板] 完成，任务=归属从编号迁移到角色，migrated={migrated}，cleared={cleared}"
    ));
    Ok(())
}

/// 一块看板里「每个角色名下那条在办议题」。没有在办议题的角色不出现在结果里。
/// 队员磁贴靠它显示议题状态：一个角色最多一条在办，所以值是单个对象。
/// 读不到看板时返回空并把原因记进日志——概览项是首页高频路径，
/// 不能因为看板读不出来就让整份会话快照失败，磁贴退回运行时状态即可。
pub(crate) fn kanban_active_issues_by_role(
    state: &AppState,
    leader_conversation_id: &str,
) -> std::collections::HashMap<String, KanbanActiveIssueSummary> {
    let leader = leader_conversation_id.trim();
    if leader.is_empty() {
        return std::collections::HashMap::new();
    }
    let issues = match kanban_conn(state)
        .and_then(|conn| kanban_store_list(&conn, leader, None, None, None))
    {
        Ok(issues) => issues,
        Err(err) => {
            runtime_log_warn(format!(
                "[看板] 跳过，任务=读取队员在办议题，leader_conversation_id={leader}，error={err}"
            ));
            return std::collections::HashMap::new();
        }
    };
    let mut by_role: std::collections::HashMap<String, KanbanActiveIssueSummary> =
        std::collections::HashMap::new();
    // 一人一条是本次新加的写入约束，库里可能还留着同一个人挂多条的历史数据。
    // 这里保留编号最小的那条：读完不报错，也不擅自改库。
    for issue in issues {
        if issue.assignee_kind.trim() != KANBAN_ASSIGNEE_MEMBER {
            continue;
        }
        if !kanban_status_occupies_member(&issue.status) {
            continue;
        }
        let Some(role) = issue
            .assignee_member_role
            .as_deref()
            .map(str::trim)
            .filter(|role| !role.is_empty())
            .map(str::to_string)
        else {
            continue;
        };
        by_role
            .entry(role)
            .or_insert(KanbanActiveIssueSummary {
                number: issue.number,
                title: issue.title,
                status: issue.status,
            });
    }
    by_role
}

/// 按名册反查历史编号对应的角色。
/// 角色取当时记下的名字并剥掉旧的「N 号 · 」前缀：人格后来改过名也不影响迁移结果。
fn kanban_legacy_member_role(
    state: &AppState,
    roster_cache: &mut std::collections::HashMap<String, Vec<TeamMemberEntry>>,
    leader_conversation_id: &str,
    member_number: u32,
) -> Option<String> {
    let members = roster_cache
        .entry(leader_conversation_id.to_string())
        .or_insert_with(|| {
            conversation_service_v2()
                .get_conversation_meta(state, leader_conversation_id)
                .map(|meta| meta.team.members)
                .unwrap_or_default()
        });
    let member = members
        .iter()
        .find(|member| member.number == member_number)?;
    let role = strip_legacy_member_number_prefix(&member.name)
        .trim()
        .to_string();
    if role.is_empty() {
        None
    } else {
        Some(role)
    }
}

/// 测试专用：按编号时代的样子往库里塞一条归属，用来验证一次性迁移。
#[cfg(test)]
pub(crate) fn kanban_test_seed_legacy_assignee(
    state: &AppState,
    leader_conversation_id: &str,
    number: u32,
    member_number: u32,
    title: &str,
) -> Result<(), String> {
    let conn = kanban_store_open(&state.data_path)?;
    let now = now_utc_rfc3339();
    let issue = KanbanIssue {
        id: format!("legacy-{leader_conversation_id}-{number}"),
        leader_conversation_id: leader_conversation_id.to_string(),
        number,
        title: title.to_string(),
        status: KANBAN_STATUS_TODO.to_string(),
        assignee_kind: KANBAN_ASSIGNEE_MEMBER.to_string(),
        assignee_member_role: None,
        note: String::new(),
        plan_path: String::new(),
        created_at: now.clone(),
        updated_at: now,
        version: 0,
    };
    kanban_store_insert_legacy_member_assignee(&conn, &issue, member_number)
}

/// 测试专用：还剩多少条没迁移，用来断言迁移是幂等的。
#[cfg(test)]
pub(crate) fn kanban_test_pending_legacy_assignees(state: &AppState) -> Result<usize, String> {
    let conn = kanban_store_open(&state.data_path)?;
    Ok(kanban_store_legacy_member_assignees(&conn)?.len())
}

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

fn kanban_board_list(
    state: &AppState,
    leader_conversation_id: &str,
    status: Option<&str>,
    assignee_kind: Option<&str>,
    assignee_role: Option<&str>,
) -> Result<Vec<KanbanIssueView>, String> {
    let conn = kanban_conn(state)?;
    let issues = kanban_store_list(
        &conn,
        leader_conversation_id,
        status,
        assignee_kind,
        assignee_role,
    )?;
    Ok(issues.iter().map(KanbanIssue::to_view).collect())
}

/// 改状态的前置校验：状态值有没有变、变了就得说明理由。
///
/// 判据是**这次请求带没带 note**，不是库里最终存着什么。否则模型把旧值原样带上来
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

/// 写库前核对：`member:<角色名>` 必须是名册里的在队队员。
///
/// 落点选在写库前，不是 `kanban_assignee_from_wire`：
/// - 解析函数只依赖一个字符串，拿不到名册；
/// - 它同时服务 `list` 的过滤条件，给它加名册校验会把「筛不到人」改写成「查询报错」，
///   那是两个语义；
/// - 写库前是所有负责人写入的汇合点（改旧题与新建都在 `kanban_apply_upsert_in` 里），
///   将来多一条写入入口也自动被这道门挡住，不用回来补名字。
///
/// `leader` / `none` 不查名册直接放行：它们不引用任何人。
fn kanban_validate_member_assignees(
    state: &AppState,
    leader_conversation_id: &str,
    items: &[KanbanIssueUpsertItem],
) -> Result<(), String> {
    // 格式非法的那几条留给写库层统一报：这里只挑出「已经能认出是队员指派」的条目，
    // 否则同一个入参会先报名册错、再报格式错，报错顺序变了。
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
    let roles = team_active_member_roles(state, leader_conversation_id)?;
    let missing = requested
        .iter()
        .filter(|role| !roles.iter().any(|known| known.trim() == role.trim()))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    // 报错要带上可选角色：模型只看到「不存在」，下一轮还会随便猜一个名字。
    let available = if roles.is_empty() {
        "（当前没有在队队员）".to_string()
    } else {
        roles.join(" / ")
    };
    Err(format!(
        "议题负责人不能指派给不在队的角色：{}；当前在队角色：{available}。不分配请用 none",
        missing.join("、")
    ))
}

/// 本批写入后每条议题的最终状态与最终负责人。
/// 校验必须看最终态：本次没传 status / assignee 时，最终值来自库里现值，
/// 否则校验看到的就不是这条议题真正会变成的样子。
#[derive(Debug, Clone)]
struct KanbanFinalAssignee {
    /// 改的那条带编号；新建的没有，编号要等写库时才分配。
    number: Option<u32>,
    title: String,
    status: String,
    assignee_kind: String,
    assignee_member_role: Option<String>,
}

impl KanbanFinalAssignee {
    fn member_role(&self) -> Option<&str> {
        if self.assignee_kind.trim() != KANBAN_ASSIGNEE_MEMBER {
            return None;
        }
        self.assignee_member_role
            .as_deref()
            .map(str::trim)
            .filter(|role| !role.is_empty())
    }

    fn has_assignee(&self) -> bool {
        self.assignee_kind.trim() != KANBAN_ASSIGNEE_NONE
    }

    /// 报错时指代这条议题：「议题 #7」或「这条新议题」。
    fn display(&self) -> String {
        match self.number {
            Some(number) => format!("议题 #{number}"),
            None => "这条新议题".to_string(),
        }
    }

    fn assignee_label(&self) -> String {
        match self.assignee_kind.trim() {
            KANBAN_ASSIGNEE_LEADER => "队长".to_string(),
            _ => self.member_role().unwrap_or("").to_string(),
        }
    }
}

/// 别人名下、此刻还算在办的那条议题。只用于报错，不进任何返回值。
#[derive(Debug, Clone)]
struct KanbanOccupiedIssue {
    /// 同批里新建的条目库里还没有编号，落 None。
    number: Option<u32>,
    title: String,
    status: String,
}

/// 整批算出每条议题的最终状态与最终负责人。
/// 逐条边算边判会把「同一批里两条绑给同一个人」和「先解绑再绑新的」都判错，
/// 所以校验一律基于这份结果，不与写库混在一起。
fn kanban_final_assignees(
    conn: &Connection,
    leader_conversation_id: &str,
    items: &[KanbanIssueUpsertItem],
) -> Result<Vec<KanbanFinalAssignee>, String> {
    let mut finals = Vec::with_capacity(items.len());
    for item in items {
        // 没带编号就是新建，新建一律从待议开始。
        let mut status = KANBAN_STATUS_DISCUSSING.to_string();
        let mut assignee_kind = KANBAN_ASSIGNEE_NONE.to_string();
        let mut assignee_member_role: Option<String> = None;
        let mut title = item
            .title
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        if let Some(number) = item.issue_number {
            let issue = kanban_store_find(conn, leader_conversation_id, number)?
                .ok_or_else(|| format!("议题 #{number} 不存在"))?;
            status = issue.status;
            assignee_kind = issue.assignee_kind;
            assignee_member_role = issue.assignee_member_role;
            if title.is_empty() {
                title = issue.title;
            }
            // 新建不收 status，那条会在写库时直接报错；这里也不把它算进最终态，
            // 免得先报出「这下不能挂人」这种隔了一层的错。
            if let Some(raw) = item.status.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                if let Some(next) = kanban_status_normalized(raw) {
                    status = next.to_string();
                }
            }
        }
        if let Some(raw) = item.assignee.as_deref() {
            let (next_kind, next_role) = kanban_assignee_from_wire(raw)?;
            assignee_kind = next_kind;
            assignee_member_role = next_role;
        }
        finals.push(KanbanFinalAssignee {
            number: item.issue_number,
            title,
            status,
            assignee_kind,
            assignee_member_role,
        });
    }
    Ok(finals)
}

/// 同一块看板里，除 `self_index` 之外还有哪些在该角色名下的在办议题。
/// 本批也在改的条目以最终态为准，没在本批里的以库里现态为准——
/// 否则「同批里先把它改成已完成」会被当成仍然占着人。
fn kanban_other_active_issues_for_role(
    conn: &Connection,
    leader_conversation_id: &str,
    finals: &[KanbanFinalAssignee],
    self_index: usize,
    role: &str,
) -> Result<Vec<KanbanOccupiedIssue>, String> {
    let mut occupied = Vec::new();
    let stored = kanban_store_list(
        conn,
        leader_conversation_id,
        None,
        Some(KANBAN_ASSIGNEE_MEMBER),
        Some(role),
    )?;
    for issue in stored {
        let final_index = finals
            .iter()
            .position(|final_state| final_state.number == Some(issue.number));
        if final_index == Some(self_index) {
            continue;
        }
        let (title, status, issue_role) = match final_index {
            Some(index) => (
                finals[index].title.clone(),
                finals[index].status.clone(),
                finals[index]
                    .assignee_member_role
                    .clone()
                    .unwrap_or_default(),
            ),
            None => (
                issue.title.clone(),
                issue.status.clone(),
                issue.assignee_member_role.clone().unwrap_or_default(),
            ),
        };
        if issue_role.trim() != role || !kanban_status_occupies_member(&status) {
            continue;
        }
        occupied.push(KanbanOccupiedIssue {
            number: Some(issue.number),
            title,
            status,
        });
    }
    // 同一批里也落在该角色名下的条目：这些改动还没写进库，只能从最终态看。
    // 只按库里的现态判会漏掉「同批两条一起绑给同一个人」（写库前两条都不在库里挂着人）。
    for (index, final_state) in finals.iter().enumerate() {
        if index == self_index
            || final_state.member_role() != Some(role)
            || !kanban_status_occupies_member(&final_state.status)
        {
            continue;
        }
        // 库里那条已经算过一次，别重复列进报错。
        if let Some(number) = final_state.number {
            if occupied.iter().any(|issue| issue.number == Some(number)) {
                continue;
            }
        }
        occupied.push(KanbanOccupiedIssue {
            number: final_state.number,
            title: final_state.title.clone(),
            status: final_state.status.clone(),
        });
    }
    Ok(occupied)
}

/// 两条绑定约束：待议 / 已完成 / 已关闭不挂人；一个人同时只能有一条在办议题。
/// 必须整批算完最终态再判——逐条判会漏掉同批内的互相冲突，也会误伤「先解绑再绑新的」。
fn kanban_validate_assignee_constraints(
    conn: &Connection,
    leader_conversation_id: &str,
    items: &[KanbanIssueUpsertItem],
) -> Result<(), String> {
    let finals = kanban_final_assignees(conn, leader_conversation_id, items)?;

    // 一、不归任何人办的三态。
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
            who = final_state.assignee_label(),
        ));
    }

    // 二、一个人同时只能有一条在办议题。
    for (index, final_state) in finals.iter().enumerate() {
        if !kanban_status_occupies_member(&final_state.status) {
            continue;
        }
        let Some(role) = final_state.member_role() else {
            continue;
        };
        let occupied =
            kanban_other_active_issues_for_role(conn, leader_conversation_id, &finals, index, role)?;
        if occupied.is_empty() {
            continue;
        }
        let listing = occupied
            .iter()
            .map(|issue| match issue.number {
                Some(number) => format!(
                    "#{number}「{}」（{}）",
                    issue.title,
                    kanban_status_label(&issue.status)
                ),
                None => format!("同批新建的另一条（{}）", kanban_status_label(&issue.status)),
            })
            .collect::<Vec<_>>()
            .join("、");
        let first = occupied[0]
            .number
            .map(|number| format!("#{number}"))
            .unwrap_or_else(|| "那条新议题".to_string());
        return Err(format!(
            "{} 不能指派给「{role}」：他名下已经挂着议题 {listing}。\
             一个人同时只能有一条在办议题，先让 {first} 收口（改成已完成或已关闭），再派这条。",
            final_state.display(),
        ));
    }

    Ok(())
}

/// 批量写入。编号不存在一律报错，绝不静默新建——
/// 队长以为在改旧题、实际悄悄新建了一条，比报错严重得多。
fn kanban_apply_upsert(
    state: &AppState,
    leader_conversation_id: &str,
    items: &[KanbanIssueUpsertItem],
) -> Result<KanbanUpsertResult, String> {
    kanban_validate_member_assignees(state, leader_conversation_id, items)?;
    let conn = kanban_conn(state)?;
    kanban_apply_upsert_in(&conn, leader_conversation_id, items)
}

/// upsert 的实际实现，只依赖一个已打开的连接。
/// 与上面分开是为了让行为可测：新建拒绝任何 status、整批失败不落库这些结论
/// 原来埋在需要完整 AppState 的入口后面，一条测试都写不了，也就锁不住。
/// 连接用借用：测试要在调用之后继续查库，确认失败时一条都没写进去。
fn kanban_apply_upsert_in(
    conn: &Connection,
    leader_conversation_id: &str,
    items: &[KanbanIssueUpsertItem],
) -> Result<KanbanUpsertResult, String> {
    if items.is_empty() {
        return Err("kanban upsert 至少要提供一条议题".to_string());
    }

    // 编号预检：整批里只要有一个编号不存在，整批失败，不做部分成功。
    let requested_numbers: Vec<u32> = items
        .iter()
        .filter_map(|item| item.issue_number)
        .collect();
    if !requested_numbers.is_empty() {
        let missing = kanban_store_missing_numbers(conn, leader_conversation_id, &requested_numbers)?;
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
    }

    // 绑定约束在开局事务之前判：整批算完最终态再判，失败时库里一条都没动。
    kanban_validate_assignee_constraints(conn, leader_conversation_id, items)?;

    let tx = conn
        .unchecked_transaction()
        .map_err(|err| format!("开始看板事务失败：{err}"))?;

    let mut created = Vec::new();
    let mut updated = Vec::new();

    for item in items {
        let now = now_utc_rfc3339();
        match item.issue_number {
            Some(number) => {
                let mut issue = kanban_store_find(&tx, leader_conversation_id, number)?
                    .ok_or_else(|| format!("议题 #{number} 不存在"))?;

                if let Some(title) = item.title.as_deref() {
                    let title = title.trim();
                    if title.is_empty() {
                        return Err(format!("议题 #{number} 的标题不能为空"));
                    }
                    issue.title = title.to_string();
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
                        &issue.status,
                        next_status,
                        item.note.as_deref(),
                    )?;
                    issue.status = next_status.to_string();
                }
                if let Some(assignee) = item.assignee.as_deref() {
                    let (kind, role) = kanban_assignee_from_wire(assignee)?;
                    issue.assignee_kind = kind;
                    issue.assignee_member_role = role;
                }
                // note 任何时候都不能被清空：判据与状态无关，
                // 即使这次同时把状态写成同一格（kanban_next_state_checked 直接放行），
                // 也不能顺手把 note 抹成空串。
                if let Some(note) = item.note.as_deref() {
                    issue.note = kanban_updated_note_checked(number, note)?;
                }
                issue.updated_at = now.clone();

                if !kanban_store_update(&tx, &issue, None)? {
                    return Err(format!("议题 #{number} 已不存在，可能刚被删除"));
                }
                updated.push(issue.to_view());
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
                let (assignee_kind, assignee_role) = match item.assignee.as_deref() {
                    Some(raw) => kanban_assignee_from_wire(raw)?,
                    None => (KANBAN_ASSIGNEE_NONE.to_string(), None),
                };
                let number = kanban_store_allocate_number(&tx, leader_conversation_id)?;
                let issue = KanbanIssue {
                    id: Uuid::new_v4().to_string(),
                    leader_conversation_id: leader_conversation_id.to_string(),
                    number,
                    title: title.to_string(),
                    status: KANBAN_STATUS_DISCUSSING.to_string(),
                    assignee_kind,
                    assignee_member_role: assignee_role,
                    note,
                    plan_path: String::new(),
                    created_at: now.clone(),
                    updated_at: now,
                    version: 0,
                };
                kanban_store_insert(&tx, &issue)?;
                created.push(issue.to_view());
            }
        }
    }

    tx.commit()
        .map_err(|err| format!("提交看板事务失败：{err}"))?;
    Ok(KanbanUpsertResult { created, updated })
}

/// 用户在看板上点「通过」：把待审查推进到已完成。
/// 走条件更新，和队长工具调用撞车时先到先得；后到的一侧拿到 0 行而不是覆盖。
fn kanban_apply_approve(
    state: &AppState,
    leader_conversation_id: &str,
    number: u32,
    note: &str,
) -> Result<KanbanIssueView, String> {
    let conn = kanban_conn(state)?;
    let existed = kanban_store_find(&conn, leader_conversation_id, number)?;
    if existed.is_none() {
        return Err(format!("议题 #{number} 不存在"));
    }
    let now = now_utc_rfc3339();
    let note = note.trim();
    let note = if note.is_empty() {
        "用户在队伍议题看板上确认通过"
    } else {
        note
    };
    let applied = kanban_store_transition(
        &conn,
        leader_conversation_id,
        number,
        KANBAN_STATUS_IN_REVIEW,
        KANBAN_STATUS_DONE,
        note,
        // 已完成不归任何人办：用户点通过也要把人摘下来，否则这条议题会一直占着那名队员。
        true,
        &now,
    )?;
    if !applied {
        let current = kanban_store_find(&conn, leader_conversation_id, number)?;
        let current_status = current
            .map(|issue| issue.status)
            .unwrap_or_else(|| "未知".to_string());
        return Err(format!(
            "议题 #{number} 当前状态是「{current_status}」，只有「待审查」才能点通过"
        ));
    }
    let issue = kanban_store_find(&conn, leader_conversation_id, number)?
        .ok_or_else(|| format!("议题 #{number} 在通过后读不回来了"))?;
    Ok(issue.to_view())
}
