<script setup lang="ts">
import { computed, defineComponent, h, type PropType, type VNodeChild } from "vue";
import { parseLightInlineText, type LightInlineSegment } from "./inline-light-markdown";
import { clipInlineSegments } from "./inline-clip";

/**
 * 行内 simple Markdown 渲染：只保留行内格式（粗体/斜体/删除线/行内代码），
 * 并把 `#{1,6} 标题` 这类标题行按加粗渲染（剥掉 # 前缀，不生成块级 <h1>）。
 * 不生成链接锚点（预览卡整体是按钮，链接会让点击语义打架）。
 * 用于思维链预览条、配置卡片预览这类「紧凑」场景。
 *
 * 解析走自己的轻量解析器，不用完整渲染器那套：完整解析器要处理公式、链接、
 * 图片、HTML 标签、脚注，对以纯文本为主的思维链是纯浪费。轻量解析器单趟扫描，
 * 长文本也不需要在字数上设降级闸门。
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

const parsedSegments = computed<LightInlineSegment[]>(() =>
  parseLightInlineText(String(props.text || "")),
);

const inlineSegments = computed<LightInlineSegment[]>(() =>
  clipInlineSegments(parsedSegments.value, props.limit, props.headRatio),
);

function renderSegments(segments: LightInlineSegment[]): VNodeChild[] {
  return segments.map((seg) => {
    if (seg.type === "text") return seg.text;
    if (seg.type === "html_br") return h("br");
    if (seg.type === "paragraph_break") return h("span", { class: "ecall-inline-md-para-gap" });
    if (seg.type === "code") return h("code", { class: "ecall-inline-md-code" }, seg.text);
    if (seg.type === "strong") return h("strong", { class: "ecall-inline-md-strong" }, renderSegments(seg.children));
    if (seg.type === "em") return h("em", { class: "ecall-inline-md-em" }, renderSegments(seg.children));
    if (seg.type === "strongEm") {
      return h("strong", { class: "ecall-inline-md-strong" }, [
        h("em", { class: "ecall-inline-md-em" }, renderSegments(seg.children)),
      ]);
    }
    if (seg.type === "delete") return h("del", { class: "ecall-inline-md-delete" }, renderSegments(seg.children));
    return "";
  });
}

const InlineRenderer = defineComponent({
  name: "InlineMarkdownTextSegments",
  props: {
    segments: { type: Array as PropType<LightInlineSegment[]>, required: true },
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

/* 空行分隔出的段落间距：按字号比例给 0.8em，与正文渲染器 .ecall-md-paragraph
   的 comfortable 档一致（那边也是 0.8em）。
   不用整行空行——行高 1.5em 时一段空行就是 1.5em，比段落间距夸张一倍。 */
:deep(.ecall-inline-md-para-gap) {
  display: block;
  height: 0.8em;
}
</style>
