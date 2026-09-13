import type { ChatMessage } from "../../../types/app";
import {
  applyAssistantToolEventToStreamBlocks,
  applyAssistantToolResultToStreamBlocks,
  appendReasoningDeltaToStreamBlocks,
  appendTextDeltaToStreamBlocks,
  assistantContentBlocksFromMessage,
  assistantTextFromStreamBlocks,
  normalizeAssistantStreamBlocks,
} from "../../../utils/chat-message-semantics";
import {
  messageWithStableRenderId,
  preserveStableRenderId,
  stableRenderIdFromMessage,
} from "../utils/stable-render-id";
import { reconcileAuthoritativeConversationMessage } from "./chat-message-reconciliation";
import type {
  AuthoritativeMessageMergeOptions,
  ChatMessageEvent,
  ChatMessageRoundProjection,
  ChatMessageState,
  ChatMessageStreamSnapshot,
} from "./chat-message-state-types";
import { messageHasVisibleContent } from "./use-chat-flow-utils";

export type {
  AuthoritativeMessageMergeOptions,
  ChatAssistantDelta,
  ChatMessageEvent,
  ChatMessageRoundPhase,
  ChatMessageRoundProjection,
  ChatMessageState,
  ChatMessageStreamSnapshot,
} from "./chat-message-state-types";

const TRANSIENT_STREAM_META_KEYS = [
  "_streaming",
  "_preStreamingStatusText",
  "_toolStatusText",
  "_toolStatusState",
  "_frontendDispatchStartedAtMs",
  "_frontendDispatchElapsedMs",
];

function emptyRound(): ChatMessageRoundProjection {
  return {
    phase: "idle",
    assistantMessageId: "",
    activationId: "",
    requestId: "",
    revision: "",
    startedAtMs: 0,
  };
}

function normalized(value: unknown): string {
  return String(value || "").trim();
}

function positiveInteger(value: unknown): number {
  const next = Math.round(Number(value) || 0);
  return next > 0 ? next : 0;
}

function normalizeToolStatus(value: unknown): "running" | "done" | "failed" | "" {
  const status = normalized(value);
  return status === "running" || status === "done" || status === "failed" ? status : "";
}

function sameConversation(state: ChatMessageState, conversationId: string): boolean {
  return !!state.conversationId && state.conversationId === normalized(conversationId);
}

function providerMetaWithoutTransientStreamState(message?: ChatMessage | null): Record<string, unknown> {
  const providerMeta = { ...((message?.providerMeta || {}) as Record<string, unknown>) };
  for (const key of TRANSIENT_STREAM_META_KEYS) delete providerMeta[key];
  return providerMeta;
}

export function assistantMessageHasCanonicalVisibleContent(message?: ChatMessage | null): boolean {
  if (!message) return false;
  const providerMeta = (message.providerMeta || {}) as Record<string, unknown>;
  if (providerMeta.planCard && typeof providerMeta.planCard === "object") return true;
  return messageHasVisibleContent({
    ...message,
    providerMeta: providerMetaWithoutTransientStreamState(message),
  });
}

export function isLocalOwnUserMessage(message?: ChatMessage | null): boolean {
  if (!message || message.role !== "user") return false;
  const meta = (message.providerMeta || {}) as Record<string, unknown>;
  const origin = meta.origin as Record<string, unknown> | undefined;
  if (origin && origin.kind === "remote_im") return false;
  const speakerAgentId = normalized(
    message.speakerAgentId || meta.speakerAgentId || meta.speaker_agent_id,
  );
  return !speakerAgentId || speakerAgentId === "user-persona";
}

export function isOptimisticOwnUserDraft(message?: ChatMessage | null): boolean {
  if (!isLocalOwnUserMessage(message)) return false;
  const messageId = normalized(message?.id);
  const meta = (message?.providerMeta || {}) as Record<string, unknown>;
  return messageId.startsWith("__draft_user__:") || meta._optimistic === true;
}

function historyMessageKind(message?: ChatMessage | null): string {
  const providerMeta = (message?.providerMeta || {}) as Record<string, unknown>;
  const messageMeta = (
    providerMeta.message_meta
    || providerMeta.messageMeta
    || {}
  ) as Record<string, unknown>;
  return normalized(messageMeta.kind || providerMeta.messageKind);
}

