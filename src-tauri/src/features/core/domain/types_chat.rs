const MEMORY_RECALL_MODE_AUTO: &str = "auto";
const MEMORY_RECALL_MODE_MANUAL: &str = "manual";
const MEMORY_RECALL_MODE_OFF: &str = "off";

fn default_agent_memory_recall_mode() -> String {
    MEMORY_RECALL_MODE_AUTO.to_string()
}

fn normalize_agent_memory_recall_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        MEMORY_RECALL_MODE_MANUAL => MEMORY_RECALL_MODE_MANUAL.to_string(),
        MEMORY_RECALL_MODE_OFF => MEMORY_RECALL_MODE_OFF.to_string(),
        _ => MEMORY_RECALL_MODE_AUTO.to_string(),
    }
}

fn agent_memory_recall_mode(agent: &AgentProfile) -> String {
    normalize_agent_memory_recall_mode(&agent.memory_recall_mode)
}

fn agent_memory_rag_enabled(agent: &AgentProfile) -> bool {
    agent_memory_recall_mode(agent) == MEMORY_RECALL_MODE_AUTO
}

fn agent_memory_recall_enabled(agent: &AgentProfile) -> bool {
    agent_memory_recall_mode(agent) != MEMORY_RECALL_MODE_OFF
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentProfile {
    id: String,
    name: String,
    system_prompt: String,
    #[serde(default = "default_agent_tools")]
    tools: Vec<ApiToolConfig>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    avatar_path: Option<String>,
    #[serde(default)]
    avatar_updated_at: Option<String>,
    #[serde(default)]
    is_built_in_user: bool,
    #[serde(default)]
    is_built_in_system: bool,
    #[serde(default)]
    private_memory_enabled: bool,
    #[serde(default = "default_agent_memory_recall_mode")]
    memory_recall_mode: String,
    #[serde(default = "default_main_source")]
    source: String,
    #[serde(default = "default_global_scope")]
    scope: String,
    /// 人格简介：一句话描述这个人格擅长什么。
    /// 用于人格列表展示，并接替原部门 `summary` 在委托清单里对目标人格的说明位。
    /// 必须独立于 skill：人格可能一个 skill 都没有，且一个人格可常驻多个 skill、各自的 description 只描述自己。
    #[serde(default)]
    summary: String,
    /// 常驻 skill：这些 skill 的说明会**全文注入**该人格的系统提示词，模型无需读文件。
    #[serde(default)]
    resident_skill_names: Vec<String>,
    /// 可选 skill：只把这些 skill 的**引用**（名字）注入系统提示词，模型需要时自行读取。
    #[serde(default)]
    optional_skill_names: Vec<String>,
    /// 人格驱动模型（会话基线，可被会话级首选覆盖）。
    #[serde(default)]
    api_config_ids: Vec<String>,
    #[serde(default)]
    api_config_id: String,
    #[serde(default)]
    model_failure_fallback_enabled: bool,
    /// 人格权限：强制约束该人格所有会话的工具/技能/MCP 可见性。
    #[serde(default)]
    permission_control: AgentPermissionControl,
    /// 下级人格 id：组织树的父子边，父子边语义为「可直接委托」。
    #[serde(default)]
    child_agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveAgentsInput {
    agents: Vec<AgentProfile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
enum MessagePart {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<String>,
    },
    Image {
        mime: String,
        #[serde(alias = "bytesBase64")]
        bytes_base64: String,
        name: Option<String>,
        #[serde(default)]
        compressed: bool,
    },
    Audio {
        mime: String,
        #[serde(alias = "bytesBase64")]
        bytes_base64: String,
        name: Option<String>,
        #[serde(default)]
        compressed: bool,
    },
    Attachment {
        path: String,
        mime: String,
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemeAnnotation {
    meme: String,
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    id: String,
    role: String,
    created_at: String,
    #[serde(default)]
    speaker_agent_id: Option<String>,
    parts: Vec<MessagePart>,
    #[serde(default)]
    extra_text_blocks: Vec<String>,
    provider_meta: Option<Value>,
    tool_call: Option<Vec<Value>>,
    mcp_call: Option<Vec<Value>>,
    // 正式 meme 替换语义：把正文中的 token 映射为 markdown 图片引用。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    meme_annotations: Option<Vec<MemeAnnotation>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImMessageSource {
    channel_id: String,
    platform: RemoteImPlatform,
    im_name: String,
    remote_contact_type: String,
    remote_contact_id: String,
    remote_contact_name: String,
    sender_id: String,
    sender_name: String,
    #[serde(default)]
    sender_avatar_url: Option<String>,
    #[serde(default)]
    platform_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RemoteImActivationSource {
    channel_id: String,
    platform: RemoteImPlatform,
    remote_contact_type: String,
    remote_contact_id: String,
    remote_contact_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationTodoItem {
    content: String,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FastRequestTurn {
    #[serde(default)]
    id: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    request_text: String,
    #[serde(default)]
    response_text: String,
    #[serde(default)]
    success: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    model_name: Option<String>,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    created_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationUsageBucket {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    cache_read_tokens: u64,
    #[serde(default)]
    cache_write_tokens: u64,
    #[serde(default)]
    reasoning_tokens: u64,
}

impl ConversationUsageBucket {
    fn needs_legacy_total_tokens_backfill(&self) -> bool {
        self.total_tokens < self.input_tokens.saturating_add(self.output_tokens)
    }

    fn normalized_legacy_totals(mut self) -> Self {
        let floor_total_tokens = self.input_tokens.saturating_add(self.output_tokens);
        self.total_tokens = self.total_tokens.max(floor_total_tokens);
        self
    }

    fn is_empty(&self) -> bool {
        self.input_tokens == 0
            && self.output_tokens == 0
            && self.total_tokens == 0
            && self.cache_read_tokens == 0
            && self.cache_write_tokens == 0
            && self.reasoning_tokens == 0
    }

    fn keep_at_least(&mut self, other: &ConversationUsageBucket) {
        self.input_tokens = self.input_tokens.max(other.input_tokens);
        self.output_tokens = self.output_tokens.max(other.output_tokens);
        self.total_tokens = self.total_tokens.max(other.total_tokens);
        self.cache_read_tokens = self.cache_read_tokens.max(other.cache_read_tokens);
        self.cache_write_tokens = self.cache_write_tokens.max(other.cache_write_tokens);
        self.reasoning_tokens = self.reasoning_tokens.max(other.reasoning_tokens);
    }

    fn saturating_add_assign(&mut self, other: &ConversationUsageBucket) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.total_tokens = self.total_tokens.saturating_add(other.total_tokens);
        self.cache_read_tokens = self.cache_read_tokens.saturating_add(other.cache_read_tokens);
        self.cache_write_tokens = self.cache_write_tokens.saturating_add(other.cache_write_tokens);
        self.reasoning_tokens = self.reasoning_tokens.saturating_add(other.reasoning_tokens);
    }

    fn saturating_sub_from_totals(
        total_input_tokens: u64,
        total_output_tokens: u64,
        total_total_tokens: u64,
        total_cache_read_tokens: u64,
        total_cache_write_tokens: u64,
        total_reasoning_tokens: u64,
        used: &ConversationUsageBucket,
    ) -> ConversationUsageBucket {
        ConversationUsageBucket {
            input_tokens: total_input_tokens.saturating_sub(used.input_tokens),
            output_tokens: total_output_tokens.saturating_sub(used.output_tokens),
            total_tokens: total_total_tokens.saturating_sub(used.total_tokens),
            cache_read_tokens: total_cache_read_tokens.saturating_sub(used.cache_read_tokens),
            cache_write_tokens: total_cache_write_tokens.saturating_sub(used.cache_write_tokens),
            reasoning_tokens: total_reasoning_tokens.saturating_sub(used.reasoning_tokens),
        }
    }
}

fn conversation_provider_model_usage_is_empty(
    value: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, ConversationUsageBucket>>,
) -> bool {
    value.is_empty()
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationCumulativeUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    total_tokens: u64,
    #[serde(default)]
    cache_read_tokens: u64,
    #[serde(default)]
    cache_write_tokens: u64,
    #[serde(default)]
    reasoning_tokens: u64,
    #[serde(default, skip_serializing_if = "conversation_provider_model_usage_is_empty")]
    by_provider_model:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, ConversationUsageBucket>>,
}

impl ConversationCumulativeUsage {
    fn needs_legacy_total_tokens_backfill(&self) -> bool {
        self.total_tokens < self.input_tokens.saturating_add(self.output_tokens)
            || self
                .by_provider_model
                .values()
                .any(|models| models.values().any(ConversationUsageBucket::needs_legacy_total_tokens_backfill))
    }

    fn normalized_legacy_totals(mut self) -> Self {
        let floor_total_tokens = self.input_tokens.saturating_add(self.output_tokens);
        self.total_tokens = self.total_tokens.max(floor_total_tokens);
        self.by_provider_model = self
            .by_provider_model
            .into_iter()
            .map(|(provider_key, models)| {
                let normalized_models = models
                    .into_iter()
                    .map(|(model_name, bucket)| {
                        (model_name, bucket.normalized_legacy_totals())
                    })
                    .collect();
                (provider_key, normalized_models)
            })
            .collect();
        self
    }

    fn is_empty(&self) -> bool {
        self.input_tokens == 0
            && self.output_tokens == 0
            && self.total_tokens == 0
            && self.cache_read_tokens == 0
            && self.cache_write_tokens == 0
            && self.reasoning_tokens == 0
            && self.by_provider_model.is_empty()
    }

    fn keep_at_least(&mut self, other: &ConversationCumulativeUsage) {
        self.input_tokens = self.input_tokens.max(other.input_tokens);
        self.output_tokens = self.output_tokens.max(other.output_tokens);
        self.total_tokens = self.total_tokens.max(other.total_tokens);
        self.cache_read_tokens = self.cache_read_tokens.max(other.cache_read_tokens);
        self.cache_write_tokens = self.cache_write_tokens.max(other.cache_write_tokens);
        self.reasoning_tokens = self.reasoning_tokens.max(other.reasoning_tokens);
        for (provider_key, other_models) in &other.by_provider_model {
            let target_models = self
                .by_provider_model
                .entry(provider_key.clone())
                .or_default();
            for (model_name, other_bucket) in other_models {
                target_models
                    .entry(model_name.clone())
                    .or_default()
                    .keep_at_least(other_bucket);
            }
        }
    }

    fn detailed_usage_sum(&self) -> ConversationUsageBucket {
        let mut sum = ConversationUsageBucket::default();
        for models in self.by_provider_model.values() {
            for bucket in models.values() {
                sum.saturating_add_assign(bucket);
            }
        }
        sum
    }

    fn legacy_remainder(&self) -> ConversationUsageBucket {
        ConversationUsageBucket::saturating_sub_from_totals(
            self.input_tokens,
            self.output_tokens,
            self.total_tokens,
            self.cache_read_tokens,
            self.cache_write_tokens,
            self.reasoning_tokens,
            &self.detailed_usage_sum(),
        )
    }
}

fn conversation_cumulative_usage_add_provider_usage(
    target: &mut ConversationCumulativeUsage,
    provider_key: Option<&str>,
    model_name: Option<&str>,
    usage: &Value,
) -> bool {
    fn read_u64(usage: &Value, keys: &[&str]) -> u64 {
        keys.iter()
            .find_map(|key| {
                let value = usage.get(*key)?;
                value
                    .as_u64()
                    .or_else(|| value.as_i64().and_then(|item| u64::try_from(item).ok()))
            })
            .unwrap_or(0)
    }

    let input_tokens = read_u64(usage, &["promptTokens", "prompt_tokens"]);
    let output_tokens = read_u64(usage, &["completionTokens", "completion_tokens"]);
    let total_tokens = read_u64(usage, &["totalTokens", "total_tokens"])
        .max(input_tokens.saturating_add(output_tokens));
    let delta = ConversationUsageBucket {
        input_tokens,
        output_tokens,
        total_tokens,
        cache_read_tokens: read_u64(usage, &["cachedTokens", "cached_tokens"]),
        cache_write_tokens: read_u64(
            usage,
            &["cacheCreationTokens", "cache_creation_tokens"],
        )
        .saturating_add(read_u64(
            usage,
            &["cacheCreation5mTokens", "cache_creation_5m_tokens"],
        ))
        .saturating_add(read_u64(
            usage,
            &["cacheCreation1hTokens", "cache_creation_1h_tokens"],
        )),
        reasoning_tokens: read_u64(usage, &["reasoningTokens", "reasoning_tokens"]),
    };
    if delta.is_empty() {
        return false;
    }
    target.input_tokens = target.input_tokens.saturating_add(delta.input_tokens);
    target.output_tokens = target.output_tokens.saturating_add(delta.output_tokens);
    target.total_tokens = target.total_tokens.saturating_add(delta.total_tokens);
    target.cache_read_tokens = target
        .cache_read_tokens
        .saturating_add(delta.cache_read_tokens);
    target.cache_write_tokens = target
        .cache_write_tokens
        .saturating_add(delta.cache_write_tokens);
    target.reasoning_tokens = target
        .reasoning_tokens
        .saturating_add(delta.reasoning_tokens);
    let normalized_provider_key = provider_key
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_model_name = model_name
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let (Some(provider_key), Some(model_name)) = (normalized_provider_key, normalized_model_name)
    {
        let models = target
            .by_provider_model
            .entry(provider_key.to_string())
            .or_default();
        models
            .entry(model_name.to_string())
            .or_default()
            .saturating_add_assign(&delta);
    }
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationGoalState {
    goal_id: String,
    status: String,
    objective: String,
    started_at: String,
    #[serde(default)]
    ended_at: Option<String>,
    #[serde(default)]
    usage_start: ConversationCumulativeUsage,
    #[serde(default)]
    usage_end: Option<ConversationCumulativeUsage>,
}

fn conversation_goal_is_active(goal: &ConversationGoalState) -> bool {
    goal.status.trim() == "active"
}

fn conversation_cumulative_usage_weighted_tokens(
    cumulative_usage: &ConversationCumulativeUsage,
) -> u64 {
    let weighted = (cumulative_usage.output_tokens as f64 * 2.0)
        + (cumulative_usage.cache_read_tokens as f64 * 0.02)
        + cumulative_usage.cache_write_tokens as f64;
    if !weighted.is_finite() || weighted <= 0.0 {
        0
    } else if weighted >= u64::MAX as f64 {
        u64::MAX
    } else {
        weighted.round() as u64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Conversation {
    id: String,
    title: String,
    agent_id: String,
    #[serde(default)]
    bound_conversation_id: Option<String>,
    #[serde(default)]
    parent_conversation_id: Option<String>,
    #[serde(default)]
    child_conversation_ids: Vec<String>,
    #[serde(default)]
    fork_message_cursor: Option<String>,
    #[serde(default)]
    unread_count: usize,
    #[serde(default)]
    conversation_kind: String,
    #[serde(default)]
    root_conversation_id: Option<String>,
    #[serde(default)]
    delegate_id: Option<String>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    last_user_at: Option<String>,
    last_assistant_at: Option<String>,
    status: String,
    #[serde(default)]
    user_profile_snapshot: String,
    #[serde(default)]
    shell_workspace_path: Option<String>,
    #[serde(default)]
    shell_workspaces: Vec<ShellWorkspaceConfig>,
    #[serde(default)]
    shell_autonomous_mode: bool,
    #[serde(default = "default_shell_work_mode")]
    shell_work_mode: String,
    #[serde(default)]
    shell_work_branch: String,
    #[serde(default)]
    archived_at: Option<String>,
    messages: Vec<ChatMessage>,
    #[serde(default)]
    fast_request_turns: Vec<FastRequestTurn>,
    #[serde(default)]
    current_todos: Vec<ConversationTodoItem>,
    #[serde(default)]
    memory_recall_table: Vec<String>,
    #[serde(default)]
    plan_mode_enabled: bool,
    #[serde(default)]
    preferred_api_config_id: Option<String>,
    #[serde(default)]
    is_draft: bool,
    #[serde(default)]
    auto_push_remote_contact_id: Option<String>,
    #[serde(default, alias = "usageSummary")]
    cumulative_usage: ConversationCumulativeUsage,
    #[serde(default)]
    active_goal: Option<ConversationGoalState>,
    #[serde(default)]
    last_error: Option<String>,
}

#[derive(Debug, Clone)]
struct RemoteImConversationAssistantContext {
    agent_id: String,
    agent_name: String,
}

#[derive(Debug, Clone)]
struct ConversationRuntimeSlot {
    state: MainSessionState,
    pending_queue: std::collections::VecDeque<ChatPendingEvent>,
    stream_cache: ConversationStreamRuntimeCache,
    active_remote_im_activation_sources: Vec<RemoteImActivationSource>,
    active_remote_im_assistant_context: Option<RemoteImConversationAssistantContext>,
    plan_mode_enabled: bool,
    last_activity_at: String,
}

#[derive(Debug, Clone, Default)]
struct ConversationStreamRuntimeCache {
    activation_id: String,
    request_id: String,
    agent_id: String,
    assistant_text: String,
    activity_reasoning_text: String,
    tool_status_text: String,
    tool_status_state: String,
    stream_blocks: Vec<AssistantStreamBlock>,
    started_at: String,
    started_at_ms: u64,
    updated_at: String,
    persisted_assistant_message_id: String,
    // 上下文用量随流式缓存下发：工具执行期间落盘用量后写入，
    // 前端随每个 delta 事件的 stream_cache 拿到最新准确占用率，
    // 切屏恢复时也直接来自缓存，无需旁路广播。
    context_usage_ratio: f64,
    context_usage_percent: u32,
    effective_prompt_tokens: u64,
    context_window_tokens: u32,
    // 调度状态唯一信源：7 步闭环枚举（preparing_context -> waiting_response -> streaming_reasoning/text/tool -> executing_tool -> idle），随快照原样推送，前端只读。
    #[allow(dead_code)]
    scheduling_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantStreamToolBlock {
    tool_call_id: String,
    name: String,
    args_text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    result_text: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantStreamBlock {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    reasoning: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AssistantStreamToolBlock>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pending_text_break: bool,
}

impl Default for ConversationRuntimeSlot {
    fn default() -> Self {
        Self {
            state: MainSessionState::Idle,
            pending_queue: std::collections::VecDeque::new(),
            stream_cache: ConversationStreamRuntimeCache::default(),
            active_remote_im_activation_sources: Vec::new(),
            active_remote_im_assistant_context: None,
            plan_mode_enabled: false,
            last_activity_at: String::new(),
        }
    }
}

#[cfg(test)]
mod conversation_usage_tests {
    use super::*;

    #[test]
    fn conversation_cumulative_usage_should_record_provider_model_breakdown() {
        let mut usage = ConversationCumulativeUsage::default();
        let payload = serde_json::json!({
            "promptTokens": 120,
            "completionTokens": 45,
            "cachedTokens": 20,
            "cacheCreationTokens": 7
        });

        let changed = conversation_cumulative_usage_add_provider_usage(
            &mut usage,
            Some("openai"),
            Some("gpt-5"),
            &payload,
        );

        assert!(changed);
        assert_eq!(usage.input_tokens, 120);
        assert_eq!(usage.output_tokens, 45);
        assert_eq!(usage.cache_read_tokens, 20);
        assert_eq!(usage.cache_write_tokens, 7);
        assert_eq!(
            usage
                .by_provider_model
                .get("openai")
                .and_then(|models| models.get("gpt-5"))
                .cloned(),
            Some(ConversationUsageBucket {
                input_tokens: 120,
                output_tokens: 45,
                total_tokens: 165,
                cache_read_tokens: 20,
                cache_write_tokens: 7,
                reasoning_tokens: 0,
            })
        );
    }

    #[test]
    fn conversation_cumulative_usage_legacy_remainder_should_subtract_breakdown_sum() {
        let mut usage = ConversationCumulativeUsage {
            input_tokens: 200,
            output_tokens: 100,
            cache_read_tokens: 50,
            cache_write_tokens: 25,
            ..ConversationCumulativeUsage::default()
        };
        usage.by_provider_model.insert(
            "openai".to_string(),
            std::collections::BTreeMap::from([(
                "gpt-5".to_string(),
                ConversationUsageBucket {
                    input_tokens: 120,
                    output_tokens: 60,
                    total_tokens: 180,
                    cache_read_tokens: 20,
                    cache_write_tokens: 5,
                    reasoning_tokens: 0,
                },
            )]),
        );

        let remainder = usage.legacy_remainder();

        assert_eq!(remainder.input_tokens, 80);
        assert_eq!(remainder.output_tokens, 40);
        assert_eq!(remainder.cache_read_tokens, 30);
        assert_eq!(remainder.cache_write_tokens, 20);
    }
}

#[derive(Debug, Clone)]
struct DelegateRuntimeThread {
    delegate_id: String,
    root_conversation_id: String,
    target_agent_id: String,
    title: String,
    call_stack: Vec<String>,
    parent_chat_session_key: Option<String>,
    archived_at: Option<String>,
    conversation: Conversation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationArchive {
    archive_id: String,
    archived_at: String,
    reason: String,
    source_conversation: Conversation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveSummary {
    archive_id: String,
    archived_at: String,
    title: String,
    message_count: usize,
    api_config_id: String,
    agent_id: String,
}
