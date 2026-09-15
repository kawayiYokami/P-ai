// ==================== 内置工具统一策略表 ====================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinToolPermissionClass {
    PermissionControlled,
    SystemExempt,
    LocalConversationExempt,
    ContactCapabilityExempt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinToolRuntimeScope {
    Any,
    LocalConversation,
    DeepRecallDelegate,
    ResolvedTaskConversation,
    BoundContactWithFileSending,
    NotRemoteGroup,
    NeverAttach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeToolOriginScope {
    Local,
    RemotePrivate,
    RemoteGroup,
    RemoteUnknown,
    Unknown,
}

impl Default for RuntimeToolOriginScope {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BuiltinToolPolicy {
    id: &'static str,
    permission_class: BuiltinToolPermissionClass,
    runtime_scope: BuiltinToolRuntimeScope,
    prompt_rule_id: Option<&'static str>,
    visible_in_permission_lists: bool,
}

const DEFAULT_BUILTIN_TOOL_POLICY: BuiltinToolPolicy = BuiltinToolPolicy {
    id: "",
    permission_class: BuiltinToolPermissionClass::PermissionControlled,
    runtime_scope: BuiltinToolRuntimeScope::Any,
    prompt_rule_id: None,
    visible_in_permission_lists: true,
};

const BUILTIN_TOOL_POLICY_TABLE: &[BuiltinToolPolicy] = &[
    BuiltinToolPolicy {
        id: "fetch",
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "websearch",
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "operate",
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "windows",
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "read",
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "read_file",
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "read_media",
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "exec",
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: Some("exec"),
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    // background 是 exec 的伴生工具（同会话作用域）：与 exec 共用 prompt rule 与 origin 门槛，
    // 强制挂载档（SystemExempt 跳过人格权限检查），不允许人格禁用
    BuiltinToolPolicy {
        id: "background",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: Some("exec"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "config",
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "write",
        prompt_rule_id: Some("file_edit"),
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "delete",
        prompt_rule_id: Some("file_edit"),
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "update",
        prompt_rule_id: Some("file_edit"),
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "move",
        prompt_rule_id: Some("file_edit"),
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "delegate",
        prompt_rule_id: Some("delegate"),
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "deeprecall",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::LocalConversation,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "deeprecall_search",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::DeepRecallDelegate,
        prompt_rule_id: Some("deeprecall"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "deeprecall_context",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::DeepRecallDelegate,
        prompt_rule_id: Some("deeprecall"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "meme",
        prompt_rule_id: Some("meme"),
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "image_generate",
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "image_edit",
        ..DEFAULT_BUILTIN_TOOL_POLICY
    },
    BuiltinToolPolicy {
        id: "remember",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "recall",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "todo",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: Some("todo"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "task",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::ResolvedTaskConversation,
        prompt_rule_id: Some("task"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "create_goal",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::NotRemoteGroup,
        prompt_rule_id: Some("goal"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "update_goal",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::NotRemoteGroup,
        prompt_rule_id: Some("goal"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "get_session",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: Some("session"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "inform_session",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: Some("session"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "plan",
        permission_class: BuiltinToolPermissionClass::LocalConversationExempt,
        runtime_scope: BuiltinToolRuntimeScope::LocalConversation,
        prompt_rule_id: Some("plan"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "contact_send_files",
        permission_class: BuiltinToolPermissionClass::ContactCapabilityExempt,
        runtime_scope: BuiltinToolRuntimeScope::BoundContactWithFileSending,
        prompt_rule_id: Some("contact_tools"),
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "contact_reply",
        permission_class: BuiltinToolPermissionClass::ContactCapabilityExempt,
        runtime_scope: BuiltinToolRuntimeScope::NeverAttach,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "contact_no_reply",
        permission_class: BuiltinToolPermissionClass::ContactCapabilityExempt,
        runtime_scope: BuiltinToolRuntimeScope::NeverAttach,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
];

fn builtin_tool_policy(tool_id: &str) -> BuiltinToolPolicy {
    let normalized = tool_id.trim();
    BUILTIN_TOOL_POLICY_TABLE
        .iter()
        .copied()
        .find(|policy| policy.id == normalized)
        .unwrap_or(DEFAULT_BUILTIN_TOOL_POLICY)
}

#[cfg(test)]
fn builtin_tool_policy_is_explicit(tool_id: &str) -> bool {
    let normalized = tool_id.trim();
    !normalized.is_empty()
        && BUILTIN_TOOL_POLICY_TABLE
            .iter()
            .any(|policy| policy.id == normalized)
}

fn builtin_tool_is_fixed_system_from_policy(tool_id: &str) -> bool {
    builtin_tool_policy(tool_id).permission_class == BuiltinToolPermissionClass::SystemExempt
}

fn builtin_tool_is_local_conversation_fixed_from_policy(tool_id: &str) -> bool {
    builtin_tool_policy(tool_id).permission_class
        == BuiltinToolPermissionClass::LocalConversationExempt
}

fn builtin_tool_is_contact_only_hidden_from_policy(tool_id: &str) -> bool {
    builtin_tool_policy(tool_id).permission_class
        == BuiltinToolPermissionClass::ContactCapabilityExempt
}

fn builtin_tool_is_permission_controlled_from_policy(tool_id: &str) -> bool {
    !tool_id.trim().is_empty()
        && builtin_tool_policy(tool_id).permission_class
            == BuiltinToolPermissionClass::PermissionControlled
}

fn builtin_tool_visible_in_permission_lists_from_policy(tool_id: &str) -> bool {
    builtin_tool_policy(tool_id).visible_in_permission_lists
}

fn runtime_tool_origin_scope_from_contact_type(contact_type: &str) -> RuntimeToolOriginScope {
    match contact_type.trim().to_ascii_lowercase().as_str() {
        "group" => RuntimeToolOriginScope::RemoteGroup,
        "private" | "direct" | "single" => RuntimeToolOriginScope::RemotePrivate,
        _ => RuntimeToolOriginScope::Unknown,
    }
}

fn runtime_tool_origin_scope_from_activation_sources(
    sources: &[RemoteImActivationSource],
) -> Option<RuntimeToolOriginScope> {
    resolve_bound_remote_im_activation_source(sources)
        .map(|source| runtime_tool_origin_scope_from_contact_type(&source.remote_contact_type))
}

fn builtin_tool_runtime_unavailable_reason(
    tool_id: &str,
    origin_scope: RuntimeToolOriginScope,
    conversation_resolved: bool,
    local_conversation: bool,
    delegate_conversation: bool,
    remote_reply_delegate: bool,
    contact_send_files_allowed: bool,
    deep_recall_delegate: bool,
) -> Option<String> {
    match builtin_tool_policy(tool_id).runtime_scope {
        BuiltinToolRuntimeScope::Any => None,
        BuiltinToolRuntimeScope::LocalConversation if local_conversation => None,
        BuiltinToolRuntimeScope::LocalConversation => {
            Some("plan 仅在本地会话中可用".to_string())
        }
        BuiltinToolRuntimeScope::DeepRecallDelegate if deep_recall_delegate => None,
        BuiltinToolRuntimeScope::DeepRecallDelegate => {
            Some("深度回忆检索工具仅在深度回忆委托会话中可用".to_string())
        }
        BuiltinToolRuntimeScope::ResolvedTaskConversation if !conversation_resolved => {
            Some("无法确认当前会话类型，任务工具已安全跳过".to_string())
        }
        BuiltinToolRuntimeScope::ResolvedTaskConversation
            if delegate_conversation && !remote_reply_delegate =>
        {
            Some("委托会话禁止再次创建任务".to_string())
        }
        BuiltinToolRuntimeScope::ResolvedTaskConversation => None,
        BuiltinToolRuntimeScope::BoundContactWithFileSending if contact_send_files_allowed => None,
        BuiltinToolRuntimeScope::BoundContactWithFileSending => {
            Some("本轮没有可发送文件的有效联系人绑定".to_string())
        }
        BuiltinToolRuntimeScope::NotRemoteGroup
            if origin_scope == RuntimeToolOriginScope::RemoteGroup => {
            Some("远程群聊及其来源委托禁止使用 Goal 工具".to_string())
        }
        BuiltinToolRuntimeScope::NotRemoteGroup
            if origin_scope == RuntimeToolOriginScope::RemoteUnknown =>
        {
            Some("远程会话来源无法确认，已安全跳过 Goal 工具".to_string())
        }
        BuiltinToolRuntimeScope::NotRemoteGroup => None,
        BuiltinToolRuntimeScope::NeverAttach => {
            Some("当前运行时不挂载该联系人内部工具".to_string())
        }
    }
}

fn builtin_tool_prompt_rule_allowed_in_origin(
    prompt_rule_id: &str,
    origin_scope: RuntimeToolOriginScope,
) -> bool {
    builtin_tool_prompt_rule_allowed_in_runtime(
        prompt_rule_id,
        origin_scope,
        true,
        true,
        false,
        false,
        true,
        false,
    )
}

fn builtin_tool_prompt_rule_allowed_in_runtime(
    prompt_rule_id: &str,
    origin_scope: RuntimeToolOriginScope,
    conversation_resolved: bool,
    local_conversation: bool,
    delegate_conversation: bool,
    remote_reply_delegate: bool,
    contact_send_files_allowed: bool,
    deep_recall_delegate: bool,
) -> bool {
    BUILTIN_TOOL_POLICY_TABLE.iter().any(|policy| {
        policy.prompt_rule_id == Some(prompt_rule_id)
            && builtin_tool_runtime_unavailable_reason(
                policy.id,
                origin_scope,
                conversation_resolved,
                local_conversation,
                delegate_conversation,
                remote_reply_delegate,
                contact_send_files_allowed,
                deep_recall_delegate,
            )
            .is_none()
    })
}

fn builtin_tool_ids_for_prompt_rule(prompt_rule_id: &str) -> Vec<&'static str> {
    BUILTIN_TOOL_POLICY_TABLE
        .iter()
        .filter(|policy| policy.prompt_rule_id == Some(prompt_rule_id))
        .map(|policy| policy.id)
        .collect()
}

fn builtin_tool_requires_execution_reauthorization(tool_id: &str) -> bool {
    builtin_tool_is_permission_controlled_from_policy(tool_id)
        || !matches!(
            builtin_tool_policy(tool_id).runtime_scope,
            BuiltinToolRuntimeScope::Any | BuiltinToolRuntimeScope::NeverAttach
        )
}

#[cfg(test)]
mod builtin_tool_policy_tests {
    use super::*;

    #[test]
    fn fixed_tools_should_not_participate_in_permission_lists() {
        assert!(!builtin_tool_is_permission_controlled_from_policy("task"));
        assert!(!builtin_tool_is_permission_controlled_from_policy("contact_send_files"));
        assert!(!builtin_tool_is_permission_controlled_from_policy("create_goal"));
        assert!(builtin_tool_is_permission_controlled_from_policy("delegate"));
    }

    #[test]
    fn goal_rule_should_be_disabled_only_for_remote_groups() {
        assert!(builtin_tool_prompt_rule_allowed_in_origin(
            "goal",
            RuntimeToolOriginScope::Local
        ));
        assert!(builtin_tool_prompt_rule_allowed_in_origin(
            "goal",
            RuntimeToolOriginScope::RemotePrivate
        ));
        assert!(!builtin_tool_prompt_rule_allowed_in_origin(
            "goal",
            RuntimeToolOriginScope::RemoteGroup
        ));
        assert!(!builtin_tool_prompt_rule_allowed_in_origin(
            "goal",
            RuntimeToolOriginScope::RemoteUnknown
        ));
    }
}
