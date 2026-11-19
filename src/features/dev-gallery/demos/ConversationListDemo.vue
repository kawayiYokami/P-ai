<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">列表卡画廊</h3>
        <p class="text-sm text-base-content/70">左侧会话列表条目（ChatConversationItem）的分组与状态样本，直接渲染真实组件；切换视图模式与预设查看不同层级、不同状态。</p>
      </div>
      <ChatConversationListGalleryDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 conversation-list 块，内容来自已有示例组件。
import ChatConversationListGalleryDemo from "../../chat/components/ChatConversationListGalleryDemo.vue";
</script>