function messageCreatedAtMs(message?: ChatMessage | null): number | null {
  const raw = normalized(message?.createdAt);
  if (!raw) return null;
  const value = Date.parse(raw);
  return Number.isFinite(value) ? value : null;
}

function insertMessageIntoTimeline(messages: ChatMessage[], incoming: ChatMessage): ChatMessage[] {
  if (historyMessageKind(incoming) === "summary_context_seed") {
    let insertIndex = 0;
    while (insertIndex < messages.length && historyMessageKind(messages[insertIndex]) === "summary_context_seed") {
      insertIndex += 1;
    }
    return [...messages.slice(0, insertIndex), incoming, ...messages.slice(insertIndex)];
  }
  const incomingAt = messageCreatedAtMs(incoming);
  if (incomingAt === null) return [...messages, incoming];
  const index = messages.findIndex((message) => {
    const existingAt = messageCreatedAtMs(message);
    return existingAt !== null && existingAt > incomingAt;
  });
  return index < 0
    ? [...messages, incoming]
    : [...messages.slice(0, index), incoming, ...messages.slice(index)];
}

function dedupeMessages(messages: ChatMessage[]): ChatMessage[] {
  const seen = new Set<string>();
  const result: ChatMessage[] = [];
  for (const message of messages) {
    const messageId = normalized(message.id);
    if (messageId && seen.has(messageId)) continue;
    if (messageId) seen.add(messageId);
    result.push(message);
  }
  return result;
}

function replaceOptimisticUserDraft(
  messages: ChatMessage[],
  incoming: ChatMessage,
  explicitDraftId = "",
): ChatMessage[] | null {
  if (!isLocalOwnUserMessage(incoming)) return null;
  const normalizedDraftId = normalized(explicitDraftId);
  const draftIndex = messages.findIndex((message) => (
    normalizedDraftId
      ? normalized(message.id) === normalizedDraftId
      : isOptimisticOwnUserDraft(message)
  ));
  if (draftIndex < 0) return null;
  const draft = messages[draftIndex];
  const committedId = normalized(incoming.id);
  const committed = preserveStableRenderId(incoming, draft);
  const next = messages.map((message, index) => index === draftIndex ? committed : message);
  return next.filter((message, index) => (
    index === draftIndex || !committedId || normalized(message.id) !== committedId
  ));
}

export function mergeAuthoritativeConversationMessages(
  messages: ChatMessage[],
  incomingMessages: ChatMessage[],
  options?: AuthoritativeMessageMergeOptions,
): ChatMessage[] {
  let next = dedupeMessages(Array.isArray(messages) ? messages : []);
  let explicitDraftId = normalized(options?.optimisticUserDraftId);
  let prependInsertIndex = 0;
  let summarySeedInsertIndex = 0;
  for (const incoming of Array.isArray(incomingMessages) ? incomingMessages : []) {
    if (!incoming) continue;
    if ((explicitDraftId || options?.replaceOptimisticUserDrafts) && isLocalOwnUserMessage(incoming)) {
      const replaced = replaceOptimisticUserDraft(next, incoming, explicitDraftId);
      explicitDraftId = "";
      if (replaced) {
        next = replaced;
        continue;
      }
    }
    const messageId = normalized(incoming.id);
    const existingIndex = messageId
      ? next.findIndex((message) => normalized(message.id) === messageId)
      : -1;
    if (existingIndex >= 0) {
      const existingMessage = next[existingIndex];
      const existingMeta = (existingMessage.providerMeta || {}) as Record<string, unknown>;
      const replacement = existingMessage.role === "assistant"
        && incoming.role === "assistant"
        && existingMeta._streaming === true
        && !options?.forceReplace
        ? (reconcileCompletedAssistantMessage(existingMessage, incoming) || incoming)
        : reconcileAuthoritativeConversationMessage(
            existingMessage,
            incoming,
            options?.forceReplace ? { forceReplace: true } : undefined,
          );
      next = next.map((message, index) => index === existingIndex ? replacement : message);
      continue;
    }
    if (options?.summarySeedsFirst && historyMessageKind(incoming) === "summary_context_seed") {
      next = [
        ...next.slice(0, summarySeedInsertIndex),
        incoming,
        ...next.slice(summarySeedInsertIndex),
      ];
      summarySeedInsertIndex += 1;
      prependInsertIndex += 1;
    } else if (options?.prependMessages) {
      next = [
        ...next.slice(0, prependInsertIndex),
        incoming,
        ...next.slice(prependInsertIndex),
      ];
      prependInsertIndex += 1;
    } else {
      next = insertMessageIntoTimeline(next, incoming);
    }
  }
  return dedupeMessages(next);
}

