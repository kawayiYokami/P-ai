#[derive(Debug, Clone)]
struct PreparedConversationPromptPayload {
    history_messages: Vec<PreparedHistoryMessage>,
    latest_user_text: String,
    latest_user_meta_text: String,
    latest_user_extra_blocks: Vec<String>,
    latest_images: Vec<PreparedBinaryPayload>,
    latest_audios: Vec<PreparedBinaryPayload>,
}

#[derive(Debug, Clone)]
struct AgentSystemPromptSnapshot {
    agent_prompt_block: String,
    agent_tool_rule_blocks: Vec<String>,
}

#[derive(Debug, Clone)]
struct AgentSystemPromptCacheEntry {
    agent_id: String,
    snapshot: AgentSystemPromptSnapshot,
    dirty_reason: Option<PromptCacheDirtyKind>,
}

#[derive(Debug, Clone)]
struct ConversationEnvironmentPromptSnapshot {
    runtime_blocks: Vec<String>,
    im_rule_blocks: Vec<String>,
}

#[derive(Debug, Clone)]
struct ConversationEnvironmentPromptCacheEntry {
    conversation_id: String,
    snapshot: ConversationEnvironmentPromptSnapshot,
    dirty_reason: Option<PromptCacheDirtyKind>,
}

#[derive(Debug, Clone)]
struct FinalSystemPromptCacheEntry {
    conversation_id: String,
    agent_id: String,
    text: String,
    dirty_state: FinalSystemPromptDirtyState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptCacheDirtyKind {
    SystemSource,
    SystemEnvironment,
}

impl PromptCacheDirtyKind {
    fn as_log_reason(self) -> &'static str {
        match self {
            Self::SystemSource => "dirty_system_source",
            Self::SystemEnvironment => "dirty_system_environment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct FinalSystemPromptDirtyState {
    system_source: bool,
    system_environment: bool,
}

impl FinalSystemPromptDirtyState {
    fn is_clean(self) -> bool {
        !self.system_source && !self.system_environment
    }

    fn mark(self, kind: PromptCacheDirtyKind) -> Self {
        let mut next = self;
        match kind {
            PromptCacheDirtyKind::SystemSource => next.system_source = true,
            PromptCacheDirtyKind::SystemEnvironment => next.system_environment = true,
        }
        next
    }

    fn rebuild_reason(self) -> &'static str {
        match (self.system_source, self.system_environment) {
            (true, true) => "dirty_system_source_and_environment",
            (true, false) => "dirty_system_source",
            (false, true) => "dirty_system_environment",
            (false, false) => "cache_miss",
        }
    }
}

fn system_prompt_text_cache(
) -> &'static Mutex<std::collections::HashMap<String, FinalSystemPromptCacheEntry>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, FinalSystemPromptCacheEntry>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn agent_system_prompt_cache(
) -> &'static Mutex<std::collections::HashMap<String, AgentSystemPromptCacheEntry>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, AgentSystemPromptCacheEntry>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn conversation_environment_prompt_cache(
) -> &'static Mutex<std::collections::HashMap<String, ConversationEnvironmentPromptCacheEntry>> {
    static CACHE: OnceLock<
        Mutex<std::collections::HashMap<String, ConversationEnvironmentPromptCacheEntry>>,
    > = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn cache_lock_recover<'a, T>(
    label: &str,
    mutex: &'a Mutex<T>,
) -> std::sync::MutexGuard<'a, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(err) => {
            runtime_log_info(format!(
                "[系统提示词] 警告: {} 锁已 poison，继续恢复使用 error={:?}",
                label, err
            ));
            err.into_inner()
        }
    }
}

fn prompt_cache_scope_key(state: Option<&AppState>) -> String {
    state
        .map(|value| value.data_path.display().to_string())
        .unwrap_or_else(|| "<global>".to_string())
}

fn prompt_runtime_rg_installed() -> bool {
    host_runtime_prerequisite_installed("rg").unwrap_or(false)
}

fn build_agent_system_prompt_cache_key(
    state: Option<&AppState>,
    agent: &AgentProfile,
    ui_language: &str,
) -> String {
    format!(
        "scope={}|agent={}|ui={}",
        prompt_cache_scope_key(state),
        agent.id.trim(),
        ui_language.trim(),
    )
}

