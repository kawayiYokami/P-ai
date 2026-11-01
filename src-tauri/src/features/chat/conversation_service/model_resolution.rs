// ==================== 会话模型解析 ====================
// 口径见 .pai/plan/chat/20261101_会话模型继承与专家模型降级.md
// 「这个会话该用哪个模型」只在这里回答，调用方不再各自兜底。
//
// 两条链刻意分开：
//   - 继承链（inherit）：会话自身 → 父链，只读会话字段，不碰专家模型。
//   - 生效链（effective）：继承链 → 专家模型 → 第一个可用聊天文本模型，用于模型丢失后的兜底。
// 创建会话时走「创建链」，终点是「第一个可用聊天文本模型」，绝不落在专家模型上。
//
// 不读 main_conversation_id：那个指针在存储层恒指向系统通知会话（conversation.rs 的
// repair_main_conversation_marker_if_needed 会强制改写），它的 preferred_api_config_id 恒为
// None，拿它当「主会话模型」永远读不到东西。用户「最近一次亲手选定的模型」由
// expert_api_config_id 承载，见文件末尾的同步函数。

/// 解析出一个具体的、可用于聊天文本的模型 id。
/// 空值、角色哨兵（role:*）、模型不存在、非聊天文本模型，一律 None。
/// 哨兵必须拒绝而不是展开：让会话字段里的 role:expert 解析成专家模型，
/// 等于把会话钉死在一个「用户看不见的兜底模型」上，与设计相悖。
fn resolve_concrete_chat_model(
    app_config: &AppConfig,
    raw_api_config_id: Option<&str>,
) -> Option<String> {
    let raw_api_config_id = raw_api_config_id
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if is_model_role_api_config_id(raw_api_config_id) {
        return None;
    }
    resolve_chat_api_config_id(app_config, raw_api_config_id)
}

/// 全链最终兜底：配置里第一个真正可用的聊天文本模型。
/// 判据用严格版（启用文本、有 base_url、有模型名、鉴权齐备）：
/// 落到一个缺 key 的模型上等于发送时必然失败，还不如当场判定「没有可用模型」。
/// 其余调用点（显式指定、格式校验、下拉列表）仍用宽松判据 is_text_chat_api，语义不同不能混。
fn first_available_chat_model(app_config: &AppConfig) -> Option<String> {
    app_config
        .api_configs
        .iter()
        .find(|api| is_usable_text_llm_api(api))
        .map(|api| api.id.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 人格显式配置的模型。
/// 只读原始字段，不用 agent_primary_api_config_id —— 后者在人格无模型时会塞入
/// role:expert 哨兵，用它判断等于「永远配了模型」，会把专家模型误认成用户显式选择。
fn agent_explicit_chat_model(
    app_config: &AppConfig,
    agents: &[AgentProfile],
    agent_id: &str,
) -> Option<String> {
    let agent_id = agent_id.trim();
    if agent_id.is_empty() {
        return None;
    }
    let agent = agents
        .iter()
        .find(|agent| agent.id.trim() == agent_id)?;
    merge_api_config_ids(&agent.api_config_ids, &agent.api_config_id)
        .into_iter()
        .find_map(|raw_id| resolve_concrete_chat_model(app_config, Some(&raw_id)))
}

/// 会话自身字段里存的模型，只看字段，不向上找。
fn conversation_stored_chat_model(
    state: &AppState,
    app_config: &AppConfig,
    conversation_id: &str,
) -> Option<String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return None;
    }
    let conversation_meta = conversation_service_v2()
        .get_conversation_meta(state, conversation_id)
        .ok()?;
    resolve_concrete_chat_model(app_config, conversation_meta.preferred_api_config_id.as_deref())
}

/// 沿 parent_conversation_id 逐级向上找会话模型。
/// 遇空、自指、已归档、会话不存在即停，交给下一级。
fn ancestor_conversation_chat_model(
    state: &AppState,
    app_config: &AppConfig,
    start_conversation_id: &str,
) -> Option<String> {
    let mut visited = std::collections::HashSet::<String>::new();
    visited.insert(start_conversation_id.trim().to_string());
    let mut cursor = conversation_service_v2()
        .get_conversation_meta(state, start_conversation_id.trim())
        .ok()
        .and_then(|conversation_meta| conversation_meta.parent_conversation_id)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    while let Some(current_id) = cursor {
        if !visited.insert(current_id.clone()) {
            return None;
        }
        let conversation_meta = match conversation_service_v2()
            .get_conversation_meta(state, &current_id)
        {
            Ok(conversation_meta) => conversation_meta,
            Err(_) => return None,
        };
        if conversation_meta.archived_at.is_some() {
            return None;
        }
        if let Some(resolved) =
            resolve_concrete_chat_model(app_config, conversation_meta.preferred_api_config_id.as_deref())
        {
            return Some(resolved);
        }
        cursor = conversation_meta
            .parent_conversation_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
    }
    None
}

