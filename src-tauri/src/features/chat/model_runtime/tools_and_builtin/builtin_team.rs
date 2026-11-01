// ==================== 会话队伍（team） ====================
//
// 队伍 = 一个普通会话（队长）+ 它的若干追问会话（队员）。
// - 队长工具：team_info / team_recruit（均只看队长身份） / team_send（看名册）
// - 队员工具：team_report
// 名册落队长会话的 team 字段，成员身份落队员会话自己的 team 字段；两处同写、以队长为准。
// 通讯一律以系统身份投递到目标会话：引导或排队由发信方选择。

const TEAM_LEADER_TOOL_NAMES: [&str; 7] = [
    "team_info",
    "team_recruit",
    "team_send",
    "team_read",
    "team_share_model",
    "kanban",
    "kanban_set_plan",
];
const TEAM_MEMBER_TOOL_NAME: &str = "team_report";
const TEAM_MESSAGE_KIND: &str = "team_message";
const TEAM_NOTICE_KIND: &str = "team_notice";
/// 邮件标题的字符上限：超长不报错，截到上限。
/// 标题现在同时是正文第一行（`# 标题`）与 `team_read` 单列返回的标题字段，两处都不该太长。
const TEAM_MAIL_TITLE_MAX_CHARS: usize = 40;
/// `team_read` 的条数：默认只读最近一条，上限 10。
/// 上限是防后门用的——没有它，这条轻量路径就能被大 limit 用成整读入口。
const TEAM_READ_LIMIT_DEFAULT: usize = 1;
const TEAM_READ_LIMIT_MAX: usize = 10;
/// 单条消息摘要的字符上限：返回值是摘要，不是正文搬运。
const TEAM_READ_SUMMARY_MAX_CHARS: usize = 200;
/// 补读的扫描上限。队员会话尾部常是一长串工具调用，没有上限时补读会一路翻到会话起点，
/// 那就等于把这条轻量路径变成整读。
const TEAM_READ_SCAN_MAX_MESSAGES: usize = 60;

// 队伍三份指引，各写各的：队长通用段每轮注入所有队长会话，远程私聊队长追加一节，
// 队员的随身份摘要交给队员。前两份不进系统提示外的其它位置，队员那份只作为会话消息出现。
const TEAM_LEADER_RULE_MD: &str =
    include_str!("../../../../../resources/prompts/team-leader-rule.md");
const TEAM_LEADER_RULE_REMOTE_MD: &str =
    include_str!("../../../../../resources/prompts/team-leader-rule-remote.md");
const TEAM_MEMBER_GUIDE_MD: &str =
    include_str!("../../../../../resources/prompts/team-member-guide.md");

fn team_session_conversation_id(session_id: &str) -> Result<String, String> {
    delegate_session_conversation_id(session_id)
        .ok_or_else(|| "队伍工具缺少 conversation_id".to_string())
}

/// 会话绑定的远程联系人类型：按 bound_conversation_id 反查联系人，与轮次出站源
/// （remote_im_auto_send_source_for_contact_conversation）走同一趟数据、同一个判据键。
fn remote_contact_type_for_bound_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Option<String> {
    state_service_list_remote_im_contacts(state, None)
        .ok()?
        .iter()
        .find(|contact| {
            contact
                .bound_conversation_id
                .as_deref()
                .map(str::trim)
                == Some(conversation_id.trim())
        })
        .map(|contact| contact.remote_contact_type.trim().to_string())
}

/// 队长指导注入块：能拥有名册且非队员身份的会话每轮注入，所有队长共用一份通用段。
pub(crate) fn team_leader_prompt_rule_block(
    state: &AppState,
    conversation: &Conversation,
) -> Option<String> {
    let meta = conversation_service_v2()
        .get_conversation_meta(state, conversation.id.trim())
        .ok()?;
    if meta.team.membership.is_some() {
        return None;
    }
    if !conversation_meta_can_own_team_roster(state, &meta) {
        return None;
    }
    // 远程私聊会话每句话都会原文送到用户手机，追加成本段；本地会话不含该段。
    let body = if conversation_is_remote_im_contact(conversation) {
        format!(
            "{}\n\n{}",
            TEAM_LEADER_RULE_MD.trim(),
            TEAM_LEADER_RULE_REMOTE_MD.trim()
        )
    } else {
        TEAM_LEADER_RULE_MD.trim().to_string()
    };
    Some(prompt_xml_block("team leader rule", body))
}

/// 能不能拥有队伍名册：前台普通会话、非草稿、未归档；远程联系人仅私聊。
/// 这是队伍策略（权限边界），不是「本地会话」运行时事实；放开或收紧队长范围只改这里。
/// 队员身份（membership）判定刻意留在调用方：执行期要给出「这是队员会话」的具体报错，
/// 装配期要 Leader/Member/None 角色，塞进本判定就得返回三态枚举。
pub(crate) fn conversation_meta_can_own_team_roster(
    state: &AppState,
    meta: &ConversationMetaView,
) -> bool {
    // 归档的事实标记是 archived_at（归档动作只写它，不改 status）；草稿走 is_draft。
    if meta.is_draft || meta.archived_at.is_some() {
        return false;
    }
    match meta.conversation_kind.trim() {
        CONVERSATION_KIND_CHAT => true,
        // 私聊与群聊的绑定会话都是 remote_im_contact，靠 remote_contact_type 区分：仅私聊可当队长。
        CONVERSATION_KIND_REMOTE_IM_CONTACT => {
            remote_contact_type_for_bound_conversation(state, &meta.id)
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("private"))
                .unwrap_or(false)
        }
        // side_chat（追问，含已请离的前队员）、delegate、system_notification 与未知 kind
        // 一律拒绝：这是名册权限，默认不放行。
        _ => false,
    }
}

/// 队长会话底座：能拥有名册、且自己不是别人的队员，不看名册内容。
/// team_info 走这里常驻；招募与名册操作在此基础上再加各自门控。
fn team_leader_session_meta(state: &AppState, session_id: &str) -> Result<ConversationMetaView, String> {
    let conversation_id = team_session_conversation_id(session_id)?;
    let meta = conversation_service_v2().get_conversation_meta(state, &conversation_id)?;
    if meta.team.membership.is_some() {
        return Err("这是队员会话，不能使用队长的队伍工具；要汇报请用 team_report".to_string());
    }
    if !conversation_meta_can_own_team_roster(state, &meta) {
        return Err("只有未归档的普通会话或远程私聊可以作为队长使用队伍工具".to_string());
    }
    Ok(meta)
}

/// 名册操作入口（send）：只看有没有在队队员。
fn team_leader_meta_for_roster(state: &AppState, session_id: &str) -> Result<ConversationMetaView, String> {
    let meta = team_leader_session_meta(state, session_id)?;
    if !team_has_active_member(&meta.team) {
        return Err("队伍里还没有在队队员；用 team_info 查看名册".to_string());
    }
    Ok(meta)
}

/// 当前会话作为队员的身份。
fn team_member_meta(
    state: &AppState,
    session_id: &str,
) -> Result<(ConversationMetaView, TeamMembership), String> {
    let conversation_id = team_session_conversation_id(session_id)?;
    let meta = conversation_service_v2().get_conversation_meta(state, &conversation_id)?;
    let membership = meta
        .team
        .membership
        .clone()
        .ok_or_else(|| "当前会话不是队员，没有可以向队长汇报的身份".to_string())?;
    // 归档的事实标记是 archived_at（归档动作只写它，不改 status），与队长门禁同判据。
    if meta.archived_at.is_some() {
        return Err("会话已归档，不能再向队长汇报".to_string());
    }
    Ok((meta, membership))
}