fn build_agent_system_prompt_snapshot_uncached(
    agent: &AgentProfile,
    agents: &[AgentProfile],
    ui_language: &str,
) -> AgentSystemPromptSnapshot {
    let agent_prompt_block =
        build_organization_prompt_block(agent.id.trim(), agents, ui_language);
    let agent_tool_rule_blocks = build_system_tools_rule_blocks(
        agent.id.trim(),
        agents,
        prompt_runtime_rg_installed(),
    );
    AgentSystemPromptSnapshot {
        agent_prompt_block,
        agent_tool_rule_blocks,
    }
}

fn get_or_build_agent_system_prompt_snapshot(
    state: Option<&AppState>,
    agent: &AgentProfile,
    agents: &[AgentProfile],
    ui_language: &str,
) -> AgentSystemPromptSnapshot {
    let cache_key = build_agent_system_prompt_cache_key(state, agent, ui_language);
    let mut rebuild_reason = "cache_miss";
    {
        let cache = cache_lock_recover("agent_system_prompt_cache", agent_system_prompt_cache());
        if let Some(entry) = cache.get(&cache_key) {
            if entry.dirty_reason.is_none() {
                return entry.snapshot.clone();
            }
            rebuild_reason = entry
                .dirty_reason
                .map(PromptCacheDirtyKind::as_log_reason)
                .unwrap_or("cache_miss");
        }
    }
    runtime_log_info(format!(
        "[人格提示词] 开始重建 agent_id={} reason={}",
        agent.id.trim(),
        rebuild_reason
    ));
    let snapshot = build_agent_system_prompt_snapshot_uncached(agent, agents, ui_language);
    let mut cache = cache_lock_recover("agent_system_prompt_cache", agent_system_prompt_cache());
    cache.insert(
        cache_key,
        AgentSystemPromptCacheEntry {
            agent_id: agent.id.trim().to_string(),
            snapshot: snapshot.clone(),
            dirty_reason: None,
        },
    );
    snapshot
}

fn split_system_preamble_blocks(
    system_preamble_blocks: &[String],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut tool_rule_blocks = Vec::<String>::new();
    let mut runtime_blocks = Vec::<String>::new();
    let mut im_rule_blocks = Vec::<String>::new();
    for block in system_preamble_blocks {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            continue;
        }
        match classify_system_prompt_extra_block(trimmed) {
            SystemPromptExtraBlockGroup::ToolRules => tool_rule_blocks.push(trimmed.to_string()),
            SystemPromptExtraBlockGroup::Runtime => runtime_blocks.push(trimmed.to_string()),
            SystemPromptExtraBlockGroup::ImRules => im_rule_blocks.push(trimmed.to_string()),
        }
    }
    (tool_rule_blocks, runtime_blocks, im_rule_blocks)
}

fn build_conversation_environment_prompt_cache_key(
    state: Option<&AppState>,
    conversation: &Conversation,
    _mode_label: &str,
) -> String {
    format!(
        "scope={}|conversation_id={}",
        prompt_cache_scope_key(state),
        conversation.id.trim(),
    )
}

fn build_conversation_environment_prompt_snapshot_uncached(
    conversation: &Conversation,
    terminal_block: Option<&str>,
    runtime_extra_blocks: &[String],
    im_extra_blocks: &[String],
) -> ConversationEnvironmentPromptSnapshot {
    let mut runtime_blocks = Vec::<String>::new();
    if let Some(terminal_block) = terminal_block
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        runtime_blocks.push(terminal_block.to_string());
    }
    runtime_blocks.extend(
        runtime_extra_blocks
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    );

    let mut im_rule_blocks = Vec::<String>::new();
    if conversation_is_remote_im_contact(conversation) {
        im_rule_blocks.push(prompt_xml_block(
            "remote im contact rules",
            "联系人是特殊用户，不是当前聊天窗口中的直接用户。\n他们的消息来自远程接口接入，应视为独立的外部用户。\n不要把联系人和当前用户混为一谈，也不要混淆回复目标。\n普通文字答复直接写在最终 assistant 回复中。",
        ));
    }
    im_rule_blocks.extend(
        im_extra_blocks
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    );

    ConversationEnvironmentPromptSnapshot {
        runtime_blocks,
        im_rule_blocks,
    }
}

