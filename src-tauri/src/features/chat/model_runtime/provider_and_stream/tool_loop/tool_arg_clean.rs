// ==================== 工具参数错误格式清洗 ====================

/// 清洗 LLM 生成的工具调用参数，修正常见错误格式。
/// 当前规则：exec（shell）工具命令中的 rg 选项误写。
/// 背景：rg 的 `-r` 是 `--replace`（必须带参数），模型常把 grep 的递归习惯带进来，
/// 写出 `-rn` / `-rln` 等连写，会被解析成 `-r n` / `-r ln`（把匹配替换成字符），
/// 产生看似正常但全错的假结果。rg 默认递归，行号用 `-n`。
fn clean_turn_tool_calls(tool_calls: Vec<genai::chat::ToolCall>) -> Vec<genai::chat::ToolCall> {
    tool_calls
        .into_iter()
        .map(|mut tool_call| {
            if tool_call.fn_name == "exec" {
                tool_call.fn_arguments = clean_exec_arguments(tool_call.fn_arguments);
            }
            tool_call
        })
        .collect()
}

fn clean_exec_arguments(arguments: Value) -> Value {
    let original = arguments.clone();
    let (mut args, was_string) = match arguments {
        Value::String(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(value) => (value, true),
            Err(_) => return original,
        },
        other => (other, false),
    };
    let cleaned = match args.get("command").and_then(Value::as_str) {
        Some(cmd) => match clean_shell_command(cmd) {
            Ok(cleaned) => Some(cleaned),
            Err(err) => {
                runtime_log_warn(format!("[工具] {err}，跳过本次清洗"));
                None
            }
        },
        None => None,
    };
    if let Some(cleaned) = cleaned {
        if args.get("command").and_then(Value::as_str) != Some(cleaned.as_str()) {
            args["command"] = Value::String(cleaned);
        }
    }
    if was_string {
        Value::String(args.to_string())
    } else {
        args
    }
}

/// 解码命令文本中索引 `i` 处的字符。`i` 必须是 UTF-8 字符边界，
/// 否则返回带上下文的可读错误，避免直接切片或 unwrap 导致 panic。
fn decode_char_at(command: &str, i: usize) -> Result<char, String> {
    command
        .get(i..)
        .and_then(|rest| rest.chars().next())
        .ok_or_else(|| format!("命令索引 {i} 无法解码为 UTF-8 字符：{}", command))
}

/// 修正 shell 命令中的 rg 选项误写。
/// 只处理「rg 作为命令词」位置之后的短选项 token，仅修正两种高频误写形态：
/// `-rn` → `-n`、`-rln` → `-ln`（grep 递归误写习惯）。
/// 其余 `-r` 连写、裸 `-r`、非 rg 命令、引号内文本、`--` 之后的位置参数一律不动。
/// 返回 Result：命令文本包含非法 UTF-8 边界时返回可读错误，由调用方决定跳过清洗。
fn clean_shell_command(command: &str) -> Result<String, String> {
    let bytes = command.as_bytes();
    let mut result = String::with_capacity(command.len());
    let mut i = 0;
    // 引号状态：None=引号外，Some(b'\'')=单引号内，Some(b'"')=双引号内
    let mut quote: Option<u8> = None;
    // 前一个字符是否由反斜杠转义：被转义的空白不是命令分隔符
    let mut prev_escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        // 转义：单引号外遇到反斜杠，复制反斜杠并跳过下一个字符（不做引号/命令识别）
        if b == b'\\' && quote != Some(b'\'') {
            result.push('\\');
            i += 1;
            if i < bytes.len() {
                let ch = decode_char_at(command, i)?;
                result.push(ch);
                i += ch.len_utf8();
                prev_escaped = true;
            }
            continue;
        }
        // 引号切换（引号内所有内容按原样复制，不识别 rg）
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                }
            }
            None => {
                if b == b'\'' || b == b'"' {
                    quote = Some(b);
                }
            }
        }
        if quote.is_some() {
            let ch = decode_char_at(command, i)?;
            result.push(ch);
            i += ch.len_utf8();
            prev_escaped = false;
            continue;
        }
        // 引号外：rg 命令词识别。前一个字符若被转义（如 `\ `），不能作为命令边界。
        if bytes[i..].starts_with(b"rg") {
            let before_ok = i == 0
                || (!prev_escaped
                    && (bytes[i - 1].is_ascii_whitespace()
                        || matches!(
                            bytes[i - 1],
                            b'&' | b';' | b'|' | b'(' | b'{' | b'\n' | b'\r' | b'\t'
                        )));
            let after_ok = bytes.get(i + 2).is_none_or(|c| {
                c.is_ascii_whitespace()
                    || matches!(c, b'&' | b';' | b'|' | b')' | b'}' | b'\n' | b'\r' | b'\t')
            });
            if before_ok && after_ok {
                result.push_str("rg");
                i += 2;
                // 跳过命令词后的空白
                while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                    result.push(bytes[i] as char);
                    i += 1;
                }
                // 处理连续短选项 token；遇到 `--`（之后全部为位置参数）或非 `-` 开头（pattern/路径）停止
                while i < bytes.len() && bytes[i] == b'-' {
                    let start = i;
                    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    let token = &command[start..i];
                    if token == "--" {
                        // 保留 `--` 及之后所有参数原样不动
                        result.push_str(&command[start..]);
                        i = bytes.len();
                        break;
                    }
                    result.push_str(&clean_rg_option_token(token));
                    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                        result.push(bytes[i] as char);
                        i += 1;
                    }
                }
                prev_escaped = false;
                continue;
            }
        }
        // 引号外普通字符：按 Unicode 标量解码后原样复制
        let ch = decode_char_at(command, i)?;
        result.push(ch);
        i += ch.len_utf8();
        prev_escaped = false;
    }
    Ok(result)
}

