<template>
  <div
    ref="rendererRootRef"
    v-bind="attrs"
    class="ecall-md-renderer"
    :class="[isDark ? 'ecall-md-dark' : 'ecall-md-light', variant === 'document' ? 'ecall-md-document' : 'ecall-md-chat', streaming ? 'ecall-md-streaming' : '']"
    @click="emit('click', $event)"
  >
    <BlockRenderer
      :blocks="visibleBlocks"
      :is-dark="isDark"
      :streaming="streaming"
      :local-image-base-path="localImageBasePath"
      :footnote-index-map="footnoteIndexMap"
      :on-image-preview="(payload) => emit('openImagePreview', payload)"
    />
  </div>
  <Teleport to="body">
    <div
      v-if="activeToolcallPreviews.length > 0"
      ref="toolcallPopupRef"
      class="ecall-md-toolcall-popup fixed z-1200 w-max min-w-[18rem] rounded-box border border-base-300 bg-base-100 text-base-content shadow-xl"
      :style="toolcallPopupStyle"
      data-toolcall-popup="true"
    >
      <div class="border-b border-base-300/70 px-2 py-1.5 text-xs font-semibold text-base-content/80">
        {{ activeToolcallPopupTitle }}
      </div>
      <div
        class="relative overflow-hidden"
        @mouseenter="toolcallScrollbarRef?.reveal()"
        @mouseleave="toolcallScrollbarRef?.hide()"
      >
        <div
          ref="toolcallScrollerRef"
          class="ecall-md-toolcall-scroll max-h-80 overflow-y-auto"
        >
          <div v-if="activeToolcallPreviews.length > 0" class="py-1">
            <div
              v-for="(preview, index) in activeToolcallPreviews"
              :key="preview.id"
              class="px-2 py-1"
              :class="index > 0 ? 'border-t border-base-300/60' : ''"
            >
              <div class="grid grid-cols-[1.1rem_minmax(0,1fr)] items-start gap-x-1.5 text-xs leading-relaxed text-base-content/75">
                <span class="mt-0.5 inline-flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-base-200 text-caption font-medium leading-none text-base-content/65">
                  {{ index + 1 }}
                </span>
                <div class="min-w-0 whitespace-normal break-all font-normal leading-relaxed">
                  <template v-if="splitToolcallPreviewTitle(preview).name">
                    <span>{{ splitToolcallPreviewTitle(preview).name }}</span>
                    <span v-if="splitToolcallPreviewTitle(preview).pathText || splitToolcallPreviewTitle(preview).rest || preview.filePath"> · </span>
                  </template>
                  <a
                    v-if="preview.filePath"
                    href="#"
                    class="ecall-md-link"
                    :data-href="preview.filePath"
                    :title="preview.filePath"
                    @click="handleToolcallFileLinkClick($event, preview.filePath)"
                  >{{ splitToolcallPreviewTitle(preview).pathText || preview.fileLabel || preview.filePath }}</a>
                  <template v-if="preview.filePath && splitToolcallPreviewTitle(preview).rest">
                    <span> · {{ splitToolcallPreviewTitle(preview).rest }}</span>
                  </template>
                  <span v-else-if="!preview.filePath && splitToolcallPreviewTitle(preview).rest">{{ splitToolcallPreviewTitle(preview).rest }}</span>
                  <span v-else-if="!preview.filePath && !splitToolcallPreviewTitle(preview).name">{{ preview.title || preview.label }}</span>
                </div>
              </div>
              <pre
                v-if="preview.body"
                class="ml-[1.6rem] mt-1 m-0 whitespace-pre-wrap break-all rounded bg-base-200/50 p-1 text-xs leading-4 text-base-content/75"
              ><code>{{ preview.body }}</code></pre>
            </div>
          </div>
        </div>
        <FloatingScrollbar ref="toolcallScrollbarRef" :target="toolcallScrollerRef" />
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { Teleport, computed, defineComponent, h, nextTick, onBeforeUnmount, onMounted, ref, useAttrs, watch, type PropType, type VNodeChild } from "vue";
import { useI18n } from "vue-i18n";
import { Wrench } from "@lucide/vue";
import FloatingScrollbar from "../../shell/components/FloatingScrollbar.vue";
import { normalizeLocalLinkHref, parseLocalFileReference } from "../utils/local-link";
import { parseMarkdownBlocks, parseInlineSegments, normalizedTableRow, type MarkdownBlock, type InlineSegment } from "./parse-markdown";
import { IncrementalMarkdownBlockParser } from "./incremental-markdown";
import {
  consumeCrossParagraphToolGroup,
  consumeGroupedToolcallRefs,
} from "./toolcall-ref-group";
import CodeBlock from "./CodeBlock";
import LazyMarkdownImage from "./LazyMarkdownImage";
import type { MarkdownImagePreviewPayload } from "./MarkdownImage";
import { stableMarkdownRuntimeKey } from "./markdown-runtime-key";

// ========== 内联与嵌套解析缓存：避免流式每帧全量重解析 ==========
const INLINE_CACHE_MAX = 800;
const inlineSegmentCache = new Map<string, InlineSegment[]>();
function cachedParseInlineSegments(text: string): InlineSegment[] {
  const cached = inlineSegmentCache.get(text);
  if (cached) return cached;
  const segs = parseInlineSegments(text);
  if (inlineSegmentCache.size >= INLINE_CACHE_MAX) {
    const first = inlineSegmentCache.keys().next().value as string | undefined;
    if (first !== undefined) inlineSegmentCache.delete(first);
  }
  inlineSegmentCache.set(text, segs);
  return segs;
}

const NESTED_SIMPLE_CACHE_MAX = 200;
const nestedSimpleCache = new Map<string, MarkdownBlock[]>();
const nestedIncrementalParsers = new Map<string, IncrementalMarkdownBlockParser>();
function evictNestedParsersIfNeeded() {
  if (nestedIncrementalParsers.size <= 120) return;
  let count = 0;
  for (const key of nestedIncrementalParsers.keys()) {
    nestedIncrementalParsers.delete(key);
    count += 1;
    if (count >= 20) break;
  }
}

defineOptions({
  inheritAttrs: false,
});

const props = defineProps<{
  text?: string;
  blocks?: MarkdownBlock[];
  isDark?: boolean;
  streaming?: boolean;
  variant?: "chat" | "document";
  localImageBasePath?: string;
  toolcallPreviewMap?: Record<string, { title?: string; body?: string; filePath?: string; fileLabel?: string }>;
}>();
const emit = defineEmits<{
  (e: "click", event: MouseEvent): void;
  (e: "mathContextMenu", payload: { clientX: number; clientY: number; copyText: string }): void;
  (e: "openImagePreview", payload: MarkdownImagePreviewPayload): void;
}>();
const attrs = useAttrs();

