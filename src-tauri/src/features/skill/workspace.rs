use super::*;

pub(crate) fn hidden_skill_summaries_cache(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, Vec<SkillSummaryItem>>> {
    static CACHE: OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Vec<SkillSummaryItem>>>,
    > = OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

pub(crate) fn hidden_skill_cache_scope_key(state: &AppState) -> String {
    state.data_path.display().to_string()
}

fn llm_workspace_skills_root_at(workspace_root: &Path) -> PathBuf {
    workspace_root.join("skills")
}

pub(crate) fn llm_workspace_skills_root(state: &AppState) -> Result<PathBuf, String> {
    Ok(llm_workspace_skills_root_at(&configured_workspace_root_path(state)?))
}

fn sync_skill_template_file(path: &PathBuf, content: &str) -> Result<(), String> {
    if path.exists() {
        if let Ok(current) = fs::read_to_string(path) {
            if current == content {
                return Ok(());
            }
        }
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

    for skill in workspace_preset_skills() {
        sync_workspace_preset_skill(&skills_root, skill.dir_name, skill.skill_md)?;
    }

    Ok(())
}

pub(crate) fn parse_skill_file(skill_md_path: &PathBuf) -> Result<(String, String, String), String> {
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
                value = value.replace("\\\"", "\"").replace("\\\\", "\\");
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
    let enabled_map = load_skill_enabled_map(state)?;
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
            errors.push(WorkspaceLoadError::with_hint(
                dir.to_string_lossy().to_string(),
                "SKILL.md not found",
                "在该技能目录下创建 SKILL.md，或移除/重命名这个无效技能目录。",
            ));
            continue;
        }
        let additional_files = scan_skill_additional_files(&dir, &skill_md);
        let dir_name_str = dir.file_name().and_then(|v| v.to_str()).unwrap_or_default();
        let is_builtin = workspace_preset_skills()
            .iter()
            .any(|preset| preset.dir_name == dir_name_str);
        match parse_skill_file(&skill_md) {
            Ok((name, description, content)) => {
                let enabled = enabled_map.get(&name).copied().unwrap_or(true);
                skills.push(SkillSummaryItem {
                    name,
                    description,
                    content,
                    path: skill_md.to_string_lossy().to_string(),
                    additional_files,
                    is_builtin,
                    enabled,
                });
            }
            Err(err) => errors.push(WorkspaceLoadError::with_hint(
                skill_md.to_string_lossy().to_string(),
                err,
                "检查 SKILL.md 的 frontmatter、name/description 与正文格式，修复后重新调用 reload。",
            )),
        }
    }
    Ok((skills, errors))
}

fn scan_skill_additional_files(skill_dir: &Path, skill_md: &Path) -> Vec<SkillFileItem> {
    let mut files = Vec::new();
    let mut dirs_to_visit = vec![skill_dir.to_path_buf()];
    while let Some(current_dir) = dirs_to_visit.pop() {
        if let Ok(entries) = fs::read_dir(&current_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();
                // 用 file_type() 而非 path.is_dir()/is_file()，不跟随符号链接：
                // 既不因指向父目录的链接遍历成环，也不越出技能目录
                let file_type = match entry.file_type() {
                    Ok(file_type) => file_type,
                    Err(_) => continue,
                };
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    if file_name.starts_with('.') || file_name == "node_modules" || file_name == "target" || file_name == "__pycache__" {
                        continue;
                    }
                    dirs_to_visit.push(path);
                } else if file_type.is_file() {
                    if path == *skill_md {
                        continue;
                    }
                    if let Ok(rel) = path.strip_prefix(skill_dir) {
                        let rel_str = rel.to_string_lossy().replace('\\', "/");
                        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                        files.push(SkillFileItem {
                            name: file_name,
                            relative_path: rel_str,
                            size_bytes,
                        });
                    }
                }
            }
        }
    }
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files
}

