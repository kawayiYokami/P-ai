import { computed, nextTick, onBeforeUnmount, onMounted, ref, type Ref, watch } from "vue";

type UseChatScrollLayoutOptions = {
  activeConversationId: Ref<string>;
  chatting: Ref<boolean>;
  frozen: Ref<boolean>;
  messageBlockCount: Ref<number>;
  lastMessageIsOwn: Ref<boolean>;
  latestOwnMessageAlignRequest: Ref<number>;
  conversationScrollToBottomRequest: Ref<number>;
  onReachedBottom: () => void;
  focusComposerInput: (options?: FocusOptions) => void;
};

export function useChatScrollLayout(options: UseChatScrollLayoutOptions) {
  const scrollContainer = ref<HTMLElement | null>(null);
  const composerContainer = ref<HTMLElement | null>(null);
  const toolbarContainer = ref<HTMLElement | null>(null);
  const chatLayoutRoot = ref<HTMLElement | null>(null);
  const activeTurnGroupMinHeight = ref(0);
  const jumpToBottomOffset = ref(96);
  const showSideConversationList = ref(false);
  const lastBottomState = ref(false);
  const lastScrollTop = ref(0);
  const userScrollingDown = ref(false);
  const suppressNextAnimatedConversationScroll = ref(false);

  let composerResizeObserver: ResizeObserver | null = null;
  let activeTurnLayoutObserver: ResizeObserver | null = null;
  let chatLayoutResizeObserver: ResizeObserver | null = null;

  const CHAT_SIDE_LIST_RATIO_THRESHOLD = 1.5;

  const showJumpToBottom = computed(() => !lastBottomState.value && userScrollingDown.value);
  const jumpToBottomStyle = computed(() => ({
    bottom: `${jumpToBottomOffset.value}px`,
  }));

  function scrollToBottom(behavior: ScrollBehavior = "smooth") {
    const el = scrollContainer.value;
    if (!el) return;
    el.scrollTo({ top: el.scrollHeight, behavior });
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

  function activeTurnUserElement(): HTMLElement | null {
    const el = scrollContainer.value;
    if (!el) return null;
    return el.querySelector<HTMLElement>("[data-active-turn-user='true'][data-message-id]");
  }

  function activeTurnGroupElement(): HTMLElement | null {
    const el = scrollContainer.value;
    if (!el) return null;
    return el.querySelector<HTMLElement>("[data-active-turn-group='true']");
  }

  function alignActiveTurnUserToTop(behavior: ScrollBehavior = "auto"): boolean {
    const activeTurnUser = activeTurnUserElement();
    if (!activeTurnUser) return false;
    activeTurnUser.scrollIntoView({
      block: "start",
      behavior,
    });
    return true;
  }

  function updateActiveTurnLayout() {
    const scrollEl = scrollContainer.value;
    const activeTurnUserEl = activeTurnUserElement();
    const activeTurnGroupEl = activeTurnGroupElement();
    if (!scrollEl || !activeTurnUserEl || !activeTurnGroupEl) {
      activeTurnGroupMinHeight.value = 0;
      return;
    }
    const scrollStyles = window.getComputedStyle(scrollEl);
    const scrollViewportHeight =
      scrollEl.clientHeight
      - parseFloat(scrollStyles.paddingTop || "0")
      - parseFloat(scrollStyles.paddingBottom || "0");
    const toolbarHeight = toolbarContainer.value?.offsetHeight ?? 0;
    const visibleTurnHeight = Math.max(0, scrollViewportHeight - toolbarHeight);
    const nextGroupMinHeight = visibleTurnHeight;
    if (Math.abs(activeTurnGroupMinHeight.value - nextGroupMinHeight) > 0.5) {
      activeTurnGroupMinHeight.value = nextGroupMinHeight;
    }
  }

  function syncActiveTurnLayoutObserver() {
    if (activeTurnLayoutObserver) {
      activeTurnLayoutObserver.disconnect();
      activeTurnLayoutObserver = null;
    }
    if (typeof ResizeObserver === "undefined") return;
    activeTurnLayoutObserver = new ResizeObserver(() => {
      updateActiveTurnLayout();
    });
    const scrollEl = scrollContainer.value;
    const activeTurnUserEl = activeTurnUserElement();
    const activeTurnGroupEl = activeTurnGroupElement();
    const toolbarEl = toolbarContainer.value;
    if (scrollEl) activeTurnLayoutObserver.observe(scrollEl);
    if (activeTurnUserEl) activeTurnLayoutObserver.observe(activeTurnUserEl);
    if (activeTurnGroupEl) activeTurnLayoutObserver.observe(activeTurnGroupEl);
    if (toolbarEl) activeTurnLayoutObserver.observe(toolbarEl);
  }

  onMounted(() => {
    nextTick(() => {
      syncConversationLayoutMode();
      updateJumpToBottomOffset();
      if (composerContainer.value && typeof ResizeObserver !== "undefined") {
        composerResizeObserver = new ResizeObserver(() => {
          updateJumpToBottomOffset();
        });
        composerResizeObserver.observe(composerContainer.value);
      }
      if (chatLayoutRoot.value && typeof ResizeObserver !== "undefined") {
        chatLayoutResizeObserver = new ResizeObserver(() => {
          syncConversationLayoutMode();
          updateActiveTurnLayout();
          syncActiveTurnLayoutObserver();
        });
        chatLayoutResizeObserver.observe(chatLayoutRoot.value);
      }
      updateActiveTurnLayout();
      syncActiveTurnLayoutObserver();
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
    if (chatLayoutResizeObserver) {
      chatLayoutResizeObserver.disconnect();
      chatLayoutResizeObserver = null;
    }
    if (activeTurnLayoutObserver) {
      activeTurnLayoutObserver.disconnect();
      activeTurnLayoutObserver = null;
    }
  });

  watch(
    options.chatting,
    (isChatting, wasChatting) => {
      if (wasChatting && !isChatting && !options.frozen.value) {
        nextTick(() => options.focusComposerInput({ preventScroll: true }));
      }
    },
  );

  watch(
    options.activeConversationId,
    () => {
      suppressNextAnimatedConversationScroll.value = true;
      nextTick(() => {
        updateActiveTurnLayout();
        syncActiveTurnLayoutObserver();
        updateJumpToBottomOffset();
        alignActiveTurnUserToTop("auto");
        const el = scrollContainer.value;
        if (el) {
          lastBottomState.value = isNearBottom(el);
          lastScrollTop.value = el.scrollTop;
          userScrollingDown.value = false;
        }
      });
    },
  );

  watch(showSideConversationList, () => {
    nextTick(() => {
      updateActiveTurnLayout();
      syncActiveTurnLayoutObserver();
      updateJumpToBottomOffset();
    });
  });

  watch(
    options.messageBlockCount,
    () => {
      nextTick(() => {
        updateActiveTurnLayout();
        syncActiveTurnLayoutObserver();
        updateJumpToBottomOffset();
        if (!options.lastMessageIsOwn.value) {
          const behavior: ScrollBehavior = suppressNextAnimatedConversationScroll.value ? "auto" : "smooth";
          requestAnimationFrame(() => {
            alignActiveTurnUserToTop(behavior);
            suppressNextAnimatedConversationScroll.value = false;
          });
        } else {
          suppressNextAnimatedConversationScroll.value = false;
        }
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
    options.latestOwnMessageAlignRequest,
    (nextValue, prevValue) => {
      if (!nextValue || nextValue === prevValue) return;
      nextTick(() => {
        updateActiveTurnLayout();
        syncActiveTurnLayoutObserver();
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
    activeTurnGroupMinHeight,
    showJumpToBottom,
    jumpToBottomStyle,
    showSideConversationList,
    onScroll,
    jumpToBottom,
  };
}
