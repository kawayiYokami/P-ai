#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptBuildMode {
    Chat,
    Delegate,
    SummaryContext,
}

#[derive(Debug, Clone, Default)]
struct ChatPromptOverrides {
    latest_user_intent: Option<LatestUserPayloadIntent>,
    // 会话主链不允许外部直接注入系统侧块；系统提示词相关块必须由提示词服务内部生成。
    todo_tool_enabled: bool,
    remote_im_activation_sources: Vec<RemoteImActivationSource>,
    latest_images: Option<Vec<PreparedBinaryPayload>>,
    latest_audios: Option<Vec<PreparedBinaryPayload>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum LatestUserPayloadIntent {
    ChatRequest {
        trigger_only: bool,
        submitted_user_text: String,
        include_task_board: bool,
        include_todo_board: bool,
        attachment_relative_paths: Vec<String>,
    },
    SummaryContext {
        scene: SummaryContextScene,
        user_alias: String,
        current_user_profile: String,
        include_todo_block: bool,
    },
    Explicit {
        text: String,
        meta_text: String,
        extra_blocks: Vec<String>,
    },
}

pub(crate) fn prompt_xml_block(block_name: &str, body: impl AsRef<str>) -> String {
    let name = block_name.trim();
    let content = body.as_ref().trim();
    if name.is_empty() || content.is_empty() {
        return String::new();
    }
    format!("<{}>\n{}\n</{}>", name, content, name)
}

fn build_prepared_prompt_for_mode(
    mode: PromptBuildMode,
    conversation: &Conversation,
    agent: &AgentProfile,
    agents: &[AgentProfile],
    departments: &[DepartmentConfig],
    user_name: &str,
    user_intro: &str,
    response_style_id: &str,
    ui_language: &str,
    data_path: Option<&PathBuf>,
    _last_archive_summary: Option<&str>,
    terminal_block_override: Option<String>,
    chat_overrides: Option<ChatPromptOverrides>,
    state: Option<&AppState>,
    selected_api: Option<&ApiConfig>,
    resolved_api: Option<&ResolvedApiConfig>,
    enable_pdf_images: Option<bool>,
) -> PreparedPrompt {
    build_prepared_prompt_for_mode_with_stage_logger(
        mode,
        conversation,
        agent,
        agents,
        departments,
        user_name,
        user_intro,
        response_style_id,
        ui_language,
        data_path,
        _last_archive_summary,
        terminal_block_override,
        chat_overrides,
        state,
        None,
        selected_api,
        resolved_api,
        enable_pdf_images,
    )
}

fn build_prepared_prompt_for_mode_with_stage_logger(
    mode: PromptBuildMode,
    conversation: &Conversation,
    agent: &AgentProfile,
    agents: &[AgentProfile],
    departments: &[DepartmentConfig],
    user_name: &str,
    user_intro: &str,
    response_style_id: &str,
    ui_language: &str,
    data_path: Option<&PathBuf>,
    _last_archive_summary: Option<&str>,
    terminal_block_override: Option<String>,
    chat_overrides: Option<ChatPromptOverrides>,
    state: Option<&AppState>,
    stage_logger: Option<&dyn Fn(&str)>,
    selected_api: Option<&ApiConfig>,
    resolved_api: Option<&ResolvedApiConfig>,
    enable_pdf_images: Option<bool>,
) -> PreparedPrompt {
    conversation_prompt_service().build_prepared_prompt_for_mode(
        mode,
        conversation,
        agent,
        agents,
        departments,
        user_name,
        user_intro,
        response_style_id,
        ui_language,
        data_path,
        _last_archive_summary,
        terminal_block_override,
        chat_overrides,
        state,
        stage_logger,
        selected_api,
        resolved_api,
        enable_pdf_images,
    )
}

fn render_user_profile_memory_block(
    memories: &[MemoryEntry],
    block_name: &str,
    intro: &str,
) -> Option<String> {
    if memories.is_empty() {
        return None;
    }
    let mut lines = vec![intro.to_string()];
    for memory in memories {
        let display_id = memory.display_id();
        lines.push(format!(
            "<{}>\n类型：{}\n判断：{}\n理由：{}\n标签：{}\n</{}>",
            display_id,
            memory.memory_type,
            clean_text(memory.judgment.trim()),
            clean_text(memory.reasoning.trim()),
            memory.tags.join("、"),
            display_id
        ));
    }
    let block = prompt_xml_block(block_name, lines.join("\n"));
    if block.trim().is_empty() {
        None
    } else {
        Some(block)
    }
}

fn build_user_profile_snapshot_block(
    data_path: &PathBuf,
    agent: &AgentProfile,
    limit: usize,
) -> Result<Option<String>, String> {
    let memories = memory_store_list_profile_memories_visible_for_agent(
        data_path,
        &agent.id,
        agent.private_memory_enabled,
        limit,
    )?;
    Ok(render_user_profile_memory_block(
        &memories,
        "user profile snapshot",
        "以下内容是用户画像快照，不是本轮即时上下文。请把它们视为用户长期稳定背景。",
    ))
}

fn build_user_profile_memory_board(
    data_path: &PathBuf,
    agent: &AgentProfile,
) -> Result<Option<String>, String> {
    let memories = memory_store_list_profile_memories_visible_for_agent(
        data_path,
        &agent.id,
        agent.private_memory_enabled,
        0,
    )?;
    Ok(render_user_profile_memory_block(
        &memories,
        "user profile memory board",
        "以下内容是完整用户画像记忆（含ID），用于合并、纠错和去除过期画像记忆。",
    ))
}

fn conversation_user_main_workspace_root(conversation: &Conversation, state: &AppState) -> Option<PathBuf> {
    conversation
        .shell_workspaces
        .iter()
        .find(|workspace| {
            !workspace.built_in
                && normalize_shell_workspace_level_text(&workspace.level) == SHELL_WORKSPACE_LEVEL_MAIN
        })
        .and_then(|workspace| shell_workspace_resolve_path_candidate(state, workspace))
}

const WORKSPACE_AGENTS_MD_MAX_BYTES: u64 = 32 * 1024;

fn build_workspace_agents_md_block(conversation: &Conversation, state: &AppState) -> Option<String> {
    let Some(workspace_root) = conversation_user_main_workspace_root(conversation, state) else {
        eprintln!("[AGENTS注入] 跳过 main_workspace=（无） reason=未命中用户指定main工作目录");
        return None;
    };
    let agents_path = workspace_root.join("AGENTS.md");
    if !agents_path.is_file() {
        eprintln!(
            "[AGENTS注入] 跳过 main_workspace={} reason=根目录缺少AGENTS.md",
            workspace_root.display()
        );
        return None;
    }
    let agents_metadata = match std::fs::metadata(&agents_path) {
        Ok(metadata) => metadata,
        Err(err) => {
            eprintln!(
                "[AGENTS注入] 失败 main_workspace={} path={} reason=读取AGENTS.md元数据失败 error={err:?}",
                workspace_root.display(),
                agents_path.display()
            );
            return None;
        }
    };
    if agents_metadata.len() > WORKSPACE_AGENTS_MD_MAX_BYTES {
        eprintln!(
            "[AGENTS注入] 跳过 main_workspace={} path={} reason=AGENTS.md超过大小上限 size_bytes={} max_bytes={}",
            workspace_root.display(),
            agents_path.display(),
            agents_metadata.len(),
            WORKSPACE_AGENTS_MD_MAX_BYTES
        );
        return None;
    }
    match std::fs::read_to_string(&agents_path) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                eprintln!(
                    "[AGENTS注入] 跳过 main_workspace={} reason=AGENTS.md为空",
                    workspace_root.display()
                );
                return None;
            }
            eprintln!(
                "[AGENTS注入] 完成 main_workspace={} chars={}",
                workspace_root.display(),
                trimmed.chars().count()
            );
            Some(prompt_xml_block(
                "workspace agents",
                format!(
                    "优先级：安全与硬边界 ＞ 用户指令 ＞ 当前工作目录的 AGENTS.md ＞ 预设系统提示词 ＞ 常识与默认习惯。\n\n当前工作目录的 AGENTS.md 视为该工作上下文的局部高优先级提示词，用于覆盖助理预设的通用系统提示词、默认流程与常规输出习惯；当用户指令未明确覆盖时，应优先遵守 AGENTS.md。\n\n以下内容来自当前主工作目录根下的 AGENTS.md，请将其视为该项目开发准则。\n\n路径：{}\n\n{}",
                    agents_path.display(),
                    trimmed
                ),
            ))
        }
        Err(err) => {
            eprintln!(
                "[AGENTS注入] 失败 main_workspace={} path={} error={err}",
                workspace_root.display(),
                agents_path.display()
            );
            None
        }
    }
}

