// ==================== 组织迁移：部门 → 人格 ====================
// V5：把「部门」承载的权责（模型 / 权限 / 提示词 / 下级）迁到人格身上，部门这一中间实体退场。
//
// 铁律：
//   - 旧结构 `AppConfig.departments` **只在本模块被读取**；迁移完成后业务代码不再感知「部门」。
//   - 本模块自带旧结构定义（`LegacyDepartment*`），不依赖业务运行态、不依赖会话；
//     只通过迁移上下文 `DataMigrationContext` 读取配置文件与人格分片的磁盘路径，**可 100% 独立工作**。
//   - 幂等：重复调用时，人格已有值不被覆盖，下级集合去重追加。
//   - 落盘前排掉无效下级边（自环、指向不存在人格、成环），保证产物可直接用于组织树遍历。
//
// 迁移规则（对应计划第四节的已确认结论）：
//   - 内置 7 部门 → 内置人格（身份映射见 `legacy_built_in_department_agent_id`）；
//     其提示词走 preset skill，本模块不生成。
//   - 自定义部门 → **溶解**进其成员人格：提示词转普通 skill，模型/权限并入人格；
//     一个人格来自多部门时，权限逐条合并、多 skill 同时常驻、模型取第一个。
//   - 层级**上提**：自定义部门的成员人格挂到其「最近的实体祖先」（内置人格或主助理根）下。
//   - 部门无所属人时，其模型与权限弃用。

/// 旧内置部门 id：仅用于迁移时把 `department_id` 映射到对应内置人格。
/// 这些字符串只在本模块内出现，业务运行态不再感知。
const ASSISTANT_DEPARTMENT_ID: &str = "assistant-department";
const DEPUTY_DEPARTMENT_ID: &str = "deputy-department";
const REVIEWER_DEPARTMENT_ID: &str = "reviewer-department";
const SADDLER_DEPARTMENT_ID: &str = "saddler-department";
const REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID: &str = "remote-customer-service-department";

/// 旧部门权限控制（仅迁移模块内使用）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyDepartmentPermissionControl {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "legacy_department_permission_default_mode")]
    mode: String,
    #[serde(default)]
    builtin_tool_names: Vec<String>,
    #[serde(default)]
    skill_names: Vec<String>,
    #[serde(default)]
    mcp_tool_names: Vec<String>,
}

fn legacy_department_permission_default_mode() -> String {
    "blacklist".to_string()
}

fn legacy_normalize_permission_mode(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "whitelist" => "whitelist".to_string(),
        _ => "blacklist".to_string(),
    }
}

impl Default for LegacyDepartmentPermissionControl {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: legacy_department_permission_default_mode(),
            builtin_tool_names: Vec::new(),
            skill_names: Vec::new(),
            mcp_tool_names: Vec::new(),
        }
    }
}

/// 旧部门结构（仅迁移模块内使用）。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyDepartment {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    guide: String,
    #[serde(default)]
    api_config_ids: Vec<String>,
    #[serde(default)]
    api_config_id: String,
    #[serde(default)]
    model_failure_fallback_enabled: bool,
    #[serde(default)]
    agent_ids: Vec<String>,
    #[serde(default)]
    child_department_ids: Vec<String>,
    #[serde(default)]
    permission_control: LegacyDepartmentPermissionControl,
}

/// 只取 `departments` 的旧配置视图（仅迁移模块内使用）。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyDepartmentsFile {
    #[serde(default)]
    departments: Vec<LegacyDepartment>,
    /// 旧「专家模型」配置键。部门退场后该键改名为 `expertApiConfigId`，
    /// 这里读出旧值并搬到新键，避免升级后用户已选的专家模型被重置。
    #[serde(default)]
    assistant_department_api_config_id: Option<String>,
}

/// 只取旧「专家模型」键的最小视图：不解析 `departments`，避免旧部门结构不兼容时连这个键一起丢掉。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyExpertModelKeyView {
    #[serde(default)]
    assistant_department_api_config_id: Option<String>,
}

/// 启动期抢救旧「专家模型」键。
/// 任何一次配置落盘都会让这个旧键消失（当前配置模型已不认识它），所以必须在启动后、
/// 任何配置写入之前执行；只在旧键有值且新键为空时改写，幂等。
fn agent_org_salvage_legacy_expert_model_key(state: &AppState) -> Result<bool, String> {
    let config_path = &state.config_path;
    if !config_path.exists() {
        return Ok(false);
    }
    let raw = fs::read_to_string(config_path)
        .map_err(|err| format!("读取配置失败 ({}): {err}", config_path.display()))?;
    let view: LegacyExpertModelKeyView = toml::from_str(&raw)
        .map_err(|err| format!("解析配置失败 ({}): {err}", config_path.display()))?;
    let legacy_value = view.assistant_department_api_config_id.unwrap_or_default();
    let migrated = carry_legacy_expert_model_key(config_path, &legacy_value)?;
    if migrated {
        runtime_log_info(
            "[配置迁移] 迁移完成，任务=启动期抢救旧专家模型键，assistantDepartmentApiConfigId → expertApiConfigId"
                .to_string(),
        );
    }
    Ok(migrated)
}

