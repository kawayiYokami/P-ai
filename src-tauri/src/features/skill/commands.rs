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

#[tauri::command]
pub(crate) async fn skill_set_enabled(
    state: State<'_, AppState>,
    name: String,
    enabled: bool,
) -> Result<SkillListResult, String> {
    skill_set_enabled_inner(state.inner(), &name, enabled).await
}

pub(crate) async fn skill_set_enabled_inner(
    state: &AppState,
    name: &str,
    enabled: bool,
) -> Result<SkillListResult, String> {
    let skill_name = name.trim();
    if skill_name.is_empty() {
        return Err("Skill 名不能为空。".to_string());
    }
    write_skill_enabled(state, skill_name, enabled)?;
    runtime_log_info(format!(
        "[技能启用] 完成：skill={skill_name}，enabled={enabled}"
    ));
    reload_workspace(state).await?;
    mcp_list_skills_inner(state)
}

pub(crate) fn mcp_list_skills_inner(state: &AppState) -> Result<SkillListResult, String> {
    let (skills, errors) = load_workspace_skill_summaries_with_errors(state)?;
    let _ = update_hidden_skill_snapshot_cache(state, &skills, None);
    Ok(SkillListResult { skills, errors })
}

#[tauri::command]
pub(crate) async fn skill_remove(
    state: State<'_, AppState>,
    path: String,
) -> Result<SkillListResult, String> {
    skill_remove_inner(state.inner(), &path).await
}

pub(crate) async fn skill_remove_inner(
    state: &AppState,
    path: &str,
) -> Result<SkillListResult, String> {
    let skill_path = path.trim();
    if skill_path.is_empty() {
        return Err("Skill 路径不能为空。".to_string());
    }
    let skills_root = llm_workspace_skills_root(state)?;
    let dir = resolve_removable_skill_dir(&skills_root, skill_path)?;
    let dir_name = dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();

    // 先解析技能名，用于随目录一并清理启用状态记录。
    let (skill_name, _, _) = parse_skill_file(&dir.join("SKILL.md"))?;
    fs::remove_dir_all(&dir)
        .map_err(|err| format!("删除 Skill 目录失败（{}）：{err}", dir.display()))?;
    remove_skill_policy(state, &skill_name)?;
    runtime_log_info(format!(
        "[技能删除] 完成：skill={skill_name}，dir={dir_name}"
    ));
    reload_workspace(state).await?;
    mcp_list_skills_inner(state)
}

/// 校验并解析可删除的技能目录。
/// 入参为 SKILL.md 路径；只允许 skills 根目录的直接子目录，且剔除内置预设技能。
fn resolve_removable_skill_dir(skills_root: &Path, skill_path: &str) -> Result<PathBuf, String> {
    let dir = PathBuf::from(skill_path)
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| format!("无法从路径解析 Skill 目录：{skill_path}"))?
        .to_path_buf();
    if !dir.is_dir() {
        return Err(format!("Skill 目录不存在：{}", dir.display()));
    }
    let canonical_root = skills_root
        .canonicalize()
        .map_err(|err| format!("解析 skills 目录失败（{}）：{err}", skills_root.display()))?;
    let canonical_dir = dir
        .canonicalize()
        .map_err(|err| format!("解析 Skill 目录失败（{}）：{err}", dir.display()))?;
    if canonical_dir.parent() != Some(canonical_root.as_path()) {
        return Err("只能删除工作区 skills 目录下的技能。".to_string());
    }
    let dir_name = canonical_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    // 内置预设技能会在 reload 时被重新写回，删除无意义。
    if workspace_preset_skills()
        .iter()
        .any(|preset| preset.dir_name == dir_name)
    {
        return Err("内置技能不可删除。".to_string());
    }
    Ok(canonical_dir)
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

#[cfg(test)]
mod skill_remove_tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("eca-skill-remove-{label}-{}", Uuid::new_v4()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("skills")).expect("create skills root");
        root
    }

    fn write_skill(root: &Path, dir_name: &str) -> PathBuf {
        let dir = root.join("skills").join(dir_name);
        fs::create_dir_all(&dir).expect("create skill dir");
        let skill_md = dir.join("SKILL.md");
        fs::write(&skill_md, "---\nname: demo\ndescription: d\n---\n\nbody\n")
            .expect("write skill md");
        skill_md
    }

    #[test]
    fn resolve_removable_skill_dir_should_accept_direct_child() {
        let root = temp_root("child");
        let skill_md = write_skill(&root, "my-skill");
        let resolved = resolve_removable_skill_dir(&root.join("skills"), &skill_md.to_string_lossy())
            .expect("direct child should be removable");
        assert_eq!(resolved.file_name().and_then(|v| v.to_str()), Some("my-skill"));
    }

    #[test]
    fn resolve_removable_skill_dir_should_reject_outside_root() {
        let root = temp_root("outside");
        // 构造 skills 根之外的技能目录，越界删除必须被拒绝。
        let stray = root.join("stray-skill");
        fs::create_dir_all(&stray).expect("create stray dir");
        let stray_md = stray.join("SKILL.md");
        fs::write(&stray_md, "---\nname: stray\ndescription: d\n---\n\nbody\n")
            .expect("write stray skill md");
        let result = resolve_removable_skill_dir(&root.join("skills"), &stray_md.to_string_lossy());
        assert!(result.is_err(), "skills 根之外的目录不应可删");
    }

    #[test]
    fn resolve_removable_skill_dir_should_reject_builtin_preset() {
        let root = temp_root("builtin");
        let skill_md = write_skill(&root, "pai-guide");
        let result = resolve_removable_skill_dir(&root.join("skills"), &skill_md.to_string_lossy());
        assert!(result.is_err(), "内置预设技能不应可删");
    }

    #[test]
    fn resolve_removable_skill_dir_should_reject_missing_dir() {
        let root = temp_root("missing");
        let missing_md = root.join("skills").join("ghost").join("SKILL.md");
        let result = resolve_removable_skill_dir(&root.join("skills"), &missing_md.to_string_lossy());
        assert!(result.is_err(), "不存在的技能目录不应可删");
    }
}

