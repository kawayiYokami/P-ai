<template>
  <div class="flex flex-col gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <button type="button" class="btn btn-xs btn-primary" @click="togglePlay">
        {{ playing ? "暂停" : "播放" }}
      </button>
      <button type="button" class="btn btn-xs btn-ghost" @click="reset">重置</button>
      <span class="text-xs text-base-content/60">输出速度：</span>
      <select v-model.number="charsPerTick" class="select select-bordered select-xs w-24">
        <option :value="1">慢</option>
        <option :value="3">中</option>
        <option :value="8">快</option>
        <option :value="20">极快</option>
      </select>
      <label class="flex cursor-pointer items-center gap-1 text-xs">
        <input v-model="scrolledAway" type="checkbox" class="checkbox checkbox-xs" />
        离开底部（不勾选即视为在最下，条隐藏）
      </label>
      <span class="badge badge-sm" :class="streaming ? 'badge-primary' : 'badge-ghost'">
        {{ streaming ? "流式中" : "已结束" }}
      </span>
    </div>

    <div class="relative rounded-box border border-base-300 bg-base-200/60 px-4 pb-4 pt-32">
      <div class="absolute inset-x-0 top-0">
        <ChatThinkingPreviewBar
          :blocks="blocks"
          :idle-text="idleText"
          :streaming="streaming"
          :visible="scrolledAway"
          :avatar-url="demoAvatarUrl"
          @jump-to-bottom="onJumpToBottom"
        />
      </div>
      <div class="rounded-field border border-base-300 bg-base-100 px-3 py-2 text-sm text-base-content/40">
        输入框占位…
      </div>
    </div>

    <div class="text-xs text-base-content/45">
      条的显隐只看「是否在最下」：不在最下才出现，在最下整条隐藏。点击条即回到底部。播放中为流式：上方思维链逐行读、下方正文等下一节点出现才显示；播放结束切到非流式，思维链收起、只留最新一行。
    </div>

    <div class="mockup-code max-h-56 overflow-auto text-xs">
      <pre class="whitespace-pre-wrap break-words"><code>{{ debugText || "（等待播放）" }}</code></pre>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import ChatThinkingPreviewBar from "./ChatThinkingPreviewBar.vue";

type DemoNode = { reasoning: string; text: string };

const SCRIPT: DemoNode[] = [
  {
    reasoning: [
      "先看用户的需求：希望消息增长时不自动贴底，只有用户主动滑到底才开始跟随。",
      "当前代码里没有跟随状态，贴底只有切会话和发送定位两处离散触发。",
      "所以这次要新增一个 `followBottom` 状态，由**用户滚动意图**驱动。",
    ].join("\n"),
    text: "好的，我先改成基于**滚动意图**的贴底跟随，发送与切屏的定位逻辑保持不动。",
  },
  {
    reasoning: [
      "接下来做预览条：上方四行思维链，下方一行正文。",
      "数据要从 `contentBlocks` 取，~~activityItems~~ 在流式期是空文本摘要。",
      "正文不能边流边出，要等这一节点输出完毕再显示。",
    ].join("\n"),
    text: "预览条已改为按节点推进的队列，思维链按阅读速度**逐行**走，正文在节点结束时才显示。",
  },
  {
    reasoning: ["最后跑一遍类型检查和测试，确认没有回归。"].join("\n"),
    text: "类型检查通过，全部测试通过。按 <kbd>Ctrl</kbd>+<kbd>C</kbd> 可以随时中断。",
  },
];

const blocks = ref<Array<{ reasoning?: string; text?: string }>>([]);
const playing = ref(false);
const streaming = ref(false);
const scrolledAway = ref(true);
const charsPerTick = ref(3);
let timer: ReturnType<typeof setTimeout> | null = null;
let nodeIndex = 0;
let phase: "reasoning" | "text" = "reasoning";
let cursor = 0;

const demoAvatarUrl = `data:image/svg+xml;utf8,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#8fd6b4"/><stop offset="1" stop-color="#3f8f74"/></linearGradient></defs><circle cx="32" cy="32" r="32" fill="url(#g)"/><circle cx="24" cy="26" r="4" fill="#fff"/><circle cx="40" cy="26" r="4" fill="#fff"/><path d="M22 40q10 10 20 0" stroke="#fff" stroke-width="3" fill="none" stroke-linecap="round"/></svg>',
)}`;

// 非流式预览：真实里由 ChatView 从最新一条助理消息正文取，demo 里用脚本最后一段正文模拟
const idleText = computed(() => {
  for (let i = blocks.value.length - 1; i >= 0; i -= 1) {
    const text = String(blocks.value[i]?.text || "").trim();
    if (text) return text;
  }
  return "";
});

const debugText = computed(() =>
  blocks.value
    .map((block, index) => {
      const parts: string[] = [];
      if (block.reasoning) parts.push(`思维链：${block.reasoning}`);
      if (block.text) parts.push(`正文：${block.text}`);
      return `节点 ${index + 1}\n${parts.join("\n")}`;
    })
    .join("\n\n"),
);

function stop() {
  if (!timer) return;
  clearTimeout(timer);
  timer = null;
}

function ensureBlock(index: number) {
  while (blocks.value.length <= index) {
    blocks.value.push({});
  }
}

function step() {
  const node = SCRIPT[nodeIndex];
  if (!node) {
    playing.value = false;
    streaming.value = false;
    stop();
    return;
  }
  ensureBlock(nodeIndex);
  const source = phase === "reasoning" ? node.reasoning : node.text;
  cursor = Math.min(source.length, cursor + charsPerTick.value);
  if (phase === "reasoning") {
    blocks.value[nodeIndex].reasoning = source.slice(0, cursor);
  } else {
    blocks.value[nodeIndex].text = source.slice(0, cursor);
  }
  if (cursor >= source.length) {
    if (phase === "reasoning") {
      phase = "text";
      cursor = 0;
    } else {
      nodeIndex += 1;
      phase = "reasoning";
      cursor = 0;
    }
  }
}

function start() {
  if (playing.value) return;
  if (nodeIndex >= SCRIPT.length) reset();
  playing.value = true;
  // 新一轮开始：由组件自身的 streaming 假→真触发队列从头读
  streaming.value = true;
  const loop = () => {
    if (!playing.value) return;
    step();
    timer = setTimeout(loop, 60);
  };
  loop();
}

function togglePlay() {
  if (playing.value) {
    playing.value = false;
    stop();
    return;
  }
  start();
}

function reset() {
  playing.value = false;
  streaming.value = false;
  stop();
  nodeIndex = 0;
  phase = "reasoning";
  cursor = 0;
  blocks.value = [];
}

function onJumpToBottom() {
  scrolledAway.value = false;
}

onBeforeUnmount(stop);
</script>
