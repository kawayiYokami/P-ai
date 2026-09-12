import type { ChatRenderItem } from "../utils/chat-render";
import { nextTick, onBeforeUnmount, ref, watch, type Ref } from "vue";
import { probeChatScroll } from "./chat-scroll-probe";

export interface UseChatScrollOrchestrationOptions {
  scrollContainer: Ref<HTMLElement | null>;
  chatScrollbarRef: Ref<{ updateThumb: () => void; hide?: () => void } | null>;
  prepareBottomAlignmentLayout?: () => Promise<void> | void;
  onScroll: () => void;
  scheduleVirtualMeasure: () => void;
  resetConversationToBottom: (behavior?: "auto" | "smooth") => void;
  // 把最新用户消息对齐到视口顶部（与时间线跳转同一套 align: "start"）
  alignLatestOwnMessageToTop?: () => void;
  // 手动「回到底部」时按当前离底距离解析真实滚动行为（近平滑、远瞬移）
  resolveManualScrollToBottomBehavior?: () => "auto" | "smooth";
  olderHistoryCorrectionAllowed?: Ref<boolean>;
  props: {
    hasMoreHistory: Ref<boolean>;
    loadingOlderHistory: Ref<boolean>;
    chatting: Ref<boolean>;
    conversationBusy: Ref<boolean>;
    frozen: Ref<boolean>;
    activeConversationId: Ref<string>;
    conversationScrollToBottomRequest: Ref<number>;
    scrollToBottomBehavior: Ref<"auto" | "smooth" | "own_top" | "manual">;
    renderItems: Ref<ChatRenderItem[]>;
  };
  emit: {
    loadOlderHistory: () => void;
    jumpToConversationBottom: () => void;
  };
}

