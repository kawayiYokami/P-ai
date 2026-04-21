import { Channel } from "@tauri-apps/api/core";
import { ref, type Ref } from "vue";
import type { ChatMentionTarget, ChatMessage, PromptCommandPreset } from "../../../types/app";

// ---------------------------------------------------------------------------
// 1. 类型声明
// ---------------------------------------------------------------------------

export type AssistantDeltaEvent = {
  delta?: string;
  kind?: string;
  requestId?: string;
  phaseId?: string;
  reason?: string;
  toolName?: string;
  toolStatus?: string;
  toolArgs?: string;
  message?: string;
};

type HistoryFlushedPayload = {
  conversationId: string;
  messageCount: number;
  messages: ChatMessage[];
  activateAssistant?: boolean;
  compactionApplied?: boolean;
};

type RoundCompletedPayload = {
  conversationId: string;
  assistantText: string;
  reasoningStandard?: string;
  reasoningInline?: string;
  archivedBeforeSend?: boolean;
  assistantMessage?: ChatMessage;
};

type RoundFailedPayload = {
  conversationId?: string;
  error: string;
};

export type FrontendRoundPhase = "idle" | "queued" | "waiting" | "streaming";

type UseChatFlowOptions = {
  chatting: Ref<boolean>;
  forcingArchive: Ref<boolean>;
  getSession: () => { apiConfigId: string; agentId: string; departmentId?: string } | null;
  getConversationId?: () => string;
  chatInput: Ref<string>;
  selectedMentions?: Ref<ChatMentionTarget[]>;
  selectedInstructionPrompts?: Ref<PromptCommandPreset[]>;
  clipboardImages: Ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>;
  queuedAttachmentNotices?: Ref<Array<{ id: string; fileName: string; relativePath: string; mime: string }>>;
  latestUserText: Ref<string>;
  latestUserImages: Ref<Array<{ mime: string; bytesBase64: string }>>;
  latestAssistantText: Ref<string>;
  latestReasoningStandardText: Ref<string>;
  latestReasoningInlineText: Ref<string>;
  toolStatusText: Ref<string>;
  toolStatusState: Ref<"running" | "done" | "failed" | "">;
  streamToolCalls?: Ref<Array<{ name: string; argsText: string; status?: "doing" | "done" }>>;
  chatErrorText: Ref<string>;
  allMessages: Ref<ChatMessage[]>;
  onOwnUserDraftInserted?: () => void;
  t: (key: string, params?: Record<string, unknown>) => string;
  formatRequestFailed: (error: unknown) => string;
  removeBinaryPlaceholders: (text: string) => string;
  invokeSendChatMessage: (input: {
    text: string;
    displayText?: string;
    images: Array<{ mime: string; bytesBase64: string; savedPath?: string }>;
    attachments?: Array<{ fileName: string; relativePath: string; mime: string }>;
    extraTextBlocks?: string[];
    mentions?: ChatMentionTarget[];
    session: { apiConfigId: string; agentId: string; departmentId?: string; conversationId?: string };
    onDelta: Channel<AssistantDeltaEvent>;
  }) => Promise<{
    assistantText: string;
    latestUserText: string;
    reasoningStandard?: string;
    reasoningInline?: string;
    archivedBeforeSend: boolean;
    assistantMessage?: ChatMessage;
  }>;
  invokeStopChatMessage?: (input: {
    session: { apiConfigId: string; agentId: string; departmentId?: string; conversationId?: string };
    partialAssistantText: string;
    partialReasoningStandard: string;
    partialReasoningInline: string;
  }) => Promise<{
    aborted: boolean;
    persisted: boolean;
    conversationId?: string | null;
    assistantText?: string;
    reasoningStandard?: string;
    reasoningInline?: string;
    assistantMessage?: ChatMessage;
  }>;
  invokeBindActiveChatViewStream?: (input: {
    conversationId?: string;
    onDelta: Channel<AssistantDeltaEvent>;
  }) => Promise<void>;
  onReloadMessages: () => Promise<void>;
  onHistoryFlushed?: (input: {
    conversationId: string;
    messageCount: number;
    pendingMessages: ChatMessage[];
    activateAssistant: boolean;
  }) => Promise<void>;
};

// ---------------------------------------------------------------------------
// 2. 常量
// ---------------------------------------------------------------------------

const DRAFT_ASSISTANT_ID_PREFIX = "__draft_assistant__:";
const DRAFT_USER_ID_PREFIX = "__draft_user__:";
// ---------------------------------------------------------------------------
// 3. 状态机
//
//   idle ──sendChat()──→ queued
//   queued ──history_flushed──→ streaming（清屏 + reload + 插 draft）
//   queued ──promise settled(无 history_flushed)──→ idle
//   streaming ──round_completed──→ idle
//   streaming ──stopChat()──→ idle
//
//   核心不变量：history_flushed 之后只允许更新 draft 气泡文字，
//   不对 allMessages 做任何其他读写。
// ---------------------------------------------------------------------------

type RoundState =
  | { phase: "idle" }
  | { phase: "queued"; gen: number }
  | { phase: "streaming"; gen: number; draftId: string };

type PendingTerminalEvent =
  | {
      kind: "completed";
      gen: number;
      result: {
        assistantText: string;
        reasoningStandard?: string;
        reasoningInline?: string;
        assistantMessage?: ChatMessage;
      };
    }
  | {
      kind: "failed";
      gen: number;
      error: unknown;
    };

type DeferredRoundCompletion = {
  gen: number;
  result: {
    assistantText: string;
    reasoningStandard?: string;
    reasoningInline?: string;
    assistantMessage?: ChatMessage;
  };
};

type SendChatOverrides = {
  text?: string;
  displayText?: string;
  extraTextBlocks?: string[];
  skipInstructionPrompts?: boolean;
};

type ConversationStreamCache = {
  assistantText: string;
  reasoningStandard: string;
  reasoningInline: string;
  toolStatusText: string;
  toolStatusState: "running" | "done" | "failed" | "";
  streamToolCalls: Array<{ name: string; argsText: string; status?: "doing" | "done" }>;
  streamToolCallCount: number;
  streamLastToolName: string;
};

