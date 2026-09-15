fn remote_im_list_channels_inner(state: &AppState) -> Result<Vec<RemoteImChannelConfig>, String> {
    let config = state_read_config_cached(state)?;
    Ok(config.remote_im_channels)
}

#[tauri::command]
fn remote_im_list_channels(state: State<'_, AppState>) -> Result<Vec<RemoteImChannelConfig>, String> {
    remote_im_list_channels_inner(state.inner())
}

fn remote_im_list_contacts_inner(state: &AppState) -> Result<Vec<RemoteImContact>, String> {
    let mut contacts = state_service_list_remote_im_contacts(state, None)?;
    contacts.sort_by(|a, b| {
        a.channel_id
            .cmp(&b.channel_id)
            .then_with(|| b.last_message_at.cmp(&a.last_message_at))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(contacts)
}

fn remote_im_mutate_contact<T>(
    state: &AppState,
    contact_id: &str,
    mutate: impl FnOnce(&mut RemoteImContact) -> Result<T, String>,
) -> Result<T, String> {
    let mut contact = state_service_get_remote_im_contact(state, contact_id)?
        .ok_or_else(|| format!("未找到远程联系人：{contact_id}"))?;
    let output = mutate(&mut contact)?;
    state_service_upsert_remote_im_contact(state, &contact)?;
    Ok(output)
}

fn remote_im_get_contact_by_id(
    state: &AppState,
    contact_id: &str,
) -> Result<RemoteImContact, String> {
    state_service_get_remote_im_contact(state, contact_id)?
        .ok_or_else(|| format!("未找到远程联系人：{contact_id}"))
}

// ========== 远程入站瞬时去重 ==========

const REMOTE_IM_INBOUND_RECENT_PLATFORM_MESSAGE_ID_LIMIT: usize = 10;

static REMOTE_IM_INBOUND_RECENT_PLATFORM_MESSAGE_IDS: OnceLock<
    Mutex<std::collections::HashMap<String, std::collections::VecDeque<String>>>,
> = OnceLock::new();

fn remote_im_inbound_recent_platform_message_ids(
) -> &'static Mutex<std::collections::HashMap<String, std::collections::VecDeque<String>>> {
    REMOTE_IM_INBOUND_RECENT_PLATFORM_MESSAGE_IDS
        .get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn remote_im_remember_inbound_platform_message_id(
    channel_id: &str,
    platform_message_id: Option<&str>,
) -> Result<bool, String> {
    let channel_id = channel_id.trim();
    let platform_message_id = platform_message_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(platform_message_id) = platform_message_id else {
        return Ok(false);
    };
    if channel_id.is_empty() {
        return Ok(false);
    }
    let mut store = remote_im_inbound_recent_platform_message_ids()
        .lock()
        .map_err(|_| "远程入站去重表不可用".to_string())?;
    let recent_ids = store
        .entry(channel_id.to_string())
        .or_insert_with(std::collections::VecDeque::new);
    if recent_ids.iter().any(|item| item == platform_message_id) {
        return Ok(true);
    }
    recent_ids.push_back(platform_message_id.to_string());
    while recent_ids.len() > REMOTE_IM_INBOUND_RECENT_PLATFORM_MESSAGE_ID_LIMIT {
        recent_ids.pop_front();
    }
    Ok(false)
}

// ========== 远程会话能量仪表盘 ==========

const REMOTE_IM_CONTACT_DASHBOARD_UPDATED_EVENT: &str =
    "easy-call:remote-im-contact-dashboard-updated";
const REMOTE_IM_CONTACT_DASHBOARD_PUSH_INTERVAL_SECONDS: u64 = 3;

#[derive(Default)]
struct RemoteImContactDashboardSubscriptions {
    contact_ids_by_window: std::collections::HashMap<String, String>,
    push_worker_running: bool,
}

static REMOTE_IM_CONTACT_DASHBOARD_SUBSCRIPTIONS: OnceLock<
    Mutex<RemoteImContactDashboardSubscriptions>,
> = OnceLock::new();

fn remote_im_contact_dashboard_subscriptions(
) -> &'static Mutex<RemoteImContactDashboardSubscriptions> {
    REMOTE_IM_CONTACT_DASHBOARD_SUBSCRIPTIONS
        .get_or_init(|| Mutex::new(RemoteImContactDashboardSubscriptions::default()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactDashboardSnapshot {
    contact_id: String,
    energy: f64,
    maximum_energy: f64,
    energy_percent: f64,
    energy_recovery_per_second: f64,
    presence: String,
    last_presence_at: Option<String>,
    watermark: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactDashboardInput {
    contact_id: String,
    #[serde(default)]
    known_watermark: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactDashboardSyncResult {
    snapshot: RemoteImContactDashboardSnapshot,
    changed: bool,
}

fn remote_im_contact_dashboard_presence_label(state: RemoteImPresenceState) -> &'static str {
    match state {
        RemoteImPresenceState::Away => "away",
        RemoteImPresenceState::Present => "present",
    }
}

fn remote_im_contact_dashboard_snapshot_inner(
    state: &AppState,
    contact_id: &str,
) -> Result<RemoteImContactDashboardSnapshot, String> {
    let contact_id = contact_id.trim();
    if contact_id.is_empty() {
        return Err("远程联系人仪表盘读取失败：联系人ID为空".to_string());
    }
    let contact = state_service_get_remote_im_contact(state, contact_id)?
        .ok_or_else(|| format!("远程联系人仪表盘读取失败：未找到联系人：{contact_id}"))?;
    let checkpoint = state_service_get_remote_im_contact_checkpoint(state, contact_id)?;
    let pacing = effective_remote_im_group_reply_pacing(state, &contact);
    let energy = remote_im_group_energy_at(checkpoint.as_ref(), &pacing, now_utc());
    let presence_runtime = lock_remote_im_contact_runtime_states(state)?
        .get(contact_id)
        .cloned()
        .unwrap_or_default();
    let presence = remote_im_contact_dashboard_presence_label(presence_runtime.presence_state);
    let checkpoint_revision = checkpoint.map(|item| item.atomic_revision).unwrap_or_default();
    let last_presence_at = presence_runtime.last_presence_at.clone();
    let watermark = format!(
        "checkpoint:{checkpoint_revision}|presence:{presence}|at:{}|work:{:?}|pending:{}",
        last_presence_at.as_deref().unwrap_or(""),
        presence_runtime.work_state,
        presence_runtime.has_pending,
    );
    let maximum_energy = pacing.maximum_energy;
    let energy_percent = if maximum_energy > 0.0 {
        energy / maximum_energy * 100.0
    } else {
        0.0
    };
    Ok(RemoteImContactDashboardSnapshot {
        contact_id: contact_id.to_string(),
        energy,
        maximum_energy,
        energy_percent,
        energy_recovery_per_second: pacing.energy_recovery_per_second,
        presence: presence.to_string(),
        last_presence_at,
        watermark,
        updated_at: now_iso(),
    })
}

fn remote_im_emit_contact_dashboard_snapshot(state: &AppState, contact_id: &str) {
    let (has_subscription, web_client_ids) = remote_im_contact_dashboard_subscriptions()
        .lock()
        .map(|subscriptions| {
            let mut web_client_ids = Vec::new();
            for (window_label, subscribed_contact_id) in &subscriptions.contact_ids_by_window {
                if subscribed_contact_id != contact_id {
                    continue;
                }
                if let Some(client_id) = window_label.strip_prefix("web:") {
                    if !client_id.trim().is_empty() {
                        web_client_ids.push(client_id.trim().to_string());
                    }
                }
            }
            (
                subscriptions
                    .contact_ids_by_window
                    .values()
                    .any(|item| item == contact_id),
                web_client_ids,
            )
        })
        .unwrap_or((false, Vec::new()));
    if !has_subscription {
        return;
    }
    let snapshot = match remote_im_contact_dashboard_snapshot_inner(state, contact_id) {
        Ok(snapshot) => snapshot,
        Err(err) => {
            runtime_log_debug(format!(
                "[远程会话仪表盘] 跳过，任务=构建推送快照，contact_id={}，error={}",
                contact_id, err
            ));
            return;
        }
    };
    for client_id in web_client_ids {
        let _ = ide_chat_emit_notification_to_client(
            &client_id,
            "remoteIm.dashboard.updated",
            serde_json::json!(snapshot),
        );
    }
    if let Ok(guard) = state.app_handle.lock() {
        if let Some(app_handle) = guard.as_ref() {
            if let Err(err) = app_handle.emit(REMOTE_IM_CONTACT_DASHBOARD_UPDATED_EVENT, snapshot) {
                runtime_log_debug(format!(
                    "[远程会话仪表盘] 跳过，任务=推送快照，contact_id={}，error={}",
                    contact_id, err
                ));
            }
        }
    }
}

fn remote_im_start_contact_dashboard_push_worker(state: &AppState) {
    let should_start = match remote_im_contact_dashboard_subscriptions().lock() {
        Ok(mut subscriptions) => {
            if subscriptions.push_worker_running {
                false
            } else {
                subscriptions.push_worker_running = true;
                true
            }
        }
        Err(_) => false,
    };
    if !should_start {
        return;
    }
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(
                REMOTE_IM_CONTACT_DASHBOARD_PUSH_INTERVAL_SECONDS,
            ))
            .await;
            let contact_ids = match remote_im_contact_dashboard_subscriptions().lock() {
                Ok(mut subscriptions) => {
                    if subscriptions.contact_ids_by_window.is_empty() {
                        subscriptions.push_worker_running = false;
                        return;
                    }
                    subscriptions
                        .contact_ids_by_window
                        .values()
                        .cloned()
                        .collect::<std::collections::HashSet<_>>()
                }
                Err(_) => return,
            };
            for contact_id in contact_ids {
                remote_im_emit_contact_dashboard_snapshot(&state, &contact_id);
            }
        }
    });
}

#[tauri::command]
async fn remote_im_subscribe_contact_dashboard(
    input: RemoteImContactDashboardInput,
    state: State<'_, AppState>,
    window: tauri::Window,
) -> Result<RemoteImContactDashboardSnapshot, String> {
    let contact_id = input.contact_id.trim().to_string();
    let snapshot = remote_im_contact_dashboard_snapshot_inner(state.inner(), &contact_id)?;
    let window_label = window.label().to_string();
    {
        let mut subscriptions = remote_im_contact_dashboard_subscriptions()
            .lock()
            .map_err(|_| "远程联系人仪表盘订阅锁不可用".to_string())?;
        subscriptions
            .contact_ids_by_window
            .insert(window_label, contact_id);
    }
    remote_im_start_contact_dashboard_push_worker(state.inner());
    Ok(snapshot)
}

#[tauri::command]
async fn remote_im_sync_contact_dashboard(
    input: RemoteImContactDashboardInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContactDashboardSyncResult, String> {
    let snapshot = remote_im_contact_dashboard_snapshot_inner(state.inner(), &input.contact_id)?;
    let known = input.known_watermark.as_deref().unwrap_or("");
    Ok(RemoteImContactDashboardSyncResult {
        changed: known != snapshot.watermark,
        snapshot,
    })
}

#[tauri::command]
async fn remote_im_unsubscribe_contact_dashboard(
    input: RemoteImContactDashboardInput,
    window: tauri::Window,
) -> Result<(), String> {
    let contact_id = input.contact_id.trim();
    let window_label = window.label().to_string();
    let mut subscriptions = remote_im_contact_dashboard_subscriptions()
        .lock()
        .map_err(|_| "远程联系人仪表盘订阅锁不可用".to_string())?;
    if subscriptions
        .contact_ids_by_window
        .get(&window_label)
        .is_some_and(|current| current == contact_id)
    {
        subscriptions.contact_ids_by_window.remove(&window_label);
    }
    Ok(())
}

fn remote_im_subscribe_contact_dashboard_for_web(
    state: &AppState,
    params: serde_json::Value,
    client_id: &str,
) -> Result<serde_json::Value, String> {
    let input = serde_json::from_value::<RemoteImContactDashboardInput>(params)
        .map_err(|err| format!("解析远程联系人仪表盘订阅参数失败：{err}"))?;
    let contact_id = input.contact_id.trim().to_string();
    let snapshot = remote_im_contact_dashboard_snapshot_inner(state, &contact_id)?;
    let subscription_key = format!("web:{}", client_id.trim());
    if subscription_key != "web:" {
        let mut subscriptions = remote_im_contact_dashboard_subscriptions()
            .lock()
            .map_err(|_| "远程联系人仪表盘订阅锁不可用".to_string())?;
        subscriptions
            .contact_ids_by_window
            .insert(subscription_key, contact_id);
    }
    remote_im_start_contact_dashboard_push_worker(state);
    serde_json::to_value(snapshot).map_err(|err| format!("序列化远程联系人仪表盘快照失败：{err}"))
}

fn remote_im_sync_contact_dashboard_for_web(
    state: &AppState,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let input = serde_json::from_value::<RemoteImContactDashboardInput>(params)
        .map_err(|err| format!("解析远程联系人仪表盘同步参数失败：{err}"))?;
    let snapshot = remote_im_contact_dashboard_snapshot_inner(state, &input.contact_id)?;
    let known = input.known_watermark.as_deref().unwrap_or("");
    serde_json::to_value(RemoteImContactDashboardSyncResult {
        changed: known != snapshot.watermark,
        snapshot,
    })
    .map_err(|err| format!("序列化远程联系人仪表盘同步结果失败：{err}"))
}

fn remote_im_unsubscribe_contact_dashboard_for_web(
    params: serde_json::Value,
    client_id: &str,
) -> Result<serde_json::Value, String> {
    let input = serde_json::from_value::<RemoteImContactDashboardInput>(params)
        .map_err(|err| format!("解析远程联系人仪表盘取消订阅参数失败：{err}"))?;
    let subscription_key = format!("web:{}", client_id.trim());
    if let Ok(mut subscriptions) = remote_im_contact_dashboard_subscriptions().lock() {
        if subscriptions
            .contact_ids_by_window
            .get(&subscription_key)
            .is_some_and(|current| current == input.contact_id.trim())
        {
            subscriptions.contact_ids_by_window.remove(&subscription_key);
        }
    }
    Ok(serde_json::Value::Null)
}

#[derive(Clone)]
struct RemoteImContactBindingSnapshot {
    bound_agent_id: Option<String>,
    bound_conversation_id: Option<String>,
    route_mode: String,
}

fn remote_im_contact_binding_snapshot(
    contact: &RemoteImContact,
) -> RemoteImContactBindingSnapshot {
    RemoteImContactBindingSnapshot {
        bound_agent_id: contact.bound_agent_id.clone(),
        bound_conversation_id: contact.bound_conversation_id.clone(),
        route_mode: contact.route_mode.clone(),
    }
}

fn remote_im_contact_binding_matches(
    contact: &RemoteImContact,
    snapshot: &RemoteImContactBindingSnapshot,
) -> bool {
    contact.bound_agent_id == snapshot.bound_agent_id
        && contact.bound_conversation_id == snapshot.bound_conversation_id
        && contact.route_mode == snapshot.route_mode
}

fn remote_im_apply_contact_binding_snapshot(
    contact: &mut RemoteImContact,
    snapshot: &RemoteImContactBindingSnapshot,
) {
    contact.bound_agent_id = snapshot.bound_agent_id.clone();
    contact.bound_conversation_id = snapshot.bound_conversation_id.clone();
    contact.route_mode = snapshot.route_mode.clone();
}

fn remote_im_resolve_contact_session_target_atomic(
    state: &AppState,
    contact_id: &str,
    mut candidate: RemoteImContact,
) -> Result<(String, String, RemoteImContact), String> {
    for attempt in 0..4 {
        let baseline = remote_im_contact_binding_snapshot(&candidate);
        let runtime_snapshot = load_runtime_organization_snapshot(state)?;
        candidate.route_mode =
            remote_im_resolve_effective_route_mode(&runtime_snapshot.config, &candidate);
        let agent_id = resolve_contact_agent_id(state, candidate.bound_agent_id.as_deref())?;
        candidate.bound_agent_id = Some(agent_id.clone());
        let route_resolved = remote_im_contact_binding_snapshot(&candidate);
        let route_commit = (|| -> Result<Result<RemoteImContact, RemoteImContact>, String> {
            let mut contact = state_service_get_remote_im_contact(state, contact_id)?
                .ok_or_else(|| format!("联系人不存在: {contact_id}"))?;
            if !remote_im_contact_binding_matches(&contact, &baseline) {
                return Ok(Err(contact));
            }
            remote_im_apply_contact_binding_snapshot(&mut contact, &route_resolved);
            state_service_upsert_remote_im_contact(state, &contact)?;
            Ok(Ok(contact))
        })()?;
        let committed = match route_commit {
            Ok(contact) => contact,
            Err(latest_contact) => {
                runtime_log_warn(format!(
                    "[远程IM] 联系人绑定并发变化，使用最新配置重新解析，contact_id={}，attempt={}",
                    contact_id,
                    attempt + 1
                ));
                candidate = latest_contact;
                continue;
            }
        };
        let conversation_baseline = remote_im_contact_binding_snapshot(&committed);
        let mut resolved_contact = committed;
        let conversation_id = ensure_remote_im_contact_conversation_id(
            state,
            &mut resolved_contact,
        )?;
        let conversation_resolved = remote_im_contact_binding_snapshot(&resolved_contact);
        let conversation_commit = (|| -> Result<Result<RemoteImContact, RemoteImContact>, String> {
            let mut contact = state_service_get_remote_im_contact(state, contact_id)?
                .ok_or_else(|| format!("联系人不存在: {contact_id}"))?;
            if !remote_im_contact_binding_matches(&contact, &conversation_baseline) {
                return Ok(Err(contact));
            }
            remote_im_apply_contact_binding_snapshot(&mut contact, &conversation_resolved);
            state_service_upsert_remote_im_contact(state, &contact)?;
            Ok(Ok(contact))
        })()?;
        match conversation_commit {
            Ok(contact) => {
                if let Err(err) = sync_remote_im_contact_conversation_binding(
                    state,
                    &contact,
                    &conversation_id,
                ) {
                    runtime_log_warn(format!(
                        "[远程IM] 联系人路由已提交，会话绑定同步降级，contact_id={}，conversation_id={}，error={}",
                        contact_id, conversation_id, err
                    ));
                }
                return Ok((agent_id, conversation_id, contact));
            }
            Err(latest_contact) => {
                runtime_log_warn(format!(
                    "[远程IM] 联系人会话绑定并发变化，使用最新配置重新解析，contact_id={}，attempt={}",
                    contact_id,
                    attempt + 1
                ));
                candidate = latest_contact;
            }
        }
    }
    runtime_log_warn(format!(
        "[远程IM] 联系人绑定持续变化，本次按最新快照继续路由且不覆盖配置，contact_id={}",
        contact_id
    ));
    let latest_contact = state_service_get_remote_im_contact(state, contact_id)?
        .ok_or_else(|| format!("联系人不存在: {contact_id}"))?;
    let mut fallback_contact = latest_contact;
    let (agent_id, conversation_id) = resolve_contact_session_target(
        state,
        &mut fallback_contact,
    )?;
    Ok((
        agent_id,
        conversation_id,
        fallback_contact,
    ))
}

fn remote_im_resolve_contact_session_target_fail_soft(
    state: &AppState,
    input: &RemoteImEnqueueInput,
    contact_id: &str,
    candidate: RemoteImContact,
) -> Option<(String, String, RemoteImContact)> {
    match remote_im_resolve_contact_session_target_atomic(
        state,
        contact_id,
        candidate.clone(),
    ) {
        Ok(resolved) => return Some(resolved),
        Err(err) => runtime_log_warn(format!(
            "[远程IM] 入站路由解析降级，contact_id={}，error={}",
            contact_id, err
        )),
    }

    let mut fallback_contact = candidate;
    let agent_id = fallback_contact
        .bound_agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let value = input.session.agent_id.trim();
            if value.is_empty() {
                None
            } else {
                Some(value)
            }
        })
        .unwrap_or(DEFAULT_AGENT_ID)
        .to_string();
    fallback_contact.bound_agent_id = Some(agent_id.clone());
    fallback_contact.route_mode = "dedicated_contact_conversation".to_string();
    let conversation_id = fallback_contact
        .bound_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            conversation_service_v2()
                .create_remote_im_contact_conversation(
                    state,
                    &remote_im_contact_conversation_title(&fallback_contact),
                    &agent_id,
                    &remote_im_contact_conversation_key(&fallback_contact),
                )
                .map(|conversation| conversation.id)
                .map_err(|err| {
                    runtime_log_warn(format!(
                        "[远程IM] 入站专属会话创建降级，contact_id={}，error={}",
                        contact_id, err
                    ));
                    err
                })
                .ok()
        })
        .or_else(|| {
            input
                .session
                .conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
        });
    let Some(conversation_id) = conversation_id else {
        runtime_log_error(format!(
            "[远程IM] 跳过，任务=接收入站消息，contact_id={}，原因=无法取得可用会话",
            contact_id
        ));
        return None;
    };
    fallback_contact.bound_conversation_id = Some(conversation_id.clone());
    if let Err(err) = (|| -> Result<(), String> {
        let Some(mut contact) = state_service_get_remote_im_contact(state, contact_id)? else {
            return Ok(());
        };
        if contact.bound_agent_id == fallback_contact.bound_agent_id {
            contact.bound_conversation_id = Some(conversation_id.clone());
            contact.route_mode = fallback_contact.route_mode.clone();
            state_service_upsert_remote_im_contact(state, &contact)?;
        }
        Ok(())
    })() {
        runtime_log_warn(format!(
            "[远程IM] 入站降级路由继续执行，联系人会话绑定未落盘，contact_id={}，conversation_id={}，error={}",
            contact_id, conversation_id, err
        ));
    }
    Some((
        agent_id,
        conversation_id,
        fallback_contact,
    ))
}

