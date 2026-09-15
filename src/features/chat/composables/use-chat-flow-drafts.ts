import type { Ref } from "vue";
import type { AssistantStreamBlock, ChatMentionTarget, ChatMessage } from "../../../types/app";
import {
  assistantTextFromStreamBlocks,
  assistantContentBlocksFromMessage,
  normalizeAssistantStreamBlocks,
  normalizeChatActivityItems,
  stripToolcallMarkers,
  streamBlocksToToolCalls,
} from "../../../utils/chat-message-semantics";
import { readMessagePlainText, messageHasVisibleContent } from "./use-chat-flow-utils";
import { messageWithStableRenderId } from "../utils/stable-render-id";
import { probeChatFlow } from "./chat-flow-probe";
import {
  assistantMessageHasCanonicalVisibleContent,
  createChatMessageState,
  reconcileCompletedAssistantMessage,
  reduceChatMessageState,
  type ChatMessageEvent,
  type ChatMessageState,
} from "./chat-message-state-machine";
import {
  transportAssistantDeltaToMessageEvent,
  transportRoundStartedToMessageEvent,
} from "./chat-message-transport-adapter";
import type { AssistantDeltaEvent, RoundStartedPayload } from "./use-chat-flow-events";

export const DRAFT_USER_ID_PREFIX = "__draft_user__:";

type UpdateMessageTextOptions = {
  preserveActivityProjection?: boolean;
};

function messageHasActivityEvents(message: ChatMessage): boolean {
  if (normalizeChatActivityItems(message.activityItems).length > 0) return true;
  if (!Array.isArray(message.toolCall)) return false;
  return message.toolCall.some((event) => {
    const raw = event && typeof event === "object" ? event as Record<string, unknown> : null;
    if (!raw) return false;
    if (String(raw.reasoning_content || "").trim()) return true;
    return Array.isArray(raw.tool_calls) && raw.tool_calls.length > 0;
  });
}

function assistantMessageHasVisibleProgress(message?: ChatMessage | null): boolean {
  if (!message) return false;
  if (readMessagePlainText(message).trim()) return true;
  if (messageHasActivityEvents(message)) return true;
  const streamBlocks = assistantContentBlocksFromMessage(message);
  return streamBlocks.length > 0 || !!assistantTextFromStreamBlocks(streamBlocks).trim();
}

type UseChatFlowDraftsOptions = {
  allMessages: Ref<ChatMessage[]>;
  latestUserText: Ref<string>;
  getActiveRoundAgentId?: () => string;
  getConversationId?: () => string;
  getSendStartedAtMs: (gen: number) => number;
  getActiveHistoryMessageCount: () => number;
  getFrontendDispatchStartedAtMs: () => number;
  currentFrontendDispatchElapsedMs: () => number;
};

