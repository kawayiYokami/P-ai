impl WeixinOcManager {
    pub(crate) async fn add_log(&self, channel_id: &str, level: &str, message: &str) {
        self.port_service.add_log(channel_id, level, message).await;
    }

    #[cfg_attr(test, allow(dead_code))]
    async fn set_state<F>(&self, channel_id: &str, update: F)
    where
        F: FnOnce(&mut WeixinOcRuntimeState),
    {
        let (status_text, last_error) = {
            let mut states = self.states.write().await;
            let state = states
                .entry(channel_id.to_string())
                .or_insert_with(WeixinOcRuntimeState::default);
            let was_connected = state.connected;
            update(state);
            if state.connected && !was_connected {
                state.connected_at = Some(chrono::Utc::now());
            }
            if !state.connected {
                state.connected_at = None;
            }
            (
                (!state.login_status.trim().is_empty()).then(|| state.login_status.clone()),
                (!state.last_error.trim().is_empty()).then(|| state.last_error.clone()),
            )
        };
        self.port_service
            .set_status_text(channel_id, status_text)
            .await;
        self.port_service.set_last_error(channel_id, last_error).await;
    }

    async fn load_state_from_channel(&self, state: &AppState, channel: &RemoteImChannelConfig) {
        let credentials = remote_im_effective_credentials(state, channel)
            .unwrap_or_else(|_| channel.credentials.clone());
        let creds = WeixinOcCredentials::from_value(&credentials);
        let private_state =
            remote_im_read_channel_private_state(state, &RemoteImPlatform::WeixinOc, &channel.id)
                .ok();
        self.set_state(&channel.id, |state| {
            state.base_url = creds.normalized_base_url();
            state.account_id = creds.account_id.trim().to_string();
            state.user_id = creds.user_id.trim().to_string();
            if state.login_status == "idle" && !creds.token.trim().is_empty() {
                state.login_status = "logged_in".to_string();
            }
        })
        .await;
        if let Some(private_state) = private_state {
            let channel_prefix = format!("{}:", channel.id.trim());
            let mut context_tokens = self.context_tokens.write().await;
            context_tokens.retain(|key, _| !key.starts_with(&channel_prefix));
            for (user_id, token) in private_state.context_tokens {
                let normalized_user_id = user_id.trim();
                let normalized_token = token.trim();
                if normalized_user_id.is_empty() || normalized_token.is_empty() {
                    continue;
                }
                context_tokens.insert(
                    format!("{}:{}", channel.id.trim(), normalized_user_id),
                    normalized_token.to_string(),
                );
            }
        }
    }

    async fn build_status(&self, channel_id: &str) -> ChannelConnectionStatus {
        let state = self
            .states
            .read()
            .await
            .get(channel_id)
            .cloned()
            .unwrap_or_default();
        ChannelConnectionStatus {
            channel_id: channel_id.to_string(),
            connected: state.connected,
            peer_addr: if state.account_id.trim().is_empty() {
                None
            } else {
                Some(state.account_id.clone())
            },
            connected_at: state.connected_at,
            listen_addr: String::new(),
            status_text: self
                .port_service
                .get_status_text(channel_id)
                .await
                .or_else(|| Some(state.login_status.clone())),
            last_error: self.port_service.get_last_error(channel_id).await.or_else(|| {
                if state.last_error.trim().is_empty() {
                    None
                } else {
                    Some(state.last_error.clone())
                }
            }),
            account_id: if state.account_id.trim().is_empty() {
                None
            } else {
                Some(state.account_id)
            },
            base_url: Some(state.base_url),
            login_session_key: if state.session_key.trim().is_empty() {
                None
            } else {
                Some(state.session_key)
            },
            qrcode_url: if state.qrcode_img_content.trim().is_empty() {
                None
            } else {
                Some(state.qrcode_img_content)
            },
        }
    }

    async fn set_context_token(
        &self,
        state: &AppState,
        channel_id: &str,
        user_id: &str,
        token: &str,
    ) {
        if user_id.trim().is_empty() || token.trim().is_empty() {
            return;
        }
        let key = format!("{}:{}", channel_id.trim(), user_id.trim());
        let normalized_token = token.trim().to_string();
        self.context_tokens
            .write()
            .await
            .insert(key, normalized_token.clone());
        if let Err(err) = remote_im_patch_channel_private_state(
            state,
            &RemoteImPlatform::WeixinOc,
            channel_id,
            |private| {
                private
                    .context_tokens
                    .insert(user_id.trim().to_string(), normalized_token.clone());
            },
        ) {
            runtime_log_error(format!(
                "[远程IM] 持久化 context_token 失败 渠道={} 发送者={} 原因={}",
                channel_id,
                user_id,
                err
            ));
        }
    }

    async fn get_context_token(&self, channel_id: &str, user_id: &str) -> Option<String> {
        self.context_tokens
            .read()
            .await
            .get(&format!("{}:{}", channel_id.trim(), user_id.trim()))
            .cloned()
    }

    async fn stop_channel_inner(&self, channel_id: &str) {
        let started_at = std::time::Instant::now();
        self.add_log(channel_id, "info", "[远程IM] 个人微信 开始停止渠道").await;
        if let Some(tx) = self.stop_senders.write().await.remove(channel_id) {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.tasks.write().await.remove(channel_id) {
            let mut handle = handle;
            tokio::select! {
                join_result = &mut handle => {
                    if let Err(err) = join_result {
                        self.add_log(
                            channel_id,
                            "warn",
                            &format!("[远程IM] 个人微信 停止旧轮询任务失败: {}", err),
                        )
                        .await;
                        runtime_log_error(format!(
                            "[远程IM] 停止旧轮询任务失败 渠道={} 原因={}",
                            channel_id,
                            err
                        ));
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(1500)) => {
                    self.add_log(
                        channel_id,
                        "warn",
                        &format!(
                            "[远程IM] 个人微信 停止旧轮询任务超时，正在中止: elapsed_ms={}",
                            started_at.elapsed().as_millis()
                        ),
                    )
                    .await;
                    runtime_log_error(format!(
                        "[远程IM] 停止旧轮询任务超时，正在中止 渠道={} 耗时={}ms",
                        channel_id,
                        started_at.elapsed().as_millis()
                    ));
                    handle.abort();
                    let _ = handle.await;
                }
            }
        }
        self.stop_all_typing_for_channel(channel_id).await;
        self.set_state(channel_id, |state| {
            state.connected = false;
        })
        .await;
        runtime_log_info(format!(
            "[远程IM] 停止渠道完成 渠道={} 耗时={}ms",
            channel_id,
            started_at.elapsed().as_millis()
        ));
        self.add_log(
            channel_id,
            "info",
            &format!(
                "[远程IM] 个人微信 渠道已停止: elapsed_ms={}",
                started_at.elapsed().as_millis()
            ),
        )
        .await;
    }

    async fn stop_channel(&self, channel_id: &str) {
        let _ = self
            .port_service
            .stop_serialized(channel_id, || async {
                self.stop_channel_inner(channel_id).await;
                Ok(())
            })
            .await;
    }

    pub(crate) async fn start_typing(
        &self,
        channel_id: &str,
        credentials: WeixinOcCredentials,
        ilink_user_id: &str,
        context_token: Option<String>,
    ) {
        let key = format!("{}:{}", channel_id.trim(), ilink_user_id.trim());
        // 先 stop 之前的 typing（同一用户只保留一个）
        self.stop_typing(channel_id, ilink_user_id).await;

        // 没有用户 ID 或没有 context_token 则不支持 typing
        let ctx_token = match context_token {
            Some(ref t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => {
                runtime_log_debug(format!(
                    "[远程IM] 跳过输入状态 原因=缺少 context_token 渠道={} 用户={}",
                    channel_id, ilink_user_id
                ));
                return;
            }
        };

        // 第一步：调用 getconfig 获取 typing_ticket
        let typing_ticket = match weixin_oc_get_typing_config(
            &credentials,
            ilink_user_id,
            Some(ctx_token.as_str()),
        )
        .await
        {
            Ok(ticket) => {
                if ticket.trim().is_empty() {
                    runtime_log_info(format!(
                        "[远程IM] 获取配置返回空输入票据 渠道={} 用户={}",
                        channel_id, ilink_user_id
                    ));
                    return;
                }
                ticket
            }
            Err(err) => {
                runtime_log_error(format!(
                    "[远程IM] 获取配置失败 渠道={} 用户={} 原因={}",
                    channel_id, ilink_user_id, err
                ));
                return;
            }
        };

        // 第二步：调用 sendtyping(status=1) 开始输入
        if let Err(err) = weixin_oc_send_typing(
            &credentials,
            ilink_user_id,
            &typing_ticket,
            1,
        )
        .await
        {
            runtime_log_error(format!(
                "[远程IM] 发送输入状态失败 渠道={} 用户={} 原因={}",
                channel_id, ilink_user_id, err
            ));
            return;
        }

        // 创建 oneshot cancel channel
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let state = WeixinOcTypingState {
            ticket_state: WeixinOcTypingTicketState {
                ilink_user_id: ilink_user_id.trim().to_string(),
                typing_ticket,
                ticket_context_token: ctx_token.clone(),
                ticket_refresh_after: std::time::Instant::now()
                    + std::time::Duration::from_secs(WEIXIN_OC_TYPING_TICKET_TTL_SECS),
            },
            cancel_tx,
        };
        self.typing_states.write().await.insert(key.clone(), state);

        // 启动后台保活任务：每 30 秒刷新一次
        let keepalive_channel_id = channel_id.to_string();
        let keepalive_ilink_user_id = ilink_user_id.trim().to_string();
        let keepalive_credentials = credentials;
        let keepalive_key = key;
        let typing_states = self.typing_states.clone();
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval_at(
                tokio::time::Instant::now() + std::time::Duration::from_secs(30),
                std::time::Duration::from_secs(30),
            );
            let mut cancel_rx = cancel_rx;
            loop {
                tokio::select! {
                    _ = &mut cancel_rx => {
                        // 收到取消信号，调用 sendtyping(status=2) 停止输入
                        // 从 typing_states 获取最新的 ticket
                        let ticket = {
                            typing_states
                                .read()
                                .await
                                .get(&keepalive_key)
                                .map(|s| s.ticket_state.typing_ticket.clone())
                                .unwrap_or_default()
                        };
                        if !ticket.is_empty() {
                            if let Err(err) = weixin_oc_send_typing(
                                &keepalive_credentials,
                                &keepalive_ilink_user_id,
                                &ticket,
                                2,
                            )
                            .await
                            {
                                runtime_log_error(format!(
                                    "[远程IM] 保活任务停止输入状态失败 渠道={} 原因={}",
                                    keepalive_channel_id, err
                                ));
                            }
                        }
                        break;
                    }
                    _ = interval.tick() => {
                        // 保活：确保 ticket 未过期，然后发送 sendtyping(status=1)
                        let state_snapshot = {
                            typing_states
                                .read()
                                .await
                                .get(&keepalive_key)
                                .map(|s| s.ticket_state.clone())
                        };
                        let Some(mut ts) = state_snapshot else {
                            break;
                        };

                        // 如果 ticket 即将过期，重新获取
                        if std::time::Instant::now() >= ts.ticket_refresh_after {
                            match weixin_oc_get_typing_config(
                                &keepalive_credentials,
                                &keepalive_ilink_user_id,
                                Some(ts.ticket_context_token.as_str()),
                            )
                            .await
                            {
                                Ok(new_ticket) if !new_ticket.trim().is_empty() => {
                                    ts.typing_ticket = new_ticket;
                                    ts.ticket_refresh_after = std::time::Instant::now()
                                        + std::time::Duration::from_secs(WEIXIN_OC_TYPING_TICKET_TTL_SECS);
                                    let mut states = typing_states.write().await;
                                    if let Some(existing) = states.get_mut(&keepalive_key) {
                                        existing.ticket_state.typing_ticket = ts.typing_ticket.clone();
                                        existing.ticket_state.ticket_refresh_after = ts.ticket_refresh_after;
                                    }
                                }
                                Ok(_) => {
                                    runtime_log_info(format!(
                                        "[远程IM] 保活获取配置返回空票据 渠道={}",
                                        keepalive_channel_id
                                    ));
                                    continue;
                                }
                                Err(err) => {
                                    runtime_log_error(format!(
                                        "[远程IM] 保活获取配置失败 渠道={} 原因={}",
                                        keepalive_channel_id, err
                                    ));
                                    continue;
                                }
                            }
                        }

                        // 发送 sendtyping(status=1) 保活
                        let current_ticket = {
                            typing_states
                                .read()
                                .await
                                .get(&keepalive_key)
                                .map(|s| s.ticket_state.typing_ticket.clone())
                                .unwrap_or_default()
                        };
                        if !current_ticket.is_empty() {
                            if let Err(err) = weixin_oc_send_typing(
                                &keepalive_credentials,
                                &keepalive_ilink_user_id,
                                &current_ticket,
                                1,
                            )
                            .await
                            {
                                runtime_log_error(format!(
                                    "[远程IM] 保活发送输入状态失败 渠道={} 原因={}",
                                    keepalive_channel_id, err
                                ));
                            }
                        }
                    }
                }
            }
        });
    }

    pub(crate) async fn stop_typing(&self, channel_id: &str, ilink_user_id: &str) {
        let key = format!("{}:{}", channel_id.trim(), ilink_user_id.trim());
        let removed = self.typing_states.write().await.remove(&key);
        if let Some(state) = removed {
            // 通过 cancel channel 通知保活任务调用 sendtyping(status=2)
            let _ = state.cancel_tx.send(());
        }
    }

    async fn stop_all_typing_for_channel(&self, channel_id: &str) {
        let prefix = format!("{}:", channel_id.trim());
        let mut keys_to_remove = Vec::<String>::new();
        {
            let states = self.typing_states.read().await;
            for key in states.keys() {
                if key.starts_with(&prefix) {
                    keys_to_remove.push(key.clone());
                }
            }
        }
        for key in keys_to_remove {
            let removed = self.typing_states.write().await.remove(&key);
            if let Some(state) = removed {
                // 通过 cancel channel 通知保活任务调用 sendtyping(status=2)
                let _ = state.cancel_tx.send(());
            }
        }
    }

    async fn reconcile_channel_runtime(
        &self,
        channel: &RemoteImChannelConfig,
        state: AppState,
    ) -> Result<(), String> {
        self.port_service
            .reconcile_serialized(&channel.id, || async move {
                runtime_log_info(format!(
                    "[远程IM] 校准渠道运行时 渠道={} 启用={} 平台={}",
                    channel.id, channel.enabled, channel.platform.as_str()
                ));
                self.load_state_from_channel(&state, channel).await;
                self.stop_channel_inner(&channel.id).await;
                if channel.platform != RemoteImPlatform::WeixinOc || !channel.enabled {
                    runtime_log_info(format!("[远程IM] 渠道已停用 渠道={}", channel.id));
                    self.set_state(&channel.id, |runtime| {
                        runtime.connected = false;
                        runtime.login_status = "disabled".to_string();
                        runtime.last_error.clear();
                    })
                    .await;
                    self.add_log(&channel.id, "info", "[远程IM] 个人微信 渠道已停用").await;
                    return Ok(());
                }
                let effective_channel = remote_im_channel_with_effective_credentials(&state, channel)?;
                let creds = WeixinOcCredentials::from_value(&effective_channel.credentials);
                runtime_log_info(format!(
                    "[远程IM] 当前凭证 渠道={} 地址={} 令牌长度={} 账号={} 发送者={}",
                    channel.id,
                    creds.normalized_base_url(),
                    creds.token.trim().len(),
                    creds.account_id.trim(),
                    creds.user_id.trim()
                ));
                if creds.token.trim().is_empty() {
                    runtime_log_info(format!("[远程IM] 渠道已启用，但尚未登录（缺少 token） 渠道={}", channel.id));
                    self.set_state(&channel.id, |runtime| {
                        runtime.connected = false;
                        runtime.login_status = "need_login".to_string();
                        runtime.last_error.clear();
                    })
                    .await;
                    self.add_log(&channel.id, "info", "[远程IM] 个人微信 渠道已启用，但尚未登录（缺少 token）")
                        .await;
                    return Ok(());
                }
                runtime_log_debug(format!("[远程IM] 渠道已启用，正在启动轮询 渠道={}", channel.id));
                self.add_log(&channel.id, "info", "[远程IM] 个人微信 渠道已启用，正在启动轮询")
                    .await;
                self.start_channel_inner(effective_channel, state).await
            })
            .await
    }

    async fn start_channel_inner(
        &self,
        channel: RemoteImChannelConfig,
        state: AppState,
    ) -> Result<(), String> {
        let channel_id = channel.id.clone();
        runtime_log_info(format!("[远程IM] 启动渠道 渠道={}", channel_id));
        self.add_log(&channel_id, "info", "[远程IM] 个人微信 开始启动渠道").await;
        self.stop_channel_inner(&channel_id).await;
        runtime_log_info(format!("[远程IM] 启动渠道前停止旧任务完成 渠道={}", channel_id));
        self.add_log(&channel_id, "info", "[远程IM] 个人微信 旧轮询任务已停止，准备创建新任务")
            .await;
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        self.stop_senders
            .write()
            .await
            .insert(channel_id.clone(), stop_tx);
        let manager = weixin_oc_manager();
        let task_channel_id = channel_id.clone();
        let handle = tauri::async_runtime::spawn(async move {
            manager
                .add_log(&task_channel_id, "info", "[远程IM] 个人微信 轮询任务开始")
                .await;
            let task_credentials = WeixinOcCredentials::from_value(&channel.credentials);
            if let Err(err) = weixin_oc_notify_start(&task_credentials).await {
                runtime_log_warn(format!(
                    "[远程IM] 通知启动失败（忽略，不阻塞启动） 渠道={} 原因={}",
                    task_channel_id, err
                ));
            }
            let mut stop_rx = stop_rx;
            loop {
                if *stop_rx.borrow() {
                    break;
                }
                let result = tokio::select! {
                    changed = stop_rx.changed() => {
                        match changed {
                            Ok(()) => {
                                if *stop_rx.borrow() {
                                    break;
                                }
                                continue;
                            }
                            Err(_) => break,
                        }
                    }
                    ret = run_single_weixin_oc_poll_cycle(&channel.id, &state) => ret,
                };
                if let Err(err) = result {
                    manager
                        .set_state(&task_channel_id, |runtime| {
                            runtime.connected = false;
                            runtime.last_error = err.clone();
                        })
                        .await;
                    manager
                        .add_log(
                            &task_channel_id,
                            "warn",
                            &format!("[远程IM] 个人微信 拉取消息失败: {}", err),
                        )
                        .await;
                    tokio::select! {
                        changed = stop_rx.changed() => {
                            match changed {
                                Ok(()) => {
                                    if *stop_rx.borrow() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {}
                    }
                } else {
                    manager
                        .set_state(&task_channel_id, |runtime| {
                            runtime.connected = true;
                            if runtime.login_status.trim().is_empty() || runtime.login_status == "need_login" {
                                runtime.login_status = "logged_in".to_string();
                            }
                        })
                        .await;
                }
            }
            manager
                .set_state(&task_channel_id, |runtime| {
                    runtime.connected = false;
                })
                .await;
            manager
                .add_log(&task_channel_id, "info", "[远程IM] 个人微信 轮询任务结束")
                .await;
            if let Err(err) = weixin_oc_notify_stop(&task_credentials).await {
                runtime_log_warn(format!(
                    "[远程IM] 通知停止失败（忽略） 渠道={} 原因={}",
                    task_channel_id, err
                ));
            }
        });
        self.tasks.write().await.insert(channel_id, handle);
        Ok(())
    }

    #[allow(dead_code)]
    async fn start_channel(
        &self,
        channel: RemoteImChannelConfig,
        state: AppState,
    ) -> Result<(), String> {
        let service_id = channel.id.clone();
        self.port_service
            .restart_serialized(&service_id, || async move { self.start_channel_inner(channel, state).await })
            .await
    }

    pub(crate) async fn get_logs(&self, channel_id: &str) -> Vec<ChannelLogEntry> {
        self.port_service.get_logs(channel_id).await
    }
}
