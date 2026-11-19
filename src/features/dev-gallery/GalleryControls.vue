<template>
  <div class="gallery-controls flex flex-wrap items-center gap-x-4 gap-y-2 rounded-field bg-base-200/60 px-3 py-2 text-xs">
    <slot />
    <template v-if="$slots.hint">
      <span class="w-full text-base-content/50">
        <slot name="hint" />
      </span>
    </template>
  </div>
</template>

<script setup lang="ts">
/**
 * 组件展示区统一的控制条：所有 demo 的开关、下拉、按钮都放这一行，
 * 位置固定在内容区顶部，样式只有这一种，不要各写各的。
 *
 * 控件多的时候允许换行（flex-wrap），但不允许把控件自身压变形——单个标签被压窄后
 * 中文会竖排成两行，看起来像控制条坏了。
 */
</script>

<style scoped>
/* daisyUI 的 .select 默认 width: clamp(3rem, 20rem, 100%)，一个就占 20rem，
   两个并排直接把整行吃满，同行的文字标签被挤到竖排。控制条里的下拉按内容宽度收。 */
.gallery-controls :slotted(.select) {
  width: auto;
  max-width: 12rem;
}

/* 标签与控件按钮不参与收缩：宁可换行，也不要把「宽度」这种两字标签压成一列。 */
.gallery-controls :slotted(label),
.gallery-controls :slotted(.btn),
.gallery-controls :slotted(.select) {
  flex: none;
}
</style>
