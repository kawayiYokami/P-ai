impl WeixinOcManager {
    async fn add_log(&self, channel_id: &str, level: &str, message: &str) {
        onebot_v11_ws_manager().add_log(channel_id, level, message).await;
    }

    async fn set_state<F>(&self, channel_id: &str, update: F)
    where
        F: FnOnce(&mut WeixinOcRuntimeState),
    {
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
    }

    async fn load_state_from_channel(&self, channel: &RemoteImChannelConfig) {
        let creds = WeixinOcCredentials::from_value(&channel.credentials);
        self.set_state(&channel.id, |state| {
            state.base_url = creds.normalized_base_url();
            state.account_id = creds.account_id.trim().to_string();
            state.user_id = creds.user_id.trim().to_string();
            if state.login_status == "idle" && !creds.token.trim().is_empty() {
                state.login_status = "logged_in".to_string();
            }
        })
        .await;
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
            status_text: Some(state.login_status),
            last_error: if state.last_error.trim().is_empty() {
                None
            } else {
                Some(state.last_error)
            },
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

    async fn set_context_token(&self, channel_id: &str, user_id: &str, token: &str) {
        if user_id.trim().is_empty() || token.trim().is_empty() {
            return;
        }
        self.context_tokens.write().await.insert(
            format!("{}:{}", channel_id.trim(), user_id.trim()),
            token.trim().to_string(),
        );
    }

    async fn get_context_token(&self, channel_id: &str, user_id: &str) -> Option<String> {
        self.context_tokens
            .read()
            .await
            .get(&format!("{}:{}", channel_id.trim(), user_id.trim()))
            .cloned()
    }

    async fn stop_channel(&self, channel_id: &str) {
        if let Some(tx) = self.stop_senders.write().await.remove(channel_id) {
            let _ = tx.send(true);
        }
        if let Some(handle) = self.tasks.write().await.remove(channel_id) {
            let _ = handle.await;
        }
        self.stop_all_typing_for_channel(channel_id).await;
        self.set_state(channel_id, |state| {
            state.connected = false;
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
                eprintln!(
                    "[个人微信] start_typing 跳过: 缺少 context_token, channel_id={}, ilink_user_id={}",
                    channel_id, ilink_user_id
                );
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
                    eprintln!(
                        "[个人微信] getconfig 返回空 typing_ticket: channel_id={}, ilink_user_id={}",
                        channel_id, ilink_user_id
                    );
                    return;
                }
                ticket
            }
            Err(err) => {
                eprintln!(
                    "[个人微信] getconfig 失败: channel_id={}, ilink_user_id={}, error={}",
                    channel_id, ilink_user_id, err
                );
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
            eprintln!(
                "[个人微信] sendtyping(start) 失败: channel_id={}, ilink_user_id={}, error={}",
                channel_id, ilink_user_id, err
            );
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
                                eprintln!(
                                    "[个人微信] 保活任务 sendtyping(stop) 失败: channel_id={}, error={}",
                                    keepalive_channel_id, err
                                );
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
                                    eprintln!(
                                        "[个人微信] 保活 getconfig 返回空 ticket: channel_id={}",
                                        keepalive_channel_id
                                    );
                                    continue;
                                }
                                Err(err) => {
                                    eprintln!(
                                        "[个人微信] 保活 getconfig 失败: channel_id={}, error={}",
                                        keepalive_channel_id, err
                                    );
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
                                eprintln!(
                                    "[个人微信] 保活 sendtyping(start) 失败: channel_id={}, error={}",
                                    keepalive_channel_id, err
                                );
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
        eprintln!(
            "[个人微信] reconcile_channel_runtime: channel_id={}, enabled={}, platform={:?}",
            channel.id, channel.enabled, channel.platform
        );
        self.load_state_from_channel(channel).await;
        self.stop_channel(&channel.id).await;
        if channel.platform != RemoteImPlatform::WeixinOc || !channel.enabled {
            eprintln!("[个人微信] 渠道已停用: channel_id={}", channel.id);
            self.add_log(&channel.id, "info", "[个人微信] 渠道已停用").await;
            return Ok(());
        }
        let creds = WeixinOcCredentials::from_value(&channel.credentials);
        eprintln!(
            "[个人微信] 当前凭证: channel_id={}, base_url={}, token_len={}, account_id={}, user_id={}",
            channel.id,
            creds.normalized_base_url(),
            creds.token.trim().len(),
            creds.account_id.trim(),
            creds.user_id.trim()
        );
        if creds.token.trim().is_empty() {
            eprintln!("[个人微信] 渠道已启用，但尚未登录（缺少 token）: channel_id={}", channel.id);
            self.set_state(&channel.id, |runtime| {
                runtime.connected = false;
                runtime.login_status = "need_login".to_string();
            })
            .await;
            self.add_log(&channel.id, "info", "[个人微信] 渠道已启用，但尚未登录（缺少 token）")
                .await;
            return Ok(());
        }
        eprintln!("[个人微信] 渠道已启用，正在启动轮询: channel_id={}", channel.id);
        self.add_log(&channel.id, "info", "[个人微信] 渠道已启用，正在启动轮询")
            .await;
        self.start_channel(channel.clone(), state).await
    }

    async fn start_channel(
        &self,
        channel: RemoteImChannelConfig,
        state: AppState,
    ) -> Result<(), String> {
        let channel_id = channel.id.clone();
        eprintln!("[个人微信] start_channel: channel_id={}", channel_id);
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        self.stop_senders
            .write()
            .await
            .insert(channel_id.clone(), stop_tx);
        let manager = weixin_oc_manager();
        let task_channel_id = channel_id.clone();
        let handle = tauri::async_runtime::spawn(async move {
            manager
                .add_log(&task_channel_id, "info", "[个人微信] 轮询任务开始")
                .await;
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
                            &format!("[个人微信] 拉取消息失败: {}", err),
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
                .add_log(&task_channel_id, "info", "[个人微信] 轮询任务结束")
                .await;
        });
        self.tasks.write().await.insert(channel_id, handle);
        Ok(())
    }
}
