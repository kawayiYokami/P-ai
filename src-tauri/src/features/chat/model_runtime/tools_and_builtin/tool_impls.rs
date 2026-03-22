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
            description: "Fetch webpage text. Prefer other available network search/fetch tools first; use this only when no other network search or fetch tool is available.".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "url": { "type": "string", "description": "URL" },
                "max_length": { "type": "integer", "description": "Max chars", "default": 1800 }
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
            description: "Search the web. Prefer other available network search/fetch tools first; use this only when no other network search or fetch tool is available.".to_string(),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "query": { "type": "string", "description": "Query" }
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
                "command": { "type": "string", "description": "命令文本，例如 `reload`、`wait 800`" }
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
                "session_id": { "type": "string", "description": "Optional shell session id; defaults to current chat session id." },
                "command": { "type": "string", "description": "Shell command to execute when action=run" },
                "timeout_ms": { "type": "integer", "minimum": 1, "maximum": 120000, "default": 20000 }
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
            description: format!(
                "{}\nUse the `apply_patch` tool to edit files.\nPatch format:\n*** Begin Patch\n[ one or more file sections ]\n*** End Patch\n\nFile headers:\n*** Add File: <path>\n*** Delete File: <path>\n*** Update File: <path>\n(optional) *** Move to: <new path>\n\nHunk lines:\n@@\n<space> context line\n- removed line\n+ added line\n\nImportant:\n- Input must be Codex patch, NOT git diff (`diff --git ...`).\n- Paths must be relative.\n\nMinimal examples:\n*** Begin Patch\n*** Add File: notes.txt\n+hello\n*** End Patch\n\n*** Begin Patch\n*** Update File: src/main.ts\n@@\n-old\n+new\n*** End Patch\n\n*** Begin Patch\n*** Delete File: old.txt\n*** End Patch",
                apply_patch_tool_description()
            ),
            parameters: serde_json::json!({
              "type": "object",
              "properties": {
                "input": { "type": "string", "description": "Full patch text. Must start with *** Begin Patch and end with *** End Patch." },
                "session_id": { "type": "string", "description": "Optional tool session id; defaults to current chat session id." }
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
            description: "Manage the persistent task board. Use action=list|get|create|update|complete. Trigger rules: if trigger.run_at is omitted, the task becomes active immediately; if trigger.run_at is set, it runs once at that local time; if trigger.every_minutes is also set, it repeats every N minutes starting from trigger.run_at and must also include trigger.end_at as the stop time. When writing trigger.run_at or trigger.end_at, copy the local RFC3339 time format shown in the hidden task/current-time hints, including timezone offset and second precision; do not use milliseconds.".to_string(),
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
                    "run_at": { "type": "string", "description": "Optional. Copy the same local RFC3339 format shown in the task/current-time hints, for example 2026-03-10T09:30:00+08:00. Include timezone offset, keep second precision, and do not include milliseconds. If omitted, the task becomes active immediately." },
                    "every_minutes": { "type": "integer", "minimum": 1, "description": "Optional. If set, repeat every N minutes starting from trigger.run_at. Requires trigger.run_at and trigger.end_at." },
                    "end_at": { "type": "string", "description": "Optional unless trigger.every_minutes is set. Defines when a repeating task stops. Must be later than trigger.run_at. Use the same local RFC3339 format shown in the task/current-time hints." }
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
