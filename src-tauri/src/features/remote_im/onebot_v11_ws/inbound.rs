fn build_remote_im_enqueue_input(
    channel_id: &str,
    sender_name: String,
    sender_id: String,
    im_name: String,
    remote_contact_type: String,
    remote_contact_id: String,
    remote_contact_name: Option<String>,
    platform_message_id: Option<String>,
    final_text: String,
    ordered_parts: Vec<ChatIngressPart>,
) -> RemoteImEnqueueInput {
    RemoteImEnqueueInput {
        channel_id: channel_id.to_string(),
        platform: RemoteImPlatform::OnebotV11,
        im_name,
        remote_contact_type,
        remote_contact_id,
        remote_contact_name,
        sender_id,
        sender_name,
        sender_avatar_url: None,
        platform_message_id,
        dingtalk_session_webhook: None,
        dingtalk_session_webhook_expired_time: None,
        session: SessionSelector {
            api_config_id: None,
            agent_id: String::new(),
            conversation_id: None,
        },
        payload: ChatInputPayload {
            text: Some(final_text),
            display_text: None,
            parts: if ordered_parts.is_empty() { None } else { Some(ordered_parts) },
            images: None,
            audios: None,
            attachments: None,
            model: None,
            extra_text_blocks: None,
            mentions: None,
            provider_meta: None,
        },
    }
}

