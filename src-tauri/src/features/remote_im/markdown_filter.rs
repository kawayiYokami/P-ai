fn remote_im_strip_simple_markdown(input: &str) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = Vec::<String>::new();
    let mut in_fenced_code = false;

    for raw_line in normalized.lines() {
        let trimmed = raw_line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fenced_code = !in_fenced_code;
            continue;
        }
        if trimmed.chars().all(|ch| ch == '-' || ch == '*' || ch == '_') && trimmed.len() >= 3 {
            continue;
        }

        let mut line = raw_line.to_string();
        if !in_fenced_code {
            line = strip_markdown_line_prefixes(&line);
        }
        let cleaned = strip_markdown_inline(&line);
        let compact = collapse_markdown_whitespace(&cleaned);
        if !compact.is_empty() || !out.last().map(|line| line.is_empty()).unwrap_or(false) {
            out.push(compact);
        }
    }

    strip_remote_im_terminal_period(out.join("\n").trim())
}

fn strip_remote_im_terminal_period(input: &str) -> String {
    if let Some(without_period) = input.strip_suffix('。') {
        return without_period.to_string();
    }
    if let Some(without_period) = input.strip_suffix('.') {
        if without_period
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
        {
            return without_period.to_string();
        }
    }
    input.to_string()
}

fn strip_markdown_line_prefixes(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return String::new();
    }

    let without_heading = trimmed.trim_start_matches('#').trim_start();
    let mut candidate = if without_heading.len() != trimmed.len() {
        without_heading.to_string()
    } else {
        trimmed.to_string()
    };

    if let Some(rest) = candidate.strip_prefix("> ") {
        candidate = rest.to_string();
    } else if let Some(rest) = candidate.strip_prefix('>') {
        candidate = rest.trim_start().to_string();
    } else if candidate == ">" {
        candidate.clear();
    }

    let chars: Vec<char> = candidate.chars().collect();
    if chars.len() >= 2 && (chars[0] == '-' || chars[0] == '*' || chars[0] == '+') && chars[1].is_whitespace() {
        return strip_markdown_task_prefixes(candidate[2..].trim_start());
    }

    let mut digit_end = 0usize;
    while digit_end < chars.len() && chars[digit_end].is_ascii_digit() {
        digit_end += 1;
    }
    if digit_end > 0 && digit_end + 1 < chars.len() && (chars[digit_end] == '.' || chars[digit_end] == ')') && chars[digit_end + 1].is_whitespace() {
        let rest = chars[(digit_end + 2)..].iter().collect::<String>();
        return strip_markdown_task_prefixes(rest.trim_start());
    }

    strip_markdown_task_prefixes(&candidate)
}

fn strip_markdown_task_prefixes(input: &str) -> String {
    if let Some(rest) = input.strip_prefix("[ ] ") {
        return rest.to_string();
    }
    if let Some(rest) = input.strip_prefix("[x] ") {
        return rest.to_string();
    }
    if let Some(rest) = input.strip_prefix("[X] ") {
        return rest.to_string();
    }
    input.to_string()
}

