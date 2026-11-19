<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">能力商店</h3>
        <p class="text-sm text-base-content/70">模拟数据。搜索跨源聚合，一条一行，只留「这是什么」和「一个动作」。</p>
      </div>
      <CatalogStoreSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 catalog-store 块。
import CatalogStoreSource from "../../config/components/CatalogStoreDemo.vue";
</script>
