<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-4 p-4">
      <FileReaderPreviewSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 file-reader-preview 块（gap-4、h4 + caption 类）。
import FileReaderPreviewSource from "../../file-reader/components/FileReaderPreviewDemo.vue";
</script>
