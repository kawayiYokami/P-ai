<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <SettingsShellGallerySource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 settings-shell 块。
import SettingsShellGallerySource from "../../config/components/SettingsShellGalleryDemo.vue";
</script>