#[allow(dead_code)]
fn enrich_prepared_prompt_with_common_preamble(
    mut prepared: PreparedPrompt,
    _last_archive_summary: Option<&str>,
    user_profile_memory_block: Option<&str>,
    terminal_block: Option<String>,
) -> PreparedPrompt {
    if let Some(block) = user_profile_memory_block {
        let trimmed = block.trim();
        if !trimmed.is_empty() {
            prepared.preamble.push('\n');
            prepared.preamble.push_str(trimmed);
            prepared.preamble.push('\n');
        }
    }
    if let Some(block) = terminal_block {
        if !block.trim().is_empty() {
            prepared.preamble.push('\n');
            prepared.preamble.push_str(&block);
            prepared.preamble.push('\n');
        }
    }
    prepared
}

fn apply_chat_latest_user_payload(
    prepared: &mut PreparedPrompt,
    latest_user_text: String,
    latest_user_meta_text: String,
    latest_user_extra_blocks: &[String],
    latest_images: Option<Vec<PreparedBinaryPayload>>,
    latest_audios: Option<Vec<PreparedBinaryPayload>>,
) {
    prepared.latest_user_text = latest_user_text;
    prepared.latest_user_meta_text = latest_user_meta_text;
    prepared_prompt_append_latest_user_extra_blocks(prepared, latest_user_extra_blocks);
    if let Some(images) = latest_images {
        prepared.latest_images = images;
    }
    if let Some(audios) = latest_audios {
        prepared.latest_audios = audios;
    }
}

