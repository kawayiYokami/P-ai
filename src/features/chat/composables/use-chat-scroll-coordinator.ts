import { ref, type Ref } from "vue";

type UseChatScrollCoordinatorOptions = {
  currentChatConversationId: Ref<string>;
};

export function useChatScrollCoordinator(options: UseChatScrollCoordinatorOptions) {
  const conversationScrollToBottomRequest = ref(0);
  const scrollToBottomBehavior = ref<"auto" | "smooth" | "smooth_light" | "manual">("auto");
  let pendingConversationScrollToBottomConversationId = "";
  let pendingConversationScrollToBottomTimer = 0;
  let pendingManualScrollToBottomConversationId = "";
  let pendingManualScrollToBottomRequestId = "";
  // 流式中切会话：滚动等该轮流式稳定（historyFlushed 落库）后再执行。
  let pendingStreamSettleScrollConversationId = "";
  let pendingStreamSettleScrollTimer = 0;

  function clearPendingConversationScrollToBottomFallback() {
    if (pendingConversationScrollToBottomTimer) {
      window.clearTimeout(pendingConversationScrollToBottomTimer);
      pendingConversationScrollToBottomTimer = 0;
    }
  }

  function clearPendingManualScrollToBottom() {
    pendingManualScrollToBottomConversationId = "";
    pendingManualScrollToBottomRequestId = "";
  }

  /** 流式中切会话：登记「等流式稳定后滚到底」，超时兜底强制滚动。 */
  function requestScrollToBottomAfterStreamSettle(conversationId: string, timeoutMs = 1000) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    pendingStreamSettleScrollConversationId = cid;
    if (pendingStreamSettleScrollTimer) {
      window.clearTimeout(pendingStreamSettleScrollTimer);
      pendingStreamSettleScrollTimer = 0;
    }
    pendingStreamSettleScrollTimer = window.setTimeout(() => {
      pendingStreamSettleScrollTimer = 0;
      if (pendingStreamSettleScrollConversationId !== cid) return;
      pendingStreamSettleScrollConversationId = "";
      triggerConversationScrollToBottom(cid, "stream_settle_timeout");
    }, timeoutMs);
  }

  /** 流式稳定（historyFlushed 落库）时调用：若该会话有登记，立即滚动并清除登记。 */
  function settleStreamScrollAfterStable(conversationId: string) {
    const cid = String(conversationId || "").trim();
    if (!cid || cid !== pendingStreamSettleScrollConversationId) return;
    pendingStreamSettleScrollConversationId = "";
    if (pendingStreamSettleScrollTimer) {
      window.clearTimeout(pendingStreamSettleScrollTimer);
      pendingStreamSettleScrollTimer = 0;
    }
    triggerConversationScrollToBottom(cid, "stream_settled");
  }

  function triggerConversationScrollToBottom(
    conversationId: string,
    reason: string,
    behavior: "auto" | "smooth" | "smooth_light" | "manual" = "auto",
  ) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    if (cid !== String(options.currentChatConversationId.value || "").trim()) return;
    scrollToBottomBehavior.value = behavior;
    conversationScrollToBottomRequest.value += 1;
    pendingConversationScrollToBottomConversationId = "";
    clearPendingConversationScrollToBottomFallback();
    void reason;
  }

  function scheduleConversationScrollToBottomFallback(conversationId: string) {
    const cid = String(conversationId || "").trim();
    if (!cid) return;
    pendingConversationScrollToBottomConversationId = cid;
    clearPendingConversationScrollToBottomFallback();
    pendingConversationScrollToBottomTimer = window.setTimeout(() => {
      pendingConversationScrollToBottomTimer = 0;
      if (pendingConversationScrollToBottomConversationId !== cid) return;
      triggerConversationScrollToBottom(cid, "fallback_timeout");
    }, 240);
  }

  function setPendingManualScrollState(conversationId: string, requestId: string) {
    pendingManualScrollToBottomConversationId = conversationId;
    pendingManualScrollToBottomRequestId = requestId;
  }

  return {
    conversationScrollToBottomRequest,
    scrollToBottomBehavior,
    clearPendingConversationScrollToBottomFallback,
    clearPendingManualScrollToBottom,
    triggerConversationScrollToBottom,
    scheduleConversationScrollToBottomFallback,
    setPendingManualScrollState,
    requestScrollToBottomAfterStreamSettle,
    settleStreamScrollAfterStable,
    getPendingConversationScrollToBottomConversationId: () => pendingConversationScrollToBottomConversationId,
    getPendingConversationScrollToBottomTimer: () => pendingConversationScrollToBottomTimer,
    getPendingManualScrollToBottomConversationId: () => pendingManualScrollToBottomConversationId,
    getPendingManualScrollToBottomRequestId: () => pendingManualScrollToBottomRequestId,
  };
}
