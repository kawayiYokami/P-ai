import type {
  AssistantStreamBlock,
  AssistantStreamToolBlock,
  ChatActivityItem,
  ChatActivityStatus,
  ChatMentionTarget,
  ChatMessage,
  MemeAnnotation,
  PlanMessageCard,
  TaskTriggerMessageCard,
} from "../types/app";
import {
  extractMessageAttachmentFiles,
  extractMessageAudios,
  extractMessageImages,
  removeBinaryPlaceholders,
  renderMessage,
} from "./chat-message";

type ToolHistoryView = "display" | "prompt";

/**
 * 「工具后新正文」分段占位符。
 * 流式累积按事件切块（appendTextDeltaToStreamBlocks 在 pendingTextBreak 时开新块），
 * 块间边界由 joinAssistantHistoryTexts 按「前段含工具标记」注入本占位符；
 * 正式/历史消息投影同样注入。渲染层按分段开关决定：开启时按占位符切段，关闭时还原为换行。
 */
export const TOOL_TEXT_BREAK_PLACEHOLDER = "\uE000TOOLBREAK\uE000";

export type NormalizedToolCall = {
  invocationId: string;
  providerCallId?: string;
  toolType: string;
  toolName: string;
  argumentsText: string;
  argumentsValue: unknown;
};

export type NormalizedToolHistoryEvent = {
  role: "assistant" | "tool";
  text: string;
  reasoningContent?: string;
  toolCalls: NormalizedToolCall[];
  toolCallId?: string;
  metadata?: Record<string, unknown>;
};

function textPartReasoning(part: ChatMessage["parts"][number]): string {
  if (!part || part.type !== "text") return "";
  const textPart = part as Extract<ChatMessage["parts"][number], { type: "text" }> & {
    reasoning_content?: string;
  };
  return String(textPart.reasoningContent || textPart.reasoning_content || "").trim();
}

function optionalNonNegativeInteger(value: unknown): number | undefined {
  const parsed = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(parsed) || parsed < 0) return undefined;
  return Math.floor(parsed);
}

function assistantStreamBlockReasoningCharCount(block: AssistantStreamBlock): number {
  return optionalNonNegativeInteger(block.reasoningCharCount) ?? String(block.reasoning || "").length;
}

export type ChatMessageDisplayProjection = {
  speakerAgentId?: string;
  mentions: ChatMentionTarget[];
  text: string;
  images: Array<{ mime: string; bytesBase64?: string; mediaRef?: string }>;
  audios: Array<{ mime: string; bytesBase64?: string; mediaRef?: string }>;
  attachmentFiles: Array<{ fileName: string; path: string; mime?: string }>;
  taskTrigger?: TaskTriggerMessageCard;
  planCard?: PlanMessageCard;
  remoteImOrigin?: {
    senderName: string;
    remoteContactName?: string;
    remoteContactType: string;
    channelId: string;
    contactId: string;
  };
  toolCallCount: number;
  lastToolName: string;
  toolCalls: Array<{ name: string; argsText: string }>;
  activityItems: ChatActivityItem[];
  activityReasoningCharCount: number;
  activityToolCountsByName: Record<string, number>;
  activityRunning: boolean;
  activityStatus: ChatActivityStatus;
};

export type TaskTriggerDisplayLabels = {
  goal: string;
  todo: string;
};

function sanitizeStoredToolHistoryEvents(
  events: ChatMessage["toolCall"],
): Array<Record<string, unknown>> {
  if (!Array.isArray(events) || events.length === 0) return [];
  const sanitized: Array<Record<string, unknown>> = [];
  let pendingAssistant:
    | {
        event: Record<string, unknown>;
        allowedIds: string[];
        matchedIds: string[];
        outputIndex: number | null;
        legacyWithoutIds: boolean;
      }
    | null = null;
  const toolCallIdsFromAssistant = (event: Record<string, unknown>): string[] => {
    const calls = Array.isArray(event.tool_calls) ? event.tool_calls : [];
    return calls.flatMap((rawCall) => {
      if (!rawCall || typeof rawCall !== "object") return [];
      const call = rawCall as Record<string, unknown>;
      return ["id", "call_id"]
        .map((key) => String(call[key] || "").trim())
        .filter(Boolean);
    });
  };
  const assistantWithMatchedToolCalls = (
    event: Record<string, unknown>,
    matchedIds: string[],
  ): Record<string, unknown> => {
    const calls = Array.isArray(event.tool_calls) ? event.tool_calls : [];
    return {
      ...event,
      tool_calls: calls.filter((rawCall) => {
        if (!rawCall || typeof rawCall !== "object") return false;
        const call = rawCall as Record<string, unknown>;
        return ["id", "call_id"].some((key) => matchedIds.includes(String(call[key] || "").trim()));
      }),
    };
  };
  for (const raw of events) {
    if (!raw || typeof raw !== "object") continue;
    const event = raw as Record<string, unknown>;
    const role = String(event.role || "").trim().toLowerCase();
    if (role === "assistant") {
      const calls = Array.isArray(event.tool_calls) ? event.tool_calls : [];
      const hasToolCalls = calls.length > 0;
      const allowedIds = toolCallIdsFromAssistant(event);
      if (hasToolCalls) {
        pendingAssistant = {
          event,
          allowedIds,
          matchedIds: [],
          outputIndex: null,
          legacyWithoutIds: allowedIds.length === 0,
        };
      } else {
        pendingAssistant = null;
        sanitized.push(event);
      }
      continue;
    }
    if (role === "tool") {
      if (pendingAssistant) {
        const toolCallId = String(event.tool_call_id || "").trim();
        const matchedIndex = pendingAssistant.allowedIds.indexOf(toolCallId);
        const legacyWithoutIds = pendingAssistant.legacyWithoutIds && pendingAssistant.outputIndex === null;
        if (legacyWithoutIds || matchedIndex >= 0) {
          if (!pendingAssistant.matchedIds.includes(toolCallId)) {
            pendingAssistant.matchedIds.push(toolCallId);
          }
          const assistantEvent = pendingAssistant.legacyWithoutIds
            ? pendingAssistant.event
            : assistantWithMatchedToolCalls(pendingAssistant.event, pendingAssistant.matchedIds);
          if (pendingAssistant.outputIndex === null) {
            pendingAssistant.outputIndex = sanitized.length;
            sanitized.push(assistantEvent);
          } else {
            sanitized[pendingAssistant.outputIndex] = assistantEvent;
          }
          sanitized.push(event);
          if (matchedIndex >= 0) {
            pendingAssistant.allowedIds.splice(matchedIndex, 1);
            if (pendingAssistant.allowedIds.length === 0) pendingAssistant = null;
          } else {
            pendingAssistant = null;
          }
        }
      }
      continue;
    }
    pendingAssistant = null;
    sanitized.push(event);
  }
  return sanitized;
}

function normalizeToolCallArguments(raw: unknown): { argumentsText: string; argumentsValue: unknown } {
  if (typeof raw === "string") {
    const text = raw.trim();
    if (!text) return { argumentsText: "", argumentsValue: {} };
    try {
      return { argumentsText: text, argumentsValue: JSON.parse(text) as unknown };
    } catch {
      return { argumentsText: text, argumentsValue: text };
    }
  }
  if (raw === null || raw === undefined) {
    return { argumentsText: "{}", argumentsValue: {} };
  }
  try {
    return {
      argumentsText: JSON.stringify(raw),
      argumentsValue: raw,
    };
  } catch {
    return {
      argumentsText: String(raw),
      argumentsValue: raw,
    };
  }
}

