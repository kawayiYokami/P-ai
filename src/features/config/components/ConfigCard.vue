<!-- 设置页统一卡片容器：标题栏（`title` 或 `#title`）+ `#actions` + 默认内容区。
     全仓设置页共用的卡片基座，新页面一律用它，不要手搓 `rounded-box border` 当卡片。
     `interactive`：整卡可点（hover 提亮 + 按下反馈），用于列表里的可点条目。
     `overflowVisible`：默认裁边（圆角干净）。卡里有内联下拉/悬浮层（未 teleport 的 `absolute` 面板）时打开它，
     否则面板越过卡片底边会被裁掉。 -->
<template>
  <div
    class="rounded-box border border-base-300 bg-base-100"
    :class="[
      overflowVisible ? 'overflow-visible' : 'overflow-hidden',
      interactive ? 'cursor-pointer select-none transition-all duration-150 hover:border-primary/50 active:scale-[0.99]' : '',
    ]"
  >
    <header
      v-if="title || $slots.title || $slots.actions"
      class="flex items-center justify-between gap-3 border-b-2 border-base-300 px-4 py-3"
    >
      <slot name="title">
        <h3 v-if="title" class="text-sm font-semibold">{{ title }}</h3>
        <div v-else />
      </slot>
      <div v-if="$slots.actions" class="flex items-center gap-2">
        <slot name="actions" />
      </div>
    </header>
    <!-- flush 时不加内边距。这里保留一个空盒：改成 `contents` 会让父级 `space-y-*`
         直接作用到内容子节点上，改变既有调用方（PersonaXxx 等把 space-y 写在 ConfigCard 上）的间距。 -->
    <div :class="flush ? '' : 'px-3'">
      <slot />
    </div>
  </div>
</template>

<script setup lang="ts">
withDefaults(defineProps<{
  title?: string;
  /** 内容区不要内边距（由调用方自己排内边距） */
  flush?: boolean;
  /** 整卡可点：hover 提亮边框 + 按下轻微缩放 */
  interactive?: boolean;
  /** 允许内容溢出卡片边界（卡内有未 teleport 的下拉面板时打开） */
  overflowVisible?: boolean;
}>(), {
  flush: false,
  interactive: false,
  overflowVisible: false,
});
</script>
