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
    /// 仅追问会话（side_chat）挂载，主会话（chat）不挂。
    ///
    /// 分工：主会话同时盯多条线，管的是议题怎么流转（看板）；追问会话单线程单任务，
    /// 管的是这条线做到哪一步（进度）。两套各管一层，互不重叠。
    /// 追问会话与队员会话都属于这一档——队员就是靠它汇报自己做到哪一步。
    /// delegate 有独立的状态源，remote_im_contact 与 system_notification 也不该有会话状态。
    SideChatOnly,
    /// 目前没有任何条目使用它，但分支逻辑与语义都保留：
    /// 恢复旧的「主会话 + 追问会话都挂」口径时，把条目改回 LocalChatOnly 即可。
    #[allow(dead_code)]
    LocalChatOnly,
    /// 最严一档：仅 `chat` 主会话，连追问会话（side_chat）与队员会话都不放行。
    /// 本函数只拿到 `local_conversation` 布尔值，判不了 kind 本身，
    /// 因此这一档在这里一律放行，真正的严判在 `RuntimeToolPolicy::tool_unavailable_reason`。
    LocalMainChatOnly,
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
        id: "image_to_video",
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
    // progress 状态工具：SystemExempt 强制挂载（人格不可关），作用域限定为追问会话。
    // 主会话用议题看板管流转，追问会话用进度管自己这条线做到哪一步；两套各管一层，不重叠。
    BuiltinToolPolicy {
        id: "progress",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::SideChatOnly,
        prompt_rule_id: Some("progress"),
        visible_in_permission_lists: false,
    },
    // 队伍议题看板：队长专属，编译期不可关；挂载与否由装配期的队伍角色二次筛选
    // （见 team_session_allows_kanban），权限层不再单独控制。
    BuiltinToolPolicy {
        id: "kanban",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: Some("kanban"),
        visible_in_permission_lists: false,
    },
    // 议题设置计划文件：与看板共用同一份规则（prompt_rule_id 相同），
    // 规则块只按 "kanban" 装载一次，不会因为两个工具而重复注入。
    BuiltinToolPolicy {
        id: "kanban_set_plan",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: Some("kanban"),
        visible_in_permission_lists: false,
    },
    // 会话提问工具：只给本地主会话（chat），追问、队员、委托、远程一律不可见。
    // 使用说明写在工具描述里，不注入系统提示词。
    BuiltinToolPolicy {
        id: "question",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::LocalMainChatOnly,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
    // MCP 展开参数的拿取入口：属于运行时机制工具，不受权限白名单影响，也不进权限列表。
    // 使用说明写在工具描述与 MCP 目录里，不注入系统提示词。
    BuiltinToolPolicy {
        id: "get_tool",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: None,
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
    // 队伍工具：挂在会话上按「是不是队长／队员」二次筛选，权限层不再单独控制。
    // 使用说明只写在各自的工具描述里，不注入系统提示词。
    BuiltinToolPolicy {
        id: "team_info",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "team_recruit",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "team_send",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "team_report",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "team_share_model",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: None,
        visible_in_permission_lists: false,
    },
    BuiltinToolPolicy {
        id: "team_read",
        permission_class: BuiltinToolPermissionClass::SystemExempt,
        runtime_scope: BuiltinToolRuntimeScope::Any,
        prompt_rule_id: None,
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
    side_chat_conversation: bool,
    delegate_conversation: bool,
    remote_reply_delegate: bool,
    contact_send_files_allowed: bool,
    deep_recall_delegate: bool,
) -> Option<String> {
    let policy = builtin_tool_policy(tool_id);
    match policy.runtime_scope {
        BuiltinToolRuntimeScope::Any => None,
        BuiltinToolRuntimeScope::LocalConversation if local_conversation => None,
        BuiltinToolRuntimeScope::LocalConversation => {
            Some("plan 仅在本地会话中可用".to_string())
        }
        // 判定顺序：先拒 resolved 为假（无法确认会话类型，安全跳过），
        // 再只放行追问会话。主会话改用议题看板，不再写会话状态。
        BuiltinToolRuntimeScope::SideChatOnly if !conversation_resolved => {
            Some("无法确认当前会话类型，进度工具已安全跳过".to_string())
        }
        BuiltinToolRuntimeScope::SideChatOnly if side_chat_conversation => None,
        BuiltinToolRuntimeScope::SideChatOnly => {
            Some("进度工具仅在追问会话（含队员会话）中可用".to_string())
        }
        // 目前没有条目使用这一档，判定链保留：恢复旧的「主会话 + 追问会话都挂」口径时直接可用。
        BuiltinToolRuntimeScope::LocalChatOnly if !conversation_resolved => {
            Some("无法确认当前会话类型，进度工具已安全跳过".to_string())
        }
        BuiltinToolRuntimeScope::LocalChatOnly if local_conversation => None,
        BuiltinToolRuntimeScope::LocalChatOnly => {
            Some("进度工具仅在主会话与追问会话中可用".to_string())
        }
        // 本档位在这里一律放行：更严的「仅 chat 主会话」判定由
        // `RuntimeToolPolicy::tool_unavailable_reason` 补齐（那里拿得到会话 kind）。
        BuiltinToolRuntimeScope::LocalMainChatOnly => None,
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
            Some("远程群聊的应答委托禁止使用 Goal 工具".to_string())
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
    side_chat_conversation: bool,
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
                side_chat_conversation,
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

    #[test]
    fn progress_policy_should_be_system_exempt_and_hidden() {
        let policy = builtin_tool_policy("progress");
        assert_eq!(
            policy.permission_class,
            BuiltinToolPermissionClass::SystemExempt
        );
        // 作用域收在主会话之外：主会话管议题流转（看板），追问会话管自己这条线（进度）。
        assert_eq!(policy.runtime_scope, BuiltinToolRuntimeScope::SideChatOnly);
        assert_eq!(policy.prompt_rule_id, Some("progress"));
        assert!(!policy.visible_in_permission_lists);
    }

    #[test]
    fn progress_tool_should_only_attach_in_side_chat() {
        // (origin, resolved, local, side_chat, 是否可用)
        for (origin_scope, resolved, local_conversation, side_chat_conversation, available) in [
            // 追问会话是唯一放行的一档；队员会话也是 side_chat，走同一档。
            (RuntimeToolOriginScope::Local, true, false, true, true),
            // 主会话改用议题看板，不再挂进度工具。
            (RuntimeToolOriginScope::Local, true, true, false, false),
            // 会话类型无法确认时安全跳过，不猜。
            (RuntimeToolOriginScope::Local, false, false, false, false),
            // delegate 有独立状态源，远程会话也没有会话状态。
            (RuntimeToolOriginScope::Unknown, true, false, false, false),
            (RuntimeToolOriginScope::RemotePrivate, true, false, false, false),
        ] {
            let reason = builtin_tool_runtime_unavailable_reason(
                "progress",
                origin_scope,
                resolved,
                local_conversation,
                side_chat_conversation,
                false,
                false,
                false,
                false,
            );
            assert_eq!(
                reason.is_none(),
                available,
                "origin={origin_scope:?} resolved={resolved} local={local_conversation} side_chat={side_chat_conversation} 结论错误，reason={reason:?}"
            );
        }
        // 规则注入与工具挂载同源：追问会话注入，主会话不注入。
        assert!(builtin_tool_prompt_rule_allowed_in_runtime(
            "progress",
            RuntimeToolOriginScope::Local,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
        ));
        assert!(!builtin_tool_prompt_rule_allowed_in_runtime(
            "progress",
            RuntimeToolOriginScope::Local,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
        ));
        // 规则正文映射与规则 id 都在，恢复或调整作用域都不需要重建资源。
        assert_eq!(
            builtin_tool_ids_for_prompt_rule("progress"),
            vec!["progress"]
        );
    }

    #[test]
    fn side_chat_only_scope_should_only_cover_progress() {
        // 收窄作用域是 progress 的单点决定，其余内置工具一律不走这一档
        for tool_id in ["todo", "fetch", "task", "delegate", "get_tool", "plan"] {
            assert_ne!(
                builtin_tool_policy(tool_id).runtime_scope,
                BuiltinToolRuntimeScope::SideChatOnly,
                "{tool_id} 不应被收窄到追问会话"
            );
        }
    }
}
