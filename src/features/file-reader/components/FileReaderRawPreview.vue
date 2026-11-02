<template>
  <pre :class="['file-reader-raw-pre', 'min-h-full', 'p-4', wrap ? 'file-reader-code-wrap' : '']">{{ content }}</pre>
</template>

<script setup lang="ts">
/**
 * 文件阅读器的纯文本 / 源码视图。
 *
 * html 文件切到源码、超大文件退化成纯文本时都落在这里；
 * 单独成组件是为了同一份渲染能在阅读器之外复用，不再另写一套。
 */
defineProps<{
  content: string;
  wrap?: boolean;
}>();
</script>

<style scoped>
/* 原来挂在阅读器面板上的样式，跟着视图一起搬过来：demo 页里没有面板作用域也能对齐 */
.file-reader-raw-pre {
  min-height: 100%;
  margin: 0;
  padding: 0.75rem 1rem;
  white-space: pre;
  font-family: var(--app-code-font-family);
  color: inherit;
  background: transparent;
}
.file-reader-code-wrap {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}
</style>