export function useChatFlowDrafts(options: UseChatFlowDraftsOptions) {
  let pendingUserDraftId = "";
  const pendingUserDraftIdByGen = new Map<number, string>();
  let messageState = createChatMessageState(
    String(options.getConversationId ? options.getConversationId() : "").trim() || "__foreground__",
    options.allMessages.value,
  );

  function machineConversationId(): string {
    return String(options.getConversationId ? options.getConversationId() : "").trim() || "__foreground__";
  }

  function synchronizeMachineInput(): ChatMessageState {
    const conversationId = machineConversationId();
    if (messageState.conversationId !== conversationId) {
      messageState = createChatMessageState(conversationId, options.allMessages.value);
      return messageState;
    }
    if (messageState.messages !== options.allMessages.value) {
      messageState = { ...messageState, messages: options.allMessages.value };
    }
    if (messageState.round.phase !== "idle") {
      const activeMessage = messageState.messages.find((message) => message.id === messageState.round.assistantMessageId);
      // An authoritative messageAppended may arrive before roundFinished and
      // legitimately clear `_streaming`. Keep the projection round anchored
      // by message identity until its terminal event is reduced; otherwise a
      // stale terminal can bypass the shared reducer's identity guard.
      if (!activeMessage || activeMessage.role !== "assistant") {
        messageState = createChatMessageState(conversationId, options.allMessages.value);
      }
    }
    return messageState;
  }

  function dispatchMessageEvent(event: ChatMessageEvent): ChatMessageState {
    synchronizeMachineInput();
    messageState = reduceChatMessageState(messageState, event);
    options.allMessages.value = messageState.messages;
    return messageState;
  }

  function ensureProjectionRound(
    messageId: string,
    statusText = "",
    phase: "waiting" | "streaming" = "streaming",
    identity?: Partial<RoundStartedPayload>,
  ) {
    const normalizedMessageId = String(messageId || "").trim();
    if (!normalizedMessageId) {
      probeChatFlow("建气泡跳过", { reason: "empty_message_id", phase });
      return;
    }
    synchronizeMachineInput();
    if (messageState.round.phase !== "idle" && messageState.round.assistantMessageId !== normalizedMessageId) {
      dispatchMessageEvent({
        type: "round_reset",
        conversationId: machineConversationId(),
        preserveVisibleContent: true,
      });
    }
    if (messageState.round.phase === "idle") {
      const existing = options.allMessages.value.find((message) => message.id === normalizedMessageId);
      const event = transportRoundStartedToMessageEvent({
        ...identity,
        startedAt: identity?.startedAt || existing?.createdAt || new Date().toISOString(),
      }, machineConversationId(), {
        assistantMessageId: normalizedMessageId,
        speakerAgentId: resolveAssistantMessageSpeakerAgentId(existing),
        statusText,
        phase,
      });
      if (event) {
        dispatchMessageEvent(event);
        probeChatFlow("建气泡", {
          messageId: normalizedMessageId,
          phase,
          total: options.allMessages.value.length,
        });
      }
    }
  }

  function resolveAssistantMessageSpeakerAgentId(existingMessage?: ChatMessage | null): string {
    const existing = String(existingMessage?.speakerAgentId || "").trim();
    if (existing) return existing;
    const activeRoundAgentId = String(options.getActiveRoundAgentId ? options.getActiveRoundAgentId() : "").trim();
    if (activeRoundAgentId) return activeRoundAgentId;
    return "";
  }

  function getPendingUserDraftId(): string {
    return pendingUserDraftId;
  }

  function getPendingUserDraftIdForGen(gen: number): string {
    return pendingUserDraftIdByGen.get(gen) || "";
  }

  function getMessageStreamBlocks(messageId: string): AssistantStreamBlock[] {
    if (!messageId) return [];
    const draft = options.allMessages.value.find((item) => item.id === messageId);
    return assistantContentBlocksFromMessage(draft);
  }

  function hasStreamingAssistantMessageInMessages(): boolean {
    return options.allMessages.value.some((message) => {
      const messageId = String(message?.id || "").trim();
      const meta = (message?.providerMeta || {}) as Record<string, unknown>;
      return String(message?.role || "").trim() === "assistant" && meta._streaming === true;
    });
  }

  function insertUserDraft(
    rawMessageId: string,
    gen: number,
    text: string,
    images: Array<{ mime: string; bytesBase64: string; savedPath?: string }>,
    attachments: Array<{ fileName: string; path: string; mime: string }>,
    extraTextBlocks: string[],
    mentions: ChatMentionTarget[],
  ): string {
    const messageId = String(rawMessageId || "").trim();
    if (!messageId) return "";
    const parts: ChatMessage["parts"] = [];
    const normalizedText = String(text || "");
    if (normalizedText) {
      parts.push({ type: "text", text: normalizedText });
    }
    const seenAttachmentPaths = new Set<string>();
    for (const image of images) {
      const mime = String(image.mime || "").trim();
      const path = String(image.savedPath || "").trim().replace(/\\/g, "/");
      if (!mime || !path) continue;
      seenAttachmentPaths.add(path.toLowerCase());
      parts.push({ type: "attachment", path, mime, name: path.split("/").pop() || "image" });
    }
    for (const attachment of attachments) {
      const path = String(attachment.path || "").trim().replace(/\\/g, "/");
      if (!path || seenAttachmentPaths.has(path.toLowerCase())) continue;
      seenAttachmentPaths.add(path.toLowerCase());
      parts.push({
        type: "attachment",
        path,
        mime: String(attachment.mime || "").trim(),
        name: String(attachment.fileName || "").trim() || path.split("/").pop() || "attachment",
      });
    }
    const msg: ChatMessage = {
      id: messageId,
      role: "user",
      createdAt: new Date().toISOString(),
      speakerAgentId: "user-persona",
      parts,
      extraTextBlocks: Array.isArray(extraTextBlocks) ? extraTextBlocks.filter((item) => !!String(item || "").trim()) : [],
      providerMeta: {
        message_meta: mentions.length > 0
          ? {
              kind: "user_message",
              mentions: mentions.map((item) => ({
                agentId: item.agentId,
                agentName: item.agentName,
              })),
            }
        : undefined,
      },
    };
    const stableMsg = messageWithStableRenderId(msg, messageId);
    const cur = options.allMessages.value;
    const idx = cur.findIndex((m) => m.id === messageId);
    if (idx >= 0) {
      return messageId;
    }
    options.allMessages.value = [...cur, stableMsg];
    return messageId;
  }

  function insertStreamingAssistantMessage(messageId: string, gen?: number, initialText = ""): string {
    const normalizedMessageId = String(messageId || "").trim();
    if (!normalizedMessageId) return "";
    ensureProjectionRound(normalizedMessageId, initialText, "streaming");
    dispatchMessageEvent({
      type: "assistant_stream_snapshot",
      conversationId: machineConversationId(),
      assistantMessageId: normalizedMessageId,
      snapshot: {
        persistedAssistantMessageId: normalizedMessageId,
        startedAtMs: typeof gen === "number" ? options.getSendStartedAtMs(gen) || undefined : undefined,
        speakerAgentId: resolveAssistantMessageSpeakerAgentId(
          options.allMessages.value.find((message) => message.id === normalizedMessageId),
        ),
        streamBlocks: [],
        toolStatusText: undefined,
        toolStatusState: undefined,
        preStreamingStatusText: String(initialText || ""),
        frontendDispatchStartedAtMs: options.getFrontendDispatchStartedAtMs(),
        frontendDispatchElapsedMs: options.currentFrontendDispatchElapsedMs(),
      },
    });
    return normalizedMessageId;
  }

  function updateQueuedAssistantMessageStatus(
    messageId: string,
    statusText: string,
    identity?: Partial<RoundStartedPayload>,
  ) {
    if (!messageId) return;
    const existingMessage = options.allMessages.value.find((item) => item.id === messageId);
    if (String(existingMessage?.role || "") === "assistant") {
      const existingMeta = (existingMessage?.providerMeta || {}) as Record<string, unknown>;
      // 已有可见正文/活动/工具事件的已完成消息不被覆盖；
      // 但若是后端已持久化但尚无正文的空占位消息（例如出队/引导消息插入时预写库的空消息），
      // 必须允许激活进入 waiting 状态并打上 _streaming 标记，否则前端气泡无法显示。
      if (assistantMessageHasVisibleProgress(existingMessage)) {
        return;
      }
    }
    ensureProjectionRound(messageId, statusText, "waiting", identity);
    dispatchMessageEvent({
      type: "assistant_stream_snapshot",
      conversationId: machineConversationId(),
      assistantMessageId: messageId,
      snapshot: {
        persistedAssistantMessageId: messageId,
        speakerAgentId: resolveAssistantMessageSpeakerAgentId(existingMessage),
        streamBlocks: assistantContentBlocksFromMessage(existingMessage),
        toolStatusText: undefined,
        toolStatusState: undefined,
        preStreamingStatusText: String(statusText || ""),
        frontendDispatchStartedAtMs: options.getFrontendDispatchStartedAtMs(),
        frontendDispatchElapsedMs: options.currentFrontendDispatchElapsedMs(),
      },
    });
  }

  function syncStreamBlocksToMessage(
    messageId: string,
    rawBlocks?: AssistantStreamBlock[],
    runtimeStatus?: { toolStatusText?: string; toolStatusState?: string },
  ) {
    if (!messageId) return;
    const blocks = normalizeAssistantStreamBlocks(rawBlocks);
    ensureProjectionRound(messageId, "", "streaming");
    const existingMessage = options.allMessages.value.find((item) => item.id === messageId);
    const existingMeta = (existingMessage?.providerMeta || {}) as Record<string, unknown>;
    // toolStatus 只在快照显式携带时覆盖（恢复路径）；普通流式 dispatch 不传，
    // 状态机保留消息上已有的调度阶段提示，避免空值覆盖真值。
    const toolStatusText = runtimeStatus?.toolStatusText !== undefined
      ? String(runtimeStatus.toolStatusText || "")
      : undefined;
    const toolStatusState = runtimeStatus?.toolStatusState !== undefined
      ? String(runtimeStatus.toolStatusState || "").trim()
      : undefined;
    dispatchMessageEvent({
      type: "assistant_stream_snapshot",
      conversationId: machineConversationId(),
      assistantMessageId: messageId,
      snapshot: {
        persistedAssistantMessageId: messageId,
        streamBlocks: blocks,
        toolStatusText,
        toolStatusState,
        frontendDispatchStartedAtMs: options.getFrontendDispatchStartedAtMs(),
        frontendDispatchElapsedMs: options.currentFrontendDispatchElapsedMs(),
      },
    });
  }

  function updateMessageText(
    messageId: string,
    rawBlocks?: AssistantStreamBlock[],
    updateOptions?: UpdateMessageTextOptions,
    runtimeStatus?: { toolStatusText?: string; toolStatusState?: string },
  ) {
    if (!messageId) return;
    const existingMessage = options.allMessages.value.find((item) => item.id === messageId);
    const agentId = resolveAssistantMessageSpeakerAgentId(existingMessage);
    const streamBlocks = rawBlocks === undefined
      ? getMessageStreamBlocks(messageId)
      : normalizeAssistantStreamBlocks(rawBlocks);
    const hasVisibleStreamContent =
      streamBlocks.length > 0
      || !!assistantTextFromStreamBlocks(streamBlocks).trim();
    const existingMeta = (existingMessage?.providerMeta || {}) as Record<string, unknown>;
    // 调度阶段提示保留消息上已有值（不主动清空）；有可见内容时由状态机归零。
    const preStreamingStatusText = hasVisibleStreamContent
      ? undefined
      : String(existingMeta._toolStatusText || existingMeta._preStreamingStatusText || "").trim() || undefined;
    // toolStatus 只在调用方显式携带时覆盖（失败/完成/恢复路径）；普通流式不传，
    // 状态机保留消息上已有的调度阶段提示，避免空值覆盖真值。
    const toolStatusText = runtimeStatus?.toolStatusText !== undefined
      ? String(runtimeStatus.toolStatusText || "")
      : undefined;
    const toolStatusState = runtimeStatus?.toolStatusState !== undefined
      ? String(runtimeStatus.toolStatusState || "").trim()
      : undefined;
    ensureProjectionRound(messageId, preStreamingStatusText || "", "streaming");
    dispatchMessageEvent({
      type: "assistant_stream_snapshot",
      conversationId: machineConversationId(),
      assistantMessageId: messageId,
      snapshot: {
        persistedAssistantMessageId: messageId,
        assistantText: assistantTextFromStreamBlocks(streamBlocks),
        speakerAgentId: agentId,
        streamBlocks,
        preStreamingStatusText,
        toolStatusText,
        toolStatusState,
        frontendDispatchStartedAtMs: options.getFrontendDispatchStartedAtMs(),
        frontendDispatchElapsedMs: options.currentFrontendDispatchElapsedMs(),
      },
    });
    void updateOptions;
  }

  function removeMessage(messageId: string) {
    if (!messageId) return;
    const existing = options.allMessages.value.find((message) => message.id === messageId);
    // 有内容的消息禁止删除；撤回走后端截断/整表替换，不经过这里。
    if (String(existing?.role || "").trim() === "assistant") {
      if (assistantMessageHasCanonicalVisibleContent(existing)) {
        finalizeMessage(messageId);
        return;
      }
      synchronizeMachineInput();
      if (messageState.round.phase !== "idle" && messageState.round.assistantMessageId === messageId) {
        dispatchMessageEvent({
          type: "round_failed",
          conversationId: machineConversationId(),
          assistantMessageId: messageId,
        });
        return;
      }
    } else if (messageHasVisibleContent(existing)) {
      return;
    }
    if (messageId === pendingUserDraftId) {
      pendingUserDraftId = "";
    }
    for (const [gen, userDraftId] of pendingUserDraftIdByGen.entries()) {
      if (userDraftId === messageId) {
        pendingUserDraftIdByGen.delete(gen);
      }
    }
    options.allMessages.value = options.allMessages.value.filter((m) => m.id !== messageId);
  }

  /**
   * 用户明确停止时，前台不能继续保留任何“正在生成”的投影。
   * 有可见内容的气泡冻结，空气泡直接移除；随后重建状态机，避免
   * `settling` 残留把已经停止的气泡重新标成流式。
   */
  function settleStreamingAssistantMessages(): string[] {
    synchronizeMachineInput();
    const settledIds: string[] = [];
    const nextMessages: ChatMessage[] = [];
    for (const message of options.allMessages.value) {
      const messageId = String(message?.id || "").trim();
      const providerMeta = (message?.providerMeta || {}) as Record<string, unknown>;
      const isStreamingAssistant = String(message?.role || "").trim() === "assistant"
        && providerMeta._streaming === true;
      if (!isStreamingAssistant) {
        nextMessages.push(message);
        continue;
      }
      if (messageId) settledIds.push(messageId);
      if (assistantMessageHasCanonicalVisibleContent(message)) {
        nextMessages.push(reconcileCompletedAssistantMessage(message) || message);
      }
    }
    options.allMessages.value = nextMessages;
    messageState = createChatMessageState(machineConversationId(), nextMessages);
    return settledIds;
  }

  function finalizeMessage(
    messageId: string,
    finalMessage?: ChatMessage,
    identity?: { activationId?: string; requestId?: string },
  ) {
    if (!messageId) return;
    dispatchMessageEvent({
      type: "round_finished",
      conversationId: machineConversationId(),
      assistantMessageId: messageId,
      activationId: identity?.activationId,
      requestId: identity?.requestId,
      assistantMessage: finalMessage,
    });
  }

  function failMessage(
    messageId: string,
    error?: unknown,
    identity?: { activationId?: string; requestId?: string },
  ) {
    if (!messageId) return;
    dispatchMessageEvent({
      type: "round_failed",
      conversationId: machineConversationId(),
      assistantMessageId: messageId,
      activationId: identity?.activationId,
      requestId: identity?.requestId,
      error,
    });
  }

  function applyAssistantEventToMessage(messageId: string, parsed: AssistantDeltaEvent) {
    if (!messageId) return;
    ensureProjectionRound(messageId, "", "streaming");
    const event = transportAssistantDeltaToMessageEvent(parsed, machineConversationId(), messageId);
    if (event) dispatchMessageEvent(event);
  }

  function applyAssistantDeltaToMessage(messageId: string, delta: string) {
    if (!messageId || !delta) return;
    applyAssistantEventToMessage(messageId, { delta });
  }

  return {
    applyAssistantDeltaToMessage,
    applyAssistantEventToMessage,
    failMessage,
    finalizeMessage,
    getMessageStreamBlocks,
    getPendingUserDraftId,
    getPendingUserDraftIdForGen,
    hasStreamingAssistantMessageInMessages,
    insertStreamingAssistantMessage,
    insertUserDraft,
    removeMessage,
    settleStreamingAssistantMessages,
    syncStreamBlocksToMessage,
    updateMessageText,
    updateQueuedAssistantMessageStatus,
  };
}

export function summarizeToolCallsText(streamBlocks?: AssistantStreamBlock[]): string {
  const toolCalls = streamBlocksToToolCalls(streamBlocks || []);
  if (toolCalls.length <= 0) return "";
  const lastToolName = toolCalls[toolCalls.length - 1]?.name || "";
  const extraCount = Math.max(0, toolCalls.length - 1);
  return extraCount > 0
    ? `调用 ${lastToolName || "-"} (+${extraCount})`
    : `调用 ${lastToolName || "-"}`;
}
