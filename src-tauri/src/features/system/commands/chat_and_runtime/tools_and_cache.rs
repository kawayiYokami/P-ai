#[tauri::command]
fn check_tools_status(
    input: CheckToolsStatusInput,
    state: State<'_, AppState>,
) -> Result<Vec<ToolLoadStatus>, String> {
    check_tools_status_inner(input, &state)
}

fn check_tools_status_inner(
    input: CheckToolsStatusInput,
    state: &AppState,
) -> Result<Vec<ToolLoadStatus>, String> {
    let runtime_org = load_runtime_organization_snapshot(&state)?;
    let mut config = runtime_org.config.clone();
    normalize_api_tools(&mut config);
    let agents = runtime_org.agents.clone();

    let target_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "缺少人格 ID".to_string())?;
    let target_agent = agents
        .iter()
        .find(|item| item.id == target_agent_id)
        .cloned()
        .ok_or_else(|| format!("未找到人格：{target_agent_id}"))?;
    let selected_tools = default_agent_tools();
    let delegate_unavailable_reason =
        agent_builtin_tool_unavailable_reason(Some(&target_agent), "delegate");

    let effective_api_id = (!target_agent.api_config_id.trim().is_empty())
        .then(|| target_agent.api_config_id.trim().to_string())
        .or_else(|| input.api_config_id.clone());
    let selected = effective_api_id
        .as_deref()
        .and_then(|api_id| resolve_selected_api_config(&config, Some(api_id)));

    if selected.is_none() {
        return Ok(selected_tools
            .iter()
            .map(|tool| {
                let restricted_reason = if tool.id == "delegate" {
                    delegate_unavailable_reason.clone()
                } else {
                    agent_builtin_tool_unavailable_reason(Some(&target_agent), &tool.id)
                };
                let detail = if let Some(reason) = restricted_reason.clone() {
                    reason
                } else if tool.enabled {
                    "系统默认已启用该工具，但当前尚未绑定人格运行模型。".to_string()
                } else {
                    "系统默认未启用该工具。".to_string()
                };
                ToolLoadStatus {
                    id: tool.id.clone(),
                    status: if restricted_reason.is_some() {
                        "unavailable".to_string()
                    } else if tool.enabled {
                        "loaded".to_string()
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
        let restricted_reason = if tool.id == "delegate" {
            delegate_unavailable_reason.clone()
        } else {
            agent_builtin_tool_unavailable_reason(Some(&target_agent), &tool.id)
        };
        if let Some(reason) = restricted_reason {
            statuses.push(ToolLoadStatus {
                id: tool.id,
                status: "unavailable".to_string(),
                detail: reason,
            });
            continue;
        }
        if !tool.enabled {
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
            "operate" => ("loaded".to_string(), "桌面输入工具可用（鼠标/键盘/文本）".to_string()),
            "wait" => (
                "unavailable".to_string(),
                "wait 工具已删除；请改用 operate 脚本中的 wait 动作。".to_string(),
            ),
            "plan" => ("loaded".to_string(), "计划协议工具可用".to_string()),
            "task" => ("loaded".to_string(), "任务工具可用".to_string()),
            "todo" => ("loaded".to_string(), "会话内 Todo 步骤追踪工具可用".to_string()),
            "delegate" => ("loaded".to_string(), "委托工具可用".to_string()),
            "meme" => (
                "loaded".to_string(),
                "表情偷图工具可用".to_string(),
            ),
            "read" | "read_file" => (
                "loaded".to_string(),
                "本地文件读取工具可用（文本/PDF/Office）".to_string(),
            ),
            "read_media" => (
                "loaded".to_string(),
                "本地多媒体解析工具可用（图片/音频/视频）".to_string(),
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
            "write" => ("loaded".to_string(), "完整文件写入工具可用".to_string()),
            "delete" => ("loaded".to_string(), "整文件删除工具可用".to_string()),
            "update" => ("loaded".to_string(), "局部内容更新工具可用".to_string()),
            "move" => ("loaded".to_string(), "文件移动与重命名工具可用".to_string()),
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
    get_image_text_cache_stats_inner(&state)
}

fn get_image_text_cache_stats_inner(state: &AppState) -> Result<ImageTextCacheStats, String> {
    let entries = state_service_list_image_text_cache(state)?;
    let entry_count = entries.len();
    let total_chars = entries
        .iter()
        .map(|(text, _)| text.chars().count())
        .sum::<usize>();
    let latest_updated_at = entries.into_iter().map(|(_, updated_at)| updated_at).max();

    Ok(ImageTextCacheStats {
        entries: entry_count,
        total_chars,
        latest_updated_at,
    })
}

#[tauri::command]
fn clear_image_text_cache(state: State<'_, AppState>) -> Result<ImageTextCacheStats, String> {
    clear_image_text_cache_inner(&state)
}

fn clear_image_text_cache_inner(state: &AppState) -> Result<ImageTextCacheStats, String> {
    state_service_clear_image_text_cache(state)?;

    Ok(ImageTextCacheStats {
        entries: 0,
        total_chars: 0,
        latest_updated_at: None,
    })
}
