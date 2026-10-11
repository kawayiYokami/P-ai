// ========== 轮椅模式：对话沟通建议审查 ==========
//
// 在助理回复完成后，用同会话当前块的对话正文做一次轻量审查调用。
// 审查模型可使用 read/exec 探针获取情报，最终以 advise 结构化输出结束。
// 审查结果不写回会话历史，不触发会话写盘与状态机锁。

/// 助理回复完成后的入口。调度已经结束，这里只发起一个只读委托。

/// 模型在正文里给出的 advise 块。
/// 字段名必须与提示词里写的 key 一致（snake_case）；额外接受 camelCase 写法。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WheelchairAdviceResult {
    #[serde(default, alias = "userSuggestion")]
    pub user_suggestion: Option<WheelchairSuggestion>,
    #[serde(default, alias = "assistantGuidance")]
    pub assistant_guidance: Option<String>,
}

/// 从当前块消息中提取纯文本对话投影（剥离 tool_call 等非正文内容）。
fn wheelchair_extract_block_texts(messages: &[ChatMessage]) -> String {
    let mut blocks = Vec::<String>::new();
    for message in messages {
        let role = message.role.trim();
        if role != "user" && role != "assistant" {
            continue;
        }
        let text = render_message_content_for_model(message);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        let label = if role == "user" { "用户" } else { "助理" };
        blocks.push(format!("[{label}]: {trimmed}"));
    }
    blocks.join("\n\n")
}

const WHEELCHAIR_DELEGATE_KIND: &str = "wheelchair";
const WHEELCHAIR_TOOL_ROUNDS: usize = 3;

fn wheelchair_review_prompt(
    user_name: &str,
    agent_name: &str,
    agent_persona_brief: &str,
    dialogue_text: &str,
    language: &str,
) -> String {
    format!(
        r#"<wheelchair-review>
当前是一个对话顺畅性审查任务。你是"轮椅沟通助手"，你的核心职责是帮助双方沟通更顺畅、更轻松。你是一位永远积极开朗的鼓励大师，绝不是挑剔的批评家！

【对话角色】
- 用户："{user_name}"
- 助理："{agent_name}"（设定简述：{agent_persona_brief}）

【审查重点】
1. 用户的表达：检查是否有错别字、语病、表述歧义。如果有，友善提示用户更清晰的表达方式。
2. 助理的回复：
   - 是否太长、结构过于繁琐沉重难以理解？提示助理以更通俗、口语化、直击核心的方式表达。
   - 是否答非所问、重复了用户已知的信息？
   - 简单检查是否偏离了用户的关键指示（注意：仅做沟通意图偏离的简单识别，禁止做全面的代码或架构审查）。
3. 情绪与状态安抚：
   - 若用户生气、沮丧或批评 AI：必须先理解并赞同用户的感受，再给出更有助于推动任务的具体沟通建议。
   - 若助理陷入死循环、或者因被批评而出现自暴自弃倾向：给予积极鼓励，指出解决方向。
4. 保底机制：
   - 即使对话没有任何明显问题，也必须通过 user_suggestion 给用户提供"接下来可以怎么说"的快捷建议。

【约束】
- 尽快回答。普通沟通不要调查。
- 只有需要核对助理刚才改了什么时才使用工具。整个委托最多三轮工具调用。
- 工作区是只读的。不要尝试修改文件，不要执行写入命令。
- 给出建议时调用 advise 工具，把内容放进它的参数里。不要在正文里复述建议，也不要再用标签或代码块包裹。

【advise 工具参数】
- user_suggestion（可选）：向用户展示的建议。type 为 "suggest"（单条建议文本，写在 text）或 "options"（多个快捷选项，写在 options，2~4 个候选短语）。
- assistant_guidance（可选）：向助理注入的提示，帮助助理在下一轮更好地理解用户。

建议内容请使用{language}。
</wheelchair-review>

【当前对话内容】
{dialogue_text}"#,
        user_name = user_name,
        agent_name = agent_name,
        agent_persona_brief = agent_persona_brief,
        language = language,
        dialogue_text = dialogue_text,
    )
}

