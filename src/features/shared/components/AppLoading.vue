<template>
  <span
    class="app-loading"
    :class="{ 'app-loading-static': reducedMotion }"
    :style="rootStyle"
    :role="label ? 'status' : undefined"
    :aria-label="label || undefined"
    :aria-hidden="label ? undefined : 'true'"
  >
    <i></i><i></i><i></i><i></i>
  </span>
</template>

<script setup lang="ts">
/**
 * 全项目统一的加载指示器：四叶草四点灯。
 *
 * - 四片叶子各是一个直角加三个圆角，直角朝中心拼成尖十字，外缘是圆弧，彼此留缝
 * - 盒子尺寸直接取 DaisyUI 的 `--size-selector`，与 `.loading-xs/sm/md/lg` 同源，
 *   全局字号或尺寸变量调整时一起跟着变，不会留在原地
 * - 垂直对齐与 DaisyUI `.loading` 一致用 middle，保证并排文字时不往下掉
 * - 颜色跟随 currentColor，主题切换自然生效；渐亮渐暗，沿 左上→右上→右下→左下 推进
 */
import { computed } from "vue";

type SizeToken = "xs" | "sm" | "md" | "lg";

/** 与 DaisyUI `.loading-*` 同源的倍数 */
const SIZE_MULTIPLIER: Record<SizeToken, number> = {
  xs: 4,
  sm: 5,
  md: 6,
  lg: 7,
};

const props = withDefaults(
  defineProps<{
    /** 尺寸档：xs 等于 .loading-xs、sm 等于 .loading-sm、md 等于 .loading 默认值、lg 等于 .loading-lg；也可直接给 CSS 长度 */
    size?: SizeToken | number | string;
    /** 转一圈的毫秒数，默认 1500 */
    duration?: number;
    /** 叶片之间留的缝，单位 em，默认 0.05 */
    gap?: number;
    /** 读屏播报文案；不传则该指示器对辅助技术隐藏 */
    label?: string;
    /** 系统开启「减少动态效果」时停止动画 */
    reducedMotion?: boolean;
  }>(),
  {
    size: "md",
    duration: 1500,
    gap: 0.05,
    label: "",
    reducedMotion: false,
  },
);

const sizeValue = computed(() => {
  const raw = props.size;
  if (typeof raw === "number") return `${raw}px`;
  if (typeof raw === "string" && raw in SIZE_MULTIPLIER) {
    return `calc(var(--size-selector, 0.25rem) * ${SIZE_MULTIPLIER[raw as SizeToken]})`;
  }
  return String(raw);
});

const rootStyle = computed(() => ({
  fontSize: sizeValue.value,
  "--app-loading-dur": `${props.duration}ms`,
  "--app-loading-gap": `${props.gap}em`,
}));
</script>

<style scoped>
.app-loading {
  display: inline-grid;
  grid-template-columns: repeat(2, 1fr);
  grid-template-rows: repeat(2, 1fr);
  gap: var(--app-loading-gap);
  width: 1em;
  height: 1em;
  /* 与 DaisyUI 的 .loading 保持一致：middle 对齐，并排文字时不会往下掉 */
  vertical-align: middle;
  /* 作为 flex 子元素时，别跟着基线对齐把同一行的文字顶偏 */
  align-self: center;
  line-height: 0;
  flex: none;
}

.app-loading i {
  background: currentColor;
  opacity: 0.12;
  animation: app-loading-run var(--app-loading-dur) linear infinite;
}

/* 直角朝中心：左上叶片直角在右下、右上在左下、左下在右上、右下在左上。
   负延迟越负 = 相位越超前 = 越先亮，所以顺时针「左上→右上→右下→左下」
   要求左上最超前（-0.75）、左下最滞后（0）。 */
.app-loading i:nth-child(1) {
  border-radius: 100% 100% 0 100%;
  animation-delay: calc(var(--app-loading-dur) * -0.75);
}

.app-loading i:nth-child(2) {
  border-radius: 100% 100% 100% 0;
  animation-delay: calc(var(--app-loading-dur) / -2);
}

.app-loading i:nth-child(4) {
  border-radius: 0 100% 100% 100%;
  animation-delay: calc(var(--app-loading-dur) / -4);
}

.app-loading i:nth-child(3) {
  border-radius: 100% 0 100% 100%;
  animation-delay: 0s;
}

/* 慢慢亮上去再缓缓压下去，全程连续插值，不做瞬间跳变 */
@keyframes app-loading-run {
  0% {
    opacity: 0.12;
  }
  28% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
  78% {
    opacity: 0.2;
  }
  100% {
    opacity: 0.12;
  }
}

.app-loading-static i {
  animation: none;
  opacity: 0.5;
}

@media (prefers-reduced-motion: reduce) {
  .app-loading i {
    animation-duration: 3s;
  }
}
</style>
