import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type ComputedRef, type Ref } from "vue";

type VirtualListItem = {
  id: string;
};

type VirtualLayoutItem<T extends VirtualListItem> = T & {
  height: number;
  top: number;
  bottom: number;
};

export type VirtualRenderSegment<T extends VirtualListItem> = {
  key: string;
  spacerBefore: number;
  items: VirtualLayoutItem<T>[];
};

type UseChatVirtualListOptions<T extends VirtualListItem> = {
  items: ComputedRef<T[]> | Ref<T[]>;
  scrollContainer: Ref<HTMLElement | null>;
  estimateHeight: (item: T) => number;
  keepAliveIds?: ComputedRef<string[]> | Ref<string[]>;
  overscanPx?: number;
  keepAlivePadding?: number;
};

const DEFAULT_OVERSCAN_PX = 720;
const DEFAULT_KEEP_ALIVE_PADDING = 1;
const BOTTOM_PIN_THRESHOLD_PX = 48;

export function useChatVirtualList<T extends VirtualListItem>(options: UseChatVirtualListOptions<T>) {
  const listContainer = ref<HTMLElement | null>(null);
  const viewportHeight = ref(0);
  const relativeScrollTop = ref(0);
  const pinnedToBottom = ref(true);
  const itemHeightCache = ref<Record<string, number>>({});

  const observedElements = new Map<string, HTMLElement>();
  const pendingViewportScrollAdjustments = new Map<string, { previousHeight: number; normalizedHeight: number }>();
  let itemResizeObserver: ResizeObserver | null = null;
  let viewportResizeObserver: ResizeObserver | null = null;
  let pinToBottomPending = false;
  let pendingViewportScrollAdjustmentFrame = 0;

  const overscanPx = Math.max(0, Math.round(options.overscanPx ?? DEFAULT_OVERSCAN_PX));
  const keepAlivePadding = Math.max(0, Math.round(options.keepAlivePadding ?? DEFAULT_KEEP_ALIVE_PADDING));

  const itemById = computed(() => {
    const map = new Map<string, T>();
    for (const item of options.items.value) {
      map.set(item.id, item);
    }
    return map;
  });

  const layoutItems = computed<VirtualLayoutItem<T>[]>(() => {
    let offset = 0;
    return options.items.value.map((item) => {
      const estimatedHeight = Math.max(1, Math.round(
        itemHeightCache.value[item.id] ?? options.estimateHeight(item),
      ));
      const next: VirtualLayoutItem<T> = {
        ...item,
        height: estimatedHeight,
        top: offset,
        bottom: offset + estimatedHeight,
      };
      offset += estimatedHeight;
      return next;
    });
  });

  const layoutIndexById = computed(() => {
    const map = new Map<string, number>();
    layoutItems.value.forEach((item, index) => {
      map.set(item.id, index);
    });
    return map;
  });

  const totalHeight = computed(() => layoutItems.value[layoutItems.value.length - 1]?.bottom ?? 0);

  function isNearBottom(el: HTMLElement): boolean {
    return el.scrollHeight - (el.scrollTop + el.clientHeight) <= BOTTOM_PIN_THRESHOLD_PX;
  }

  function updateViewportMetrics() {
    const scrollEl = options.scrollContainer.value;
    if (!scrollEl) return;
    viewportHeight.value = scrollEl.clientHeight;
    const listOffsetTop = listContainer.value?.offsetTop ?? 0;
    relativeScrollTop.value = Math.max(0, scrollEl.scrollTop - listOffsetTop);
    pinnedToBottom.value = isNearBottom(scrollEl);
  }

  function scheduleViewportSync() {
    void nextTick(() => {
      requestAnimationFrame(() => {
        updateViewportMetrics();
      });
    });
  }

  function clampIndex(index: number): number {
    return Math.min(Math.max(0, index), layoutItems.value.length - 1);
  }

  function flushPendingViewportScrollAdjustments() {
    pendingViewportScrollAdjustmentFrame = 0;
    const scrollEl = options.scrollContainer.value;
    if (!scrollEl || pendingViewportScrollAdjustments.size <= 0) {
      pendingViewportScrollAdjustments.clear();
      updateViewportMetrics();
      return;
    }
    let totalDelta = 0;
    for (const { previousHeight, normalizedHeight } of pendingViewportScrollAdjustments.values()) {
      totalDelta += normalizedHeight - previousHeight;
    }
    pendingViewportScrollAdjustments.clear();
    if (totalDelta !== 0) {
      scrollEl.scrollTop += totalDelta;
    }
    updateViewportMetrics();
  }

  function findRangeBoundary(targetPx: number): number {
    const items = layoutItems.value;
    if (items.length <= 0) return 0;
    let low = 0;
    let high = items.length - 1;
    while (low < high) {
      const mid = Math.floor((low + high) / 2);
      if (items[mid].bottom < targetPx) low = mid + 1;
      else high = mid;
    }
    return low;
  }

  const primaryRange = computed(() => {
    const items = layoutItems.value;
    if (items.length <= 0) return null;
    if (viewportHeight.value <= 0) return null;
    const viewportStart = Math.max(0, relativeScrollTop.value - overscanPx);
    const viewportEnd = Math.max(0, relativeScrollTop.value + viewportHeight.value + overscanPx);
    const start = clampIndex(findRangeBoundary(viewportStart));
    let end = clampIndex(findRangeBoundary(viewportEnd));
    while (end < items.length - 1 && items[end].bottom < viewportEnd) {
      end += 1;
    }
    return { start, end };
  });

  const mergedRanges = computed(() => {
    const items = layoutItems.value;
    if (items.length <= 0) return [] as Array<{ start: number; end: number }>;
    const ranges: Array<{ start: number; end: number }> = [];
    const primary = primaryRange.value;
    if (primary) ranges.push(primary);
    const keepAliveIds = options.keepAliveIds?.value || [];
    for (const id of keepAliveIds) {
      const index = layoutIndexById.value.get(id);
      if (index === undefined) continue;
      ranges.push({
        start: Math.max(0, index - keepAlivePadding),
        end: Math.min(items.length - 1, index + keepAlivePadding),
      });
    }
    if (ranges.length <= 0) return [];
    ranges.sort((left, right) => left.start - right.start);
    const merged: Array<{ start: number; end: number }> = [];
    for (const range of ranges) {
      const current = merged[merged.length - 1];
      if (!current || range.start > current.end + 1) {
        merged.push({ ...range });
        continue;
      }
      current.end = Math.max(current.end, range.end);
    }
    return merged;
  });

  const renderSegments = computed<VirtualRenderSegment<T>[]>(() => {
    const items = layoutItems.value;
    if (items.length <= 0) return [];
    let consumedHeight = 0;
    return mergedRanges.value.map((range) => {
      const startItem = items[range.start];
      const segmentItems = items.slice(range.start, range.end + 1);
      const spacerBefore = Math.max(0, startItem.top - consumedHeight);
      consumedHeight = segmentItems[segmentItems.length - 1]?.bottom ?? consumedHeight;
      return {
        key: `${range.start}-${range.end}`,
        spacerBefore,
        items: segmentItems,
      };
    });
  });

  const bottomSpacerHeight = computed(() => {
    const lastSegment = renderSegments.value[renderSegments.value.length - 1];
    const renderedBottom = lastSegment?.items[lastSegment.items.length - 1]?.bottom ?? 0;
    return Math.max(0, totalHeight.value - renderedBottom);
  });

  function setCachedHeight(itemId: string, nextHeight: number) {
    const normalizedHeight = Math.max(1, Math.round(nextHeight));
    const sourceItem = itemById.value.get(itemId);
    const previousHeight = itemHeightCache.value[itemId] ?? (sourceItem ? options.estimateHeight(sourceItem) : normalizedHeight);
    if (Math.abs(previousHeight - normalizedHeight) <= 1) return;
    const itemIndex = layoutIndexById.value.get(itemId);
    const itemTop = itemIndex === undefined ? 0 : layoutItems.value[itemIndex]?.top ?? 0;
    const scrollEl = options.scrollContainer.value;
    const shouldPreserveViewport =
      !!scrollEl
      && !pinnedToBottom.value
      && itemTop < relativeScrollTop.value - 1;
    itemHeightCache.value = {
      ...itemHeightCache.value,
      [itemId]: normalizedHeight,
    };
    if (shouldPreserveViewport && scrollEl) {
      pendingViewportScrollAdjustments.set(itemId, {
        previousHeight: pendingViewportScrollAdjustments.get(itemId)?.previousHeight ?? previousHeight,
        normalizedHeight,
      });
      if (!pendingViewportScrollAdjustmentFrame) {
        pendingViewportScrollAdjustmentFrame = requestAnimationFrame(() => {
          flushPendingViewportScrollAdjustments();
        });
      }
    }
  }

  function measureElement(itemId: string, element: HTMLElement) {
    const rect = element.getBoundingClientRect();
    if (rect.height > 0) {
      setCachedHeight(itemId, rect.height);
    }
  }

  function bindItemElement(itemId: string, element: Element | null) {
    const previous = observedElements.get(itemId);
    if (!element || !(element instanceof HTMLElement)) {
      if (previous && itemResizeObserver) {
        itemResizeObserver.unobserve(previous);
      }
      observedElements.delete(itemId);
      return;
    }
    if (previous && previous !== element && itemResizeObserver) {
      itemResizeObserver.unobserve(previous);
    }
    observedElements.set(itemId, element);
    element.dataset.virtualItemId = itemId;
    if (itemResizeObserver) {
      itemResizeObserver.observe(element);
    }
    measureElement(itemId, element);
  }

  function handleScroll() {
    updateViewportMetrics();
  }

  function pinToBottomOnNextLayout() {
    pinToBottomPending = true;
    scheduleViewportSync();
  }

  watch(
    () => options.items.value.map((item) => item.id),
    (nextIds) => {
      const nextIdSet = new Set(nextIds);
      const nextCacheEntries = Object.entries(itemHeightCache.value)
        .filter(([itemId]) => nextIdSet.has(itemId));
      if (nextCacheEntries.length !== Object.keys(itemHeightCache.value).length) {
        itemHeightCache.value = Object.fromEntries(nextCacheEntries);
      }
      for (const [itemId, element] of observedElements.entries()) {
        if (nextIdSet.has(itemId)) continue;
        if (itemResizeObserver) itemResizeObserver.unobserve(element);
        observedElements.delete(itemId);
      }
      scheduleViewportSync();
    },
    { immediate: true },
  );

  watch(totalHeight, () => {
    const scrollEl = options.scrollContainer.value;
    if (!scrollEl) return;
    if (!pinToBottomPending && !pinnedToBottom.value) {
      scheduleViewportSync();
      return;
    }
    void nextTick(() => {
      requestAnimationFrame(() => {
        scrollEl.scrollTop = scrollEl.scrollHeight;
        pinToBottomPending = false;
        updateViewportMetrics();
      });
    });
  });

  onMounted(() => {
    if (typeof ResizeObserver !== "undefined") {
      itemResizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) {
          const target = entry.target;
          if (!(target instanceof HTMLElement)) continue;
          const itemId = String(target.dataset.virtualItemId || "").trim();
          if (!itemId) continue;
          measureElement(itemId, target);
        }
      });
      const scrollEl = options.scrollContainer.value;
      if (scrollEl) {
        viewportResizeObserver = new ResizeObserver(() => {
          updateViewportMetrics();
        });
        viewportResizeObserver.observe(scrollEl);
      }
    }
    scheduleViewportSync();
  });

  onBeforeUnmount(() => {
    if (pendingViewportScrollAdjustmentFrame) {
      cancelAnimationFrame(pendingViewportScrollAdjustmentFrame);
      pendingViewportScrollAdjustmentFrame = 0;
    }
    pendingViewportScrollAdjustments.clear();
    if (itemResizeObserver) {
      itemResizeObserver.disconnect();
      itemResizeObserver = null;
    }
    if (viewportResizeObserver) {
      viewportResizeObserver.disconnect();
      viewportResizeObserver = null;
    }
    observedElements.clear();
  });

  return {
    listContainer,
    renderSegments,
    bottomSpacerHeight,
    bindItemElement,
    handleScroll,
    pinToBottomOnNextLayout,
    syncViewportMetrics: scheduleViewportSync,
  };
}
