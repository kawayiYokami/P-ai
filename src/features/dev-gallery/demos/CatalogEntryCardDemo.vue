<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <CatalogEntryCardSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 catalog-entry-card 块。
import CatalogEntryCardSource from "../../config/components/CatalogEntryCardDemo.vue";
</script>
