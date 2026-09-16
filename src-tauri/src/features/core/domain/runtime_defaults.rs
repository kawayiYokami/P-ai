fn format_message_time_rfc3339_local(raw: &str) -> String {
    format_utc_storage_time_to_local_rfc3339(raw)
}

fn format_message_time_text(raw: &str) -> String {
    format_utc_storage_time_to_local_text(raw)
}

fn default_agent() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: DEFAULT_AGENT_ID.to_string(),
        name: "Pai".to_string(),
        system_prompt: "你是谁：你是Pai，是用户默认会先对话的助手。\n台词技巧：表达自然、直接、有人味；先给结论，再补必要说明；少空话，少套话。\n性格画像：耐心、友善、靠谱、利落。".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: false,
        private_memory_enabled: false,
        memory_recall_mode: default_agent_memory_recall_mode(),
        source: default_main_source(),
        scope: default_global_scope(),
        summary: "主助理人格：理解用户需求、决定是否委派、汇总下级结果并继续推进主对话。".to_string(),
        resident_skill_names: Vec::new(),
        optional_skill_names: Vec::new(),
        api_config_ids: Vec::new(),
        api_config_id: String::new(),
        model_failure_fallback_enabled: false,
        permission_control: AgentPermissionControl::default(),
        child_agent_ids: built_in_assistant_child_agent_ids(),
    }
}

/// 内置组织骨架的完整层级：父人格 id → 直接下级人格 id。
/// 内置人格之间的上下级只在这里定义；旧 `departments` 的迁移不再推导内置↔内置边，
/// 以免原内置部门「多根 + leader 反过来指向助理」造成环。迁移只负责自定义部门相关的人-人边。
/// 内置组织以 `assistants`（人格 id `default-agent`）为根，其直接下级是唯一一条内置层级：
/// 原内置无上级的（leader/support/hr）与 reviewer/saddler、explorer（`deputy-agent`）都挂到根下。
fn built_in_organization_child_edges() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![(
        DEFAULT_AGENT_ID,
        vec![
            DEPUTY_AGENT_ID,
            REVIEWER_AGENT_ID,
            SADDLER_AGENT_ID,
            SUPPORT_AGENT_ID,
        ],
    )]
}

/// `assistants`（人格 id `default-agent`）的直接下级，取自内置层级预设。
fn built_in_assistant_child_agent_ids() -> Vec<String> {
    built_in_organization_child_edges()
        .into_iter()
        .find(|(parent_id, _)| *parent_id == DEFAULT_AGENT_ID)
        .map(|(_, children)| children.into_iter().map(|child| child.to_string()).collect())
        .unwrap_or_default()
}

/// 判断人格 id 是否是内置人格（迁移用来区分「内置↔内置边」与「自定义相关边」）。
fn is_built_in_organization_agent_id(agent_id: &str) -> bool {
    matches!(
        agent_id.trim(),
        DEFAULT_AGENT_ID
            | DEPUTY_AGENT_ID
            | REVIEWER_AGENT_ID
            | SADDLER_AGENT_ID
            | SUPPORT_AGENT_ID
    )
}

/// 内置人格的公共构造：只读内置、无模型、无权限、无下级。
/// 标记与 `default-agent`（派师傅）保持一致：**不带系统标记**——「内置」与「系统」是两件事，
/// 前者是出厂预设、靠一份写死的 id 名单实现只读；后者是「只作为系统播报、不在选择器出现」的隐藏标记。
/// 内置人格是组织成员，要能被看到、被选为上下级，故只走前者。
/// 详细职责由其常驻的内置 guide skill 承担（常驻 skill 全文注入，结论 24），
/// 故 system_prompt 只留一句自我介绍。
fn built_in_organization_agent(
    id: &str,
    name: &str,
    summary: &str,
    system_prompt: &str,
    resident_skill_name: &str,
) -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: id.to_string(),
        name: name.to_string(),
        system_prompt: system_prompt.to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: false,
        private_memory_enabled: false,
        memory_recall_mode: default_agent_memory_recall_mode(),
        source: default_main_source(),
        scope: default_global_scope(),
        summary: summary.to_string(),
        resident_skill_names: vec![resident_skill_name.to_string()],
        optional_skill_names: Vec::new(),
        api_config_ids: Vec::new(),
        api_config_id: String::new(),
        model_failure_fallback_enabled: false,
        permission_control: AgentPermissionControl::default(),
        child_agent_ids: Vec::new(),
    }
}

fn default_reviewer_agent() -> AgentProfile {
    let mut agent = built_in_organization_agent(
        REVIEWER_AGENT_ID,
        "reviewer",
        "当你完成复杂功能、关键修复或高风险改动后，请委托我进行代码审查。",
        "你是谁：你是 reviewer，负责对已完成的实现做独立审查，只报告真实、可复现、影响正确性/稳定性/安全的缺陷。详细职责见你的常驻 skill。\n台词技巧：先列问题再下判断；有证据才说，没有就说没有。\n性格画像：严谨、克制、就事论事。",
        "reviewer",
    );
    agent.permission_control = reviewer_permission_control();
    agent
}

