async fn collect_streaming_model_reply_genai<S>(
    mut stream: S,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
) -> Result<ModelReply, String>
where
    S: futures_util::Stream<Item = Result<genai::chat::ChatStreamEvent, genai::Error>> + Unpin,
{
    let mut assistant_text = String::new();
    let mut reasoning_standard = String::new();
    let mut trusted_input_tokens: Option<u64> = None;
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(genai::chat::ChatStreamEvent::Start) => {}
            Ok(genai::chat::ChatStreamEvent::Chunk(text)) => {
                assistant_text.push_str(&text.content);
                if let Some(channel) = on_delta {
                    let _ = channel.send(AssistantDeltaEvent {
                        delta: text.content,
                        kind: None,
                        tool_name: None,
                        tool_status: None,
                        tool_args: None,
                        message: None,
                    });
                }
            }
            Ok(genai::chat::ChatStreamEvent::ReasoningChunk(reasoning)) => {
                if !reasoning.content.is_empty() {
                    reasoning_standard.push_str(&reasoning.content);
                    if let Some(channel) = on_delta {
                        let _ = channel.send(AssistantDeltaEvent {
                            delta: reasoning.content,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            tool_args: None,
                            message: None,
                        });
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
                        let joined = captured_texts.join("\n");
                        assistant_text = joined.clone();
                        if let Some(channel) = on_delta {
                            let _ = channel.send(AssistantDeltaEvent {
                                delta: joined,
                                kind: None,
                                tool_name: None,
                                tool_status: None,
                                tool_args: None,
                                message: None,
                            });
                        }
                    }
                }
                if reasoning_standard.is_empty() {
                    if let Some(captured_reasoning) = end
                        .captured_reasoning_content
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        reasoning_standard = captured_reasoning.to_string();
                    }
                }
                trusted_input_tokens = end
                    .captured_usage
                    .as_ref()
                    .and_then(|usage| usage.prompt_tokens)
                    .and_then(|value| u64::try_from(value).ok())
                    .filter(|value| *value > 0);
            }
            Err(err) => {
                runtime_log_error(format!(
                    "[聊天] GenAI 流式收集失败: error={:?}",
                    err
                ));
                return Err(format!("GenAI 流式收集失败：{:?}", err));
            }
        }
    }
    Ok(ModelReply {
        assistant_text,
        reasoning_standard,
        reasoning_inline: String::new(),
        assistant_provider_meta: None,
        tool_history_events: Vec::new(),
        suppress_assistant_message: false,
        trusted_input_tokens,
    })
}