export function replaceConversationHistory(
  previousMessages: ChatMessage[],
  incomingMessages: ChatMessage[],
): ChatMessage[] {
  const previousById = new Map<string, ChatMessage>();
  for (const message of previousMessages) {
    const messageId = normalized(message.id);
    if (messageId && !previousById.has(messageId)) previousById.set(messageId, message);
  }
  return dedupeMessages(incomingMessages).map((message) => (
    preserveStableRenderId(message, previousById.get(normalized(message.id)))
  ));
}

function stripStreamingState(message: ChatMessage): ChatMessage {
  return {
    ...message,
    providerMeta: providerMetaWithoutTransientStreamState(message),
  };
}

export function reconcileCompletedAssistantMessage(
  existingMessage: ChatMessage | undefined,
  incomingMessage?: ChatMessage,
): ChatMessage | undefined {
  if (!existingMessage) return incomingMessage;
  const existingMeta = (existingMessage.providerMeta || {}) as Record<string, unknown>;
  const isStreaming = existingMeta._streaming === true;
  if (!incomingMessage) return stripStreamingState(existingMessage);
  if (!isStreaming && assistantMessageHasCanonicalVisibleContent(existingMessage)) {
    return reconcileAuthoritativeConversationMessage(existingMessage, incomingMessage);
  }

  const existingBlocks = assistantContentBlocksFromMessage(existingMessage);
  const incomingBlocks = assistantContentBlocksFromMessage(incomingMessage);
  const contentBlocks = incomingBlocks.length > 0 ? incomingBlocks : existingBlocks;
  // 流式状态归回合所有：正在流式的投影保留其回合状态（`_streaming` 与状态文案/计时），
  // 权威/持久化副本只贡献内容与非瞬态权威字段。只有回合结束路径才显式剥离这些瞬态键。
  const providerMeta: Record<string, unknown> = isStreaming
    ? {
        ...((existingMessage.providerMeta || {}) as Record<string, unknown>),
        ...providerMetaWithoutTransientStreamState(incomingMessage),
      }
    : {
        ...providerMetaWithoutTransientStreamState(existingMessage),
        ...providerMetaWithoutTransientStreamState(incomingMessage),
      };
  const stableRenderId = stableRenderIdFromMessage(existingMessage) || normalized(existingMessage.id);
  return messageWithStableRenderId({
    ...existingMessage,
    ...incomingMessage,
    id: normalized(existingMessage.id) || normalized(incomingMessage.id),
    createdAt: existingMessage.createdAt || incomingMessage.createdAt,
    speakerAgentId: incomingMessage.speakerAgentId || existingMessage.speakerAgentId,
    contentBlocks: contentBlocks.length > 0 ? contentBlocks : incomingMessage.contentBlocks,
    toolCall: incomingMessage.toolCall || existingMessage.toolCall,
    activityItems: incomingMessage.activityItems || existingMessage.activityItems,
    providerMeta,
  }, stableRenderId);
}

function identityValues(activationId?: string, requestId?: string): Set<string> {
  const values = new Set<string>();
  const activation = normalized(activationId);
  const request = normalized(requestId);
  if (activation) values.add(activation);
  if (request) values.add(request);
  return values;
}

function roundIdentityMatches(
  round: ChatMessageRoundProjection,
  input: { assistantMessageId?: string; activationId?: string; requestId?: string },
): boolean {
  if (round.phase === "idle") return false;
  const currentMessageId = normalized(round.assistantMessageId);
  const incomingMessageId = normalized(input.assistantMessageId);
  if (currentMessageId && incomingMessageId && currentMessageId !== incomingMessageId) return false;
  const currentIds = identityValues(round.activationId, round.requestId);
  const incomingIds = identityValues(input.activationId, input.requestId);
  if (currentIds.size === 0 || incomingIds.size === 0) return true;
  return [...incomingIds].some((value) => currentIds.has(value));
}