#[cfg(test)]
mod prompt_assembly_tests {
    use super::*;
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::sync::{Arc, Mutex};

    fn temp_data_path(name: &str) -> PathBuf {
        let root = std::env::temp_dir()
            .join("easy_call_ai_tests")
            .join(format!("{}_{}", name, Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create temp dir");
        root.join("app_data.json")
    }

    #[test]
    fn user_profile_memory_board_should_mark_block_as_profile() {
        let data_path = temp_data_path("prompt_profile_board");
        let drafts = vec![MemoryDraftInput {
            memory_type: "knowledge".to_string(),
            judgment: "用户当前长期在深圳生活".to_string(),
            reasoning: "本轮明确说明".to_string(),
            tags: vec!["深圳".to_string(), "居住地".to_string()],
            owner_agent_id: None,
        }];
        let (saved, _) = memory_store_upsert_drafts(&data_path, &drafts).expect("seed memories");
        let memory_id = saved[0].id.clone().expect("memory id");
        memory_store_upsert_profile_memory_links(&data_path, &vec![memory_id], "auto")
            .expect("link profile memory");

        let block = build_user_profile_memory_board(&data_path, &default_agent())
            .expect("build profile board")
            .expect("profile board should exist");
        assert!(block.contains("用户画像记忆"));
        assert!(block.contains("用户当前长期在深圳生活"));
    }

    #[test]
    fn common_preamble_should_allow_profile_without_archive_summary() {
        let prepared = PreparedPrompt {
            preamble: "系统前言".to_string(),
            history_messages: Vec::new(),
            latest_user_text: String::new(),
            latest_user_meta_text: String::new(),
            latest_user_extra_text: String::new(),
            latest_user_extra_blocks: Vec::new(),
            latest_images: Vec::new(),
            latest_audios: Vec::new(),
        };
        let enriched = enrich_prepared_prompt_with_common_preamble(
            prepared,
            None,
            Some("<user profile memory board>\n用户画像记忆\n</user profile memory board>"),
            None,
        );
        assert!(enriched.preamble.contains("用户画像记忆"));
        assert!(!enriched.preamble.contains("上次我们聊到哪里"));
    }

    fn build_test_state(llm_workspace_path: PathBuf) -> AppState {
        let terminal_shell = detect_default_terminal_shell();
        AppState {
            app_handle: Arc::new(Mutex::new(None)),
            config_path: llm_workspace_path.join("app_config.toml"),
            data_path: llm_workspace_path.join("app_data.json"),
            llm_workspace_path,
            shared_http_client: reqwest::Client::new(),
            terminal_shell: terminal_shell.clone(),
            terminal_shell_candidates: vec![terminal_shell],
            conversation_lock: Arc::new(ConversationDomainLock::new()),
            memory_lock: Arc::new(Mutex::new(())),
            cached_config: Arc::new(Mutex::new(None)),
            cached_config_mtime: Arc::new(Mutex::new(None)),
            cached_agents: Arc::new(Mutex::new(None)),
            cached_agents_mtime: Arc::new(Mutex::new(None)),
            cached_runtime_state: Arc::new(Mutex::new(None)),
            cached_runtime_state_mtime: Arc::new(Mutex::new(None)),
            cached_chat_index: Arc::new(Mutex::new(None)),
            cached_chat_index_mtime: Arc::new(Mutex::new(None)),
            cached_conversations: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_conversation_mtimes: Arc::new(Mutex::new(std::collections::HashMap::new())),
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_signature: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_pending: Arc::new(Mutex::new(None)),
            app_data_persist_notify: Arc::new(tokio::sync::Notify::new()),
            app_data_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            conversation_persist_pending: Arc::new(Mutex::new(None)),
            conversation_persist_notify: Arc::new(tokio::sync::Notify::new()),
            conversation_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            conversation_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            cached_conversation_dirty_ids: Arc::new(Mutex::new(HashSet::new())),
            cached_deleted_conversation_ids: Arc::new(Mutex::new(HashSet::new())),
            cached_chat_index_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_write_lock: Arc::new(Mutex::new(())),
            last_panic_snapshot: Arc::new(Mutex::new(None)),
            inflight_chat_abort_handles: Arc::new(Mutex::new(HashMap::new())),
            inflight_tool_abort_handles: Arc::new(Mutex::new(HashMap::new())),
            inflight_completed_tool_history: Arc::new(Mutex::new(HashMap::new())),
            terminal_session_roots: Arc::new(Mutex::new(HashMap::new())),
            terminal_live_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            terminal_pending_approvals: Arc::new(Mutex::new(HashMap::new())),
            llm_round_logs: Arc::new(Mutex::new(VecDeque::new())),
            conversation_runtime_slots: Arc::new(Mutex::new(HashMap::new())),
            conversation_processing_claims: Arc::new(Mutex::new(HashSet::new())),
            pending_chat_result_senders: Arc::new(Mutex::new(HashMap::new())),
            pending_chat_delta_channels: Arc::new(Mutex::new(HashMap::new())),
            active_chat_view_bindings: Arc::new(Mutex::new(HashMap::new())),
            dequeue_lock: Arc::new(Mutex::new(())),
            delegate_runtime_threads: Arc::new(Mutex::new(HashMap::new())),
            delegate_recent_threads: Arc::new(Mutex::new(VecDeque::new())),
            provider_streaming_disabled_keys: Arc::new(Mutex::new(HashMap::new())),
            provider_system_message_user_fallback_keys: Arc::new(Mutex::new(HashSet::new())),
            provider_request_gates: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            conversation_index_repair_gates: Arc::new(Mutex::new(HashMap::new())),
            remote_im_contact_runtime_states: Arc::new(Mutex::new(HashMap::new())),
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new(String::new())),
            migration_preview_dirs: Arc::new(Mutex::new(HashMap::new())),
            delegate_active_ids: Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        }
    }

    fn build_test_conversation(workspaces: Vec<ShellWorkspaceConfig>) -> Conversation {
        Conversation {
            id: "conv-1".to_string(),
            title: "test".to_string(),
            agent_id: "assistant".to_string(),
            department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
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
            summary: String::new(),
            user_profile_snapshot: String::new(),
            shell_workspace_path: None,
            shell_workspaces: workspaces,
            archived_at: None,
            messages: Vec::new(),
            current_todos: Vec::new(),
            memory_recall_table: Vec::new(),
            plan_mode_enabled: false,
        }
    }

    fn build_test_message(role: &str, text: &str) -> ChatMessage {
        ChatMessage {
            id: Uuid::new_v4().to_string(),
            role: role.to_string(),
            created_at: now_iso(),
            speaker_agent_id: None,
            parts: vec![MessagePart::Text {
                text: text.to_string(),
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: None,
            mcp_call: None,
        }
    }

    #[test]
    fn build_workspace_agents_md_block_should_inject_user_main_workspace_agents() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-prompt-assembly-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("llm-workspace");
        let project_root = temp_root.join("project-main");
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        fs::create_dir_all(&project_root).expect("create project root");
        fs::write(
            project_root.join("AGENTS.md"),
            "# AGENTS.md\n\n- use pnpm\n- run tests",
        )
        .expect("write agents");
        let state = build_test_state(llm_workspace_path);
        let conversation = build_test_conversation(vec![ShellWorkspaceConfig {
            id: "main-1".to_string(),
            name: "project".to_string(),
            path: terminal_path_for_user(&project_root),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: false,
        }]);

        let block = build_workspace_agents_md_block(&conversation, &state).expect("agents block");

        assert!(block.contains("当前主工作目录根下的 AGENTS.md"));
        assert!(block.contains("use pnpm"));
        assert!(block.contains("run tests"));
    }

    #[test]
    fn build_workspace_agents_md_block_should_skip_oversized_agents_md() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-prompt-assembly-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("llm-workspace");
        let project_root = temp_root.join("project-main-large-agents");
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        fs::create_dir_all(&project_root).expect("create project root");
        fs::write(
            project_root.join("AGENTS.md"),
            "a".repeat((WORKSPACE_AGENTS_MD_MAX_BYTES + 1) as usize),
        )
        .expect("write oversized agents");
        let state = build_test_state(llm_workspace_path);
        let conversation = build_test_conversation(vec![ShellWorkspaceConfig {
            id: "main-1".to_string(),
            name: "project".to_string(),
            path: terminal_path_for_user(&project_root),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: false,
        }]);

        assert!(build_workspace_agents_md_block(&conversation, &state).is_none());
    }

    #[test]
    fn build_workspace_agents_md_block_should_skip_built_in_main_workspace() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-prompt-assembly-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("llm-workspace");
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        fs::write(llm_workspace_path.join("AGENTS.md"), "# AGENTS.md\n\nshould skip")
            .expect("write agents");
        let state = build_test_state(llm_workspace_path.clone());
        let conversation = build_test_conversation(vec![ShellWorkspaceConfig {
            id: "system-workspace".to_string(),
            name: "派蒙的家".to_string(),
            path: terminal_path_for_user(&llm_workspace_path),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: true,
        }]);

        assert!(build_workspace_agents_md_block(&conversation, &state).is_none());
    }

