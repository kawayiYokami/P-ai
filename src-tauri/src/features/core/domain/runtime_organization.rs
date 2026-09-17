#[derive(Debug, Clone)]
struct RuntimeOrganizationSnapshot {
    config: AppConfig,
    agents: Vec<AgentProfile>,
    /// 人格主索引：组织就是人格本身，一切解析优先走这里。
    agents_by_id: std::collections::HashMap<String, AgentProfile>,
}

/// 归一化运行态的直属下级：去掉空值、自环、指向不存在人格的边，并移除成环的边。
/// 配置与私有组织都可能写入层级，运行态必须先收敛，后续遍历才安全。
fn normalize_runtime_organization_agent_children(agents: &mut [AgentProfile]) {
    let valid_ids = agents
        .iter()
        .map(|agent| agent.id.trim().to_string())
        .filter(|agent_id| !agent_id.is_empty())
        .collect::<std::collections::HashSet<_>>();
    let mut children_by_parent = std::collections::BTreeMap::<String, Vec<String>>::new();
    for agent in agents.iter() {
        let agent_id = agent.id.trim().to_string();
        if agent_id.is_empty() {
            continue;
        }
        let children = normalize_agent_child_ids(&agent.child_agent_ids, &agent.id)
            .into_iter()
            .filter(|child_id| valid_ids.contains(child_id))
            .collect::<Vec<_>>();
        children_by_parent.insert(agent_id, children);
    }
    let removed = remove_cyclic_child_edges(&mut children_by_parent);
    for agent in agents.iter_mut() {
        let agent_id = agent.id.trim().to_string();
        if agent_id.is_empty() {
            continue;
        }
        if let Some(children) = children_by_parent.get(&agent_id) {
            agent.child_agent_ids = children.clone();
        }
    }
    if !removed.is_empty() {
        let edges = removed
            .iter()
            .map(|(parent_id, child_id)| format!("{parent_id}->{child_id}"))
            .collect::<Vec<_>>()
            .join(", ");
        runtime_log_warn(format!(
            "[运行组织] 跳过成环人格关系: count={}, edges={}",
            removed.len(),
            edges
        ));
    }
}

fn build_runtime_organization_snapshot_from_parts(
    data_path: &PathBuf,
    base_config: &AppConfig,
    base_agents: &[AgentProfile],
) -> Result<RuntimeOrganizationSnapshot, String> {
    let config = base_config.clone();
    let mut runtime_data = AppData::default();
    runtime_data.agents = base_agents.to_vec();
    merge_private_organization_into_runtime_data(data_path, &config, &mut runtime_data)?;
    normalize_runtime_organization_agent_children(&mut runtime_data.agents);

    let mut agents_by_id = std::collections::HashMap::<String, AgentProfile>::new();
    for agent in &runtime_data.agents {
        let agent_id = agent.id.trim();
        if agent_id.is_empty() {
            continue;
        }
        agents_by_id.insert(agent_id.to_string(), agent.clone());
    }

    Ok(RuntimeOrganizationSnapshot {
        config,
        agents: runtime_data.agents,
        agents_by_id,
    })
}

fn load_runtime_organization_snapshot(
    state: &AppState,
) -> Result<RuntimeOrganizationSnapshot, String> {
    let config = state_read_config_cached(state)?;
    let agents = state_read_agents_cached(state)?;
    build_runtime_organization_snapshot_from_parts(&state.data_path, &config, &agents)
}

/// 人格主索引取人格。组织就是人格本身，新模型下的解析一律走这里。
fn runtime_agent_by_id<'a>(
    snapshot: &'a RuntimeOrganizationSnapshot,
    agent_id: &str,
) -> Option<&'a AgentProfile> {
    let agent_id = agent_id.trim();
    if agent_id.is_empty() {
        return None;
    }
    snapshot.agents_by_id.get(agent_id)
}

/// 一个候选人格是否属于「可被指派/解析」的人格：排除用户人格。
/// 系统人格（`system-persona`）不在此排除，与既有的 `available_non_user_agent` 口径一致。
fn runtime_agent_is_available(agent: &AgentProfile) -> bool {
    !agent.is_built_in_user
}

