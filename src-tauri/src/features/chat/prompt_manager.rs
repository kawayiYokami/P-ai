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
struct DepartmentSystemPromptSnapshot {
    department_prompt_block: String,
    department_tool_rule_blocks: Vec<String>,
}

#[derive(Debug, Clone)]
struct DepartmentSystemPromptCacheEntry {
    agent_id: String,
    department_id: String,
    snapshot: DepartmentSystemPromptSnapshot,
    dirty: bool,
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
    dirty: bool,
}

#[derive(Debug, Clone)]
struct FinalSystemPromptCacheEntry {
    conversation_id: String,
    agent_id: String,
    department_id: String,
    text: String,
    dirty: bool,
}

fn system_prompt_text_cache(
) -> &'static Mutex<std::collections::HashMap<String, FinalSystemPromptCacheEntry>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, FinalSystemPromptCacheEntry>>> =
        OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn department_system_prompt_cache(
) -> &'static Mutex<std::collections::HashMap<String, DepartmentSystemPromptCacheEntry>> {
    static CACHE: OnceLock<
        Mutex<std::collections::HashMap<String, DepartmentSystemPromptCacheEntry>>,
    > = OnceLock::new();
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

fn department_id_for_agent(departments: &[DepartmentConfig], agent_id: &str) -> String {
    department_for_agent_id(&departments_only_config(departments), agent_id)
        .map(|item| item.id.trim().to_string())
        .unwrap_or_default()
}

