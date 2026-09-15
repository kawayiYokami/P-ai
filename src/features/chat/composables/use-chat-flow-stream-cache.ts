import type { AssistantStreamBlock } from "../../../types/app";
import {
  applyAssistantToolEventToStreamBlocks,
  applyAssistantToolResultToStreamBlocks,
  appendReasoningDeltaToStreamBlocks,
  appendTextDeltaToStreamBlocks,
  normalizeAssistantStreamBlocks,
} from "../../../utils/chat-message-semantics";
import {
  readDeltaMessage,
  type AssistantDeltaEvent,
} from "./use-chat-flow-events";
import {
  normalizeConversationId,
  positiveRoundedNumber,
} from "./use-chat-flow-utils";

export type ConversationStreamCache = {
  activationId?: string;
  requestId?: string;
  speakerAgentId?: string;
  startedAt?: string;
  startedAtMs?: number;
  updatedAt?: string;
  frontendDispatchStartedAtMs?: number;
  frontendDispatchElapsedMs?: number;
  assistantText: string;
  toolStatusText: string;
  toolStatusState: "running" | "done" | "failed" | "";
  streamBlocks: AssistantStreamBlock[];
  persistedAssistantMessageId?: string;
};

export type ConversationRuntimeStreamCacheSnapshot = {
  activationId?: string;
  requestId?: string;
  agentId?: string;
  speakerAgentId?: string;
  startedAt?: string;
  startedAtMs?: number;
  updatedAt?: string;
  frontendDispatchStartedAtMs?: number;
  frontendDispatchElapsedMs?: number;
  assistantText?: string;
  toolStatusText?: string;
  toolStatusState?: "running" | "done" | "failed" | "" | string;
  streamBlocks?: AssistantStreamBlock[];
  hasVisibleProgress?: boolean;
  persistedAssistantMessageId?: string;
  contextUsageRatio?: number;
  contextUsagePercent?: number;
  effectivePromptTokens?: number;
  contextWindowTokens?: number;
  schedulingState?: string;
};

type UseChatFlowStreamCacheOptions = {
  getConversationId?: () => string;
  getCurrentDisplayState?: () => {
    assistantText: string;
    toolStatusText: string;
    toolStatusState: "running" | "done" | "failed" | "";
    streamBlocks: AssistantStreamBlock[];
  } | null;
  getActiveActivationId: () => string;
  getFrontendDispatchStartedAtMs: () => number;
  getFrontendDispatchElapsedMs: () => number;
  currentFrontendDispatchElapsedMs: () => number;
  restoreFrontendDispatchTimerFromCache: (cache: ConversationStreamCache) => void;
  setActiveRoundAgentId?: (value: string) => void;
};

function normalizeToolStatusState(value: unknown): "running" | "done" | "failed" | "" {
  const status = String(value || "").trim();
  return status === "running" || status === "done" || status === "failed" ? status : "";
}

export function streamCacheHasVisibleProgress(
  cache?: ConversationRuntimeStreamCacheSnapshot | ConversationStreamCache | null,
): boolean {
  if (!cache) return false;
  return !!(
    String(cache.assistantText || "").trim()
    || String(cache.toolStatusText || "").trim()
    || String(cache.toolStatusState || "").trim()
    || (Array.isArray(cache.streamBlocks) && cache.streamBlocks.length > 0)
  );
}

function emptyConversationStreamCache(): ConversationStreamCache {
  return {
    activationId: "",
    requestId: "",
    speakerAgentId: "",
    startedAt: "",
    startedAtMs: 0,
    updatedAt: "",
    frontendDispatchStartedAtMs: 0,
    frontendDispatchElapsedMs: 0,
    assistantText: "",
    toolStatusText: "",
    toolStatusState: "",
    streamBlocks: [],
    persistedAssistantMessageId: "",
  };
}