/// 解析 OneBot v11 message 事件并入队
async fn parse_and_enqueue_onebot_event(
    channel_id: &str,
    event: &Value,
    state: &AppState,
    manager: &OnebotV11WsManager,
) -> Result<RemoteImEnqueueResult, String> {
    runtime_log_info(format!(
        "[远程IM][OneBot v11 事件][trace] channel_id={}, message_type={}, user_id={}, message_id={}",
        channel_id,
        event
            .get("message_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown"),
        event
            .get("user_id")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        event
            .get("message_id")
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ));
    let user_id_for_media = onebot_read_u64_like(event, "user_id").unwrap_or(0);
    let group_id = onebot_read_u64_like(event, "group_id");
    let sender_id = onebot_read_id_as_string(event, "user_id").unwrap_or_else(|| user_id_for_media.to_string());
    let sender_name = resolve_sender_name(event);
    let (remote_contact_type, remote_contact_id, mut remote_contact_name) =
        resolve_contact_info(event, manager, channel_id).await?;
    if remote_contact_type != "group" {
        remote_contact_name = Some(sender_name.clone());
    }
    let channel_config = read_channel_config(state, channel_id)?;
    let im_name = channel_config
        .as_ref()
        .map(|ch| ch.name.clone())
        .unwrap_or_else(|| "OneBot v11".to_string());
    let mut group_member_cache = if remote_contact_type == "group" {
        onebot_group_member_cache_for_contact(state, channel_id, group_id)
    } else {
        std::collections::HashMap::new()
    };
    if remote_contact_type == "group" {
        let updated_at = now_iso();
        if let Some(member) =
            onebot_group_member_info_from_event_sender(event, &sender_name, &updated_at)
        {
            onebot_merge_group_member_cache_entry(&mut group_member_cache, member);
        }
    }
    let message_field = event.get("message");
    let parsed = extract_message_content_detail(event);
    let mention_refs = parsed.mention_refs;
    let mut ordered_segments = parsed.ordered_segments;
    if ordered_segments.is_empty() {
        if !parsed.text.is_empty() {
            ordered_segments.push(OnebotParsedSegment::Text(parsed.text));
        }
        ordered_segments.extend(parsed.media_refs.into_iter().map(OnebotParsedSegment::Media));
        ordered_segments.extend(
            parsed
                .embedded_refs
                .into_iter()
                .map(OnebotParsedSegment::Embedded),
        );
    }
    let mut ordered_parts = Vec::<ChatIngressPart>::new();
    for segment in ordered_segments {
        match segment {
            OnebotParsedSegment::Text(text) => {
                let text = onebot_resolve_mentions_in_text(
                    manager,
                    channel_id,
                    group_id,
                    text,
                    &mention_refs,
                    &mut group_member_cache,
                )
                .await;
                if !text.trim().is_empty() {
                    ordered_parts.push(ChatIngressPart::Text { text });
                }
            }
            OnebotParsedSegment::Media(media_ref) => {
                let (mut parts, notices) = onebot_resolve_inbound_media(
                    manager,
                    channel_id,
                    group_id,
                    Some(user_id_for_media),
                    &[media_ref],
                )
                .await;
                ordered_parts.append(&mut parts);
                ordered_parts.extend(
                    notices
                        .into_iter()
                        .map(|text| ChatIngressPart::Text { text }),
                );
            }
            OnebotParsedSegment::Embedded(embedded_ref) => {
                let (embedded_text, nested_media_refs) = onebot_expand_embedded_content(
                    manager,
                    channel_id,
                    group_id,
                    &mut group_member_cache,
                    &[embedded_ref],
                )
                .await;
                if !embedded_text.trim().is_empty() {
                    ordered_parts.push(ChatIngressPart::Text {
                        text: embedded_text,
                    });
                }
                let (mut parts, notices) = onebot_resolve_inbound_media(
                    manager,
                    channel_id,
                    group_id,
                    Some(user_id_for_media),
                    &nested_media_refs,
                )
                .await;
                ordered_parts.append(&mut parts);
                ordered_parts.extend(
                    notices
                        .into_iter()
                        .map(|text| ChatIngressPart::Text { text }),
                );
            }
        }
    }
    let text = ordered_parts
        .iter()
        .filter_map(|part| match part {
            ChatIngressPart::Text { text } => Some(text.trim()),
            ChatIngressPart::Attachment { .. } => None,
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if ordered_parts.is_empty() {
        return Err(format!(
            "消息内容为空，跳过 (message_type={}, user_id={}, message_field_type={})",
            event
                .get("message_type")
                .and_then(|v| v.as_str())
                .unwrap_or("private"),
            sender_id,
            message_field_kind(message_field)
        ));
    }

    let platform_message_id = onebot_read_id_as_string(event, "message_id");
    let input = build_remote_im_enqueue_input(
        channel_id,
        sender_name,
        sender_id,
        im_name,
        remote_contact_type,
        remote_contact_id,
        remote_contact_name,
        platform_message_id,
        text,
        ordered_parts,
    );
    let result = remote_im_enqueue_message_internal(input, state).await?;
    if !result.conversation_id.trim().is_empty() {
        let group_members = group_member_cache.into_values().collect::<Vec<_>>();
        if let Err(err) =
            onebot_persist_group_member_cache(state, &result.contact_id, group_members)
        {
            runtime_log_warn(format!(
                "[群聊入站] 群成员缓存更新失败，不影响消息入队，contact_id={}，error={}",
                result.contact_id, err
            ));
        }
    }
    Ok(result)
}

async fn napcat_run_event_consumer_loop(
    manager: OnebotV11WsManager,
    channel_id: String,
    state: AppState,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        // 等待连接建立后才能订阅事件
        let (mut event_rx, cancel_token) = loop {
            if *stop_rx.borrow() {
                manager.add_log(&channel_id, "info", "事件消费器收到停止信号").await;
                return;
            }
            if let Some(rx) = manager.subscribe_events(&channel_id).await {
                if let Some(token) = manager.get_channel_cancel_token(&channel_id).await {
                    break (rx, token);
                }
            }
            // 连接尚未建立或渠道已停止，按节流间隔重试
            tokio::select! {
                changed = stop_rx.changed() => {
                    match changed {
                        Ok(()) => {
                            if *stop_rx.borrow() {
                                manager.add_log(&channel_id, "info", "事件消费器收到停止信号").await;
                                return;
                            }
                        }
                        Err(_) => return,
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(NAPCAT_RECONNECT_INTERVAL_SECS)) => {}
            }
        };

        runtime_log_info(format!("[远程IM][OneBot v11 事件] 渠道 {} 开始消费事件", channel_id));
        manager.add_log(&channel_id, "info", "事件消费器已启动").await;

        loop {
            tokio::select! {
                event_result = event_rx.recv() => {
                    match event_result {
                        Ok(event) => {
                            // 只处理 message 事件
                            if event.get("post_type").and_then(|v| v.as_str()) != Some("message") {
                                continue;
                            }

                            match parse_and_enqueue_onebot_event(&channel_id, &event, &state, &manager).await {
                                Ok(result) => {
                                    runtime_log_info(format!("[远程IM][OneBot v11 事件] 渠道 {} 入队成功: 事件ID={}", channel_id, result.event_id));
                                }
                                Err(err) if err.contains("跳过") => {
                                    // 正常跳过（联系人未开启、内容为空等），仅输出调试日志，不写渠道日志
                                    runtime_log_info(format!("[远程IM][OneBot v11 事件] 渠道 {} {}", channel_id, err));
                                }
                                Err(err) => {
                                    runtime_log_error(format!("[远程IM][OneBot v11 事件] 渠道 {} 入队失败: {}", channel_id, err));
                                    manager.add_log(&channel_id, "warn", &format!("消息入队失败: {}", err)).await;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            runtime_log_info(format!("[远程IM][OneBot v11 事件] 渠道 {} 落后 {} 条事件", channel_id, n));
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            runtime_log_info(format!("[远程IM][OneBot v11 事件] 渠道 {} 事件通道关闭", channel_id));
                            break;
                        }
                    }
                }
                changed = stop_rx.changed() => {
                    match changed {
                        Ok(()) => {
                            if *stop_rx.borrow() {
                                runtime_log_info(format!("[远程IM][OneBot v11 事件] 渠道 {} 收到消费器停止信号", channel_id));
                                manager.add_log(&channel_id, "info", "事件消费器已停止").await;
                                return;
                            }
                        }
                        Err(_) => return,
                    }
                }
                _ = cancel_token.cancelled() => {
                    runtime_log_info(format!("[远程IM][OneBot v11 事件] 渠道 {} 收到取消信号，停止事件消费", channel_id));
                    manager.add_log(&channel_id, "info", "事件消费器已停止").await;
                    return; // 渠道已停止，完全退出消费循环
                }
            }
        }

        // 事件通道关闭（客户端断开），按节流间隔等待重连
        runtime_log_info(format!("[远程IM][OneBot v11 事件] 渠道 {} 等待重新连接...", channel_id));
        tokio::select! {
            changed = stop_rx.changed() => {
                match changed {
                    Ok(()) => {
                        if *stop_rx.borrow() {
                            manager.add_log(&channel_id, "info", "事件消费器已停止").await;
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(NAPCAT_RECONNECT_INTERVAL_SECS)) => {}
        }
    }
}

impl OnebotV11WsManager {
    async fn stop_event_consumer_inner(&self, channel_id: &str) -> Result<(), String> {
        self.add_log(channel_id, "info", "开始停止事件消费器").await;
        let stop_sender = {
            self.event_consumer_stop_senders
                .write()
                .await
                .remove(channel_id)
        };
        if let Some(tx) = stop_sender {
            let _ = tx.send(true);
        }
        let handle = { self.event_consumer_tasks.write().await.remove(channel_id) };
        if let Some(handle) = handle {
            let mut handle = handle;
            match tokio::time::timeout(Duration::from_secs(5), &mut handle).await {
                Ok(join_result) => {
                    if let Err(err) = join_result {
                        self.add_log(
                            channel_id,
                            "warn",
                            &format!("停止消费器失败，已移除任务句柄: {}", err),
                        )
                        .await;
                        runtime_log_error(format!(
                            "[远程IM][OneBot v11 事件] 停止消费器失败，已移除任务句柄: channel_id={}, error={}",
                            channel_id, err
                        ));
                    }
                }
                Err(_) => {
                    handle.abort();
                    let _ = handle.await;
                    self.add_log(channel_id, "warn", "停止消费器超时，已强制中止")
                        .await;
                    runtime_log_error(format!(
                        "[远程IM][OneBot v11 事件] 停止消费器超时，已强制中止: {}",
                        channel_id
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn start_event_consumer(
        &self,
        channel_id: String,
        state: AppState,
    ) -> Result<(), String> {
        let service_id = channel_id.clone();
        self.port_service
            .restart_serialized(&service_id, || async move {
                self.add_log(&channel_id, "info", "准备启动事件消费器").await;
                self.stop_event_consumer_inner(&channel_id).await?;
                let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
                self.event_consumer_stop_senders
                    .write()
                    .await
                    .insert(channel_id.clone(), stop_tx);
                let tasks = self.event_consumer_tasks.clone();
                let stop_senders = self.event_consumer_stop_senders.clone();
                let task_channel_id = channel_id.clone();
                let manager = self.clone();
                let handle = tokio::spawn(async move {
                    napcat_run_event_consumer_loop(manager, task_channel_id.clone(), state, stop_rx).await;
                    stop_senders.write().await.remove(&task_channel_id);
                    tasks.write().await.remove(&task_channel_id);
                });
                self.event_consumer_tasks
                    .write()
                    .await
                    .insert(channel_id, handle);
                Ok(())
            })
            .await
    }
}
