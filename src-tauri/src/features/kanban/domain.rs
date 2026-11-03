// ==================== 队伍议题看板：领域模型 ====================
// 议题挂在队长会话的元数据里（`Conversation.issues`），与会话同生共死：
// 队长会话删掉，它名下的议题跟着消失，不会留下没人认领也删不掉的孤儿。
// 对外一律用 number 定位；归属存名册编号，不存角色名。

// ========== 状态枚举 ==========
// 状态之间没有先后约束：任何一格都能切到任何一格（包括从已完成改回解决中）。
// 顺序只用来决定看板里的展示次序，不代表流转规则。
// 没有「放弃」态：不做了就是阻塞，不必为它单开一格。
const KANBAN_STATUS_DISCUSSING: &str = "discussing";
const KANBAN_STATUS_TODO: &str = "todo";
const KANBAN_STATUS_IN_PROGRESS: &str = "in_progress";
const KANBAN_STATUS_IN_REVIEW: &str = "in_review";
const KANBAN_STATUS_BLOCKED: &str = "blocked";
const KANBAN_STATUS_DONE: &str = "done";

/// 合法状态全集。顺序即看板里的展示次序（待议 → 待办 → 解决中 → 待审 → 已完成），
/// `blocked` 是侧枝，不参加这条展示线。
const KANBAN_STATUSES: [&str; 6] = [
    KANBAN_STATUS_DISCUSSING,
    KANBAN_STATUS_TODO,
    KANBAN_STATUS_IN_PROGRESS,
    KANBAN_STATUS_IN_REVIEW,
    KANBAN_STATUS_BLOCKED,
    KANBAN_STATUS_DONE,
];

/// 需要用户介入的四类，决定磁贴是否出现、磁贴四段的顺序，以及看板「决策」视图的成员。
/// 顺序（待审 → 待办 → 阻塞 → 待议）按产品口径定，不由紧迫度推导。
/// 看板页时间线走的是主链的流程顺序，两者不同是有意的，不要对齐。
#[allow(dead_code)]
const KANBAN_ATTENTION_STATUSES: [&str; 4] = [
    KANBAN_STATUS_IN_REVIEW,
    KANBAN_STATUS_TODO,
    KANBAN_STATUS_BLOCKED,
    KANBAN_STATUS_DISCUSSING,
];

// ========== 归属 ==========
const KANBAN_ASSIGNEE_LEADER: &str = "leader";
const KANBAN_ASSIGNEE_NONE: &str = "none";
/// 存储里的 kind 值。指向谁另存 `member_number`（名册编号）。
const KANBAN_ASSIGNEE_MEMBER: &str = "member";
/// 对外写法前缀：`member:李工`。队员按角色认人，与队伍工具同一套标识。
const KANBAN_ASSIGNEE_MEMBER_PREFIX: &str = "member:";

/// 对外视图：**不含存储细节**。工具返回值、事件载荷、前端都只认这个。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanIssueView {
    pub(crate) number: u32,
    pub(crate) title: String,
    pub(crate) status: String,
    /// 扁平原字符串：`leader` / `member:李工` / `none`。比嵌套对象省 token。
    pub(crate) assignee: String,
    pub(crate) note: String,
    /// 挂在这条议题上的计划文件路径，空串表示没挂。
    pub(crate) plan_path: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) version: u64,
}

/// 队员此刻挂着的在办议题，随队员会话概览项下发给前端磁贴。
/// 一个队员最多一条在办，所以是单个对象；没有议题时整个字段不出现。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanActiveIssueSummary {
    pub(crate) number: u32,
    pub(crate) title: String,
    pub(crate) status: String,
}

/// 一块看板的成员视角：名册编号 ⇄ 角色名。
///
/// 角色名不是存下来的，是运行时按「该队员会话当前绑定的人格」推出来的
/// （人格改了名就是新名字，人格被删才退回名册里那份旧名字）。
/// 所以议题只存编号，收发边界上才翻译成名字；翻译必须带着名册走，
/// 不能指望议题自己能说出「5 号现在叫什么」。
#[derive(Debug, Clone, Default)]
pub(crate) struct KanbanBoardRoster {
    /// 名册编号 -> 角色名。含已请离的队员：历史议题还挂着当年的负责人，
    /// 显示上要能说出那是谁，不能因为人走了就变成 `none`。
    roles_by_number: std::collections::HashMap<u32, String>,
    /// 在队队员的角色名 -> 编号。写入时按角色名找编号，请离的人不能再被指派。
    numbers_by_active_role: std::collections::HashMap<String, u32>,
    /// 在队角色名，按名册顺序。报错时告诉模型「可选角色有哪些」。
    active_roles: Vec<String>,
}

