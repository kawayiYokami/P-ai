<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">代码块 / mermaid 内嵌气泡（ChatRichBlockEmbedDemo）</h3>
        <p class="text-sm text-base-content/70">左边是改动前的分段气泡（代码块、mermaid 各自独立成块），右边是改动后富块内嵌进气泡、代码块去掉底色与框线的样子。两边用的是同一段 markdown 与真实的 markdown 渲染器。</p>
      </div>
      <ChatRichBlockEmbedDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 rich-block-embed 块，内容来自已有示例组件。
import ChatRichBlockEmbedDemo from "../../chat/components/ChatRichBlockEmbedDemo.vue";
</script>
