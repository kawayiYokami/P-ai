<script setup lang="ts">
import { computed, defineComponent, h, type PropType, type VNodeChild } from "vue";
import { parseInlineSegments, type InlineSegment } from "../markdown/parse-markdown";

/**
 * previewMarkdown 轻量白名单渲染
 * - 仅标题/引用/列表占位转行，code/表格/图表等块级重型不渲染
 * - 内联仅保留 strong/em/code/kbd/mark/链接文本，跟随正文字号不放大
 * - 段落 margin 0.12em 无大间距
 */
const props = defineProps<{
  text: string;
  clamp?: number;
}>();

const TOOL_BREAK = "\uE000TOOLBREAK\uE000";

function stripHeavyBlocks(raw: string): string {
  const normalized = String(raw || "")
    .split(TOOL_BREAK).join("\n\n")
    .replace(/\s*\[toolcall:[^\]\n]+\]/g, "")
    .replace(/\r\n?/g, "\n");
  const lines = normalized.split("\n");
  const out: string[] = [];
  let inCodeFence = false;
  for (const line of lines) {
    const trimmed = line.trim();
    // code fence
    if (/^\s*```/.test(line)) {
      inCodeFence = !inCodeFence;
      continue;
    }
    if (inCodeFence) continue;
    // 表格行（含 | 且有分隔线感）
    if (/^\s*\|.*\|\s*$/.test(trimmed) && trimmed.includes("|")) {
      continue;
    }
    if (/^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)+\|?\s*$/.test(trimmed)) continue;
    // mermaid / 图表关键字块首行跳过
    if (/^\s*(```)?\s*mermaid\b/i.test(trimmed)) continue;
    // hr
    if (/^\s*([-*_])\s*(\1\s*){2,}$/.test(trimmed)) continue;
    // details/html 块标签
    if (/^\s*<\/?(details|summary)\b/i.test(trimmed)) continue;
    out.push(line);
  }
  return out.join("\n").replace(/\n{3,}/g, "\n\n").trim();
}

function normalizePreviewText(raw: string, maxLen: number): string {
  const withoutBlocks = stripHeavyBlocks(raw);
  // 标题/列表/引用转空白，保留语义但不占结构
  const flattened = withoutBlocks
    .split("\n")
    .map((line) => {
      const t = line.trim();
      // heading: ### 标题 -> 标题
      const hm = t.match(/^#{1,4}\s+(.*)$/);
      if (hm) return hm[1].trim();
      // quote: > 引用 -> 引用
      const qm = t.match(/^>\s?(.*)$/);
      if (qm) return qm[1].trim();
      // list: - / 1. -> 去标记
      const lm = t.match(/^(?:[-*+]\s+|\d+[.)]\s+)(.*)$/);
      if (lm) return lm[1].trim();
      return t;
    })
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
  if (!flattened) return "";
  return flattened.length > maxLen ? `${flattened.slice(0, maxLen).trimEnd()}…` : flattened;
}

const previewMarkdown = computed(() => {
  const maxLen = props.clamp ?? 120;
  return normalizePreviewText(props.text || "", maxLen);
});

const segments = computed<InlineSegment[]>(() => {
  if (!previewMarkdown.value) return [];
  return parseInlineSegments(previewMarkdown.value);
});

const Inline = defineComponent({
  name: "TimelinePreviewInline",
  props: {
    segments: { type: Array as PropType<InlineSegment[]>, required: true },
  },
  setup(p) {
    function renderSeg(seg: InlineSegment): VNodeChild {
      if (seg.type === "text") return seg.text;
      if (seg.type === "code") return h("code", { class: "ecall-timeline-inline-code" }, seg.text);
      if (seg.type === "html_br") return h("br");
      if (seg.type === "strong") return h("strong", { class: "font-semibold" }, (seg.children || []).map(renderSeg));
      if (seg.type === "em") return h("em", { class: "italic" }, (seg.children || []).map(renderSeg));
      if (seg.type === "strongEm") return h("strong", { class: "font-semibold" }, [h("em", { class: "italic" }, (seg.children || []).map(renderSeg))]);
      if (seg.type === "delete") return h("s", { class: "opacity-70" }, (seg.children || []).map(renderSeg));
      if (seg.type === "html_kbd") return h("kbd", { class: "ecall-timeline-kbd" }, (seg.children || []).map(renderSeg));
      if (seg.type === "html_mark") return h("mark", { class: "ecall-timeline-mark" }, (seg.children || []).map(renderSeg));
      if (seg.type === "html_sub") return h("sub", {}, (seg.children || []).map(renderSeg));
      if (seg.type === "html_sup") return h("sup", {}, (seg.children || []).map(renderSeg));
      if (seg.type === "link" || seg.type === "imageLink") return (seg as any).text || (seg as any).href || "";
      if (seg.type === "image") return (seg as any).alt || "";
      if (seg.type === "math") return seg.text;
      if (seg.type === "toolcall_ref" || seg.type === "footnote_ref") return "";
      return "";
    }
    return () => h("span", { class: "ecall-timeline-preview-inline" }, p.segments.map(renderSeg));
  },
});
</script>

<template>
  <span class="ecall-timeline-preview-markdown inline">
    <Inline v-if="segments.length > 0" :segments="segments" />
    <span v-else class="opacity-60">{{ previewMarkdown || "（空消息）" }}</span>
  </span>
</template>

<style scoped>
.ecall-timeline-preview-markdown {
  font-size: var(--app-chat-message-text-size, var(--app-text-sm-size, 0.875rem));
  line-height: 1.5;
}
.ecall-timeline-preview-markdown :deep(.ecall-timeline-inline-code) {
  color: var(--ecall-md-inline-code-color);
  background: transparent;
  border-radius: 0.35rem;
  padding: 0.06rem 0.28rem;
  line-height: 1.4;
  font-family: var(--app-code-font-family);
}
.ecall-timeline-preview-markdown :deep(.ecall-timeline-kbd) {
  border: 1px solid color-mix(in srgb, var(--color-base-content) 18%, transparent);
  border-bottom-width: 2px;
  border-radius: 0.32rem;
  padding: 0 0.28rem;
  background: color-mix(in srgb, var(--color-base-100) 78%, var(--color-base-300));
}
.ecall-timeline-preview-markdown :deep(.ecall-timeline-mark) {
  background: color-mix(in srgb, var(--color-warning) 22%, transparent);
  border-radius: 0.2rem;
  padding: 0 0.18rem;
}
</style>
