import { computed, nextTick, onBeforeUnmount, onMounted, ref, type Ref, watch } from "vue";

type UseChatScrollLayoutOptions = {
  activeConversationId: Ref<string>;
  chatting: Ref<boolean>;
  busy: Ref<boolean>;
  frozen: Ref<boolean>;
  messageBlockCount: Ref<number>;
  conversationScrollToBottomRequest: Ref<number>;
  onReachedBottom: () => void;
  focusComposerInput: (options?: FocusOptions) => void;
};

export function useChatScrollLayout(options: UseChatScrollLayoutOptions) {
  const scrollContainer = ref<HTMLElement | null>(null);
  const composerContainer = ref<HTMLElement | null>(null);
  const toolbarContainer = ref<HTMLElement | null>(null);
  const chatLayoutRoot = ref<HTMLElement | null>(null);
  const jumpToBottomOffset = ref(96);
  const showSideConversationList = ref(false);
  const lastBottomState = ref(false);
  const lastScrollTop = ref(0);
  const userScrollingDown = ref(false);
  let composerResizeObserver: ResizeObserver | null = null;
  let chatLayoutResizeObserver: ResizeObserver | null = null;
  let pendingComposerResizeFrame = 0;
  let pendingChatLayoutResizeFrame = 0;

  const CHAT_SIDE_LIST_RATIO_THRESHOLD = 1.5;

  const showJumpToBottom = computed(() => !lastBottomState.value && userScrollingDown.value);
  const jumpToBottomStyle = computed(() => ({
    bottom: `${jumpToBottomOffset.value}px`,
  }));

  function scrollToBottom(behavior: ScrollBehavior = "smooth") {
    const el = scrollContainer.value;
    if (!el) return;
    const distance = Math.abs(el.scrollHeight - el.clientHeight - el.scrollTop);
    const finalBehavior: ScrollBehavior =
      behavior === "smooth" && distance > el.clientHeight * 3 ? "auto" : behavior;
    el.scrollTo({ top: el.scrollHeight, behavior: finalBehavior });
  }

  function jumpToBottom() {
    scrollToBottom("smooth");
  }

  function syncConversationLayoutMode() {
    const el = chatLayoutRoot.value;
    if (!el) return;
    const width = el.clientWidth;
    const height = el.clientHeight;
    if (width <= 0 || height <= 0) return;
    showSideConversationList.value = width > height * CHAT_SIDE_LIST_RATIO_THRESHOLD;
  }

  function updateJumpToBottomOffset() {
    const composerHeight = composerContainer.value?.offsetHeight ?? 0;
    jumpToBottomOffset.value = Math.max(16, composerHeight + 12);
  }

  function isNearBottom(el: HTMLElement): boolean {
    const threshold = 24;
    const distance = el.scrollHeight - (el.scrollTop + el.clientHeight);
    return distance <= threshold;
  }

  function onScroll() {
    const el = scrollContainer.value;
    if (!el) return;
    userScrollingDown.value = el.scrollTop > lastScrollTop.value;
    lastScrollTop.value = el.scrollTop;
    const nearBottom = isNearBottom(el);
    if (nearBottom && !lastBottomState.value) {
      options.onReachedBottom();
    }
    if (nearBottom) {
      userScrollingDown.value = false;
    }
    lastBottomState.value = nearBottom;
  }

  onMounted(() => {
    nextTick(() => {
      syncConversationLayoutMode();
      updateJumpToBottomOffset();
      if (composerContainer.value && typeof ResizeObserver !== "undefined") {
        composerResizeObserver = new ResizeObserver(() => {
          if (typeof window === "undefined") {
            updateJumpToBottomOffset();
            return;
          }
          if (pendingComposerResizeFrame) return;
          pendingComposerResizeFrame = window.requestAnimationFrame(() => {
            pendingComposerResizeFrame = 0;
            updateJumpToBottomOffset();
          });
        });
        composerResizeObserver.observe(composerContainer.value);
      }
      if (chatLayoutRoot.value && typeof ResizeObserver !== "undefined") {
        chatLayoutResizeObserver = new ResizeObserver(() => {
          if (typeof window === "undefined") {
            syncConversationLayoutMode();
            updateJumpToBottomOffset();
            return;
          }
          if (pendingChatLayoutResizeFrame) return;
          pendingChatLayoutResizeFrame = window.requestAnimationFrame(() => {
            pendingChatLayoutResizeFrame = 0;
            syncConversationLayoutMode();
            updateJumpToBottomOffset();
          });
        });
        chatLayoutResizeObserver.observe(chatLayoutRoot.value);
      }
      const el = scrollContainer.value;
      if (el) {
        lastBottomState.value = isNearBottom(el);
        lastScrollTop.value = el.scrollTop;
        userScrollingDown.value = false;
      }
    });
  });

  onBeforeUnmount(() => {
    if (composerResizeObserver) {
      composerResizeObserver.disconnect();
      composerResizeObserver = null;
    }
    if (pendingComposerResizeFrame && typeof window !== "undefined") {
      window.cancelAnimationFrame(pendingComposerResizeFrame);
      pendingComposerResizeFrame = 0;
    }
    if (chatLayoutResizeObserver) {
      chatLayoutResizeObserver.disconnect();
      chatLayoutResizeObserver = null;
    }
    if (pendingChatLayoutResizeFrame && typeof window !== "undefined") {
      window.cancelAnimationFrame(pendingChatLayoutResizeFrame);
      pendingChatLayoutResizeFrame = 0;
    }
  });

  watch(
    options.chatting,
    (isChatting, wasChatting) => {
      if (wasChatting && !isChatting && !options.frozen.value && !options.busy.value) {
        nextTick(() => options.focusComposerInput({ preventScroll: true }));
      }
    },
  );

  watch(
    options.activeConversationId,
    () => {
      nextTick(() => {
        updateJumpToBottomOffset();
        const el = scrollContainer.value;
        if (el) {
          lastBottomState.value = isNearBottom(el);
          lastScrollTop.value = el.scrollTop;
          userScrollingDown.value = false;
        }
      });
    },
    { immediate: true },
  );

  watch(showSideConversationList, () => {
    nextTick(() => {
      updateJumpToBottomOffset();
    });
  });

  watch(
    options.messageBlockCount,
    () => {
      nextTick(() => {
        updateJumpToBottomOffset();
        const el = scrollContainer.value;
        if (el) {
          lastBottomState.value = isNearBottom(el);
          lastScrollTop.value = el.scrollTop;
          userScrollingDown.value = false;
        }
      });
    },
  );

  watch(
    options.conversationScrollToBottomRequest,
    (nextValue, prevValue) => {
      if (!nextValue || nextValue === prevValue) return;
      nextTick(() => {
        requestAnimationFrame(() => {
          scrollToBottom("auto");
          const el = scrollContainer.value;
          if (el) {
            lastBottomState.value = isNearBottom(el);
            lastScrollTop.value = el.scrollTop;
            userScrollingDown.value = false;
          }
        });
      });
    },
  );

  return {
    scrollContainer,
    composerContainer,
    toolbarContainer,
    chatLayoutRoot,
    showJumpToBottom,
    jumpToBottomStyle,
    showSideConversationList,
    onScroll,
    jumpToBottom,
  };
}
