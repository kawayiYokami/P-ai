// ==================== 会话队伍（team） ====================
//
// 队伍 = 一个普通会话（队长）+ 它的若干追问会话（队员）。
// - 队长工具：team_info / team_recruit / team_dismiss / team_send
// - 队员工具：team_report
// 名册落队长会话的 team 字段，成员身份落队员会话自己的 team 字段；两处同写、以队长为准。
// 通讯一律以系统身份投递到目标会话：引导或排队由发信方选择。

const TEAM_LEADER_TOOL_NAMES: [&str; 4] = ["team_info", "team_recruit", "team_dismiss", "team_send"];
const TEAM_MEMBER_TOOL_NAME: &str = "team_report";
const TEAM_MESSAGE_KIND: &str = "team_message";
const TEAM_NOTICE_KIND: &str = "team_notice";

// 两名角色的行事指引，各写各的：队长的随招募回执交给队长，队员的随身份摘要交给队员。
// 都不进系统提示词，只作为会话消息或工具回执出现。
const TEAM_LEADER_GUIDE_MD: &str =
    include_str!("../../../../../resources/prompts/team-leader-guide.md");
const TEAM_MEMBER_GUIDE_MD: &str =
    include_str!("../../../../../resources/prompts/team-member-guide.md");

fn team_session_conversation_id(session_id: &str) -> Result<String, String> {
    delegate_session_conversation_id(session_id)
        .ok_or_else(|| "队伍工具缺少 conversation_id".to_string())
}

/// 当前会话是不是队长：普通会话、且自己不是别人的队员。
fn team_leader_meta(state: &AppState, session_id: &str) -> Result<ConversationMetaView, String> {
    let conversation_id = team_session_conversation_id(session_id)?;
    let meta = conversation_service_v2().get_conversation_meta(state, &conversation_id)?;
    if meta.team.membership.is_some() {
        return Err("这是队员会话，不能使用队长的队伍工具；要汇报请用 team_report".to_string());
    }
    if meta.conversation_kind.trim() != CONVERSATION_KIND_CHAT
        || meta.is_draft
        || meta.status.trim() == "archived"
    {
        return Err("只有未归档的普通会话可以作为队长使用队伍工具".to_string());
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
    if meta.status.trim() == "archived" {
        return Err("会话已归档，不能再向队长汇报".to_string());
    }
    Ok((meta, membership))
}

/// 工具装配用的角色判定：队长（普通会话）/ 队员（带成员身份的追问）/ 都不是。
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
    if meta.conversation_kind.trim() == CONVERSATION_KIND_CHAT
        && meta.status.trim() != "archived"
        && !meta.is_draft
    {
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

fn team_tool_is_leader_tool(name: &str) -> bool {
    TEAM_LEADER_TOOL_NAMES.contains(&name)
}

fn team_identity_summary_message(
    number: u32,
    name: &str,
    duty: &str,
) -> ChatMessage {
    let duty_line = if duty.trim().is_empty() {
        String::new()
    } else {
        format!("\n- 你的职责：{}", duty.trim())
    };
    let text = format!(
        "## 摘要说明\n\n- 你是第 {number} 号队员，名字叫「{name}」。{duty_line}\n\n---\n\n{guide}",
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
                "title": name,
            }
        })),
        tool_call: None,
        mcp_call: None,
        meme_annotations: None,
    }
}

/// 队伍消息的发信方署名：谁发的、几号、叫什么，都落进 provider_meta，前端不必解析正文。
struct TeamMessageSender {
    /// leader / member / team（系统口径，如请离通知）。
    role: &'static str,
    number: Option<u32>,
    name: String,
}