export function chatMessageRoundMatchesIdentity(
  round: ChatMessageRoundProjection,
  input: { assistantMessageId?: string; activationId?: string; requestId?: string },
): boolean {
  return roundIdentityMatches(round, input);
}

function revisionIsOlder(currentRevision: string, incomingRevision: string): boolean {
  const current = normalized(currentRevision);
  const incoming = normalized(incomingRevision);
  if (!current || !incoming) return false;
  const currentNumber = Number(current);
  const incomingNumber = Number(incoming);
  if (Number.isFinite(currentNumber) && Number.isFinite(incomingNumber)) {
    return incomingNumber < currentNumber;
  }
  const currentAt = Date.parse(current);
  const incomingAt = Date.parse(incoming);
  if (Number.isFinite(currentAt) && Number.isFinite(incomingAt)) return incomingAt < currentAt;
  return incoming < current;
}

function findMessage(messages: ChatMessage[], messageId: string): ChatMessage | undefined {
  const id = normalized(messageId);
  return id ? messages.find((message) => normalized(message.id) === id) : undefined;
}

function replaceMessage(messages: ChatMessage[], messageId: string, nextMessage: ChatMessage): ChatMessage[] {
  const id = normalized(messageId);
  const index = messages.findIndex((message) => normalized(message.id) === id);
  return index < 0
    ? [...messages, nextMessage]
    : messages.map((message, itemIndex) => itemIndex === index ? nextMessage : message);
}

function createStreamingMessage(input: {
  existing?: ChatMessage;
  messageId: string;
  createdAt?: string;
  speakerAgentId?: string;
  statusText?: string;
}): ChatMessage {
  const existing = input.existing;
  const existingMeta = (existing?.providerMeta || {}) as Record<string, unknown>;
  const hasCanonicalContent = assistantMessageHasCanonicalVisibleContent(existing);
  return messageWithStableRenderId({
    ...(existing || {}),
    id: input.messageId,
    role: "assistant",
    createdAt: existing?.createdAt || normalized(input.createdAt) || undefined,
    speakerAgentId: normalized(input.speakerAgentId) || existing?.speakerAgentId || undefined,
    parts: existing?.parts || [{ type: "text", text: "" }],
    contentBlocks: existing?.contentBlocks,
    providerMeta: {
      ...existingMeta,
      _streaming: true,
      _preStreamingStatusText: hasCanonicalContent ? "" : normalized(input.statusText),
      _toolStatusText: normalized(existingMeta._toolStatusText),
      _toolStatusState: normalizeToolStatus(existingMeta._toolStatusState),
    },
  } as ChatMessage, stableRenderIdFromMessage(existing) || input.messageId);
}

function reduceRoundStarted(
  state: ChatMessageState,
  event: Extract<ChatMessageEvent, { type: "round_started" }>,
): ChatMessageState {
  const messageId = normalized(event.assistantMessageId);
  if (!messageId) return state;
  let current = state;
  if (
    current.round.phase === "settling"
    && normalized(current.round.assistantMessageId) !== messageId
  ) {
    const previousMessageId = current.round.assistantMessageId;
    const previousMessage = findMessage(current.messages, previousMessageId);
    const messages = previousMessage && assistantMessageHasCanonicalVisibleContent(previousMessage)
      ? replaceMessage(current.messages, previousMessageId, stripStreamingState(previousMessage))
      : current.messages.filter((message) => normalized(message.id) !== normalized(previousMessageId));
    current = { ...current, messages, round: emptyRound(), error: "" };
  }
  if (
    current.round.phase === "settling"
    && normalized(current.round.assistantMessageId) === messageId
  ) {
    return current;
  }
  if (current.round.phase !== "idle" && !roundIdentityMatches(current.round, event)) return current;
  const existing = findMessage(current.messages, messageId);
  if (existing && !((existing.providerMeta || {}) as Record<string, unknown>)._streaming
    && assistantMessageHasCanonicalVisibleContent(existing)) {
    return current;
  }
  const message = createStreamingMessage({
    existing,
    messageId,
    createdAt: event.startedAt,
    speakerAgentId: event.speakerAgentId,
    statusText: event.statusText,
  });
  return {
    ...current,
    messages: replaceMessage(current.messages, messageId, message),
    round: {
      phase: current.round.phase === "streaming" ? "streaming" : (event.phase || "waiting"),
      assistantMessageId: messageId,
      activationId: normalized(event.activationId || event.requestId || current.round.activationId),
      requestId: normalized(event.requestId || event.activationId || current.round.requestId),
      revision: normalized(event.revision || current.round.revision),
      startedAtMs: positiveInteger(event.startedAtMs || current.round.startedAtMs),
    },
    error: "",
  };
}