export function normalizeMessageToolHistoryEvents(
  message: ChatMessage,
  view: ToolHistoryView = "display",
): NormalizedToolHistoryEvent[] {
  const sourceEvents =
    view === "prompt"
      ? sanitizeStoredToolHistoryEvents(message.toolCall)
      : Array.isArray(message.toolCall)
        ? (message.toolCall as Array<Record<string, unknown>>)
        : [];
  const normalized: NormalizedToolHistoryEvent[] = [];
  for (const event of sourceEvents) {
    const role = String(event.role || "").trim().toLowerCase();
    if (role === "assistant") {
      const calls = Array.isArray(event.tool_calls) ? event.tool_calls : [];
      if (view === "prompt" && calls.length === 0) continue;
      normalized.push({
        role: "assistant",
        text: typeof event.content === "string" ? event.content : "",
        reasoningContent: typeof event.reasoning_content === "string" ? event.reasoning_content : undefined,
        toolCalls: calls
          .map((raw) => {
            const call = raw as Record<string, unknown>;
            const func = (call.function || {}) as Record<string, unknown>;
            const { argumentsText, argumentsValue } = normalizeToolCallArguments(func.arguments);
            return {
              invocationId: String(call.id || "").trim(),
              providerCallId: String(call.call_id || "").trim() || undefined,
              toolType: String(call.type || "function").trim() || "function",
              toolName: String(func.name || "").trim() || "unknown",
              argumentsText,
              argumentsValue,
            } satisfies NormalizedToolCall;
          }),
      });
      continue;
    }
    if (role === "tool") {
      normalized.push({
        role: "tool",
        text: typeof event.content === "string" ? event.content : "",
        toolCalls: [],
        toolCallId: String(event.tool_call_id || "").trim() || undefined,
        metadata: event.metadata && typeof event.metadata === "object"
          ? event.metadata as Record<string, unknown>
          : undefined,
      });
    }
  }
  return normalized;
}

export function summarizeToolActivityForDisplay(
  message: ChatMessage,
): { count: number; lastToolName: string; calls: Array<{ name: string; argsText: string }> } {
  const calls = normalizeMessageToolHistoryEvents(message, "display")
    .flatMap((event) => event.role === "assistant" ? event.toolCalls : [])
    .filter((call) => !!call.toolName);
  return {
    count: calls.length,
    lastToolName: calls.length > 0 ? calls[calls.length - 1].toolName : "",
    calls: calls.map((call) => ({ name: call.toolName, argsText: call.argumentsText || "{}" })),
  };
}

function isToolCallAssistantTextEvent(event: NormalizedToolHistoryEvent): boolean {
  return event.role === "assistant"
    && event.toolCalls.length > 0
    && !!String(event.text || "").trim();
}

function toolCallInlineSuffix(event: NormalizedToolHistoryEvent): string {
  const suffix = event.toolCalls
    .map((call) => String(call.invocationId || call.providerCallId || "").trim())
    .filter(Boolean)
    .map((id) => `[toolcall:${id}]`)
    .join("");
  return suffix ? ` ${suffix}` : "";
}

function hasInlineToolMarker(text: string, toolCallId: string): boolean {
  return !!toolCallId && text.includes(`[toolcall:${toolCallId}]`);
}

function injectMissingDoneToolMarkersIntoStreamText(text: string, block: AssistantStreamBlock): string {
  let output = String(text || "");
  if (!output.trim()) return output;
  for (const tool of (block.tools || [])) {
    if (tool.status !== "done") continue;
    const toolCallId = String(tool.toolCallId || "").trim();
    if (!toolCallId || hasInlineToolMarker(output, toolCallId)) continue;
    // 模型 delta 常以换行结尾；尾随空白会让渲染层 pre-wrap 段落把标记徽章顶到下一行
    output = `${output.trimEnd()} [toolcall:${toolCallId}]`;
  }
  return output;
}

function assistantEventDisplayText(event: NormalizedToolHistoryEvent): string {
  const text = String(event.text || "").trim();
  if (event.role !== "assistant" || event.toolCalls.length === 0) return text;
  if (!text) return toolCallInlineSuffix(event).trim();
  let output = text;
  for (const call of event.toolCalls) {
    const toolCallId = String(call.invocationId || call.providerCallId || "").trim();
    if (!toolCallId || hasInlineToolMarker(output, toolCallId)) continue;
    output = `${output} [toolcall:${toolCallId}]`;
  }
  return output;
}

function injectToolInlineMarkersIntoMergedText(text: string, events: NormalizedToolHistoryEvent[]): string {
  let output = String(text || "");
  for (const event of events) {
    if (!isToolCallAssistantTextEvent(event)) continue;
    const raw = String(event.text || "").trim();
    if (!raw) continue;
    const marked = `${raw}${toolCallInlineSuffix(event)}`;
    if (output.includes(marked)) continue;
    const index = output.indexOf(raw);
    if (index < 0) continue;
    output = `${output.slice(0, index)}${marked}${output.slice(index + raw.length)}`;
  }
  // 事件驱动就地插占位符：工具调用事件（含纯标记事件）的标记串之后紧接正文内容时，
  // 在边界插入 TOOL_TEXT_BREAK_PLACEHOLDER，与 joinAssistantHistoryTexts 的
  // 「前段含工具标记 && 后段有正文」规则对齐。已有占位符或后接另一标记时不插。
  for (const event of events) {
    if (event.toolCalls.length === 0) continue;
    const markerText = toolCallInlineSuffix(event).trim();
    if (!markerText) continue;
    // 用 lastIndexOf 定位「该事件追加/自带于最晚位置」的标记串：
    // 事件文本本身可能已含同名 [toolcall:id]，indexOf 会命中文本中更早的自带标记，
    // 导致把「事件文本内容」误判为工具后新正文边界。
    const markerIndex = output.lastIndexOf(markerText);
    if (markerIndex < 0) continue;
    const markerEnd = markerIndex + markerText.length;
    const after = output.slice(markerEnd);
    const nextBody = after.search(/\S/);
    if (nextBody < 0) continue;
    const rest = after.slice(nextBody);
    if (rest.startsWith(TOOL_TEXT_BREAK_PLACEHOLDER) || rest.startsWith("[toolcall:")) continue;
    output = `${output.slice(0, markerEnd)}${TOOL_TEXT_BREAK_PLACEHOLDER}${after.slice(nextBody)}`;
  }
  return output;
}

/**
 * 正式/历史消息投影时，把「工具后新正文」边界写成占位符。
 * 规则：相邻两段中，前一段含工具标记、后一段是含正文的新段时用占位符连接；
 * 其余情况保持 `\n\n`，与非分段渲染现状一致。
 */
function joinAssistantHistoryTexts(texts: string[]): string {
  const parts: string[] = [];
  for (let index = 0; index < texts.length; index += 1) {
    const text = String(texts[index] || "");
    if (index === 0) {
      parts.push(text);
      continue;
    }
    const previous = String(texts[index - 1] || "");
    const previousHasToolMarker = /\[toolcall:[^\]\n]+\]/.test(previous);
    const currentHasBody = !!stripToolcallMarkers(text);
    parts.push(previousHasToolMarker && currentHasBody ? TOOL_TEXT_BREAK_PLACEHOLDER : "\n\n");
    parts.push(text);
  }
  return parts.join("");
}