fn strip_markdown_inline(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some((alt, next_index)) = markdown_bracket_link_label(&chars, i + 1) {
                out.push_str(alt.trim());
                i = next_index;
                continue;
            }
        }

        if chars[i] == '[' {
            if let Some((label, next_index)) = markdown_bracket_link_label(&chars, i) {
                out.push_str(label.trim());
                i = next_index;
                continue;
            }
        }

        if chars[i] == '<' {
            if let Some(close_index) = chars[i + 1..].iter().position(|ch| *ch == '>') {
                let inner = chars[(i + 1)..(i + 1 + close_index)].iter().collect::<String>();
                out.push_str(inner.trim());
                i += close_index + 2;
                continue;
            }
        }

        match chars[i] {
            '`' | '*' | '_' | '~' => {
                i += 1;
            }
            '|' => {
                out.push(' ');
                i += 1;
            }
            // 到这一步拿到的是 JSON 解析后的纯文本，`\` 就是字面反斜杠，
            // 不存在转义序列。原样输出它和后面那个字符，不让后面的字符再被当语法处理——
            // 否则 Windows 路径（E:\github\...）会被整条吃掉。
            '\\' => {
                out.push('\\');
                if i + 1 < chars.len() {
                    out.push(chars[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            _ => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }

    out
}

fn markdown_bracket_link_label(chars: &[char], open_index: usize) -> Option<(String, usize)> {
    let close_bracket = chars[(open_index + 1)..]
        .iter()
        .position(|ch| *ch == ']')
        .map(|offset| open_index + 1 + offset)?;
    if close_bracket + 1 >= chars.len() || chars[close_bracket + 1] != '(' {
        return None;
    }
    let close_paren = chars[(close_bracket + 2)..]
        .iter()
        .position(|ch| *ch == ')')
        .map(|offset| close_bracket + 2 + offset)?;
    let label = chars[(open_index + 1)..close_bracket].iter().collect::<String>();
    Some((label, close_paren + 1))
}

fn collapse_markdown_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut previous_was_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() && ch != '\n' {
            if !previous_was_space {
                out.push(' ');
                previous_was_space = true;
            }
            continue;
        }
        previous_was_space = false;
        out.push(ch);
    }
    out.trim().to_string()
}

fn remote_im_filter_markdown_content_items(
    channel: &RemoteImChannelConfig,
    content: Vec<Value>,
) -> Vec<Value> {
    if !channel.filter_markdown {
        return content;
    }
    content
        .into_iter()
        .filter_map(|item| {
            let Some(item_type) = item.get("type").and_then(Value::as_str) else {
                return Some(item);
            };
            if item_type != "text" {
                return Some(item);
            }
            let text = item.get("text").and_then(Value::as_str).unwrap_or_default();
            let cleaned = remote_im_strip_simple_markdown(text);
            if cleaned.is_empty() {
                return None;
            }
            let mut object = item.as_object().cloned().unwrap_or_default();
            object.insert("text".to_string(), Value::String(cleaned));
            Some(Value::Object(object))
        })
        .collect()
}

#[cfg(test)]
mod remote_im_markdown_filter_tests {
    use super::*;

    #[test]
    fn strip_simple_markdown_should_keep_text_content() {
        let input = "# 标题\n- **加粗** [链接](https://example.com)\n> `代码`\n";
        assert_eq!(
            remote_im_strip_simple_markdown(input),
            "标题\n加粗 链接\n代码"
        );
    }

    #[test]
    fn strip_simple_markdown_should_handle_ai_style_rich_reply() {
        let input = r#"## 今日总结

先说结论：**可以推进**，但有几个前置项要确认。

### 你可以这样看
1. 先检查 `config.toml` 是否存在
2. 再确认 [服务状态](https://example.com/status) 正常
3. 如果失败，执行 `pnpm dev`

> 提示：如果你只是想快速验证，优先看日志。

- 优点：**接入快**
- 风险：需要处理 _边界情况_
- 备注：支持 ~~旧方案~~ 新方案

- [x] 已完成接口梳理
- [ ] 待补一次联调验证

```ts
const result = await runTask();
console.log(result);
```

如需更多信息，可以查看 ![截图说明](mock-image.png) 或访问 <https://example.com/docs>。
"#;
        let expected = r#"今日总结

先说结论：可以推进，但有几个前置项要确认。

你可以这样看
先检查 config.toml 是否存在
再确认 服务状态 正常
如果失败，执行 pnpm dev

提示：如果你只是想快速验证，优先看日志。

优点：接入快
风险：需要处理 边界情况
备注：支持 旧方案 新方案

已完成接口梳理
待补一次联调验证

const result = await runTask();
console.log(result);

如需更多信息，可以查看 截图说明 或访问 https://example.com/docs"#;

        assert_eq!(remote_im_strip_simple_markdown(input), expected);
    }

    #[test]
    fn strip_simple_markdown_should_soften_terminal_period() {
        assert_eq!(remote_im_strip_simple_markdown("好的。"), "好的");
        assert_eq!(remote_im_strip_simple_markdown("版本 2。"), "版本 2");
        assert_eq!(remote_im_strip_simple_markdown("Okay."), "Okay");
        assert_eq!(remote_im_strip_simple_markdown("3.0"), "3.0");
        assert_eq!(remote_im_strip_simple_markdown("收到."), "收到.");
        assert_eq!(remote_im_strip_simple_markdown("Wait..."), "Wait...");
    }

    #[test]
    fn strip_simple_markdown_should_keep_windows_path_backslash() {
        // JSON 解析后拿到的是纯文本，`\` 是字面反斜杠，不是转义序列。
        // 清理只去语法标记，不能碰它，否则 Windows 路径会被整条吃掉。
        assert_eq!(
            remote_im_strip_simple_markdown(r"E:\github\paimonhome\remote"),
            r"E:\github\paimonhome\remote"
        );
        assert_eq!(
            remote_im_strip_simple_markdown(r"当前工作区是 E:\github\paimonhome\remote"),
            r"当前工作区是 E:\github\paimonhome\remote"
        );
        // 语法标记照旧清掉。
        assert_eq!(remote_im_strip_simple_markdown(r"**加粗**"), "加粗");
        // 反斜杠后面的字符不再被当语法处理，两边都留着，不丢内容。
        assert_eq!(remote_im_strip_simple_markdown(r"a \* b"), r"a \* b");
    }

    #[test]
    fn filter_markdown_content_items_should_only_touch_text_items() {
        let channel = RemoteImChannelConfig {
            short_id: None,
            id: "channel-a".to_string(),
            name: "A".to_string(),
            platform: RemoteImPlatform::Dingtalk,
            enabled: true,
            credentials: serde_json::json!({}),
            receive_files: true,
            streaming_send: false,
            show_tool_calls: false,
            filter_markdown: true,
            allow_send_files: false,
            behavior_settings: RemoteImChannelBehaviorSettings::default(),
        };
        let content = vec![
            serde_json::json!({ "type": "text", "text": "**hello**" }),
            serde_json::json!({ "type": "image", "name": "a.png" }),
        ];

        let filtered = remote_im_filter_markdown_content_items(&channel, content);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0]["text"], "hello");
        assert_eq!(filtered[1]["type"], "image");
    }
}