fn get_or_build_conversation_environment_prompt_snapshot(
    state: Option<&AppState>,
    conversation: &Conversation,
    mode_label: &str,
    terminal_block: Option<&str>,
    runtime_extra_blocks: &[String],
    im_extra_blocks: &[String],
) -> ConversationEnvironmentPromptSnapshot {
    let cache_key = build_conversation_environment_prompt_cache_key(
        state,
        conversation,
        mode_label,
    );
    let mut rebuild_reason = "cache_miss";
    {
        let cache = cache_lock_recover(
            "conversation_environment_prompt_cache",
            conversation_environment_prompt_cache(),
        );
        if let Some(entry) = cache.get(&cache_key) {
            if entry.dirty_reason.is_none() {
                return entry.snapshot.clone();
            }
            rebuild_reason = entry
                .dirty_reason
                .map(PromptCacheDirtyKind::as_log_reason)
                .unwrap_or("cache_miss");
        }
    }
    runtime_log_info(format!(
        "[会话环境提示词] 开始重建 conversation_id={} reason={}",
        conversation.id.trim(),
        rebuild_reason
    ));
    let snapshot = build_conversation_environment_prompt_snapshot_uncached(
        conversation,
        terminal_block,
        runtime_extra_blocks,
        im_extra_blocks,
    );
    let mut cache = cache_lock_recover(
        "conversation_environment_prompt_cache",
        conversation_environment_prompt_cache(),
    );
    cache.insert(
        cache_key,
        ConversationEnvironmentPromptCacheEntry {
            conversation_id: conversation.id.trim().to_string(),
            snapshot: snapshot.clone(),
            dirty_reason: None,
        },
    );
    snapshot
}

fn append_system_prompt_block(target: &mut String, block: Option<&str>) {
    let Some(trimmed) = block.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    if !target.trim().is_empty() {
        if !target.ends_with('\n') {
            target.push('\n');
        }
    }
    target.push_str(trimmed);
    target.push('\n');
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SystemPromptExtraBlockGroup {
    ToolRules,
    Runtime,
    ImRules,
}

const REMOTE_CONVERSATION_IDENTITY_FALLBACK_MESSAGE_LIMIT: usize = 32;

fn classify_system_prompt_extra_block(block: &str) -> SystemPromptExtraBlockGroup {
    let trimmed = block.trim();
    if trimmed.contains("<remote im runtime activation>")
        || trimmed.contains("<remote im contact downloads>")
    {
        return SystemPromptExtraBlockGroup::ImRules;
    }
    if trimmed.contains("<skill usage>")
        || trimmed.contains("<skill index>")
        || trimmed.contains("<todo guide>")
    {
        return SystemPromptExtraBlockGroup::ToolRules;
    }
    SystemPromptExtraBlockGroup::Runtime
}

#[derive(Debug, Clone)]
struct RemoteConversationPromptIdentity {
    conversation_type_label: &'static str,
    name_label: &'static str,
    id_label: &'static str,
    channel_id: String,
    display_name: String,
    id: String,
    latest_sender_name: String,
    latest_sender_id: String,
}

fn remote_conversation_type_label(contact_type: &str) -> &'static str {
    match contact_type.trim().to_ascii_lowercase().as_str() {
        "group" => "群聊",
        "private" => "私聊",
        _ => "远程联系人会话",
    }
}

fn remote_conversation_name_label(contact_type: &str) -> &'static str {
    match contact_type.trim().to_ascii_lowercase().as_str() {
        "group" => "群名",
        "private" => "用户名",
        _ => "联系人名称",
    }
}

fn remote_conversation_id_label(contact_type: &str) -> &'static str {
    match contact_type.trim().to_ascii_lowercase().as_str() {
        "group" => "群ID",
        "private" => "用户ID",
        _ => "联系人ID",
    }
}

fn remote_conversation_identity_from_contact(
    contact: &RemoteImContact,
) -> RemoteConversationPromptIdentity {
    RemoteConversationPromptIdentity {
        conversation_type_label: remote_conversation_type_label(&contact.remote_contact_type),
        name_label: remote_conversation_name_label(&contact.remote_contact_type),
        id_label: remote_conversation_id_label(&contact.remote_contact_type),
        channel_id: contact.channel_id.trim().to_string(),
        display_name: remote_im_contact_display_name(contact),
        id: contact.remote_contact_id.trim().to_string(),
        latest_sender_name: String::new(),
        latest_sender_id: String::new(),
    }
}

