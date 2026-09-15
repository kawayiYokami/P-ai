#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeContextReferenceInput {
    id: String,
    file_path: String,
    #[serde(default)]
    start_line: Option<u32>,
    #[serde(default)]
    end_line: Option<u32>,
    #[serde(default)]
    content: String,
    #[serde(default)]
    language_id: Option<String>,
    source: String,
    captured_at: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpsertIdeContextSnapshotInput {
    client_id: String,
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    editor: String,
    #[serde(default)]
    workspace_roots: Vec<String>,
    #[serde(default)]
    references: Vec<IdeContextReferenceInput>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeContextWorkspaceQueryInput {
    #[serde(default)]
    workspaces: Vec<IdeContextWorkspaceInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeContextWorkspaceInput {
    path: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdeContextReferenceItemOutput {
    id: String,
    workspace_path: String,
    workspace_name: String,
    file_path: String,
    file_name: String,
    relative_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    start_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
    display_label: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    language_id: Option<String>,
    source: String,
    captured_at: String,
    text_block: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdeContextWorkspaceGroupOutput {
    workspace_path: String,
    workspace_name: String,
    references: Vec<IdeContextReferenceItemOutput>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdeContextQueryResultOutput {
    groups: Vec<IdeContextWorkspaceGroupOutput>,
    updated_at: String,
}

const IDE_CONTEXT_BRIDGE_HOST: &str = "127.0.0.1";
const IDE_CONTEXT_BRIDGE_BIND_HOST: &str = "0.0.0.0";
#[cfg(test)]
const IDE_CONTEXT_BRIDGE_BASE_PORT: u16 = 8429;
const IDE_CONTEXT_BRIDGE_PATH: &str = "/ide-context";
const IDE_CONTEXT_CHAT_BRIDGE_PATH: &str = "/chat";
const IDE_CONTEXT_BRIDGE_DISCOVERY_FILE: &str = "p-ai-ide-context-bridge.json";
const IDE_CONTEXT_SNAPSHOT_TTL_SECS: i64 = 30;
const IDE_CONTEXT_AUTH_TOKEN_TTL_SECS: i64 = 90 * 24 * 60 * 60;
const IDE_CONTEXT_MAX_AUTH_TOKENS: usize = 128;
const WEB_ACCESS_SERVICE_ID: &str = "web_access";
static IDE_CONTEXT_BRIDGE_STARTED: AtomicBool = AtomicBool::new(false);
static IDE_CONTEXT_BRIDGE_SHUTDOWN: OnceLock<
    Arc<Mutex<Option<tokio_util::sync::CancellationToken>>>,
> = OnceLock::new();
static IDE_CONTEXT_BRIDGE_SERVER_TASK: OnceLock<
    Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
> = OnceLock::new();
static IDE_CONTEXT_PORT_SERVICE_CORE: OnceLock<Arc<LocalPortServiceCore>> = OnceLock::new();
static IDE_CONTEXT_CHAT_CLIENTS: OnceLock<
    Arc<Mutex<std::collections::HashMap<String, tokio::sync::mpsc::UnboundedSender<serde_json::Value>>>>,
> = OnceLock::new();
static IDE_CONTEXT_CHAT_CLIENT_CONVERSATIONS: OnceLock<
    Arc<Mutex<std::collections::HashMap<String, String>>>,
> = OnceLock::new();
static WEB_ACCESS_CONNECTIONS: OnceLock<
    Arc<Mutex<std::collections::HashMap<String, WebAccessConnectionEntry>>>,
> = OnceLock::new();

#[derive(Debug, Clone)]
struct IdeContextRuntime {
    snapshots: Arc<Mutex<std::collections::HashMap<String, IdeContextSnapshot>>>,
    bridge_auth: Arc<Mutex<IdeContextBridgeAuthRuntime>>,
    current_port: Arc<Mutex<Option<u16>>>,
    web_access_cache: Arc<Mutex<IdeContextWebAccessCache>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebAccessConnectionSummary {
    id: String,
    path: String,
    peer_addr: String,
    local: bool,
    authenticated: bool,
    connected_at: String,
    client_id: String,
}

#[derive(Debug, Clone)]
struct WebAccessConnectionEntry {
    id: String,
    path: String,
    peer_addr: String,
    local: bool,
    authenticated: bool,
    connected_at: String,
    client_id: String,
}

#[derive(Debug, Default)]
struct IdeContextBridgeAuthRuntime {
    valid_tokens: std::collections::HashMap<String, OffsetDateTime>,
    remote_password: String,
}

#[derive(Debug, Default)]
struct IdeContextWebAccessCache {
    lan_hosts: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct GetWebAccessInfoInput {
    #[serde(default)]
    force_refresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct IdeContextPersistedBridgeToken {
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    token: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "String::is_empty")]
    expires_at: String,
    #[serde(default)]
    tokens: Vec<IdeContextPersistedBridgeTokenEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeContextPersistedBridgeTokenEntry {
    token: String,
    expires_at: String,
}

impl IdeContextPersistedBridgeToken {
    fn normalized_entries(self) -> Vec<IdeContextPersistedBridgeTokenEntry> {
        if !self.tokens.is_empty() {
            return self.tokens;
        }
        if self.token.trim().is_empty() || self.expires_at.trim().is_empty() {
            return Vec::new();
        }
        vec![IdeContextPersistedBridgeTokenEntry {
            token: self.token,
            expires_at: self.expires_at,
        }]
    }
}

impl IdeContextRuntime {
    fn new() -> Self {
        let mut bridge_auth = IdeContextBridgeAuthRuntime::default();
        bridge_auth.remote_password = ide_context_generate_remote_password();
        Self {
            snapshots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            bridge_auth: Arc::new(Mutex::new(bridge_auth)),
            current_port: Arc::new(Mutex::new(None)),
            web_access_cache: Arc::new(Mutex::new(IdeContextWebAccessCache::default())),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdeContextUpdatedEvent {
    client_id: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdeContextBridgeDiscovery {
    url: String,
    bridge_url: String,
    chat_url: String,
    host: String,
    bind_host: String,
    port: u16,
    path: String,
    chat_path: String,
    pid: u32,
    updated_at: String,
    #[serde(default)]
    token: String,
    remote_password: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebAccessInfoOutput {
    running: bool,
    enabled: bool,
    configured_port: u16,
    port: u16,
    #[serde(default)]
    listen_addr: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
    local_url: String,
    remote_urls: Vec<String>,
    remote_password: String,
    active_connections: Vec<WebAccessConnectionSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatJsonRpcRequest {
    #[serde(default)]
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatJsonRpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatAuthLoginInput {
    #[serde(default)]
    password: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatConversationInput {
    conversation_id: String,
    workspace_path: Option<String>,
    workspace_name: Option<String>,
}

// ========== 统一传输协议的桌面入口 ==========
// Sidebar/Web 使用的协议方法也必须有同一份 Tauri 入口；协议差异只能由
// tauri-api.ts 适配器处理，不能让前端维护“Web 专用方法”。

#[tauri::command]
fn list_transport_conversations(state: State<'_, AppState>) -> Result<Value, String> {
    ide_chat_conversation_list(state.inner(), "desktop")
}

#[tauri::command]
fn list_conversation_create_options(state: State<'_, AppState>) -> Result<Value, String> {
    ide_chat_create_conversation_options(state.inner())
}

#[tauri::command]
fn get_conversation_workspace_permission(
    input: Value,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    ide_chat_workspace_permission(state.inner(), input)
}

#[tauri::command]
fn select_conversation_workspace_permission(
    input: Value,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    ide_chat_select_workspace_permission(state.inner(), input)
}

#[tauri::command]
fn save_conversation_workspace_layout(
    input: Value,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    ide_chat_workspace_layout_save(state.inner(), input)
}

#[tauri::command]
async fn list_conversation_workspaces(
    input: Value,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        ide_chat_workspace_list(&app_state, input)
    })
    .await
    .map_err(|err| format!("读取会话工作区列表任务异常：{err}"))?
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatStreamProbeInput {
    conversation_id: String,
    probe_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatConversationBlockPageInput {
    conversation_id: String,
    #[serde(default)]
    block_id: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatSelectModelInput {
    conversation_id: String,
    #[serde(default)]
    api_config_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatResolveTerminalApprovalInput {
    request_id: String,
    approved: bool,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatTerminalApprovalRequestIdInput {
    request_id: String,
}

fn ide_chat_avatar_data_url(state: &AppState, path: Option<&str>) -> String {
    let Some(path) = path.map(str::trim).filter(|value| !value.is_empty()) else {
        return String::new();
    };
    let Ok(avatars_dir) = avatar_storage_dir(state) else {
        return String::new();
    };
    let Ok(root) = fs::canonicalize(&avatars_dir) else {
        return String::new();
    };
    let Ok(target) = fs::canonicalize(path) else {
        return String::new();
    };
    if !target.starts_with(&root) {
        return String::new();
    }
    let Ok(metadata) = fs::metadata(&target) else {
        return String::new();
    };
    if !metadata.is_file() {
        return String::new();
    }
    let ext = target
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "webp" => "image/webp",
        "png" => "image/png",
        _ => return String::new(),
    };
    let Ok(bytes) = fs::read(&target) else {
        return String::new();
    };
    format!("data:{mime};base64,{}", B64.encode(bytes))
}

fn ide_chat_persona_payload(state: &AppState, active_agent_id: Option<&str>) -> Result<Value, String> {
    let assistant_agent_id = state_service_get_assistant_agent_id(state)?;
    let runtime_org = load_runtime_organization_snapshot(state)?;
    let agents = runtime_org.agents;
    let user_alias = agents
        .iter()
        .find(|agent| agent.id == USER_PERSONA_ID || agent.is_built_in_user)
        .map(|agent| agent.name.trim().to_string())
        .unwrap_or_default();
    let active_agent_id = active_agent_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| assistant_agent_id.trim());
    let mut persona_name_map = serde_json::Map::new();
    let mut persona_avatar_url_map = serde_json::Map::new();
    let mut assistant_name = String::new();
    let mut assistant_avatar_url = String::new();
    let mut user_avatar_url = String::new();
    for agent in &agents {
        let id = agent.id.trim();
        if id.is_empty() {
            continue;
        }
        let name = agent.name.trim();
        persona_name_map.insert(
            id.to_string(),
            serde_json::json!(if name.is_empty() { id } else { name }),
        );
        let avatar_url = ide_chat_avatar_data_url(state, agent.avatar_path.as_deref());
        if !avatar_url.is_empty() {
            persona_avatar_url_map.insert(id.to_string(), serde_json::json!(avatar_url.clone()));
        }
        if id == USER_PERSONA_ID || agent.is_built_in_user {
            if !avatar_url.is_empty() {
                user_avatar_url = avatar_url.clone();
            }
        }
        if id == active_agent_id {
            assistant_name = if name.is_empty() { id.to_string() } else { name.to_string() };
            assistant_avatar_url = avatar_url;
        }
    }
    if assistant_name.is_empty() {
        assistant_name = active_agent_id.to_string();
    }
    Ok(serde_json::json!({
        "userAlias": if user_alias.is_empty() { default_user_alias() } else { user_alias.to_string() },
        "userAvatarUrl": user_avatar_url,
        "assistantName": assistant_name,
        "assistantAvatarUrl": assistant_avatar_url,
        "personaNameMap": persona_name_map,
        "personaAvatarUrlMap": persona_avatar_url_map,
    }))
}

fn ide_chat_model_payload_for_conversation(state: &AppState, conversation: &Conversation) -> Result<Value, String> {
    let runtime_snapshot = load_runtime_organization_snapshot(state)?;
    let agent_primary_id = runtime_agent_by_id(&runtime_snapshot, &conversation.agent_id)
        .map(agent_primary_api_config_id)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| runtime_snapshot.config.expert_api_config_id.trim().to_string());
    let config = runtime_snapshot.config;
    let resolved_agent_primary_id = resolve_model_role_api_config_id(&config, &agent_primary_id)
        .unwrap_or_else(|| agent_primary_id.clone());
    let preferred_id = repair_conversation_preferred_model_for_snapshot(state, conversation)?;
    let conversation_call_primary_id = preferred_id
        .as_deref()
        .unwrap_or(resolved_agent_primary_id.as_str())
        .to_string();
    let options = config
        .api_configs
        .iter()
        .filter(|api| is_text_chat_api(api))
        .map(|api| {
            serde_json::json!({
                "id": api.id,
                "name": api.name,
                "requestFormat": api.request_format,
                "model": api.model,
                "enableText": api.enable_text,
                "enableImage": api.enable_image,
            })
        })
        .collect::<Vec<_>>();
    Ok(serde_json::json!({
        "conversationCallPrimaryApiConfigId": conversation_call_primary_id,
        "preferredChatModelId": preferred_id,
        "toolReviewApiConfigId": config.tool_review_api_config_id,
        "chatModelOptions": options,
    }))
}

fn ide_chat_conversation_from_meta_view(conversation_meta: &ConversationMetaView) -> Conversation {
    Conversation {
        id: conversation_meta.id.clone(),
        title: conversation_meta.title.clone(),
        agent_id: conversation_meta.agent_id.clone(),
        bound_conversation_id: None,
        parent_conversation_id: None,
        child_conversation_ids: Vec::new(),
        fork_message_cursor: None,
        unread_count: conversation_meta.unread_count,
        conversation_kind: conversation_meta.conversation_kind.clone(),
        root_conversation_id: conversation_meta.root_conversation_id.clone(),
        delegate_id: conversation_meta.delegate_id.clone(),
        created_at: conversation_meta.created_at.clone(),
        updated_at: conversation_meta.updated_at.clone(),
        last_user_at: None,
        last_assistant_at: None,
        status: conversation_meta.status.clone(),
        user_profile_snapshot: String::new(),
        shell_workspace_path: conversation_meta.shell_workspace_path.clone(),
        shell_workspaces: conversation_meta.shell_workspaces.clone(),
        shell_autonomous_mode: conversation_meta.shell_autonomous_mode,
        shell_work_mode: normalize_shell_work_mode_text(&conversation_meta.shell_work_mode),
        shell_work_branch: conversation_meta.shell_work_branch.clone(),
        archived_at: conversation_meta.archived_at.clone(),
        messages: Vec::new(),
        fast_request_turns: conversation_meta.fast_request_turns.clone(),
        current_todos: conversation_meta.current_todos.clone(),
        memory_recall_table: Vec::new(),
        plan_mode_enabled: false,
        preferred_api_config_id: conversation_meta.preferred_api_config_id.clone(),
        is_draft: conversation_meta.is_draft,
        auto_push_remote_contact_id: conversation_meta.auto_push_remote_contact_id.clone(),
        cumulative_usage: conversation_meta.cumulative_usage.clone(),
        active_goal: conversation_meta.active_goal.clone(),
        last_error: conversation_meta.last_error.clone(),
    }
}

fn ide_chat_delegate_statuses(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<ListConversationDelegateStatusesInput>(params)?;
    serde_json::to_value(list_conversation_delegate_statuses_inner(input, state)?)
        .map_err(|err| format!("Serialize delegate statuses failed: {err}"))
}

fn ide_chat_delegate_abort(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<AbortDelegateConversationInput>(params)?;
    serde_json::to_value(abort_delegate_conversation_inner(input, state)?)
        .map_err(|err| format!("Serialize delegate abort result failed: {err}"))
}

fn ide_chat_delegate_block_page(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<GetConversationBlockPageInput>(params)?;
    serde_json::to_value(get_delegate_conversation_block_page_inner(input, state)?)
        .map_err(|err| format!("Serialize delegate block page failed: {err}"))
}

fn ide_chat_create_conversation_options(state: &AppState) -> Result<Value, String> {
    let runtime_org = load_runtime_organization_snapshot(state)?;
    let config = runtime_org.config;
    let agents = runtime_org.agents;
    let options = agents
        .iter()
        .filter(|agent| !agent.is_built_in_user)
        .filter_map(|agent| {
            let agent_id = agent.id.trim();
            if agent_id.is_empty() {
                return None;
            }
            let api_config_id = agent_primary_chat_api_config_id(&config, agent)?;
            let api_config = config
                .api_configs
                .iter()
                .find(|api| api.id.trim() == api_config_id && is_text_chat_api(api))?;
            let agent_name = if agent.name.trim().is_empty() {
                agent_id
            } else {
                agent.name.trim()
            };
            Some(serde_json::json!({
                "id": agent_id,
                "agentId": agent_id,
                "agentName": agent_name,
                "label": agent_name,
                "name": agent_name,
                "ownerAgentId": agent_id,
                "ownerName": agent_name,
                "providerName": if api_config.name.trim().is_empty() { api_config.id.trim() } else { api_config.name.trim() },
                "modelName": api_config.model.trim(),
                "apiConfigId": api_config_id,
                "childAgentIds": &agent.child_agent_ids,
            }))
        })
        .collect::<Vec<_>>();
    let default_agent_id = state_service_get_assistant_agent_id(state)?;
    Ok(serde_json::json!({
        "agents": options,
        "defaultAgentId": default_agent_id,
    }))
}

include!("ide_context/bridge_access.rs");

include!("ide_context/bridge_clients.rs");

include!("ide_context/context_references.rs");


include!("ide_context/bridge_http.rs");

include!("ide_context/context_commands.rs");

include!("ide_context/jsonrpc_methods.rs");

include!("ide_context/jsonrpc_dispatch.rs");

include!("ide_context/bridge_server.rs");

#[cfg(test)]
mod ide_context_tests {
    use super::*;
    static IDE_CONTEXT_TEST_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

    fn ide_context_test_lock() -> std::sync::MutexGuard<'static, ()> {
        IDE_CONTEXT_TEST_MUTEX
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .expect("lock ide context test mutex")
    }

    fn ide_context_test_state() -> AppState {
        let root = std::env::temp_dir().join(format!("eca-ide-context-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create temp test root");
        std::fs::create_dir_all(root.join("llm-workspace")).expect("create temp llm workspace");
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: root.join("app_config.toml"),
            data_path: root.join("config_mark"),
            llm_workspace_path: root.join("llm-workspace"),
            shared_http_client: reqwest::Client::new(),
            terminal_shell: detect_default_terminal_shell(),
            terminal_shell_candidates: detect_terminal_shell_candidates(),
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_agents: Arc::new(Mutex::new(None)),
            cached_agents_mtime: Arc::new(Mutex::new(None)),
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_conversation_metadata: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_field_metadata_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(std::collections::HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            terminal_background_shell_tasks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            terminal_pending_approvals: Arc::new(Mutex::new(std::collections::HashMap::new())),
            schedule_events: Arc::new(Mutex::new(ScheduleEventStore::default())),
            conversation_runtime_slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(std::collections::HashSet::new())),
            goal_continue_suppressed_conversation_ids: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            pending_chat_result_senders: Arc::new(Mutex::new(std::collections::HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(std::collections::HashMap::new())),
            accepted_submit_trace_ids: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(std::collections::HashMap::new())),
            conversation_list_activity_marks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            task_scheduler_notify: Arc::new(tokio::sync::Notify::new()),
            delegate_runtime_threads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(std::collections::VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(std::collections::HashMap::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(
                std::collections::HashSet::new(),
            )),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_runtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            remote_im_reply_delegate_semaphore: Arc::new(tokio::sync::Semaphore::new(8)),
            remote_im_channel_state_write_locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new("github".to_string())),
            migration_preview_dirs: Arc::new(Mutex::new(std::collections::HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
            backend_ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    #[test]
    fn config_migration_web_transport_should_round_trip_base64_package() {
        let _test_guard = ide_context_test_lock();
        let state = ide_context_test_state();
        state_write_config_cached(&state, &AppConfig::default()).expect("write test config");
        state_write_agents_cached(&state, &[]).expect("write test agents");

        let exported = export_config_migration_package_for_web(
            ExportConfigMigrationPackageInput {
                password: "secret1".to_string(),
            },
            &state,
        )
        .expect("export web migration package");
        assert!(exported.path.is_empty());
        assert_eq!(exported.file_name, "p-ai-migration.zip");
        assert!(!exported.bytes_base64.is_empty());

        let preview = preview_import_config_migration_package_for_web(
            PreviewImportConfigMigrationPackageInput {
                password: "secret1".to_string(),
                package_path: None,
                package_file_name: Some(exported.file_name),
                package_bytes_base64: Some(exported.bytes_base64),
            },
            &state,
        )
        .expect("preview web migration package");
        assert!(!preview.preview_id.is_empty());
        assert!(preview.package_version.starts_with('V'));

        let preview_dir = state
            .migration_preview_dirs
            .lock()
            .expect("lock preview dirs")
            .remove(&preview.preview_id);
        if let Some(path) = preview_dir {
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn ide_context_remote_password_accepts_human_input_format() {
        let runtime = IdeContextRuntime::new();
        let password = ide_context_remote_password(&runtime).expect("remote password");
        let compact_lowercase = password.replace('-', "").to_ascii_lowercase();

        assert!(ide_context_verify_remote_password(&runtime, None, &password).expect("verify password"));
        assert!(
            ide_context_verify_remote_password(&runtime, None, &compact_lowercase)
                .expect("verify compact password")
        );
        assert!(!ide_context_verify_remote_password(&runtime, None, "").expect("reject empty"));
        assert!(!ide_context_verify_remote_password(&runtime, None, "wrong-password").expect("reject wrong"));
    }

    #[test]
    fn ide_context_peer_is_local_only_allows_loopback() {
        let ipv4_local: std::net::SocketAddr = "127.0.0.1:8429".parse().expect("ipv4 local");
        let ipv6_local: std::net::SocketAddr = "[::1]:8429".parse().expect("ipv6 local");
        let remote: std::net::SocketAddr = "192.168.1.10:8429".parse().expect("remote");

        assert!(ide_context_peer_is_local(&ipv4_local));
        assert!(ide_context_peer_is_local(&ipv6_local));
        assert!(!ide_context_peer_is_local(&remote));
    }

    fn ide_context_ws_test_request(origin: Option<&str>) -> Request {
        ide_context_ws_test_request_with_host(origin, "127.0.0.1:8429")
    }

    fn ide_context_ws_test_request_with_host(origin: Option<&str>, host: &str) -> Request {
        let mut builder = Request::builder()
            .uri(IDE_CONTEXT_CHAT_BRIDGE_PATH)
            .header("host", host);
        if let Some(origin) = origin {
            builder = builder.header("origin", origin);
        }
        builder.body(()).expect("build websocket request")
    }

    #[test]
    fn ide_context_ws_origin_allows_owned_pages_and_vscode_webview() {
        assert!(ide_context_ws_origin_allowed(
            &ide_context_ws_test_request(None),
            IDE_CONTEXT_BRIDGE_BASE_PORT
        ));
        assert!(ide_context_ws_origin_allowed(
            &ide_context_ws_test_request(Some("vscode-webview://abc123")),
            IDE_CONTEXT_BRIDGE_BASE_PORT
        ));
        assert!(ide_context_ws_origin_allowed(
            &ide_context_ws_test_request(Some("http://127.0.0.1:8429")),
            IDE_CONTEXT_BRIDGE_BASE_PORT
        ));
        assert!(ide_context_ws_origin_allowed(
            &ide_context_ws_test_request_with_host(Some("http://192.168.1.20:8429"), "192.168.1.20:8429"),
            IDE_CONTEXT_BRIDGE_BASE_PORT
        ));
    }

    #[test]
    fn ide_context_ws_origin_rejects_external_pages() {
        assert!(!ide_context_ws_origin_allowed(
            &ide_context_ws_test_request(Some("https://example.com")),
            IDE_CONTEXT_BRIDGE_BASE_PORT
        ));
        assert!(!ide_context_ws_origin_allowed(
            &ide_context_ws_test_request(Some("http://example.com:8429")),
            IDE_CONTEXT_BRIDGE_BASE_PORT
        ));
        assert!(!ide_context_ws_origin_allowed(
            &ide_context_ws_test_request(Some("http://127.0.0.1:43130")),
            IDE_CONTEXT_BRIDGE_BASE_PORT
        ));
        assert!(!ide_context_ws_origin_allowed(
            &ide_context_ws_test_request_with_host(Some("http://192.168.1.50:8429"), "127.0.0.1:8429"),
            IDE_CONTEXT_BRIDGE_BASE_PORT
        ));
        assert!(!ide_context_ws_origin_allowed(
            &ide_context_ws_test_request(Some("null")),
            IDE_CONTEXT_BRIDGE_BASE_PORT
        ));
    }

    #[test]
    fn ide_context_lan_host_filter_rejects_reserved_and_accepts_private_lan() {
        let mihomo: std::net::Ipv4Addr = "198.18.0.1".parse().expect("mihomo ip");
        let hyperv: std::net::Ipv4Addr = "192.168.240.1".parse().expect("hyperv ip");
        let ethernet: std::net::Ipv4Addr = "192.168.5.23".parse().expect("ethernet ip");
        let cgnat: std::net::Ipv4Addr = "100.64.1.2".parse().expect("cgnat ip");

        assert!(!ide_context_ipv4_is_remote_link_candidate(mihomo));
        assert!(!ide_context_ipv4_is_remote_link_candidate(cgnat));
        assert!(ide_context_ipv4_is_remote_link_candidate(hyperv));
        assert!(ide_context_ipv4_is_remote_link_candidate(ethernet));
    }

    #[test]
    fn ide_context_lan_host_rank_prefers_real_gateway_adapter() {
        let ethernet = IdeContextLanHostCandidate {
            ip: "192.168.5.23".parse().expect("ethernet ip"),
            adapter_name: "以太网".to_string(),
            adapter_description: "Realtek PCIe GbE Family Controller".to_string(),
            has_gateway: true,
            active: true,
        };
        let hyperv = IdeContextLanHostCandidate {
            ip: "192.168.240.1".parse().expect("hyperv ip"),
            adapter_name: "vEthernet (Default Switch)".to_string(),
            adapter_description: "Hyper-V Virtual Ethernet Adapter".to_string(),
            has_gateway: false,
            active: true,
        };

        assert!(ide_context_lan_host_rank(&ethernet) < ide_context_lan_host_rank(&hyperv));
    }

    #[tokio::test]
    async fn ide_context_bind_listener_should_fail_when_fixed_port_is_occupied() {
        let occupied_port = IDE_CONTEXT_BRIDGE_BASE_PORT;
        let occupied_listener = match std::net::TcpListener::bind((IDE_CONTEXT_BRIDGE_BIND_HOST, occupied_port)) {
            Ok(listener) => listener,
            Err(_) => return,
        };

        let error = bind_ide_context_bridge_listener(occupied_port)
            .await
            .expect_err("bind should fail on occupied fixed port");

        drop(occupied_listener);
        assert!(error.contains("已被占用"));
    }

    #[tokio::test]
    async fn local_port_service_restart_serialized_should_run_exclusively_per_service() {
        let core = std::sync::Arc::new(LocalPortServiceCore::new());
        let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let peak = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let first_core = core.clone();
        let first_active = active.clone();
        let first_peak = peak.clone();
        let first = tokio::spawn(async move {
            first_core
                .restart_serialized("web_access", || async move {
                    let current = first_active.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    first_peak.fetch_max(current, std::sync::atomic::Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    first_active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                })
                .await
        });
        let second_core = core.clone();
        let second_active = active.clone();
        let second_peak = peak.clone();
        let second = tokio::spawn(async move {
            second_core
                .restart_serialized("web_access", || async move {
                    let current = second_active.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    second_peak.fetch_max(current, std::sync::atomic::Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    second_active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                })
                .await
        });

        first.await.expect("first task join").expect("first task ok");
        second.await.expect("second task join").expect("second task ok");
        assert_eq!(peak.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn shutdown_web_access_server_should_update_runtime_snapshot() {
        let _test_guard = ide_context_test_lock();
        let port_service = ide_context_port_service_core();
        port_service.clear_runtime_state(WEB_ACCESS_SERVICE_ID).await;
        port_service
            .set_listen_addr(WEB_ACCESS_SERVICE_ID, Some("0.0.0.0:8429".to_string()))
            .await;
        port_service
            .set_status_text(WEB_ACCESS_SERVICE_ID, Some("listening".to_string()))
            .await;
        port_service.set_last_error(WEB_ACCESS_SERVICE_ID, None).await;
        IDE_CONTEXT_BRIDGE_STARTED.store(true, Ordering::SeqCst);

        let shutdown_token = ide_context_bridge_create_shutdown_token();
        let task = tauri::async_runtime::spawn(async move {
            shutdown_token.cancelled().await;
        });
        ide_context_bridge_set_server_task(task);

        shutdown_web_access_server().await;

        let snapshot = port_service.status_snapshot(WEB_ACCESS_SERVICE_ID).await;
        let logs = port_service.get_logs(WEB_ACCESS_SERVICE_ID).await;
        assert_eq!(snapshot.status_text.as_deref(), Some("stopped"));
        assert!(snapshot.listen_addr.is_empty());
        assert!(snapshot.last_error.is_none());
        assert!(
            logs.iter().any(|entry| entry.message.contains("服务已停止")),
            "shutdown should append stop log"
        );

        port_service.clear_runtime_state(WEB_ACCESS_SERVICE_ID).await;
        IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn shutdown_web_access_server_should_clear_stale_error_when_already_stopped() {
        let _test_guard = ide_context_test_lock();
        let port_service = ide_context_port_service_core();
        port_service
            .set_listen_addr(WEB_ACCESS_SERVICE_ID, Some("0.0.0.0:8429".to_string()))
            .await;
        port_service
            .set_status_text(WEB_ACCESS_SERVICE_ID, Some("error".to_string()))
            .await;
        port_service
            .set_last_error(WEB_ACCESS_SERVICE_ID, Some("stale failure".to_string()))
            .await;
        IDE_CONTEXT_BRIDGE_STARTED.store(false, Ordering::SeqCst);

        shutdown_web_access_server().await;

        let snapshot = port_service.status_snapshot(WEB_ACCESS_SERVICE_ID).await;
        assert_eq!(snapshot.status_text.as_deref(), Some("stopped"));
        assert!(snapshot.listen_addr.is_empty());
        assert!(snapshot.last_error.is_none());

        port_service.clear_runtime_state(WEB_ACCESS_SERVICE_ID).await;
    }

    #[test]
    fn ide_context_bridge_tokens_allow_concurrent_consumers_until_expiry() {
        let runtime = IdeContextRuntime::new();
        let token = ide_context_issue_bridge_token_with_state(&runtime, None).expect("issue token");

        let next_token = ide_context_consume_bridge_token_with_state(&runtime, None, &token)
            .expect("first consume");
        assert_eq!(next_token, token);

        let second_next = ide_context_consume_bridge_token_with_state(&runtime, None, &token)
            .expect("second consume with same token");
        assert_eq!(second_next, token);
    }

    #[test]
    fn ide_context_bridge_tokens_keep_multiple_login_tokens() {
        let runtime = IdeContextRuntime::new();
        let first = ide_context_issue_bridge_token_with_state(&runtime, None).expect("issue first token");
        let second = ide_context_issue_bridge_token_with_state(&runtime, None).expect("issue second token");

        assert_ne!(first, second);
        assert_eq!(
            ide_context_consume_bridge_token_with_state(&runtime, None, &first)
                .expect("first token remains valid"),
            first
        );
        assert_eq!(
            ide_context_consume_bridge_token_with_state(&runtime, None, &second)
                .expect("second token remains valid"),
            second
        );
    }

    #[test]
    fn ide_context_bridge_tokens_reject_unknown_token() {
        let runtime = IdeContextRuntime::new();
        let _ = ide_context_issue_bridge_token_with_state(&runtime, None).expect("issue token");
        let err = ide_context_consume_bridge_token_with_state(&runtime, None, "bad-token")
            .expect_err("invalid token");
        assert!(err.0.contains("invalid authToken"));
    }

    #[test]
    fn ide_context_bridge_tokens_reissue_when_cache_expired() {
        let runtime = IdeContextRuntime::new();
        {
            let mut auth = runtime.bridge_auth.lock().expect("lock auth");
            auth.valid_tokens.insert(
                "expired-token".to_string(),
                time::OffsetDateTime::now_utc() - time::Duration::seconds(1),
            );
        }

        let err = ide_context_consume_bridge_token_with_state(&runtime, None, "expired-token")
            .expect_err("expired token should refresh discovery");
        assert!(err.0.contains("expired"));
        let refreshed = err.1.expect("should issue refreshed token");
        let auth = runtime.bridge_auth.lock().expect("lock auth");
        assert!(auth.valid_tokens.contains_key(&refreshed));
    }

    #[tokio::test]
    async fn ide_chat_send_message_should_return_formal_assistant_message_id_immediately() {
        let state = ide_context_test_state();
        let created = conversation_service_v2()
            .create_conversation(
                &state,
                &CreateUnarchivedConversationInput {
                    api_config_id: None,
                    agent_id: Some(DEFAULT_AGENT_ID.to_string()),
                    title: Some("IDE发送即时assistant气泡".to_string()),
                    copy_source_conversation_id: None,
                    shell_workspaces: None,
                    shell_work_mode: None,
                    shell_work_branch: None,
                    shell_autonomous_mode: None,
                    is_draft: Some(false),
                },
            )
            .expect("create conversation");
        flush_pending_persists_blocking(&state).expect("flush created conversation");

        let result = ide_chat_send_message(
            &state,
            serde_json::json!({
                "payload": {
                    "text": "你好",
                },
                "session": {
                    "apiConfigId": null,
                    "agentId": DEFAULT_AGENT_ID,
                    "conversationId": created.conversation_id,
                },
            }),
        )
        .await
        .expect("send message");

        let assistant_message_id = result
            .get("assistantMessageId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let user_message_id = result
            .get("userMessageId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        let accepted = result
            .get("accepted")
            .and_then(Value::as_bool);
        let ingress = result
            .get("ingress")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();

        assert!(
            !user_message_id.is_empty(),
            "chat.send 应立即返回正式 userMessageId"
        );
        assert!(
            !assistant_message_id.is_empty(),
            "chat.send 应立即返回正式 assistantMessageId"
        );
        assert_eq!(accepted, Some(true));
        assert!(!ingress.is_empty(), "chat.send 应返回 ingress");
    }

    #[tokio::test]
    async fn ide_chat_create_and_delete_should_use_canonical_conversation_contract() {
        let state = ide_context_test_state();
        let created = ide_chat_create_conversation(
            &state,
            serde_json::json!({
                "apiConfigId": null,
                "agentId": DEFAULT_AGENT_ID,
                "title": "Web canonical conversation",
                "shellWorkspaces": null,
                "shellAutonomousMode": false,
            }),
        )
        .await
        .expect("create conversation");
        flush_pending_persists_blocking(&state).expect("flush created conversation");
        let conversation_id = created
            .get("conversationId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(!conversation_id.is_empty());
        assert!(created.get("unarchivedConversations").is_some());
        assert!(created.get("conversation").is_none());

        let rebound = ide_chat_rebind_conversation_recipient(
            &state,
            serde_json::json!({
                "conversationId": conversation_id.clone(),
                "agentId": DEFAULT_AGENT_ID,
            }),
        )
        .await
        .expect("rebind conversation");
        assert_eq!(
            rebound.get("conversationId").and_then(Value::as_str),
            Some(conversation_id.as_str())
        );
        assert!(rebound.get("preferredApiConfigId").is_some());
        assert!(rebound.get("unarchivedConversations").is_none());

        let compaction_preview_error = ide_chat_compact_preview(
            &state,
            serde_json::json!({ "conversationId": conversation_id.clone() }),
        )
        .expect_err("test config should fail canonical model validation");
        assert!(compaction_preview_error.contains("API key is empty"));

        let deleted = ide_chat_delete_conversation(
            &state,
            serde_json::json!({ "conversationId": conversation_id.clone() }),
        )
        .await
        .expect("delete conversation");
        assert_eq!(
            deleted.get("deletedConversationId").and_then(Value::as_str),
            Some(conversation_id.as_str())
        );
        assert!(deleted.get("activeConversationId").is_some());
        assert!(deleted.get("unarchivedConversations").is_some());
        assert!(deleted.get("preferredConversationId").is_none());
    }

    #[tokio::test]
    async fn ide_chat_rewind_should_reject_local_patch_restore_on_web() {
        let state = ide_context_test_state();
        let error = ide_chat_rewind_conversation(
            &state,
            serde_json::json!({
                "session": {
                    "apiConfigId": null,
                    "agentId": DEFAULT_AGENT_ID,
                    "conversationId": "conversation-1"
                },
                "messageId": "message-1",
                "undoApplyPatch": true
            }),
        )
        .await
        .expect_err("web must reject local patch restore");
        assert!(error.starts_with("WEB_NATIVE_CAPABILITY_UNAVAILABLE:"));
    }

    #[test]
    fn ide_chat_llm_round_logs_should_use_canonical_log_contract() {
        let state = ide_context_test_state();
        let listed = ide_chat_list_recent_llm_round_logs_for_web_settings(&state)
            .expect("list round logs");
        assert!(listed.is_array());

        let section = ide_chat_get_recent_llm_round_log_section_for_web_settings(
            &state,
            serde_json::json!({ "id": "", "section": "answer" }),
        )
        .expect("get empty log section");
        assert!(section.is_null());

        let cleared = ide_chat_clear_recent_llm_round_logs_for_web_settings(&state)
            .expect("clear round logs");
        assert_eq!(cleared, serde_json::json!(true));
    }

    #[test]
    fn ide_chat_mcp_validation_should_match_canonical_command_output() {
        let definition_json = serde_json::json!({
            "name": "test-http-server",
            "transport": {
                "type": "http",
                "url": "https://example.com/mcp"
            }
        })
        .to_string();
        let canonical = mcp_validate_definition_inner(McpDefinitionValidateInput {
            definition_json: definition_json.clone(),
            existing_member_names: Vec::new(),
        })
        .expect("canonical validation");
        let web = ide_chat_mcp_validate_definition_for_web_settings(serde_json::json!({
            "input": { "definitionJson": definition_json }
        }))
        .expect("web validation");

        assert_eq!(web, serde_json::to_value(canonical).expect("serialize canonical"));
    }
}