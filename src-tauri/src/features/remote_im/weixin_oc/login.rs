impl WeixinOcManager {
    async fn start_login(
        &self,
        state: &AppState,
        input: WeixinOcLoginStartInput,
    ) -> Result<WeixinOcLoginStartResult, String> {
        let config = state_read_config_cached(state)?;
        let channel = remote_im_channel_by_id(&config, &input.channel_id)
            .ok_or_else(|| format!("渠道不存在: {}", input.channel_id))?;
        if channel.platform != RemoteImPlatform::WeixinOc {
            return Err("该渠道不是个人微信渠道".to_string());
        }
        self.load_state_from_channel(channel).await;
        if !input.force_refresh {
            if let Some(existing) = self
                .login_sessions
                .read()
                .await
                .get(&input.channel_id)
                .cloned()
            {
                if login_session_is_fresh(&existing) {
                    return Ok(WeixinOcLoginStartResult {
                        channel_id: input.channel_id,
                        session_key: existing.session_key,
                        qrcode: existing.qrcode,
                        qrcode_img_content: existing.qrcode_img_content,
                        status: existing.status,
                        message: "二维码已就绪，请使用微信扫码。".to_string(),
                    });
                }
            }
        }
        let creds = WeixinOcCredentials::from_value(&channel.credentials);
        let client = build_weixin_oc_http_client(creds.normalized_api_timeout_ms())?;
        let url = format!(
            "{}/ilink/bot/get_bot_qrcode",
            creds.normalized_base_url().trim_end_matches('/')
        );
        let resp = client
            .get(url)
            .query(&[("bot_type", creds.normalized_bot_type())])
            .send()
            .await
            .map_err(|err| format!("请求二维码失败: {err}"))?;
        let status_code = resp.status();
        if !status_code.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("请求二维码失败: status={} body={}", status_code, text));
        }
        let data = resp
            .json::<WeixinOcGetBotQrCodeResp>()
            .await
            .map_err(|err| format!("解析二维码响应失败: {err}"))?;
        let qrcode = data.qrcode.unwrap_or_default().trim().to_string();
        let qrcode_img_content = data
            .qrcode_img_content
            .unwrap_or_default()
            .trim()
            .to_string();
        if qrcode.is_empty() || qrcode_img_content.is_empty() {
            return Err("二维码响应缺少 qrcode 或 qrcode_img_content".to_string());
        }
        let session = WeixinOcLoginSession {
            session_key: Uuid::new_v4().to_string(),
            qrcode: qrcode.clone(),
            qrcode_img_content: qrcode_img_content.clone(),
            started_at: now_iso(),
            status: "wait".to_string(),
            error: String::new(),
        };
        self.login_sessions
            .write()
            .await
            .insert(input.channel_id.clone(), session.clone());
        self.set_state(&input.channel_id, |runtime| {
            runtime.session_key = session.session_key.clone();
            runtime.qrcode = session.qrcode.clone();
            runtime.qrcode_img_content = session.qrcode_img_content.clone();
            runtime.login_status = session.status.clone();
            runtime.last_error.clear();
        })
        .await;
        self.add_log(
            &input.channel_id,
            "info",
            &format!("[个人微信] 已生成扫码二维码: {}", qrcode_img_content),
        )
        .await;
        Ok(WeixinOcLoginStartResult {
            channel_id: input.channel_id,
            session_key: session.session_key,
            qrcode,
            qrcode_img_content,
            status: "wait".to_string(),
            message: "请使用微信扫码登录。".to_string(),
        })
    }

    async fn poll_login_status(
        &self,
        state: &AppState,
        input: WeixinOcLoginStatusInput,
    ) -> Result<WeixinOcLoginStatusResult, String> {
        let mut login = {
            let sessions = self.login_sessions.read().await;
            sessions
                .get(&input.channel_id)
                .cloned()
                .ok_or_else(|| "当前没有进行中的扫码登录".to_string())?
        };
        if !login_session_is_fresh(&login) {
            self.login_sessions.write().await.remove(&input.channel_id);
            self.set_state(&input.channel_id, |runtime| {
                runtime.login_status = "expired".to_string();
                runtime.last_error = "二维码已过期，请重新生成".to_string();
            })
            .await;
            return Ok(WeixinOcLoginStatusResult {
                channel_id: input.channel_id,
                connected: false,
                status: "expired".to_string(),
                message: "二维码已过期，请重新生成。".to_string(),
                session_key: login.session_key,
                qrcode: login.qrcode,
                qrcode_img_content: login.qrcode_img_content,
                account_id: String::new(),
                user_id: String::new(),
                base_url: String::new(),
                last_error: "二维码已过期，请重新生成".to_string(),
            });
        }
        let config = state_read_config_cached(state)?;
        let channel = remote_im_channel_by_id(&config, &input.channel_id)
            .ok_or_else(|| format!("渠道不存在: {}", input.channel_id))?;
        let creds = WeixinOcCredentials::from_value(&channel.credentials);
        let client = build_weixin_oc_http_client(creds.normalized_long_poll_timeout_ms())?;
        let url = format!(
            "{}/ilink/bot/get_qrcode_status",
            creds.normalized_base_url().trim_end_matches('/')
        );
        let resp = client
            .get(url)
            .query(&[("qrcode", login.qrcode.clone())])
            .header("iLink-App-ClientVersion", "1")
            .send()
            .await
            .map_err(|err| format!("查询二维码状态失败: {err}"))?;
        let status_code = resp.status();
        if !status_code.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("查询二维码状态失败: status={} body={}", status_code, text));
        }
        let data = resp
            .json::<WeixinOcQrStatusResp>()
            .await
            .map_err(|err| format!("解析二维码状态失败: {err}"))?;
        if data.ret.unwrap_or(0) != 0 || data.errcode.unwrap_or(0) != 0 {
            return Err(format!(
                "查询二维码状态失败: ret={} errcode={} errmsg={}",
                data.ret.unwrap_or(0),
                data.errcode.unwrap_or(0),
                data.errmsg.unwrap_or_default()
            ));
        }
        let status = data.status.unwrap_or_else(|| "wait".to_string());
        login.status = status.clone();
        if weixin_oc_is_login_confirmed(&status) {
            let bot_token = data.bot_token.unwrap_or_default().trim().to_string();
            let account_id = data.ilink_bot_id.unwrap_or_default().trim().to_string();
            let user_id = data.ilink_user_id.unwrap_or_default().trim().to_string();
            let base_url = data
                .baseurl
                .unwrap_or_else(|| creds.normalized_base_url())
                .trim()
                .trim_end_matches('/')
                .to_string();
            if bot_token.is_empty() || account_id.is_empty() {
                login.error = "扫码已确认，等待凭证返回".to_string();
                self.login_sessions
                    .write()
                    .await
                    .insert(input.channel_id.clone(), login.clone());
                self.set_state(&input.channel_id, |runtime| {
                    runtime.session_key = login.session_key.clone();
                    runtime.qrcode = login.qrcode.clone();
                    runtime.qrcode_img_content = login.qrcode_img_content.clone();
                    runtime.login_status = status.clone();
                    runtime.last_error = login.error.clone();
                })
                .await;
                return Ok(WeixinOcLoginStatusResult {
                    channel_id: input.channel_id,
                    connected: false,
                    status,
                    message: "扫码已确认，等待微信返回凭证。".to_string(),
                    session_key: login.session_key,
                    qrcode: login.qrcode,
                    qrcode_img_content: login.qrcode_img_content,
                    account_id: String::new(),
                    user_id: String::new(),
                    base_url: creds.normalized_base_url(),
                    last_error: login.error,
                });
            }
            let updated_channel = {
                let guard = state
                    .conversation_lock
                    .lock()
                    .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
                let mut writable = state_read_config_cached(state)?;
                let writable_channel = writable
                    .remote_im_channels
                    .iter_mut()
                    .find(|item| item.id == input.channel_id)
                    .ok_or_else(|| format!("渠道不存在: {}", input.channel_id))?;
                let mut writable_creds = WeixinOcCredentials::from_value(&writable_channel.credentials);
                writable_creds.token = bot_token.clone();
                writable_creds.account_id = account_id.clone();
                writable_creds.user_id = user_id.clone();
                writable_creds.base_url = base_url.clone();
                writable_channel.credentials = serde_json::to_value(&writable_creds)
                    .map_err(|err| format!("序列化个人微信凭证失败: {err}"))?;
                let updated_channel = writable_channel.clone();
                state_write_config_cached(state, &writable)?;
                drop(guard);
                updated_channel
            };
            self.login_sessions.write().await.remove(&input.channel_id);
            self.set_state(&input.channel_id, |runtime| {
                runtime.connected = false;
                runtime.account_id = account_id.clone();
                runtime.user_id = user_id.clone();
                runtime.base_url = base_url.clone();
                runtime.login_status = "confirmed".to_string();
                runtime.last_error.clear();
                runtime.session_key.clear();
                runtime.qrcode.clear();
                runtime.qrcode_img_content.clear();
            })
            .await;
            self.add_log(
                &input.channel_id,
                "info",
                &format!(
                    "[个人微信] 扫码登录成功: account_id={}, user_id={}",
                    account_id, user_id
                ),
            )
            .await;
            if !user_id.is_empty() {
                let (_, created) = sync_weixin_oc_contact_from_user_id(state, channel, &user_id)?;
                let log_message = if created {
                    format!("[个人微信] 已自动补录联系人: {}", user_id)
                } else {
                    format!("[个人微信] 联系人已存在，跳过补录: {}", user_id)
                };
                self.add_log(&input.channel_id, "info", &log_message).await;
            }
            if updated_channel.enabled {
                self.reconcile_channel_runtime(&updated_channel, state.clone()).await?;
            }
            return Ok(WeixinOcLoginStatusResult {
                channel_id: input.channel_id,
                connected: true,
                status: "confirmed".to_string(),
                message: "扫码登录成功。".to_string(),
                session_key: String::new(),
                qrcode: String::new(),
                qrcode_img_content: String::new(),
                account_id,
                user_id,
                base_url,
                last_error: String::new(),
            });
        }
        if status == "expired" {
            self.login_sessions.write().await.remove(&input.channel_id);
            self.set_state(&input.channel_id, |runtime| {
                runtime.login_status = "expired".to_string();
                runtime.last_error = "二维码已过期，请重新生成".to_string();
            })
            .await;
            return Ok(WeixinOcLoginStatusResult {
                channel_id: input.channel_id,
                connected: false,
                status,
                message: "二维码已过期，请重新生成。".to_string(),
                session_key: login.session_key,
                qrcode: login.qrcode,
                qrcode_img_content: login.qrcode_img_content,
                account_id: String::new(),
                user_id: String::new(),
                base_url: creds.normalized_base_url(),
                last_error: "二维码已过期，请重新生成".to_string(),
            });
        }
        self.login_sessions
            .write()
            .await
            .insert(input.channel_id.clone(), login.clone());
        self.set_state(&input.channel_id, |runtime| {
            runtime.session_key = login.session_key.clone();
            runtime.qrcode = login.qrcode.clone();
            runtime.qrcode_img_content = login.qrcode_img_content.clone();
            runtime.login_status = status.clone();
            runtime.last_error = login.error.clone();
        })
        .await;
        Ok(WeixinOcLoginStatusResult {
            channel_id: input.channel_id,
            connected: false,
            status,
            message: "等待扫码确认。".to_string(),
            session_key: login.session_key,
            qrcode: login.qrcode,
            qrcode_img_content: login.qrcode_img_content,
            account_id: String::new(),
            user_id: String::new(),
            base_url: creds.normalized_base_url(),
            last_error: login.error,
        })
    }

    async fn logout(&self, state: &AppState, channel_id: &str) -> Result<(), String> {
        self.stop_channel(channel_id).await;
        self.login_sessions.write().await.remove(channel_id);
        {
            let guard = state
                .conversation_lock
                .lock()
                .map_err(|err| state_lock_error_with_panic(file!(), line!(), module_path!(), &err))?;
            let mut writable = state_read_config_cached(state)?;
            let channel = writable
                .remote_im_channels
                .iter_mut()
                .find(|item| item.id == channel_id)
                .ok_or_else(|| format!("渠道不存在: {}", channel_id))?;
            let mut creds = WeixinOcCredentials::from_value(&channel.credentials);
            creds.token.clear();
            creds.account_id.clear();
            creds.user_id.clear();
            creds.sync_buf.clear();
            channel.credentials = serde_json::to_value(&creds)
                .map_err(|err| format!("序列化个人微信凭证失败: {err}"))?;
            state_write_config_cached(state, &writable)?;
            drop(guard);
        }
        self.set_state(channel_id, |runtime| {
            *runtime = WeixinOcRuntimeState::default();
            runtime.login_status = "logged_out".to_string();
        })
        .await;
        self.add_log(channel_id, "info", "[个人微信] 已退出登录").await;
        Ok(())
    }
}