const { t } = useI18n();
const rendererRootRef = ref<HTMLElement | null>(null);
const activeToolcallIds = ref<string[]>([]);
const activeToolcallAnchorEl = ref<HTMLButtonElement | null>(null);
const toolcallPopupRef = ref<HTMLElement | null>(null);
const toolcallScrollerRef = ref<HTMLElement | null>(null);
const toolcallScrollbarRef = ref<InstanceType<typeof FloatingScrollbar> | null>(null);
const rendererInstanceId = `md-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
const activeFootnoteId = ref("");
let activeFootnoteTimer = 0;
const toolcallPopupStyle = ref<Record<string, string>>({
  left: "0px",
  top: "0px",
});

const activeToolcallPreviews = computed(() => {
  return activeToolcallIds.value
    .map((rawId) => String(rawId || "").trim())
    .filter(Boolean)
    .map((id) => {
      const preview = props.toolcallPreviewMap?.[id];
      if (!preview) return null;
      return {
        id,
        label: `toolcall:${id}`,
        title: String(preview.title || "").trim(),
        body: String(preview.body || "").trim(),
        filePath: String(preview.filePath || "").trim(),
        fileLabel: String(preview.fileLabel || preview.filePath || "").trim(),
      };
    })
    .filter((preview): preview is {
      id: string;
      label: string;
      title: string;
      body: string;
      filePath: string;
      fileLabel: string;
    } => !!preview);
});

function resolveMathCopyText(text: string, raw: string, display: boolean): string {
  const normalizedRaw = String(raw || "").trim();
  if (normalizedRaw) return normalizedRaw;
  return display ? `$$\n${text}\n$$` : `$${text}$`;
}

const activeToolcallPopupTitle = computed(() => {
  const count = activeToolcallPreviews.value.length;
  if (count <= 0) return t("chat.shareExport.toolLabel");
  if (count === 1) return t("chat.shareExport.toolLabel");
  return `${t("chat.shareExport.toolLabel")} +${count}`;
});

function closeToolcallPreview() {
  activeToolcallIds.value = [];
  activeToolcallAnchorEl.value = null;
}

function splitToolcallPreviewTitle(preview: {
  title?: string;
  filePath?: string;
  fileLabel?: string;
}): { name: string; rest: string; pathText: string } {
  const title = String(preview.title || "").trim();
  const filePath = String(preview.filePath || "").trim();
  const fileLabel = String(preview.fileLabel || filePath).trim();
  if (!title) {
    return { name: "", rest: "", pathText: fileLabel };
  }
  if (!filePath) {
    const parts = title.split(" · ");
    if (parts.length <= 1) return { name: title, rest: "", pathText: "" };
    return {
      name: parts[0] || "",
      rest: parts.slice(1).join(" · "),
      pathText: "",
    };
  }

  // title 形如 "read · E:/a.ts · offset: 150"
  const separator = " · ";
  const parts = title.split(separator);
  if (parts.length === 0) return { name: title, rest: "", pathText: fileLabel };

  const name = parts[0] || "";
  const remaining = parts.slice(1);
  // 优先匹配完整 path / label
  let pathIndex = remaining.findIndex((part) => {
    const text = part.trim();
    return text === filePath || text === fileLabel || normalizeLocalLinkHref(text) === normalizeLocalLinkHref(filePath);
  });
  if (pathIndex < 0) {
    // 次选：包含路径片段
    pathIndex = remaining.findIndex((part) => part.includes(filePath) || (fileLabel && part.includes(fileLabel)));
  }
  if (pathIndex < 0) {
    return {
      name,
      rest: remaining.join(separator),
      pathText: fileLabel || filePath,
    };
  }

  const pathText = remaining[pathIndex]?.trim() || fileLabel || filePath;
  const restParts = remaining.filter((_, index) => index !== pathIndex);
  return {
    name,
    rest: restParts.join(separator),
    pathText,
  };
}

function handleToolcallFileLinkClick(event: MouseEvent, filePath: string) {
  event.preventDefault();
  event.stopPropagation();
  const path = String(filePath || "").trim();
  if (!path) return;
  // 复用正文链接点击链路：ChatMessageItem -> ChatView.assistantLinkClick
  emit("click", event);
  closeToolcallPreview();
}

function resolveToolcallPopupContainer(anchor: HTMLElement | null): HTMLElement | null {
  if (!(anchor instanceof HTMLElement)) return null;
  return anchor.closest("[data-chat-center-pane]")
    ?? anchor.closest(".ecall-chat-message-row")
    ?? null;
}

function resolveToolcallPopupMaxWidth(anchor: HTMLElement | null): number {
  // 以中间对话区域为基准：弹窗宽度不超过其 92%
  const container = resolveToolcallPopupContainer(anchor);
  const baseWidth = container instanceof HTMLElement
    ? container.getBoundingClientRect().width
    : (window.innerWidth || 0);
  return Math.max(288, Math.round(baseWidth * 0.92));
}

async function positionToolcallPopup() {
  const anchor = activeToolcallAnchorEl.value;
  const popup = toolcallPopupRef.value;
  if (!(anchor instanceof HTMLElement) || !(popup instanceof HTMLElement)) return;
  const margin = 8;
  const anchorRect = anchor.getBoundingClientRect();
  const popupRect = popup.getBoundingClientRect();
  const viewportWidth = window.innerWidth || document.documentElement.clientWidth || 0;
  const viewportHeight = window.innerHeight || document.documentElement.clientHeight || 0;
  // 弹窗不允许超出中间对话区域（找不到时退回视口边界）
  const container = resolveToolcallPopupContainer(anchor);
  let leftMin = margin;
  let leftMax = Math.max(margin, viewportWidth - popupRect.width - margin);
  if (container instanceof HTMLElement) {
    const paneRect = container.getBoundingClientRect();
    leftMin = Math.max(margin, paneRect.left + margin);
    leftMax = Math.min(leftMax, paneRect.right - popupRect.width - margin);
  }
  const left = Math.min(
    Math.max(leftMin, anchorRect.left),
    Math.max(leftMin, leftMax),
  );
  const belowTop = anchorRect.bottom + 8;
  const aboveTop = anchorRect.top - popupRect.height - 8;
  const top = belowTop + popupRect.height + margin <= viewportHeight
    ? belowTop
    : Math.max(margin, aboveTop);
  toolcallPopupStyle.value = {
    ...toolcallPopupStyle.value,
    left: `${Math.round(left)}px`,
    top: `${Math.round(top)}px`,
  };
}

async function openToolcallPreview(ids: string[], anchorEl: HTMLButtonElement | null) {
  const normalizedIds = ids
    .map((id) => String(id || "").trim())
    .filter(Boolean);
  const availableIds = normalizedIds.filter((id) => {
    const preview = props.toolcallPreviewMap?.[id];
    return !!preview && (!!String(preview.title || "").trim() || !!String(preview.body || "").trim());
  });
  if (availableIds.length === 0) return;
  activeToolcallIds.value = availableIds;
  activeToolcallAnchorEl.value = anchorEl;
  toolcallPopupStyle.value.maxWidth = `${resolveToolcallPopupMaxWidth(anchorEl)}px`;
  await nextTick();
  toolcallScrollbarRef.value?.updateThumb();
  await positionToolcallPopup();
}

function sameToolcallGroup(left: string[], right: string[]): boolean {
  if (left.length !== right.length) return false;
  return left.every((id, index) => id === right[index]);
}

function toggleToolcallPreview(ids: string[], anchorEl: HTMLButtonElement | null) {
  const normalizedIds = ids
    .map((id) => String(id || "").trim())
    .filter(Boolean);
  if (normalizedIds.length === 0) return;
  if (sameToolcallGroup(activeToolcallIds.value, normalizedIds)) {
    closeToolcallPreview();
    return;
  }
  void openToolcallPreview(normalizedIds, anchorEl);
}

function handleDocumentPointerDown(event: PointerEvent) {
  if (activeToolcallIds.value.length === 0) return;
  const target = event.target;
  if (!(target instanceof Node)) {
    closeToolcallPreview();
    return;
  }
  if (target instanceof HTMLElement && target.closest('[data-toolcall-pill="true"]')) return;
  if (target instanceof HTMLElement && target.closest(".ecall-md-toolcall-popup")) return;
  if (toolcallPopupRef.value?.contains(target)) return;
  closeToolcallPreview();
}

function handleWindowResizeOrScroll() {
  if (activeToolcallIds.value.length === 0) return;
  if (!(activeToolcallAnchorEl.value instanceof HTMLButtonElement) || !activeToolcallAnchorEl.value.isConnected) {
    closeToolcallPreview();
    return;
  }
  void nextTick(() => positionToolcallPopup());
}

// ==================== Streaming rAF 合批 ====================
const committedText = ref(props.text ?? "");
let pendingRawText = props.text ?? "";
let rafId = 0;

// blocks 模式的 rAF 合批（ChatMessageItem 走 blocks，不走 text）
const committedBlocks = ref<MarkdownBlock[]>(
  Array.isArray(props.blocks) ? [...(props.blocks as MarkdownBlock[])] : [],
);
let pendingBlocks: MarkdownBlock[] | null = null;
let blocksRafId = 0;

function flushPendingTextToCommitted() {
  rafId = 0;
  if (pendingRawText !== committedText.value) {
    committedText.value = pendingRawText;
  }
}

function scheduleRafFlush() {
  if (rafId) return;
  rafId = window.requestAnimationFrame(flushPendingTextToCommitted);
}

function commitTextImmediately(next: string) {
  if (rafId) {
    window.cancelAnimationFrame(rafId);
    rafId = 0;
  }
  pendingRawText = next;
  committedText.value = next;
}

function flushPendingBlocks() {
  blocksRafId = 0;
  if (pendingBlocks) {
    committedBlocks.value = pendingBlocks;
    pendingBlocks = null;
  }
}

function scheduleBlocksFlush() {
  if (blocksRafId) return;
  blocksRafId = window.requestAnimationFrame(flushPendingBlocks);
}

function commitBlocksImmediately(next: MarkdownBlock[]) {
  if (blocksRafId) {
    window.cancelAnimationFrame(blocksRafId);
    blocksRafId = 0;
  }
  pendingBlocks = null;
  committedBlocks.value = [...next];
}

watch(
  () => props.text,
  (nextRaw) => {
    const next = nextRaw ?? "";
    pendingRawText = next;
    if (Array.isArray(props.blocks)) return;
    if (!props.streaming) {
      commitTextImmediately(next);
      return;
    }
    scheduleRafFlush();
  },
);

watch(
  () => props.blocks,
  (nextBlocks) => {
    if (!Array.isArray(nextBlocks)) return;
    const cloned = [...(nextBlocks as MarkdownBlock[])];
    pendingBlocks = cloned;
    if (!props.streaming) {
      commitBlocksImmediately(cloned);
      return;
    }
    scheduleBlocksFlush();
  },
  { deep: false },
);

watch(
  () => props.streaming,
  (streaming, prevStreaming) => {
    if (streaming && !prevStreaming) {
      if (Array.isArray(props.blocks)) {
        if (Array.isArray(props.blocks) && props.blocks !== committedBlocks.value) {
          pendingBlocks = [...(props.blocks as MarkdownBlock[])];
          scheduleBlocksFlush();
        }
      } else {
        pendingRawText = props.text ?? "";
        if (pendingRawText !== committedText.value) scheduleRafFlush();
      }
    }
    if (!streaming) {
      if (rafId) {
        window.cancelAnimationFrame(rafId);
        rafId = 0;
      }
      if (blocksRafId) {
        window.cancelAnimationFrame(blocksRafId);
        blocksRafId = 0;
      }
      pendingBlocks = null;
      if (Array.isArray(props.blocks)) {
        committedBlocks.value = [...(props.blocks as MarkdownBlock[])];
      } else {
        const next = props.text ?? "";
        pendingRawText = next;
        committedText.value = next;
      }
    }
  },
);

// 8ms 配合 rAF 逐帧攒批，120Hz≈8.3ms 直出，避免无谓的高频重复重解析
const STREAM_PARSE_THROTTLE_MS = 8;

// 实例级状态（每个组件实例独立）
const parseState = {
  lastParseTime: 0,
  cachedBlocks: [] as MarkdownBlock[],
  cachedText: "",
  incrementalParser: new IncrementalMarkdownBlockParser(),
  batchLimit: 0,
  batchTimer: 0,
  parseRetryTimer: 0,
};

const batchRendered = ref(0);
const parseRetryTick = ref(0);

function clearParseRetryTimer() {
  if (!parseState.parseRetryTimer) return;
  clearTimeout(parseState.parseRetryTimer);
  parseState.parseRetryTimer = 0;
}

function clearNestedCaches() {
  nestedSimpleCache.clear();
  nestedIncrementalParsers.clear();
}

function parseAndCacheBlocks(text: string, streaming: boolean): MarkdownBlock[] {
  clearParseRetryTimer();
  const prevText = parseState.cachedText;
  const isAppend = !prevText || text.startsWith(prevText);
  if (streaming && prevText && !isAppend) {
    // 非追加改写（如回退/重放）清空嵌套增量解析器，避免旧尾巴残留
    clearNestedCaches();
  }
  if (!streaming && (nestedSimpleCache.size > 0 || nestedIncrementalParsers.size > 0)) {
    clearNestedCaches();
  }
  parseState.lastParseTime = Date.now();
  parseState.cachedText = text;
  parseState.cachedBlocks = streaming
    ? parseState.incrementalParser.parse(text)
    : parseMarkdownBlocks(text, false);
  if (!streaming) parseState.incrementalParser.reset();
  return parseState.cachedBlocks;
}

function scheduleStreamingParseRetry(delayMs: number) {
  if (parseState.parseRetryTimer) return;
  parseState.parseRetryTimer = window.setTimeout(() => {
    parseState.parseRetryTimer = 0;
    parseRetryTick.value += 1;
  }, Math.max(1, delayMs));
}

const allBlocks = computed<MarkdownBlock[]>(() => {
  void parseRetryTick.value;
  if (Array.isArray(props.blocks)) {
    return props.streaming ? committedBlocks.value : (props.blocks as MarkdownBlock[]);
  }
  const text = props.streaming ? committedText.value : (props.text ?? "");
  if (!text) {
    parseState.incrementalParser.reset();
    parseState.cachedText = "";
    parseState.cachedBlocks = [];
    return [];
  }

  if (!props.streaming) {
    return parseAndCacheBlocks(text, false);
  }

  // Streaming: throttle re-parses
  const now = Date.now();
  if (parseState.cachedText === text) return parseState.cachedBlocks;
  const elapsed = now - parseState.lastParseTime;
  if (elapsed < STREAM_PARSE_THROTTLE_MS && parseState.cachedBlocks.length > 0) {
    scheduleStreamingParseRetry(STREAM_PARSE_THROTTLE_MS - elapsed);
    return parseState.cachedBlocks;
  }
  return parseAndCacheBlocks(text, true);
});

// Batch rendering for streaming: reveal blocks progressively
// 参考 x-markdown-vue：流式时不做 20/10/24ms 逐批放出，改为 rAF 直出；仅首包超长时仍保留批限作为保护
const visibleBlocks = computed<MarkdownBlock[]>(() => {
  const blocks = allBlocks.value;
  if (!props.streaming) {
    return blocks;
  }
  // blocks 模式（ChatMessageItem）已走 rAF + tail 增量，直接全量展示，避免尾块被 batchLimit 藏住导致看不到淡入
  if (Array.isArray(props.blocks)) {
    return blocks;
  }
  if (parseState.batchLimit > 0 && parseState.batchLimit < blocks.length) {
    return blocks.slice(0, parseState.batchLimit);
  }
  return blocks;
});

const footnoteIndexMap = computed<Record<string, number>>(() => {
  const map: Record<string, number> = {};
  for (const block of allBlocks.value) {
    if (block.type !== "footnotes") continue;
    block.items.forEach((item, index) => {
      map[item.id] = index + 1;
    });
  }
  return map;
});

// Progressive batch reveal during streaming
watch(
  () => allBlocks.value.length,
  (newLen) => {
    if (!props.streaming) {
      parseState.batchLimit = 0;
      batchRendered.value = newLen;
      return;
    }
    if (parseState.batchLimit === 0) {
      parseState.batchLimit = Math.min(newLen, 20);
      batchRendered.value = parseState.batchLimit;
    }
    scheduleBatchReveal(newLen);
  },
);

watch(
  () => props.streaming,
  (streaming) => {
    if (!streaming) {
      clearParseRetryTimer();
      parseState.incrementalParser.reset();
      parseState.batchLimit = 0;
      if (parseState.batchTimer) {
        clearTimeout(parseState.batchTimer);
        parseState.batchTimer = 0;
      }
    }
  },
);

function scheduleBatchReveal(targetLen: number) {
  if (parseState.batchTimer) return;
  if (parseState.batchLimit >= targetLen) return;
  parseState.batchTimer = window.setTimeout(() => {
    parseState.batchTimer = 0;
    parseState.batchLimit = Math.min(parseState.batchLimit + 10, allBlocks.value.length);
    batchRendered.value = parseState.batchLimit;
    if (parseState.batchLimit < allBlocks.value.length) {
      scheduleBatchReveal(allBlocks.value.length);
    }
  }, 24);
}

onBeforeUnmount(() => {
  if (rafId) {
    window.cancelAnimationFrame(rafId);
    rafId = 0;
  }
  if (blocksRafId) {
    window.cancelAnimationFrame(blocksRafId);
    blocksRafId = 0;
  }
  clearParseRetryTimer();
  if (parseState.batchTimer) {
    clearTimeout(parseState.batchTimer);
    parseState.batchTimer = 0;
  }
  clearNestedCaches();
  document.removeEventListener("pointerdown", handleDocumentPointerDown, true);
  window.removeEventListener("resize", handleWindowResizeOrScroll, true);
  document.removeEventListener("scroll", handleWindowResizeOrScroll, true);
  if (activeFootnoteTimer) {
    clearTimeout(activeFootnoteTimer);
    activeFootnoteTimer = 0;
  }
});

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown, true);
  window.addEventListener("resize", handleWindowResizeOrScroll, true);
  document.addEventListener("scroll", handleWindowResizeOrScroll, true);
});

// ==================== Heading Tag Helper ====================

function headingTag(level: unknown): "h1" | "h2" | "h3" | "h4" {
  const normalized = Math.min(4, Math.max(1, Number(level) || 4));
  return `h${normalized}` as "h1" | "h2" | "h3" | "h4";
}

function nestedMarkdownBlocks(text: string, streaming: boolean, blockKey?: string): MarkdownBlock[] {
  if (!blockKey) {
    const cacheKey = `${streaming ? "s" : "n"}:${text}`;
    const cached = nestedSimpleCache.get(cacheKey);
    if (cached) return cached;
    const blocks = parseMarkdownBlocks(text, streaming);
    if (nestedSimpleCache.size >= NESTED_SIMPLE_CACHE_MAX) {
      const first = nestedSimpleCache.keys().next().value as string | undefined;
      if (first !== undefined) nestedSimpleCache.delete(first);
    }
    nestedSimpleCache.set(cacheKey, blocks);
    return blocks;
  }
  if (!streaming) {
    nestedIncrementalParsers.delete(blockKey);
    const cacheKey = `n:${blockKey}:${text}`;
    const cached = nestedSimpleCache.get(cacheKey);
    if (cached) return cached;
    const blocks = parseMarkdownBlocks(text, false);
    if (nestedSimpleCache.size >= NESTED_SIMPLE_CACHE_MAX) {
      const first = nestedSimpleCache.keys().next().value as string | undefined;
      if (first !== undefined) nestedSimpleCache.delete(first);
    }
    nestedSimpleCache.set(cacheKey, blocks);
    return blocks;
  }
  let parser = nestedIncrementalParsers.get(blockKey);
  if (!parser) {
    parser = new IncrementalMarkdownBlockParser();
    nestedIncrementalParsers.set(blockKey, parser);
    evictNestedParsersIfNeeded();
  }
  return parser.parse(text);
}

// ==================== Inline Renderer ====================

const InlineRenderer = defineComponent({
  name: "InlineRenderer",
  props: {
    segments: {
      type: Array as PropType<InlineSegment[]>,
      required: true,
    },
    localImageBasePath: { type: String, default: "" },
    footnoteIndexMap: {
      type: Object as PropType<Record<string, number>>,
      default: () => ({}),
    },
    onImagePreview: {
      type: Function as PropType<(payload: MarkdownImagePreviewPayload) => void>,
      default: undefined,
    },
  },
  setup(inlineProps) {
    return () => renderSegments(
      inlineProps.segments,
      "root",
      inlineProps.localImageBasePath,
      {
        onToolcallClick: toggleToolcallPreview,
        footnoteIndexMap: inlineProps.footnoteIndexMap,
        onImagePreview: inlineProps.onImagePreview,
      },
    );
  },
});

const BlockRenderer = defineComponent({
  name: "BlockRenderer",
  props: {
    blocks: {
      type: Array as PropType<MarkdownBlock[]>,
      required: true,
    },
    isDark: { type: Boolean, default: false },
    streaming: { type: Boolean, default: false },
    localImageBasePath: { type: String, default: "" },
    footnoteIndexMap: {
      type: Object as PropType<Record<string, number>>,
      default: () => ({}),
    },
    onImagePreview: {
      type: Function as PropType<(payload: MarkdownImagePreviewPayload) => void>,
      default: undefined,
    },
  },
  setup(blockProps) {
    const renderToolcallPill = (
      ids: string[],
      key: string,
    ): VNodeChild => {
      const count = ids.length;
      return h("button", {
        key,
        type: "button",
        class: "ecall-md-toolcall-ref",
        title: count > 1 ? ids.map((id) => `toolcall:${id}`).join("\n") : `toolcall:${ids[0]}`,
        "data-toolcall-id": ids[0],
        "data-toolcall-pill": "true",
        onClick: (event: MouseEvent) => {
          event.preventDefault();
          event.stopPropagation();
          toggleToolcallPreview(ids, event.currentTarget instanceof HTMLButtonElement ? event.currentTarget : null);
        },
      }, [
        h(Wrench, { class: "ecall-md-toolcall-ref-icon" }),
        count > 1
          ? h("span", { class: "ecall-md-toolcall-ref-count" }, `+${count}`)
          : null,
      ]);
    };

    const isLastBlock = (idx: number) => idx === blockProps.blocks.length - 1;

    const renderBlock = (block: MarkdownBlock, index: number): VNodeChild => {
      if (block.type === "heading") {
        return h(headingTag(block.level), { key: `${block.type}-${index}-${block.key}`, class: "ecall-md-heading" }, [
          h(InlineRenderer, {
            segments: cachedParseInlineSegments(block.text),
            localImageBasePath: blockProps.localImageBasePath,
            footnoteIndexMap: blockProps.footnoteIndexMap,
            onImagePreview: blockProps.onImagePreview,
          }),
        ]);
      }
      if (block.type === "quote") {
        const isCurrentLast = isLastBlock(index);
        const nestedStreaming = blockProps.streaming && isCurrentLast;
        const nestedBlocks = nestedMarkdownBlocks(block.text, nestedStreaming, block.key);
        return h("blockquote", { key: `${block.type}-${index}-${block.key}`, class: "ecall-md-quote" }, [
          h(BlockRenderer, {
            blocks: nestedBlocks,
            isDark: blockProps.isDark,
            streaming: nestedStreaming,
            localImageBasePath: blockProps.localImageBasePath,
            footnoteIndexMap: blockProps.footnoteIndexMap,
            onImagePreview: blockProps.onImagePreview,
          }),
        ]);
      }
      if (block.type === "list") {
        const tag = block.ordered ? "ol" : "ul";
        return h(tag, {
          key: `${block.type}-${index}-${block.key}`,
          class: block.ordered ? "ecall-md-list ecall-md-list-ordered" : "ecall-md-list",
        }, block.items.map((item, itemIndex) => h("li", {
          key: `${index}-${itemIndex}`,
          value: block.ordered && item.value ? item.value : undefined,
        }, [
          h(InlineRenderer, {
            segments: cachedParseInlineSegments(item.text),
            localImageBasePath: blockProps.localImageBasePath,
            footnoteIndexMap: blockProps.footnoteIndexMap,
            onImagePreview: blockProps.onImagePreview,
          }),
        ])));
      }
      if (block.type === "table") {
        return h("div", { key: `${block.type}-${index}-${block.key}`, class: "ecall-md-table-wrap" }, [
          h("table", { class: "ecall-md-table" }, [
            h("thead", [
              h("tr", block.headers.map((cell, ci) => h("th", { key: `${index}-h-${ci}` }, [
                h(InlineRenderer, {
                  segments: cachedParseInlineSegments(cell),
                  localImageBasePath: blockProps.localImageBasePath,
                  footnoteIndexMap: blockProps.footnoteIndexMap,
                  onImagePreview: blockProps.onImagePreview,
                }),
              ]))),
            ]),
            h("tbody", block.rows.map((row, ri) => h("tr", { key: `${index}-r-${ri}` }, normalizedTableRow(row, block.headers.length).map((cell, ci) => h("td", { key: `${index}-r-${ri}-c-${ci}` }, [
              h(InlineRenderer, {
                segments: cachedParseInlineSegments(cell),
                localImageBasePath: blockProps.localImageBasePath,
                footnoteIndexMap: blockProps.footnoteIndexMap,
                onImagePreview: blockProps.onImagePreview,
              }),
            ]))))),
          ]),
        ]);
      }
      if (block.type === "code") {
        return h(CodeBlock, {
          key: `${block.type}-${index}-${block.key}`,
          lang: block.lang,
          code: block.text,
          blockKey: block.key,
          isDark: blockProps.isDark,
          streaming: blockProps.streaming,
          copyText: t("common.copy"),
          copiedText: "已复制",
          expandText: t("common.expand"),
          preparingText: t("chat.statusPreparingMessage"),
        });
      }
      if (block.type === "math") {
        return h(MathBlock, {
          key: `${block.type}-${index}-${block.key}`,
          text: block.text,
          raw: block.raw,
          blockKey: block.key,
          streaming: blockProps.streaming,
        });
      }
      if (block.type === "details") {
        const isCurrentLast = isLastBlock(index);
        const nestedStreaming = blockProps.streaming && isCurrentLast;
        const nestedBlocks = nestedMarkdownBlocks(block.body, nestedStreaming, `${block.key}::body`);
        return h("details", {
          key: `${block.type}-${index}-${block.key}`,
          class: "ecall-md-details",
          open: block.open || undefined,
        }, [
          h("summary", { class: "ecall-md-details-summary" }, [
            h(InlineRenderer, {
              segments: cachedParseInlineSegments(block.summary),
              localImageBasePath: blockProps.localImageBasePath,
              footnoteIndexMap: blockProps.footnoteIndexMap,
              onImagePreview: blockProps.onImagePreview,
            }),
          ]),
          block.body
            ? h("div", { class: "ecall-md-details-body" }, [
              h(BlockRenderer, {
                blocks: nestedBlocks,
                isDark: blockProps.isDark,
                streaming: nestedStreaming,
                localImageBasePath: blockProps.localImageBasePath,
                footnoteIndexMap: blockProps.footnoteIndexMap,
                onImagePreview: blockProps.onImagePreview,
              }),
            ])
            : null,
        ]);
      }
      if (block.type === "footnotes") {
        return h("section", { key: `${block.type}-${index}-${block.key}`, class: "ecall-md-footnotes" }, [
          h("ol", { class: "ecall-md-footnote-list" }, block.items.map((item) => h("li", {
            id: footnoteDomId(item.id),
            key: `${index}-${item.id}`,
            class: ["ecall-md-footnote-item", activeFootnoteId.value === item.id ? "ecall-md-footnote-active" : ""],
          }, [
            h(InlineRenderer, {
              segments: cachedParseInlineSegments(item.text),
              localImageBasePath: blockProps.localImageBasePath,
              footnoteIndexMap: blockProps.footnoteIndexMap,
              onImagePreview: blockProps.onImagePreview,
            }),
          ]))),
        ]);
      }
      if (block.type === "hr") {
        return h("hr", { key: `${block.type}-${index}-${block.key}`, class: "ecall-md-hr" });
      }
      return h("p", { key: `${block.type}-${index}-${block.key}`, class: "ecall-md-paragraph" }, [
        h(InlineRenderer, {
          segments: cachedParseInlineSegments(block.text),
          localImageBasePath: blockProps.localImageBasePath,
          footnoteIndexMap: blockProps.footnoteIndexMap,
          onImagePreview: blockProps.onImagePreview,
        }),
      ]);
    };

    return () => {
      const nodes: VNodeChild[] = [];
      for (let index = 0; index < blockProps.blocks.length; index += 1) {
        const block = blockProps.blocks[index];
        const grouped = consumeCrossParagraphToolGroup(blockProps.blocks, index);
        if (grouped && grouped.ids.length > 0) {
          const pill = renderToolcallPill(
            grouped.ids,
            `toolcall-group-pill-${index}-${grouped.endIndex}`,
          );

          if (grouped.mode === "replace") {
            // marker-only 起点：整段替换为一个合并扳手；若终点带正文，紧跟其后渲染剩余正文
            if (!grouped.stripLeadingOnEnd || grouped.endIndex === index) {
              nodes.push(h("p", {
                key: `toolcall-group-${index}-${grouped.endIndex}`,
                class: "ecall-md-paragraph",
              }, [pill]));
            } else {
              nodes.push(h("p", {
                key: `toolcall-group-${index}-${grouped.endIndex}`,
                class: "ecall-md-paragraph",
              }, [
                pill,
                ...(grouped.endBodySegments.length > 0
                  ? renderSegments(
                    grouped.endBodySegments,
                    `toolcall-group-end-${index}`,
                    blockProps.localImageBasePath,
                    {
                      onToolcallClick: toggleToolcallPreview,
                      footnoteIndexMap: blockProps.footnoteIndexMap,
                      onImagePreview: blockProps.onImagePreview,
                    },
                  )
                  : []),
              ]));
            }
            index = grouped.endIndex;
            continue;
          }

          // trailing 模式：保留起始正文，尾部 tools 换成合并扳手
          const startChildren: VNodeChild[] = [];
          if (grouped.startBodySegments.length > 0) {
            startChildren.push(...renderSegments(
              grouped.startBodySegments,
              `toolcall-group-start-${index}`,
              blockProps.localImageBasePath,
              {
                onToolcallClick: toggleToolcallPreview,
                footnoteIndexMap: blockProps.footnoteIndexMap,
                onImagePreview: blockProps.onImagePreview,
              },
            ));
          }
          startChildren.push(pill);
          nodes.push(h("p", {
            key: `toolcall-group-start-${index}`,
            class: "ecall-md-paragraph",
          }, startChildren));

          // 中间 marker-only 段落已被吞掉；终点若有正文（strip leading 后），单独输出
          if (grouped.endIndex > index && grouped.stripLeadingOnEnd && grouped.endBodySegments.length > 0) {
            nodes.push(h("p", {
              key: `toolcall-group-end-${grouped.endIndex}`,
              class: "ecall-md-paragraph",
            }, renderSegments(
              grouped.endBodySegments,
              `toolcall-group-end-body-${grouped.endIndex}`,
              blockProps.localImageBasePath,
              {
                onToolcallClick: toggleToolcallPreview,
                footnoteIndexMap: blockProps.footnoteIndexMap,
                onImagePreview: blockProps.onImagePreview,
              },
            )));
          } else if (grouped.endIndex > index && !grouped.stripLeadingOnEnd) {
            // 终点是 marker-only，已并入 pill，无需再画
          } else if (grouped.endIndex > index && grouped.stripLeadingOnEnd && grouped.endBodySegments.length === 0) {
            // 终点只剩 leading tools，已并入 pill
          }

          index = grouped.endIndex;
          continue;
        }
        nodes.push(renderBlock(block, index));
      }
      return nodes;
    };
  },
});

type RenderSegmentOptions = {
  onToolcallClick?: (ids: string[], anchorEl: HTMLButtonElement | null) => void;
  footnoteIndexMap?: Record<string, number>;
  onImagePreview?: (payload: MarkdownImagePreviewPayload) => void;
};

function footnoteDomId(rawId: string): string {
  const id = String(rawId || "").trim();
  const slug = id
    .replace(/[^A-Za-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40) || "note";
  return `ecall-fn-${rendererInstanceId}-${slug}-${stableMarkdownRuntimeKey(id)}`;
}

function scrollToFootnote(rawId: string) {
  const id = String(rawId || "").trim();
  if (!id) return;
  const target = rendererRootRef.value?.ownerDocument.getElementById(footnoteDomId(id));
  if (!(target instanceof HTMLElement)) return;
  activeFootnoteId.value = id;
  target.scrollIntoView({ behavior: "smooth", block: "center", inline: "nearest" });
  if (activeFootnoteTimer) clearTimeout(activeFootnoteTimer);
  activeFootnoteTimer = window.setTimeout(() => {
    if (activeFootnoteId.value === id) activeFootnoteId.value = "";
    activeFootnoteTimer = 0;
  }, 1800);
}

function renderSegments(
  segments: InlineSegment[],
  keyPrefix: string,
  localImageBasePath = "",
  options: RenderSegmentOptions = {},
): VNodeChild[] {
  const nodes: VNodeChild[] = [];
  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    if (segment.type === "code") {
      nodes.push(h("code", { key: `${keyPrefix}-c-${index}`, class: "ecall-md-inline-code" }, segment.text));
      continue;
    }
    if (segment.type === "html_br") {
      nodes.push(h("br", { key: `${keyPrefix}-br-${index}` }));
      continue;
    }
    if (segment.type === "toolcall_ref") {
      const grouped = consumeGroupedToolcallRefs(segments, index);
      const ids = grouped?.ids || [segment.id];
      index = grouped?.endIndex ?? index;
      const count = ids.length;
      nodes.push(h("button", {
        key: `${keyPrefix}-toolcall-${index}`,
        type: "button",
        class: "ecall-md-toolcall-ref",
        title: count > 1 ? ids.map((id) => `toolcall:${id}`).join("\n") : `toolcall:${ids[0]}`,
        "data-toolcall-id": ids[0],
        "data-toolcall-pill": "true",
        onClick: (event: MouseEvent) => {
          event.preventDefault();
          event.stopPropagation();
          options.onToolcallClick?.(ids, event.currentTarget instanceof HTMLButtonElement ? event.currentTarget : null);
        },
      }, [
        h(Wrench, { class: "ecall-md-toolcall-ref-icon" }),
        count > 1
          ? h("span", { class: "ecall-md-toolcall-ref-count" }, `+${count}`)
          : null,
      ]));
      continue;
    }
    if (segment.type === "footnote_ref") {
      const footnoteIndex = options.footnoteIndexMap?.[segment.id];
      if (!footnoteIndex) {
        nodes.push(`[^${segment.id}]`);
        continue;
      }
      nodes.push(h("sup", {
        key: `${keyPrefix}-fn-${index}`,
        class: "ecall-md-footnote-ref",
      }, [
        h("button", {
          type: "button",
          class: "ecall-md-footnote-link",
          "aria-label": `注脚 ${footnoteIndex}`,
          onClick: (event: MouseEvent) => {
            event.preventDefault();
            event.stopPropagation();
            scrollToFootnote(segment.id);
          },
        }, String(footnoteIndex)),
      ]));
      continue;
    }
    if (segment.type === "math") {
      nodes.push(h(InlineMath, { key: `${keyPrefix}-m-${index}`, text: segment.text, raw: segment.raw, display: segment.display }));
      continue;
    }
    if (segment.type === "html_sub") {
      nodes.push(h("sub", { key: `${keyPrefix}-sub-${index}`, class: "ecall-md-sub" }, renderSegments(segment.children, `${keyPrefix}-sub-${index}`, localImageBasePath, options)));
      continue;
    }
    if (segment.type === "html_sup") {
      nodes.push(h("sup", { key: `${keyPrefix}-sup-${index}`, class: "ecall-md-sup" }, renderSegments(segment.children, `${keyPrefix}-sup-${index}`, localImageBasePath, options)));
      continue;
    }
    if (segment.type === "html_kbd") {
      nodes.push(h("kbd", { key: `${keyPrefix}-kbd-${index}`, class: "ecall-md-kbd" }, renderSegments(segment.children, `${keyPrefix}-kbd-${index}`, localImageBasePath, options)));
      continue;
    }
    if (segment.type === "html_mark") {
      nodes.push(h("mark", { key: `${keyPrefix}-mark-${index}`, class: "ecall-md-mark" }, renderSegments(segment.children, `${keyPrefix}-mark-${index}`, localImageBasePath, options)));
      continue;
    }
    if (segment.type === "link") {
      const href = sanitizeMarkdownHref(segment.href);
      if (!href) {
        nodes.push(h("span", { key: `${keyPrefix}-a-${index}` }, segment.text));
        continue;
      }
      const isExternalUrl = /^https?:\/\//i.test(href);
      const localReference = isExternalUrl ? null : parseLocalFileReference(href);
      const lineSuffix = localReference?.line
        ? `:${localReference.line}${localReference.column ? `:${localReference.column}` : ""}`
        : "";
      const linkText = lineSuffix && !segment.text.trim().endsWith(lineSuffix)
        ? `${segment.text}${lineSuffix}`
        : segment.text;
      nodes.push(h("a", {
        key: `${keyPrefix}-a-${index}`,
        href: isExternalUrl ? href : "#",
        "data-href": isExternalUrl ? undefined : href,
        class: "ecall-md-link",
        ...(isExternalUrl ? { target: "_blank", rel: "noopener noreferrer" } : {}),
      }, linkText));
      continue;
    }
    if (segment.type === "image") {
      nodes.push(h(LazyMarkdownImage, {
        key: `${keyPrefix}-img-${index}`,
        src: segment.src,
        alt: segment.alt,
        localImageBasePath,
        onOpenPreview: options.onImagePreview,
      }));
      continue;
    }
    if (segment.type === "imageLink") {
      const href = sanitizeMarkdownHref(segment.href);
      const imageNode = h(LazyMarkdownImage, {
        src: segment.src,
        alt: segment.alt,
        localImageBasePath,
        onOpenPreview: options.onImagePreview,
      });
      if (!href) {
        nodes.push(imageNode);
        continue;
      }
      const isExternalUrl = /^https?:\/\//i.test(href);
      nodes.push(h("a", {
        key: `${keyPrefix}-img-link-${index}`,
        href: isExternalUrl ? href : "#",
        "data-href": isExternalUrl ? undefined : href,
        class: "ecall-md-image-link",
        ...(isExternalUrl ? { target: "_blank", rel: "noopener noreferrer" } : {}),
      }, [imageNode]));
      continue;
    }
    if (segment.type === "strong") {
      nodes.push(h("strong", { key: `${keyPrefix}-b-${index}`, class: "ecall-md-strong" }, renderSegments(segment.children, `${keyPrefix}-b-${index}`, localImageBasePath, options)));
      continue;
    }
    if (segment.type === "em") {
      nodes.push(h("em", { key: `${keyPrefix}-i-${index}`, class: "ecall-md-em" }, renderSegments(segment.children, `${keyPrefix}-i-${index}`, localImageBasePath, options)));
      continue;
    }
    if (segment.type === "strongEm") {
      nodes.push(h("strong", { key: `${keyPrefix}-bi-${index}`, class: "ecall-md-strong" }, [
        h("em", { class: "ecall-md-em" }, renderSegments(segment.children, `${keyPrefix}-bi-${index}`, localImageBasePath, options)),
      ]));
      continue;
    }
    if (segment.type === "delete") {
      nodes.push(h("del", { key: `${keyPrefix}-d-${index}`, class: "ecall-md-del" }, renderSegments(segment.children, `${keyPrefix}-d-${index}`, localImageBasePath, options)));
      continue;
    }
    nodes.push(segment.text);
  }
  return nodes;
}

function sanitizeMarkdownHref(rawHref: string): string {
  const href = String(rawHref || "").replace(/[\u0000-\u001F\u007F]/g, "").trim();
  if (!href) return "";
  if (href.startsWith("#") || href.startsWith("/") || href.startsWith("./") || href.startsWith("../")) {
    return href;
  }
  if (href.startsWith("\\\\") || /^[A-Za-z]:[\\/]/.test(href)) {
    return href.replace(/\\/g, "/");
  }
  if (/^file:/i.test(href)) {
    try {
      const url = new URL(href);
      const decodedPath = decodeURIComponent(url.pathname || "");
      if (url.host && url.host !== "localhost") {
        return `\\\\${url.host}${decodedPath.replace(/\//g, "\\")}`;
      }
      return decodedPath.replace(/^\/([A-Za-z]:)/, "$1");
    } catch {
      return "";
    }
  }
  const schemeMatch = href.match(/^([A-Za-z][A-Za-z0-9+.-]*):/);
  if (!schemeMatch) return href;
  const scheme = schemeMatch[1].toLowerCase();
  if (scheme === "http" || scheme === "https" || scheme === "mailto") {
    return href;
  }
  return "";
}

