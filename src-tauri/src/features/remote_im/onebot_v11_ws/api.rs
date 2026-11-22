impl OnebotV11WsManager {
    /// 戳一戳群成员。
    ///
    /// action 名是 OneBot v11 各实现约定的 `group_poke`（SnowLuma / NapCat /
    /// Lagrange 一致），参数 `group_id` + `user_id`。失败只由调用方记日志，
    /// 不阻断任何业务流程。
    pub async fn send_group_poke(
        &self,
        channel_id: &str,
        group_id: &str,
        user_id: &str,
    ) -> Result<(), String> {
        let group_id = group_id
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("群号不是数字：{group_id}"))?;
        let user_id = user_id
            .trim()
            .parse::<i64>()
            .map_err(|_| format!("群友号不是数字：{user_id}"))?;
        self.call_api(
            channel_id,
            "group_poke",
            serde_json::json!({
                "group_id": group_id,
                "user_id": user_id,
            }),
            5000,
        )
        .await
        .map(|_| ())
    }

    /// 调用 OneBot API 并等待响应
    pub async fn call_api(
        &self,
        channel_id: &str,
        action: &str,
        params: Value,
        timeout_ms: u64,
    ) -> Result<Value, String> {
        self.call_api_classified(channel_id, action, params, timeout_ms)
            .await
            .map_err(|err| err.message)
    }

    async fn call_api_classified(
        &self,
        channel_id: &str,
        action: &str,
        params: Value,
        timeout_ms: u64,
    ) -> Result<Value, RemoteImSdkSendError> {
        let connections = self.connections.read().await;
        let conn = connections
            .get(channel_id)
            .ok_or_else(|| {
                RemoteImSdkSendError::definitely_not_sent(format!(
                    "渠道 {} 未连接",
                    channel_id
                ))
            })?;
        let pending_responses = conn.pending_responses.clone();

        // 生成唯一 echo
        let echo = uuid::Uuid::new_v4().to_string();

        // 创建响应等待通道
        let (tx, rx) = oneshot::channel();
        pending_responses.write().await.insert(echo.clone(), tx);

        // 构建请求
        let request = OneBotApiRequest {
            action: action.to_string(),
            params,
            echo: Some(serde_json::json!(echo.clone())),
        };

        let payload = match serde_json::to_string(&request) {
            Ok(payload) => payload,
            Err(err) => {
                pending_responses.write().await.remove(&echo);
                return Err(RemoteImSdkSendError::definitely_not_sent(format!(
                    "序列化请求失败: {}",
                    err
                )));
            }
        };

        // 发送请求
        if let Err(err) = conn.tx.send(payload) {
            pending_responses.write().await.remove(&echo);
            return Err(RemoteImSdkSendError::definitely_not_sent(format!(
                "发送失败: {}",
                err
            )));
        }

        // 释放连接锁，等待响应
        drop(connections);

        // 等待响应或超时
        let result = tokio::time::timeout(
            Duration::from_millis(timeout_ms),
            rx
        ).await;

        match result {
            Ok(Ok(response)) => {
                if response.status == "ok" {
                    Ok(response.data)
                } else {
                    Err(RemoteImSdkSendError::definitely_not_sent(format!(
                        "API 调用失败: status={}, retcode={}",
                        response.status, response.retcode
                    )))
                }
            }
            Ok(Err(_)) => Err(RemoteImSdkSendError::uncertain("响应通道已关闭")),
            Err(_) => {
                // 超时，清理 pending
                pending_responses.write().await.remove(&echo);
                Err(RemoteImSdkSendError::uncertain(format!(
                    "API 调用超时 ({}ms)",
                    timeout_ms
                )))
            }
        }
    }
}