fn team_system_message(
    text: String,
    message_kind: &str,
    sender: TeamMessageSender,
) -> ChatMessage {
    let mut provider_meta = serde_json::json!({
        "messageKind": message_kind,
        "teamMessage": true,
        "teamSenderRole": sender.role,
    });
    if let Some(number) = sender.number {
        provider_meta["teamNumber"] = serde_json::json!(number);
    }
    if !sender.name.trim().is_empty() {
        provider_meta["teamName"] = serde_json::json!(sender.name.trim());
    }
    ChatMessage {
        id: Uuid::new_v4().to_string(),
        role: "assistant".to_string(),
        created_at: now_iso(),
        speaker_agent_id: Some(SYSTEM_PERSONA_ID.to_string()),
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

/// 把一条队伍消息投递到目标会话：来源=系统，是否立即激活由调用方决定。
fn team_enqueue_message(
    state: &AppState,
    target_conversation_id: &str,
    message: ChatMessage,
    mode: &str,
) -> Result<(), String> {
    let target_meta =
        conversation_service_v2().get_conversation_meta(state, target_conversation_id)?;
    let queue_mode = if mode.trim().eq_ignore_ascii_case("guided") {
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
        activate_assistant: true,
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

fn team_member_entry_json(state: &AppState, member: &TeamMemberEntry) -> Value {
    let (member_state, state_detail) = team_member_state(state, &member.conversation_id);
    let mut value = serde_json::json!({
        "number": member.number,
        "name": member.name,
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
) -> Vec<(u32, String)> {
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
        .map(|member| (member.number, member.name.clone()))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return missing;
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
                "[队伍] 完成，任务=收口失效队员，leader_conversation_id={}，numbers={:?}，status=dismissed",
                leader_conversation_id, missing_numbers
            ));
        }
        Err(err) => runtime_log_warn(format!(
            "[队伍] 跳过，任务=收口失效队员，leader_conversation_id={}，numbers={:?}，error={}",
            leader_conversation_id, missing_numbers, err
        )),
    }
    missing
}

fn builtin_team_info(
    state: &AppState,
    session_id: &str,
    args: TeamInfoToolArgs,
) -> Result<Value, String> {
    let mut leader = team_leader_meta(state, session_id)?;
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
        .map(|member| team_member_entry_json(state, member))
        .collect::<Vec<_>>();
    let mut output = serde_json::json!({
        "ok": true,
        "scope": if include_history { "history" } else { "active" },
        "activeMemberCount": leader.team.active_member_count(),
        "nextNumber": leader.team.next_number_for_recruit(),
        "members": members,
    });
    if !closed_missing_members.is_empty() {
        output["closedMissingMembers"] = serde_json::json!(
            closed_missing_members
                .iter()
                .map(|(number, name)| serde_json::json!({ "number": number, "name": name }))
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
    let (_, membership) = team_member_meta(state, session_id)?;
    let content = args.content.trim();
    if content.is_empty() {
        return Err("content 不能为空".to_string());
    }
    let mode = args.mode.as_deref().unwrap_or("guided");
    let number = membership.number;
    let name = membership.name.trim().to_string();
    let body = format!("第 {number} 号队员「{name}」说：{content}");
    let message = team_system_message(
        body.clone(),
        TEAM_MESSAGE_KIND,
        TeamMessageSender {
            role: "member",
            number: Some(number),
            name: name.clone(),
        },
    );
    let message_id = message.id.clone();
    team_enqueue_message(state, &membership.leader_conversation_id, message, mode)?;
    runtime_log_info(format!(
        "[队伍] 完成，任务=队员汇报，conversation_id={}，number={number}，mode={mode}",
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

async fn builtin_team_dismiss(
    state: &AppState,
    session_id: &str,
    args: TeamDismissToolArgs,
) -> Result<Value, String> {
    let leader = team_leader_meta(state, session_id)?;
    let leader_conversation_id = leader.id.clone();
    let number = args.number;
    let mut dismissed: Option<(String, String)> = None;
    state_update_conversation_metadata_cached(state, &leader_conversation_id, |conversation| {
        let Some(member) = conversation
            .team
            .members
            .iter_mut()
            .find(|member| {
                member.number == number && member.status.trim() == TEAM_MEMBER_STATUS_ACTIVE
            })
        else {
            return Err(format!("队伍里没有 {number} 号在队队员"));
        };
        member.status = TEAM_MEMBER_STATUS_DISMISSED.to_string();
        member.dismissed_at = Some(now_iso());
        dismissed = Some((member.conversation_id.clone(), member.name.clone()));
        Ok(())
    })?;
    let Some((member_conversation_id, member_name)) = dismissed else {
        return Err(format!("队伍里没有 {number} 号在队队员"));
    };
    // 队员会话转为普通追问：清身份标记，并留一条不激活的系统消息说明缘由。
    // 身份清除失败就必须把名册条目退回在队，否则会出现「名册说已请离、会话仍是队员」的死结。
    // 但会话已被用户删掉是另一回事：没有会话可清、也没有地方留通知，这时名册照样收口。
    let member_conversation_exists = !member_conversation_id.trim().is_empty()
        && conversation_service_v2()
            .get_conversation_meta(state, &member_conversation_id)
            .is_ok();
    if member_conversation_exists {
        let member_id = member_conversation_id.clone();
        if let Err(err) = state_update_conversation_metadata_cached(state, &member_id, |conversation| {
            conversation.team.membership = None;
            Ok(())
        }) {
            restore_team_member_entry(state, &leader_conversation_id, number);
            runtime_log_error(format!(
                "[队伍] 失败，任务=请离队员，leader_conversation_id={leader_conversation_id}，number={number}，error={err}"
            ));
            return Err(format!(
                "请离失败：{number} 号队员的身份没有清除成功，名册已退回在队状态：{err}"
            ));
        }
    }
    let notice = format!(
        "你已经完成了你的职责，队长决定把你移出队伍。此后你是一个普通的追问会话，不用再等队长的指示。"
    );
    let message = team_system_message(
        notice,
        TEAM_NOTICE_KIND,
        TeamMessageSender {
            role: "team",
            number: None,
            name: String::new(),
        },
    );
    // 通知会写进队员会话；写失败要能被工具返回值看见，不能让成功与真实写入脱钩。
    let mut notice_warning: Option<String> = None;
    if member_conversation_exists {
        if let Err(err) = conversation_service_v2()
            .append_message(state, &member_conversation_id, &message)
            .await
        {
            runtime_log_warn(format!(
                "[队伍] 失败，任务=请离通知写入，conversation_id={member_conversation_id}，error={err}"
            ));
            notice_warning = Some(err);
        } else {
            let _ = emit_unarchived_conversation_overview_item_updated_from_state(
                state,
                &member_conversation_id,
            );
        }
    }
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &leader_conversation_id,
    );
    runtime_log_info(format!(
        "[队伍] 完成，任务=请离队员，leader_conversation_id={leader_conversation_id}，number={number}，name={member_name}"
    ));
    Ok(serde_json::json!({
        "ok": true,
        "number": number,
        "name": member_name,
        "conversationId": member_conversation_id,
        "noticeWarning": notice_warning,
        "note": "已转为普通追问，会话内容与工作树都保留",
    }))
}

fn builtin_team_send(
    state: &AppState,
    session_id: &str,
    args: TeamSendToolArgs,
) -> Result<Value, String> {
    let leader = team_leader_meta(state, session_id)?;
    let content = args.content.trim();
    if content.is_empty() {
        return Err("content 不能为空".to_string());
    }
    let Some(member) = leader.team.find_active_member_by_number(args.number) else {
        return Err(format!("队伍里没有 {} 号在队队员", args.number));
    };
    let member_conversation_id = member.conversation_id.clone();
    if member_conversation_id.trim().is_empty() {
        return Err(format!("{} 号队员还没有可用的会话", args.number));
    }
    let mode = args.mode.as_deref().unwrap_or("guided");
    let body = format!("队长说：{content}");
    let message = team_system_message(
        body.clone(),
        TEAM_MESSAGE_KIND,
        TeamMessageSender {
            role: "leader",
            number: None,
            name: String::new(),
        },
    );
    let message_id = message.id.clone();
    team_enqueue_message(state, &member_conversation_id, message, mode)?;
    runtime_log_info(format!(
        "[队伍] 完成，任务=队长派话，conversation_id={member_conversation_id}，number={}，mode={mode}",
        args.number
    ));
    Ok(serde_json::json!({
        "ok": true,
        "accepted": true,
        "number": args.number,
        "name": member.name,
        "mode": if mode.trim().eq_ignore_ascii_case("guided") { "guided" } else { "queue" },
        "conversationId": member_conversation_id,
        "messageId": message_id,
        "content": body,
    }))
}

async fn builtin_team_recruit(
    state: &AppState,
    session_id: &str,
    args: TeamRecruitToolArgs,
) -> Result<Value, String> {
    let leader = team_leader_meta(state, session_id)?;
    let leader_conversation_id = leader.id.clone();
    let name = args.name.trim().to_string();
    if name.is_empty() {
        return Err("name 不能为空".to_string());
    }
    let duty = args.duty.unwrap_or_default().trim().to_string();
    let work_branch = args.work_branch.unwrap_or_default().trim().to_string();

    // 先占编号并登记占位条目：编号单调递增、失败也不回收。
    let mut allocated_number: Option<u32> = None;
    let recruited_at = now_iso();
    state_update_conversation_metadata_cached(state, &leader_conversation_id, |conversation| {
        if conversation.team.has_active_member_named(&name) {
            return Err(format!("队伍里已经有叫「{name}」的队员了"));
        }
        let number = conversation.team.next_number_for_recruit();
        conversation.team.next_number = number + 1;
        conversation.team.members.push(TeamMemberEntry {
            number,
            conversation_id: String::new(),
            name: name.clone(),
            duty: duty.clone(),
            status: TEAM_MEMBER_STATUS_ACTIVE.to_string(),
            recruited_at: recruited_at.clone(),
            dismissed_at: None,
            worktree_path: String::new(),
            work_branch: work_branch.clone(),
        });
        allocated_number = Some(number);
        Ok(())
    })?;
    let Some(number) = allocated_number else {
        return Err("招募失败：编号未能分配".to_string());
    };

    // 建队员会话：不带上下文，只写一条身份摘要消息；招募本身不激活队员。
    let mut member = match side_chat_build_record(state, &leader_conversation_id, &leader, false) {
        Ok(record) => record,
        Err(err) => {
            rollback_team_member_entry(state, &leader_conversation_id, number, "");
            return Err(err);
        }
    };
    let member_conversation_id = member.id.clone();
    member.title = name.clone();
    member.team.membership = Some(TeamMembership {
        leader_conversation_id: leader_conversation_id.clone(),
        number,
        name: name.clone(),
        duty: duty.clone(),
    });
    member
        .messages
        .push(team_identity_summary_message(
            number,
            &name,
            &duty,
        ));
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

    // 可选：为这名队员建一条自己的工作树，从队长给的分支起。
    let mut worktree_path = String::new();
    let mut worktree_warning: Option<String> = None;
    if !work_branch.is_empty() {
        match team_create_member_worktree(state, &member, &work_branch).await {
            Ok(path) => {
                worktree_path = path;
                runtime_log_info(format!(
                    "[队伍] 完成，任务=队员工作树，conversation_id={member_conversation_id}，branch={work_branch}，path={worktree_path}"
                ));
            }
            Err(err) => {
                runtime_log_warn(format!(
                    "[队伍] 跳过，任务=队员工作树，conversation_id={member_conversation_id}，error={err}"
                ));
                worktree_warning = Some(err);
            }
        }
    }
    if !worktree_path.is_empty() {
        let path_for_entry = worktree_path.clone();
        let branch_for_entry = work_branch.clone();
        state_update_conversation_metadata_cached(state, &leader_conversation_id, |conversation| {
            if let Some(entry) = conversation.team.members.iter_mut().find(|entry| entry.number == number) {
                entry.worktree_path = path_for_entry.clone();
                entry.work_branch = branch_for_entry.clone();
            }
            Ok(())
        })?;
    }
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &leader_conversation_id,
    );
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(
        state,
        &member_conversation_id,
    );
    runtime_log_info(format!(
        "[队伍] 完成，任务=招募队员，leader_conversation_id={leader_conversation_id}，number={number}，name={name}，conversation_id={member_conversation_id}"
    ));
    Ok(serde_json::json!({
        "ok": true,
        "number": number,
        "name": name,
        "duty": duty,
        "conversationId": member_conversation_id,
        "worktreePath": worktree_path,
        "workBranch": work_branch,
        "worktreeWarning": worktree_warning,
        "note": "队员已就位但不会被激活；要用它请再用 team_send 发一条指示",
        "leaderGuide": TEAM_LEADER_GUIDE_MD.trim(),
    }))
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

/// 请离过程中队员身份清除失败时的退回：把名册条目恢复成在队，保持两侧一致。
fn restore_team_member_entry(state: &AppState, leader_conversation_id: &str, number: u32) {
    let _ = state_update_conversation_metadata_cached(state, leader_conversation_id, |conversation| {
        if let Some(entry) = conversation
            .team
            .members
            .iter_mut()
            .find(|entry| entry.number == number)
        {
            entry.status = TEAM_MEMBER_STATUS_ACTIVE.to_string();
            entry.dismissed_at = None;
        }
        Ok(())
    });
}

/// 为队员建专属工作树并从指定分支起，成功后把绑定写回队员会话字段。
async fn team_create_member_worktree(
    state: &AppState,
    member: &Conversation,
    work_branch: &str,
) -> Result<String, String> {
    let git_root = member.shell_workspace_path.clone().unwrap_or_default();
    if git_root.trim().is_empty() {
        return Err("队员会话没有可用的工作目录，无法创建工作树".to_string());
    }
    let worktree_path = PathBuf::from(&git_root)
        .join(".pai")
        .join(".worktree")
        .join(conversation_worktree_dir_name(&member.id));
    ensure_conversation_worktree_internal(&git_root, &worktree_path, work_branch).await?;
    let path_text = worktree_path.to_string_lossy().to_string();
    let member_id = member.id.clone();
    let path_for_meta = path_text.clone();
    let branch_for_meta = work_branch.to_string();
    state_update_conversation_metadata_cached(state, &member_id, |conversation| {
        conversation.shell_work_mode = SHELL_WORK_MODE_WORKTREE.to_string();
        conversation.shell_worktree_path = path_for_meta.clone();
        conversation.shell_work_branch = branch_for_meta.clone();
        Ok(())
    })?;
    let _ = emit_unarchived_conversation_overview_item_updated_from_state(state, &member_id);
    Ok(path_text)
}
