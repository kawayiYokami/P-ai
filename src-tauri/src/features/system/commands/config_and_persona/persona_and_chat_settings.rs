#[tauri::command]
fn load_agents(state: State<'_, AppState>) -> Result<Vec<AgentProfile>, String> {
    let mut config = state_read_config_cached(&state)?;
    let data = state_read_agents_runtime_snapshot(&state)?;
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut config, &mut runtime_data)?;
    Ok(runtime_data.agents)
}

#[tauri::command]
fn save_agents(
    input: SaveAgentsInput,
    state: State<'_, AppState>,
) -> Result<Vec<AgentProfile>, String> {
    if input.agents.is_empty() {
        return Err("At least one agent is required.".to_string());
    }

    let base_config = read_config(&state.config_path)?;
    let runtime = state_read_runtime_state_cached(&state)?;
    let mut data = AppData::default();
    data.agents = state_read_agents_cached(&state)?;
    apply_runtime_state_to_app_data(&mut data, &runtime);
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    let previous_agents = data.agents.clone();
    let existing_user_persona = data
        .agents
        .iter()
        .find(|a| a.id == USER_PERSONA_ID)
        .cloned();
    let existing_system_persona = data
        .agents
        .iter()
        .find(|a| a.id == SYSTEM_PERSONA_ID)
        .cloned();
    let existing_deputy_persona = data
        .agents
        .iter()
        .find(|a| a.id == DEPUTY_AGENT_ID)
        .cloned();
    data.agents = input
        .agents
        .into_iter()
        .filter(|agent| !private_agent_ids.contains(&agent.id))
        .collect();
    if !data.agents.iter().any(|a| a.id == USER_PERSONA_ID) {
        if let Some(user_persona) = existing_user_persona {
            data.agents.push(user_persona);
        }
    }
    if !data.agents.iter().any(|a| a.id == SYSTEM_PERSONA_ID) {
        if let Some(system_persona) = existing_system_persona {
            data.agents.push(system_persona);
        }
    }
    if !data.agents.iter().any(|a| a.id == DEPUTY_AGENT_ID) {
        data.agents
            .push(existing_deputy_persona.unwrap_or_else(default_deputy_agent));
    }
    let next_ids = data
        .agents
        .iter()
        .map(|a| a.id.clone())
        .collect::<std::collections::HashSet<_>>();
    let previous_by_id = previous_agents
        .iter()
        .map(|a| (a.id.clone(), a))
        .collect::<std::collections::HashMap<_, _>>();
    let removed_agent_ids = previous_agents
        .iter()
        .filter(|a| !a.is_built_in_user && !a.is_built_in_system && a.id != USER_PERSONA_ID && a.id != SYSTEM_PERSONA_ID)
        .filter(|a| !next_ids.contains(&a.id))
        .map(|a| a.id.clone())
        .collect::<Vec<_>>();
    let disabled_private_memory_agent_ids = data
        .agents
        .iter()
        .filter(|a| !a.is_built_in_user && !a.is_built_in_system && a.id != USER_PERSONA_ID && a.id != SYSTEM_PERSONA_ID)
        .filter(|a| {
            previous_by_id
                .get(&a.id)
                .map(|old| old.private_memory_enabled && !a.private_memory_enabled)
                .unwrap_or(false)
        })
        .map(|a| a.id.clone())
        .collect::<Vec<_>>();

    for agent_id in &removed_agent_ids {
        let started_at = std::time::Instant::now();
        eprintln!(
            "[会话] 开始，任务=导出并删除私有记忆，status=开始，agent_id={}，trigger=agent_removed",
            agent_id
        );
        let export = match memory_store_export_agent_private_memories(&state.data_path, agent_id) {
            Ok(export) => export,
            Err(error) => {
                let elapsed_ms = started_at.elapsed().as_millis();
                eprintln!(
                    "[会话] 失败，任务=导出并删除私有记忆，status=失败，agent_id={}，trigger=agent_removed，stage=export，duration_ms={}，error={}",
                    agent_id, elapsed_ms, error
                );
                return Err(error);
            }
        };
        let deleted = match memory_store_delete_memories_by_owner_agent_id(&state.data_path, agent_id) {
            Ok(deleted) => deleted,
            Err(error) => {
                let elapsed_ms = started_at.elapsed().as_millis();
                eprintln!(
                    "[会话] 失败，任务=导出并删除私有记忆，status=失败，agent_id={}，trigger=agent_removed，stage=delete，duration_ms={}，error={}",
                    agent_id, elapsed_ms, error
                );
                return Err(error);
            }
        };
        let elapsed_ms = started_at.elapsed().as_millis();
        eprintln!(
            "[会话] 完成，任务=导出并删除私有记忆，status=完成，agent_id={}，export.count={}，export.path={}，deleted={}，duration_ms={}",
            agent_id,
            export.count,
            export.path,
            deleted,
            elapsed_ms
        );
    }
    for agent_id in &disabled_private_memory_agent_ids {
        let started_at = std::time::Instant::now();
        eprintln!(
            "[会话] 开始，任务=导出并删除私有记忆，status=开始，agent_id={}，trigger=private_memory_disabled",
            agent_id
        );
        let export = match memory_store_export_agent_private_memories(&state.data_path, agent_id) {
            Ok(export) => export,
            Err(error) => {
                let elapsed_ms = started_at.elapsed().as_millis();
                eprintln!(
                    "[会话] 失败，任务=导出并删除私有记忆，status=失败，agent_id={}，trigger=private_memory_disabled，stage=export，duration_ms={}，error={}",
                    agent_id, elapsed_ms, error
                );
                return Err(error);
            }
        };
        let deleted = match memory_store_delete_memories_by_owner_agent_id(&state.data_path, agent_id) {
            Ok(deleted) => deleted,
            Err(error) => {
                let elapsed_ms = started_at.elapsed().as_millis();
                eprintln!(
                    "[会话] 失败，任务=导出并删除私有记忆，status=失败，agent_id={}，trigger=private_memory_disabled，stage=delete，duration_ms={}，error={}",
                    agent_id, elapsed_ms, error
                );
                return Err(error);
            }
        };
        let elapsed_ms = started_at.elapsed().as_millis();
        eprintln!(
            "[会话] 完成，任务=导出并删除私有记忆，status=完成，agent_id={}，export.count={}，export.path={}，deleted={}，duration_ms={}",
            agent_id,
            export.count,
            export.path,
            deleted,
            elapsed_ms
        );
    }

    let affected_agent_ids = previous_agents
        .iter()
        .chain(data.agents.iter())
        .filter(|a| !a.is_built_in_user && !a.is_built_in_system && a.id != USER_PERSONA_ID && a.id != SYSTEM_PERSONA_ID)
        .map(|a| a.id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    state_write_agents_cached(&state, &data.agents)?;
    if !affected_agent_ids.is_empty() {
        mark_prompt_cache_rebuild_for_system_sources_by_agents(&state, &affected_agent_ids);
    }
    let mut config = state_read_config_cached(&state)?;
    let runtime_agents = runtime_agents_with_private_organization(&state, &config, &data)?;
    let valid_agent_ids = runtime_agents
        .iter()
        .filter(|a| !a.is_built_in_user)
        .map(|a| a.id.clone())
        .collect::<std::collections::HashSet<_>>();
    let mut config_changed = false;
    for dept in &mut config.departments {
        let original_len = dept.agent_ids.len();
        dept.agent_ids.retain(|id| valid_agent_ids.contains(id));
        if dept.id == ASSISTANT_DEPARTMENT_ID && dept.agent_ids.is_empty() {
            dept.agent_ids.push(data.assistant_department_agent_id.clone());
        }
        if dept.agent_ids.len() != original_len || (dept.id == ASSISTANT_DEPARTMENT_ID && dept.agent_ids.first() != Some(&data.assistant_department_agent_id)) {
            config_changed = true;
            if dept.id == ASSISTANT_DEPARTMENT_ID {
                dept.agent_ids = vec![data.assistant_department_agent_id.clone()];
            }
            dept.updated_at = now_iso();
        }
    }
    if config_changed {
        let changed_departments = {
            let old_by_id = base_config
                .departments
                .iter()
                .map(|item| (item.id.clone(), item.clone()))
                .collect::<std::collections::HashMap<_, _>>();
            let new_by_id = config
                .departments
                .iter()
                .map(|item| (item.id.clone(), item.clone()))
                .collect::<std::collections::HashMap<_, _>>();
            old_by_id
                .keys()
                .chain(new_by_id.keys())
                .cloned()
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .filter(|id| old_by_id.get(id) != new_by_id.get(id))
                .collect::<Vec<_>>()
        };
        normalize_app_config(&mut config);
        state_write_config_cached(&state, &config)?;
        if !changed_departments.is_empty() {
            mark_prompt_cache_rebuild_for_system_sources_by_departments(
                &state,
                &changed_departments,
            );
        }
    }
    let runtime_agents = runtime_agents_with_private_organization(&state, &config, &data)?;
    Ok(runtime_agents)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportAgentMemoriesInput {
    agent_id: String,
    memories: Vec<MemoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportAgentMemoriesResult {
    imported_count: usize,
    created_count: usize,
    merged_count: usize,
    total_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentPrivateMemoryCountInput {
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentPrivateMemoryCountResult {
    count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetAgentPrivateMemoryEnabledInput {
    agent_id: String,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetAgentPrivateMemoryEnabledResult {
    agent_id: String,
    enabled: bool,
    exported_count: usize,
    deleted_count: usize,
    export_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportAgentPrivateMemoriesInput {
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportAgentPrivateMemoriesResult {
    count: usize,
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisableAgentPrivateMemoryInput {
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DisableAgentPrivateMemoryResult {
    agent_id: String,
    enabled: bool,
    deleted_count: usize,
}

#[tauri::command]
fn get_agent_private_memory_count(
    input: AgentPrivateMemoryCountInput,
    state: State<'_, AppState>,
) -> Result<AgentPrivateMemoryCountResult, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }
    let config = read_config(&state.config_path)?;
    let agents = state_read_agents_cached(&state)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &config, &agents)?;
    if private_agent_ids.contains(agent_id) {
        return Err(private_agent_operation_error(agent_id));
    }
    Ok(AgentPrivateMemoryCountResult {
        count: memory_store_count_private_memories_by_agent(&state.data_path, agent_id)?,
    })
}

#[tauri::command]
fn set_agent_private_memory_enabled(
    input: SetAgentPrivateMemoryEnabledInput,
    state: State<'_, AppState>,
) -> Result<SetAgentPrivateMemoryEnabledResult, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }

    let mut agents = state_read_agents_cached(&state)?;
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &agents)?;
    if private_agent_ids.contains(agent_id) {
        return Err(private_agent_operation_error(agent_id));
    }

    let agent_idx = agents
        .iter()
        .position(|a| a.id == agent_id && !a.is_built_in_user)
        .ok_or_else(|| format!("Agent '{}' not found.", agent_id))?;

    let current = agents[agent_idx].private_memory_enabled;
    if current == input.enabled {
        return Ok(SetAgentPrivateMemoryEnabledResult {
            agent_id: agent_id.to_string(),
            enabled: current,
            exported_count: 0,
            deleted_count: 0,
            export_path: None,
        });
    }

    if input.enabled {
        agents[agent_idx].private_memory_enabled = true;
        state_write_agents_cached(&state, &agents)?;
        return Ok(SetAgentPrivateMemoryEnabledResult {
            agent_id: agent_id.to_string(),
            enabled: true,
            exported_count: 0,
            deleted_count: 0,
            export_path: None,
        });
    }

    let export = memory_store_export_agent_private_memories(&state.data_path, agent_id)?;
    let deleted = memory_store_delete_memories_by_owner_agent_id(&state.data_path, agent_id)?;
    agents[agent_idx].private_memory_enabled = false;
    state_write_agents_cached(&state, &agents)?;

    Ok(SetAgentPrivateMemoryEnabledResult {
        agent_id: agent_id.to_string(),
        enabled: false,
        exported_count: export.count,
        deleted_count: deleted,
        export_path: Some(export.path),
    })
}

#[tauri::command]
fn export_agent_private_memories(
    input: ExportAgentPrivateMemoriesInput,
    state: State<'_, AppState>,
) -> Result<ExportAgentPrivateMemoriesResult, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }
    let config = read_config(&state.config_path)?;
    let agents = state_read_agents_cached(&state)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &config, &agents)?;
    if private_agent_ids.contains(agent_id) {
        return Err(private_agent_operation_error(agent_id));
    }
    let export = memory_store_export_agent_private_memories(&state.data_path, agent_id)?;
    Ok(ExportAgentPrivateMemoriesResult {
        count: export.count,
        path: export.path,
    })
}

#[tauri::command]
fn disable_agent_private_memory(
    input: DisableAgentPrivateMemoryInput,
    state: State<'_, AppState>,
) -> Result<DisableAgentPrivateMemoryResult, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }

    let mut agents = state_read_agents_cached(&state)?;
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &agents)?;
    if private_agent_ids.contains(agent_id) {
        return Err(private_agent_operation_error(agent_id));
    }

    let agent_idx = agents
        .iter()
        .position(|a| a.id == agent_id && !a.is_built_in_user)
        .ok_or_else(|| format!("Agent '{}' not found.", agent_id))?;

    if !agents[agent_idx].private_memory_enabled {
        return Ok(DisableAgentPrivateMemoryResult {
            agent_id: agent_id.to_string(),
            enabled: false,
            deleted_count: 0,
        });
    }

    let deleted = memory_store_delete_memories_by_owner_agent_id(&state.data_path, agent_id)?;
    agents[agent_idx].private_memory_enabled = false;
    state_write_agents_cached(&state, &agents)?;

    Ok(DisableAgentPrivateMemoryResult {
        agent_id: agent_id.to_string(),
        enabled: false,
        deleted_count: deleted,
    })
}