#[tauri::command]
fn remote_im_list_contacts(state: State<'_, AppState>) -> Result<Vec<RemoteImContact>, String> {
    remote_im_list_contacts_inner(state.inner())
}

#[tauri::command]
fn remote_im_get_default_group_response_guidance() -> String {
    default_remote_im_contact_response_guidance()
}

fn remote_im_update_contact_allow_send_inner(
    state: &AppState,
    input: RemoteImContactAllowSendUpdateInput,
) -> Result<RemoteImContact, String> {
    remote_im_mutate_contact(state, &input.contact_id, |contact| {
        contact.allow_send = input.allow_send;
        contact.allow_receive = input.allow_send;
        Ok(contact.clone())
    })
}

#[tauri::command]
fn remote_im_update_contact_allow_send(
    input: RemoteImContactAllowSendUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_update_contact_allow_send_inner(state.inner(), input)
}

fn remote_im_update_contact_allow_send_files_inner(
    state: &AppState,
    input: RemoteImContactAllowSendFilesUpdateInput,
) -> Result<RemoteImContact, String> {
    remote_im_mutate_contact(state, &input.contact_id, |contact| {
        contact.allow_send_files = input.allow_send_files;
        Ok(contact.clone())
    })
}

#[tauri::command]
fn remote_im_update_contact_allow_send_files(
    input: RemoteImContactAllowSendFilesUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_update_contact_allow_send_files_inner(state.inner(), input)
}

