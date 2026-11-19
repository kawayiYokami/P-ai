<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <ChatThinkingAndToolsDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 thinking-and-tools 块；压测（300 TPS / 200K）由示例组件自身驱动，搬家不改其调度。
import ChatThinkingAndToolsDemo from "../../chat/components/ChatThinkingAndToolsDemo.vue";
</script>
