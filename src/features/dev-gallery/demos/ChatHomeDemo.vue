<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <ChatHomePanelSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 chat-home 块。
import ChatHomePanelSource from "../../chat/components/ChatHomePanelDemo.vue";
</script>