fn update_skill_frontmatter(
    raw: &str,
    new_name: Option<&str>,
    new_description: Option<&str>,
    fallback_name: &str,
) -> String {
    let raw_clean = raw.trim_start_matches('\u{feff}');
    let existing_lines = if raw_clean.starts_with("---") {
        if let Some(pos) = raw_clean[3..].find("\n---") {
            let inner = &raw_clean[3..3 + pos];
            inner.lines().map(|s| s.to_string()).collect::<Vec<String>>()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    let mut lines = existing_lines;

    let format_yaml_str = |val: &str| -> String {
        let trimmed = val.trim();
        if trimmed.contains(':')
            || trimmed.contains('#')
            || trimmed.contains('\'')
            || trimmed.contains('"')
            || trimmed.contains('\n')
            || trimmed.starts_with('@')
            || trimmed.starts_with('`')
            || trimmed.starts_with('%')
            || trimmed.is_empty()
        {
            let escaped = trimmed
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\r', "")
                .replace('\n', " ");
            format!("\"{escaped}\"")
        } else {
            trimmed.to_string()
        }
    };

    if let Some(name) = new_name {
        let trimmed_name = name.trim();
        let effective_name = if trimmed_name.is_empty() {
            fallback_name
        } else {
            trimmed_name
        };
        let name_val = format_yaml_str(effective_name);
        let name_line = format!("name: {name_val}");
        let mut found = false;
        for line in lines.iter_mut() {
            if line.trim().starts_with("name:") {
                *line = name_line.clone();
                found = true;
                break;
            }
        }
        if !found {
            lines.insert(0, name_line);
        }
    } else if !lines.iter().any(|l| l.trim().starts_with("name:")) {
        lines.insert(0, format!("name: {}", format_yaml_str(fallback_name)));
    }

    if let Some(desc) = new_description {
        let desc_val = format_yaml_str(desc);
        let desc_line = format!("description: {desc_val}");
        let mut found = false;
        for line in lines.iter_mut() {
            if line.trim().starts_with("description:") {
                *line = desc_line.clone();
                found = true;
                break;
            }
        }
        if !found {
            let insert_idx = lines
                .iter()
                .position(|l| l.trim().starts_with("name:"))
                .map(|i| i + 1)
                .unwrap_or(lines.len());
            lines.insert(insert_idx, desc_line);
        }
    }

    format!("---\n{}\n---", lines.join("\n").trim())
}

pub(crate) fn save_workspace_skill_content(
    state: &AppState,
    path: &str,
    new_content: &str,
    new_name: Option<&str>,
    new_description: Option<&str>,
) -> Result<SkillSummaryItem, String> {
    let skill_path = PathBuf::from(path);
    if !skill_path.is_file() {
        return Err(format!("Skill file not found: {path}"));
    }
    let skills_root = llm_workspace_skills_root(state)?;
    let canonical_root = skills_root.canonicalize().map_err(|e| e.to_string())?;
    let canonical_file = skill_path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err("Permission denied: skill file is outside the skills workspace directory".to_string());
    }

    let skill_dir_name = canonical_file
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|v| v.to_str())
        .unwrap_or_default();
    let is_builtin = workspace_preset_skills()
        .iter()
        .any(|preset| preset.dir_name == skill_dir_name);
    if is_builtin {
        return Err("系统内置技能不可修改".to_string());
    }

    let raw_existing = fs::read_to_string(&canonical_file)
        .map_err(|e| format!("Failed to read SKILL.md: {e}"))?;

    let fallback_name = if skill_dir_name.is_empty() { "skill" } else { skill_dir_name };
    let frontmatter = update_skill_frontmatter(&raw_existing, new_name, new_description, fallback_name);

    let updated_full_text = format!("{}\n\n{}", frontmatter.trim_end(), new_content.trim());
    fs::write(&canonical_file, updated_full_text)
        .map_err(|e| format!("Failed to write SKILL.md: {e}"))?;

    let (name, description, content) = parse_skill_file(&canonical_file)?;
    let skill_dir = canonical_file.parent().unwrap_or(&canonical_root);
    let additional_files = scan_skill_additional_files(skill_dir, &canonical_file);

    let summary_item = SkillSummaryItem {
        name,
        description,
        content,
        path: canonical_file.to_string_lossy().to_string(),
        additional_files,
        is_builtin,
        enabled: true,
    };

    if let Ok((skills, _)) = load_workspace_skill_summaries_with_errors(state) {
        let _ = update_hidden_skill_snapshot_cache(state, &skills, None);
    }

    Ok(summary_item)
}

pub(crate) fn read_workspace_skill_file(
    state: &AppState,
    path: &str,
) -> Result<String, String> {
    let target_path = PathBuf::from(path);
    if !target_path.is_file() {
        return Err(format!("File not found: {path}"));
    }
    let skills_root = llm_workspace_skills_root(state)?;
    let canonical_root = skills_root.canonicalize().map_err(|e| e.to_string())?;
    let canonical_file = target_path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err("Permission denied: file is outside the skills workspace directory".to_string());
    }

    fs::read_to_string(&canonical_file)
        .map_err(|e| format!("Failed to read file: {e}"))
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

