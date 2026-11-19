<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <HomeCardGalleryDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 home-cards 块，内容来自已有示例组件。
import HomeCardGalleryDemo from "../../chat/components/HomeCardGalleryDemo.vue";
</script>