/// 专家模型当前指向的模型（配置里可能为空或已失效）。
fn expert_chat_model(app_config: &AppConfig) -> Option<String> {
    resolve_concrete_chat_model(app_config, Some(&app_config.expert_api_config_id))
}

/// 继承链：会话自身 → 父链。
/// 创建会话时用它找「该继承谁」，因此不含专家模型，也不含「第一个可用模型」。
fn resolve_inherited_conversation_model(
    state: &AppState,
    app_config: &AppConfig,
    conversation_id: &str,
) -> Option<String> {
    conversation_stored_chat_model(state, app_config, conversation_id)
        .or_else(|| ancestor_conversation_chat_model(state, app_config, conversation_id))
}

/// 当前生效模型：这一轮调度真正会用的那个。
/// 继承链 → 专家模型 → 第一个可用聊天文本模型。
pub(crate) fn resolve_effective_conversation_model(
    state: &AppState,
    app_config: &AppConfig,
    conversation_id: &str,
) -> Option<String> {
    resolve_inherited_conversation_model(state, app_config, conversation_id)
        .or_else(|| expert_chat_model(app_config))
        .or_else(|| first_available_chat_model(app_config))
}

/// 生效链整条落空时的提示，也是全仓「没有可用聊天模型」的唯一文案。
///
/// 三处共用这一句：本文件的生效链解析、候选队列构建（core_send_inner 的
/// build_chat_candidate_api_ids 之后）、专家模型 JSON 调用路径（inference_gateway）。
/// 三处说的是同一件事，各写一句会让用户看到三种不同措辞，也会让以后改口径时漏掉两处。
///
/// 三件必须同时满足，少一件就会误导：
/// 1. 说「没有可用的聊天模型」而不是「模型出错」——这是配置状态，不是故障；
///    说成出错会让用户去查网络和供应商状态，方向完全错。
/// 2. 指向「去设置里配置一个可用的模型」这个动作，不能说「请在输入面板选择模型」——
///    模型被删之后那个面板上没有可选项，用户拿到这句话就是死路，只能猜或者重装。
/// 3. 把「可用」的判准写出来（启用文本聊天、接口地址、模型名、鉴权），
///    否则用户配了一个不合格的模型、回来还是这句话。
pub(crate) const NO_USABLE_CHAT_MODEL_MESSAGE: &str =
    "没有可用的聊天模型：请在设置里配置一个可用的模型（需要启用文本聊天、填写接口地址与模型名，并完成鉴权）";

/// 「解析结果与存量不同就写回」的唯一实现。
/// 快照修复与发送前写回共用这一处，避免两处判据将来各写一套再漂移。
/// 传入 None 表示「整条链都没有可用模型」，此时会清空存量值——快照修复需要这个行为；
/// 发送路径在调用前已经确认解析成功，不会传 None。
fn persist_conversation_model_when_changed(
    state: &AppState,
    conversation_id: &str,
    current: Option<&str>,
    resolved: Option<String>,
    reason: &str,
) -> Result<bool, String> {
    let current = current.map(str::trim).filter(|value| !value.is_empty());
    if resolved.as_deref() == current {
        return Ok(false);
    }
    conversation_service_v2().set_preferred_api_config_id(state, conversation_id, resolved.clone())?;
    runtime_log_debug(format!(
        "[会话模型] 完成，任务={reason}，conversation_id={conversation_id}，旧值={}，新值={}",
        current.unwrap_or(""),
        resolved.as_deref().unwrap_or("")
    ));
    Ok(true)
}

/// 发送与继续执行前的取模型口径：解析出生效模型，并把它写回会话字段。
///
/// 写回只在真的解析出可用模型、且与存量不同时发生；写回失败只记 warn，不阻断发送。
/// 不弹提示：模型已经被删了，用户当场处理不了，每次发消息弹一次是噪音；
/// 写回本身是自愈，前端下次拿快照看到的就是真实值，不再出现「显示未选择、实际在跑兜底模型」。
pub(crate) fn resolve_and_persist_dispatch_model(
    state: &AppState,
    app_config: &AppConfig,
    conversation_meta: &ConversationMetaView,
) -> Result<String, String> {
    let resolved = resolve_effective_conversation_model(state, app_config, &conversation_meta.id)
        .ok_or_else(|| NO_USABLE_CHAT_MODEL_MESSAGE.to_string())?;
    if let Err(err) = persist_conversation_model_when_changed(
        state,
        &conversation_meta.id,
        conversation_meta.preferred_api_config_id.as_deref(),
        Some(resolved.clone()),
        "发送前写回实际模型",
    ) {
        runtime_log_warn(format!(
            "[会话模型] 跳过，任务=发送前写回实际模型，conversation_id={}，api_config_id={}，error={err}",
            conversation_meta.id, resolved
        ));
    }
    Ok(resolved)
}