fn filter_skills_for_agent(
    agent: Option<&AgentProfile>,
    skills: &[SkillSummaryItem],
) -> Vec<SkillSummaryItem> {
    skills
        .iter()
        .filter(|item| {
            agent_permission_allows_any_name(
                agent,
                AgentPermissionCategory::Skill,
                &[item.name.as_str()],
            )
        })
        .cloned()
        .collect()
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
        .find(|item| item.name.trim().eq_ignore_ascii_case("assistant-space-guide"))
        .map(|item| item.path.trim().to_string())
        .unwrap_or_else(|| {
            skills_root_path
                .join("assistant-space-guide")
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
                "System skill directory path: {}\n\nhow to read skill:\nexample:\nassistant-space-guide\nread this path: {}",
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
    // 全局关闭的 Skill 不进入提示词注入链路，也不参与人格白名单过滤。
    let injectable = skills
        .iter()
        .filter(|item| item.enabled)
        .cloned()
        .collect::<Vec<_>>();
    let snapshot = render_hidden_skill_snapshot_block(state, &injectable, scan_error);
    let mut guard = state
        .hidden_skill_snapshot_cache
        .lock()
        .map_err(|_| "Failed to lock hidden skill snapshot cache".to_string())?;
    *guard = snapshot.clone();
    drop(guard);

    let mut summaries_guard = hidden_skill_summaries_cache()
        .lock()
        .map_err(|_| "Failed to lock hidden skill summaries cache".to_string())?;
    summaries_guard.insert(hidden_skill_cache_scope_key(state), injectable);
    Ok(snapshot)
}

pub(crate) fn clear_hidden_skill_snapshot_cache(state: &AppState) -> Result<(), String> {
    let mut guard = state
        .hidden_skill_snapshot_cache
        .lock()
        .map_err(|_| "Failed to lock hidden skill snapshot cache".to_string())?;
    *guard = String::new();
    drop(guard);

    let mut summaries_guard = hidden_skill_summaries_cache()
        .lock()
        .map_err(|_| "Failed to lock hidden skill summaries cache".to_string())?;
    summaries_guard.remove(&hidden_skill_cache_scope_key(state));
    Ok(())
}

pub(crate) fn build_hidden_skill_snapshot_block(state: &AppState) -> String {
    match state.hidden_skill_snapshot_cache.lock() {
        Ok(guard) if !guard.trim().is_empty() => guard.clone(),
        _ => String::new(),
    }
}

pub(crate) fn build_hidden_skill_snapshot_block_for_agent(
    state: &AppState,
    agent: Option<&AgentProfile>,
) -> String {
    if agent
        .map(|item| !item.permission_control.enabled)
        .unwrap_or(true)
    {
        return build_hidden_skill_snapshot_block(state);
    }
    let cache_key = hidden_skill_cache_scope_key(state);
    let cached_skills = hidden_skill_summaries_cache()
        .lock()
        .ok()
        .and_then(|guard| guard.get(&cache_key).cloned());
    match cached_skills {
        Some(skills) => {
            let filtered = filter_skills_for_agent(agent, &skills);
            render_hidden_skill_snapshot_block(state, &filtered, None)
        }
        None => {
            runtime_log_warn(
                "[技能工作区] 隐藏技能快照未命中结构化缓存，返回现有快照文本；如需更新请显式刷新技能工作区。"
                    .to_string(),
            );
            build_hidden_skill_snapshot_block(state)
        }
    }
}

/// 某人格当前可用的 skill（已启用 + 权限允许）。
fn agent_available_skills(state: &AppState, agent: &AgentProfile) -> Vec<SkillSummaryItem> {
    let cache_key = hidden_skill_cache_scope_key(state);
    let cached_skills = hidden_skill_summaries_cache()
        .lock()
        .ok()
        .and_then(|guard| guard.get(&cache_key).cloned());
    match cached_skills {
        Some(skills) => filter_skills_for_agent(Some(agent), &skills),
        None => {
            runtime_log_warn(
                "[技能工作区] 人格 skill 注入未命中结构化缓存，本次跳过注入；如需更新请显式刷新技能工作区。"
                    .to_string(),
            );
            Vec::new()
        }
    }
}

/// 常驻 skill 全文注入（结论 24）：把这些 SKILL.md 的正文整段拼进该人格系统提示词，模型无需读文件。
/// skill 不在或未启用（或权限不允许）时直接跳过，不阻断装配。
pub(crate) fn build_resident_skill_fulltext_block(state: &AppState, agent: &AgentProfile) -> String {
    if agent.resident_skill_names.is_empty() {
        return String::new();
    }
    let available = agent_available_skills(state, agent);
    let mut sections = Vec::<String>::new();
    for name in &agent.resident_skill_names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let Some(skill) = available.iter().find(|item| item.name.trim() == name) else {
            continue;
        };
        let description = skill.description.trim();
        let header = if description.is_empty() {
            format!("### {}", skill.name.trim())
        } else {
            format!("### {}：{description}", skill.name.trim())
        };
        sections.push(format!("{header}\n\n{}", skill.content.trim()));
    }
    if sections.is_empty() {
        return String::new();
    }
    prompt_xml_block(
        "resident skills",
        format!(
            "## 你的常驻技能\n以下技能说明已全文注入，无需再读取文件。\n\n{}",
            sections.join("\n\n")
        ),
    )
}

/// 可选 skill 只注入引用（结论 8）：给名字与 SKILL.md 路径，需要时模型自行读取。
pub(crate) fn build_optional_skill_reference_block(state: &AppState, agent: &AgentProfile) -> String {
    if agent.optional_skill_names.is_empty() {
        return String::new();
    }
    let available = agent_available_skills(state, agent);
    let mut lines = Vec::<String>::new();
    for name in &agent.optional_skill_names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let Some(skill) = available.iter().find(|item| item.name.trim() == name) else {
            continue;
        };
        let description = skill.description.trim();
        if description.is_empty() {
            lines.push(format!("- {}：{}", skill.name.trim(), skill.path.trim()));
        } else {
            lines.push(format!(
                "- {}（{}）：{}",
                skill.name.trim(),
                description,
                skill.path.trim()
            ));
        }
    }
    if lines.is_empty() {
        return String::new();
    }
    prompt_xml_block(
        "optional skills",
        format!(
            "## 你的可选技能\n需要时自行读取对应 SKILL.md，不必全文记忆。\n\n{}",
            lines.join("\n")
        ),
    )
}

