// ==================== 前后端展示边界 ====================
// 意图：前端展示专用方法是渲染层投影，后端（摘要 / 压缩 / prompt / 持久化 / 撤回）
// 禁止调用。本文件用两道测试把这条边界钉死：
// 1. 前端专用函数清单：新增、改名、删除都会报错；
// 2. 允许调用的文件清单：后端文件一旦出现调用就会报错。
// TS 侧的等价方法是 projectMessageForDisplay / assistantTextFromStreamBlocks，
// 同样只属于渲染层，后端 Rust 侧无权依赖其输出。

#[cfg(test)]
mod frontend_display_boundary_tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    // 前端展示专用函数清单：`文件相对路径::函数名`，相对路径以 src/ 为根。
    // 改动这里 = 在承认“新增了一个前端展示方法”，必须同步确认后端无人调用。
    const EXPECTED_FRONTEND_ONLY_FNS: &[&str] = &[
        "features/chat/conversation_service/conversation_service_v2.rs::project_tool_history_event_for_frontend_message_display_only",
        "features/chat/conversation_service/conversation_service_v2.rs::project_message_for_frontend_display_only",
        "features/chat/conversation_service/conversation_service_v2.rs::project_messages_for_frontend_display_only",
        "features/chat/conversation_service/conversation_reads.rs::get_recent_messages_for_frontend_display_only",
        "features/chat/conversation_service/conversation_reads.rs::get_message_by_id_for_frontend_display_only",
    ];

    // 允许出现前端专用方法调用的文件清单（相对路径，以 src/ 为根）。
    // 全部是“返回给前端渲染层”的出口：前端读取、快照、事件推送、命令回包。
    // 后端语义文件（摘要 / 压缩 / prompt / 持久化 / 归档 / 撤回）禁止出现在这里；
    // 新增调用方必须先过评审，再更新本清单。
    const ALLOWED_FRONTEND_REF_FILES: &[&str] = &[
        "features/chat/conversation_service/conversation_service_v2.rs",
        "features/chat/conversation_service/conversation_reads.rs",
        "features/chat/conversation_service/remote_im_sessions.rs",
        "features/chat/conversation_service/foreground_lifecycle.rs",
        "features/chat/scheduler.rs",
        "features/chat/scheduler/round_events.rs",
        "features/system/commands/chat_and_runtime/core_commands.rs",
        "features/system/commands/config_and_persona/unarchived_conversations.rs",
        "features/system/commands/ide_context/chat_methods.rs",
    ];

    // 判定口径：同时含 `for_frontend_` 与 `display_only`。
    // 覆盖 `..._for_frontend_display_only` 与 `..._for_frontend_message_display_only` 两种形态。
    const FRONTEND_PART_A: &str = "for_frontend_";
    const FRONTEND_PART_B: &str = "display_only";
    // 边界测试自身只描述清单，不属于产品代码，扫描时跳过，避免自指误报。
    const SELF_FILE_SUFFIX: &str = "frontend_display_boundary.rs";

    fn is_frontend_only_symbol(text: &str) -> bool {
        text.contains(FRONTEND_PART_A) && text.contains(FRONTEND_PART_B)
    }

    fn src_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
    }

    fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = std::fs::read_dir(dir).expect("boundary test: cannot read src dir");
        for entry in entries {
            let entry = entry.expect("boundary test: cannot read dir entry");
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    fn relative_suffix(path: &Path) -> String {
        let root = src_root();
        let normalized = path.to_string_lossy().replace('\\', "/");
        let root_normalized = root.to_string_lossy().replace('\\', "/");
        normalized
            .strip_prefix(root_normalized.trim_end_matches('/'))
            .map(|suffix| suffix.trim_start_matches('/').to_string())
            .unwrap_or(normalized)
    }

    fn extract_frontend_fn_name(line: &str) -> Option<String> {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            return None;
        }
        if !line.contains("fn ") || !is_frontend_only_symbol(line) {
            return None;
        }
        let fn_pos = line.find("fn ")? + "fn ".len();
        let name: String = line[fn_pos..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if is_frontend_only_symbol(&name) && !name.is_empty() {
            Some(name)
        } else {
            None
        }
    }

    fn line_has_code_ref(line: &str) -> bool {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            return false;
        }
        // 去掉行尾注释，避免注释里提及方法名被误判为调用。
        let code_part = line.split("//").next().unwrap_or(line);
        is_frontend_only_symbol(code_part)
    }

    fn should_skip_path(path: &Path) -> bool {
        path.to_string_lossy().replace('\\', "/").ends_with(SELF_FILE_SUFFIX)
    }

    fn collect_definitions() -> BTreeSet<String> {
        let mut files = Vec::new();
        collect_rs_files(&src_root(), &mut files);
        let mut found = BTreeSet::new();
        for path in files {
            if should_skip_path(&path) {
                continue;
            }
            let content =
                std::fs::read_to_string(&path).expect("boundary test: cannot read rs file");
            let suffix = relative_suffix(&path);
            for line in content.lines() {
                if let Some(name) = extract_frontend_fn_name(line) {
                    found.insert(format!("{suffix}::{name}"));
                }
            }
        }
        found
    }

    fn collect_code_ref_files() -> BTreeSet<String> {
        let mut files = Vec::new();
        collect_rs_files(&src_root(), &mut files);
        let mut found = BTreeSet::new();
        for path in files {
            if should_skip_path(&path) {
                continue;
            }
            let content =
                std::fs::read_to_string(&path).expect("boundary test: cannot read rs file");
            if content.lines().any(line_has_code_ref) {
                found.insert(relative_suffix(&path));
            }
        }
        found
    }

    #[test]
    fn frontend_only_definition_inventory_should_match_allowlist() {
        let found = collect_definitions();
        let expected: BTreeSet<String> = EXPECTED_FRONTEND_ONLY_FNS
            .iter()
            .map(|item| item.to_string())
            .collect();
        if found != expected {
            let unexpected: Vec<_> = found.difference(&expected).collect();
            let missing: Vec<_> = expected.difference(&found).collect();
            panic!(
                "前端展示专用方法清单变化：这些是前端渲染用方法，后端（摘要/压缩/prompt/持久化/撤回）禁止使用。\n新增或改名：{unexpected:?}\n已删除或移动：{missing:?}\n若确认是前端展示用途，先更新 EXPECTED_FRONTEND_ONLY_FNS，再检查后端无人调用。"
            );
        }
    }

    #[test]
    fn backend_paths_should_not_call_frontend_display_only() {
        let found = collect_code_ref_files();
        let allowed: BTreeSet<String> = ALLOWED_FRONTEND_REF_FILES
            .iter()
            .map(|item| item.to_string())
            .collect();
        if found != allowed {
            let unexpected: Vec<_> = found.difference(&allowed).collect();
            let removed: Vec<_> = allowed.difference(&found).collect();
            panic!(
                "前端展示方法被非出口文件引用：这些是前端渲染用方法，后端语义文件禁止调用。\n新增引用文件（先查是否为后端误用）：{unexpected:?}\n不再引用的文件（请同步收紧清单）：{removed:?}\n后端需要正文请用 get_raw_message_by_id / render_prompt_message_text / preserved_conversation_message_backend_text。"
            );
        }
    }

    #[test]
    fn extract_frontend_fn_name_should_parse_definition() {
        let line = "fn project_message_for_frontend_display_only(mut message: ChatMessage) -> ChatMessage {";
        assert_eq!(
            extract_frontend_fn_name(line).as_deref(),
            Some("project_message_for_frontend_display_only")
        );
        assert_eq!(
            extract_frontend_fn_name(
                "fn project_tool_history_event_for_frontend_message_display_only(event: &Value) -> Value {"
            )
            .as_deref(),
            Some("project_tool_history_event_for_frontend_message_display_only")
        );
        assert_eq!(
            extract_frontend_fn_name(
                "// 见 project_message_for_frontend_display_only，注释不算定义"
            ),
            None
        );
        assert_eq!(extract_frontend_fn_name("fn render_prompt_message_text(m: &M)"), None);
    }

    #[test]
    fn line_has_code_ref_should_ignore_comments() {
        assert!(line_has_code_ref(
            "    messages: project_messages_for_frontend_display_only(messages),"
        ));
        assert!(!line_has_code_ref(
            "// 前端展示串禁止进入摘要，见 project_message_for_frontend_display_only。"
        ));
        assert!(!line_has_code_ref(
            "    let text = render_prompt_message_text(message); // 后端正文"
        ));
    }
}
