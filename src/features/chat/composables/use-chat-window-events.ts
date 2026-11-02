import { onBeforeUnmount, onMounted } from "vue";
import { onTransportNotification } from "../../../services/tauri-api";
import { chatFlowRuntimesForConversation } from "./chat-flow-runtime-registry";

/**
 * 窗口级事件胶水只负责把非流式会话通知交给当前运行时。
 * 流式 history/round/delta/rebind 事件由 useChatFlow 自己订阅，确保所有窗口
 * 共用同一条消息状态机，不再维护第二套事件处理器。
 */
export function useChatWindowEvents(bindings: Record<string, any>) {
  function flowsForConversation(conversationId: string) {
    const normalizedConversationId = String(conversationId || "").trim();
    const registered = chatFlowRuntimesForConversation(normalizedConversationId);
    if (registered.length > 0) return registered;
    if (!normalizedConversationId || bindings.matchesForegroundConversation(normalizedConversationId)) {
      const fallback = bindings.getChatFlow?.();
      return fallback ? [fallback] : [];
    }
    return [];
  }

  const subscriptions: Array<{ key: string; stop: () => void }> = [];

  function subscribe<T>(key: string, method: string, handler: (payload: T) => void) {
    const stop = onTransportNotification<T>(method, handler);
    subscriptions.push({ key, stop });
    bindings.unlisteners[key] = stop;
  }

  onMounted(() => {
    // 能力自检：报错现场出现过 bindings 缺方法，挂载时打一次关键方法类型，便于定位产物/组装问题
    console.info("[窗口事件] bindings 能力自检", {
      mergeMessagesIntoTimeline: typeof bindings.mergeMessagesIntoTimeline,
      cacheConversationMessages: typeof bindings.cacheConversationMessages,
      formalizeConversationMessages: typeof bindings.formalizeConversationMessages,
      getChatFlow: typeof bindings.getChatFlow,
      hasUnlisteners: typeof bindings.unlisteners,
    });
    subscribe<any>("chatHistoryFlushed", "chat.historyFlushed", (payload) => {
      const conversationId = bindings.readConversationIdFromPayload(payload);
      if (!conversationId || flowsForConversation(conversationId).length > 0) return;
      bindings.mergeIncomingMessagesIntoCache(
        conversationId,
        bindings.readMessagesFromPayload(payload),
      );
      try {
        const cid = String(conversationId || "").trim();
        const openSideId = String(bindings.sideConversationId?.value || "").trim();
        const msgs = bindings.readMessagesFromPayload(payload) || [];
        if (cid && cid !== openSideId && msgs.length > 0 && typeof bindings.bumpSideConversationUnread === "function") {
          bindings.bumpSideConversationUnread(cid);
        }
      } catch { /* 侧会话未读自维护失败不影响主链路 */ }
    });

    subscribe<any>("chatRoundCompleted", "chat.roundFinished", (payload) => {
      const conversationId = bindings.readConversationIdFromPayload(payload);
      const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
      const value = payload && typeof payload === "object" ? payload as Record<string, any> : null;
      const failed = String(value?.status || "").trim() === "failed" || !!String(value?.error || "").trim();
      // 侧会话失败即记红标：不管它在不在前台、有没有注册 flow，磁贴都要能亮红；
      // 前台主会话的失败走会话内报错卡，不记角标。
      if (failed && conversationId && conversationId !== currentConversationId) {
        try { bindings.setConversationBadge?.(conversationId, "failed"); } catch { /* 记红标失败不影响主链路 */ }
      }
      const targetFlows = flowsForConversation(conversationId);
      if (targetFlows.length === 0 && conversationId && conversationId !== currentConversationId) {
        if (failed) {
          return;
        }
        const assistantMessage = value?.assistantMessage || null;
        if (assistantMessage && String(assistantMessage?.id || "").trim()) {
          const cachedMessages = bindings.formalizeConversationMessages(
            bindings.conversationMessageCache.value[conversationId] || [],
          );
          if (typeof bindings.mergeMessagesIntoTimeline !== "function") {
            console.warn("[窗口事件] bindings 缺少 mergeMessagesIntoTimeline，回退为直接拼接", {
              conversationId,
              bindingsKeys: Object.keys(bindings).filter((key) => /merge|message/i.test(key)),
            });
            bindings.cacheConversationMessages(conversationId, [...cachedMessages, assistantMessage]);
            return;
          }
          bindings.cacheConversationMessages(
            conversationId,
            bindings.mergeMessagesIntoTimeline(cachedMessages, [assistantMessage]),
          );
        }
        return;
      }
      if (conversationId !== currentConversationId) return;
      bindings.clearConversationBadge(conversationId);
      if (failed) return;
      bindings.toolReviewRefreshTick.value += 1;
      queueMicrotask(() => {
        void bindings.refreshActiveGoalTask({ silent: true });
      });
    });

    subscribe<any>("chatConversationTodosUpdated", "conversation.todosUpdated", (payload) => {
      const conversationId = bindings.readConversationIdFromPayload(payload);
      for (const flow of flowsForConversation(conversationId)) {
        void flow.handleExternalTodosUpdated?.(payload);
      }
      bindings.applyConversationTodosUpdated(payload);
    });

    subscribe<any>("chatConversationPinUpdated", "conversation.pinUpdated", (payload) => {
      bindings.applyConversationPinUpdated(payload);
    });

    subscribe<any>("chatConversationChatErrorCleared", "conversation.chatErrorCleared", (payload) => {
      const conversationId = bindings.readConversationIdFromPayload(payload);
      if (!conversationId) return;
      bindings.setConversationChatErrorText?.(conversationId, "");
      bindings.clearConversationBadge?.(conversationId);
    });

    subscribe<any>("chatConversationGoalUpdated", "conversation.goalUpdated", (payload) => {
      bindings.applyConversationGoalUpdated(payload);
    });

    subscribe<any>("chatConversationRuntimeStateUpdated", "conversation.runtimeStateUpdated", (payload) => {
      const conversationId = bindings.readConversationIdFromPayload(payload);
      for (const flow of flowsForConversation(conversationId)) {
        void flow.handleExternalRuntimeStateUpdated?.(payload);
      }
      bindings.applyConversationRuntimeStateUpdated(payload);
      try {
        const cid = String(conversationId || "").trim();
        const runtimeState = String((payload as any)?.runtimeState || "").trim();
        if (cid && runtimeState && typeof bindings.setSideConversationRuntimeState === "function") {
          bindings.setSideConversationRuntimeState(cid, runtimeState);
        }
      } catch { /* 侧会话忙态自维护失败不影响主链路 */ }
    });

    subscribe<any>("chatConversationOverviewUpdated", "conversation.overviewUpdated", (payload) => {
      bindings.applyConversationOverviewUpdated(payload);
    });

    subscribe<any>("chatConversationOverviewItemUpdated", "conversation.overviewItemUpdated", (payload) => {
      bindings.applyConversationOverviewItemUpdated(payload);
    });

    subscribe<any>("chatConversationMessagesAfterSynced", "conversation.messagesAfterSynced", (payload) => {
      const conversationId = bindings.readConversationIdFromPayload(payload);
      for (const flow of flowsForConversation(conversationId)) {
        void flow.handleExternalMessagesAfterSynced?.(payload);
      }
      void bindings.applyConversationMessagesAfterSynced(payload);
    });

    subscribe<any>("chatConversationMessageAppended", "conversation.messageAppended", (payload) => {
      const conversationId = bindings.readConversationIdFromPayload(payload);
      for (const flow of flowsForConversation(conversationId)) {
        void flow.handleExternalMessageAppended?.(payload);
      }
      bindings.applyConversationMessageAppended(payload);
      try {
        const cid = String(conversationId || "").trim();
        const openSideId = String(bindings.sideConversationId?.value || "").trim();
        const message = (payload as any)?.message || null;
        const role = String(message?.role || "").trim().toLowerCase();
        // 用户自己发的不算未读；打开正在看的也不算；其余后台侧会话来消息记一条未读
        if (cid && cid !== openSideId && role !== "user" && typeof bindings.bumpSideConversationUnread === "function") {
          bindings.bumpSideConversationUnread(cid);
        }
      } catch { /* 侧会话未读自维护失败不影响主链路 */ }
    });

    // 冷启动强制恢复：进程被杀后重载无边沿事件可依赖，不走普通 mounted 调度；
    // 老宿主没有冷启动入口时回退到原来的 mounted 调度。
    if (typeof bindings.handleColdStartForStateSync === "function") {
      bindings.handleColdStartForStateSync("cold_start");
    } else {
      bindings.scheduleChatWindowActiveStateSync("mounted");
    }
    bindings.startGoalTaskPolling();
    void bindings.refreshActiveGoalTask({ silent: true });
    window.addEventListener("focus", bindings.handleWindowFocusForStateSync);
    window.addEventListener("blur", bindings.handleWindowBlurForStateSync);
    document.addEventListener("visibilitychange", bindings.handleVisibilityForStateSync);
    window.addEventListener("pageshow", bindings.handlePageShowForStateSync);
    window.addEventListener("online", bindings.handleOnlineForStateSync);
    document.addEventListener("resume", bindings.handleResumeForStateSync);
    document.addEventListener("freeze", bindings.handleFreezeForStateSync);
    window.addEventListener("focus", bindings.handleWindowFocusForMicPrewarm);
    document.addEventListener("visibilitychange", bindings.handleVisibilityForMicPrewarm);
  });

  onBeforeUnmount(() => {
    for (const { key, stop } of subscriptions.splice(0)) {
      stop();
      if (bindings.unlisteners[key] === stop) bindings.unlisteners[key] = null;
    }
    window.removeEventListener("focus", bindings.handleWindowFocusForStateSync);
    window.removeEventListener("blur", bindings.handleWindowBlurForStateSync);
    document.removeEventListener("visibilitychange", bindings.handleVisibilityForStateSync);
    window.removeEventListener("pageshow", bindings.handlePageShowForStateSync);
    window.removeEventListener("online", bindings.handleOnlineForStateSync);
    document.removeEventListener("resume", bindings.handleResumeForStateSync);
    document.removeEventListener("freeze", bindings.handleFreezeForStateSync);
    bindings.clearChatWindowActiveSyncTimer();
    bindings.clearChatMicPrewarmTimer();
    bindings.clearGoalTaskPollTimer();
    bindings.cleanupChatForegroundActivity();
    bindings.agentWorkPresence.cleanup();
    void bindings.getChatFlow()?.unbindActiveConversationStream?.().catch(() => {});
    window.removeEventListener("focus", bindings.handleWindowFocusForMicPrewarm);
    document.removeEventListener("visibilitychange", bindings.handleVisibilityForMicPrewarm);
    bindings.cancelPendingRewindConfirm();
  });
}
