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
    latest_images: Option<Vec<(String, String)>>,
    latest_audios: Option<Vec<(String, String)>>,
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

fn enrich_prepared_prompt_with_common_preamble(
    mut prepared: PreparedPrompt,
    _last_archive_summary: Option<&str>,
    _user_profile_memory_block: Option<&str>,
    terminal_block: Option<String>,
) -> PreparedPrompt {
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
    latest_images: Option<Vec<(String, String)>>,
    latest_audios: Option<Vec<(String, String)>>,
) {
    prepared.latest_user_text = latest_user_text;
    prepared.latest_user_meta_text = latest_user_meta_text;
    prepared.latest_user_extra_text = merge_latest_user_extra_text(
        &prepared.latest_user_extra_text,
        latest_user_extra_blocks,
    );
    if let Some(images) = latest_images {
        prepared.latest_images = images;
    }
    if let Some(audios) = latest_audios {
        prepared.latest_audios = audios;
    }
}

fn merge_latest_user_extra_text(existing: &str, appended_blocks: &[String]) -> String {
    let mut merged = Vec::<String>::new();
    if !existing.trim().is_empty() {
        merged.push(existing.trim().to_string());
    }
    for block in appended_blocks {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            continue;
        }
        merged.push(trimmed.to_string());
    }
    merged.join("\n\n")
}

#[cfg(test)]
mod prompt_assembly_tests {
    use super::*;

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
}
