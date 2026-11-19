<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">预览卡画廊</h3>
        <p class="text-sm text-base-content/70">右侧主页的每张卡单独展示一次，面板宽度按聊天窗口右侧还原；模拟数据只用于看样式。</p>
      </div>
      <HomeCardGalleryDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 home-cards 块，内容来自已有示例组件。
import HomeCardGalleryDemo from "../../chat/components/HomeCardGalleryDemo.vue";
</script>
