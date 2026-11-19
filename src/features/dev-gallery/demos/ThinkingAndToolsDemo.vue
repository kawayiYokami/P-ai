<template>
  <div class="card border border-base-300 bg-base-100">
    <div class="card-body gap-3 p-4">
      <div class="space-y-1">
        <h3 class="card-title text-base">思考与工具（折叠块样本 + 流式压测）</h3>
        <p class="text-sm text-base-content/70">
          直接渲染真实 <code>ChatMessageItem</code>，展示块由 <code>useChatMessageBlocks</code> 从模拟消息派生——派生与渲染两段都是产品代码，这里只换数据。
          上面是形态样本：长思维链、三种工具形态、正文代码块与表格，以及跑完 / 正在跑 / 失败中断三种状态；
          下面是流式压测，按时间表往真实消息的正文里逐帧追加字符，默认 300 TPS / 200K，可暂停与重置。
        </p>
      </div>
      <ChatThinkingAndToolsDemo />
    </div>
  </div>
</template>

<script setup lang="ts">
// 容器与文案照搬原 DemoTab 的 thinking-and-tools 块；压测（300 TPS / 200K）由示例组件自身驱动，搬家不改其调度。
import ChatThinkingAndToolsDemo from "../../chat/components/ChatThinkingAndToolsDemo.vue";
</script>