/// 把旧配置里的 `assistantDepartmentApiConfigId` 搬到新键 `expertApiConfigId`。
/// 只在旧键有值且新键为空时改写，幂等。
fn carry_legacy_expert_model_key(config_path: &PathBuf, legacy_value: &str) -> Result<bool, String> {
    let legacy_value = legacy_value.trim();
    if legacy_value.is_empty() {
        return Ok(false);
    }
    let raw = fs::read_to_string(config_path)
        .map_err(|err| format!("读取配置失败 ({}): {err}", config_path.display()))?;
    let mut value = toml::from_str::<toml::Value>(&raw)
        .map_err(|err| format!("解析配置失败 ({}): {err}", config_path.display()))?;
    let Some(table) = value.as_table_mut() else {
        return Ok(false);
    };
    let already_migrated = table
        .get("expertApiConfigId")
        .and_then(|item| item.as_str())
        .map(str::trim)
        .is_some_and(|item| !item.is_empty());
    if already_migrated {
        return Ok(false);
    }
    table.remove("assistantDepartmentApiConfigId");
    table.insert(
        "expertApiConfigId".to_string(),
        toml::Value::String(legacy_value.to_string()),
    );
    let serialized = toml::to_string(&value)
        .map_err(|err| format!("序列化配置失败 ({}): {err}", config_path.display()))?;
    fs::write(config_path, serialized)
        .map_err(|err| format!("写入配置失败 ({}): {err}", config_path.display()))?;
    Ok(true)
}

/// 内置部门 → 内置人格 id。返回 `None` 表示该部门是自定义部门。
/// 注意：`assistants`/`explorer` 是显示名，其人格 id 分别是 `default-agent`/`deputy-agent`。
fn legacy_built_in_department_agent_id(department_id: &str) -> Option<&'static str> {
    match department_id.trim() {
        ASSISTANT_DEPARTMENT_ID => Some(DEFAULT_AGENT_ID),
        DEPUTY_DEPARTMENT_ID => Some(DEPUTY_AGENT_ID),
        REVIEWER_DEPARTMENT_ID => Some(REVIEWER_AGENT_ID),
        SADDLER_DEPARTMENT_ID => Some(SADDLER_AGENT_ID),
        REMOTE_CUSTOMER_SERVICE_DEPARTMENT_ID => Some(SUPPORT_AGENT_ID),
        _ => None,
    }
}

/// 一个人格在迁移中要接收的增量。
#[derive(Debug, Clone, Default)]
struct AgentOrgIncrement {
    summary: String,
    resident_skill_names: Vec<String>,
    api_config_ids: Vec<String>,
    api_config_id: String,
    model_failure_fallback_enabled: bool,
    permission_control: AgentPermissionControl,
}

/// 迁移计划：人格 id → 增量，以及人格 id → 下级人格 id 集合。
#[derive(Debug, Clone, Default)]
struct AgentOrgMigrationPlan {
    increments: std::collections::BTreeMap<String, AgentOrgIncrement>,
    child_agent_ids: std::collections::BTreeMap<String, Vec<String>>,
    /// 需要写出的自定义部门 skill：skill 名 → (description, 正文)
    custom_skills: std::collections::BTreeMap<String, (String, String)>,
    /// 迁移告警（如权限模式冲突导致的名单丢弃），供落盘时一次性上报。
    warnings: Vec<String>,
}

fn legacy_push_unique(target: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if value.is_empty() || target.iter().any(|item| item == value) {
        return;
    }
    target.push(value.to_string());
}

/// 部门在组织树里的直接代表人格集合（人格 id）。
/// 内置部门 → 其内置人格；自定义部门 → 其成员人格；纯目录部门（无成员）→ 空。
fn legacy_department_representative_agent_ids(
    department: &LegacyDepartment,
) -> Vec<String> {
    if let Some(agent_id) = legacy_built_in_department_agent_id(&department.id) {
        return vec![agent_id.to_string()];
    }
    let mut out = Vec::<String>::new();
    for agent_id in &department.agent_ids {
        legacy_push_unique(&mut out, agent_id);
    }
    out
}

/// 一个部门的「上级锚点人格」：沿父链**按层**（BFS）上溯，取第一个「代表人格非空」的层级上的全部代表人格。
/// - 用 BFS 逐层推进：多父图下祖先距离不同，必须取最近的层，而非 DFS 先碰到的那个。
/// - 跨过纯目录部门：中间层无成员时继续上溯，使 `P(有成员) → C(空目录) → C2(有成员)` 的层级不丢边。
/// - 到顶都没有可映射祖先时，兜底为主助理根 `default-agent`。
fn legacy_ancestor_anchors_of_department(
    department_id: &str,
    departments_by_id: &std::collections::HashMap<String, &LegacyDepartment>,
) -> Vec<String> {
    let parents_of = |child_id: &str| -> Vec<String> {
        let mut out = Vec::<String>::new();
        for department in departments_by_id.values() {
            if department
                .child_department_ids
                .iter()
                .any(|item| item.trim() == child_id)
            {
                legacy_push_unique(&mut out, &department.id);
            }
        }
        out
    };

    let mut frontier = parents_of(department_id.trim());
    let mut seen = std::collections::HashSet::<String>::new();
    while !frontier.is_empty() {
        let mut next = Vec::<String>::new();
        let mut anchors = Vec::<String>::new();
        for id in frontier {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(parent) = departments_by_id.get(&id) {
                for agent_id in legacy_department_representative_agent_ids(parent) {
                    legacy_push_unique(&mut anchors, &agent_id);
                }
            }
            for grand_parent in parents_of(&id) {
                next.push(grand_parent);
            }
        }
        if !anchors.is_empty() {
            return anchors;
        }
        frontier = next;
    }
    vec![DEFAULT_AGENT_ID.to_string()]
}

