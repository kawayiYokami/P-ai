<template>
  <div class="relative flex flex-col overflow-hidden rounded-box border border-base-300 bg-base-100">
    <div v-if="showChrome" class="demo-controls flex flex-wrap items-center gap-3 border-b border-base-300 px-3 py-2">
      <label class="flex items-center gap-1 text-xs">
        <input v-model="isDark" type="checkbox" class="checkbox checkbox-xs" />
        <span>isDark</span>
      </label>
      <label class="flex items-center gap-1 text-xs">
        <span>宽度</span>
        <input v-model.number="demoWidth" type="number" min="320" max="900" class="input input-bordered input-xs w-20" />
      </label>
    </div>

    <div class="p-4">
      <div :style="{ width: `min(${demoWidth}px, 100%)` }">
        <ChatBubbleShell
          name="大黑塔"
          :streaming="true"
          streaming-text="24秒 · 思考中"
          :is-dark="isDark"
          :show-activity="false"
        >
          <div class="text-sm">这个问题问到了我们自己身上。让我把自家记录查清楚再回答。</div>
        </ChatBubbleShell>
      </div>
    </div>

    <div v-if="showChrome" class="demo-note border-t border-base-300 px-3 py-2 text-xs text-base-content/50">
      真实 ChatBubbleShell，streaming=true、streamingText="24秒 · 思考中"。isDark 由上面勾选框控制（生产里由 ChatMessageItem 传 markdownIsDark）。
      URL 加 ?dark=1 可直接以深色态加载。
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue";
import ChatBubbleShell from "./ChatBubbleShell.vue";

const params = new URLSearchParams(window.location.search);
const showChrome = params.get("ui") === "1";
const isDark = ref(params.get("dark") === "1");
const demoWidth = ref(560);
</script>