export function stripToolcallMarkers(text: string): string {
  return String(text || "").replace(/\s*\[toolcall:[^\]\n]+\]/g, "").trim();
}

/**
 * 把旧协议（后端 streamCache 快照）里「工具标记后正文边界」的真实换行
 * 归一化为分段占位符。本地流式追加与正式消息投影已直接写占位符，
 * 只有刷新恢复路径的 streamBlocks 仍是 `\n\n`，渲染层无法据此分段。
 */
export function normalizeLegacyToolBreakToPlaceholder(text: string): string {
  return String(text || "")
    .replace(/(\[toolcall:[^\]\n]+\])\n\n(?=\S)/g, `$1${TOOL_TEXT_BREAK_PLACEHOLDER}`);
}

function chatActivityStats(
  items: ChatActivityItem[],
  running: boolean,
  status?: ChatActivityStatus,
): {
  activityReasoningCharCount: number;
  activityToolCountsByName: Record<string, number>;
  activityRunning: boolean;
  activityStatus: ChatActivityStatus;
} {
  const activityToolCountsByName: Record<string, number> = {};
  let activityReasoningCharCount = 0;
  for (const item of items) {
    if (item.kind === "reasoning") {
      activityReasoningCharCount += String(item.text || "").length;
      continue;
    }
    if (item.kind === "content") {
      continue;
    }
    const name = String(item.name || "").trim() || "unknown";
    activityToolCountsByName[name] = (activityToolCountsByName[name] || 0) + 1;
  }
  return {
    activityReasoningCharCount,
    activityToolCountsByName,
    activityRunning: running,
    activityStatus: status || (items.length > 0 ? "complete" : "idle"),
  };
}

function findAdjacentToolResult(
  events: NormalizedToolHistoryEvent[],
  assistantIndex: number,
  invocationId: string,
): NormalizedToolHistoryEvent | undefined {
  for (let index = assistantIndex + 1; index < events.length; index += 1) {
    const event = events[index];
    if (event.role === "assistant") return undefined;
    if (event.role !== "tool") continue;
    if (!invocationId || !event.toolCallId || event.toolCallId === invocationId) {
      return event;
    }
  }
  return undefined;
}

export function projectChatActivityForDisplay(message: ChatMessage): {
  items: ChatActivityItem[];
  activityReasoningCharCount: number;
  activityToolCountsByName: Record<string, number>;
  activityRunning: boolean;
  activityStatus: ChatActivityStatus;
} {
  const messageItems = normalizeChatActivityItems(message.activityItems);
  if (messageItems.length > 0) {
    return {
      items: messageItems,
      ...chatActivityStats(messageItems, false),
    };
  }
  const canonicalBlocks = assistantContentBlocksFromMessage(message);
  if (canonicalBlocks.length > 0) {
    const items = streamBlocksToActivityItems(
      canonicalBlocks,
      Boolean((message.providerMeta as Record<string, unknown> | undefined)?._streaming),
    );
    return {
      items,
      ...chatActivityStats(items, Boolean((message.providerMeta as Record<string, unknown> | undefined)?._streaming)),
    };
  }
  const events = normalizeMessageToolHistoryEvents(message, "display");
  const items: ChatActivityItem[] = [];
  for (let eventIndex = 0; eventIndex < events.length; eventIndex += 1) {
    const event = events[eventIndex];
    if (event.role !== "assistant") continue;
    const thinkingText = String(event.reasoningContent || "").trim();
    if (thinkingText) {
      items.push({
        kind: "reasoning",
        id: `reasoning-${eventIndex}-${items.length}`,
        text: thinkingText,
      });
    }
    const bodyText = String(event.text || "").trim();
    if (bodyText) {
      items.push({
        kind: "content",
        id: `content-${eventIndex}-${items.length}`,
        text: assistantEventDisplayText(event),
      });
    }
    for (const call of event.toolCalls) {
      const result = findAdjacentToolResult(events, eventIndex, call.invocationId);
      items.push({
        kind: "tool",
        id: call.invocationId || call.providerCallId || `tool-${eventIndex}-${items.length}`,
        toolCallId: call.invocationId || undefined,
        name: call.toolName,
        argsText: call.argumentsText || "{}",
        resultText: result ? result.text : undefined,
        status: "done",
      });
    }
  }
  const finalReasoning = Array.isArray(message.parts)
    ? message.parts
      .filter((part): part is Extract<ChatMessage["parts"][number], { type: "text" }> => part?.type === "text")
      .map((part) => textPartReasoning(part))
      .filter(Boolean)
      .join("\n")
    : "";
  if (finalReasoning) {
    items.push({
      kind: "reasoning",
      id: `final-reasoning-${items.length}`,
      text: finalReasoning,
    });
  }
  return {
    items,
    ...chatActivityStats(items, false),
  };
}

export function normalizeChatActivityItems(rawItems: unknown): ChatActivityItem[] {
  if (!Array.isArray(rawItems)) return [];
  const items: ChatActivityItem[] = [];
  for (const [index, raw] of rawItems.entries()) {
    const item = raw && typeof raw === "object" ? raw as Record<string, unknown> : null;
    const kind = String(item?.kind || "").trim();
    if (kind === "reasoning") {
      const text = String(item?.text || "");
      if (!text.trim()) continue;
      items.push({
        kind: "reasoning",
        id: String(item?.id || "").trim() || `stream-reasoning-${index}`,
        text,
        running: !!item?.running,
      });
      continue;
    }
    if (kind === "content") {
      const text = String(item?.text || "");
      if (!text.trim()) continue;
      items.push({
        kind: "content",
        id: String(item?.id || "").trim() || `stream-content-${index}`,
        text,
        running: !!item?.running,
      });
      continue;
    }
    if (kind === "tool") {
      const name = String(item?.name || "").trim();
      if (!name) continue;
      const toolCallId = String(item?.toolCallId || "").trim();
      items.push({
        kind: "tool",
        id: String(item?.id || "").trim() || toolCallId || `stream-tool-${index}`,
        toolCallId: toolCallId || undefined,
        name,
        argsText: String(item?.argsText || ""),
        resultText: typeof item?.resultText === "string" ? item.resultText : undefined,
        status: String(item?.status || "") === "doing" ? "doing" : "done",
      });
    }
  }
  return items;
}

function normalizeAssistantStreamToolBlocks(rawTools: unknown): AssistantStreamToolBlock[] {
  if (!Array.isArray(rawTools)) return [];
  const tools: AssistantStreamToolBlock[] = [];
  for (const raw of rawTools) {
    const item = raw && typeof raw === "object" ? raw as Record<string, unknown> : null;
    const toolCallId = String(item?.toolCallId || item?.tool_call_id || "").trim();
    const name = String(item?.name || item?.toolName || item?.tool_name || "").trim();
    if (!toolCallId || !name) continue;
    const status = String(item?.status || "").trim();
    tools.push({
      toolCallId,
      name,
      argsText: String(item?.argsText || item?.args_text || item?.toolArgs || item?.tool_args || ""),
      resultText: typeof item?.resultText === "string"
        ? item.resultText
        : typeof item?.result_text === "string"
          ? item.result_text
          : undefined,
      resultMetadata: item?.resultMetadata && typeof item.resultMetadata === "object"
        ? item.resultMetadata as Record<string, unknown>
        : item?.result_metadata && typeof item.result_metadata === "object"
          ? item.result_metadata as Record<string, unknown>
          : undefined,
      status: status === "doing" || status === "running" ? "doing" : "done",
    });
  }
  return tools;
}