export function useChatFlowStreamCache(options: UseChatFlowStreamCacheOptions) {
  const conversationStreamCache = new Map<string, ConversationStreamCache>();

  function readConversationStreamCache(conversationId?: string | null): ConversationStreamCache | null {
    const cid = normalizeConversationId(conversationId);
    if (!cid) return null;
    const cache = conversationStreamCache.get(cid);
    if (!cache) return null;
    return {
      activationId: String(cache.activationId || "").trim(),
      requestId: String(cache.requestId || "").trim(),
      speakerAgentId: String(cache.speakerAgentId || "").trim(),
      startedAt: String(cache.startedAt || "").trim(),
      startedAtMs: positiveRoundedNumber(cache.startedAtMs),
      updatedAt: String(cache.updatedAt || "").trim(),
      frontendDispatchStartedAtMs: positiveRoundedNumber(cache.frontendDispatchStartedAtMs),
      frontendDispatchElapsedMs: positiveRoundedNumber(cache.frontendDispatchElapsedMs),
      assistantText: cache.assistantText,
      toolStatusText: cache.toolStatusText,
      toolStatusState: cache.toolStatusState,
      streamBlocks: normalizeAssistantStreamBlocks(cache.streamBlocks),
      persistedAssistantMessageId: String(cache.persistedAssistantMessageId || "").trim(),
    };
  }

  function writeConversationStreamCache(
    conversationId: string,
    updater: (current: ConversationStreamCache) => ConversationStreamCache,
  ) {
    const cid = normalizeConversationId(conversationId);
    if (!cid) return;
    const next = updater(readConversationStreamCache(cid) || emptyConversationStreamCache());
    conversationStreamCache.set(cid, {
      ...next,
      activationId: String(next.activationId || "").trim(),
      requestId: String(next.requestId || "").trim(),
      speakerAgentId: String(next.speakerAgentId || "").trim(),
      startedAt: String(next.startedAt || "").trim(),
      startedAtMs: positiveRoundedNumber(next.startedAtMs),
      updatedAt: String(next.updatedAt || "").trim(),
      frontendDispatchStartedAtMs: positiveRoundedNumber(next.frontendDispatchStartedAtMs),
      frontendDispatchElapsedMs: positiveRoundedNumber(next.frontendDispatchElapsedMs),
      streamBlocks: normalizeAssistantStreamBlocks(next.streamBlocks),
      persistedAssistantMessageId: String(next.persistedAssistantMessageId || "").trim(),
    });
  }

  function clearConversationStreamCache(conversationId?: string | null) {
    const cid = normalizeConversationId(conversationId);
    if (!cid) return;
    conversationStreamCache.delete(cid);
  }

  function syncCurrentDisplayStateToConversationStreamCache(conversationId?: string | null) {
    const cid = normalizeConversationId(conversationId || (options.getConversationId ? options.getConversationId() : ""));
    if (!cid) return;
    const activeActivationId = options.getActiveActivationId();
    const display = options.getCurrentDisplayState ? options.getCurrentDisplayState() : null;
    writeConversationStreamCache(cid, (current) => ({
      assistantText: String(display?.assistantText || current.assistantText || ""),
      activationId: activeActivationId,
      requestId: activeActivationId,
      speakerAgentId: current.speakerAgentId,
      startedAt: current.startedAt,
      startedAtMs: current.startedAtMs,
      updatedAt: current.updatedAt,
      frontendDispatchStartedAtMs: options.getFrontendDispatchStartedAtMs(),
      frontendDispatchElapsedMs: options.currentFrontendDispatchElapsedMs(),
      toolStatusText: String(display?.toolStatusText || current.toolStatusText || ""),
      toolStatusState: display ? display.toolStatusState : current.toolStatusState,
      streamBlocks: display
        ? normalizeAssistantStreamBlocks(display.streamBlocks)
        : normalizeAssistantStreamBlocks(current.streamBlocks),
      persistedAssistantMessageId: current.persistedAssistantMessageId,
    }));
  }

  function applyConversationStreamCacheToDisplay(
    conversationId?: string | null,
    input?: { ignoreActivationId?: boolean; skipStreamBlocks?: boolean },
  ): boolean {
    const cache = readConversationStreamCache(conversationId);
    if (!cache) return false;
    const activeActivationId = options.getActiveActivationId();
    if (!input?.ignoreActivationId && activeActivationId && cache.activationId && cache.activationId !== activeActivationId) {
      return false;
    }
    if (cache.speakerAgentId) {
      options.setActiveRoundAgentId?.(cache.speakerAgentId);
    }
    options.restoreFrontendDispatchTimerFromCache(cache);
    return true;
  }

  function writeConversationStreamCacheSnapshot(
    conversationId: string,
    snapshot?: ConversationRuntimeStreamCacheSnapshot | null,
  ) {
    const cid = normalizeConversationId(conversationId);
    if (!cid || !snapshot) return;
    const snapshotSpeakerAgentId = String(snapshot.speakerAgentId || snapshot.agentId || "").trim();
    writeConversationStreamCache(cid, (current) => ({
      activationId: String(snapshot.activationId || snapshot.requestId || current.activationId || "").trim(),
      requestId: String(snapshot.requestId || snapshot.activationId || current.requestId || "").trim(),
      speakerAgentId: String(snapshotSpeakerAgentId || current.speakerAgentId || "").trim(),
      startedAt: String(snapshot.startedAt || current.startedAt || "").trim(),
      startedAtMs: positiveRoundedNumber(snapshot.startedAtMs || current.startedAtMs),
      updatedAt: String(snapshot.updatedAt || current.updatedAt || "").trim(),
      frontendDispatchStartedAtMs: positiveRoundedNumber(snapshot.startedAtMs || snapshot.frontendDispatchStartedAtMs || current.frontendDispatchStartedAtMs),
      frontendDispatchElapsedMs: positiveRoundedNumber(snapshot.frontendDispatchElapsedMs || current.frontendDispatchElapsedMs),
      assistantText: String(snapshot.assistantText || ""),
      toolStatusText: String(snapshot.toolStatusText || ""),
      toolStatusState: normalizeToolStatusState(snapshot.toolStatusState),
      streamBlocks: normalizeAssistantStreamBlocks(snapshot.streamBlocks).length > 0
        ? normalizeAssistantStreamBlocks(snapshot.streamBlocks)
        : normalizeAssistantStreamBlocks(current.streamBlocks),
      persistedAssistantMessageId: String(snapshot.persistedAssistantMessageId || current.persistedAssistantMessageId || "").trim(),
    }));
  }

  function applyConversationStreamCacheSnapshotToDisplay(
    conversationId: string,
    snapshot?: ConversationRuntimeStreamCacheSnapshot | null,
    input?: { ignoreActivationId?: boolean },
  ): boolean {
    writeConversationStreamCacheSnapshot(conversationId, snapshot);
    return applyConversationStreamCacheToDisplay(conversationId, input);
  }

  function applyAssistantEventToConversationStreamCache(
    conversationId: string,
    parsed: AssistantDeltaEvent,
  ): boolean {
    const cid = normalizeConversationId(conversationId);
    if (!cid) return false;
    let changed = false;
    writeConversationStreamCache(cid, (current) => {
      const activeActivationId = options.getActiveActivationId();
      const eventStreamCache = parsed.streamCache;
      const eventSpeakerAgentId = String(eventStreamCache?.speakerAgentId || eventStreamCache?.agentId || "").trim();
      const next: ConversationStreamCache = {
        ...current,
        activationId: String(parsed.activationId || parsed.requestId || current.activationId || activeActivationId || "").trim(),
        requestId: String(parsed.requestId || parsed.activationId || current.requestId || activeActivationId || "").trim(),
        speakerAgentId: String(eventSpeakerAgentId || current.speakerAgentId || "").trim(),
        startedAt: current.startedAt,
        startedAtMs: current.startedAtMs,
        updatedAt: String(eventStreamCache?.updatedAt || current.updatedAt || "").trim(),
        frontendDispatchStartedAtMs: options.getFrontendDispatchStartedAtMs(),
        frontendDispatchElapsedMs: options.currentFrontendDispatchElapsedMs(),
        streamBlocks: normalizeAssistantStreamBlocks(current.streamBlocks),
      };
      const delta = readDeltaMessage(parsed);
      if (parsed.kind === "tool_status") {
        next.toolStatusText = parsed.message || "";
        next.toolStatusState =
          parsed.toolStatus === "running" || parsed.toolStatus === "done" || parsed.toolStatus === "failed"
            ? parsed.toolStatus : "";
        changed = true;
        return next;
      }
      if (parsed.kind === "assistant_tool_event") {
        next.streamBlocks = applyAssistantToolEventToStreamBlocks(next.streamBlocks, parsed.message);
        changed = true;
        return next;
      }
      if (parsed.kind === "assistant_tool_result") {
        next.streamBlocks = applyAssistantToolResultToStreamBlocks(next.streamBlocks, parsed.message);
        changed = true;
        return next;
      }
      if (parsed.kind === "activity_reasoning_delta" && delta) {
        next.streamBlocks = appendReasoningDeltaToStreamBlocks(next.streamBlocks, delta);
        changed = true;
        return next;
      }
      if (delta) {
        next.assistantText += delta;
        next.streamBlocks = appendTextDeltaToStreamBlocks(next.streamBlocks, delta);
        changed = true;
      }
      return next;
    });
    return changed;
  }

  return {
    applyAssistantEventToConversationStreamCache,
    applyConversationStreamCacheSnapshotToDisplay,
    applyConversationStreamCacheToDisplay,
    clearConversationStreamCache,
    readConversationStreamCache,
    syncCurrentDisplayStateToConversationStreamCache,
    writeConversationStreamCacheSnapshot,
  };
}