fn remote_im_update_contact_blocked_message_prefixes_inner(
    state: &AppState,
    input: RemoteImContactBlockedMessagePrefixesUpdateInput,
) -> Result<RemoteImContact, String> {
    let _ = input.blocked_message_prefixes;
    runtime_log_warn(format!(
        "[远程IM] 已忽略旧联系人消息头过滤保存，请改用渠道统一行为设置，contact_id={}",
        input.contact_id
    ));
    remote_im_get_contact_by_id(state, &input.contact_id)
}

#[tauri::command]
fn remote_im_update_contact_blocked_message_prefixes(
    input: RemoteImContactBlockedMessagePrefixesUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_update_contact_blocked_message_prefixes_inner(state.inner(), input)
}

fn remote_im_update_contact_behavior_inner(
    state: &AppState,
    input: RemoteImContactBehaviorUpdateInput,
) -> Result<RemoteImContact, String> {
    let contact_id = input.contact_id;
    runtime_log_warn(format!(
        "[远程IM] 已忽略旧联系人行为保存，请改用渠道统一行为设置，contact_id={}",
        contact_id
    ));
    remote_im_get_contact_by_id(state, &contact_id)
}

#[tauri::command]
fn remote_im_update_contact_behavior(
    input: RemoteImContactBehaviorUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_update_contact_behavior_inner(state.inner(), input)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImChannelBehaviorReconfigureResult {
    reconfigured_contacts: usize,
    skipped_contacts: usize,
}

fn remote_im_reconfigure_channel_behavior_inner(
    state: &AppState,
    channel_id: &str,
) -> RemoteImChannelBehaviorReconfigureResult {
    let channel_id = channel_id.trim();
    if channel_id.is_empty() {
        runtime_log_warn("[群聊巡检] 渠道行为重排跳过，原因=渠道ID为空".to_string());
        return RemoteImChannelBehaviorReconfigureResult {
            reconfigured_contacts: 0,
            skipped_contacts: 0,
        };
    }
    let contacts = match state_service_list_remote_im_contacts(state, Some(channel_id)) {
        Ok(contacts) => contacts,
        Err(err) => {
            runtime_log_warn(format!(
                "[群聊巡检] 渠道行为已保存，但联系人快照读取失败，跳过本次重排并保持业务继续，channel_id={}，error={}",
                channel_id, err
            ));
            return RemoteImChannelBehaviorReconfigureResult {
                reconfigured_contacts: 0,
                skipped_contacts: 0,
            };
        }
    };
    let mut reconfigured_contacts = 0usize;
    let mut skipped_contacts = 0usize;
    for contact in contacts.iter().filter(|contact| {
        contact.remote_contact_type
            .trim()
            .eq_ignore_ascii_case("group")
    }) {
        match remote_im_group_reply_reconfigure_contact(state, contact) {
            Ok(()) => reconfigured_contacts = reconfigured_contacts.saturating_add(1),
            Err(err) => {
                skipped_contacts = skipped_contacts.saturating_add(1);
                runtime_log_warn(format!(
                    "[群聊巡检] 渠道行为保存后单联系人重排降级，channel_id={}，contact_id={}，error={}",
                    channel_id, contact.id, err
                ));
            }
        }
    }
    runtime_log_info(format!(
        "[群聊巡检] 渠道行为重排完成，channel_id={}，reconfigured_contacts={}，skipped_contacts={}",
        channel_id, reconfigured_contacts, skipped_contacts
    ));
    RemoteImChannelBehaviorReconfigureResult {
        reconfigured_contacts,
        skipped_contacts,
    }
}

#[tauri::command]
fn remote_im_reconfigure_channel_behavior(
    channel_id: String,
    state: State<'_, AppState>,
) -> RemoteImChannelBehaviorReconfigureResult {
    remote_im_reconfigure_channel_behavior_inner(state.inner(), &channel_id)
}

fn remote_im_patch_contact_settings_inner(
    state: &AppState,
    input: RemoteImContactSettingsPatchInput,
) -> Result<RemoteImContact, String> {
    let runtime_snapshot = match load_runtime_organization_snapshot(state) {
        Ok(snapshot) => Some(snapshot),
        Err(err) => {
            runtime_log_warn(format!(
                "[远程IM] 联系人完整设置读取组织配置失败，本次保存用户输入并延后校验，contact_id={}，error={}",
                input.contact_id, err
            ));
            None
        }
    };
    let next_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let next_agent = if let Some(agent_id) = next_agent_id.as_deref() {
        if runtime_snapshot.is_some() {
            Some(resolve_contact_agent_id(state, Some(agent_id))?)
        } else {
            Some(agent_id.to_string())
        }
    } else {
        None
    };
    let output = remote_im_mutate_contact(state, &input.contact_id, |contact| {
        let is_private = remote_im_contact_is_private(contact);
        contact.bound_agent_id = next_agent.clone();
        contact.route_mode = runtime_snapshot
            .as_ref()
            .map(|snapshot| remote_im_resolve_effective_route_mode(&snapshot.config, contact))
            .unwrap_or_else(|| "dedicated_contact_conversation".to_string());
        contact.processing_mode = normalize_contact_processing_mode(&input.processing_mode);
        if is_private {
            contact.activation_mode = "always".to_string();
            contact.activation_keywords.clear();
            contact.response_strategy = "always_reply".to_string();
        } else {
            contact.activation_mode = normalize_contact_activation_mode(&input.activation_mode);
            contact.activation_keywords =
                normalize_contact_activation_keywords(&input.activation_keywords);
            contact.response_strategy =
                normalize_contact_response_strategy(&input.response_strategy);
        }
        let communication_enabled = input.allow_receive || input.allow_send;
        contact.allow_receive = communication_enabled;
        contact.allow_send = communication_enabled;
        contact.allow_send_files = input.allow_send_files;
        Ok(contact.clone())
    })?;

    if let Some(conversation_id) = output
        .bound_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let resolved_agent = runtime_snapshot.as_ref().map_or_else(
            || {
                Err(
                    "组织配置暂时不可读，已保存联系人设置并延后同步会话路由"
                        .to_string(),
                )
            },
            |_snapshot| resolve_contact_agent_id(state, output.bound_agent_id.as_deref()),
        );
        match resolved_agent {
            Ok(_agent_id) => {
                if let Err(err) = sync_remote_im_contact_conversation_binding(
                    state,
                    &output,
                    conversation_id,
                ) {
                    runtime_log_warn(format!(
                        "[远程IM] 联系人设置已保存，会话绑定同步降级，contact_id={}，conversation_id={}，error={}",
                        output.id, conversation_id, err
                    ));
                }
            }
            Err(err) => runtime_log_warn(format!(
                "[远程IM] 联系人设置已保存，会话路由解析降级，contact_id={}，error={}",
                output.id, err
            )),
        }
    }
    Ok(output)
}

