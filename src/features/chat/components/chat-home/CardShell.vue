<template>
  <div
    class="ecall-home-card flex min-w-0 flex-col gap-3 overflow-hidden rounded-box border border-base-content/10 bg-base-100 p-3.5 transition-all duration-150"
    :class="[
      variant === 'wide' ? 'ecall-home-card-wide' : 'ecall-home-card-small',
      layout === 'tile' ? 'ecall-home-card-tile' : '',
      interactive
        ? 'cursor-pointer hover:bg-base-200/70 active:scale-[0.99] focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-primary/50'
        : '',
    ]"
    :role="interactive ? 'button' : undefined"
    :tabindex="interactive ? 0 : undefined"
    @click="handleSelect"
    @keydown.enter.prevent="handleSelect"
    @keydown.space.prevent="handleSelect"
  >
    <template v-if="layout === 'tile'">
      <span class="ecall-home-card-icon ecall-home-card-icon-tile" :class="toneTileClass">
        <component :is="icon" class="size-5" aria-hidden="true" />
      </span>
      <span class="min-w-0 max-w-full text-sm font-medium text-base-content/85">{{ label }}</span>
      <slot />
    </template>
    <template v-else>
      <div class="flex min-w-0 shrink-0 items-center gap-2">
        <span class="ecall-home-card-icon" :class="toneRowClass">
          <component :is="icon" class="size-3.5 shrink-0" aria-hidden="true" />
        </span>
        <span v-if="pulsing" class="relative flex h-2 w-2 shrink-0 items-center justify-center">
          <span class="absolute inline-flex h-full w-full animate-ping rounded-full opacity-75" :class="toneDot.ping"></span>
          <span class="relative inline-flex h-1.5 w-1.5 rounded-full" :class="toneDot.core"></span>
        </span>
        <span class="min-w-0 flex-1 truncate text-xs font-semibold text-base-content/80">{{ label }}</span>
        <slot name="trailing" />
      </div>
      <div class="flex min-h-0 min-w-0 flex-1 flex-col">
        <slot />
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, type Component } from "vue";

const props = withDefaults(defineProps<{
  label: string;
  icon: Component;
  tone?: "primary" | "secondary" | "info" | "success" | "warning" | "neutral";
  /** wide 为大卡（横跨 2 列、占 1 行），small 为小卡（方形，占 1 行 1 列） */
  variant?: "small" | "wide";
  /** row 为带标题行的内容卡；tile 为入口磁贴（图标与标题居中，不出现标题行） */
  layout?: "row" | "tile";
  interactive?: boolean;
  /** 标题行显示活动脉冲指示灯，形态与运行监控胶囊一致；颜色默认取卡片色调，可用 pulseTone 单独指定 */
  pulsing?: boolean;
  /** 脉冲点颜色；不给则跟随卡片 tone */
  pulseTone?: "primary" | "secondary" | "info" | "success" | "warning" | "neutral";
}>(), {
  tone: "neutral",
  variant: "small",
  layout: "row",
  interactive: false,
  pulsing: false,
  pulseTone: undefined,
});

const emit = defineEmits<{
  (e: "select"): void;
}>();

const toneMap: Record<string, { tile: string; row: string }> = {
  primary: {
    tile: "bg-primary/15 text-primary",
    row: "bg-primary/10 text-primary",
  },
  secondary: {
    tile: "bg-secondary/15 text-secondary",
    row: "bg-secondary/10 text-secondary",
  },
  info: {
    tile: "bg-info/15 text-info",
    row: "bg-info/10 text-info",
  },
  success: {
    tile: "bg-success/15 text-success",
    row: "bg-success/10 text-success",
  },
  warning: {
    tile: "bg-warning/15 text-warning",
    row: "bg-warning/10 text-warning",
  },
  neutral: {
    tile: "bg-base-content/10 text-base-content/75",
    row: "bg-base-content/8 text-base-content/65",
  },
};

const toneTileClass = computed(() => toneMap[props.tone]?.tile || toneMap.neutral.tile);
const toneRowClass = computed(() => toneMap[props.tone]?.row || toneMap.neutral.row);

// 脉冲点取卡片色调；有运行时另行指定取指定色。三张运行卡刻意分色：委托＝primary、任务＝warning、终端＝success
const toneDotMap: Record<string, { ping: string; core: string }> = {
  primary: { ping: "bg-primary/60", core: "bg-primary" },
  secondary: { ping: "bg-secondary/60", core: "bg-secondary" },
  info: { ping: "bg-info/60", core: "bg-info" },
  success: { ping: "bg-success/60", core: "bg-success" },
  warning: { ping: "bg-warning/60", core: "bg-warning" },
  neutral: { ping: "bg-base-content/60", core: "bg-base-content/70" },
};
const toneDot = computed(() => toneDotMap[props.pulseTone || props.tone] || toneDotMap.neutral);

function handleSelect() {
  if (!props.interactive) return;
  emit("select");
}
</script>

<style scoped>
/* 宽度由所在网格列决定；宽卡跨两格 */
.ecall-home-card-small {
  width: 100%;
}

.ecall-home-card-wide {
  grid-column: span 2;
  width: 100%;
}

.ecall-home-card-icon {
  display: grid;
  place-items: center;
  width: 1.5rem;
  height: 1.5rem;
  flex-shrink: 0;
  border-radius: 0.5rem;
}

/* 入口磁贴：整卡居中，图标与标题竖排，不与内容卡的标题行同构 */
.ecall-home-card-tile {
  align-items: center;
  justify-content: center;
  text-align: center;
  gap: 0.5rem;
}

.ecall-home-card-icon-tile {
  width: 2.25rem;
  height: 2.25rem;
  border-radius: 0.75rem;
}
</style>
