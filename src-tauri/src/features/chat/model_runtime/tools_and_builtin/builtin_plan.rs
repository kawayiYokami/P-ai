const PLAN_MARKDOWN_MAX_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone)]
struct ResolvedPlanFilePath {
    canonical_path: PathBuf,
    display_path: String,
}

fn plan_directory_for_root(root: &Path) -> PathBuf {
    root.join(".pai").join("plan")
}

fn plan_markdown_extension_allowed(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.trim().to_ascii_lowercase())
        .map(|value| value == "md")
        .unwrap_or(false)
}

fn plan_assistant_space_canonical(state: &AppState) -> Result<PathBuf, String> {
    terminal_system_workspace_resolved(state)
        .map(|workspace| workspace.path)
        .or_else(|_| {
            configured_workspace_root_path(state).and_then(|path| {
                path.canonicalize()
                    .map_err(|err| format!("解析助理空间失败: {err}"))
            })
        })
}

fn plan_preferred_directory_for_conversation(
    state: &AppState,
    conversation: Option<&Conversation>,
) -> Result<PathBuf, String> {
    if let Some(conversation) = conversation {
        if let Ok(workspace) =
            terminal_default_workspace_for_conversation_resolved(state, Some(conversation))
        {
            return Ok(plan_directory_for_root(&workspace.path));
        }
    }
    Ok(plan_directory_for_root(&plan_assistant_space_canonical(state)?))
}

pub(crate) fn plan_preferred_directory_display_for_conversation(
    state: &AppState,
    conversation: Option<&Conversation>,
) -> Result<String, String> {
    Ok(terminal_path_for_user(
        &plan_preferred_directory_for_conversation(state, conversation)?,
    ))
}

fn resolve_plan_candidate_path(base_root: &Path, raw_path: &str) -> Result<PathBuf, String> {
    let normalized = normalize_terminal_path_input_for_current_platform(raw_path.trim());
    if normalized.is_empty() {
        return Err("plan.path 不能为空".to_string());
    }
    let candidate = PathBuf::from(&normalized);
    if candidate.is_absolute() {
        Ok(candidate)
    } else {
        resolve_terminal_path(base_root, &normalized).map_err(|_| {
            format!(
                "plan.path 无效：{}。请先把计划写成 Markdown 文件，再传该文件的完整 path。",
                normalized
            )
        })
    }
}

fn plan_file_metadata(canonical_path: &Path) -> Result<std::fs::Metadata, String> {
    std::fs::metadata(canonical_path).map_err(|err| {
        format!(
            "读取计划文件元数据失败，path={}，error={err}",
            terminal_path_for_user(canonical_path)
        )
    })
}

pub(crate) fn resolve_plan_file_for_conversation(
    base_root: &Path,
    raw_path: &str,
) -> Result<ResolvedPlanFilePath, String> {
    let candidate = resolve_plan_candidate_path(base_root, raw_path)?;
    let canonical = candidate.canonicalize().map_err(|err| {
        format!(
            "plan.path 无效：{}。请确认该 Markdown 文件已写入磁盘，并传入该文件的完整 path。error={err}",
            terminal_path_for_user(&candidate)
        )
    })?;
    if !canonical.is_file() {
        return Err(format!(
            "plan.path 必须指向一个现有的 Markdown 文件，当前收到：{}",
            terminal_path_for_user(&canonical)
        ));
    }
    if !plan_markdown_extension_allowed(&canonical) {
        return Err("plan.path 必须指向 .md Markdown 文件".to_string());
    }
    let metadata = plan_file_metadata(&canonical)?;
    if metadata.len() > PLAN_MARKDOWN_MAX_BYTES {
        return Err(format!(
            "计划文件过大，path={}，size_bytes={}，max_bytes={}",
            terminal_path_for_user(&canonical),
            metadata.len(),
            PLAN_MARKDOWN_MAX_BYTES
        ));
    }
    Ok(ResolvedPlanFilePath {
        canonical_path: canonical.clone(),
        display_path: terminal_path_for_user(&canonical),
    })
}

