#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImEnqueueInput {
    channel_id: String,
    platform: RemoteImPlatform,
    im_name: String,
    remote_contact_type: String,
    remote_contact_id: String,
    #[serde(default)]
    remote_contact_name: Option<String>,
    sender_id: String,
    sender_name: String,
    #[serde(default)]
    sender_avatar_url: Option<String>,
    #[serde(default)]
    platform_message_id: Option<String>,
    #[serde(default)]
    dingtalk_session_webhook: Option<String>,
    #[serde(default)]
    dingtalk_session_webhook_expired_time: Option<i64>,
    session: SessionSelector,
    payload: ChatInputPayload,
}

const FAST_REQUEST_KIND_REMOTE_IM_REPLY_DECISION: &str = "remote_im_reply_decision";
// 群聊长度门改写（默认禁用，保留待重新启用）。
#[allow(dead_code)]
const FAST_REQUEST_KIND_REMOTE_IM_REPLY_REWRITE: &str = "remote_im_reply_rewrite";

fn provider_meta_string(meta: &Option<Value>, key: &str) -> Option<String> {
    meta.as_ref()
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn provider_meta_i64(meta: &Option<Value>, key: &str) -> Option<i64> {
    let value = meta.as_ref().and_then(|item| item.get(key))?;
    if let Some(v) = value.as_i64() {
        return Some(v);
    }
    value
        .as_str()
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .and_then(|raw| raw.parse::<i64>().ok())
}

fn resolve_dingtalk_session_webhook(input: &RemoteImEnqueueInput) -> Option<String> {
    let direct = input
        .dingtalk_session_webhook
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    if direct.is_some() {
        return direct;
    }

    provider_meta_string(&input.payload.provider_meta, "sessionWebhook")
        .or_else(|| provider_meta_string(&input.payload.provider_meta, "dingtalkSessionWebhook"))
}

fn resolve_dingtalk_session_webhook_expired_time(input: &RemoteImEnqueueInput) -> Option<i64> {
    input.dingtalk_session_webhook_expired_time
        .or_else(|| provider_meta_i64(&input.payload.provider_meta, "sessionWebhookExpiredTime"))
        .or_else(|| {
            provider_meta_i64(
                &input.payload.provider_meta,
                "dingtalkSessionWebhookExpiredTime",
            )
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImEnqueueResult {
    event_id: String,
    conversation_id: String,
    activate_assistant: bool,
    contact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactAllowSendUpdateInput {
    contact_id: String,
    allow_send: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactAllowSendFilesUpdateInput {
    contact_id: String,
    allow_send_files: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactAllowReceiveUpdateInput {
    contact_id: String,
    allow_receive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactBlockedMessagePrefixesUpdateInput {
    contact_id: String,
    #[serde(default)]
    blocked_message_prefixes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactBehaviorUpdateInput {
    contact_id: String,
    #[serde(default = "default_remote_im_contact_mute_keywords")]
    mute_keywords: Vec<String>,
    #[serde(default = "default_remote_im_contact_unmute_keywords")]
    unmute_keywords: Vec<String>,
    #[serde(default = "default_remote_im_contact_patience_seconds")]
    patience_seconds: u64,
    #[serde(default = "default_remote_im_contact_mute_duration_seconds")]
    mute_duration_seconds: u64,
    #[serde(default)]
    activation_cooldown_seconds: u64,
    #[serde(default = "default_remote_im_contact_blocked_message_prefixes")]
    blocked_message_prefixes: Vec<String>,
    #[serde(default)]
    group_reply_pacing: RemoteImGroupReplyPacing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactSettingsPatchInput {
    contact_id: String,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default = "default_remote_im_contact_processing_mode")]
    processing_mode: String,
    #[serde(default = "default_remote_im_contact_blocked_message_prefixes")]
    blocked_message_prefixes: Vec<String>,
    #[serde(default = "default_remote_im_contact_activation_mode")]
    activation_mode: String,
    #[serde(default)]
    activation_keywords: Vec<String>,
    #[serde(default = "default_remote_im_contact_mute_keywords")]
    mute_keywords: Vec<String>,
    #[serde(default = "default_remote_im_contact_unmute_keywords")]
    unmute_keywords: Vec<String>,
    #[serde(default = "default_remote_im_contact_patience_seconds")]
    patience_seconds: u64,
    #[serde(default = "default_remote_im_contact_mute_duration_seconds")]
    mute_duration_seconds: u64,
    #[serde(default)]
    activation_cooldown_seconds: u64,
    #[serde(default)]
    group_reply_pacing: RemoteImGroupReplyPacing,
    #[serde(default = "default_remote_im_contact_response_strategy")]
    response_strategy: String,
    #[serde(default = "default_remote_im_contact_response_guidance")]
    response_guidance: String,
    #[serde(default)]
    allow_receive: bool,
    #[serde(default)]
    allow_send: bool,
    #[serde(default)]
    allow_send_files: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactGroupCreateInput {
    channel_id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactGroupRenameInput {
    group_id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactGroupDeleteInput {
    group_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImSetContactGroupInput {
    channel_id: String,
    contact_ids: Vec<String>,
    /// 为空表示移入未分组
    #[serde(default)]
    group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactGroupDeleteResult {
    detached_contact_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImSetContactGroupResult {
    moved_contact_count: usize,
}

/// 批量设置：字段为 None 表示本次不改该项。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImBatchContactSettingsInput {
    contact_ids: Vec<String>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    processing_mode: Option<String>,
    #[serde(default)]
    activation_mode: Option<String>,
    #[serde(default)]
    activation_keywords: Option<Vec<String>>,
    #[serde(default)]
    response_strategy: Option<String>,
    /// 通信开关（可接收与可发送同值），与单联系人设置保持同一语义
    #[serde(default)]
    allow_communication: Option<bool>,
    #[serde(default)]
    allow_send_files: Option<bool>,
    /// 把选中联系人已有工作区的权限统一为该值
    #[serde(default)]
    workspace_access: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImBatchContactSettingsResult {
    updated_contact_ids: Vec<String>,
    /// 工作区权限被实际改动的联系人数
    updated_workspace_contact_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactActivationUpdateInput {
    contact_id: String,
    activation_mode: String,
    #[serde(default)]
    activation_keywords: Vec<String>,
    #[serde(default = "default_remote_im_contact_mute_keywords")]
    mute_keywords: Vec<String>,
    #[serde(default = "default_remote_im_contact_unmute_keywords")]
    unmute_keywords: Vec<String>,
    #[serde(default = "default_remote_im_contact_patience_seconds")]
    patience_seconds: u64,
    #[serde(default = "default_remote_im_contact_mute_duration_seconds")]
    mute_duration_seconds: u64,
    #[serde(default)]
    activation_cooldown_seconds: u64,
    #[serde(default = "default_remote_im_contact_response_strategy")]
    response_strategy: String,
    #[serde(default = "default_remote_im_contact_response_guidance")]
    response_guidance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactRemarkUpdateInput {
    contact_id: String,
    #[serde(default)]
    remark_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactRouteModeUpdateInput {
    contact_id: String,
    route_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactAgentBindingUpdateInput {
    contact_id: String,
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationModelInput {
    contact_id: String,
}

/// 联系人处理模型的实际归属：模型挂在联系人的会话上（会话 preferred_api_config_id）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationModelOutput {
    conversation_id: Option<String>,
    conversation_exists: bool,
    preferred_api_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactProcessingModeUpdateInput {
    contact_id: String,
    processing_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactDeleteInput {
    contact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactLogsInput {
    contact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactWorkspaceUpdateInput {
    contact_id: String,
    #[serde(default)]
    shell_workspaces: Vec<ShellWorkspaceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationSummary {
    contact_id: String,
    conversation_id: String,
    title: String,
    updated_at: String,
    last_message_at: Option<String>,
    message_count: usize,
    channel_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_name: Option<String>,
    channel_enabled: bool,
    platform: RemoteImPlatform,
    contact_display_name: String,
    bound_agent_id: Option<String>,
    processing_mode: String,
    /// private / group：前端据此判断该联系人会话能否拥有队伍名册。
    #[serde(default)]
    remote_contact_type: String,
    /// 该会话作为队长时的在队队员数（队伍磁贴直接显示）。
    #[serde(default)]
    team_member_count: usize,
    /// 在队队员名单（队伍管理页展示）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    team_members: Vec<RemoteImContactTeamMemberSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    preview_messages: Vec<ConversationPreviewMessage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactTeamMemberSummary {
    conversation_id: String,
    /// 角色名（人格名）。编号只在库里当主键，不随名册下发。
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationMessagesInput {
    contact_id: String,
}

pub(crate) fn remote_im_channel_by_id<'a>(
    config: &'a AppConfig,
    channel_id: &str,
) -> Option<&'a RemoteImChannelConfig> {
    config
        .remote_im_channels
        .iter()
        .find(|channel| channel.id == channel_id)
}

fn remote_im_upsert_contact_for_inbound(
    state: &AppState,
    input: &RemoteImEnqueueInput,
    now: &str,
) -> Result<String, String> {
    let default_allow_receive = false;
    if let Some(mut contact) = state_service_find_remote_im_contact_by_identity(
        state,
        &input.channel_id,
        input.remote_contact_type.trim(),
        &input.remote_contact_id,
    )? {
        if let Some(name) = input
            .remote_contact_name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            contact.remote_contact_name = name.to_string();
        }
        if let Some(avatar_url) = input
            .sender_avatar_url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            contact.avatar_url = avatar_url.to_string();
        }
        if matches!(input.platform, RemoteImPlatform::Dingtalk) {
            let session_webhook = resolve_dingtalk_session_webhook(input);
            if session_webhook.is_some() {
                contact.dingtalk_session_webhook = session_webhook;
            }
            let expired_time = resolve_dingtalk_session_webhook_expired_time(input);
            if expired_time.is_some() {
                contact.dingtalk_session_webhook_expired_time = expired_time;
            }
        }
        contact.last_message_at = Some(now.to_string());
        state_service_upsert_remote_im_contact(state, &contact)?;
        return Ok(contact.id.clone());
    }

    let contact_id = Uuid::new_v4().to_string();
    let contact = RemoteImContact {
        id: contact_id.clone(),
        channel_id: input.channel_id.clone(),
        platform: input.platform.clone(),
        remote_contact_type: input.remote_contact_type.trim().to_string(),
        remote_contact_id: input.remote_contact_id.trim().to_string(),
        remote_contact_name: input
            .remote_contact_name
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .to_string(),
        avatar_url: input
            .sender_avatar_url
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .to_string(),
        remark_name: String::new(),
        allow_send: default_allow_receive,
        // 微信渠道为私聊场景（bot 为本人扫码授权账号），默认允许发送文件；其余渠道保持默认关闭
        allow_send_files: matches!(input.platform, RemoteImPlatform::WeixinOc),
        allow_receive: default_allow_receive,
        activation_mode: "never".to_string(),
        activation_keywords: Vec::new(),
        mute_keywords: default_remote_im_contact_mute_keywords(),
        unmute_keywords: default_remote_im_contact_unmute_keywords(),
        patience_seconds: default_remote_im_contact_patience_seconds(),
        mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
        activation_cooldown_seconds: 0,
        route_mode: "dedicated_contact_conversation".to_string(),
        bound_agent_id: None,
        bound_conversation_id: None,
        processing_mode: "continuous".to_string(),
        response_strategy: default_remote_im_contact_response_strategy_for_type(
            input.remote_contact_type.as_str(),
        ),
        response_guidance: default_remote_im_contact_response_guidance(),
        blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
        group_reply_pacing: RemoteImGroupReplyPacing::default(),
        last_activated_at: None,
        last_message_at: Some(now.to_string()),
        dingtalk_session_webhook: if matches!(input.platform, RemoteImPlatform::Dingtalk) {
            resolve_dingtalk_session_webhook(input)
        } else {
            None
        },
        dingtalk_session_webhook_expired_time: if matches!(input.platform, RemoteImPlatform::Dingtalk)
        {
            resolve_dingtalk_session_webhook_expired_time(input)
        } else {
            None
        },
        onebot_group_members: Vec::new(),
        shell_workspaces: Vec::new(),
        group_id: None,
    };
    state_service_upsert_remote_im_contact(state, &contact)?;
    Ok(contact_id)
}

fn normalize_contact_activation_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "always" | "keyword" => value.trim().to_ascii_lowercase(),
        "never" => "never".to_string(),
        _ => "never".to_string(),
    }
}

fn normalize_contact_keyword_list(values: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for value in values {
        for segment in value.split(|ch| matches!(ch, ',' | '，' | '\n' | '\r')) {
            let trimmed = segment.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !seen.insert(trimmed.to_string()) {
                continue;
            }
            out.push(trimmed.to_string());
        }
    }
    out
}

fn normalize_contact_activation_keywords(values: &[String]) -> Vec<String> {
    normalize_contact_keyword_list(values)
}

fn normalize_contact_blocked_message_prefixes(values: &[String]) -> Vec<String> {
    let mut out = Vec::<String>::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for value in values {
        for segment in value.split(|ch: char| ch.is_whitespace()) {
            let trimmed = segment.trim();
            if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
                continue;
            }
            out.push(trimmed.to_string());
        }
    }
    out
}

fn remote_im_blocked_inbound_message_prefix(
    text: &str,
    blocked_prefixes: &[String],
) -> Option<String> {
    let trimmed = text.trim_start();
    blocked_prefixes
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .find(|prefix| trimmed.starts_with(*prefix))
        .map(ToOwned::to_owned)
}

fn normalize_contact_route_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "dedicated_contact_conversation" => "dedicated_contact_conversation".to_string(),
        _ => "dedicated_contact_conversation".to_string(),
    }
}

fn normalize_contact_processing_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "qa" => "qa".to_string(),
        _ => "continuous".to_string(),
    }
}

fn normalize_contact_response_strategy(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "smart_judge" => "smart_judge".to_string(),
        _ => "always_reply".to_string(),
    }
}

fn remote_im_contact_is_private(contact: &RemoteImContact) -> bool {
    contact
        .remote_contact_type
        .trim()
        .eq_ignore_ascii_case("private")
}

fn default_remote_im_contact_response_strategy_for_type(remote_contact_type: &str) -> String {
    if remote_contact_type.trim().eq_ignore_ascii_case("private") {
        "always_reply".to_string()
    } else {
        "smart_judge".to_string()
    }
}

fn effective_remote_im_contact_response_strategy(contact: &RemoteImContact) -> String {
    if remote_im_contact_is_private(contact) {
        "always_reply".to_string()
    } else {
        normalize_contact_response_strategy(&contact.response_strategy)
    }
}

fn normalize_remote_im_channel_response_guidance(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default_remote_im_contact_response_guidance()
    } else {
        trimmed.to_string()
    }
}

fn normalize_remote_im_channel_behavior_settings(
    value: &RemoteImChannelBehaviorSettings,
) -> RemoteImChannelBehaviorSettings {
    let mut normalized = value.clone();
    normalized.blocked_message_prefixes =
        normalize_contact_blocked_message_prefixes(&normalized.blocked_message_prefixes);
    normalized.mute_keywords = normalize_contact_keyword_list(&normalized.mute_keywords);
    normalized.unmute_keywords = normalize_contact_keyword_list(&normalized.unmute_keywords);
    normalized.response_guidance =
        normalize_remote_im_channel_response_guidance(&normalized.response_guidance);
    normalized
}

fn remote_im_channel_behavior_settings_from_channel(
    channel: &RemoteImChannelConfig,
) -> RemoteImChannelBehaviorSettings {
    normalize_remote_im_channel_behavior_settings(&channel.behavior_settings)
}

fn remote_im_channel_behavior_settings_for_contact(
    state: &AppState,
    contact: &RemoteImContact,
) -> RemoteImChannelBehaviorSettings {
    match state_read_config_cached(state) {
        Ok(config) => match config
            .remote_im_channels
            .iter()
            .find(|channel| channel.id == contact.channel_id)
        {
            Some(channel) => remote_im_channel_behavior_settings_from_channel(channel),
            None => {
                runtime_log_warn(format!(
                    "[远程IM] 渠道行为配置缺失，已使用默认值继续处理 渠道={}",
                    contact.channel_id
                ));
                RemoteImChannelBehaviorSettings::default()
            }
        },
        Err(err) => {
            runtime_log_warn(format!(
                "[远程IM] 渠道行为配置读取失败，已使用默认值继续处理 渠道={} 错误={}",
                contact.channel_id, err
            ));
            RemoteImChannelBehaviorSettings::default()
        }
    }
}

fn effective_remote_im_channel_response_guidance(
    state: &AppState,
    contact: &RemoteImContact,
) -> String {
    remote_im_channel_behavior_settings_for_contact(state, contact).response_guidance
}

include!("remote_im/group_reply_focus.rs");

include!("remote_im/multilingual_text_units.rs");

include!("remote_im/group_reply_energy.rs");

include!("remote_im/cursor.rs");

include!("remote_im/group_reply_state.rs");

include!("remote_im/reply_debounce.rs");

include!("remote_im/reply_delegate.rs");

include!("remote_im/presence_lifecycle.rs");

fn remote_im_contact_runtime_state_mut<'a>(
    states: &'a mut std::collections::HashMap<String, RemoteImContactRuntimeState>,
    contact_id: &str,
) -> &'a mut RemoteImContactRuntimeState {
    states
        .entry(contact_id.to_string())
        .or_insert_with(RemoteImContactRuntimeState::default)
}

/// 内存运行时的唯一写入口。
///
/// 状态是模块自己的私有信息，变化也由模块自己记录：调用方只交出「要改什么」和一句原因，
/// 这里比对新旧值、写状态、把真正的变化写进日志。调用点不再保留 before 快照，
/// 也不再自己拼在场日志——同一份状态只在这一处被描述。没变化就不打日志。
fn remote_im_apply_contact_runtime_state<F>(
    states: &mut std::collections::HashMap<String, RemoteImContactRuntimeState>,
    contact: &RemoteImContact,
    reason: &str,
    patience_seconds: Option<u64>,
    apply: F,
) where
    F: FnOnce(&mut RemoteImContactRuntimeState),
{
    let runtime = remote_im_contact_runtime_state_mut(states, &contact.id);
    let before = runtime.presence_state;
    apply(runtime);
    if before == runtime.presence_state {
        return;
    }
    let after = runtime.presence_state;
    // 预计离场只在「变成在场」时有意义：离场之后没有倒计时可说。
    let expire_at = match (after, patience_seconds) {
        (RemoteImPresenceState::Present, Some(seconds)) => {
            Some(format_local_time_after_seconds(seconds))
        }
        _ => None,
    };
    // 只描述一次：这条日志本身就会写进唯一日志，窗口按 contact_id 过滤得到。
    let expire_contact_log = expire_at
        .as_deref()
        .map(|at| format!("，预计 {at} 离场"))
        .unwrap_or_default();
    remote_im_append_contact_log(
        contact,
        "info",
        format!(
            "[在场切换] {} → {}{}（{}）",
            remote_im_presence_state_label(before),
            remote_im_presence_state_label(after),
            expire_contact_log,
            reason
        ),
    );
}

fn remote_im_contact_checkpoint_mut_in_list<'a>(
    checkpoints: &'a mut Vec<RemoteImContactCheckpoint>,
    contact_id: &str,
) -> &'a mut RemoteImContactCheckpoint {
    if let Some(index) = checkpoints
        .iter()
        .position(|item| item.contact_id == contact_id)
    {
        return &mut checkpoints[index];
    }
    checkpoints.push(RemoteImContactCheckpoint {
        contact_id: contact_id.to_string(),
        ..RemoteImContactCheckpoint::default()
    });
    let last_index = checkpoints.len().saturating_sub(1);
    &mut checkpoints[last_index]
}

fn remote_im_contact_by_source_in_runtime<'a>(
    contacts: &'a [RemoteImContact],
    source: &RemoteImMessageSource,
) -> Option<&'a RemoteImContact> {
    contacts.iter().find(|item| {
        item.channel_id == source.channel_id
            && item.remote_contact_type == source.remote_contact_type
            && item.remote_contact_id == source.remote_contact_id
    })
}

fn remote_im_contact_matches_reply_target(
    source: &RemoteImActivationSource,
    target: &RemoteImReplyTarget,
) -> bool {
    source.channel_id.trim() == target.channel_id.trim()
        && source.remote_contact_id.trim() == target.contact_id.trim()
}

fn remote_im_text_contains_keyword(text: &str, keyword: &str) -> bool {
    text.to_ascii_lowercase()
        .contains(&keyword.to_ascii_lowercase())
}

fn remote_im_find_matched_keyword<'a>(text: &str, keywords: &'a [String]) -> Option<&'a str> {
    keywords
        .iter()
        .map(|item| item.trim())
        .filter(|item| !item.is_empty())
        .find(|keyword| remote_im_text_contains_keyword(text, keyword))
}

fn remote_im_keyword_matched(contact: &RemoteImContact, message_text: &str) -> bool {
    remote_im_find_matched_keyword(message_text, &contact.activation_keywords).is_some()
}

fn remote_im_resolve_mute_until(now: time::OffsetDateTime, duration_seconds: u64) -> String {
    normalize_time_for_utc_storage(
        now + time::Duration::seconds(duration_seconds.min(i64::MAX as u64) as i64),
    )
    .unwrap_or_else(|_| now_iso())
}

fn remote_im_is_mute_expired(mute_until: &str, now: time::OffsetDateTime) -> bool {
    parse_iso(mute_until).map(|value| value <= now).unwrap_or(true)
}

fn remote_im_should_enter_group_inspection(
    contact: &RemoteImContact,
    message_text: &str,
) -> (bool, String) {
    match contact.activation_mode.trim().to_ascii_lowercase().as_str() {
        "always" => (true, "始终入场，进场".to_string()),
        "keyword" => {
            let matched = remote_im_keyword_matched(contact, message_text);
            if matched {
                (true, "命中点名词，进场".to_string())
            } else {
                (false, "未命中点名词，只入库".to_string())
            }
        }
        _ => (false, "不入场，只入库".to_string()),
    }
}

fn remote_im_prepare_enqueue_runtime_state(
    state: &AppState,
    contact: &RemoteImContact,
    message_text: &str,
) -> Result<(bool, String), String> {
    if contact
        .remote_contact_type
        .trim()
        .eq_ignore_ascii_case("private")
    {
        return Ok((
            true,
            "私聊禁用秘书与在场状态，消息直接调度绑定会话".to_string(),
        ));
    }
    let behavior = remote_im_channel_behavior_settings_for_contact(state, contact);
    let now = now_utc();
    let mut mute_prefix = String::new();
    let supports_mute = contact
        .remote_contact_type
        .trim()
        .eq_ignore_ascii_case("group");
    let mut runtime_states = lock_remote_im_contact_runtime_states(state)?;

    // 闭嘴：只动 mute_until 这一个状态位，读写都在这一段内完成，出去时已经知道要不要拦截。
    let mut blocked_reason: Option<String> = None;
    {
        let runtime = remote_im_contact_runtime_state_mut(&mut runtime_states, &contact.id);
        if !supports_mute {
            runtime.mute_until = None;
        }
        if supports_mute {
            if let Some(mute_until) = runtime.mute_until.clone() {
                if remote_im_is_mute_expired(&mute_until, now) {
                    runtime.mute_until = None;
                    mute_prefix = format!("闭嘴超时自动解除(截止={mute_until})；");
                }
            }
        }
        if supports_mute {
            if let Some(keyword) = remote_im_find_matched_keyword(message_text, &behavior.mute_keywords) {
                let mute_until = remote_im_resolve_mute_until(now, behavior.mute_duration_seconds);
                runtime.mute_until = Some(mute_until.clone());
                blocked_reason = Some(format!(
                    "{}命中闭嘴词“{}”，进入闭嘴直到 {}，直接拦截后续判定",
                    mute_prefix, keyword, mute_until
                ));
            }
        }
        if blocked_reason.is_none() && supports_mute && runtime.mute_until.is_some() {
            if let Some(keyword) = remote_im_find_matched_keyword(message_text, &behavior.unmute_keywords) {
                runtime.mute_until = None;
                mute_prefix.push_str(&format!("命中张嘴词“{}”，解除闭嘴；", keyword));
            } else {
                let mute_until = runtime.mute_until.clone().unwrap_or_default();
                blocked_reason = Some(format!(
                    "{}当前仍处于闭嘴期(截止={})，未命中张嘴词，直接拦截后续判定",
                    mute_prefix, mute_until
                ));
            }
        }
    }
    if let Some(reason) = blocked_reason {
        drop(runtime_states);
        runtime_log_info(format!(
            "[远程IM] 闭嘴 联系人={} 原因={}",
            remote_im_contact_log_label(contact),
            reason
        ));
        remote_im_append_contact_log(
            contact,
            "info",
            format!(
                "[远程IM] 闭嘴 联系人={}, 原因={}",
                remote_im_contact_log_label(contact),
                reason
            ),
        );
        remote_im_enforce_mute_side_effects(state, &contact.id, &reason);
        return Ok((false, reason));
    }

    // 在场/离场切换：走运行时唯一写入口，日志由运行时自己记录。已在场时状态不变，不产生日志。
    let currently_present = runtime_states
        .get(&contact.id)
        .map(|runtime| runtime.presence_state == RemoteImPresenceState::Present)
        .unwrap_or(false);
    let (activate_assistant, entry_reason) = if currently_present {
        (true, "已在场".to_string())
    } else {
        remote_im_should_enter_group_inspection(contact, message_text)
    };
    let reason = format!("{mute_prefix}{entry_reason}");
    if activate_assistant {
        remote_im_apply_contact_runtime_state(
            &mut runtime_states,
            contact,
            &reason,
            Some(behavior.patience_seconds),
            |runtime| {
                runtime.presence_state = RemoteImPresenceState::Present;
                runtime.last_presence_at = Some(now_iso());
                runtime.consecutive_no_reply_count = 0;
            },
        );
    }
    drop(runtime_states);
    if activate_assistant {
        if let Err(err) = remote_im_schedule_presence_timeout(
            state,
            &contact.id,
            behavior.patience_seconds,
        ) {
            runtime_log_warn(format!(
                "[远程IM] 在场 计时刷新失败 错误={}",
                err
            ));
        }
    }
    Ok((activate_assistant, reason))
}

include!("remote_im/secretary_decision.rs");

include!("remote_im/round_completion.rs");

include!("remote_im/message_routing.rs");

include!("remote_im/inbound_message.rs");

include!("remote_im/contact_commands.rs");
