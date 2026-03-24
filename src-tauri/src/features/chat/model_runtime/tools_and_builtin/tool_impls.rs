#[derive(Debug, Clone, Copy)]
struct BuiltinFetchTool;

impl Tool for BuiltinFetchTool {
    const NAME: &'static str = "fetch";
    type Error = ToolInvokeError;
    type Args = FetchToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "fetch".to_string(),
            description: "抓取网页文本内容。优先使用其他可用的联网搜索或抓取工具；仅在没有其他网络搜索或抓取能力时再使用。".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "url": { "type": "string", "description": "要抓取的网页地址" },
                "max_length": { "type": "integer", "description": "返回内容的最大字符数", "default": 1800 }
              },
              "required": ["url"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=fetch args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_fetch(&args.url, args.max_length.unwrap_or(1800))
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=fetch result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[工具执行] 内置工具 fetch 执行失败: 错误={err}"),
        }
        result
    }
}

#[derive(Debug, Clone, Copy)]
struct BuiltinBingSearchTool;

impl Tool for BuiltinBingSearchTool {
    const NAME: &'static str = "websearch";
    type Error = ToolInvokeError;
    type Args = BingSearchToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "websearch".to_string(),
            description: "搜索互联网内容。优先使用其他可用的联网搜索或抓取工具；仅在没有其他网络搜索或抓取能力时再使用。".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "query": { "type": "string", "description": "搜索关键词或问题" }
              },
              "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=websearch args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_bing_search(&args.query)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=websearch result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[工具执行] 内置工具 websearch 执行失败: 错误={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinRememberTool {
    app_state: AppState,
}

impl Tool for BuiltinRememberTool {
    const NAME: &'static str = "remember";
    type Error = ToolInvokeError;
    type Args = MemorySaveToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "remember".to_string(),
            description: "保存与用户相关、长期有价值的记忆。禁止保存密码、密钥等敏感信息。"
                .to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "memory_type": { "type": "string", "enum": ["knowledge", "skill", "emotion", "event"], "description": "记忆类型（不支持 task）" },
                "judgment": { "type": "string", "description": "记忆论断，单条可检索判断句" },
                "reasoning": { "type": "string", "description": "理由，可为空" },
                "tags": {
                  "type": "array",
                  "items": { "type": "string" },
                  "description": "标签列表，用于后续命中提示板"
                }
              },
              "required": ["memory_type", "judgment", "tags"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::json!({
            "memoryType": args.memory_type,
            "judgment": args.judgment,
            "reasoning": args.reasoning.unwrap_or_default(),
            "tags": args.tags,
        });
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=remember args={}",
            debug_value_snippet(&args_json, 240)
        );
        let result = builtin_memory_save(&self.app_state, args_json).map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=remember result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => {
                eprintln!("[工具执行] 内置工具 remember 执行失败: 错误={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinRecallTool {
    app_state: AppState,
}

impl Tool for BuiltinRecallTool {
    const NAME: &'static str = "recall";
    type Error = ToolInvokeError;
    type Args = RecallToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "recall".to_string(),
            description: "按查询回忆相关记忆，并返回可直接注入提示词的记忆板。"
                .to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "query": { "type": "string", "description": "回忆查询文本" }
              },
              "required": ["query"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::to_value(&args).unwrap_or(Value::Null);
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=recall args={}",
            debug_value_snippet(&args_json, 240)
        );
        let result = builtin_recall(&self.app_state, &args.query).map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=recall result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[工具执行] 内置工具 recall 执行失败: 错误={err}"),
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinCommandTool {
    app_state: AppState,
    api_config_id: String,
    agent_id: String,
}

fn command_help_text() -> String {
    [
        "command 可用子命令：",
        "- help：查看命令详细说明",
        "- reload：重载工作区 MCP 与技能",
        "- organize_context：整理当前活跃对话上下文",
        "- wait <ms>：等待毫秒（1~120000）",
    ]
    .join("\n")
}

impl Tool for BuiltinCommandTool {
    const NAME: &'static str = "command";
    type Error = ToolInvokeError;
    type Args = CommandToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "command".to_string(),
            description: "统一命令工具。通过 command 文本调用系统能力（help/reload/organize_context/wait）。"
                .to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "command": { "type": "string", "description": "命令文本" }
              },
              "required": ["command"],
              "additionalProperties": false
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let raw = args.command.trim();
        if raw.is_empty() {
            return Err(ToolInvokeError::from(
                "command.command 不能为空，可先执行 `help`。".to_string(),
            ));
        }
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=command command={}",
            debug_text_snippet(raw, 220)
        );
        let mut parts = raw.split_whitespace();
        let op = parts
            .next()
            .map(|item| item.to_ascii_lowercase())
            .unwrap_or_default();
        let result = match op.as_str() {
            "help" => Ok(serde_json::json!({
                "ok": true,
                "command": "help",
                "message": command_help_text(),
            })),
            "reload" => {
                if parts.next().is_some() {
                    Err(ToolInvokeError::from(
                        "reload 不接受参数。".to_string(),
                    ))
                } else {
                    let value = builtin_reload(&self.app_state)
                        .await
                        .map_err(ToolInvokeError::from)?;
                    Ok(serde_json::json!({
                        "ok": true,
                        "command": "reload",
                        "result": value
                    }))
                }
            }
            "organize_context" => {
                if parts.next().is_some() {
                    Err(ToolInvokeError::from(
                        "organize_context 不接受参数。".to_string(),
                    ))
                } else {
                    let value = builtin_organize_context(
                        &self.app_state,
                        &self.api_config_id,
                        &self.agent_id,
                    )
                    .await
                    .map_err(ToolInvokeError::from)?;
                    Ok(serde_json::json!({
                        "ok": true,
                        "command": "organize_context",
                        "result": value
                    }))
                }
            }
            "wait" => {
                let ms_text = parts.next().ok_or_else(|| {
                    ToolInvokeError::from("wait 需要毫秒参数，例如 `wait 800`。".to_string())
                })?;
                if parts.next().is_some() {
                    return Err(ToolInvokeError::from(
                        "wait 仅支持一个毫秒参数。".to_string(),
                    ));
                }
                let ms = ms_text.parse::<u64>().map_err(|_| {
                    ToolInvokeError::from(format!(
                        "wait 毫秒参数必须是正整数，当前收到：{ms_text}"
                    ))
                })?;
                if !(1..=120_000).contains(&ms) {
                    return Err(ToolInvokeError::from(format!(
                        "wait 毫秒参数超出范围，要求 1~120000，当前收到：{ms}"
                    )));
                }
                let value = builtin_desktop_wait(ms)
                    .await
                    .map_err(ToolInvokeError::from)?;
                Ok(serde_json::json!({
                    "ok": true,
                    "command": format!("wait {ms}"),
                    "result": value
                }))
            }
            _ => Err(ToolInvokeError::from(format!(
                "未知命令：`{raw}`。可执行 `help` 查看支持命令。"
            ))),
        };
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=command result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[TOOL-DEBUG] execute_builtin_tool.err name=command err={err}"),
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinTerminalExecTool {
    app_state: AppState,
    session_id: String,
}

