<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { UnfoldVertical } from "@lucide/vue";
import TimelinePreviewMarkdown from "./TimelinePreviewMarkdown.vue";

type TimelineAnchor = {
  id: string;
  userText: string;
  assistantTail: string;
  index: number;
};

const props = withDefaults(defineProps<{
  anchors: TimelineAnchor[];
  activeIndex: number | null;
  hoveredIndex: number | null;
  anchorEl?: HTMLElement | null;
  /** 蛇形起点后面再挂一个样式不同的预览点：底行让出一格给它，点它就是打开时间线预览 */
  previewEnabled?: boolean;
  previewLabel?: string;
  visible?: boolean;
}>(), {
  previewEnabled: false,
  previewLabel: "",
  visible: true,
});

const emit = defineEmits<{
  (e: "hover", index: number | null): void;
  (e: "jump", index: number): void;
  (e: "preview"): void;
  (e: "enter-zone"): void;
  (e: "leave-zone"): void;
}>();

const SAFE = 12;
const MAX_GAP = 31;
const MIN_GAP = 21;
const PADDING = 21;
const DOT_HIT = 23;
const DOT_SMALL = 8;
const DOT_FOCUSED = 14;
const PREVIEW_MAX_W = 440;
const PREVIEW_PAD = 10;
const PREVIEW_GAP = 4;
const PREVIEW_H = 152;
// 卡片与目标点的间隙
const PREVIEW_OFFSET = 14;

const hostRef = ref<HTMLElement | null>(null);
const winSize = ref({
  w: typeof window !== "undefined" ? window.innerWidth : 1920,
  h: typeof window !== "undefined" ? window.innerHeight : 1080,
});
// 布局基准视口：聊天中间对话区（data-chat-center-pane），而不是整个窗口。
// 窗口里除了聊天区还可能有左右分栏，按窗口算会让蛇板和预览卡飘出聊天区。
const containerRect = ref<Rect | null>(null);
const anchorRect = ref<DOMRect | null>(null);
let anchorRo: ResizeObserver | null = null;
let viewportTimer: number | null = null;

type Rect = { left: number; top: number; right: number; bottom: number; width: number; height: number };

function resolveContainerEl(): HTMLElement | null {
  const el = props.anchorEl;
  if (!el) return null;
  return el.closest('[data-chat-center-pane="true"]') as HTMLElement | null;
}

function updateViewport() {
  if (typeof window === "undefined") return;
  winSize.value = { w: window.innerWidth, h: window.innerHeight };
  const container = resolveContainerEl();
  if (container) {
    const r = container.getBoundingClientRect();
    containerRect.value = { left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height };
    return;
  }
  // 兜底：拿不到聊天区时退回整窗视口
  containerRect.value = {
    left: 0,
    top: 0,
    right: winSize.value.w,
    bottom: winSize.value.h,
    width: winSize.value.w,
    height: winSize.value.h,
  };
}

const viewport = computed<Rect>(() => containerRect.value ?? {
  left: 0,
  top: 0,
  right: winSize.value.w,
  bottom: winSize.value.h,
  width: winSize.value.w,
  height: winSize.value.h,
});

// 把「距窗口右边/下边的距离」限制在聊天区之内
function clampRight(desired: number, cardW: number): number {
  const vw = winSize.value.w;
  const vp = viewport.value;
  const min = vw - vp.right + SAFE;
  const max = vw - vp.left - cardW - SAFE;
  return Math.min(Math.max(desired, min), Math.max(min, max));
}

function clampBottom(desired: number, cardH: number): number {
  const vh = winSize.value.h;
  const vp = viewport.value;
  const min = vh - vp.bottom + SAFE;
  const max = vh - vp.top - cardH - SAFE;
  return Math.min(Math.max(desired, min), Math.max(min, max));
}

