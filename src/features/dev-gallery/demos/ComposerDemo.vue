<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">输入面板新结构</h3>
        <p class="text-sm text-base-content/70">矩形容器包裹，底色 base100→base200→base300 循环切换；灌满 ↔ 悬浮 一键切换。</p>
      </div>
      <ChatComposerStructureDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
import ChatComposerStructureDemo from "../../chat/components/ChatComposerStructureDemo.vue";
</script>