fn format_workspace_named_item(name: &str, id: &str) -> String {
    let trimmed_name = name.trim();
    let trimmed_id = id.trim();
    if trimmed_name.is_empty() {
        return trimmed_id.to_string();
    }
    if trimmed_id.is_empty() || trimmed_name == trimmed_id {
        return trimmed_name.to_string();
    }
    format!("{trimmed_name} ({trimmed_id})")
}

fn build_workspace_loaded_groups(
    servers: &[McpServerConfig],
    skills: &[SkillSummaryItem],
    agents: &[AgentProfile],
    private_agent_ids: &[String],
) -> Vec<WorkspaceLoadedGroup> {
    let mcp_items = servers
        .iter()
        .map(|server| format_workspace_named_item(&server.name, &server.id))
        .collect::<Vec<_>>();
    let skill_items = skills
        .iter()
        .map(|item| item.name.trim().to_string())
        .collect::<Vec<_>>();
    let private_agent_items = private_agent_ids
        .iter()
        .map(|id| {
            agents
                .iter()
                .find(|agent| agent.id == *id)
                .map(|agent| format_workspace_named_item(&agent.name, &agent.id))
                .unwrap_or_else(|| id.clone())
        })
        .collect::<Vec<_>>();
    vec![
        WorkspaceLoadedGroup {
            kind: "mcp".to_string(),
            label: "MCP".to_string(),
            count: mcp_items.len(),
            items: mcp_items,
        },
        WorkspaceLoadedGroup {
            kind: "skill".to_string(),
            label: "SKILL".to_string(),
            count: skill_items.len(),
            items: skill_items,
        },
        WorkspaceLoadedGroup {
            kind: "private_agent".to_string(),
            label: "私有人格".to_string(),
            count: private_agent_items.len(),
            items: private_agent_items,
        },
    ]
}

fn build_workspace_failed_groups(
    mcp_failed: &[WorkspaceLoadError],
    skills_failed: &[WorkspaceLoadError],
    private_agents_failed: &[WorkspaceLoadError],
) -> Vec<WorkspaceFailedGroup> {
    vec![
        WorkspaceFailedGroup {
            kind: "mcp".to_string(),
            label: "MCP".to_string(),
            count: mcp_failed.len(),
            items: mcp_failed.to_vec(),
        },
        WorkspaceFailedGroup {
            kind: "skill".to_string(),
            label: "SKILL".to_string(),
            count: skills_failed.len(),
            items: skills_failed.to_vec(),
        },
        WorkspaceFailedGroup {
            kind: "private_agent".to_string(),
            label: "私有人格".to_string(),
            count: private_agents_failed.len(),
            items: private_agents_failed.to_vec(),
        },
    ]
}

