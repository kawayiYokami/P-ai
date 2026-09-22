#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
    show_window(&app, "main")
}

#[tauri::command]
fn show_chat_window(app: AppHandle) -> Result<(), String> {
    show_window(&app, "chat")
}

#[tauri::command]
fn show_archives_window(app: AppHandle) -> Result<(), String> {
    show_window(&app, "archives")
}

#[tauri::command]
fn open_runtime_logs_window(app: AppHandle) -> Result<(), String> {
    show_runtime_logs_window(&app)
}

#[tauri::command]
fn hide_current_window(window: tauri::Window) -> Result<(), String> {
    window.hide().map_err(|err| format!("隐藏当前窗口失败：{err}"))
}

#[tauri::command]
fn toggle_current_window_maximize(window: tauri::Window, app: AppHandle) -> Result<bool, String> {
    toggle_window_maximize_with_default_restore(&app, window.label())
}

#[tauri::command]
fn start_current_window_drag(window: tauri::Window, app: AppHandle) -> Result<(), String> {
    start_window_drag_with_default_restore(&app, window.label())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateRecordHotkeyInput {
    record_hotkey: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateRecordBackgroundWakeInput {
    enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecordHotkeyUpdateResult {
    record_hotkey: String,
    record_background_wake_enabled: bool,
    min_record_seconds: u32,
    max_record_seconds: u32,
}

#[tauri::command]
fn set_chat_window_active(active: bool) {
    static CHAT_WINDOW_INACTIVE_LOGGED_ONCE: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    if !active && !CHAT_WINDOW_INACTIVE_LOGGED_ONCE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        runtime_log_warn(format!("[系统] 聊天窗口激活状态变更：跳过"));
    }
    set_record_hotkey_probe_chat_window_active(active);
}

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppRuntimeInfo {
    app_root: String,
    data_path: String,
    config_path: String,
    is_portable: bool,
}

#[tauri::command]
fn get_app_runtime_info(state: State<'_, AppState>) -> Result<AppRuntimeInfo, String> {
    let data_path = state.data_path.clone();
    let config_path = state.config_path.clone();
    let app_root = app_root_from_data_path(&data_path);
    let is_portable = detect_portable_runtime_root().is_some();
    Ok(AppRuntimeInfo {
        app_root: app_root.to_string_lossy().to_string(),
        data_path: data_path.to_string_lossy().to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        is_portable,
    })
}

fn parse_version_parts(input: &str) -> Vec<u64> {
    let cleaned = input
        .trim()
        .trim_start_matches(['v', 'V'])
        .split(['-', '+'])
        .next()
        .unwrap_or_default();
    cleaned
        .split('.')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

fn config_provider_domain_changed(old_config: &AppConfig, new_config: &AppConfig) -> bool {
    let old_providers = serde_json::to_string(&old_config.api_providers).unwrap_or_default();
    let new_providers = serde_json::to_string(&new_config.api_providers).unwrap_or_default();
    let old_api_configs = serde_json::to_string(&old_config.api_configs).unwrap_or_default();
    let new_api_configs = serde_json::to_string(&new_config.api_configs).unwrap_or_default();
    old_providers != new_providers
        || old_api_configs != new_api_configs
        || old_config.expert_api_config_id
            != new_config.expert_api_config_id
        || old_config.tool_review_api_config_id != new_config.tool_review_api_config_id
        || old_config.selected_api_config_id != new_config.selected_api_config_id
}

fn ide_chat_broadcast_simple_notification(method: &str) {
    ide_chat_broadcast_notification(method, serde_json::json!({}));
}

fn broadcast_sidebar_persona_changed() {
    ide_chat_broadcast_simple_notification("persona.changed");
}

fn broadcast_sidebar_provider_changed() {
    ide_chat_broadcast_simple_notification("provider.changed");
}

fn runtime_config_with_private_organization(
    state: &AppState,
    config: &AppConfig,
    data: &AppData,
) -> Result<AppConfig, String> {
    build_runtime_organization_snapshot_from_parts(&state.data_path, config, &data.agents)
        .map(|snapshot| snapshot.config)
}

fn runtime_agents_with_private_organization(
    state: &AppState,
    config: &AppConfig,
    data: &AppData,
) -> Result<Vec<AgentProfile>, String> {
    build_runtime_organization_snapshot_from_parts(&state.data_path, config, &data.agents)
        .map(|snapshot| snapshot.agents)
}

fn private_agent_operation_error(agent_id: &str) -> String {
    format!("当前人格来自私有工作区，不能直接在主配置中修改：{agent_id}")
}

fn is_newer_version(current: &str, latest: &str) -> bool {
    let a = parse_version_parts(current);
    let b = parse_version_parts(latest);
    let max_len = a.len().max(b.len());
    for idx in 0..max_len {
        let av = *a.get(idx).unwrap_or(&0);
        let bv = *b.get(idx).unwrap_or(&0);
        if bv > av {
            return true;
        }
        if bv < av {
            return false;
        }
    }
    false
}

const GITHUB_REPO_PAGE: &str = "https://github.com/kawayiYokami/P-ai";

fn set_preferred_release_source(state: &AppState, source: &str) {
    match state.preferred_release_source.lock() {
        Ok(mut slot) => {
            *slot = source.to_string();
        }
        Err(err) => {
            runtime_log_error(format!(
                "set_preferred_release_source 锁定 preferred_release_source 失败：source={}, err={}",
                source,
                err
            ));
        }
    }
}

async fn probe_release_source_once(state: &AppState) {
    set_preferred_release_source(state, "github");
}

#[tauri::command]
fn get_project_repository_url(_state: State<'_, AppState>) -> String {
    GITHUB_REPO_PAGE.to_string()
}

#[tauri::command]
fn set_github_update_method(
    update_method: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    set_github_update_method_inner(update_method, &app, state.inner())
}

fn set_github_update_method_inner(
    update_method: String,
    app: &AppHandle,
    state: &AppState,
) -> Result<AppConfig, String> {
    let normalized = normalize_github_update_method(&update_method);
    let mut config = state_read_config_cached(state)?;
    normalize_app_config(&mut config);
    if config.github_update_method != normalized {
        config.github_update_method = normalized.clone();
        state_write_config_cached(state, &config)?;
        runtime_log_info(format!("[自动更新] 更新方式偏好已保存：method={normalized}"));
    }
    let agents = state_read_agents_cached(state)?;
    let mut data = AppData::default();
    data.agents = agents;
    let runtime_config = runtime_config_with_private_organization(state, &config, &data)?;
    let _ = app.emit("easy-call:config-updated", &runtime_config);
    Ok(runtime_config)
}

#[tauri::command]
fn set_skipped_github_update_version(
    version: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    set_skipped_github_update_version_inner(version, &app, state.inner())
}

fn set_skipped_github_update_version_inner(
    version: String,
    app: &AppHandle,
    state: &AppState,
) -> Result<AppConfig, String> {
    let normalized = normalize_skipped_github_update_version(&version);
    let mut config = state_read_config_cached(state)?;
    normalize_app_config(&mut config);
    if config.skipped_github_update_version != normalized {
        config.skipped_github_update_version = normalized.clone();
        state_write_config_cached(state, &config)?;
        runtime_log_warn(format!("[自动更新] 已保存跳过版本：version={normalized}"));
    }
    sync_update_state_from_skip_version(app, &normalized);
    let agents = state_read_agents_cached(state)?;
    let mut data = AppData::default();
    data.agents = agents;
    let runtime_config = runtime_config_with_private_organization(state, &config, &data)?;
    let _ = app.emit("easy-call:config-updated", &runtime_config);
    Ok(runtime_config)
}

fn normalize_ui_language(value: &str) -> String {
    match value.trim() {
        "en-US" => "en-US".to_string(),
        "zh-TW" => "zh-TW".to_string(),
        _ => "zh-CN".to_string(),
    }
}

#[tauri::command]
fn set_ui_language(
    ui_language: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AppConfig, String> {
    set_ui_language_inner(ui_language, &app, state.inner())
}

fn set_ui_language_inner(
    ui_language: String,
    app: &AppHandle,
    state: &AppState,
) -> Result<AppConfig, String> {
    let normalized = normalize_ui_language(&ui_language);
    let mut config = state_read_config_cached(state)?;
    normalize_app_config(&mut config);
    if config.ui_language != normalized {
        config.ui_language = normalized.clone();
        state_write_config_cached(state, &config)?;
        runtime_log_info(format!("[配置] 界面语言已保存：ui_language={normalized}"));
    }
    let agents = state_read_agents_cached(state)?;
    let mut data = AppData::default();
    data.agents = agents;
    let runtime_config = runtime_config_with_private_organization(state, &config, &data)?;
    let _ = app.emit("easy-call:config-updated", &runtime_config);
    Ok(runtime_config)
}

#[tauri::command]
fn load_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    load_config_inner(&state)
}

fn load_config_inner(state: &AppState) -> Result<AppConfig, String> {
    require_message_store_migration_completed_for_runtime(state, "加载应用配置")?;
    let mut result = state_read_config_cached(&state)?;
    normalize_app_config(&mut result);
    // 数据迁移会读取 app_config.toml 里的旧结构（旧键、旧 departments）并直接改写磁盘，
    // 而任何一次配置落盘都会丢掉这些旧结构。因此迁移必须先于下面的补齐写盘执行；
    // 这一步的补齐只改内存，仅作为迁移的上下文，落盘放到迁移之后。
    let _ = ensure_default_shell_workspace_in_config(&mut result, &state);
    let _ = run_app_data_migrations_with_state(&state, &result)?;
    // 迁移可能已改写磁盘配置，按 mtime 重新读取，避免把迁移前的旧快照回写覆盖。
    let mut result = state_read_config_cached(&state)?;
    normalize_app_config(&mut result);
    let workspace_changed = ensure_default_shell_workspace_in_config(&mut result, &state);
    let remote_im_private_state_migrated =
        remote_im_migrate_channel_private_states(&state, &mut result)?;
    if workspace_changed || remote_im_private_state_migrated {
        state_write_config_cached(&state, &result)?;
    }
    // 无可用 LLM 时强制进入简单设置模式，方便首次启动用户直接配置供应商。
    if !has_usable_text_llm(&result) {
        result.simple_setup_mode = true;
    }
    let runtime_agents = state_read_agents_cached(&state)?;
    let snapshot =
        build_runtime_organization_snapshot_from_parts(&state.data_path, &result, &runtime_agents)?;
    Ok(snapshot.config)
}

fn read_app_bootstrap_snapshot(state: &AppState) -> Result<AppBootstrapSnapshot, String> {
    require_message_store_migration_completed_for_runtime(state, "加载应用启动快照")?;
    let mut config = state_read_config_cached(state)?;
    normalize_app_config(&mut config);
    // 同 load_config_inner：迁移必须早于任何配置落盘，否则旧键与旧 departments 会被新快照抹掉。
    let _ = ensure_default_shell_workspace_in_config(&mut config, state);
    let _ = run_app_data_migrations_with_state(state, &config)?;
    let mut config = state_read_config_cached(state)?;
    normalize_app_config(&mut config);
    let workspace_changed = ensure_default_shell_workspace_in_config(&mut config, state);
    let remote_im_private_state_migrated =
        remote_im_migrate_channel_private_states(state, &mut config)?;
    if workspace_changed || remote_im_private_state_migrated {
        state_write_config_cached(state, &config)?;
    }
    // 无可用 LLM 时强制进入简单设置模式，方便首次启动用户直接配置供应商。
    if !has_usable_text_llm(&config) {
        config.simple_setup_mode = true;
    }
    // 启动快照阶段修复会话总索引，避免旧版本误删归档入口后仍需人工恢复。
    let _ = ensure_system_notification_conversation_shard(&state.data_path)?;
    let _ = state_read_chat_index_cached(state)?;
    let agents = state_read_agents_cached(state)?;
    let runtime_snapshot =
        build_runtime_organization_snapshot_from_parts(&state.data_path, &config, &agents)?;
    let mut runtime_data = AppData::default();
    runtime_data.agents = runtime_snapshot.agents.clone();
    let chat_settings = ChatSettings {
        assistant_agent_id: state_service_get_assistant_agent_id(state)?,
        user_alias: user_persona_name(&runtime_data),
        response_style_id: state_service_get_response_style_id(state)?,
        pdf_read_mode: state_service_get_pdf_read_mode(state)?,
        background_voice_screenshot_keywords: state_service_get_background_voice_screenshot_keywords(state)?,
        background_voice_screenshot_mode: state_service_get_background_voice_screenshot_mode(state)?,
        instruction_presets: state_service_get_instruction_presets(state)?,
    };
    Ok(AppBootstrapSnapshot {
        config: runtime_snapshot.config,
        agents: runtime_data.agents,
        chat_settings,
    })
}

#[tauri::command]
fn load_app_bootstrap_snapshot(state: State<'_, AppState>) -> Result<AppBootstrapSnapshot, String> {
    read_app_bootstrap_snapshot(&state)
}

#[tauri::command]
fn is_backend_ready(state: State<'_, AppState>) -> bool {
    state.backend_ready.load(std::sync::atomic::Ordering::Acquire)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SystemFontInfo {
    family: String,
    monospace: bool,
}

/// 系统字体列表进程内缓存：字体安装状态几乎不变，避免每次打开设置页都重新枚举+加载字形。
static SYSTEM_FONTS_CACHE: std::sync::OnceLock<std::sync::Mutex<Option<Vec<SystemFontInfo>>>> =
    std::sync::OnceLock::new();

/// 枚举并分类系统字体（耗时操作：数百字体族逐一加载字形判断等宽）。
fn enumerate_system_fonts() -> Result<Vec<SystemFontInfo>, String> {
    let source = font_kit::source::SystemSource::new();
    let mut families = source
        .all_families()
        .map_err(|err| format!("列出系统字体失败：{err}"))?;
    families.sort_by_key(|name| name.to_ascii_lowercase());
    families.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    let mut result = Vec::with_capacity(families.len());
    for family in families {
        // 取该族第一个字形判断等宽属性；加载失败时保守视为非等宽，避免漏列字体
        let handle = source
            .select_family_by_name(&family)
            .ok()
            .and_then(|fh| fh.fonts().first().cloned());
        let monospace = handle
            .as_ref()
            .and_then(|handle| handle.load().ok())
            .map(|font| font.is_monospace())
            .unwrap_or(false);
        result.push(SystemFontInfo {
            family,
            monospace,
        });
    }
    Ok(result)
}

#[tauri::command]
async fn list_system_fonts() -> Result<Vec<SystemFontInfo>, String> {
    // 命中缓存直接返回，避免重复枚举；miss 时把重活丢到阻塞线程池，不占 IPC 线程。
    let cache = SYSTEM_FONTS_CACHE.get_or_init(|| std::sync::Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some(cached) = guard.as_ref() {
            return Ok(cached.clone());
        }
    }
    let result = tauri::async_runtime::spawn_blocking(enumerate_system_fonts)
        .await
        .map_err(|err| format!("枚举系统字体任务失败：{err}"))?
        .map_err(|err| err.to_string())?;
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(result.clone());
    }
    Ok(result)
}

fn validate_record_hotkey_available(config: &AppConfig) -> Result<String, String> {
    let normalized = normalize_record_hotkey_label(&config.record_hotkey)?;
    if normalized.is_empty() {
        return Ok(normalized);
    }
    let record_signature = record_hotkey_signature(&normalized)
        .ok_or_else(|| format!("录音热键格式无效：{}", normalized))?;
    let summon_signature = record_hotkey_signature(&config.hotkey);
    if summon_signature.as_deref() == Some(record_signature.as_str()) {
        return Err(format!(
            "录音热键 {} 不能与呼唤热键 {} 相同。",
            normalized, config.hotkey
        ));
    }
    Ok(normalized)
}

#[tauri::command]
fn update_record_hotkey(
    input: UpdateRecordHotkeyInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RecordHotkeyUpdateResult, String> {
    let mut config = state_read_config_cached(&state)?;
    normalize_app_config(&mut config);
    let candidate = input.record_hotkey.trim();
    if config.record_hotkey.trim() == candidate {
        let normalized = normalize_record_hotkey_label(&config.record_hotkey)?;
        if normalized != config.record_hotkey {
            config.record_hotkey = normalized.clone();
            state_write_config_cached(&state, &config)?;
            let agents = state_read_agents_cached(&state)?;
    let mut data = AppData::default();
    data.agents = agents;
            let runtime_config = runtime_config_with_private_organization(&state, &config, &data)?;
            let _ = app.emit("easy-call:config-updated", &runtime_config);
        }
        return Ok(RecordHotkeyUpdateResult {
            record_hotkey: config.record_hotkey.clone(),
            record_background_wake_enabled: config.record_background_wake_enabled,
            min_record_seconds: config.min_record_seconds,
            max_record_seconds: config.max_record_seconds,
        });
    }
    let normalized = {
        let mut next = config.clone();
        next.record_hotkey = candidate.to_string();
        validate_record_hotkey_available(&next)?
    };
    config.record_hotkey = normalized.clone();
    state_write_config_cached(&state, &config)?;
    set_record_hotkey_probe_hotkey(&normalized)?;
    let agents = state_read_agents_cached(&state)?;
    let mut data = AppData::default();
    data.agents = agents;
    let runtime_config = runtime_config_with_private_organization(&state, &config, &data)?;
    let _ = app.emit("easy-call:config-updated", &runtime_config);
    Ok(RecordHotkeyUpdateResult {
        record_hotkey: normalized,
        record_background_wake_enabled: config.record_background_wake_enabled,
        min_record_seconds: config.min_record_seconds,
        max_record_seconds: config.max_record_seconds,
    })
}

#[tauri::command]
fn update_record_background_wake(
    input: UpdateRecordBackgroundWakeInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<RecordHotkeyUpdateResult, String> {
    let mut config = state_read_config_cached(&state)?;
    normalize_app_config(&mut config);
    let next_enabled = input.enabled;
    if config.record_background_wake_enabled != next_enabled {
        config.record_background_wake_enabled = next_enabled;
        state_write_config_cached(&state, &config)?;
    }
    set_record_hotkey_probe_background_wake_enabled(config.record_background_wake_enabled);
    runtime_log_info(format!(
        "[录音热键] 完成，任务=后台唤醒切换，enabled={}",
        config.record_background_wake_enabled
    ));
    let agents = state_read_agents_cached(&state)?;
    let mut data = AppData::default();
    data.agents = agents;
    let runtime_config = runtime_config_with_private_organization(&state, &config, &data)?;
    let _ = app.emit("easy-call:config-updated", &runtime_config);
    Ok(RecordHotkeyUpdateResult {
        record_hotkey: config.record_hotkey.clone(),
        record_background_wake_enabled: config.record_background_wake_enabled,
        min_record_seconds: config.min_record_seconds,
        max_record_seconds: config.max_record_seconds,
    })
}

#[tauri::command]
fn save_config(
    config: AppConfig,
    app: AppHandle,
    state: State<'_, AppState>,
    ide_context_runtime: State<'_, IdeContextRuntime>,
) -> Result<SaveConfigOutput, String> {
    save_config_inner(config, &app, &state, &ide_context_runtime)
}

fn removed_remote_im_channels(
    previous: &AppConfig,
    next: &AppConfig,
) -> Vec<RemoteImChannelConfig> {
    let next_ids = next
        .remote_im_channels
        .iter()
        .map(|channel| channel.id.trim().to_ascii_lowercase())
        .collect::<std::collections::HashSet<_>>();
    previous
        .remote_im_channels
        .iter()
        .filter(|channel| !next_ids.contains(&channel.id.trim().to_ascii_lowercase()))
        .cloned()
        .collect()
}

fn stop_removed_remote_im_channel_runtimes(
    state: AppState,
    channels: Vec<RemoteImChannelConfig>,
) {
    if channels.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        for channel in channels {
            let channel_id = channel.id.trim().to_string();
            if channel_id.is_empty() {
                continue;
            }
            match channel.platform {
                RemoteImPlatform::OnebotV11 => {
                    if let Err(err) = onebot_v11_ws_manager().stop_channel(&channel_id).await {
                        runtime_log_error(format!(
                            "[远程IM] 删除渠道后停止 OneBot v11 运行态失败: channel_id={}, error={}",
                            channel_id, err
                        ));
                    }
                }
                RemoteImPlatform::Dingtalk => {
                    dingtalk_stream_manager().stop_channel(&channel_id).await;
                }
                RemoteImPlatform::WeixinOc => {
                    weixin_oc_manager().stop_channel(&channel_id).await;
                }
                RemoteImPlatform::Feishu => {}
            }
            if let Err(err) =
                remote_im_delete_channel_private_state(&state, &channel.platform, &channel_id)
            {
                runtime_log_error(format!(
                    "[远程IM] 删除渠道后清理私有状态失败: channel_id={}, platform={:?}, error={}",
                    channel_id, channel.platform, err
                ));
            }
        }
    });
}

fn save_config_inner(
    config: AppConfig,
    app: &AppHandle,
    state: &AppState,
    ide_context_runtime: &IdeContextRuntime,
) -> Result<SaveConfigOutput, String> {
    if config.api_configs.is_empty() && config.api_providers.is_empty() {
        return Err("至少需要配置一个 API 供应商。".to_string());
    }
    let mut config = config;
    normalize_app_config(&mut config);
    remote_im_migrate_channel_private_states(&state, &mut config)?;
    let _ = ensure_default_shell_workspace_in_config(&mut config, &state);
    set_record_hotkey_probe_background_wake_enabled(config.record_background_wake_enabled);

    let agents = state_read_agents_cached(&state)?;
    let mut data = AppData::default();
    data.agents = agents;
    let base_config = state_read_config_cached(&state)?;
    let removed_remote_im_channels = removed_remote_im_channels(&base_config, &config);
    let provider_changed = config_provider_domain_changed(&base_config, &config);
    let shell_workspaces_changed = base_config.shell_workspaces != config.shell_workspaces;
    let main_config = config.clone();
    state_write_config_cached(&state, &main_config)?;
    if shell_workspaces_changed {
        mark_prompt_cache_rebuild_for_all_system_environments(&state);
    }
    let assistant_workspace_label_synced = if shell_workspaces_changed {
        sync_assistant_workspace_label_for_unarchived_conversations(
            &state,
            &base_config,
            &config,
        )?
    } else {
        0
    };
    if base_config.hotkey != main_config.hotkey {
        if let Err(err) = register_hotkey_from_config(&app, &main_config) {
            runtime_log_error(format!(
                "[热键] 召唤热键运行时注册失败，配置已保存但该热键暂不可用：hotkey={}, err={}",
                main_config.hotkey,
                err
            ));
            return Err(format!(
                "Register hotkey failed: {}, config saved but hotkey inactive. err={}",
                main_config.hotkey, err
            ));
        }
    }
    if !main_config.web_access_enabled && IDE_CONTEXT_BRIDGE_STARTED.load(Ordering::SeqCst) {
        runtime_log_info(format!(
            "[网络访问] 配置已关闭，停止 Web 访问服务: port={}",
            base_config.web_access_port
        ));
        tauri::async_runtime::spawn(async move {
            shutdown_web_access_server().await;
        });
    } else if main_config.web_access_enabled
        && (!base_config.web_access_enabled
            || ((base_config.web_access_port != main_config.web_access_port
                || base_config.web_access_password != main_config.web_access_password)
                && IDE_CONTEXT_BRIDGE_STARTED.load(Ordering::SeqCst)))
    {
        runtime_log_info(format!(
            "[网络访问] 配置已启用、端口或密码已变更，重启 Web 访问服务: old_enabled={}, new_enabled={}, old_port={}, new_port={}",
            base_config.web_access_enabled,
            main_config.web_access_enabled,
            base_config.web_access_port,
            main_config.web_access_port
        ));
        let app = app.clone();
        let state = state.clone();
        let ide_context_runtime = ide_context_runtime.clone();
        tauri::async_runtime::spawn(async move {
            restart_web_access_server(
                app,
                state,
                ide_context_runtime,
            )
            .await;
        });
    }
    let runtime_config = runtime_config_with_private_organization(&state, &main_config, &data)
        .map_err(|err| format!("配置已保存，但运行时配置刷新失败：{err}"))?;
    let _ = app.emit("easy-call:config-updated", &runtime_config);
    if assistant_workspace_label_synced > 0 {
        emit_unarchived_conversation_overview_updated_from_state(&state)?;
    }
    if provider_changed {
        broadcast_sidebar_provider_changed();
    }
    stop_removed_remote_im_channel_runtimes(state.clone(), removed_remote_im_channels);
    Ok(SaveConfigOutput {
        config: runtime_config,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenaiChatAdapterInfo {
    id: String,
    label: String,
    /// 后端 RequestFormat 路由是否已支持该适配器；不支持的只作候选展示，不能直接保存使用。
    supported: bool,
}

/// 暴露 genai 内置的 chat 适配器清单（来源：AdapterKind::all()），
/// 供前端生成文本供应商协议候选，避免手工维护静态列表产生漂移。
#[tauri::command]
fn list_genai_chat_adapters() -> Vec<GenaiChatAdapterInfo> {
    genai::adapter::AdapterKind::all()
        .iter()
        .map(|kind| GenaiChatAdapterInfo {
            id: kind.as_lower_str().to_string(),
            label: kind.as_str().to_string(),
            supported: request_format_from_genai_adapter(*kind).is_some(),
        })
        .collect()
}

/// 项目 RequestFormat 中有对应 chat 变体时返回 Some；无对应路由视为暂不支持。
fn request_format_from_genai_adapter(kind: genai::adapter::AdapterKind) -> Option<RequestFormat> {
    use RequestFormat::*;
    Some(match kind {
        genai::adapter::AdapterKind::OpenAI => OpenAI,
        genai::adapter::AdapterKind::OpenAIResp => OpenAIResponses,
        genai::adapter::AdapterKind::DeepSeek => DeepSeek,
        genai::adapter::AdapterKind::Gemini => Gemini,
        genai::adapter::AdapterKind::Anthropic => Anthropic,
        genai::adapter::AdapterKind::Fireworks => Fireworks,
        genai::adapter::AdapterKind::Together => Together,
        genai::adapter::AdapterKind::Groq => Groq,
        genai::adapter::AdapterKind::Kimi | genai::adapter::AdapterKind::Moonshot => Moonshot,
        genai::adapter::AdapterKind::Mimo => Mimo,
        genai::adapter::AdapterKind::MiniMax => MiniMax,
        genai::adapter::AdapterKind::Nebius => Nebius,
        genai::adapter::AdapterKind::Xai => Xai,
        genai::adapter::AdapterKind::Zai => Zai,
        genai::adapter::AdapterKind::BigModel => BigModel,
        genai::adapter::AdapterKind::Aliyun => Aliyun,
        genai::adapter::AdapterKind::Baidu => Baidu,
        genai::adapter::AdapterKind::Cohere => Cohere,
        genai::adapter::AdapterKind::Ollama => Ollama,
        genai::adapter::AdapterKind::OllamaCloud => OllamaCloud,
        genai::adapter::AdapterKind::Vertex => Vertex,
        genai::adapter::AdapterKind::GithubCopilot => GithubCopilot,
        genai::adapter::AdapterKind::OpenCodeGo => OpenCodeGo,
        genai::adapter::AdapterKind::BedrockApi => BedrockApi,
        // 以下为 genai 新内置但项目 RequestFormat 尚无对应路由的适配器：
        // Aihubmix / QwenCloud / Omlx / OpenRouter / AtlasCloud / MiniMax(已有) /
        // BedrockSigv4(feature-gated) —— 均返回 None，仅候选展示。
        _ => return None,
    })
}
