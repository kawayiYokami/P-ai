<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">双层卡（DoubleDeck）</h3>
        <p class="text-sm text-base-content/70">底卡包面卡 · 露头在面卡上方 · [[abc]]空底座 / [[abc]def]多队列 · 先调顺再回输入面板。</p>
      </div>
      <DoubleDeckCardDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
import DoubleDeckCardDemo from "../../chat/components/DoubleDeckCardDemo.vue";
</script>