// ==================== Inline Math (KaTeX) ====================

const InlineMath = defineComponent({
  name: "InlineMath",
  props: {
    text: { type: String, required: true },
    raw: { type: String, default: "" },
    display: { type: Boolean, default: false },
  },
  setup(mathProps) {
    const html = computed(() => {
      try {
        const katex = (window as any).__ecall_katex;
        if (!katex) return null;
        return katex.renderToString(mathProps.text, { throwOnError: false, displayMode: false });
      } catch {
        return null;
      }
    });

    function openContextMenu(event: MouseEvent) {
      event.preventDefault();
      event.stopPropagation();
      emit("mathContextMenu", {
        clientX: event.clientX,
        clientY: event.clientY,
        copyText: resolveMathCopyText(mathProps.text, mathProps.raw, mathProps.display),
      });
    }

    return () => {
      const mathNode = html.value
        ? h("span", { class: "ecall-md-inline-math", innerHTML: html.value })
        : h("code", { class: "ecall-md-inline-code" }, `$${mathProps.text}$`);
      if (html.value) {
        return h("span", { class: "ecall-md-inline-math-wrap", onContextmenu: openContextMenu }, [mathNode]);
      }
      return h("span", { class: "ecall-md-inline-math-wrap ecall-md-inline-math-fallback", onContextmenu: openContextMenu }, [mathNode]);
    };
  },
});

