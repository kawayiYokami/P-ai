<template>
  <div class="min-h-0">
    <FileReaderPanelSource />
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 file-reader-wrap 块（该块原本不是 card，只有 min-h-0）。
import FileReaderPanelSource from "../../file-reader/components/FileReaderPanelDemo.vue";
</script>