    #[test]
    fn build_workspace_agents_md_block_should_skip_when_only_secondary_workspace_exists() {
        let temp_root = std::env::temp_dir().join(format!(
            "easy-call-ai-prompt-assembly-test-{}",
            uuid::Uuid::new_v4()
        ));
        let llm_workspace_path = temp_root.join("llm-workspace");
        let project_root = temp_root.join("project-secondary");
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        fs::create_dir_all(&project_root).expect("create project root");
        fs::write(project_root.join("AGENTS.md"), "# AGENTS.md\n\nshould skip")
            .expect("write agents");
        let state = build_test_state(llm_workspace_path);
        let conversation = build_test_conversation(vec![ShellWorkspaceConfig {
            id: "secondary-1".to_string(),
            name: "project".to_string(),
            path: terminal_path_for_user(&project_root),
            level: SHELL_WORKSPACE_LEVEL_SECONDARY.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: false,
        }]);

        assert!(build_workspace_agents_md_block(&conversation, &state).is_none());
    }

    #[test]
    fn build_prepared_prompt_for_mode_should_keep_system_and_conversation_sides_separated() {
        let agent = default_agent();
        let agents = vec![agent.clone()];
        let departments = default_departments("api-1");
        let mut conversation = build_test_conversation(Vec::new());
        conversation.messages.push(build_test_message("user", "这一句只属于用户消息"));

        let prepared = build_prepared_prompt_for_mode(
            PromptBuildMode::Chat,
            &conversation,
            &agent,
            &agents,
            &departments,
            "测试用户",
            "",
            "default",
            "zh-CN",
            None,
            None,
            Some("<terminal env>\n当前 shell: PowerShell\n</terminal env>".to_string()),
            None,
            None,
            Some(&ApiConfig::default()),
            None,
            Some(false),
        );

        assert!(prepared.preamble.contains("当前 shell: PowerShell"));
        assert!(!prepared.preamble.contains("这一句只属于用户消息"));
        assert_eq!(prepared.latest_user_text, "这一句只属于用户消息");
    }

