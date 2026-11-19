<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">消息项（队伍邮件 = 普通气泡）</h3>
        <p class="text-sm text-base-content/70">队伍消息就是一条普通 assistant 消息：头像、人格名、时间、正文按 markdown 渲染。邮件本身拼成一段 markdown，标题在 <code>&lt;summary&gt;</code> 里、正文与状态行在折叠体内，默认收起。与上一条普通消息对照看，气泡形态完全一致，多出来的只是标题那一行可点开。</p>
      </div>
      <ChatMessageItemDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 chat-message 块，内容来自已有示例组件。
import ChatMessageItemDemo from "../../chat/components/ChatMessageItemDemo.vue";
</script>
