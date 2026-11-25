    // ==================== 外部激活与自动化托管 ====================

    /// 托管登记表是进程级静态表，这几个用例必须串行跑，否则互相踩。
    static AUTOMATION_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn automation_test_guard() -> std::sync::MutexGuard<'static, ()> {
        AUTOMATION_TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn automation_test_conversation(state: &AppState) -> String {
        let input = CreateUnarchivedConversationInput {
            api_config_id: None,
            agent_id: None,
            title: Some("自动化测试会话".to_string()),
            copy_source_conversation_id: None,
            shell_workspaces: None,
            shell_work_mode: None,
            shell_work_branch: None,
            shell_autonomous_mode: None,
            conversation_kind: None,
            is_draft: Some(true),
        };
        conversation_service_v2()
            .create_conversation(state, &input)
            .expect("create conversation")
            .conversation_id
    }

    /// 心跳脚本：进程树真被杀掉，文件就不再增长。
    fn automation_heartbeat_command(file_name: &str) -> String {
        // 次数要盖住测试的取样窗口：脚本跑完就退出，之后是 5 秒重启退避，
        // 若循环太短（原来 60 次约 1 秒），窗口里会正好撞上退避期，文件不再增长。
        if cfg!(target_os = "windows") {
            format!(
                "for /L %i in (1,1,600) do @(echo x>>{file_name} & ping -n 1 -w 200 127.0.0.1 >nul)"
            )
        } else {
            format!("for i in $(seq 1 600); do echo x >> {file_name}; sleep 0.2; done")
        }
    }

    /// 把一条原始请求拆成（头文本，请求体字节）。
    fn activation_split_request(request: &str) -> (String, Vec<u8>) {
        match request.find("\r\n\r\n") {
            Some(position) => (
                request[..position].to_string(),
                request[position + 4..].as_bytes().to_vec(),
            ),
            None => (request.to_string(), Vec::new()),
        }
    }

    /// 打一次激活端点，返回原始响应文本。
    fn activation_call_endpoint(
        state: &AppState,
        authorization: Option<&str>,
        body: &str,
    ) -> String {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let state = state.clone();
        let authorization = authorization.map(str::to_string);
        let body = body.to_string();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let address = listener.local_addr().expect("local addr");
            let auth_line = authorization
                .map(|value| format!("Authorization: {value}\r\n"))
                .unwrap_or_default();
            let request = format!(
                "POST /api/v1/sessions/activate HTTP/1.1\r\nHost: 127.0.0.1\r\n{auth_line}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let (header_text, body_bytes) = activation_split_request(&request);
            let request_for_client = request.clone();
            let client = tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut stream = tokio::net::TcpStream::connect(address)
                    .await
                    .expect("connect");
                stream
                    .write_all(request_for_client.as_bytes())
                    .await
                    .expect("write request");
                let mut buffer = vec![0u8; 8192];
                let mut response = Vec::new();
                loop {
                    let count = stream.read(&mut buffer).await.expect("read response");
                    if count == 0 {
                        break;
                    }
                    response.extend_from_slice(&buffer[..count]);
                }
                String::from_utf8_lossy(&response).to_string()
            });
            let (server_stream, _) = listener.accept().await.expect("accept");
            activation_http_handle_request(server_stream, state, "POST", header_text, body_bytes)
                .await;
            client.await.expect("join client")
        })
    }

    fn activation_response_body(response: &str) -> Value {
        let body = response
            .split("\r\n\r\n")
            .nth(1)
            .expect("response body");
        serde_json::from_str(body).expect("parse response body")
    }

    #[test]
    fn activation_token_should_store_only_hash() {
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        let (record, secret) = activation_token_create(&state, &conversation_id).expect("create");

        // 库里只有哈希：明文不可从库里读回。
        let conn = state_db_open(&state.data_path).expect("open state db");
        let stored_hash: String = conn
            .query_row(
                "SELECT token_hash FROM activation_tokens WHERE token_id=?1",
                rusqlite::params![record.token_id as i64],
                |row| row.get(0),
            )
            .expect("read token hash");
        assert_eq!(stored_hash, activation_token_hash_hex(&secret));
        assert!(!stored_hash.contains(&secret), "库里不允许出现令牌明文");

        let verified = activation_token_verify(&state, &secret).expect("verify");
        assert_eq!(verified.token_id, record.token_id);
        assert_eq!(verified.conversation_id, conversation_id);
        assert!(
            activation_token_verify(&state, "pai_act_deadbeef").is_err(),
            "查不到的令牌必须校验失败"
        );
    }

    #[test]
    fn activation_endpoint_test_flag_should_not_enqueue() {
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        set_conversation_runtime_state_and_emit(
            &state,
            &conversation_id,
            MainSessionState::AssistantStreaming,
        )
        .expect("mark busy");
        let (_record, secret) = activation_token_create(&state, &conversation_id).expect("create");

        let response = activation_call_endpoint(
            &state,
            Some(&format!("Bearer {secret}")),
            r#"{"test":true}"#,
        );
        assert!(response.starts_with("HTTP/1.1 200"), "实际响应: {response}");
        let body = activation_response_body(&response);
        assert_eq!(body["ok"], serde_json::json!(true));
        assert!(
            body["conversationId"].as_str().is_some(),
            "自测要回目标会话短号: {body}"
        );
        let slots = lock_conversation_runtime_slots(&state).expect("slots");
        let queued = slots
            .get(&conversation_id)
            .map(|slot| slot.pending_queue.len())
            .unwrap_or(0);
        assert_eq!(queued, 0, "test 请求不得投递任何消息");
    }

    #[test]
    fn activation_endpoint_should_enqueue_guided_system_event() {
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        // 会话正忙：事件只能进队列，便于断言投递形态。
        set_conversation_runtime_state_and_emit(
            &state,
            &conversation_id,
            MainSessionState::AssistantStreaming,
        )
        .expect("mark busy");
        let (_record, secret) = activation_token_create(&state, &conversation_id).expect("create");

        let response = activation_call_endpoint(
            &state,
            Some(&format!("Bearer {secret}")),
            r#"{"message":"检测到 GitHub 有新 issue #12"}"#,
        );
        assert!(response.starts_with("HTTP/1.1 200"), "实际响应: {response}");
        let body = activation_response_body(&response);
        assert_eq!(body["ok"], serde_json::json!(true));

        let slots = lock_conversation_runtime_slots(&state).expect("slots");
        let slot = slots.get(&conversation_id).expect("runtime slot");
        let event = slot.pending_queue.back().expect("queued event");
        assert!(matches!(event.source, ChatEventSource::System));
        assert_eq!(event.queue_mode, ChatQueueMode::Guided);
        assert!(event.activate_assistant);
        assert_eq!(
            event.messages[0]
                .provider_meta
                .as_ref()
                .and_then(|meta| meta.get("messageKind"))
                .and_then(Value::as_str),
            Some("activation_event")
        );
        assert!(
            message_text_content(&event.messages[0]).contains("issue #12"),
            "激活消息正文要原样带进会话"
        );
    }

    #[test]
    fn activation_endpoint_should_reject_bad_token() {
        let state = test_chat_runtime_state();
        for authorization in [None, Some("Bearer not-a-real-token")] {
            let response = activation_call_endpoint(
                &state,
                authorization,
                r#"{"message":"x"}"#,
            );
            assert!(
                response.starts_with("HTTP/1.1 401"),
                "鉴权失败必须回 401，实际响应: {response}"
            );
        }
    }

    #[test]
    fn activation_endpoint_should_reject_non_post() {
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        let (_record, secret) = activation_token_create(&state, &conversation_id).expect("create");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let response = runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let address = listener.local_addr().expect("local addr");
            let request = format!(
                "GET /api/v1/sessions/activate HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {secret}\r\n\r\n"
            );
            let request_for_client = request.clone();
            let client = tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut stream = tokio::net::TcpStream::connect(address)
                    .await
                    .expect("connect");
                stream
                    .write_all(request_for_client.as_bytes())
                    .await
                    .expect("write request");
                let mut buffer = vec![0u8; 8192];
                let mut response = Vec::new();
                loop {
                    let count = stream.read(&mut buffer).await.expect("read response");
                    if count == 0 {
                        break;
                    }
                    response.extend_from_slice(&buffer[..count]);
                }
                String::from_utf8_lossy(&response).to_string()
            });
            let (server_stream, _) = listener.accept().await.expect("accept");
            let (header_text, body_bytes) = activation_split_request(&request);
            activation_http_handle_request(
                server_stream,
                state.clone(),
                "GET",
                header_text,
                body_bytes,
            )
            .await;
            client.await.expect("join client")
        });
        assert!(
            response.starts_with("HTTP/1.1 405"),
            "激活端点只收 POST，实际响应: {response}"
        );
    }

    #[test]
    fn activation_endpoint_should_read_authorization_across_segments() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        runtime.block_on(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let address = listener.local_addr().expect("local addr");
            let first = "POST /api/v1/sessions/activate HTTP/1.1\r\n".to_string();
            let first_for_client = first.clone();
            let second = "Host: 127.0.0.1\r\nAuthorization: Bearer abc\r\nContent-Length: 2\r\n\r\n{}".to_string();
            let client = tokio::spawn(async move {
                use tokio::io::AsyncWriteExt;
                let mut stream = tokio::net::TcpStream::connect(address)
                    .await
                    .expect("connect");
                stream
                    .write_all(first_for_client.as_bytes())
                    .await
                    .expect("write first");
                stream.flush().await.expect("flush");
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                stream
                    .write_all(second.as_bytes())
                    .await
                    .expect("write second");
            });
            let (mut server_stream, _) = listener.accept().await.expect("accept");
            // 只把第一段当已读字节交进去：头没读完时必须继续读，而不是拿半截头去鉴权。
            let (headers, rest) =
                ide_context_http_read_headers(&mut server_stream, first.as_bytes().to_vec())
                    .await
                    .expect("read headers");
            assert!(
                headers.contains("Authorization: Bearer abc"),
                "头要读满才交给鉴权，实际: {headers}"
            );
            assert_eq!(rest, b"{}", "多读到的请求体字节要原样留下");
            client.await.expect("join client");
        });
    }

    #[test]
    fn automation_tool_get_should_return_dir_token_endpoint_and_guide() {
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        let session_id = format!("{}::{}", DEFAULT_AGENT_ID, conversation_id);
        let output = builtin_automation(
            &state,
            &session_id,
            AutomationToolArgs {
                command: "get".to_string(),
            },
        )
        .expect("automation get");

        let token = output["token"].as_str().expect("token");
        assert!(
            activation_token_verify(&state, token).is_ok(),
            "回执给的就是能用的明文令牌"
        );
        let automation_dir = output["automationDir"].as_str().expect("automation dir");
        assert!(
            automation_dir.ends_with("automations"),
            "脚本目录必须是绝对路径且落在会话私有目录: {automation_dir}"
        );
        assert!(
            automation_dir.replace('\\', "/").contains(&conversation_id),
            "目录要带上会话主键，不让模型自己拼: {automation_dir}"
        );
        let guide = output["guide"].as_str().expect("guide");
        assert!(guide.contains(automation_dir), "guide 必须给出脚本目录");
        let endpoint = output["endpoint"].as_str().expect("endpoint");
        if endpoint.is_empty() {
            assert!(
                guide.contains("网络访问"),
                "端点没开时 guide 要说明去哪儿打开: {guide}"
            );
            assert!(
                !guide.contains("curl"),
                "端点没开时不该给一条跑不通的 curl: {guide}"
            );
        } else {
            assert!(guide.contains(endpoint), "guide 要给出端点");
            assert!(guide.contains(token), "guide 里的 curl 要能直接跑");
        }
        assert!(output["tasks"].is_array(), "回执要带当前清单");
    }

    #[test]
    fn automation_manifest_should_map_interval_to_backoff_or_resident() {
        let task = |interval: Option<u64>| ConversationAutomationTask {
            id: "beat".to_string(),
            command: "python beat.py".to_string(),
            description: String::new(),
            interval_seconds: interval,
        };
        assert_eq!(task(None).interval(), None, "缺省是常驻");
        assert_eq!(task(Some(0)).interval(), None, "0 也是常驻");
        assert_eq!(
            task(Some(300)).interval(),
            Some(std::time::Duration::from_secs(300))
        );
    }

    #[test]
    fn automation_reload_should_start_and_stop_tasks() {
        let _guard = automation_test_guard();
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        let cwd = automation_dir_for_conversation(&state.data_path, &conversation_id);
        std::fs::create_dir_all(&cwd).expect("create automation dir");
        let beats = cwd.join("reload.txt");
        let config_path = automation_config_for_conversation(&state.data_path, &conversation_id);

        std::fs::write(
            &config_path,
            serde_json::json!({
                "tasks": [{
                    "id": "beat",
                    "command": automation_heartbeat_command("reload.txt"),
                }],
            })
            .to_string(),
        )
        .expect("write automation config");

        let outcome =
            conversation_automation_reload_conversation(&state, &conversation_id).expect("reload");
        assert_eq!(
            outcome.tasks.len(),
            1,
            "清单里新出现的条目要起任务：{:?}",
            outcome.tasks
        );
        assert_eq!(outcome.tasks[0]["id"], "beat");
        assert_eq!(outcome.tasks[0]["state"], "started", "新条目这轮该是 started");
        assert!(outcome.errors.is_empty(), "正常清单不该报错：{:?}", outcome.errors);
        std::thread::sleep(std::time::Duration::from_millis(900));
        assert!(
            std::fs::metadata(&beats).map(|meta| meta.len()).unwrap_or(0) > 0,
            "任务应真的跑起来"
        );

        // 再 reload 一次：清单没变，什么都不该重起。
        let outcome = conversation_automation_reload_conversation(&state, &conversation_id)
            .expect("reload again");
        assert_eq!(
            outcome.tasks[0]["state"], "running",
            "清单没变就不该重起任务，实际 {:?}",
            outcome.tasks
        );

        std::fs::write(&config_path, serde_json::json!({ "tasks": [] }).to_string())
            .expect("write empty config");
        let outcome = conversation_automation_reload_conversation(&state, &conversation_id)
            .expect("reload after removal");
        assert!(outcome.tasks.is_empty(), "清单里删掉的条目要停任务：{:?}", outcome.tasks);
        let size_after_stop = std::fs::metadata(&beats).map(|meta| meta.len()).unwrap_or(0);
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let size_later = std::fs::metadata(&beats).map(|meta| meta.len()).unwrap_or(0);
        assert_eq!(
            size_after_stop, size_later,
            "停掉的任务不能再写心跳，进程树必须整体回收"
        );
    }

    #[test]
    fn automation_reload_should_restart_when_definition_changes() {
        let _guard = automation_test_guard();
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        let cwd = automation_dir_for_conversation(&state.data_path, &conversation_id);
        std::fs::create_dir_all(&cwd).expect("create automation dir");
        let config_path = automation_config_for_conversation(&state.data_path, &conversation_id);

        std::fs::write(
            &config_path,
            serde_json::json!({
                "tasks": [{ "id": "beat", "command": automation_heartbeat_command("a.txt") }],
            })
            .to_string(),
        )
        .expect("write automation config");
        conversation_automation_reload_conversation(&state, &conversation_id).expect("reload");

        // 同一 id、换了 command：算换了任务，先停再起。
        std::fs::write(
            &config_path,
            serde_json::json!({
                "tasks": [{ "id": "beat", "command": automation_heartbeat_command("b.txt") }],
            })
            .to_string(),
        )
        .expect("rewrite automation config");
        let outcome = conversation_automation_reload_conversation(&state, &conversation_id)
            .expect("reload after change");
        assert_eq!(
            outcome.tasks[0]["state"], "started",
            "定义变了要按新定义起任务，实际 {:?}",
            outcome.tasks
        );

        // 缺省与 0 都是常驻：语义没变就不该重启。
        std::fs::write(
            &config_path,
            serde_json::json!({
                "tasks": [{ "id": "beat", "command": automation_heartbeat_command("b.txt"), "intervalSeconds": 0 }],
            })
            .to_string(),
        )
        .expect("write resident config");
        let outcome = conversation_automation_reload_conversation(&state, &conversation_id)
            .expect("reload resident");
        assert_eq!(
            outcome.tasks[0]["state"], "running",
            "intervalSeconds=0 与缺省同为常驻，不该重启，实际 {:?}",
            outcome.tasks
        );
    }

    #[test]
    fn automation_terminate_should_kill_whole_process_tree() {
        let _guard = automation_test_guard();
        let state = test_chat_runtime_state();
        let conversation_id = "conversation-automation-tree";
        let cwd = automation_dir_for_conversation(&state.data_path, conversation_id);
        std::fs::create_dir_all(&cwd).expect("create automation dir");
        let beats = cwd.join("beats.txt");
        let command = automation_heartbeat_command("beats.txt");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        runtime.block_on(async {
            let mut process =
                automation_spawn(&state, &command, &cwd, conversation_id, "heartbeat").expect("spawn");
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            assert!(
                std::fs::metadata(&beats).map(|meta| meta.len()).unwrap_or(0) > 0,
                "任务应先跑起来并写下心跳"
            );
            process.terminate().await;
        });
        let size_after_kill = std::fs::metadata(&beats).map(|meta| meta.len()).unwrap_or(0);
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let size_later = std::fs::metadata(&beats).map(|meta| meta.len()).unwrap_or(0);
        assert_eq!(
            size_after_kill, size_later,
            "托管进程树必须整体回收，不能留下还在写心跳的孤儿进程"
        );
    }

    #[test]
    fn automation_stop_all_should_really_kill_registered_processes() {
        let _guard = automation_test_guard();
        let state = test_chat_runtime_state();
        let conversation_id = "conversation-automation-stop-all";
        let cwd = automation_dir_for_conversation(&state.data_path, conversation_id);
        std::fs::create_dir_all(&cwd).expect("create automation dir");
        let beats = cwd.join("stopall.txt");
        let command = automation_heartbeat_command("stopall.txt");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        let pid = runtime.block_on(async {
            let process =
                automation_spawn(&state, &command, &cwd, conversation_id, "heartbeat").expect("spawn");
            tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            let pid = process.pid.expect("pid");
            assert!(
                live_automation_processes()
                    .lock()
                    .expect("live")
                    .get(conversation_id)
                    .and_then(|tasks| tasks.get("heartbeat").copied())
                    == Some(pid),
                "启动后必须登记进存活表"
            );
            // 退出路径：不依赖协程收口，stop_all 自己把登记的进程同步杀掉。
            conversation_automation_stop_all();
            assert!(
                !automation_process_alive(pid),
                "stop_all 返回时进程必须已经消失"
            );
            pid
        });
        let size_after_kill = std::fs::metadata(&beats).map(|meta| meta.len()).unwrap_or(0);
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let size_later = std::fs::metadata(&beats).map(|meta| meta.len()).unwrap_or(0);
        assert_eq!(size_after_kill, size_later, "stop_all 之后不得再有心跳增长");
        assert!(!automation_process_alive(pid));
    }

    /// 单会话 reload 不得碰其他会话的任务。
    #[test]
    fn automation_reload_conversation_should_not_touch_other_conversations() {
        let _guard = automation_test_guard();
        let state = test_chat_runtime_state();
        let first = automation_test_conversation(&state);
        let second = automation_test_conversation(&state);
        let mut configs = Vec::new();
        for conversation_id in [&first, &second] {
            let cwd = automation_dir_for_conversation(&state.data_path, conversation_id);
            std::fs::create_dir_all(&cwd).expect("create automation dir");
            let config_path = automation_config_for_conversation(&state.data_path, conversation_id);
            std::fs::write(
                &config_path,
                serde_json::json!({
                    "tasks": [{ "id": "beat", "command": automation_heartbeat_command("beat.txt") }],
                })
                .to_string(),
            )
            .expect("write config");
            configs.push(config_path);
        }
        conversation_automation_reload_conversation(&state, &first).expect("reload first");
        conversation_automation_reload_conversation(&state, &second).expect("reload second");

        // 只改第一个会话的清单。
        std::fs::write(
            &configs[0],
            serde_json::json!({
                "tasks": [{ "id": "beat", "command": automation_heartbeat_command("other.txt") }],
            })
            .to_string(),
        )
        .expect("rewrite first config");
        conversation_automation_reload_conversation(&state, &first).expect("reload first again");

        // 第二个会话的定义没变，必须还是 running，不能被第一个会话的重读牵连。
        let outcome = conversation_automation_reload_conversation(&state, &second)
            .expect("reload second again");
        assert_eq!(
            outcome.tasks[0]["state"], "running",
            "另一个会话改清单不该重起本会话的任务，实际 {:?}",
            outcome.tasks
        );

        // 收尾：两个会话都清空清单，别留进程。
        for (conversation_id, config_path) in [(&first, &configs[0]), (&second, &configs[1])] {
            std::fs::write(config_path, serde_json::json!({ "tasks": [] }).to_string())
                .expect("clear config");
            conversation_automation_reload_conversation(&state, conversation_id)
                .expect("reload empty");
        }
    }

    /// 清单写坏要报出行列与原文，并且不能把已经在跑的任务停掉。
    #[test]
    fn automation_reload_conversation_should_report_broken_json_and_keep_running() {
        let _guard = automation_test_guard();
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        let cwd = automation_dir_for_conversation(&state.data_path, &conversation_id);
        std::fs::create_dir_all(&cwd).expect("create automation dir");
        let beats = cwd.join("beat.txt");
        let config_path = automation_config_for_conversation(&state.data_path, &conversation_id);
        std::fs::write(
            &config_path,
            serde_json::json!({
                "tasks": [{ "id": "beat", "command": automation_heartbeat_command("beat.txt") }],
            })
            .to_string(),
        )
        .expect("write config");
        conversation_automation_reload_conversation(&state, &conversation_id).expect("reload");
        std::thread::sleep(std::time::Duration::from_millis(900));

        std::fs::write(
            &config_path,
            "{\n  \"tasks\": [\n    {\"id\": \"beat\", \"command\": }\n  ]\n}",
        )
        .expect("write broken config");
        let outcome = conversation_automation_reload_conversation(&state, &conversation_id)
            .expect("坏清单不该让 reload 整体失败");
        let error = outcome.errors.join("\n");
        assert!(error.contains("第 3 行"), "要报出行号：{error}");
        assert!(error.contains("\"command\""), "要带出出错那一行的原文：{error}");
        assert!(outcome.tasks.is_empty(), "读不到清单就不该报任务：{:?}", outcome.tasks);

        // 清单坏了不动已经在跑的任务。
        let size_before = std::fs::metadata(&beats).map(|meta| meta.len()).unwrap_or(0);
        std::thread::sleep(std::time::Duration::from_millis(900));
        let size_after = std::fs::metadata(&beats).map(|meta| meta.len()).unwrap_or(0);
        assert!(
            size_after > size_before,
            "清单写坏不该把已经在跑的任务停掉：前 {size_before} 后 {size_after}，文件 {}",
            beats.display()
        );

        std::fs::write(&config_path, serde_json::json!({ "tasks": [] }).to_string())
            .expect("clear config");
        conversation_automation_reload_conversation(&state, &conversation_id)
            .expect("reload empty");
    }

    /// get 回执要带出最近一次执行结果：脚本跑没跑、成没成，不该靠猜别的东西。
    #[test]
    fn automation_describe_should_report_last_run() {
        let _guard = automation_test_guard();
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        let cwd = automation_dir_for_conversation(&state.data_path, &conversation_id);
        std::fs::create_dir_all(&cwd).expect("create automation dir");
        let config_path = automation_config_for_conversation(&state.data_path, &conversation_id);
        // 常驻脚本立即以 3 退出：等一轮跑完就能看到退出码。
        std::fs::write(
            &config_path,
            serde_json::json!({ "tasks": [{ "id": "probe", "command": "exit 3" }] }).to_string(),
        )
        .expect("write config");
        conversation_automation_reload_conversation(&state, &conversation_id).expect("reload");

        // 起进程、跑完一轮需要时间，给足余量再读。
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let tasks = conversation_automation_describe(&state, &conversation_id).expect("describe");
        assert_eq!(tasks.len(), 1, "清单里有一条：{:?}", tasks);
        let record = &tasks[0]["lastRun"];
        assert_eq!(
            record["exitCode"], 3,
            "要带出上一轮的退出码，实际 {:?}",
            tasks
        );
        assert!(
            record["durationMs"].is_number(),
            "要带出耗时，实际 {:?}",
            tasks
        );
        assert!(
            record["finishedAtMs"].as_u64().unwrap_or(0) > 0,
            "要带出结束时间，实际 {:?}",
            tasks
        );

        std::fs::write(&config_path, serde_json::json!({ "tasks": [] }).to_string())
            .expect("clear config");
        conversation_automation_reload_conversation(&state, &conversation_id)
            .expect("reload empty");
    }

    /// stdout 直投的判定：只认单行 JSON 里的非空 message，别的一律不进会话。
    /// 判宽了会把脚本正常打印的 JSON 当消息投进来，判窄了脚本想说话却投不出去。
    #[test]
    fn automation_stdout_delivery_should_only_take_message_json() {
        assert_eq!(
            automation_stdout_delivery_text(r#"{"message":"有新 issue"}"#).as_deref(),
            Some("有新 issue")
        );
        assert_eq!(
            automation_stdout_delivery_text(r#"  {"message":"  两边空白  "}  "#).as_deref(),
            Some("两边空白")
        );
        // 空 message、非字符串 message、没有 message 都不是投递。
        assert_eq!(automation_stdout_delivery_text(r#"{"message":""}"#), None);
        assert_eq!(automation_stdout_delivery_text(r#"{"message":"   "}"#), None);
        assert_eq!(automation_stdout_delivery_text(r#"{"message":123}"#), None);
        assert_eq!(automation_stdout_delivery_text(r#"{"other":"x"}"#), None);
        // 普通输出与非法 JSON 照常进日志，不进会话。
        assert_eq!(automation_stdout_delivery_text("无新增、无状态变化，共 7 条 issue"), None);
        assert_eq!(automation_stdout_delivery_text("[1,2,3]"), None);
        assert_eq!(automation_stdout_delivery_text("{message: 坏 JSON}"), None);
    }

    /// 日志里的会话标记：标准 UUID 是前 8 位，非 UUID 的 id 原样保留。
    #[test]
    fn automation_log_conversation_tag_should_match_shortened_uuid() {
        assert_eq!(
            automation_log_conversation_tag("fe6a005b-ff76-46bd-91dd-2ea436910c79"),
            "fe6a005b"
        );
        assert_eq!(
            automation_log_conversation_tag("conversation-automation-tree"),
            "conversation-automation-tree"
        );
    }

    /// 取最近日志：只挑本会话的自动化行，别的会话与别的模块都不进来，超上限留最后几条。
    #[test]
    fn automation_tail_should_pick_only_this_conversation() {
        let path = std::env::temp_dir().join(format!("pai-automation-log-{}.log", Uuid::new_v4()));
        let body = [
            "[2026-11-26][03:00:00][INFO][p_ai] [自动化] 输出 任务=a 会话=aaaaaaaa line-1",
            "[2026-11-26][03:00:01][INFO][p_ai] [聊天] 无关模块 会话=bbbbbbbb",
            "[2026-11-26][03:00:02][INFO][p_ai] [自动化] 输出 任务=a 会话=bbbbbbbb 别的会话",
            "[2026-11-26][03:00:03][WARN][p_ai] [自动化] 执行 任务=a 会话=aaaaaaaa 退出码=1 耗时=5ms",
        ]
        .join("\n");
        std::fs::write(&path, body).expect("write log");

        let lines = automation_tail_matching_lines(&path, "会话=aaaaaaaa", 10);
        assert_eq!(lines.len(), 2, "只挑本会话的自动化行：{lines:?}");
        assert!(lines[0].contains("line-1"), "{lines:?}");
        assert!(lines[1].contains("退出码=1"), "{lines:?}");

        let capped = automation_tail_matching_lines(&path, "会话=aaaaaaaa", 1);
        assert_eq!(capped.len(), 1, "按上限截断：{capped:?}");
        assert!(capped[0].contains("退出码=1"), "要留最后一条：{capped:?}");

        let _ = std::fs::remove_file(&path);
    }

    /// 空 command 的条目要明确报出来，不能悄悄消失。
    #[test]
    fn automation_reload_conversation_should_report_empty_command() {
        let _guard = automation_test_guard();
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        let cwd = automation_dir_for_conversation(&state.data_path, &conversation_id);
        std::fs::create_dir_all(&cwd).expect("create automation dir");
        let config_path = automation_config_for_conversation(&state.data_path, &conversation_id);
        std::fs::write(
            &config_path,
            serde_json::json!({ "tasks": [{ "id": "noop", "command": "   " }] }).to_string(),
        )
        .expect("write config with empty command");

        let outcome = conversation_automation_reload_conversation(&state, &conversation_id)
            .expect("reload");
        let error = outcome.errors.join("\n");
        assert!(error.contains("command 为空"), "空 command 要报出来：{error}");
        assert!(outcome.tasks.is_empty(), "空 command 不该被托管");

        std::fs::remove_file(&config_path).ok();
    }

    /// 清单文件不存在时说清期望路径，而不是回一句空结果。
    #[test]
    fn automation_reload_conversation_should_report_missing_config() {
        let _guard = automation_test_guard();
        let state = test_chat_runtime_state();
        let conversation_id = automation_test_conversation(&state);
        let config_path = automation_config_for_conversation(&state.data_path, &conversation_id);

        let outcome = conversation_automation_reload_conversation(&state, &conversation_id)
            .expect("reload");
        let error = outcome.errors.join("\n");
        assert!(error.contains("不存在"), "要说明清单不存在：{error}");
        assert!(
            error.contains("automations.json"),
            "要给出期望的绝对路径：{error}"
        );
        assert!(outcome.tasks.is_empty());
        assert!(!config_path.is_file(), "这条用例不该写出清单文件");
    }
