<template>
  <section class="flex flex-col">
    <header class="flex flex-wrap items-end justify-between gap-x-6 gap-y-3 border-b border-base-content/10 pb-3">
      <div class="min-w-0">
        <h2 class="truncate text-xl font-bold tracking-tight">{{ title }}</h2>
        <p v-if="subtitle" class="mt-1 truncate font-mono text-xs text-base-content/45">{{ subtitle }}</p>
        <p v-if="description" class="mt-1.5 text-sm text-base-content/55">{{ description }}</p>
      </div>
      <!-- 操作区：控制项固定在这一行，不塞进内容里，找得到也不顶开布局 -->
      <div v-if="$slots.actions" class="flex shrink-0 flex-wrap items-center gap-2">
        <slot name="actions" />
      </div>
    </header>
    <div class="pt-4">
      <slot />
    </div>
  </section>
</template>

<script setup lang="ts">
defineProps<{
  /** 中文标题：这一块展示的是什么 */
  title: string;
  /** 英文组件名或标识，小字排在标题下 */
  subtitle?: string;
  /** 一句话说明，可省略 */
  description?: string;
}>();
</script>