/// 合并一个来源部门的权限到人格权限。
/// 语义：**第一个启用权限的部门的模式为准**；后续部门若模式不同，其名单整体丢弃并返回 `Some(mode)` 供调用方告警，
/// 因为人格上只有一个权限对象，白名单与黑名单无法共存，逐条并入会让「禁止」的名字变成「允许」。
fn legacy_merge_permission_control(
    current: &mut AgentPermissionControl,
    incoming: &LegacyDepartmentPermissionControl,
) -> Option<String> {
    if !incoming.enabled {
        return None;
    }
    let incoming_mode = legacy_normalize_permission_mode(&incoming.mode);
    if !current.enabled {
        current.enabled = true;
        current.mode = incoming_mode;
    } else if legacy_normalize_permission_mode(&current.mode) != incoming_mode {
        // 模式冲突：丢弃该部门的名单，交由调用方告警。
        return Some(incoming_mode);
    }
    for name in &incoming.builtin_tool_names {
        legacy_push_unique(&mut current.builtin_tool_names, name);
    }
    for name in &incoming.skill_names {
        legacy_push_unique(&mut current.skill_names, name);
    }
    for name in &incoming.mcp_tool_names {
        legacy_push_unique(&mut current.mcp_tool_names, name);
    }
    None
}

/// 自定义部门转出的 skill 名。
/// 必须与落盘目录名一致（都走 `sanitize_skill_dir_name`），否则人格引用的 skill 名找不到对应目录。
fn legacy_custom_department_skill_name(department: &LegacyDepartment) -> String {
    let raw = {
        let name = department.name.trim();
        if !name.is_empty() {
            name.to_string()
        } else {
            department.id.trim().to_string()
        }
    };
    sanitize_skill_dir_name(&raw)
}

/// 计算迁移计划（纯函数，不触碰磁盘，便于独立测试）。
fn plan_agent_org_migration(departments: &[LegacyDepartment]) -> AgentOrgMigrationPlan {
    let mut plan = AgentOrgMigrationPlan::default();
    let mut departments_by_id = std::collections::HashMap::<String, &LegacyDepartment>::new();
    for department in departments {
        let id = department.id.trim();
        if !id.is_empty() {
            departments_by_id.insert(id.to_string(), department);
        }
    }

    // 1) 部门层级 → 人格组织边。
    // 对每个「有直接代表人格」的部门 C：其代表人格的直接上级 = C 的最近可映射祖先的代表人格。
    // 纯目录部门（无成员）自己不对应人格，由子孙跨过它继续上溯，保证 P(有成员)→C(空)→C2(有成员) 不断链。
    // 内置人格之间的上下级由代码预设定义（`built_in_organization_child_edges`），这里一律跳过：
    // 原内置部门是多根，且 leader 反过来指向助理，若照搬会与预设的「根→leader」互指成环。
    for department in departments {
        let targets = legacy_department_representative_agent_ids(department);
        if targets.is_empty() {
            continue;
        }
        let anchors = legacy_ancestor_anchors_of_department(&department.id, &departments_by_id);
        for anchor in &anchors {
            for target in &targets {
                // 跳过自环：顶层内置部门（如助理部门即主助理）无需自己指向自己。
                if anchor == target {
                    continue;
                }
                // 跳过内置↔内置边：这类层级由代码预设，不由迁移推导。
                if is_built_in_organization_agent_id(anchor)
                    && is_built_in_organization_agent_id(target)
                {
                    continue;
                }
                let slot = plan.child_agent_ids.entry(anchor.clone()).or_default();
                legacy_push_unique(slot, target);
            }
        }
    }

    // 2) 自定义部门：成员人格溶解 + 权责并入。
    for department in departments {
        let department_id = department.id.trim();
        if department_id.is_empty()
            || legacy_built_in_department_agent_id(department_id).is_some()
        {
            continue;
        }
        let mut member_agent_ids = Vec::<String>::new();
        for agent_id in &department.agent_ids {
            legacy_push_unique(&mut member_agent_ids, agent_id);
        }
        if member_agent_ids.is_empty() {
            // 部门无所属人：模型与权限弃用，不产生任何人格增量。
            continue;
        }

        let skill_name = legacy_custom_department_skill_name(department);
        if !skill_name.is_empty() {
            plan.custom_skills
                .entry(skill_name.clone())
                .or_insert_with(|| (department.summary.trim().to_string(), department.guide.to_string()));
        }

        for agent_id in &member_agent_ids {
            let increment = plan.increments.entry(agent_id.clone()).or_default();
            if increment.summary.trim().is_empty() && !department.summary.trim().is_empty() {
                increment.summary = department.summary.trim().to_string();
            }
            legacy_push_unique(&mut increment.resident_skill_names, &skill_name);
            if increment.api_config_ids.is_empty() {
                let mut api_config_ids = Vec::<String>::new();
                for id in &department.api_config_ids {
                    legacy_push_unique(&mut api_config_ids, id);
                }
                if api_config_ids.is_empty() {
                    legacy_push_unique(&mut api_config_ids, &department.api_config_id);
                }
                if !api_config_ids.is_empty() {
                    increment.api_config_ids = api_config_ids;
                    increment.api_config_id = increment.api_config_ids[0].clone();
                    increment.model_failure_fallback_enabled =
                        department.model_failure_fallback_enabled;
                }
            }
            if let Some(conflict_mode) =
                legacy_merge_permission_control(
                    &mut increment.permission_control,
                    &department.permission_control,
                )
            {
                plan.warnings.push(format!(
                    "[应用数据迁移] 权限模式冲突，人格={agent_id}，部门={department_id}，该部门模式={conflict_mode}，已保留先到模式与名单，丢弃该部门权限名单"
                ));
            }
        }
    }

    plan
}