// ==================== Math Block (KaTeX) ====================

const MathBlock = defineComponent({
  name: "MathBlock",
  props: {
    text: { type: String, required: true },
    raw: { type: String, default: "" },
    blockKey: { type: String, default: "" },
    streaming: { type: Boolean, default: false },
  },
  setup(mathProps) {
    const html = computed(() => {
      try {
        const katex = (window as any).__ecall_katex;
        if (!katex) return null;
        return katex.renderToString(mathProps.text, { throwOnError: false, displayMode: true });
      } catch {
        return null;
      }
    });

    function openContextMenu(event: MouseEvent) {
      event.preventDefault();
      event.stopPropagation();
      emit("mathContextMenu", {
        clientX: event.clientX,
        clientY: event.clientY,
        copyText: resolveMathCopyText(mathProps.text, mathProps.raw, true),
      });
    }

    return () => {
      const mathNode = html.value
        ? h("div", { class: "ecall-md-math-block", innerHTML: html.value })
        : h("pre", { class: "ecall-md-math-fallback" }, [h("code", null, mathProps.text)]);
      return h("div", { class: "ecall-md-math-block-shell", onContextmenu: openContextMenu }, [mathNode]);
    };
  },
});

</script>

<style>
.ecall-md-renderer {
  --ecall-md-block-radius: var(--radius-box, 0.5rem);
  min-width: 0;
  max-width: 100%;
  overflow-wrap: anywhere;
  white-space: normal;
  font-size: var(--app-text-sm-size);
  line-height: 1.5;
  font-weight: var(--ecall-md-body-weight-setting, var(--app-font-weight, 400));
  font-variation-settings: "wght" var(--ecall-md-body-weight-setting, var(--app-font-weight, 400));
}

