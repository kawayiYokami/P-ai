fn ide_context_chat_clients() -> Arc<Mutex<std::collections::HashMap<String, tokio::sync::mpsc::UnboundedSender<serde_json::Value>>>> {
    IDE_CONTEXT_CHAT_CLIENTS
        .get_or_init(|| Arc::new(Mutex::new(std::collections::HashMap::new())))
        .clone()
}

/// 只读快照：广播前记录当前有哪些 Web/VS Code 客户端，用于排障定位投递范围。
fn ide_context_chat_client_ids() -> Vec<String> {
    let clients = ide_context_chat_clients();
    let ids = match clients.lock() {
        Ok(guard) => guard.keys().cloned().collect::<Vec<String>>(),
        Err(_) => Vec::new(),
    };
    ids
}

fn ide_context_chat_client_conversations() -> Arc<Mutex<std::collections::HashMap<String, String>>> {
    IDE_CONTEXT_CHAT_CLIENT_CONVERSATIONS
        .get_or_init(|| Arc::new(Mutex::new(std::collections::HashMap::new())))
        .clone()
}

fn web_access_connections() -> Arc<Mutex<std::collections::HashMap<String, WebAccessConnectionEntry>>> {
    WEB_ACCESS_CONNECTIONS
        .get_or_init(|| Arc::new(Mutex::new(std::collections::HashMap::new())))
        .clone()
}

fn ide_context_web_access_enabled(state: &AppState) -> bool {
    match state_read_config_cached(state) {
        Ok(config) => config.web_access_enabled,
        Err(err) => {
            runtime_log_error(format!("[网络访问] 读取配置失败，按关闭处理: {}", err));
            false
        }
    }
}

fn ide_context_bridge_shutdown_notification(reason: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "bridge.shutdown",
        "params": {
            "reason": reason,
        },
    })
}

fn ide_context_notify_chat_clients_shutdown(reason: &str) {
    let notification = ide_context_bridge_shutdown_notification(reason);
    if let Ok(clients) = ide_context_chat_clients().lock() {
        for sender in clients.values() {
            let _ = sender.send(notification.clone());
        }
    }
}

fn web_access_register_connection(
    path: &str,
    peer_addr: &std::net::SocketAddr,
    local: bool,
    authenticated: bool,
    client_id: &str,
) -> String {
    let id = Uuid::new_v4().to_string();
    let entry = WebAccessConnectionEntry {
        id: id.clone(),
        path: path.trim().to_string(),
        peer_addr: peer_addr.to_string(),
        local,
        authenticated,
        connected_at: now_iso(),
        client_id: client_id.trim().to_string(),
    };
    if let Ok(mut connections) = web_access_connections().lock() {
        connections.insert(id.clone(), entry);
    }
    id
}

fn web_access_update_connection_auth(connection_id: &str, authenticated: bool, client_id: Option<&str>) {
    if let Ok(mut connections) = web_access_connections().lock() {
        if let Some(entry) = connections.get_mut(connection_id) {
            entry.authenticated = authenticated;
            if let Some(client_id) = client_id.map(str::trim).filter(|value| !value.is_empty()) {
                entry.client_id = client_id.to_string();
            }
        }
    }
}

fn web_access_remove_connection(connection_id: &str) {
    if let Ok(mut connections) = web_access_connections().lock() {
        connections.remove(connection_id);
    }
}

fn web_access_connection_summaries() -> Vec<WebAccessConnectionSummary> {
    let mut items = if let Ok(connections) = web_access_connections().lock() {
        connections
            .values()
            .cloned()
            .map(|entry| WebAccessConnectionSummary {
                id: entry.id,
                path: entry.path,
                peer_addr: entry.peer_addr,
                local: entry.local,
                authenticated: entry.authenticated,
                connected_at: entry.connected_at,
                client_id: entry.client_id,
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    items.sort_by(|left, right| right.connected_at.cmp(&left.connected_at));
    items
}

fn ide_chat_broadcast_notification(method: &str, params: serde_json::Value) {
    let clients = ide_context_chat_clients();
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    });
    let mut stale_ids = Vec::<String>::new();
    if let Ok(clients_guard) = clients.lock() {
        for (client_id, sender) in clients_guard.iter() {
            if sender.send(message.clone()).is_err() {
                stale_ids.push(client_id.clone());
            }
        }
    }
    if !stale_ids.is_empty() {
        if let Ok(mut clients_guard) = clients.lock() {
            for client_id in stale_ids {
                clients_guard.remove(&client_id);
            }
        }
    }
}

fn ide_chat_emit_notification_to_client(
    client_id: &str,
    method: &str,
    params: serde_json::Value,
) -> bool {
    let normalized_client_id = client_id.trim();
    if normalized_client_id.is_empty() {
        return false;
    }
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    });
    let clients = ide_context_chat_clients();
    let mut stale = false;
    let delivered = if let Ok(clients_guard) = clients.lock() {
        match clients_guard.get(normalized_client_id) {
            Some(sender) => {
                if sender.send(message).is_ok() {
                    true
                } else {
                    stale = true;
                    false
                }
            }
            None => false,
        }
    } else {
        false
    };
    if stale {
        if let Ok(mut clients_guard) = clients.lock() {
            clients_guard.remove(normalized_client_id);
        }
        if let Ok(mut conversations) = ide_context_chat_client_conversations().lock() {
            conversations.remove(normalized_client_id);
        }
    }
    delivered
}

