<template>
  <div class="relative overflow-hidden" @mouseenter="revealScrollbar" @mouseleave="hideScrollbar">
    <div
      ref="scrollerRef"
      class="conversation-list-scroll h-full overflow-y-auto"
      @scroll.passive="handleScroll"
    >
      <slot />
    </div>
    <div v-if="canScroll" class="pointer-events-none absolute bottom-1 right-1 top-1 w-1.5">
      <div
        class="absolute right-0 w-1.5 rounded-full bg-base-content/30"
        :class="scrollbarVisible ? 'opacity-100' : 'opacity-0'"
        :style="thumbStyle"
      ></div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";

const scrollerRef = ref<HTMLElement | null>(null);
const canScroll = ref(false);
const scrollbarVisible = ref(false);
const thumbHeight = ref(24);
const thumbTop = ref(0);

let resizeObserver: ResizeObserver | null = null;
let mutationObserver: MutationObserver | null = null;
const thumbStyle = computed(() => ({
  height: `${thumbHeight.value}px`,
  transform: `translateY(${thumbTop.value}px)`,
}));

function updateThumb() {
  const scroller = scrollerRef.value;
  if (!scroller) return;

  const { clientHeight, scrollHeight, scrollTop } = scroller;
  const scrollable = scrollHeight > clientHeight + 1;
  canScroll.value = scrollable;
  if (!scrollable) return;

  const trackHeight = Math.max(clientHeight - 8, 0);
  const height = Math.max(24, Math.round((clientHeight / scrollHeight) * trackHeight));
  const maxTop = Math.max(trackHeight - height, 0);
  thumbHeight.value = height;
  thumbTop.value = maxTop === 0
    ? 0
    : Math.round((scrollTop / (scrollHeight - clientHeight)) * maxTop);
}

function revealScrollbar() {
  updateThumb();
  if (!canScroll.value) return;
  scrollbarVisible.value = true;
}

function hideScrollbar() {
  scrollbarVisible.value = false;
}

function handleScroll() {
  updateThumb();
  revealScrollbar();
}

onMounted(() => {
  nextTick(updateThumb);
  const scroller = scrollerRef.value;
  if (!scroller) return;

  resizeObserver = new ResizeObserver(updateThumb);
  resizeObserver.observe(scroller);

  mutationObserver = new MutationObserver(updateThumb);
  mutationObserver.observe(scroller, { childList: true, subtree: true });
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  mutationObserver?.disconnect();
});
</script>

<style scoped>
.conversation-list-scroll {
  scrollbar-gutter: auto;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.conversation-list-scroll::-webkit-scrollbar {
  width: 0;
  height: 0;
}
</style>