function snapshotHasVisibleProgress(snapshot: ChatMessageStreamSnapshot, blocks: unknown): boolean {
  return !!(
    normalized(snapshot.assistantText)
    || normalized(snapshot.toolStatusText)
    || normalized(snapshot.toolStatusState)
    || normalizeAssistantStreamBlocks(blocks).length > 0
  );
}

function reduceStreamSnapshot(
  state: ChatMessageState,
  event: Extract<ChatMessageEvent, { type: "assistant_stream_snapshot" }>,
): ChatMessageState {
  if (state.round.phase === "settling") return state;
  const snapshot = event.snapshot;
  const messageId = normalized(
    snapshot.persistedAssistantMessageId
    || event.assistantMessageId
    || state.round.assistantMessageId,
  );
  if (!messageId) return state;
  const identity = {
    assistantMessageId: messageId,
    activationId: snapshot.activationId,
    requestId: snapshot.requestId,
  };
  let current = state;
  const startedFromIdle = current.round.phase === "idle";
  if (current.round.phase === "idle") {
    current = reduceRoundStarted(current, {
      type: "round_started",
      conversationId: event.conversationId,
      assistantMessageId: messageId,
      activationId: snapshot.activationId,
      requestId: snapshot.requestId,
      revision: snapshot.updatedAt,
      startedAt: snapshot.startedAt,
      startedAtMs: snapshot.startedAtMs,
      speakerAgentId: snapshot.speakerAgentId || snapshot.agentId,
      phase: "waiting",
    });
    if (current.round.phase === "idle") return state;
  } else if (!roundIdentityMatches(current.round, identity)) {
    return state;
  }
  const incomingRevision = normalized(snapshot.updatedAt);
  if (!startedFromIdle && revisionIsOlder(current.round.revision, incomingRevision)) return current;
  const existing = findMessage(current.messages, messageId);
  if (!existing) return current;
  if (
    !startedFromIdle
    && ((existing.providerMeta || {}) as Record<string, unknown>)._streaming !== true
  ) {
    return current;
  }
  const existingBlocks = assistantContentBlocksFromMessage(existing);
  let blocks = normalizeAssistantStreamBlocks(snapshot.streamBlocks);
  if (blocks.length === 0 && normalized(snapshot.assistantText)) {
    blocks = appendTextDeltaToStreamBlocks([], String(snapshot.assistantText || ""));
  }
  if (blocks.length === 0 && existingBlocks.length > 0) blocks = existingBlocks;
  const meta = (existing.providerMeta || {}) as Record<string, unknown>;
  // 内容真相统一收敛在 contentBlocks；_streamSegments/_streamTail/_streamAnimatedDelta
  // 是它的可派生投影，不再写入。分段由渲染层全量重算（乐观渲染 chunks=[全量文本]）。
  // _toolStatusText/_toolStatusState/_preStreamingStatusText 承载调度阶段提示
  // （准备调度/等待回应等），来自后端 streamCache，不是 contentBlocks 的拷贝。
  const toolStatusText = typeof snapshot.toolStatusText === "string"
    ? String(snapshot.toolStatusText || "")
    : String(meta._toolStatusText || "");
  const toolStatusState = typeof snapshot.toolStatusState === "string"
    ? normalizeToolStatus(snapshot.toolStatusState)
    : normalizeToolStatus(meta._toolStatusState);
  const preStreamingStatusText = blocks.length > 0
    ? ""
    : (Object.prototype.hasOwnProperty.call(snapshot, "preStreamingStatusText")
      ? String(snapshot.preStreamingStatusText || "")
      : String(meta._preStreamingStatusText || ""));
  const schedulingState = typeof snapshot.schedulingState === "string"
    ? String(snapshot.schedulingState || "").trim()
    : String((meta as Record<string, unknown>)._schedulingState || "").trim();
  const contextUsagePercent = typeof snapshot.contextUsagePercent === "number"
    ? snapshot.contextUsagePercent
    : (typeof (meta as Record<string, unknown>)._contextUsagePercent === "number"
      ? (meta as Record<string, unknown>)._contextUsagePercent as number
      : undefined);
  const nextMessage = preserveStableRenderId({
    ...existing,
    speakerAgentId: normalized(snapshot.speakerAgentId || snapshot.agentId) || existing.speakerAgentId,
    contentBlocks: blocks,
    providerMeta: {
      ...meta,
      _streaming: true,
      _preStreamingStatusText: preStreamingStatusText,
      _toolStatusText: toolStatusText,
      _toolStatusState: toolStatusState,
      _schedulingState: schedulingState,
      _contextUsagePercent: contextUsagePercent,
      _frontendDispatchStartedAtMs: positiveInteger(
        snapshot.frontendDispatchStartedAtMs || meta._frontendDispatchStartedAtMs,
      ),
      _frontendDispatchElapsedMs: positiveInteger(
        snapshot.frontendDispatchElapsedMs || meta._frontendDispatchElapsedMs,
      ),
    },
  }, existing);
  return {
    ...current,
    messages: replaceMessage(current.messages, messageId, nextMessage),
    round: {
      phase: snapshotHasVisibleProgress(snapshot, blocks) ? "streaming" : "waiting",
      assistantMessageId: messageId,
      activationId: normalized(snapshot.activationId || snapshot.requestId || current.round.activationId),
      requestId: normalized(snapshot.requestId || snapshot.activationId || current.round.requestId),
      revision: incomingRevision || current.round.revision,
      startedAtMs: positiveInteger(snapshot.startedAtMs || current.round.startedAtMs),
    },
  };
}