export function useChatFlow(options: UseChatFlowOptions) {
  // ── 状态 ──
  let round: RoundState = { phase: "idle" };
  const frontendRoundPhase = ref<FrontendRoundPhase>("idle");
  let generation = 0;
  let sendChatActiveGen = 0; // 防止 bound channel 抢占 sendChat 轮次
  let historyFlushedReceivedGen = 0; // 记录 sendChat 轮次是否已收到 history_flushed，避免 finally 误回收
  let pendingTerminalEvent: PendingTerminalEvent | null = null;
  let deferredRoundCompletion: DeferredRoundCompletion | null = null;
  let queuedStreamingState: {
    assistantText: string;
    reasoningStandard: string;
    reasoningInline: string;
    toolStatusText: string;
    toolStatusState: "running" | "done" | "failed" | "";
    streamToolCalls: Array<{ name: string; argsText: string; status?: "doing" | "done" }>;
    streamToolCallCount: number;
    streamLastToolName: string;
  } | null = null;
  const sendStartedAtMsByGen = new Map<number, number>();

  // ── 流式统计 ──
  let streamToolCallCount = 0;
  let streamLastToolName = "";
  let activeHistoryMessageCount = 0;
  const conversationStreamCache = new Map<string, ConversationStreamCache>();

  function setRound(next: RoundState, frontendPhase?: FrontendRoundPhase) {
    round = next;
    frontendRoundPhase.value = frontendPhase ?? next.phase;
  }
  const reasoningStartedAtMs = ref(0);
  let pendingUserDraftId = "";

  // =========================================================================
  // 工具函数（纯逻辑，无副作用）
  // =========================================================================

  function normalizeConversationId(conversationId?: string | null): string {
    return String(conversationId || "").trim();
  }

  function emptyConversationStreamCache(): ConversationStreamCache {
    return {
      assistantText: "",
      reasoningStandard: "",
      reasoningInline: "",
      toolStatusText: "",
      toolStatusState: "",
      streamToolCalls: [],
      streamToolCallCount: 0,
      streamLastToolName: "",
    };
  }

  function readConversationStreamCache(conversationId?: string | null): ConversationStreamCache | null {
    const cid = normalizeConversationId(conversationId);
    if (!cid) return null;
    const cache = conversationStreamCache.get(cid);
    if (!cache) return null;
    return {
      assistantText: cache.assistantText,
      reasoningStandard: cache.reasoningStandard,
      reasoningInline: cache.reasoningInline,
      toolStatusText: cache.toolStatusText,
      toolStatusState: cache.toolStatusState,
      streamToolCalls: cache.streamToolCalls.map((item) => ({ ...item })),
      streamToolCallCount: cache.streamToolCallCount,
      streamLastToolName: cache.streamLastToolName,
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
      streamToolCalls: Array.isArray(next.streamToolCalls) ? next.streamToolCalls.map((item) => ({ ...item })) : [],
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
    writeConversationStreamCache(cid, () => ({
      assistantText: String(options.latestAssistantText.value || ""),
      reasoningStandard: String(options.latestReasoningStandardText.value || ""),
      reasoningInline: String(options.latestReasoningInlineText.value || ""),
      toolStatusText: String(options.toolStatusText.value || ""),
      toolStatusState: options.toolStatusState.value,
      streamToolCalls: Array.isArray(options.streamToolCalls?.value)
        ? options.streamToolCalls.value.map((item) => ({ ...item }))
        : [],
      streamToolCallCount,
      streamLastToolName,
    }));
  }

  function applyConversationStreamCacheToDisplay(conversationId?: string | null): boolean {
    const cache = readConversationStreamCache(conversationId);
    if (!cache) return false;
    options.latestAssistantText.value = cache.assistantText;
    options.latestReasoningStandardText.value = cache.reasoningStandard;
    options.latestReasoningInlineText.value = cache.reasoningInline;
    options.toolStatusText.value = cache.toolStatusText;
    options.toolStatusState.value = cache.toolStatusState;
    if (options.streamToolCalls) {
      options.streamToolCalls.value = cache.streamToolCalls.map((item) => ({ ...item }));
    }
    streamToolCallCount = cache.streamToolCallCount;
    streamLastToolName = cache.streamLastToolName;
    return true;
  }

  function applyAssistantEventToConversationStreamCache(
    conversationId: string,
    parsed: AssistantDeltaEvent,
  ): boolean {
    const cid = normalizeConversationId(conversationId);
    if (!cid) return false;
    let changed = false;
    writeConversationStreamCache(cid, (current) => {
      const next: ConversationStreamCache = {
        ...current,
        streamToolCalls: current.streamToolCalls.map((item) => ({ ...item })),
      };
      const delta = readDeltaMessage(parsed);
      if (parsed.kind === "tool_status") {
        const toolName = String(parsed.toolName || "").trim();
        if (parsed.toolStatus === "running" && toolName) {
          next.streamToolCallCount += 1;
          next.streamLastToolName = toolName;
          next.streamToolCalls = next.streamToolCalls.map((call, idx, arr) => {
            if (idx !== arr.length - 1) return call;
            if (call.status === "done") return call;
            return { ...call, status: "done" as const };
          });
          next.streamToolCalls.push({
            name: toolName,
            argsText: String(parsed.toolArgs || "").trim(),
            status: "doing",
          });
        }
        next.toolStatusText = parsed.message || "";
        next.toolStatusState =
          parsed.toolStatus === "running" || parsed.toolStatus === "done" || parsed.toolStatus === "failed"
            ? parsed.toolStatus : "";
        changed = true;
        return next;
      }
      if (parsed.kind === "reasoning_standard" && delta) {
        next.reasoningStandard += delta;
        changed = true;
        return next;
      }
      if (parsed.kind === "reasoning_inline" && delta) {
        next.reasoningInline += delta;
        changed = true;
        return next;
      }
      if (delta) {
        next.assistantText += delta;
        changed = true;
      }
      return next;
    });
    return changed;
  }

  function hasAssistantDraftInMessages(): boolean {
    return options.allMessages.value.some((message) =>
      String(message?.id || "").trim().startsWith(DRAFT_ASSISTANT_ID_PREFIX)
    );
  }

  function mergeAssistantText(currentText: string, finalText: string): string {
    const current = String(currentText || "");
    const finalValue = String(finalText || "");
    if (!current) return finalValue;
    if (!finalValue) return current;
    if (finalValue.startsWith(current)) return finalValue;
    return finalValue;
  }

  function readHistoryFlushedPayload(raw: string | undefined): HistoryFlushedPayload | null {
    const text = String(raw || "").trim();
    if (!text) return null;
    try {
      const parsed = JSON.parse(text) as Record<string, unknown>;
      return {
        conversationId: String(parsed.conversationId || "").trim(),
        messageCount: Math.max(0, Math.round(Number(parsed.messageCount) || 0)),
        messages: Array.isArray(parsed.messages) ? (parsed.messages as ChatMessage[]) : [],
        activateAssistant: !!parsed.activateAssistant,
        compactionApplied: !!parsed.compactionApplied,
      };
    } catch {
      return {
        conversationId: text,
        messageCount: 0,
        messages: [],
        activateAssistant: false,
        compactionApplied: false,
      };
    }
  }

  function readRoundCompletedPayload(raw: string | undefined): RoundCompletedPayload | null {
    const text = String(raw || "").trim();
    if (!text) return null;
    try {
      const parsed = JSON.parse(text) as Record<string, unknown>;
      return {
        conversationId: String(parsed.conversationId || "").trim(),
        assistantText: String(parsed.assistantText || ""),
        reasoningStandard: typeof parsed.reasoningStandard === "string" ? parsed.reasoningStandard : undefined,
        reasoningInline: typeof parsed.reasoningInline === "string" ? parsed.reasoningInline : undefined,
        archivedBeforeSend: !!parsed.archivedBeforeSend,
        assistantMessage: (parsed.assistantMessage as ChatMessage | undefined) || undefined,
      };
    } catch {
      return null;
    }
  }

  function readRoundFailedPayload(raw: string | undefined): RoundFailedPayload | null {
    const text = String(raw || "").trim();
    if (!text) return null;
    try {
      const parsed = JSON.parse(text) as Record<string, unknown>;
      return {
        conversationId: typeof parsed.conversationId === "string" ? parsed.conversationId : undefined,
        error: String(parsed.error || ""),
      };
    } catch {
      return { error: text };
    }
  }

  function readDeltaMessage(message: unknown): string {
    if (typeof message === "string") return message;
    if (message && typeof message === "object" && "delta" in message) {
      const value = (message as { delta?: unknown }).delta;
      return typeof value === "string" ? value : "";
    }
    return "";
  }

  function readAssistantEvent(message: unknown): AssistantDeltaEvent {
    if (!message || typeof message !== "object") return {};
    const m = message as Record<string, unknown>;
    return {
      delta: typeof m.delta === "string" ? m.delta : undefined,
      kind: typeof m.kind === "string" ? m.kind : undefined,
      requestId: typeof m.requestId === "string" ? m.requestId : undefined,
      phaseId: typeof m.phaseId === "string" ? m.phaseId : undefined,
      reason: typeof m.reason === "string" ? m.reason : undefined,
      toolName: typeof m.toolName === "string" ? m.toolName : undefined,
      toolStatus: typeof m.toolStatus === "string" ? m.toolStatus : undefined,
      toolArgs: typeof m.toolArgs === "string" ? m.toolArgs : undefined,
      message: typeof m.message === "string" ? m.message : undefined,
    };
  }

  function summarizeToolCallsText(): string {
    if (streamToolCallCount <= 0) return "";
    const extraCount = Math.max(0, streamToolCallCount - 1);
    return extraCount > 0
      ? `调用 ${streamLastToolName || "-"} (+${extraCount})`
      : `调用 ${streamLastToolName || "-"}`;
  }

  function hasAssistantVisibleOutput(result: {
    assistantText: string;
    reasoningStandard?: string;
    reasoningInline?: string;
  }): boolean {
    return (
      !!result.assistantText.trim() ||
      !!(result.reasoningStandard || "").trim() ||
      !!(result.reasoningInline || "").trim()
    );
  }

  function buildQueuedAttachmentPayload(): Array<{ fileName: string; relativePath: string; mime: string }> {
    const list = options.queuedAttachmentNotices?.value || [];
    if (list.length === 0) return [];
    return list
      .map((item) => {
        const fileName = String(item.fileName || "").trim();
        const relativePath = String(item.relativePath || "").trim().replace(/\\/g, "/");
        const mime = String(item.mime || "").trim();
        if (!fileName || !relativePath) return null;
        return { fileName, relativePath, mime };
      })
      .filter((v): v is { fileName: string; relativePath: string; mime: string } => !!v);
  }

  function buildImageAttachmentPayload(
    images: Array<{ mime: string; bytesBase64: string; savedPath?: string }>,
  ): Array<{ fileName: string; relativePath: string; mime: string }> {
    const dedup = new Map<string, { fileName: string; relativePath: string; mime: string }>();
    for (const image of images) {
      const rawPath = String(image.savedPath || "").trim();
      if (!rawPath) continue;
      const relativePath = rawPath.replace(/\\/g, "/");
      if (!relativePath) continue;
      const fileName = relativePath.split("/").pop() || "attachment";
      const mime = String(image.mime || "").trim();
      const key = `${relativePath}::${mime}`;
      if (dedup.has(key)) continue;
      dedup.set(key, { fileName, relativePath, mime });
    }
    return Array.from(dedup.values());
  }

  function buildInstructionExtraTextBlocks(): string[] {
    const list = options.selectedInstructionPrompts?.value || [];
    if (list.length === 0) return [];
    return list
      .map((item) => {
        const prompt = String(item?.prompt || "").trim();
        if (!prompt) return "";
        return `<user instruction>\n${prompt}\n</user instruction>`;
      })
      .filter((item) => !!item);
  }

  // =========================================================================
  // Draft 操作 —— 唯一允许写 allMessages 的地方
  //
  // insertDraft: history_flushed 时插入空气泡
  // updateDraftText: 流式期间把 latestAssistantText 同步到气泡
  // removeDraft: history_flushed 清屏时移除上一轮残留
  // =========================================================================

  function insertUserDraft(
    gen: number,
    text: string,
    images: Array<{ mime: string; bytesBase64: string; savedPath?: string }>,
    attachments: Array<{ fileName: string; relativePath: string; mime: string }>,
    mentions: ChatMentionTarget[],
  ): string {
    const draftId = `${DRAFT_USER_ID_PREFIX}${gen}`;
    const parts: ChatMessage["parts"] = [];
    const normalizedText = String(text || "");
    if (normalizedText) {
      parts.push({ type: "text", text: normalizedText });
    }
    for (const image of images) {
      const mime = String(image.mime || "").trim();
      const bytesBase64 = String(image.bytesBase64 || "").trim();
      if (!mime || !bytesBase64) continue;
      parts.push({ type: "image", mime, bytesBase64 });
    }
    const attachmentPayload = [...attachments, ...buildImageAttachmentPayload(images)];
    const msg: ChatMessage = {
      id: draftId,
      role: "user",
      createdAt: new Date().toISOString(),
      speakerAgentId: "user-persona",
      parts,
      providerMeta: {
        attachments: attachmentPayload.length > 0 ? attachmentPayload : undefined,
        message_meta: mentions.length > 0
          ? {
              kind: "user_message",
              mentions: mentions.map((item) => ({
                agentId: item.agentId,
                agentName: item.agentName,
                departmentId: item.departmentId,
                departmentName: item.departmentName,
              })),
            }
          : undefined,
        _optimistic: true,
      },
    };
    const cur = options.allMessages.value;
    const idx = cur.findIndex((m) => m.id === draftId);
    options.allMessages.value = idx < 0 ? [...cur, msg] : cur.map((m, i) => (i === idx ? msg : m));
    pendingUserDraftId = draftId;
    return draftId;
  }

  function insertDraft(gen: number, initialText = ""): string {
    const draftId = `${DRAFT_ASSISTANT_ID_PREFIX}${gen}`;
    const startedAtMs = sendStartedAtMsByGen.get(gen) || 0;
    const elapsedMs = startedAtMs > 0 ? Math.max(0, Date.now() - startedAtMs) : -1;
    console.warn("[聊天前端耗时] 助理草稿出现", {
      gen,
      elapsedMs,
      conversationId: String(options.getConversationId ? options.getConversationId() : "").trim(),
      activeHistoryMessageCount,
      latestUserTextLength: String(options.latestUserText.value || "").length,
    });
    const agentId = String(options.getSession()?.agentId || "").trim();
    const msg: ChatMessage = {
      id: draftId,
      role: "assistant",
      createdAt: new Date().toISOString(),
      speakerAgentId: agentId || "assistant-draft",
      parts: [{ type: "text", text: "" }],
      providerMeta: {
        reasoningStandard: "",
        reasoningInline: "",
        _streaming: true,
        _streamSegments: [] as string[],
        _streamTail: "",
        _preStreamingStatusText: String(initialText || ""),
      },
    };
    const cur = options.allMessages.value;
    const idx = cur.findIndex((m) => m.id === draftId);
    if (idx >= 0) {
      options.allMessages.value = cur.map((m, i) => (i === idx ? msg : m));
      return draftId;
    }
    const relatedUserDraftId = `${DRAFT_USER_ID_PREFIX}${gen}`;
    const userDraftIdx = cur.findIndex((m) => m.id === relatedUserDraftId);
    if (userDraftIdx >= 0) {
      options.allMessages.value = [
        ...cur.slice(0, userDraftIdx + 1),
        msg,
        ...cur.slice(userDraftIdx + 1),
      ];
      return draftId;
    }
    options.allMessages.value = [...cur, msg];
    return draftId;
  }

  function updateQueuedAssistantDraftStatus(draftId: string, statusText: string) {
    if (!draftId) return;
    const agentId = String(options.getSession()?.agentId || "").trim();
    const existingDraft = options.allMessages.value.find((item) => item.id === draftId);
    const existingMeta = ((existingDraft?.providerMeta || {}) as Record<string, unknown>);
    const msg: ChatMessage = {
      id: draftId,
      role: "assistant",
      createdAt: String(existingDraft?.createdAt || new Date().toISOString()),
      speakerAgentId: agentId || "assistant-draft",
      parts: [{ type: "text", text: "" }],
      providerMeta: {
        ...existingMeta,
        reasoningStandard: "",
        reasoningInline: "",
        _streaming: true,
        _streamSegments: [] as string[],
        _streamTail: "",
        _streamAnimatedDelta: "",
        _preStreamingStatusText: String(statusText || ""),
      },
    };
    const cur = options.allMessages.value;
    const idx = cur.findIndex((m) => m.id === draftId);
    if (idx >= 0) {
      options.allMessages.value = cur.map((m, i) => (i === idx ? msg : m));
    } else {
      const gen = Number(String(draftId).split(":").pop() || 0);
      const relatedUserDraftId = `${DRAFT_USER_ID_PREFIX}${gen}`;
      const userDraftIdx = cur.findIndex((m) => m.id === relatedUserDraftId);
      options.allMessages.value = userDraftIdx >= 0
        ? [
            ...cur.slice(0, userDraftIdx + 1),
            msg,
            ...cur.slice(userDraftIdx + 1),
          ]
        : [...cur, msg];
    }
    console.info("[聊天草稿] 更新预流式草稿状态", {
      draftId,
      statusText,
      conversationId: String(options.getConversationId ? options.getConversationId() : "").trim(),
    });
  }

  function readDraftStreamSegments(draftId: string): string[] {
    if (!draftId) return [];
    const draft = options.allMessages.value.find((item) => item.id === draftId);
    const meta = (draft?.providerMeta || {}) as Record<string, unknown>;
    if (!Array.isArray(meta._streamSegments)) return [];
    return (meta._streamSegments as unknown[])
      .map((item) => String(item ?? ""))
      .filter((item) => item.length > 0);
  }

  function readDraftStreamTail(draftId: string): string {
    if (!draftId) return "";
    const draft = options.allMessages.value.find((item) => item.id === draftId);
    const meta = (draft?.providerMeta || {}) as Record<string, unknown>;
    return String(meta._streamTail ?? "");
  }

  function consumeClosedMarkdownBlocks(input: string): { chunks: string[]; tail: string } {
    const chunks: string[] = [];
    let cursor = 0;
    let scan = 0;
    let inFence = false;
    let fenceMarker = "";
    let lineStart = 0;
    let lastSafe = 0;
    let prevBlank = false;

    while (scan <= input.length) {
      const isEnd = scan === input.length;
      const ch = isEnd ? "\n" : input[scan];
      if (ch !== "\n" && !isEnd) {
        scan += 1;
        continue;
      }
      const lineEnd = scan;
      const line = input.slice(lineStart, lineEnd);
      const trimmed = line.trimStart();
      const isBlank = line.trim().length === 0;

      if (!inFence && (trimmed.startsWith("```") || trimmed.startsWith("~~~"))) {
        inFence = true;
        fenceMarker = trimmed.startsWith("~~~") ? "~~~" : "```";
      } else if (inFence && fenceMarker && trimmed.startsWith(fenceMarker)) {
        inFence = false;
        lastSafe = isEnd ? lineEnd : lineEnd + 1;
      }

      if (!inFence && prevBlank && !isBlank) {
        const splitAt = lineStart;
        if (splitAt > cursor) {
          const chunk = input.slice(cursor, splitAt).trim();
          if (chunk) chunks.push(chunk);
          cursor = splitAt;
          lastSafe = splitAt;
        }
      }

      prevBlank = isBlank;
      lineStart = scan + 1;
      scan += 1;
    }

    if (lastSafe > cursor) {
      const chunk = input.slice(cursor, lastSafe).trim();
      if (chunk) chunks.push(chunk);
      cursor = lastSafe;
    }

    const tail = input.slice(cursor);
    return { chunks, tail };
  }

  function updateDraftText(
    draftId: string,
    streamSegments?: string[],
    streamTail?: string,
    streamAnimatedDelta = "",
  ) {
    if (!draftId) return;
    const agentId = String(options.getSession()?.agentId || "").trim();
    const nextStreamSegments = streamSegments || readDraftStreamSegments(draftId);
    const nextStreamTail = streamTail ?? readDraftStreamTail(draftId);
    const existingDraft = options.allMessages.value.find((item) => item.id === draftId);
    const msg: ChatMessage = {
      id: draftId,
      role: "assistant",
      createdAt: String(existingDraft?.createdAt || new Date().toISOString()),
      speakerAgentId: agentId || "assistant-draft",
      parts: [{ type: "text", text: String(options.latestAssistantText.value || "") }],
      providerMeta: {
        reasoningStandard: String(options.latestReasoningStandardText.value || ""),
        reasoningInline: String(options.latestReasoningInlineText.value || ""),
        _streaming: true,
        _streamSegments: nextStreamSegments,
        _streamTail: nextStreamTail,
        _streamAnimatedDelta: String(streamAnimatedDelta || ""),
      },
    };
    const cur = options.allMessages.value;
    const idx = cur.findIndex((m) => m.id === draftId);
    options.allMessages.value = idx < 0 ? [...cur, msg] : cur.map((m, i) => (i === idx ? msg : m));
  }

  function removeDraft(draftId: string) {
    if (!draftId) return;
    if (draftId === pendingUserDraftId) {
      pendingUserDraftId = "";
    }
    options.allMessages.value = options.allMessages.value.filter((m) => m.id !== draftId);
  }

  function finalizeDraft(draftId: string, finalMessage?: ChatMessage) {
    if (!draftId) return;
    const current = options.allMessages.value;
    const draftIdx = current.findIndex((m) => m.id === draftId);
    if (draftIdx < 0) return;

    if (finalMessage) {
      const deduped = current.filter((m, idx) => idx === draftIdx || m.id !== finalMessage.id);
      const nextDraftIdx = deduped.findIndex((m) => m.id === draftId);
      if (nextDraftIdx < 0) {
        options.allMessages.value = deduped;
        return;
      }
      options.allMessages.value = deduped.map((m, idx) => (idx === nextDraftIdx ? finalMessage : m));
      return;
    }

    // 没有后端正式消息时，至少将 draft 退为非 streaming，避免残留流式态。
    const draft = current[draftIdx];
    const draftMeta = ((draft.providerMeta || {}) as Record<string, unknown>);
    const nextMeta = { ...draftMeta };
    delete (nextMeta as Record<string, unknown>)._streaming;
    const normalized: ChatMessage = { ...draft, providerMeta: nextMeta };
    options.allMessages.value = current.map((m, idx) => (idx === draftIdx ? normalized : m));
  }

  // =========================================================================
  // 流式输出
  // =========================================================================

  function applyAssistantDeltaToDraft(draftId: string, delta: string) {
    if (!draftId || !delta) return;
    options.latestAssistantText.value += delta;
    const currentSegments = readDraftStreamSegments(draftId);
    const currentTail = readDraftStreamTail(draftId);
    const parsed = consumeClosedMarkdownBlocks(`${currentTail}${delta}`);
    const nextStreamSegments = parsed.chunks.length > 0
      ? [...currentSegments, ...parsed.chunks]
      : currentSegments;
    updateDraftText(draftId, nextStreamSegments, parsed.tail, delta);
  }

  function finalizeDeferredRoundCompletion() {
    if (!deferredRoundCompletion) return;
    if (round.phase !== "streaming" || round.gen !== deferredRoundCompletion.gen) {
      deferredRoundCompletion = null;
      return;
    }
    const { draftId } = round;
    const { result } = deferredRoundCompletion;
    deferredRoundCompletion = null;

    options.latestAssistantText.value = mergeAssistantText(
      options.latestAssistantText.value,
      String(result.assistantText || ""),
    );

    if (typeof result.reasoningStandard === "string") {
      options.latestReasoningStandardText.value = result.reasoningStandard;
    }
    if (typeof result.reasoningInline === "string") {
      options.latestReasoningInlineText.value = result.reasoningInline;
    }
    options.chatErrorText.value = "";
    if ((options.toolStatusState.value as string) === "running") {
      options.toolStatusState.value = "done";
      options.toolStatusText.value = summarizeToolCallsText() || options.t("status.toolCallDone");
    }

    updateDraftText(draftId);
    finalizeDraft(draftId, result.assistantMessage);
    setRound({ phase: "idle" });
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
    clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
  }

  async function finalizeQueuedRoundWithoutDraft(
    gen: number,
    result: {
      assistantText: string;
      reasoningStandard?: string;
      reasoningInline?: string;
      assistantMessage?: ChatMessage;
    },
  ) {
    sendStartedAtMsByGen.delete(gen);
    if (round.phase !== "queued" || round.gen !== gen) return;
    removeDraft(`${DRAFT_ASSISTANT_ID_PREFIX}${gen}`);
    pendingTerminalEvent = null;
    deferredRoundCompletion = null;
    queuedStreamingState = null;
    options.chatErrorText.value = "";
    setRound({ phase: "idle" });
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
    clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
    await options.onReloadMessages();
  }

  async function failQueuedRoundWithoutDraft(gen: number, error: unknown) {
    sendStartedAtMsByGen.delete(gen);
    if (round.phase !== "queued" || round.gen !== gen) return;
    removeDraft(`${DRAFT_ASSISTANT_ID_PREFIX}${gen}`);
    pendingTerminalEvent = null;
    deferredRoundCompletion = null;
    queuedStreamingState = null;
    options.latestAssistantText.value = "";
    options.latestReasoningStandardText.value = "";
    options.latestReasoningInlineText.value = "";
    options.chatErrorText.value = options.formatRequestFailed(error);
    if (!options.toolStatusText.value) {
      options.toolStatusState.value = "failed";
      options.toolStatusText.value = summarizeToolCallsText() || options.t("status.toolCallFailed");
    }
    if (pendingUserDraftId === `${DRAFT_USER_ID_PREFIX}${gen}`) {
      removeDraft(pendingUserDraftId);
    }
    setRound({ phase: "idle" });
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
    clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
    await options.onReloadMessages();
  }

  function enqueueStreamDelta(gen: number, delta: string) {
    if (round.phase !== "streaming" || round.gen !== gen || !delta) return;
    applyAssistantDeltaToDraft(round.draftId, delta);
    finalizeDeferredRoundCompletion();
  }

  // =========================================================================
  // 显示状态重置（只在 history_flushed 清屏时调用）
  // =========================================================================

  function resetDisplayState() {
    deferredRoundCompletion = null;
    queuedStreamingState = null;
    streamToolCallCount = 0;
    streamLastToolName = "";
    options.latestUserText.value = "";
    options.latestUserImages.value = [];
    options.latestAssistantText.value = "";
    options.latestReasoningStandardText.value = "";
    options.latestReasoningInlineText.value = "";
    options.toolStatusText.value = "";
    options.toolStatusState.value = "";
    if (options.streamToolCalls) options.streamToolCalls.value = [];
  }

  function clearForegroundRoundState() {
    ++generation;
    sendChatActiveGen = 0;
    deferredRoundCompletion = null;
    if (pendingUserDraftId) {
      removeDraft(pendingUserDraftId);
    }
    if (round.phase === "streaming") {
      removeDraft(round.draftId);
    } else if (round.phase === "queued") {
      removeDraft(`${DRAFT_ASSISTANT_ID_PREFIX}${round.gen}`);
    }
    setRound({ phase: "idle" });
    activeHistoryMessageCount = 0;
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
    resetDisplayState();
    options.chatErrorText.value = "";
    clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
  }

  function readMessagePlainText(message?: ChatMessage): string {
    if (!message) return "";
    const parts = Array.isArray(message.parts) ? message.parts : [];
    return parts
      .filter((part) => part && typeof part === "object" && (part as { type?: unknown }).type === "text")
      .map((part) => String((part as { text?: unknown }).text || ""))
      .join("");
  }

  function freezeForegroundRoundState() {
    ++generation;
    sendChatActiveGen = 0;
    if (pendingUserDraftId) {
      removeDraft(pendingUserDraftId);
    }
    if (round.phase === "streaming") {
      finalizeDraft(round.draftId);
    } else if (round.phase === "queued") {
      removeDraft(`${DRAFT_ASSISTANT_ID_PREFIX}${round.gen}`);
    }
    setRound({ phase: "idle" });
    activeHistoryMessageCount = 0;
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
    resetDisplayState();
    options.chatErrorText.value = "";
    clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
  }

  function applyQueuedStreamingStateIfNeeded(draftId: string) {
    if (!queuedStreamingState) return;
    options.latestAssistantText.value = queuedStreamingState.assistantText;
    options.latestReasoningStandardText.value = queuedStreamingState.reasoningStandard;
    options.latestReasoningInlineText.value = queuedStreamingState.reasoningInline;
    options.toolStatusText.value = queuedStreamingState.toolStatusText;
    options.toolStatusState.value = queuedStreamingState.toolStatusState;
    if (options.streamToolCalls) {
      options.streamToolCalls.value = queuedStreamingState.streamToolCalls;
    }
    streamToolCallCount = queuedStreamingState.streamToolCallCount;
    streamLastToolName = queuedStreamingState.streamLastToolName;
    queuedStreamingState = null;
    updateDraftText(draftId);
  }

  function ensureForegroundStreamingRound() {
    const conversationId = options.getConversationId ? options.getConversationId() : "";
    if (round.phase === "streaming") {
      if (!hasAssistantDraftInMessages()) {
        applyConversationStreamCacheToDisplay(conversationId);
        const draftId = insertDraft(round.gen);
        updateDraftText(draftId);
        setRound({ phase: "streaming", gen: round.gen, draftId });
      }
      return round.gen;
    }
    const gen = ++generation;
    const existingDraft = [...options.allMessages.value]
      .reverse()
      .find((message) => String(message?.id || "").trim().startsWith(DRAFT_ASSISTANT_ID_PREFIX));
    const existingDraftId = String(existingDraft?.id || "").trim();
    const existingDraftMeta = ((existingDraft?.providerMeta || {}) as Record<string, unknown>);
    const restoredFromCache = !existingDraftId && applyConversationStreamCacheToDisplay(conversationId);
    if (!restoredFromCache) {
      options.latestAssistantText.value = readMessagePlainText(existingDraft);
      options.latestReasoningStandardText.value = String(existingDraftMeta.reasoningStandard || "");
      options.latestReasoningInlineText.value = String(existingDraftMeta.reasoningInline || "");
    }
    activeHistoryMessageCount = formalizeMessages(options.allMessages.value).length;
    const draftId = existingDraftId || insertDraft(gen);
    if (existingDraftId || restoredFromCache) {
      updateDraftText(draftId);
    }
    setRound({ phase: "streaming", gen, draftId });
    options.chatting.value = true;
    applyQueuedStreamingStateIfNeeded(draftId);
    return gen;
  }

  function promoteQueuedRoundToStreaming(gen: number) {
    if (round.phase === "streaming" && round.gen === gen) {
      return gen;
    }
    if (round.phase !== "queued" || round.gen !== gen) {
      return 0;
    }
    const conversationId = options.getConversationId ? options.getConversationId() : "";
    const existingDraft = [...options.allMessages.value]
      .reverse()
      .find((message) => String(message?.id || "").trim().startsWith(DRAFT_ASSISTANT_ID_PREFIX));
    const existingDraftId = String(existingDraft?.id || "").trim();
    const existingDraftMeta = ((existingDraft?.providerMeta || {}) as Record<string, unknown>);
    const restoredFromCache = !existingDraftId && applyConversationStreamCacheToDisplay(conversationId);
    if (!restoredFromCache) {
      options.latestAssistantText.value = readMessagePlainText(existingDraft);
      options.latestReasoningStandardText.value = String(existingDraftMeta.reasoningStandard || "");
      options.latestReasoningInlineText.value = String(existingDraftMeta.reasoningInline || "");
    }
    activeHistoryMessageCount = formalizeMessages(options.allMessages.value).length;
    const draftId = existingDraftId || insertDraft(gen);
    if (existingDraftId || restoredFromCache) {
      updateDraftText(draftId);
    }
    setRound({ phase: "streaming", gen, draftId });
    options.chatting.value = true;
    applyQueuedStreamingStateIfNeeded(draftId);
    applyPendingTerminalEvent(gen);
    return gen;
  }

  function assistantEventHasVisibleProgress(parsed: AssistantDeltaEvent): boolean {
    return (
      !!readDeltaMessage(parsed)
      || parsed.kind === "reasoning_standard"
      || parsed.kind === "reasoning_inline"
      || parsed.kind === "tool_status"
    );
  }

  function formalizeMessages(messages: ChatMessage[]): ChatMessage[] {
    return messages.filter((item) => {
      const messageId = String(item?.id || "").trim();
      return (
        !messageId.startsWith(DRAFT_ASSISTANT_ID_PREFIX)
        && !messageId.startsWith(DRAFT_USER_ID_PREFIX)
      );
    });
  }

  // =========================================================================
  // 事件处理
  // =========================================================================

  /**
 * history_flushed：唯一做 allMessages 大规模合并的地方。
 * 1. 移除旧 draft   2. reload / onHistoryFlushed   3. 保持 queued，等待真正流式进展后再插 draft
 * 之后不再碰 allMessages（除了 updateDraftText）。
   */
  async function handleHistoryFlushed(
    gen: number,
    parsed: AssistantDeltaEvent,
    source: "sendChat" | "bound",
  ) {
    const flushed = readHistoryFlushedPayload(parsed.message);
    const startedAtMs = sendStartedAtMsByGen.get(gen) || 0;
    const elapsedMs = startedAtMs > 0 ? Math.max(0, Date.now() - startedAtMs) : -1;
    const shouldActivate = source === "sendChat" || !!flushed?.activateAssistant;
    const shouldForceReset = !!flushed?.compactionApplied;
    console.warn("[聊天前端耗时] history_flushed 到达", {
      source,
      gen,
      elapsedMs,
      conversationId: String(flushed?.conversationId || "").trim(),
      shouldActivate,
      shouldForceReset,
      messageCount: Math.max(0, Math.round(Number(flushed?.messageCount || 0))),
    });
    console.info("[CHAT_TRACE][history_flushed] 开始", {
      source,
      gen,
      sendChatActiveGen,
      shouldActivate,
      shouldForceReset,
      payloadConversationId: String(flushed?.conversationId || "").trim(),
    });
    // sendChat 活跃时，仅拦截“会激活助理”的 bound 批次，避免抢占当前轮次；
    // 非激活批次只做历史追加，不应被阻塞。
    if (source === "bound" && sendChatActiveGen > 0 && shouldActivate) {
      console.info("[CHAT_TRACE][history_flushed] 跳过", {
        source,
        gen,
        sendChatActiveGen,
        shouldActivate,
        shouldForceReset,
      });
      return;
    }
    const replayMessages = Array.isArray(flushed?.messages) ? flushed!.messages : [];
    console.info("[CHAT_TRACE][history_flushed] 完成", {
      source,
      gen,
      shouldActivate,
      shouldForceReset,
      payloadConversationId: String(flushed?.conversationId || "").trim(),
      replayCount: replayMessages.length,
      messageCount: Number(flushed?.messageCount || 0),
      firstMessageId: String(replayMessages[0]?.id || ""),
      lastMessageId: String(replayMessages[replayMessages.length - 1]?.id || ""),
    });
    // 测试里 history_flushed 可能只给 messageCount，不给 messages 数组。
    // 若只看 replayMessages.length，会把可见窗口错误压成 1，导致轮次显示异常。
    const payloadMessageCount = Math.max(0, Math.round(Number(flushed?.messageCount || 0)));
    const batchVisibleCount = Math.max(1, replayMessages.length, payloadMessageCount);
    activeHistoryMessageCount = batchVisibleCount;
    const shouldPreserveStreamingState =
      shouldActivate && round.phase === "streaming" && round.gen === gen;
    const preservedStreamingState = shouldPreserveStreamingState ? {
      assistantText: options.latestAssistantText.value,
      reasoningStandard: options.latestReasoningStandardText.value,
      reasoningInline: options.latestReasoningInlineText.value,
      toolStatusText: options.toolStatusText.value,
      toolStatusState: options.toolStatusState.value,
      streamToolCalls: options.streamToolCalls ? [...options.streamToolCalls.value] : [],
      streamToolCallCount,
      streamLastToolName,
    } : null;
    if (shouldActivate || shouldForceReset) {
      // 激活助理或上下文整理改写消息序列时，先强制收口当前显示态。
      const oldDraftId = round.phase === "streaming" ? round.draftId : "";
      resetDisplayState();
      if (oldDraftId) removeDraft(oldDraftId);
      setRound(shouldActivate ? { phase: "queued", gen } : { phase: "idle" });
      queuedStreamingState = preservedStreamingState;
    }

    // ── reload ──
    if (options.onHistoryFlushed) {
      console.info("[CHAT_TRACE][history_flushed] apply_start", {
        source,
        gen,
        shouldActivate,
        shouldForceReset,
        batchVisibleCount,
      });
      await options.onHistoryFlushed({
        conversationId: String(flushed?.conversationId || "").trim(),
        messageCount: batchVisibleCount,
        pendingMessages: replayMessages,
        activateAssistant: shouldActivate,
      });
      console.info("[CHAT_TRACE][history_flushed] apply_done", {
        source,
        gen,
        shouldActivate,
        shouldForceReset,
        batchVisibleCount,
      });
    } else {
      await options.onReloadMessages();
    }

    if (!shouldActivate) {
      // await 期间可能有新的 sendChat/轮次启动，避免回写旧状态覆盖新轮次
      if (gen !== generation) return;
      setRound({ phase: "idle" });
      options.chatting.value = false;
      console.info("[CHAT_TRACE][history_flushed] non_activate_finish", {
        source,
        gen,
        generation,
      });
      return;
    }

    // await 后校验：可能已被新 sendChat 抢占
    if (round.phase !== "queued" || round.gen !== gen) return;

    // queued 阶段不提前创建草稿，等待真正的思维链/工具/正文流式事件到达。
    // 若终态已先到达，则直接收口，不再制造一闪而过的空草稿。
    if (pendingTerminalEvent && pendingTerminalEvent.gen === gen) {
      const pending = pendingTerminalEvent;
      pendingTerminalEvent = null;
      queuedStreamingState = null;
      if (pending.kind === "completed") {
        await finalizeQueuedRoundWithoutDraft(gen, pending.result);
        return;
      }
      await failQueuedRoundWithoutDraft(gen, pending.error);
      return;
    }
    updateQueuedAssistantDraftStatus(`${DRAFT_ASSISTANT_ID_PREFIX}${gen}`, options.t("chat.statusWaitingReply"));
    frontendRoundPhase.value = "waiting";
  }

  /**
   * round_completed：终结当前轮次。
   * 只做文字收尾 + 状态转换，不碰 allMessages（除了 updateDraftText）。
   */
  async function handleRoundCompleted(
    gen: number,
    result: {
      assistantText: string;
      reasoningStandard?: string;
      reasoningInline?: string;
      assistantMessage?: ChatMessage;
    },
  ) {
    sendStartedAtMsByGen.delete(gen);
    if (round.phase === "queued" && round.gen === gen) {
      await finalizeQueuedRoundWithoutDraft(gen, result);
      return;
    }
    if (round.phase !== "streaming" || round.gen !== gen) return;
    deferredRoundCompletion = { gen, result };
    finalizeDeferredRoundCompletion();
  }

  async function handleRoundFailed(gen: number, error: unknown) {
    sendStartedAtMsByGen.delete(gen);
    if (round.phase === "queued" && round.gen === gen) {
      await failQueuedRoundWithoutDraft(gen, error);
      return;
    }
    if (round.phase !== "streaming" || round.gen !== gen) return;
    const { draftId } = round;
    deferredRoundCompletion = null;

    options.latestAssistantText.value = "";
    options.latestReasoningStandardText.value = "";
    options.latestReasoningInlineText.value = "";
    options.chatErrorText.value = options.formatRequestFailed(error);
    if (!options.toolStatusText.value) {
      options.toolStatusState.value = "failed";
      options.toolStatusText.value = summarizeToolCallsText() || options.t("status.toolCallFailed");
    }
    removeDraft(draftId);
    setRound({ phase: "idle" });
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
  }

  function applyPendingTerminalEvent(gen: number) {
    if (!pendingTerminalEvent || pendingTerminalEvent.gen !== gen) return false;
    const pending = pendingTerminalEvent;
    pendingTerminalEvent = null;
    deferredRoundCompletion = null;
    if (pending.kind === "completed") {
      void handleRoundCompleted(gen, pending.result);
      return true;
    }
    void handleRoundFailed(gen, pending.error);
    return true;
  }

  // =========================================================================
  // Delta 分发
  // =========================================================================

  function handleStreamingEvent(currentGen: number, parsed: AssistantDeltaEvent) {
    if (!currentGen) return;
    if (round.phase === "queued" && round.gen === currentGen && assistantEventHasVisibleProgress(parsed)) {
      promoteQueuedRoundToStreaming(currentGen);
    }
    const currentRound = round;
    if (currentRound.phase !== "streaming" && currentRound.phase !== "queued") return;
    if (currentRound.gen !== currentGen) return;

    if (parsed.kind === "round_completed") {
      const p = readRoundCompletedPayload(parsed.message);
      if (currentRound.phase === "queued") {
        pendingTerminalEvent = {
          kind: "completed",
          gen: currentGen,
          result: {
            assistantText: String(p?.assistantText || ""),
            reasoningStandard: p?.reasoningStandard,
            reasoningInline: p?.reasoningInline,
            assistantMessage: p?.assistantMessage,
          },
        };
        return;
      }
      void handleRoundCompleted(currentGen, {
        assistantText: String(p?.assistantText || ""),
        reasoningStandard: p?.reasoningStandard,
        reasoningInline: p?.reasoningInline,
        assistantMessage: p?.assistantMessage,
      });
      return;
    }
    if (parsed.kind === "round_failed") {
      const p = readRoundFailedPayload(parsed.message);
      if (currentRound.phase === "queued") {
        pendingTerminalEvent = {
          kind: "failed",
          gen: currentGen,
          error: p?.error || parsed.message || JSON.stringify(parsed),
        };
        return;
      }
      void handleRoundFailed(currentGen, p?.error || parsed.message || JSON.stringify(parsed));
      return;
    }

    if (parsed.kind === "tool_status") {
      const toolName = String(parsed.toolName || "").trim();
      if (parsed.toolStatus === "running" && toolName) {
        streamToolCallCount += 1;
        streamLastToolName = toolName;
        if (options.streamToolCalls) {
          const next = options.streamToolCalls.value.map((call, idx, arr) => {
            if (idx !== arr.length - 1) return call;
            if (call.status === "done") return call;
            return { ...call, status: "done" as const };
          });
          next.push({
            name: toolName,
            argsText: String(parsed.toolArgs || "").trim(),
            status: "doing",
          });
          options.streamToolCalls.value = next;
        }
      }
      options.toolStatusText.value = parsed.message || "";
      options.toolStatusState.value =
        parsed.toolStatus === "running" || parsed.toolStatus === "done" || parsed.toolStatus === "failed"
          ? parsed.toolStatus : "";
      syncCurrentDisplayStateToConversationStreamCache();
      if (currentRound.phase === "streaming") {
        updateDraftText(currentRound.draftId);
      }
      return;
    }
    if (parsed.kind === "reasoning_standard") {
      const dt = readDeltaMessage(parsed);
      if (dt && reasoningStartedAtMs.value === 0) reasoningStartedAtMs.value = Date.now();
      options.latestReasoningStandardText.value += dt;
      syncCurrentDisplayStateToConversationStreamCache();
      if (currentRound.phase === "streaming") {
        updateDraftText(currentRound.draftId);
      }
      return;
    }
    if (parsed.kind === "reasoning_inline") {
      const dt = readDeltaMessage(parsed);
      if (dt && reasoningStartedAtMs.value === 0) reasoningStartedAtMs.value = Date.now();
      options.latestReasoningInlineText.value += dt;
      syncCurrentDisplayStateToConversationStreamCache();
      if (currentRound.phase === "streaming") {
        updateDraftText(currentRound.draftId);
      }
      return;
    }

    enqueueStreamDelta(currentGen, readDeltaMessage(parsed));
    syncCurrentDisplayStateToConversationStreamCache();
  }

  function attachDeltaHandler(
    channel: Channel<AssistantDeltaEvent>,
    source: "sendChat" | "bound",
    getGen: () => number,
    nextGenOnHistoryFlushed: () => number,
  ) {
    channel.onmessage = (event) => {
      const parsed = readAssistantEvent(event);

      if (parsed.kind === "history_flushed") {
        const hfGen = nextGenOnHistoryFlushed();
        // sendChat 轮次如果已被本地中断（generation 已前进），忽略迟到的 history_flushed。
        if (source === "sendChat" && hfGen !== generation) {
          return;
        }
        if (source === "sendChat") {
          historyFlushedReceivedGen = Math.max(historyFlushedReceivedGen, hfGen);
        }
        void handleHistoryFlushed(hfGen, parsed, source).catch((err) => {
          console.error("[聊天] history_flushed 处理失败", {
            message: String((err as { message?: string })?.message ?? err ?? ""),
            gen: hfGen,
          });
          options.chatErrorText.value = options.formatRequestFailed(err);
        });
        return;
      }

      const currentGen = getGen();
      handleStreamingEvent(currentGen, parsed);
    };
  }

  let boundConversationId = "";
  let boundConversationInitialized = false;
  let boundDisplayGeneration = 0;
  let boundDeltaChannel: Channel<AssistantDeltaEvent> | null = null;

  async function bindActiveConversationStream(conversationId: string, force = false) {
    if (!options.invokeBindActiveChatViewStream) return;
    const id = String(conversationId || "").trim();
    if (!force && boundConversationInitialized && id === boundConversationId) return;
    const channel = new Channel<AssistantDeltaEvent>();
    attachDeltaHandler(
      channel,
      "bound",
      () => (round.phase === "streaming" ? round.gen : boundDisplayGeneration),
      () => (round.phase === "streaming" ? round.gen : boundDisplayGeneration),
    );
    await options.invokeBindActiveChatViewStream({
      conversationId: id || undefined,
      onDelta: channel,
    });
    boundDeltaChannel = channel;
    boundConversationId = id;
    boundConversationInitialized = true;
    if (!id) boundDisplayGeneration = 0;
  }

  async function handleExternalStreamRebindRequired(payload: unknown) {
    const raw = payload && typeof payload === "object" ? payload as Record<string, unknown> : null;
    const payloadConversationId = String(raw?.conversationId || "").trim();
    const currentConversationId = String(options.getConversationId ? options.getConversationId() : "").trim();
    if (!payloadConversationId || !currentConversationId || payloadConversationId !== currentConversationId) {
      return;
    }
    const requestId = String(raw?.requestId || "").trim();
    const phaseId = String(raw?.phaseId || "").trim();
    const reason = String(raw?.reason || "").trim();
    console.info("[聊天] 流式通道重绑 开始", {
      conversationId: currentConversationId,
      requestId,
      phaseId,
      reason,
      roundPhase: round.phase,
    });
    try {
      await bindActiveConversationStream(currentConversationId, true);
      if (round.phase !== "streaming") {
        console.info("[聊天流式重绑][前端] 重绑事件触发恢复草稿", {
          conversationId: currentConversationId,
          requestId,
          phaseId,
          reason,
          roundPhase: round.phase,
        });
        ensureForegroundStreamingRound();
      }
      console.info("[聊天] 流式通道重绑 完成", {
        conversationId: currentConversationId,
        requestId,
        phaseId,
        reason,
      });
    } catch (error) {
      console.error("[聊天] 流式通道重绑 失败", {
        conversationId: currentConversationId,
        requestId,
        phaseId,
        reason,
        error,
      });
      throw error;
    }
  }

  async function handleExternalHistoryFlushed(payload: unknown) {
    const raw = (() => {
      if (typeof payload === "string") return payload;
      if (payload && typeof payload === "object") {
        try {
          return JSON.stringify(payload);
        } catch {
          return "";
        }
      }
      return "";
    })();
    const parsed = readHistoryFlushedPayload(raw);
    if (!parsed) return;
    const currentConversationId = String(options.getConversationId ? options.getConversationId() : "").trim();
    const payloadConversationId = String(parsed.conversationId || "").trim();
    if (currentConversationId && payloadConversationId && currentConversationId !== payloadConversationId) {
      return;
    }
    const treatAsSendChat = sendChatActiveGen > 0 && !!parsed.activateAssistant;
    const source: "sendChat" | "bound" = treatAsSendChat ? "sendChat" : "bound";
    const gen = treatAsSendChat ? sendChatActiveGen : ++generation;
    if (!treatAsSendChat) {
      boundDisplayGeneration = gen;
    }
    await handleHistoryFlushed(
      gen,
      {
        kind: "history_flushed",
        message: JSON.stringify(parsed),
      },
      source,
    );
  }

  async function handleExternalRoundCompleted(payload: unknown) {
    const raw = (() => {
      if (typeof payload === "string") return payload;
      if (payload && typeof payload === "object") {
        try {
          return JSON.stringify(payload);
        } catch {
          return "";
        }
      }
      return "";
    })();
    const parsed = readRoundCompletedPayload(raw);
    if (!parsed) return;
    const currentConversationId = String(options.getConversationId ? options.getConversationId() : "").trim();
    const payloadConversationId = String(parsed.conversationId || "").trim();
    if (currentConversationId && payloadConversationId && currentConversationId !== payloadConversationId) {
      clearConversationStreamCache(payloadConversationId);
      return;
    }
    if (round.phase !== "streaming") {
      resetDisplayState();
      options.chatting.value = false;
      reasoningStartedAtMs.value = 0;
      await options.onReloadMessages();
      return;
    }
    handleRoundCompleted(round.gen, {
      assistantText: String(parsed.assistantText || ""),
      reasoningStandard: parsed.reasoningStandard,
      reasoningInline: parsed.reasoningInline,
      assistantMessage: parsed.assistantMessage,
    });
  }

  async function handleExternalRoundFailed(payload: unknown) {
    const raw = (() => {
      if (typeof payload === "string") return payload;
      if (payload && typeof payload === "object") {
        try {
          return JSON.stringify(payload);
        } catch {
          return "";
        }
      }
      return "";
    })();
    const parsed = readRoundFailedPayload(raw);
    const currentConversationId = String(options.getConversationId ? options.getConversationId() : "").trim();
    const payloadConversationId = String(parsed?.conversationId || "").trim();
    if (currentConversationId && payloadConversationId && currentConversationId !== payloadConversationId) {
      clearConversationStreamCache(payloadConversationId);
      return;
    }
    if (round.phase !== "streaming") {
      options.latestAssistantText.value = "";
      options.latestReasoningStandardText.value = "";
      options.latestReasoningInlineText.value = "";
      options.chatting.value = false;
      reasoningStartedAtMs.value = 0;
      // 记录非流式阶段的轮次失败错误，包含上下文和错误详情
      const errorDetail = parsed?.error || raw || String(raw);
      const errorObj = typeof errorDetail === "string" ? (
        (() => {
          try {
            const obj = JSON.parse(errorDetail);
            return obj;
          } catch {
            return { message: errorDetail };
          }
        })()
      ) : errorDetail;
      console.error(
        "[聊天流程] 非流式轮次失败",
        {
          roundPhase: round.phase,
          roundGen: round.phase === "idle" ? null : round.gen,
          error: errorObj,
          rawPayload: raw,
        }
      );
      options.chatErrorText.value = options.formatRequestFailed(errorDetail);
      await options.onReloadMessages();
      return;
    }
    handleRoundFailed(round.gen, parsed?.error || raw || String(raw));
  }

  async function handleExternalAssistantDelta(payload: unknown) {
    const rawObj = payload && typeof payload === "object" ? payload as Record<string, unknown> : null;
    const currentConversationId = String(options.getConversationId ? options.getConversationId() : "").trim();
    const payloadConversationId = String(rawObj?.conversationId || "").trim();
    const parsed = readAssistantEvent(rawObj?.event ?? payload);
    const cacheConversationId = payloadConversationId || currentConversationId;
    if (cacheConversationId) {
      applyAssistantEventToConversationStreamCache(cacheConversationId, parsed);
    }
    if (currentConversationId && payloadConversationId && currentConversationId !== payloadConversationId) {
      return;
    }
    console.info("[聊天流式重绑][前端] 处理助手增量普通事件", {
      currentConversationId,
      payloadConversationId,
      kind: parsed.kind || "delta",
      toolName: parsed.toolName || "",
      toolStatus: parsed.toolStatus || "",
      deltaLength: String(parsed.delta || "").length,
    });
    const shouldProjectFromAppEvent =
      parsed.kind === "tool_status"
      || round.phase !== "streaming"
      || !hasAssistantDraftInMessages();
    const shouldResumeForegroundRound =
      shouldProjectFromAppEvent
      && assistantEventHasVisibleProgress(parsed);
    if (shouldResumeForegroundRound) {
      console.info("[聊天流式重绑][前端] 普通事件触发恢复前景流式", {
        currentConversationId,
        payloadConversationId,
        kind: parsed.kind || "delta",
      });
    }
    if (!shouldProjectFromAppEvent) {
      return;
    }
    const currentGen = shouldResumeForegroundRound
      ? ensureForegroundStreamingRound()
      : (round.phase === "streaming" ? round.gen : 0);
    if (!currentGen) return;
    if (parsed.kind === "reasoning_standard" || parsed.kind === "reasoning_inline") {
      const delta = readDeltaMessage(parsed);
      if (delta && reasoningStartedAtMs.value === 0) {
        reasoningStartedAtMs.value = Date.now();
      }
    }
    if (parsed.kind === "tool_status") {
      applyConversationStreamCacheToDisplay(cacheConversationId);
      if (round.phase === "streaming") {
        updateDraftText(round.draftId);
      }
      return;
    }
  }

  // =========================================================================
  // 公共方法
  // =========================================================================

  async function sendChat(overrides?: SendChatOverrides) {
    const useOverrideMessage = !!overrides && typeof overrides.text === "string";
    const plainText =
      useOverrideMessage
        ? String(overrides.text || "").trim()
        : options.chatInput.value.trim();
    const queuedAttachments = useOverrideMessage ? [] : buildQueuedAttachmentPayload();
    const instructionExtraTextBlocks = overrides?.skipInstructionPrompts ? [] : buildInstructionExtraTextBlocks();
    const selectedMentions = Array.isArray(options.selectedMentions?.value)
      ? options.selectedMentions.value
        .map((item) => ({
          agentId: String(item.agentId || "").trim(),
          agentName: String(item.agentName || "").trim(),
          departmentId: String(item.departmentId || "").trim(),
          departmentName: String(item.departmentName || "").trim(),
          avatarUrl: String(item.avatarUrl || "").trim() || undefined,
        }))
        .filter((item) => !!item.agentId && !!item.departmentId)
      : [];
    const extraTextBlocks = [
      ...instructionExtraTextBlocks,
      ...(Array.isArray(overrides?.extraTextBlocks) ? overrides.extraTextBlocks : []),
    ].filter((item) => !!String(item || "").trim());
    const finalImages = useOverrideMessage ? [] : [...options.clipboardImages.value];
    if (!plainText && finalImages.length === 0 && queuedAttachments.length === 0 && extraTextBlocks.length === 0) return;
    const sendSession = options.getSession();
    if (!sendSession || !sendSession.apiConfigId || !sendSession.agentId) return;

    const hasForegroundRoundInFlight = options.chatting.value || round.phase !== "idle";
    if (!hasForegroundRoundInFlight) {
      clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
      options.toolStatusText.value = "";
      options.toolStatusState.value = "";
      if (options.streamToolCalls) options.streamToolCalls.value = [];
      options.chatErrorText.value = "";
    }

    const sentImages = finalImages;
    const attachments = [...queuedAttachments, ...buildImageAttachmentPayload(sentImages)];
    options.latestUserText.value = plainText;
    options.latestUserImages.value = sentImages.map((image) => ({
      mime: String(image.mime || ""),
      bytesBase64: String(image.bytesBase64 || ""),
    }));
    if (!useOverrideMessage) {
      options.chatInput.value = "";
      options.clipboardImages.value = [];
      if (options.queuedAttachmentNotices) options.queuedAttachmentNotices.value = [];
      if (options.selectedMentions) options.selectedMentions.value = [];
    }

    const gen = ++generation;
    sendChatActiveGen = gen;
    sendStartedAtMsByGen.set(gen, Date.now());
    console.warn("[聊天前端耗时] 发送开始", {
      gen,
      conversationId: String(options.getConversationId ? options.getConversationId() : "").trim(),
      textLength: plainText.length,
      imageCount: sentImages.length,
      attachmentCount: attachments.length,
      extraBlockCount: extraTextBlocks.length,
    });
    pendingTerminalEvent = null;
    if (!hasForegroundRoundInFlight) {
      insertUserDraft(gen, plainText, sentImages, attachments, selectedMentions);
      options.onOwnUserDraftInserted?.();
    }

    if (!hasForegroundRoundInFlight) {
      resetDisplayState();
      if (round.phase === "streaming") removeDraft(round.draftId);
      setRound({ phase: "queued", gen });
      updateQueuedAssistantDraftStatus(`${DRAFT_ASSISTANT_ID_PREFIX}${gen}`, options.t("chat.statusPreparingMessage"));
      // 注意：queued 阶段不应提前置 chatting=true。
      // 之前这里提前置 true，会让“未收到 history_flushed 前 UI 不应进入流式态”的测试失败。
    }

    const deltaChannel = new Channel<AssistantDeltaEvent>();
    attachDeltaHandler(deltaChannel, "sendChat", () => gen, () => gen);

    try {
      const result = await options.invokeSendChatMessage({
        text: plainText,
        displayText:
          overrides && typeof overrides.displayText === "string"
            ? overrides.displayText
            : plainText,
        images: sentImages,
        attachments: attachments.length > 0 ? attachments : undefined,
        extraTextBlocks: extraTextBlocks.length > 0 ? extraTextBlocks : undefined,
        mentions: selectedMentions.length > 0 ? selectedMentions : undefined,
        session: {
          ...sendSession,
          conversationId: options.getConversationId ? options.getConversationId() : "",
        },
        onDelta: deltaChannel,
      });

      const cur = options.getSession();
      if (!cur || cur.apiConfigId !== sendSession.apiConfigId || cur.agentId !== sendSession.agentId) return;

      // Promise fallback：delta 通道已处理过就跳过
      if ((round.phase === "streaming" || round.phase === "queued") && round.gen === gen) {
        await handleRoundCompleted(gen, {
          assistantText: String(result.assistantText || ""),
          reasoningStandard: result.reasoningStandard,
          reasoningInline: result.reasoningInline,
          assistantMessage: result.assistantMessage,
        });
      }
    } catch (error) {
      console.error("[聊天] 聊天流程请求失败", {
        action: "sendChat", apiConfigId: sendSession.apiConfigId, agentId: sendSession.agentId,
        gen, message: String((error as { message?: string })?.message ?? error ?? ""),
      });

      if (round.phase === "idle" || round.gen !== gen) {
        if (pendingUserDraftId === `${DRAFT_USER_ID_PREFIX}${gen}`) {
          removeDraft(pendingUserDraftId);
        }
        removeDraft(`${DRAFT_ASSISTANT_ID_PREFIX}${gen}`);
        sendStartedAtMsByGen.delete(gen);
        options.chatErrorText.value = options.formatRequestFailed(error);
        return;
      }

      options.latestAssistantText.value = "";
      options.latestReasoningStandardText.value = "";
      options.latestReasoningInlineText.value = "";
      options.chatErrorText.value = options.formatRequestFailed(error);
      if (!options.toolStatusText.value) {
        options.toolStatusState.value = "failed";
        options.toolStatusText.value = summarizeToolCallsText() || options.t("status.toolCallFailed");
      }

      const cur = options.getSession();
      if (cur && cur.apiConfigId === sendSession.apiConfigId && cur.agentId === sendSession.agentId) {
        if (round.phase === "streaming" && round.gen === gen) {
          removeDraft(round.draftId);
          if (pendingUserDraftId === `${DRAFT_USER_ID_PREFIX}${gen}`) {
            removeDraft(pendingUserDraftId);
          }
          sendStartedAtMsByGen.delete(gen);
          setRound({ phase: "idle" });
          options.chatting.value = false;
          reasoningStartedAtMs.value = 0;
        } else if (round.phase === "queued" && round.gen === gen) {
          await failQueuedRoundWithoutDraft(gen, error);
        }
      }
    } finally {
      if (sendChatActiveGen === gen) sendChatActiveGen = 0;
      // 仅在该轮次未收到 history_flushed 时，才执行 queued 兜底回收。
      // 否则可能与 handleHistoryFlushed 的 await 竞态，导致 draft 无法插入。
      if (round.phase === "queued" && round.gen === gen && historyFlushedReceivedGen !== gen) {
        removeDraft(`${DRAFT_ASSISTANT_ID_PREFIX}${gen}`);
        if (pendingUserDraftId === `${DRAFT_USER_ID_PREFIX}${gen}`) {
          removeDraft(pendingUserDraftId);
        }
        sendStartedAtMsByGen.delete(gen);
        setRound({ phase: "idle" });
        options.chatting.value = false;
        reasoningStartedAtMs.value = 0;
        await options.onReloadMessages();
      }
    }
  }

  async function stopChat() {
    // queued 也允许 stop：请求已发出但 UI 还没进入 streaming 时，仍需要可中断。
    // 之前只看 chatting.value 会把 queued stop 直接短路。
    if (!options.chatting.value && round.phase !== "queued") return;
    const stopSession = options.getSession();
    const cid = options.getConversationId ? options.getConversationId() : "";
    const partialAssistantText = options.latestAssistantText.value;
    const partialReasoningStandard = options.latestReasoningStandardText.value;
    const partialReasoningInline = options.latestReasoningInlineText.value;

    // queued 阶段：尚未进入流式，直接本地中断，不请求后端 stop。
    if (round.phase === "queued") {
      sendStartedAtMsByGen.delete(round.gen);
      ++generation;
      sendChatActiveGen = 0;
      pendingTerminalEvent = null;
      deferredRoundCompletion = null;
      removeDraft(`${DRAFT_ASSISTANT_ID_PREFIX}${round.gen}`);
      if (pendingUserDraftId) {
        removeDraft(pendingUserDraftId);
      }
      setRound({ phase: "idle" });
      options.chatting.value = false;
      reasoningStartedAtMs.value = 0;
      options.toolStatusState.value = "";
      options.toolStatusText.value = "";
      clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
      // 本地立即停的同时，异步通知后端中断正在排队/执行中的请求。
      if (stopSession && options.invokeStopChatMessage) {
        void options
          .invokeStopChatMessage({
            session: cid ? { ...stopSession, conversationId: cid } : stopSession,
            partialAssistantText,
            partialReasoningStandard,
            partialReasoningInline,
          })
          .catch((error) => {
            const et = error instanceof Error
              ? `${error.message}\n${error.stack || ""}`.trim()
              : (() => { try { return JSON.stringify(error); } catch { return String(error); } })();
            console.warn(`[聊天] queued 停止后端中断失败，apiConfigId=${stopSession.apiConfigId}，agentId=${stopSession.agentId}，错误=${et}`);
          });
      }
      return;
    }

    if (stopSession && options.invokeStopChatMessage) {
      try {
        const stopResult = await options.invokeStopChatMessage({
          session: cid ? { ...stopSession, conversationId: cid } : stopSession,
          partialAssistantText,
          partialReasoningStandard,
          partialReasoningInline,
        });
        const activeGen = round.phase === "streaming" ? round.gen : 0;
        if (activeGen > 0) {
          await handleRoundCompleted(activeGen, {
            assistantText: String(stopResult?.assistantText || partialAssistantText),
            reasoningStandard:
              typeof stopResult?.reasoningStandard === "string"
                ? stopResult.reasoningStandard
                : partialReasoningStandard,
            reasoningInline:
              typeof stopResult?.reasoningInline === "string"
                ? stopResult.reasoningInline
                : partialReasoningInline,
            assistantMessage: stopResult?.assistantMessage,
          });
        }
        // stop 成功后也要刷新一次历史，确保本地草稿态与后端持久化结果一致。
        // 对应测试期望：history_flushed 一次 + stop 成功后二次 reload。
        await options.onReloadMessages();
        return;
      } catch (error) {
        const et = error instanceof Error
          ? `${error.message}\n${error.stack || ""}`.trim()
          : (() => { try { return JSON.stringify(error); } catch { return String(error); } })();
        console.warn(`[聊天] 停止消息失败，apiConfigId=${stopSession.apiConfigId}，agentId=${stopSession.agentId}，len=${partialAssistantText.length}，错误=${et}`);
      }
    }

    // stop 失败时，回退本地中断，避免 UI 挂在 streaming 态。
    ++generation;
    sendChatActiveGen = 0;
    deferredRoundCompletion = null;
    if (pendingUserDraftId) {
      removeDraft(pendingUserDraftId);
    }
    if (round.phase === "streaming") {
      removeDraft(round.draftId);
    }
    setRound({ phase: "idle" });
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
    options.toolStatusState.value = "failed";
    options.toolStatusText.value = summarizeToolCallsText() || options.t("status.interrupted");
    clearConversationStreamCache(options.getConversationId ? options.getConversationId() : "");
    await options.onReloadMessages();
  }

  return {
    sendChat,
    stopChat,
    clearForegroundRoundState,
    freezeForegroundRoundState,
    resumeForegroundStreamingRound: ensureForegroundStreamingRound,
    bindActiveConversationStream,
    handleExternalStreamRebindRequired,
    handleExternalHistoryFlushed,
    handleExternalRoundCompleted,
    handleExternalRoundFailed,
    handleExternalAssistantDelta,
    frontendRoundPhase,
    reasoningStartedAtMs,
  };
}
