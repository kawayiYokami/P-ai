import { computed, nextTick, onBeforeUnmount, onMounted, ref, type Ref, watch } from "vue";
import { isMobileTouchViewport } from "../../shared/utils/mobile-viewport";
import { probeChatScroll } from "./chat-scroll-probe";

const TODO_DROPDOWN_SAFE_GAP = 30;
const FLOATING_TOOLBAR_MIN_RESERVE = 24;
const SESSION_CONTROL_PANEL_HIDE_DELAY_MS = 200;
// 思维链预览条：滚动停止满这么久才出现；期间任何一次滚动都重新计时，也就是一滚动就收起
const PREVIEW_IDLE_DELAY_MS = 3_000;

type UseChatScrollLayoutOptions = {
  activeConversationId: Ref<string>;
  chatting: Ref<boolean>;
  busy: Ref<boolean>;
  frozen: Ref<boolean>;
  timelineItemCount: Ref<number>;
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
  // 滚动是否已静止到可以出预览条
  const scrollIdle = ref(false);
  let composerResizeObserver: ResizeObserver | null = null;
  let chatLayoutResizeObserver: ResizeObserver | null = null;
  let pendingComposerResizeFrame = 0;
  let pendingChatLayoutResizeFrame = 0;
  let wheelScrollIntentUntil = 0;
  let pointerScrollIntentActive = false;
  // harness 式跟随动画锁：smooth 贴底进行中有未完成 target 时，
  // 动画自身送达的滚动不算用户意图，不翻转 followBottom；
  // 用户手势先 interrupt 瞬冻，再按正常意图归因。
  let followAnimTarget: number | null = null;
  let sessionControlPanelHideTimer: ReturnType<typeof setTimeout> | null = null;
  let previewIdleTimer: ReturnType<typeof setTimeout> | null = null;

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
    const nextOffset = Math.max(16, composerHeight + 16);
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
    // 尾部留白给「最新用户消息」留出上滑余量，但不能大到把消息顶出屏幕：
    // 上限取视口高扣掉工具条保留高度与下拉安全距，再留 5% 富余。
    const scrollStyles = window.getComputedStyle(scrollEl);
    const scrollViewportHeight =
      scrollEl.clientHeight
      - parseFloat(scrollStyles.paddingTop || "0")
      - parseFloat(scrollStyles.paddingBottom || "0");
    const nextMinHeight = Math.max(
      0,
      Math.round((scrollViewportHeight - toolbarReservedHeight.value - TODO_DROPDOWN_SAFE_GAP) * 0.95),
    );
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

  // 滚动静止计时：任一滚动事件都把预览条打回收起态并重新计时
  function restartScrollIdleTimer() {
    if (previewIdleTimer) {
      clearTimeout(previewIdleTimer);
      previewIdleTimer = null;
    }
    scrollIdle.value = false;
    previewIdleTimer = setTimeout(() => {
      previewIdleTimer = null;
      scrollIdle.value = true;
    }, PREVIEW_IDLE_DELAY_MS);
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

  // suppressFollowIntent：补载历史期间 virtua 会程序化抬高 scrollTop 保持视口锚定，
  // 这时的滚动事件不是用户手势，不得据此翻转跟随意图
  function onScroll(options: { suppressFollowIntent?: boolean } = {}) {
    const el = scrollContainer.value;
    if (!el) return;
    restartScrollIdleTimer();
    const nextScrollTop = el.scrollTop;
    const previousScrollTop = lastScrollTop.value;
    // 动画到岸：距底回到阈值内即视为送达，清掉未完成的 target；
    // 到岸前动画自身的滚动不参与意图翻转（用户手势已在入口处先 interrupt）。
    if (followAnimTarget !== null && isNearBottom(el)) {
      followAnimTarget = null;
    }
    const followAnimating = followAnimTarget !== null;
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
    // 跟随意图只由用户手势或显式跳转决定，程序化滚动（跳转落点、跟随自身贴底）不改动它：
    // 用户朝底部方向滚动＝有「滚到最下」的意图，解锁跟随；朝历史方向滚动＝离开底部，锁定。
    // 动画进行中（smooth 贴底未到岸）的滚动是动画自身送达，不翻转。
    if (userInitiatedScroll && !followAnimating) {
      const before = followBottom.value;
      if (nextScrollTop > previousScrollTop) {
        if (!options.suppressFollowIntent) followBottom.value = true;
      } else if (nextScrollTop < previousScrollTop) {
        if (!options.suppressFollowIntent) followBottom.value = false;
      }
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

  // 显式表达贴底意图（如点击「回到底部」）：解锁跟随
  function startFollowBottom() {
    if (!followBottom.value) {
      probeChatScroll("startFollowBottom", { followBottom: true });
    }
    followBottom.value = true;
  }

  // 显式锁定跟随：跳转（时间线、跳转到用户消息、发送后的自动上推）都会把视口挪走，
  // 锁定后流式内容增长不再贴底，直到用户再次表达「滚到最下」的意图
  function stopFollowBottom() {
    if (followBottom.value) {
      probeChatScroll("stopFollowBottom", {});
    }
    followBottom.value = false;
    followAnimTarget = null;
  }

  // smooth 贴底起飞前登记未完成 target；到岸由 onScroll 按贴底阈值清掉。
  function setFollowAnimating(targetTop: number) {
    followAnimTarget = targetTop;
  }

  function isFollowAnimating() {
    return followAnimTarget !== null;
  }

  // 用户手势打断：先把进行中的 smooth 瞬冻在当前位置，再交还意图归因，
  // 避免 smooth 动画与人手滚动互相顶牛（与 harness interrupt 同语义）。
  function interruptFollowAnimation() {
    const el = scrollContainer.value;
    if (!el || followAnimTarget === null) return false;
    followAnimTarget = null;
    try {
      el.scrollTo({ top: el.scrollTop, behavior: "instant" });
    } catch {
      el.scrollTop = el.scrollTop;
    }
    return true;
  }

  function noteWheelScrollIntent() {
    interruptFollowAnimation();
    wheelScrollIntentUntil = Date.now() + 500;
  }

  function noteKeyboardScrollIntent() {
    interruptFollowAnimation();
    wheelScrollIntentUntil = Date.now() + 500;
  }

  function endPointerScrollIntent() {
    pointerScrollIntentActive = false;
    window.removeEventListener("pointerup", endPointerScrollIntent);
    window.removeEventListener("pointercancel", endPointerScrollIntent);
  }

  function beginPointerScrollIntent() {
    interruptFollowAnimation();
    pointerScrollIntentActive = true;
    window.addEventListener("pointerup", endPointerScrollIntent);
    window.addEventListener("pointercancel", endPointerScrollIntent);
  }

  onMounted(() => {
    nextTick(() => {
      updateJumpToBottomOffset();
      updateLatestOwnElasticMinHeight();
      restartScrollIdleTimer();
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
    if (previewIdleTimer) {
      clearTimeout(previewIdleTimer);
      previewIdleTimer = null;
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
        restartScrollIdleTimer();
        const el = scrollContainer.value;
        if (el) {
          sessionControlPanelVisible.value = updateScrollPositionState(el);
          lastScrollTop.value = el.scrollTop;
          userScrollingDown.value = false;
          userScrollingUp.value = false;
          // 切换会话属于「定位到新消息」，不继承上一会话的跟随意图
          followBottom.value = false;
          followAnimTarget = null;
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
    scrollIdle,
    sessionFloatDockStyle,
    toolbarReservedHeight,
    onScroll,
    noteWheelScrollIntent,
    noteKeyboardScrollIntent,
    beginPointerScrollIntent,
    setFollowAnimating,
    isFollowAnimating,
    interruptFollowAnimation,
    prepareBottomAlignmentLayout,
  };
}
