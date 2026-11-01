// ==================== 会话状态（六态）进度工具 ====================
// 会话状态是「这个目标走到哪一步了」的用户可见事实，与运行生命周期
// （scheduling_state）分层：本工具只写会话元数据 progress_state，
// 不返回任何内部控制字段、不影响工具循环。
const PROGRESS_TOOL_NAME: &str = "progress";

/// 六态 key。空串 = 未设置（会话元数据默认值），不作为合法入参。
const PROGRESS_STATES: [&str; 6] = [
    "planning",
    "todo",
    "in_progress",
    "in_review",
    "blocked",
    "done",
];

/// 入参键名与 schema 保持 snake_case 一致（progress_state），不做 camelCase 重命名，
/// 避免 LLM 按 schema 传 progress_state 时反序列化失败；
/// alias 兼容按 camelCase 习惯传来的 progressState（照 goal.rs blocking_condition 先例）。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct ProgressWriteRequest {
    /// 六态 key 之一，必填。
    #[serde(alias = "progressState")]
    progress_state: String,
    /// 标 done 时必填：用户表态的原话或可核对依据，不是复述自己的判断。
    #[serde(default)]
    evidence: String,
    /// 标 blocked 时必填：缺什么、需要补充什么。
    #[serde(default)]
    reason: String,
}

fn progress_state_normalized(raw: &str) -> Option<&'static str> {
    let trimmed = raw.trim().to_ascii_lowercase();
    PROGRESS_STATES.into_iter().find(|key| *key == trimmed)
}

/// 六态的中文短名，只用于写进给人看的文本（队伍邮件的状态行）。
/// 与前端 `chat.conversationStatus.*` 文案对齐；两边都改时记得同步。
fn progress_state_label_zh(state_key: &str) -> &'static str {
    match state_key {
        "planning" => "规划中",
        "todo" => "待办",
        "in_progress" => "进行中",
        "in_review" => "待审核",
        "blocked" => "阻塞",
        "done" => "完成",
        _ => "未设置",
    }
}

/// 入参格式校验（仿 goal.rs 的证据强制：格式层强制，不做语义拦截）：
/// - progress_state 必须是六态 key，空值拒绝；
/// - done 必须带非空 evidence；
/// - blocked 必须带非空 reason；
/// - 其余状态两个可选字段留空即可。
/// 返回统一落库的 evidence 文本（blocked 时用 reason 承担，见字段注释）。
fn progress_request_evidence_for_store(
    state_key: &str,
    evidence: &str,
    reason: &str,
) -> Result<String, String> {
    match state_key {
        "done" => {
            let evidence = evidence.trim();
            if evidence.is_empty() {
                return Err(
                    "标 done 必须提供非空 evidence（用户表态的原话或可核对依据）".to_string(),
                );
            }
            Ok(evidence.to_string())
        }
        "blocked" => {
            let reason = reason.trim();
            if reason.is_empty() {
                return Err("标 blocked 必须提供非空 reason（缺什么、需要补充什么）".to_string());
            }
            Ok(reason.to_string())
        }
        // 离开 done / blocked 后旧证据失效，统一清空，避免 UI 挂着过期证据。
        _ => Ok(String::new()),
    }
}

/// 六段阶段提示词（设计文档 §二）：每段两拍——现在处于什么阶段 → 接下来自然该做什么。
/// 「进行中」埋了推向待审核的默认出口；「待审核」埋了禁止自行宣布完成；
/// 「阻塞」埋了下一轮先查解除。
fn progress_stage_prompt(state_key: &str) -> &'static str {
    match state_key {
        "planning" => "当前处于规划阶段：方案还没有定。先把目标、边界和已知约束理清楚，给出一个可执行的方案；方案没定之前不要动手实现。",
        "todo" => "当前处于待办阶段：下一步已经明确，但还没开始。直接从第一步做起，不要重复规划。",
        "in_progress" => "当前处于执行阶段：正在产出这次请求要的东西。做完后自查是否覆盖了原始要求，然后转入待审核等用户确认。",
        "in_review" => "当前处于待验收阶段：交付物已经产出，等用户表态。不要自行宣布完成；把结果和需要用户确认的点摆出来，等确认后再收口。",
        "blocked" => "当前处于阻塞阶段：缺少继续推进的必要条件，硬推只会产出错的结果。先确认缺什么，把原因和需要补充的东西说清楚，然后停下等待；下一轮先检查阻塞是否已经解除。",
        "done" => "当前处于完成阶段：用户已确认目标达成，这一目标收口。不要继续做额外的事；用户提出新要求后，按新一轮工作重新更新状态。",
        _ => "",
    }
}

