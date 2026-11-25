fn send_reasoning_delta_event(
    on_delta: &tauri::ipc::Channel<AssistantDeltaEvent>,
    reasoning: &str,
) {
    if reasoning.is_empty() {
        return;
    }
    let _ = on_delta.send(AssistantDeltaEvent {
        delta: reasoning.to_string(),
        kind: Some("activity_reasoning_delta".to_string()),
        request_id: None,
        activation_id: None,
        phase_id: None,
        reason: None,
        tool_name: None,
        tool_call_id: None,
        tool_status: None,
        tool_args: None,
        message: None,
        stream_cache: None,
    });
}

fn join_model_text_blocks<'a, I>(texts: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let mut joined = Vec::<String>::new();
    let mut previous: Option<String> = None;
    for text in texts {
        if text.is_empty() {
            continue;
        }
        if previous.as_deref() == Some(text) {
            continue;
        }
        joined.push(text.to_string());
        previous = Some(text.to_string());
    }
    joined.join("\n")
}

async fn collect_streaming_model_reply_genai<S>(
    mut stream: S,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
    app_state: Option<&AppState>,
    usage_conversation_id: Option<&str>,
    usage_provider_key: Option<&str>,
    usage_model_name: Option<&str>,
    stream_guard: Option<&StreamFailureGuard>,
    // 零 delta 门禁的上报口：只要向 on_delta 推送过任何事件（含空文本块）就置 true。
    // collect 复用方（provider_calls 的流式入口）把它透给 retry_request_once。
    pushed_any: &mut bool,
) -> Result<ModelReply, String>
where
    S: futures_util::Stream<Item = Result<genai::chat::ChatStreamEvent, genai::Error>> + Unpin,
{
    let mut assistant_text = String::new();
    let mut activity_reasoning_text = String::new();
    let mut trusted_input_tokens: Option<u64> = None;
    let mut usage = None;
    let mut assistant_provider_meta = None::<Value>;
    // 这一轮是否已被要求停止。命中就按「流已结束」处理：已收到的增量就是本轮正文，
    // 继续往下走，不抛错、不取消。
    //
    // 判据是待停列表里的调度号：按本轮会话取自己用过的号（取不到就当这一轮不会被打断）。
    let stop_request_ids = match (app_state, usage_conversation_id) {
        (Some(state), Some(conversation_id)) => {
            inflight_round_request_ids_by_conversation(state, conversation_id)
        }
        _ => Vec::new(),
    };
    let stop_check_conversation_id = app_state
        .and(usage_conversation_id)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    while let Some(chunk) = stream.next().await {
        if let (Some(state), Some(conversation_id)) = (app_state, stop_check_conversation_id) {
            if round_stop_requested(state, &stop_request_ids) {
                runtime_log_info(format!(
                    "[聊天] 流式读到本轮被点名，按流结束处理 会话={}",
                    conversation_id
                ));
                break;
            }
        }
        match chunk {
            Ok(genai::chat::ChatStreamEvent::Start) => {}
            Ok(genai::chat::ChatStreamEvent::Heartbeat) => {}
            Ok(genai::chat::ChatStreamEvent::Chunk(text)) => {
                assistant_text.push_str(&text.content);
                if let Some(channel) = on_delta {
                    // 文本块不过滤空内容：只要 send 过就算推过（零 delta 门禁看推送动作本身）。
                    *pushed_any = true;
                    let _ = channel.send(AssistantDeltaEvent {
                        delta: text.content,
                        kind: None,
                        request_id: None,
                        activation_id: None,
                        phase_id: None,
                        reason: None,
                        tool_name: None,
                        tool_call_id: None,
                        tool_status: None,
                        tool_args: None,
                        message: None,
                        stream_cache: None,
                    });
                }
            }
            Ok(genai::chat::ChatStreamEvent::ReasoningChunk(reasoning)) => {
                if !reasoning.content.is_empty() {
                    activity_reasoning_text.push_str(&reasoning.content);
                    if let Some(channel) = on_delta {
                        *pushed_any = true;
                        send_reasoning_delta_event(channel, &reasoning.content);
                    }
                }
            }
            Ok(genai::chat::ChatStreamEvent::ThoughtSignatureChunk(_)) => {}
            Ok(genai::chat::ChatStreamEvent::ToolCallChunk(_)) => {}
            Ok(genai::chat::ChatStreamEvent::End(end)) => {
                if assistant_text.is_empty() {
                    if let Some(captured_texts) = end
                        .captured_content
                        .as_ref()
                        .map(|content| content.texts())
                        .filter(|texts| !texts.is_empty())
                    {
                        let joined = join_model_text_blocks(captured_texts);
                        assistant_text = joined.clone();
                        if let Some(channel) = on_delta {
                            *pushed_any = true;
                            let _ = channel.send(AssistantDeltaEvent {
                                delta: joined,
                                kind: None,
                                request_id: None,
                                activation_id: None,
                                phase_id: None,
                                reason: None,
                                tool_name: None,
                                tool_call_id: None,
                                tool_status: None,
                                tool_args: None,
                                message: None,
                                stream_cache: None,
                            });
                        }
                    }
                }
                if activity_reasoning_text.is_empty() {
                    if let Some(captured_reasoning) = end
                        .captured_reasoning_content
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        activity_reasoning_text = captured_reasoning.to_string();
                        if let Some(channel) = on_delta {
                            *pushed_any = true;
                            send_reasoning_delta_event(channel, captured_reasoning);
                        }
                    }
                }
                trusted_input_tokens = end
                    .captured_usage
                    .as_ref()
                    .and_then(|usage| usage.prompt_tokens)
                    .and_then(|value| u64::try_from(value).ok())
                    .filter(|value| *value > 0);
                usage = end.captured_usage.as_ref().and_then(genai_usage_to_log_value);
                assistant_provider_meta =
                    genai_response_id_provider_meta(end.captured_response_id.as_deref());
                if let Some(usage) = usage.as_ref() {
                    add_provider_usage_delta_to_conversation(
                        app_state,
                        usage_conversation_id,
                        usage_provider_key,
                        usage_model_name,
                        usage,
                    );
                }
            }
            Err(err) => {
                runtime_log_error(format!(
                    "[聊天] GenAI 流式收集失败 原因={}",
                    err
                ));
                if let Some(guard) = stream_guard {
                    guard.note_stream_error(&err);
                }
                return Err(format!("GenAI 流式收集失败：{:?}", err));
            }
        }
    }
    Ok(ModelReply {
        assistant_text: assistant_text.clone(),
        final_response_text: assistant_text,
        activity_reasoning_text,
        assistant_provider_meta,
        tool_history_events: Vec::new(),
        suppress_assistant_message: false,
        trusted_input_tokens,
        usage,
        round_logs_recorded_internally: false,
    })
}

