import { ref, type Ref } from "vue";

type UseChatScrollCoordinatorOptions = {
  currentChatConversationId: Ref<string>;
};

export function useChatScrollCoordinator(options: UseChatScrollCoordinatorOptions) {
  const conversationScrollToBottomRequest = ref(0);
  const scrollToBottomBehavior = ref<"auto" | "smooth" | "own_top" | "manual" | "follow">("auto");
  let pendingConversationScrollToBottomConversationId = "";
  let pendingConversationScrollToBottomTimer = 0;
  let pendingManualScrollToBottomConversationId = "";
  let pendingManualScrollToBottomRequestId = "";

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

  function triggerConversationScrollToBottom(
    conversationId: string,
    reason: string,
    behavior: "auto" | "smooth" | "own_top" | "manual" | "follow" = "auto",
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
    getPendingConversationScrollToBottomConversationId: () => pendingConversationScrollToBottomConversationId,
    getPendingConversationScrollToBottomTimer: () => pendingConversationScrollToBottomTimer,
    getPendingManualScrollToBottomConversationId: () => pendingManualScrollToBottomConversationId,
    getPendingManualScrollToBottomRequestId: () => pendingManualScrollToBottomRequestId,
  };
}
