import { computed, nextTick, ref, watch, type Ref } from "vue";

type PaneResizeSide = "left" | "right";

const PANE_RESIZE_MOVE_THRESHOLD = 4;
const PANE_COLLAPSE_EDGE_RATIO = 0.1;

export const PANE_WIDTH_LIMITS = {
  left: { min: 200, max: 320, default: 320 },
  // 主页是固定尺寸的卡片墙：窄于 320px 时放不下两张小卡（140×2 + 卡片间距 + 面板内边距 24）
  right: { min: 320, max: 10000, default: 320 },
} as const;

export const PANE_CENTER_MIN_WIDTH = 350;
export const PANE_OVERLAY_VISIBLE_MARGIN = 56;
/** 窄容器（手机竖屏等）下浮层侧边栏直接全屏覆盖，不再保留主区缝隙。 */
const NARROW_CONTAINER_BREAKPOINT_PX = 768;
export const PANE_WIDTH_STORAGE_KEYS = {
  left: "easy_call.chat_left_sidebar_width.v1",
  right: "easy_call.chat_right_sidebar_width.v1",
} as const;
const LEGACY_PANE_WIDTH_STORAGE_KEYS = {
  left: "easy-call.chat.left-sidebar-width",
  right: "easy-call.chat.right-sidebar-width",
} as const;

export function loadStoredPaneWidth(side: PaneResizeSide): number {
  if (typeof window === "undefined") return PANE_WIDTH_LIMITS[side].default;
  const raw = window.localStorage.getItem(PANE_WIDTH_STORAGE_KEYS[side])
    ?? window.localStorage.getItem(LEGACY_PANE_WIDTH_STORAGE_KEYS[side]);
  const parsed = Number(raw);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : PANE_WIDTH_LIMITS[side].default;
}

export interface UseChatPanesOptions {
  chatLayoutRoot: Ref<HTMLElement | null>;
  toolReviewPanelOpen: Ref<boolean>;
  showSideConversationList: Ref<boolean>;
  syncViewportMetrics: () => void;
  onPaneWidthsChange: (leftWidth: number, rightWidth: number) => void;
  onPaneWidthsCommit: (leftWidth: number, rightWidth: number) => void;
  onPaneCloseRequest: (side: PaneResizeSide) => void;
  onBeforeUnmountCleanup: (fn: () => void) => void;
}