/// 把计划应用到人格集合。返回是否有数据变化。
/// 幂等策略：人格已有值不覆盖，下级集合去重追加；
/// **并只追加指向现存人格的边**——指向未创建人格的边（如后续阶段才建的内置人格）在写入前就被丢弃，
/// 否则每次运行都会重复添加、再被剪枝删掉，破坏幂等。
fn apply_agent_org_migration_plan(
    agents: &mut [AgentProfile],
    plan: &AgentOrgMigrationPlan,
) -> bool {
    let valid_ids = agents
        .iter()
        .map(|agent| agent.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let mut changed = false;
    for agent in agents.iter_mut() {
        let agent_id = agent.id.trim().to_string();
        if let Some(increment) = plan.increments.get(&agent_id) {
            if agent.summary.trim().is_empty() && !increment.summary.trim().is_empty() {
                agent.summary = increment.summary.clone();
                changed = true;
            }
            for skill_name in &increment.resident_skill_names {
                if !agent.resident_skill_names.iter().any(|item| item == skill_name) {
                    agent.resident_skill_names.push(skill_name.clone());
                    changed = true;
                }
            }
            if agent.api_config_ids.is_empty()
                && agent.api_config_id.trim().is_empty()
                && !increment.api_config_ids.is_empty()
            {
                agent.api_config_ids = increment.api_config_ids.clone();
                agent.api_config_id = increment.api_config_id.clone();
                agent.model_failure_fallback_enabled = increment.model_failure_fallback_enabled;
                changed = true;
            }
            if !agent.permission_control.enabled && increment.permission_control.enabled {
                agent.permission_control = increment.permission_control.clone();
                changed = true;
            }
        }
        if let Some(children) = plan.child_agent_ids.get(&agent_id) {
            for child in children {
                // 自环与指向不存在人格的边直接丢弃，不落盘也不计变化。
                if child.trim() == agent_id || !valid_ids.contains(child.as_str()) {
                    continue;
                }
                if !agent.child_agent_ids.iter().any(|item| item == child) {
                    agent.child_agent_ids.push(child.clone());
                    changed = true;
                }
            }
        }
    }
    if changed {
        let removed = legacy_prune_agent_child_edges(agents);
        if !removed.is_empty() {
            let edges = removed
                .iter()
                .map(|(parent, child)| format!("{parent}->{child}"))
                .collect::<Vec<_>>()
                .join(", ");
            runtime_log_warn(format!(
                "[应用数据迁移] 组织迁移剔除无效下级边，count={}，edges={}（成环）",
                removed.len(),
                edges
            ));
        }
    }
    changed
}

/// 剔除人格组织图中的无效下级边：自环、指向不存在人格的边、以及成环边。
/// 返回被剔除的边列表，供迁移日志告警。
/// 内置人格之间的上下级由代码预设且不会成环；此处主要用于兜住自定义部门迁移可能产生的悬空/成环边。
fn legacy_prune_agent_child_edges(agents: &mut [AgentProfile]) -> Vec<(String, String)> {
    let valid_ids = agents
        .iter()
        .map(|agent| agent.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let mut adjacency = agents
        .iter()
        .map(|agent| {
            let id = agent.id.trim().to_string();
            let mut children = Vec::<String>::new();
            for child in &agent.child_agent_ids {
                let child = child.trim();
                if child.is_empty() || child == id {
                    continue;
                }
                legacy_push_unique(&mut children, child);
            }
            (id, children)
        })
        .filter(|(id, _)| !id.is_empty())
        .collect::<std::collections::BTreeMap<_, _>>();

    let mut removed = Vec::<(String, String)>::new();
    let agent_ids = adjacency.keys().cloned().collect::<Vec<_>>();
    for parent_id in agent_ids {
        let children = adjacency.get(&parent_id).cloned().unwrap_or_default();
        let mut retained = Vec::<String>::new();
        for child_id in children {
            // 目标人格不存在：剔除（后续阶段负责重建内置层级）。
            if !valid_ids.contains(&child_id) {
                removed.push((parent_id.clone(), child_id));
                continue;
            }
            // 成环：从 child 沿其它边能走回 parent，则该边必须删除。
            if agent_child_path_exists(&adjacency, &child_id, &parent_id, Some((&parent_id, &child_id))) {
                removed.push((parent_id.clone(), child_id));
            } else {
                retained.push(child_id);
            }
        }
        adjacency.insert(parent_id, retained);
    }

    for agent in agents.iter_mut() {
        let id = agent.id.trim().to_string();
        let normalized = adjacency.remove(&id).unwrap_or_default();
        if agent.child_agent_ids != normalized {
            agent.child_agent_ids = normalized;
        }
    }
    removed
}

/// 在不使用 `skip_edge` 这条边的前提下，从 `start_id` 沿下级边能否走到 `target_id`。
fn agent_child_path_exists(
    adjacency: &std::collections::BTreeMap<String, Vec<String>>,
    start_id: &str,
    target_id: &str,
    skip_edge: Option<(&str, &str)>,
) -> bool {
    if start_id.is_empty() || target_id.is_empty() {
        return false;
    }
    let mut queue = std::collections::VecDeque::<String>::new();
    queue.push_back(start_id.to_string());
    let mut seen = std::collections::HashSet::<String>::new();
    while let Some(current_id) = queue.pop_front() {
        if current_id == target_id {
            return true;
        }
        if !seen.insert(current_id.clone()) {
            continue;
        }
        let Some(children) = adjacency.get(&current_id) else {
            continue;
        };
        for child_id in children {
            if let Some((skip_parent, skip_child)) = skip_edge {
                if current_id == skip_parent && child_id == skip_child {
                    continue;
                }
            }
            queue.push_back(child_id.clone());
        }
    }
    false
}

/// 写出自定义部门转出的普通 skill（工作区 `skills/<name>/SKILL.md`）。
/// 已存在则视为用户已改，不覆盖。
fn write_custom_department_skills(
    skills_root: &PathBuf,
    plan: &AgentOrgMigrationPlan,
) -> Result<usize, String> {
    if plan.custom_skills.is_empty() {
        return Ok(0);
    }
    let mut written = 0usize;
    for (name, (description, body)) in &plan.custom_skills {
        let dir_name = sanitize_skill_dir_name(name);
        if dir_name.is_empty() {
            continue;
        }
        let dir = skills_root.join(&dir_name);
        let path = dir.join("SKILL.md");
        if path.exists() {
            continue;
        }
        fs::create_dir_all(&dir)
            .map_err(|err| format!("创建自定义部门技能目录失败 ({}): {err}", dir.display()))?;
        let content = format!(
            "---\nname: {}\ndescription: {}\n---\n\n{}\n",
            legacy_yaml_inline(&dir_name),
            legacy_yaml_inline(description),
            body
        );
        fs::write(&path, content)
            .map_err(|err| format!("写入自定义部门技能失败 ({}): {err}", path.display()))?;
        written += 1;
    }
    Ok(written)
}

/// 把任意文本安全地写进 YAML frontmatter 的单行标量：换行折叠为空格，双引号转义。
/// 不转义会让含 `:` 或换行的部门名/简介破坏 frontmatter，导致 skill 解析失败。
fn legacy_yaml_inline(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.trim().chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// 把 skill/目录名里不适合做路径的字符替换掉，保留中文与常规字符。
fn sanitize_skill_dir_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.trim().chars() {
        if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    out.trim().trim_matches('.').to_string()
}

/// 确保内置人格存在，并把内置层级预设并入现有的人格。
/// 现有用户的 `agents.json` 是权威来源，新增内置人格不会被 `AppData::default()` 自动补上，
/// 因此在这里补齐：只补缺失的内置人格（不覆盖 `default-agent`/`deputy-agent` 的既有数据），
/// 再把内置层级预设的边并进去（只追加、去重）。返回是否有变化。
fn ensure_built_in_organization_nodes(agents: &mut Vec<AgentProfile>) -> bool {
    let mut changed = false;
    for preset in built_in_organization_agents() {
        if !agents.iter().any(|agent| agent.id == preset.id) {
            agents.push(preset);
            changed = true;
        }
    }
    for (parent_id, child_ids) in built_in_organization_child_edges() {
        let Some(parent) = agents.iter_mut().find(|agent| agent.id == parent_id) else {
            continue;
        };
        for child_id in child_ids {
            if !parent
                .child_agent_ids
                .iter()
                .any(|item| item.as_str() == child_id)
            {
                parent.child_agent_ids.push(child_id.to_string());
                changed = true;
            }
        }
    }
    changed
}

/// 把旧键 `assistant_department_agent_id` 搬到新键 `assistant_agent_id`。
/// 只在旧键有值且新键为空时写，幂等。
fn carry_legacy_assistant_agent_key(state: &AppState) -> Result<bool, String> {
    let Some(legacy_value) = state_service_get_kv(state, "assistant_department_agent_id")? else {
        return Ok(false);
    };
    let legacy_value = legacy_value.trim().to_string();
    if legacy_value.is_empty() {
        return Ok(false);
    }
    let already_migrated = state_service_get_kv(state, "assistant_agent_id")?
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if already_migrated {
        return Ok(false);
    }
    state_service_set_kv(state, "assistant_agent_id", &legacy_value)?;
    Ok(true)
}

/// V5 迁移步骤：读取旧 `departments`，转写为人格组织。
fn migrate_departments_into_agent_organization(
    context: &DataMigrationContext<'_>,
) -> Result<DataMigrationStepStats, String> {
    let mut stats = DataMigrationStepStats::default();
    let mut agents = read_agents_shard(&context.state.data_path)?;
    let mut changed = ensure_built_in_organization_nodes(&mut agents);
    if carry_legacy_assistant_agent_key(context.state)? {
        stats.data_changed = true;
        runtime_log_info(
            "[应用数据迁移] 主助理人格键迁移完成，任务=v5部门转人格，assistant_department_agent_id → assistant_agent_id".to_string(),
        );
    }

    let config_path = &context.state.config_path;
    let mut plan = AgentOrgMigrationPlan::default();
    let mut department_count = 0usize;
    if config_path.exists() {
        let raw = fs::read_to_string(config_path)
            .map_err(|err| format!("读取配置失败 ({}): {err}", config_path.display()))?;
        let legacy: LegacyDepartmentsFile = toml::from_str(&raw)
            .map_err(|err| format!("解析旧配置 departments 失败: {err}"))?;
        let legacy_expert_api_config_id = legacy
            .assistant_department_api_config_id
            .clone()
            .unwrap_or_default();
        department_count = legacy.departments.len();
        if !legacy.departments.is_empty() {
            plan = plan_agent_org_migration(&legacy.departments);
            changed |= apply_agent_org_migration_plan(&mut agents, &plan);
        }
        if carry_legacy_expert_model_key(config_path, &legacy_expert_api_config_id)? {
            runtime_log_info(
                "[应用数据迁移] 专家模型配置键迁移完成，任务=v5部门转人格，assistantDepartmentApiConfigId → expertApiConfigId".to_string(),
            );
        }
    }

    if changed {
        write_agents_shard(&context.state.data_path, &agents)?;
        stats.data_changed = true;
    }

    // skill 必须落在配置的工作区（可能已被用户改为助理空间等），不能假定默认工作区目录。
    let workspace_root =
        configured_workspace_root_from_config(context.config, context.state);
    let skills_written = write_custom_department_skills(&workspace_root.join("skills"), &plan)?;
    for warning in &plan.warnings {
        runtime_log_warn(warning.clone());
    }
    runtime_log_info(format!(
        "[应用数据迁移] 组织迁移完成，任务=v5部门转人格，departments={}，人格增量={}，组织边={}，自定义技能写出={}，告警={}",
        department_count,
        plan.increments.len(),
        plan.child_agent_ids.len(),
        skills_written,
        plan.warnings.len()
    ));
    Ok(stats)
}

#[cfg(test)]
mod agent_org_migration_tests {
    use super::*;

    fn dept(
        id: &str,
        name: &str,
        agent_ids: &[&str],
        child_ids: &[&str],
    ) -> LegacyDepartment {
        LegacyDepartment {
            id: id.to_string(),
            name: name.to_string(),
            agent_ids: agent_ids.iter().map(|s| s.to_string()).collect(),
            child_department_ids: child_ids.iter().map(|s| s.to_string()).collect(),
            ..LegacyDepartment::default()
        }
    }

    #[test]
    fn built_in_departments_map_to_built_in_agent_nodes() {
        assert_eq!(
            legacy_built_in_department_agent_id(ASSISTANT_DEPARTMENT_ID),
            Some(DEFAULT_AGENT_ID)
        );
        assert_eq!(
            legacy_built_in_department_agent_id(DEPUTY_DEPARTMENT_ID),
            Some(DEPUTY_AGENT_ID)
        );
        assert_eq!(legacy_built_in_department_agent_id("department-1"), None);
    }

    #[test]
    fn custom_department_should_dissolve_into_member_agent() {
        let mut department = dept("department-1", "八重堂", &["yae-miko"], &[]);
        department.summary = "当需要捏人时，请委托给我。".to_string();
        department.guide = "你主理八重堂。".to_string();
        department.api_config_ids = vec!["model-x".to_string()];
        department.permission_control = LegacyDepartmentPermissionControl {
            enabled: true,
            mode: "whitelist".to_string(),
            builtin_tool_names: vec!["read".to_string()],
            skill_names: vec!["role-prompt-optimizer".to_string()],
            mcp_tool_names: vec!["akasha_read".to_string()],
        };
        let plan = plan_agent_org_migration(&[department]);

        let increment = plan.increments.get("yae-miko").expect("increment");
        assert_eq!(increment.summary, "当需要捏人时，请委托给我。");
        assert_eq!(increment.resident_skill_names, vec!["八重堂".to_string()]);
        assert_eq!(increment.api_config_ids, vec!["model-x".to_string()]);
        assert!(increment.permission_control.enabled);
        assert_eq!(increment.permission_control.mode, "whitelist");
        assert_eq!(
            increment.permission_control.skill_names,
            vec!["role-prompt-optimizer".to_string()]
        );
        assert!(plan.custom_skills.contains_key("八重堂"));
    }

    #[test]
    fn custom_department_without_member_should_drop_model_and_permission() {
        let department = dept("department-empty", "空部门", &[], &[]);
        let plan = plan_agent_org_migration(&[department]);
        assert!(plan.increments.is_empty());
        assert!(plan.child_agent_ids.is_empty());
    }

    #[test]
    fn agent_from_multiple_departments_should_merge_permissions_and_skills() {
        let mut first = dept("department-1", "甲", &["shared-agent"], &[]);
        first.api_config_ids = vec!["model-first".to_string()];
        first.permission_control = LegacyDepartmentPermissionControl {
            enabled: true,
            mode: "whitelist".to_string(),
            builtin_tool_names: vec!["read".to_string()],
            skill_names: vec!["skill-a".to_string()],
            mcp_tool_names: Vec::new(),
        };
        let mut second = dept("department-2", "乙", &["shared-agent"], &[]);
        second.api_config_ids = vec!["model-second".to_string()];
        second.permission_control = LegacyDepartmentPermissionControl {
            enabled: true,
            mode: "whitelist".to_string(),
            builtin_tool_names: vec!["write".to_string()],
            skill_names: vec!["skill-b".to_string()],
            mcp_tool_names: Vec::new(),
        };
        let plan = plan_agent_org_migration(&[first, second]);

        let increment = plan.increments.get("shared-agent").expect("increment");
        assert_eq!(increment.api_config_ids, vec!["model-first".to_string()]);
        assert_eq!(
            increment.resident_skill_names,
            vec!["甲".to_string(), "乙".to_string()]
        );
        assert_eq!(
            increment.permission_control.builtin_tool_names,
            vec!["read".to_string(), "write".to_string()]
        );
        assert_eq!(
            increment.permission_control.skill_names,
            vec!["skill-a".to_string(), "skill-b".to_string()]
        );
    }

    #[test]
    fn conflicting_permission_modes_should_keep_first_and_drop_second_with_warning() {
        // 第一个人格来自白名单部门、第二个来自黑名单部门：模式冲突，保留先到者，丢弃后者名单并告警。
        let mut first = dept("department-1", "甲", &["shared-agent"], &[]);
        first.permission_control = LegacyDepartmentPermissionControl {
            enabled: true,
            mode: "whitelist".to_string(),
            builtin_tool_names: vec!["read".to_string()],
            skill_names: vec!["skill-a".to_string()],
            mcp_tool_names: Vec::new(),
        };
        let mut second = dept("department-2", "乙", &["shared-agent"], &[]);
        second.permission_control = LegacyDepartmentPermissionControl {
            enabled: true,
            mode: "blacklist".to_string(),
            builtin_tool_names: vec!["write".to_string()],
            skill_names: vec!["skill-b".to_string()],
            mcp_tool_names: vec!["mcp-x".to_string()],
        };
        let plan = plan_agent_org_migration(&[first, second]);

        let increment = plan.increments.get("shared-agent").expect("increment");
        assert_eq!(increment.permission_control.mode, "whitelist");
        assert_eq!(
            increment.permission_control.builtin_tool_names,
            vec!["read".to_string()],
            "异模式部门的名单必须丢弃，不能反向并入"
        );
        assert_eq!(
            increment.permission_control.skill_names,
            vec!["skill-a".to_string()]
        );
        assert!(increment.permission_control.mcp_tool_names.is_empty());
        assert_eq!(plan.warnings.len(), 1, "模式冲突必须告警");
    }

    #[test]
    fn nested_custom_departments_should_hoist_to_nearest_entity_ancestor() {
        // 内置根 → 自定义父（全栈工程师）→ 自定义子（文本整理员）
        let assistant = dept(
            ASSISTANT_DEPARTMENT_ID,
            "助理部门",
            &[DEFAULT_AGENT_ID],
            &["department-parent"],
        );
        let parent = dept("department-parent", "全栈工程师", &["engineer"], &["department-child"]);
        let child = dept("department-child", "文本整理员", &["organizer"], &[]);
        let plan = plan_agent_org_migration(&[assistant, parent, child]);

        // 全栈工程师的成员挂到内置根 default-agent 下。
        let assistants_children = plan
            .child_agent_ids
            .get(DEFAULT_AGENT_ID)
            .expect("assistants children");
        assert!(assistants_children.contains(&"engineer".to_string()));

        // 文本整理员的成员挂到「最近的可映射祖先」全栈工程师下，而不是越过它直接挂根。
        let engineer_children = plan
            .child_agent_ids
            .get("engineer")
            .expect("engineer children");
        assert!(engineer_children.contains(&"organizer".to_string()));
        assert!(
            !assistants_children.contains(&"organizer".to_string()),
            "文本整理员应挂在全栈工程师下，而非直接挂根"
        );
    }

    #[test]
    fn empty_directory_department_should_not_break_child_edges() {
        // 父部门 P（有成员 p1）→ 纯目录部门 C（无成员）→ 孙部门 C2（有成员 c2）
        // C 在人格图里不对应人格，c2 应直接挂到 p1 下，不能断链也不能丢层级。
        let p = dept("dept-p", "P", &["p1"], &["dept-c"]);
        let c = dept("dept-c", "C", &[], &["dept-c2"]);
        let c2 = dept("dept-c2", "C2", &["c2"], &[]);
        let plan = plan_agent_org_migration(&[p, c, c2]);

        let p1_children = plan
            .child_agent_ids
            .get("p1")
            .expect("p1 children");
        assert!(
            p1_children.contains(&"c2".to_string()),
            "纯目录部门应被旁路，c2 挂到最近的实体祖先 p1 下"
        );
        // 纯目录部门自己不对应人格，不产生 p1 -> c 之类的空边。
        assert!(plan.child_agent_ids.get("dept-c").is_none());
    }

    #[test]
    fn prune_should_remove_missing_target_edges_and_self_loops() {
        // 顶层内置部门（主助理）不指向自己；指向尚未创建的内置人格的边应被剔除并告警。
        let assistant = dept(
            ASSISTANT_DEPARTMENT_ID,
            "助理部门",
            &[DEFAULT_AGENT_ID],
            &["reviewer-department"],
        );
        let plan = plan_agent_org_migration(&[assistant]);
        // 主助理自身不产生自环边。
        assert!(
            !plan
                .child_agent_ids
                .get(DEFAULT_AGENT_ID)
                .map(|children| children.contains(&DEFAULT_AGENT_ID.to_string()))
                .unwrap_or(false),
            "主助理根不应指向自己"
        );

        let mut agents = vec![
            test_agent(DEFAULT_AGENT_ID),
            test_agent("reviewer"),
        ];
        // 手工构造一条指向不存在人格的边 + 一条自环，验证剪枝。
        agents[0].child_agent_ids = vec![
            "reviewer".to_string(),
            "ghost".to_string(),
            DEFAULT_AGENT_ID.to_string(),
        ];
        let removed = legacy_prune_agent_child_edges(&mut agents);
        // 自环被静默丢弃，指向不存在人格的边被剔除并记录。
        assert_eq!(agents[0].child_agent_ids, vec!["reviewer".to_string()]);
        assert!(removed.iter().any(|(_, child)| child == "ghost"));
        assert!(
            !agents[0]
                .child_agent_ids
                .iter()
                .any(|child| child == DEFAULT_AGENT_ID),
            "自环必须被丢弃"
        );
    }

    #[test]
    fn migration_should_skip_built_in_to_built_in_edges() {
        // 内置↔内置边由代码预设，不由迁移推导：assistant→reviewer、reviewer 反过来锚到 assistant 都不该进计划。
        let assistant = dept(
            ASSISTANT_DEPARTMENT_ID,
            "助理部门",
            &[DEFAULT_AGENT_ID],
            &[REVIEWER_DEPARTMENT_ID],
        );
        let reviewer = dept(REVIEWER_DEPARTMENT_ID, "reviewer", &[REVIEWER_AGENT_ID], &[]);
        let plan = plan_agent_org_migration(&[assistant, reviewer]);
        assert!(
            plan.child_agent_ids.get(DEFAULT_AGENT_ID).is_none(),
            "内置→内置边不应由迁移产生"
        );
    }

    #[test]
    fn ensure_should_add_missing_built_in_nodes_and_preset_edges() {
        let mut agents = vec![test_agent(DEFAULT_AGENT_ID), test_agent(DEPUTY_AGENT_ID)];
        assert!(ensure_built_in_organization_nodes(&mut agents));
        for id in [
            REVIEWER_AGENT_ID,
            SADDLER_AGENT_ID,
            SUPPORT_AGENT_ID,
        ] {
            assert!(agents.iter().any(|agent| agent.id == id), "内置人格 {id} 应被补建");
        }
        let root = agents
            .iter()
            .find(|agent| agent.id == DEFAULT_AGENT_ID)
            .expect("root");
        for child in [
            DEPUTY_AGENT_ID,
            REVIEWER_AGENT_ID,
            SADDLER_AGENT_ID,
            SUPPORT_AGENT_ID,
        ] {
            assert!(
                root.child_agent_ids.iter().any(|item| item == child),
                "根应预设下级 {child}"
            );
        }
        // 幂等：再次执行不再产生变化。
        assert!(!ensure_built_in_organization_nodes(&mut agents));
    }

    fn test_agent(id: &str) -> AgentProfile {
        AgentProfile {
            id: id.to_string(),
            name: id.to_string(),
            system_prompt: String::new(),
            tools: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: "main_config".to_string(),
            scope: "global".to_string(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        }
    }

    #[test]
    fn built_in_department_child_custom_department_should_hoist_member_to_built_in_node() {
        let assistant = dept(
            ASSISTANT_DEPARTMENT_ID,
            "助理部门",
            &[DEFAULT_AGENT_ID],
            &["department-yae"],
        );
        let yae = dept("department-yae", "八重堂", &["yae-miko"], &[]);
        let plan = plan_agent_org_migration(&[assistant, yae]);

        let assistants_children = plan
            .child_agent_ids
            .get(DEFAULT_AGENT_ID)
            .expect("assistants children");
        assert!(assistants_children.contains(&"yae-miko".to_string()));
    }

    #[test]
    fn apply_should_be_idempotent_and_not_overwrite_existing_values() {
        let mut department = dept("department-1", "八重堂", &["yae-miko"], &[]);
        department.summary = "部门简介".to_string();
        department.api_config_ids = vec!["model-dept".to_string()];
        let plan = plan_agent_org_migration(&[department]);

        let mut agents = vec![AgentProfile {
            id: "yae-miko".to_string(),
            name: "八重神子".to_string(),
            system_prompt: "existing".to_string(),
            tools: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
            avatar_path: None,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_agent_memory_recall_mode(),
            source: "main_config".to_string(),
            scope: "global".to_string(),
            summary: "人格已有简介".to_string(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            api_config_ids: vec!["model-existing".to_string()],
            api_config_id: "model-existing".to_string(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        }];

        assert!(apply_agent_org_migration_plan(&mut agents, &plan));
        assert_eq!(agents[0].summary, "人格已有简介");
        assert_eq!(agents[0].api_config_ids, vec!["model-existing".to_string()]);
        // 已有值不覆盖，但缺失的常驻 skill 应补上。
        assert_eq!(agents[0].resident_skill_names, vec!["八重堂".to_string()]);

        // 再次应用：已无增量，幂等。
        assert!(!apply_agent_org_migration_plan(&mut agents, &plan));
    }

    #[test]
    fn legacy_expert_key_view_should_ignore_unknown_departments_shape() {
        // 旧配置里 departments 的结构可能和当前迁移视图对不上；
        // 抢救旧键的最小视图必须不受其影响，否则连键一起拿不到。
        let raw = "expertApiConfigId = \"\"\n\
                   assistantDepartmentApiConfigId = \"model-x\"\n\
                   departments = [{ id = \"department-1\", extra = 1 }]\n";
        let view: LegacyExpertModelKeyView = toml::from_str(raw).expect("解析旧配置视图");
        assert_eq!(
            view.assistant_department_api_config_id.as_deref(),
            Some("model-x")
        );
    }

    #[test]
    fn carry_legacy_expert_model_key_should_move_key_and_be_idempotent() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "pai-legacy-expert-key-{}-{unique}.toml",
            std::process::id()
        ));
        fs::write(
            &path,
            "assistantDepartmentApiConfigId = \"model-x\"\nexpertApiConfigId = \"\"\n",
        )
        .expect("写入临时配置");

        assert!(carry_legacy_expert_model_key(&path, "model-x").expect("首次搬迁"));
        let raw = fs::read_to_string(&path).expect("读取临时配置");
        assert!(raw.contains("expertApiConfigId = \"model-x\""));
        assert!(!raw.contains("assistantDepartmentApiConfigId"));

        assert!(!carry_legacy_expert_model_key(&path, "model-x").expect("幂等搬迁"));
        let _ = fs::remove_file(&path);
    }
}
