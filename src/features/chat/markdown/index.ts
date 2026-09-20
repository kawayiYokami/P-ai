// ==================== App Markdown Module ====================
// Central export for the custom markdown renderer that replaces markstream-vue.

export { default as AppMarkdownRenderer } from "./AppMarkdownRenderer.vue";
export { parseMarkdownBlocks, parseInlineSegments, normalizedTableRow } from "./parse-markdown";
export type { MarkdownBlock, InlineSegment } from "./parse-markdown";
export {
  consumeCrossParagraphToolGroup,
  consumeGroupedMarkerOnlyParagraphs,
  consumeGroupedToolcallRefs,
  isMarkerOnlyParagraph,
} from "./toolcall-ref-group";
export { initKatex } from "./init-katex";
