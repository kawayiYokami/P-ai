<template>
  <div class="h-[720px] min-h-0 overflow-hidden rounded-box border border-base-300">
    <ChatViewStreamSource />
  </div>
</template>

<script setup lang="ts">
// 容器类照搬原 DemoTab 的 chat-view-stream 块；高度由固定 720px 限定，不能改成自适应。
import ChatViewStreamSource from "../../chat/views/ChatViewStreamDemo.vue";
</script>
