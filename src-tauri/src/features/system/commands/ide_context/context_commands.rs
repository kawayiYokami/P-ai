fn upsert_ide_context_snapshot_internal(
    input: UpsertIdeContextSnapshotInput,
    runtime: &IdeContextRuntime,
) -> Result<(String, String), String> {
    let client_id = input.client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("clientId is required".to_string());
    }
    let updated_at = input
        .updated_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| ide_context_normalize_time_or_now("updatedAt", value))
        .unwrap_or_else(now_iso);
    let snapshot = IdeContextSnapshot {
        client_id: client_id.clone(),
        editor: {
            let editor = input.editor.trim();
            if editor.is_empty() {
                "vscode".to_string()
            } else {
                editor.to_string()
            }
        },
        workspace_roots: input
            .workspace_roots
            .into_iter()
            .map(|path| ide_context_display_path(&path))
            .filter(|path| !path.trim().is_empty())
            .collect(),
        references: input
            .references
            .into_iter()
            .filter_map(|reference| {
                let id = reference.id.trim().to_string();
                let file_path = ide_context_display_path(&reference.file_path);
                let content = reference.content.trim().to_string();
                let source = reference.source.trim().to_string();
                let allow_empty_content = source == "active_file";
                if id.is_empty() || file_path.is_empty() || (!allow_empty_content && content.is_empty()) {
                    return None;
                }
                Some(IdeContextReference {
                    id,
                    file_path,
                    start_line: reference.start_line,
                    end_line: reference.end_line,
                    content,
                    language_id: reference
                        .language_id
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty()),
                    source,
                    captured_at: ide_context_normalize_time_or_now(
                        "references[].capturedAt",
                        &reference.captured_at,
                    ),
                })
            })
            .collect(),
        updated_at: updated_at.clone(),
    };
    let mut snapshots = runtime
        .snapshots
        .lock()
        .map_err(|_| "Failed to lock ide context snapshots".to_string())?;
    snapshots.insert(client_id.clone(), snapshot);
    Ok((client_id, updated_at))
}