fn remote_conversation_identity_from_message_origin(
    origin: &Value,
) -> Option<RemoteConversationPromptIdentity> {
    let contact_type = remote_im_origin_string(origin, "contact_type").unwrap_or("");
    let channel_id = remote_im_origin_string(origin, "channel_id").unwrap_or("");
    let contact_name = remote_im_origin_string(origin, "contact_name").unwrap_or("");
    let contact_id = remote_im_origin_string(origin, "contact_id").unwrap_or("");
    let sender_name = remote_im_origin_string(origin, "sender_name").unwrap_or("");
    let sender_id = remote_im_origin_string(origin, "sender_id").unwrap_or("");
    let is_group = contact_type.trim().eq_ignore_ascii_case("group");
    let display_name = if is_group {
        contact_name
    } else if !contact_name.trim().is_empty() {
        contact_name
    } else {
        sender_name
    };
    let id = if !contact_id.trim().is_empty() {
        contact_id
    } else {
        sender_id
    };
    if contact_type.trim().is_empty()
        && display_name.trim().is_empty()
        && id.trim().is_empty()
        && sender_name.trim().is_empty()
    {
        return None;
    }
    Some(RemoteConversationPromptIdentity {
        conversation_type_label: remote_conversation_type_label(contact_type),
        name_label: remote_conversation_name_label(contact_type),
        id_label: remote_conversation_id_label(contact_type),
        channel_id: channel_id.trim().to_string(),
        display_name: display_name.trim().to_string(),
        id: id.trim().to_string(),
        latest_sender_name: sender_name.trim().to_string(),
        latest_sender_id: sender_id.trim().to_string(),
    })
}

fn remote_conversation_identity_from_messages(
    conversation: &Conversation,
) -> Option<RemoteConversationPromptIdentity> {
    conversation
        .messages
        .iter()
        .rev()
        .take(REMOTE_CONVERSATION_IDENTITY_FALLBACK_MESSAGE_LIMIT)
        .filter_map(remote_im_origin_from_message)
        .find_map(remote_conversation_identity_from_message_origin)
}

fn remote_conversation_identity_from_state(
    state: Option<&AppState>,
    conversation: &Conversation,
) -> Option<RemoteConversationPromptIdentity> {
    let state = state?;
    let conversation_id = conversation.id.trim();
    let root_key = conversation
        .root_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let contacts = state_service_list_remote_im_contacts(state, None).ok()?;
    contacts
        .into_iter()
        .find(|contact| {
            root_key
                .map(|key| remote_im_contact_conversation_key(contact) == key)
                .unwrap_or(false)
                || contact
                    .bound_conversation_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    == Some(conversation_id)
        })
        .map(|contact| remote_conversation_identity_from_contact(&contact))
}

fn remote_conversation_settings_body(
    state: Option<&AppState>,
    conversation: &Conversation,
) -> String {
    let identity = remote_conversation_identity_from_state(state, conversation)
        .or_else(|| remote_conversation_identity_from_messages(conversation));
    let Some(identity) = identity else {
        return "本会话是远程联系人会话。".to_string();
    };
    let mut lines = vec![format!("本会话是{}。", identity.conversation_type_label)];
    if !identity.channel_id.trim().is_empty() {
        lines.push(format!(
            "渠道ID：{}",
            xml_escape_prompt(&identity.channel_id)
        ));
    }
    if !identity.display_name.trim().is_empty() {
        lines.push(format!(
            "{}：{}",
            identity.name_label,
            xml_escape_prompt(&identity.display_name)
        ));
    }
    if !identity.id.trim().is_empty() {
        lines.push(format!(
            "{}：{}",
            identity.id_label,
            xml_escape_prompt(&identity.id)
        ));
    }
    if identity.conversation_type_label == "群聊" {
        if !identity.latest_sender_name.trim().is_empty() {
            lines.push(format!(
                "最近发言用户名：{}",
                xml_escape_prompt(&identity.latest_sender_name)
            ));
        }
        if !identity.latest_sender_id.trim().is_empty() {
            lines.push(format!(
                "最近发言用户ID：{}",
                xml_escape_prompt(&identity.latest_sender_id)
            ));
        }
    }
    lines.join("\n")
}

fn selected_api_prompt_model_name(selected_api: Option<&ApiConfig>) -> Option<String> {
    let raw = selected_api?.model.trim();
    if raw.is_empty() {
        return None;
    }
    let model = raw
        .rsplit_once("::")
        .map(|(_, model)| model.trim())
        .unwrap_or(raw)
        .trim();
    if model.is_empty() {
        None
    } else {
        Some(model.to_string())
    }
}