.ecall-md-chat {
  font-size: var(--app-chat-message-text-size, var(--app-text-sm-size));
}

.ecall-md-renderer > :first-child {
  margin-top: 0;
}

.ecall-md-renderer > :last-child {
  margin-bottom: 0;
}

/* ==================== Headings ==================== */
.ecall-md-heading {
  margin: 0.25rem 0;
  font-weight: var(--ecall-md-heading-weight-setting, var(--app-font-strong-weight, 600));
  font-variation-settings: "wght" var(--ecall-md-heading-weight-setting, var(--app-font-strong-weight, 600));
  line-height: 1.45;
}

h1.ecall-md-heading { font-size: var(--app-text-markdown-heading-1-size); }
h2.ecall-md-heading { font-size: var(--app-text-markdown-heading-2-size); }
h3.ecall-md-heading { font-size: var(--app-text-markdown-heading-3-size); }
h4.ecall-md-heading { font-size: var(--app-text-markdown-heading-4-size); }

/* ==================== Paragraph ==================== */
.ecall-md-paragraph {
  margin: 0.25rem 0;
  white-space: pre-wrap;
}

.ecall-md-toolcall-ref {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.15rem;
  min-width: 1.15rem;
  height: 1.15rem;
  padding: 0 0.34rem;
  margin-left: 0.18rem;
  border: 0;
  border-radius: 999px;
  background: var(--color-base-300);
  color: color-mix(in srgb, currentColor 72%, transparent);
  cursor: pointer;
  line-height: 1;
  vertical-align: text-top;
  white-space: nowrap;
}

