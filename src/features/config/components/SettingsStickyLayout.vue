<template>
  <div class="relative flex h-full min-h-0 flex-col">
    <div v-if="$slots.header" class="shrink-0 border-b border-base-300">
      <div class="w-full p-4" :class="contentClass">
        <slot name="header" />
      </div>
    </div>

    <div class="relative min-h-0 flex-1 overflow-hidden" @mouseenter="scrollbarRef?.reveal()" @mouseleave="scrollbarRef?.hide()">
      <div ref="scrollerRef" class="ecall-floating-scroll-target scrollbar-gutter-stable min-h-0 h-full overflow-y-auto overflow-x-hidden pb-24">
        <div class="w-full p-4" :class="contentClass">
          <slot />
        </div>
      </div>
      <FloatingScrollbar ref="scrollbarRef" :target="scrollerRef" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import FloatingScrollbar from "../../shell/components/FloatingScrollbar.vue";

const props = withDefaults(defineProps<{
  contentClass?: string;
}>(), {
  contentClass: "mx-auto max-w-5xl",
});

const scrollerRef = ref<HTMLElement | null>(null);
const scrollbarRef = ref<InstanceType<typeof FloatingScrollbar> | null>(null);
</script>

<style scoped>
.scrollbar-gutter-stable {
  scrollbar-gutter: stable;
}
</style>