fn ide_chat_sidebar_client_id_from_label(label: &str) -> Option<String> {
    let label = label.trim();
    if let Some(value) = label.strip_prefix("vscode-sidebar:") {
        let client_id = value.trim();
        if !client_id.is_empty() {
            return Some(client_id.to_string());
        }
    }
    if let Some(value) = label.strip_prefix("ide-chat-sidebar-") {
        let client_id = value.trim();
        if !client_id.is_empty() {
            return Some(client_id.to_string());
        }
    }
    None
}

fn ide_chat_emit_notification_to_sidebar_conversation(
    conversation_id: &str,
    method: &str,
    params: serde_json::Value,
) -> usize {
    let cid = conversation_id.trim();
    if cid.is_empty() {
        return 0;
    }
    let client_ids = ide_context_chat_client_conversations()
        .lock()
        .ok()
        .map(|conversations| {
            conversations
                .iter()
                .filter_map(|(client_id, mapped_conversation_id)| {
                    if mapped_conversation_id.trim() == cid {
                        Some(client_id.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if client_ids.is_empty() {
        return 0;
    }
    let clients = ide_context_chat_clients();
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    });
    let mut delivered = 0usize;
    let mut stale_ids = Vec::<String>::new();
    if let Ok(clients_guard) = clients.lock() {
        for client_id in &client_ids {
            match clients_guard.get(client_id) {
                Some(sender) if sender.send(message.clone()).is_ok() => {
                    delivered += 1;
                }
                Some(_) => stale_ids.push(client_id.clone()),
                None => stale_ids.push(client_id.clone()),
            }
        }
    }
    if !stale_ids.is_empty() {
        if let Ok(mut clients_guard) = clients.lock() {
            for client_id in &stale_ids {
                clients_guard.remove(client_id);
            }
        }
        if let Ok(mut conversations) = ide_context_chat_client_conversations().lock() {
            for client_id in stale_ids {
                conversations.remove(&client_id);
            }
        }
    }
    delivered
}

fn ide_context_prune_expired_bridge_tokens(auth: &mut IdeContextBridgeAuthRuntime, now: OffsetDateTime) {
    auth.valid_tokens.retain(|_, expires_at| *expires_at > now);
    if auth.valid_tokens.len() <= IDE_CONTEXT_MAX_AUTH_TOKENS {
        return;
    }
    let mut tokens = auth
        .valid_tokens
        .iter()
        .map(|(token, expires_at)| (token.clone(), *expires_at))
        .collect::<Vec<_>>();
    tokens.sort_by(|left, right| right.1.cmp(&left.1));
    auth.valid_tokens = tokens
        .into_iter()
        .take(IDE_CONTEXT_MAX_AUTH_TOKENS)
        .collect();
}

fn ide_context_bridge_token_store_path(state: &AppState) -> PathBuf {
    app_root_from_data_path(&state.data_path)
        .join("web-access")
        .join("bridge-auth-token.json")
}

fn ide_context_clear_persisted_bridge_token(state: &AppState) -> Result<(), String> {
    let path = ide_context_bridge_token_store_path(state);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(&path)
        .map_err(|err| format!("删除 Web 访问令牌失败，path={}，error={err}", path.display()))
}

fn ide_context_persist_bridge_tokens(
    state: &AppState,
    tokens: &std::collections::HashMap<String, OffsetDateTime>,
) -> Result<(), String> {
    let path = ide_context_bridge_token_store_path(state);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建 Web 访问令牌目录失败，path={}，error={err}", parent.display()))?;
    }
    let mut entries = tokens
        .iter()
        .filter_map(|(token, expires_at)| {
            let normalized_token = token.trim();
            if normalized_token.is_empty() {
                return None;
            }
            Some(IdeContextPersistedBridgeTokenEntry {
                token: normalized_token.to_string(),
                expires_at: expires_at.format(&Rfc3339).ok()?,
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| right.expires_at.cmp(&left.expires_at));
    let payload = IdeContextPersistedBridgeToken {
        tokens: entries,
        ..Default::default()
    };
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|err| format!("序列化 Web 访问令牌失败: {err}"))?;
    fs::write(&path, text)
        .map_err(|err| format!("写入 Web 访问令牌失败，path={}，error={err}", path.display()))
}

fn ide_context_try_restore_persisted_bridge_token(
    state: &AppState,
    runtime: &IdeContextRuntime,
) -> Result<(), String> {
    let path = ide_context_bridge_token_store_path(state);
    if !path.exists() {
        return Ok(());
    }
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("读取 Web 访问令牌失败，path={}，error={err}", path.display()))?;
    let payload: IdeContextPersistedBridgeToken = serde_json::from_str(&text)
        .map_err(|err| format!("解析 Web 访问令牌失败，path={}，error={err}", path.display()))?;
    let now = now_utc();
    let mut restored = std::collections::HashMap::new();
    for entry in payload.normalized_entries() {
        let token = entry.token.trim().to_string();
        if token.is_empty() {
            continue;
        }
        let Some(expires_at) = parse_iso(&entry.expires_at) else {
            continue;
        };
        if expires_at > now {
            restored.insert(token, expires_at);
        }
    }
    if restored.is_empty() {
        let _ = ide_context_clear_persisted_bridge_token(state);
        return Ok(());
    }
    let mut auth = runtime
        .bridge_auth
        .lock()
        .map_err(|_| "Failed to lock ide context bridge auth".to_string())?;
    auth.valid_tokens.clear();
    auth.valid_tokens.extend(restored);
    ide_context_prune_expired_bridge_tokens(&mut auth, now);
    Ok(())
}

fn ide_context_store_bridge_token(
    runtime: &IdeContextRuntime,
    state: Option<&AppState>,
    token: &str,
    expires_at: OffsetDateTime,
) -> Result<(), String> {
    let normalized_token = token.trim().to_string();
    if normalized_token.is_empty() {
        return Err("Web 访问令牌为空，无法保存".to_string());
    }
    {
        let mut auth = runtime
            .bridge_auth
            .lock()
            .map_err(|_| "Failed to lock ide context bridge auth".to_string())?;
        auth.valid_tokens.insert(normalized_token.clone(), expires_at);
        ide_context_prune_expired_bridge_tokens(&mut auth, now_utc());
        if let Some(state) = state {
            ide_context_persist_bridge_tokens(state, &auth.valid_tokens)?;
        }
    }
    Ok(())
}

fn ide_context_issue_bridge_token_with_state(
    runtime: &IdeContextRuntime,
    state: Option<&AppState>,
) -> Result<String, String> {
    let token = ide_context_generate_bridge_token();
    let now = now_utc();
    let expires_at = now + time::Duration::seconds(IDE_CONTEXT_AUTH_TOKEN_TTL_SECS);
    ide_context_store_bridge_token(runtime, state, &token, expires_at)?;
    Ok(token)
}

fn ide_context_consume_bridge_token_with_state(
    runtime: &IdeContextRuntime,
    state: Option<&AppState>,
    provided: &str,
) -> Result<String, (String, Option<String>)> {
    let provided = provided.trim();
    if provided.is_empty() {
        return Err(("authToken is required".to_string(), None));
    }
    if let Some(state) = state {
        let should_restore = runtime
            .bridge_auth
            .lock()
            .map(|auth| auth.valid_tokens.is_empty())
            .unwrap_or(false);
        if should_restore {
            if let Err(err) = ide_context_try_restore_persisted_bridge_token(state, runtime) {
                runtime_log_error(format!("[IDE 上下文桥] 恢复持久化 Web 访问令牌失败: {}", err));
            }
        }
    }
    let mut auth = runtime
        .bridge_auth
        .lock()
        .map_err(|_| ("Failed to lock ide context bridge auth".to_string(), None))?;
    let now = now_utc();
    ide_context_prune_expired_bridge_tokens(&mut auth, now);
    if auth.valid_tokens.is_empty() {
        drop(auth);
        if let Some(state) = state {
            let _ = ide_context_clear_persisted_bridge_token(state);
        }
        let refreshed_token = ide_context_issue_bridge_token_with_state(runtime, state)
            .map_err(|err| (err, None))?;
        return Err((
            "IDE context bridge token expired, discovery refreshed".to_string(),
            Some(refreshed_token),
        ));
    }
    if !auth.valid_tokens.contains_key(provided) {
        return Err(("invalid authToken".to_string(), None));
    }
    let expires_at = now + time::Duration::seconds(IDE_CONTEXT_AUTH_TOKEN_TTL_SECS);
    drop(auth);
    ide_context_store_bridge_token(runtime, state, provided, expires_at)
        .map_err(|err| (err, None))?;
    Ok(provided.to_string())
}