/// 单个 rg 短选项 token：只修正两种高频误写形态 `-rn` → `-n`、`-rln` → `-ln`。
/// 其余 `-r` 连写（低频）、裸 `-r`、`-r=...`、其他选项一律原样保留。
fn clean_rg_option_token(token: &str) -> String {
    match token {
        "-rn" => "-n".to_string(),
        "-rln" => "-ln".to_string(),
        _ => token.to_string(),
    }
}

#[cfg(test)]
mod tool_arg_clean_tests {
    use super::*;

    fn exec_call(command: &str) -> genai::chat::ToolCall {
        genai::chat::ToolCall {
            call_id: "call-1".to_string(),
            fn_name: "exec".to_string(),
            fn_arguments: serde_json::json!({ "command": command }),
            thought_signatures: None,
        }
    }

    #[test]
    fn rg_rn_is_cleaned_to_rg_n() {
        let calls = clean_turn_tool_calls(vec![exec_call(r#"rg -rn "foo" src"#)]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -n \"foo\" src");
    }

    #[test]
    fn rg_rln_is_cleaned_to_rg_ln() {
        let calls = clean_turn_tool_calls(vec![exec_call("rg -rln foo")]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -ln foo");
    }

    #[test]
    fn rg_rni_is_left_untouched() {
        let calls = clean_turn_tool_calls(vec![exec_call("rg -rni foo")]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -rni foo");
    }

    #[test]
    fn rg_rli_is_left_untouched() {
        let calls = clean_turn_tool_calls(vec![exec_call("rg -rli foo")]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -rli foo");
    }

    #[test]
    fn rg_rl_is_left_untouched() {
        let calls = clean_turn_tool_calls(vec![exec_call("rg -rl foo")]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -rl foo");
    }

    #[test]
    fn rg_rn_with_extra_options_keeps_them() {
        let calls = clean_turn_tool_calls(vec![exec_call("rg -rn -i foo")]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -n -i foo");
    }

    #[test]
    fn rg_after_and_operator_is_cleaned() {
        let calls = clean_turn_tool_calls(vec![exec_call("cd src && rg -rn foo")]);
        assert_eq!(calls[0].fn_arguments["command"], "cd src && rg -n foo");
    }

    #[test]
    fn rg_after_pipe_is_cleaned() {
        let calls = clean_turn_tool_calls(vec![exec_call("cat a.txt | rg -rn foo")]);
        assert_eq!(calls[0].fn_arguments["command"], "cat a.txt | rg -n foo");
    }

    #[test]
    fn bare_r_replace_is_left_untouched() {
        let calls = clean_turn_tool_calls(vec![exec_call("rg -r n foo")]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -r n foo");
    }

    #[test]
    fn grep_rn_is_left_untouched() {
        let calls = clean_turn_tool_calls(vec![exec_call("grep -rn foo")]);
        assert_eq!(calls[0].fn_arguments["command"], "grep -rn foo");
    }

    #[test]
    fn rg_inside_quotes_is_left_untouched() {
        let calls = clean_turn_tool_calls(vec![exec_call(r#"echo "rg -rn""#)]);
        assert_eq!(calls[0].fn_arguments["command"], r#"echo "rg -rn""#);
    }

    #[test]
    fn rg_in_quotes_after_space_is_left_untouched() {
        let calls = clean_turn_tool_calls(vec![exec_call(r#"echo "foo rg -rn""#)]);
        assert_eq!(calls[0].fn_arguments["command"], r#"echo "foo rg -rn""#);
    }

    #[test]
    fn rg_in_single_quotes_is_left_untouched() {
        let calls = clean_turn_tool_calls(vec![exec_call("echo 'rg -rn foo'")]);
        assert_eq!(calls[0].fn_arguments["command"], "echo 'rg -rn foo'");
    }

    #[test]
    fn double_dash_preserves_positional_args() {
        let calls = clean_turn_tool_calls(vec![exec_call("rg -- -rn file")]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -- -rn file");
    }

    #[test]
    fn double_dash_after_cleaned_option_keeps_rest() {
        let calls = clean_turn_tool_calls(vec![exec_call("rg -rn -- -rln file")]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -n -- -rln file");
    }

    #[test]
    fn non_ascii_command_is_preserved() {
        let calls = clean_turn_tool_calls(vec![exec_call("rg -rn 中文路径")]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -n 中文路径");
    }

    #[test]
    fn escaped_quote_does_not_open_string() {
        let calls = clean_turn_tool_calls(vec![exec_call(r#"echo \"rg -rn\""#)]);
        assert_eq!(calls[0].fn_arguments["command"], r#"echo \"rg -rn\""#);
    }

    #[test]
    fn rg_after_escaped_space_is_left_untouched() {
        let calls = clean_turn_tool_calls(vec![exec_call(r#"echo \ rg -rn"#)]);
        assert_eq!(calls[0].fn_arguments["command"], r#"echo \ rg -rn"#);
    }

    #[test]
    fn rg_as_word_part_is_left_untouched() {
        let calls = clean_turn_tool_calls(vec![exec_call("marg -rn foo")]);
        assert_eq!(calls[0].fn_arguments["command"], "marg -rn foo");
    }

    #[test]
    fn command_without_rg_stays_unchanged() {
        let calls = clean_turn_tool_calls(vec![exec_call("git status")]);
        assert_eq!(calls[0].fn_arguments["command"], "git status");
    }

    #[test]
    fn non_shell_tool_is_left_untouched() {
        let tool_call = genai::chat::ToolCall {
            call_id: "call-2".to_string(),
            fn_name: "read".to_string(),
            fn_arguments: serde_json::json!({ "path": "a.rs" }),
            thought_signatures: None,
        };
        let calls = clean_turn_tool_calls(vec![tool_call]);
        assert_eq!(calls[0].fn_arguments["path"], "a.rs");
    }

    #[test]
    fn object_arguments_are_cleaned_in_place() {
        let mut tool_call = exec_call(r#"rg -rn pattern"#);
        tool_call.fn_arguments =
            serde_json::json!({ "command": "rg -rn pattern", "timeout_ms": 1000 });
        let calls = clean_turn_tool_calls(vec![tool_call]);
        assert_eq!(calls[0].fn_arguments["command"], "rg -n pattern");
        assert_eq!(calls[0].fn_arguments["timeout_ms"], 1000);
    }

    #[test]
    fn string_arguments_keep_string_shape() {
        let mut tool_call = exec_call(r#"rg -rn "foo""#);
        tool_call.fn_arguments = Value::String(r#"{"command": "rg -rn \"foo\""}"#.to_string());
        let calls = clean_turn_tool_calls(vec![tool_call]);
        assert!(calls[0].fn_arguments.is_string());
        let args = serde_json::from_str::<Value>(calls[0].fn_arguments.as_str().unwrap()).unwrap();
        assert_eq!(args["command"], "rg -n \"foo\"");
    }
}