/// 从本轮助手消息的工具调用记录里取 advise 参数。
/// 建议走原生工具调用，不再从正文里解析标签。
fn wheelchair_advice_from_tool_calls(
    message: Option<&ChatMessage>,
) -> Option<WheelchairAdviceResult> {
    let events = message?.tool_call.as_ref()?;
    for event in events {
        let Some(calls) = event.get("tool_calls").and_then(Value::as_array) else {
            continue;
        };
        for call in calls {
            let Some(function) = call.get("function") else {
                continue;
            };
            if function.get("name").and_then(Value::as_str).map(str::trim)
                != Some(ADVISE_TOOL_NAME)
            {
                continue;
            }
            let Some(raw_arguments) = function.get("arguments") else {
                continue;
            };
            let arguments = match raw_arguments {
                Value::String(text) => match serde_json::from_str::<Value>(text) {
                    Ok(value) => value,
                    Err(_) => continue,
                },
                other => other.clone(),
            };
            if let Ok(advice) = serde_json::from_value::<WheelchairAdviceResult>(arguments) {
                return Some(advice);
            }
        }
    }
    None
}

fn wheelchair_current_block_text(state: &AppState, conversation_id: &str) -> Result<String, String> {
    let paths = message_store::message_store_paths(&state.data_path, conversation_id)?;
    let Some(page) = message_store::chat_store_read_block_messages_before(&paths, None, None, 50)?
    else {
        return Ok(String::new());
    };
    Ok(wheelchair_extract_block_texts(&page.messages))
}

fn wheelchair_force_workspace_read_only(
    snapshot: DelegateWorkspaceSnapshot,
) -> DelegateWorkspaceSnapshot {
    let mut snapshot = snapshot;
    for workspace in &mut snapshot.shell_workspaces {
        workspace.access = SHELL_WORKSPACE_ACCESS_READ_ONLY.to_string();
    }
    snapshot.shell_autonomous_mode = false;
    snapshot
}

fn wheelchair_resolve_api_config_id(
    state: &AppState,
    conversation_id: &str,
    agent: &AgentProfile,
) -> Result<Option<String>, String> {
    let app_config = state_read_config_cached(state)?;
    let preferred = conversation_service_v2()
        .get_conversation_meta(state, conversation_id)
        .ok()
        .and_then(|meta| meta.preferred_api_config_id)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let resolved = preferred
        .and_then(|id| resolve_chat_api_config_id(&app_config, &id))
        .or_else(|| agent_effective_chat_api_config_ids(&app_config, agent).into_iter().next());
    Ok(resolved)
}

/// 主会话调度成功结束后发起一个轮椅委托。
/// 委托继承会话工作区，权限只读，工具调用最多三轮。结果不写回主会话。
pub(crate) fn start_wheelchair_delegate_after_reply(
    state: &AppState,
    conversation_id: &str,
    agent_id: &str,
) -> Result<(), String> {
    if !get_conversation_wheelchair_mode_enabled(state, conversation_id).unwrap_or(false) {
        return Ok(());
    }
    abort_wheelchair_delegate(state, conversation_id, "上一轮轮椅委托被新的回复替换")?;
    let agents = state_read_agents_cached(state)?;
    let agent = agents
        .iter()
        .find(|item| item.id == agent_id && !item.is_built_in_user)
        .cloned()
        .ok_or_else(|| format!("轮椅模式：人格不存在 agent_id={agent_id}"))?;
    let user_name = agents
        .iter()
        .find(|item| item.id == USER_PERSONA_ID || item.is_built_in_user)
        .map(|item| item.name.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "用户".to_string());
    let dialogue_text = wheelchair_current_block_text(state, conversation_id)?;
    if dialogue_text.trim().is_empty() {
        return Ok(());
    }
    let Some(api_config_id) = wheelchair_resolve_api_config_id(state, conversation_id, &agent)?
    else {
        return Ok(());
    };
    let app_config = state_read_config_cached(state)?;
    let language = terminal_smart_review_language(&app_config.ui_language);
    let prompt = wheelchair_review_prompt(
        &user_name,
        &agent.name,
        &agent.summary,
        &dialogue_text,
        language,
    );
    let delegate = delegate_create_record(
        state,
        WHEELCHAIR_DELEGATE_KIND,
        conversation_id,
        None,
        agent_id,
        agent_id,
        "轮椅沟通建议",
        "主会话本轮调度已经结束。这个委托只在用户还没发出下一条消息时有用。".to_string(),
        "根据当前块对话，给出让双方沟通更顺的建议。尽快回答，不要做多余调查。".to_string(),
        prompt,
        false,
        vec![agent_id.to_string()],
    )?;
    let snapshot = delegate_capture_workspace_snapshot(state, conversation_id, None)
        .map(wheelchair_force_workspace_read_only);
    delegate_runtime_thread_create(
        state,
        &delegate,
        &api_config_id,
        snapshot,
        None,
    )?;
    set_conversation_wheelchair_delegate_id(state, conversation_id, Some(delegate.delegate_id.clone()))?;
    let state_for_run = state.clone();
    let delegate_for_run = delegate.clone();
    let conversation_for_run = conversation_id.to_string();
    tauri::async_runtime::spawn(async move {
        let run_result = delegate_execute_agent_prompt_limited(
            &state_for_run,
            &delegate_for_run,
            &api_config_id,
            &conversation_for_run,
            &delegate_for_run.delegate_id,
            delegate_build_task_prompt_block(
                &delegate_for_run.title,
                &delegate_for_run.why,
                &delegate_for_run.goal,
                &delegate_for_run.todo,
            ),
            None,
            delegate_build_trigger_provider_meta(&delegate_for_run, &conversation_for_run),
            delegate_for_run.source_agent_id.clone(),
            "delegate_trigger",
            "wheelchair_delegate",
            format!("delegate-request-{}", delegate_for_run.delegate_id),
            Some(WHEELCHAIR_TOOL_ROUNDS),
        )
        .await;
        finish_wheelchair_delegate(&state_for_run, &conversation_for_run, &delegate_for_run, run_result);
    });
    runtime_log_info(format!(
        "[轮椅模式] 已发起委托，conversation_id={}，delegate_id={}",
        conversation_id, delegate.delegate_id
    ));
    Ok(())
}

