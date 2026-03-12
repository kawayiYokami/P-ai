async fn collect_streaming_model_reply<R, S>(
    mut stream: S,
    on_delta: Option<&tauri::ipc::Channel<AssistantDeltaEvent>>,
) -> Result<ModelReply, String>
where
    R: rig::completion::GetTokenUsage,
    S: futures_util::Stream<
            Item = Result<StreamedAssistantContent<R>, rig::completion::CompletionError>,
        > + Unpin,
{
    let mut assistant_text = String::new();
    let mut reasoning_standard = String::new();
    let mut trusted_input_tokens: Option<u64> = None;
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(StreamedAssistantContent::Text(text)) => {
                assistant_text.push_str(&text.text);
                if let Some(channel) = on_delta {
                    let _ = channel.send(AssistantDeltaEvent {
                        delta: text.text,
                        kind: None,
                        tool_name: None,
                        tool_status: None,
                        tool_args: None,
                        message: None,
                    });
                }
            }
            Ok(StreamedAssistantContent::Reasoning(reasoning)) => {
                let merged = reasoning.display_text();
                if !merged.is_empty() {
                    if !reasoning_standard.is_empty() {
                        reasoning_standard.push('\n');
                    }
                    reasoning_standard.push_str(&merged);
                    if let Some(channel) = on_delta {
                        let _ = channel.send(AssistantDeltaEvent {
                            delta: merged,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            tool_args: None,
                            message: None,
                        });
                    }
                }
            }
            Ok(StreamedAssistantContent::ReasoningDelta { reasoning, .. }) => {
                if !reasoning.is_empty() {
                    reasoning_standard.push_str(&reasoning);
                    if let Some(channel) = on_delta {
                        let _ = channel.send(AssistantDeltaEvent {
                            delta: reasoning,
                            kind: Some("reasoning_standard".to_string()),
                            tool_name: None,
                            tool_status: None,
                            tool_args: None,
                            message: None,
                        });
                    }
                }
            }
            Ok(StreamedAssistantContent::ToolCall { .. }) => {}
            Ok(StreamedAssistantContent::ToolCallDelta { .. }) => {}
            Ok(StreamedAssistantContent::Final(res)) => {
                trusted_input_tokens = rig::completion::GetTokenUsage::token_usage(&res)
                    .map(|usage| usage.input_tokens.saturating_add(usage.cached_input_tokens))
                    .filter(|value| *value > 0);
            }
            Err(err) => return Err(format!("rig streaming failed: {err}")),
        }
    }
    Ok(ModelReply {
        assistant_text,
        reasoning_standard,
        reasoning_inline: String::new(),
        tool_history_events: Vec::new(),
        suppress_assistant_message: false,
        trusted_input_tokens,
    })
}