fn driving_model_prompt_block(selected_api: Option<&ApiConfig>) -> Option<String> {
    let model = selected_api_prompt_model_name(selected_api)?;
    Some(prompt_xml_block(
        "model settings",
        format!("驱动模型：{}", xml_escape_prompt(&model)),
    ))
}

fn build_core_system_prompt_text(
    conversation: &Conversation,
    agent: &AgentProfile,
    user_profile: Option<(&str, &str)>,
    response_style_id: &str,
    ui_language: &str,
    state: Option<&AppState>,
) -> String {
    let response_style_block = response_style_preset_optional(response_style_id).map(|response_style| {
        prompt_xml_block(
            "conversation style",
            format!("当前风格：{}\n{}", response_style.name, response_style.prompt),
        )
    });
    let date_timezone_line = prompt_current_date_timezone_line(ui_language);
    let highest_instruction_md = highest_instruction_markdown();
    let (
        not_provided_label,
        assistant_settings_label,
        user_settings_label,
        remote_settings_label,
        language_settings_label,
        user_nickname_label,
        user_intro_label,
        language_follow_user_line,
        language_instruction,
    ) = (
        "未提供",
        "persona settings",
        "admin user settings",
        "remote conversation settings",
        "language settings",
        "用户昵称",
        "用户自我介绍",
        "- 若用户明确指定回答语言，以用户指定为准。",
        "默认使用中文回答。",
    );
    if conversation_is_remote_im_contact(conversation) {
        return [
            highest_instruction_md.to_string(),
            prompt_xml_block(assistant_settings_label, agent.system_prompt.trim()),
            prompt_xml_block(
                remote_settings_label,
                remote_conversation_settings_body(state, conversation),
            ),
            response_style_block.clone().unwrap_or_default(),
            prompt_xml_block(
                language_settings_label,
                format!(
                    "{}\n{}\n{}",
                    language_instruction, language_follow_user_line, date_timezone_line
                ),
            ),
        ]
        .join("\n");
    }
    if let Some((user_name, user_intro)) = user_profile {
        let user_intro_display = if user_intro.trim().is_empty() {
            not_provided_label.to_string()
        } else {
            user_intro.trim().to_string()
        };
        let user_profile_snapshot = conversation.user_profile_snapshot.trim();
        let user_settings_body = if user_profile_snapshot.is_empty() {
            format!(
                "{}：{}\n{}：{}",
                user_nickname_label,
                xml_escape_prompt(user_name),
                user_intro_label,
                xml_escape_prompt(&user_intro_display)
            )
        } else {
            format!(
                "{}：{}\n{}：{}\n用户画像快照：\n{}",
                user_nickname_label,
                xml_escape_prompt(user_name),
                user_intro_label,
                xml_escape_prompt(&user_intro_display),
                user_profile_snapshot
            )
        };
        [
            highest_instruction_md.to_string(),
            prompt_xml_block(assistant_settings_label, agent.system_prompt.trim()),
            prompt_xml_block(user_settings_label, user_settings_body),
            response_style_block.clone().unwrap_or_default(),
            prompt_xml_block(
                language_settings_label,
                format!(
                    "{}\n{}\n{}",
                    language_instruction, language_follow_user_line, date_timezone_line
                ),
            ),
        ]
        .join("\n")
    } else {
        [
            highest_instruction_md.to_string(),
            prompt_xml_block(assistant_settings_label, agent.system_prompt.trim()),
            response_style_block.unwrap_or_default(),
            prompt_xml_block(
                language_settings_label,
                format!("{}\n{}", language_instruction, date_timezone_line),
            ),
        ]
        .join("\n")
    }
}

fn build_system_prompt_text_uncached(ordered_blocks: &[String]) -> String {
    let mut prompt = String::new();
    for block in ordered_blocks {
        append_system_prompt_block(&mut prompt, Some(block));
    }
    prompt
}

