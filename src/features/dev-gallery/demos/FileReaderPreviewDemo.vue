<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-4 p-4">
      <div>
        <h4 class="file-reader-preview-demo-caption card-title text-base">右栏 reader 内容视图（HTML 沙箱 / 源码 / 图片）</h4>
        <p class="file-reader-preview-demo-caption text-xs text-base-content/60">
          面板里用的同一批组件：html 走 InteractiveHtmlSandbox 沙箱渲染，切源码复用同一段原文，图片走 image 预览通道（聊天图片就是靠这条通道进右栏）。
        </p>
      </div>
      <FileReaderPreviewSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 file-reader-preview 块（gap-4、h4 + caption 类）。
import FileReaderPreviewSource from "../../file-reader/components/FileReaderPreviewDemo.vue";
</script>
