<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">能力商店条目卡</h3>
        <p class="text-sm text-base-content/70">真实组件 + mock 条目。一行只放图标、名称、来源、两行说明和一个动作。</p>
      </div>
      <CatalogEntryCardSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 catalog-entry-card 块。
import CatalogEntryCardSource from "../../config/components/CatalogEntryCardDemo.vue";
</script>
