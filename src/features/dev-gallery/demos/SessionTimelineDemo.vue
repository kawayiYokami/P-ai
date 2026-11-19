<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">会话时间线（垂直）</h3>
        <p class="text-sm text-base-content/70">
          时间线按钮悬停拉成胶囊，点击展开一块几乎盖满聊天区的垂直时间线；用 daisyUI 的 <code>timeline timeline-vertical</code> 逐轮铺开历史消息，内部可上下滚动。
        </p>
      </div>
      <SessionTimelineSource />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 session-timeline 块，内容来自已有示例组件。
import SessionTimelineSource from "../../chat/components/SessionTimelineDemo.vue";
</script>