export function useChatPanes(options: UseChatPanesOptions) {
  const { chatLayoutRoot, toolReviewPanelOpen, showSideConversationList, syncViewportMetrics, onPaneWidthsChange, onPaneWidthsCommit, onPaneCloseRequest, onBeforeUnmountCleanup } = options;

  const leftSidebarWidth = ref(loadStoredPaneWidth("left"));
  const rightSidebarWidth = ref(loadStoredPaneWidth("right"));
  const activePaneResizeSide = ref<PaneResizeSide | null>(null);
  const collapsePreviewSide = ref<PaneResizeSide | null>(null);
  const resizeLockedLeftPaneInLayout = ref<boolean | null>(null);
  const resizeLockedRightPaneInLayout = ref<boolean | null>(null);
  const lastOpenedPane = ref<PaneResizeSide | null>(null);
  let paneResizeStartX = 0;
  let paneResizeStartWidth = 0;
  let paneResizePreviousBodyCursor = "";
  let paneResizePreviousBodyUserSelect = "";
  let paneResizeMoved = false;
  let paneResizeHandle: HTMLElement | null = null;
  let paneResizePointerId: number | null = null;

  // ========== pane push animation lock (220ms) ==========

  const isPaneTransitioning = ref(false);
  let paneTransitionTimer: ReturnType<typeof setTimeout> | null = null;

  function notifyPaneTransitionStart() {
    isPaneTransitioning.value = true;
    if (paneTransitionTimer) {
      clearTimeout(paneTransitionTimer);
      paneTransitionTimer = null;
    }
    // fallback auto-unlock if hooks miss (e.g. overlay instant)
    paneTransitionTimer = setTimeout(() => {
      isPaneTransitioning.value = false;
      paneTransitionTimer = null;
      measureContainerWidthImmediate();
      void nextTick(() => syncViewportMetrics());
    }, 320);
  }

  function notifyPaneTransitionEnd() {
    if (paneTransitionTimer) {
      clearTimeout(paneTransitionTimer);
      paneTransitionTimer = null;
    }
    isPaneTransitioning.value = false;
    measureContainerWidthImmediate();
    void nextTick(() => syncViewportMetrics());
  }

  function measureContainerWidthImmediate() {
    const el = chatLayoutRoot.value;
    containerWidth.value = el ? Math.round(el.getBoundingClientRect().width) : 0;
  }

  // ========== responsive layout measurement ==========

  const containerWidth = ref(0);
  let layoutResizeObserver: ResizeObserver | null = null;

  function measureContainerWidth() {
    if (isPaneTransitioning.value) return;
    measureContainerWidthImmediate();
  }

  function effectivePaneWidth(side: PaneResizeSide): number {
    const limits = PANE_WIDTH_LIMITS[side];
    const width = side === "left" ? leftSidebarWidth.value : rightSidebarWidth.value;
    return Math.min(limits.max, Math.max(limits.min, Math.round(width || limits.default)));
  }

  function overlayVisibleWidth(side: PaneResizeSide): number {
    const width = effectivePaneWidth(side);
    if (containerWidth.value <= 0) return width;
    if (containerWidth.value < NARROW_CONTAINER_BREAKPOINT_PX) return containerWidth.value;
    return Math.min(width, Math.max(PANE_WIDTH_LIMITS[side].min, containerWidth.value - PANE_OVERLAY_VISIBLE_MARGIN));
  }

  function overlayMaxWidth(side: PaneResizeSide): number {
    const limits = PANE_WIDTH_LIMITS[side];
    if (containerWidth.value <= 0) return limits.max;
    if (containerWidth.value < NARROW_CONTAINER_BREAKPOINT_PX) return containerWidth.value;
    return Math.min(limits.max, Math.max(limits.min, containerWidth.value - PANE_OVERLAY_VISIBLE_MARGIN));
  }

  function canFitInLayout(leftW: number, rightW: number): boolean {
    return containerWidth.value <= 0 || leftW + PANE_CENTER_MIN_WIDTH + rightW <= containerWidth.value;
  }

  const collapsePreviewWidth = computed(() => {
    if (containerWidth.value <= 0) return 0;
    return Math.max(48, Math.round(containerWidth.value * PANE_COLLAPSE_EDGE_RATIO));
  });

  // ========== layout mode computeds ==========

  const basePaneLayoutState = computed(() => {
    const leftOpen = showSideConversationList.value;
    const rightOpen = toolReviewPanelOpen.value;
    const leftW = effectivePaneWidth("left");
    const rightW = effectivePaneWidth("right");

    if (!leftOpen && !rightOpen) return { left: false, right: false };
    if (!leftOpen) return { left: false, right: canFitInLayout(0, rightW) };
    if (!rightOpen) return { left: canFitInLayout(leftW, 0), right: false };
    if (canFitInLayout(leftW, rightW)) return { left: true, right: true };

    if (lastOpenedPane.value === "left") {
      return { left: false, right: canFitInLayout(0, rightW) };
    }
    if (lastOpenedPane.value === "right") {
      return { left: canFitInLayout(leftW, 0), right: false };
    }

    // Fallback when no reliable open-order is available (e.g. restored state):
    // keep at most one pane embedded so the center area never collapses.
    if (canFitInLayout(leftW, 0)) return { left: true, right: false };
    if (canFitInLayout(0, rightW)) return { left: false, right: true };
    return { left: false, right: false };
  });

  /** Left pane is embedded in the flex layout (not overlay) */
  const leftPaneInLayout = computed(() => {
    if (activePaneResizeSide.value === "left" && resizeLockedLeftPaneInLayout.value === false) {
      return false;
    }
    return basePaneLayoutState.value.left;
  });

  /** Right pane is embedded in the flex layout (not overlay) */
  const rightPaneInLayout = computed(() => {
    if (activePaneResizeSide.value === "right" && resizeLockedRightPaneInLayout.value === false) {
      return false;
    }
    if (activePaneResizeSide.value === "left" && resizeLockedRightPaneInLayout.value === false) {
      return false;
    }
    return basePaneLayoutState.value.right;
  });

  /** Left pane is open but shown as overlay (not in layout) */
  const leftPaneOverlay = computed(() =>
    showSideConversationList.value && !leftPaneInLayout.value,
  );

  /** Right pane is open but shown as overlay (not in layout) */
  const rightPaneOverlay = computed(() =>
    toolReviewPanelOpen.value && !rightPaneInLayout.value,
  );

  const leftPaneVisibleWidth = computed(() =>
    leftPaneOverlay.value ? overlayVisibleWidth("left") : effectivePaneWidth("left"),
  );
  const rightPaneVisibleWidth = computed(() =>
    rightPaneOverlay.value ? overlayVisibleWidth("right") : effectivePaneWidth("right"),
  );

  // ========== width clamping & persistence ==========

  function storePaneWidth(side: PaneResizeSide, width: number) {
    if (typeof window === "undefined") return;
    window.localStorage.setItem(PANE_WIDTH_STORAGE_KEYS[side], String(Math.round(width)));
  }

  function clampPaneWidth(side: PaneResizeSide, width: number): number {
    const limits = PANE_WIDTH_LIMITS[side];
    if (activePaneResizeSide.value === side) {
      return Math.round(Math.min(overlayMaxWidth(side), Math.max(limits.min, width)));
    }
    const cw = containerWidth.value || 0;
    const otherPaneWidth =
      side === "left"
        ? (rightPaneInLayout.value ? rightSidebarWidth.value : 0)
        : (leftPaneInLayout.value ? leftSidebarWidth.value : 0);
    const layoutMax = cw > 0 ? cw - otherPaneWidth - PANE_CENTER_MIN_WIDTH : limits.max;
    const effectiveMax = Math.min(
      limits.max,
      Math.max(limits.min, layoutMax > 0 ? layoutMax : limits.max),
    );
    return Math.round(Math.min(effectiveMax, Math.max(limits.min, width)));
  }

  async function setPaneWidth(side: PaneResizeSide, width: number, persist = false) {
    const nextWidth = clampPaneWidth(side, width);
    if (side === "left") {
      leftSidebarWidth.value = nextWidth;
    } else {
      rightSidebarWidth.value = nextWidth;
    }
    if (persist) storePaneWidth(side, nextWidth);
    await nextTick();
    syncViewportMetrics();
  }

  // ========== pointer resize ==========

  function startPaneResize(side: PaneResizeSide, event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    activePaneResizeSide.value = side;
    resizeLockedLeftPaneInLayout.value = leftPaneInLayout.value;
    resizeLockedRightPaneInLayout.value = rightPaneInLayout.value;
    paneResizeStartX = event.clientX;
    paneResizeStartWidth = side === "left" ? leftPaneVisibleWidth.value : rightPaneVisibleWidth.value;
    paneResizeMoved = false;
    paneResizeHandle = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
    paneResizePointerId = Number.isFinite(event.pointerId) ? event.pointerId : null;
    paneResizeHandle?.setPointerCapture?.(event.pointerId);
    collapsePreviewSide.value = null;
    paneResizePreviousBodyCursor = document.body.style.cursor;
    paneResizePreviousBodyUserSelect = document.body.style.userSelect;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", handlePaneResizeMove);
    window.addEventListener("pointerup", stopPaneResize, { once: true });
    window.addEventListener("pointercancel", stopPaneResize, { once: true });
  }

  function handlePaneResizeMove(event: PointerEvent) {
    const side = activePaneResizeSide.value;
    if (!side) return;
    const pointerDelta = event.clientX - paneResizeStartX;
    if (!paneResizeMoved && Math.abs(pointerDelta) >= PANE_RESIZE_MOVE_THRESHOLD) {
      paneResizeMoved = true;
    }
    const nextWidth = side === "left" ? paneResizeStartWidth + pointerDelta : paneResizeStartWidth - pointerDelta;
    const nextWidthClamped = clampPaneWidth(side, nextWidth);
    if (side === "left") {
      leftSidebarWidth.value = nextWidthClamped;
    } else {
      rightSidebarWidth.value = nextWidthClamped;
    }
    collapsePreviewSide.value = shouldCollapsePaneFromClientX(side, event.clientX) ? side : null;
  }

  function shouldCollapsePaneFromClientX(side: PaneResizeSide, clientX: number): boolean {
    const root = chatLayoutRoot.value;
    if (!root) return false;
    const rect = root.getBoundingClientRect();
    if (!(rect.width > 0)) return false;
    const threshold = rect.width * PANE_COLLAPSE_EDGE_RATIO;
    if (side === "left") {
      return clientX <= rect.left + threshold;
    }
    return clientX >= rect.right - threshold;
  }

  function shouldCollapsePaneFromRelease(side: PaneResizeSide, event?: PointerEvent): boolean {
    if (!paneResizeMoved || !event || event.type !== "pointerup") return false;
    return shouldCollapsePaneFromClientX(side, event.clientX);
  }

  async function stopPaneResize(event?: PointerEvent) {
    const side = activePaneResizeSide.value;
    const shouldCollapse = side ? shouldCollapsePaneFromRelease(side, event) : false;
    const restoreWidthOnCollapse = side
      ? clampPaneWidth(side, paneResizeStartWidth)
      : 0;
    window.removeEventListener("pointermove", handlePaneResizeMove);
    window.removeEventListener("pointerup", stopPaneResize);
    window.removeEventListener("pointercancel", stopPaneResize);
    document.body.style.cursor = paneResizePreviousBodyCursor;
    document.body.style.userSelect = paneResizePreviousBodyUserSelect;
    if (paneResizeHandle && paneResizePointerId !== null && paneResizeHandle.hasPointerCapture?.(paneResizePointerId)) {
      paneResizeHandle.releasePointerCapture(paneResizePointerId);
    }
    paneResizeHandle = null;
    paneResizePointerId = null;
    collapsePreviewSide.value = null;
    activePaneResizeSide.value = null;
    resizeLockedLeftPaneInLayout.value = null;
    resizeLockedRightPaneInLayout.value = null;
    if (side) {
      if (shouldCollapse) {
        if (side === "left") {
          leftSidebarWidth.value = restoreWidthOnCollapse;
        } else {
          rightSidebarWidth.value = restoreWidthOnCollapse;
        }
        storePaneWidth(side, restoreWidthOnCollapse);
        onPaneWidthsCommit(leftSidebarWidth.value, rightSidebarWidth.value);
        onPaneCloseRequest(side);
        void nextTick(() => syncViewportMetrics());
      } else {
        storePaneWidth(side, side === "left" ? leftSidebarWidth.value : rightSidebarWidth.value);
        onPaneWidthsCommit(leftSidebarWidth.value, rightSidebarWidth.value);
      }
    }
    paneResizeMoved = false;
  }

  function adjustPaneWidthByKeyboard(side: PaneResizeSide, delta: number) {
    const currentWidth = side === "left" ? leftPaneVisibleWidth.value : rightPaneVisibleWidth.value;
    setPaneWidth(side, currentWidth + delta, true);
    onPaneWidthsCommit(leftSidebarWidth.value, rightSidebarWidth.value);
  }

  // ========== watchers & lifecycle ==========

  watch(
    [leftSidebarWidth, rightSidebarWidth],
    ([leftWidth, rightWidth]) => {
      onPaneWidthsChange(leftWidth, rightWidth);
    },
    { immediate: true },
  );

  watch(
    () => showSideConversationList.value,
    (open, prevOpen) => {
      if (open && !prevOpen) lastOpenedPane.value = "left";
    },
  );

  watch(
    () => toolReviewPanelOpen.value,
    (open, prevOpen) => {
      if (open && !prevOpen) lastOpenedPane.value = "right";
    },
  );

  // 窄容器下两侧都是覆盖层，同时打开会叠在一起（遮罩只有一层）。
  // 以最后打开的一侧为准，自动收起另一侧，形成切换而不是叠加。
  // 用 lastOpenedPane 当裁判，避免程序同时打开两侧时互相踢成死循环。
  watch(leftPaneOverlay, (open) => {
    if (open && lastOpenedPane.value === "left" && rightPaneOverlay.value) {
      onPaneCloseRequest("right");
    }
  });

  watch(rightPaneOverlay, (open) => {
    if (open && lastOpenedPane.value === "right" && leftPaneOverlay.value) {
      onPaneCloseRequest("left");
    }
  });


  void nextTick(() => {
    measureContainerWidth();
    if (typeof ResizeObserver !== "undefined" && chatLayoutRoot.value) {
      layoutResizeObserver = new ResizeObserver(() => measureContainerWidth());
      layoutResizeObserver.observe(chatLayoutRoot.value);
    }
  });

  onBeforeUnmountCleanup(() => {
    stopPaneResize();
    if (paneTransitionTimer) {
      clearTimeout(paneTransitionTimer);
      paneTransitionTimer = null;
    }
    layoutResizeObserver?.disconnect();
    layoutResizeObserver = null;
  });

  return {
    leftSidebarWidth,
    rightSidebarWidth,
    leftPaneInLayout,
    rightPaneInLayout,
    leftPaneOverlay,
    rightPaneOverlay,
    leftPaneVisibleWidth,
    rightPaneVisibleWidth,
    activePaneResizeSide,
    collapsePreviewSide,
    collapsePreviewWidth,
    isPaneTransitioning,
    notifyPaneTransitionStart,
    notifyPaneTransitionEnd,
    clampPaneWidth,
    setPaneWidth,
    startPaneResize,
    adjustPaneWidthByKeyboard,
  };
}
