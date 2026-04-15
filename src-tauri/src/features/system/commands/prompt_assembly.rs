#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PromptBuildMode {
    Chat,
    Delegate,
    SummaryContext,
}

#[derive(Debug, Clone, Default)]
struct ChatPromptOverrides {
    latest_user_text: Option<String>,
    latest_user_meta_text: Option<String>,
    latest_user_extra_blocks: Vec<String>,
    system_preamble_blocks: Vec<String>,
    latest_images: Option<Vec<PreparedBinaryPayload>>,
    latest_audios: Option<Vec<PreparedBinaryPayload>>,
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
    last_archive_summary: Option<&str>,
    terminal_block: Option<String>,
    chat_overrides: Option<ChatPromptOverrides>,
    state: Option<&AppState>,
    resolved_api: Option<&ResolvedApiConfig>,
    enable_pdf_images: Option<bool>,
) -> PreparedPrompt {
    match mode {
        PromptBuildMode::Chat => {
            let mut prepared = build_prompt(
                conversation,
                agent,
                agents,
                departments,
                user_name,
                user_intro,
                response_style_id,
                ui_language,
                data_path,
                state,
                resolved_api,
                enable_pdf_images.unwrap_or(false),
            );
            prepared = enrich_prepared_prompt_with_common_preamble(prepared, last_archive_summary, None, terminal_block);
            if let Some(overrides) = chat_overrides {
                append_preamble_blocks(&mut prepared.preamble, &overrides.system_preamble_blocks);
                let latest_user_text = overrides
                    .latest_user_text
                    .unwrap_or_else(|| prepared.latest_user_text.clone());
                let latest_user_meta_text = overrides
                    .latest_user_meta_text
                    .unwrap_or_else(|| prepared.latest_user_meta_text.clone());
                apply_chat_latest_user_payload(
                    &mut prepared,
                    latest_user_text,
                    latest_user_meta_text,
                    &overrides.latest_user_extra_blocks,
                    overrides.latest_images,
                    overrides.latest_audios,
                );
            }
            prepared
        }
        PromptBuildMode::Delegate => {
            let mut prepared = build_delegate_prompt(
                conversation,
                agent,
                agents,
                departments,
                response_style_id,
                ui_language,
                data_path,
                state,
                resolved_api,
                enable_pdf_images.unwrap_or(false),
            );
            prepared = enrich_prepared_prompt_with_common_preamble(prepared, None, None, terminal_block);
            if let Some(overrides) = chat_overrides {
                append_preamble_blocks(&mut prepared.preamble, &overrides.system_preamble_blocks);
                let latest_user_text = overrides
                    .latest_user_text
                    .unwrap_or_else(|| prepared.latest_user_text.clone());
                let latest_user_meta_text = overrides
                    .latest_user_meta_text
                    .unwrap_or_else(|| prepared.latest_user_meta_text.clone());
                apply_chat_latest_user_payload(
                    &mut prepared,
                    latest_user_text,
                    latest_user_meta_text,
                    &overrides.latest_user_extra_blocks,
                    overrides.latest_images,
                    overrides.latest_audios,
                );
            }
            prepared
        }
        PromptBuildMode::SummaryContext => {
            let prepared = build_prompt(
                conversation,
                agent,
                agents,
                departments,
                user_name,
                user_intro,
                response_style_id,
                ui_language,
                data_path,
                state,
                resolved_api,
                enable_pdf_images.unwrap_or(false),
            );
            let mut prepared =
                enrich_prepared_prompt_with_common_preamble(prepared, last_archive_summary, None, terminal_block);
            for message in &mut prepared.history_messages {
                message.images.clear();
                message.audios.clear();
            }
            prepared.latest_images.clear();
            prepared.latest_audios.clear();
            if let Some(overrides) = chat_overrides {
                append_preamble_blocks(&mut prepared.preamble, &overrides.system_preamble_blocks);
                let latest_user_text = overrides
                    .latest_user_text
                    .unwrap_or_else(|| prepared.latest_user_text.clone());
                let latest_user_meta_text = overrides
                    .latest_user_meta_text
                    .unwrap_or_else(|| prepared.latest_user_meta_text.clone());
                apply_chat_latest_user_payload(
                    &mut prepared,
                    latest_user_text,
                    latest_user_meta_text,
                    &overrides.latest_user_extra_blocks,
                    overrides.latest_images,
                    overrides.latest_audios,
                );
            }
            prepared
        }
    }
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
                    "以下内容来自当前主工作目录根下的 AGENTS.md，请将其视为该项目开发准则。\n\n路径：{}\n\n{}",
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

fn append_preamble_blocks(preamble: &mut String, blocks: &[String]) {
    for block in blocks {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !preamble.ends_with('\n') {
            preamble.push('\n');
        }
        preamble.push('\n');
        preamble.push_str(trimmed);
        preamble.push('\n');
    }
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
            cached_app_data: Arc::new(Mutex::new(None)),
            cached_app_data_mtime: Arc::new(Mutex::new(None)),
            cached_app_data_dirty: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_pending: Arc::new(Mutex::new(None)),
            app_data_persist_notify: Arc::new(tokio::sync::Notify::new()),
            app_data_persist_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            app_data_persist_latest_seq: Arc::new(std::sync::atomic::AtomicU64::new(0)),
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
            hidden_skill_snapshot_cache: Arc::new(Mutex::new(String::new())),
            preferred_release_source: Arc::new(Mutex::new(String::new())),
        }
    }

    fn build_test_conversation(workspaces: Vec<ShellWorkspaceConfig>) -> Conversation {
        Conversation {
            id: "conv-1".to_string(),
            title: "test".to_string(),
            agent_id: "assistant".to_string(),
            department_id: ASSISTANT_DEPARTMENT_ID.to_string(),
            last_read_message_id: String::new(),
            conversation_kind: CONVERSATION_KIND_CHAT.to_string(),
            root_conversation_id: None,
            delegate_id: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            last_user_at: None,
            last_assistant_at: None,
            last_context_usage_ratio: 0.0,
            last_effective_prompt_tokens: 0,
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
}