#[tauri::command]
fn remote_im_patch_contact_settings(
    input: RemoteImContactSettingsPatchInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_patch_contact_settings_inner(state.inner(), input)
}

#[tauri::command]
fn remote_im_update_contact_allow_receive(
    input: RemoteImContactAllowReceiveUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_mutate_contact(state.inner(), &input.contact_id, |contact| {
        contact.allow_receive = input.allow_receive;
        contact.allow_send = input.allow_receive;
        Ok(contact.clone())
    })
}

fn remote_im_update_contact_activation_inner(
    state: &AppState,
    input: RemoteImContactActivationUpdateInput,
) -> Result<RemoteImContact, String> {
    remote_im_mutate_contact(state, &input.contact_id, |contact| {
        contact.activation_mode = normalize_contact_activation_mode(&input.activation_mode);
        contact.activation_keywords =
            normalize_contact_activation_keywords(&input.activation_keywords);
        if remote_im_contact_is_private(contact) {
            contact.response_strategy = "always_reply".to_string();
        } else {
            contact.response_strategy =
                normalize_contact_response_strategy(&input.response_strategy);
        }
        Ok(contact.clone())
    })
}

#[tauri::command]
fn remote_im_update_contact_activation(
    input: RemoteImContactActivationUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_update_contact_activation_inner(state.inner(), input)
}

