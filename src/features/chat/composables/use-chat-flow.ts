import { Channel } from "@tauri-apps/api/core";
import { ref, type Ref } from "vue";
import type { ChatMessage } from "../../../types/app";

// ---------------------------------------------------------------------------
// 1. 类型声明
// ---------------------------------------------------------------------------

export type AssistantDeltaEvent = {
  delta?: string;
  kind?: string;
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
  error: string;
};

type UseChatFlowOptions = {
  chatting: Ref<boolean>;
  forcingArchive: Ref<boolean>;
  getSession: () => { apiConfigId: string; agentId: string } | null;
  getConversationId?: () => string;
  chatInput: Ref<string>;
  clipboardImages: Ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>;
  queuedAttachmentNotices?: Ref<Array<{ id: string; fileName: string; relativePath: string; mime: string }>>;
  latestUserText: Ref<string>;
  latestUserImages: Ref<Array<{ mime: string; bytesBase64: string }>>;
  latestAssistantText: Ref<string>;
  latestReasoningStandardText: Ref<string>;
  latestReasoningInlineText: Ref<string>;
  toolStatusText: Ref<string>;
  toolStatusState: Ref<"running" | "done" | "failed" | "">;
  streamToolCalls?: Ref<Array<{ name: string; argsText: string }>>;
  chatErrorText: Ref<string>;
  allMessages: Ref<ChatMessage[]>;
  visibleMessageBlockCount: Ref<number>;
  t: (key: string, params?: Record<string, unknown>) => string;
  formatRequestFailed: (error: unknown) => string;
  removeBinaryPlaceholders: (text: string) => string;
  invokeSendChatMessage: (input: {
    text: string;
    displayText?: string;
    images: Array<{ mime: string; bytesBase64: string; savedPath?: string }>;
    attachments?: Array<{ fileName: string; relativePath: string; mime: string }>;
    extraTextBlocks?: string[];
    session: { apiConfigId: string; agentId: string; conversationId?: string };
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
    session: { apiConfigId: string; agentId: string; conversationId?: string };
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

export function useChatFlow(options: UseChatFlowOptions) {
  // ── 状态 ──
  let round: RoundState = { phase: "idle" };
  let generation = 0;
  let sendChatActiveGen = 0; // 防止 bound channel 抢占 sendChat 轮次
  let historyFlushedReceivedGen = 0; // 记录 sendChat 轮次是否已收到 history_flushed，避免 finally 误回收

  // ── 流式统计 ──
  let streamToolCallCount = 0;
  let streamLastToolName = "";

  let activeHistoryMessageCount = 0;
  const reasoningStartedAtMs = ref(0);

  // =========================================================================
  // 工具函数（纯逻辑，无副作用）
  // =========================================================================

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
      };
    } catch {
      return { conversationId: text, messageCount: 0, messages: [], activateAssistant: false };
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
      return { error: String(parsed.error || "") };
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

  // =========================================================================
  // Draft 操作 —— 唯一允许写 allMessages 的地方
  //
  // insertDraft: history_flushed 时插入空气泡
  // updateDraftText: 流式期间把 latestAssistantText 同步到气泡
  // removeDraft: history_flushed 清屏时移除上一轮残留
  // =========================================================================

  function insertDraft(gen: number): string {
    const draftId = `${DRAFT_ASSISTANT_ID_PREFIX}${gen}`;
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
      },
    };
    const cur = options.allMessages.value;
    const idx = cur.findIndex((m) => m.id === draftId);
    options.allMessages.value = idx < 0 ? [...cur, msg] : cur.map((m, i) => (i === idx ? msg : m));
    return draftId;
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

  function updateDraftText(draftId: string, streamSegments?: string[], streamTail?: string) {
    if (!draftId) return;
    const agentId = String(options.getSession()?.agentId || "").trim();
    const nextStreamSegments = streamSegments || readDraftStreamSegments(draftId);
    const nextStreamTail = streamTail ?? readDraftStreamTail(draftId);
    const msg: ChatMessage = {
      id: draftId,
      role: "assistant",
      createdAt: new Date().toISOString(),
      speakerAgentId: agentId || "assistant-draft",
      parts: [{ type: "text", text: String(options.latestAssistantText.value || "") }],
      providerMeta: {
        reasoningStandard: String(options.latestReasoningStandardText.value || ""),
        reasoningInline: String(options.latestReasoningInlineText.value || ""),
        _streaming: true,
        _streamSegments: nextStreamSegments,
        _streamTail: nextStreamTail,
      },
    };
    const cur = options.allMessages.value;
    const idx = cur.findIndex((m) => m.id === draftId);
    options.allMessages.value = idx < 0 ? [...cur, msg] : cur.map((m, i) => (i === idx ? msg : m));
  }

  function removeDraft(draftId: string) {
    if (!draftId) return;
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

  function clearStreamBuffer() {
    // 兼容保留：当前改为“delta 直写”，无额外缓冲需要清理。
  }

  function enqueueStreamDelta(gen: number, delta: string) {
    if (round.phase !== "streaming" || round.gen !== gen || !delta) return;
    options.latestAssistantText.value += delta;
    const currentSegments = readDraftStreamSegments(round.draftId);
    const currentTail = readDraftStreamTail(round.draftId);
    const parsed = consumeClosedMarkdownBlocks(`${currentTail}${delta}`);
    const nextStreamSegments = parsed.chunks.length > 0
      ? [...currentSegments, ...parsed.chunks]
      : currentSegments;
    updateDraftText(round.draftId, nextStreamSegments, parsed.tail);
  }

  // =========================================================================
  // 显示状态重置（只在 history_flushed 清屏时调用）
  // =========================================================================

  function resetDisplayState() {
    clearStreamBuffer();
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

  // =========================================================================
  // 事件处理
  // =========================================================================

  /**
   * history_flushed：唯一做 allMessages 大规模变更的地方。
   * 1. 移除旧 draft   2. reload / onHistoryFlushed   3. 插入新 draft
   * 之后不再碰 allMessages（除了 updateDraftText）。
   */
  async function handleHistoryFlushed(
    gen: number,
    parsed: AssistantDeltaEvent,
    source: "sendChat" | "bound",
  ) {
    const flushed = readHistoryFlushedPayload(parsed.message);
    const shouldActivate = source === "sendChat" || !!flushed?.activateAssistant;
    console.info("[CHAT_TRACE][history_flushed] 开始", {
      source,
      gen,
      sendChatActiveGen,
      shouldActivate,
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
      });
      return;
    }
    const replayMessages = Array.isArray(flushed?.messages) ? flushed!.messages : [];
    console.info("[CHAT_TRACE][history_flushed] 完成", {
      source,
      gen,
      shouldActivate,
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
    if (shouldActivate) {
      options.visibleMessageBlockCount.value = batchVisibleCount;
      // 仅激活助理时清屏，非激活批次保持当前视图并顺序追加历史。
      const oldDraftId = round.phase === "streaming" ? round.draftId : "";
      resetDisplayState();
      if (oldDraftId) removeDraft(oldDraftId);
      round = { phase: "queued", gen };
    }

    // ── reload ──
    if (options.onHistoryFlushed) {
      console.info("[CHAT_TRACE][history_flushed] apply_start", {
        source,
        gen,
        shouldActivate,
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
        batchVisibleCount,
      });
    } else {
      await options.onReloadMessages();
    }

    if (!shouldActivate) {
      // await 期间可能有新的 sendChat/轮次启动，避免回写旧状态覆盖新轮次
      if (gen !== generation) return;
      round = { phase: "idle" };
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

    // ── 插 draft / 进入 streaming ──
    const draftId = insertDraft(gen);
    // 流式进行中保持历史可见窗口稳定，待 round_completed 再按最终可见输出结算。
    options.visibleMessageBlockCount.value = batchVisibleCount;
    round = { phase: "streaming", gen, draftId };
    options.chatting.value = true;
  }

  /**
   * round_completed：终结当前轮次。
   * 只做文字收尾 + 状态转换，不碰 allMessages（除了 updateDraftText）。
   */
  function handleRoundCompleted(
    gen: number,
    result: {
      assistantText: string;
      reasoningStandard?: string;
      reasoningInline?: string;
      assistantMessage?: ChatMessage;
    },
  ) {
    if (round.phase !== "streaming" || round.gen !== gen) return;
    const { draftId } = round;

    // 对齐最终文本（delta 直写后只做最终兜底对齐）
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

    options.visibleMessageBlockCount.value =
      activeHistoryMessageCount + (hasAssistantVisibleOutput(result) ? 1 : 0);

    updateDraftText(draftId);
    finalizeDraft(draftId, result.assistantMessage);
    round = { phase: "idle" };
    clearStreamBuffer();
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
  }

  function handleRoundFailed(gen: number, error: unknown) {
    if (round.phase !== "streaming" || round.gen !== gen) return;
    const { draftId } = round;

    clearStreamBuffer();
    options.latestAssistantText.value = "";
    options.latestReasoningStandardText.value = "";
    options.latestReasoningInlineText.value = "";
    options.chatErrorText.value = options.formatRequestFailed(error);
    if (!options.toolStatusText.value) {
      options.toolStatusState.value = "failed";
      options.toolStatusText.value = summarizeToolCallsText() || options.t("status.toolCallFailed");
    }
    options.visibleMessageBlockCount.value = activeHistoryMessageCount;
    removeDraft(draftId);
    round = { phase: "idle" };
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
  }

  // =========================================================================
  // Delta 分发
  // =========================================================================

  function handleStreamingEvent(currentGen: number, parsed: AssistantDeltaEvent) {
    if (!currentGen) return;
    if (round.phase !== "streaming") return;
    if (round.gen !== currentGen) return;

    if (parsed.kind === "round_completed") {
      const p = readRoundCompletedPayload(parsed.message);
      handleRoundCompleted(currentGen, {
        assistantText: String(p?.assistantText || ""),
        reasoningStandard: p?.reasoningStandard,
        reasoningInline: p?.reasoningInline,
        assistantMessage: p?.assistantMessage,
      });
      return;
    }
    if (parsed.kind === "round_failed") {
      const p = readRoundFailedPayload(parsed.message);
      handleRoundFailed(currentGen, p?.error || parsed.message || JSON.stringify(parsed));
      return;
    }

    if (parsed.kind === "tool_status") {
      const toolName = String(parsed.toolName || "").trim();
      if (parsed.toolStatus === "running" && toolName) {
        streamToolCallCount += 1;
        streamLastToolName = toolName;
        if (options.streamToolCalls) {
          options.streamToolCalls.value = [
            ...options.streamToolCalls.value,
            { name: toolName, argsText: String(parsed.toolArgs || "").trim() },
          ];
        }
      }
      options.toolStatusText.value = parsed.message || "";
      options.toolStatusState.value =
        parsed.toolStatus === "running" || parsed.toolStatus === "done" || parsed.toolStatus === "failed"
          ? parsed.toolStatus : "";
      return;
    }
    if (parsed.kind === "reasoning_standard") {
      const dt = readDeltaMessage(parsed);
      if (dt && reasoningStartedAtMs.value === 0) reasoningStartedAtMs.value = Date.now();
      options.latestReasoningStandardText.value += dt;
      updateDraftText(round.draftId);
      return;
    }
    if (parsed.kind === "reasoning_inline") {
      const dt = readDeltaMessage(parsed);
      if (dt && reasoningStartedAtMs.value === 0) reasoningStartedAtMs.value = Date.now();
      options.latestReasoningInlineText.value += dt;
      updateDraftText(round.draftId);
      return;
    }

    enqueueStreamDelta(currentGen, readDeltaMessage(parsed));
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

  // =========================================================================
  // Bound channel
  // =========================================================================

  let boundConversationId = "";
  let boundConversationInitialized = false;
  let boundDisplayGeneration = 0;
  const boundDeltaChannel = new Channel<AssistantDeltaEvent>();
  attachDeltaHandler(
    boundDeltaChannel,
    "bound",
    () => boundDisplayGeneration,
    () => { boundDisplayGeneration = ++generation; return boundDisplayGeneration; },
  );

  async function bindActiveConversationStream(conversationId: string, force = false) {
    if (!options.invokeBindActiveChatViewStream) return;
    const id = String(conversationId || "").trim();
    if (!force && boundConversationInitialized && id === boundConversationId) return;
    await options.invokeBindActiveChatViewStream({ conversationId: id || undefined, onDelta: boundDeltaChannel });
    boundConversationId = id;
    boundConversationInitialized = true;
    if (!id) boundDisplayGeneration = 0;
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
    if (round.phase !== "streaming") return;
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
    if (round.phase !== "streaming") return;
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
    handleRoundFailed(round.gen, parsed?.error || raw || String(raw));
  }

  async function handleExternalAssistantDelta(payload: unknown) {
    const rawObj = payload && typeof payload === "object" ? payload as Record<string, unknown> : null;
    const currentConversationId = String(options.getConversationId ? options.getConversationId() : "").trim();
    const payloadConversationId = String(rawObj?.conversationId || "").trim();
    if (currentConversationId && payloadConversationId && currentConversationId !== payloadConversationId) {
      return;
    }
    const parsed = readAssistantEvent(rawObj?.event ?? payload);
    const currentGen = round.phase === "streaming" ? round.gen : 0;
    handleStreamingEvent(currentGen, parsed);
  }

  // =========================================================================
  // 公共方法
  // =========================================================================

  async function sendChat() {
    const plainText = options.chatInput.value.trim();
    const attachments = buildQueuedAttachmentPayload();
    if (!plainText && options.clipboardImages.value.length === 0 && attachments.length === 0) return;
    const sendSession = options.getSession();
    if (!sendSession || !sendSession.apiConfigId || !sendSession.agentId) return;

    const wasChatting = options.chatting.value;
    options.toolStatusText.value = "";
    options.toolStatusState.value = "";
    if (options.streamToolCalls) options.streamToolCalls.value = [];
    options.chatErrorText.value = "";

    const sentImages = [...options.clipboardImages.value];
    options.chatInput.value = "";
    options.clipboardImages.value = [];
    if (options.queuedAttachmentNotices) options.queuedAttachmentNotices.value = [];

    const gen = ++generation;
    sendChatActiveGen = gen;

    if (!wasChatting) {
      resetDisplayState();
      if (round.phase === "streaming") removeDraft(round.draftId);
      round = { phase: "queued", gen };
      // 注意：queued 阶段不应提前置 chatting=true。
      // 之前这里提前置 true，会让“未收到 history_flushed 前 UI 不应进入流式态”的测试失败。
    }

    const deltaChannel = new Channel<AssistantDeltaEvent>();
    attachDeltaHandler(deltaChannel, "sendChat", () => gen, () => gen);

    try {
      const result = await options.invokeSendChatMessage({
        text: plainText,
        displayText: plainText,
        images: sentImages,
        attachments: attachments.length > 0 ? attachments : undefined,
        session: {
          ...sendSession,
          conversationId: options.getConversationId ? options.getConversationId() : "",
        },
        onDelta: deltaChannel,
      });

      const cur = options.getSession();
      if (!cur || cur.apiConfigId !== sendSession.apiConfigId || cur.agentId !== sendSession.agentId) return;

      // Promise fallback：delta 通道已处理过就跳过
      if (round.phase === "streaming" && round.gen === gen) {
        handleRoundCompleted(gen, {
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
        options.chatErrorText.value = options.formatRequestFailed(error);
        return;
      }

      clearStreamBuffer();
      options.latestAssistantText.value = "";
      options.latestReasoningStandardText.value = "";
      options.latestReasoningInlineText.value = "";
      options.chatErrorText.value = options.formatRequestFailed(error);
      if (!options.toolStatusText.value) {
        options.toolStatusState.value = "failed";
        options.toolStatusText.value = summarizeToolCallsText() || options.t("status.toolCallFailed");
      }

      const cur = options.getSession();
      if (cur && cur.apiConfigId === sendSession.apiConfigId && cur.agentId === sendSession.agentId
          && round.phase === "streaming" && round.gen === gen) {
        removeDraft(round.draftId);
        round = { phase: "idle" };
        options.chatting.value = false;
        reasoningStartedAtMs.value = 0;
        options.visibleMessageBlockCount.value = activeHistoryMessageCount;
      }
    } finally {
      if (sendChatActiveGen === gen) sendChatActiveGen = 0;
      // 仅在该轮次未收到 history_flushed 时，才执行 queued 兜底回收。
      // 否则可能与 handleHistoryFlushed 的 await 竞态，导致 draft 无法插入。
      if (round.phase === "queued" && round.gen === gen && historyFlushedReceivedGen !== gen) {
        round = { phase: "idle" };
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
      ++generation;
      sendChatActiveGen = 0;
      clearStreamBuffer();
      round = { phase: "idle" };
      options.chatting.value = false;
      reasoningStartedAtMs.value = 0;
      options.toolStatusState.value = "";
      options.toolStatusText.value = "";
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
          handleRoundCompleted(activeGen, {
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
    clearStreamBuffer();
    if (round.phase === "streaming") {
      removeDraft(round.draftId);
    }
    round = { phase: "idle" };
    options.chatting.value = false;
    reasoningStartedAtMs.value = 0;
    options.toolStatusState.value = "failed";
    options.toolStatusText.value = summarizeToolCallsText() || options.t("status.interrupted");
    await options.onReloadMessages();
  }

  return {
    sendChat,
    stopChat,
    bindActiveConversationStream,
    handleExternalHistoryFlushed,
    handleExternalRoundCompleted,
    handleExternalRoundFailed,
    handleExternalAssistantDelta,
    clearStreamBuffer,
    reasoningStartedAtMs,
  };
}
