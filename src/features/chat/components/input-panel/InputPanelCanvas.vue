<template>
  <!-- 模拟窗口：矩形容器，只负责衬底色看对比，不参与输入面板逻辑 -->
  <div
    class="transition-all duration-300 ease-in-out"
    :class="[bgClass, isRounded ? 'rounded-box' : '']"
  >
    <slot />
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(defineProps<{ bg?: "base-200" | "base-100" | "base-300"; isRounded?: boolean }>(), {
  bg: "base-200",
  isRounded: false,
});

const bgClass = computed(() => {
  if (props.bg === "base-100") return "bg-base-100";
  if (props.bg === "base-300") return "bg-base-300";
  return "bg-base-200";
});
</script>