impl Tool for BuiltinTerminalExecTool {
    const NAME: &'static str = "exec";
    type Error = ToolInvokeError;
    type Args = TerminalExecToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "exec".to_string(),
            description: terminal_exec_tool_description(&terminal_shell_for_state(&self.app_state)),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "action": { "type": "string", "enum": ["run", "list", "close"], "default": "run" },
                "session_id": { "type": "string", "description": "可选 shell 会话 ID；默认使用当前聊天会话" },
                "command": { "type": "string", "description": "action=run 时要执行的 shell 命令" },
                "timeout_ms": { "type": "integer", "minimum": 1, "maximum": 120000, "default": 20000, "description": "命令超时时间，单位毫秒" }
              },
              "required": []
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::to_value(&args).unwrap_or(Value::Null);
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=exec args={}",
            debug_value_snippet(&args_json, 240)
        );
        let resolved_session_id = args
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(&self.session_id);
        let resolved_action = args
            .action
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("run");
        let resolved_command = args.command.as_deref().map(str::trim).unwrap_or("");
        if resolved_action == "run" && resolved_command.is_empty() {
            return Err(ToolInvokeError::from(
                "shell_exec.command is required when action=run".to_string(),
            ));
        }
        let result = builtin_shell_exec(
            &self.app_state,
            resolved_session_id,
            resolved_action,
            resolved_command,
            args.timeout_ms,
        )
        .await
        .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => {
                eprintln!(
                    "[TOOL-DEBUG] execute_builtin_tool.ok name=exec result={}",
                    debug_value_snippet(v, 240)
                );
                eprintln!(
                    "[TOOL-DEBUG] execute_builtin_tool.ok name=exec summary={}",
                    debug_exec_result_summary(v)
                );
            }
            Err(err) => {
                eprintln!("[工具执行] 内置工具 exec 执行失败: 错误={err}")
            }
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinApplyPatchTool {
    app_state: AppState,
    session_id: String,
}

