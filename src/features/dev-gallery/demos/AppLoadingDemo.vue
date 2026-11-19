<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body p-0">
      <AppLoadingSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器类照搬原 DemoTab 的 app-loading 块，内容来自已有示例组件。
import AppLoadingSource from "../../shared/components/AppLoadingDemo.vue";
</script>
