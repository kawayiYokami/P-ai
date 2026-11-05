<template>
  <div
    v-if="canScroll"
    ref="trackRef"
    class="floating-scrollbar-track absolute z-20 transition-opacity"
    :class="[trackClassName, persistent || scrollbarVisible || dragging ? 'opacity-100' : 'opacity-0']"
    @mouseenter="reveal"
    @mouseleave="hide"
    @pointerdown="onTrackPointerDown"
  >
    <div
      ref="thumbRef"
      class="floating-scrollbar-thumb absolute rounded-full transition-[width,height,background-color] hover:w-2 hover:h-2"
      :class="[thumbClassName, thumbGeometryClassName, dragging ? draggingClassName : '']"
      @pointerdown.stop="onThumbPointerDown"
    ></div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, toRef, watch, type Ref } from "vue";

const props = defineProps<{
  target: HTMLElement | null;
  variant?: "theme" | "code-dark";
  orientation?: "vertical" | "horizontal";
  /** 常驻显示：有可滚动内容时不再随悬停淡出，用于日志等持续增长的列表。 */
  persistent?: boolean;
}>();

const targetRef = toRef(props, "target") as Ref<HTMLElement | null>;
const trackRef = ref<HTMLElement | null>(null);
const thumbRef = ref<HTMLElement | null>(null);
const canScroll = ref(false);
const scrollbarVisible = ref(false);
const dragging = ref(false);
let currentThumbSize = 24;
let currentThumbOffset = 0;
let appliedThumbSize = -1;
let appliedThumbOffset = -1;

let resizeObserver: ResizeObserver | null = null;
let mutationObserver: MutationObserver | null = null;
let observedChild: Element | null = null;
let dragStartPointer = 0;
let dragStartScrollOffset = 0;
let activePointerId: number | null = null;
let observedScroller: HTMLElement | null = null;
let pendingThumbFrame = 0;
let needsExtentRefresh = false;
let cachedClientExtent = 0;
let cachedScrollExtent = 0;
let cachedScrollOffset = 0;

const isHorizontal = computed(() => props.orientation === "horizontal");

const thumbClassName = computed(() => (
  props.variant === "code-dark"
    ? "bg-slate-400/45 hover:bg-slate-300/60"
    : "bg-base-content/30 hover:bg-base-content/45"
));

const trackClassName = computed(() => (
  isHorizontal.value
    ? "bottom-1 left-1 right-1 h-2"
    : "bottom-1 right-1 top-1 w-2"
));

const thumbGeometryClassName = computed(() => (
  isHorizontal.value
    ? "bottom-0 h-1.5"
    : "right-0 w-1.5"
));

const draggingClassName = computed(() => (isHorizontal.value ? "h-2" : "w-2"));

function applyThumbGeometry(size: number, offset: number) {
  const thumb = thumbRef.value;
  if (!thumb) return;
  if (appliedThumbSize !== size) {
    appliedThumbSize = size;
    if (isHorizontal.value) {
      thumb.style.width = `${size}px`;
    } else {
      thumb.style.height = `${size}px`;
    }
  }
  if (appliedThumbOffset !== offset) {
    appliedThumbOffset = offset;
    if (isHorizontal.value) {
      thumb.style.transform = `translateX(${offset}px)`;
    } else {
      thumb.style.transform = `translateY(${offset}px)`;
    }
  }
}

watch(canScroll, (val) => {
  if (val) {
    appliedThumbSize = -1;
    appliedThumbOffset = -1;
    void nextTick(() => applyThumbGeometry(currentThumbSize, currentThumbOffset));
  }
});

function setDocumentDragging(active: boolean) {
  document.body.classList.toggle("floating-scrollbar-dragging", active);
}