function reduceAssistantDelta(
  state: ChatMessageState,
  event: Extract<ChatMessageEvent, { type: "assistant_delta" }>,
): ChatMessageState {
  if (state.round.phase === "settling") return state;
  const deltaEvent = event.event;
  // Context usage is a coordination/preview signal, not message content.
  // Some transports broadcast it through the assistant-delta channel; it
  // must not promote a waiting round or mutate the assistant bubble.
  if (normalized(deltaEvent.kind) === "context_usage_update") return state;
  if (deltaEvent.streamCache) {
    const snapshotState = reduceStreamSnapshot(state, {
      type: "assistant_stream_snapshot",
      conversationId: event.conversationId,
      assistantMessageId: deltaEvent.assistantMessageId,
      snapshot: {
        ...deltaEvent.streamCache,
        activationId: deltaEvent.streamCache.activationId || deltaEvent.activationId,
        requestId: deltaEvent.streamCache.requestId || deltaEvent.requestId,
        updatedAt: deltaEvent.streamCache.updatedAt || deltaEvent.revision,
      },
    });
    // streamCache 是正文/活动的权威快照，但旧后端与部分通知不会把本次
    // tool_status 同步写入快照。状态事件仍须覆盖到同一条消息，否则
    // reducer 随后的显示同步会把刚收到的工具/重试状态清空。
    if (normalized(deltaEvent.kind) === "tool_status") {
      return reduceAssistantDelta(snapshotState, {
        ...event,
        event: { ...deltaEvent, streamCache: undefined },
      });
    }
    return snapshotState;
  }
  const messageId = normalized(deltaEvent.assistantMessageId || state.round.assistantMessageId);
  if (!messageId || !roundIdentityMatches(state.round, {
    assistantMessageId: messageId,
    activationId: deltaEvent.activationId,
    requestId: deltaEvent.requestId,
  })) return state;
  const existing = findMessage(state.messages, messageId);
  if (!existing || !((existing.providerMeta || {}) as Record<string, unknown>)._streaming) return state;
  const kind = normalized(deltaEvent.kind);
  const delta = String(deltaEvent.delta || "");
  const meta = (existing.providerMeta || {}) as Record<string, unknown>;
  let blocks = assistantContentBlocksFromMessage(existing);
  let toolStatusText = normalized(meta._toolStatusText);
  let toolStatusState = normalizeToolStatus(meta._toolStatusState);
  let schedulingState = String((meta as Record<string, unknown>)._schedulingState || "").trim();

  if (kind === "tool_status") {
    toolStatusText = String(deltaEvent.message || "");
    toolStatusState = normalizeToolStatus(deltaEvent.toolStatus);
    const msg = toolStatusText;
    if (toolStatusState === "running") {
      if (msg.includes("正在调用工具")) schedulingState = "executing_tool";
      else if (msg.includes("正在进入模型请求") || msg.includes("等待回应") || msg.includes("等待响应")) schedulingState = "waiting_response";
      else if (msg.includes("正在准备调度") || msg.includes("正在处理附件") || msg.includes("上下文")) schedulingState = "preparing_context";
    }
  } else if (kind === "assistant_tool_event") {
    blocks = applyAssistantToolEventToStreamBlocks(blocks, deltaEvent.message);
    schedulingState = "streaming_tool";
  } else if (kind === "assistant_tool_result") {
    blocks = applyAssistantToolResultToStreamBlocks(blocks, deltaEvent.message);
    schedulingState = "executing_tool";
  } else if (kind === "activity_reasoning_delta") {
    if (delta) blocks = appendReasoningDeltaToStreamBlocks(blocks, delta);
    if (delta) schedulingState = "streaming_reasoning";
  } else if (delta) {
    blocks = appendTextDeltaToStreamBlocks(blocks, delta);
    if (delta.trim()) schedulingState = "streaming_text";
  }

  const hasVisibleContent = blocks.length > 0 || !!assistantTextFromStreamBlocks(blocks).trim();
  const nextMessage = preserveStableRenderId({
    ...existing,
    contentBlocks: blocks,
    providerMeta: {
      ...meta,
      _streaming: true,
      _preStreamingStatusText: hasVisibleContent ? "" : normalized(meta._preStreamingStatusText),
      _toolStatusText: toolStatusText,
      _toolStatusState: toolStatusState,
      _schedulingState: schedulingState,
    },
  }, existing);
  return {
    ...state,
    messages: replaceMessage(state.messages, messageId, nextMessage),
    round: {
      ...state.round,
      phase: "streaming",
      activationId: normalized(deltaEvent.activationId || deltaEvent.requestId || state.round.activationId),
      requestId: normalized(deltaEvent.requestId || deltaEvent.activationId || state.round.requestId),
      revision: normalized(deltaEvent.revision || state.round.revision),
    },
  };
}