.ecall-md-toolcall-ref:hover {
  background: color-mix(in srgb, var(--color-base-content) 10%, var(--color-base-300));
  color: currentColor;
}

.ecall-md-toolcall-ref-icon {
  width: 0.72rem;
  height: 0.72rem;
  pointer-events: none;
}

.ecall-md-toolcall-ref-count {
  font-size: var(--app-text-micro-size);
  font-weight: 700;
  line-height: 1;
  pointer-events: none;
}

.ecall-md-toolcall-scroll {
  scrollbar-gutter: auto;
  scrollbar-width: none;
  -ms-overflow-style: none;
}

.ecall-md-toolcall-scroll::-webkit-scrollbar {
  width: 0;
  height: 0;
}

/* ==================== Footnotes ==================== */
.ecall-md-footnote-ref {
  margin-left: 0.08rem;
  font-size: var(--app-text-caption-size);
  font-weight: var(--ecall-md-strong-weight-setting, var(--app-font-strong-weight, 600));
  font-variation-settings: "wght" var(--ecall-md-strong-weight-setting, var(--app-font-strong-weight, 600));
  line-height: 0;
  vertical-align: super;
}

.ecall-md-footnote-link {
  border: 0;
  padding: 0;
  background: transparent;
  color: var(--color-primary);
  font: inherit;
  text-decoration: none;
  cursor: pointer;
}

