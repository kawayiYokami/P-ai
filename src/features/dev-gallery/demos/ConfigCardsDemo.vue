<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">配置卡片画廊（五大卡片第一性原理）</h3>
        <p class="text-sm text-base-content/70">集中渲染联系人渠道、连接器、供应商、人格、技能 5 类卡片的真实模拟数据与自适应网格，支持切换宽度与主题进行视觉检验。</p>
      </div>
      <ConfigCardsGallerySource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 config-cards 块。
import ConfigCardsGallerySource from "../../config/components/ConfigCardsGalleryDemo.vue";
</script>