pub(crate) fn read_plan_markdown_file(canonical_path: &Path) -> Result<String, String> {
    let metadata = plan_file_metadata(canonical_path)?;
    if metadata.len() > PLAN_MARKDOWN_MAX_BYTES {
        return Err(format!(
            "计划文件过大，path={}，size_bytes={}，max_bytes={}",
            terminal_path_for_user(canonical_path),
            metadata.len(),
            PLAN_MARKDOWN_MAX_BYTES
        ));
    }
    let content = std::fs::read_to_string(canonical_path).map_err(|err| {
        format!(
            "读取计划文件失败，path={}，error={err}",
            terminal_path_for_user(canonical_path)
        )
    })?;
    if content.trim().is_empty() {
        return Err(format!(
            "计划文件为空，path={}",
            terminal_path_for_user(canonical_path)
        ));
    }
    Ok(content)
}

pub(crate) fn plan_tool_description() -> String {
    [
        "计划协议工具。",
        "用途：提交计划文件路径，或标记该计划完成。",
        "参数：",
        "- action: present | complete",
        "- path: 计划 Markdown 文件路径",
    ]
    .join("\n")
}

fn plan_tool_should_auto_approve_present(conversation: Option<&Conversation>) -> bool {
    conversation
        .map(|value| value.shell_autonomous_mode || conversation_is_remote_im_contact(value))
        .unwrap_or(false)
}

pub(crate) fn builtin_plan(
    app_state: &AppState,
    session_id: &str,
    args: PlanToolArgs,
) -> Result<Value, String> {
    let action = args.action.trim().to_ascii_lowercase();
    if action != "present" && action != "complete" {
        return Err("plan.action 必须是 present 或 complete".to_string());
    }

    let conversation = terminal_session_conversation(app_state, session_id)?;
    let session_root = terminal_session_root_canonical(app_state, session_id)
        .or_else(|_| plan_assistant_space_canonical(app_state))?;
    let resolved_path = resolve_plan_file_for_conversation(&session_root, &args.path)?;
    let conversation_id = conversation
        .as_ref()
        .map(|value| value.id.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let auto_approved =
        action == "present" && plan_tool_should_auto_approve_present(conversation.as_ref());

    if action == "present" && auto_approved {
        if let Some(conversation_id) = conversation_id.as_deref() {
            let source_message_id = format!("auto-approved-plan-{}", Uuid::new_v4());
            if let Err(err) = message_store::active_plan_append_in_progress(
                &app_state.data_path,
                conversation_id,
                &source_message_id,
                &resolved_path.display_path,
            ) {
                runtime_log_warn(format!(
                    "[计划] 自动同意写入执行中计划失败 conversation_id={} session_id={} error={}",
                    conversation_id, session_id, err
                ));
            }
            if let Err(err) = set_conversation_plan_mode_enabled(app_state, conversation_id, false) {
                runtime_log_warn(format!(
                    "[计划] 自动同意关闭计划模式失败 conversation_id={} session_id={} error={}",
                    conversation_id, session_id, err
                ));
            }
        }
    }

    if action == "complete" {
        if let Some(conversation_id) = conversation_id.as_deref() {
            if let Err(err) = message_store::active_plan_complete_by_path(
                &app_state.data_path,
                conversation_id,
                &resolved_path.display_path,
                None,
            ) {
                runtime_log_warn(format!(
                    "[计划] 标记完成失败 conversation_id={} session_id={} error={}",
                    conversation_id, session_id, err
                ));
            }
        }
    }

    // 回执只说「这次调用之后计划处在什么状态」，不铺内部开关。
    // 保留的都是有消费者的：`action` / `path` / `should_stop_tool_loop` 被
    // `provider_tool_metadata_from_value` 读取驱动工具循环与计划验收，
    // `auto_approved` 决定回执该说「等确认」还是「已自动批准」。
    Ok(serde_json::json!({
        "action": action,
        "path": resolved_path.display_path,
        "should_stop_tool_loop": action == "present" && !auto_approved,
        "auto_approved": auto_approved,
    }))
}
