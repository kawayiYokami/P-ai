type TranslateFn = (key: string, params?: Record<string, string | number>) => string;

type ToolCallPresentationOptions = {
  t: TranslateFn;
  agentName: (agentId: string) => string;
};

export function createToolCallPresentation(options: ToolCallPresentationOptions) {
  const t = options.t;

  const internalToolNames = new Set<string>([
    "apply_patch",
    "exec",
    "shell_exec",
    "read",
    "read_file",
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
    "task",
    "delegate",
    "remember",
    "recall",
    "fetch",
    "websearch",
    "operate",
    "wait",
    "akasha_search",
    "akasha_read",
    "akasha_catalog",
    "akasha_link",
    "tavily_search",
    "tavily_extract",
    "tavily_crawl",
    "tavily_map",
    "tavily_research",
  ]);
  
  const compactListKeys = new Set<string>([
    "todos",
    "files",
    "urls",
    "queries",
    "lineRanges",
    "tags",
  ]);
  
  const ignorableSummaryKeys = new Set<string>([
    "status",
    "reasoning",
    "background",
    "why",
    "max_length",
    "maxResults",
    "max_results",
    "tokens",
    "timeout_ms",
    "quality",
    "exact_match",
    "include_raw_content",
    "include_images",
    "include_image_descriptions",
    "include_favicon",
    "format",
    "topic",
    "country",
    "search_depth",
    "extract_depth",
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
  
  function compactText(text: string, maxLen = 180): string {
    const trimmed = text.replace(/\s+/g, " ").trim();
    if (trimmed.length <= maxLen) return trimmed;
    return `${trimmed.slice(0, maxLen - 3)}...`;
  }
  
  function joinNonEmpty(parts: string[], separator = " · "): string {
    return parts.map((part) => part.trim()).filter(Boolean).join(separator);
  }
  
  function safeStringValue(data: Record<string, unknown>, key: string): string {
    const value = data[key];
    return typeof value === "string" ? value.trim() : "";
  }
  
  function toolTimelineText(key: string, params?: Record<string, string | number>): string {
    return String(t(`status.toolTimeline.${key}`, params ?? {}));
  }
  
  function toolTimelineNameValue(name: string, value: string): string {
    return `${name}：${value}`;
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
  
  function compactObjectEntries(data: Record<string, unknown>, maxItems = 3): string {
    return Object.entries(data)
      .filter(([key, value]) => !ignorableSummaryKeys.has(key) && value !== undefined && value !== null && value !== "")
      .map(([key, value]) => {
        if (compactListKeys.has(key) && Array.isArray(value)) {
          return `${value.length} ${key}`;
        }
        const text = toCompactValue(value, 1);
        return text ? `${key}: ${text}` : "";
      })
      .filter(Boolean)
      .slice(0, maxItems)
      .join(" · ");
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
  
  function compactSingleLineJson(payload: unknown, maxLen = 180): string {
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
          const compactValue = toCompactValue(rawValue, depth + 1);
          return compactValue ? `${key}: ${compactValue}` : "";
        })
        .filter(Boolean)
        .slice(0, 2);
      if (pairs.length > 0) {
        return pairs.join("；");
      }
    }
  
    return "";
  }
  
  function applyPatchOperationLabel(operation: string): string {
    if (operation === "add") return toolTimelineText("patchAdd");
    if (operation === "delete") return toolTimelineText("patchDelete");
    if (operation === "move") return toolTimelineText("patchMove");
    return toolTimelineText("patchUpdate");
  }
  
  function summarizeApplyPatchInput(input: string): string {
    const lines = input.split(/\r?\n/);
    const entries: Array<{ operation: string; path: string }> = [];
    let pendingUpdatePath = "";
  
    for (const line of lines) {
      const addMatch = line.match(/^\*\*\* Add File:\s+(.+)$/);
      if (addMatch?.[1]) {
        entries.push({ operation: "add", path: addMatch[1].trim() });
        pendingUpdatePath = "";
        continue;
      }
  
      const deleteMatch = line.match(/^\*\*\* Delete File:\s+(.+)$/);
      if (deleteMatch?.[1]) {
        entries.push({ operation: "delete", path: deleteMatch[1].trim() });
        pendingUpdatePath = "";
        continue;
      }
  
      const updateMatch = line.match(/^\*\*\* Update File:\s+(.+)$/);
      if (updateMatch?.[1]) {
        pendingUpdatePath = updateMatch[1].trim();
        entries.push({ operation: "update", path: pendingUpdatePath });
        continue;
      }
  
      const moveMatch = line.match(/^\*\*\* Move to:\s+(.+)$/);
      if (moveMatch?.[1] && pendingUpdatePath) {
        const last = entries[entries.length - 1];
        if (last && last.path === pendingUpdatePath) {
          last.operation = "move";
          last.path = `${pendingUpdatePath} → ${moveMatch[1].trim()}`;
        }
        pendingUpdatePath = "";
      }
    }
  
    if (entries.length === 0) return toolTimelineText("inlinePatch");
    return entries
      .slice(0, 5)
      .map((entry) => `${applyPatchOperationLabel(entry.operation)} ${entry.path}`)
      .join("，");
  }
  
  function summarizeApplyPatchTool(args: unknown): string {
    const argsText = toSingleLineJsonText(args);
    if (!argsText) return toolTimelineText("checkChanges");
  
    if (typeof args === "string") {
      if (!args.trim()) return toolTimelineText("checkChanges");
      return summarizeApplyPatchInput(args);
    }
  
    if (typeof args === "object" && args !== null) {
      const obj = args as Record<string, unknown>;
      const input = typeof obj.input === "string" ? obj.input.trim() : "";
      if (input) return summarizeApplyPatchInput(input);
  
      const patch = (typeof obj.patch === "string" ? obj.patch : typeof obj.diff === "string" ? obj.diff : "").trim();
  
      const fileFromArgs = safeTextFromRecord(obj, ["file", "target", "path", "files", "pathnames"]);
      if (fileFromArgs) {
        return `${toolTimelineText("patchUpdate")} ${fileFromArgs}`;
      }
  
      if (patch) {
        const files = Array.from(new Set(
          patch
            .split(/\r?\n/)
            .map((line) => {
              const match = line.match(/^diff --git\s+(?:a\/|\S+)\s+(?:b\/|\S+)(.+)$/);
              if (match && match[1]) {
                return String(match[1]).replace(/^b\//, "").trim();
              }
              const simpleMatch = line.match(/^---\s+([ab]\/)?(.+)$/);
              if (simpleMatch && simpleMatch[2]) {
                return String(simpleMatch[2]).trim();
              }
              return "";
            })
            .filter((file) => Boolean(file) && !file.includes("/dev/null")),
        ));
  
        const filtered = files.filter((file) => file.length > 0);
        if (filtered.length > 0) {
          return filtered.map((file) => `${toolTimelineText("patchUpdate")} ${file}`).join("，");
        }
      }
  
      return compactSingleLineJson(args, 180) || toolTimelineText("checkArgs");
    }
  
    return toolTimelineText("patchCall");
  }
  
  function summarizeCommandTool(args: unknown): string {
    if (!args) return toolTimelineText("notProvided");
    if (typeof args === "string") return args;
    if (typeof args !== "object") return String(args);
  
    const obj = args as Record<string, unknown>;
    const command = safeTextFromRecord(obj, ["command", "cmd", "shell", "input", "commandText"]);
    const fallback = safeTextFromRecord(obj, ["args", "arguments", "argv", "params"]);
    if (command) return command;
    if (fallback) return fallback;
    const compact = toCompactValue(obj);
    return compact || toolTimelineText("checkArgs");
  }
  
  function summarizeFileTool(args: unknown): string {
    if (!args) return toolTimelineText("missingArgs");
    if (typeof args === "string") {
      const text = args.trim();
      return text || toolTimelineText("missingArgs");
    }
    if (typeof args !== "object") {
      return String(args);
    }
    const obj = args as Record<string, unknown>;
    const path = safeTextFromRecord(obj, ["absolute_path", "absolutePath", "path", "file", "target", "source", "destination", "from", "to"]);
    return path || toCompactValue(obj) || toolTimelineText("missingArgs");
  }
  
  function summarizeReadFileTool(args: unknown): string {
    if (!args) return toolTimelineText("missingArgs");
    if (typeof args === "string") {
      const text = args.trim();
      return text || toolTimelineText("missingArgs");
    }
    if (typeof args !== "object") {
      return String(args);
    }
    const obj = args as Record<string, unknown>;
    const path = safeTextFromRecord(obj, ["absolute_path", "absolutePath", "path", "file"]);
    const offset = obj.offset ?? obj.start;
    const limit = obj.limit ?? obj.count;
    return joinNonEmpty([
      path,
      offset !== undefined && offset !== null ? `offset: ${String(offset)}` : "",
      limit !== undefined && limit !== null ? `limit: ${String(limit)}` : "",
    ]) || toCompactValue(obj) || toolTimelineText("missingArgs");
  }
  
  function summarizeReadMediaTool(args: unknown): string {
    if (!args || typeof args !== "object") return summarizeReadFileTool(args);
    const obj = args as Record<string, unknown>;
    const path = safeTextFromRecord(obj, ["path", "absolute_path", "absolutePath", "file"]);
    const description = safeTextFromRecord(obj, ["description", "focus", "prompt"]);
    return joinNonEmpty([path, description ? `description: ${description}` : ""]) || compactObjectEntries(obj);
  }
  
  function summarizeTodoTool(args: unknown): string {
    if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
    const todos = (args as Record<string, unknown>).todos;
    if (!Array.isArray(todos)) return toolTimelineText("missingArgs");
    const counts = todos.reduce((acc, item) => {
      const status = typeof item === "object" && item !== null ? String((item as Record<string, unknown>).status || "pending") : "pending";
      acc[status] = (acc[status] || 0) + 1;
      return acc;
    }, {} as Record<string, number>);
    const active = todos
      .map((item) => (typeof item === "object" && item !== null ? item as Record<string, unknown> : null))
      .find((item) => String(item?.status || "") === "in_progress")
      ?? (typeof todos[0] === "object" && todos[0] !== null ? todos[0] as Record<string, unknown> : null);
    const activeText = active ? compactText(String(active.content || ""), 120) : "";
    return joinNonEmpty([
      toolTimelineText("todoItems", { count: todos.length }),
      counts.in_progress ? toolTimelineText("todoInProgress", { count: counts.in_progress }) : "",
      counts.pending ? toolTimelineText("todoPending", { count: counts.pending }) : "",
      counts.completed ? toolTimelineText("todoCompleted", { count: counts.completed }) : "",
      activeText,
    ]);
  }
  
  function summarizeTaskTool(args: unknown): string {
    if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
    const obj = args as Record<string, unknown>;
    return joinNonEmpty([
      safeStringValue(obj, "action"),
      safeStringValue(obj, "goal"),
      taskTriggerSummary(obj.trigger),
    ]) || compactObjectEntries(obj);
  }
  
  function summarizeGoalTool(args: unknown): string {
    if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
    const obj = args as Record<string, unknown>;
    return joinNonEmpty([
      safeStringValue(obj, "status"),
      compactText(safeStringValue(obj, "objective"), 120),
      compactText(safeStringValue(obj, "evidence"), 120),
      compactText(safeStringValue(obj, "blocking_condition"), 120),
    ]) || compactObjectEntries(obj);
  }
  
  function summarizePlanTool(args: unknown): string {
    if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
    const obj = args as Record<string, unknown>;
    return joinNonEmpty([
      safeStringValue(obj, "action"),
      compactText(safeStringValue(obj, "context"), 160),
    ]) || compactObjectEntries(obj);
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
  
  function summarizeDelegateTool(args: unknown): string {
    if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
    const obj = args as Record<string, unknown>;
    const mode = delegateModeDisplayText(safeStringValue(obj, "mode") || "wait");
    const targetAgent = delegateAgentDisplayText(safeStringValue(obj, "agent_id"));
    const content = compactText(
      safeStringValue(obj, "question")
        || safeStringValue(obj, "specific_goal")
        || safeStringValue(obj, "instruction")
        || safeStringValue(obj, "focus")
        || safeStringValue(obj, "background"),
      50,
    );
    return joinNonEmpty([
      safeStringValue(obj, "task_name"),
      targetAgent,
      mode,
      content,
    ]) || compactObjectEntries(obj);
  }
  
  function summarizeMemoryTool(args: unknown): string {
    if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
    const obj = args as Record<string, unknown>;
    return joinNonEmpty([
      safeStringValue(obj, "memory_type"),
      safeStringValue(obj, "judgment"),
      safeStringValue(obj, "query"),
    ]) || compactObjectEntries(obj);
  }
  
  function summarizeWebTool(args: unknown): string {
    if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
    const obj = args as Record<string, unknown>;
    return joinNonEmpty([
      safeStringValue(obj, "query"),
      safeStringValue(obj, "url"),
      Array.isArray(obj.urls) ? `${obj.urls.length} URLs` : "",
      safeStringValue(obj, "instructions"),
    ]) || compactObjectEntries(obj);
  }
  
  function summarizeAkashaTool(args: unknown): string {
    if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
    const obj = args as Record<string, unknown>;
    return joinNonEmpty([
      safeStringValue(obj, "world"),
      safeStringValue(obj, "keyword"),
      safeStringValue(obj, "documentPath"),
      safeStringValue(obj, "documentTitle"),
      Array.isArray(obj.lineRanges) ? obj.lineRanges.join("，") : "",
    ]) || compactObjectEntries(obj);
  }
  
  function summarizeOperateTool(args: unknown): string {
    if (typeof args !== "object" || args === null) return compactText(toSingleLineJsonText(args) || toolTimelineText("missingArgs"));
    const script = safeStringValue(args as Record<string, unknown>, "script");
    if (!script) return compactObjectEntries(args as Record<string, unknown>);
    const lines = script.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
    return compactText(lines.slice(0, 3).join("；"), 180);
  }
  
  function summarizeBuiltinTool(toolName: string, args: unknown): string {
    if (toolName === "read" || toolName === "read_file") return summarizeReadFileTool(args);
    if (toolName === "write" || toolName === "delete" || toolName === "move") return summarizeFileTool(args);
    if (toolName === "update") return summarizeFileTool(args);
    if (toolName === "read_media") return summarizeReadMediaTool(args);
    if (toolName === "todo") return summarizeTodoTool(args);
    if (toolName === "task") return summarizeTaskTool(args);
    if (toolName === "create_goal" || toolName === "update_goal") return summarizeGoalTool(args);
    if (toolName === "plan") return summarizePlanTool(args);
    if (toolName === "delegate") return summarizeDelegateTool(args);
    if (toolName === "remember" || toolName === "recall") return summarizeMemoryTool(args);
    if (toolName === "fetch" || toolName === "websearch" || toolName.startsWith("tavily_")) return summarizeWebTool(args);
    if (toolName.startsWith("akasha_")) return summarizeAkashaTool(args);
    if (toolName === "operate") return summarizeOperateTool(args);
    if (toolName === "wait") return compactObjectEntries((typeof args === "object" && args !== null ? args : { ms: args }) as Record<string, unknown>);
    return "";
  }
  
  function summarizeExternalTool(name: string, args: unknown): string {
    if (args === undefined || args === null) return toolTimelineText("noArgs");
    if (typeof args === "string") {
      const text = args.trim();
      return text || toolTimelineText("missingArgs");
    }
    if (typeof args !== "object") {
      return String(args);
    }
  
    const compact = toCompactValue(args);
    if (compact) {
      return compact;
    }
  
    const jsonText = compactSingleLineJson(args, 180);
    if (jsonText) return jsonText;
  
    return toolTimelineText("missingArgs");
  }
  
  function toolCallSummaryText(toolCall: { name: string; argsText: string; status?: "doing" | "done" }): string {
    const toolName = String(toolCall.name || "").trim() || "unknown";
    const args = normalizeToolCallArgs(toolCall.argsText);
  
    if (internalToolNames.has(toolName)) {
      if (toolName === "read" || toolName === "read_file") return summarizeReadFileTool(args);
      if (toolName === "read_media") return summarizeReadMediaTool(args);
      if (toolName === "apply_patch") return summarizeApplyPatchTool(args);
      if (toolName === "exec" || toolName === "shell_exec") return summarizeCommandTool(args);
      if (toolName.includes("file")) return summarizeFileTool(args);
      const builtinSummary = summarizeBuiltinTool(toolName, args);
      if (builtinSummary) return builtinSummary;
      const compact = toCompactValue(args);
      return compact || toolTimelineText("missingArgs");
    }
  
    return summarizeExternalTool(toolCallDisplayName(toolName), args);
  }
  
  function toolCallTitle(toolCall: { name: string }, index: number): string {
    return `#${index} ${toolCallDisplayName(toolCall.name)}`;
  }
  
  function toolCallDisplayName(toolName: string): string {
    if (toolName === "shell_exec") return "exec";
    if (toolName === "read_file") return "read";
    if (toolName === "read_dir") return "read_dir";
    if (toolName === "list_dir") return "list_dir";
    return String(toolName || toolTimelineText("unknownTool")).trim() || toolTimelineText("unknownTool");
  }

  return {
    compactText,
    joinNonEmpty,
    normalizeToolCallArgs,
    toolCallDisplayName,
    toolCallSummaryText,
    toolCallTitle,
    toolTimelineText,
  };
}