.ecall-md-footnote-link:hover {
  text-decoration: underline;
  text-underline-offset: 0.12em;
}

.ecall-md-footnotes {
  margin: 0.75rem 0 0.25rem;
  padding-top: 0.45rem;
  border-top: 1px solid color-mix(in srgb, currentColor 14%, transparent);
  color: color-mix(in srgb, currentColor 76%, transparent);
  font-size: var(--app-text-xs-size);
  line-height: 1.45;
}

.ecall-md-footnote-list {
  margin: 0;
  padding-left: 1.15rem;
  list-style: decimal;
  list-style-position: outside;
}

.ecall-md-footnote-item {
  margin: 0.18rem 0;
  padding-left: 0.12rem;
  white-space: pre-wrap;
}

.ecall-md-footnote-item::marker {
  color: color-mix(in srgb, currentColor 72%, transparent);
  font-weight: var(--ecall-md-strong-weight-setting, var(--app-font-strong-weight, 600));
  font-variation-settings: "wght" var(--ecall-md-strong-weight-setting, var(--app-font-strong-weight, 600));
}

.ecall-md-footnote-item:target,
.ecall-md-footnote-active {
  border-radius: 0.25rem;
  background: color-mix(in srgb, var(--color-primary) 12%, transparent);
}

/* ==================== Blockquote ==================== */
.ecall-md-quote {
  margin: 0.35rem 0;
  padding: 0.5rem 0.68rem 0.5rem 0.82rem;
  border: 1px solid color-mix(in srgb, var(--color-base-300) 72%, transparent);
  border-radius: var(--ecall-md-block-radius);
  background: color-mix(in srgb, var(--color-base-300) 58%, transparent);
  color: color-mix(in srgb, currentColor 86%, transparent);
  white-space: pre-wrap;
}

/* ==================== Lists ==================== */
.ecall-md-list {
  margin: 0.25rem 0;
  padding-left: 0.85rem;
}

.ecall-md-list li {
  margin: 0.12rem 0;
  padding-left: 0;
}

.ecall-md-list-ordered {
  list-style: decimal;
}

ul.ecall-md-list {
  list-style: disc;
}

/* ==================== Table ==================== */
.ecall-md-table-wrap {
  max-width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  margin: 0.35rem 0;
  border-radius: var(--ecall-md-block-radius);
}

.ecall-md-table {
  width: max-content;
  min-width: 100%;
  border-collapse: separate;
  border-spacing: 0;
  border: 1px solid color-mix(in srgb, var(--color-base-300) 72%, transparent);
  border-radius: inherit;
  font-size: var(--app-text-xs-size);
  line-height: 1.45;
  overflow: hidden;
}

.ecall-md-table th,
.ecall-md-table td {
  border: 0;
  border-bottom: 1px solid color-mix(in srgb, currentColor 20%, transparent);
  padding: 0.32rem 0.48rem;
  text-align: left;
  vertical-align: top;
}

.ecall-md-table th {
  font-weight: var(--ecall-md-table-heading-weight-setting, var(--app-font-strong-weight, 600));
  font-variation-settings: "wght" var(--ecall-md-table-heading-weight-setting, var(--app-font-strong-weight, 600));
  background: var(--color-base-300);
}

.ecall-md-table td {
  background: color-mix(in srgb, var(--color-base-300) 34%, transparent);
}

.ecall-md-table th:last-child,
.ecall-md-table td:last-child {
  border-right: 0;
}

.ecall-md-table tbody tr:last-child td {
  border-bottom: 0;
}

/* ==================== Code Block ==================== */
.ecall-md-code-block {
  margin: 0.25rem 0;
  border-radius: var(--ecall-md-block-radius);
  overflow: hidden;
  background: var(--ecall-md-code-bg);
}

.ecall-md-code-title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.4rem 0.75rem;
  background: transparent;
}

.ecall-md-code-lang {
  font-size: var(--app-text-caption-size);
  color: color-mix(in srgb, currentColor 80%, transparent);
}

.ecall-md-code-actions {
  display: flex;
  align-items: center;
  gap: 0.3rem;
}

.ecall-md-code-action,
.ecall-md-code-copy {
  border: none;
  background: none;
  padding: 0.1rem 0.35rem;
  font-size: var(--app-text-caption-size);
  color: color-mix(in srgb, currentColor 80%, transparent);
  cursor: pointer;
  border-radius: 0.25rem;
}

.ecall-md-code-action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0.18rem;
}

.ecall-md-code-action-icon {
  width: 0.9rem;
  height: 0.9rem;
}

.ecall-md-code-action:hover,
.ecall-md-code-copy:hover {
  background: color-mix(in srgb, currentColor 10%, transparent);
  color: currentColor;
}

.ecall-md-code-body {
  overflow-x: auto;
  padding: 0.75rem;
  font-family: var(--app-code-font-family);
  font-weight: var(--ecall-md-code-weight-setting, var(--app-font-medium-weight, 500));
  font-variation-settings: "wght" var(--ecall-md-code-weight-setting, var(--app-font-medium-weight, 500));
  font-size: inherit;
  line-height: inherit;
  margin: 0 !important;
  white-space: pre;
}

.ecall-md-code-plain {
  margin: 0 !important;
}

.ecall-md-code-plain code {
  background: transparent;
  border: 0;
  padding: 0;
  font: inherit;
  color: inherit;
}

/* shiki 输出的 pre 去掉自带的 margin/padding/圆角，由外壳统一控制 */
.ecall-md-code-body pre,
.ecall-md-code-body pre.shiki,
.ecall-md-code-body .shiki {
  margin: 0 !important;
  padding: 0 !important;
  border-radius: 0 !important;
  border: 0 !important;
  overflow: visible !important;
  background: transparent !important;
}

.ecall-md-code-body pre code {
  background: transparent !important;
  border: 0 !important;
  padding: 0 !important;
  box-shadow: none !important;
  font: inherit;
}

.ecall-md-code-body .line,
.ecall-md-code-body .shiki span {
  background: transparent !important;
  box-shadow: none !important;
  text-shadow: none !important;
}

/* 代码块背景跟随 DaisyUI 主题，Shiki 只负责 token 颜色 */
.ecall-md-dark .ecall-md-code-block {
  --ecall-md-code-bg: color-mix(in srgb, var(--color-base-300) 50%, transparent);
}

