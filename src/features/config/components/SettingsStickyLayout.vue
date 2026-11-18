<template>
  <div class="relative flex h-full min-h-0 flex-col">
    <div v-if="$slots.header" class="shrink-0 border-b border-base-300">
      <!-- 顶部不再留 16px：面包屑那一行由设置页外壳统一占位，头部直接贴它下面。 -->
      <div class="w-full px-4 pt-1" :class="[contentClass, headerClass]">
        <slot name="header" />
      </div>
    </div>

    <div v-if="noScroll" class="relative min-h-0 flex-1 overflow-hidden flex flex-col">
      <div class="h-full w-full min-h-0 flex flex-col flex-1" :class="contentClass">
        <slot />
      </div>
    </div>

    <div v-else class="relative min-h-0 flex-1 overflow-hidden" @mouseenter="scrollbarRef?.reveal()" @mouseleave="scrollbarRef?.hide()">
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

// headerClass：附加到头部内层的类，用于让内容（如 tabs-border 下划线）贴合头部分界线
const props = withDefaults(defineProps<{
  contentClass?: string;
  headerClass?: string;
  noScroll?: boolean;
}>(), {
  contentClass: "mx-auto max-w-5xl",
  headerClass: "",
  noScroll: false,
});

const scrollerRef = ref<HTMLElement | null>(null);
const scrollbarRef = ref<InstanceType<typeof FloatingScrollbar> | null>(null);
</script>

<style scoped>
.scrollbar-gutter-stable {
  scrollbar-gutter: stable;
}
</style>