#[tauri::command]
fn import_agent_memories(
    input: ImportAgentMemoriesInput,
    state: State<'_, AppState>,
) -> Result<ImportAgentMemoriesResult, String> {
    let agent_id = input.agent_id.trim();
    if agent_id.is_empty() {
        return Err("agentId is required".to_string());
    }

    let data = state_read_app_data_cached(&state)?;
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &data.agents)?;
    if private_agent_ids.contains(agent_id) {
        return Err(private_agent_operation_error(agent_id));
    }
    if !data
        .agents
        .iter()
        .any(|a| a.id == agent_id && !a.is_built_in_user)
    {
        return Err(format!("Agent '{}' not found.", agent_id));
    }

    let stats = memory_store_import_memories_for_agent(&state.data_path, &input.memories, agent_id)?;
    Ok(ImportAgentMemoriesResult {
        imported_count: stats.imported_count,
        created_count: stats.created_count,
        merged_count: stats.merged_count,
        total_count: stats.total_count,
    })
}

#[tauri::command]
fn load_chat_settings(state: State<'_, AppState>) -> Result<ChatSettings, String> {
    let mut config = read_config(&state.config_path)?;
    let mut data = state_read_agents_runtime_snapshot(&state)?;
    let assistant_agent_id = assistant_department_agent_id(&config).unwrap_or_else(default_assistant_department_agent_id);
    let runtime_changed = if data.assistant_department_agent_id != assistant_agent_id {
        data.assistant_department_agent_id = assistant_agent_id.clone();
        true
    } else {
        false
    };
    if runtime_changed {
        state_write_runtime_state_cached(&state, &build_runtime_state_file(&data))?;
    }
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut config, &mut runtime_data)?;

    Ok(ChatSettings {
        assistant_department_agent_id: data.assistant_department_agent_id.clone(),
        user_alias: user_persona_name(&runtime_data),
        response_style_id: data.response_style_id.clone(),
        pdf_read_mode: data.pdf_read_mode.clone(),
        background_voice_screenshot_keywords: data.background_voice_screenshot_keywords.clone(),
        background_voice_screenshot_mode: data.background_voice_screenshot_mode.clone(),
        instruction_presets: data.instruction_presets.clone(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ChatSettingsPatch {
    #[serde(default)]
    assistant_department_agent_id: Option<String>,
    #[serde(default)]
    user_alias: Option<String>,
    #[serde(default)]
    response_style_id: Option<String>,
    #[serde(default)]
    pdf_read_mode: Option<String>,
    #[serde(default)]
    background_voice_screenshot_keywords: Option<String>,
    #[serde(default)]
    background_voice_screenshot_mode: Option<String>,
    #[serde(default)]
    instruction_presets: Option<Vec<PromptCommandPreset>>,
}

fn build_chat_settings_payload(state: &AppState, data: &AppData, config: &AppConfig) -> Result<ChatSettings, String> {
    let mut runtime_config = config.clone();
    let mut runtime_data = data.clone();
    merge_private_organization_into_runtime_data(&state.data_path, &mut runtime_config, &mut runtime_data)?;
    Ok(ChatSettings {
        assistant_department_agent_id: data.assistant_department_agent_id.clone(),
        user_alias: user_persona_name(&runtime_data),
        response_style_id: data.response_style_id.clone(),
        pdf_read_mode: data.pdf_read_mode.clone(),
        background_voice_screenshot_keywords: data.background_voice_screenshot_keywords.clone(),
        background_voice_screenshot_mode: data.background_voice_screenshot_mode.clone(),
        instruction_presets: data.instruction_presets.clone(),
    })
}

fn apply_chat_settings_patch(
    state: &AppState,
    agents: &mut Vec<AgentProfile>,
    runtime: &mut RuntimeStateFile,
    config: &AppConfig,
    input: ChatSettingsPatch,
) -> Result<ChatSettings, String> {
    let mut agents_changed = false;
    let mut runtime_changed = false;
    if let Some(agent_id) = input.assistant_department_agent_id {
        let fallback = runtime.assistant_department_agent_id.clone();
        let target_agent_id = assistant_department_agent_id(config).unwrap_or_else(|| {
            let trimmed = agent_id.trim();
            if trimmed.is_empty() {
                fallback.clone()
            } else {
                trimmed.to_string()
            }
        });
        let mut runtime_config = config.clone();
        let mut runtime_data = AppData::default();
        runtime_data.agents = agents.clone();
        apply_runtime_state_to_app_data(&mut runtime_data, runtime);
        merge_private_organization_into_runtime_data(&state.data_path, &mut runtime_config, &mut runtime_data)?;
        if !runtime_data
            .agents
            .iter()
            .any(|a| a.id == target_agent_id && !a.is_built_in_user)
        {
            return Err("Selected agent not found.".to_string());
        }
        if runtime.assistant_department_agent_id != target_agent_id {
            runtime.assistant_department_agent_id = target_agent_id;
            runtime_changed = true;
        }
    }
    if let Some(response_style_id) = input.response_style_id {
        let next = normalize_response_style_id(&response_style_id);
        if runtime.response_style_id != next {
            runtime.response_style_id = next;
            runtime_changed = true;
        }
    }
    if let Some(pdf_read_mode) = input.pdf_read_mode {
        let next = normalize_pdf_read_mode(&pdf_read_mode);
        if runtime.pdf_read_mode != next {
            runtime.pdf_read_mode = next;
            runtime_changed = true;
        }
    }
    if let Some(background_voice_screenshot_keywords) = input.background_voice_screenshot_keywords {
        let next = background_voice_screenshot_keywords.trim().to_string();
        if runtime.background_voice_screenshot_keywords != next {
            runtime.background_voice_screenshot_keywords = next;
            runtime_changed = true;
        }
    }
    if let Some(background_voice_screenshot_mode) = input.background_voice_screenshot_mode {
        let next = normalize_background_voice_screenshot_mode(&background_voice_screenshot_mode);
        if runtime.background_voice_screenshot_mode != next {
            runtime.background_voice_screenshot_mode = next;
            runtime_changed = true;
        }
    }
    if let Some(instruction_presets) = input.instruction_presets {
        let next = instruction_presets
            .into_iter()
            .map(|item| PromptCommandPreset {
                id: item.id.trim().to_string(),
                name: item.name.trim().to_string(),
                prompt: item.prompt.trim().to_string(),
            })
            .filter(|item| !item.id.is_empty() && !item.name.is_empty() && !item.prompt.is_empty())
            .collect::<Vec<_>>();
        if runtime.instruction_presets != next {
            runtime.instruction_presets = next;
            runtime_changed = true;
        }
    }
    if let Some(user_alias) = input.user_alias {
        let trimmed = user_alias.trim();
        if !trimmed.is_empty() {
            if let Some(user_persona) = agents.iter_mut().find(|a| a.id == USER_PERSONA_ID) {
                user_persona.name = trimmed.to_string();
                user_persona.updated_at = now_iso();
                agents_changed = true;
            }
        }
    }
    if agents_changed {
        state_write_agents_cached(state, agents)?;
    }
    if runtime_changed {
        state_write_runtime_state_cached(state, runtime)?;
    }
    let mut data = AppData::default();
    data.agents = agents.clone();
    apply_runtime_state_to_app_data(&mut data, runtime);
    build_chat_settings_payload(state, &data, config)
}

#[tauri::command]
fn save_chat_settings(
    input: ChatSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ChatSettings, String> {
    patch_chat_settings(
        ChatSettingsPatch {
            assistant_department_agent_id: Some(input.assistant_department_agent_id),
            user_alias: Some(input.user_alias),
            response_style_id: Some(input.response_style_id),
            pdf_read_mode: Some(input.pdf_read_mode),
            background_voice_screenshot_keywords: Some(input.background_voice_screenshot_keywords),
            background_voice_screenshot_mode: Some(input.background_voice_screenshot_mode),
            instruction_presets: Some(input.instruction_presets),
        },
        app,
        state,
    )
}

#[tauri::command]
fn patch_chat_settings(
    input: ChatSettingsPatch,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ChatSettings, String> {
    let mut data = state_read_agents_runtime_snapshot(&state)?;
    let config = read_config(&state.config_path)?;
    let mut runtime = build_runtime_state_file(&data);
    let payload = apply_chat_settings_patch(&state, &mut data.agents, &mut runtime, &config, input)?;

    let _ = app.emit("easy-call:chat-settings-updated", &payload);

    Ok(payload)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveAgentAvatarInput {
    agent_id: String,
    mime: String,
    bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClearAgentAvatarInput {
    agent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AvatarDataPathInput {
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncTrayIconInput {
    #[serde(default)]
    agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AvatarMeta {
    path: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AvatarDataUrlOutput {
    data_url: String,
}

fn avatar_storage_dir(state: &AppState) -> Result<PathBuf, String> {
    Ok(app_root_from_data_path(&state.data_path).join("avatars"))
}

fn sanitize_avatar_key(value: &str) -> String {
    let trimmed = value.trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let normalized = out.trim_matches('_');
    if normalized.is_empty() {
        "unknown".to_string()
    } else {
        normalized.to_string()
    }
}

fn normalize_avatar_bytes_to_webp(raw: &[u8]) -> Result<Vec<u8>, String> {
    let image = image::load_from_memory(raw)
        .map_err(|err| format!("Decode avatar image failed: {err}"))?;
    let resized = image.resize_to_fill(128, 128, image::imageops::FilterType::Lanczos3);
    let mut out = Vec::<u8>::new();
    let mut cursor = Cursor::new(&mut out);
    resized
        .write_to(&mut cursor, ImageFormat::WebP)
        .map_err(|err| format!("Encode avatar as webp failed: {err}"))?;
    Ok(out)
}

#[tauri::command]
fn save_agent_avatar(
    input: SaveAgentAvatarInput,
    state: State<'_, AppState>,
) -> Result<AvatarMeta, String> {
    if input.agent_id.trim().is_empty() {
        return Err("agentId is required".to_string());
    }
    if input.bytes_base64.trim().is_empty() {
        return Err("avatar payload is empty".to_string());
    }
    if !input.mime.trim().starts_with("image/") {
        return Err("avatar mime must be image/*".to_string());
    }

    let mut agents = state_read_agents_cached(&state)?;
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &agents)?;
    if private_agent_ids.contains(input.agent_id.trim()) {
        return Err(private_agent_operation_error(input.agent_id.trim()));
    }

    let idx = agents
        .iter()
        .position(|a| a.id == input.agent_id)
        .ok_or_else(|| "Agent not found".to_string())?;

    let raw = B64
        .decode(input.bytes_base64.trim())
        .map_err(|err| format!("Decode avatar base64 failed: {err}"))?;
    let webp = normalize_avatar_bytes_to_webp(&raw)?;

    let dir = avatar_storage_dir(&state)?;
    fs::create_dir_all(&dir).map_err(|err| format!("Create avatar directory failed: {err}"))?;
    let safe_id = sanitize_avatar_key(&input.agent_id);
    let path = dir.join(format!("agent-{safe_id}.webp"));
    fs::write(&path, webp).map_err(|err| format!("Write avatar file failed: {err}"))?;

    let now = now_iso();
    agents[idx].avatar_path = Some(path.to_string_lossy().to_string());
    agents[idx].avatar_updated_at = Some(now.clone());
    agents[idx].updated_at = now.clone();
    state_write_agents_cached(&state, &agents)?;

    Ok(AvatarMeta {
        path: path.to_string_lossy().to_string(),
        updated_at: now,
    })
}

#[tauri::command]
fn clear_agent_avatar(
    input: ClearAgentAvatarInput,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if input.agent_id.trim().is_empty() {
        return Err("agentId is required".to_string());
    }

    let mut agents = state_read_agents_cached(&state)?;
    let base_config = read_config(&state.config_path)?;
    let (private_agent_ids, _) =
        runtime_private_organization_ids(&state.data_path, &base_config, &agents)?;
    if private_agent_ids.contains(input.agent_id.trim()) {
        return Err(private_agent_operation_error(input.agent_id.trim()));
    }
    let idx = agents
        .iter()
        .position(|a| a.id == input.agent_id)
        .ok_or_else(|| "Agent not found".to_string())?;

    if let Some(path) = agents[idx].avatar_path.as_deref() {
        let p = PathBuf::from(path);
        if p.exists() {
            let _ = fs::remove_file(p);
        }
    }
    agents[idx].avatar_path = None;
    agents[idx].avatar_updated_at = None;
    agents[idx].updated_at = now_iso();
    state_write_agents_cached(&state, &agents)?;
    Ok(())
}

#[tauri::command]
fn read_avatar_data_url(
    input: AvatarDataPathInput,
    state: State<'_, AppState>,
) -> Result<AvatarDataUrlOutput, String> {
    if input.path.trim().is_empty() {
        return Ok(AvatarDataUrlOutput {
            data_url: String::new(),
        });
    }
    let avatars_dir = avatar_storage_dir(&state)?;
    let root = fs::canonicalize(&avatars_dir).map_err(|err| {
        format!(
            "Resolve avatar root failed ({}): {err}",
            avatars_dir.to_string_lossy()
        )
    })?;
    let target = fs::canonicalize(input.path.trim()).map_err(|err| {
        format!("Resolve avatar path failed ({}): {err}", input.path.trim())
    })?;
    if !target.starts_with(&root) {
        return Err("Avatar path is outside allowed avatar directory.".to_string());
    }
    let metadata = fs::metadata(&target)
        .map_err(|err| format!("Read avatar metadata failed: {err}"))?;
    if !metadata.is_file() {
        return Err("Avatar path must be a regular file.".to_string());
    }
    let ext = target
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_ascii_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "webp" => "image/webp",
        "png" => "image/png",
        _ => return Err("Avatar file type is not allowed (only .webp/.png).".to_string()),
    };
    let bytes = fs::read(&target)
        .map_err(|err| format!("Read avatar file failed: {err}"))?;
    let base64 = B64.encode(bytes);
    Ok(AvatarDataUrlOutput {
        data_url: format!("data:{mime};base64,{base64}"),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatImageDataUrlInput {
    media_ref: String,
    mime: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatImageDataUrlOutput {
    data_url: String,
}

#[tauri::command]
fn read_chat_image_data_url(
    input: ChatImageDataUrlInput,
    state: State<'_, AppState>,
) -> Result<ChatImageDataUrlOutput, String> {
    let media_ref = input.media_ref.trim();
    if media_ref.is_empty() {
        return Ok(ChatImageDataUrlOutput {
            data_url: String::new(),
        });
    }
    if stored_binary_ref_from_marker(media_ref).is_none() {
        return Err("Chat image mediaRef is invalid.".to_string());
    }
    let mime = input.mime.trim().to_ascii_lowercase();
    if !mime.starts_with("image/") {
        return Err("Chat image mime is invalid.".to_string());
    }
    let base64 = resolve_stored_binary_base64(&state.data_path, media_ref)?;
    Ok(ChatImageDataUrlOutput {
        data_url: format!("data:{mime};base64,{base64}"),
    })
}

#[tauri::command]
fn sync_tray_icon(
    input: SyncTrayIconInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let data = state_read_app_data_cached(&state)?;
    let target_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(data.assistant_department_agent_id.as_str());
    let avatar_path = data
        .agents
        .iter()
        .find(|a| a.id == target_agent_id)
        .and_then(|a| a.avatar_path.clone());
    sync_tray_icon_from_avatar_path(&app, avatar_path.as_deref())
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ConversationApiSettingsPatch {
    #[serde(default)]
    assistant_department_api_config_id: Option<String>,
    #[serde(default)]
    vision_api_config_id: Option<Option<String>>,
    #[serde(default)]
    tool_review_api_config_id: Option<Option<String>>,
    #[serde(default)]
    stt_api_config_id: Option<Option<String>>,
    #[serde(default)]
    stt_auto_send: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetDepartmentPrimaryApiConfigInput {
    department_id: String,
    api_config_id: String,
}

fn build_conversation_api_settings_payload(config: &AppConfig) -> ConversationApiSettings {
    ConversationApiSettings {
        assistant_department_api_config_id: config.assistant_department_api_config_id.clone(),
        vision_api_config_id: config.vision_api_config_id.clone(),
        tool_review_api_config_id: config.tool_review_api_config_id.clone(),
        stt_api_config_id: config.stt_api_config_id.clone(),
        stt_auto_send: config.stt_auto_send,
    }
}

fn apply_conversation_api_settings_patch(config: &mut AppConfig, input: ConversationApiSettingsPatch) {
    if let Some(assistant_department_api_config_id) = input.assistant_department_api_config_id {
        config.assistant_department_api_config_id = assistant_department_api_config_id;
    }
    if let Some(vision_api_config_id) = input.vision_api_config_id {
        config.vision_api_config_id = vision_api_config_id;
    }
    if let Some(tool_review_api_config_id) = input.tool_review_api_config_id {
        config.tool_review_api_config_id = tool_review_api_config_id;
    }
    if let Some(stt_api_config_id) = input.stt_api_config_id {
        config.stt_api_config_id = stt_api_config_id;
    }
    if let Some(stt_auto_send) = input.stt_auto_send {
        config.stt_auto_send = stt_auto_send;
    }
}

#[tauri::command]
fn save_conversation_api_settings(
    input: ConversationApiSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ConversationApiSettings, String> {
    patch_conversation_api_settings(
        ConversationApiSettingsPatch {
            assistant_department_api_config_id: Some(input.assistant_department_api_config_id),
            vision_api_config_id: Some(input.vision_api_config_id),
            tool_review_api_config_id: Some(input.tool_review_api_config_id),
            stt_api_config_id: Some(input.stt_api_config_id),
            stt_auto_send: Some(input.stt_auto_send),
        },
        app,
        state,
    )
}

#[tauri::command]
fn patch_conversation_api_settings(
    input: ConversationApiSettingsPatch,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ConversationApiSettings, String> {
    let mut config = state_read_config_cached(&state)?;
    apply_conversation_api_settings_patch(&mut config, input);
    normalize_app_config(&mut config);
    let assistant_api_config_id = config.assistant_department_api_config_id.clone();
    if let Some(dept) = assistant_department_mut(&mut config) {
        let cleaned = assistant_api_config_id.trim();
        dept.api_config_ids = if cleaned.is_empty() {
            Vec::new()
        } else {
            vec![cleaned.to_string()]
        };
        dept.api_config_id = if cleaned.is_empty() {
            String::new()
        } else {
            cleaned.to_string()
        };
        dept.updated_at = now_iso();
    }
    state_write_config_cached(&state, &config)?;

    let payload = build_conversation_api_settings_payload(&config);

    let _ = app.emit("easy-call:conversation-api-updated", &payload);

    Ok(payload)
}

#[tauri::command]
fn set_department_primary_api_config(
    input: SetDepartmentPrimaryApiConfigInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    let department_id = input.department_id.trim();
    if department_id.is_empty() {
        return Err("Department ID is required.".to_string());
    }
    let api_config_id = input.api_config_id.trim();
    if api_config_id.is_empty() {
        return Err("API config ID is required.".to_string());
    }

    let mut config = state_read_config_cached(&state)?;
    let selected_api = config
        .api_configs
        .iter()
        .find(|item| item.id.trim() == api_config_id)
        .ok_or_else(|| format!("API config '{api_config_id}' not found."))?;
    if !selected_api.enable_text {
        return Err(format!("API config '{api_config_id}' does not support chat text."));
    }

    let (department_primary_api_config_id, assistant_department_changed) = {
        let Some(target_department) = config
            .departments
            .iter_mut()
            .find(|item| item.id.trim() == department_id)
        else {
            return Err(format!("Department '{department_id}' not found."));
        };

        let mut next_ids = department_api_config_ids(target_department);
        if next_ids.first().map(|item| item.trim()) == Some(api_config_id) {
            // 保持当前顺序，只同步全局选中模型即可。
        } else {
            next_ids.retain(|item| !item.trim().eq_ignore_ascii_case(api_config_id));
            if next_ids.is_empty() {
                next_ids.push(api_config_id.to_string());
            } else {
                next_ids[0] = api_config_id.to_string();
            }
        }

        let mut seen = std::collections::HashSet::<String>::new();
        target_department.api_config_ids = next_ids
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .filter(|item| seen.insert(item.to_ascii_lowercase()))
            .collect::<Vec<_>>();
        target_department.api_config_id = target_department
            .api_config_ids
            .first()
            .cloned()
            .unwrap_or_default();
        target_department.updated_at = now_iso();

        (
            target_department.api_config_id.clone(),
            target_department.id == ASSISTANT_DEPARTMENT_ID || target_department.is_built_in_assistant,
        )
    };

    if assistant_department_changed {
        config.assistant_department_api_config_id = department_primary_api_config_id;
    }
    config.selected_api_config_id = api_config_id.to_string();

    state_write_config_cached(&state, &config)?;
    let data = state_read_app_data_cached(&state)?;
    let runtime_config = runtime_config_with_private_organization(&state, &config, &data)?;

    let _ = app.emit("easy-call:config-updated", &runtime_config);

    Ok(runtime_config)
}
