const QUESTION_TOOL_NAME: &str = "question";

fn question_provider_tool_definition() -> ProviderToolDefinition {
    ProviderToolDefinition::new(
        QUESTION_TOOL_NAME,
        "会话提问工具：把需要用户拍板的问题写进当前会话的问题面板，替代在回复正文里罗列问题（正文里的问题会被后续消息刷走）。\
         用户会在输入框上方看到带编号的题目卡片，选完即作为一条普通用户消息队列发送给你。\
         动作只有两个：`list` 读取当前会话的全部问题；`update` 新建、修改或清理问题。\
         编号由系统分配，在会话内单调递增且不回收：`update` 不带 number 即新建，编号取当前最大值加一；\
         带 number 即定位该编号，不存在会报错而不是新建。\
         清理的写法是把 title、background、options 三项同时留空并带上 number。\
         单会话同时最多保留 8 条问题，超过会报错，请先清理已回答的问题。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "update"],
                    "description": "list：列出当前会话的全部问题；update：按 items 新建、修改或清理。"
                },
                "items": {
                    "type": "array",
                    "description": "update 时的条目列表，可以一次提交多条。每条的处置规则：带 number 且三项全空 = 清理该编号；带 number 且有内容 = 覆盖该编号；不带 number 且有内容 = 新建。",
                    "items": {
                        "type": "object",
                        "properties": {
                            "number": {
                                "type": "integer",
                                "description": "要修改或清理的问题编号。新建时不要传。编号不存在会报错，请先 list 查看当前问题。"
                            },
                            "title": {
                                "type": "string",
                                "description": "问题本身，一句话说清要用户拍板什么，不超过 120 字。"
                            },
                            "background": {
                                "type": "string",
                                "description": "问题背景，可选。问题较复杂、需要补充上下文时才写，不超过 1000 字。"
                            },
                            "options": {
                                "type": "array",
                                "description": "选项文本列表，1~6 个，用户单选。每个选项不超过 60 字，要能直接作为答复发回给你。",
                                "items": { "type": "string" }
                            }
                        },
                        "required": []
                    }
                }
            },
            "required": ["action"]
        }),
    )
}

fn question_target_conversation_id(session_id: &str) -> Result<String, String> {
    delegate_session_conversation_id(session_id)
        .ok_or_else(|| "question 工具缺少 conversation_id，无法定位当前会话".to_string())
}

/// 执行层严判：工具可能经由其他会话的上下文被调用，不能只靠装配层拦。
fn question_main_chat_conversation(
    state: &AppState,
    session_id: &str,
) -> Result<String, String> {
    let conversation_id = question_target_conversation_id(session_id)?;
    let meta = conversation_service_v2()
        .get_conversation_meta(state, &conversation_id)
        .map_err(|_| format!("question 工具无法定位会话：{conversation_id}"))?;
    if !conversation_kind_is_local_main_chat(&meta.conversation_kind) {
        return Err(format!(
            "question 工具仅在本地主会话中可用，当前会话类型为 {}",
            meta.conversation_kind.trim()
        ));
    }
    Ok(conversation_id)
}

/// 会话提问工具执行入口：校验主会话 → 执行动作 → 回返完整问题列表。
fn builtin_question(
    app_state: &AppState,
    session_id: &str,
    request: QuestionToolRequest,
) -> Result<String, String> {
    let conversation_id = question_main_chat_conversation(app_state, session_id)?;
    let is_update = request.action.trim().eq_ignore_ascii_case("update");
    let views = question_run_tool(app_state, &conversation_id, request)?;
    if is_update {
        // 回执不复述刚提交的 title / background / options：那是入参原文，模型自己刚写过。
        return Ok(format!("已更新当前会话问题，共 {} 条。", views.len()));
    }
    Ok(question_list_text(&views))
}
