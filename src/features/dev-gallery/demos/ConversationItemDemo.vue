<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">会话条目（状态 × 未读）</h3>
        <p class="text-sm text-base-content/70">状态胶囊只讲状态，未读红圆只讲未读，两者并排不合并。含当前会话与 mini 行。</p>
      </div>
      <ChatConversationItemDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 conversation-item 块，内容来自已有示例组件。
import ChatConversationItemDemo from "../../chat/components/ChatConversationItemDemo.vue";
</script>
