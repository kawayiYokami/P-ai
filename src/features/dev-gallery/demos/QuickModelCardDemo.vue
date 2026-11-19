<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <QuickModelUnavailableCardSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 quick-model-card 块。
import QuickModelUnavailableCardSource from "../../config/components/QuickModelUnavailableCardDemo.vue";
</script>
