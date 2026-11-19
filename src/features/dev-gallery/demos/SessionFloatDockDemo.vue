<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <SessionFloatDockSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 session-float-dock 块，内容来自已有示例组件。
import SessionFloatDockSource from "../../chat/components/SessionFloatDockDemo.vue";
</script>