fn summarize_workspace_loaded_groups(groups: &[WorkspaceLoadedGroup]) -> String {
    let mut lines = Vec::<String>::new();
    for group in groups {
        let details = if group.items.is_empty() {
            "无".to_string()
        } else {
            group.items.join("、")
        };
        lines.push(format!(
            "{}：成功加载 {} 个；{}",
            group.label, group.count, details
        ));
    }
    lines.join("\n")
}

fn summarize_workspace_failed_groups(groups: &[WorkspaceFailedGroup]) -> String {
    let mut lines = Vec::<String>::new();
    for group in groups {
        if group.items.is_empty() {
            lines.push(format!("{}：0 个加载失败", group.label));
            continue;
        }
        lines.push(format!("{}：{} 个加载失败", group.label, group.count));
        for item in &group.items {
            if item.hint.trim().is_empty() {
                lines.push(format!("- {} | {}", item.item, item.error));
            } else {
                lines.push(format!("- {} | {} | 修复：{}", item.item, item.error, item.hint));
            }
        }
    }
    lines.join("\n")
}

fn collect_workspace_repair_items(groups: &[WorkspaceFailedGroup]) -> Vec<WorkspaceLoadError> {
    groups
        .iter()
        .flat_map(|group| group.items.iter().cloned())
        .collect::<Vec<_>>()
}

fn summarize_workspace_repair_items(items: &[WorkspaceLoadError]) -> String {
    if items.is_empty() {
        return "reload 完成，未发现需要修复的 LLM 工作区配置。".to_string();
    }
    let mut lines = Vec::<String>::new();
    lines.push(format!(
        "reload 已跳过 {} 个不合法配置；请按下面的 path/error/hint 修复后再次调用 reload。",
        items.len()
    ));
    for item in items {
        let hint = if item.hint.trim().is_empty() {
            "根据错误信息修复该配置项。"
        } else {
            item.hint.trim()
        };
        lines.push(format!(
            "- path: {}\n  error: {}\n  hint: {}",
            item.item, item.error, hint
        ));
    }
    lines.join("\n")
}

fn finalize_workspace_load_result(
    mut result: RefreshMcpAndSkillsResult,
    servers: &[McpServerConfig],
    merged_agents: &[AgentProfile],
) -> RefreshMcpAndSkillsResult {
    let loaded_groups = build_workspace_loaded_groups(
        servers,
        &result.skills,
        merged_agents,
        &result.private_agents_loaded,
    );
    let failed_groups = build_workspace_failed_groups(
        &result.mcp_failed,
        &result.skills_failed,
        &result.private_agents_failed,
    );
    let total_loaded = loaded_groups.iter().map(|group| group.count).sum::<usize>();
    let total_failed = failed_groups.iter().map(|group| group.count).sum::<usize>();
    let loaded_summary = summarize_workspace_loaded_groups(&loaded_groups);
    let failed_summary = summarize_workspace_failed_groups(&failed_groups);
    let repair_items = collect_workspace_repair_items(&failed_groups);
    let repair_summary = summarize_workspace_repair_items(&repair_items);
    let needs_repair = total_failed > 0;
    result.ok = !needs_repair;
    result.status = if needs_repair {
        "needs_repair".to_string()
    } else {
        "ok".to_string()
    };
    result.loaded_groups = loaded_groups;
    result.failed_groups = failed_groups;
    result.total_loaded = total_loaded;
    result.total_failed = total_failed;
    result.loaded_summary = loaded_summary;
    result.failed_summary = failed_summary;
    result.repair_summary = repair_summary;
    result.repair_items = repair_items;
    result.needs_repair = needs_repair;
    result
}