fn default_saddler_agent() -> AgentProfile {
    let mut agent = built_in_organization_agent(
        SADDLER_AGENT_ID,
        "saddler",
        "当项目需要沉淀协作规范、AGENTS.md、Skill、workflow 或其他 .pai 能力资产时，请委托给我。",
        "你是谁：你是 saddler，专门在当前项目 `.pai/` 目录下生成和维护能力资产。详细职责见你的常驻 skill。\n台词技巧：说清写在哪、为什么这么定；不越界改业务代码。\n性格画像：细致、有规范意识、克制。",
        "saddler",
    );
    agent.permission_control = saddler_permission_control();
    agent
}

fn default_support_agent() -> AgentProfile {
    let mut agent = built_in_organization_agent(
        SUPPORT_AGENT_ID,
        "support",
        SUPPORT_AGENT_SUMMARY,
        "你是谁：你是 support，负责远程客服场景的应答，处理外部联系人的咨询与消息。详细职责见你的常驻 skill。\n台词技巧：礼貌、清楚、直接回应对方诉求，不寒暄过度。\n性格画像：耐心、稳妥、有服务意识。",
        "support",
    );
    agent.permission_control = support_permission_control();
    agent
}

fn built_in_organization_agents() -> Vec<AgentProfile> {
    vec![
        default_reviewer_agent(),
        default_saddler_agent(),
        default_support_agent(),
    ]
}

#[allow(dead_code)]
fn default_deputy_agent() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: DEPUTY_AGENT_ID.to_string(),
        name: "副手".to_string(),
        system_prompt: "你是谁：你是副手，是一个偏执行、偏推进的助手分身。\n台词技巧：短句作答，直给重点，少铺垫，少客套。\n性格画像：简洁、干脆、克制、利落。".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: true,
        private_memory_enabled: false,
        memory_recall_mode: default_agent_memory_recall_mode(),
        source: default_main_source(),
        scope: default_global_scope(),
        summary: "围绕明确主题做大范围摸底：搜集证据、定位文件与调用链、梳理影响面与风险。".to_string(),
        resident_skill_names: Vec::new(),
        optional_skill_names: Vec::new(),
        api_config_ids: Vec::new(),
        api_config_id: String::new(),
        model_failure_fallback_enabled: false,
        permission_control: explorer_permission_control(),
        child_agent_ids: Vec::new(),
    }
}

fn default_user_persona() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: USER_PERSONA_ID.to_string(),
        name: "用户".to_string(),
        system_prompt: "我是...".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: true,
        is_built_in_system: false,
        private_memory_enabled: false,
        memory_recall_mode: default_agent_memory_recall_mode(),
        source: default_main_source(),
        scope: default_global_scope(),
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

fn default_system_persona() -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: SYSTEM_PERSONA_ID.to_string(),
        name: "pai system".to_string(),
        system_prompt: "你是谁：你是 pai system，是系统消息与状态播报使用的人格。\n台词技巧：用词明确、稳定、客观，像系统通知，不抒情，不延展。\n性格画像：冷静、克制、严谨。".to_string(),
        tools: default_agent_tools(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: None,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: true,
        private_memory_enabled: false,
        memory_recall_mode: default_agent_memory_recall_mode(),
        source: default_main_source(),
        scope: default_global_scope(),
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

fn normalize_agent_tools(agent: &mut AgentProfile) -> bool {
    let defaults = default_agent_tools();
    let mut next = Vec::<ApiToolConfig>::new();
    for default in defaults {
        let found = agent.tools.iter().find(|tool| {
            tool.id == default.id || (default.id == "read" && tool.id == "read_file")
        });
        if let Some(found) = found {
            next.push(ApiToolConfig {
                id: default.id.clone(),
                command: if found.command.trim().is_empty() {
                    default.command.clone()
                } else {
                    found.command.clone()
                },
                args: if found.args.is_empty() {
                    default.args.clone()
                } else if default.id == "read" && found.id == "read_file" {
                    default.args.clone()
                } else {
                    found.args.clone()
                },
                enabled: found.enabled,
                values: found.values.clone(),
            });
        } else {
            next.push(default);
        }
    }
    let changed = agent.tools.len() != next.len()
        || agent.tools.iter().zip(next.iter()).any(|(left, right)| {
            left.id != right.id
                || left.enabled != right.enabled
                || left.command != right.command
                || left.args != right.args
                || left.values != right.values
        });
    if changed {
        agent.tools = next;
    }
    changed
}

fn fill_missing_conversation_message_speaker_agent_ids(conversation: &mut Conversation) -> bool {
    fn provider_meta_speaker_agent_id(message: &ChatMessage) -> Option<String> {
        let meta = message.provider_meta.as_ref()?;
        let object = meta.as_object()?;
        for key in [
            "speakerAgentId",
            "speaker_agent_id",
            "targetAgentId",
            "target_agent_id",
            "agentId",
            "agent_id",
            "sourceAgentId",
            "source_agent_id",
        ] {
            let value = object
                .get(key)
                .and_then(|item| item.as_str())
                .unwrap_or("")
                .trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        None
    }

    let host_agent_id = conversation.agent_id.trim().to_string();
    if host_agent_id.is_empty() {
        return false;
    }
    let mut changed = false;
    for message in &mut conversation.messages {
        let current = message
            .speaker_agent_id
            .as_deref()
            .map(str::trim)
            .unwrap_or("");
        if current.is_empty() {
            message.speaker_agent_id =
                Some(provider_meta_speaker_agent_id(message).unwrap_or_else(|| {
                    if message.role == "user" {
                        USER_PERSONA_ID.to_string()
                    } else {
                        host_agent_id.clone()
                    }
                }));
            changed = true;
        }
    }
    changed
}