function reduceRoundFinished(
  state: ChatMessageState,
  event: Extract<ChatMessageEvent, { type: "round_finished" }>,
): ChatMessageState {
  const incomingFormalMessageId = normalized(event.assistantMessage?.id);
  const activeMessageId = normalized(state.round.assistantMessageId);
  if (
    state.round.phase !== "idle"
    && incomingFormalMessageId
    && activeMessageId
    && incomingFormalMessageId !== activeMessageId
  ) {
    return state;
  }
  const messageId = normalized(
    event.assistantMessageId
    || incomingFormalMessageId
    || state.round.assistantMessageId,
  );
  const matchesActiveRound = roundIdentityMatches(state.round, {
    assistantMessageId: messageId,
    activationId: event.activationId,
    requestId: event.requestId,
  });
  // A terminal event carrying a different formal message must never be
  // allowed to append an old round while another round is active. The same
  // guard also rejects an explicitly stale activation/request identity.
  if (state.round.phase !== "idle" && !matchesActiveRound) return state;
  let messages = state.messages;
  if (event.assistantMessage) {
    messages = mergeAuthoritativeConversationMessages(messages, [event.assistantMessage]);
  }
  if (!matchesActiveRound) {
    // 机器回合已 idle 时，终态合并也必须剥离流式标记：此时没有任何活动回合会再清除它。
    if (state.round.phase === "idle" && event.assistantMessage) {
      const terminalMessageId = normalized(event.assistantMessage.id);
      const terminalMessage = findMessage(messages, terminalMessageId);
      if (terminalMessage
        && ((terminalMessage.providerMeta || {}) as Record<string, unknown>)._streaming === true) {
        messages = replaceMessage(messages, terminalMessageId, stripStreamingState(terminalMessage));
      }
    }
    return messages === state.messages ? state : { ...state, messages };
  }

  const existing = findMessage(messages, messageId);
  if (event.assistantMessage || assistantMessageHasCanonicalVisibleContent(existing)) {
    // 回合结束：合并不再隐式清除流式状态，这里显式剥离，避免气泡挂死。
    if (existing && (!event.assistantMessage
      || ((existing.providerMeta || {}) as Record<string, unknown>)._streaming === true)) {
      messages = replaceMessage(messages, messageId, stripStreamingState(existing));
    }
    return { ...state, messages, round: emptyRound(), error: "" };
  }
  return {
    ...state,
    messages,
    round: { ...state.round, phase: "settling", assistantMessageId: messageId },
    error: "",
  };
}

