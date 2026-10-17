import { fileNameFromPath } from "./chat-attachment-display";

export type TranslateFn = (key: string, params?: Record<string, string | number>) => string;

export type ToolCallPresentationOptions = {
  t: TranslateFn;
  agentName: (agentId: string) => string;
};

export type SemanticToolPresentation = {
  action: string;
  fileExt?: string;
  target: string;
  lineRange?: string;
  extra?: string;
  adds?: number;
  removes?: number;
  text: string;
};

export function createToolCallPresentation(options: ToolCallPresentationOptions) {
  const t = options.t;

  const internalToolNames = new Set<string>([
    "apply_patch",
    "exec",
    "shell_exec",
    "read",
    "read_file",
    "read_media",
    "write",
    "delete",
    "update",
    "move",
    "write_file",
    "append_text",
    "delete_file",
    "create_file",
    "rename_file",
    "move_file",
    "list_dir",
    "read_dir",
    "find",
    "search",
    "todo",
    "plan",
    "create_goal",
    "update_goal",
    "get_session",
    "background",
    "inform_session",
    "task",
    "deeprecall",
    "deeprecall_search",
    "deeprecall_context",
    "delegate",
    "contact_send_files",
    "meme",
    "image_generate",
    "image_edit",
    "image_to_video",
    "remember",
    "recall",
    "fetch",
    "websearch",
    "operate",
    "wait",
    "team_info",
    "team_recruit",
    "team_dismiss",
    "team_send",
    "team_report",
    "config",
    "pai_config",
    "configure_pai",
    "app_config",
  ]);

  function normalizeToolCallArgs(argsText: string): unknown {
    const text = String(argsText || "").trim();
    if (!text) return undefined;
    try {
      return JSON.parse(text);
    } catch {
      return text;
    }
  }

  function toolTimelineText(key: string, params?: Record<string, string | number>): string {
    return String(t(`status.toolTimeline.${key}`, params ?? {}));
  }

  function toolTimelineNameValue(name: string, value: string): string {
    return `${name}：${value}`;
  }

  function compactText(text: string, maxLen = 120): string {
    const trimmed = text.replace(/\s+/g, " ").trim();
    if (trimmed.length <= maxLen) return trimmed;
    return `${trimmed.slice(0, maxLen - 3)}...`;
  }

  function joinNonEmpty(parts: (string | undefined | null)[], separator = " · "): string {
    return parts
      .map((part) => (typeof part === "string" ? part.trim() : ""))
      .filter(Boolean)
      .join(separator);
  }

  function safeStringValue(data: Record<string, unknown>, key: string): string {
    const value = data[key];
    return typeof value === "string" ? value.trim() : "";
  }

  function safeTextFromRecord(data: Record<string, unknown>, keys: string[]): string {
    for (const key of keys) {
      const value = data[key];
      if (typeof value === "string") {
        const trimmed = value.trim();
        if (trimmed) return trimmed;
      }
      if (Array.isArray(value)) {
        const joined = value
          .map((item) => (typeof item === "string" ? item.trim() : ""))
          .filter(Boolean)
          .join(" ");
        if (joined) return joined;
      }
    }
    return "";
  }

  function countTextLines(text: string): number {
    const normalized = String(text || "").replace(/\r\n/g, "\n");
    if (!normalized.trim()) return 0;
    return normalized.split("\n").length;
  }

  function extractFileExt(filePath: string): string {
    const name = fileNameFromPath(filePath);
    const dotIndex = name.lastIndexOf(".");
    if (dotIndex <= 0 || dotIndex === name.length - 1) return "";
    const ext = name.slice(dotIndex + 1).toLowerCase();
    if (ext.length > 8 || /[^a-z0-9_-]/i.test(ext)) return "";
    return ext;
  }

  function extractLineRange(argsObj: Record<string, unknown>, pathStr?: string): string {
    if (pathStr) {
      const match = pathStr.match(/:(\d+(?:-\d+)?)$/);
      if (match?.[1]) return `#${match[1]}`;
    }
    const startLine = argsObj.start_line ?? argsObj.startLine;
    const endLine = argsObj.end_line ?? argsObj.endLine;
    if (startLine !== undefined && startLine !== null && endLine !== undefined && endLine !== null) {
      return `#${startLine}-${endLine}`;
    }
    if (startLine !== undefined && startLine !== null) {
      return `#${startLine}`;
    }
    const offset = argsObj.offset ?? argsObj.start;
    const limit = argsObj.limit ?? argsObj.count;
    if (offset !== undefined && offset !== null && limit !== undefined && limit !== null) {
      const numOffset = Number(offset);
      const numLimit = Number(limit);
      if (!Number.isNaN(numOffset) && !Number.isNaN(numLimit)) {
        return `#${numOffset}-${numOffset + numLimit}`;
      }
      return `#${offset}-${limit}`;
    }
    if (offset !== undefined && offset !== null) {
      return `#${offset}`;
    }
    if (argsObj.line !== undefined && argsObj.line !== null) {
      return `#${argsObj.line}`;
    }
    return "";
  }

  function collapseCommandForSummary(command: string): string {
    const text = String(command || "");
    const lines = text.split(/\r\n|\n|\r/);
    const firstLine = lines[0].replace(/\s+/g, " ").trim();
    if (lines.length <= 1) return firstLine;
    return `${firstLine} (${lines.length} 行)`;
  }

  function toSingleLineJsonText(payload: unknown): string {
    if (payload === undefined || payload === null) return "";
    if (typeof payload === "string") return payload.trim() || "";
    try {
      return JSON.stringify(payload);
    } catch {
      return String(payload);
    }
  }

  function compactSingleLineJson(payload: unknown, maxLen = 120): string {
    const text = toSingleLineJsonText(payload);
    if (!text) return "";
    const oneLine = text.replace(/\s+/g, " ").trim();
    if (oneLine.length <= maxLen) return oneLine;
    return `${oneLine.slice(0, maxLen - 3)}...`;
  }

  function toCompactValue(value: unknown, depth = 0): string {
    if (value === undefined || value === null) return "";
    if (typeof value === "string") return value.trim();
    if (typeof value === "number" || typeof value === "boolean") return String(value);
    if (depth > 1) return "";

    if (Array.isArray(value)) {
      const parts = value
        .map((item) => toCompactValue(item, depth + 1))
        .filter((item) => item !== "")
        .slice(0, 3);
      return parts.join(" | ");
    }

    if (typeof value === "object") {
      const obj = value as Record<string, unknown>;
      const orderedKeys = [
        "path",
        "file",
        "target",
        "source",
        "destination",
        "from",
        "to",
        "command",
        "cmd",
        "url",
        "query",
        "name",
        "id",
        "text",
        "content",
        "input",
        "output",
        "method",
      ];

      for (const key of orderedKeys) {
        const valueText = toCompactValue(obj[key], depth + 1);
        if (valueText) return `${key}: ${valueText}`;
      }

      const pairs = Object.entries(obj)
        .map(([key, rawValue]) => {
          const compactVal = toCompactValue(rawValue, depth + 1);
          return compactVal ? `${key}: ${compactVal}` : "";
        })
        .filter(Boolean)
        .slice(0, 2);
      if (pairs.length > 0) {
        return pairs.join("；");
      }
    }

    return "";
  }

  function taskTriggerSummary(value: unknown): string {
    if (typeof value !== "object" || value === null) return "";
    const obj = value as Record<string, unknown>;
    return joinNonEmpty([
      safeStringValue(obj, "run_at") || safeStringValue(obj, "runAt") || safeStringValue(obj, "runAtLocal"),
      safeStringValue(obj, "cron_expression")
        ? toolTimelineNameValue("cron", safeStringValue(obj, "cron_expression"))
        : (safeStringValue(obj, "cronExpression")
          ? toolTimelineNameValue("cron", safeStringValue(obj, "cronExpression"))
          : (safeStringValue(obj, "every_minutes")
            ? toolTimelineNameValue("everyMinutes", safeStringValue(obj, "every_minutes"))
            : (safeStringValue(obj, "everyMinutes")
              ? toolTimelineNameValue("everyMinutes", safeStringValue(obj, "everyMinutes"))
              : ""))),
      safeStringValue(obj, "end_at")
        ? toolTimelineText("until", { time: safeStringValue(obj, "end_at") })
        : (safeStringValue(obj, "endAt")
          ? toolTimelineText("until", { time: safeStringValue(obj, "endAt") })
          : (safeStringValue(obj, "endAtLocal")
            ? toolTimelineText("until", { time: safeStringValue(obj, "endAtLocal") })
            : "")),
    ]);
  }

  function delegateModeDisplayText(mode: string): string {
    const normalized = mode.trim().toLowerCase();
    if (normalized === "wait" || normalized === "sync") return "等待结果";
    if (normalized === "background" || normalized === "async") return "后台运行";
    return mode.trim();
  }

  function delegateAgentDisplayText(agentId: string): string {
    const normalized = agentId.trim();
    if (!normalized) return "";
    return String(options.agentName(normalized) || "").trim() || normalized;
  }

  function formatFullText(action: string, target: string, lineRange?: string, extra?: string): string {
    const parts = [action, target, lineRange, extra].filter(Boolean);
    return parts.join(" ");
  }

  // ==================== 语义化解析入口 ====================

  function toolCallSemanticPresentation(toolCall: {
    name: string;
    argsText: string;
    status?: "doing" | "done";
  }): SemanticToolPresentation {
    const toolName = String(toolCall.name || "").trim() || "unknown";
    const args = normalizeToolCallArgs(toolCall.argsText);
    const obj = (typeof args === "object" && args !== null ? args : {}) as Record<string, unknown>;

    // 1. 阅读文件 (read / read_file)
    if (toolName === "read" || toolName === "read_file") {
      const rawPath = safeTextFromRecord(obj, ["absolute_path", "absolutePath", "path", "file"]) || (typeof args === "string" ? args.trim() : "");
      const target = fileNameFromPath(rawPath) || toolTimelineText("missingArgs");
      const lineRange = extractLineRange(obj, rawPath);
      const action = toolTimelineText("actionRead");
      return {
        action,
        fileExt: extractFileExt(target),
        target,
        lineRange,
        text: formatFullText(action, target, lineRange),
      };
    }

    // 2. 阅读媒体 (read_media)
    if (toolName === "read_media") {
      const rawPath = safeTextFromRecord(obj, ["path", "absolute_path", "absolutePath", "file"]);
      const target = fileNameFromPath(rawPath) || toolTimelineText("missingArgs");
      const action = toolTimelineText("actionRead");
      const desc = safeTextFromRecord(obj, ["description", "focus", "prompt"]);
      return {
        action,
        fileExt: extractFileExt(target),
        target,
        extra: desc ? compactText(desc, 40) : undefined,
        text: formatFullText(action, target, undefined, desc ? compactText(desc, 40) : undefined),
      };
    }

    // 3. 更新文件 (update)
    if (toolName === "update") {
      const rawPath = safeTextFromRecord(obj, ["path", "file", "absolute_path", "absolutePath", "target"]);
      const target = fileNameFromPath(rawPath) || toolTimelineText("missingArgs");
      const oldLines = countTextLines(String(obj.oldString || obj.old_string || ""));
      const newLines = countTextLines(String(obj.newString || obj.new_string || ""));
      const action = toolTimelineText("actionUpdate");
      return {
        action,
        fileExt: extractFileExt(target),
        target,
        adds: newLines,
        removes: oldLines,
        text: formatFullText(action, target),
      };
    }

    // 4. 写入文件 (write / write_file / create_file / append_text)
    if (toolName === "write" || toolName === "write_file" || toolName === "create_file" || toolName === "append_text") {
      const rawPath = safeTextFromRecord(obj, ["path", "file", "absolute_path", "absolutePath", "target"]);
      const target = fileNameFromPath(rawPath) || toolTimelineText("missingArgs");
      const lines = countTextLines(String(obj.content || obj.text || ""));
      const action = toolTimelineText("actionWrite");
      return {
        action,
        fileExt: extractFileExt(target),
        target,
        adds: lines,
        text: formatFullText(action, target),
      };
    }

    // 5. 删除文件 (delete / delete_file)
    if (toolName === "delete" || toolName === "delete_file") {
      const rawPath = safeTextFromRecord(obj, ["path", "file", "absolute_path", "absolutePath", "target"]);
      const target = fileNameFromPath(rawPath) || toolTimelineText("missingArgs");
      const action = toolTimelineText("actionDelete");
      return {
        action,
        fileExt: extractFileExt(target),
        target,
        text: formatFullText(action, target),
      };
    }

    // 6. 移动文件 (move / move_file / rename_file)
    if (toolName === "move" || toolName === "move_file" || toolName === "rename_file") {
      const fromPath = safeTextFromRecord(obj, ["source", "from", "old_path", "oldPath", "path"]);
      const toPath = safeTextFromRecord(obj, ["destination", "to", "target", "new_path", "newPath"]);
      const fromName = fileNameFromPath(fromPath);
      const toName = fileNameFromPath(toPath);
      const target = fromName && toName ? `${fromName} → ${toName}` : (fromName || toName || toolTimelineText("missingArgs"));
      const action = toolTimelineText("actionMove");
      return {
        action,
        fileExt: extractFileExt(toName || fromName),
        target,
        text: formatFullText(action, target),
      };
    }

    // 7. 应用补丁 (apply_patch)
    if (toolName === "apply_patch") {
      const action = toolTimelineText("actionPatch");
      const patchInput = typeof args === "string" ? args : safeTextFromRecord(obj, ["input", "patch", "diff"]);
      const fileNames: string[] = [];
      let adds = 0;
      let removes = 0;

      if (patchInput) {
        for (const line of patchInput.split(/\r?\n/)) {
          const updateMatch = line.match(/^\*\*\* Update File:\s+(.+)$/);
          const addMatch = line.match(/^\*\*\* Add File:\s+(.+)$/);
          const delMatch = line.match(/^\*\*\* Delete File:\s+(.+)$/);
          const gitMatch = line.match(/^diff --git\s+(?:a\/|\S+)\s+(?:b\/|\S+)(.+)$/);
          const filePath = updateMatch?.[1] || addMatch?.[1] || delMatch?.[1] || gitMatch?.[1];
          if (filePath) {
            fileNames.push(fileNameFromPath(filePath));
          }
          if (line.startsWith("+") && !line.startsWith("+++")) adds += 1;
          if (line.startsWith("-") && !line.startsWith("---")) removes += 1;
        }
      }

      const explicitFile = safeTextFromRecord(obj, ["file", "target", "path", "files"]);
      if (explicitFile) fileNames.push(fileNameFromPath(explicitFile));

      const uniqueFiles = Array.from(new Set(fileNames.filter(Boolean)));
      const target = uniqueFiles.length > 0 ? uniqueFiles.slice(0, 3).join(", ") : toolTimelineText("inlinePatch");
      const fileExt = uniqueFiles[0] ? extractFileExt(uniqueFiles[0]) : undefined;
      return {
        action,
        fileExt,
        target,
        adds: adds || undefined,
        removes: removes || undefined,
        text: formatFullText(action, target),
      };
    }

    // 8. 执行命令 (exec / shell_exec)
    if (toolName === "exec" || toolName === "shell_exec") {
      const rawCmd = safeTextFromRecord(obj, ["command", "cmd", "shell", "input", "commandText"])
        || (typeof args === "string" ? args : safeTextFromRecord(obj, ["args", "arguments"]));
      const target = collapseCommandForSummary(rawCmd) || toolTimelineText("notProvided");
      const action = toolTimelineText("actionExec");
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    // 9. 配置 PAI (config / pai_config / configure_pai / app_config)
    if (toolName === "config" || toolName === "pai_config" || toolName === "configure_pai" || toolName === "app_config") {
      const action = toolTimelineText("actionConfig");
      const cmd = safeStringValue(obj, "command") || safeStringValue(obj, "cmd") || (typeof args === "string" ? args : "");
      const target = cmd ? collapseCommandForSummary(cmd) : (toCompactValue(args) || toolTimelineText("checkArgs"));
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    // 10. 计划 (plan)
    if (toolName === "plan") {
      const action = toolTimelineText("actionPlan");
      const rawPath = safeStringValue(obj, "path") || (typeof args === "string" ? args : "");
      const target = fileNameFromPath(rawPath) || rawPath || toolTimelineText("missingArgs");
      const act = safeStringValue(obj, "action");
      const extra = act === "complete" ? "完成" : (act === "present" ? "提交" : act || undefined);
      return {
        action,
        fileExt: extractFileExt(target),
        target,
        extra,
        text: formatFullText(action, target, undefined, extra),
      };
    }

    // 11. 待办清单 (todo)
    if (toolName === "todo") {
      const action = toolTimelineText("actionTodo");
      const todos = obj.todos;
      if (Array.isArray(todos)) {
        const counts = todos.reduce((acc, item) => {
          const status = typeof item === "object" && item !== null ? String((item as Record<string, unknown>).status || "pending") : "pending";
          acc[status] = (acc[status] || 0) + 1;
          return acc;
        }, {} as Record<string, number>);
        const parts = [
          toolTimelineText("todoItems", { count: todos.length }),
          counts.in_progress ? toolTimelineText("todoInProgress", { count: counts.in_progress }) : "",
        ].filter(Boolean);
        const target = parts.join(" ");
        return {
          action,
          target,
          text: formatFullText(action, target),
        };
      }
      return {
        action,
        target: compactSingleLineJson(args, 40),
        text: formatFullText(action, compactSingleLineJson(args, 40)),
      };
    }

    // 12. 目标管理 (create_goal / update_goal)
    if (toolName === "create_goal" || toolName === "update_goal") {
      const action = toolTimelineText("actionGoal");
      const objText = safeStringValue(obj, "objective");
      const status = safeStringValue(obj, "status");
      const evidence = safeStringValue(obj, "evidence") || safeStringValue(obj, "blocking_condition");
      const statusLabel = status === "complete" ? "完成" : (status === "blocked" ? "阻塞" : status);
      const target = compactText(objText || (statusLabel && evidence ? `${statusLabel}: ${evidence}` : (statusLabel || toCompactValue(args))), 50);
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    // 13. 会话查询与治理 (get_session / background / inform_session)
    if (toolName === "get_session") {
      const action = toolTimelineText("actionGetSession");
      const kw = safeStringValue(obj, "keyword") || (typeof args === "string" ? args : "");
      const target = kw ? compactText(kw, 40) : "全部会话";
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    if (toolName === "background") {
      const action = toolTimelineText("actionBackground");
      const act = safeStringValue(obj, "action") || "list";
      const id = safeStringValue(obj, "id");
      const target = id ? `${act} ${compactText(id, 30)}` : act;
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    if (toolName === "inform_session") {
      const action = toolTimelineText("actionInformSession");
      const sessionId = safeStringValue(obj, "session_id");
      const content = safeStringValue(obj, "content");
      const target = sessionId ? compactText(sessionId, 30) : (toCompactValue(args) || toolTimelineText("missingArgs"));
      const extra = content ? compactText(content, 30) : undefined;
      return {
        action,
        target,
        extra,
        text: formatFullText(action, target, undefined, extra),
      };
    }

    // 14. 定时任务 (task)
    if (toolName === "task") {
      const action = toolTimelineText("actionTask");
      const act = safeStringValue(obj, "action");
      const goal = safeStringValue(obj, "goal");
      const trigger = taskTriggerSummary(obj.trigger);
      const target = compactText(joinNonEmpty([act, goal || trigger]), 50) || toCompactValue(args);
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    // 15. 深度回忆 (deeprecall / deeprecall_search / deeprecall_context)
    if (toolName === "deeprecall" || toolName === "deeprecall_search") {
      const action = toolTimelineText("actionDeepRecall");
      const query = safeStringValue(obj, "query") || (typeof args === "string" ? args : "");
      const target = compactText(query || toCompactValue(args), 50);
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    if (toolName === "deeprecall_context") {
      const action = toolTimelineText("actionDeepRecall");
      const convIdx = obj.conversation_index ?? obj.conversationIndex;
      const startIdx = obj.start_index ?? obj.startIndex;
      const endIdx = obj.end_index ?? obj.endIndex;
      const range = (startIdx !== undefined && endIdx !== undefined) ? `#${startIdx}-${endIdx}` : (startIdx ? `#${startIdx}` : "");
      const target = convIdx !== undefined ? `会话 ${convIdx}${range ? ` ${range}` : ""}` : (toCompactValue(args) || toolTimelineText("missingArgs"));
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    // 16. 委托 (delegate)
    if (toolName === "delegate") {
      const action = toolTimelineText("actionDelegate");
      const targetAgent = delegateAgentDisplayText(safeStringValue(obj, "agent_id"));
      const content = compactText(
        safeStringValue(obj, "goal")
          || safeStringValue(obj, "task_name")
          || safeStringValue(obj, "question")
          || safeStringValue(obj, "specific_goal")
          || safeStringValue(obj, "instruction"),
        40,
      );
      const target = targetAgent ? (content ? `${targetAgent}：${content}` : targetAgent) : content || toCompactValue(args);
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    // 17. 记忆 (remember / recall)
    if (toolName === "remember") {
      const action = toolTimelineText("actionRemember");
      const mem = (obj.memory && typeof obj.memory === "object" ? obj.memory : {}) as Record<string, unknown>;
      const judgment = safeStringValue(mem, "judgment") || safeStringValue(obj, "judgment");
      const memType = safeStringValue(mem, "memoryType") || safeStringValue(obj, "memoryType");
      const act = safeStringValue(obj, "action");
      const target = compactText(judgment || toCompactValue(args), 50);
      const extraParts = [act, memType].filter(Boolean);
      const extra = extraParts.length > 0 ? extraParts.join(" · ") : undefined;
      return {
        action,
        target,
        extra,
        text: formatFullText(action, target, undefined, extra),
      };
    }

    if (toolName === "recall") {
      const action = toolTimelineText("actionRecall");
      const query = safeStringValue(obj, "query");
      const time = safeStringValue(obj, "time");
      const target = query ? compactText(query, 50) : (time ? `时间: ${time}` : "全部可见记忆");
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    // 18. 网络 / 网页搜索 (fetch / websearch / tavily_*)
    if (toolName === "fetch") {
      const action = toolTimelineText("actionFetch");
      const target = safeStringValue(obj, "url") || toCompactValue(args);
      return {
        action,
        target: compactText(target, 50),
        text: formatFullText(action, compactText(target, 50)),
      };
    }

    if (toolName === "websearch") {
      const action = toolTimelineText("actionWebSearch");
      const target = safeStringValue(obj, "query") || safeStringValue(obj, "url") || toCompactValue(args);
      return {
        action,
        target: compactText(target, 50),
        text: formatFullText(action, compactText(target, 50)),
      };
    }

    // 19. 桌面操作 (operate)
    if (toolName === "operate") {
      const action = toolTimelineText("actionOperate");
      const script = safeStringValue(obj, "script");
      const lines = script ? script.split(/\r?\n/).map((l) => l.trim()).filter(Boolean) : [];
      let target = "";
      if (lines.length > 0) {
        target = lines.length > 1 ? `${compactText(lines[0], 40)} (${lines.length} 步)` : compactText(lines[0], 50);
      } else {
        target = toCompactValue(args) || toolTimelineText("missingArgs");
      }
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    // 20. 联系人发送附件 (contact_send_files)
    if (toolName === "contact_send_files") {
      const action = toolTimelineText("actionContactSendFiles");
      const filePaths = Array.isArray(obj.file_paths) ? obj.file_paths : (Array.isArray(obj.files) ? obj.files : []);
      const fileNames = filePaths.map((p) => fileNameFromPath(p)).filter(Boolean);
      const target = fileNames.length > 0 ? compactText(fileNames.join(", "), 50) : (toCompactValue(args) || toolTimelineText("missingArgs"));
      return {
        action,
        fileExt: fileNames[0] ? extractFileExt(fileNames[0]) : undefined,
        target,
        text: formatFullText(action, target),
      };
    }

    // 21. 表情收藏 (meme)
    if (toolName === "meme") {
      const action = toolTimelineText("actionMeme");
      const emotion = safeStringValue(obj, "emotion");
      const rawPath = safeStringValue(obj, "path");
      const fileName = fileNameFromPath(rawPath);
      const target = emotion ? `:${emotion}:` : (fileName || toCompactValue(args));
      return {
        action,
        target,
        extra: emotion && fileName ? fileName : undefined,
        text: formatFullText(action, target),
      };
    }

    // 22. 图像生成与编辑 (image_generate / image_edit)
    if (toolName === "image_generate") {
      const action = toolTimelineText("actionImageGenerate");
      const prompt = safeStringValue(obj, "prompt") || (typeof args === "string" ? args : "");
      const resolution = safeStringValue(obj, "resolution");
      const target = compactText(prompt || toCompactValue(args), 50);
      return {
        action,
        target,
        extra: resolution || undefined,
        text: formatFullText(action, target, undefined, resolution || undefined),
      };
    }

    if (toolName === "image_to_video") {
      const action = toolTimelineText("actionImageToVideo");
      const prompt = safeStringValue(obj, "prompt") || (typeof args === "string" ? args : "");
      const image = fileNameFromPath(safeStringValue(obj, "image"));
      const target = compactText(prompt || image || toCompactValue(args), 50);
      return {
        action,
        target,
        extra: image || undefined,
        text: formatFullText(action, target, undefined, image || undefined),
      };
    }

    if (toolName === "image_edit") {
      const action = toolTimelineText("actionImageEdit");
      const prompt = safeStringValue(obj, "prompt") || (typeof args === "string" ? args : "");
      const images = Array.isArray(obj.images) ? obj.images : [];
      const firstImg = images[0] ? fileNameFromPath(images[0]) : "";
      const target = compactText(prompt || toCompactValue(args), 50);
      return {
        action,
        fileExt: firstImg ? extractFileExt(firstImg) : undefined,
        target,
        extra: firstImg || undefined,
        text: formatFullText(action, target, undefined, firstImg || undefined),
      };
    }

    // 23. 目录与搜索 (list_dir / read_dir / find / search)
    if (toolName === "list_dir" || toolName === "read_dir") {
      const action = toolTimelineText("actionListDir");
      const rawPath = safeTextFromRecord(obj, ["path", "dir", "directory", "folder"]) || (typeof args === "string" ? args.trim() : "");
      const target = fileNameFromPath(rawPath) || rawPath || ".";
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    if (toolName === "find" || toolName === "search") {
      const action = toolTimelineText("actionSearch");
      const target = safeTextFromRecord(obj, ["pattern", "query", "path", "keyword"]) || toCompactValue(args) || toolTimelineText("missingArgs");
      return {
        action,
        target: compactText(target, 40),
        text: formatFullText(action, compactText(target, 40)),
      };
    }

    // 24. 等待 (wait)
    if (toolName === "wait") {
      const action = toolTimelineText("actionWait");
      const ms = obj.ms ?? obj.timeout_ms ?? args;
      const target = ms !== undefined && ms !== null ? `${ms}ms` : "";
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }


    // 25. 队伍工具 (team_info / team_recruit / team_dismiss / team_send / team_report)
    if (toolName === "team_info") {
      const action = toolTimelineText("actionTeamInfo");
      const scope = safeStringValue(obj, "scope");
      const target = scope === "history"
        ? toolTimelineText("teamScopeHistory")
        : toolTimelineText("teamScopeActive");
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    if (toolName === "team_recruit") {
      const action = toolTimelineText("actionTeamRecruit");
      const name = safeStringValue(obj, "name");
      const target = compactText(name || toCompactValue(args), 40);
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    if (toolName === "team_dismiss") {
      const action = toolTimelineText("actionTeamDismiss");
      const number = obj.number;
      const target = number !== undefined && number !== null
        ? toolTimelineText("teamMemberNumber", { number: String(number) })
        : toCompactValue(args);
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    if (toolName === "team_send") {
      const action = toolTimelineText("actionTeamSend");
      const number = obj.number;
      const numberText = number !== undefined && number !== null
        ? toolTimelineText("teamMemberNumber", { number: String(number) })
        : "";
      const content = compactText(safeStringValue(obj, "content") || toCompactValue(args), 40);
      const target = [numberText, content].filter(Boolean).join(" ");
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }

    if (toolName === "team_report") {
      const action = toolTimelineText("actionTeamReport");
      const target = compactText(safeStringValue(obj, "content") || toCompactValue(args), 40);
      return {
        action,
        target,
        text: formatFullText(action, target),
      };
    }


    // 26. 外部 / MCP 工具：统一显示「链接到 {mcp工具名}」
    const action = toolTimelineText("actionConnectMcp");
    const target = toolName;
    const compactArg = toCompactValue(args);
    return {
      action,
      target,
      extra: compactArg ? compactText(compactArg, 40) : undefined,
      text: formatFullText(action, target),
    };
  }

  function toolCallDisplayName(toolName: string): string {
    const raw = String(toolName || "").trim();
    if (!raw) return toolTimelineText("unknownTool");

    if (raw === "read" || raw === "read_file" || raw === "read_media") return toolTimelineText("actionRead");
    if (raw === "update") return toolTimelineText("actionUpdate");
    if (raw === "write" || raw === "write_file" || raw === "create_file" || raw === "append_text") return toolTimelineText("actionWrite");
    if (raw === "delete" || raw === "delete_file") return toolTimelineText("actionDelete");
    if (raw === "move" || raw === "move_file" || raw === "rename_file") return toolTimelineText("actionMove");
    if (raw === "apply_patch") return toolTimelineText("actionPatch");
    if (raw === "exec" || raw === "shell_exec") return toolTimelineText("actionExec");
    if (raw === "config" || raw === "pai_config" || raw === "configure_pai" || raw === "app_config") return toolTimelineText("actionConfig");
    if (raw === "plan") return toolTimelineText("actionPlan");
    if (raw === "list_dir" || raw === "read_dir") return toolTimelineText("actionListDir");
    if (raw === "find" || raw === "search") return toolTimelineText("actionSearch");
    if (raw === "todo") return toolTimelineText("actionTodo");
    if (raw === "create_goal" || raw === "update_goal") return toolTimelineText("actionGoal");
    if (raw === "get_session") return toolTimelineText("actionGetSession");
    if (raw === "background") return toolTimelineText("actionBackground");
    if (raw === "inform_session") return toolTimelineText("actionInformSession");
    if (raw === "task") return toolTimelineText("actionTask");
    if (raw === "deeprecall" || raw === "deeprecall_search" || raw === "deeprecall_context") return toolTimelineText("actionDeepRecall");
    if (raw === "delegate") return toolTimelineText("actionDelegate");
    if (raw === "contact_send_files") return toolTimelineText("actionContactSendFiles");
    if (raw === "meme") return toolTimelineText("actionMeme");
    if (raw === "image_generate") return toolTimelineText("actionImageGenerate");
    if (raw === "image_edit") return toolTimelineText("actionImageEdit");
    if (raw === "image_to_video") return toolTimelineText("actionImageToVideo");
    if (raw === "remember") return toolTimelineText("actionRemember");
    if (raw === "recall") return toolTimelineText("actionRecall");
    if (raw === "fetch") return toolTimelineText("actionFetch");
    if (raw === "websearch") return toolTimelineText("actionWebSearch");
    if (raw === "operate") return toolTimelineText("actionOperate");
    if (raw === "wait") return toolTimelineText("actionWait");
    if (raw === "team_info") return toolTimelineText("actionTeamInfo");
    if (raw === "team_recruit") return toolTimelineText("actionTeamRecruit");
    if (raw === "team_dismiss") return toolTimelineText("actionTeamDismiss");
    if (raw === "team_send") return toolTimelineText("actionTeamSend");
    if (raw === "team_report") return toolTimelineText("actionTeamReport");

    // 外部 / MCP 工具：统一使用「链接到 {toolName}」
    return `${toolTimelineText("actionConnectMcp")} ${raw}`;
  }

  function toolCallSummaryText(toolCall: { name: string; argsText: string; status?: "doing" | "done" }): string {
    const semantic = toolCallSemanticPresentation(toolCall);
    return semantic.text;
  }

  function toolCallTitle(toolCall: { name: string; argsText: string }, index: number): string {
    const semantic = toolCallSemanticPresentation(toolCall);
    return `#${index} ${semantic.text}`;
  }

  return {
    compactText,
    countTextLines,
    extractFileExt,
    extractLineRange,
    internalToolNames,
    joinNonEmpty,
    normalizeToolCallArgs,
    toolCallDisplayName,
    toolCallSemanticPresentation,
    toolCallSummaryText,
    toolCallTitle,
    toolTimelineText,
  };
}