export function normalizeAssistantStreamBlocks(rawBlocks: unknown): AssistantStreamBlock[] {
  if (!Array.isArray(rawBlocks)) return [];
  const blocks: AssistantStreamBlock[] = [];
  for (const raw of rawBlocks) {
    const item = raw && typeof raw === "object" ? raw as Record<string, unknown> : null;
    if (!item) continue;
    const reasoning = String(item.reasoning || item.reasoningText || item.reasoning_text || "");
    const reasoningCharCount = optionalNonNegativeInteger(
      item.reasoningCharCount ?? item.reasoning_char_count,
    ) ?? reasoning.length;
    const text = String(item.text || "");
    const tools = normalizeAssistantStreamToolBlocks(item.tools);
    if (!reasoning.trim() && !text.trim() && tools.length === 0) continue;
    blocks.push({
      reasoning,
      reasoningCharCount,
      text,
      tools,
      pendingTextBreak: item.pendingTextBreak === true || item.pending_text_break === true,
    });
  }
  return blocks;
}

export function assistantContentBlocksFromMessage(message: unknown): AssistantStreamBlock[] {
  const value = message && typeof message === "object" ? message as Record<string, unknown> : {};
  return normalizeAssistantStreamBlocks(value.contentBlocks);
}

export function assistantTextFromStreamBlocks(rawBlocks: unknown): string {
  return joinAssistantHistoryTexts(
    normalizeAssistantStreamBlocks(rawBlocks)
      .map((block) => {
        const text = String(block.text || "");
        if (!text.trim()) return "";
        return injectMissingDoneToolMarkersIntoStreamText(text, block);
      })
      .filter((text) => text.length > 0),
  );
}

export function assistantStreamBlocksFromMessageForDisplay(
  message: ChatMessage,
  fallbackText = "",
): AssistantStreamBlock[] {
  const events = normalizeMessageToolHistoryEvents(message, "display");
  const blocks: AssistantStreamBlock[] = [];
  for (let eventIndex = 0; eventIndex < events.length; eventIndex += 1) {
    const event = events[eventIndex];
    if (event.role !== "assistant") continue;
    const tools = event.toolCalls.map((call) => {
      const result = findAdjacentToolResult(events, eventIndex, call.invocationId);
      return {
        toolCallId: call.invocationId || call.providerCallId || "",
        name: call.toolName,
        argsText: call.argumentsText || "{}",
        resultText: result ? result.text : undefined,
        resultMetadata: result?.metadata,
        status: "done" as const,
      };
    }).filter((tool) => !!tool.toolCallId && !!tool.name);
    const block: AssistantStreamBlock = {
      reasoning: String(event.reasoningContent || ""),
      text: assistantEventDisplayText(event),
      tools,
    };
    if (block.reasoning?.trim() || block.text?.trim() || tools.length > 0) {
      blocks.push(block);
    }
  }

  const normalized = normalizeAssistantStreamBlocks(blocks);
  const text = String(fallbackText || "");
  const finalReasoning = Array.isArray(message.parts)
    ? message.parts
      .filter((part): part is Extract<ChatMessage["parts"][number], { type: "text" }> => part?.type === "text")
      .map((part) => textPartReasoning(part))
      .filter(Boolean)
      .join("\n")
    : "";
  if (!text.trim()) return normalized;
  if (normalized.length === 0) {
    return normalizeAssistantStreamBlocks([{ reasoning: finalReasoning, text }]);
  }
  if (normalized.some((block) => !!stripToolcallMarkers(block.text || ""))) {
    return normalized;
  }
  if (finalReasoning) {
    return normalizeAssistantStreamBlocks([...normalized, { reasoning: finalReasoning, text }]);
  }
  const lastIndex = normalized.length - 1;
  return normalizeAssistantStreamBlocks(normalized.map((block, index) =>
    index === lastIndex ? { ...block, text } : block
  ));
}

function mergedAssistantDisplayText(message: ChatMessage, fallbackText: string): string {
  const finalText = String(fallbackText || "");
  const assistantEvents = normalizeMessageToolHistoryEvents(message, "display")
    .filter((event) => event.role === "assistant");
  const assistantHistoryTexts: string[] = [];
  let pendingMarkerOnlyText = "";
  for (const event of assistantEvents) {
    const displayText = assistantEventDisplayText(event);
    if (!displayText) continue;
    if (!String(event.text || "").trim()) {
      pendingMarkerOnlyText = pendingMarkerOnlyText
        ? `${pendingMarkerOnlyText} ${displayText}`
        : displayText;
      continue;
    }
    if (pendingMarkerOnlyText) {
      assistantHistoryTexts.push(pendingMarkerOnlyText);
      pendingMarkerOnlyText = "";
    }
    assistantHistoryTexts.push(displayText);
  }
  if (pendingMarkerOnlyText) {
    assistantHistoryTexts.push(pendingMarkerOnlyText);
  }
  if (assistantHistoryTexts.length === 0) return finalText;
  if (!finalText.trim()) return joinAssistantHistoryTexts(assistantHistoryTexts);
  const assistantHistoryTextsWithoutRawText = assistantHistoryTexts.filter((text) => !stripToolcallMarkers(text));
  if (
    assistantHistoryTextsWithoutRawText.length === assistantHistoryTexts.length
    && assistantHistoryTexts.every((text) => finalText.includes(text))
  ) {
    return finalText;
  }
  const rawAssistantTexts = assistantEvents
    .map((event) => String(event.text || "").trim())
    .filter(Boolean);
  if (rawAssistantTexts.length > 0 && rawAssistantTexts.every((text) => finalText.includes(text))) {
    const injected = injectToolInlineMarkersIntoMergedText(finalText, assistantEvents);
    const missingMarkerOnlyTexts = assistantHistoryTextsWithoutRawText
      .filter((text) => !injected.includes(text));
    return missingMarkerOnlyTexts.length > 0
      ? joinAssistantHistoryTexts([...missingMarkerOnlyTexts, injected])
      : injected;
  }
  return joinAssistantHistoryTexts([...assistantHistoryTexts, finalText]);
}

export function streamBlocksToActivityItems(rawBlocks: unknown, running = false): ChatActivityItem[] {
  const items: ChatActivityItem[] = [];
  for (const [blockIndex, block] of normalizeAssistantStreamBlocks(rawBlocks).entries()) {
    const reasoning = String(block.reasoning || "");
    if (reasoning.trim()) {
      items.push({
        kind: "reasoning",
        id: `stream-block-${blockIndex}-reasoning`,
        text: reasoning,
        running,
      });
    }
    const text = String(block.text || "").trim();
    if (stripToolcallMarkers(text)) {
      items.push({
        kind: "content",
        id: `stream-block-${blockIndex}-content`,
        text: injectMissingDoneToolMarkersIntoStreamText(text, block),
        running,
      });
    }
    for (const [toolIndex, tool] of (block.tools || []).entries()) {
      items.push({
        kind: "tool",
        id: tool.toolCallId || `stream-block-${blockIndex}-tool-${toolIndex}`,
        toolCallId: tool.toolCallId || undefined,
        name: tool.name,
        argsText: tool.argsText || "",
        resultText: tool.resultText,
        status: tool.status === "doing" ? "doing" : "done",
      });
    }
  }
  return items;
}