function updateThumbNow(forceExtentRefresh = false) {
  const scroller = targetRef.value;
  if (!scroller) return;

  if (forceExtentRefresh || cachedClientExtent === 0 || cachedScrollExtent === 0) {
    cachedClientExtent = isHorizontal.value ? scroller.clientWidth : scroller.clientHeight;
    cachedScrollExtent = isHorizontal.value ? scroller.scrollWidth : scroller.scrollHeight;
    cachedScrollOffset = isHorizontal.value ? scroller.scrollLeft : scroller.scrollTop;
  }
  const clientExtent = cachedClientExtent;
  const scrollExtent = cachedScrollExtent;
  const scrollOffset = cachedScrollOffset;
  const scrollable = scrollExtent > clientExtent + 1;
  if (canScroll.value !== scrollable) {
    canScroll.value = scrollable;
  }
  if (!scrollable) {
    currentThumbOffset = 0;
    return;
  }

  const trackExtent = Math.max(clientExtent - 8, 0);
  const size = Math.max(24, Math.round((clientExtent / scrollExtent) * trackExtent));
  const maxOffset = Math.max(trackExtent - size, 0);
  const nextOffset = maxOffset === 0
    ? 0
    : Math.round((scrollOffset / Math.max(scrollExtent - clientExtent, 1)) * maxOffset);
  currentThumbSize = size;
  currentThumbOffset = nextOffset;
  applyThumbGeometry(size, nextOffset);
}

function updateThumb(forceExtentRefresh = false) {
  if (forceExtentRefresh) {
    needsExtentRefresh = true;
  }
  if (pendingThumbFrame) return;
  pendingThumbFrame = requestAnimationFrame(() => {
    pendingThumbFrame = 0;
    const force = needsExtentRefresh;
    needsExtentRefresh = false;
    updateThumbNow(force);
  });
}

function reveal() {
  updateThumbNow(true);
  if (!canScroll.value) return;
  scrollbarVisible.value = true;
}

function hide() {
  if (dragging.value) return;
  scrollbarVisible.value = false;
}

function handleScroll(e?: Event) {
  const target = (e?.currentTarget || e?.target || targetRef.value) as HTMLElement | null;
  if (target) {
    cachedScrollOffset = isHorizontal.value ? target.scrollLeft : target.scrollTop;
  }
  updateThumb();
  if (!scrollbarVisible.value) scrollbarVisible.value = true;
}

function disconnectObservers() {
  resizeObserver?.disconnect();
  resizeObserver = null;
  mutationObserver?.disconnect();
  mutationObserver = null;
  observedChild = null;
  if (pendingThumbFrame) {
    cancelAnimationFrame(pendingThumbFrame);
    pendingThumbFrame = 0;
  }
}

function removeObservedScrollerListener(scroller = observedScroller) {
  scroller?.removeEventListener("scroll", handleScroll);
  if (!scroller || scroller === observedScroller) {
    observedScroller = null;
  }
}

function syncObservedChild(scroller: HTMLElement) {
  const firstChild = scroller.firstElementChild;
  if (firstChild === observedChild) return;
  if (observedChild && resizeObserver) {
    resizeObserver.unobserve(observedChild);
  }
  observedChild = firstChild;
  if (observedChild && resizeObserver) {
    resizeObserver.observe(observedChild);
  }
}

function observeScroller(scroller: HTMLElement | null) {
  removeObservedScrollerListener();
  disconnectObservers();
  cachedClientExtent = 0;
  cachedScrollExtent = 0;
  cachedScrollOffset = 0;
  appliedThumbSize = -1;
  appliedThumbOffset = -1;
  if (!scroller) {
    canScroll.value = false;
    return;
  }

  observedScroller = scroller;
  scroller.addEventListener("scroll", handleScroll, { passive: true });
  if (typeof ResizeObserver !== "undefined") {
    resizeObserver = new ResizeObserver(() => updateThumb(true));
    resizeObserver.observe(scroller);
    syncObservedChild(scroller);
  }
  if (typeof MutationObserver !== "undefined") {
    mutationObserver = new MutationObserver(() => {
      syncObservedChild(scroller);
      updateThumb(true);
    });
    mutationObserver.observe(scroller, { childList: true });
  }
  void nextTick(() => updateThumb(true));
}

function scrollByThumbDelta(deltaPointer: number) {
  const scroller = targetRef.value;
  if (!scroller) return;
  const clientExtent = cachedClientExtent || (isHorizontal.value ? scroller.clientWidth : scroller.clientHeight);
  const scrollExtent = cachedScrollExtent || (isHorizontal.value ? scroller.scrollWidth : scroller.scrollHeight);
  const maxScrollOffset = Math.max(scrollExtent - clientExtent, 0);
  const maxThumbOffset = Math.max(clientExtent - 8 - currentThumbSize, 0);
  if (maxScrollOffset <= 0 || maxThumbOffset <= 0) return;
  const nextOffset = dragStartScrollOffset + (deltaPointer / maxThumbOffset) * maxScrollOffset;
  if (isHorizontal.value) {
    scroller.scrollLeft = nextOffset;
  } else {
    scroller.scrollTop = nextOffset;
  }
}

