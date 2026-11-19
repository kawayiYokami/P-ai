<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <ConfigCardsGallerySource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 config-cards 块。
import ConfigCardsGallerySource from "../../config/components/ConfigCardsGalleryDemo.vue";
</script>
