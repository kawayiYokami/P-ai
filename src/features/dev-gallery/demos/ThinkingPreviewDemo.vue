<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">思考预览 bar（ChatThinkingPreviewBar）</h3>
        <p class="text-sm text-base-content/70">输入框上方的思维链预览：毛玻璃底、节流切换、显示末尾四行并带上/下位移动画。</p>
      </div>
      <ChatThinkingPreviewBarDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 thinking-preview 块，内容来自已有示例组件。
import ChatThinkingPreviewBarDemo from "../../chat/components/ChatThinkingPreviewBarDemo.vue";
</script>