#[tauri::command]
fn remote_im_update_contact_remark(
    input: RemoteImContactRemarkUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_mutate_contact(state.inner(), &input.contact_id, |contact| {
        contact.remark_name = input.remark_name.trim().to_string();
        Ok(contact.clone())
    })
}

#[tauri::command]
fn remote_im_update_contact_route_mode(
    input: RemoteImContactRouteModeUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    let config = state_read_config_cached(&state)?;
    remote_im_mutate_contact(state.inner(), &input.contact_id, |contact| {
        let requested_mode = normalize_contact_route_mode(&input.route_mode);
        let final_mode = remote_im_resolve_effective_route_mode(&config, contact);
        if requested_mode != final_mode {
            runtime_log_info(format!(
                "[远程IM] 联系人路由模式已被约束修正: contact_id={}, requested={}, final={}",
                contact.id, requested_mode, final_mode
            ));
        }
        contact.route_mode = final_mode;
        Ok(contact.clone())
    })
}

fn remote_im_update_contact_agent_binding_inner(
    state: &AppState,
    input: RemoteImContactAgentBindingUpdateInput,
) -> Result<RemoteImContact, String> {
    let runtime_snapshot = match load_runtime_organization_snapshot(state) {
        Ok(snapshot) => Some(snapshot),
        Err(err) => {
            runtime_log_warn(format!(
                "[远程IM] 更新联系人处理人格时组织配置读取失败，本次保存原始绑定并延后校验，contact_id={}，error={}",
                input.contact_id, err
            ));
            None
        }
    };
    let next_agent_id = input
        .agent_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let next_agent = if let Some(agent_id) = next_agent_id.as_deref() {
        if runtime_snapshot.is_some() {
            Some(resolve_contact_agent_id(state, Some(agent_id))?)
        } else {
            Some(agent_id.to_string())
        }
    } else {
        None
    };
    let output = remote_im_mutate_contact(state, &input.contact_id, |contact| {
        contact.bound_agent_id = next_agent.clone();
        contact.route_mode = runtime_snapshot
            .as_ref()
            .map(|snapshot| remote_im_resolve_effective_route_mode(&snapshot.config, contact))
            .unwrap_or_else(|| "dedicated_contact_conversation".to_string());
        Ok(contact.clone())
    })?;
    let resolved = match remote_im_resolve_contact_session_target_atomic(
        state,
        &input.contact_id,
        output.clone(),
    ) {
        Ok((_, _, resolved)) => resolved,
        Err(err) => {
            runtime_log_warn(format!(
                "[远程IM] 联系人处理人格已保存，会话绑定修复降级，contact_id={}，error={}",
                input.contact_id, err
            ));
            return Ok(output);
        }
    };
    let conversation_id = resolved
        .bound_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    runtime_log_info(format!(
        "[远程IM] 完成，任务=更新联系人处理人格，contact_id={}，conversation_id={}，agent_id={}",
        resolved.id,
        conversation_id,
        conversation_service_v2()
            .get_conversation_meta(state, &conversation_id)
            .map(|conversation| conversation.agent_id)
            .unwrap_or_default()
    ));
    Ok(resolved)
}

#[tauri::command]
fn remote_im_update_contact_agent_binding(
    input: RemoteImContactAgentBindingUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_update_contact_agent_binding_inner(state.inner(), input)
}

fn remote_im_update_contact_processing_mode_inner(
    state: &AppState,
    input: RemoteImContactProcessingModeUpdateInput,
) -> Result<RemoteImContact, String> {
    remote_im_mutate_contact(state, &input.contact_id, |contact| {
        contact.processing_mode = normalize_contact_processing_mode(&input.processing_mode);
        Ok(contact.clone())
    })
}

#[tauri::command]
fn remote_im_update_contact_processing_mode(
    input: RemoteImContactProcessingModeUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_update_contact_processing_mode_inner(state.inner(), input)
}

