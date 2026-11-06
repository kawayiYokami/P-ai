<template>
  <div class="flex flex-col gap-6 p-4">
    <section>
      <h4 class="text-sm font-semibold">行高对照（实测，数字是这一行的实际高度）</h4>
      <p class="mt-1 text-xs text-base-content/60">
        每种字号三行：只有文字、文字 + daisy 的 spinner、文字 + 四叶草。行高不一致的就是被撑开了。
      </p>
      <div class="mt-3 flex flex-col">
        <div
          v-for="row in measureRows"
          :key="row.key"
          class="flex items-center gap-3 border-b border-base-300/50 py-1"
        >
          <span class="w-24 shrink-0 text-[10px] text-base-content/45">{{ row.label }}</span>
          <div :ref="(el) => setRowRef(row.key, el)" class="flex items-center gap-2" :class="row.textClass">
            <span>搜索中</span>
            <span v-if="row.kind === 'daisy'" class="loading loading-spinner" :class="row.sizeClass"></span>
            <AppLoading v-else-if="row.kind === 'app'" :size="row.appSize" />
          </div>
          <span class="ml-auto shrink-0 font-mono text-[10px] text-base-content/50">
            {{ heights[row.key] ?? '-' }} px
          </span>
        </div>
      </div>
    </section>

    <section>
      <h4 class="text-sm font-semibold">盒子尺寸（实测宽 × 高）</h4>
      <div class="mt-3 flex flex-wrap items-end gap-6">
        <div v-for="pair in boxPairs" :key="pair.daisy" class="flex flex-col items-center gap-2">
          <div :ref="(el) => setBoxRef(pair.daisy, el)" class="flex items-center gap-3">
            <span class="loading loading-spinner" :class="pair.daisy"></span>
            <AppLoading :size="pair.app" />
          </div>
          <span class="font-mono text-[10px] text-base-content/50">
            {{ boxSizes[pair.daisy] ?? '-' }}
          </span>
          <span class="text-[10px] text-base-content/45">{{ pair.daisy }} ｜ {{ pair.app }}</span>
        </div>
      </div>
    </section>

    <section>
      <h4 class="text-sm font-semibold">气泡头状态行（真实 ChatBubbleShell）</h4>
      <p class="mt-1 text-xs text-base-content/60">
        上面那条是流式中（带四叶草），下面那条是普通状态（纯文字）。两条绿色的状态文字应当与作者名在同一条线上。
      </p>
      <div class="mt-3 flex flex-col gap-3 rounded-box border border-base-300 bg-base-100 p-3">
        <ChatBubbleShell name="钟离" streaming streaming-text="03秒 · 准备上下文" />
        <ChatBubbleShell name="钟离" streaming streaming-text="准备上下文" />
        <ChatBubbleShell name="钟离" meta="03秒 · 准备上下文" />
      </div>
    </section>

    <section>
      <h4 class="text-sm font-semibold">并排观感</h4>
      <div class="mt-3 flex flex-col gap-2">
        <div
          v-for="pair in boxPairs"
          :key="'row-' + pair.daisy"
          class="flex items-center gap-3"
          :class="pair.textClass"
        >
          <span class="w-24 shrink-0 text-[10px] text-base-content/45">{{ pair.daisy }}</span>
          <span class="flex items-center gap-2">
            <span class="loading loading-spinner" :class="pair.daisy"></span>
            daisy
          </span>
          <span class="flex items-center gap-2">
            <AppLoading :size="pair.app" />
            四叶草
          </span>
        </div>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { nextTick, onMounted, reactive, ref } from "vue";
import AppLoading from "./AppLoading.vue";
import ChatBubbleShell from "../../chat/components/ChatBubbleShell.vue";

const measureRows = [
  { key: "xs-daisy", label: "text-xs · daisy", textClass: "text-xs", kind: "daisy", sizeClass: "loading-xs" },
  { key: "xs-app", label: "text-xs · 四叶草", textClass: "text-xs", kind: "app", appSize: "xs" },
  { key: "xs-text", label: "text-xs · 纯文字", textClass: "text-xs", kind: "text" },
  { key: "sm-daisy", label: "text-sm · daisy", textClass: "text-sm", kind: "daisy", sizeClass: "loading-sm" },
  { key: "sm-app", label: "text-sm · 四叶草", textClass: "text-sm", kind: "app", appSize: "sm" },
  { key: "sm-text", label: "text-sm · 纯文字", textClass: "text-sm", kind: "text" },
  { key: "base-daisy", label: "text-base · daisy", textClass: "text-base", kind: "daisy", sizeClass: "loading-md" },
  { key: "base-app", label: "text-base · 四叶草", textClass: "text-base", kind: "app", appSize: "md" },
  { key: "base-text", label: "text-base · 纯文字", textClass: "text-base", kind: "text" },
  { key: "lg-daisy", label: "text-lg · daisy", textClass: "text-lg", kind: "daisy", sizeClass: "loading-lg" },
  { key: "lg-app", label: "text-lg · 四叶草", textClass: "text-lg", kind: "app", appSize: "lg" },
  { key: "lg-text", label: "text-lg · 纯文字", textClass: "text-lg", kind: "text" },
] as const;

const boxPairs = [
  { daisy: "loading-xs", app: "xs", textClass: "text-sm" },
  { daisy: "loading-sm", app: "sm", textClass: "text-sm" },
  { daisy: "loading-md", app: "md", textClass: "text-sm" },
  { daisy: "loading-lg", app: "lg", textClass: "text-sm" },
] as const;

const rowRefs = reactive<Record<string, HTMLElement | null>>({});
const boxRefs = reactive<Record<string, HTMLElement | null>>({});
const heights = reactive<Record<string, number>>({});
const boxSizes = reactive<Record<string, string>>({});

function setRowRef(key: string, el: unknown) {
  rowRefs[key] = (el as HTMLElement) ?? null;
}

function setBoxRef(key: string, el: unknown) {
  boxRefs[key] = (el as HTMLElement) ?? null;
}

onMounted(async () => {
  await nextTick();
  for (const [key, el] of Object.entries(rowRefs)) {
    if (el) heights[key] = Math.round(el.getBoundingClientRect().height * 10) / 10;
  }
  for (const [key, el] of Object.entries(boxRefs)) {
    if (!el) continue;
    const parts = [...el.children].map((child) => {
      const rect = child.getBoundingClientRect();
      return `${Math.round(rect.width * 10) / 10}×${Math.round(rect.height * 10) / 10}`;
    });
    boxSizes[key] = parts.join(" / ");
  }
});
</script>
