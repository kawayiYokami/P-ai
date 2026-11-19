<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1" data-demo-chrome>
        <h3 class="card-title text-base">搜索设置（供应商页第六个标签）</h3>
        <p class="text-sm text-base-content/70">真实 SettingsPageShell + SearchEnginesPanel：调度模式一张卡，五个搜索来源各一张卡。</p>
      </div>
      <SearchEnginesDemoSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 search-engines 块；源组件与本文件同名，import 改名区分。
import SearchEnginesDemoSource from "../../config/views/config-tabs/SearchEnginesDemo.vue";
</script>