function updateAnchorRect() {
  const el = props.anchorEl;
  if (!el) {
    anchorRect.value = null;
    return;
  }
  anchorRect.value = el.getBoundingClientRect();
}

function measureAll() {
  updateViewport();
  updateAnchorRect();
}

function scheduleMeasure() {
  if (viewportTimer != null) return;
  viewportTimer = window.setTimeout(() => {
    viewportTimer = null;
    measureAll();
  }, 16);
}

function onWindowResize() {
  scheduleMeasure();
}

watch(
  () => props.anchorEl,
  (el) => {
    if (anchorRo) {
      anchorRo.disconnect();
      anchorRo = null;
    }
    if (el && typeof ResizeObserver !== "undefined") {
      anchorRo = new ResizeObserver(() => scheduleMeasure());
      anchorRo.observe(el);
      const parent = el.parentElement;
      if (parent) anchorRo.observe(parent);
      const container = resolveContainerEl();
      if (container) anchorRo.observe(container);
    }
    scheduleMeasure();
  },
  { immediate: true },
);

onMounted(() => {
  measureAll();
  window.addEventListener("resize", onWindowResize);
  if (typeof window !== "undefined" && (window as any).visualViewport) {
    (window as any).visualViewport.addEventListener("resize", onWindowResize);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", onWindowResize);
  if (typeof window !== "undefined" && (window as any).visualViewport) {
    (window as any).visualViewport.removeEventListener("resize", onWindowResize);
  }
  if (anchorRo) {
    anchorRo.disconnect();
    anchorRo = null;
  }
  if (viewportTimer != null) {
    clearTimeout(viewportTimer);
    viewportTimer = null;
  }
});

// 起点后面那一格让给预览点：它自己占一格，锚点从它左边开始排
function previewColsFor(): number {
  return props.previewEnabled ? 1 : 0;
}

// 底行让出若干格位给预览点，其余行满铺；据此估算容纳 N 个锚点需要几行
function rowsForCols(N: number, cols: number, previewCols: number): number {
  const bottomCapacity = Math.max(0, cols - previewCols);
  if (N <= bottomCapacity) return 1;
  return 1 + Math.ceil((N - bottomCapacity) / cols);
}

const layout = computed(() => {
  const N = props.anchors.length;
  const vp = viewport.value;
  const ar = anchorRect.value;
  const availW = vp.width - SAFE * 2;
  const availH = ar ? Math.max(32, ar.bottom - vp.top - SAFE) : vp.height - SAFE * 2;
  if (N === 0 || availW < MIN_GAP || availH < MIN_GAP) return null;
  let gap = MAX_GAP;
  let cols = 1;
  let rows = 1;
  let previewCols = 0;
  let fits = false;
  for (; gap >= MIN_GAP; gap -= 2) {
    const cc = previewColsFor();
    const maxCols = Math.max(1, Math.floor(availW / gap));
    const maxRows = Math.max(1, Math.floor(availH / gap));
    // 底行要铺满全部锚点，预览点另占一格；列数不超过可用宽度
    const wantCols = Math.min(maxCols, Math.max(cc + 1, N + cc));
    const wantRows = rowsForCols(N, wantCols, cc);
    if (wantRows <= maxRows) {
      cols = wantCols;
      rows = wantRows;
      previewCols = cc;
      fits = true;
      break;
    }
  }
  if (!fits) {
    gap = MIN_GAP;
    previewCols = previewColsFor();
    cols = Math.max(previewCols + 1, Math.floor(availW / gap));
    rows = rowsForCols(N, cols, previewCols);
  }
  const cardW = Math.max(Math.min((cols - 1) * gap + PADDING * 2, availW), 32);
  const cardH = Math.max(Math.min((rows - 1) * gap + PADDING * 2, availH), 32);
  const posByIndex = new Map<number, { x: number; y: number }>();
  const ordered: Array<{ x: number; y: number }> = [];
  let k = 0;
  for (let rowFromBottom = 0; rowFromBottom < rows && k < N; rowFromBottom++) {
    const colsInRow = rowFromBottom === 0 ? Math.max(1, cols - previewCols) : cols;
    for (let i = 0; i < colsInRow && k < N; i++, k++) {
      const anchor = props.anchors[N - 1 - k];
      if (!anchor) continue;
      // 底行从预览点左侧一格起往左排，奇数行反向，蛇形衔接
      const col = rowFromBottom % 2 === 0 ? cols - 1 - previewCols - i : i;
      const row = rows - 1 - rowFromBottom;
      const x = PADDING + Math.max(0, col) * gap;
      const y = PADDING + row * gap;
      posByIndex.set(anchor.index, { x, y });
      ordered.push({ x, y });
    }
  }
  return { gap, cols, rows, cardW, cardH, posByIndex, ordered };
});

// 预览点就落在底行最右那一格：与锚点用同一套网格坐标，卡片被夹紧时也不会错位
const previewDotStyle = computed(() => {
  const l = layout.value;
  if (!l || !props.previewEnabled) return { display: "none" };
  return {
    // inline 定位，避免 daisyUI tooltip 自带的 position 规则抢走定位
    position: "absolute",
    left: `${PADDING + (l.cols - 1) * l.gap}px`,
    top: `${PADDING + (l.rows - 1) * l.gap}px`,
    width: `${DOT_HIT}px`,
    height: `${DOT_HIT}px`,
    transform: "translate(-50%, -50%)",
  } as Record<string, string>;
});

const boardFixedStyle = computed(() => {
  const vw = winSize.value.w;
  const vh = winSize.value.h;
  if (!layout.value) {
    const ar = anchorRect.value;
    if (ar) {
      const right = clampRight(vw - ar.right, 32);
      const bottom = clampBottom(vh - ar.bottom, 32);
      return {
        position: "fixed",
        right: `${Math.round(right)}px`,
        bottom: `${Math.round(bottom)}px`,
        width: "32px",
        height: "32px",
      } as Record<string, string>;
    }
    return {
      position: "fixed",
      right: `${clampRight(vw - SAFE - 32, 32)}px`,
      bottom: `${clampBottom(vh - SAFE - 40 - 32, 32)}px`,
      width: "32px",
      height: "32px",
    } as Record<string, string>;
  }
  const cardW = layout.value.cardW;
  const cardH = layout.value.cardH;
  const ar = anchorRect.value;
  if (!ar) {
    return {
      position: "fixed",
      right: `${SAFE}px`,
      bottom: `${SAFE + 40}px`,
      width: `${cardW}px`,
      height: `${cardH}px`,
    } as Record<string, string>;
  }
  // 蛇板底边对齐 anchor 底边：从按钮原位展开，不悬浮；边界限制在聊天区内
  const right = clampRight(vw - ar.right, cardW);
  const bottom = clampBottom(vh - ar.bottom, cardH);
  return {
    position: "fixed",
    right: `${Math.round(right)}px`,
    bottom: `${Math.round(bottom)}px`,
    width: `${cardW}px`,
    height: `${cardH}px`,
  } as Record<string, string>;
});

const boardViewportPos = computed(() => {
  const vw = winSize.value.w;
  const vh = winSize.value.h;
  if (!layout.value) {
    const ar = anchorRect.value;
    if (ar) {
      const right = clampRight(vw - ar.right, 32);
      const bottom = clampBottom(vh - ar.bottom, 32);
      return { left: vw - right - 32, top: vh - bottom - 32 };
    }
    return { left: vw - clampRight(vw - SAFE - 32, 32) - 32, top: vh - clampBottom(vh - SAFE - 40 - 32, 32) - 32 };
  }
  const s = boardFixedStyle.value;
  const right = parseFloat(String(s.right).replace("px", "")) || 0;
  const bottom = parseFloat(String(s.bottom).replace("px", "")) || 0;
  return {
    left: vw - right - layout.value.cardW,
    top: vh - bottom - layout.value.cardH,
  };
});

const smoothSnakePath = computed(() => {
  const ordered = layout.value?.ordered;
  if (!ordered || ordered.length < 2) return "";
  if (ordered.length === 2) return `M ${ordered[0].x} ${ordered[0].y} L ${ordered[1].x} ${ordered[1].y}`;

  let d = `M ${ordered[0].x} ${ordered[0].y}`;
  for (let i = 1; i < ordered.length - 1; i++) {
    const pPrev = ordered[i - 1]!;
    const pCurr = ordered[i]!;
    const pNext = ordered[i + 1]!;

    const v1x = pCurr.x - pPrev.x;
    const v1y = pCurr.y - pPrev.y;
    const v2x = pNext.x - pCurr.x;
    const v2y = pNext.y - pCurr.y;

    const cross = v1x * v2y - v1y * v2x;
    if (Math.abs(cross) < 1e-4) {
      d += ` L ${pCurr.x} ${pCurr.y}`;
      continue;
    }

    const d1 = Math.hypot(v1x, v1y);
    const d2 = Math.hypot(v2x, v2y);
    const r = Math.min(8, d1 / 2, d2 / 2);

    const entryX = (pCurr.x - (v1x / d1) * r).toFixed(1);
    const entryY = (pCurr.y - (v1y / d1) * r).toFixed(1);
    const exitX = (pCurr.x + (v2x / d2) * r).toFixed(1);
    const exitY = (pCurr.y + (v2y / d2) * r).toFixed(1);

    d += ` L ${entryX} ${entryY} Q ${pCurr.x} ${pCurr.y} ${exitX} ${exitY}`;
  }
  const last = ordered[ordered.length - 1]!;
  d += ` L ${last.x} ${last.y}`;
  return d;
});

// 悬停在预览点上时，不要让高亮回落到当前位：放大反馈只属于被悬停的预览点本身
const previewDotHovered = ref(false);
const focusedIndex = computed(() => props.hoveredIndex ?? (previewDotHovered.value ? null : props.activeIndex));

function handlePreviewDotEnter() {
  previewDotHovered.value = true;
  // 清掉锚点悬停，避免预览卡贴在光标旁盖住这个点
  emit("hover", null);
}

function handlePreviewDotLeave() {
  previewDotHovered.value = false;
}

function isFocused(index: number) {
  return focusedIndex.value === index;
}

function isActive(index: number) {
  return props.activeIndex === index;
}

function dotStyle(anchor: TimelineAnchor): Record<string, string> {
  const p = layout.value?.posByIndex.get(anchor.index);
  if (!p) return { display: "none" };
  return {
    left: `${p.x}px`,
    top: `${p.y}px`,
    width: `${DOT_HIT}px`,
    height: `${DOT_HIT}px`,
    transform: "translate(-50%, -50%)",
  };
}

function toLocal(clientX: number, clientY: number) {
  const r = hostRef.value?.getBoundingClientRect();
  if (!r) return null;
  return { x: clientX - r.left, y: clientY - r.top };
}

function findNearestAnchor(clientX: number, clientY: number): TimelineAnchor | null {
  const l = layout.value;
  if (!l || props.anchors.length === 0) return null;
  const p = toLocal(clientX, clientY);
  if (!p) return null;
  let best: TimelineAnchor | null = null;
  let bestD = Infinity;
  for (const a of props.anchors) {
    const pos = l.posByIndex.get(a.index);
    if (!pos) continue;
    const dx = pos.x - p.x;
    const dy = pos.y - p.y;
    const d = dx * dx + dy * dy;
    if (d < bestD) { bestD = d; best = a; }
  }
  return best;
}

let scrubPointerId: number | null = null;
let scrubStartX = 0;
let scrubStartY = 0;
let lastScrubJumpAt = 0;
let lastScrubJumpIndex: number | null = null;

function handleBoardPointerDown(event: PointerEvent) {
  if (event.button !== 0) return;
  const nearest = findNearestAnchor(event.clientX, event.clientY);
  if (!nearest) return;
  scrubPointerId = event.pointerId;
  scrubStartX = event.clientX;
  scrubStartY = event.clientY;
  emit("hover", nearest.index);
  try { (hostRef.value as HTMLElement | null)?.setPointerCapture(event.pointerId); } catch {}
  if (event.pointerType === "touch") event.preventDefault();
}

function handleBoardPointerMove(event: PointerEvent) {
  if (scrubPointerId == null || scrubPointerId !== event.pointerId) return;
  const dx = event.clientX - scrubStartX;
  const dy = event.clientY - scrubStartY;
  if (dx * dx + dy * dy > 16) {}
  const nearest = findNearestAnchor(event.clientX, event.clientY);
  if (nearest) emit("hover", nearest.index);
}

function handleBoardPointerUp(event: PointerEvent) {
  if (scrubPointerId == null || scrubPointerId !== event.pointerId) return;
  const nearest = findNearestAnchor(event.clientX, event.clientY);
  const jumpIndex = nearest?.index ?? props.hoveredIndex ?? props.activeIndex;
  if (jumpIndex != null) {
    lastScrubJumpAt = Date.now();
    lastScrubJumpIndex = jumpIndex;
    emit("jump", jumpIndex);
  }
  try { (hostRef.value as HTMLElement | null)?.releasePointerCapture(event.pointerId); } catch {}
  scrubPointerId = null;
}

function handleDotClick(anchor: TimelineAnchor) {
  if (lastScrubJumpIndex === anchor.index && Date.now() - lastScrubJumpAt < 500) return;
  emit("jump", anchor.index);
}

const previewAnchor = computed(() => {
  const idx = props.hoveredIndex;
  if (idx == null) return null;
  return props.anchors.find((a) => a.index === idx) ?? null;
});

const tooltipW = computed(() => {
  const cardW = layout.value?.cardW ?? PREVIEW_MAX_W;
  return Math.min(PREVIEW_MAX_W, Math.max(280, cardW - PREVIEW_PAD * 2 - 8));
});

// 卡片钉在被悬停的那个锚点上方，横向对准它
const tipAnchor = computed(() => {
  const l = layout.value;
  if (!l) return null;
  const idx = props.hoveredIndex;
  if (idx == null) return null;
  return l.posByIndex.get(idx) ?? null;
});

const tooltipBelow = computed(() => {
  const a = tipAnchor.value;
  if (!a) return false;
  const vp = viewport.value;
  const anchorY = boardViewportPos.value.top + a.y;
  const aboveTop = anchorY - PREVIEW_OFFSET - PREVIEW_H;
  const belowBottom = anchorY + PREVIEW_OFFSET + PREVIEW_H;
  if (aboveTop >= vp.top + SAFE) return false;
  if (belowBottom <= vp.bottom - SAFE) return true;
  const spaceAbove = anchorY - (vp.top + SAFE);
  const spaceBelow = (vp.bottom - SAFE) - anchorY;
  return spaceBelow > spaceAbove;
});

const tooltipStyle = computed(() => {
  const a = tipAnchor.value;
  const vp = viewport.value;
  if (!a) return { display: "none" } as Record<string, string>;
  const w = tooltipW.value;
  const boardPos = boardViewportPos.value;
  // 卡片钉在被悬停的那个点上：横向对准它，纵向贴在它上方（上方真的放不下才翻到下方）
  const minX = vp.left + SAFE + w / 2;
  const maxX = vp.right - SAFE - w / 2;
  const left = Math.min(Math.max(boardPos.left + a.x, minX), Math.max(minX, maxX));
  const anchorY = boardPos.top + a.y;
  const style: Record<string, string> = {
    position: "fixed",
    left: `${Math.round(left)}px`,
    width: `${w}px`,
    transform: "translateX(-50%)",
  };
  if (tooltipBelow.value) {
    style.top = `${Math.round(anchorY + PREVIEW_OFFSET)}px`;
  } else {
    // 用底边定位，卡片多高都不会把自己推远
    style.bottom = `${Math.round(vp.bottom - (anchorY - PREVIEW_OFFSET))}px`;
  }
  return style;
});
</script>

<template>
  <Teleport to="body">
    <Transition name="ecall-snake-board">
      <div
        v-if="visible && layout"
        ref="hostRef"
        class="ecall-snake-board-card pointer-events-auto fixed z-[100] rounded-2xl border border-base-300 bg-base-100/80 shadow-lg backdrop-blur-md backdrop-saturate-150 select-none touch-none"
        :style="boardFixedStyle"
        @pointerdown="handleBoardPointerDown"
        @pointermove="handleBoardPointerMove"
        @pointerup="handleBoardPointerUp"
        @pointercancel="handleBoardPointerUp"
        @mouseenter="emit('enter-zone')"
        @mouseleave="emit('leave-zone'); emit('hover', null)"
      >
        <div class="ecall-snake-board-content relative h-full w-full overflow-hidden rounded-2xl">
          <svg
            v-if="smoothSnakePath"
            class="pointer-events-none absolute inset-0 h-full w-full"
            :viewBox="`0 0 ${layout.cardW} ${layout.cardH}`"
          >
            <path
              :d="smoothSnakePath"
              fill="none"
              stroke="var(--color-base-300)"
              stroke-width="2.5"
              stroke-linecap="round"
              stroke-linejoin="round"
              class="opacity-70"
            />
                    </svg>
          <button
            v-for="anchor in anchors"
            :key="anchor.id"
            type="button"
            class="absolute flex items-center justify-center rounded-full"
            :style="dotStyle(anchor)"
            :aria-label="anchor.userText.slice(0, 30)"
            @mouseenter="emit('hover', anchor.index)"
            @focus="emit('hover', anchor.index)"
            @click.stop="handleDotClick(anchor)"
          >
            <!-- 当前激活锚点：双层脉冲光环定位标 -->
            <span
              v-if="isActive(anchor.index)"
              class="relative flex items-center justify-center pointer-events-none transition-transform duration-150"
              :style="isFocused(anchor.index) ? 'transform: scale(1.18)' : ''"
              :aria-hidden="true"
            >
              <span class="absolute h-5 w-5 rounded-full bg-primary/20 animate-ping opacity-60 pointer-events-none" style="animation-duration: 2.4s" />
              <span class="absolute h-4 w-4 rounded-full border-2 border-primary bg-primary/10 shadow-sm" />
              <span class="h-1.5 w-1.5 rounded-full bg-primary shadow" />
            </span>
            <!-- 其他普通/悬停锚点 -->
            <span
              v-else
              class="rounded-full transition-all duration-150"
              :class="isFocused(anchor.index)
                ? 'bg-primary shadow-sm ring-4 ring-primary/25 scale-125'
                : 'bg-base-content/40 hover:bg-base-content/70'"
              :style="{
                width: isFocused(anchor.index) ? `${DOT_FOCUSED}px` : `${DOT_SMALL}px`,
                height: isFocused(anchor.index) ? `${DOT_FOCUSED}px` : `${DOT_SMALL}px`,
              }"
            />
          </button>
        </div>
        <!-- 起点后面那一格：预览点，悬停由 daisyUI 原生 tooltip 说明，点击打开时间线预览 -->
        <div
          v-if="previewEnabled"
          class="tooltip tooltip-top flex items-center justify-center"
          :style="previewDotStyle"
          :data-tip="previewLabel"
        >
          <button
            type="button"
            class="ecall-snake-preview-dot group flex items-center justify-center rounded-full"
            :aria-label="previewLabel"
            @mouseenter="handlePreviewDotEnter"
            @mouseleave="handlePreviewDotLeave"
            @pointerdown.stop
            @click.stop="emit('preview')"
          >
            <span
              class="flex h-[18px] w-[18px] items-center justify-center rounded-full border border-primary/50 bg-primary/10 transition-transform duration-150 group-hover:scale-125 group-hover:border-primary group-hover:bg-primary/20"
              aria-hidden="true"
            >
              <UnfoldVertical
                class="h-3 w-3 text-primary/70 transition-colors group-hover:text-primary"
                aria-hidden="true"
              />
            </span>
          </button>
        </div>
      </div>
    </Transition>
    <Transition name="ecall-timeline-preview">
      <div
        v-if="visible && previewAnchor"
        class="pointer-events-none fixed z-[101]"
        :style="tooltipStyle"
      >
        <div v-if="tooltipBelow" class="mx-auto h-2 w-2 -translate-y-[1px] rotate-45 border-t border-l border-base-300 bg-base-100" />
        <div class="overflow-hidden rounded-xl border border-base-300 bg-base-100/95 shadow-xl backdrop-blur-md" :style="{ padding: `${PREVIEW_PAD}px` }">
          <div class="flex min-w-0 flex-col overflow-hidden" :style="{ maxHeight: `calc(5 * 1.25rem + ${PREVIEW_GAP}px)` }">
            <span class="block shrink-0 truncate font-semibold leading-5 text-base-content" style="height: 1.25rem; line-height: 1.25rem">
              <TimelinePreviewMarkdown :text="previewAnchor.userText" :clamp="80" />
            </span>
            <span
              v-if="(previewAnchor.assistantTail || '').trim()"
              class="block overflow-hidden leading-5 text-base-content/65"
              :style="{ marginTop: `${PREVIEW_GAP}px`, display: '-webkit-box', WebkitLineClamp: 4, WebkitBoxOrient: 'vertical', maxHeight: 'calc(4 * 1.25rem)' }"
            >
              <TimelinePreviewMarkdown :text="previewAnchor.assistantTail" :clamp="320" />
            </span>
          </div>
        </div>
        <div v-if="!tooltipBelow" class="mx-auto h-2 w-2 -translate-y-[1px] rotate-45 border-b border-r border-base-300 bg-base-100" />
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.ecall-snake-board-enter-active {
  transition:
    width 260ms cubic-bezier(0.16, 1, 0.3, 1),
    height 260ms cubic-bezier(0.16, 1, 0.3, 1),
    right 260ms cubic-bezier(0.16, 1, 0.3, 1),
    bottom 260ms cubic-bezier(0.16, 1, 0.3, 1),
    border-radius 260ms cubic-bezier(0.16, 1, 0.3, 1),
    opacity 180ms ease-out;
}
.ecall-snake-board-leave-active {
  transition:
    width 180ms ease-in,
    height 180ms ease-in,
    right 180ms ease-in,
    bottom 180ms ease-in,
    border-radius 180ms ease-in,
    opacity 150ms ease-in;
}
.ecall-snake-board-enter-from,
.ecall-snake-board-leave-to {
  width: 2.25rem !important;
  height: 2.25rem !important;
  border-radius: 9999px !important;
  opacity: 0;
}
.ecall-snake-board-content {
  transition: opacity 180ms ease 60ms;
}
.ecall-snake-board-enter-from .ecall-snake-board-content,
.ecall-snake-board-leave-to .ecall-snake-board-content {
  opacity: 0;
}
.ecall-timeline-preview-enter-active,
.ecall-timeline-preview-leave-active {
  transition: opacity 150ms ease, transform 150ms ease;
}
.ecall-timeline-preview-enter-from,
.ecall-timeline-preview-leave-to {
  opacity: 0;
  transform: translateX(-50%) scale(0.96);
}
</style>
