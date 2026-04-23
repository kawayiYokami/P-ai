#[tauri::command]
fn check_tools_status(
    input: CheckToolsStatusInput,
    state: State<'_, AppState>,
) -> Result<Vec<ToolLoadStatus>, String> {
    let mut config = state_read_config_cached(&state)?;
    normalize_api_tools(&mut config);
    let mut agents = with_app_data_cached_ref(&state, |data, _detail| Ok(data.agents.clone()))?;
    merge_private_organization_into_runtime(&state.data_path, &mut config, &mut agents)?;

    let target_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "缺少人格 ID".to_string())?;
    agents
        .iter()
        .find(|item| item.id == target_agent_id)
        .cloned()
        .ok_or_else(|| format!("未找到人格：{target_agent_id}"))?;
    let selected_tools = default_agent_tools();
    let current_department = department_for_agent_id(&config, &target_agent_id);

    let effective_api_id = department_for_agent_id(&config, &target_agent_id)
        .map(|item| item.api_config_id.clone())
        .or_else(|| input.api_config_id.clone());
    let selected = effective_api_id
        .as_deref()
        .and_then(|api_id| resolve_selected_api_config(&config, Some(api_id)));

    if selected.is_none() {
        return Ok(selected_tools
            .iter()
            .map(|tool| {
                let restricted_reason = tool_restricted_by_department(current_department, &tool.id);
                let forced_by_department = tool_forced_by_department(current_department, &tool.id);
                let detail = if let Some(reason) = restricted_reason.clone() {
                    reason
                } else if forced_by_department {
                    "远程客服部门已强制启用该工具。".to_string()
                } else if tool.id == "screenshot" {
                    "旧 screenshot 工具已并入 operate，请改用 operate。".to_string()
                } else if tool.enabled {
                    "系统默认已启用该工具，但当前尚未绑定部门运行模型。".to_string()
                } else {
                    "系统默认未启用该工具。".to_string()
                };
                ToolLoadStatus {
                    id: tool.id.clone(),
                    status: if restricted_reason.is_some() {
                        "unavailable".to_string()
                    } else if forced_by_department {
                        "loaded".to_string()
                    } else if tool.enabled {
                        if tool.id == "screenshot" { "unavailable".to_string() } else { "loaded".to_string() }
                    } else {
                        "disabled".to_string()
                    },
                    detail,
                }
            })
            .collect());
    }
    let selected = selected.ok_or_else(|| "No API config configured. Please add one.".to_string())?;

    if !selected.enable_tools {
        return Ok(selected_tools
            .iter()
            .map(|tool| ToolLoadStatus {
                id: tool.id.clone(),
                status: "disabled".to_string(),
                detail: "当前模型未启用工具调用。".to_string(),
            })
            .collect());
    }

    let runtime_shell = terminal_shell_for_state(&state);
    let mut statuses = Vec::new();
    for tool in selected_tools {
        let forced_by_department = tool_forced_by_department(current_department, &tool.id);
        if let Some(reason) = tool_restricted_by_department(current_department, &tool.id) {
            statuses.push(ToolLoadStatus {
                id: tool.id,
                status: "unavailable".to_string(),
                detail: reason,
            });
            continue;
        }
        if !tool.enabled && !forced_by_department {
            statuses.push(ToolLoadStatus {
                id: tool.id,
                status: "disabled".to_string(),
                detail: "系统默认未启用该工具。".to_string(),
            });
            continue;
        }
        let (status, detail) = match tool.id.as_str() {
            "fetch" => ("loaded".to_string(), "内置网页抓取工具可用".to_string()),
            "websearch" => ("loaded".to_string(), "内置网页搜索工具可用".to_string()),
            "remember" => ("loaded".to_string(), "记住工具可用".to_string()),
            "recall" => ("loaded".to_string(), "回忆工具可用".to_string()),
            "screenshot" => (
                "unavailable".to_string(),
                "旧 screenshot 工具已并入 operate，请改用 operate 脚本中的 screenshot 动作。".to_string(),
            ),
            "operate" => ("loaded".to_string(), "桌面输入工具可用（鼠标/键盘/文本）".to_string()),
            "reload" => ("loaded".to_string(), "刷新工作区 MCP 与技能工具可用".to_string()),
            "organize_context" => ("loaded".to_string(), "整理当前活跃对话上下文工具可用".to_string()),
            "wait" => ("loaded".to_string(), "等待毫秒工具可用".to_string()),
            "plan" => ("loaded".to_string(), "计划协议工具可用".to_string()),
            "task" => ("loaded".to_string(), "任务工具可用".to_string()),
            "todo" => ("loaded".to_string(), "会话内 Todo 步骤追踪工具可用".to_string()),
            "delegate" => ("loaded".to_string(), "委托工具可用".to_string()),
            "meme" => (
                "loaded".to_string(),
                "表情偷图工具可用".to_string(),
            ),
            "read_file" => (
                "loaded".to_string(),
                "本地文件读取工具可用（文本/图片/PDF/Office）".to_string(),
            ),
            "exec" => {
                #[cfg(target_os = "windows")]
                {
                    if runtime_shell.kind == "missing-terminal-shell" {
                        (
                            "unavailable".to_string(),
                            "未检测到可用终端。请先安装 Git 并使用 Git Bash： https://git-scm.com/downloads"
                                .to_string(),
                        )
                    } else {
                        (
                            "loaded".to_string(),
                            format!("执行工具可用（{}）", terminal_shell_runtime_label(&runtime_shell)),
                        )
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    (
                        "loaded".to_string(),
                        format!("执行工具可用（{}）", terminal_shell_runtime_label(&runtime_shell)),
                    )
                }
            }
            "apply_patch" => ("loaded".to_string(), "结构化补丁编辑工具可用".to_string()),
            other => ("failed".to_string(), format!("未支持的内置工具: {other}")),
        };
        statuses.push(ToolLoadStatus {
            id: tool.id,
            status,
            detail,
        });
    }
    Ok(statuses)
}

#[tauri::command]
fn get_image_text_cache_stats(state: State<'_, AppState>) -> Result<ImageTextCacheStats, String> {
    let runtime = state_read_runtime_state_cached(&state)?;

    let entries = runtime.image_text_cache.len();
    let total_chars = runtime
        .image_text_cache
        .iter()
        .map(|entry| entry.text.chars().count())
        .sum::<usize>();
    let latest_updated_at = runtime
        .image_text_cache
        .iter()
        .map(|entry| entry.updated_at.clone())
        .max();

    Ok(ImageTextCacheStats {
        entries,
        total_chars,
        latest_updated_at,
    })
}

#[tauri::command]
fn clear_image_text_cache(state: State<'_, AppState>) -> Result<ImageTextCacheStats, String> {
    let mut runtime = state_read_runtime_state_cached(&state)?;
    runtime.image_text_cache.clear();
    state_write_runtime_state_cached(&state, &runtime)?;

    Ok(ImageTextCacheStats {
        entries: 0,
        total_chars: 0,
        latest_updated_at: None,
    })
}