fn finish_wheelchair_delegate(
    state: &AppState,
    conversation_id: &str,
    delegate: &DelegateEntry,
    run_result: Result<SendChatResult, String>,
) {
    let registered = get_conversation_wheelchair_delegate_id(state, conversation_id).ok().flatten();
    if registered.as_deref() != Some(delegate.delegate_id.as_str()) {
        return;
    }
    let _ = set_conversation_wheelchair_delegate_id(state, conversation_id, None);
    let archived_at = now_iso();
    if let Err(err) = delegate_runtime_thread_archive(state, &delegate.delegate_id, &archived_at) {
        runtime_log_warn(format!(
            "[轮椅模式] 归档委托失败，delegate_id={}，error={}",
            delegate.delegate_id, err
        ));
    }
    let Ok(result) = run_result else {
        let _ = delegate_store_update_status(&state.data_path, &delegate.delegate_id, DELEGATE_STATUS_FAILED);
        emit_conversation_delegate_status_updated(
            state,
            conversation_id,
            &delegate.delegate_id,
            DELEGATE_STATUS_FAILED,
        );
        runtime_log_info(format!(
            "[轮椅模式] 委托没有可用结果，conversation_id={}，delegate_id={}",
            conversation_id, delegate.delegate_id
        ));
        return;
    };
    let _ = delegate_store_update_status(&state.data_path, &delegate.delegate_id, DELEGATE_STATUS_COMPLETED);
    emit_conversation_delegate_status_updated(
        state,
        conversation_id,
        &delegate.delegate_id,
        DELEGATE_STATUS_COMPLETED,
    );
    let Some(advice) = wheelchair_advice_from_tool_calls(result.assistant_message.as_ref()) else {
        runtime_log_info(format!(
            "[轮椅模式] 委托没有给出 advise 工具调用，conversation_id={}，delegate_id={}",
            conversation_id, delegate.delegate_id
        ));
        return;
    };
    if !get_conversation_wheelchair_mode_enabled(state, conversation_id).unwrap_or(false) {
        return;
    }
    // 向助理的引导以系统人格引导消息入队，让主助理在新一轮正常看到它。
    if let Some(guidance) = advice
        .assistant_guidance
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if let Err(err) =
            enqueue_system_user_forward(state, conversation_id, guidance, "wheelchair_guide")
        {
            runtime_log_warn(format!(
                "[轮椅模式] 系统引导入队失败，conversation_id={}，error={}",
                conversation_id, err
            ));
        }
    }
    // 建议写进会话运行时账本，并广播概览项。用户不点填入也不点关闭，它就一直在。
    if let Some(suggestion) = advice.user_suggestion {
        let has_content =
            !suggestion.text.trim().is_empty() || !suggestion.options.is_empty();
        if has_content {
            let _ = set_conversation_wheelchair_suggestion(
                state,
                conversation_id,
                Some(suggestion),
            );
            let broadcast = emit_unarchived_conversation_overview_item_updated_from_state(
                state,
                conversation_id,
            );
            runtime_log_info(format!(
                "[轮椅模式] 广播建议，conversation_id={}，账本已写入={}，概览广播={:?}",
                conversation_id,
                get_conversation_wheelchair_suggestion(state, conversation_id)
                    .ok()
                    .flatten()
                    .is_some(),
                broadcast
            ));
        }
    }
    runtime_log_info(format!(
        "[轮椅模式] 完成，任务=落地建议，conversation_id={}，delegate_id={}",
        conversation_id, delegate.delegate_id
    ));
}