function streamBlocksReasoningCharCount(blocks: AssistantStreamBlock[]): number {
  return blocks.reduce((total, block) => total + assistantStreamBlockReasoningCharCount(block), 0);
}

function streamBlocksToolCountsByName(blocks: AssistantStreamBlock[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const block of blocks) {
    for (const tool of (block.tools || [])) {
      const name = String(tool.name || "").trim() || "unknown";
      counts[name] = (counts[name] || 0) + 1;
    }
  }
  return counts;
}

export function streamBlocksToActivitySummaryItems(rawBlocks: unknown, running = false): ChatActivityItem[] {
  const blocks = normalizeAssistantStreamBlocks(rawBlocks);
  const items: ChatActivityItem[] = [];
  for (const [blockIndex, block] of blocks.entries()) {
    if (String(block.reasoning || "").trim()) {
      items.push({
        kind: "reasoning",
        id: `stream-summary-${blockIndex}-reasoning`,
        text: "",
        running,
      });
    }
    if (String(block.text || "").trim()) {
      items.push({
        kind: "content",
        id: `stream-summary-${blockIndex}-content`,
        text: "",
        running,
      });
    }
    for (const [toolIndex, tool] of (block.tools || []).entries()) {
      items.push({
        kind: "tool",
        id: tool.toolCallId || `stream-summary-${blockIndex}-tool-${toolIndex}`,
        toolCallId: tool.toolCallId || undefined,
        name: tool.name,
        // 折叠标题只依赖 name；但同一批 summary item 也可能被明细层读取。
        // 参数保留在完整工具变量里，这里不再主动清空，避免圆点明细丢参。
        argsText: tool.argsText || "",
        status: tool.status === "doing" ? "doing" : "done",
      });
    }
  }
  return items;
}

export function streamBlocksActivitySignature(rawBlocks: unknown): string {
  function textSignature(text?: string): string {
    const input = String(text || "");
    let hash = 0x811c9dc5;
    for (let index = 0; index < input.length; index += 1) {
      hash ^= input.charCodeAt(index);
      hash = Math.imul(hash, 0x01000193);
    }
    return `${input.length}:${(hash >>> 0).toString(36)}`;
  }

  return normalizeAssistantStreamBlocks(rawBlocks)
    .map((block, blockIndex) => [
      `b:${blockIndex}`,
      `rlen:${assistantStreamBlockReasoningCharCount(block)}`,
      `tlen:${String(block.text || "").length}`,
      ...((block.tools || []).map((tool, toolIndex) => [
        `t:${toolIndex}`,
        String(tool.toolCallId || "").trim(),
        String(tool.name || "").trim(),
        String(tool.status || "").trim(),
        `a:${textSignature(tool.argsText)}`,
        `r:${textSignature(tool.resultText)}`,
      ].join(":"))),
    ].join("|"))
    .join("||");
}

export function streamBlocksToToolCalls(
  rawBlocks: unknown,
): Array<{ toolCallId?: string; name: string; argsText: string; status?: "doing" | "done" }> {
  return normalizeAssistantStreamBlocks(rawBlocks)
    .flatMap((block) => block.tools || [])
    .map((tool) => ({
      toolCallId: tool.toolCallId || undefined,
      name: tool.name,
      argsText: tool.argsText || "",
      status: tool.status === "doing" ? "doing" as const : "done" as const,
    }));
}

function copyAssistantStreamBlocksForAppend(rawBlocks: unknown): AssistantStreamBlock[] {
  const blocks = normalizeAssistantStreamBlocks(rawBlocks);
  return blocks.map((block) => ({
    ...block,
    tools: Array.isArray(block.tools) ? block.tools : [],
  }));
}

function cloneAssistantStreamBlocks(rawBlocks: unknown): AssistantStreamBlock[] {
  return normalizeAssistantStreamBlocks(rawBlocks).map((block) => ({
    reasoning: String(block.reasoning || ""),
    reasoningCharCount: assistantStreamBlockReasoningCharCount(block),
    text: String(block.text || ""),
    tools: (block.tools || []).map((tool) => ({ ...tool })),
    pendingTextBreak: block.pendingTextBreak === true,
  }));
}

function ensureAssistantStreamBlock(blocks: AssistantStreamBlock[]): AssistantStreamBlock {
  if (blocks.length === 0) {
    blocks.push({ reasoning: "", text: "", tools: [], pendingTextBreak: false });
  }
  const last = blocks[blocks.length - 1];
  if (!Array.isArray(last.tools)) last.tools = [];
  if (last.pendingTextBreak !== true) last.pendingTextBreak = false;
  return last;
}

function attachInlineToolMarkerToStreamBlock(
  blocks: AssistantStreamBlock[],
  blockIndex: number,
  toolCallId: string,
): boolean {
  const normalizedToolCallId = String(toolCallId || "").trim();
  if (!normalizedToolCallId) return false;
  if (blocks.some((block) => hasInlineToolMarker(String(block.text || ""), normalizedToolCallId))) {
    return false;
  }
  let targetBlock = blocks[blockIndex];
  if (!targetBlock) return false;
  if (!String(targetBlock.text || "").trim()) {
    for (let index = blockIndex - 1; index >= 0; index -= 1) {
      if (!String(blocks[index].text || "").trim()) continue;
      targetBlock = blocks[index];
      break;
    }
  }
  const currentText = String(targetBlock.text || "");
  // 模型 delta 常以换行结尾；尾随空白会让渲染层 pre-wrap 段落把标记徽章顶到下一行
  targetBlock.text = currentText.trim()
    ? `${currentText.trimEnd()} [toolcall:${normalizedToolCallId}]`
    : `[toolcall:${normalizedToolCallId}]`;
  targetBlock.pendingTextBreak = true;
  return true;
}

export function appendReasoningDeltaToStreamBlocks(rawBlocks: unknown, delta: string): AssistantStreamBlock[] {
  const text = String(delta || "");
  const blocks = copyAssistantStreamBlocksForAppend(rawBlocks);
  if (!text) return blocks;
  const lastBlock = blocks[blocks.length - 1];
  if (lastBlock?.text?.trim() || (lastBlock?.tools || []).length > 0) {
    blocks.push({ reasoning: "", text: "", tools: [], pendingTextBreak: false });
  }
  const block = ensureAssistantStreamBlock(blocks);
  const previousReasoningCharCount = assistantStreamBlockReasoningCharCount(block);
  block.reasoning = `${String(block.reasoning || "")}${text}`;
  block.reasoningCharCount = previousReasoningCharCount + text.length;
  return blocks;
}

