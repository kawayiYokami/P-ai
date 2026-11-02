// ==================== 队伍议题看板：领域模型 ====================
// 议题是队伍级工作项，独立于会话元数据：
// 不进 Conversation、不复用 current_todos，自带状态枚举与存储。
// 对外一律用 number 定位；id 只是存储主键，不进工具参数、不上界面。

const KANBAN_DB_FILE_NAME: &str = "kanban_store.db";

// ========== 状态枚举 ==========
// 状态之间没有先后约束：任何一格都能切到任何一格（包括从已完成改回解决中、
// 从已关闭重新打开）。顺序只用来决定看板里的展示次序，不代表流转规则。
const KANBAN_STATUS_DISCUSSING: &str = "discussing";
const KANBAN_STATUS_TODO: &str = "todo";
const KANBAN_STATUS_IN_PROGRESS: &str = "in_progress";
const KANBAN_STATUS_IN_REVIEW: &str = "in_review";
const KANBAN_STATUS_BLOCKED: &str = "blocked";
const KANBAN_STATUS_DONE: &str = "done";
const KANBAN_STATUS_CLOSED: &str = "closed";

/// 合法状态全集。顺序即看板里的展示次序（待议 → 待办 → 解决中 → 待审 → 已完成 → 已关闭），
/// `blocked` 是侧枝，不参加这条展示线。
const KANBAN_STATUSES: [&str; 7] = [
    KANBAN_STATUS_DISCUSSING,
    KANBAN_STATUS_TODO,
    KANBAN_STATUS_IN_PROGRESS,
    KANBAN_STATUS_IN_REVIEW,
    KANBAN_STATUS_BLOCKED,
    KANBAN_STATUS_DONE,
    KANBAN_STATUS_CLOSED,
];

/// 需要用户介入的四类，决定磁贴是否出现、磁贴四段的顺序，以及看板「决策」视图的成员。
/// 顺序（待审 → 待办 → 阻塞 → 待议）按产品口径定，不由紧迫度推导。
/// 看板页时间线走的是主链的流程顺序，两者不同是有意的，不要对齐。
const KANBAN_ATTENTION_STATUSES: [&str; 4] = [
    KANBAN_STATUS_IN_REVIEW,
    KANBAN_STATUS_TODO,
    KANBAN_STATUS_BLOCKED,
    KANBAN_STATUS_DISCUSSING,
];

// ========== 归属 ==========
const KANBAN_ASSIGNEE_LEADER: &str = "leader";
const KANBAN_ASSIGNEE_NONE: &str = "none";
/// 存储里的 kind 值。真正的队员角色（人格名）另存 assignee_member_role。
const KANBAN_ASSIGNEE_MEMBER: &str = "member";
/// 对外写法前缀：`member:李工`。队员按角色认人，与队伍工具同一套标识。
const KANBAN_ASSIGNEE_MEMBER_PREFIX: &str = "member:";

#[derive(Debug, Clone)]
struct KanbanIssue {
    /// 存储主键。不对工具暴露、不进 UI、不对外序列化。
    id: String,
    /// 分区键：属于哪块看板（哪个主会话）。
    leader_conversation_id: String,
    /// 看板内自增编号，唯一对外引用。
    number: u32,
    title: String,
    status: String,
    assignee_kind: String,
    /// 队员角色（人格名）。kind == member 时才有值。
    assignee_member_role: Option<String>,
    note: String,
    /// 挂在这条议题上的计划文件路径（磁盘绝对路径）。空串表示没挂。
    /// 只存引用不存正文：计划正文始终以磁盘上那份为准。
    plan_path: String,
    created_at: String,
    updated_at: String,
    /// 乐观锁版本，每次写入 +1。
    version: u64,
}

/// 对外视图：**不含 id**。工具返回值、事件载荷、前端都只认这个。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KanbanIssueView {
    number: u32,
    title: String,
    status: String,
    /// 扁平原字符串：`leader` / `member:李工` / `none`。比嵌套对象省 token。
    assignee: String,
    note: String,
    /// 挂在这条议题上的计划文件路径，空串表示没挂。
    plan_path: String,
    created_at: String,
    updated_at: String,
    version: u64,
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

impl KanbanIssue {
    fn to_view(&self) -> KanbanIssueView {
        KanbanIssueView {
            number: self.number,
            title: self.title.clone(),
            status: self.status.clone(),
            assignee: kanban_assignee_to_wire(
                &self.assignee_kind,
                self.assignee_member_role.as_deref(),
            ),
            note: self.note.clone(),
            plan_path: self.plan_path.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            version: self.version,
        }
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
        KANBAN_STATUS_CLOSED => "已关闭",
        _ => "未知",
    }
}

/// 这个状态算不算「在办」，也就是占不占着负责人。
/// 只有已完成与已关闭不占人：处理完的议题留着负责人名字，只表示「谁做的」，不妨碍他接新活。
/// 待议也算在办——方向没定不等于没派给人。
fn kanban_status_occupies_member(status: &str) -> bool {
    let trimmed = status.trim();
    trimmed != KANBAN_STATUS_DONE && trimmed != KANBAN_STATUS_CLOSED
}

/// 这个状态允不允许挂负责人。待议、已完成、已关闭三态不归任何人办。
/// 待审与阻塞刻意保留：待审是等用户点头、人还在名下，审出问题还找得到人；
/// 阻塞是卡住了，责任人更该留在上面。
fn kanban_status_allows_assignee(status: &str) -> bool {
    !matches!(
        status.trim(),
        KANBAN_STATUS_DISCUSSING | KANBAN_STATUS_DONE | KANBAN_STATUS_CLOSED
    )
}

// ==================== 归属编解码 ====================

fn kanban_assignee_to_wire(kind: &str, member_role: Option<&str>) -> String {
    match kind.trim() {
        KANBAN_ASSIGNEE_LEADER => KANBAN_ASSIGNEE_LEADER.to_string(),
        KANBAN_ASSIGNEE_MEMBER => member_role
            .map(str::trim)
            .filter(|role| !role.is_empty())
            .map(|role| format!("{KANBAN_ASSIGNEE_MEMBER_PREFIX}{role}"))
            .unwrap_or_else(|| KANBAN_ASSIGNEE_NONE.to_string()),
        _ => KANBAN_ASSIGNEE_NONE.to_string(),
    }
}

/// 解析 `leader` / `member:<角色名>` / `none` / 空。
/// 空值与 `unassigned` 一并落到 none，避免模型换个说法就报错。
/// 队员按角色认人：编号在队伍工具那侧已经不对模型暴露，
/// 这里再接受编号，只会让模型拿着一个它根本没渠道看到的数字来试。
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