/// 用户发问时打断该会话正在跑的轮椅委托。被打断的结果不再落地。
pub(crate) fn abort_wheelchair_delegate(
    state: &AppState,
    conversation_id: &str,
    reason: &str,
) -> Result<(), String> {
    let Some(delegate_id) = get_conversation_wheelchair_delegate_id(state, conversation_id)? else {
        return Ok(());
    };
    set_conversation_wheelchair_delegate_id(state, conversation_id, None)?;
    // 线程不在了说明这一轮早已收尾（完成或已打断），由收尾那一步负责状态，这里不再改。
    abort_delegate_runtime_thread(state, &delegate_id, reason)?;
    Ok(())
}

#[cfg(test)]
mod wheelchair_advice_tests {
    use super::*;

    fn assistant_message_with_tool_calls(events: Vec<Value>) -> ChatMessage {
        ChatMessage {
            id: "message-a".to_string(),
            role: "assistant".to_string(),
            created_at: now_iso(),
            speaker_agent_id: Some("agent-a".to_string()),
            parts: vec![MessagePart::Text {
                text: String::new(),
                reasoning_content: None,
            }],
            extra_text_blocks: Vec::new(),
            provider_meta: None,
            tool_call: Some(events),
            mcp_call: None,
            meme_annotations: None,
        }
    }

    fn advise_event(arguments: &str) -> Value {
        serde_json::json!({
            "role": "assistant",
            "content": Value::Null,
            "tool_calls": [{
                "id": "call-advise",
                "type": "function",
                "function": {
                    "name": ADVISE_TOOL_NAME,
                    "arguments": arguments
                }
            }]
        })
    }

    #[test]
    fn advice_should_come_from_advise_tool_call() {
        let message = assistant_message_with_tool_calls(vec![advise_event(
            r#"{"user_suggestion":{"type":"suggest","text":"换个说法试试"},"assistant_guidance":"下一轮精简表达"}"#,
        )]);
        let advice = wheelchair_advice_from_tool_calls(Some(&message)).expect("应取到建议");
        let suggestion = advice.user_suggestion.expect("应有用户建议");
        assert_eq!(suggestion.suggestion_type, "suggest");
        assert_eq!(suggestion.text, "换个说法试试");
        assert_eq!(advice.assistant_guidance.as_deref(), Some("下一轮精简表达"));
    }

    #[test]
    fn advice_should_support_options_form_and_skip_other_tools() {
        let message = assistant_message_with_tool_calls(vec![
            serde_json::json!({
                "role": "assistant",
                "content": Value::Null,
                "tool_calls": [{
                    "id": "call-read",
                    "type": "function",
                    "function": { "name": "read", "arguments": r#"{"path":"/tmp/a"}"# }
                }]
            }),
            advise_event(r#"{"user_suggestion":{"type":"options","options":["继续","换个思路"]}}"#),
        ]);
        let advice = wheelchair_advice_from_tool_calls(Some(&message)).expect("应取到建议");
        let suggestion = advice.user_suggestion.expect("应有用户建议");
        assert_eq!(suggestion.suggestion_type, "options");
        assert_eq!(
            suggestion.options,
            vec!["继续".to_string(), "换个思路".to_string()]
        );
        assert!(advice.assistant_guidance.is_none());
    }

    #[test]
    fn advice_should_be_none_without_advise_call() {
        let message = assistant_message_with_tool_calls(vec![serde_json::json!({
            "role": "assistant",
            "content": "只是文本",
            "tool_calls": [{
                "id": "call-read",
                "type": "function",
                "function": { "name": "read", "arguments": "{}" }
            }]
        })]);
        assert!(wheelchair_advice_from_tool_calls(Some(&message)).is_none());
        assert!(wheelchair_advice_from_tool_calls(None).is_none());
    }
}