function onDocumentPointerMove(event: PointerEvent) {
  if (!dragging.value || activePointerId !== event.pointerId) return;
  event.preventDefault();
  scrollByThumbDelta((isHorizontal.value ? event.clientX : event.clientY) - dragStartPointer);
}

function stopDragging() {
  if (!dragging.value) return;
  dragging.value = false;
  setDocumentDragging(false);
  document.removeEventListener("pointermove", onDocumentPointerMove);
  document.removeEventListener("pointerup", onDocumentPointerUp);
  document.removeEventListener("pointercancel", onDocumentPointerUp);
  if (activePointerId !== null) {
    thumbRef.value?.releasePointerCapture?.(activePointerId);
  }
  activePointerId = null;
  updateThumb();
  if (!trackRef.value?.matches(":hover")) hide();
}

function onDocumentPointerUp(event: PointerEvent) {
  if (activePointerId !== null && activePointerId !== event.pointerId) return;
  stopDragging();
}

function onThumbPointerDown(event: PointerEvent) {
  if (event.button !== 0) return;
  const scroller = targetRef.value;
  if (!scroller) return;
  event.preventDefault();
  reveal();
  dragging.value = true;
  activePointerId = event.pointerId;
  dragStartPointer = isHorizontal.value ? event.clientX : event.clientY;
  dragStartScrollOffset = isHorizontal.value ? scroller.scrollLeft : scroller.scrollTop;
  setDocumentDragging(true);
  thumbRef.value?.setPointerCapture?.(event.pointerId);
  document.addEventListener("pointermove", onDocumentPointerMove, { passive: false });
  document.addEventListener("pointerup", onDocumentPointerUp);
  document.addEventListener("pointercancel", onDocumentPointerUp);
}

function onTrackPointerDown(event: PointerEvent) {
  if (event.button !== 0 || event.target === thumbRef.value) return;
  const scroller = targetRef.value;
  const track = trackRef.value;
  if (!scroller || !track) return;
  event.preventDefault();
  reveal();
  const trackRect = track.getBoundingClientRect();
  const trackStart = isHorizontal.value ? trackRect.left : trackRect.top;
  const trackExtent = isHorizontal.value ? trackRect.width : trackRect.height;
  const pointer = isHorizontal.value ? event.clientX : event.clientY;
  const nextThumbOffset = Math.min(
    Math.max(pointer - trackStart - currentThumbSize / 2, 0),
    Math.max(trackExtent - currentThumbSize, 0),
  );
  const maxThumbOffset = Math.max(trackExtent - currentThumbSize, 0);
  const clientExtent = cachedClientExtent || (isHorizontal.value ? scroller.clientWidth : scroller.clientHeight);
  const scrollExtent = cachedScrollExtent || (isHorizontal.value ? scroller.scrollWidth : scroller.scrollHeight);
  const maxScrollOffset = Math.max(scrollExtent - clientExtent, 0);
  const nextScrollOffset = maxThumbOffset === 0 ? 0 : (nextThumbOffset / maxThumbOffset) * maxScrollOffset;
  if (isHorizontal.value) {
    scroller.scrollLeft = nextScrollOffset;
  } else {
    scroller.scrollTop = nextScrollOffset;
  }
}

defineExpose({
  reveal,
  hide,
  updateThumb,
});

onMounted(() => observeScroller(targetRef.value));

watch(targetRef, (nextScroller, previousScroller) => {
  removeObservedScrollerListener(previousScroller);
  observeScroller(nextScroller);
});

onBeforeUnmount(() => {
  stopDragging();
  removeObservedScrollerListener();
  disconnectObservers();
});
</script>

<style scoped>
.floating-scrollbar-track {
  cursor: pointer;
  touch-action: none;
}

.floating-scrollbar-thumb {
  cursor: grab;
  touch-action: none;
}

.floating-scrollbar-thumb:active {
  cursor: grabbing;
}
</style>

<style>
body.floating-scrollbar-dragging {
  cursor: grabbing !important;
  user-select: none !important;
}

body.floating-scrollbar-dragging * {
  cursor: grabbing !important;
  user-select: none !important;
}
</style>
