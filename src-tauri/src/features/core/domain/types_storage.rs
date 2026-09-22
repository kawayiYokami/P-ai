#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfTextCacheEntry {
    pub file_hash: String,
    pub file_path: String,
    pub file_name: String,
    pub extracted_text: String,
    pub total_pages: u32,
    pub extracted_pages: u32,
    pub is_truncated: bool,
    pub conversation_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfImageCacheEntry {
    pub file_hash: String,
    pub file_path: String,
    pub file_name: String,
    pub total_pages: u32,
    pub rendered_pages: u32,
    pub dpi: u32,
    pub images: Vec<PdfRenderedImage>,
    pub conversation_ids: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfRenderedImage {
    pub page_index: usize,
    pub width: u32,
    pub height: u32,
    pub bytes_base64: String,
    pub mime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryEntry {
    id: String,
    #[serde(default)]
    memory_no: Option<u64>,
    #[serde(default, alias = "memoryType")]
    memory_type: String,
    #[serde(default, alias = "content")]
    judgment: String,
    #[serde(default)]
    reasoning: String,
    #[serde(default, alias = "keywords")]
    tags: Vec<String>,
    #[serde(default)]
    owner_agent_id: Option<String>,
    created_at: String,
    updated_at: String,
}

impl MemoryEntry {
    fn display_id(&self) -> String {
        self.memory_no
            .map(|value| value.to_string())
            .unwrap_or_else(|| self.id.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptCommandPreset {
    id: String,
    name: String,
    prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppData {
    version: u32,
    #[serde(default)]
    data_migration_version: u32,
    agents: Vec<AgentProfile>,
    #[serde(default = "default_user_alias")]
    user_alias: String,
    conversations: Vec<Conversation>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            version: APP_DATA_SCHEMA_VERSION,
            data_migration_version: 0,
            agents: {
                let mut agents = vec![
                    default_agent(),
                    default_deputy_agent(),
                ];
                agents.extend(built_in_organization_agents());
                agents.push(default_user_persona());
                agents.push(default_system_persona());
                agents
            },
            user_alias: default_user_alias(),
            conversations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImGroupMemberInfo {
    user_id: String,
    #[serde(default)]
    nickname: String,
    #[serde(default)]
    card: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    raw: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContact {
    id: String,
    channel_id: String,
    platform: RemoteImPlatform,
    remote_contact_type: String,
    remote_contact_id: String,
    #[serde(default)]
    remote_contact_name: String,
    #[serde(default)]
    avatar_url: String,
    #[serde(default)]
    remark_name: String,
    #[serde(default)]
    allow_send: bool,
    #[serde(default)]
    allow_send_files: bool,
    #[serde(default)]
    allow_receive: bool,
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
    #[serde(default = "default_remote_im_contact_route_mode")]
    route_mode: String,
    #[serde(default)]
    bound_agent_id: Option<String>,
    #[serde(default)]
    bound_conversation_id: Option<String>,
    #[serde(default = "default_remote_im_contact_processing_mode")]
    processing_mode: String,
    #[serde(default = "default_remote_im_contact_response_strategy")]
    response_strategy: String,
    #[allow(dead_code)]
    #[serde(default = "default_remote_im_contact_response_guidance", skip_serializing)]
    response_guidance: String,
    #[serde(default = "default_remote_im_contact_blocked_message_prefixes")]
    blocked_message_prefixes: Vec<String>,
    #[serde(default)]
    group_reply_pacing: RemoteImGroupReplyPacing,
    #[serde(default)]
    last_activated_at: Option<String>,
    #[serde(default)]
    last_message_at: Option<String>,
    #[serde(default)]
    dingtalk_session_webhook: Option<String>,
    #[serde(default)]
    dingtalk_session_webhook_expired_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    onebot_group_members: Vec<RemoteImGroupMemberInfo>,
    #[serde(default)]
    shell_workspaces: Vec<ShellWorkspaceConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImGroupReplyPacing {
    #[serde(default = "default_remote_im_assistant_debounce_seconds")]
    assistant_debounce_seconds: u64,
    #[serde(default = "default_remote_im_secretary_inspection_seconds")]
    secretary_inspection_seconds: u64,
    #[serde(default = "default_remote_im_reply_cooldown_seconds")]
    reply_cooldown_seconds: u64,
    #[serde(default = "default_remote_im_inspection_jitter_ratio")]
    inspection_jitter_ratio: f64,
    #[serde(default = "default_remote_im_maximum_energy")]
    maximum_energy: f64,
    #[serde(default = "default_remote_im_base_reply_energy_cost")]
    base_reply_energy_cost: f64,
    #[serde(default = "default_remote_im_energy_cost_per_character")]
    energy_cost_per_character: f64,
    #[serde(default = "default_remote_im_energy_recovery_per_second")]
    energy_recovery_per_second: f64,
    #[serde(default = "default_remote_im_positive_energy_phrases")]
    positive_energy_phrases: Vec<String>,
    #[serde(default = "default_remote_im_negative_energy_phrases")]
    negative_energy_phrases: Vec<String>,
    #[serde(default = "default_remote_im_positive_energy_delta")]
    positive_energy_delta: f64,
    #[serde(default = "default_remote_im_negative_energy_delta")]
    negative_energy_delta: f64,
    #[serde(default = "default_remote_im_normal_reply_max_chars")]
    normal_reply_max_chars: u32,
    #[serde(default = "default_remote_im_focus_reply_max_chars")]
    focus_reply_max_chars: u32,
    #[serde(default = "default_remote_im_focus_instructions")]
    focus_instructions: Vec<String>,
}

impl Default for RemoteImGroupReplyPacing {
    fn default() -> Self {
        Self {
            assistant_debounce_seconds: default_remote_im_assistant_debounce_seconds(),
            secretary_inspection_seconds: default_remote_im_secretary_inspection_seconds(),
            reply_cooldown_seconds: default_remote_im_reply_cooldown_seconds(),
            inspection_jitter_ratio: default_remote_im_inspection_jitter_ratio(),
            maximum_energy: default_remote_im_maximum_energy(),
            base_reply_energy_cost: default_remote_im_base_reply_energy_cost(),
            energy_cost_per_character: default_remote_im_energy_cost_per_character(),
            energy_recovery_per_second: default_remote_im_energy_recovery_per_second(),
            positive_energy_phrases: default_remote_im_positive_energy_phrases(),
            negative_energy_phrases: default_remote_im_negative_energy_phrases(),
            positive_energy_delta: default_remote_im_positive_energy_delta(),
            negative_energy_delta: default_remote_im_negative_energy_delta(),
            normal_reply_max_chars: default_remote_im_normal_reply_max_chars(),
            focus_reply_max_chars: default_remote_im_focus_reply_max_chars(),
            focus_instructions: default_remote_im_focus_instructions(),
        }
    }
}

/// 渠道统一的静态行为参数。
///
/// 联系人仅保留路由、应答策略和运行时账本；这里的值是该渠道全部联系人的
/// 消息过滤、闭嘴、什么时候应该回答、在场和群聊巡检策略的唯一真值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImChannelBehaviorSettings {
    #[serde(default = "default_remote_im_contact_response_guidance")]
    response_guidance: String,
    #[serde(default = "default_remote_im_contact_blocked_message_prefixes")]
    blocked_message_prefixes: Vec<String>,
    #[serde(default = "default_remote_im_contact_mute_keywords")]
    mute_keywords: Vec<String>,
    #[serde(default = "default_remote_im_contact_unmute_keywords")]
    unmute_keywords: Vec<String>,
    #[serde(default = "default_remote_im_contact_patience_seconds")]
    patience_seconds: u64,
    #[serde(default = "default_remote_im_contact_mute_duration_seconds")]
    mute_duration_seconds: u64,
    #[serde(default)]
    group_reply_pacing: RemoteImGroupReplyPacing,
}

impl Default for RemoteImChannelBehaviorSettings {
    fn default() -> Self {
        Self {
            response_guidance: default_remote_im_contact_response_guidance(),
            blocked_message_prefixes: default_remote_im_contact_blocked_message_prefixes(),
            mute_keywords: default_remote_im_contact_mute_keywords(),
            unmute_keywords: default_remote_im_contact_unmute_keywords(),
            patience_seconds: default_remote_im_contact_patience_seconds(),
            mute_duration_seconds: default_remote_im_contact_mute_duration_seconds(),
            group_reply_pacing: RemoteImGroupReplyPacing::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RemoteImGroupReplyDeliveryMarker {
    #[serde(default)]
    generation: u64,
    #[serde(default)]
    boundary_message_id: String,
    #[serde(default)]
    outbound_key: String,
    #[serde(default)]
    final_text: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    platform_message_id: Option<String>,
    #[serde(default)]
    energy_applied: bool,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactCheckpoint {
    contact_id: String,
    #[serde(default)]
    atomic_revision: u64,
    #[serde(default)]
    latest_seen_message_id: Option<String>,
    #[serde(default)]
    last_boundary_message_id: Option<String>,
    #[serde(default)]
    last_boundary_covers_message_id: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    energy: Option<f64>,
    #[serde(default)]
    energy_updated_at: Option<String>,
    #[serde(default)]
    last_success_reply_at: Option<String>,
    #[serde(default)]
    group_reply_delivery: Option<RemoteImGroupReplyDeliveryMarker>,
}

fn default_assistant_agent_id() -> String {
    DEFAULT_AGENT_ID.to_string()
}

fn default_remote_im_contact_activation_mode() -> String {
    "never".to_string()
}

fn default_remote_im_contact_patience_seconds() -> u64 {
    60
}

fn default_remote_im_contact_mute_keywords() -> Vec<String> {
    vec!["闭嘴".to_string()]
}

fn default_remote_im_contact_unmute_keywords() -> Vec<String> {
    vec!["张嘴".to_string()]
}

fn default_remote_im_contact_mute_duration_seconds() -> u64 {
    600
}

fn default_remote_im_contact_route_mode() -> String {
    "main_session".to_string()
}

fn default_remote_im_contact_processing_mode() -> String {
    "continuous".to_string()
}

fn default_remote_im_contact_response_strategy() -> String {
    "smart_judge".to_string()
}

const DEFAULT_REMOTE_IM_GROUP_RESPONSE_GUIDANCE: &str =
    include_str!("../../../../resources/prompts/remote_im_group_response_guidance.md");

fn default_remote_im_contact_response_guidance() -> String {
    DEFAULT_REMOTE_IM_GROUP_RESPONSE_GUIDANCE.trim().to_string()
}

fn default_remote_im_contact_blocked_message_prefixes() -> Vec<String> {
    vec!["#".to_string(), "/".to_string(), "%".to_string()]
}

fn default_remote_im_assistant_debounce_seconds() -> u64 {
    1
}

fn default_remote_im_secretary_inspection_seconds() -> u64 {
    60
}

fn default_remote_im_reply_cooldown_seconds() -> u64 {
    10
}

fn default_remote_im_inspection_jitter_ratio() -> f64 {
    0.2
}

fn default_remote_im_maximum_energy() -> f64 {
    100.0
}

fn default_remote_im_base_reply_energy_cost() -> f64 {
    14.0
}

fn default_remote_im_energy_cost_per_character() -> f64 {
    0.12
}

fn default_remote_im_energy_recovery_per_second() -> f64 {
    0.6
}

fn default_remote_im_positive_energy_phrases() -> Vec<String> {
    vec!["厉害".to_string(), "像人".to_string()]
}

fn default_remote_im_negative_energy_phrases() -> Vec<String> {
    vec!["够了".to_string(), "烦".to_string(), "串了".to_string()]
}

fn default_remote_im_positive_energy_delta() -> f64 {
    6.0
}

fn default_remote_im_negative_energy_delta() -> f64 {
    -15.0
}

fn default_remote_im_normal_reply_max_chars() -> u32 {
    20
}

fn default_remote_im_focus_reply_max_chars() -> u32 {
    200
}

fn default_remote_im_focus_instructions() -> Vec<String> {
    vec![
        "分析".to_string(),
        "总结".to_string(),
        "好好想想".to_string(),
        "为什么".to_string(),
        "到底".to_string(),
    ]
}

fn default_user_alias() -> String {
    "用户".to_string()
}

fn agent_by_id<'a>(agents: &'a [AgentProfile], agent_id: &str) -> Option<&'a AgentProfile> {
    let trimmed = agent_id.trim();
    if trimmed.is_empty() {
        return None;
    }
    agents.iter().find(|item| item.id.trim() == trimmed)
}

fn agent_direct_child_ids(agents: &[AgentProfile], agent: &AgentProfile) -> Vec<String> {
    let valid_ids = agents
        .iter()
        .map(|item| item.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<std::collections::HashSet<_>>();
    normalize_agent_child_ids(&agent.child_agent_ids, &agent.id)
        .into_iter()
        .filter(|id| valid_ids.contains(id))
        .collect::<Vec<_>>()
}

fn agent_direct_child_agents<'a>(
    agents: &'a [AgentProfile],
    agent: &AgentProfile,
) -> Vec<&'a AgentProfile> {
    agent_direct_child_ids(agents, agent)
        .into_iter()
        .filter_map(|id| agent_by_id(agents, &id))
        .collect::<Vec<_>>()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentPermissionCategory {
    BuiltinTool,
    Skill,
    McpTool,
}

fn builtin_tool_is_fixed_system(tool_id: &str) -> bool {
    builtin_tool_is_fixed_system_from_policy(tool_id)
}

fn builtin_tool_is_local_conversation_fixed(tool_id: &str) -> bool {
    builtin_tool_is_local_conversation_fixed_from_policy(tool_id)
}

fn builtin_tool_is_contact_only_hidden(tool_id: &str) -> bool {
    builtin_tool_is_contact_only_hidden_from_policy(tool_id)
}

fn builtin_tool_is_permission_controlled(tool_id: &str) -> bool {
    builtin_tool_is_permission_controlled_from_policy(tool_id)
}

fn builtin_tool_visible_in_permission_lists(tool_id: &str) -> bool {
    builtin_tool_visible_in_permission_lists_from_policy(tool_id)
}

fn normalize_permission_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "whitelist" => "whitelist".to_string(),
        _ => "blacklist".to_string(),
    }
}

fn permission_control_candidates<'a>(
    control: Option<&'a AgentPermissionControl>,
    category: AgentPermissionCategory,
) -> Option<(&'a AgentPermissionControl, &'a [String])> {
    let control = control?;
    if !control.enabled {
        return None;
    }
    let list = match category {
        AgentPermissionCategory::BuiltinTool => &control.builtin_tool_names,
        AgentPermissionCategory::Skill => &control.skill_names,
        AgentPermissionCategory::McpTool => &control.mcp_tool_names,
    };
    Some((control, list.as_slice()))
}

fn agent_permission_candidates<'a>(
    agent: Option<&'a AgentProfile>,
    category: AgentPermissionCategory,
) -> Option<(&'a AgentPermissionControl, &'a [String])> {
    permission_control_candidates(agent.map(|item| &item.permission_control), category)
}

fn permission_control_allows_any_name(
    control: Option<(&AgentPermissionControl, &[String])>,
    candidate_names: &[&str],
) -> bool {
    let Some((control, list)) = control else {
        return true;
    };
    let matches = candidate_names.iter().any(|candidate| {
        let candidate = candidate.trim();
        !candidate.is_empty() && list.iter().any(|item| item == candidate)
    });
    if normalize_permission_mode(&control.mode) == "whitelist" {
        matches
    } else {
        !matches
    }
}

fn agent_permission_allows_any_name(
    agent: Option<&AgentProfile>,
    category: AgentPermissionCategory,
    candidate_names: &[&str],
) -> bool {
    permission_control_allows_any_name(
        agent_permission_candidates(agent, category),
        candidate_names,
    )
}

fn permission_mode_label(mode: &str) -> &'static str {
    if normalize_permission_mode(mode) == "whitelist" {
        "白名单"
    } else {
        "黑名单"
    }
}

fn agent_permission_restricted_reason(
    agent: Option<&AgentProfile>,
    category: AgentPermissionCategory,
    item_name: &str,
) -> Option<String> {
    let Some((control, _)) = agent_permission_candidates(agent, category) else {
        return None;
    };
    if agent_permission_allows_any_name(agent, category, &[item_name]) {
        return None;
    }
    let category_label = match category {
        AgentPermissionCategory::BuiltinTool => "工具",
        AgentPermissionCategory::Skill => "Skill",
        AgentPermissionCategory::McpTool => "MCP 工具",
    };
    Some(format!(
        "因为当前人格权限采用{}机制，{} `{}` 未被允许",
        permission_mode_label(&control.mode),
        category_label,
        item_name.trim()
    ))
}

fn tool_restricted_by_agent_permission(agent: &AgentProfile, tool_id: &str) -> Option<String> {
    if !builtin_tool_is_permission_controlled(tool_id) {
        return None;
    }
    agent_permission_restricted_reason(
        Some(agent),
        AgentPermissionCategory::BuiltinTool,
        tool_id,
    )
}

fn agent_delegate_unavailable_reason(agent: Option<&AgentProfile>) -> Option<String> {
    let Some(agent) = agent else {
        return Some("缺少当前执行人格，无法使用委托".to_string());
    };
    if agent
        .child_agent_ids
        .iter()
        .any(|id| !id.trim().is_empty())
    {
        return None;
    }
    Some("当前人格没有直接下级，无法使用委托".to_string())
}

fn agent_builtin_tool_unavailable_reason(
    agent: Option<&AgentProfile>,
    tool_id: &str,
) -> Option<String> {
    if tool_id.trim() == "delegate" {
        if let Some(reason) = agent_delegate_unavailable_reason(agent) {
            return Some(reason);
        }
    }
    match agent {
        Some(agent) => tool_restricted_by_agent_permission(agent, tool_id),
        None => None,
    }
}

fn user_persona_name(data: &AppData) -> String {
    data.agents
        .iter()
        .find(|a| a.id == USER_PERSONA_ID || a.is_built_in_user)
        .map(|a| a.name.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(default_user_alias)
}

#[cfg(test)]
mod types_storage_tests {
    use super::*;

    #[test]
    fn remote_im_group_reply_pacing_should_use_demonstrative_phrase_defaults() {
        let defaults = RemoteImGroupReplyPacing::default();
        assert_eq!(defaults.secretary_inspection_seconds, 60);
        assert_eq!(defaults.positive_energy_phrases, vec!["厉害", "像人"]);
        assert_eq!(defaults.negative_energy_phrases, vec!["够了", "烦", "串了"]);
        assert_eq!(
            defaults.focus_instructions,
            vec!["分析", "总结", "好好想想", "为什么", "到底"]
        );

        let legacy: RemoteImGroupReplyPacing = serde_json::from_value(serde_json::json!({}))
            .expect("missing phrase fields should use the same defaults");
        assert_eq!(legacy.positive_energy_phrases, defaults.positive_energy_phrases);
        assert_eq!(legacy.negative_energy_phrases, defaults.negative_energy_phrases);
        assert_eq!(legacy.focus_instructions, defaults.focus_instructions);
    }

    fn build_agent_with_permission_control(
        mode: &str,
        builtin_tool_names: Vec<&str>,
        skill_names: Vec<&str>,
        mcp_tool_names: Vec<&str>,
    ) -> AgentProfile {
        let mut agent = default_agent();
        agent.permission_control = AgentPermissionControl {
            enabled: true,
            mode: mode.to_string(),
            builtin_tool_names: builtin_tool_names.into_iter().map(|value| value.to_string()).collect(),
            skill_names: skill_names.into_iter().map(|value| value.to_string()).collect(),
            mcp_tool_names: mcp_tool_names.into_iter().map(|value| value.to_string()).collect(),
        };
        agent
    }

    #[test]
    fn agent_permission_allows_any_name_should_handle_whitelist_and_blacklist() {
        let whitelist = build_agent_with_permission_control(
            "whitelist",
            vec!["fetch"],
            vec!["assistant-space-guide"],
            vec!["server-a::search"],
        );
        assert!(agent_permission_allows_any_name(
            Some(&whitelist),
            AgentPermissionCategory::BuiltinTool,
            &["fetch"],
        ));
        assert!(!agent_permission_allows_any_name(
            Some(&whitelist),
            AgentPermissionCategory::BuiltinTool,
            &["websearch"],
        ));
        assert!(!agent_permission_allows_any_name(
            Some(&whitelist),
            AgentPermissionCategory::Skill,
            &["mcp-setup"],
        ));
        assert!(agent_permission_allows_any_name(
            Some(&whitelist),
            AgentPermissionCategory::McpTool,
            &["server-a::search", "server-id::search", "search"],
        ));
        assert!(!agent_permission_allows_any_name(
            Some(&whitelist),
            AgentPermissionCategory::McpTool,
            &["server-b::other", "other"],
        ));

        let blacklist = build_agent_with_permission_control(
            "blacklist",
            vec!["fetch"],
            vec!["assistant-space-guide"],
            vec!["server-a::search"],
        );
        assert!(!agent_permission_allows_any_name(
            Some(&blacklist),
            AgentPermissionCategory::BuiltinTool,
            &["fetch"],
        ));
        assert!(agent_permission_allows_any_name(
            Some(&blacklist),
            AgentPermissionCategory::BuiltinTool,
            &["websearch"],
        ));
        assert!(!agent_permission_allows_any_name(
            Some(&blacklist),
            AgentPermissionCategory::McpTool,
            &["server-a::search", "search"],
        ));
        assert!(agent_permission_allows_any_name(
            Some(&blacklist),
            AgentPermissionCategory::McpTool,
            &["server-b::other", "other"],
        ));
    }

    #[test]
    fn operate_should_be_controlled_by_permission_card_for_regular_agents() {
        let regular_whitelisted = build_agent_with_permission_control(
            "whitelist",
            vec!["fetch", "operate"],
            vec![],
            vec![],
        );

        let regular_blocklisted = build_agent_with_permission_control(
            "blacklist",
            vec!["operate"],
            vec![],
            vec![],
        );

        let regular_whitelist_without_operate = build_agent_with_permission_control(
            "whitelist",
            vec!["fetch"],
            vec![],
            vec![],
        );

        let mut regular_control_disabled = build_agent_with_permission_control(
            "whitelist",
            vec![],
            vec![],
            vec![],
        );
        regular_control_disabled.permission_control.enabled = false;

        // 白名单显式授权 operate → 允许
        assert_eq!(
            tool_restricted_by_agent_permission(&regular_whitelisted, "operate"),
            None
        );
        // 黑名单显式拒绝 operate → 拒绝
        assert!(tool_restricted_by_agent_permission(&regular_blocklisted, "operate").is_some());
        // 白名单未授权 operate → 拒绝
        assert!(
            tool_restricted_by_agent_permission(&regular_whitelist_without_operate, "operate")
                .is_some()
        );
        // 权限卡未启用 → 默认放行（普通工具语义）
        assert_eq!(
            tool_restricted_by_agent_permission(&regular_control_disabled, "operate"),
            None
        );
    }

    #[test]
    fn delegate_should_require_direct_child_agents() {
        assert_eq!(
            agent_builtin_tool_unavailable_reason(None, "delegate"),
            Some("缺少当前执行人格，无法使用委托".to_string())
        );

        let mut parent = default_agent();
        parent.id = "agent-parent".to_string();
        parent.name = "父人格".to_string();
        parent.child_agent_ids = Vec::new();
        assert_eq!(
            agent_builtin_tool_unavailable_reason(Some(&parent), "delegate"),
            Some("当前人格没有直接下级，无法使用委托".to_string())
        );

        parent.child_agent_ids = vec!["dept-child".to_string()];
        assert_eq!(agent_builtin_tool_unavailable_reason(Some(&parent), "delegate"), None);
    }
}