impl Tool for BuiltinApplyPatchTool {
    const NAME: &'static str = "apply_patch";
    type Error = ToolInvokeError;
    type Args = ApplyPatchToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "apply_patch".to_string(),
            description: [
                "编辑文件的结构化补丁工具。",
                "输入必须是固定补丁格式：以 *** Begin Patch 开始，以 *** End Patch 结束。",
                "支持的文件头有：*** Add File:、*** Delete File:、*** Update File:，以及可选的 *** Move to:。",
                "Update File 的每个 hunk 都必须先写一行 @@ 头。",
                "在 @@ 头之后，hunk 中空格前缀表示上下文，- 表示删除，+ 表示新增。",
                "文件路径必须是绝对路径。",
                "不接受 git diff；不要使用 diff --git、---、+++ 等格式。",
            ].join("\n"),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "input": { "type": "string", "description": "完整补丁文本；必须以 *** Begin Patch 开始并以 *** End Patch 结束" },
                "session_id": { "type": "string", "description": "可选工具会话 ID；默认使用当前聊天会话" }
              },
              "required": ["input"],
              "additionalProperties": false
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let args_json = serde_json::to_value(&args).unwrap_or(Value::Null);
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=apply_patch args={}",
            debug_value_snippet(&args_json, 240)
        );
        let resolved_session_id = args
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(&self.session_id);
        let result = builtin_apply_patch(&self.app_state, resolved_session_id, &args.input)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=apply_patch result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[工具执行] 内置工具 apply_patch 执行失败: 错误={err}"),
        }
        result
    }
}




#[derive(Debug, Clone)]
struct BuiltinTaskTool {
    app_state: AppState,
}

impl Tool for BuiltinTaskTool {
    const NAME: &'static str = "task";
    type Error = ToolInvokeError;
    type Args = TaskToolArgsWire;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "task".to_string(),
            description: "管理持久化任务板。支持 list、get、create、update、complete 五种动作。若未填写 trigger.run_at，任务会立即生效；若填写 trigger.run_at，则在该本地时间执行一次；若同时填写 trigger.every_minutes，则会从 trigger.run_at 开始按分钟重复执行，并且必须同时填写 trigger.end_at 作为停止时间。trigger.run_at 与 trigger.end_at 必须使用当前提示中提供的本地 RFC3339 时间格式，保留时区偏移与秒级精度，不要包含毫秒。".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "action": { "type": "string", "enum": ["list", "get", "create", "update", "complete"] },
                "task_id": { "type": "string" },
                "title": { "type": "string" },
                "cause": { "type": "string" },
                "goal": { "type": "string" },
                "flow": { "type": "string" },
                "todos": { "type": "array", "items": { "type": "string" } },
                "status_summary": { "type": "string" },
                "stage_key": { "type": "string" },
                "append_note": { "type": "string" },
                "completion_state": { "type": "string", "enum": ["completed", "failed_completed"] },
                "completion_conclusion": { "type": "string" },
                "trigger": {
                  "type": "object",
                  "properties": {
                    "run_at": { "type": "string", "description": "可选。本地 RFC3339 执行时间；需保留时区偏移与秒级精度，不要包含毫秒。未填写时任务立即生效" },
                    "every_minutes": { "type": "integer", "minimum": 1, "description": "可选。按分钟重复执行间隔；填写后必须同时提供 trigger.run_at 与 trigger.end_at" },
                    "end_at": { "type": "string", "description": "可选；当填写 trigger.every_minutes 时必填。表示重复任务的停止时间，且必须晚于 trigger.run_at" }
                  }
                }
              },
              "required": ["action"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=task args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_task(&self.app_state, args)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=task result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[工具执行] 内置工具 task 执行失败: 错误={err}"),
        }
        result
    }
}

#[derive(Debug, Clone)]
struct BuiltinDelegateTool {
    app_state: AppState,
    session_id: String,
}

impl Tool for BuiltinDelegateTool {
    const NAME: &'static str = "delegate";
    type Error = ToolInvokeError;
    type Args = DelegateToolArgs;
    type Output = Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "delegate".to_string(),
            description: "向某个部门发起委托。mode=async 仅主助理可用，表示后台处理并立即返回送达结果；mode=sync 表示当前线程等待下级完成后再继续。".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "department_id": { "type": "string", "description": "目标部门 ID" },
                "mode": { "type": "string", "enum": ["async", "sync"], "description": "委托模式。async=仅主助理可用，后台处理并立即返回；sync=当前线程等待结果再继续。" },
                "task_name": { "type": "string", "description": "委托标题" },
                "instruction": { "type": "string", "description": "明确委托内容" },
                "background": { "type": "string", "description": "当前已知背景" },
                "specific_goal": { "type": "string", "description": "具体目标" },
                "deliverable_requirement": { "type": "string", "description": "交付要求" },
                "notify_assistant_when_done": { "type": "boolean", "description": "仅在 mode=async 时生效；完成后是否额外提醒主助理。mode=sync 会忽略此字段。", "default": true }
              },
              "required": ["department_id", "mode", "instruction"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        eprintln!(
            "[TOOL-DEBUG] execute_builtin_tool.start name=delegate args={}",
            debug_value_snippet(&serde_json::to_value(&args).unwrap_or(Value::Null), 240)
        );
        let result = builtin_delegate(&self.app_state, &self.session_id, args)
            .await
            .map_err(ToolInvokeError::from);
        match &result {
            Ok(v) => eprintln!(
                "[TOOL-DEBUG] execute_builtin_tool.ok name=delegate result={}",
                debug_value_snippet(v, 240)
            ),
            Err(err) => eprintln!("[工具执行] 内置工具 delegate 执行失败: 错误={err}"),
        }
        result
    }
}