#[cfg(test)]
fn finalize_system_prompt_with_manager(
    state: Option<&AppState>,
    mode_label: &str,
    conversation: &Conversation,
    agent: &AgentProfile,
    agents: &[AgentProfile],
    selected_api: Option<&ApiConfig>,
    _user_profile: Option<(&str, &str)>,
    _response_style_id: &str,
    ui_language: &str,
    fixed_system_prompt_text: &str,
    user_profile_memory_block: Option<&str>,
    terminal_block: Option<&str>,
    _system_preamble_blocks: &[String],
    stage_logger: Option<&dyn Fn(&str)>,
) -> String {
    conversation_prompt_service().finalize_system_prompt(
        state,
        mode_label,
        conversation,
        agent,
        agents,
        selected_api,
        ui_language,
        fixed_system_prompt_text,
        user_profile_memory_block,
        terminal_block,
        &ChatPromptOverrides {
            latest_user_intent: None,
            todo_tool_enabled: false,
            remote_im_activation_sources: Vec::new(),
            latest_images: None,
            latest_audios: None,
        },
        stage_logger,
    )
}

fn mark_prompt_cache_rebuild_internal(
    state: &AppState,
    agent_ids: &[String],
    conversation_ids: &[String],
    mark_agent: bool,
    mark_environment: bool,
    mark_final: bool,
    dirty_kind: PromptCacheDirtyKind,
) {
    let scope_prefix = format!("scope={}|", prompt_cache_scope_key(Some(state)));
    let agent_ids = agent_ids
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let conversation_ids = conversation_ids
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let mark_all = agent_ids.is_empty() && conversation_ids.is_empty();

    let mut agent_marked = 0usize;
    if mark_agent {
        let mut cache = cache_lock_recover("agent_system_prompt_cache", agent_system_prompt_cache());
        for (key, entry) in cache.iter_mut() {
            if !key.starts_with(&scope_prefix) {
                continue;
            }
            let matched = mark_all || agent_ids.contains(entry.agent_id.trim());
            if matched && entry.dirty_reason.is_none() {
                entry.dirty_reason = Some(dirty_kind);
                agent_marked += 1;
            }
        }
    }

    let mut environment_marked = 0usize;
    if mark_environment {
        let mut cache = cache_lock_recover(
            "conversation_environment_prompt_cache",
            conversation_environment_prompt_cache(),
        );
        for (key, entry) in cache.iter_mut() {
            if !key.starts_with(&scope_prefix) {
                continue;
            }
            let matched = mark_all
                || conversation_ids.contains(entry.conversation_id.trim());
            if matched && entry.dirty_reason.is_none() {
                entry.dirty_reason = Some(dirty_kind);
                environment_marked += 1;
            }
        }
    }

    let mut final_marked = 0usize;
    if mark_final {
        let mut cache = cache_lock_recover("system_prompt_text_cache", system_prompt_text_cache());
        for (key, entry) in cache.iter_mut() {
            if !key.starts_with(&scope_prefix) {
                continue;
            }
            let matched = mark_all
                || conversation_ids.contains(entry.conversation_id.trim())
                || agent_ids.contains(entry.agent_id.trim());
            let next_state = entry.dirty_state.mark(dirty_kind);
            if matched && next_state != entry.dirty_state {
                entry.dirty_state = next_state;
                final_marked += 1;
            }
        }
    }

    runtime_log_debug(format!(
        "[系统提示词] 标记重建 完成 reason={} agent_ids={:?} conversation_ids={:?} agent_marked={} environment_marked={} final_marked={}",
        dirty_kind.as_log_reason(),
        agent_ids,
        conversation_ids,
        agent_marked,
        environment_marked,
        final_marked
    ));
}

fn mark_prompt_cache_rebuild_for_system_sources_by_agents(
    state: &AppState,
    agent_ids: &[String],
) {
    mark_prompt_cache_rebuild_internal(
        state,
        agent_ids,
        &[],
        true,
        false,
        true,
        PromptCacheDirtyKind::SystemSource,
    );
}

fn mark_prompt_cache_rebuild_for_system_environment_by_conversation(
    state: &AppState,
    conversation_id: &str,
) {
    mark_prompt_cache_rebuild_internal(
        state,
        &[],
        &[conversation_id.trim().to_string()],
        false,
        true,
        true,
        PromptCacheDirtyKind::SystemEnvironment,
    );
}

fn mark_prompt_cache_rebuild_for_all_system_environments(state: &AppState) {
    mark_prompt_cache_rebuild_internal(
        state,
        &[],
        &[],
        false,
        true,
        true,
        PromptCacheDirtyKind::SystemEnvironment,
    );
}

fn mark_prompt_cache_rebuild_for_all_final_system_sources(state: &AppState) {
    mark_prompt_cache_rebuild_internal(
        state,
        &[],
        &[],
        false,
        false,
        true,
        PromptCacheDirtyKind::SystemSource,
    );
}
