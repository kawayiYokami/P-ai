#[tauri::command]
fn check_tools_status(
    input: CheckToolsStatusInput,
    state: State<'_, AppState>,
) -> Result<Vec<ToolLoadStatus>, String> {
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut config = state_read_config_cached(&state)?;
    normalize_api_tools(&mut config);
    let mut data = state_read_app_data_cached(&state)?;
    ensure_default_agent(&mut data);
    merge_private_organization_into_runtime_data(&state.data_path, &mut config, &mut data)?;
    drop(guard);

    let target_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "缺少人格 ID".to_string())?;
    let selected_agent = data
        .agents
        .iter()
        .find(|item| item.id == target_agent_id)
        .cloned()
        .ok_or_else(|| format!("未找到人格：{target_agent_id}"))?;
    let mut selected_tools = selected_agent.tools.clone();
    if !selected_tools.iter().any(|tool| tool.id == "command") {
        selected_tools.push(ApiToolConfig {
            id: "command".to_string(),
            command: "builtin".to_string(),
            args: vec!["command".to_string()],
            enabled: true,
            values: serde_json::json!({}),
        });
    }
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
                let detail = if let Some(reason) = restricted_reason.clone() {
                    reason
                } else if tool.id == "screenshot" {
                    "当前人格尚未委任部门，需绑定支持图像的部门模型后才能运行。".to_string()
                } else if tool.enabled {
                    "当前人格已启用该工具，但尚未委任部门，暂不校验运行模型。".to_string()
                } else {
                    "当前人格未启用该工具。".to_string()
                };
                ToolLoadStatus {
                    id: tool.id.clone(),
                    status: if restricted_reason.is_some() {
                        "unavailable".to_string()
                    } else if tool.enabled {
                        if tool.id == "screenshot" {
                            "unavailable".to_string()
                        } else {
                            "loaded".to_string()
                        }
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
        if let Some(reason) = tool_restricted_by_department(current_department, &tool.id) {
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
                detail: "当前人格未启用该工具。".to_string(),
            });
            continue;
        }
        if tool.id == "screenshot" && !selected.enable_image {
            statuses.push(ToolLoadStatus {
                id: tool.id,
                status: "unavailable".to_string(),
                detail: "已启用，但当前模型不支持图像，运行时将跳过。".to_string(),
            });
            continue;
        }
        let (status, detail) = match tool.id.as_str() {
            "fetch" => ("loaded".to_string(), "内置网页抓取工具可用".to_string()),
            "websearch" => ("loaded".to_string(), "内置网页搜索工具可用".to_string()),
            "remember" => ("loaded".to_string(), "记住工具可用".to_string()),
            "recall" => ("loaded".to_string(), "回忆工具可用".to_string()),
            "screenshot" => ("loaded".to_string(), "截图工具可用".to_string()),
            "command" => ("loaded".to_string(), "统一命令工具可用".to_string()),
            "task" => ("loaded".to_string(), "任务工具可用".to_string()),
            "delegate" => ("loaded".to_string(), "委托工具可用".to_string()),
            "remote_im_send" => (
                "loaded".to_string(),
                "远程联系人通讯工具可用（支持 list/send）".to_string(),
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
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let data = state_read_app_data_cached(&state)?;
    drop(guard);

    let entries = data.image_text_cache.len();
    let total_chars = data
        .image_text_cache
        .iter()
        .map(|entry| entry.text.chars().count())
        .sum::<usize>();
    let latest_updated_at = data
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
    let guard = state
        .state_lock
        .lock()
        .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
    let mut data = state_read_app_data_cached(&state)?;
    data.image_text_cache.clear();
    state_write_app_data_cached(&state, &data)?;
    drop(guard);

    Ok(ImageTextCacheStats {
        entries: 0,
        total_chars: 0,
        latest_updated_at: None,
    })
}
