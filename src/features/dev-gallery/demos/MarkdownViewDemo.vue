<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">Markdown 全量渲染（AppMarkdownRendererDemo）</h3>
        <p class="text-sm text-base-content/70">一份文本覆盖解析器全部块与行内类型，可切三种疏密档位、chat/document 变体、streaming 与深色配色。</p>
      </div>
      <AppMarkdownRendererDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 markdown-view 块，内容来自已有示例组件。
import AppMarkdownRendererDemo from "../../chat/markdown/AppMarkdownRendererDemo.vue";
</script>
