<script setup lang="ts">
import { computed, defineComponent, h, type PropType, type VNodeChild } from "vue";
import { parseInlineSegments, type InlineSegment } from "./parse-markdown";
import { clipInlineSegments } from "./inline-clip";

/**
 * 行内 simple Markdown 渲染：保留行内格式（粗体/斜体/删除线/行内代码/kbd/mark/上下标），
 * 并把 `#{1,6} 标题` 这类标题行按加粗渲染（剥掉 # 前缀，不生成块级 <h1>）。
 * 不生成链接锚点（预览卡整体是按钮，链接会让点击语义打架）。
 * 用于思维链预览条、配置卡片预览这类「紧凑」场景。
 */
const props = withDefaults(defineProps<{
  text: string;
  /** 可见字数上限，0 表示不截断；截断在段边界处发生，不会切断行内代码等标记 */
  limit?: number;
  /** 截断时保留头部所占比例，其余留给尾部 */
  headRatio?: number;
}>(), {
  limit: 0,
  headRatio: 0.3,
});

const HEADING_LINE_PATTERN = /^\s{0,3}#{1,6}\s+(.*)$/;

function parseInlineWithHeadings(text: string): InlineSegment[] {
  const segments: InlineSegment[] = [];
  text.split("\n").forEach((line, index) => {
    if (index > 0) segments.push({ type: "html_br" });
    const heading = line.match(HEADING_LINE_PATTERN);
    if (heading) {
      const children = parseInlineSegments(heading[1]);
      if (children.length > 0) {
        segments.push({ type: "strong", children });
        return;
      }
    }
    segments.push(...parseInlineSegments(line));
  });
  return segments;
}

const parsedSegments = computed<InlineSegment[]>(() =>
  parseInlineWithHeadings(String(props.text || "")),
);

const inlineSegments = computed<InlineSegment[]>(() =>
  clipInlineSegments(parsedSegments.value, props.limit, props.headRatio),
);

function renderSegments(segments: InlineSegment[]): VNodeChild[] {
  return segments.map((seg) => {
    if (seg.type === "text") return seg.text;
    if (seg.type === "html_br") return h("br");
    if (seg.type === "code") return h("code", { class: "ecall-inline-md-code" }, seg.text);
    if (seg.type === "strong") return h("strong", { class: "ecall-inline-md-strong" }, renderSegments(seg.children || []));
    if (seg.type === "em") return h("em", { class: "ecall-inline-md-em" }, renderSegments(seg.children || []));
    if (seg.type === "strongEm") {
      return h("strong", { class: "ecall-inline-md-strong" }, [
        h("em", { class: "ecall-inline-md-em" }, renderSegments(seg.children || [])),
      ]);
    }
    if (seg.type === "delete") return h("del", { class: "ecall-inline-md-delete" }, renderSegments(seg.children || []));
    if (seg.type === "html_kbd") return h("kbd", { class: "ecall-inline-md-kbd" }, renderSegments(seg.children || []));
    if (seg.type === "html_mark") return h("mark", { class: "ecall-inline-md-mark" }, renderSegments(seg.children || []));
    if (seg.type === "html_sub") return h("sub", {}, renderSegments(seg.children || []));
    if (seg.type === "html_sup") return h("sup", {}, renderSegments(seg.children || []));
    if (seg.type === "link") return seg.text;
    if (seg.type === "imageLink") return seg.alt || "";
    if (seg.type === "image") return seg.alt || "";
    if (seg.type === "math") return seg.text;
    if (seg.type === "toolcall_ref" || seg.type === "footnote_ref") return "";
    return "";
  });
}

const InlineRenderer = defineComponent({
  name: "InlineMarkdownTextSegments",
  props: {
    segments: { type: Array as PropType<InlineSegment[]>, required: true },
  },
  setup(props) {
    return () => h("span", { class: "ecall-inline-md" }, renderSegments(props.segments));
  },
});
</script>

<template>
  <InlineRenderer :segments="inlineSegments" />
</template>

<style scoped>
/* 这些元素由脚本里的 h() 动态创建，vnode 上没有本组件的 scopeId，
   普通 scoped 选择器（.x[data-v-xxx]）永远匹配不到，必须从带 scopeId 的根节点往下穿透。 */
:deep(.ecall-inline-md-code) {
  padding: 0 0.25em;
  border-radius: 0.25rem;
  /* 与主渲染器同一口径：红字、无底色 */
  color: var(--ecall-md-inline-code-color);
  background: transparent;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.95em;
}

:deep(.ecall-inline-md-strong) {
  font-weight: 600;
}

:deep(.ecall-inline-md-em) {
  font-style: italic;
}

:deep(.ecall-inline-md-delete) {
  opacity: 0.7;
}

:deep(.ecall-inline-md-kbd) {
  padding: 0 0.3em;
  border: 1px solid color-mix(in oklab, currentColor 25%, transparent);
  border-radius: 0.25rem;
  font-size: 0.85em;
}

:deep(.ecall-inline-md-mark) {
  background-color: color-mix(in oklab, currentColor 20%, transparent);
  color: inherit;
}
</style>
