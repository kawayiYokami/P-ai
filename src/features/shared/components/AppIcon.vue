<template>
  <HugeiconsIcon
    :icon="icon"
    :size="size"
    :stroke-width="strokeWidthNumber"
  />
</template>

<script setup lang="ts">
/**
 * 全项目统一的图标组件。调用方只写语义名，具体图标库藏在 icon-map.ts 里。
 *
 * - 颜色跟随 currentColor，不写死色值，DaisyUI 主题切换自然生效
 * - 尺寸默认 16；仍可继续用 class="h-4 w-4" 这种写法压覆盖，宽度高度以 CSS 为准
 * - 线条粗细默认 2，与换库前 lucide 的默认值一致；个别地方要更粗就传 stroke-width
 */
import { computed } from "vue";
import { HugeiconsIcon } from "@hugeicons/vue";
import { APP_ICON_MAP, type AppIconName } from "../icons/icon-map";

const props = withDefaults(
  defineProps<{
    name: AppIconName;
    size?: number | string;
    strokeWidth?: number | string;
  }>(),
  {
    size: 16,
    strokeWidth: 2,
  },
);

const icon = computed(() => APP_ICON_MAP[props.name]);

const strokeWidthNumber = computed(() => {
  const value = typeof props.strokeWidth === "string" ? Number.parseFloat(props.strokeWidth) : props.strokeWidth;
  return Number.isFinite(value) && value > 0 ? value : 2;
});
</script>