/// 按角色解析人格：填人格名优先，也接受人格 id。同名人格不止一条时要求用人格 id 指定。
fn team_resolve_persona<'a>(
    snapshot: &'a RuntimeOrganizationSnapshot,
    role: &str,
) -> Result<&'a AgentProfile, String> {
    let role = role.trim();
    if role.is_empty() {
        return Err("role 不能为空：填队员的人格名".to_string());
    }
    if let Some(agent) = snapshot
        .agents
        .iter()
        .find(|agent| agent.id.trim() == role)
    {
        return Ok(agent);
    }
    let named = snapshot
        .agents
        .iter()
        .filter(|agent| agent.name.trim() == role)
        .collect::<Vec<_>>();
    match named.len() {
        0 => Err(format!("没有「{role}」这个角色：人格里找不到同名或同 id 的项")),
        1 => Ok(named[0]),
        count => Err(format!(
            "有 {count} 个人格都叫「{role}」；请改用其中一个人格 id 指定"
        )),
    }
}

/// 一个角色只能有一名在队队员，队长本人也算占了这个角色。
fn team_ensure_role_available(
    state: &AppState,
    leader: &ConversationMetaView,
    persona_id: &str,
    persona_name: &str,
) -> Result<(), String> {
    if !persona_id.trim().is_empty() && leader.agent_id.trim() == persona_id.trim() {
        return Err(format!(
            "你本身就是「{persona_name}」，这个角色已经被你占用；队伍里每个人都要是不同的人格"
        ));
    }
    let persona_names = TeamPersonaNames::load(state);
    for member in leader
        .team
        .members
        .iter()
        .filter(|member| member.status.trim() == TEAM_MEMBER_STATUS_ACTIVE)
    {
        let member_agent_id = team_member_agent_id(state, member);
        let member_role = persona_names
            .name_of(&member_agent_id)
            .map(str::to_string)
            .unwrap_or_else(|| strip_legacy_member_number_prefix(&member.name));
        let same_persona =
            !member_agent_id.is_empty() && member_agent_id == persona_id.trim();
        if same_persona || member_role == persona_name.trim() {
            return Err(format!(
                "队伍里已经有「{member_role}」这名队员了；队伍里每个人都要是不同的人格"
            ));
        }
    }
    Ok(())
}


/// 名册里有没有在队队员：send 与挂载共用，只看这个，不看招募开关。
fn team_has_active_member(team: &TeamState) -> bool {
    team.members
        .iter()
        .any(|member| member.status.trim() == TEAM_MEMBER_STATUS_ACTIVE)
}

// ==================== 角色：队员的对外身份 ====================
//
// 编号只留在库里当主键，不对外。对外一律用「角色」——角色就是队员自己的人格，
// 队伍里一个角色只能有一名队员，所以角色本身就是唯一标识，不需要再加序号。

/// 人格名表：agent_id → 人格显示名。一次工具调用只读一次人格快照，队员逐个查表。
struct TeamPersonaNames(Vec<(String, String)>);

impl TeamPersonaNames {
    fn load(state: &AppState) -> Self {
        match load_runtime_organization_snapshot(state) {
            Ok(snapshot) => Self(
                snapshot
                    .agents
                    .iter()
                    .filter(|agent| !agent.name.trim().is_empty())
                    .map(|agent| {
                        (
                            agent.id.trim().to_string(),
                            agent.name.trim().to_string(),
                        )
                    })
                    .collect(),
            ),
            Err(err) => {
                runtime_log_warn(format!(
                    "[队伍] 跳过，任务=读取人格名，原因=快照读取失败，error={err}"
                ));
                Self(Vec::new())
            }
        }
    }

    fn name_of(&self, agent_id: &str) -> Option<&str> {
        let agent_id = agent_id.trim();
        if agent_id.is_empty() {
            return None;
        }
        self.0
            .iter()
            .find(|(id, _)| id == agent_id)
            .map(|(_, name)| name.as_str())
    }
}

/// 历史数据把编号写进了名字（「3 号 · 莫娜」）。编号不再对外，
/// 读出来时去掉这个前缀即可，库里的原值不动——不做数据迁移。
fn strip_legacy_member_number_prefix(name: &str) -> String {
    let text = name.trim();
    let Some((head, tail)) = text.split_once('·') else {
        return text.to_string();
    };
    let Some(number) = head.trim().strip_suffix('号') else {
        return text.to_string();
    };
    let number = number.trim();
    let tail = tail.trim();
    if number.is_empty() || tail.is_empty() || !number.chars().all(|value| value.is_ascii_digit()) {
        return text.to_string();
    }
    tail.to_string()
}

/// 队员会话当前挂的人格：名册条目不存 agent_id，人格是会话属性。
fn team_member_agent_id(state: &AppState, member: &TeamMemberEntry) -> String {
    let conversation_id = member.conversation_id.trim();
    if conversation_id.is_empty() {
        return String::new();
    }
    conversation_service_v2()
        .get_conversation_meta(state, conversation_id)
        .map(|meta| meta.agent_id.trim().to_string())
        .unwrap_or_default()
}

/// 角色名：人格名优先；人格已被删除时退回名册里的名字（去编号前缀）。
fn team_role_name(persona_names: &TeamPersonaNames, agent_id: &str, stored_name: &str) -> String {
    persona_names
        .name_of(agent_id)
        .map(str::to_string)
        .unwrap_or_else(|| strip_legacy_member_number_prefix(stored_name))
}

/// 队员角色 = 会话当前人格的名字；人格已被删除时退回名册里的名字（去编号前缀）。
fn team_member_role(
    state: &AppState,
    persona_names: &TeamPersonaNames,
    member: &TeamMemberEntry,
) -> String {
    let agent_id = team_member_agent_id(state, member);
    team_role_name(persona_names, &agent_id, &member.name)
}

/// 按角色定位在队队员：role 填人格名，也兼容人格 id。
/// 历史名册里可能有两名同角色队员（招募规则收紧之前建的），这时要求用会话 id 指定。
fn team_find_active_member_by_role(
    state: &AppState,
    team: &TeamState,
    persona_names: &TeamPersonaNames,
    role: &str,
    conversation_id: Option<&str>,
) -> Result<TeamMemberEntry, String> {
    let role = role.trim();
    let conversation_id = conversation_id.map(str::trim).unwrap_or_default();
    let active = team
        .members
        .iter()
        .filter(|member| member.status.trim() == TEAM_MEMBER_STATUS_ACTIVE)
        .collect::<Vec<_>>();
    if !conversation_id.is_empty() {
        return active
            .into_iter()
            .find(|member| member.conversation_id.trim() == conversation_id)
            .cloned()
            .ok_or_else(|| {
                format!("会话 {conversation_id} 不是这支队伍的在队队员；用 team_info 查看名册")
            });
    }
    if role.is_empty() {
        return Err("role 不能为空：填队员的人格名".to_string());
    }
    let mut matched = Vec::new();
    for member in active {
        let agent_id = team_member_agent_id(state, member);
        let member_role = persona_names
            .name_of(&agent_id)
            .map(str::to_string)
            .unwrap_or_else(|| strip_legacy_member_number_prefix(&member.name));
        if member_role == role || agent_id == role {
            matched.push((member.clone(), member_role));
        }
    }
    match matched.len() {
        0 => Err(format!(
            "队伍里没有「{role}」这名在队队员；用 team_info 查看名册"
        )),
        1 => Ok(matched.remove(0).0),
        count => {
            let ids = matched
                .iter()
                .map(|(member, _)| member.conversation_id.trim().to_string())
                .collect::<Vec<_>>()
                .join("、");
            Err(format!(
                "队伍里有 {count} 名在队队员都是「{role}」：{ids}；请用 conversation_id 指定其中一个"
            ))
        }
    }
}


/// 队长挂载位的名册快照：装配处按这个细粒度过滤。
struct TeamLeaderMount {
    has_active_member: bool,
}

fn team_leader_mount_flags(state: &AppState, session_id: &str) -> Option<TeamLeaderMount> {
    let conversation_id = team_session_conversation_id(session_id).ok()?;
    team_leader_mount_for_conversation(state, &conversation_id)
}