fn ide_chat_parse_workspace_params<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, String> {
    match ide_chat_parse_params::<T>(params.clone()) {
        Ok(value) => Ok(value),
        Err(_) => ide_chat_parse_param_field::<T>(params, "input"),
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatWorkspacePermissionInput {
    conversation_id: String,
    access: String,
    workspace_path: Option<String>,
    workspace_name: Option<String>,
}

fn ide_chat_workspace_permission_payload(state: &AppState, conversation: &Conversation) -> Result<Value, String> {
    let workspaces = terminal_allowed_workspaces_for_conversation_canonical(state, Some(conversation))?;
    let main = workspaces.iter().find(|w| w.level == SHELL_WORKSPACE_LEVEL_MAIN)
        .or_else(|| workspaces.iter().find(|w| w.level == SHELL_WORKSPACE_LEVEL_SYSTEM));
    Ok(serde_json::json!({
        "access": main.map(|w| w.access.trim()).filter(|v| !v.is_empty()).unwrap_or(SHELL_WORKSPACE_ACCESS_APPROVAL),
        "workspaceName": main.map(|w| w.name.clone()).unwrap_or_default(),
        "rootPath": main.map(|w| w.path.to_string_lossy().to_string()).unwrap_or_default(),
    }))
}

fn ide_chat_workspace_permission(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatConversationInput>(params)?;
    let meta = conversation_service_v2().get_conversation_meta(state, input.conversation_id.trim())?;
    ide_chat_workspace_permission_payload(state, &ide_chat_conversation_from_meta_view(&meta))
}

fn ide_chat_select_workspace_permission(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatWorkspacePermissionInput>(params)?;
    let conversation_id = input.conversation_id.trim();
    if conversation_id.is_empty() { return Err("conversationId is required".to_string()); }
    let access = match input.access.trim() {
        SHELL_WORKSPACE_ACCESS_READ_ONLY | SHELL_WORKSPACE_ACCESS_APPROVAL | SHELL_WORKSPACE_ACCESS_FULL_ACCESS => input.access.trim().to_string(),
        _ => return Err("Unsupported workspace access".to_string()),
    };
    let meta = conversation_service_v2().get_conversation_meta(state, conversation_id)?;
    let mut workspaces = meta.shell_workspaces.clone();
    let mut changed = false;
    for workspace in &mut workspaces { if normalize_shell_workspace_level_text(&workspace.level) == SHELL_WORKSPACE_LEVEL_MAIN { workspace.access = access.clone(); changed = true; } }
    if !changed {
        let path = input.workspace_path.as_deref().map(str::trim).unwrap_or_default();
        if path.is_empty() { return Err("当前会话没有主工作目录，无法设置权限。".to_string()); }
        let fallback = path.replace('\\', "/").trim_end_matches('/').rsplit('/').next().unwrap_or("VS Code").to_string();
        workspaces.push(ShellWorkspaceConfig { id: "vscode-sidebar-main-workspace".to_string(), name: input.workspace_name.as_deref().map(str::trim).filter(|v| !v.is_empty()).unwrap_or(fallback.as_str()).to_string(), path: path.to_string(), level: SHELL_WORKSPACE_LEVEL_MAIN.to_string(), access: access.clone(), built_in: false });
    }
    let updated = apply_conversation_chat_workspace_changes(state, conversation_id, Some(None), Some(normalize_conversation_shell_workspaces(state, &workspaces)), None, None, None)?;
    ide_chat_workspace_permission_payload(state, &updated)
}

fn ide_chat_workspace_layout_save(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_workspace_params::<SaveChatShellWorkspacesInput>(params)?;
    ide_chat_serialize(update_chat_shell_workspace_layout_inner(input, state)?)
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatWorkspaceDirectoryListInput { path: String }

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatFileReaderReadInput { path: String }

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatFileReaderReadBlockInput { path: String, start_line: usize, line_count: usize }

fn ide_chat_workspace_list(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_workspace_params::<ChatShellWorkspaceInput>(params)?;
    ide_chat_serialize(get_chat_shell_workspace_inner(input, state)?)
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdeChatWorkspaceGitRootCheckInput {
    workspace_path: String,
}

async fn ide_chat_workspace_git_root_check(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_workspace_params::<IdeChatWorkspaceGitRootCheckInput>(params)?;
    let result = check_git_workspace_root(ShellWorkspacePathInput {
        workspace_path: Some(input.workspace_path),
    })
    .await?;
    serde_json::to_value(result).map_err(|err| format!("serialize git root check failed: {err}"))
}

async fn ide_chat_workspace_directory_list(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatWorkspaceDirectoryListInput>(params)?;
    let path = input.path;
    let app_state = (*state).clone();
    let payload = tokio::task::spawn_blocking(move || list_file_reader_directory_inner_with_state(path, app_state))
        .await
        .map_err(|err| format!("读取目录任务失败：{err}"))??;
    Ok(serde_json::json!({"path": payload.path, "name": payload.name,
        "directories": payload.entries.into_iter().filter(|e| e.is_directory)
            .map(|e| serde_json::json!({"path": e.path, "name": e.name})).collect::<Vec<_>>() }))
}

async fn ide_chat_file_reader_directory_list(state: &AppState, params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatWorkspaceDirectoryListInput>(params)?;
    let path = input.path;
    let app_state = (*state).clone();
    let payload = tokio::task::spawn_blocking(move || list_file_reader_directory_inner_with_state(path, app_state))
        .await
        .map_err(|err| format!("读取目录任务失败：{err}"))??;
    serde_json::to_value(payload).map_err(|err| format!("serialize file reader directory failed: {err}"))
}

async fn ide_chat_file_reader_read(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatFileReaderReadInput>(params)?;
    let path = input.path;
    let payload = tokio::task::spawn_blocking(move || read_file_reader_file_inner(path, None))
        .await
        .map_err(|err| format!("读取文件任务失败：{err}"))??;
    serde_json::to_value(payload).map_err(|err| format!("serialize file reader payload failed: {err}"))
}

/// 下载用的原始字节读取：Web 宿主没有系统保存对话框，用它拿真实字节做浏览器下载。
/// 限制单文件大小，避免一次性把超大文件塞进桥接消息。
const FILE_DOWNLOAD_MAX_BYTES: u64 = 64 * 1024 * 1024;

async fn ide_chat_file_reader_read_raw(params: Value) -> Result<Value, String> {
    let path = params
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "path is required".to_string())?
        .to_string();
    tokio::task::spawn_blocking(move || {
        let file_path = std::path::PathBuf::from(&path);
        if !file_path.is_file() {
            return Err(format!("文件不存在：{path}"));
        }
        let metadata = fs::metadata(&file_path).map_err(|err| format!("读取文件信息失败：{err}"))?;
        if metadata.len() > FILE_DOWNLOAD_MAX_BYTES {
            return Err(format!(
                "文件过大（{} 字节），超过下载上限 {} 字节。",
                metadata.len(),
                FILE_DOWNLOAD_MAX_BYTES
            ));
        }
        let raw = fs::read(&file_path).map_err(|err| format!("读取文件失败：{err}"))?;
        let name = file_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("download")
            .to_string();
        Ok(serde_json::json!({
            "name": name,
            "bytesBase64": B64.encode(raw),
        }))
    })
    .await
    .map_err(|err| format!("读取文件任务异常：{err}"))?
}

async fn ide_chat_file_reader_read_block(params: Value) -> Result<Value, String> {
    let input = ide_chat_parse_params::<IdeChatFileReaderReadBlockInput>(params)?;
    let path = input.path;
    let start_line = input.start_line;
    let line_count = input.line_count;
    let payload = tokio::task::spawn_blocking(move || {
        read_file_reader_file_block_inner(path, start_line, line_count)
    })
    .await
    .map_err(|err| format!("读取文件块任务失败：{err}"))??;
    serde_json::to_value(payload).map_err(|err| format!("serialize file reader block failed: {err}"))
}

#[tauri::command]
fn upsert_ide_context_snapshot(
    input: UpsertIdeContextSnapshotInput,
    state: State<'_, AppState>,
    ide_context_runtime: State<'_, IdeContextRuntime>,
) -> Result<(), String> {
    let (client_id, updated_at) =
        upsert_ide_context_snapshot_internal(input, ide_context_runtime.inner())?;
    emit_ide_context_updated(&state, &client_id, &updated_at);
    Ok(())
}

#[tauri::command]
async fn query_ide_context_references(
    input: IdeContextWorkspaceQueryInput,
    ide_context_runtime: State<'_, IdeContextRuntime>,
) -> Result<IdeContextQueryResultOutput, String> {
    let runtime = ide_context_runtime.inner().clone();
    tokio::task::spawn_blocking(move || {
        query_ide_context_references_internal(input, &runtime)
    })
    .await
    .map_err(|err| format!("查询 IDE 上下文引用任务异常：{err}"))?
}

#[tauri::command]
async fn get_web_access_info(
    app: AppHandle,
    state: State<'_, AppState>,
    ide_context_runtime: State<'_, IdeContextRuntime>,
    input: Option<GetWebAccessInfoInput>,
) -> Result<WebAccessInfoOutput, String> {
    get_web_access_info_inner(
        &app,
        &state,
        &ide_context_runtime,
        input.unwrap_or_default().force_refresh,
    )
    .await
}

async fn get_web_access_info_inner(
    app: &AppHandle,
    state: &AppState,
    ide_context_runtime: &IdeContextRuntime,
    force_refresh: bool,
) -> Result<WebAccessInfoOutput, String> {
    let status_snapshot = ide_context_port_service_core()
        .status_snapshot(WEB_ACCESS_SERVICE_ID)
        .await;
    let config = state_read_config_cached(&state)?;
    let configured_port = normalize_web_access_port(config.web_access_port);
    if !config.web_access_enabled {
        return Ok(WebAccessInfoOutput {
            running: false,
            enabled: false,
            configured_port,
            port: configured_port,
            listen_addr: status_snapshot.listen_addr,
            status_text: status_snapshot.status_text,
            last_error: status_snapshot.last_error,
            local_url: String::new(),
            remote_urls: Vec::new(),
            remote_password: String::new(),
            active_connections: Vec::new(),
        });
    }
    if !IDE_CONTEXT_BRIDGE_STARTED.load(Ordering::SeqCst)
        && !ide_context_bridge_server_task_is_running()
    {
        start_web_access_server(
            app.clone(),
            state.clone(),
            ide_context_runtime.clone(),
        )
        .await;
    }
    let status_snapshot = ide_context_port_service_core()
        .status_snapshot(WEB_ACCESS_SERVICE_ID)
        .await;
    let running = IDE_CONTEXT_BRIDGE_STARTED.load(Ordering::SeqCst);
    let actual_port = ide_context_current_port(ide_context_runtime);
    let port = actual_port.unwrap_or(configured_port);
    let (local_url, remote_urls) = match actual_port {
        Some(actual_port) => (
            ide_context_sidebar_url_for_host(IDE_CONTEXT_BRIDGE_HOST, actual_port),
            ide_context_get_cached_lan_hosts(ide_context_runtime, force_refresh)?
                .into_iter()
                .map(|host| ide_context_sidebar_url_for_host(&host, actual_port))
                .collect::<Vec<_>>(),
        ),
        None => (String::new(), Vec::new()),
    };
    Ok(WebAccessInfoOutput {
        running,
        enabled: true,
        configured_port,
        port,
        listen_addr: status_snapshot.listen_addr,
        status_text: status_snapshot.status_text,
        last_error: status_snapshot.last_error,
        local_url,
        remote_urls,
        remote_password: ide_context_effective_remote_password(state, ide_context_runtime)?,
        active_connections: web_access_connection_summaries(),
    })
}

fn ide_context_get_cached_lan_hosts(
    ide_context_runtime: &IdeContextRuntime,
    force_refresh: bool,
) -> Result<Vec<String>, String> {
    let mut cache = ide_context_runtime
        .web_access_cache
        .lock()
        .map_err(|_| "Failed to lock web access cache".to_string())?;
    if !force_refresh {
        if let Some(lan_hosts) = cache.lan_hosts.clone() {
            return Ok(lan_hosts);
        }
    }
    let lan_hosts = ide_context_lan_hosts();
    cache.lan_hosts = Some(lan_hosts.clone());
    Ok(lan_hosts)
}

fn query_ide_context_references_internal(
    input: IdeContextWorkspaceQueryInput,
    ide_context_runtime: &IdeContextRuntime,
) -> Result<IdeContextQueryResultOutput, String> {
    let mut workspaces: Vec<IdeContextWorkspaceInput> = input
        .workspaces
        .into_iter()
        .filter(|workspace| !workspace.path.trim().is_empty())
        .collect();

    let mut snapshots = ide_context_runtime
        .snapshots
        .lock()
        .map_err(|_| "Failed to lock ide context snapshots".to_string())?;
    ide_context_prune_expired_snapshots(&mut snapshots);

    // Web 页面不会携带 VS Code 工作区；此时从仍有效的 VS Code 快照恢复工作区，
    // 以便展示 IDE 桥同步的当前打开文件。
    if workspaces.is_empty() {
        let mut workspace_paths = snapshots
            .values()
            .filter(|snapshot| snapshot.editor.eq_ignore_ascii_case("vscode"))
            .flat_map(|snapshot| snapshot.workspace_roots.iter())
            .map(|path| ide_context_display_path(path))
            .filter(|path| !path.trim().is_empty())
            .collect::<Vec<_>>();
        workspace_paths.sort_by(|left, right| {
            ide_context_compare_key(left).cmp(&ide_context_compare_key(right))
        });
        workspace_paths.dedup_by(|left, right| {
            ide_context_compare_key(left) == ide_context_compare_key(right)
        });
        workspaces = workspace_paths
            .into_iter()
            .map(|path| IdeContextWorkspaceInput { path, name: None })
            .collect();
    }
    if workspaces.is_empty() {
        return Ok(IdeContextQueryResultOutput {
            groups: Vec::new(),
            updated_at: String::new(),
        });
    }

    let mut groups = workspaces
        .iter()
        .map(|workspace| IdeContextWorkspaceGroupOutput {
            workspace_path: ide_context_display_path(&workspace.path),
            workspace_name: ide_context_workspace_name(workspace),
            references: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut latest_updated_at = String::new();

    for snapshot in snapshots.values() {
        if ide_context_timestamp_is_newer(&snapshot.updated_at, &latest_updated_at) {
            latest_updated_at = snapshot.updated_at.clone();
        }
        for reference in &snapshot.references {
            for group in &mut groups {
                if !ide_context_path_is_within_workspace(&reference.file_path, &group.workspace_path) {
                    continue;
                }
                let file_path = ide_context_display_path(&reference.file_path);
                let file_name = std::path::Path::new(&file_path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| file_path.clone());
                let relative_path = ide_context_relative_display_path(&file_path, &group.workspace_path);
                let display_label = format!(
                    "{}{}",
                    file_name,
                    ide_context_line_suffix(reference.start_line, reference.end_line)
                );
                let text_block = ide_context_text_block(&file_path, reference);
                group.references.push(IdeContextReferenceItemOutput {
                    id: format!("{}:{}:{}", snapshot.client_id, reference.id, reference.captured_at),
                    workspace_path: group.workspace_path.clone(),
                    workspace_name: group.workspace_name.clone(),
                    file_path,
                    file_name,
                    relative_path,
                    start_line: reference.start_line,
                    end_line: reference.end_line,
                    display_label,
                    content: reference.content.clone(),
                    language_id: reference.language_id.clone(),
                    source: reference.source.clone(),
                    captured_at: reference.captured_at.clone(),
                    text_block,
                });
                break;
            }
        }
    }

    for group in &mut groups {
        let mut latest_by_file = std::collections::HashMap::<String, IdeContextReferenceItemOutput>::new();
        for item in group.references.drain(..) {
            let key = ide_context_reference_dedup_key(&item);
            let should_replace = latest_by_file
                .get(&key)
                .map(|existing| ide_context_should_replace_reference(&item, existing))
                .unwrap_or(true);
            if should_replace {
                latest_by_file.insert(key, item);
            }
        }
        group.references = latest_by_file.into_values().collect();
        group.references.sort_by(|left, right| {
            ide_context_timestamp_compare_desc(&left.captured_at, &right.captured_at)
                .then_with(|| left.display_label.cmp(&right.display_label))
        });
    }
    groups.retain(|group| !group.references.is_empty());

    Ok(IdeContextQueryResultOutput {
        groups,
        updated_at: latest_updated_at,
    })
}

#[cfg(test)]
mod ide_context_query_tests {
    use super::*;

    fn upsert_test_snapshot(
        runtime: &IdeContextRuntime,
        client_id: &str,
        workspace_root: &str,
        file_path: &str,
    ) {
        let result = upsert_ide_context_snapshot_internal(
            UpsertIdeContextSnapshotInput {
                client_id: client_id.to_string(),
                auth_token: None,
                editor: "vscode".to_string(),
                workspace_roots: vec![workspace_root.to_string()],
                references: vec![IdeContextReferenceInput {
                    id: "active".to_string(),
                    file_path: file_path.to_string(),
                    start_line: Some(1),
                    end_line: Some(1),
                    content: "const value = 1;".to_string(),
                    language_id: Some("typescript".to_string()),
                    source: "active_file".to_string(),
                    captured_at: now_iso(),
                }],
                updated_at: Some(now_iso()),
            },
            runtime,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn empty_workspace_query_uses_active_vscode_snapshot_roots() {
        let runtime = IdeContextRuntime::new();
        upsert_test_snapshot(
            &runtime,
            "vscode-client",
            "E:/repo",
            "E:/repo/src/main.ts",
        );

        let result = query_ide_context_references_internal(
            IdeContextWorkspaceQueryInput { workspaces: Vec::new() },
            &runtime,
        );

        assert!(result.is_ok());
        let result = match result {
            Ok(value) => value,
            Err(error) => panic!("query failed: {error}"),
        };
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].workspace_path, "E:/repo");
        assert_eq!(result.groups[0].references.len(), 1);
        assert_eq!(result.groups[0].references[0].file_path, "E:/repo/src/main.ts");
    }

    #[test]
    fn explicit_workspace_query_stays_scoped_to_requested_workspace() {
        let runtime = IdeContextRuntime::new();
        upsert_test_snapshot(
            &runtime,
            "vscode-client-a",
            "E:/repo-a",
            "E:/repo-a/src/a.ts",
        );
        upsert_test_snapshot(
            &runtime,
            "vscode-client-b",
            "E:/repo-b",
            "E:/repo-b/src/b.ts",
        );

        let result = query_ide_context_references_internal(
            IdeContextWorkspaceQueryInput {
                workspaces: vec![IdeContextWorkspaceInput {
                    path: "E:/repo-b".to_string(),
                    name: Some("repo-b".to_string()),
                }],
            },
            &runtime,
        );

        assert!(result.is_ok());
        let result = match result {
            Ok(value) => value,
            Err(error) => panic!("query failed: {error}"),
        };
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].references.len(), 1);
        assert_eq!(result.groups[0].references[0].file_path, "E:/repo-b/src/b.ts");
    }
}