impl KanbanBoardRoster {
    pub(crate) fn new(active: Vec<(u32, String)>, all: Vec<(u32, String)>) -> Self {
        let mut roster = Self::default();
        for (number, role) in all {
            roster.roles_by_number.insert(number, role);
        }
        for (number, role) in active {
            let role = role.trim().to_string();
            if role.is_empty() {
                continue;
            }
            roster.active_roles.push(role.clone());
            roster.numbers_by_active_role.insert(role, number);
        }
        roster
    }

    /// 在队角色名，报错文案用。
    pub(crate) fn active_roles(&self) -> &[String] {
        self.active_roles.as_slice()
    }

    /// 名册编号对应的角色名。含已请离的队员，所以历史议题也翻得出来。
    /// 翻不出来时退回编号本身：报错文案宁可生硬，也不能说「不能指派给「」」。
    pub(crate) fn member_label(&self, number: u32) -> String {
        self.roles_by_number
            .get(&number)
            .map(|role| role.trim().to_string())
            .filter(|role| !role.is_empty())
            .unwrap_or_else(|| format!("{number} 号队员"))
    }

    /// 在队角色名对应的名册编号。过滤与指派都从这里过一道。
    pub(crate) fn member_number_of_role(&self, role: &str) -> Option<u32> {
        self.numbers_by_active_role
            .get(role.trim())
            .copied()
    }

    /// 把编号翻成对外写法。编号指不到人时落 `none`：那条议题的负责人已经无法解析，
    /// 与其把 `member:5` 这种内部编号暴露给模型，不如显式说「没有负责人」。
    fn wire_assignee(&self, assignee: &KanbanAssigneeRef) -> String {
        match assignee.kind.trim() {
            KANBAN_ASSIGNEE_LEADER => KANBAN_ASSIGNEE_LEADER.to_string(),
            KANBAN_ASSIGNEE_MEMBER => assignee
                .member_number
                .and_then(|number| self.roles_by_number.get(&number))
                .map(|role| role.trim())
                .filter(|role| !role.is_empty())
                .map(|role| format!("{KANBAN_ASSIGNEE_MEMBER_PREFIX}{role}"))
                .unwrap_or_else(|| KANBAN_ASSIGNEE_NONE.to_string()),
            _ => KANBAN_ASSIGNEE_NONE.to_string(),
        }
    }

    /// 把对外写法翻成编号。
    /// `leader` / `none` 不查名册；`member:<角色名>` 必须是在队队员，否则报错——
    /// 指派给一个不存在的人，比直接拒绝更难查。
    fn resolve_assignee(&self, raw: &str) -> Result<KanbanAssigneeRef, String> {
        let (kind, role) = kanban_assignee_from_wire(raw)?;
        if kind != KANBAN_ASSIGNEE_MEMBER {
            return Ok(KanbanAssigneeRef {
                kind,
                member_number: None,
            });
        }
        let role = role.unwrap_or_default();
        match self.numbers_by_active_role.get(role.trim()) {
            Some(number) => Ok(KanbanAssigneeRef {
                kind,
                member_number: Some(*number),
            }),
            None => {
                let available = if self.active_roles.is_empty() {
                    "（当前没有在队队员）".to_string()
                } else {
                    self.active_roles.join(" / ")
                };
                Err(format!(
                    "议题负责人不能指派给不在队的角色：{}；当前在队角色：{available}。不分配请用 none",
                    role.trim()
                ))
            }
        }
    }
}