/// 队长挂载位的判定本体，按会话 id 直接查一次 meta。
///
/// 工具装配（`team_leader_mount_flags`）与技能装配（[`conversation_is_team_leader`]）共用这一份：
/// 托管技能要按会话角色去留，若技能层另写一份判定，「成员已离队但名册还在」这类边界上
/// 两处会分叉——工具当他是队员、技能当他是队长，同一轮里自相矛盾。
fn team_leader_mount_for_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Option<TeamLeaderMount> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return None;
    }
    let meta = conversation_service_v2()
        .get_conversation_meta(state, conversation_id)
        .ok()?;
    if meta.team.membership.is_some() {
        return None;
    }
    if !conversation_meta_can_own_team_roster(state, &meta) {
        return None;
    }
    Some(TeamLeaderMount {
        has_active_member: team_has_active_member(&meta.team),
    })
}

/// 这条会话是不是队长会话。托管技能的注入条件用这一个判据。
///
/// 只看名册权限与成员身份，不看名册内容：队长名下还没有队员时，也仍然是队长会话。
pub(crate) fn conversation_is_team_leader(state: &AppState, conversation_id: &str) -> bool {
    team_leader_mount_for_conversation(state, conversation_id).is_some()
}

/// 工具装配用的角色判定：队长（普通会话，不看开关与名册）/ 队员（带成员身份的追问）/ 都不是。
/// 开关与名册的细粒度过滤在装配处按 team_leader_mount_flags 再做。
fn team_role_for_session(state: &AppState, session_id: &str) -> TeamToolRole {
    let Ok(conversation_id) = team_session_conversation_id(session_id) else {
        return TeamToolRole::None;
    };
    let Ok(meta) = conversation_service_v2().get_conversation_meta(state, &conversation_id) else {
        return TeamToolRole::None;
    };
    if meta.team.membership.is_some() {
        return TeamToolRole::Member;
    }
    if conversation_meta_can_own_team_roster(state, &meta) {
        return TeamToolRole::Leader;
    }
    TeamToolRole::None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TeamToolRole {
    Leader,
    Member,
    None,
}

/// 建好一名队员后的结果，招募工具与预创建共用。
/// number 只给内部收尾（回滚、日志）用，不进入任何工具返回值。
struct TeamCreatedMember {
    role: String,
    duty: String,
    conversation_id: String,
}

fn team_tool_is_leader_tool(name: &str) -> bool {
    TEAM_LEADER_TOOL_NAMES.contains(&name)
}

/// 议题看板只在本地主会话挂载。
/// 队长身份比看板宽：远程私聊会话也能当队长，但它没有本地看板，
/// 挂上去只会让模型调用后吃一个「只能在主会话使用」的错误。
pub(crate) fn team_session_allows_kanban(state: &AppState, session_id: &str) -> bool {
    let Ok(conversation_id) = team_session_conversation_id(session_id) else {
        return false;
    };
    let Ok(meta) = conversation_service_v2().get_conversation_meta(state, &conversation_id) else {
        return false;
    };
    meta.conversation_kind.trim() == CONVERSATION_KIND_CHAT
        && !meta.is_remote_im_contact
        && !meta.is_delegate
        && !meta.is_draft
        && meta.archived_at.is_none()
}

fn team_identity_summary_message(role: &str, duty: &str) -> ChatMessage {
    let duty_line = if duty.trim().is_empty() {
        String::new()
    } else {
        format!("\n- 你的职责：{}", duty.trim())
    };
    let text = format!(
        "## 摘要说明\n\n- 你是这支队伍里的一名队员，角色是「{}」。{duty_line}\n\n---\n\n{guide}",
        role.trim(),
        guide = TEAM_MEMBER_GUIDE_MD.trim()
    );
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "user".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
        parts: vec![MessagePart::Text {
            text,
            reasoning_content: None,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(serde_json::json!({
            "message_meta": {
                "kind": "summary_context_seed",
                "scene": "team_member_seed",
                "schemaVersion": SUMMARY_CONTEXT_MESSAGE_SCHEMA_VERSION,
                // 这里不能写角色名：这块摘要的 title 会被当成会话的最新摘要标题，
                // 队员会话就再也不会生成内容标题了。名字由界面从 agent 取，不靠标题。
            }
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

/// 队伍消息的发信方：署名与发件人本人格。谁发的已由正文的「队长说：」「某某说：」写明，
/// 这里只补人格名与发件人本人格，不另存角色标记。
struct TeamMessageSender {
    /// 发信方角色名（人格名）；系统口径的通知为空。
    name: String,
    /// 发件人本人格。邮件必须由人本人发出，只有通知类才留空走系统人格。
    agent_id: Option<String>,
}

/// 队伍邮件的头：标题与随信携带的那条线状态。
/// 邮件与通知的分别就在这个结构有没有值——没有它的是通知，有它的是邮件。
struct TeamMailHeader {
    title: String,
    /// 六态 key。空表示这封邮件没有改状态。
    state: Option<String>,
    /// 状态是谁设的：leader / member。
    state_by: Option<&'static str>,
}

/// 标题超长不报错，截到上限。截断处补省略号，
/// 既让模型看得出没被原样采用，正文里也不像写漏了。
fn team_mail_title(raw: &str) -> String {
    let trimmed = raw.trim();
    let mut chars = trimmed.chars();
    let head: String = chars.by_ref().take(TEAM_MAIL_TITLE_MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// 邮件随信携带的状态写入。
///
/// `progress_state` 是唯一状态源。邮件正文里那行「谁把这条线设成了什么」与它是同一次
/// 操作的产物，所以写入只在这里发生一次：落库成功才允许入队，非法状态在落库前就被拒绝。
fn team_mail_apply_status(
    state: &AppState,
    conversation_id: &str,
    raw_status: Option<&str>,
    by: &str,
    title: &str,
) -> Result<Option<&'static str>, String> {
    let Some(raw) = raw_status.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let key = progress_state_normalized(raw).ok_or_else(|| {
        format!(
            "status 非法：{raw}，必须是六态 key 之一（{}）",
            PROGRESS_STATES.join(" / ")
        )
    })?;
    let who = if by == "member" { "队员" } else { "队长" };
    // 证据要能核对：写清是谁、在哪封邮件里设的，别让后来的人只能靠状态值猜。
    let evidence = format!("{who}在邮件「{}」里设置", title.trim());
    update_conversation_progress_state_and_emit(state, conversation_id, key, &evidence)?;
    Ok(Some(key))
}

fn team_system_message(
    text: String,
    message_kind: &str,
    sender: TeamMessageSender,
    mail: Option<TeamMailHeader>,
) -> ChatMessage {
    let mut provider_meta = serde_json::json!({
        "messageKind": message_kind,
        "teamMessage": true,
    });
    if !sender.name.trim().is_empty() {
        provider_meta["teamName"] = serde_json::json!(sender.name.trim());
    }
    // 邮件正文由服务端拼成一整段 markdown：标题进 <summary>，正文（含状态行）在折叠体内。
    // 默认收起：标题在外面、内容点开看，收起态也要能一眼读到标题说清了什么。
    // 写法与系统提醒共用同一份，正文里自带的结构标签会先被中和，队员写的正文撑不破外层。
    // 标题同时单列进 meta，供 team_read 回报时不重复搬运正文。
    let text = match mail {
        Some(mail) => {
            provider_meta["teamMailTitle"] = serde_json::json!(mail.title.trim());
            // 状态行由服务端写死：收件人与模型都直接读到「谁把这条线设成了什么」，
            // 不靠模型自己转述。状态只有 progress_state 一个源，这里只是把它写进正文；
            // meta 里不再存第二份，界面上也没有第二个投影要读。
            let status_line = match mail.state.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(state) => {
                    let who = if mail.state_by == Some("member") { "队员" } else { "队长" };
                    Some(format!(
                        "（{who}把这条线的状态设为：{}）",
                        progress_state_label_zh(state)
                    ))
                }
                None => None,
            };
            let body = match status_line {
                Some(line) => format!("{line}\n\n{text}"),
                None => text,
            };
            format_fold_block(mail.title.trim(), &body, false)
        }
        None => text,
    };
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now_iso(),
        // 邮件由发件人本人格署名；只有通知类留空，走系统人格。
        speaker_agent_id: Some(
            sender
                .agent_id
                .as_deref()
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .unwrap_or(SYSTEM_PERSONA_ID)
                .to_string(),
        ),
        parts: vec![MessagePart::Text {
            text,
            reasoning_content: None,
        }],
        extra_text_blocks: Vec::new(),
        provider_meta: Some(provider_meta),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

/// 队伍消息是否需要助手立刻作出反应。
///
/// 判据取消息类别，不取目标会话类型——按会话类型判断会让远端会话行为分叉。
/// `team_notice` 是状态留痕（「队员上下文已整理」「你已被请离」），读不读都不影响任何人干活；
/// 其余队伍消息是待处理结果（队长派的活、队员的汇报），不激活就会一直躺在历史里没人管。
fn team_message_activates_assistant(message: &ChatMessage) -> bool {
    let kind = message
        .provider_meta
        .as_ref()
        .and_then(|meta| meta.get("messageKind"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    !kind.eq_ignore_ascii_case(TEAM_NOTICE_KIND)
}

/// 把一条队伍消息投递到目标会话：来源=系统。
///
/// 是否激活助手只由消息类别决定（见 `team_message_activates_assistant`）：
/// notice 一律不激活、走普通队列；report 与派活一律激活。
/// `mode` 只对会激活的消息生效——留痕消息即使调用方要求 guided 也降级为普通队列，
/// 否则出队提升时会被重新置为激活（`claim_guided_queue_events_for_conversation`），
/// 只置 `activate_assistant = false` 而不收窄队列模式等于没改。
fn team_enqueue_message(
    state: &AppState,
    target_conversation_id: &str,
    message: ChatMessage,
    mode: &str,
) -> Result<(), String> {
    let target_meta =
        conversation_service_v2().get_conversation_meta(state, target_conversation_id)?;
    let activates_assistant = team_message_activates_assistant(&message);
    let queue_mode = if activates_assistant && mode.trim().eq_ignore_ascii_case("guided") {
        ChatQueueMode::Guided
    } else {
        ChatQueueMode::Normal
    };
    let mut runtime_context = runtime_context_new("team_message", TEAM_MESSAGE_KIND);
    runtime_context.request_id = Some(format!("team-message-request-{}", Uuid::new_v4()));
    runtime_context.dispatch_id = Some(format!("team-message-dispatch-{}", Uuid::new_v4()));
    runtime_context.target_conversation_id = Some(target_conversation_id.to_string());
    runtime_context.root_conversation_id = target_meta
        .root_conversation_id
        .clone()
        .or_else(|| Some(target_conversation_id.to_string()));
    runtime_context.executor_agent_id = Some(target_meta.agent_id.clone());
    let event = ChatPendingEvent {
        id: format!("team-message-{}", Uuid::new_v4()),
        conversation_id: target_conversation_id.to_string(),
        created_at: now_iso(),
        source: ChatEventSource::System,
        queue_mode,
        messages: vec![message],
        activate_assistant: activates_assistant,
        assistant_message_id: None,
        session_info: ChatSessionInfo {
            agent_id: target_meta.agent_id.clone(),
        },
        runtime_context: Some(runtime_context),
        sender_info: None,
    };
    let ingress = ingress_chat_event(state, event)?;
    trigger_chat_event_after_ingress_with_delay(state, ingress, std::time::Duration::from_secs(1));
    Ok(())
}

/// 队员那一轮请求出错时，把原始报错转给队长。
///
/// 判据只有一条：出错了没有。拿会话自己的 `team.membership`——有就是队员，发给它的
/// 队长；没有就什么都不做。不做计数、不做节流、不加任何按时间或按进度的判断。
/// 用户主动中断不算出错，不通知。
fn team_notify_leader_member_failed(state: &AppState, conversation_id: &str, error: &str) {
    let conversation_id = conversation_id.trim();
    let error = error.trim();
    if conversation_id.is_empty() || error.is_empty() || error == CHAT_ABORTED_BY_USER_ERROR {
        return;
    }
    let meta = match conversation_service_v2().get_conversation_meta(state, conversation_id) {
        Ok(meta) => meta,
        Err(err) => {
            runtime_log_warn(format!(
                "[队伍] 跳过，任务=队员出错通知，conversation_id={conversation_id}，原因=会话元数据读取失败，error={err}"
            ));
            return;
        }
    };
    let Some(membership) = meta.team.membership.as_ref() else {
        return;
    };
    let leader_conversation_id = membership.leader_conversation_id.trim().to_string();
    if leader_conversation_id.is_empty() {
        return;
    }
    let role = membership.name.trim();
    let role = if role.is_empty() { "队员" } else { role };
    // 标题只说两件事：哪个队员、什么错。多行报错取第一行当摘要，正文仍是原文。
    let brief = error.lines().next().unwrap_or("").trim();
    let title = team_mail_title(&format!("队员「{role}」请求失败：{brief}"));
    let message = team_system_message(
        error.to_string(),
        TEAM_MESSAGE_KIND,
        TeamMessageSender {
            name: String::new(),
            agent_id: None,
        },
        Some(TeamMailHeader {
            title,
            state: None,
            state_by: None,
        }),
    );
    match team_enqueue_message(state, &leader_conversation_id, message, "queue") {
        Ok(()) => runtime_log_info(format!(
            "[队伍] 完成，任务=队员出错通知，role={role}，conversation_id={conversation_id}，leader_conversation_id={leader_conversation_id}"
        )),
        Err(err) => runtime_log_warn(format!(
            "[队伍] 失败，任务=队员出错通知，role={role}，conversation_id={conversation_id}，leader_conversation_id={leader_conversation_id}，error={err}"
        )),
    }
}

/// 队员当前状态：正在跑（忙碌）/ 最近一轮失败（报错）/ 空闲。
/// 忙碌看运行态与待处理队列；报错看会话活动标记（后台跑完才会写，前台正在观看的会话不写）。
fn team_member_state(state: &AppState, conversation_id: &str) -> (String, Option<String>) {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return ("idle".to_string(), None);
    }
    let running = match get_conversation_runtime_state(state, conversation_id) {
        Ok(MainSessionState::AssistantStreaming) => Some("正在生成回复".to_string()),
        Ok(MainSessionState::OrganizingContext) => Some("正在整理上下文".to_string()),
        Ok(MainSessionState::Idle) => None,
        Err(err) => {
            runtime_log_warn(format!(
                "[队伍] 跳过，任务=读取队员运行态，conversation_id={conversation_id}，error={err}"
            ));
            None
        }
    };
    if let Some(detail) = running {
        return ("busy".to_string(), Some(detail));
    }
    match conversation_has_pending_queue_events(state, conversation_id) {
        Ok(true) => return ("busy".to_string(), Some("有排队消息待处理".to_string())),
        Ok(false) => {}
        Err(err) => runtime_log_warn(format!(
            "[队伍] 跳过，任务=读取队员待处理队列，conversation_id={conversation_id}，error={err}"
        )),
    }
    match conversation_list_activity_mark(state, conversation_id) {
        Some(mark) if mark.activity.trim() == "failed" => (
            "error".to_string(),
            Some(
                mark.failed_message
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| "上一轮执行失败".to_string()),
            ),
        ),
        _ => ("idle".to_string(), None),
    }
}

fn team_member_entry_json(
    state: &AppState,
    persona_names: &TeamPersonaNames,
    member: &TeamMemberEntry,
) -> Value {
    let (member_state, state_detail) = team_member_state(state, &member.conversation_id);
    let mut value = serde_json::json!({
        "role": team_member_role(state, persona_names, member),
        "duty": member.duty,
        "status": member.status,
        "conversationId": member.conversation_id,
        "recruitedAt": member.recruited_at,
        "dismissedAt": member.dismissed_at,
        "worktreePath": member.worktree_path,
        "workBranch": member.work_branch,
        "state": member_state,
    });
    if let Some(detail) = state_detail {
        value["stateDetail"] = serde_json::json!(detail);
    }
    value
}

/// 名册挂在队长会话上，队员会话被删掉时名册不会自己知道。
/// 查名册时就地核对一次：会话已不在的在队条目收口成已请离（编号保留），并把结果带回去。
fn team_close_missing_member_entries(
    state: &AppState,
    leader: &mut ConversationMetaView,
) -> Vec<String> {
    let missing = leader
        .team
        .members
        .iter()
        .filter(|member| member.status.trim() == TEAM_MEMBER_STATUS_ACTIVE)
        .filter(|member| !member.conversation_id.trim().is_empty())
        .filter(|member| {
            conversation_service_v2()
                .get_conversation_meta(state, member.conversation_id.trim())
                .is_err()
        })
        .map(|member| {
            (
                member.number,
                strip_legacy_member_number_prefix(&member.name),
            )
        })
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Vec::new();
    }
    let missing_numbers = missing
        .iter()
        .map(|(number, _)| *number)
        .collect::<Vec<_>>();
    let dismissed_at = now_iso();
    let leader_conversation_id = leader.id.clone();
    match state_update_conversation_metadata_cached(state, &leader_conversation_id, |conversation| {
        for member in conversation.team.members.iter_mut() {
            if member.status.trim() == TEAM_MEMBER_STATUS_ACTIVE
                && missing_numbers.contains(&member.number)
            {
                member.status = TEAM_MEMBER_STATUS_DISMISSED.to_string();
                member.dismissed_at = Some(dismissed_at.clone());
            }
        }
        Ok(())
    }) {
        Ok(_) => {
            for member in leader.team.members.iter_mut() {
                if member.status.trim() == TEAM_MEMBER_STATUS_ACTIVE
                    && missing_numbers.contains(&member.number)
                {
                    member.status = TEAM_MEMBER_STATUS_DISMISSED.to_string();
                    member.dismissed_at = Some(dismissed_at.clone());
                }
            }
            runtime_log_info(format!(
                "[队伍] 完成，任务=收口失效队员，leader_conversation_id={}，roles={:?}，status=dismissed",
                leader_conversation_id,
                missing.iter().map(|(_, role)| role).collect::<Vec<_>>()
            ));
        }
        Err(err) => runtime_log_warn(format!(
            "[队伍] 跳过，任务=收口失效队员，leader_conversation_id={}，error={}",
            leader_conversation_id, err
        )),
    }
    missing.into_iter().map(|(_, role)| role).collect()
}

/// 名册里在队队员的角色（人格名优先），与 `team_info` 的 `role` 字段同一口径。
///
/// 队伍以外的调用方（看板这类）要判断「某个角色在不在队」，统一走这里。
/// 两点刻意对齐 `team_info`，不要在外面自己读 `meta.team`：
/// - 先做一次失效队员收口：会话已删的队员必须先从名册里消失，否则「名册里查无此人」
///   这个判断会在别处永远成立失败，脏引用只是换个入口继续产；
/// - 角色取人格名而不是名册里存的历史名字，模型在 `team_info` 里看到的是哪个字符串，
///   这里就必须认哪个字符串，否则会出现「team_info 说有人、别处不认」的错位。
fn team_active_member_roles(
    state: &AppState,
    leader_conversation_id: &str,
) -> Result<Vec<String>, String> {
    let mut leader = conversation_service_v2().get_conversation_meta(state, leader_conversation_id)?;
    let persona_names = TeamPersonaNames::load(state);
    team_close_missing_member_entries(state, &mut leader);
    Ok(leader
        .team
        .members
        .iter()
        .filter(|member| member.status.trim() == TEAM_MEMBER_STATUS_ACTIVE)
        .map(|member| team_member_role(state, &persona_names, member))
        .filter(|role| !role.trim().is_empty())
        .collect())
}

fn builtin_team_info(
    state: &AppState,
    session_id: &str,
    args: TeamInfoToolArgs,
) -> Result<Value, String> {
    let mut leader = team_leader_session_meta(state, session_id)?;
    let persona_names = TeamPersonaNames::load(state);
    let closed_missing_members = team_close_missing_member_entries(state, &mut leader);
    let include_history = args
        .scope
        .as_deref()
        .map(|value| value.trim().eq_ignore_ascii_case("history"))
        .unwrap_or(false);
    let members = leader
        .team
        .members
        .iter()
        .filter(|member| {
            include_history || member.status.trim() == TEAM_MEMBER_STATUS_ACTIVE
        })
        .map(|member| team_member_entry_json(state, &persona_names, member))
        .collect::<Vec<_>>();
    let mut output = serde_json::json!({
        "ok": true,
        "scope": if include_history { "history" } else { "active" },
        "activeMemberCount": leader.team.active_member_count(),
        "members": members,
        "hasActiveMember": team_has_active_member(&leader.team),
    });
    if !closed_missing_members.is_empty() {
        output["closedMissingMembers"] = serde_json::json!(
            closed_missing_members
                .iter()
                .map(|role| serde_json::json!({ "role": role }))
                .collect::<Vec<_>>()
        );
    }
    Ok(output)
}

/// 队员自己的身份说明（给队员自己看名册范围用）。
fn builtin_team_report(
    state: &AppState,
    session_id: &str,
    args: TeamReportToolArgs,
) -> Result<Value, String> {
    let (meta, membership) = team_member_meta(state, session_id)?;
    let content = args.content.trim();
    if content.is_empty() {
        return Err("content 不能为空".to_string());
    }
    let title = team_mail_title(&args.title);
    if title.is_empty() {
        return Err("title 不能为空：汇报要有个标题，它是界面上始终露出的那一行".to_string());
    }
    let mode = args.mode.as_deref().unwrap_or("guided");
    // 署名用角色（人格名），不用名册里的历史名字：老数据把编号写进了名字。
    let persona_names = TeamPersonaNames::load(state);
    let role = persona_names
        .name_of(&meta.agent_id)
        .map(str::to_string)
        .unwrap_or_else(|| strip_legacy_member_number_prefix(&membership.name));
    // 队员自己推进时也能改自己这条线的状态：同样先落库，再入队。
    let state_key = team_mail_apply_status(
        state,
        &meta.id,
        args.status.as_deref(),
        "member",
        &title,
    )?;
    let body = format!("队员「{role}」说：{content}");
    let message = team_system_message(
        body.clone(),
        TEAM_MESSAGE_KIND,
        TeamMessageSender {
            name: role.clone(),
            agent_id: Some(meta.agent_id.clone()),
        },
        Some(TeamMailHeader {
            title: title.clone(),
            state: state_key.map(str::to_string),
            state_by: state_key.map(|_| "member"),
        }),
    );
    let message_id = message.id.clone();
    team_enqueue_message(state, &membership.leader_conversation_id, message, mode)?;
    runtime_log_info(format!(
        "[队伍] 完成，任务=队员汇报，conversation_id={}，role={role}，mode={mode}",
        membership.leader_conversation_id
    ));
    Ok(serde_json::json!({
        "ok": true,
        "accepted": true,
        "mode": if mode.trim().eq_ignore_ascii_case("guided") { "guided" } else { "queue" },
        "leaderConversationId": membership.leader_conversation_id,
        "messageId": message_id,
        "content": body,
    }))
}

/// 队长把自身当前生效模型推给指定队员。
/// 模型在调度准备阶段一次性快照，队员正在跑时改了也不影响当前这一轮，
/// 所以返回值必须带生效时机，避免队长误判「已经救活了」。
async fn builtin_team_share_model(
    state: &AppState,
    session_id: &str,
    args: TeamShareModelToolArgs,
) -> Result<Value, String> {
    let leader = team_leader_meta_for_roster(state, session_id)?;
    let leader_conversation_id = leader.id.clone();
    let persona_names = TeamPersonaNames::load(state);
    let member = team_find_active_member_by_role(
        state,
        &leader.team,
        &persona_names,
        &args.role,
        args.conversation_id.as_deref(),
    )?;
    let role = team_member_role(state, &persona_names, &member);
    let member_conversation_id = member.conversation_id.trim().to_string();
    if member_conversation_id.is_empty() {
        return Err(format!("「{role}」还没有可用的会话"));
    }
    if conversation_service_v2()
        .get_conversation_meta(state, &member_conversation_id)
        .is_err()
    {
        return Err(format!("「{role}」的会话已不存在，无法推送模型"));
    }
    let config = state_read_config_cached(state)?;
    let requested_model = args
        .model_api_config_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    // 指定模型走统一校验；不指定就用队长此刻真正在用的那个模型。
    let target_model = match requested_model {
        Some(requested) => resolve_validated_conversation_model_id(state, requested)?,
        None => resolve_effective_conversation_model(state, &config, &leader_conversation_id)
            .ok_or_else(|| "队长当前没有可用模型，无法推送".to_string())?,
    };
    let previous_model = conversation_service_v2()
        .get_conversation_meta(state, &member_conversation_id)
        .ok()
        .and_then(|conversation_meta| conversation_meta.preferred_api_config_id)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    conversation_service_v2().set_preferred_api_config_id(
        state,
        &member_conversation_id,
        Some(target_model.clone()),
    )?;
    let (member_state, _) = team_member_state(state, &member_conversation_id);
    let effective_timing = if member_state == "busy" {
        "after_current_round"
    } else {
        "next_dispatch"
    };
    // 队员自己的上下文里必须有这件事，否则下一轮它会困惑于行为突变。
    let notice = format!(
        "队长把你使用的模型换成了 {target_model}。原模型无法调用，你的职责和工作内容不变，接着手上的事继续。"
    );
    let message = team_system_message(
        notice,
        TEAM_NOTICE_KIND,
        TeamMessageSender {
            name: String::new(),
            agent_id: None,
        },
        None,
    );
    let mut notice_warning: Option<String> = None;
    if let Err(err) = conversation_service_v2()
        .append_message(state, &member_conversation_id, &message)
        .await
    {
        runtime_log_warn(format!(
            "[队伍] 失败，任务=推送模型通知写入，conversation_id={member_conversation_id}，error={err}"
        ));
        notice_warning = Some(err);
    } else {
        let _ = emit_unarchived_conversation_overview_item_updated_from_state(
            state,
            &member_conversation_id,
        );
    }
    runtime_log_info(format!(
        "[队伍] 完成，任务=队长推送模型，role={role}，conversation_id={member_conversation_id}，previous={}，model={target_model}，timing={effective_timing}",
        previous_model.as_deref().unwrap_or("")
    ));
    let mut output = serde_json::json!({
        "ok": true,
        "role": role,
        "conversationId": member_conversation_id,
        "previousModelApiConfigId": previous_model,
        "modelApiConfigId": target_model,
        "effectiveTiming": effective_timing,
        "notified": notice_warning.is_none(),
    });
    if let Some(warning) = notice_warning {
        output["noticeWarning"] = serde_json::json!(warning);
    }
    Ok(output)
}

fn builtin_team_send(
    state: &AppState,
    session_id: &str,
    args: TeamSendToolArgs,
) -> Result<Value, String> {
    let leader = team_leader_meta_for_roster(state, session_id)?;
    let content = args.content.trim();
    if content.is_empty() {
        return Err("content 不能为空".to_string());
    }
    let title = team_mail_title(&args.title);
    if title.is_empty() {
        return Err("title 不能为空：邮件要有个标题，它是界面上始终露出的那一行".to_string());
    }
    let persona_names = TeamPersonaNames::load(state);
    let member = team_find_active_member_by_role(
        state,
        &leader.team,
        &persona_names,
        &args.role,
        args.conversation_id.as_deref(),
    )?;
    let role = team_member_role(state, &persona_names, &member);
    let member_conversation_id = member.conversation_id.clone();
    if member_conversation_id.trim().is_empty() {
        return Err(format!("「{role}」还没有可用的会话"));
    }
    let mode = args.mode.as_deref().unwrap_or("guided");
    // 状态先落库再入队：正文状态行出现，就一定有一份对应的 progress_state。
    let state_key = team_mail_apply_status(
        state,
        &member_conversation_id,
        args.status.as_deref(),
        "leader",
        &title,
    )?;
    let body = format!("队长说：{content}");
    let message = team_system_message(
        body.clone(),
        TEAM_MESSAGE_KIND,
        TeamMessageSender {
            name: String::new(),
            agent_id: Some(leader.agent_id.clone()),
        },
        Some(TeamMailHeader {
            title: title.clone(),
            state: state_key.map(str::to_string),
            state_by: state_key.map(|_| "leader"),
        }),
    );
    let message_id = message.id.clone();
    team_enqueue_message(state, &member_conversation_id, message, mode)?;
    runtime_log_info(format!(
        "[队伍] 完成，任务=队长派话，conversation_id={member_conversation_id}，role={role}，mode={mode}"
    ));
    Ok(serde_json::json!({
        "ok": true,
        "accepted": true,
        "role": role,
        "mode": if mode.trim().eq_ignore_ascii_case("guided") { "guided" } else { "queue" },
        "conversationId": member_conversation_id,
        "messageId": message_id,
        "content": body,
    }))
}

/// 队长读队员最近的发言。
///
/// 两条硬边界：
/// - 目标一律经队长自己的名册解析，不接受任意 `conversation_id`。否则这个工具就是
///   「读任意会话」的通用入口，`limit` 上限也拦不住定向窥探。
/// - `limit` 一律 clamp 到上限，超限时在返回值里说明，不做静默截断——静默截断会让模型
///   以为自己看全了。
fn builtin_team_read(
    state: &AppState,
    session_id: &str,
    args: TeamReadToolArgs,
) -> Result<Value, String> {
    let leader = team_leader_meta_for_roster(state, session_id)?;
    let role_arg = args.role.as_deref().unwrap_or("").trim().to_string();
    let conversation_id_arg = args
        .conversation_id
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if role_arg.is_empty() && conversation_id_arg.is_empty() {
        return Err(
            "role 与 conversation_id 至少填一个：填队员的人格名，或 team_info 给出的 conversationId"
                .to_string(),
        );
    }
    let persona_names = TeamPersonaNames::load(state);
    let member = team_find_active_member_by_role(
        state,
        &leader.team,
        &persona_names,
        &role_arg,
        Some(conversation_id_arg.as_str()),
    )?;
    let role = team_member_role(state, &persona_names, &member);
    let member_conversation_id = member.conversation_id.trim().to_string();
    if member_conversation_id.is_empty() {
        return Err(format!("「{role}」还没有可用的会话"));
    }
    let requested_limit = args.limit.unwrap_or(TEAM_READ_LIMIT_DEFAULT);
    let limit = requested_limit.clamp(1, TEAM_READ_LIMIT_MAX);
    let paths = message_store::message_store_paths(&state.data_path, &member_conversation_id)?;

    let mut collected: Vec<ChatMessage> = Vec::new();
    let mut has_more = false;
    let mut scan_truncated = false;
    if let Some(page) = message_store::chat_store_read_recent_messages_page_cached(&paths, limit)? {
        collected = page.messages;
        has_more = page.has_more;
    }
    // 队员会话尾部常是工具调用或工具结果，没有可读正文。补读让「有正文的条数」够 limit，
    // 但只在还没到扫描上限时往前翻，否则长尾工具消息就能把这条轻量路径变成整读。
    while team_read_body_count(&collected) < limit && has_more {
        if collected.len() >= TEAM_READ_SCAN_MAX_MESSAGES {
            scan_truncated = true;
            break;
        }
        let Some(oldest_id) = collected.first().map(|message| message.id.clone()) else {
            break;
        };
        let Some(page) = message_store::chat_store_read_messages_before(&paths, &oldest_id, limit)?
        else {
            break;
        };
        if page.messages.is_empty() {
            has_more = false;
            break;
        }
        has_more = page.has_more;
        let mut merged = page.messages;
        merged.extend(collected);
        collected = merged;
    }

    let body_count = team_read_body_count(&collected);
    let messages = collected
        .iter()
        .map(team_read_message_entry)
        .collect::<Vec<_>>();
    let mut notes: Vec<String> = Vec::new();
    if requested_limit > TEAM_READ_LIMIT_MAX {
        notes.push(format!(
            "limit={requested_limit} 超过上限，本次按 {TEAM_READ_LIMIT_MAX} 条返回"
        ));
    }
    if body_count < limit {
        notes.push(format!(
            "「{role}」最近只有 {body_count} 条有正文的消息，其余是工具调用"
        ));
    }
    if scan_truncated {
        notes.push(format!(
            "补读到扫描上限（{TEAM_READ_SCAN_MAX_MESSAGES} 条）就停了，更早的内容没有继续往前翻"
        ));
    }
    runtime_log_info(format!(
        "[队伍] 完成，任务=队长读队员消息，leader_conversation_id={}，role={role}，limit={limit}，returned={}，body={body_count}",
        leader.id,
        messages.len()
    ));
    Ok(serde_json::json!({
        "ok": true,
        "role": role,
        "conversationId": member_conversation_id,
        "limit": limit,
        "returnedCount": messages.len(),
        "bodyMessageCount": body_count,
        "hasMore": has_more,
        "messages": messages,
        "notes": notes,
    }))
}

/// 消息的可读正文：只取 Text 片段。工具调用、工具结果没有 Text 片段，取出来是空串。
fn team_read_message_text(message: &ChatMessage) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn team_read_body_count(messages: &[ChatMessage]) -> usize {
    messages
        .iter()
        .filter(|message| !team_read_message_text(message).is_empty())
        .count()
}

/// 单条消息的摘要。
///
/// 标题单列，正文剥掉服务端拼在外层的折叠结构——两处用途不同，标题已经在 `title` 字段里，
/// 摘要不该把 `<details>` 这些结构标签搬运给模型。老消息首行是 `# 标题`，两种形态都认。
/// 不剥「队长说：」/「队员「X」说：」：那是模型读上下文时判断谁在说话的依据，与界面渲染不同。
/// 摘要压成一行并截断，返回值不搬运正文。
fn team_read_message_entry(message: &ChatMessage) -> Value {
    let meta_title = message
        .provider_meta
        .as_ref()
        .and_then(|meta| meta.get("teamMailTitle"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let text = team_read_message_text(message);
    let (fold_title, body) = match split_team_mail_fold(&text) {
        Some((fold_title, body)) => (Some(fold_title), body),
        None => (None, strip_mail_heading(&text, &meta_title)),
    };
    let title = if meta_title.is_empty() {
        fold_title.unwrap_or_default()
    } else {
        meta_title
    };
    let (summary, has_body) = if body.is_empty() {
        ("（工具调用，无正文）".to_string(), false)
    } else {
        (team_read_summary(&body), true)
    };
    let mut entry = serde_json::json!({
        "id": message.id,
        "role": message.role,
        "createdAt": message.created_at,
        "hasBody": has_body,
        "summary": summary,
    });
    if !title.is_empty() {
        entry["title"] = serde_json::json!(title);
    }
    entry
}

/// 拆开服务端拼的邮件折叠块：`<details>` / `<details open>` + `<summary>标题</summary>` + 正文 + `</details>`。
///
/// 只认 `format_fold_block` 生成的这一种形态，认不出就返回 None，交给老格式处理。
fn split_team_mail_fold(text: &str) -> Option<(String, String)> {
    let mut lines = text.trim().lines();
    let head = lines.next()?.trim().to_ascii_lowercase();
    if head != "<details>" && head != "<details open>" {
        return None;
    }
    let summary = lines
        .next()?
        .trim()
        .strip_prefix("<summary>")?
        .strip_suffix("</summary>")?
        .trim()
        .to_string();
    let body = lines.collect::<Vec<_>>().join("\n");
    let body = body.trim();
    let body = body.strip_suffix("</details>").unwrap_or(body);
    Some((summary, body.trim().to_string()))
}

/// 老格式邮件正文剥掉首行的 `# 标题`：标题已单列，摘要里不再重复一遍。
fn strip_mail_heading(text: &str, title: &str) -> String {
    if title.is_empty() {
        return text.to_string();
    }
    text.strip_prefix(&format!("# {title}"))
        .map(|rest| rest.trim_start().to_string())
        .unwrap_or_else(|| text.to_string())
}

fn team_read_summary(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = one_line.chars();
    let head: String = chars.by_ref().take(TEAM_READ_SUMMARY_MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    }
}

/// 建一名队员：占编号 → 建追问会话（带该角色的人格）→ 写成员身份与身份摘要 → 回填名册会话 id。
/// 编号只当内部主键，对外一律用角色（人格名）。招募三条入口（工具、界面、预创建）共用这一条路径。
/// 工作树不归招募管：队长自己用 git 建好，再把路径发给队员。
async fn team_create_member(
    state: &AppState,
    leader: &ConversationMetaView,
    role: &str,
    duty: &str,
    agent_id: &str,
) -> Result<TeamCreatedMember, String> {
    let leader_conversation_id = leader.id.clone();
    let role = role.trim().to_string();
    if role.is_empty() {
        return Err("role 不能为空".to_string());
    }
    let duty = duty.trim().to_string();
    let agent_id = agent_id.trim().to_string();

    // 先占编号并登记占位条目：编号单调递增、失败也不回收。
    let mut allocated_number: Option<u32> = None;
    let recruited_at = now_iso();
    state_update_conversation_metadata_cached(state, &leader_conversation_id, |conversation| {
        let number = conversation.team.next_number_for_recruit();
        conversation.team.next_number = number + 1;
        conversation.team.members.push(TeamMemberEntry {
            number,
            conversation_id: String::new(),
            name: role.clone(),
            duty: duty.clone(),
            status: TEAM_MEMBER_STATUS_ACTIVE.to_string(),
            recruited_at: recruited_at.clone(),
            dismissed_at: None,
            worktree_path: String::new(),
            work_branch: String::new(),
        });
        allocated_number = Some(number);
        Ok(())
    })?;
    let Some(number) = allocated_number else {
        return Err("招募失败：编号未能分配".to_string());
    };

    // 建队员会话：不带上下文，只写一条身份摘要消息；招募本身不激活队员。
    let mut member = match side_chat_build_record(state, &leader_conversation_id, leader, false) {
        Ok(record) => record,
        Err(err) => {
            rollback_team_member_entry(state, &leader_conversation_id, number, "");
            return Err(err);
        }
    };
    let member_conversation_id = member.id.clone();
    // 队员会话的标题留给自动标题去生成（与主会话同一套）：
    // 预填成角色名会让 `conversation_has_visible_title` 一直为真，自动标题永远不跑。
    // 标题已在 `side_chat_build_record` 按「不带上下文」留空，这里不再覆盖。
    // 队员名也不再挂在标题上——界面上需要名字的地方直接读会话绑定的 agent。

    // 队员会话模型：人格显式配置的模型优先，人格没配就跟随队长（沿父链向上）。
    // 不再走 conversation_preferred_model_repair_candidate —— 那条链在人格无模型时
    // 经 role:expert 哨兵落到专家模型，正是「队员新建即以专家模型工作」的根因。
    match load_runtime_organization_snapshot(state) {
        Ok(runtime_org) => {
            member.preferred_api_config_id = resolve_new_team_member_conversation_model(
                state,
                &runtime_org.config,
                &runtime_org.agents,
                &agent_id,
                &leader_conversation_id,
            );
        }
        Err(err) => {
            runtime_log_warn(format!(
                "[队伍] 跳过，任务=解析队员会话模型，agent_id={agent_id}，error={err}"
            ));
        }
    }
    if !agent_id.is_empty() {
        member.agent_id = agent_id.clone();
    }
    member.team.membership = Some(TeamMembership {
        leader_conversation_id: leader_conversation_id.clone(),
        number,
        name: role.clone(),
        duty: duty.clone(),
    });
    member
        .messages
        .push(team_identity_summary_message(&role, &duty));
    member.updated_at = now_iso();
    member.last_user_at = Some(member.updated_at.clone());
    if let Err(err) = side_chat_persist_record(state, &leader_conversation_id, &member) {
        rollback_team_member_entry(state, &leader_conversation_id, number, &member_conversation_id);
        return Err(err);
    }

    // 回填占位条目的会话 id。
    let member_id_for_entry = member_conversation_id.clone();
    state_update_conversation_metadata_cached(state, &leader_conversation_id, |conversation| {
        if let Some(entry) = conversation.team.members.iter_mut().find(|entry| entry.number == number) {
            entry.conversation_id = member_id_for_entry.clone();
        }
        Ok(())
    })?;

    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &leader_conversation_id,
    );
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &member_conversation_id,
    );
    runtime_log_info(format!(
        "[队伍] 完成，任务=招募队员，leader_conversation_id={leader_conversation_id}，role={role}，conversation_id={member_conversation_id}"
    ));
    Ok(TeamCreatedMember {
        role,
        duty,
        conversation_id: member_conversation_id,
    })
}

async fn builtin_team_recruit(
    state: &AppState,
    session_id: &str,
    args: TeamRecruitToolArgs,
) -> Result<Value, String> {
    let leader = team_leader_session_meta(state, session_id)?;
    let runtime_org = load_runtime_organization_snapshot(state)
        .map_err(|err| format!("读取人格失败：{err}"))?;
    let persona = team_resolve_persona(&runtime_org, &args.role)?;
    if persona.is_built_in_user || persona.is_built_in_system {
        return Err("队员不能是用户或系统人格".to_string());
    }
    let persona_id = persona.id.trim().to_string();
    let persona_name = persona.name.trim().to_string();
    if persona_name.is_empty() {
        return Err(format!("人格 {persona_id} 缺少名字，无法作为角色"));
    }
    team_ensure_role_available(state, &leader, &persona_id, &persona_name)?;
    let member = team_create_member(
        state,
        &leader,
        &persona_name,
        args.duty.as_deref().unwrap_or_default(),
        &persona_id,
    )
    .await?;
    Ok(serde_json::json!({
        "ok": true,
        "role": member.role,
        "duty": member.duty,
        "conversationId": member.conversation_id,
        "note": "队员已就位但不会被激活；要用它请再用 team_send 发一条指示",
    }))
}

/// 首轮发言时按新建会话里预选的顺序，把队员一次性建出来；只做一次，建完清空预选列表。
async fn team_create_pending_members(
    state: &AppState,
    conversation_id: &str,
) -> Result<Vec<String>, String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(Vec::new());
    }
    let meta = match conversation_service_v2().get_conversation_meta(state, conversation_id) {
        Ok(meta) => meta,
        Err(err) => {
            runtime_log_warn(format!(
                "[队伍] 跳过，任务=预创建队员，会话ID={conversation_id}，原因=读取会话失败，error={err}"
            ));
            return Ok(Vec::new());
        }
    };
    // 预选只在草稿阶段可写、建完立刻清空，本身就是一次性的。
    // 这里不能用 is_draft 判定：第一条用户消息落盘时草稿已经转正，等到调度开始时
    // is_draft 恒为 false，会把建员整个跳过。
    // 预选写入通道已随 set_conversation_team_mode 命令移除：本函数在 pending 恒空时
    // 自然跳过；如需恢复预选功能，见 git 历史。
    if meta.team.pending_member_agents.is_empty() {
        runtime_log_debug(format!(
            "[队伍] 跳过，任务=预创建队员，会话ID={conversation_id}，原因=没有预选队员"
        ));
        return Ok(Vec::new());
    }
    let pending = meta.team.pending_member_agents.clone();
    let pending_count = pending.len();
    // 先取人格快照再清预选：快照失败时预选还在，下次发言还能重试
    let runtime_org = match load_runtime_organization_snapshot(state) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            runtime_log_warn(format!(
                "[队伍] 跳过，任务=预创建队员，会话ID={conversation_id}，原因=读取人格失败，error={err}"
            ));
            return Ok(Vec::new());
        }
    };
    // 清空预选，避免中途失败后下一轮重复建员
    state_update_conversation_metadata_cached(state, conversation_id, |conversation| {
        conversation.team.pending_member_agents.clear();
        Ok(())
    })?;
    let mut created = Vec::new();
    for agent_id in pending {
        let agent_id = agent_id.trim().to_string();
        let Some(agent) = runtime_org
            .agents
            .iter()
            .find(|item| item.id.trim() == agent_id.as_str())
        else {
            runtime_log_warn(format!(
                "[队伍] 跳过，任务=预创建队员，会话ID={conversation_id}，原因=人格不存在，agent_id={agent_id}"
            ));
            continue;
        };
        let name = agent.name.trim().to_string();
        if name.is_empty() {
            runtime_log_warn(format!(
                "[队伍] 跳过，任务=预创建队员，会话ID={conversation_id}，原因=人格名为空，agent_id={agent_id}"
            ));
            continue;
        }
        // 预选是旧功能，但同样受「一个角色只能有一名队员」约束，不能绕过招募规则。
        // meta 是循环外的快照，建出前一名队员后要重读，否则同一批里的重复人格会漏检。
        let current = match conversation_service_v2().get_conversation_meta(state, conversation_id) {
            Ok(current) => current,
            Err(err) => {
                runtime_log_warn(format!(
                    "[队伍] 跳过，任务=预创建队员，会话ID={conversation_id}，原因=读取会话失败，error={err}"
                ));
                continue;
            }
        };
        if let Err(err) = team_ensure_role_available(state, &current, &agent_id, &name) {
            runtime_log_warn(format!(
                "[队伍] 跳过，任务=预创建队员，会话ID={conversation_id}，原因=角色已被占用，agent_id={agent_id}，error={err}"
            ));
            continue;
        }
        match team_create_member(state, &current, &name, "", &agent_id).await {
            Ok(member) => created.push(member.conversation_id),
            Err(err) => runtime_log_warn(format!(
                "[队伍] 失败，任务=预创建队员，会话ID={conversation_id}，agent_id={agent_id}，error={err}"
            )),
        }
    }
    if created.is_empty() {
        runtime_log_warn(format!(
            "[队伍] 跳过，任务=预创建队员，会话ID={conversation_id}，原因=没有建出任何队员，预选数={pending_count}"
        ));
    } else {
        runtime_log_info(format!(
            "[队伍] 完成，任务=预创建队员，会话ID={conversation_id}，创建数={}，预选数={pending_count}",
            created.len()
        ));
    }
    Ok(created)
}

/// 招募失败回滚：撤掉名册条目，并顺手拆掉可能已经落盘的队员会话引用，
/// 避免留下「存在于会话列表、带成员身份、却不在名册里」的孤儿。
fn rollback_team_member_entry(
    state: &AppState,
    leader_conversation_id: &str,
    number: u32,
    member_conversation_id: &str,
) {
    let member_id = member_conversation_id.trim().to_string();
    let member_id_for_leader = member_id.clone();
    let _ = state_update_conversation_metadata_cached(state, leader_conversation_id, |conversation| {
        conversation.team.members.retain(|entry| entry.number != number);
        if !member_id_for_leader.is_empty() {
            conversation
                .child_conversation_ids
                .retain(|id| id != &member_id_for_leader);
        }
        Ok(())
    });
    if !member_id.is_empty() {
        let _ = state_update_conversation_metadata_cached(state, &member_id, |conversation| {
            conversation.team.membership = None;
            Ok(())
        });
    }
}
