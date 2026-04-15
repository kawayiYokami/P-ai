import type { ChatMessage, PlanMessageCard, TaskTriggerMessageCard } from "../types/app";
import {
  extractMessageAttachmentFiles,
  extractMessageAudios,
  extractMessageImages,
  parseAssistantStoredText,
  removeBinaryPlaceholders,
  renderMessage,
} from "./chat-message";

type ToolHistoryView = "display" | "prompt";

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
};

export type ChatMessageDisplayProjection = {
  speakerAgentId?: string;
  text: string;
  images: Array<{ mime: string; bytesBase64?: string; mediaRef?: string }>;
  audios: Array<{ mime: string; bytesBase64: string }>;
  attachmentFiles: Array<{ fileName: string; relativePath: string }>;
  taskTrigger?: TaskTriggerMessageCard;
  planCard?: PlanMessageCard;
  remoteImOrigin?: {
    senderName: string;
    remoteContactName?: string;
    remoteContactType: string;
    channelId: string;
    contactId: string;
  };
  reasoningStandard: string;
  reasoningInline: string;
  toolCallCount: number;
  lastToolName: string;
  toolCalls: Array<{ name: string; argsText: string }>;
};

function sanitizeStoredToolHistoryEvents(
  events: ChatMessage["toolCall"],
): Array<Record<string, unknown>> {
  if (!Array.isArray(events) || events.length === 0) return [];
  const sanitized: Array<Record<string, unknown>> = [];
  let pendingAssistantIndex: number | null = null;
  for (const raw of events) {
    if (!raw || typeof raw !== "object") continue;
    const event = raw as Record<string, unknown>;
    const role = String(event.role || "").trim().toLowerCase();
    if (role === "assistant") {
      const calls = Array.isArray(event.tool_calls) ? event.tool_calls : [];
      const hasToolCalls = calls.length > 0;
      const index = sanitized.length;
      sanitized.push(event);
      pendingAssistantIndex = hasToolCalls ? index : null;
      continue;
    }
    if (role === "tool") {
      if (pendingAssistantIndex !== null) {
        sanitized.push(event);
        pendingAssistantIndex = null;
      }
      continue;
    }
    pendingAssistantIndex = null;
    sanitized.push(event);
  }
  if (pendingAssistantIndex !== null) {
    sanitized.length = pendingAssistantIndex;
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
    runAtLocal: String(card.runAtLocal || card.runAt || "").trim() || undefined,
    endAtLocal: String(card.endAtLocal || card.endAt || "").trim() || undefined,
    nextRunAtLocal: String(card.nextRunAtLocal || card.nextRunAt || "").trim() || undefined,
    everyMinutes: Number.isFinite(Number(card.everyMinutes)) ? Number(card.everyMinutes) : undefined,
  };
}

function resolvePlanCard(message: ChatMessage): PlanMessageCard | undefined {
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  const raw = meta.planCard;
  if (!raw || typeof raw !== "object") return undefined;
  const card = raw as Record<string, unknown>;
  const action = String(card.action || "").trim().toLowerCase();
  if (action !== "present" && action !== "complete") return undefined;
  const context = String(card.context || "").trim();
  if (!context) return undefined;
  return {
    action,
    context,
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

export function projectMessageForDisplay(message: ChatMessage): ChatMessageDisplayProjection {
  const rendered = removeBinaryPlaceholders(renderMessage(message));
  const parsed = parseAssistantStoredText(rendered);
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  const toolSummary = summarizeToolActivityForDisplay(message);
  const taskTrigger = resolveTaskTrigger(message);
  const planCard = resolvePlanCard(message);
  const origin = meta.origin as Record<string, unknown> | undefined;
  const senderName = String(origin?.sender_name || "").trim();
  const remoteContactName = String(origin?.contact_name || "").trim();
  const channelId = String(origin?.channel_id || "").trim();
  const contactId = String(origin?.contact_id || "").trim();
  const messageKind = String(meta.messageKind || "").trim();
  const displayText =
    taskTrigger && messageKind === "task_trigger"
      ? ""
      : message.role === "assistant"
        ? parsed.assistantText
        : rendered;
  return {
    speakerAgentId: resolveSpeakerAgentId(message) || undefined,
    text: displayText,
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
    reasoningStandard: parsed.reasoningStandard || String(meta.reasoningStandard || "").trim(),
    reasoningInline: parsed.reasoningInline || String(meta.reasoningInline || "").trim(),
    toolCallCount: toolSummary.count,
    lastToolName: toolSummary.lastToolName,
    toolCalls: toolSummary.calls,
  };
}

function defaultToolResultSuccess(rawContent: unknown): boolean {
  const text = String(rawContent || "").trim();
  if (!text) return false;
  try {
    const parsed = JSON.parse(text) as { ok?: unknown; approved?: unknown };
    return parsed.ok === true && parsed.approved !== false;
  } catch {
    return false;
  }
}

export function inspectUndoablePatchCalls(
  messages: ChatMessage[],
  turnId: string,
  options?: {
    isApplyPatchArgsUndoable?: (rawArgs: string) => boolean;
    isToolResultSuccess?: (rawContent: unknown) => boolean;
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
        if (isToolResultSuccess(event.text)) {
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
