<template>
  <div class="mb-4 flex flex-wrap items-center gap-x-4 gap-y-2 rounded-field bg-base-200/60 px-3 py-2 text-xs">
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
 */
</script>