fn remote_im_update_contact_workspace_inner(
    state: &AppState,
    input: RemoteImContactWorkspaceUpdateInput,
) -> Result<RemoteImContact, String> {
    let output = remote_im_mutate_contact(state, &input.contact_id, |contact| {
        contact.shell_workspaces = input.shell_workspaces;
        Ok(contact.clone())
    })?;
    if let Some(conversation_id) = output
        .bound_conversation_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        mark_prompt_cache_rebuild_for_system_environment_by_conversation(state, conversation_id);
    }
    Ok(output)
}

#[tauri::command]
fn remote_im_update_contact_workspace(
    input: RemoteImContactWorkspaceUpdateInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContact, String> {
    remote_im_update_contact_workspace_inner(state.inner(), input)
}

#[tauri::command]
fn remote_im_list_contact_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<RemoteImContactConversationSummary>, String> {
    let started_at = std::time::Instant::now();
    runtime_log_debug("[远程IM][联系人会话][列表] 开始".to_string());
    let items = conversation_service_v2().list_remote_im_contact_conversations(state.inner())?;
    runtime_log_debug(format!(
        "[远程IM][联系人会话][列表] 完成: contact_count={}, elapsed_ms={}",
        items.len(),
        started_at.elapsed().as_millis()
    ));
    Ok(items)
}

#[tauri::command]
async fn remote_im_get_contact_conversation_messages(
    input: RemoteImContactConversationMessagesInput,
    state: State<'_, AppState>,
) -> Result<Vec<ChatMessage>, String> {
    let contact_id = input.contact_id.trim().to_string();
    if contact_id.is_empty() {
        return Err("contact_id 为必填项。".to_string());
    }
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let started_at = std::time::Instant::now();
        runtime_log_debug(format!(
            "[远程IM][联系人会话][读取] 开始: contact_id={}",
            contact_id
        ));
        let messages = conversation_service_v2()
            .get_remote_im_contact_conversation_messages(&app_state, &contact_id)?;
        runtime_log_debug(format!(
            "[远程IM][联系人会话][读取] 完成: contact_id={}, message_count={}, elapsed_ms={}",
            contact_id,
            messages.len(),
            started_at.elapsed().as_millis()
        ));
        Ok(messages)
    })
    .await
    .map_err(|err| format!("读取远程 IM 联系人会话消息任务异常：{err}"))?
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationBlockPageInput {
    contact_id: String,
    #[serde(default)]
    block_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationBlockSummaryOutput {
    block_id: u32,
    message_count: usize,
    first_message_id: String,
    last_message_id: String,
    first_created_at: Option<String>,
    last_created_at: Option<String>,
    is_latest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteImContactConversationBlockPageOutput {
    blocks: Vec<RemoteImContactConversationBlockSummaryOutput>,
    selected_block_id: u32,
    messages: Vec<ChatMessage>,
    has_prev_block: bool,
    has_next_block: bool,
}

#[tauri::command]
async fn remote_im_get_contact_conversation_block_page(
    input: RemoteImContactConversationBlockPageInput,
    state: State<'_, AppState>,
) -> Result<RemoteImContactConversationBlockPageOutput, String> {
    let app_state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        remote_im_get_contact_conversation_block_page_inner(input, &app_state)
    })
    .await
    .map_err(|err| format!("读取远程 IM 联系人会话块分页任务异常：{err}"))?
}

fn remote_im_get_contact_conversation_block_page_inner(
    input: RemoteImContactConversationBlockPageInput,
    state: &AppState,
) -> Result<RemoteImContactConversationBlockPageOutput, String> {
    let contact_id = input.contact_id.trim();
    if contact_id.is_empty() {
        return Err("contact_id 为必填项。".to_string());
    }
    let started_at = std::time::Instant::now();
    runtime_log_debug(format!(
        "[远程IM][联系人会话][块分页] 开始: contact_id={}, requested_block_id={}",
        contact_id,
        input.block_id
            .map(|value| value.to_string())
            .unwrap_or_else(|| "latest".to_string())
    ));
    let page = conversation_service_v2().get_remote_im_contact_conversation_block_page(
        state,
        contact_id,
        input.block_id,
    )?;
    runtime_log_debug(format!(
        "[远程IM][联系人会话][块分页] 完成: contact_id={}, selected_block_id={}, message_count={}, elapsed_ms={}",
        contact_id,
        page.selected_block_id,
        page.messages.len(),
        started_at.elapsed().as_millis()
    ));
    Ok(RemoteImContactConversationBlockPageOutput {
        blocks: page
            .blocks
            .into_iter()
            .map(|item| RemoteImContactConversationBlockSummaryOutput {
                block_id: item.block_id,
                message_count: item.message_count,
                first_message_id: item.first_message_id,
                last_message_id: item.last_message_id,
                first_created_at: item.first_created_at,
                last_created_at: item.last_created_at,
                is_latest: item.is_latest,
            })
            .collect(),
        selected_block_id: page.selected_block_id,
        messages: page.messages,
        has_prev_block: page.has_prev_block,
        has_next_block: page.has_next_block,
    })
}

fn remote_im_delete_contact_inner(
    state: &AppState,
    input: RemoteImContactDeleteInput,
) -> Result<bool, String> {
    let contact_id = input.contact_id.trim();
    if contact_id.is_empty() {
        return Err("contact_id 为必填项。".to_string());
    }
    let removed = state_service_remove_remote_im_contact(state, contact_id)?;
    if removed {
        if let Err(err) = clear_remote_im_debounces_for_contact(state, contact_id) {
            runtime_log_warn(format!(
                "[群聊巡检] 删除联系人后清理状态失败，contact_id={}，error={}",
                contact_id, err
            ));
        }
        if let Err(err) = abort_remote_im_reply_delegates_for_contact(
            state,
            contact_id,
            "远程联系人已删除",
        ) {
            runtime_log_warn(format!(
                "[远程应答委托] 删除联系人后终止委托失败，contact_id={}，error={}",
                contact_id, err
            ));
        }
    }
    Ok(removed)
}

#[tauri::command]
fn remote_im_delete_contact(
    input: RemoteImContactDeleteInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    remote_im_delete_contact_inner(state.inner(), input)
}

#[tauri::command]
fn remote_im_clear_contact_conversation(
    input: RemoteImContactDeleteInput,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    remote_im_clear_contact_conversation_inner(input, state.inner())
}

fn remote_im_clear_contact_conversation_inner(
    input: RemoteImContactDeleteInput,
    state: &AppState,
) -> Result<bool, String> {
    let contact_id = input.contact_id.trim();
    if contact_id.is_empty() {
        return Err("contact_id 为必填项。".to_string());
    }
    let started_at = std::time::Instant::now();
    runtime_log_info(format!(
        "[远程IM][联系人会话][清空] 开始: contact_id={}",
        contact_id
    ));
    let cleared =
        conversation_service_v2().clear_remote_im_contact_conversation(state, contact_id)?;
    runtime_log_info(format!(
        "[远程IM][联系人会话][清空] 完成: contact_id={}, elapsed_ms={}",
        contact_id,
        started_at.elapsed().as_millis()
    ));
    Ok(cleared)
}