#[cfg(test)]
mod stream_collect_tests {
    use super::*;

    /// 流式阶段也必须能被结束：慢流（10ms 一块）读到结束信号就按流结束处理。
    ///
    /// 钉的是「已收到的增量完整保留 + 一百毫秒内收口」，不是零毫秒——
    /// 检查点在每块之间，这是这一阶段的物理粒度。
    #[test]
    fn stop_during_stream_breaks_within_100ms() {
        use crate::tests::test_chat_runtime_state;
        use std::time::{Duration, Instant};

        let state = test_chat_runtime_state();
        let conversation_id = "conversation-stop-stream";
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");

        runtime.block_on(async {
            let stream = futures_util::stream::unfold(0usize, |index| async move {
                // 最多推 60 块（约 600ms）：修复被摘掉时用例会在这之后收口并因超时断言变红，
                // 而不是永远挂着。
                if index >= 60 {
                    return None;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
                let chunk = genai::chat::ChatStreamEvent::Chunk(genai::chat::StreamChunk {
                    content: format!("第{index}块"),
                });
                Some((Ok(chunk), index + 1))
            });
            let stream = Box::pin(stream);

            // 先按真实链路登记这一轮：停止方按这一轮自己的号记进待停列表。
            let chat_key = format!("agent-a::{conversation_id}");
            register_inflight_chat_abort_handle(
                &state,
                &chat_key,
                "req-stop-stream",
                "assistant-stop-stream",
            )
            .expect("登记本轮");

            // 20ms 后按下停止：此时流已经推过一两块。
            let stopper_state = state.clone();
            let stopper_key = chat_key.clone();
            let stopper_conversation_id = conversation_id.to_string();
            let stopper = tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(20)).await;
                request_stop_inflight_chat_round(
                    &stopper_state,
                    &stopper_key,
                    &stopper_conversation_id,
                )
                .expect("request stop");
            });

            let mut pushed_any = false;
            let started = Instant::now();
            let reply = collect_streaming_model_reply_genai(
                stream,
                None,
                Some(&state),
                Some(conversation_id),
                None,
                None,
                None,
                &mut pushed_any,
            )
            .await
            .expect("打断不是错误：流式收集必须正常返回");
            let elapsed = started.elapsed();
            stopper.await.expect("stopper");

            assert!(
                elapsed < Duration::from_millis(100),
                "结束信号命中后必须立刻收口，实测 {elapsed:?}"
            );
            assert!(
                reply.assistant_text.starts_with("第0块"),
                "已收到的增量必须完整保留，实测 {:?}",
                reply.assistant_text
            );
        });
    }

    #[test]
    fn join_model_text_blocks_should_drop_consecutive_duplicate_blocks() {
        let table = "三并发，都在1秒内返回：\n\n| 工具 | 版本 |\n|---|---|\n| Node | v24.2.0 |";

        let joined = join_model_text_blocks([table, table]);

        assert_eq!(joined, table);
    }

    #[test]
    fn join_model_text_blocks_should_keep_distinct_blocks() {
        let joined = join_model_text_blocks(["第一段", "第二段"]);

        assert_eq!(joined, "第一段\n第二段");
    }

    #[tokio::test]
    async fn captured_reasoning_at_stream_end_should_emit_reasoning_delta() {
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<AssistantDeltaEvent>::new()));
        let events_for_channel = events.clone();
        let channel = tauri::ipc::Channel::new(move |body| {
            let parsed_event = match body {
                tauri::ipc::InvokeResponseBody::Json(json) => {
                    serde_json::from_str::<AssistantDeltaEvent>(&json).ok()
                }
                tauri::ipc::InvokeResponseBody::Raw(bytes) => {
                    serde_json::from_slice::<AssistantDeltaEvent>(&bytes).ok()
                }
            };
            if let Some(event) = parsed_event {
                if let Ok(mut guard) = events_for_channel.lock() {
                    guard.push(event);
                }
            }
            Ok(())
        });
        let stream = futures_util::stream::iter(vec![Ok(genai::chat::ChatStreamEvent::End(
            genai::chat::StreamEnd {
                captured_reasoning_content: Some("先判断工具是否需要执行".to_string()),
                ..Default::default()
            },
        ))]);

        let mut pushed_any = false;
        let reply = collect_streaming_model_reply_genai(
            stream,
            Some(&channel),
            None,
            None,
            None,
            None,
            None,
            &mut pushed_any,
        )
            .await
            .expect("stream collection should succeed");

        assert_eq!(reply.activity_reasoning_text, "先判断工具是否需要执行");
        let guard = events.lock().expect("events should be readable");
        assert_eq!(guard.len(), 1);
        assert_eq!(
            guard[0].kind.as_deref(),
            Some("activity_reasoning_delta")
        );
        assert_eq!(guard[0].delta, "先判断工具是否需要执行");
    }
}