export function appendTextDeltaToStreamBlocks(rawBlocks: unknown, delta: string): AssistantStreamBlock[] {
  const text = String(delta || "");
  const blocks = copyAssistantStreamBlocksForAppend(rawBlocks);
  if (!text) return blocks;
  const block = ensureAssistantStreamBlock(blocks);
  const lastHasReasoning = !!String(block.reasoning || "").trim()
    || assistantStreamBlockReasoningCharCount(block) > 0;
  const needNewBlockForReasoning = lastHasReasoning && !String(block.text || "").trim()
    && (block.tools || []).length === 0;
  const needNewBlockForPendingBreak = !!block.pendingTextBreak && !!String(block.text || "").trim();
  if (needNewBlockForReasoning || needNewBlockForPendingBreak) {
    // 思维链块与正文块分离：reasoning-only 块后必切新块，避免“末尾正文”被吸到第0块
    // 贴在思维链后；工具标记后同理。块间边界由 joinAssistantHistoryTexts 按工具标记注入占位符。
    blocks.push({ reasoning: "", reasoningCharCount: 0, text: "", tools: [], pendingTextBreak: false });
  }
  const target = ensureAssistantStreamBlock(blocks);
  target.text = `${String(target.text || "")}${text}`;
  target.pendingTextBreak = false;
  return blocks;
}

export function applyAssistantToolEventToStreamBlocks(
  rawBlocks: unknown,
  rawMessage: unknown,
): AssistantStreamBlock[] {
  const blocks = cloneAssistantStreamBlocks(rawBlocks);
  const text = String(rawMessage || "").trim();
  if (!text) return normalizeAssistantStreamBlocks(blocks);
  let event: Record<string, unknown>;
  try {
    const parsed = JSON.parse(text);
    if (!parsed || typeof parsed !== "object") return normalizeAssistantStreamBlocks(blocks);
    event = parsed as Record<string, unknown>;
  } catch {
    return normalizeAssistantStreamBlocks(blocks);
  }
  const assistantText = String(event.content || "").trim();
  const reasoning = String(event.reasoning_content || "").trim();
  const tools = (Array.isArray(event.tool_calls) ? event.tool_calls : [])
    .map((raw) => {
      const call = raw && typeof raw === "object" ? raw as Record<string, unknown> : null;
      const func = (call?.function && typeof call.function === "object")
        ? call.function as Record<string, unknown>
        : {};
      const toolCallId = String(call?.id || call?.call_id || "").trim();
      const name = String(func.name || "").trim();
      if (!toolCallId || !name) return null;
      const { argumentsText } = normalizeToolCallArguments(func.arguments);
      return {
        toolCallId,
        name,
        argsText: argumentsText || "{}",
        status: "doing" as const,
      };
    })
    .filter((tool): tool is { toolCallId: string; name: string; argsText: string; status: "doing" } => !!tool);
  if (!assistantText && !reasoning && tools.length === 0) return normalizeAssistantStreamBlocks(blocks);
  if ((assistantText || reasoning) && blocks[blocks.length - 1]?.text?.trim()) {
    blocks.push({ reasoning: "", text: "", tools: [], pendingTextBreak: false });
  }
  const block = ensureAssistantStreamBlock(blocks);
  if (assistantText && !String(block.text || "").trim()) {
    block.text = assistantText;
  }
  if (reasoning && !String(block.reasoning || "").trim()) {
    block.reasoning = reasoning;
  }
  const blockIndex = blocks.indexOf(block);
  for (const tool of tools) {
    const existing = blocks
      .flatMap((item) => item.tools || [])
      .find((item) => String(item.toolCallId || "").trim() === tool.toolCallId);
    if (existing) {
      existing.name = tool.name;
      existing.argsText = tool.argsText || existing.argsText;
      existing.status = tool.status;
      continue;
    }
    block.tools = [...(block.tools || []), tool];
    attachInlineToolMarkerToStreamBlock(blocks, blockIndex, tool.toolCallId);
  }
  return normalizeAssistantStreamBlocks(blocks);
}

export function applyAssistantToolResultToStreamBlocks(
  rawBlocks: unknown,
  rawMessage: unknown,
): AssistantStreamBlock[] {
  const blocks = cloneAssistantStreamBlocks(rawBlocks);
  const text = String(rawMessage || "").trim();
  if (!text) return normalizeAssistantStreamBlocks(blocks);
  let event: Record<string, unknown>;
  try {
    const parsed = JSON.parse(text);
    if (!parsed || typeof parsed !== "object") return normalizeAssistantStreamBlocks(blocks);
    event = parsed as Record<string, unknown>;
  } catch {
    return normalizeAssistantStreamBlocks(blocks);
  }
  if (String(event.role || "").trim() !== "tool") {
    return normalizeAssistantStreamBlocks(blocks);
  }
  const toolCallId = String(event.tool_call_id || "").trim();
  if (!toolCallId) return normalizeAssistantStreamBlocks(blocks);
  const resultText = typeof event.content === "string" ? event.content : String(event.content || "");
  const resultMetadata = event.metadata && typeof event.metadata === "object"
    ? event.metadata as Record<string, unknown>
    : undefined;
  for (let blockIndex = 0; blockIndex < blocks.length; blockIndex += 1) {
    const block = blocks[blockIndex];
    const tool = (block.tools || []).find((item) => String(item.toolCallId || "").trim() === toolCallId);
    if (!tool) continue;
    tool.resultText = resultText;
    tool.resultMetadata = resultMetadata;
    tool.status = "done";
    attachInlineToolMarkerToStreamBlock(blocks, blockIndex, toolCallId);
    return normalizeAssistantStreamBlocks(blocks);
  }
  return normalizeAssistantStreamBlocks(blocks);
}

export function streamBlocksToToolHistoryEvents(rawBlocks: unknown): ChatMessage["toolCall"] {
  const events: NonNullable<ChatMessage["toolCall"]> = [];
  for (const block of normalizeAssistantStreamBlocks(rawBlocks)) {
    const tools = block.tools || [];
    const reasoning = String(block.reasoning || "").trim();
    if (!reasoning && tools.length === 0) continue;
    events.push({
      role: "assistant",
      content: String(block.text || "").trim() ? String(block.text || "") : null,
      reasoning_content: reasoning || undefined,
      tool_calls: tools.length > 0
        ? tools.map((tool) => ({
            id: tool.toolCallId,
            type: "function",
            function: {
              name: tool.name,
              arguments: tool.argsText || "{}",
            },
          }))
        : undefined,
    });
    for (const tool of tools) {
      if (tool.status === "doing" && !String(tool.resultText || "").trim()) continue;
      events.push({
        role: "tool",
        tool_call_id: tool.toolCallId,
        content: String(tool.resultText || ""),
        metadata: tool.resultMetadata,
      });
    }
  }
  return events.length > 0 ? events : undefined;
}

export function appendReasoningToStreamActivityItems(
  currentItems: ChatActivityItem[],
  delta: string,
): ChatActivityItem[] {
  const text = String(delta || "");
  if (!text) return normalizeChatActivityItems(currentItems);
  const items = normalizeChatActivityItems(currentItems);
  const last = items[items.length - 1];
  if (last?.kind === "reasoning") {
    return [
      ...items.slice(0, -1),
      {
        ...last,
        text: `${last.text}${text}`,
        running: true,
      },
    ];
  }
  return [
    ...items,
    {
      kind: "reasoning",
      id: `stream-reasoning-${items.length}`,
      text,
      running: true,
    },
  ];
}