/// 主会话创建时的取值：入参指定 → 人格显式模型 → 第一个可用聊天文本模型。
pub(crate) fn resolve_new_main_conversation_model(
    app_config: &AppConfig,
    agents: &[AgentProfile],
    requested_api_config_id: Option<&str>,
    agent_id: &str,
) -> Option<String> {
    resolve_concrete_chat_model(app_config, requested_api_config_id)
        .or_else(|| agent_explicit_chat_model(app_config, agents, agent_id))
        .or_else(|| first_available_chat_model(app_config))
}

/// 追问会话创建时的取值：沿父会话继承。
pub(crate) fn resolve_new_side_chat_model(
    state: &AppState,
    app_config: &AppConfig,
    parent_conversation_id: &str,
) -> Option<String> {
    resolve_inherited_conversation_model(state, app_config, parent_conversation_id)
        .or_else(|| first_available_chat_model(app_config))
}

/// 队员会话创建时的取值：人格显式模型 → 队长会话模型（沿父链） → 第一个可用聊天文本模型。
/// 人格没显式配模型时不参与决策，直接跟随队长，避免落到专家模型。
pub(crate) fn resolve_new_team_member_conversation_model(
    state: &AppState,
    app_config: &AppConfig,
    agents: &[AgentProfile],
    agent_id: &str,
    leader_conversation_id: &str,
) -> Option<String> {
    agent_explicit_chat_model(app_config, agents, agent_id)
        .or_else(|| {
            resolve_inherited_conversation_model(state, app_config, leader_conversation_id)
        })
        .or_else(|| first_available_chat_model(app_config))
}

/// 远程联系人会话创建时的取值：根会话模型（沿父链） → 第一个可用聊天文本模型。
pub(crate) fn resolve_new_remote_im_contact_model(
    state: &AppState,
    app_config: &AppConfig,
    root_conversation_id: &str,
) -> Option<String> {
    resolve_inherited_conversation_model(state, app_config, root_conversation_id)
        .or_else(|| first_available_chat_model(app_config))
}

/// 专家模型被动跟随：把用户最近一次亲手选定的模型记为兜底值。
/// 只认「普通对话会话」（kind == chat）：追问、队员、联系人、系统通知、草稿都不是
/// 用户在对主对话里做选择的地方；自动修复写回也走不到这里。
/// 判据不能用 conversation_kind_is_local_chat —— 它把 side_chat 也算进来。
pub(crate) fn sync_expert_model_from_user_conversation(
    state: &AppState,
    conversation_id: &str,
    preferred_api_config_id: Option<&str>,
) -> Result<(), String> {
    let conversation_id = conversation_id.trim();
    if conversation_id.is_empty() {
        return Ok(());
    }
    let conversation_meta = conversation_service_v2()
        .get_conversation_meta(state, conversation_id)?;
    if conversation_meta.conversation_kind.trim() != CONVERSATION_KIND_CHAT
        || conversation_meta.is_draft
        || conversation_meta.archived_at.is_some()
    {
        return Ok(());
    }
    let resolved = resolve_concrete_chat_model(
        &state_read_config_cached(state)?,
        preferred_api_config_id,
    );
    let Some(resolved) = resolved else {
        return Ok(());
    };
    let mut config = state_read_config_cached(state)?;
    if config.expert_api_config_id.trim() == resolved {
        return Ok(());
    }
    config.expert_api_config_id = resolved.clone();
    state_write_config_cached(state, &config)?;
    runtime_log_info(format!(
        "[会话模型] 完成，任务=专家模型跟随用户选择，api_config_id={resolved}"
    ));
    Ok(())
}

/// 专家模型为空（或指向的模型已失效）时补一个可用值。
/// 只在「还没有一个可用的专家模型」时生效，之后由用户手动改会话模型来跟随，
/// 新建第二个会话不会把兜底值改走。
pub(crate) fn sync_expert_model_when_unset(
    state: &AppState,
    preferred_api_config_id: Option<&str>,
) -> Result<(), String> {
    let config = state_read_config_cached(state)?;
    if expert_chat_model(&config).is_some() {
        return Ok(());
    }
    let resolved = resolve_concrete_chat_model(&config, preferred_api_config_id);
    let Some(resolved) = resolved else {
        return Ok(());
    };
    if config.expert_api_config_id.trim() == resolved {
        return Ok(());
    }
    let mut config = config;
    config.expert_api_config_id = resolved.clone();
    state_write_config_cached(state, &config)?;
    runtime_log_info(format!(
        "[会话模型] 完成，任务=补齐缺失的专家模型，api_config_id={resolved}"
    ));
    Ok(())
}
