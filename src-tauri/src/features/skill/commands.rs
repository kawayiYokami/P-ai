use super::*;

#[tauri::command]
pub(crate) async fn mcp_refresh_mcp_and_skills(
    state: State<'_, AppState>,
) -> Result<RefreshMcpAndSkillsResult, String> {
    mcp_refresh_mcp_and_skills_inner(state.inner()).await
}

pub(crate) async fn mcp_refresh_mcp_and_skills_inner(
    state: &AppState,
) -> Result<RefreshMcpAndSkillsResult, String> {
    reload_workspace(state).await
}

#[tauri::command]
pub(crate) fn mcp_list_skills(state: State<'_, AppState>) -> Result<SkillListResult, String> {
    mcp_list_skills_inner(state.inner())
}

pub(crate) fn mcp_list_skills_inner(state: &AppState) -> Result<SkillListResult, String> {
    let (skills, errors) = load_workspace_skill_summaries_with_errors(state)?;
    let _ = update_hidden_skill_snapshot_cache(state, &skills, None);
    Ok(SkillListResult { skills, errors })
}

#[tauri::command]
pub(crate) fn skill_open_workspace_dir(state: State<'_, AppState>) -> Result<String, String> {
    open_skills_workspace_dir(&state)
}

#[tauri::command]
pub(crate) fn skill_open_item_dir(
    state: State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    skill_open_item_dir_inner(state.inner(), &path)
}

pub(crate) fn skill_open_item_dir_inner(
    state: &AppState,
    path: &str,
) -> Result<String, String> {
    open_skill_item_dir(state, path)
}

#[tauri::command]
pub(crate) fn mcp_save_skill(
    state: State<'_, AppState>,
    path: String,
    content: String,
    name: Option<String>,
    description: Option<String>,
) -> Result<SkillSummaryItem, String> {
    mcp_save_skill_inner(
        state.inner(),
        &path,
        &content,
        name.as_deref(),
        description.as_deref(),
    )
}

pub(crate) fn mcp_save_skill_inner(
    state: &AppState,
    path: &str,
    content: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Result<SkillSummaryItem, String> {
    save_workspace_skill_content(state, path, content, name, description)
}

#[tauri::command]
pub(crate) fn mcp_read_skill_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    mcp_read_skill_file_inner(state.inner(), &path)
}

pub(crate) fn mcp_read_skill_file_inner(
    state: &AppState,
    path: &str,
) -> Result<String, String> {
    read_workspace_skill_file(state, path)
}

