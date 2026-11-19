<template>
  <div class="min-h-0">
    <ChatImagePreviewDialogSource />
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 image-preview-dialog 块（该块原本不是 card，只有 min-h-0）。
import ChatImagePreviewDialogSource from "../../chat/components/dialogs/ChatImagePreviewDialogDemo.vue";
</script>