fn build_department_system_prompt_cache_key(
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

fn build_department_system_prompt_snapshot_uncached(
    _state: Option<&AppState>,
    conversation: &Conversation,
    agent: &AgentProfile,
    departments: &[DepartmentConfig],
    ui_language: &str,
) -> DepartmentSystemPromptSnapshot {
    let department_prompt_block =
        build_departments_prompt_block(conversation, agent, departments, ui_language);
    let department_tool_rule_blocks = build_system_tools_rule_blocks(agent, departments);
    DepartmentSystemPromptSnapshot {
        department_prompt_block,
        department_tool_rule_blocks,
    }
}

fn get_or_build_department_system_prompt_snapshot(
    state: Option<&AppState>,
    conversation: &Conversation,
    agent: &AgentProfile,
    departments: &[DepartmentConfig],
    ui_language: &str,
) -> DepartmentSystemPromptSnapshot {
    let cache_key = build_department_system_prompt_cache_key(state, agent, ui_language);
    let mut rebuild_reason = "cache_miss";
    {
        let cache = cache_lock_recover(
            "department_system_prompt_cache",
            department_system_prompt_cache(),
        );
        if let Some(entry) = cache.get(&cache_key) {
            if !entry.dirty {
                return entry.snapshot.clone();
            }
            rebuild_reason = "dirty";
        }
    }
    runtime_log_info(format!(
        "[部门提示词] 开始重建 department_id={} reason={}",
        department_id_for_agent(departments, &agent.id),
        rebuild_reason
    ));
    let snapshot = build_department_system_prompt_snapshot_uncached(
        state,
        conversation,
        agent,
        departments,
        ui_language,
    );
    let mut cache = cache_lock_recover(
        "department_system_prompt_cache",
        department_system_prompt_cache(),
    );
    cache.insert(
        cache_key,
        DepartmentSystemPromptCacheEntry {
            agent_id: agent.id.trim().to_string(),
            department_id: department_id_for_agent(departments, &agent.id),
            snapshot: snapshot.clone(),
            dirty: false,
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
    mode_label: &str,
) -> String {
    format!(
        "scope={}|conversation_id={}|mode={}",
        prompt_cache_scope_key(state),
        conversation.id.trim(),
        mode_label.trim(),
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
            "联系人是特殊用户，不是当前聊天窗口中的直接用户。\n他们的消息来自远程接口接入，应视为独立的外部用户。\n不要把联系人和当前用户混为一谈，也不要混淆回复目标。\n如果需要回复远程联系人，必须调用 `remote_im_send`。",
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
            if !entry.dirty {
                return entry.snapshot.clone();
            }
            rebuild_reason = "dirty";
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
            dirty: false,
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

fn classify_system_prompt_extra_block(block: &str) -> SystemPromptExtraBlockGroup {
    let trimmed = block.trim();
    if trimmed.contains("<remote im runtime activation>") {
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

fn build_core_system_prompt_text(
    _conversation: &Conversation,
    agent: &AgentProfile,
    _departments: &[DepartmentConfig],
    user_profile: Option<(&str, &str)>,
    response_style_id: &str,
    ui_language: &str,
    _state: Option<&AppState>,
) -> String {
    let response_style = response_style_preset(response_style_id);
    let date_timezone_line = prompt_current_date_timezone_line(ui_language);
    let highest_instruction_md = highest_instruction_markdown();
    let (
        not_provided_label,
        assistant_settings_label,
        user_settings_label,
        role_constraints_label,
        conversation_style_label,
        language_settings_label,
        user_nickname_label,
        user_intro_label,
        role_identity_line,
        role_confusion_line,
        language_follow_user_line,
        language_instruction,
    ) = (
        "未提供",
        "persona settings",
        "admin user settings",
        "role constraints",
        "conversation style",
        "language settings",
        "用户昵称",
        "用户自我介绍",
        "- 你是“{}”，用户是“{}”。",
        "- 不要把自己当作用户，不要混淆双方身份。",
        "- 若用户明确指定回答语言，以用户指定为准。",
        "默认使用中文回答。",
    );
    if let Some((user_name, user_intro)) = user_profile {
        let user_intro_display = if user_intro.trim().is_empty() {
            not_provided_label.to_string()
        } else {
            user_intro.trim().to_string()
        };
        let role_identity_text = role_identity_line
            .replacen("{}", &xml_escape_prompt(&agent.name), 1)
            .replacen("{}", &xml_escape_prompt(user_name), 1);
        [
            highest_instruction_md.to_string(),
            prompt_xml_block(assistant_settings_label, agent.system_prompt.trim()),
            prompt_xml_block(
                user_settings_label,
                format!(
                    "{}：{}\n{}：{}",
                    user_nickname_label,
                    xml_escape_prompt(user_name),
                    user_intro_label,
                    xml_escape_prompt(&user_intro_display)
                ),
            ),
            prompt_xml_block(
                role_constraints_label,
                format!("{}\n{}", role_identity_text, role_confusion_line),
            ),
            prompt_xml_block(
                conversation_style_label,
                format!("当前风格：{}\n{}", response_style.name, response_style.prompt),
            ),
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
        let delegate_role_line = "- 这是一条委托线程。此线程不存在默认用户人格。";
        let delegate_scope_line =
            "- 只依据本轮委托任务块与本线程历史处理工作，不要自行补充用户设定、昵称或主会话背景。";
        [
            highest_instruction_md.to_string(),
            prompt_xml_block(assistant_settings_label, agent.system_prompt.trim()),
            prompt_xml_block(
                role_constraints_label,
                format!("{}\n{}", delegate_role_line, delegate_scope_line),
            ),
            prompt_xml_block(
                conversation_style_label,
                format!("当前风格：{}\n{}", response_style.name, response_style.prompt),
            ),
            prompt_xml_block(
                language_settings_label,
                format!("{}\n{}", language_instruction, date_timezone_line),
            ),
        ]
        .join("\n")
    }
}

fn build_system_prompt_ordered_blocks(
    state: Option<&AppState>,
    mode_label: &str,
    conversation: &Conversation,
    agent: &AgentProfile,
    departments: &[DepartmentConfig],
    ui_language: &str,
    _selected_api: Option<&ApiConfig>,
    fixed_system_prompt_text: &str,
    user_profile_memory_block: Option<&str>,
    terminal_block: Option<&str>,
    system_preamble_blocks: &[String],
) -> Vec<String> {
    let department_snapshot = get_or_build_department_system_prompt_snapshot(
        state,
        conversation,
        agent,
        departments,
        ui_language,
    );
    let department_config = departments_only_config(departments);
    let current_department = department_for_agent_id(&department_config, &agent.id);
    let mut tool_rule_blocks = Vec::<String>::new();
    if ["remember", "recall"]
        .into_iter()
        .any(|tool_id| department_builtin_tool_enabled(current_department, tool_id))
    {
        tool_rule_blocks.push(build_memory_rag_rule_block());
    }
    tool_rule_blocks.extend(department_snapshot.department_tool_rule_blocks.iter().cloned());
    if department_builtin_tool_enabled(current_department, "plan") {
        tool_rule_blocks.push(build_question_and_planning_rule_block());
    }
    if department_builtin_tool_enabled(current_department, "meme") {
        if let Some(meme_block) = meme_prompt_rule_block(state).as_deref() {
            tool_rule_blocks.push(meme_block.trim().to_string());
        }
    }

    let (tool_rule_extra_blocks, runtime_extra_blocks, im_extra_blocks) =
        split_system_preamble_blocks(system_preamble_blocks);
    tool_rule_blocks.extend(tool_rule_extra_blocks);

    let environment_snapshot = get_or_build_conversation_environment_prompt_snapshot(
        state,
        conversation,
        mode_label,
        terminal_block,
        &runtime_extra_blocks,
        &im_extra_blocks,
    );

    let mut ordered_blocks = Vec::<String>::new();
    if !fixed_system_prompt_text.trim().is_empty() {
        ordered_blocks.push(fixed_system_prompt_text.trim().to_string());
    }
    if !department_snapshot.department_prompt_block.trim().is_empty() {
        ordered_blocks.push(department_snapshot.department_prompt_block.trim().to_string());
    }
    ordered_blocks.extend(
        tool_rule_blocks
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    );
    if let Some(profile_block) = user_profile_memory_block
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        ordered_blocks.push(profile_block.to_string());
    }
    ordered_blocks.extend(
        environment_snapshot
            .runtime_blocks
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    );
    ordered_blocks.extend(
        environment_snapshot
            .im_rule_blocks
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    );
    ordered_blocks
}

fn build_system_prompt_text_uncached(ordered_blocks: &[String]) -> String {
    let mut prompt = String::new();
    for block in ordered_blocks {
        append_system_prompt_block(&mut prompt, Some(block));
    }
    prompt
}

fn finalize_system_prompt_with_manager(
    state: Option<&AppState>,
    mode_label: &str,
    conversation: &Conversation,
    agent: &AgentProfile,
    departments: &[DepartmentConfig],
    selected_api: Option<&ApiConfig>,
    _user_profile: Option<(&str, &str)>,
    _response_style_id: &str,
    ui_language: &str,
    fixed_system_prompt_text: &str,
    user_profile_memory_block: Option<&str>,
    terminal_block: Option<&str>,
    system_preamble_blocks: &[String],
    stage_logger: Option<&dyn Fn(&str)>,
) -> String {
    let final_cache_key = format!(
        "scope={}|conversation_id={}|mode={}|agent={}",
        prompt_cache_scope_key(state),
        conversation.id.trim(),
        mode_label.trim(),
        agent.id.trim(),
    );
    let mut rebuild_reason = "cache_miss";
    {
        let cache = cache_lock_recover("system_prompt_text_cache", system_prompt_text_cache());
        if let Some(entry) = cache.get(&final_cache_key) {
            if !entry.dirty {
                if let Some(log_stage) = stage_logger {
                    log_stage("prepare_context.prompt_system_cache_hit");
                }
                return entry.text.clone();
            }
            rebuild_reason = "dirty";
        }
    }
    let department_id = department_id_for_agent(departments, &agent.id);
    runtime_log_info(format!(
        "[系统提示词] 开始重建 conversation_id={} agent_id={} department_id={} reason={}",
        conversation.id.trim(),
        agent.id.trim(),
        department_id,
        rebuild_reason
    ));
    let ordered_blocks = build_system_prompt_ordered_blocks(
        state,
        mode_label,
        conversation,
        agent,
        departments,
        ui_language,
        selected_api,
        fixed_system_prompt_text,
        user_profile_memory_block,
        terminal_block,
        system_preamble_blocks,
    );
    let prompt_text = build_system_prompt_text_uncached(&ordered_blocks);
    let mut cache = cache_lock_recover("system_prompt_text_cache", system_prompt_text_cache());
    cache.insert(
        final_cache_key,
        FinalSystemPromptCacheEntry {
            conversation_id: conversation.id.trim().to_string(),
            agent_id: agent.id.trim().to_string(),
            department_id,
            text: prompt_text.clone(),
            dirty: false,
        },
    );
    if let Some(log_stage) = stage_logger {
        log_stage("prepare_context.prompt_system_cache_rebuilt");
    }
    prompt_text
}

fn mark_prompt_cache_rebuild_internal(
    state: &AppState,
    department_ids: &[String],
    agent_ids: &[String],
    conversation_ids: &[String],
    mark_department: bool,
    mark_environment: bool,
    mark_final: bool,
) {
    let scope_prefix = format!("scope={}|", prompt_cache_scope_key(Some(state)));
    let department_ids = department_ids
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<std::collections::HashSet<_>>();
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
    let mark_all = department_ids.is_empty() && agent_ids.is_empty() && conversation_ids.is_empty();

    let mut department_marked = 0usize;
    if mark_department {
        let mut cache = cache_lock_recover(
            "department_system_prompt_cache",
            department_system_prompt_cache(),
        );
        for (key, entry) in cache.iter_mut() {
            if !key.starts_with(&scope_prefix) {
                continue;
            }
            let matched = mark_all
                || agent_ids.contains(entry.agent_id.trim())
                || department_ids.contains(entry.department_id.trim());
            if matched && !entry.dirty {
                entry.dirty = true;
                department_marked += 1;
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
            if matched && !entry.dirty {
                entry.dirty = true;
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
                || agent_ids.contains(entry.agent_id.trim())
                || department_ids.contains(entry.department_id.trim());
            if matched && !entry.dirty {
                entry.dirty = true;
                final_marked += 1;
            }
        }
    }

    runtime_log_debug(format!(
        "[系统提示词] 标记重建 完成 department_ids={:?} agent_ids={:?} conversation_ids={:?} department_marked={} environment_marked={} final_marked={}",
        department_ids,
        agent_ids,
        conversation_ids,
        department_marked,
        environment_marked,
        final_marked
    ));
}

fn mark_prompt_cache_rebuild_for_departments(state: &AppState, department_ids: &[String]) {
    mark_prompt_cache_rebuild_internal(state, department_ids, &[], &[], true, false, true);
}

fn mark_prompt_cache_rebuild_for_agents(state: &AppState, agent_ids: &[String]) {
    mark_prompt_cache_rebuild_internal(state, &[], agent_ids, &[], false, false, true);
}

fn mark_prompt_cache_rebuild_for_conversation_environment(
    state: &AppState,
    conversation_id: &str,
) {
    mark_prompt_cache_rebuild_internal(
        state,
        &[],
        &[],
        &[conversation_id.trim().to_string()],
        false,
        true,
        true,
    );
}

fn mark_prompt_cache_rebuild_for_all_environments(state: &AppState) {
    mark_prompt_cache_rebuild_internal(state, &[], &[], &[], false, true, true);
}

fn mark_prompt_cache_rebuild_for_all_final(state: &AppState) {
    mark_prompt_cache_rebuild_internal(state, &[], &[], &[], false, false, true);
}