fn progress_tool_description() -> String {
    format!(
        "会话状态（六态）更新工具，把「这个目标走到哪一步了」写进会话元数据，随列表下发给用户可见。\
         六态判据：planning=方案还没定；todo=下一步已明确但没开始；in_progress=正在产出交付物；\
         in_review=交付物已产出等用户确认（不要自行宣布完成）；blocked=缺少继续推进的必要条件；\
         done=用户已确认目标达成。\
         规则：progress_state 必填且为六态 key 之一；标 done 必须提供 evidence（用户表态的原话或可核对依据，\
         不是复述自己的判断）；标 blocked 必须提供 reason（缺什么）；两者为空会被拒绝写入。\
         本轮没有产出或事实没有变化时不要调用本工具（什么都不写是默认行为）；\
         状态与证据均未变化时不会落盘也不会推送事件（防抖），但仍返回对应阶段提示词。\
         会话状态与运行生命周期分层，与 todo 工具互补：todo 是步骤追踪器，本工具是面向用户的阶段事实。"
    )
}

fn progress_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        PROGRESS_TOOL_NAME,
        progress_tool_description(),
        serde_json::json!({
            "type": "object",
            "properties": {
                "progress_state": {
                    "type": "string",
                    "enum": PROGRESS_STATES,
                    "description": "六态 key 之一：planning / todo / in_progress / in_review / blocked / done"
                },
                "evidence": {
                    "type": "string",
                    "description": "标 done 时必填：用户表态的原话或可核对依据，不是复述自己的判断"
                },
                "reason": {
                    "type": "string",
                    "description": "标 blocked 时必填：缺什么、需要补充什么"
                }
            },
            "required": ["progress_state"]
        }),
    )
}

/// 从工具 session key 解析会话 ID。
/// 注意：`delegate_session_conversation_id` 的 `delegate_` 前缀是历史命名，
/// 它解析的是通用 `agent::conversation` session key（goal 工具同样在用），
/// 不是委托会话专用；progress 只在 chat/side_chat 挂载与此解析无关。
fn progress_target_conversation_id(session_id: &str) -> Result<String, String> {
    delegate_session_conversation_id(session_id)
        .ok_or_else(|| "progress 工具缺少 conversation_id，无法定位当前会话".to_string())
}

/// 进度工具执行入口：格式校验 → 防抖写入（含事件双发）→ 返回确认 + 阶段提示词。
/// 防抖跳过时同样返回阶段提示词（照常引导模型），只是不落盘不推送。
fn builtin_progress(
    app_state: &AppState,
    session_id: &str,
    args: ProgressWriteRequest,
) -> Result<String, String> {
    let conversation_id = progress_target_conversation_id(session_id)?;
    let state_key = progress_state_normalized(&args.progress_state)
        .ok_or_else(|| {
            format!(
                "progress_state 非法：{}，必须是六态 key 之一（{}）",
                args.progress_state.trim(),
                PROGRESS_STATES.join(" / ")
            )
        })?;
    let stored_evidence =
        progress_request_evidence_for_store(state_key, &args.evidence, &args.reason)?;
    let changed = update_conversation_progress_state_and_emit(
        app_state,
        &conversation_id,
        state_key,
        &stored_evidence,
    )?;
    let confirmation = if changed {
        format!("已记录：当前状态 {}", state_key)
    } else {
        // 防抖：状态与证据未变，不落盘不推送，但照常返回提示词引导本轮行为。
        format!("状态未变化（{}），保持不变", state_key)
    };
    Ok(format!(
        "{}\n\n{}",
        confirmation,
        progress_stage_prompt(state_key)
    ))
}

#[cfg(test)]
fn progress_state_of_conversation(
    state: &AppState,
    conversation_id: &str,
) -> Result<(String, String), String> {
    conversation_service_v2()
        .get_conversation_meta(state, conversation_id.trim())
        .map(|meta| (meta.progress_state, meta.evidence))
        .map_err(|_| format!("未找到会话，conversation_id={conversation_id}"))
}

/// 状态注入块：把当前会话状态与最近证据注入系统提示，让 LLM 每轮都知道
/// 「这个目标走到哪一步了」。未设置（空串）不注入。
/// 注意：系统提示在工具循环内只构建一次，本轮工具写入的状态要到下一次轮次才生效
/// （注入点注释同步说明，防误读为实时）。
fn build_progress_state_block(conversation: &Conversation) -> Option<String> {
    let state_key = progress_state_normalized(&conversation.progress_state)?;
    // 白名单与挂载档位一致：仅追问会话注入——主会话管议题流转（看板），不写会话状态；
    // 其余 kind（delegate / remote_im_contact / system_notification）同样不注入。
    if conversation.conversation_kind.trim() != CONVERSATION_KIND_SIDE_CHAT {
        return None;
    }
    let mut body = format!("当前会话状态：{state_key}");
    let evidence = conversation.evidence.trim();
    if !evidence.is_empty() {
        body.push_str("\n最近一次状态证据：");
        body.push_str(evidence);
    }
    body.push_str("\n若本轮事实已变化，用进度工具更新状态；状态必须与事实一致。");
    Some(prompt_xml_block("conversation progress state", body))
}

#[cfg(test)]
mod progress_tool_tests {
    use super::*;