/// 存储形态 -> 对外视图。
pub(crate) fn kanban_issue_view(
    issue: &KanbanIssueEntry,
    roster: &KanbanBoardRoster,
) -> KanbanIssueView {
    KanbanIssueView {
        number: issue.number,
        title: issue.title.clone(),
        status: issue.status.clone(),
        assignee: roster.wire_assignee(&issue.assignee),
        note: issue.note.clone(),
        plan_path: issue.plan_path.clone(),
        created_at: issue.created_at.clone(),
        updated_at: issue.updated_at.clone(),
        version: issue.version,
    }
}

// ==================== 状态判定 ====================

fn kanban_status_normalized(raw: &str) -> Option<&'static str> {
    let trimmed = raw.trim().to_ascii_lowercase();
    KANBAN_STATUSES.into_iter().find(|key| *key == trimmed)
}

/// 状态 key 的中文标签。给工具报错与界面文案共用，别在两处各写一份。
fn kanban_status_label(status: &str) -> &'static str {
    match status.trim() {
        KANBAN_STATUS_DISCUSSING => "待议",
        KANBAN_STATUS_TODO => "待办",
        KANBAN_STATUS_IN_PROGRESS => "解决中",
        KANBAN_STATUS_IN_REVIEW => "待审",
        KANBAN_STATUS_BLOCKED => "阻塞",
        KANBAN_STATUS_DONE => "已完成",
        _ => "未知",
    }
}

/// 这个状态算不算「在办」，也就是占不占着负责人。
/// 只有已完成不占人：已完成的那一刻负责人就被摘掉，人自然腾出手可以接新活。
/// 待议也算在办——方向没定不等于没派给人。
fn kanban_status_occupies_member(status: &str) -> bool {
    let trimmed = status.trim();
    trimmed != KANBAN_STATUS_DONE
}

/// 这个状态允不允许挂负责人。待议、已完成两态不归任何人办。
/// 待审与阻塞刻意保留：待审是等用户点头、人还在名下，审出问题还找得到人；
/// 阻塞是卡住了，责任人更该留在上面。
fn kanban_status_allows_assignee(status: &str) -> bool {
    !matches!(
        status.trim(),
        KANBAN_STATUS_DISCUSSING | KANBAN_STATUS_DONE
    )
}

// ==================== 归属编解码 ====================

/// 解析 `leader` / `member:<角色名>` / `none` / 空。
/// 空值与 `unassigned` 一并落到 none，避免模型换个说法就报错。
/// 队员按角色认人：编号是存储内部的事，不进工具参数——
/// 模型手里没有编号，让它填编号只会逼它瞎猜。
fn kanban_assignee_from_wire(raw: &str) -> Result<(String, Option<String>), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.eq_ignore_ascii_case(KANBAN_ASSIGNEE_NONE)
        || trimmed.eq_ignore_ascii_case("unassigned")
    {
        return Ok((KANBAN_ASSIGNEE_NONE.to_string(), None));
    }
    if trimmed.eq_ignore_ascii_case(KANBAN_ASSIGNEE_LEADER) {
        return Ok((KANBAN_ASSIGNEE_LEADER.to_string(), None));
    }
    if let Some(rest) = trimmed.strip_prefix(KANBAN_ASSIGNEE_MEMBER_PREFIX) {
        let role = rest.trim();
        if role.is_empty() {
            return Err(
                "归属写法不合法：member: 后面要写队员角色，例如 member:李工".to_string(),
            );
        }
        if role.chars().all(|value| value.is_ascii_digit()) {
            return Err(format!(
                "归属写法不合法：{trimmed}；队员按角色认人，请写 member:<角色名>（team_info 里的 role），不要写编号"
            ));
        }
        return Ok((KANBAN_ASSIGNEE_MEMBER.to_string(), Some(role.to_string())));
    }
    Err(format!(
        "归属写法不合法：{trimmed}；只接受 leader / member:<角色名> / none"
    ))
}

// ==================== 入参 ====================

/// 单条 upsert 入参。带 number 是改，不带是新建。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanIssueUpsertItem {
    #[serde(default, alias = "number", alias = "issue_number")]
    pub(crate) issue_number: Option<u32>,
    #[serde(default)]
    pub(crate) title: Option<String>,
    #[serde(default)]
    pub(crate) status: Option<String>,
    #[serde(default)]
    pub(crate) assignee: Option<String>,
    #[serde(default)]
    pub(crate) note: Option<String>,
}
