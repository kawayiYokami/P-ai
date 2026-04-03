use super::*;

fn llm_workspace_skills_root_at(workspace_root: &Path) -> PathBuf {
    workspace_root.join("skills")
}

fn llm_workspace_skills_root(state: &AppState) -> Result<PathBuf, String> {
    Ok(llm_workspace_skills_root_at(&configured_workspace_root_path(state)?))
}

fn sync_skill_template_file(path: &PathBuf, content: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    fs::write(path, content).map_err(|err| format!("Write file failed ({}): {err}", path.display()))
}

fn sync_workspace_preset_skill(
    skills_root: &PathBuf,
    skill_dir_name: &str,
    skill_md: &str,
) -> Result<(), String> {
    let dir = skills_root.join(skill_dir_name);
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Create preset skill dir failed ({}): {err}", dir.display()))?;
    sync_skill_template_file(&dir.join("SKILL.md"), skill_md)
}

pub(crate) fn ensure_workspace_skills_layout(state: &AppState) -> Result<(), String> {
    let workspace_root = ensure_workspace_root_ready(&configured_workspace_root_path(state)?)?;
    ensure_workspace_skills_layout_at_root(&workspace_root)
}

pub(crate) fn ensure_workspace_skills_layout_at_root(workspace_root: &Path) -> Result<(), String> {
    let skills_root = llm_workspace_skills_root_at(&workspace_root);
    fs::create_dir_all(&skills_root)
        .map_err(|err| format!("Create skills dir failed ({}): {err}", skills_root.display()))?;

    let legacy_readme = skills_root.join("README.md");
    if legacy_readme.exists() {
        let _ = fs::remove_file(&legacy_readme);
    }

    sync_workspace_preset_skill(
        &skills_root,
        "browser-automation",
        include_str!("../../../resources/preset-skills/browser-automation/SKILL.md"),
    )?;
    sync_workspace_preset_skill(
        &skills_root,
        "news-analyst",
        include_str!("../../../resources/preset-skills/news-analyst/SKILL.md"),
    )?;
    sync_workspace_preset_skill(
        &skills_root,
        "agent-office",
        include_str!("../../../resources/preset-skills/agent-office/SKILL.md"),
    )?;
    sync_workspace_preset_skill(
        &skills_root,
        "assistant-interaction-guide",
        include_str!("../../../resources/preset-skills/assistant-interaction-guide/SKILL.md"),
    )?;
    sync_workspace_preset_skill(
        &skills_root,
        "skill-setup",
        include_str!("../../../resources/preset-skills/skill-setup/SKILL.md"),
    )?;
    sync_workspace_preset_skill(
        &skills_root,
        "mcp-setup",
        include_str!("../../../resources/preset-skills/mcp-setup/SKILL.md"),
    )?;
    sync_workspace_preset_skill(
        &skills_root,
        "workspace-guide",
        include_str!("../../../resources/preset-skills/workspace-guide/SKILL.md"),
    )?;
    sync_workspace_preset_skill(
        &skills_root,
        "private-organization-guide",
        include_str!("../../../resources/preset-skills/private-organization-guide/SKILL.md"),
    )?;
    sync_workspace_preset_skill(
        &skills_root,
        "pai-guide",
        include_str!("../../../resources/preset-skills/pai-guide/SKILL.md"),
    )?;

    Ok(())
}

fn parse_skill_file(skill_md_path: &PathBuf) -> Result<(String, String, String), String> {
    let content = fs::read_to_string(skill_md_path)
        .map_err(|err| format!("Read SKILL.md failed ({}): {err}", skill_md_path.display()))?;
    let mut lines = content.lines();
    let first = lines
        .next()
        .unwrap_or_default()
        .trim_start_matches('\u{feff}')
        .trim()
        .to_string();
    if first != "---" {
        return Err("SKILL.md must start with YAML frontmatter".to_string());
    }
    let mut name = String::new();
    let mut description = String::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        let Some((key, raw_value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let mut value = raw_value.trim().to_string();
        if value.len() >= 2 {
            let bytes = value.as_bytes();
            let first = bytes[0];
            let last = bytes[value.len() - 1];
            if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
                value = value[1..value.len() - 1].to_string();
            }
        }
        if key == "name" {
            name = value;
        } else if key == "description" {
            description = value;
        }
    }
    if name.trim().is_empty() {
        name = skill_md_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|v| v.to_str())
            .unwrap_or("skill")
            .to_string();
    }
    let body = if let Some((_, rest)) = content.split_once("\n---") {
        rest.trim_start_matches(['\r', '\n']).trim().to_string()
    } else {
        String::new()
    };
    Ok((name, description, body))
}

