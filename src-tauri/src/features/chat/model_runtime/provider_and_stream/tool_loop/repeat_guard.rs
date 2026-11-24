#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ToolRepeatGuard {
    last_tool_name: String,
    last_args_signature: String,
    same_call_streak: usize,
}

fn canonical_json_signature(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string()),
        Value::Array(items) => {
            let parts = items
                .iter()
                .map(canonical_json_signature)
                .collect::<Vec<_>>()
                .join(",");
            format!("[{}]", parts)
        }
        Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let parts = keys
                .iter()
                .map(|key| {
                    let key_text =
                        serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string());
                    let value_text = map
                        .get(key)
                        .map(canonical_json_signature)
                        .unwrap_or_else(|| "null".to_string());
                    format!("{key_text}:{value_text}")
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{parts}}}")
        }
    }
}

fn normalized_tool_args_signature(tool_args: &str) -> String {
    let trimmed = tool_args.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => canonical_json_signature(&value),
        Err(_) => trimmed.to_string(),
    }
}

fn tool_args_effectively_empty(tool_args: &str) -> bool {
    let trimmed = tool_args.trim();
    if trimmed.is_empty() {
        return true;
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::Null) => true,
        Ok(Value::String(text)) => text.trim().is_empty(),
        Ok(Value::Array(items)) => items.is_empty(),
        Ok(Value::Object(map)) => map.is_empty(),
        _ => false,
    }
}

fn repeated_tool_call_note_message(tool_name: &str, tool_args: &str, repeat_streak: usize) -> String {
    if tool_args_effectively_empty(tool_args) {
        format!(
            "提示：{} 已连续 {} 次使用空参数调用，本次调用已正常执行。若缺少必要参数，请补齐后重试。",
            tool_name, repeat_streak
        )
    } else {
        format!(
            "提示：相同工具与相同参数已连续调用 {} 次，本次调用已正常执行。如非必要请勿继续重复调用。",
            repeat_streak
        )
    }
}

fn register_tool_repeat_attempt(
    guard: &mut ToolRepeatGuard,
    tool_name: &str,
    tool_args: &str,
) -> usize {
    let next_signature = normalized_tool_args_signature(tool_args);
    if guard.last_tool_name == tool_name && guard.last_args_signature == next_signature {
        guard.same_call_streak = guard.same_call_streak.saturating_add(1);
    } else {
        guard.last_tool_name = tool_name.to_string();
        guard.last_args_signature = next_signature;
        guard.same_call_streak = 1;
    }
    guard.same_call_streak
}

fn register_tool_repeat_attempt_once_per_batch(
    guard: &mut ToolRepeatGuard,
    batch_registered_signatures: &mut std::collections::HashSet<(String, String)>,
    tool_name: &str,
    tool_args: &str,
) -> usize {
    let args_signature = normalized_tool_args_signature(tool_args);
    let batch_signature = (tool_name.to_string(), args_signature);
    if !batch_registered_signatures.insert(batch_signature) {
        return guard.same_call_streak;
    }
    register_tool_repeat_attempt(guard, tool_name, tool_args)
}

