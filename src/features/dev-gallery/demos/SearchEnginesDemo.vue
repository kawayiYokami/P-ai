<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <SearchEnginesDemoSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 search-engines 块；源组件与本文件同名，import 改名区分。
import SearchEnginesDemoSource from "../../config/views/config-tabs/SearchEnginesDemo.vue";
</script>
