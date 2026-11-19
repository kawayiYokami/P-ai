<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">快速模型不可用卡片</h3>
        <p class="text-sm text-base-content/70">
          头像生成失败时弹出：一句可读的原因、快速模型下拉框、以及「想禁用就设为不配置」的说明。招募人格在聊天窗口弹、保存人格在配置窗口弹，同一份组件。
        </p>
      </div>
      <QuickModelUnavailableCardSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器照搬旧页 quick-model-card 块。
import QuickModelUnavailableCardSource from "../../config/components/QuickModelUnavailableCardDemo.vue";
</script>