export function projectStreamingChatActivityForDisplay(input: {
  toolCalls?: Array<{ toolCallId?: string; name: string; argsText: string; status?: "doing" | "done" }>;
  activityItems?: ChatActivityItem[];
  streamBlocks?: AssistantStreamBlock[];
  running?: boolean;
}): {
  items: ChatActivityItem[];
  activityReasoningCharCount: number;
  activityToolCountsByName: Record<string, number>;
  activityRunning: boolean;
  activityStatus: ChatActivityStatus;
} {
  const activityItems = normalizeChatActivityItems(input.activityItems);
  const normalizedBlocks = normalizeAssistantStreamBlocks(input.streamBlocks);
  const blockItems = streamBlocksToActivitySummaryItems(normalizedBlocks, !!input.running);
  const eventItems = blockItems.length > 0 ? blockItems : activityItems;
  const usingEventItems = eventItems.length > 0;
  const items: ChatActivityItem[] = usingEventItems ? eventItems : [];
  const toolCalls = Array.isArray(input.toolCalls) ? input.toolCalls : [];
  if (!usingEventItems) {
    for (const [index, call] of toolCalls.entries()) {
      const name = String(call.name || "").trim();
      if (!name) continue;
      items.push({
        kind: "tool",
        id: String(call.toolCallId || "").trim() || `stream-tool-${index}`,
        toolCallId: String(call.toolCallId || "").trim() || undefined,
        name,
        argsText: String(call.argsText || ""),
        status: String(call.status || "") === "doing" ? "doing" : "done",
      });
    }
  }
  const activityRunning = !!input.running;
  const hasDoingTool = items.some((item) => item.kind === "tool" && item.status === "doing");
  const blockReasoningCharCount = streamBlocksReasoningCharCount(normalizedBlocks);
  const hasReasoningItem = blockReasoningCharCount > 0
    || items.some((item) => item.kind === "reasoning" && !!String(item.text || "").trim());
  const hasContentItem = normalizedBlocks.some((block) => !!String(block.text || "").trim())
    || items.some((item) => item.kind === "content" && !!String(item.text || "").trim());
  const status: ChatActivityStatus = hasDoingTool
    ? "running_tool"
    : hasReasoningItem
      ? "thinking"
      : hasContentItem
        ? (activityRunning ? "thinking" : "complete")
        : activityRunning
        ? "requesting"
        : items.length > 0
          ? "complete"
          : "idle";
  const fallbackStats = normalizedBlocks.length > 0
    ? null
    : chatActivityStats(items, activityRunning, status);
  return {
    items,
    activityReasoningCharCount: normalizedBlocks.length > 0
      ? blockReasoningCharCount
      : (fallbackStats?.activityReasoningCharCount || 0),
    activityToolCountsByName: normalizedBlocks.length > 0
      ? streamBlocksToolCountsByName(normalizedBlocks)
      : (fallbackStats?.activityToolCountsByName || {}),
    activityRunning,
    activityStatus: status,
  };
}

function resolveTaskTrigger(message: ChatMessage): TaskTriggerMessageCard | undefined {
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  if (String(meta.messageKind || "").trim() !== "task_trigger") return undefined;
  const raw = meta.taskTrigger;
  if (!raw || typeof raw !== "object") return undefined;
  const card = raw as Record<string, unknown>;
  const goal = String(card.goal || card.title || "").trim();
  if (!goal) return undefined;
  return {
    taskId: String(card.taskId || "").trim() || undefined,
    goal,
    why: String(card.why || card.cause || "").trim() || undefined,
    todo: String(card.how || "").trim() || undefined,
    runAt: String(card.run_at || card.runAt || card.runAtLocal || "").trim() || undefined,
    cronExpression:
      String(card.cron_expression || card.cronExpression || card.every_minutes || card.everyMinutes || "").trim()
      || undefined,
    endAt: String(card.end_at || card.endAt || card.endAtLocal || "").trim() || undefined,
    nextRunAt: String(card.next_run_at || card.nextRunAt || card.nextRunAtLocal || "").trim() || undefined,
  };
}

function resolvePlanCard(message: ChatMessage): PlanMessageCard | undefined {
  // fallback: providerMeta 无 planCard 时从 tool call 历史找
  if (!(message.providerMeta || {}).planCard) {
    try {
      const events = Array.isArray(message.toolCall) ? message.toolCall : [];
      for (const event of events) {
        const role = String(event?.role || "").trim().toLowerCase();
        if (role !== "assistant") continue;
        const calls = Array.isArray(event?.tool_calls) ? event.tool_calls : [];
        for (const call of calls) {
          const func = (call?.function || {}) as Record<string, unknown>;
          if (String(func.name || "").trim().toLowerCase() !== "plan") continue;
          const args = JSON.parse(String(func.arguments || "{}").trim());
          const action = String(args.action || "").trim().toLowerCase();
          const path = String(args.path || "").trim();
          if (action === "present" && path) return { action, path };
        }
      }
    } catch { /* ignore */ }
  }
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  const raw = meta.planCard;
  if (!raw || typeof raw !== "object") return undefined;
  const card = raw as Record<string, unknown>;
  const action = String(card.action || "").trim().toLowerCase();
  if (action !== "present" && action !== "complete") return undefined;
  const path = String(card.path || "").trim();
  if (!path) return undefined;
  const context = String(card.context || "").trim();
  return {
    action,
    path,
    context: context || undefined,
  };
}

function resolveSpeakerAgentId(message: ChatMessage): string {
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  const origin = meta.origin as Record<string, unknown> | undefined;
  if (origin && origin.kind === "remote_im") {
    return "";
  }
  const direct = String(message.speakerAgentId || "").trim();
  if (direct) return direct;
  for (const key of [
    "speakerAgentId",
    "speaker_agent_id",
    "targetAgentId",
    "target_agent_id",
    "agentId",
    "agent_id",
    "sourceAgentId",
    "source_agent_id",
  ]) {
    const value = String(meta[key] || "").trim();
    if (value) return value;
  }
  return "";
}

function resolveMessageMentions(message: ChatMessage): ChatMentionTarget[] {
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  const messageMeta = ((meta.message_meta || meta.messageMeta || {}) as Record<string, unknown>);
  const raw = Array.isArray(messageMeta.mentions) ? messageMeta.mentions : [];
  const seen = new Set<string>();
  const mentions: ChatMentionTarget[] = [];
  for (const item of raw) {
    if (!item || typeof item !== "object") continue;
    const mention = item as Record<string, unknown>;
    const agentId = String(mention.agentId || "").trim();
    if (!agentId) continue;
    if (seen.has(agentId)) continue;
    seen.add(agentId);
    mentions.push({
      agentId,
      agentName: String(mention.agentName || agentId).trim() || agentId,
      avatarUrl: undefined,
    });
  }
  return mentions;
}