fn runtime_available_agent<'a>(
    snapshot: &'a RuntimeOrganizationSnapshot,
    agent_id: &str,
) -> Option<&'a AgentProfile> {
    runtime_agent_by_id(snapshot, agent_id).filter(|agent| runtime_agent_is_available(agent))
}

/// 解析外部（模型或前端）给出的人格引用：先按 id 精确匹配，再退回按名称忽略大小写匹配。
/// 组织清单只向模型展示人格名称，因此委托链路必须同时接受名称。
/// 名称不唯一时不擅自挑选，直接返回错误让调用方改用人格 id。
fn runtime_resolve_agent_ref<'a>(
    snapshot: &'a RuntimeOrganizationSnapshot,
    agent_ref: &str,
) -> Result<&'a AgentProfile, String> {
    let agent_ref = agent_ref.trim();
    if agent_ref.is_empty() {
        return Err("缺少人格引用".to_string());
    }
    if let Some(agent) = snapshot.agents_by_id.get(agent_ref) {
        return Ok(agent);
    }
    let mut matched = snapshot
        .agents
        .iter()
        .filter(|agent| agent.name.trim().eq_ignore_ascii_case(agent_ref));
    match (matched.next(), matched.next()) {
        (Some(agent), None) => Ok(agent),
        (Some(_), Some(_)) => Err(format!("人格名称不唯一，请改用人格 id：{agent_ref}")),
        (None, _) => Err(format!("未找到人格：{agent_ref}")),
    }
}

#[cfg(test)]
mod runtime_organization_tests {
    use super::*;

    fn agent_profile(id: &str, name: &str) -> AgentProfile {
        AgentProfile {
            id: id.to_string(),
            name: name.to_string(),
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
            source: "global".to_string(),
            scope: "global".to_string(),
            summary: String::new(),
            resident_skill_names: Vec::new(),
            optional_skill_names: Vec::new(),
            include_system_rules: true,
            api_config_ids: Vec::new(),
            api_config_id: String::new(),
            model_failure_fallback_enabled: false,
            permission_control: AgentPermissionControl::default(),
            child_agent_ids: Vec::new(),
        }
    }

    fn snapshot_with(agents: Vec<AgentProfile>) -> RuntimeOrganizationSnapshot {
        let agents_by_id = agents
            .iter()
            .map(|agent| (agent.id.clone(), agent.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        RuntimeOrganizationSnapshot {
            config: AppConfig::default(),
            agents,
            agents_by_id,
        }
    }

    #[test]
    fn resolve_agent_ref_should_match_id_and_name() {
        let snapshot = snapshot_with(vec![
            agent_profile("deputy-agent", "副手"),
            agent_profile("hr", "HR"),
        ]);
        assert_eq!(
            runtime_resolve_agent_ref(&snapshot, "deputy-agent")
                .map(|agent| agent.id.as_str()),
            Ok("deputy-agent")
        );
        assert_eq!(
            runtime_resolve_agent_ref(&snapshot, "  副手 ").map(|agent| agent.id.as_str()),
            Ok("deputy-agent")
        );
        assert_eq!(
            runtime_resolve_agent_ref(&snapshot, "hr").map(|agent| agent.id.as_str()),
            Ok("hr")
        );
        assert_eq!(
            runtime_resolve_agent_ref(&snapshot, "HR").map(|agent| agent.id.as_str()),
            Ok("hr")
        );
    }

    #[test]
    fn resolve_agent_ref_should_reject_ambiguous_name_and_unknown_ref() {
        let snapshot = snapshot_with(vec![
            agent_profile("worker-a", "整理员"),
            agent_profile("worker-b", "整理员"),
        ]);
        let ambiguous = runtime_resolve_agent_ref(&snapshot, "整理员").unwrap_err();
        assert!(ambiguous.contains("名称不唯一"), "unexpected: {ambiguous}");
        assert!(runtime_resolve_agent_ref(&snapshot, "worker-a").is_ok());
        let missing = runtime_resolve_agent_ref(&snapshot, "不存在").unwrap_err();
        assert!(missing.contains("未找到人格"), "unexpected: {missing}");
        assert!(runtime_resolve_agent_ref(&snapshot, "   ").is_err());
    }
}