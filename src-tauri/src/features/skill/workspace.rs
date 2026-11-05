use super::*;

pub(crate) fn hidden_skill_summaries_cache(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, Vec<SkillSummaryItem>>> {
    static CACHE: OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Vec<SkillSummaryItem>>>,
    > = OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// 助理空间技能清单缓存的键：非队长会话用这一个。
///
/// 键里必须能区分会话角色——托管技能按会话角色决定去留之后，
/// 同一个人格在队长会话与队员会话下拿到的列表不同，共用一个键会两边同时错。
/// 只有「队长」与「非队长」两种取值，不按会话 id 累积缓存。
pub(crate) fn hidden_skill_cache_scope_key(state: &AppState) -> String {
    state.data_path.display().to_string()
}

/// 队长会话那份清单的键。与 [`hidden_skill_cache_scope_key`] 只差一个后缀。
pub(crate) fn hidden_skill_leader_cache_key(state: &AppState) -> String {
    format!("{}#leader", hidden_skill_cache_scope_key(state))
}

/// 按会话缓存的项目专用 Skill 清单。
#[derive(Clone, Debug)]
pub(crate) struct ConversationProjectSkillsCacheEntry {
    /// 扫描来源项目工作目录；用于检测项目/工作树切换后需要重扫。无项目绑定时为 None。
    project_dir: Option<PathBuf>,
    skills: Vec<SkillSummaryItem>,
    last_activity: std::time::SystemTime,
}

/// 会话项目 Skill 缓存保留时长：3 天（按会话最后活动时间计算）。
const CONVERSATION_PROJECT_SKILLS_TTL_SECS: u64 = 3 * 24 * 60 * 60;

/// 会话绑定的项目 Skill 目录，按兼容来源优先级排列。
/// 工作树会话仍取项目工作目录，不取工作树目录。
fn conversation_project_skill_dirs(
    conversation: &Conversation,
    state: &AppState,
) -> Option<(PathBuf, Vec<PathBuf>)> {
    let root = conversation_user_main_workspace_root(conversation, state)?;
    let dirs = [".pai", ".claude", ".agents", ".codex"]
        .into_iter()
        .map(|folder| root.join(folder).join("skills"))
        .collect::<Vec<_>>();
    Some((root, dirs))
}

/// 扫描单个项目 skills 目录，解析出可选 Skill 清单。
/// 项目 Skill 不使用助理空间的启用状态表，默认全部启用、非内置。
fn load_project_skill_summaries_from_dir(
    skills_dir: &Path,
    seen_names: &mut std::collections::HashSet<String>,
) -> Vec<SkillSummaryItem> {
    let mut skills = Vec::<SkillSummaryItem>::new();
    let Ok(entries) = fs::read_dir(skills_dir) else {
        return skills;
    };
    let mut dirs = entries
        .filter_map(|entry| entry.ok().map(|value| value.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    for dir in dirs {
        let skill_md = dir.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let Ok((name, description, content)) = parse_skill_file(&skill_md) else {
            continue;
        };
        if !seen_names.insert(name.trim().to_string()) {
            continue;
        }
        let additional_files = scan_skill_additional_files(&dir, &skill_md);
        skills.push(SkillSummaryItem {
            name,
            description,
            content,
            path: skill_md.to_string_lossy().to_string(),
            additional_files,
            is_builtin: false,
            // 项目 Skill 永远不是托管：托管只来自随应用释放的预设清单。
            managed: false,
            enabled: true,
        });
    }
    skills
}

/// 失败时打印目录不存在、无 SKILL.md 或解析失败的具体原因，便于定位“项目 Skill 没进来”的环节。
fn log_project_skill_scan_result(skills_dir: &Path, skills: &[SkillSummaryItem]) {
    runtime_log_debug(format!(
        "[项目Skill] 目录扫描完成，dir={}，skills={}",
        skills_dir.display(),
        skills.len()
    ));
}

/// 懒加载当前会话的项目 Skill 清单：未初始化或项目目录变化时才重新扫描；
/// 每次命中都会刷新最后活动时间，并清理超过 3 天未活动的会话缓存。
pub(crate) fn ensure_conversation_project_skills(
    state: &AppState,
    conversation: &Conversation,
) -> Vec<SkillSummaryItem> {
    let conversation_id = conversation.id.trim().to_string();
    if conversation_id.is_empty() {
        return Vec::new();
    }
    let project_sources = conversation_project_skill_dirs(conversation, state);
    let project_dir = project_sources.as_ref().map(|(root, _)| root.clone());
    let now = std::time::SystemTime::now();
    let Ok(mut guard) = state.conversation_project_skills_cache.lock() else {
        runtime_log_warn(format!(
            "[项目Skill] 缓存加锁失败，conversation_id={conversation_id}"
        ));
        return Vec::new();
    };
    guard.retain(|_, entry| {
        now.duration_since(entry.last_activity)
            .map(|age| age.as_secs() < CONVERSATION_PROJECT_SKILLS_TTL_SECS)
            .unwrap_or(true)
    });
    if let Some(entry) = guard.get_mut(&conversation_id) {
        if entry.project_dir == project_dir {
            entry.last_activity = now;
            return entry.skills.clone();
        }
    }
    let skills = match project_sources.as_ref() {
        Some((_, dirs)) => {
            let mut seen_names = std::collections::HashSet::new();
            let mut skills = Vec::new();
            for dir in dirs {
                let loaded = load_project_skill_summaries_from_dir(dir, &mut seen_names);
                log_project_skill_scan_result(dir, &loaded);
                skills.extend(loaded);
            }
            skills
        }
        None => Vec::new(),
    };
    runtime_log_debug(format!(
        "[项目Skill] 会话缓存建立完成，conversation_id={conversation_id}，project_dir={}，skills={}",
        project_dir
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".to_string()),
        skills.len()
    ));
    guard.insert(
        conversation_id,
        ConversationProjectSkillsCacheEntry {
            project_dir,
            skills: skills.clone(),
            last_activity: now,
        },
    );
    skills
}

/// 失效单个会话的项目 Skill 缓存（压缩、项目/工作树切换后使用）。
pub(crate) fn invalidate_conversation_project_skills(state: &AppState, conversation_id: &str) {
    let key = conversation_id.trim();
    if key.is_empty() {
        return;
    }
    if let Ok(mut guard) = state.conversation_project_skills_cache.lock() {
        guard.remove(key);
    }
}

/// 清空全部会话的项目 Skill 缓存（手动 reload 时使用）。
pub(crate) fn clear_all_conversation_project_skills(state: &AppState) {
    if let Ok(mut guard) = state.conversation_project_skills_cache.lock() {
        guard.clear();
    }
}

/// 全局（助理空间）可注入的 Skill 结构化清单；缓存缺失时返回 None。
///
/// `team_leader = false` 取「非托管且已启用」那一份，人格常驻 / 可选技能也从这里取，
/// 所以托管技能天然进不了人格配置；`true` 取队长会话那一份，额外带上托管技能。
fn global_enabled_skills(state: &AppState, team_leader: bool) -> Option<Vec<SkillSummaryItem>> {
    let cache_key = if team_leader {
        hidden_skill_leader_cache_key(state)
    } else {
        hidden_skill_cache_scope_key(state)
    };
    hidden_skill_summaries_cache()
        .lock()
        .ok()
        .and_then(|guard| guard.get(&cache_key).cloned())
}

/// 重扫助理空间 Skill 并回填两份清单缓存（`hidden_skill_summaries_cache` 与字符串快照）。
///
/// 这是「清空」的唯一配对动作：`clear_hidden_skill_snapshot_cache` 只清不建，
/// 清完之后必须有人重新扫盘，否则本次进程余下的装配只能拿到空全局清单，助理空间技能会静默消失。
/// 只重扫技能这一块，不碰 MCP 运行时客户端——写命令的代价不该等同于一次 `reload_workspace`。
/// 扫盘是同步 fs：先扫完，再由 [`update_hidden_skill_snapshot_cache`] 回填，不持锁扫盘。
pub(crate) fn refresh_hidden_skill_snapshot(state: &AppState) -> Result<(), String> {
    let (skills, _errors) = load_workspace_skill_summaries_with_errors(state)?;
    update_hidden_skill_snapshot_cache(state, &skills, None).map(|_| ())
}

/// 项目 Skill 覆盖同名助理空间 Skill，其余合并。
fn merge_skill_lists(
    mut base: Vec<SkillSummaryItem>,
    project: Vec<SkillSummaryItem>,
) -> Vec<SkillSummaryItem> {
    if project.is_empty() {
        return base;
    }
    let project_names = project
        .iter()
        .map(|item| item.name.trim().to_string())
        .collect::<std::collections::HashSet<_>>();
    base.retain(|item| !project_names.contains(item.name.trim()));
    base.extend(project);
    base
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
    load_skill_summaries_from_root(&configured_workspace_root_path(state)?)
}

/// 纯读：按给定工作区根解析 Skill 清单，不建目录、不铺预设、不写任何缓存。
///
/// `workspace_root` 必须与渲染侧（`llm_workspace_skills_root`）同一形态，不在这里自行规范化：
/// 渲染用路径前缀判断「系统技能 / 项目技能」，两边形态不一致会让 `[system]` / `[project]` 分组错位。
pub(crate) fn load_skill_summaries_from_root(
    workspace_root: &Path,
) -> Result<(Vec<SkillSummaryItem>, Vec<WorkspaceLoadError>), String> {
    let mut skills = Vec::<SkillSummaryItem>::new();
    let mut errors = Vec::<WorkspaceLoadError>::new();
    let enabled_map = load_skill_enabled_map_at_root(workspace_root)?;
    let skills_dir = llm_workspace_skills_root_at(workspace_root);
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
        // 托管只按目录名认：清单是代码写死的，目录名与技能名一一对应。
        let managed = managed_injection_for_dir(dir_name_str).is_some();
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
                    managed,
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
        // 走到这里的一定是自定义技能：内置（含托管）在上面的 is_builtin 处已被拒。
        managed: false,
        enabled: true,
    };

    if let Err(err) = refresh_hidden_skill_snapshot(state) {
        runtime_log_warn(format!(
            "[技能工作区] 保存技能后刷新技能快照失败：{err}"
        ));
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

fn skill_snapshot_path_under_root(skill_path: &str, root: &std::path::Path) -> bool {
    let normalized_skill = skill_path.replace('\\', "/").to_lowercase();
    let mut normalized_root = root.to_string_lossy().replace('\\', "/").to_lowercase();
    if !normalized_root.ends_with('/') {
        normalized_root.push('/');
    }
    normalized_skill.starts_with(&normalized_root)
}

/// 快照索引渲染：纯助理空间 Skill 保持旧格式（名字+描述），避免无意义的提示词膨胀；
/// 一旦混入项目 Skill（路径不在 System 目录下），按双根树形输出，公共前缀只在根上写一次：
/// [system] + System 根 + 相对路径，[project] + 各项目 skills 根 + 相对路径，
/// 每行再带名字与说明，否则模型会按 System 目录拼接路径，读到不存在的文件。
fn render_skill_index_for_snapshot(
    skills: &[SkillSummaryItem],
    skills_root: &std::path::Path,
) -> String {
    if skills.is_empty() {
        return render_skill_summary(skills);
    }
    let has_external = skills
        .iter()
        .any(|item| !skill_snapshot_path_under_root(item.path.trim(), skills_root));
    if !has_external {
        return render_skill_summary(skills);
    }
    fn skill_desc(item: &SkillSummaryItem) -> &str {
        let desc = item.description.trim();
        if desc.is_empty() {
            "(no description)"
        } else {
            desc
        }
    }
    fn relative_under(path: &str, root: &std::path::Path) -> String {
        let normalized = path.replace('\\', "/");
        let mut root_str = root.to_string_lossy().replace('\\', "/");
        if !root_str.ends_with('/') {
            root_str.push('/');
        }
        if let Some(rel) = normalized
            .to_lowercase()
            .strip_prefix(&root_str.to_lowercase())
            .and_then(|_| normalized.get(root_str.len()..))
        {
            rel.to_string()
        } else {
            normalized
        }
    }
    fn root_display(root: &std::path::Path) -> String {
        let mut s = root.to_string_lossy().replace('\\', "/");
        if !s.ends_with('/') {
            s.push('/');
        }
        s
    }
    let mut out = Vec::<String>::new();
    let internal: Vec<&SkillSummaryItem> = skills
        .iter()
        .filter(|item| skill_snapshot_path_under_root(item.path.trim(), skills_root))
        .collect();
    if !internal.is_empty() {
        out.push("[system]".to_string());
        out.push(root_display(skills_root));
        for item in internal {
            let rel = relative_under(item.path.trim(), skills_root);
            out.push(format!(
                "  {} \u{2014} {}\u{ff1a}{}",
                rel,
                item.name.trim(),
                skill_desc(item)
            ));
        }
    }
    // 项目 Skill 按各自的 skills 根分组（如 <project>/.pai/skills/），同根共一棵树。
    let mut groups: std::collections::BTreeMap<String, Vec<&SkillSummaryItem>> =
        std::collections::BTreeMap::new();
    for item in skills
        .iter()
        .filter(|item| !skill_snapshot_path_under_root(item.path.trim(), skills_root))
    {
        let path = std::path::PathBuf::from(item.path.trim());
        let group_root = path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| root_display(p))
            .unwrap_or_else(|| item.path.trim().to_string());
        groups.entry(group_root).or_default().push(item);
    }
    for (group_root, items) in groups {
        if !out.is_empty() {
            out.push(String::new());
        }
        out.push("[project]".to_string());
        out.push(group_root.clone());
        // 组根用于求相对路径：去掉尾部斜杠转回 Path
        let group_root_path = std::path::PathBuf::from(group_root.trim_end_matches('/'));
        for item in items {
            let rel = relative_under(item.path.trim(), &group_root_path);
            out.push(format!(
                "  {} \u{2014} {}\u{ff1a}{}",
                rel,
                item.name.trim(),
                skill_desc(item)
            ));
        }
    }
    out.join("\n")
}

fn filter_skills_for_agent(
    agent: Option<&AgentProfile>,
    skills: &[SkillSummaryItem],
) -> Vec<SkillSummaryItem> {
    skills
        .iter()
        .filter(|item| {
            agent_permission_allows_name(agent, AgentPermissionCategory::Skill, &item.name)
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
    // 示例名必须与示例路径的真实 Skill 对上：之前写死 assistant-space-guide，
    // 路径却取 p-ai-config-guide，模型照抄就会读错文件。
    let example_item = skills
        .iter()
        .find(|item| item.name.trim().eq_ignore_ascii_case("p-ai-config-guide"))
        .or_else(|| skills.first());
    let (example_name, example_path) = match example_item {
        Some(item) => (
            item.name.trim().to_string(),
            item.path.trim().to_string(),
        ),
        None => (
            "p-ai-config-guide".to_string(),
            skills_root_path
                .join("p-ai-config-guide")
                .join("SKILL.md")
                .to_string_lossy()
                .to_string(),
        ),
    };
    let summary = render_skill_index_for_snapshot(skills, &skills_root_path);
    let has_external = skills
        .iter()
        .any(|item| !skill_snapshot_path_under_root(item.path.trim(), &skills_root_path));
    let usage = if has_external {
        format!(
            "System skill directory path: {}\n(global skills live here; project skills live outside it, use the full path in skill index)\n\nhow to read skill:\nexample:\n{}\nread this path: {}",
            skills_root, example_name, example_path
        )
    } else {
        format!(
            "System skill directory path: {}\n\nhow to read skill:\nexample:\n{}\nread this path: {}",
            skills_root, example_name, example_path
        )
    };
    format!(
        "{}\n\n{}",
        prompt_xml_block("skill usage", usage),
        prompt_xml_block("skill index", summary)
    )
}

pub(crate) fn update_hidden_skill_snapshot_cache(
    state: &AppState,
    skills: &[SkillSummaryItem],
    scan_error: Option<&str>,
) -> Result<String, String> {
    // 非队长会话那一份：全局关闭的不注入，托管技能一律不进——
    // 人格常驻 / 可选技能也从这份清单取，托管技能因此天然不可勾选。
    let injectable = skills
        .iter()
        .filter(|item| item.enabled && !item.managed)
        .cloned()
        .collect::<Vec<_>>();
    // 队长会话那一份：托管技能不看启用表，其余照旧。
    let leader_injectable = skills
        .iter()
        .filter(|item| item.managed || item.enabled)
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
    summaries_guard.insert(hidden_skill_leader_cache_key(state), leader_injectable);
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
    summaries_guard.remove(&hidden_skill_leader_cache_key(state));
    drop(summaries_guard);
    // 手动 reload 时同步清空全部会话的项目 Skill 缓存，下次装配提示词时重新扫描。
    clear_all_conversation_project_skills(state);
    Ok(())
}

/// 读取旧的整块字符串快照。
///
/// 生产代码已无调用：提示词装配改为按结构化清单重新渲染，字符串快照只由
/// `update_hidden_skill_snapshot_cache` 写入。保留它是因为废弃要连
/// `AppState::hidden_skill_snapshot_cache` 字段一起收，属于另一件事。
#[allow(dead_code)]
pub(crate) fn build_hidden_skill_snapshot_block(state: &AppState) -> String {
    match state.hidden_skill_snapshot_cache.lock() {
        Ok(guard) if !guard.trim().is_empty() => guard.clone(),
        _ => String::new(),
    }
}

pub(crate) fn build_hidden_skill_snapshot_block_for_agent(
    state: &AppState,
    conversation: &Conversation,
    agent: Option<&AgentProfile>,
) -> String {
    let permission_enabled = agent
        .map(|item| item.permission_control.enabled)
        .unwrap_or(false);
    // 项目 Skill 不吃人格权限控制：当前项目是显式工作上下文，直接可用。
    let project = ensure_conversation_project_skills(state, conversation);
    // 托管技能（pai-guide / assistant-interaction-guide / persona-craft）只注入队长会话。
    // 判据复用队伍侧的 conversation_is_team_leader，不在这里另写一套。
    let team_leader = conversation_is_team_leader(state, &conversation.id);
    let mut scan_error: Option<String> = None;
    let global = match global_enabled_skills(state, team_leader) {
        Some(list) => list,
        None => {
            // 缓存被清空但没人重建时不能把全局技能当成空：现场重扫一次并回填，再照常取用。
            match refresh_hidden_skill_snapshot(state) {
                Ok(()) => {
                    runtime_log_debug(
                        "[技能工作区] 结构化缓存未命中，已重扫助理空间 skill 并回填缓存。"
                            .to_string(),
                    );
                }
                Err(err) => {
                    runtime_log_warn(format!(
                        "[技能工作区] 结构化缓存未命中，重扫助理空间 skill 失败：{err}"
                    ));
                    scan_error = Some(err);
                }
            }
            match global_enabled_skills(state, team_leader) {
                Some(list) => list,
                None => {
                    if scan_error.is_none() {
                        let err =
                            "隐藏技能快照缓存回填后仍未命中结构化缓存".to_string();
                        runtime_log_warn(format!("[技能工作区] {err}"));
                        scan_error = Some(err);
                    }
                    Vec::new()
                }
            }
        }
    };
    let global = if permission_enabled {
        filter_skills_for_agent(agent, &global)
    } else {
        global
    };
    let merged = merge_skill_lists(global, project);
    render_hidden_skill_snapshot_block(state, &merged, scan_error.as_deref())
}

/// 某人格当前可用的 skill：助理空间全局 Skill（吃权限控制）+ 当前会话项目 Skill（不吃权限控制，项目同名覆盖）。
///
/// 全局那半固定取非队长那份清单，托管技能不在其中——人格的常驻 / 可选技能因此天然勾不到托管技能。
fn merged_available_skills(
    state: &AppState,
    conversation: &Conversation,
    agent: &AgentProfile,
) -> Vec<SkillSummaryItem> {
    let global = match global_enabled_skills(state, false) {
        Some(skills) => skills,
        None => {
            // 与提示词装配同一条兜底：缓存未命中就重扫回填，不能让人格的常驻 / 可选技能静默失效。
            match refresh_hidden_skill_snapshot(state) {
                Ok(()) => {
                    runtime_log_debug(
                        "[技能工作区] 结构化缓存未命中，已重扫助理空间 skill 并回填缓存。"
                            .to_string(),
                    );
                    global_enabled_skills(state, false).unwrap_or_default()
                }
                Err(err) => {
                    runtime_log_warn(format!(
                        "[技能工作区] 结构化缓存未命中，重扫助理空间 skill 失败，本次跳过全局 skill：{err}"
                    ));
                    Vec::new()
                }
            }
        }
    };
    let global = filter_skills_for_agent(Some(agent), &global);
    let project = ensure_conversation_project_skills(state, conversation);
    merge_skill_lists(global, project)
}

/// 常驻 skill 全文注入（结论 24）：把这些 SKILL.md 的正文整段拼进该人格系统提示词，模型无需读文件。
/// skill 不在或未启用（或权限不允许）时直接跳过，不阻断装配。
pub(crate) fn build_resident_skill_fulltext_block(
    state: &AppState,
    conversation: &Conversation,
    agent: &AgentProfile,
) -> String {
    if agent.resident_skill_names.is_empty() {
        return String::new();
    }
    let available = merged_available_skills(state, conversation, agent);
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
pub(crate) fn build_optional_skill_reference_block(
    state: &AppState,
    conversation: &Conversation,
    agent: &AgentProfile,
) -> String {
    if agent.optional_skill_names.is_empty() {
        return String::new();
    }
    let available = merged_available_skills(state, conversation, agent);
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
) -> Vec<WorkspaceLoadedGroup> {
    let mcp_items = servers
        .iter()
        .map(|server| format_workspace_named_item(&server.name, &server.id))
        .collect::<Vec<_>>();
    let skill_items = skills
        .iter()
        .map(|item| item.name.trim().to_string())
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
    ]
}

fn build_workspace_failed_groups(
    mcp_failed: &[WorkspaceLoadError],
    skills_failed: &[WorkspaceLoadError],
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
) -> RefreshMcpAndSkillsResult {
    let loaded_groups = build_workspace_loaded_groups(servers, &result.skills);
    let failed_groups =
        build_workspace_failed_groups(&result.mcp_failed, &result.skills_failed);
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

/// 构建一次工作区快照所需的输入：加载哪个工作区。
pub(crate) struct WorkspaceSnapshotInput {
    pub workspace_root: PathBuf,
}

/// 一次纯加载的全部产物；写缓存、启动 MCP、发事件都由调用方自己决定。
pub(crate) struct WorkspaceSnapshot {
    pub skills: Vec<SkillSummaryItem>,
    pub skill_errors: Vec<WorkspaceLoadError>,
    pub mcp_servers: Vec<McpServerConfig>,
    pub mcp_errors: Vec<WorkspaceLoadError>,
}

/// 唯一的「建」：给定工作区根、配置与数据路径，把工作区里的技能、MCP、人格读出来。
///
/// 纯加载：不写缓存、不建目录、不设全局状态、不启动 MCP 运行时、不发事件，只读磁盘并返回值。
/// 代价是调用方必须先铺好目录与预设技能（`ensure_workspace_*_layout*`），
/// 并且只有「铺过 layout」的路径才能保证 MCP OAuth 全局目录已设置。
/// `workspace_root` 保持调用方给的原样：渲染侧用未规范化的根做路径前缀比对，
/// 这里若自作主张 `canonicalize`（Windows 会带 `\\?\`）反而会让技能路径与渲染根对不上。
/// 单个技能 / MCP 文件的问题收进 `*_errors`，不影响其余条目。
pub(crate) fn build_workspace_snapshot(
    input: &WorkspaceSnapshotInput,
) -> Result<WorkspaceSnapshot, String> {
    let workspace_root = &input.workspace_root;
    let (mcp_servers, mcp_errors) =
        load_mcp_servers_from_dir(&llm_workspace_mcp_root_at(workspace_root).join("servers"))?;
    let (skills, skill_errors) = load_skill_summaries_from_root(workspace_root)?;
    Ok(WorkspaceSnapshot {
        skills,
        skill_errors,
        mcp_servers,
        mcp_errors: mcp_errors
            .into_iter()
            .map(|v| {
                WorkspaceLoadError::with_hint(
                    v.item,
                    v.error,
                    "检查该 MCP JSON 配置的格式、command/args/env 与启用状态；修复后重新调用 reload。",
                )
            })
            .collect(),
    })
}

/// 薄壳：铺目录 → 纯建快照 → 写缓存 → 组装返回结果。
fn collect_workspace_load_snapshot(
    state: &AppState,
) -> Result<(RefreshMcpAndSkillsResult, Vec<McpServerConfig>), String> {
    // 写盘与全局副作用只允许出现在这一层。
    // 根保持未规范化的形态，与渲染侧 `llm_workspace_skills_root(state)` 一致。
    let workspace_root = configured_workspace_root_path(state)?;
    ensure_workspace_mcp_layout_at_root(&workspace_root)?;
    ensure_workspace_skills_layout_at_root(&workspace_root)?;
    let snapshot = build_workspace_snapshot(&WorkspaceSnapshotInput {
        workspace_root,
    })?;
    if let Err(err) = update_hidden_skill_snapshot_cache(state, &snapshot.skills, None) {
        runtime_log_error(format!(
            "[技能工作区] 更新隐藏技能快照缓存失败: skills={}, error={}",
            snapshot.skills.len(),
            err
        ));
    }
    let mcp_loaded = snapshot
        .mcp_servers
        .iter()
        .map(|s| s.id.clone())
        .collect::<Vec<_>>();
    let skills_loaded = snapshot
        .skills
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.name.clone())
        .collect::<Vec<_>>();
    // 摘要反映实际参与运行的 Skill，与注入内容保持一致。
    let enabled_skills = snapshot
        .skills
        .iter()
        .filter(|s| s.enabled)
        .cloned()
        .collect::<Vec<_>>();
    let skill_summary = render_skill_summary(&enabled_skills);
    let result = RefreshMcpAndSkillsResult {
        mcp_loaded,
        ok: false,
        status: String::new(),
        mcp_failed: snapshot.mcp_errors,
        skills_loaded,
        skills_failed: snapshot.skill_errors,
        skills: snapshot.skills.clone(),
        skill_summary,
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
    Ok((result, snapshot.mcp_servers))
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
    let (mut result, servers) = collect_workspace_load_snapshot(state)?;
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
    Ok(finalize_workspace_load_result(result, &servers))
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

#[cfg(test)]
mod snapshot_index_tests {
    use super::*;

    fn drop_global_summaries_cache(state: &AppState) {
        let mut guard = hidden_skill_summaries_cache()
            .lock()
            .expect("lock skill summaries cache");
        guard.remove(&hidden_skill_cache_scope_key(state));
        guard.remove(&hidden_skill_leader_cache_key(state));
    }

    fn write_project_skill(project_root: &std::path::Path, dir: &str, name: &str, desc: &str) {
        let skill_dir = project_root.join(".pai").join("skills").join(dir);
        std::fs::create_dir_all(&skill_dir).expect("create project skill dir");
        std::fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: {desc}\n---\n\n正文\n"),
        )
        .expect("write project skill");
    }

    fn test_conversation_with_project(
        conversation_id: &str,
        project_root: &std::path::Path,
    ) -> Conversation {
        let mut conversation =
            crate::tests::test_chat_conversation(conversation_id, "active", "2026-01-01T00:00:00Z");
        conversation.shell_workspaces = vec![ShellWorkspaceConfig {
            id: "project".to_string(),
            name: "project".to_string(),
            path: project_root.to_string_lossy().to_string(),
            level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(),
            access: SHELL_WORKSPACE_ACCESS_FULL_ACCESS.to_string(),
            built_in: false,
        }];
        conversation
    }

    fn summary_item(name: &str, description: &str, path: &str) -> SkillSummaryItem {
        SkillSummaryItem {
            name: name.to_string(),
            description: description.to_string(),
            content: String::new(),
            path: path.to_string(),
            additional_files: Vec::new(),
            is_builtin: false,
            managed: false,
            enabled: true,
        }
    }

    #[test]
    fn pure_system_keeps_legacy_format() {
        let root = std::path::PathBuf::from("E:/sys/skills");
        let skills = vec![summary_item(
            "foo",
            "foo desc",
            "E:/sys/skills/foo/SKILL.md",
        )];
        assert_eq!(
            render_skill_index_for_snapshot(&skills, &root),
            "- foo: foo desc"
        );
    }

    #[test]
    fn mixed_system_and_project_renders_dual_root_tree() {
        let root = std::path::PathBuf::from("E:/sys/skills");
        let skills = vec![
            summary_item("foo", "foo desc", "E:/sys/skills/foo/SKILL.md"),
            summary_item(
                "demo",
                "项目专用能力",
                "E:/proj/.pai/skills/demo/SKILL.md",
            ),
        ];
        let out = render_skill_index_for_snapshot(&skills, &root);
        assert!(out.contains("[system]"), "missing [system] in:\n{out}");
        assert!(out.contains("[project]"), "missing [project] in:\n{out}");
        assert!(
            out.contains("foo/SKILL.md \u{2014} foo\u{ff1a}foo desc"),
            "system leaf wrong in:\n{out}"
        );
        assert!(
            out.contains("demo/SKILL.md \u{2014} demo\u{ff1a}项目专用能力"),
            "project leaf wrong in:\n{out}"
        );
        // 公共前缀只在根上出现一次，不摊到每行。
        assert!(out.contains("E:/sys/skills/"), "missing system root in:\n{out}");
        assert!(
            out.contains("E:/proj/.pai/skills/"),
            "missing project root in:\n{out}"
        );
    }

    // ==================== 缓存清空后的重扫回填 ====================

    #[test]
    fn empty_cache_should_rescan_global_skills_alongside_project_skills() {
        let state = crate::tests::test_chat_runtime_state();
        drop_global_summaries_cache(&state);
        let project_root = state
            .llm_workspace_path
            .parent()
            .expect("workspace parent")
            .join("project-rescan-with-project");
        write_project_skill(&project_root, "demo", "demo", "项目专用能力");
        let conversation =
            test_conversation_with_project("conversation-rescan-with-project", &project_root);

        let block = build_hidden_skill_snapshot_block_for_agent(&state, &conversation, None);

        assert!(
            block.contains("[system]"),
            "缓存被清空后重扫应补回全局技能，got:\n{block}"
        );
        // 助理空间那批技能（测试环境里来自随应用释放的预设清单）必须回到索引里。
        assert!(
            block.contains("code-review"),
            "全局技能缺失，got:\n{block}"
        );
        assert!(block.contains("[project]"), "项目段缺失，got:\n{block}");
        assert!(block.contains("demo"), "项目技能缺失，got:\n{block}");

        let _ = std::fs::remove_dir_all(&project_root);
    }

    #[test]
    fn empty_cache_without_project_skills_should_still_rescan_global_skills() {
        let state = crate::tests::test_chat_runtime_state();
        drop_global_summaries_cache(&state);
        let conversation = crate::tests::test_chat_conversation(
            "conversation-rescan-without-project",
            "active",
            "2026-01-01T00:00:00Z",
        );

        let block = build_hidden_skill_snapshot_block_for_agent(&state, &conversation, None);

        assert!(!block.trim().is_empty(), "装配结果不应为空");
        // 只有全局技能时按既有格式走扁平列表（`[system]` 段只在存在项目技能、需要区分两个根时出现），
        // 这里钉的是「重扫后全局技能确实回来了」，不是渲染格式。
        assert!(
            block.contains("code-review"),
            "全局技能缺失，got:\n{block}"
        );
        assert!(
            block.contains("p-ai-config-guide"),
            "全局技能缺失，got:\n{block}"
        );
        assert!(!block.contains("scan failed:"), "正常重扫不应报错：\n{block}");
    }

    #[test]
    fn rescan_failure_should_render_scan_failed() {
        let root = std::env::temp_dir().join(format!(
            "eca-skill-rescan-fail-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create temp root");
        // 把工作区根指成一个普通文件，扫描前的布局准备必然失败。
        let blocked_root = root.join("blocked-workspace");
        std::fs::write(&blocked_root, b"not a dir").expect("write blocked workspace file");
        let mut state = crate::tests::test_chat_runtime_state();
        state.llm_workspace_path = blocked_root.clone();
        drop_global_summaries_cache(&state);
        let conversation = crate::tests::test_chat_conversation(
            "conversation-rescan-failure",
            "active",
            "2026-01-01T00:00:00Z",
        );

        let block = build_hidden_skill_snapshot_block_for_agent(&state, &conversation, None);

        assert!(
            block.contains("scan failed:"),
            "重扫失败必须显式报出来，got:\n{block}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn backfilled_cache_should_skip_rescan_on_second_build() {
        let state = crate::tests::test_chat_runtime_state();
        drop_global_summaries_cache(&state);
        // 非预设目录：重扫时才会出现，删掉后只有走缓存命中分支才还在。
        let probe_dir = state.llm_workspace_path.join("skills").join("cache-probe-skill");
        std::fs::create_dir_all(&probe_dir).expect("create probe skill dir");
        std::fs::write(
            probe_dir.join("SKILL.md"),
            "---\nname: cache-probe-skill\ndescription: 探针\n---\n\n正文\n",
        )
        .expect("write probe skill");
        let conversation = crate::tests::test_chat_conversation(
            "conversation-rescan-cache-hit",
            "active",
            "2026-01-01T00:00:00Z",
        );

        let first = build_hidden_skill_snapshot_block_for_agent(&state, &conversation, None);
        assert!(
            first.contains("cache-probe-skill"),
            "首次装配应扫描到探针技能，got:\n{first}"
        );
        assert!(
            hidden_skill_summaries_cache()
                .lock()
                .expect("lock skill summaries cache")
                .contains_key(&hidden_skill_cache_scope_key(&state)),
            "重扫后应回填结构化缓存"
        );

        std::fs::remove_dir_all(&probe_dir).expect("remove probe skill dir");
        let second = build_hidden_skill_snapshot_block_for_agent(&state, &conversation, None);
        assert!(
            second.contains("cache-probe-skill"),
            "第二次装配应命中缓存、不再扫盘，got:\n{second}"
        );
    }

    // ==================== 纯构建 build_workspace_snapshot ====================

    /// 造一个可独立加载的临时工作区：workspace/ 下铺好 skills 与 mcp/servers，data/ 空目录。
    fn make_workspace_root(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "eca-workspace-snapshot-{}-{label}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("workspace").join("skills")).expect("create skills root");
        std::fs::create_dir_all(root.join("workspace").join("mcp").join("servers"))
            .expect("create mcp servers root");
        std::fs::create_dir_all(root.join("data")).expect("create data root");
        root
    }

    fn write_workspace_skill(
        workspace_root: &std::path::Path,
        dir: &str,
        content: &str,
    ) {
        let skill_dir = workspace_root.join("skills").join(dir);
        std::fs::create_dir_all(&skill_dir).expect("create skill dir");
        std::fs::write(skill_dir.join("SKILL.md"), content).expect("write SKILL.md");
    }

    fn build_from(root: &std::path::Path) -> Result<WorkspaceSnapshot, String> {
        build_workspace_snapshot(&WorkspaceSnapshotInput {
            workspace_root: root.join("workspace"),
        })
    }

    /// 全局清单缓存的可比较指纹：纯构建不该改动它。
    fn summaries_cache_fingerprint() -> Vec<(String, Vec<String>)> {
        let guard = hidden_skill_summaries_cache()
            .lock()
            .expect("lock skill summaries cache");
        let mut out = guard
            .iter()
            .map(|(key, skills)| {
                (
                    key.clone(),
                    skills.iter().map(|item| item.name.clone()).collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        out.sort();
        out
    }

    #[test]
    fn build_snapshot_should_read_skills_in_stable_order() {
        let root = make_workspace_root("order");
        let workspace = root.join("workspace");
        write_workspace_skill(&workspace, "beta", "---\nname: beta\ndescription: 第二个\n---\n\n正文\n");
        write_workspace_skill(&workspace, "alpha", "---\nname: alpha\ndescription: 第一个\n---\n\n正文\n");

        let snapshot = build_from(&root).expect("build snapshot");

        let names = snapshot
            .skills
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["alpha", "beta"], "技能应按目录名排序");
        assert!(snapshot.skill_errors.is_empty(), "不该有错误：{:?}", snapshot.skill_errors);
        assert!(
            snapshot.skills.iter().all(|item| item.enabled),
            "无启用表记录时缺省启用"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn build_snapshot_should_collect_bad_skill_without_dropping_rest() {
        let root = make_workspace_root("bad-skill");
        let workspace = root.join("workspace");
        write_workspace_skill(&workspace, "good", "---\nname: good\ndescription: 正常\n---\n\n正文\n");
        // 缺 frontmatter
        write_workspace_skill(&workspace, "broken", "这段没有 YAML frontmatter\n");
        // 缺 SKILL.md
        std::fs::create_dir_all(workspace.join("skills").join("empty")).expect("create empty dir");

        let snapshot = build_from(&root).expect("build snapshot");

        assert_eq!(
            snapshot
                .skills
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["good"],
            "坏条目不该影响其余技能"
        );
        let error_items = snapshot
            .skill_errors
            .iter()
            .map(|item| item.item.replace('\\', "/"))
            .collect::<Vec<_>>();
        assert!(
            error_items.iter().any(|item| item.ends_with("broken/SKILL.md")),
            "frontmatter 解析失败应进 skill_errors，got: {error_items:?}"
        );
        assert!(
            error_items.iter().any(|item| item.ends_with("/empty")),
            "缺 SKILL.md 应进 skill_errors，got: {error_items:?}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn build_snapshot_should_follow_workspace_root_argument() {
        let root_a = make_workspace_root("root-a");
        write_workspace_skill(
            &root_a.join("workspace"),
            "only-in-a",
            "---\nname: only-in-a\ndescription: a 独有\n---\n\n正文\n",
        );
        let root_b = make_workspace_root("root-b");
        write_workspace_skill(
            &root_b.join("workspace"),
            "only-in-b",
            "---\nname: only-in-b\ndescription: b 独有\n---\n\n正文\n",
        );

        let names_a = build_from(&root_a)
            .expect("build a")
            .skills
            .iter()
            .map(|item| item.name.clone())
            .collect::<Vec<_>>();
        let names_b = build_from(&root_b)
            .expect("build b")
            .skills
            .iter()
            .map(|item| item.name.clone())
            .collect::<Vec<_>>();

        assert_eq!(names_a, vec!["only-in-a"], "结果必须跟着入参工作区走");
        assert_eq!(names_b, vec!["only-in-b"], "结果必须跟着入参工作区走");

        let _ = std::fs::remove_dir_all(&root_a);
        let _ = std::fs::remove_dir_all(&root_b);
    }

    #[test]
    fn build_snapshot_should_be_repeatable_and_leave_global_cache_alone() {
        let root = make_workspace_root("repeatable");
        write_workspace_skill(
            &root.join("workspace"),
            "stable",
            "---\nname: stable\ndescription: 稳定\n---\n\n正文\n",
        );
        let before = summaries_cache_fingerprint();
        let probe_state = crate::tests::test_chat_runtime_state();

        let first = build_from(&root).expect("first build");
        let second = build_from(&root).expect("second build");

        let fingerprint = |snapshot: &WorkspaceSnapshot| {
            snapshot
                .skills
                .iter()
                .map(|item| (item.name.clone(), item.path.clone(), item.enabled))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            fingerprint(&first),
            fingerprint(&second),
            "同一工作区两次构建结果必须一致"
        );
        assert_eq!(
            before,
            summaries_cache_fingerprint(),
            "纯构建不得写 hidden_skill_summaries_cache"
        );
        assert!(
            build_hidden_skill_snapshot_block(&probe_state).is_empty(),
            "纯构建不得写字符串快照缓存"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
