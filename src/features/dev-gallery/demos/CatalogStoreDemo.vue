<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <CatalogStoreSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 catalog-store 块。
import CatalogStoreSource from "../../config/components/CatalogStoreDemo.vue";
</script>
