use super::*;

#[tauri::command]
pub(crate) async fn mcp_refresh_mcp_and_skills(
    state: State<'_, AppState>,
) -> Result<RefreshMcpAndSkillsResult, String> {
    let mut out = {
        let guard = state
            .conversation_lock
            .lock()
            .map_err(|err| named_lock_error("conversation_lock", file!(), line!(), module_path!(), &err))?;
        let out = refresh_workspace_mcp_and_skills(&state)?;
        drop(guard);
        out
    };
    match mcp_redeploy_all_from_policy(&state).await {
        Ok(deploy_errors) => {
            if !deploy_errors.is_empty() {
                out.mcp_failed.extend(deploy_errors);
            }
        }
        Err(err) => {
            out.mcp_failed.push(WorkspaceLoadError {
                item: "mcp_redeploy_all_from_policy".to_string(),
                error: err,
            });
        }
    }
    mark_prompt_cache_rebuild_for_all_final(&state);
    Ok(out)
}

#[tauri::command]
pub(crate) fn mcp_list_skills(state: State<'_, AppState>) -> Result<SkillListResult, String> {
    let guard = state
        .conversation_lock
        .lock()
        .map_err(|err| named_lock_error("conversation_lock", file!(), line!(), module_path!(), &err))?;
    let (skills, errors) = load_workspace_skill_summaries_with_errors(&state)?;
    let _ = update_hidden_skill_snapshot_cache(&state, &skills, None);
    drop(guard);
    Ok(SkillListResult { skills, errors })
}

#[tauri::command]
pub(crate) fn skill_open_workspace_dir(state: State<'_, AppState>) -> Result<String, String> {
    open_skills_workspace_dir(&state)
}

