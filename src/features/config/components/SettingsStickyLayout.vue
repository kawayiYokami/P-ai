<template>
  <div class="relative flex h-full min-h-0 flex-col">
    <div v-if="$slots.header" class="shrink-0 border-b border-base-300">
      <!-- 行高固定 40px：这一行放的是 tab 条或搜索框，两者的高度与上下留白必须一致，
           不能让"有没有 tab"改变整行高度（之前 pb 随 tabs 数量变，56px 与 40px 两种行高）。 -->
      <!-- 宽度只由 contentClass 决定：这里再写 `w-full` 会和定宽类打架，谁生效看样式生成顺序。 -->
      <div class="flex h-10 items-center px-4" :class="[contentClass, headerClass]">
        <slot name="header" />
      </div>
    </div>

    <div v-if="noScroll" class="relative min-h-0 flex-1 overflow-hidden flex flex-col">
      <div class="h-full min-h-0 flex flex-col flex-1" :class="contentClass">
        <slot />
      </div>
    </div>

    <div v-else class="relative min-h-0 flex-1 overflow-hidden" @mouseenter="scrollbarRef?.reveal()" @mouseleave="scrollbarRef?.hide()">
      <div ref="scrollerRef" class="ecall-floating-scroll-target scrollbar-gutter-stable min-h-0 h-full overflow-y-auto overflow-x-hidden pb-24">
        <div class="p-4" :class="contentClass">
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
  // 内容区不靠"锁总宽"来冻结布局：元素尺寸由各自的固定列宽决定，
  // 窗口拉宽只增加列数，不把已有元素拉伸。这里只给一个很宽的上限，防超宽屏一行排太多。
  contentClass: "mx-auto w-full max-w-[80rem]",
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