.ecall-md-light .ecall-md-code-block {
  --ecall-md-code-bg: color-mix(in srgb, var(--color-base-300) 50%, transparent);
}

/* ==================== Inline Code ==================== */
.ecall-md-inline-code {
  border-radius: 0.28rem;
  /* 行内代码统一红字、无底色：底色在 code 密集处会盖过文字本身 */
  color: var(--ecall-md-inline-code-color);
  background: transparent;
  padding: 0.08rem 0.28rem;
  font-family: var(--app-code-font-family);
  font-weight: var(--ecall-md-code-weight-setting, var(--app-font-medium-weight, 500));
  font-variation-settings: "wght" var(--ecall-md-code-weight-setting, var(--app-font-medium-weight, 500));
  font-size: inherit;
  line-height: inherit;
}

/* ==================== Links ==================== */
.ecall-md-link {
  color: var(--color-primary);
  text-decoration: underline;
  text-decoration-thickness: 0.08em;
  text-underline-offset: 0.18em;
}

.ecall-md-image-link {
  display: inline-flex;
  max-width: 100%;
  text-decoration: none;
  vertical-align: middle;
}

/* ==================== Images ==================== */
.ecall-md-image {
  display: inline-block;
  max-width: min(28rem, 80vw);
  max-height: 18rem;
  border-radius: 0.5rem;
  object-fit: contain;
  vertical-align: middle;
}

.ecall-md-meme-image {
  max-width: min(150px, 40vw);
  max-height: 150px;
}

.ecall-md-local-image {
  cursor: zoom-in;
}

.ecall-md-image-placeholder {
  display: inline-flex;
  min-width: 4rem;
  max-width: min(16rem, 60vw);
  min-height: 2.75rem;
  align-items: center;
  justify-content: center;
  padding: 0.35rem 0.55rem;
  border: 1px dashed color-mix(in srgb, currentColor 28%, transparent);
  border-radius: 0.5rem;
  color: color-mix(in srgb, currentColor 62%, transparent);
  font-size: var(--app-text-xs-size);
  line-height: 1.25;
  vertical-align: middle;
  cursor: zoom-in;
}

.ecall-md-image-error {
  cursor: default;
  color: color-mix(in srgb, currentColor 42%, transparent);
}

/* ==================== Emphasis ==================== */
.ecall-md-strong {
  font-weight: var(--ecall-md-strong-weight-setting, var(--app-font-strong-weight, 600));
  font-variation-settings: "wght" var(--ecall-md-strong-weight-setting, var(--app-font-strong-weight, 600));
}

.ecall-md-em {
  font-style: italic;
}

.ecall-md-del {
  text-decoration: line-through;
  color: color-mix(in srgb, currentColor 76%, transparent);
}

/* ==================== HR ==================== */
.ecall-md-hr {
  margin: 0.65rem 0;
  border: 0;
  border-top: 1px solid color-mix(in srgb, currentColor 16%, transparent);
}

/* ==================== Math ==================== */
.ecall-md-inline-math-wrap {
  position: relative;
  display: inline-flex;
  max-width: 100%;
  align-items: baseline;
  gap: 0.15rem;
  vertical-align: baseline;
}

.ecall-md-inline-math {
  display: inline;
}

.ecall-md-inline-math-fallback {
  align-items: center;
}

.ecall-md-math-block-shell {
  display: block;
  position: relative;
  width: 100%;
  max-width: 100%;
  margin: 0.35rem 0;
  vertical-align: top;
}

.ecall-md-math-block {
  margin: 0;
  width: 100%;
  max-width: 100%;
  overflow-x: auto;
  overflow-y: hidden;
  padding: 0.1rem 2rem 0.1rem 0.25rem;
  text-align: center;
  scrollbar-gutter: auto;
}

.ecall-md-math-block :where(.katex-display) {
  margin: 0;
  overflow: visible;
}

.ecall-md-math-fallback {
  margin: 0;
  padding: 0.5rem 2.2rem 0.5rem 0.65rem;
  background: color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 0.4rem;
  font-size: var(--app-text-xs-size);
  overflow-x: auto;
}

/* ==================== Mermaid ==================== */
.ecall-md-mermaid-shell {
  position: relative;
  margin: 0.35rem 0;
}

.ecall-md-mermaid-copy {
  position: absolute;
  top: 0.35rem;
  right: 0.35rem;
  z-index: 1;
  opacity: 0.58;
}

.ecall-md-mermaid-shell:hover .ecall-md-mermaid-copy,
.ecall-md-mermaid-copy:focus-visible {
  opacity: 1;
}

.ecall-md-mermaid-block {
  margin: 0;
  overflow-x: auto;
  padding-right: 2.2rem;
  text-align: center;
  transition: opacity 120ms ease;
}

.ecall-md-mermaid-block-buffering {
  opacity: 0.82;
}

.ecall-md-mermaid-block svg {
  max-width: 100%;
  height: auto;
}

.ecall-md-mermaid-loading {
  margin: 0.35rem 0;
  min-height: 3.5rem;
  padding: 1.1rem 2.2rem 1.1rem 0.65rem;
  display: flex;
  align-items: center;
  justify-content: center;
  text-align: center;
  color: color-mix(in srgb, currentColor 55%, transparent);
  font-size: var(--app-text-xs-size);
}

.ecall-md-mermaid-error {
  margin: 0.35rem 0;
}

.ecall-md-mermaid-error pre {
  padding: 0.5rem 2.2rem 0.5rem 0.65rem;
  background: color-mix(in srgb, currentColor 8%, transparent);
  border-radius: 0.4rem;
  font-size: var(--app-text-xs-size);
  overflow-x: auto;
}

.ecall-md-mermaid-error-msg {
  margin-top: 0.25rem;
  font-size: var(--app-text-xs-size);
  color: var(--color-error, #ef4444);
}

/* ==================== Document Variant ==================== */
.ecall-md-document {
  font-size: var(--app-text-base-size);
  line-height: 1.7;
}

.ecall-md-document .ecall-md-heading {
  margin: 1.2rem 0 0.5rem;
  font-weight: var(--ecall-md-heading-weight-setting, var(--app-font-strong-weight, 600));
  font-variation-settings: "wght" var(--ecall-md-heading-weight-setting, var(--app-font-strong-weight, 600));
  line-height: 1.35;
}

.ecall-md-document h1.ecall-md-heading { font-size: var(--app-text-markdown-document-heading-1-size); }
.ecall-md-document h2.ecall-md-heading { font-size: var(--app-text-markdown-document-heading-2-size); }
.ecall-md-document h3.ecall-md-heading { font-size: var(--app-text-markdown-document-heading-3-size); }
.ecall-md-document h4.ecall-md-heading { font-size: var(--app-text-markdown-document-heading-4-size); }

.ecall-md-document h1.ecall-md-heading,
.ecall-md-document h2.ecall-md-heading {
  padding-bottom: 0.35rem;
  border-bottom: 1px solid color-mix(in srgb, var(--color-base-300) 84%, var(--color-base-content) 10%);
}

.ecall-md-document .ecall-md-paragraph {
  margin: 0.6rem 0;
  line-height: 1.8;
}

.ecall-md-document .ecall-md-quote {
  margin: 0.6rem 0;
  padding: 0.65rem 0.85rem 0.65rem 0.95rem;
  line-height: 1.75;
}

.ecall-md-document .ecall-md-footnotes {
  margin: 1.1rem 0 0.45rem;
  padding-top: 0.65rem;
  font-size: var(--app-text-xs-size);
  line-height: 1.65;
}

.ecall-md-document .ecall-md-list {
  margin: 0.5rem 0;
  padding-left: 1.4rem;
  line-height: 1.75;
}

.ecall-md-document .ecall-md-list li {
  margin: 0.25rem 0;
}

.ecall-md-document .ecall-md-table-wrap {
  margin: 0.75rem 0;
}

.ecall-md-document .ecall-md-table {
  font-size: var(--app-text-sm-size);
  line-height: 1.55;
}

.ecall-md-document .ecall-md-table th,
.ecall-md-document .ecall-md-table td {
  padding: 0.45rem 0.65rem;
}

.ecall-md-document .ecall-md-code-block {
  margin: 0.75rem 0;
}

.ecall-md-document .ecall-md-code-title {
  padding: 0.45rem 0.85rem;
}

.ecall-md-document .ecall-md-code-body {
  padding: 0.85rem 1.1rem;
  font-size: var(--app-text-sm-size);
  line-height: 1.6;
}

.ecall-md-document .ecall-md-hr {
  margin: 1.2rem 0;
}

.ecall-md-document .ecall-md-math-block {
  margin: 0.75rem 0;
}

.ecall-md-document .ecall-md-mermaid-block {
  margin: 0.75rem 0;
}

</style>
