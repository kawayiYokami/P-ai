<script setup lang="ts">
import { defineComponent, h, type PropType, type VNodeChild } from "vue";
import { parseInlineSegments, type InlineSegment } from "./parse-markdown";

/**
 * 行内 simple Markdown 渲染：只保留行内格式（粗体/斜体/删除线/行内代码/kbd/mark/上下标），
 * 不渲染块级结构、不生成链接锚点（预览卡整体是按钮，链接会让点击语义打架）。
 * 用于思维链预览条这类「单行紧凑」场景。
 */
defineProps<{
  text: string;
}>();

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
  <InlineRenderer :segments="parseInlineSegments(String(text || ''))" />
</template>

<style scoped>
.ecall-inline-md-code {
  padding: 0 0.25em;
  border-radius: 0.25rem;
  background-color: color-mix(in oklab, currentColor 12%, transparent);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.95em;
}

.ecall-inline-md-strong {
  font-weight: 600;
}

.ecall-inline-md-em {
  font-style: italic;
}

.ecall-inline-md-delete {
  opacity: 0.7;
}

.ecall-inline-md-kbd {
  padding: 0 0.3em;
  border: 1px solid color-mix(in oklab, currentColor 25%, transparent);
  border-radius: 0.25rem;
  font-size: 0.85em;
}

.ecall-inline-md-mark {
  background-color: color-mix(in oklab, currentColor 20%, transparent);
  color: inherit;
}
</style>
