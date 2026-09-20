<template>
  <div class="flex flex-col gap-3">
    <div class="text-xs text-base-content/60">
      同一条助理消息：左边是现在的分段气泡，右边是代码块 / mermaid 内嵌进气泡后的样子。两边内容一致，只差富块是否自成一块。
    </div>

    <div class="grid gap-4 md:grid-cols-2">
      <section
        v-for="column in columns"
        :key="column.key"
        class="flex min-w-0 flex-col gap-2"
      >
        <div class="text-xs font-medium text-base-content/70">{{ column.caption }}</div>

        <div
          class="min-w-0 overflow-hidden rounded-box border border-base-300 bg-base-200/70 p-3"
          :class="column.key === 'target' ? 'ecall-rich-embed-demo-target' : ''"
        >
          <div
            class="assistant-markdown ecall-assistant-bubble max-w-full"
            data-bubble-background="on"
            data-segmented-markdown="on"
          >
            <div class="ecall-assistant-segment-list">
              <div
                v-for="segment in column.segments"
                :key="segment.key"
                :class="[
                  'ecall-assistant-segment',
                  segment.kind === 'text' ? 'ecall-assistant-segment-text' : 'ecall-assistant-segment-rich',
                  segment.withRich ? 'ecall-assistant-segment-with-rich' : '',
                ]"
              >
                <AppMarkdownRenderer
                  class="ecall-markdown-content max-w-none"
                  :blocks="segment.blocks"
                  :is-dark="isDark"
                />
              </div>
            </div>
          </div>
        </div>

        <p class="text-xs text-base-content/55">{{ column.note }}</p>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { isDarkAppTheme } from "../../shell/composables/use-app-theme";
import {
  AppMarkdownRenderer,
  parseMarkdownBlocks,
  type MarkdownBlock,
} from "../markdown";

// 复刻改动前的分段：富块各自成段，正文成段——仅用于这一页的「现状」列对照
function legacySegments(blocks: MarkdownBlock[]): Array<{ key: string; kind: "text" | "rich"; blocks: MarkdownBlock[] }> {
  const out: Array<{ key: string; kind: "text" | "rich"; blocks: MarkdownBlock[] }> = [];
  let buffer: MarkdownBlock[] = [];
  const flush = () => {
    if (buffer.length <= 0) return;
    out.push({ key: `legacy-text-${out.length}`, kind: "text", blocks: buffer });
    buffer = [];
  };
  for (const block of blocks) {
    if (!block) continue;
    const rich = block.type === "code" || block.type === "table";
    if (rich) {
      flush();
      out.push({ key: `legacy-rich-${out.length}`, kind: "rich", blocks: [block] });
    } else {
      buffer.push(block);
    }
  }
  flush();
  return out;
}

// 与聊天区同源：深色与否只看 documentElement 上的主题名
const isDark = ref(false);
let themeObserver: MutationObserver | null = null;

function syncTheme(): void {
  isDark.value = isDarkAppTheme(document.documentElement.getAttribute("data-theme") || "");
}

onMounted(() => {
  syncTheme();
  themeObserver = new MutationObserver(syncTheme);
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
});

onBeforeUnmount(() => {
  themeObserver?.disconnect();
  themeObserver = null;
});

const MARKDOWN = [
  "### 兜底检索的顺序",
  "",
  "先按关键词捞一遍，再走向量召回，最后用 **RRF** 把两路结果融合：",
  "",
  "```ts",
  "export function fuse(keyword: Hit[], vector: Hit[]) {",
  "  return rrf([keyword, vector], { k: 60 });",
  "}",
  "```",
  "",
  "融合前后的差距：",
  "",
  "| 方案 | 召回率 | 延迟 |",
  "| --- | --- | --- |",
  "| 纯关键词 | 0.62 | 8ms |",
  "| RRF 融合 | 0.83 | 31ms |",
  "",
  "调用链长这样：",
  "",
  "```mermaid",
  "flowchart LR",
  "  Q[query] --> K[关键词]",
  "  Q --> V[向量]",
  "  K --> R[RRF]",
  "  V --> R",
  "```",
  "",
  "> 融合前先把两路归一到同一打分空间，否则 RRF 会被量纲带偏。",
].join("\n");