export function useChatScrollOrchestration(options: UseChatScrollOrchestrationOptions) {
  const {
    scrollContainer,
    chatScrollbarRef,
    prepareBottomAlignmentLayout,
    onScroll,
    scheduleVirtualMeasure,
    resetConversationToBottom,
    alignLatestOwnMessageToTop,
    resolveManualScrollToBottomBehavior,
    olderHistoryCorrectionAllowed,
    props,
    emit,
  } = options;

  const LOAD_OLDER_HISTORY_THRESHOLD_PX = 8;
  const OLDER_HISTORY_MAX_COOLDOWN_MS = 1000;
  const SCROLL_SETTLE_DELAY_MS = 120;
  const olderHistoryRequestPending = ref(false);
  const suppressOlderHistoryPaginationOnce = ref(false);
  let pendingProgrammaticScrollPaginationResetFrame = 0;
  let pendingScrollSettleTimer = 0;
  let pendingOlderHistoryReleaseTimer = 0;
  let olderHistoryCooldownUntil = 0;

  function waitAnimationFrames(count: number) {
    return new Promise<void>((resolve) => {
      let remaining = Math.max(1, count);
      const step = () => {
        remaining -= 1;
        if (remaining <= 0) {
          resolve();
          return;
        }
        requestAnimationFrame(step);
      };
      requestAnimationFrame(step);
    });
  }

  function clearOlderHistoryReleaseTimer() {
    if (!pendingOlderHistoryReleaseTimer) return;
    window.clearTimeout(pendingOlderHistoryReleaseTimer);
    pendingOlderHistoryReleaseTimer = 0;
  }

  function releaseOlderHistoryRequestGate() {
    clearOlderHistoryReleaseTimer();
    olderHistoryCooldownUntil = 0;
    olderHistoryRequestPending.value = false;
    if (olderHistoryCorrectionAllowed) {
      olderHistoryCorrectionAllowed.value = false;
    }
  }

  function armOlderHistoryRequestGate() {
    clearOlderHistoryReleaseTimer();
    olderHistoryRequestPending.value = true;
    if (olderHistoryCorrectionAllowed) {
      olderHistoryCorrectionAllowed.value = true;
    }
    olderHistoryCooldownUntil = Date.now() + OLDER_HISTORY_MAX_COOLDOWN_MS;
    pendingOlderHistoryReleaseTimer = window.setTimeout(() => {
      pendingOlderHistoryReleaseTimer = 0;
      if (props.loadingOlderHistory.value) return;
      releaseOlderHistoryRequestGate();
      scheduleSettledOlderHistoryCheck();
    }, OLDER_HISTORY_MAX_COOLDOWN_MS);
  }

  function armProgrammaticScrollPaginationSuppression() {
    suppressOlderHistoryPaginationOnce.value = true;
    if (pendingProgrammaticScrollPaginationResetFrame) {
      cancelAnimationFrame(pendingProgrammaticScrollPaginationResetFrame);
      pendingProgrammaticScrollPaginationResetFrame = 0;
    }
    pendingProgrammaticScrollPaginationResetFrame = requestAnimationFrame(() => {
      pendingProgrammaticScrollPaginationResetFrame = requestAnimationFrame(() => {
        suppressOlderHistoryPaginationOnce.value = false;
        pendingProgrammaticScrollPaginationResetFrame = 0;
      });
    });
  }

  function maybeRequestOlderHistory() {
    const scrollEl = scrollContainer.value;
    if (!scrollEl) return;
    if (!props.hasMoreHistory.value || props.loadingOlderHistory.value || olderHistoryRequestPending.value) return;
    if (scrollEl.scrollTop > LOAD_OLDER_HISTORY_THRESHOLD_PX) return;
    if (Date.now() < olderHistoryCooldownUntil) return;
    armOlderHistoryRequestGate();
    emit.loadOlderHistory();
  }

  function isNearOlderHistoryTriggerPoint() {
    const scrollEl = scrollContainer.value;
    return !!scrollEl && scrollEl.scrollTop <= LOAD_OLDER_HISTORY_THRESHOLD_PX;
  }

  function scheduleSettledOlderHistoryCheck() {
    if (pendingScrollSettleTimer) {
      window.clearTimeout(pendingScrollSettleTimer);
      pendingScrollSettleTimer = 0;
    }
    pendingScrollSettleTimer = window.setTimeout(() => {
      pendingScrollSettleTimer = 0;
      maybeRequestOlderHistory();
    }, SCROLL_SETTLE_DELAY_MS);
  }

  function shouldLockUpwardWheelInput(deltaY: number): boolean {
    if (deltaY >= 0) return false;
    if (!props.hasMoreHistory.value) return false;
    if (props.loadingOlderHistory.value || olderHistoryRequestPending.value) return true;
    const inCooldown = Date.now() < olderHistoryCooldownUntil;
    return isNearOlderHistoryTriggerPoint() && inCooldown;
  }

  function onConversationWheel(event: WheelEvent) {
    if (event.shiftKey) return;
    if (event.deltaY < 0 && isNearOlderHistoryTriggerPoint()) {
      maybeRequestOlderHistory();
    }
    if (!shouldLockUpwardWheelInput(event.deltaY)) return;
    event.preventDefault();
    event.stopPropagation();
  }

  function onConversationScroll() {
    onScroll();
    chatScrollbarRef.value?.updateThumb();
    if (suppressOlderHistoryPaginationOnce.value) {
      suppressOlderHistoryPaginationOnce.value = false;
      if (pendingProgrammaticScrollPaginationResetFrame) {
        cancelAnimationFrame(pendingProgrammaticScrollPaginationResetFrame);
        pendingProgrammaticScrollPaginationResetFrame = 0;
      }
    } else {
      scheduleSettledOlderHistoryCheck();
    }
  }

  function doScrollToBottom(behavior: "auto" | "smooth" = "auto") {
    armProgrammaticScrollPaginationSuppression();
    scheduleVirtualMeasure();
    void nextTick(async () => {
      await prepareBottomAlignmentLayout?.();
      resetConversationToBottom(behavior);
    });
  }

  function handleJumpToBottom(behavior: "auto" | "smooth" = "auto") {
    doScrollToBottom(behavior);
    if (props.chatting.value || props.conversationBusy.value || props.frozen.value) return;
    emit.jumpToConversationBottom();
  }

  // ==================== watchers ====================

  watch(
    () => String(props.activeConversationId.value || "").trim(),
    () => {
      chatScrollbarRef.value?.hide?.();
      releaseOlderHistoryRequestGate();
      armProgrammaticScrollPaginationSuppression();
      void prepareBottomAlignmentLayout?.();
    },
    { immediate: true },
  );

  watch(
    () => props.conversationScrollToBottomRequest.value,
    (nextValue, prevValue) => {
      if (!nextValue || nextValue === prevValue) return;
      probeChatScroll("滚动请求", { behavior: props.scrollToBottomBehavior.value, request: nextValue });
      if (props.scrollToBottomBehavior.value === "own_top") {
        armProgrammaticScrollPaginationSuppression();
        alignLatestOwnMessageToTop?.();
        return;
      }
      if (props.scrollToBottomBehavior.value === "manual") {
        doScrollToBottom(resolveManualScrollToBottomBehavior?.() ?? "auto");
        return;
      }
      doScrollToBottom(props.scrollToBottomBehavior.value);
    },
  );

  watch(
    () => props.renderItems.value,
    () => {
      void nextTick(() => {
        chatScrollbarRef.value?.updateThumb();
      });
    },
  );

  watch(
    () => props.loadingOlderHistory.value,
    async (loading, wasLoading) => {
      if (loading || !wasLoading) return;
      await nextTick();
      await waitAnimationFrames(1);
      chatScrollbarRef.value?.updateThumb();
      await waitAnimationFrames(1);
      chatScrollbarRef.value?.updateThumb();
      releaseOlderHistoryRequestGate();
      scheduleSettledOlderHistoryCheck();
    },
  );

  onBeforeUnmount(() => {
    if (pendingProgrammaticScrollPaginationResetFrame) {
      cancelAnimationFrame(pendingProgrammaticScrollPaginationResetFrame);
      pendingProgrammaticScrollPaginationResetFrame = 0;
    }
    if (pendingScrollSettleTimer) {
      window.clearTimeout(pendingScrollSettleTimer);
      pendingScrollSettleTimer = 0;
    }
    clearOlderHistoryReleaseTimer();
  });

  return {
    onConversationScroll,
    onConversationWheel,
    handleJumpToBottom,
    activeConversationChangedCleanup: () => {
      releaseOlderHistoryRequestGate();
    },
    armProgrammaticScrollPaginationSuppression,
  };
}