    #[test]
    fn finalize_system_prompt_with_manager_should_reuse_cached_department_blocks() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-prompt-manager-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let agent = default_agent();
        let departments = default_departments("api-1");
        let conversation = build_test_conversation(Vec::new());
        let system_blocks = vec!["<runtime block>\n测试块\n</runtime block>".to_string()];
        let cache_key = build_department_system_prompt_cache_key(
            Some(&state),
            &agent,
            "zh-CN",
        );
        {
            let mut cache = cache_lock_recover(
                "department_system_prompt_cache",
                department_system_prompt_cache(),
            );
            cache.remove(&cache_key);
        }

        let first = finalize_system_prompt_with_manager(
            Some(&state),
            "chat",
            &conversation,
            &agent,
            &departments,
            Some(&ApiConfig::default()),
            Some(("测试用户", "")),
            "default",
            "zh-CN",
            "核心 system prompt",
            None,
            Some("<terminal env>\n当前 shell: PowerShell\n</terminal env>"),
            &system_blocks,
            None,
        );
        let second = finalize_system_prompt_with_manager(
            Some(&state),
            "chat",
            &conversation,
            &agent,
            &departments,
            Some(&ApiConfig::default()),
            Some(("测试用户", "")),
            "default",
            "zh-CN",
            "核心 system prompt",
            None,
            Some("<terminal env>\n当前 shell: PowerShell\n</terminal env>"),
            &system_blocks,
            None,
        );