#[tauri::command]
async fn remote_im_enqueue_message(
    input: RemoteImEnqueueInput,
    state: State<'_, AppState>,
) -> Result<RemoteImEnqueueResult, String> {
    remote_im_enqueue_message_internal(input, state.inner()).await
}

fn remote_im_stale_cached_config_best_effort(state: &AppState) -> Option<AppConfig> {
    match state.cached_config.lock() {
        Ok(cached) => cached.clone(),
        Err(poisoned) => {
            runtime_log_warn(
                "[远程IM] 渠道配置缓存锁中毒，恢复锁但不使用不确定权限快照".to_string(),
            );
            state.cached_config.clear_poison();
            drop(poisoned.into_inner());
            None
        }
    }
}

/// 内部入队函数，供事件消费循环调用
pub(crate) async fn remote_im_enqueue_message_internal(
    input: RemoteImEnqueueInput,
    state: &AppState,
) -> Result<RemoteImEnqueueResult, String> {
    let validated = match state_read_config_cached(state) {
        Ok(config) => validate_enqueue_input(&input, &config)?,
        Err(err) => {
            runtime_log_warn(format!(
                "[远程IM] 入站渠道配置读取失败，尝试使用最后可信快照，channel_id={}，error={}",
                input.channel_id.trim(), err
            ));
            let Some(config) = remote_im_stale_cached_config_best_effort(state) else {
                runtime_log_warn(format!(
                    "[远程IM] 跳过，任务=接收入站消息，channel_id={}，原因=没有可信渠道权限快照",
                    input.channel_id.trim()
                ));
                return Ok(RemoteImEnqueueResult {
                    event_id: String::new(),
                    conversation_id: String::new(),
                    activate_assistant: false,
                    contact_id: String::new(),
                });
            };
            match validate_enqueue_input(&input, &config) {
                Ok(validated) => validated,
                Err(permission_err) => {
                    runtime_log_warn(format!(
                        "[远程IM] 跳过，任务=接收入站消息，channel_id={}，原因=最后可信权限快照拒绝，error={}",
                        input.channel_id.trim(), permission_err
                    ));
                    return Ok(RemoteImEnqueueResult {
                        event_id: String::new(),
                        conversation_id: String::new(),
                        activate_assistant: false,
                        contact_id: String::new(),
                    });
                }
            }
        }
    };
    let channel = validated.channel;
    let channel_label = if channel.name.trim().is_empty() {
        input.im_name.trim().to_string()
    } else {
        channel.name.trim().to_string()
    };
    let text = validated.text;
    let images = validated.images;
    let audios = validated.audios;
    let attachments = validated.attachments;

    let channel_behavior = remote_im_channel_behavior_settings_from_channel(&channel);
    let blocked_prefixes = channel_behavior.blocked_message_prefixes;
    if let Some(prefix) = remote_im_blocked_inbound_message_prefix(&text, &blocked_prefixes) {
        let contact_label = input
            .remote_contact_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or("未知联系人")
            .to_string();
        runtime_log_info(format!(
            "[远程IM] 入站消息跳过：渠道={}，联系人={}，原因=命中消息头过滤，过滤前缀={}，文本长度={}",
            channel_label,
            contact_label,
            prefix,
            text.chars().count()
        ));
        let log_message = format!(
                "[联系人消息] 过滤跳过: contact={}, prefix={}, text_len={}",
                contact_label,
                prefix,
                text.chars().count()
            );
        remote_im_append_channel_log(input.channel_id.trim(), "info", log_message);
        return Ok(RemoteImEnqueueResult {
            event_id: String::new(),
            conversation_id: String::new(),
            activate_assistant: false,
            contact_id: String::new(),
        });
    }
    if remote_im_remember_inbound_platform_message_id(
        input.channel_id.trim(),
        input.platform_message_id.as_deref(),
    )? {
        runtime_log_info(format!(
            "[远程IM] 入站消息跳过：渠道={}，原因=命中最近平台消息ID去重，platform_message_id={}",
            channel_label,
            input
                .platform_message_id
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
        ));
        remote_im_append_channel_log(
            input.channel_id.trim(),
            "info",
            format!(
                "[联系人消息] 去重跳过: channel={}, platform_message_id={}",
                channel_label,
                input
                    .platform_message_id
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
            ),
        );
        return Ok(RemoteImEnqueueResult {
            event_id: String::new(),
            conversation_id: String::new(),
            activate_assistant: false,
            contact_id: String::new(),
        });
    }

    let now = now_iso();
    let persisted_contact = (|| -> Result<(String, RemoteImContact, bool), String> {
        let contact_id = remote_im_upsert_contact_for_inbound(state, &input, &now)?;
        let contact = state_service_get_remote_im_contact(state, &contact_id)?
            .ok_or_else(|| format!("联系人不存在: {contact_id}"))?;
        let allow_receive = contact.allow_receive;
        Ok((contact_id, contact, allow_receive))
    })();
    let (contact_id, detached_contact, allow_receive) = match persisted_contact {
        Ok(result) => result,
        Err(err) => {
            runtime_log_warn(format!(
                "[远程IM] 入站联系人落盘失败，使用临时联系人继续接入，channel_id={}，remote_contact_id={}，error={}",
                input.channel_id.trim(), input.remote_contact_id.trim(), err
            ));
            let contact_id = match remote_im_upsert_contact_for_inbound(state, &input, &now) {
                Ok(contact_id) => contact_id,
                Err(fallback_err) => {
                    runtime_log_error(format!(
                        "[远程IM] 跳过，任务=接收入站消息，channel_id={}，remote_contact_id={}，原因=临时联系人创建失败，error={}",
                        input.channel_id.trim(), input.remote_contact_id.trim(), fallback_err
                    ));
                    return Ok(RemoteImEnqueueResult {
                        event_id: String::new(),
                        conversation_id: String::new(),
                        activate_assistant: false,
                        contact_id: String::new(),
                    });
                }
            };
            let Some(contact) = state_service_get_remote_im_contact(state, &contact_id).ok().flatten()
            else {
                runtime_log_error(format!(
                    "[远程IM] 跳过，任务=接收入站消息，channel_id={}，remote_contact_id={}，原因=临时联系人创建失败",
                    input.channel_id.trim(), input.remote_contact_id.trim()
                ));
                return Ok(RemoteImEnqueueResult {
                    event_id: String::new(),
                    conversation_id: String::new(),
                    activate_assistant: false,
                    contact_id,
                });
            };
            let allow_receive = contact.allow_receive;
            (contact_id, contact, allow_receive)
        }
    };
    if !allow_receive {
        runtime_log_info(format!(
            "[远程IM] 入站消息忽略：渠道={}，联系人={}，原因=联系人未开启收信",
            channel_label,
            remote_im_contact_log_label(&detached_contact)
        ));
        remote_im_append_contact_log(
            &detached_contact,
            "info",
            format!(
                "[联系人消息] 忽略: channel={}, contact={}, reason=联系人未开启收信",
                channel_label,
                remote_im_contact_log_label(&detached_contact)
            ),
        );
        return Ok(RemoteImEnqueueResult {
            event_id: String::new(),
            conversation_id: detached_contact
                .bound_conversation_id
                .clone()
                .unwrap_or_default(),
            activate_assistant: false,
            contact_id,
        });
    }
    let Some((agent_id, conversation_id, contact_for_log)) =
        remote_im_resolve_contact_session_target_fail_soft(
            state,
            &input,
            &contact_id,
            detached_contact,
        )
    else {
        return Ok(RemoteImEnqueueResult {
            event_id: String::new(),
            conversation_id: String::new(),
            activate_assistant: false,
            contact_id,
        });
    };
    let sender_label = if input.sender_name.trim().is_empty() {
        "未知发送人".to_string()
    } else {
        input.sender_name.trim().to_string()
    };
    runtime_log_info(format!(
        "[远程IM] 入站路由完成：渠道={}，联系人={}，处理模式={}，应答策略={}",
        channel_label,
        remote_im_contact_log_label(&contact_for_log),
        contact_for_log.route_mode,
        contact_for_log.response_strategy
    ));
    let total_image_count = images.len() + validated.parts_image_count;
    let total_audio_count = audios.len() + validated.parts_audio_count;
    let total_attachment_count = attachments.len() + validated.parts_attachment_count;

    runtime_log_info(format!(
        "[远程IM] 收到消息：渠道={}，联系人={}，发送人={}，内容={}，图片={}，音频={}，附件={}",
        channel_label,
        remote_im_contact_log_label(&contact_for_log),
        sender_label,
        remote_im_preview_text(&text, 100),
        total_image_count,
        total_audio_count,
        total_attachment_count
    ));
    remote_im_append_contact_log(
        &contact_for_log,
        "info",
        format!(
            "[联系人消息] 收到: channel={}, contact={}, sender={}, image_count={}, audio_count={}, attachment_count={}, preview={}",
            channel_label,
            remote_im_contact_log_label(&contact_for_log),
            sender_label,
            total_image_count,
            total_audio_count,
            total_attachment_count,
            remote_im_preview_text(&text, 100)
        ),
    );
    let message = build_chat_message_from_input(
        state,
        &input,
        &conversation_id,
        &contact_for_log,
        &now,
        &text,
        &images,
        &audios,
        &attachments,
    );

    let sender_info = RemoteImMessageSource {
        channel_id: input.channel_id.trim().to_string(),
        platform: input.platform,
        im_name: input.im_name,
        remote_contact_type: input.remote_contact_type,
        remote_contact_id: input.remote_contact_id,
        remote_contact_name: input.remote_contact_name.unwrap_or_default(),
        sender_id: input.sender_id,
        sender_name: input.sender_name,
        sender_avatar_url: input.sender_avatar_url,
        platform_message_id: input.platform_message_id,
    };
    let event_id = Uuid::new_v4().to_string();
    let session_info = ChatSessionInfo {
                agent_id,
    };
    if sender_info
        .remote_contact_type
        .trim()
        .eq_ignore_ascii_case("group")
    {
        let mut persisted_message = message.clone();
        if let Err(err) =
            externalize_message_parts_to_media_refs(&mut persisted_message.parts, &state.data_path)
        {
            runtime_log_warn(format!(
                "[远程IM] 入站附件外部化降级，contact_id={}，conversation_id={}，message_id={}，error={}",
                contact_id, conversation_id, persisted_message.id, err
            ));
        }
        let persisted_message = conversation_service_v2()
            .append_remote_im_user_message(state, &conversation_id, &persisted_message)
            .await?;
        if let Err(err) = (|| -> Result<(), String> {
            let mut checkpoint = state_service_get_remote_im_contact_checkpoint(
                state,
                &contact_id,
            )?
            .unwrap_or_else(|| RemoteImContactCheckpoint {
                contact_id: contact_id.clone(),
                ..Default::default()
            });
            remote_im_update_checkpoint_latest_seen_in_checkpoint(
                &mut checkpoint,
                Some(&persisted_message.id),
                &now,
            );
            state_service_set_remote_im_contact_checkpoint(state, &checkpoint)
        })() {
            runtime_log_warn(format!(
                "[远程IM] 已收消息标记更新失败：渠道={}，联系人={}，内容={}，error={}",
                channel_label,
                remote_im_contact_log_label(&contact_for_log),
                remote_im_preview_text(&text, 100),
                err
            ));
        }
        let (activate_assistant, state_reason) = match remote_im_prepare_enqueue_runtime_state(
            state,
            &contact_for_log,
            &text,
        ) {
            Ok(result) => result,
            Err(err) => {
                runtime_log_warn(format!(
                    "[远程IM] 入库后入场判定失败，本次只保留消息，contact_id={}，conversation_id={}，error={}",
                    contact_id, conversation_id, err
                ));
                (false, "入场判定失败，仅保留已入库消息".to_string())
            }
        };
        runtime_log_info(format!(
            "[远程联系人状态机] 入站判定完成：渠道={}，联系人={}，发送人={}，内容={}，入场={}，原因={}",
            channel_label,
            remote_im_contact_log_label(&contact_for_log),
            sender_label,
            remote_im_preview_text(&text, 100),
            remote_im_yes_no(activate_assistant),
            state_reason
        ));
        if activate_assistant {
            let event = create_pending_event(
                event_id.clone(),
                conversation_id.clone(),
                vec![persisted_message],
                true,
                session_info,
                sender_info,
            );
            observe_remote_im_persisted_event(state, &contact_for_log, &event);
        }
        remote_im_append_contact_log(
            &contact_for_log,
            "info",
            format!(
                "[联系人状态] 已交接: action=群聊巡检, channel={}, contact={}, message={}, activate={}, reason={}",
                channel_label,
                remote_im_contact_log_label(&contact_for_log),
                remote_im_preview_text(&text, 100),
                remote_im_yes_no(activate_assistant),
                state_reason
            ),
        );
        return Ok(RemoteImEnqueueResult {
            event_id,
            conversation_id,
            activate_assistant,
            contact_id,
        });
    }
    runtime_log_info(format!(
        "[远程IM] 私聊入站交接：渠道={}，联系人={}，发送人={}，内容={}，处理=绑定会话引导",
        channel_label,
        remote_im_contact_log_label(&contact_for_log),
        sender_label,
        remote_im_preview_text(&text, 100)
    ));
    let event = create_pending_event(
        event_id.clone(),
        conversation_id.clone(),
        vec![message],
        true,
        session_info,
        sender_info,
    );
    let ingress = ingress_chat_event(state, event)?;
    remote_im_append_contact_log(
        &contact_for_log,
        "info",
        format!(
            "[联系人状态] 已交接: action=绑定会话引导, channel={}, contact={}, message={}",
            channel_label,
            remote_im_contact_log_label(&contact_for_log),
            remote_im_preview_text(&text, 100)
        ),
    );
    if matches!(&ingress, ChatEventIngress::Queued { .. }) {
        trigger_guided_queue_processing(state, &conversation_id);
    } else {
        trigger_chat_event_after_ingress(state, ingress);
    }
    Ok(RemoteImEnqueueResult {
        event_id,
        conversation_id,
        activate_assistant: true,
        contact_id,
    })
}