pub(crate) fn load_workspace_skill_summaries_with_errors(
    state: &AppState,
) -> Result<(Vec<SkillSummaryItem>, Vec<WorkspaceLoadError>), String> {
    ensure_workspace_skills_layout(state)?;
    let mut skills = Vec::<SkillSummaryItem>::new();
    let mut errors = Vec::<WorkspaceLoadError>::new();
    let skills_dir = llm_workspace_skills_root(state)?;
    let mut dirs = fs::read_dir(&skills_dir)
        .map_err(|err| format!("Read skills dir failed ({}): {err}", skills_dir.display()))?
        .filter_map(|entry| entry.ok().map(|v| v.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    for dir in dirs {
        let skill_md = dir.join("SKILL.md");
        if !skill_md.is_file() {
            errors.push(WorkspaceLoadError {
                item: dir.to_string_lossy().to_string(),
                error: "SKILL.md not found".to_string(),
            });
            continue;
        }
        match parse_skill_file(&skill_md) {
            Ok((name, description, content)) => {
                skills.push(SkillSummaryItem {
                    name,
                    description,
                    content,
                    path: skill_md.to_string_lossy().to_string(),
                });
            }
            Err(err) => errors.push(WorkspaceLoadError {
                item: skill_md.to_string_lossy().to_string(),
                error: err,
            }),
        }
    }
    Ok((skills, errors))
}

pub(crate) fn render_skill_summary(skills: &[SkillSummaryItem]) -> String {
    if skills.is_empty() {
        return "No skills found in current skills directory.".to_string();
    }
    let mut lines = Vec::<String>::new();
    for item in skills {
        let desc = if item.description.trim().is_empty() {
            "(no description)"
        } else {
            item.description.trim()
        };
        lines.push(format!("- {}: {}", item.name.trim(), desc));
    }
    lines.join("\n")
}

fn render_hidden_skill_snapshot_block(
    state: &AppState,
    skills: &[SkillSummaryItem],
    scan_error: Option<&str>,
) -> String {
    let skills_root_path = llm_workspace_skills_root(state)
        .unwrap_or_else(|_| state.llm_workspace_path.join("skills"));
    let skills_root = skills_root_path.to_string_lossy();
    if let Some(err) = scan_error {
        return prompt_xml_block(
            "skill usage",
            format!(
                "System skill directory path: {}\nscan failed: {}",
                skills_root, err
            ),
        );
    }
    let example_path = skills
        .iter()
        .find(|item| item.name.trim().eq_ignore_ascii_case("workspace-guide"))
        .map(|item| item.path.trim().to_string())
        .unwrap_or_else(|| {
            skills_root_path
                .join("workspace-guide")
                .join("SKILL.md")
                .to_string_lossy()
                .to_string()
        });
    let summary = render_skill_summary(skills);
    format!(
        "{}\n\n{}",
        prompt_xml_block(
            "skill usage",
            format!(
                "System skill directory path: {}\n\nhow to read skill:\nexample:\nworkspace-guide\nread this path: {}",
                skills_root, example_path
            ),
        ),
        prompt_xml_block("skill index", summary)
    )
}

pub(crate) fn update_hidden_skill_snapshot_cache(
    state: &AppState,
    skills: &[SkillSummaryItem],
    scan_error: Option<&str>,
) -> Result<String, String> {
    let snapshot = render_hidden_skill_snapshot_block(state, skills, scan_error);
    let mut guard = state
        .hidden_skill_snapshot_cache
        .lock()
        .map_err(|_| "Failed to lock hidden skill snapshot cache".to_string())?;
    *guard = snapshot.clone();
    Ok(snapshot)
}

pub(crate) fn warm_hidden_skill_snapshot_cache(state: &AppState) -> Result<String, String> {
    match load_workspace_skill_summaries_with_errors(state) {
        Ok((skills, _errors)) => update_hidden_skill_snapshot_cache(state, &skills, None),
        Err(err) => update_hidden_skill_snapshot_cache(state, &[], Some(err.as_str())),
    }
}

pub(crate) fn build_hidden_skill_snapshot_block(state: &AppState) -> String {
    match state.hidden_skill_snapshot_cache.lock() {
        Ok(guard) if !guard.trim().is_empty() => guard.clone(),
        _ => String::new(),
    }
}

pub(crate) fn refresh_workspace_mcp_and_skills(state: &AppState) -> Result<RefreshMcpAndSkillsResult, String> {
    ensure_workspace_mcp_layout(state)?;
    ensure_workspace_skills_layout(state)?;
    ensure_workspace_private_organization_layout(state)?;
    let (servers, mcp_errors) = load_workspace_mcp_servers_with_errors(state)?;
    let (skills, skill_errors) = load_workspace_skill_summaries_with_errors(state)?;
    if let Err(err) = update_hidden_skill_snapshot_cache(state, &skills, None) {
        runtime_log_error(format!(
            "[技能工作区] 更新隐藏技能快照缓存失败: skills={}, error={}",
            skills.len(),
            err
        ));
    }
    let mut config = read_config(&state.config_path)?;
    let mut data = read_app_data(&state.data_path)?;
    let private_org = merge_private_organization_into_runtime(&state.data_path, &mut config, &mut data.agents)?;
    let mcp_loaded = servers.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    let skills_loaded = skills.iter().map(|s| s.name.clone()).collect::<Vec<_>>();
    let skill_summary = render_skill_summary(&skills);
    Ok(RefreshMcpAndSkillsResult {
        mcp_loaded,
        mcp_failed: mcp_errors
            .into_iter()
            .map(|v| WorkspaceLoadError {
                item: v.item,
                error: v.error,
            })
            .collect(),
        skills_loaded,
        skills_failed: skill_errors,
        skills: skills.clone(),
        skill_summary,
        private_agents_loaded: private_org.private_agents_loaded,
        private_agents_failed: private_org.private_agents_failed,
        private_departments_loaded: private_org.private_departments_loaded,
        private_departments_failed: private_org.private_departments_failed,
    })
}

pub(crate) fn open_skills_workspace_dir(state: &AppState) -> Result<String, String> {
    ensure_workspace_skills_layout(state)?;
    let path = llm_workspace_skills_root(state)?;
    open_path_in_file_manager(&path)?;
    Ok(path.to_string_lossy().to_string())
}