export function applyMemeAnnotationReplacements(text: string, annotations?: MemeAnnotation[]): string {
  if (!annotations || annotations.length === 0) return text;
  let cursor = 0;
  let result = "";
  const appendMemeImageBlock = (base: string, imageMarkdown: string): string => {
    const trimmedBase = base.replace(/[ \t]+$/g, "");
    const separator = !trimmedBase
      ? ""
      : trimmedBase.endsWith("\n\n")
        ? ""
        : trimmedBase.endsWith("\n")
          ? "\n"
          : "\n\n";
    return `${trimmedBase}${separator}${imageMarkdown}\n\n`;
  };
  for (const { meme, path } of annotations) {
    const token = String(meme || "").trim();
    const imagePath = String(path || "").trim();
    if (!token || !imagePath) continue;
    const nextIndex = text.indexOf(token, cursor);
    if (nextIndex < 0) continue;
    let replaceStart = nextIndex;
    let replaceEnd = nextIndex + token.length;
    // AI 常写成 (:坏笑:) / （:坏笑:），替换时一并吃掉成对括号
    const open = replaceStart > 0 ? text[replaceStart - 1] : "";
    const close = replaceEnd < text.length ? text[replaceEnd] : "";
    if ((open === "(" && close === ")") || (open === "（" && close === "）")) {
      replaceStart -= 1;
      replaceEnd += 1;
    }
    const alt = token.startsWith(":") && token.endsWith(":") && token.length > 2
      ? token.slice(1, -1)
      : token;
    result += text.slice(cursor, replaceStart);
    result = appendMemeImageBlock(result, `![${alt}](${imagePath})`);
    cursor = replaceEnd;
    while (cursor < text.length && /[ \t]/.test(text[cursor] || "")) {
      cursor += 1;
    }
  }
  return result ? `${result}${text.slice(cursor)}` : text;
}

export function projectMessageForDisplay(
  message: ChatMessage,
  taskTriggerLabels?: TaskTriggerDisplayLabels,
): ChatMessageDisplayProjection {
  const rendered = removeBinaryPlaceholders(renderMessage(message));
  const canonicalAssistantText = message.role === "assistant"
    ? assistantTextFromStreamBlocks(assistantContentBlocksFromMessage(message))
    : "";
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  const toolSummary = summarizeToolActivityForDisplay(message);
  const activity = projectChatActivityForDisplay(message);
  const taskTrigger = resolveTaskTrigger(message);
  const planCard = resolvePlanCard(message);
  const origin = meta.origin as Record<string, unknown> | undefined;
  const senderName = String(origin?.sender_name || "").trim();
  const remoteContactName = String(origin?.contact_name || "").trim();
  const channelId = String(origin?.channel_id || "").trim();
  const contactId = String(origin?.contact_id || "").trim();
  const messageKind = String(meta.messageKind || "").trim();
  const goalLabel = String(taskTriggerLabels?.goal || "").trim() || "Goal";
  const todoLabel = String(taskTriggerLabels?.todo || "").trim() || "Todo";
  const displayText =
    taskTrigger && messageKind === "task_trigger"
      ? [
        `**${goalLabel}**`,
        taskTrigger.goal,
        "",
        `**${todoLabel}**`,
        String(taskTrigger.todo || "").trim(),
      ].filter((line, index) => {
        if (!String(line || "").trim()) return index === 0 || index === 3;
        return true;
      }).join("\n")
      : message.role === "assistant"
        ? mergedAssistantDisplayText(message, canonicalAssistantText || rendered.trim())
        : rendered;
  const displayTextWithMeme = applyMemeAnnotationReplacements(displayText, message.memeAnnotations);
  return {
    speakerAgentId: resolveSpeakerAgentId(message) || undefined,
    mentions: resolveMessageMentions(message),
    text: displayTextWithMeme,
    images: extractMessageImages(message),
    audios: extractMessageAudios(message),
    attachmentFiles: extractMessageAttachmentFiles(message),
    taskTrigger,
    planCard,
    remoteImOrigin:
      origin && origin.kind === "remote_im" && (senderName || remoteContactName || channelId || contactId)
        ? {
          senderName,
          remoteContactName: remoteContactName || undefined,
          remoteContactType: String(origin.contact_type || "private").trim() || "private",
          channelId,
          contactId,
        }
        : undefined,
    toolCallCount: toolSummary.count,
    lastToolName: toolSummary.lastToolName,
    toolCalls: toolSummary.calls,
    activityItems: activity.items,
    activityReasoningCharCount: activity.activityReasoningCharCount,
    activityToolCountsByName: activity.activityToolCountsByName,
    activityRunning: activity.activityRunning,
    activityStatus: activity.activityStatus,
  };
}

function defaultToolResultSuccess(rawMetadata: unknown): boolean {
  if (!rawMetadata || typeof rawMetadata !== "object") return false;
  const metadata = rawMetadata as Record<string, unknown>;
  return typeof metadata.backup_record_id === "string" && metadata.backup_record_id.trim().length > 0;
}

export function inspectUndoablePatchCalls(
  messages: ChatMessage[],
  turnId: string,
  options?: {
    isApplyPatchArgsUndoable?: (rawArgs: string) => boolean;
    isToolResultSuccess?: (rawMetadata: unknown) => boolean;
  },
): { canUndo: boolean; hint: string } {
  const targetId = String(turnId || "").trim();
  if (!targetId) {
    return { canUndo: false, hint: "未找到有效消息 ID。" };
  }
  const directIndex = messages.findIndex((item) => String(item.id || "").trim() === targetId);
  if (directIndex < 0) {
    return { canUndo: false, hint: "未找到目标消息。" };
  }
  let removeFrom = directIndex;
  if (String(messages[directIndex]?.role || "").trim() !== "user") {
    removeFrom = -1;
    for (let i = directIndex - 1; i >= 0; i -= 1) {
      if (String(messages[i]?.role || "").trim() === "user") {
        removeFrom = i;
        break;
      }
    }
    if (removeFrom < 0) {
      return { canUndo: false, hint: "未找到可撤回的用户消息。" };
    }
  }

  const isApplyPatchArgsUndoable = options?.isApplyPatchArgsUndoable || (() => false);
  const isToolResultSuccess = options?.isToolResultSuccess || defaultToolResultSuccess;
  const pendingApplyPatchCalls = new Set<string>();
  let sawApplyPatchCall = false;
  let sawUndoableApplyPatchCall = false;
  for (const message of messages.slice(removeFrom)) {
    for (const event of normalizeMessageToolHistoryEvents(message, "display")) {
      if (event.role === "assistant") {
        for (const call of event.toolCalls) {
          if (call.toolName === "apply_patch") {
            sawApplyPatchCall = true;
          }
          if (
            call.toolName === "apply_patch"
            && call.invocationId
            && isApplyPatchArgsUndoable(call.argumentsText)
          ) {
            sawUndoableApplyPatchCall = true;
            pendingApplyPatchCalls.add(call.invocationId);
          }
        }
        continue;
      }
      if (event.role === "tool" && event.toolCallId && pendingApplyPatchCalls.has(event.toolCallId)) {
        if (isToolResultSuccess(event.metadata)) {
          return { canUndo: true, hint: "" };
        }
        pendingApplyPatchCalls.delete(event.toolCallId);
      }
    }
  }

  if (!sawApplyPatchCall) {
    return { canUndo: false, hint: "该范围内没有检测到可撤回的工具修改。" };
  }
  if (!sawUndoableApplyPatchCall) {
    return { canUndo: false, hint: "检测到工具调用，但参数不完整，无法安全撤回修改。" };
  }
  return { canUndo: false, hint: "检测到 apply_patch，但执行未成功或结果不可逆，无法撤回修改。" };
}
