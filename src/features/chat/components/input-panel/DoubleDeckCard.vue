<template>
  <!-- 双层卡：两张同宽扑克牌叠放，底卡包面卡，露头内容在面卡上方 -->
  <div
    class="flex flex-col p-0 transition-all duration-300 ease-in-out"
    :class="[bgClass, roundedClass, 'gap-0']"
  >
    <!-- 露头：底卡多出来的那截，空时收到零 -->
    <div
      class="grid transition-all duration-300 ease-in-out"
      :class="extraVisible ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'"
    >
      <div class="min-h-0 overflow-hidden">
        <div class="w-full overflow-hidden bg-transparent">
          <slot name="extra" />
        </div>
      </div>
    </div>
    <!-- 面卡：永远在底部，内容层只留内边距，露头时加顶阴影显叠放。
         底色与磨砂单独铺在背板层，内容层不再带 backdrop-filter：
         背板一旦包住内容就会新建堆叠上下文，把输入卡向上弹出的指令/提及浮层
         永远压在会话悬浮工具条之下。 -->
    <div
      class="relative flex flex-col p-2 transition-all duration-300 ease-in-out"
      :class="mainClass"
    >
      <div
        class="pointer-events-none absolute inset-0 bg-base-100/90 backdrop-blur-md"
        :class="mainClass"
        aria-hidden="true"
      ></div>
      <div class="relative flex flex-col">
        <slot name="main" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(defineProps<{
  extraVisible?: boolean;
  isRounded?: boolean;
  bg?: "base-100" | "base-200" | "base-300";
}>(), {
  extraVisible: false,
  isRounded: false,
  bg: "base-200",
});

const bgClass = computed(() => {
  if (props.bg === "base-100") return "double-deck-bg-base-100";
  if (props.bg === "base-300") return "double-deck-bg-base-300";
  return "double-deck-bg-base-200";
});

const roundedClass = computed(() =>
  props.isRounded
    ? "rounded-[20px] border border-base-300 shadow-xl"
    : "rounded-none border-0 border-t border-base-300 shadow-none",
);

const mainClass = computed(() => {
  const classes: string[] = [];
  if (props.isRounded) classes.push("rounded-[16px]");
  return classes;
});
</script>

<style scoped>
.double-deck-bg-base-100 {
  background-color: color-mix(in srgb, var(--color-primary) 2%, var(--color-base-100));
}
.double-deck-bg-base-200 {
  background-color: color-mix(in srgb, var(--color-primary) 2%, var(--color-base-200));
}
.double-deck-bg-base-300 {
  background-color: color-mix(in srgb, var(--color-primary) 2%, var(--color-base-300));
}
</style>
