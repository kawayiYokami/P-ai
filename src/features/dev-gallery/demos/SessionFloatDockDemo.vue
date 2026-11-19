<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">会话悬浮操作区（下排独立按钮）</h3>
        <p class="text-sm text-base-content/70">下排改成一个个彼此独立的磨砂元素，左对齐、放不下换行；元素之间不互斥，有就显示、没有就隐藏。开关可逐项试「有／没有」，勾「窄容器」看换行。</p>
      </div>
      <SessionFloatDockSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 session-float-dock 块，内容来自已有示例组件。
import SessionFloatDockSource from "../../chat/components/SessionFloatDockDemo.vue";
</script>