    #[test]
    fn progress_state_normalized_should_accept_six_keys_and_reject_others() {
        for key in ["planning", "todo", "in_progress", "in_review", "blocked", "done"] {
            assert_eq!(progress_state_normalized(key), Some(key));
            assert_eq!(progress_state_normalized(&format!("  {key}  ")), Some(key));
        }
        // 空串 = 未设置，不是合法入参
        assert_eq!(progress_state_normalized(""), None);
        assert_eq!(progress_state_normalized("   "), None);
        assert_eq!(progress_state_normalized("finished"), None);
        assert_eq!(progress_state_normalized("PLAN"), None);
    }

    #[test]
    fn progress_evidence_store_should_enforce_done_and_blocked_format() {
        // done 无 evidence → 报错
        assert!(progress_request_evidence_for_store("done", "", "").is_err());
        assert!(progress_request_evidence_for_store("done", "   ", "").is_err());
        // done 有 evidence → 通过并原样返回
        assert_eq!(
            progress_request_evidence_for_store("done", " 用户说：可以交付了 ", "")
                .expect("done with evidence should pass"),
            "用户说：可以交付了"
        );
        // blocked 无 reason → 报错
        assert!(progress_request_evidence_for_store("blocked", "", "").is_err());
        assert!(progress_request_evidence_for_store("blocked", "", "  ").is_err());
        // blocked 有 reason → 通过，reason 承担落库 evidence 字段
        assert_eq!(
            progress_request_evidence_for_store("blocked", "", " 缺少 API Key ")
                .expect("blocked with reason should pass"),
            "缺少 API Key"
        );
        // 中间态不要求证据，且清空旧证据
        for key in ["planning", "todo", "in_progress", "in_review"] {
            assert_eq!(
                progress_request_evidence_for_store(key, "过期证据", "过期原因")
                    .expect("intermediate states should pass"),
                ""
            );
        }
    }

    #[test]
    fn progress_stage_prompt_block_should_skip_unset_and_foreign_kinds() {
        let mut chat = Conversation::default_for_progress_test();
        // 未设置 → 不注入
        assert!(build_progress_state_block(&chat).is_none());
        chat.progress_state = "in_progress".to_string();
        // 主会话管议题流转用看板，库里残留的旧状态也不再注入
        assert!(build_progress_state_block(&chat).is_none());
        // 追问会话（含队员会话）注入；state key 与证据进块
        chat.conversation_kind = CONVERSATION_KIND_SIDE_CHAT.to_string();
        let block = build_progress_state_block(&chat).expect("side chat should inject");
        assert!(block.contains("in_progress"));
        chat.evidence = "用户原话".to_string();
        let block =
            build_progress_state_block(&chat).expect("side chat should inject with evidence");
        assert!(block.contains("用户原话"));
        // 白名单：chat / system_notification / delegate / remote_im_contact 都不注入
        for kind in [
            CONVERSATION_KIND_CHAT,
            CONVERSATION_KIND_SYSTEM_NOTIFICATION,
            CONVERSATION_KIND_DELEGATE,
            CONVERSATION_KIND_REMOTE_IM_CONTACT,
        ] {
            chat.conversation_kind = kind.to_string();
            assert!(
                build_progress_state_block(&chat).is_none(),
                "kind={kind} should not inject"
            );
        }
    }

    #[test]
    fn progress_rule_block_should_exist_and_keep_unknown_fallback() {
        // 两处注册之一：build_builtin_tool_rule_block 的 progress 分支。
        // 漏分支时 match 兜底 `_ => return None` 静默不注入，不报错，故必须有断言看守。
        let block =
            build_builtin_tool_rule_block("progress", true).expect("progress rule block should exist");
        assert!(block.contains("progress tool rule"));
        assert!(block.contains("evidence"), "规则块应包含证据要求");
        // 兜底分支保持：未知 tool_id 仍返回 None
        assert!(build_builtin_tool_rule_block("no_such_tool", true).is_none());
    }
}

#[cfg(test)]
impl Conversation {
    /// 测试专用最小构造：只填状态注入块需要的字段。
    fn default_for_progress_test() -> Self {
        Conversation {
            id: "conversation-progress-test".to_string(),
            title: String::new(),
            agent_id: DEFAULT_AGENT_ID.to_string(),
            bound_conversation_id: None,
            parent_conversation_id: None,
            child_conversation_ids: Vec::new(),
            fork_message_cursor: None,
            unread_count: 0,
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            status: "active".to_string(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: Vec::new(),
            shell_autonomous_mode: false,
            shell_work_mode: default_shell_work_mode(),
            shell_work_branch: String::new(),
            shell_worktree_path: String::new(),
            shell_recorded_branch: String::new(),
            archived_at: None,
            messages: Vec::new(),
            fast_request_turns: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
            preferred_api_config_id: None,
            is_draft: false,
            auto_push_remote_contact_id: None,
            cumulative_usage: ConversationCumulativeUsage::default(),
            active_goal: None,
            team: TeamState::default(),
            loaded_mcp_tools: Vec::new(),
            last_error: None,
            progress_state: String::new(),
            evidence: String::new(),
            evidence_updated_at: String::new(),
        }
    }
}