        assert_eq!(first, second);
        assert!(first.contains("核心 system prompt"));
        assert!(first.contains("当前 shell: PowerShell"));
        let cache = cache_lock_recover(
            "department_system_prompt_cache",
            department_system_prompt_cache(),
        );
        assert!(cache.contains_key(&cache_key));
    }

    #[test]
    fn conversation_environment_prompt_snapshot_should_hit_cache_for_same_signature() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-environment-cache-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let conversation = build_test_conversation(Vec::new());
        let terminal_block = "<shell workspace>\n运行环境块\n</shell workspace>";
        let runtime_blocks = vec!["<workspace agents>\n项目约束块\n</workspace agents>".to_string()];
        let im_blocks = vec!["<remote im runtime activation>\nIM 激活块\n</remote im runtime activation>".to_string()];

        let cache_key = build_conversation_environment_prompt_cache_key(
            Some(&state),
            &conversation,
            "chat",
        );
        {
            let mut cache = cache_lock_recover(
                "conversation_environment_prompt_cache",
                conversation_environment_prompt_cache(),
            );
            cache.remove(&cache_key);
        }

        let first = get_or_build_conversation_environment_prompt_snapshot(
            Some(&state),
            &conversation,
            "chat",
            Some(terminal_block),
            &runtime_blocks,
            &im_blocks,
        );
        let second = get_or_build_conversation_environment_prompt_snapshot(
            Some(&state),
            &conversation,
            "chat",
            Some(terminal_block),
            &runtime_blocks,
            &im_blocks,
        );

        assert_eq!(first.runtime_blocks, second.runtime_blocks);
        assert_eq!(first.im_rule_blocks, second.im_rule_blocks);
        let cache = cache_lock_recover(
            "conversation_environment_prompt_cache",
            conversation_environment_prompt_cache(),
        );
        assert!(cache.contains_key(&cache_key));
    }

    #[test]
    fn conversation_environment_prompt_snapshot_should_reuse_cache_across_modes() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-environment-cross-mode-cache-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let conversation = build_test_conversation(Vec::new());
        let terminal_block = "<shell workspace>\n运行环境块\n</shell workspace>";
        let runtime_blocks = vec!["<workspace agents>\n项目约束块\n</workspace agents>".to_string()];
        let im_blocks = vec!["<remote im runtime activation>\nIM 激活块\n</remote im runtime activation>".to_string()];

        let cache_key = build_conversation_environment_prompt_cache_key(
            Some(&state),
            &conversation,
            "chat",
        );
        {
            let mut cache = cache_lock_recover(
                "conversation_environment_prompt_cache",
                conversation_environment_prompt_cache(),
            );
            cache.clear();
        }

        let first = get_or_build_conversation_environment_prompt_snapshot(
            Some(&state),
            &conversation,
            "chat",
            Some(terminal_block),
            &runtime_blocks,
            &im_blocks,
        );
        let second = get_or_build_conversation_environment_prompt_snapshot(
            Some(&state),
            &conversation,
            "summary_context",
            Some(terminal_block),
            &runtime_blocks,
            &im_blocks,
        );

        assert_eq!(first.runtime_blocks, second.runtime_blocks);
        assert_eq!(first.im_rule_blocks, second.im_rule_blocks);
        let cache = cache_lock_recover(
            "conversation_environment_prompt_cache",
            conversation_environment_prompt_cache(),
        );
        assert_eq!(cache.len(), 1);
        assert!(cache.contains_key(&cache_key));
    }

    #[test]
    fn finalize_system_prompt_with_manager_should_reuse_cache_across_modes() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-final-system-cross-mode-cache-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let agent = default_agent();
        let departments = default_departments("api-1");
        let conversation = build_test_conversation(Vec::new());
        let terminal_block = Some("<terminal env>\n当前 shell: PowerShell\n</terminal env>");
        let system_blocks = vec!["<workspace agents>\n项目约束块\n</workspace agents>".to_string()];
        let cache_key = format!(
            "scope={}|conversation_id={}|agent={}",
            prompt_cache_scope_key(Some(&state)),
            conversation.id.trim(),
            agent.id.trim(),
        );
        {
            let mut cache = cache_lock_recover("system_prompt_text_cache", system_prompt_text_cache());
            cache.clear();
        }

        let first = finalize_system_prompt_with_manager(
            Some(&state),
            "chat",
            &conversation,
            &agent,
            &departments,
            Some(&ApiConfig::default()),
            Some(("测试用户", "")),
            "default",
            "zh-CN",
            "核心 system prompt",
            None,
            terminal_block,
            &system_blocks,
            None,
        );
        let second = finalize_system_prompt_with_manager(
            Some(&state),
            "summary_context",
            &conversation,
            &agent,
            &departments,
            Some(&ApiConfig::default()),
            Some(("测试用户", "")),
            "default",
            "zh-CN",
            "核心 system prompt",
            None,
            terminal_block,
            &system_blocks,
            None,
        );

        assert_eq!(first, second);
        let cache = cache_lock_recover("system_prompt_text_cache", system_prompt_text_cache());
        assert_eq!(cache.len(), 1);
        assert!(cache.contains_key(&cache_key));
    }

    #[test]
    fn conversation_environment_prompt_snapshot_should_rebuild_after_environment_invalidation() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-environment-rebuild-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let conversation = build_test_conversation(Vec::new());
        let mut remote_im_conversation = conversation.clone();
        remote_im_conversation.conversation_kind = CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string();
        let terminal_block = "<shell workspace>\n运行环境块\n</shell workspace>";
        let flatten_snapshot = |snapshot: &ConversationEnvironmentPromptSnapshot| {
            snapshot
                .runtime_blocks
                .iter()
                .chain(snapshot.im_rule_blocks.iter())
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        };

        let normal = get_or_build_conversation_environment_prompt_snapshot(
            Some(&state),
            &conversation,
            "chat",
            Some(terminal_block),
            &[],
            &[],
        );
        mark_prompt_cache_rebuild_for_system_environment_by_conversation(
            &state,
            &conversation.id,
        );
        let remote = get_or_build_conversation_environment_prompt_snapshot(
            Some(&state),
            &remote_im_conversation,
            "chat",
            Some(terminal_block),
            &[],
            &[],
        );

        let normal_text = flatten_snapshot(&normal);
        let remote_text = flatten_snapshot(&remote);
        assert_ne!(normal_text, remote_text);
        assert!(!normal_text.contains("联系人是特殊用户"));
        assert!(remote_text.contains("联系人是特殊用户"));
    }

    #[test]
    fn system_source_invalidation_should_not_dirty_conversation_environment_cache() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-system-source-dirty-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let agent = default_agent();
        let conversation = build_test_conversation(Vec::new());
        let cache_key = build_conversation_environment_prompt_cache_key(
            Some(&state),
            &conversation,
            "chat",
        );
        let _ = get_or_build_conversation_environment_prompt_snapshot(
            Some(&state),
            &conversation,
            "chat",
            Some("<shell workspace>\n运行环境块\n</shell workspace>"),
            &[],
            &[],
        );

        mark_prompt_cache_rebuild_for_system_sources_by_agents(&state, &[agent.id.clone()]);

        let cache = cache_lock_recover(
            "conversation_environment_prompt_cache",
            conversation_environment_prompt_cache(),
        );
        let entry = cache.get(&cache_key).expect("environment cache entry");
        assert!(entry.dirty_reason.is_none());
    }

    #[test]
    fn system_environment_invalidation_should_not_dirty_department_cache() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-system-environment-dirty-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let agent = default_agent();
        let departments = default_departments("api-1");
        let conversation = build_test_conversation(Vec::new());
        let cache_key = build_department_system_prompt_cache_key(
            Some(&state),
            &agent,
            "zh-CN",
        );
        let _ = finalize_system_prompt_with_manager(
            Some(&state),
            "chat",
            &conversation,
            &agent,
            &departments,
            Some(&ApiConfig::default()),
            Some(("测试用户", "")),
            "default",
            "zh-CN",
            "核心 system prompt",
            None,
            Some("<terminal env>\n当前 shell: PowerShell\n</terminal env>"),
            &[],
            None,
        );

        mark_prompt_cache_rebuild_for_system_environment_by_conversation(
            &state,
            &conversation.id,
        );

        let cache = cache_lock_recover(
            "department_system_prompt_cache",
            department_system_prompt_cache(),
        );
        let entry = cache.get(&cache_key).expect("department cache entry");
        assert!(entry.dirty_reason.is_none());
    }

    #[test]
    fn finalize_system_prompt_with_manager_should_keep_tool_runtime_and_im_order() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-prompt-order-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let agent = default_agent();
        let departments = default_departments("api-1");
        let mut conversation = build_test_conversation(Vec::new());
        conversation.conversation_kind = CONVERSATION_KIND_REMOTE_IM_CONTACT.to_string();
        let system_blocks = vec![
            "<skill usage>\n技能索引块\n</skill usage>".to_string(),
            "<workspace agents>\n项目约束块\n</workspace agents>".to_string(),
            "<todo guide>\nTodo 说明块\n</todo guide>".to_string(),
            "<remote im runtime activation>\nIM 激活块\n</remote im runtime activation>".to_string(),
        ];

        let prompt = finalize_system_prompt_with_manager(
            Some(&state),
            "chat",
            &conversation,
            &agent,
            &departments,
            Some(&ApiConfig::default()),
            Some(("测试用户", "")),
            "default",
            "zh-CN",
            "固定系统块",
            None,
            Some("<shell workspace>\n运行环境块\n</shell workspace>"),
            &system_blocks,
            None,
        );

        let fixed_index = prompt.find("固定系统块").expect("fixed block");
        let plan_index = prompt.find("提问之法").expect("plan tool rule");
        let runtime_index = prompt.find("运行环境块").expect("runtime block");
        let remote_contact_index = prompt
            .find("联系人是特殊用户")
            .expect("remote im contact rules");

        assert!(fixed_index < plan_index);
        assert!(plan_index < runtime_index);
        assert!(runtime_index < remote_contact_index);
        assert!(!prompt.contains("技能索引块"));
        assert!(!prompt.contains("Todo 说明块"));
        assert!(!prompt.contains("项目约束块"));
        assert!(!prompt.contains("IM 激活块"));
    }

    #[test]
    fn finalize_system_prompt_with_manager_should_skip_remote_contact_rules_for_normal_chat() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-prompt-im-skip-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let agent = default_agent();
        let departments = default_departments("api-1");
        let conversation = build_test_conversation(Vec::new());
        let system_blocks = vec![
            "<remote im runtime activation>\nIM 激活块\n</remote im runtime activation>".to_string(),
        ];

        let prompt = finalize_system_prompt_with_manager(
            Some(&state),
            "chat",
            &conversation,
            &agent,
            &departments,
            Some(&ApiConfig::default()),
            Some(("测试用户", "")),
            "default",
            "zh-CN",
            "固定系统块",
            None,
            Some("<shell workspace>\n运行环境块\n</shell workspace>"),
            &system_blocks,
            None,
        );

        assert!(!prompt.contains("联系人是特殊用户"));
        assert!(!prompt.contains("IM 激活块"));
    }

    #[test]
    fn finalize_system_prompt_with_manager_should_not_insert_blank_lines_between_blocks() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-prompt-spacing-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let agent = default_agent();
        let agents = vec![agent.clone()];
        let departments = default_departments("api-1");
        let mut conversation = build_test_conversation(Vec::new());
        conversation
            .messages
            .push(build_test_message("user", "测试一下 system prompt 间距"));

        let prepared = build_prepared_prompt_for_mode(
            PromptBuildMode::Chat,
            &conversation,
            &agent,
            &agents,
            &departments,
            "测试用户",
            "",
            "default",
            "zh-CN",
            None,
            None,
            Some("<shell workspace>\n运行环境块\n</shell workspace>".to_string()),
            None,
            Some(&state),
            Some(&ApiConfig::default()),
            None,
            Some(false),
        );

        assert!(prepared.preamble.contains("</system rules>\n<persona settings>"));
        assert!(prepared.preamble.contains("</department context>\n<memory rag rule>"));
        assert!(!prepared
            .preamble
            .contains("</system rules>\n\n<persona settings>"));
        assert!(!prepared
            .preamble
            .contains("</department context>\n\n<memory rag rule>"));
    }

    #[test]
    fn finalize_system_prompt_with_manager_should_skip_disabled_builtin_tool_rules() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-prompt-tool-filter-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let agent = default_agent();
        let mut departments = default_departments("api-1");
        let assistant_department = departments
            .iter_mut()
            .find(|item| item.id == ASSISTANT_DEPARTMENT_ID)
            .expect("assistant department");
        assistant_department.permission_control = DepartmentPermissionControl {
            enabled: true,
            mode: "blacklist".to_string(),
            builtin_tool_names: vec!["apply_patch".to_string()],
            skill_names: Vec::new(),
            mcp_tool_names: Vec::new(),
        };
        let conversation = build_test_conversation(Vec::new());
        let api = ApiConfig::default();

        let prompt = finalize_system_prompt_with_manager(
            Some(&state),
            "chat",
            &conversation,
            &agent,
            &departments,
            Some(&api),
            Some(("测试用户", "")),
            "default",
            "zh-CN",
            "固定系统块",
            None,
            None,
            &[],
            None,
        );

        assert!(prompt.contains("<todo tool rule>"));
        assert!(!prompt.contains("<apply_patch tool rule>"));
    }

    #[test]
    fn finalize_system_prompt_with_manager_should_ignore_api_tool_switches() {
        let llm_workspace_path = std::env::temp_dir().join(format!(
            "easy-call-ai-prompt-api-independent-test-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&llm_workspace_path).expect("create llm workspace");
        let state = build_test_state(llm_workspace_path);
        let agent = default_agent();
        let departments = default_departments("api-1");
        let conversation = build_test_conversation(Vec::new());

        let mut api_enabled = ApiConfig::default();
        api_enabled.enable_tools = true;
        let mut api_disabled = api_enabled.clone();
        api_disabled.id = "api-2".to_string();
        api_disabled.enable_tools = false;
        api_disabled.tools.clear();

        let prompt_enabled = finalize_system_prompt_with_manager(
            Some(&state),
            "chat",
            &conversation,
            &agent,
            &departments,
            Some(&api_enabled),
            Some(("测试用户", "")),
            "default",
            "zh-CN",
            "固定系统块",
            None,
            None,
            &[],
            None,
        );
        let prompt_disabled = finalize_system_prompt_with_manager(
            Some(&state),
            "chat",
            &conversation,
            &agent,
            &departments,
            Some(&api_disabled),
            Some(("测试用户", "")),
            "default",
            "zh-CN",
            "固定系统块",
            None,
            None,
            &[],
            None,
        );

        assert_eq!(prompt_enabled, prompt_disabled);
    }
}