fn collect_workspace_load_snapshot(
    state: &AppState,
) -> Result<(RefreshMcpAndSkillsResult, Vec<McpServerConfig>, Vec<AgentProfile>), String> {
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
    let config = read_config(&state.config_path)?;
    let mut agents = state_read_agents_cached(state)?;
    let private_org = merge_private_organization_into_runtime(&state.data_path, &config, &mut agents)?;
    let mcp_loaded = servers.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    let skills_loaded = skills
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.name.clone())
        .collect::<Vec<_>>();
    // 摘要反映实际参与运行的 Skill，与注入内容保持一致。
    let enabled_skills = skills
        .iter()
        .filter(|s| s.enabled)
        .cloned()
        .collect::<Vec<_>>();
    let skill_summary = render_skill_summary(&enabled_skills);
    let result = RefreshMcpAndSkillsResult {
        mcp_loaded,
        ok: false,
        status: String::new(),
        mcp_failed: mcp_errors
            .into_iter()
            .map(|v| WorkspaceLoadError::with_hint(
                v.item,
                v.error,
                "检查该 MCP JSON 配置的格式、command/args/env 与启用状态；修复后重新调用 reload。",
            ))
            .collect(),
        skills_loaded,
        skills_failed: skill_errors,
        skills: skills.clone(),
        skill_summary,
        private_agents_loaded: private_org.private_agents_loaded,
        private_agents_failed: private_org.private_agents_failed,
        loaded_groups: Vec::new(),
        failed_groups: Vec::new(),
        total_loaded: 0,
        total_failed: 0,
        loaded_summary: String::new(),
        failed_summary: String::new(),
        repair_summary: String::new(),
        repair_items: Vec::new(),
        needs_repair: false,
    };
    Ok((result, servers, agents))
}

async fn disconnect_workspace_mcp_runtime_clients(state: &AppState) {
    match load_workspace_mcp_servers(state) {
        Ok(servers) => {
            for server in servers {
                mcp_disconnect_cached_client(&server.id).await;
                mcp_runtime_state_set(&server.id, false, "stopped", "", Vec::new());
            }
        }
        Err(err) => runtime_log_warn(format!(
            "[工作区加载] reload 前清理 MCP 运行态失败，继续尝试重新加载：{}",
            err
        )),
    }
}

pub(crate) async fn load_workspace(state: &AppState) -> Result<RefreshMcpAndSkillsResult, String> {
    let (mut result, servers, merged_agents) = collect_workspace_load_snapshot(state)?;
    match mcp_start_supervisor_probe_all_from_policy(state.clone(), "workspace_load") {
        Ok(()) => {}
        Err(err) => {
            result.mcp_failed.push(WorkspaceLoadError::with_hint(
                "mcp_supervisor_start",
                err,
                "检查 MCP 运行时启动日志和服务器配置；修复后重新调用 reload。",
            ));
        }
    }
    refresh_global_tool_schema_cache(state);
    mark_prompt_cache_rebuild_for_all_final_system_sources(state);
    Ok(finalize_workspace_load_result(result, &servers, &merged_agents))
}

pub(crate) async fn reload_workspace(
    state: &AppState,
) -> Result<RefreshMcpAndSkillsResult, String> {
    disconnect_workspace_mcp_runtime_clients(state).await;
    clear_global_tool_schema_cache();
    if let Err(err) = clear_hidden_skill_snapshot_cache(state) {
        runtime_log_warn(format!("[工作区加载] reload 前清空技能快照缓存失败：{}", err));
    }
    load_workspace(state).await
}

pub(crate) fn log_workspace_load_result(prefix: &str, result: &RefreshMcpAndSkillsResult) {
    let line = format!(
        "{} 状态=完成，成功加载={}，加载失败={}，需修复={}",
        prefix, result.total_loaded, result.total_failed, result.needs_repair
    );
    if result.total_failed > 0 || result.needs_repair {
        runtime_log_warn(line);
    } else {
        runtime_log_info(line);
    }
    if !result.loaded_summary.trim().is_empty() {
        for line in result.loaded_summary.lines() {
            runtime_log_info(format!("{} {}", prefix, line));
        }
    }
    if !result.failed_summary.trim().is_empty() {
        for line in result.failed_summary.lines() {
            runtime_log_info(format!("{} {}", prefix, line));
        }
    }
}

pub(crate) fn open_skills_workspace_dir(state: &AppState) -> Result<String, String> {
    ensure_workspace_skills_layout(state)?;
    let path = llm_workspace_skills_root(state)?;
    open_path_in_file_manager(&path)?;
    Ok(path.to_string_lossy().to_string())
}

pub(crate) fn open_skill_item_dir(state: &AppState, skill_path: &str) -> Result<String, String> {
    let p = PathBuf::from(skill_path);
    let target = if p.is_file() {
        p.parent().unwrap_or(&p).to_path_buf()
    } else {
        p
    };
    if target.exists() {
        open_path_in_file_manager(&target)?;
        Ok(target.to_string_lossy().to_string())
    } else {
        open_skills_workspace_dir(state)
    }
}
