import { computed, nextTick, onBeforeUnmount, onMounted, ref, type Ref, watch } from "vue";
import { isMobileTouchViewport } from "../../shared/utils/mobile-viewport";
import { isDesktopTauriHost } from "../../../services/tauri-api";
import { probeChatScroll } from "./chat-scroll-probe";

const FLOATING_TOOLBAR_MIN_RESERVE = 24;
const SESSION_CONTROL_PANEL_HIDE_DELAY_MS = 200;

type UseChatScrollLayoutOptions = {
  activeConversationId: Ref<string>;
  chatting: Ref<boolean>;
  busy: Ref<boolean>;
  frozen: Ref<boolean>;
  timelineItemCount: Ref<number>;
  isWebRoundedMode?: Ref<boolean>;
  onReachedBottom: () => void;
  focusComposerInput: (options?: FocusOptions) => void;
};

export function useChatScrollLayout(options: UseChatScrollLayoutOptions) {
  const scrollContainer = ref<HTMLElement | null>(null);
  const composerContainer = ref<HTMLElement | null>(null);
  const toolbarContainer = ref<HTMLElement | null>(null);
  const chatLayoutRoot = ref<HTMLElement | null>(null);
  const latestOwnElasticMinHeight = ref(0);
  const composerReservedHeight = ref(0);
  const jumpToBottomOffset = ref(96);
  const lastBottomState = ref(false);
  // 贴底跟随：默认关闭。只有用户主动滚动到达底部、或点击「回到底部」时才开启；
  // 内容增长时据此决定是否持续贴底，向上滚离底部即退出。
  const followBottom = ref(false);
  const lastScrollTop = ref(0);
  const userScrollingDown = ref(false);
  const userScrollingUp = ref(false);
  const sessionControlPanelVisible = ref(true);
  let composerResizeObserver: ResizeObserver | null = null;
  let chatLayoutResizeObserver: ResizeObserver | null = null;
  let pendingComposerResizeFrame = 0;
  let pendingChatLayoutResizeFrame = 0;
  let wheelScrollIntentUntil = 0;
  let pointerScrollIntentActive = false;
  let sessionControlPanelHideTimer: ReturnType<typeof setTimeout> | null = null;

  // 会话悬浮操作区：工作区 bar、思维链预览 bar、时间线按钮共用同一容器，
  // 统一锚定在输入框上沿并留 8px（p-2）底部间距，横向跟随容器宽度
  const sessionFloatDockStyle = computed(() => ({
    bottom: `${composerReservedHeight.value + 8}px`,
  }));
  const toolbarReservedHeight = computed(() => {
    const measuredHeight = toolbarContainer.value?.offsetHeight ?? 0;
    if (measuredHeight <= 0) return 0;
    return Math.max(FLOATING_TOOLBAR_MIN_RESERVE, measuredHeight);
  });

  function updateJumpToBottomOffset() {
    const composerHeight = composerContainer.value?.offsetHeight ?? 0;
    if (composerReservedHeight.value !== composerHeight) {
      composerReservedHeight.value = composerHeight;
    }
    // 桌面灌满 +16，Web双栏悬浮突出 24px 圆角 +40
    const isFloating = options.isWebRoundedMode?.value ?? (!isDesktopTauriHost() && false);
    const nextOffset = Math.max(16, composerHeight + (isFloating ? 40 : 16));
    if (jumpToBottomOffset.value !== nextOffset) {
      jumpToBottomOffset.value = nextOffset;
    }
  }

  function updateLatestOwnElasticMinHeight() {
    const scrollEl = scrollContainer.value;
    if (!scrollEl) {
      if (latestOwnElasticMinHeight.value !== 0) {
        latestOwnElasticMinHeight.value = 0;
      }
      return;
    }
    // 尾部留白要让「最新用户消息」能正好顶到视口顶部：
    // 需要 留白 = 可滚动高度 − 尾段内容高，故这里的上限取视口高扣掉容器底部内边距。
    const scrollStyles = window.getComputedStyle(scrollEl);
    const paddingBottom = parseFloat(scrollStyles.paddingBottom || "0");
    const nextMinHeight = Math.max(0, Math.round(scrollEl.clientHeight - paddingBottom));
    if (latestOwnElasticMinHeight.value !== nextMinHeight) {
      latestOwnElasticMinHeight.value = nextMinHeight;
    }
  }

  async function prepareBottomAlignmentLayout() {
    updateJumpToBottomOffset();
    updateLatestOwnElasticMinHeight();
    await nextTick();
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    updateJumpToBottomOffset();
    updateLatestOwnElasticMinHeight();
    await nextTick();
  }

  function isNearBottom(el: HTMLElement): boolean {
    const threshold = 24;
    const distance = el.scrollHeight - (el.scrollTop + el.clientHeight);
    return distance <= threshold;
  }

  function updateSessionControlPanelVisibility(nearBottom: boolean) {
    if (nearBottom) {
      if (sessionControlPanelHideTimer) {
        clearTimeout(sessionControlPanelHideTimer);
        sessionControlPanelHideTimer = null;
      }
      sessionControlPanelVisible.value = true;
      return;
    }
    if (sessionControlPanelHideTimer) return;
    sessionControlPanelHideTimer = setTimeout(() => {
      sessionControlPanelHideTimer = null;
      if (!lastBottomState.value) {
        sessionControlPanelVisible.value = false;
      }
    }, SESSION_CONTROL_PANEL_HIDE_DELAY_MS);
  }

  function updateScrollPositionState(el: HTMLElement, optionsOverride: { notifyReachedBottom?: boolean } = {}) {
    const nearBottom = isNearBottom(el);
    if (optionsOverride.notifyReachedBottom && nearBottom && !lastBottomState.value) {
      options.onReachedBottom();
    }
    if (nearBottom) {
      userScrollingDown.value = false;
      userScrollingUp.value = false;
    }
    lastBottomState.value = nearBottom;
    updateSessionControlPanelVisibility(nearBottom);
    return nearBottom;
  }

  function onScroll() {
    const el = scrollContainer.value;
    if (!el) return;
    const nextScrollTop = el.scrollTop;
    const previousScrollTop = lastScrollTop.value;
    const userInitiatedScroll = pointerScrollIntentActive || Date.now() <= wheelScrollIntentUntil;
    if (userInitiatedScroll) {
      if (nextScrollTop > previousScrollTop) {
        userScrollingDown.value = true;
        userScrollingUp.value = false;
      } else if (nextScrollTop < previousScrollTop) {
        userScrollingDown.value = false;
        userScrollingUp.value = true;
      }
    }
    lastScrollTop.value = nextScrollTop;
    const nearBottom = updateScrollPositionState(el, { notifyReachedBottom: true });
    // 只有用户主动滚动才改变跟随意图：滚到底进入、离开底部退出；
    // 程序化滚动（切会话、发送定位、跟随自身贴底）不改动它
    if (userInitiatedScroll) {
      const before = followBottom.value;
      followBottom.value = nearBottom;
      if (before !== followBottom.value) {
        probeChatScroll("followBottom变化", {
          to: followBottom.value,
          nearBottom,
          scrollTop: Math.round(nextScrollTop),
          delta: Math.round(nextScrollTop - previousScrollTop),
        });
      }
    }
  }

  // 显式表达贴底意图（如点击「回到底部」）：进入跟随
  function startFollowBottom() {
    if (!followBottom.value) {
      probeChatScroll("startFollowBottom", { followBottom: true });
    }
    followBottom.value = true;
  }

  // 显式退出贴底跟随（发送消息后改为对齐到顶部，不再贴底）
  function stopFollowBottom() {
    if (followBottom.value) {
      probeChatScroll("stopFollowBottom", {});
    }
    followBottom.value = false;
  }

  function noteWheelScrollIntent() {
    wheelScrollIntentUntil = Date.now() + 500;
  }

  function endPointerScrollIntent() {
    pointerScrollIntentActive = false;
    window.removeEventListener("pointerup", endPointerScrollIntent);
    window.removeEventListener("pointercancel", endPointerScrollIntent);
  }

  function beginPointerScrollIntent() {
    pointerScrollIntentActive = true;
    window.addEventListener("pointerup", endPointerScrollIntent);
    window.addEventListener("pointercancel", endPointerScrollIntent);
  }

  onMounted(() => {
    nextTick(() => {
      updateJumpToBottomOffset();
      updateLatestOwnElasticMinHeight();
      if (composerContainer.value && typeof ResizeObserver !== "undefined") {
        composerResizeObserver = new ResizeObserver(() => {
          if (typeof window === "undefined") {
            updateJumpToBottomOffset();
            updateLatestOwnElasticMinHeight();
            return;
          }
          if (pendingComposerResizeFrame) return;
          pendingComposerResizeFrame = window.requestAnimationFrame(() => {
            pendingComposerResizeFrame = 0;
            updateJumpToBottomOffset();
            updateLatestOwnElasticMinHeight();
          });
        });
        composerResizeObserver.observe(composerContainer.value);
      }
      if (chatLayoutRoot.value && typeof ResizeObserver !== "undefined") {
        chatLayoutResizeObserver = new ResizeObserver(() => {
          if (typeof window === "undefined") {
            updateJumpToBottomOffset();
            updateLatestOwnElasticMinHeight();
            return;
          }
          if (pendingChatLayoutResizeFrame) return;
          pendingChatLayoutResizeFrame = window.requestAnimationFrame(() => {
            pendingChatLayoutResizeFrame = 0;
            updateJumpToBottomOffset();
            updateLatestOwnElasticMinHeight();
          });
        });
        chatLayoutResizeObserver.observe(chatLayoutRoot.value);
      }
      const el = scrollContainer.value;
      if (el) {
        sessionControlPanelVisible.value = updateScrollPositionState(el);
        lastScrollTop.value = el.scrollTop;
        userScrollingDown.value = false;
        userScrollingUp.value = false;
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
    if (typeof window !== "undefined") {
      window.removeEventListener("pointerup", endPointerScrollIntent);
      window.removeEventListener("pointercancel", endPointerScrollIntent);
    }
    if (sessionControlPanelHideTimer) {
      clearTimeout(sessionControlPanelHideTimer);
      sessionControlPanelHideTimer = null;
    }
  });

  watch(
    options.chatting,
    (isChatting, wasChatting) => {
      if (
        wasChatting
        && !isChatting
        && !options.frozen.value
        && !options.busy.value
        && !isMobileTouchViewport()
      ) {
        nextTick(() => options.focusComposerInput({ preventScroll: true }));
      }
    },
  );

  watch(
    options.activeConversationId,
    () => {
      nextTick(() => {
        updateJumpToBottomOffset();
        updateLatestOwnElasticMinHeight();
        const el = scrollContainer.value;
        if (el) {
          sessionControlPanelVisible.value = updateScrollPositionState(el);
          lastScrollTop.value = el.scrollTop;
          userScrollingDown.value = false;
          userScrollingUp.value = false;
          // 切换会话属于「定位到新消息」，不继承上一会话的跟随意图
          followBottom.value = false;
        }
      });
    },
    { immediate: true },
  );

  watch(
    options.timelineItemCount,
    () => {
      nextTick(() => {
        updateJumpToBottomOffset();
        updateLatestOwnElasticMinHeight();
        const el = scrollContainer.value;
        if (el) {
          updateScrollPositionState(el);
          lastScrollTop.value = el.scrollTop;
        }
      });
    },
  );

  return {
    scrollContainer,
    composerContainer,
    toolbarContainer,
    chatLayoutRoot,
    latestOwnElasticMinHeight,
    composerReservedHeight,
    atConversationBottom: lastBottomState,
    followBottom,
    startFollowBottom,
    stopFollowBottom,
    userScrollingDown,
    userScrollingUp,
    sessionControlPanelVisible,
    sessionFloatDockStyle,
    toolbarReservedHeight,
    onScroll,
    noteWheelScrollIntent,
    beginPointerScrollIntent,
    prepareBottomAlignmentLayout,
  };
}
