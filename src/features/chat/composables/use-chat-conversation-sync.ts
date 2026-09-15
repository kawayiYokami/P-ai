import { nextTick } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import {
  carryStreamingProjection,
  readConversationIdFromPayload,
  readMessagesFromPayload,
  useChatConversationMessageUtils,
} from "./use-chat-conversation-message-utils";
import { useChatConversationOverviewUtils } from "./use-chat-conversation-overview-utils";
import { applyStreamingHistoryOverlay } from "./use-chat-flow-stream-overlay";

type ForegroundPaintTrace = {
  id: number;
  conversationId: string;
  startedAt: number;
};

export function useChatConversationSync(bindings: Record<string, any>) {
  let foregroundPaintTraceSeq = 0;
  let foregroundRuntimeResumeSeq = 0;
  const {
    areMessagesEquivalent,
    formalizeConversationMessages,
    freezeConversationMessages,
    insertMessagesBeforeStreamingAssistantProjection,
    mergeMessagesIntoTimeline,
    messageContentSignature,
    replaceConversationMessage,
    replaceConversationHistory,
    reuseStableMessageReferences,
  } = useChatConversationMessageUtils({
    ensureConversationMessageIds: bindings.ensureConversationMessageIds,
  });
  const {
    sortUnarchivedConversationOverviewItems,
    unarchivedConversationActivityAt,
  } = useChatConversationOverviewUtils();

  function matchesForegroundConversation(conversationId?: string | null): boolean {
    const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (!currentConversationId) return false;
    const targetConversationId = String(conversationId || "").trim();
    return !targetConversationId || targetConversationId === currentConversationId;
  }

  function currentConversationRuntimeState(conversationId?: string | null) {
    const cid = String(conversationId || "").trim();
    if (!cid) return "";
    return String(
      bindings.unarchivedConversations.value.find((item: any) => String(item.conversationId || "").trim() === cid)?.runtimeState || "",
    ).trim();
  }

  function maybeResumeForegroundStreamingBubble(conversationId?: string | null, reason = "unknown") {
    void conversationId;
    void reason;
  }

  function conversationRuntimeSnapshotIsBusy(snapshot?: any): boolean {
    if (!snapshot) return false;
    // runtimeState/isProcessing/pendingQueue 才表示仍有运行中轮次。
    // streamCache 只是运行中恢复显示用的投影缓存；完成收尾期间它可能短暂残留，
    // 不能单独把前台状态重新拉回 chatting=true。
    return snapshot.runtimeState === "assistant_streaming"
      || !!snapshot.isProcessing
      || !!snapshot.hasPendingQueue
      || Math.max(0, Number(snapshot.pendingQueueCount || 0)) > 0;
  }

  async function requestConversationRuntimeSnapshot(conversationId: string) {
    return invokeTauri<any>("conversation.runtimeSnapshot", {
      conversationId,
    });
  }

  async function resumeForegroundRuntimeFromBackend(conversationId?: string | null, reason = "unknown") {
    void conversationId;
    void reason;
    return "disabled";
  }

  function beginForegroundPaintTrace(conversationId: string): ForegroundPaintTrace {
    return {
      id: ++foregroundPaintTraceSeq,
      conversationId: String(conversationId || "").trim(),
      startedAt: bindings.perfNow(),
    };
  }

  function logForegroundPaintTrace(
    trace: ForegroundPaintTrace,
    label: string,
    detail?: Record<string, unknown>,
  ) {
    void trace;
    void label;
    void detail;
  }

  function cacheConversationMessages(conversationId: string, messages: any[]) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    const cachedMessages = freezeConversationMessages(Array.isArray(messages) ? messages : []);
    bindings.conversationMessageCache.value = {
      ...bindings.conversationMessageCache.value,
      [cid]: cachedMessages.slice(-bindings.BACKGROUND_CONVERSATION_CACHE_LIMIT),
    };
  }

  function inferHasMoreHistoryFromSnapshot(messages: any[]): boolean {
    return Array.isArray(messages) && messages.length >= bindings.BACKGROUND_CONVERSATION_CACHE_LIMIT;
  }

  function clearConversationBadge(conversationId: string) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    const hasBackgroundBadge = !!bindings.backgroundConversationBadgeMap.value[cid];
    if (hasBackgroundBadge) {
      const next = { ...bindings.backgroundConversationBadgeMap.value };
      delete next[cid];
      bindings.backgroundConversationBadgeMap.value = next;
    }
    let changed = false;
    const nextItems = bindings.unarchivedConversations.value.map((item: any) => {
      if (String(item.conversationId || "").trim() !== cid) return item;
      if (Math.max(0, Number(item.unreadCount || 0)) <= 0) return item;
      changed = true;
      return {
        ...item,
        unreadCount: 0,
      };
    });
    if (changed) {
      bindings.unarchivedConversations.value = nextItems;
    }
  }

  function markConversationReadPersisted(conversationId: string) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    void invokeTauri("conversation.markRead", {
      input: { conversationId: cid },
    }).catch((error) => {
      console.warn("[会话已读] 持久化失败", {
        conversationId: cid,
        error,
      });
    });
  }

  function setConversationBadge(conversationId: string, status: string) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    bindings.backgroundConversationBadgeMap.value = {
      ...bindings.backgroundConversationBadgeMap.value,
      [cid]: status,
    };
  }

  function mergeIncomingMessagesIntoCache(conversationId: string, messages: any[]) {
    const cid = String(conversationId || "").trim();
    if (!cid || !Array.isArray(messages) || messages.length <= 0) return;
    const incoming = messages.filter((message) => !!String(message?.id || "").trim());
    if (incoming.length <= 0) return;
    const cachedDisplay = freezeConversationMessages(bindings.conversationMessageCache.value[cid] || []);
    const cachedFormal = formalizeConversationMessages(cachedDisplay);
    const nextCached = mergeMessagesIntoTimeline(cachedFormal, incoming);
    cacheConversationMessages(cid, nextCached);
  }

  function buildConversationMessagesAfterAnchor(conversationId: string): string | null {
    const cid = String(conversationId || "").trim();
    if (!cid) return null;
    // 前台会话以当前 allMessages 为准，避免 stop 后仍用过期 cache 当 after 锚点。
    const isForeground = String(bindings.currentChatConversationId.value || "").trim() === cid;
    const sourceMessages = isForeground
      ? bindings.allMessages.value
      : (bindings.conversationMessageCache.value[cid] || []);
    const cachedDisplay = freezeConversationMessages(sourceMessages);
    const cachedFormal = formalizeConversationMessages(cachedDisplay);
    const lastFormalMessageId = String(cachedFormal[cachedFormal.length - 1]?.id || "").trim();
    return lastFormalMessageId || null;
  }

  async function requestConversationMessagesAfterAsync(conversationId: string, trace?: ForegroundPaintTrace) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    const afterMessageId = buildConversationMessagesAfterAnchor(cid);
    if (trace) {
      logForegroundPaintTrace(trace, "开始请求后台异步补消息", {
        afterMessageId: afterMessageId || "",
      });
    }
    await invokeTauri("conversation.messagesAfterAsync", {
      input: {
        conversationId: cid,
        afterMessageId,
        fallbackLimit: bindings.BACKGROUND_CONVERSATION_CACHE_LIMIT,
      },
    });
  }

  async function requestConversationMessageById(conversationId: string, messageId: string) {
    return invokeTauri("conversation.messageById", {
      input: {
        conversationId,
        messageId,
      },
    });
  }

  async function reloadForegroundConversationMessages(reason = "unknown") {
    const conversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (!conversationId) {
      await bindings.loadAllMessages();
      return;
    }
    try {
      await requestConversationMessagesAfterAsync(conversationId);
    } catch (error) {
      console.warn("[会话缓存] 增量补消息失败，回退全量加载", {
        reason,
        conversationId,
        error,
      });
      await bindings.loadAllMessages();
    }
  }

  async function refreshForegroundConversationMessageById(payload: { conversationId: string; messageId: string }) {
    const conversationId = String(payload?.conversationId || "").trim();
    const messageId = String(payload?.messageId || "").trim();
    if (!conversationId || !messageId) return;
    try {
      const refreshedMessage = freezeConversationMessages([
        await requestConversationMessageById(conversationId, messageId),
      ])[0];
      if (!refreshedMessage) return;

      const cachedDisplay = freezeConversationMessages(bindings.conversationMessageCache.value[conversationId] || []);
      const nextCached = replaceConversationMessage(cachedDisplay, refreshedMessage);
      if (nextCached !== cachedDisplay) {
        cacheConversationMessages(conversationId, nextCached);
      }

      if (String(bindings.currentChatConversationId.value || "").trim() !== conversationId) {
        return;
      }
      const nextMessages = replaceConversationMessage(bindings.allMessages.value, refreshedMessage);
      if (nextMessages === bindings.allMessages.value) {
        return;
      }
      bindings.allMessages.value = nextMessages;
      cacheConversationMessages(conversationId, nextMessages);
    } catch (error) {
      console.warn("[会话缓存] 单条消息刷新失败", {
        conversationId,
        messageId,
        error,
      });
    }
  }

  async function loadOlderConversationHistory() {
    const conversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (!conversationId || bindings.loadingOlderConversationHistory.value || !bindings.hasMoreBackendHistory.value) {
      return;
    }
    const formalMessages = formalizeConversationMessages(bindings.allMessages.value);
    const oldestMessageId = String(formalMessages[0]?.id || "").trim();
    if (!oldestMessageId) {
      bindings.hasMoreBackendHistory.value = false;
      return;
    }

    bindings.loadingOlderConversationHistory.value = true;
    try {
      const result = await invokeTauri("conversation.messagesBefore", {
        input: {
          conversationId,
          beforeMessageId: oldestMessageId,
          limit: bindings.OLDER_HISTORY_PAGE_SIZE,
        },
      }) as { messages?: any[]; hasMore?: boolean };
      if (
        String(bindings.currentChatConversationId.value || "").trim() !== conversationId
      ) {
        return;
      }
      const previousMessages = Array.isArray(bindings.allMessages.value) ? bindings.allMessages.value : [];
      const incomingMessages = freezeConversationMessages(Array.isArray(result?.messages) ? result.messages : []);
      const nextMessages = mergeMessagesIntoTimeline(previousMessages, incomingMessages, {
        prependMessages: true,
      });
      bindings.allMessages.value = nextMessages;
      cacheConversationMessages(conversationId, nextMessages);
      bindings.hasMoreBackendHistory.value = !!result?.hasMore;
    } catch (error) {
      console.warn("[会话缓存] 向上补历史失败", {
        conversationId,
        error,
      });
      bindings.setStatusError("status.loadMessagesFailed", error);
    } finally {
      bindings.loadingOlderConversationHistory.value = false;
    }
  }

  // 时间线按压缩段补载：一次取锚点之前的一整段（服务端按压缩边界切块，一段即一个物理块文件）。
  async function loadOlderCompactionSegmentHistory() {
    const conversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (!conversationId || bindings.loadingOlderConversationHistory.value || !bindings.hasMoreBackendHistory.value) {
      return;
    }
    const formalMessages = formalizeConversationMessages(bindings.allMessages.value);
    const oldestMessageId = String(formalMessages[0]?.id || "").trim();
    if (!oldestMessageId) {
      bindings.hasMoreBackendHistory.value = false;
      return;
    }

    bindings.loadingOlderConversationHistory.value = true;
    try {
      const result = await invokeTauri("conversation.compactionSegmentBefore", {
        input: { conversationId, anchorMessageId: oldestMessageId },
      }) as { messages?: any[]; hasMore?: boolean };
      if (
        String(bindings.currentChatConversationId.value || "").trim() !== conversationId
      ) {
        return;
      }
      const previousMessages = Array.isArray(bindings.allMessages.value) ? bindings.allMessages.value : [];
      const incomingMessages = freezeConversationMessages(Array.isArray(result?.messages) ? result.messages : []);
      if (incomingMessages.length > 0) {
        const nextMessages = mergeMessagesIntoTimeline(previousMessages, incomingMessages, {
          prependMessages: true,
        });
        bindings.allMessages.value = nextMessages;
        cacheConversationMessages(conversationId, nextMessages);
      }
      bindings.hasMoreBackendHistory.value = !!result?.hasMore;
    } catch (error) {
      console.warn("[会话缓存] 按压缩段向上补历史失败", {
        conversationId,
        error,
      });
      bindings.setStatusError("status.loadMessagesFailed", error);
    } finally {
      bindings.loadingOlderConversationHistory.value = false;
    }
  }

  function mergeConversationMessagesFromSyncPayload(
    conversationId: string,
    payloadMessages: any[],
    fallbackMode?: string | null,
    options?: { baseMessages?: any[] },
  ) {
    const cid = String(conversationId || "").trim();
    const nextPayloadMessages = freezeConversationMessages(Array.isArray(payloadMessages) ? payloadMessages : []);
    // 前台以当前 allMessages 为底；后台才回落到 conversationMessageCache。
    // 不能只用过期 cache，否则 stop 冻结的正文会被整表替换冲掉。
    const baseSource = Array.isArray(options?.baseMessages)
      ? options.baseMessages
      : (bindings.conversationMessageCache.value[cid] || []);
    const baseDisplay = freezeConversationMessages(baseSource);
    const fallback = String(fallbackMode || "").trim();
    if (fallback === "recent_limit") {
      // recent 页也只做合并，不整表替换，避免盖掉本地已有可见内容。
      const recentMerged = mergeMessagesIntoTimeline(baseDisplay, nextPayloadMessages);
      const recentBase = recentMerged.length > 0 ? recentMerged : baseDisplay;
      return reuseStableMessageReferences(
        carryStreamingProjection(baseSource, recentBase),
        baseDisplay,
      );
    }
    const nextMerged = mergeMessagesIntoTimeline(baseDisplay, nextPayloadMessages);
    const fallbackMerged = nextMerged.length > 0 ? nextMerged : baseDisplay;
    return reuseStableMessageReferences(carryStreamingProjection(baseSource, fallbackMerged), baseDisplay);
  }

  async function applyConversationMessagesAfterSynced(payload: Record<string, any>) {
    const conversationId = String(payload?.conversationId || "").trim();
    const requestId = String(payload?.requestId || "").trim();
    if (!conversationId) return;
    if (payload?.error) {
      console.warn("[会话缓存] 异步补消息失败", {
        conversationId,
        requestId,
        error: payload.error,
      });
      if (
        requestId
        && requestId === bindings.getPendingManualScrollToBottomRequestId()
        && conversationId === bindings.getPendingManualScrollToBottomConversationId()
      ) {
        bindings.clearPendingManualScrollToBottom();
      }
      return;
    }
    const isForeground = String(bindings.currentChatConversationId.value || "").trim() === conversationId;
    const nextMessages = mergeConversationMessagesFromSyncPayload(
      conversationId,
      Array.isArray(payload?.messages) ? payload.messages : [],
      payload?.fallbackMode ?? null,
      {
        baseMessages: isForeground
          ? bindings.allMessages.value
          : (bindings.conversationMessageCache.value[conversationId] || []),
      },
    );
    cacheConversationMessages(conversationId, nextMessages);
    if (isForeground) {
      if (!areMessagesEquivalent(bindings.allMessages.value, nextMessages)) {
        bindings.allMessages.value = nextMessages;
      }
      bindings.foregroundTailLatestReady.value = true;
      await nextTick();
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      if (
        requestId
        && requestId === bindings.getPendingManualScrollToBottomRequestId()
        && conversationId === bindings.getPendingManualScrollToBottomConversationId()
      ) {
        bindings.clearPendingManualScrollToBottom();
        bindings.triggerConversationScrollToBottom(conversationId, "manual_after_synced", "manual");
      }
    }
  }

  function applyConversationMessageAppended(payload?: Record<string, any> | null) {
    const conversationId = String(payload?.conversationId || "").trim();
    const message = payload?.message || null;
    const messageId = String(message?.id || "").trim();
    if (!conversationId || !message || !messageId) return;

    const cachedDisplay = freezeConversationMessages(bindings.conversationMessageCache.value[conversationId] || []);
    const cachedFormal = formalizeConversationMessages(cachedDisplay);
    const nextCached = mergeMessagesIntoTimeline(cachedFormal, [message]);
    cacheConversationMessages(conversationId, nextCached);

    const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (conversationId !== currentConversationId) {
      return;
    }

    bindings.allMessages.value = mergeMessagesIntoTimeline(bindings.allMessages.value, [message]);
    bindings.foregroundTailLatestReady.value = true;
  }

  function applyConversationSnapshot(
    snapshot: Record<string, any>,
    options?: { preserveExistingHistory?: boolean },
  ) {
    const nextConversationId = String(snapshot.conversationId || "").trim();
    const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
    const preserveExistingHistory =
      !!options?.preserveExistingHistory
      && !!nextConversationId
      && nextConversationId === currentConversationId;
    const runtimeState = String(snapshot.runtimeState || "").trim();
    const resumeProjectionAuthoritative = !!snapshot.resumeProjectionAuthoritative;
    const streamCache = bindings.readConversationStreamCache(nextConversationId);
    const shouldApplyStreamingOverlay =
      runtimeState === "assistant_streaming" && !resumeProjectionAuthoritative;
    let rawNextMessages = freezeConversationMessages(Array.isArray(snapshot.messages) ? snapshot.messages : []);
    const overlay = shouldApplyStreamingOverlay
      ? applyStreamingHistoryOverlay(rawNextMessages, streamCache)
      : { messages: rawNextMessages, replacedMessageId: "", removed: false };
    rawNextMessages = overlay.messages;
    const mergedMessages = preserveExistingHistory
      ? mergeMessagesIntoTimeline(formalizeConversationMessages(bindings.allMessages.value), rawNextMessages)
      : replaceConversationHistory(bindings.allMessages.value, rawNextMessages);
    // 保留历史分支是「同会话重放」，正在流式的投影不能被持久化副本抹掉。
    const projectedMessages = preserveExistingHistory
      ? carryStreamingProjection(bindings.allMessages.value, mergedMessages)
      : mergedMessages;
    const nextMessages = reuseStableMessageReferences(projectedMessages, bindings.allMessages.value);
    bindings.currentChatConversationId.value = nextConversationId;
    bindings.currentChatPreferredApiConfigId.value = String(snapshot.preferredApiConfigId || "").trim();
    bindings.currentChatTodos.value = Array.isArray(snapshot.currentTodos)
      ? snapshot.currentTodos
        .map((item: any) => ({
          content: String(item?.content || "").trim(),
          status: String(item?.status || "").trim(),
        }))
        .filter((item: any) => item.content && (item.status === "pending" || item.status === "in_progress" || item.status === "completed"))
      : [];
    bindings.allMessages.value = nextMessages;
    bindings.hasMoreBackendHistory.value = !!snapshot.hasMoreHistory;
    bindings.foregroundTailLatestReady.value = true;
    cacheConversationMessages(nextConversationId, nextMessages);
    clearConversationBadge(nextConversationId);
    if (snapshot.conversation) {
      applyConversationOverviewItemUpdated({ conversation: snapshot.conversation });
    }
  }

  function applyConversationTodosUpdated(payload?: Record<string, any> | null) {
    const conversationId = String(payload?.conversationId || "").trim();
    if (!conversationId) return;
    const nextTodos = Array.isArray(payload?.currentTodos)
      ? payload.currentTodos
        .map((item: any) => ({
          content: String(item?.content || "").trim(),
          status: String(item?.status || "").trim(),
        }))
        .filter((item: any) => item.content && (item.status === "pending" || item.status === "in_progress" || item.status === "completed"))
      : [];
    if (conversationId === String(bindings.currentChatConversationId.value || "").trim()) {
      bindings.currentChatTodos.value = nextTodos;
    }
    const nextCurrentTodo = String(payload?.currentTodo || "").trim();
    bindings.unarchivedConversations.value = bindings.unarchivedConversations.value.map((item: any) =>
      String(item.conversationId || "").trim() === conversationId
        ? {
          ...item,
          currentTodo: nextCurrentTodo,
          currentTodos: nextTodos,
        }
        : item
    );
  }

  function applyConversationOverviewUpdated(payload?: Record<string, any> | null) {
    if (!Array.isArray(payload?.unarchivedConversations)) return;
    bindings.unarchivedConversations.value = payload.unarchivedConversations;
    syncOverviewConversationErrors(payload.unarchivedConversations);
    const serverTime = String(payload?.serverTime || "").trim();
    if (serverTime && bindings.lastOverviewSyncAt) {
      bindings.lastOverviewSyncAt.value = serverTime;
    }
  }

  function syncOverviewConversationErrors(items: Record<string, any>[]) {
    if (typeof bindings.setConversationChatErrorText !== "function") return;
    for (const item of items) {
      const conversationId = String(item?.conversationId || "").trim();
      if (!conversationId) continue;
      bindings.setConversationChatErrorText(
        conversationId,
        String(item?.lastError || "").trim(),
      );
    }
  }

  function conversationOverviewItemSignature(item: Record<string, any>): string {
    return [
      String(item.conversationId || "").trim(),
      // 切换人格只改这个字段，不带动 updatedAt；漏判会把这类概览项更新当成无变化丢弃
      String(item.agentId || "").trim(),
      String(item.updatedAt || "").trim(),
      String(item.lastMessageAt || "").trim(),
      String(item.runtimeState || "").trim(),
      String(item.title || "").trim(),
      String(item.summary || "").trim(),
      String(item.currentTodo || "").trim(),
      String(item.lastError || "").trim(),
      Math.max(0, Number(item.unreadCount || 0)),
      !!item.isPinned,
      !!item.isSystemNotificationConversation,
    ].join("|");
  }

  function conversationOverviewOrderKey(item: Record<string, any>): string {
    return [
      !!item.isSystemNotificationConversation,
      !!item.isPinned,
      Number.isFinite(Number(item.pinIndex)) ? Number(item.pinIndex) : Number.MAX_SAFE_INTEGER,
      String(item.lastMessageAt || item.updatedAt || "").trim(),
    ].join("|");
  }

  function applyConversationOverviewItemUpdated(payload?: Record<string, any> | null) {
    const conversation = payload?.conversation;
    const conversationId = String(conversation?.conversationId || "").trim();
    if (!conversationId) return;
    const existing = bindings.unarchivedConversations.value.find(
      (item: any) => String(item?.conversationId || "").trim() === conversationId,
    );
    if (existing && conversationOverviewItemSignature(existing) === conversationOverviewItemSignature(conversation)) {
      return;
    }
    let replaced = false;
    const nextItems = bindings.unarchivedConversations.value.map((item: any) => {
      if (String(item.conversationId || "").trim() !== conversationId) {
        return item;
      }
      replaced = true;
      return conversation;
    });
    if (!replaced) {
      nextItems.push(conversation);
    }
    // 排序键（置顶/最近活动时间）没变时保持原位替换，避免切换会话等场景全量重排整个列表
    const orderChanged = !existing
      || conversationOverviewOrderKey(existing) !== conversationOverviewOrderKey(conversation);
    bindings.unarchivedConversations.value = orderChanged
      ? sortUnarchivedConversationOverviewItems(nextItems)
      : nextItems;
    const serverTime = String(payload?.serverTime || "").trim();
    if (serverTime && bindings.lastOverviewSyncAt) {
      bindings.lastOverviewSyncAt.value = serverTime;
    }
    syncOverviewConversationErrors([conversation]);
  }

  function applyConversationPinUpdated(payload?: Record<string, any> | null) {
    const conversationId = String(payload?.conversationId || "").trim();
    if (!conversationId) return;
    const isPinned = !!payload?.isPinned;
    const pinIndex = Number.isFinite(Number(payload?.pinIndex)) ? Number(payload?.pinIndex) : undefined;
    let changed = false;
    const nextItems = bindings.unarchivedConversations.value.map((item: any) => {
      if (String(item.conversationId || "").trim() !== conversationId) {
        return item;
      }
      changed = true;
      return {
        ...item,
        isPinned,
        pinIndex,
      };
    });
    if (!changed) return;
    bindings.unarchivedConversations.value = sortUnarchivedConversationOverviewItems(nextItems);
  }

  function applyConversationRuntimeStateUpdated(payload?: Record<string, any> | null) {
    const conversationId = String(payload?.conversationId || "").trim();
    const runtimeState = String(payload?.runtimeState || "").trim();
    if (!conversationId) return;
    if (runtimeState !== "idle" && runtimeState !== "assistant_streaming" && runtimeState !== "organizing_context") {
      return;
    }
    let localChanged = false;
    let localMatched = false;
    const nextItems = bindings.unarchivedConversations.value.map((item: any) => {
      if (String(item.conversationId || "").trim() !== conversationId) {
        return item;
      }
      localMatched = true;
      if (item.runtimeState === runtimeState) {
        return item;
      }
      localChanged = true;
      return {
        ...item,
        runtimeState,
      };
    });
    if (localChanged) {
      bindings.unarchivedConversations.value = nextItems;
    }

    let remoteChanged = false;
    let remoteMatched = false;
    const nextRemoteItems = Array.isArray(bindings.remoteImContactConversations?.value)
      ? bindings.remoteImContactConversations.value.map((item: any) => {
        if (String(item.conversationId || "").trim() !== conversationId) {
          return item;
        }
        remoteMatched = true;
        if (String(item.runtimeState || "").trim() === runtimeState) {
          return item;
        }
        remoteChanged = true;
        return {
          ...item,
          runtimeState,
        };
      })
      : [];
    if (remoteChanged) {
      bindings.remoteImContactConversations.value = nextRemoteItems;
    }

    if (!localMatched && !remoteMatched && typeof bindings.refreshRemoteImConversationOverview === "function") {
      void bindings.refreshRemoteImConversationOverview().catch((error: unknown) => {
        console.warn("[远程会话] 运行态事件命中缺失会话，重拉远程列表失败", {
          conversationId,
          runtimeState,
          error,
        });
      });
    }
  }

  return {
    matchesForegroundConversation,
    formalizeConversationMessages,
    freezeConversationMessages,
    insertMessagesBeforeStreamingAssistantProjection,
    mergeMessagesIntoTimeline,
    currentConversationRuntimeState,
    maybeResumeForegroundStreamingBubble,
    conversationRuntimeSnapshotIsBusy,
    requestConversationRuntimeSnapshot,
    resumeForegroundRuntimeFromBackend,
    areMessagesEquivalent,
    messageContentSignature,
    reuseStableMessageReferences,
    beginForegroundPaintTrace,
    logForegroundPaintTrace,
    cacheConversationMessages,
    inferHasMoreHistoryFromSnapshot,
    clearConversationBadge,
    markConversationReadPersisted,
    setConversationBadge,
    readConversationIdFromPayload,
    readMessagesFromPayload,
    mergeIncomingMessagesIntoCache,
    buildConversationMessagesAfterAnchor,
    requestConversationMessagesAfterAsync,
    requestConversationMessageById,
    replaceConversationMessage,
    reloadForegroundConversationMessages,
    refreshForegroundConversationMessageById,
    loadOlderConversationHistory,
    loadOlderCompactionSegmentHistory,
    mergeConversationMessagesFromSyncPayload,
    applyConversationMessagesAfterSynced,
    applyConversationMessageAppended,
    applyConversationSnapshot,
    applyConversationTodosUpdated,
    applyConversationOverviewUpdated,
    applyConversationOverviewItemUpdated,
    applyConversationPinUpdated,
    applyConversationRuntimeStateUpdated,
    unarchivedConversationActivityAt,
    sortUnarchivedConversationOverviewItems,
  };
}