const blocks = computed(() => parseMarkdownBlocks(MARKDOWN));

type DemoColumn = {
  key: string;
  caption: string;
  note: string;
  segments: Array<{ key: string; kind: "text" | "rich"; blocks: MarkdownBlock[]; withRich?: boolean }>;
};

const columns = computed<DemoColumn[]>(() => [
  {
    key: "current",
    caption: "改动前：正文被富块切成多个气泡，代码块 / mermaid 各自独立成块",
    note: "代码块自带底色与圆角，与正文气泡同色，看着像又一个气泡；mermaid 与表格各自占满一行，把气泡断开。这一列是按改动前的分段逻辑复刻的。",
    segments: legacySegments(blocks.value),
  },
  {
    key: "target",
    caption: "改动后：一段正文一个气泡，富块内嵌其中",
    note: "富块不再切开气泡，代码块去掉底色与框线、表格去掉外框；含富块的那段恢复整行宽度，否则代码会被压窄成横向滚动。",
    segments: [{ key: "target-all", kind: "text", blocks: blocks.value, withRich: true }],
  },
]);
</script>

<style scoped>
/* ==================== 复刻 ChatMessageItem 的气泡与分段样式 ====================
   ChatMessageItem 的样式是 scoped 的，跨组件用不了，这里逐条抄一份。
   选择器形状与 ChatMessageItem 保持一致（不额外加外层类），否则层叠顺序会和真实聊天不一样。 */
.assistant-markdown {
  --ecall-chat-rich-block-bg: var(--color-base-100);
  font-size: var(--app-chat-message-text-size, var(--app-text-sm-size));
}

.assistant-markdown :deep(.ecall-md-code-block) {
  --ecall-md-code-bg: var(--ecall-chat-rich-block-bg);
}

.assistant-markdown :deep(.ecall-markdown-content :where(blockquote, .blockquote)) {
  background: var(--ecall-chat-rich-block-bg);
}

.assistant-markdown :deep(.ecall-markdown-content :where(th, .table-node th)) {
  background: var(--ecall-chat-rich-block-bg) !important;
}

.assistant-markdown :deep(.ecall-markdown-content :where(td, .table-node td)) {
  background: var(--ecall-chat-rich-block-bg) !important;
}

.assistant-markdown :deep(.ecall-markdown-content .ecall-md-details) {
  background: var(--ecall-chat-rich-block-bg);
}

.ecall-assistant-segment-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.ecall-assistant-segment {
  min-width: 0;
}

.ecall-assistant-segment-text {
  display: inline-block;
  width: fit-content;
  max-width: 100%;
  padding: 0.68rem 1rem;
}

.ecall-assistant-segment-rich {
  display: block;
  width: 100%;
  padding: 0;
}

/* 含富块的正文段：回到整行宽度，否则代码块会被 fit-content 压窄成横向滚动 */
.ecall-assistant-segment-with-rich {
  display: block;
  width: 100%;
}

.ecall-assistant-bubble[data-bubble-background="on"] .ecall-assistant-segment-text {
  border-radius: var(--radius-box, 1rem);
  background: var(--color-base-100);
}

/* ==================== 目标：富块不再自成一块，底色与框线归零 ==================== */
.ecall-rich-embed-demo-target .assistant-markdown {
  --ecall-chat-rich-block-bg: transparent;
}

.ecall-rich-embed-demo-target .assistant-markdown :deep(.ecall-md-code-block) {
  --ecall-md-code-bg: transparent;
  background: transparent;
}

/* 表格外框在 markdown-content.css 里是 !important 声明的，覆盖要跟上 !important */
.ecall-rich-embed-demo-target .assistant-markdown :deep(.ecall-md-table-wrap) {
  border-radius: 0;
}

.ecall-rich-embed-demo-target .assistant-markdown :deep(.ecall-md-table) {
  border: 0 !important;
  border-radius: 0;
}
</style>