function reduceRoundFailed(
  state: ChatMessageState,
  event: Extract<ChatMessageEvent, { type: "round_failed" }>,
): ChatMessageState {
  const messageId = normalized(event.assistantMessageId || state.round.assistantMessageId);
  if (!roundIdentityMatches(state.round, {
    assistantMessageId: messageId,
    activationId: event.activationId,
    requestId: event.requestId,
  })) return state;
  const existing = findMessage(state.messages, messageId);
  let messages = state.messages;
  if (existing && assistantMessageHasCanonicalVisibleContent(existing)) {
    messages = replaceMessage(messages, messageId, stripStreamingState(existing));
  } else if (messageId) {
    messages = messages.filter((message) => normalized(message.id) !== messageId);
  }
  return {
    ...state,
    messages,
    round: emptyRound(),
    error: normalized((event.error as { message?: unknown } | null)?.message || event.error),
  };
}

export function createChatMessageState(
  conversationId: string,
  messages: ChatMessage[] = [],
): ChatMessageState {
  return {
    conversationId: normalized(conversationId),
    messages: dedupeMessages(messages),
    round: emptyRound(),
    error: "",
  };
}

export function reduceChatMessageState(
  state: ChatMessageState,
  event: ChatMessageEvent,
): ChatMessageState {
  if (!sameConversation(state, event.conversationId)) return state;
  if (event.type === "history_replaced") {
    return {
      ...state,
      messages: replaceConversationHistory(state.messages, event.messages),
      round: emptyRound(),
      error: "",
    };
  }
  if (event.type === "authoritative_messages_merged") {
    let messages = mergeAuthoritativeConversationMessages(state.messages, event.messages, event.options);
    const settledMessageId = normalized(state.round.assistantMessageId);
    const settledMessage = settledMessageId
      ? messages.find((message) => normalized(message.id) === settledMessageId)
      : undefined;
    const settled = state.round.phase === "settling"
      && !!settledMessage
      && assistantMessageHasCanonicalVisibleContent(settledMessage);
    if (settled && settledMessage
      && ((settledMessage.providerMeta || {}) as Record<string, unknown>)._streaming === true) {
      // settling 收尾即回合结束：显式剥离流式状态，避免气泡挂死。
      messages = replaceMessage(messages, settledMessageId, stripStreamingState(settledMessage));
    }
    return {
      ...state,
      messages,
      round: settled ? emptyRound() : state.round,
      error: settled ? "" : state.error,
    };
  }
  if (event.type === "round_started") return reduceRoundStarted(state, event);
  if (event.type === "assistant_stream_snapshot") return reduceStreamSnapshot(state, event);
  if (event.type === "assistant_delta") return reduceAssistantDelta(state, event);
  if (event.type === "round_finished") return reduceRoundFinished(state, event);
  if (event.type === "round_failed") return reduceRoundFailed(state, event);

  const messageId = state.round.assistantMessageId;
  const existing = findMessage(state.messages, messageId);
  let messages = state.messages;
  if (existing && event.preserveVisibleContent !== false && assistantMessageHasCanonicalVisibleContent(existing)) {
    messages = replaceMessage(messages, messageId, stripStreamingState(existing));
  } else if (messageId) {
    messages = messages.filter((message) => normalized(message.id) !== messageId);
  }
  return { ...state, messages, round: emptyRound(), error: "" };
}

export function activeAssistantMessage(state: ChatMessageState): ChatMessage | undefined {
  return findMessage(state.messages, state.round.assistantMessageId);
}
