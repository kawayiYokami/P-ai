<template>
  <div
    class="overlay-scroll-area relative"
    @mouseenter="revealScrollbars"
    @mouseleave="hideScrollbars"
  >
    <div
      ref="scrollerRef"
      class="overlay-scroll-area-scroller"
      :class="[props.scrollerClass, scrollerOverflowClass]"
      :data-overscroll="props.overscroll"
      @wheel="handleWheel"
    >
      <slot />
    </div>
    <FloatingScrollbar
      v-if="showVerticalScrollbar"
      ref="verticalScrollbarRef"
      :target="scrollerRef"
      :variant="variant"
      :persistent="persistent"
    />
    <FloatingScrollbar
      v-if="showHorizontalScrollbar"
      ref="horizontalScrollbarRef"
      :target="scrollerRef"
      :variant="variant"
      :persistent="persistent"
      orientation="horizontal"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import FloatingScrollbar from "../../shell/components/FloatingScrollbar.vue";

const props = withDefaults(defineProps<{
  orientation?: "vertical" | "horizontal" | "both";
  variant?: "theme" | "code-dark";
  scrollerClass?: string;
  persistent?: boolean;
  hideScrollbar?: boolean;
  isolateScroll?: boolean;
  /** 滚到边界后是否继续带动外层滚动。contain=不带动（浮层默认），auto=带动（聊天内容区）。 */
  overscroll?: "contain" | "auto";
}>(), {
  orientation: "vertical",
  variant: "theme",
  scrollerClass: "",
  persistent: false,
  hideScrollbar: false,
  isolateScroll: false,
  overscroll: "contain",
});

const scrollerRef = ref<HTMLElement | null>(null);
const verticalScrollbarRef = ref<InstanceType<typeof FloatingScrollbar> | null>(null);
const horizontalScrollbarRef = ref<InstanceType<typeof FloatingScrollbar> | null>(null);

const showVerticalScrollbar = computed(() => !props.hideScrollbar && (props.orientation === "vertical" || props.orientation === "both"));
const showHorizontalScrollbar = computed(() => !props.hideScrollbar && (props.orientation === "horizontal" || props.orientation === "both"));

const scrollerOverflowClass = computed(() => {
  if (props.orientation === "horizontal") return "overflow-x-auto overflow-y-hidden";
  if (props.orientation === "both") return "overflow-auto";
  return "overflow-x-hidden overflow-y-auto";
});

function revealScrollbars() {
  verticalScrollbarRef.value?.reveal();
  horizontalScrollbarRef.value?.reveal();
}

function hideScrollbars() {
  verticalScrollbarRef.value?.hide();
  horizontalScrollbarRef.value?.hide();
}

function updateScrollbars() {
  verticalScrollbarRef.value?.updateThumb();
  horizontalScrollbarRef.value?.updateThumb();
}

function wheelDeltaUnit(event: WheelEvent, scroller: HTMLElement) {
  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) return 16;
  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) return scroller.clientWidth;
  return 1;
}

function isChildVerticallyScrollable(target: EventTarget | null, boundary: HTMLElement): boolean {
  let curr = target as HTMLElement | null;
  while (curr && curr !== boundary) {
    if (curr.scrollHeight > curr.clientHeight) {
      const style = window.getComputedStyle(curr);
      if (style.overflowY === "auto" || style.overflowY === "scroll") {
        return true;
      }
    }
    curr = curr.parentElement;
  }
  return false;
}

function handleWheel(event: WheelEvent) {
  const scroller = scrollerRef.value;
  if (!scroller) return;

  // 1. 若配置了独立隔离滚动，阻止事件冒泡到外层祖先容器
  if (props.isolateScroll) {
    event.stopPropagation();
  }

  // 2. 仅横向滚动区需要做普通滚轮到 scrollLeft 的映射
  if (props.orientation !== "horizontal") return;

  const maxScrollLeft = Math.max(scroller.scrollWidth - scroller.clientWidth, 0);
  if (maxScrollLeft <= 0) return;

  // 若事件来源于能够垂直滚动的子元素内部，且用户未按 Shift 或非主横向手势，
  // 必须将垂直滚轮完整留给子容器，坚决不转化为当前横向容器的滚动
  const isTargetInVerticalScroller = isChildVerticallyScrollable(event.target, scroller);
  if (isTargetInVerticalScroller && !event.shiftKey && Math.abs(event.deltaY) >= Math.abs(event.deltaX)) {
    return;
  }

  const unit = wheelDeltaUnit(event, scroller);
  const deltaX = event.deltaX * unit;
  const deltaY = event.deltaY * unit;
  const delta = Math.abs(deltaX) > Math.abs(deltaY)
    ? deltaX
    : (isTargetInVerticalScroller ? 0 : deltaY);
  if (!delta) return;

  const current = scroller.scrollLeft;
  const next = Math.min(maxScrollLeft, Math.max(0, current + delta));
  if (next === current) return;

  event.preventDefault();
  scroller.scrollLeft = next;
  revealScrollbars();
  updateScrollbars();
}

watch(
  () => props.orientation,
  () => {
    void nextTick(updateScrollbars);
  },
);

defineExpose({
  scrollerRef,
  updateThumb: updateScrollbars,
  reveal: revealScrollbars,
  hide: hideScrollbars,
});
</script>

<style scoped>
.overlay-scroll-area-scroller {
  scrollbar-width: none;
  overscroll-behavior: contain;
}

/* 属性选择器比基底规则多一段，能压过它；从 scroller-class 传 Tailwind 的 .overscroll-auto 压不动 */
.overlay-scroll-area-scroller[data-overscroll="auto"] {
  overscroll-behavior: auto;
}

.overlay-scroll-area-scroller::-webkit-scrollbar {
  display: none;
}
</style>
