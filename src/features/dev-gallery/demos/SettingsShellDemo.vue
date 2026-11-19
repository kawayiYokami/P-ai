<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">设置页头部（面包屑 + 左格 + 右格）</h3>
        <p class="text-sm text-base-content/70">所有设置页共用的头部骨架：面包屑、左格（标签页或搜索框）、右格（操作按钮），以及切换标签时内容区的左右进出。直接渲染真实组件。</p>
      </div>
      <SettingsShellGallerySource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 settings-shell 块。
import SettingsShellGallerySource from "../../config/components/SettingsShellGalleryDemo.vue";
</script>
