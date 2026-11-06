<template>
  <TransitionGroup
    name="ecall-activity-item"
    tag="ul"
    class="ecall-activity-timeline space-y-2"
    :appear="false"
    @scroll.capture="onTimelineScroll"
  >
    <li
      v-for="(item, itemIndex) in items"
      :key="`activity-${activityItemKey(item)}`"
      v-memo="activityItemMemo(item, itemIndex === items.length - 1)"
      :class="[
        activityItemNodeClass(item),
        'relative flex gap-1.5 min-w-0',
      ]"
    >
      <!-- ========== 思维链节点：左列圆点与竖线 / 右列折叠头、正文 ========== -->
      <template v-if="item.kind === 'reasoning'">
        <!-- 左列：圆点与竖线。竖线绝对定位，从圆点下沿一直画到下一个条目的圆点上沿，
             负 bottom 抵消 ul 的 space-y 间距；改间距档位时这里要同步 -->
        <div class="relative flex w-4 shrink-0 flex-col items-center">
          <span
            v-if="itemIndex !== 0"
            class="absolute top-0 h-1 w-px bg-current opacity-30"
          />
          <div class="flex h-[1.5em] shrink-0 items-center justify-center">
            <svg viewBox="0 0 24 24" class="h-4 w-4">
              <circle cx="12" cy="12" r="10" fill="currentColor" />
              <path d="M12 4Q13 11 20 12Q13 13 12 20Q11 13 4 12Q11 11 12 4Z" class="text-base-200" fill="currentColor" />
            </svg>
          </div>
          <span
            v-if="itemIndex !== items.length - 1"
            class="absolute top-[1.5em] -bottom-2 left-1/2 w-px -translate-x-1/2 bg-current opacity-30"
          />
        </div>

        <!-- 右列：正文滚动区 -->
        <div class="flex min-w-0 flex-1 flex-col">
          <div
            v-if="activityItemHasBody(item)"
            class="relative min-w-0 pb-1.5"
            :style="{ '--ecall-reasoning-max-h': `calc(${REASONING_SCROLL_LINES} * 1.5em)` }"
          >
            <!-- 正文：常驻滚动区，高度锁在 30 行。不截断、不展开、不收起——
                 全文进 DOM，超出就滚。上下边缘渐隐跟着滚动位置走，
                 渐隐挂在这一层（scroller）而不是内容壳：内容壳会跟着内容滚，mask 位置就飘了。 -->
            <OverlayScrollArea
              :scroller-class="`ecall-reasoning-scroll-limit ${reasoningFadeClass(item)}`.trim()"
              overscroll="auto"
            >
              <div
                class="relative min-w-0 whitespace-pre-wrap wrap-break-word text-xs leading-normal pr-3"
                :class="activityItemDetailClass(item)"
                :ref="(el) => bindReasoningBody(item, el)"
              >
                <InlineMarkdownText v-if="!activityItemPlainBody(item)" :text="activityItemText(item)" />
                <template v-else>
                  <!-- 长文本按行切块：每块独立排版，配合 content-visibility 跳过屏外块的布局 -->
                  <div
                    v-for="chunk in activityItemPlainChunks(item)"
                    :key="chunk.index"
                    class="ecall-reasoning-plain-chunk"
                  >{{ chunk.text }}</div>
                </template>
              </div>
            </OverlayScrollArea>
          </div>

        </div>
      </template>

      <!-- ========== 工具与正文节点：同一套骨架（左列圆点与竖线 / 右列内容与留白行） ========== -->
      <template v-else>
        <div class="relative flex w-4 shrink-0 flex-col items-center">
          <span
            v-if="itemIndex !== 0"
            class="absolute top-0 h-1 w-px bg-current opacity-30"
          />
          <div class="flex h-6 shrink-0 items-center justify-center">
            <!-- 颜色不写死：与同一列的圆点、工具图标一样继承条目颜色。
                 doing 时条目还没有 resultText，继承到的是普通档位；
                 失败/拒绝时条目整体转成 error/warning，指示器跟着一起转。 -->
            <AppLoading
              v-if="item.kind === 'tool' && item.status === 'doing'"
              size="xs"
            />
            <svg
              v-else-if="item.kind === 'content'"
              viewBox="0 0 24 24"
              class="h-4 w-4 text-base-content/60"
            >
              <circle cx="12" cy="12" r="10" fill="currentColor" />
              <path d="M8.4 10.2h7.2M8.4 13.8h7.2" class="text-base-100" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
            </svg>
            <svg
              v-else
              viewBox="0 0 24 24"
              class="h-4 w-4"
            >
              <circle cx="12" cy="12" r="10" fill="currentColor" />
              <path
                d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"
                class="text-base-100"
                fill="currentColor"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
                transform="translate(3.84 3.84) scale(0.68)"
              />
            </svg>
          </div>
          <!-- 时间轴竖线：绝对定位跨过条目间隙，负 bottom 抵消 ul 的 space-y 间距 -->
          <span
            v-if="itemIndex !== items.length - 1"
            class="absolute top-6 -bottom-2 left-1/2 w-px -translate-x-1/2 bg-current opacity-30"
          />
        </div>
        <div class="flex min-w-0 flex-1 flex-col">
          <details
            v-if="item.kind === 'tool' && activityItemCanExpand(item)"
            class="collapse rounded-none"
            :open="activityItemExpanded(item)"
            @toggle="onToolDetailsToggle(item, $event)"
          >
            <summary class="collapse-title group flex min-h-0 items-center gap-1.5 pl-0 pr-1 py-1 text-xs hover:bg-base-200">
              <span
                class="ecall-activity-item-summary min-w-0 inline-flex items-center gap-1.5 overflow-hidden text-ellipsis whitespace-nowrap"
                :class="activityItemTitleClass(item)"
              >
                <template v-if="activityItemSemantic(item)">
                  <span class="font-medium shrink-0">{{ activityItemSemantic(item)!.action }}</span>
                  <span class="truncate">{{ activityItemSemantic(item)!.target }}</span>
                  <span
                    v-if="activityItemSemantic(item)!.lineRange"
                    class="font-mono text-xs opacity-75 shrink-0"
                  >{{ activityItemSemantic(item)!.lineRange }}</span>
                  <span
                    v-if="activityItemSemantic(item)!.extra"
                    class="opacity-75 shrink-0 truncate"
                  >{{ activityItemSemantic(item)!.extra }}</span>
                  <span
                    v-if="activityItemStatusSuffix(item)"
                    class="opacity-75 shrink-0 font-normal"
                  >{{ activityItemStatusSuffix(item) }}</span>
                </template>
                <template v-else>
                  <span>{{ activityItemDisplay(item).text }}</span>
                </template>
                <span
                  v-if="activityItemDisplay(item).adds > 0"
                  class="ml-1 shrink-0 font-mono text-success"
                >+{{ activityItemDisplay(item).adds }}</span>
                <span
                  v-if="activityItemDisplay(item).removes > 0"
                  class="ml-1 shrink-0 font-mono text-error"
                >-{{ activityItemDisplay(item).removes }}</span>
              </span>
              <AppIcon
                name="chevron-right"
                class="ecall-activity-chevron mt-0.5 h-3.5 w-3.5 shrink-0 transition-transform duration-150"
              />
            </summary>
            <div v-if="activityItemExpanded(item)" class="collapse-content pb-2 pr-1 pt-1 pl-0">
              <div class="relative">
                <OverlayScrollArea scroller-class="max-h-72 py-2 pr-2" overscroll="auto">
                  <div class="flex flex-col gap-2">
                    <!-- 卡片一：参数（输入）。不设标题行，内容直接当卡片 -->
                    <div
                      v-if="toolDetailParts(item).args"
                      :class="['rounded-lg', TOOL_CARD_SURFACE]"
                    >
                      <!-- 参数区：整块走工具绿，其中的键名单独跳到行内代码红 -->
                      <ToolArgsView
                        :text="toolDetailParts(item).args"
                        :dark="props.dark"
                        class="px-2.5 py-2 font-mono text-xs leading-relaxed"
                      />
                    </div>

                    <!-- 卡片二：结果（输出）。默认空态，点一下才把结果取回来 -->
                    <div
                      v-if="toolDetailParts(item).result || (item.contentOmitted && props.state.toolResultOverrides[activityItemKey(item)] === undefined)"
                      :class="['rounded-lg', TOOL_CARD_SURFACE]"
                    >
                      <!-- 结果区：沿用条目那一档 context 灰 -->
                      <pre
                        v-if="toolDetailParts(item).result"
                        class="m-0 whitespace-pre-wrap break-all px-2.5 py-2 text-xs leading-relaxed"
                        :class="props.dark ? 'ecall-activity-result-dark' : 'text-base-content/56'"
                      ><code>{{ toolDetailParts(item).result }}</code></pre>
                      <div v-else class="px-2.5 py-2">
                        <button
                          v-if="toolResultLoadable(item)"
                          type="button"
                          class="btn btn-ghost btn-xs -ml-1 h-6 min-h-0 justify-start gap-1.5 px-1 text-xs font-normal text-base-content/60 hover:bg-base-300/60 hover:text-base-content"
                          :disabled="!!props.state.toolResultLoading[activityItemKey(item)]"
                          @click.stop="loadToolResult(item)"
                        >
                          <AppLoading
                            v-if="props.state.toolResultLoading[activityItemKey(item)]"
                            size="xs"
                          />
                          <AppIcon name="chevron-right" v-else class="h-3 w-3 shrink-0" />
                          {{ t("chat.toolReview.viewResult") || "查看结果" }}
                        </button>
                        <!-- 缺定位上下文时不给按钮：点了也取不回，只留一句说明 -->
                        <span v-else class="text-xs text-base-content/45">
                          {{ t("chat.toolReview.resultOmitted") || "结果已省略" }}
                        </span>
                        <span
                          v-if="props.state.toolResultErrors[activityItemKey(item)]"
                          class="ml-2 text-error"
                        >{{ props.state.toolResultErrors[activityItemKey(item)] }}</span>
                      </div>
                    </div>
                  </div>
                </OverlayScrollArea>
                <!-- 滚动区上下边缘各 0.5rem 淡出 -->
                <div
                  class="ecall-activity-fade-top absolute inset-x-0 top-0 h-2 bg-base-200 pointer-events-none"
                />
                <div
                  class="ecall-activity-fade-bottom absolute inset-x-0 bottom-0 h-2 bg-base-200 pointer-events-none"
                />
              </div>
            </div>
          </details>
          <div
            v-else-if="item.kind === 'tool'"
            class="flex min-h-0 items-center gap-1.5 pr-1 py-1 text-xs"
            @click.stop
          >
            <span
              class="ecall-activity-item-summary min-w-0 flex-1 inline-flex items-center gap-1.5 overflow-hidden text-ellipsis whitespace-nowrap"
              :class="activityItemTitleClass(item)"
            >
              <template v-if="activityItemSemantic(item)">
                <span class="font-medium shrink-0">{{ activityItemSemantic(item)!.action }}</span>
                <span class="truncate">{{ activityItemSemantic(item)!.target }}</span>
                <span
                  v-if="activityItemSemantic(item)!.lineRange"
                  class="font-mono text-xs opacity-75 shrink-0"
                >{{ activityItemSemantic(item)!.lineRange }}</span>
                <span
                  v-if="activityItemSemantic(item)!.extra"
                  class="opacity-75 shrink-0 truncate"
                >{{ activityItemSemantic(item)!.extra }}</span>
                <span
                  v-if="activityItemStatusSuffix(item)"
                  class="opacity-75 shrink-0 font-normal"
                >{{ activityItemStatusSuffix(item) }}</span>
              </template>
              <template v-else>
                <span>{{ activityItemDisplay(item).text }}</span>
              </template>
              <span
                v-if="activityItemDisplay(item).adds > 0"
                class="ml-1 shrink-0 font-mono text-success"
              >+{{ activityItemDisplay(item).adds }}</span>
              <span
                v-if="activityItemDisplay(item).removes > 0"
                class="ml-1 shrink-0 font-mono text-error"
              >-{{ activityItemDisplay(item).removes }}</span>
            </span>
          </div>
          <!-- 正文条目：只露一小截，没有展开入口 -->
          <div v-else-if="item.kind === 'content'" class="flex pr-1 py-1">
            <PlainMarkdownRenderer
              class="min-w-0 flex-1 whitespace-pre-wrap wrap-break-word text-xs leading-relaxed"
              :text="activityItemContentText(item)"
            />
          </div>
        </div>
      </template>
    </li>
  </TransitionGroup>
</template>

<script setup lang="ts">
// 思考与工具时间线：消息内与委托卡共用同一套渲染与语义。
//
// 只负责「怎么画一条条活动」，不负责顶层折叠面板的开合：
// 面板的 details 元素、summary 文案与展开态由调用方持有，
// 这样消息里的气泡头与委托卡里的「执行过程」标题各自保留自己的外壳。
import { onBeforeUnmount, reactive } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import AppLoading from "../../shared/components/AppLoading.vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { ChatActivityItem } from "../../../types/app";
import { textContentSignature } from "../utils/text-signature";
import { createToolCallPresentation } from "../utils/tool-call-presentation";
import { parseToolCallResultStatus } from "../utils/toolcall-preview";
import { parseToolArgs } from "../utils/tool-args-tree";
// 工具详情卡片底座：避免使用 backdrop-blur，防止在滚动区内触发大面积 GPU 离屏模糊着色
const TOOL_CARD_SURFACE = "border border-base-300 bg-base-100/70";
import InlineMarkdownText from "../markdown/InlineMarkdownText.vue";
import PlainMarkdownRenderer from "./PlainMarkdownRenderer.vue";
import ToolArgsView from "./ToolArgsView.vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";

const props = defineProps<{
  /** 父级算好的活动条目序列，本组件不参与派生 */
  items: ChatActivityItem[];
  dark: boolean;
  /**
   * 由调用方持有的条目级状态。面板收起会卸载本组件，
   * 状态放在外面才能保住「收起再展开，条目还在原来的展开态」。
   */
  state: ActivityTimelineState;
  /** 选择模式下标题行不响应点击，避免与消息选择冲突 */
  selectionMode?: boolean;
  /** 工具结果为占位文案时，按需取回真实内容的上下文；缺任一项则不提供该能力 */
  conversationId?: string;
  messageId?: string;
  /** 语义标题里的人格显示名；缺省时直接用人格 id */
  agentNameMap?: Record<string, string>;
}>();

/** 条目级状态：由调用方持有并传入，面板收起重建后仍保留。 */
type ActivityTimelineState = {
  expandedOverrides: Record<string, boolean>;
  toolResultOverrides: Record<string, string>;
  toolResultLoading: Record<string, boolean>;
  toolResultErrors: Record<string, string>;
};

const emit = defineEmits<{
  (e: "itemExpanded", payload: { key: string; expanded: boolean }): void;
  (e: "itemCollapsed", payload: { key: string }): void;
}>();

const { t } = useI18n();
const { toolCallSemanticPresentation, toolCallSummaryText } = createToolCallPresentation({
  t: (key, params) => String(t(key, params ?? {})),
  agentName: (agentId) => props.agentNameMap?.[agentId] || agentId,
});

// ==================== 条目展开态 ====================

function activityItemKey(item: ChatActivityItem): string {
  return `${item.kind}:${String(item.id || "")}`;
}

function activityItemExpanded(item: ChatActivityItem): boolean {
  return props.state.expandedOverrides[activityItemKey(item)] === true;
}

/**
 * 思维链正文是常驻滚动区，没有展开态。上下边缘渐隐跟着滚动位置走：
 * 在顶部只淡底边、在底部只淡顶边、在中间两边都淡、内容不够高时两边都不淡。
 * 是否可滚 CSS 判断不了（mask 按盒子算，不看内容多少），只能实测。
 */
const reasoningFade = reactive<Record<string, { top: boolean; bottom: boolean }>>({});
const reasoningBodyElements = new Map<string, HTMLElement>();
const reasoningScrollerKeys = new Map<HTMLElement, string>();
let reasoningOverflowObserver: ResizeObserver | null = null;

function reasoningFadeClass(item: ChatActivityItem): string {
  const state = reasoningFade[activityItemKey(item)];
  if (!state) return "";
  if (state.top && state.bottom) return "ecall-reasoning-fade-both";
  if (state.top) return "ecall-reasoning-fade-top";
  if (state.bottom) return "ecall-reasoning-fade-bottom";
  return "";
}

function measureReasoningFade(key: string, scroller: HTMLElement): void {
  // 贴边判定留 1px 容差：亚像素高度会让 scrollTop 永远差一点点到不了端点
  const atTop = scroller.scrollTop <= 0;
  const atBottom = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 1;
  const next = { top: !atTop, bottom: !atBottom };
  const prev = reasoningFade[key];
  // 不溢出时两者都是 false，正好等于「不挂任何渐隐」
  if (prev && prev.top === next.top && prev.bottom === next.bottom) return;
  reasoningFade[key] = next;
}

/** 内容壳的 ref 绑定：登记 scroller 供滚动监听反查，尺寸变化时重算边缘状态。 */
function bindReasoningBody(item: ChatActivityItem, element: unknown): void {
  const key = activityItemKey(item);
  const body = element instanceof HTMLElement ? element : null;
  const known = reasoningBodyElements.get(key);
  if (!body) {
    if (known) {
      reasoningOverflowObserver?.unobserve(known);
      const scroller = known.parentElement;
      if (scroller) reasoningScrollerKeys.delete(scroller);
      reasoningBodyElements.delete(key);
    }
    if (reasoningFade[key]) delete reasoningFade[key];
    return;
  }
  // ref 每次更新都会重调，同一个元素不重复登记
  if (known === body) return;
  if (known) reasoningOverflowObserver?.unobserve(known);
  reasoningBodyElements.set(key, body);
  const scroller = body.parentElement;
  if (scroller) {
    reasoningScrollerKeys.set(scroller, key);
    measureReasoningFade(key, scroller);
  }
  if (typeof ResizeObserver !== "undefined") {
    reasoningOverflowObserver ??= new ResizeObserver((entries) => {
      for (const entry of entries) {
        const target = entry.target as HTMLElement;
        for (const [itemKey, el] of reasoningBodyElements) {
          if (el !== target) continue;
          const owner = el.parentElement;
          if (owner) measureReasoningFade(itemKey, owner);
          break;
        }
      }
    });
    reasoningOverflowObserver.observe(body);
  }
}

/** 滚动监听挂在列表根节点上（capture 阶段能接住内部 scroller 的 scroll），只挂一个。 */
function onTimelineScroll(event: Event): void {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  const key = reasoningScrollerKeys.get(target);
  if (!key) return;
  measureReasoningFade(key, target);
}

onBeforeUnmount(() => {
  reasoningOverflowObserver?.disconnect();
  reasoningOverflowObserver = null;
  reasoningBodyElements.clear();
  reasoningScrollerKeys.clear();
});

function applyExpandedChange(item: ChatActivityItem, expanded: boolean): void {
  const key = activityItemKey(item);
  if (expanded) {
    // 条目之间互不干扰：同一条消息里可以同时展开思维链和工具详情
    props.state.expandedOverrides = { ...props.state.expandedOverrides, [key]: true };
    emit("itemExpanded", { key, expanded: true });
    return;
  }
  props.state.expandedOverrides = {
    ...props.state.expandedOverrides,
    [key]: false,
  };
  // 收起后内容变矮，需要通知外层重测：外层不监听展开态。
  emit("itemCollapsed", { key });
}

// ==================== 点击与展开入口 ====================

function onToolDetailsToggle(item: ChatActivityItem, event: Event): void {
  const details = event.target instanceof HTMLDetailsElement
    ? event.target
    : (event.currentTarget instanceof HTMLDetailsElement ? event.currentTarget : null);
  const open = details ? details.open : !activityItemExpanded(item);
  applyExpandedChange(item, open);
}

// ==================== 条目文本 ====================

function activityItemText(item: ChatActivityItem): string {
  if (item.kind === "content") return stripToolcallMarkers(item.text);
  if (item.kind === "reasoning") return String(item.text || "");
  return "";
}

function stripToolcallMarkers(text: string): string {
  return String(text || "").replace(/\[toolcall:[^\]\n]+\]/g, "");
}

/** 思维链滚动区的高度行数（与 .ecall-reasoning-scroll-limit 的 max-height 一致）。 */
const REASONING_SCROLL_LINES = 30;

/**
 * 流式中的思维链走纯文本：文本每帧都在变，每帧重解析不值得。
 * 与展开态无关——思维链已经是常驻滚动区，没有「展开」这个状态了。
 */
function activityItemPlainBody(item: ChatActivityItem): boolean {
  return item.kind === "reasoning" && item.running === true;
}

/** 长纯文本按行切块，单块约 2000 字。块数按内容缓存，流式中每次只比对源文本。 */
const PLAIN_CHUNK_TARGET_CHARS = 2000;
const plainChunkCache = new WeakMap<object, { source: string; chunks: { index: number; text: string }[] }>();

function activityItemPlainChunks(item: ChatActivityItem): { index: number; text: string }[] {
  const source = activityItemText(item);
  const cached = plainChunkCache.get(item);
  if (cached && cached.source === source) return cached.chunks;
  const chunks: { index: number; text: string }[] = [];
  let buffer = "";
  for (const line of source.split("\n")) {
    if (buffer && buffer.length + line.length + 1 > PLAIN_CHUNK_TARGET_CHARS) {
      chunks.push({ index: chunks.length, text: buffer });
      buffer = "";
    }
    buffer = buffer ? `${buffer}\n${line}` : line;
  }
  if (buffer) chunks.push({ index: chunks.length, text: buffer });
  plainChunkCache.set(item, { source, chunks });
  return chunks;
}

/** 生长中的正文只看长度和末尾字符，避免每个新字都扫描全文。 */
function liveActivityTextSignature(text: string): string {
  const length = text.length;
  return `${length}:${length > 0 ? text.charCodeAt(length - 1) : 0}`;
}

/** 已闭合条目只保留身份。正在生长的条目用常量级签名，不哈希全文。 */
function activityItemMemo(item: ChatActivityItem, isLast: boolean): unknown[] {
  const expanded = activityItemExpanded(item);
  // 思维链上下渐隐由实测滚动位置决定，必须进依赖，否则量出结果也不会重渲染。
  // 编码成两位串：只有 bottom 变 both 时布尔值没变，但类名变了。
  const fade = item.kind === "reasoning" ? reasoningFade[activityItemKey(item)] : undefined;
  const fadeKey = fade ? `${fade.top ? 1 : 0}${fade.bottom ? 1 : 0}` : "";
  // isLast 必须进依赖：末条追加新条目后会被翻转成非末条，要补上竖线与末尾留白行
  if (item.kind === "tool") {
    return [activityItemKey(item), item.status || "", textContentSignature(item.argsText), textContentSignature(item.resultText), isLast];
  }
  if (!item.running) return [activityItemKey(item), expanded, fadeKey, isLast];
  return [activityItemKey(item), expanded, fadeKey, liveActivityTextSignature(activityItemText(item)), isLast];
}

/** 条目有没有正文可显示。思维链只要有内容就显示，工具条目看参数或结果在不在。 */
function activityItemHasBody(item: ChatActivityItem): boolean {
  if (item.kind === "reasoning") return !!activityItemText(item).trim();
  if (item.kind === "tool") {
    return !!toolDetailParts(item).args
      || !!String(item.resultText || "").trim()
      || item.contentOmitted === true;
  }
  return false;
}

function activityItemCanExpand(item: ChatActivityItem): boolean {
  if (!activityItemHasBody(item)) return false;
  // 工具条目有正文就能展开：参数或结果总有一块可看
  if (item.kind === "tool") return true;
  // 思维链是常驻滚动区，没有展开入口；正文条目也只露头一小截，同样不给
  return false;
}

/** 正文条目在折叠块里只露出的字数。不提供展开，超出部分就写省略号。 */
const ACTIVITY_CONTENT_PREVIEW_CHARS = 40;

/** 正文条目的渲染文本：固定字数收尾，不随任何展开状态变化。 */
function activityItemContentText(item: ChatActivityItem): string {
  const text = activityItemText(item);
  if (text.length <= ACTIVITY_CONTENT_PREVIEW_CHARS) return text;
  return `${text.slice(0, ACTIVITY_CONTENT_PREVIEW_CHARS)}…`;
}

/** 条目容器（li）的颜色：图标用 currentColor，颜色挂在容器上，图标与文字一起继承。 */
function activityItemNodeClass(item: ChatActivityItem): string {
  if (item.kind === "reasoning") {
    return props.dark ? "ecall-activity-reasoning-dark" : "text-base-content/70";
  }
  if (item.kind === "content") return "";
  if (item.kind === "tool" && item.resultText) {
    const status = parseToolCallResultStatus(item.resultText);
    if (status.isDenied) return "text-warning";
    if (status.isFailed) return "text-error";
  }
  return props.dark ? "ecall-activity-tool-dark" : "text-base-content/56";
}

/** 标题文字只补状态色，浅色档位由容器继承，不再各写一遍。 */
function activityItemTitleClass(item: ChatActivityItem): string {
  if (item.kind === "tool" && item.resultText) {
    const status = parseToolCallResultStatus(item.resultText);
    if (status.isDenied) return "text-warning";
    if (status.isFailed) return "text-error";
  }
  return "";
}

function activityItemDetailClass(item: ChatActivityItem): string {
  if (item.kind === "reasoning") {
    return props.dark ? "ecall-activity-reasoning-dark" : "text-base-content/70";
  }
  if (item.kind === "content") return "";
  return props.dark ? "ecall-activity-tool-dark" : "text-base-content/56";
}

function activityItemStatusSuffix(item: ChatActivityItem): string {
  if (item.kind !== "tool" || !item.resultText) return "";
  const status = parseToolCallResultStatus(item.resultText);
  if (status.isDenied) return `(${t("chat.toolReview.denied") || "已拒绝"})`;
  if (status.isFailed) return `(${t("chat.toolReview.failed") || "失败"})`;
  return "";
}

function activityItemSemantic(item: ChatActivityItem) {
  if (item.kind !== "tool") return null;
  return toolCallSemanticPresentation(item);
}

function activityItemTitle(item: ChatActivityItem): string {
  // 思考块标题固定成文案：首行切分那套像素估算已经去掉，标题不再跟着正文首句变
  if (item.kind === "reasoning") return t("chat.messageItem.reasoningTitle");
  if (item.kind === "content") return activityItemText(item);
  const baseTitle = toolCallSummaryText(item);
  const suffix = activityItemStatusSuffix(item);
  return suffix ? `${baseTitle} ${suffix}` : baseTitle;
}

function toolCallDiffStats(toolCall: { name: string; argsText: string; resultText?: string }): { adds: number; removes: number } {
  if (toolCall.resultText) {
    const status = parseToolCallResultStatus(toolCall.resultText);
    if (status.isDenied || status.isFailed) {
      return { adds: 0, removes: 0 };
    }
  }
  const semantic = toolCallSemanticPresentation(toolCall);
  return {
    adds: semantic.adds || 0,
    removes: semantic.removes || 0,
  };
}

function activityItemDisplay(item: ChatActivityItem): { text: string; adds: number; removes: number } {
  if (item.kind !== "tool") {
    return { text: activityItemTitle(item), adds: 0, removes: 0 };
  }
  return {
    text: activityItemTitle(item),
    ...toolCallDiffStats(item),
  };
}

// ==================== 工具参数与结果 ====================

type ToolDetailParts = { args: string; result: string };
const EMPTY_TOOL_DETAIL_PARTS: ToolDetailParts = { args: "", result: "" };
const toolDetailPartsCache = new WeakMap<ChatActivityItem, { stamp: string; parts: ToolDetailParts }>();

// 工具结果按需加载：后端默认只下发占位文案（contentOmitted），
// 用户点「查看结果」后才把真实内容填进 state.toolResultOverrides。

function activityToolArgsText(item: ChatActivityItem): string {
  if (item.kind !== "tool") return "";
  const raw = String(item.argsText || "");
  if (!raw.trim()) return raw;
  try {
    const parsed = JSON.parse(raw);
    if (parsed !== null && parsed !== undefined && (typeof parsed === "object" || Array.isArray(parsed))) {
      return JSON.stringify(parsed, null, 2);
    }
  } catch {
    // 非 JSON 原文保留
  }
  return raw;
}

/**
 * 摘要行已经用掉 description 的工具：参数区不再重复列同一个字段。
 * 摘要看「这次读它是干什么的」，参数区只看路径这类真正要核对的输入。
 */
const SUMMARY_CONSUMES_DESCRIPTION_TOOLS = new Set(["read_media"]);
const SUMMARY_CONSUMED_ARG_KEYS = ["description", "focus", "prompt"];

function stripSummaryConsumedArgs(name: string, text: string): string {
  if (!SUMMARY_CONSUMES_DESCRIPTION_TOOLS.has(String(name || "").trim())) return text;
  const trimmed = text.trim();
  if (!trimmed.startsWith("{")) return text;
  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    return text;
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return text;
  const next: Record<string, unknown> = { ...(parsed as Record<string, unknown>) };
  let removed = false;
  for (const key of SUMMARY_CONSUMED_ARG_KEYS) {
    if (key in next) {
      delete next[key];
      removed = true;
    }
  }
  return removed ? JSON.stringify(next) : text;
}

/**
 * 参数区实际要显示的文本。
 * 空对象 / 空数组摊平后一行字段都没有，展开只会看到一对括号，按无参数处理；
 * 不能直接判原文非空——`{}` 也是两个字符。
 */
function toolArgsDisplayText(item: ChatActivityItem): string {
  if (item.kind !== "tool") return "";
  const text = stripSummaryConsumedArgs(item.name, activityToolArgsText(item));
  const trimmed = text.trim();
  if (!trimmed) return "";
  if (parseToolArgs(trimmed)) return text;
  if (trimmed === "{}" || trimmed === "[]") return "";
  return text;
}

/** 缺会话或消息定位时取不回结果，按钮不该出现。 */
function toolResultLoadable(item: ChatActivityItem): boolean {
  if (item.kind !== "tool") return false;
  if (!String(item.toolCallId || "").trim()) return false;
  return !!String(props.conversationId || "").trim() && !!String(props.messageId || "").trim();
}

async function loadToolResult(item: ChatActivityItem): Promise<void> {
  if (item.kind !== "tool") return;
  const key = activityItemKey(item);
  const conversationId = String(props.conversationId || "").trim();
  const messageId = String(props.messageId || "").trim();
  const toolCallId = String(item.toolCallId || "").trim();
  if (!conversationId || !messageId || !toolCallId) return;
  if (props.state.toolResultLoading[key]) return;
  props.state.toolResultLoading = { ...props.state.toolResultLoading, [key]: true };
  props.state.toolResultErrors = { ...props.state.toolResultErrors, [key]: "" };
  try {
    const content = await invokeTauri<string>("conversation.toolResultContent", {
      input: { conversationId, messageId, toolCallId },
    });
    props.state.toolResultOverrides = { ...props.state.toolResultOverrides, [key]: String(content || "") };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    props.state.toolResultErrors = { ...props.state.toolResultErrors, [key]: message };
  } finally {
    props.state.toolResultLoading = { ...props.state.toolResultLoading, [key]: false };
  }
}

// 参数与结果拆成两块：参数整块走工具绿、其中键名单独标红；结果保持原来的灰。
// 原来的 '\n\n---\n\n' 分隔线随之取消。
function buildToolDetailParts(item: ChatActivityItem, override: string | undefined): ToolDetailParts {
  if (item.kind !== "tool") return { args: "", result: "" };
  const args = toolArgsDisplayText(item);
  // 占位结果已被省略时，优先展示按需加载回来的真实结果；
  // 未加载时结果区由模板渲染「查看结果」按钮，这里只返回参数部分。
  if (item.contentOmitted) {
    return { args, result: override === undefined ? "" : String(override) };
  }
  const result = String(item.resultText || "").trim();
  if (!result) return { args, result: "" };
  const status = parseToolCallResultStatus(item.resultText);
  if (status?.isDenied) {
    const reason = status.blockedReason || status.message;
    const label = reason
      ? `[${t("chat.toolReview.denied") || "已拒绝"}]: ${reason}`
      : `[${t("chat.toolReview.denied") || "已拒绝"}]`;
    return { args, result: `${result}\n\n${label}` };
  }
  if (status?.isFailed) {
    const reason = status.message || status.blockedReason;
    const label = reason
      ? `[${t("chat.toolReview.failed") || "执行失败"}]: ${reason}`
      : `[${t("chat.toolReview.failed") || "执行失败"}]`;
    return { args, result: `${result}\n\n${label}` };
  }
  return { args, result };
}

/** 参数与结果都含长文本拼接和 JSON 解析，按 item 缓存；流式期间靠 stamp 比对失效。 */
function toolDetailParts(item: ChatActivityItem): ToolDetailParts {
  if (item.kind !== "tool") return EMPTY_TOOL_DETAIL_PARTS;
  const override = props.state.toolResultOverrides[activityItemKey(item)];
  const stamp = [
    String(item.argsText || ""),
    String(item.resultText || ""),
    item.contentOmitted ? "1" : "0",
    override ?? "\u0000",
  ].join("\u0001");
  const cached = toolDetailPartsCache.get(item);
  if (cached && cached.stamp === stamp) return cached.parts;
  const parts = buildToolDetailParts(item, override);
  toolDetailPartsCache.set(item, { stamp, parts });
  return parts;
}
</script>

<style scoped>
/* 条目颜色：随组件一起走，宿主（消息 / 委托）不用各自再补一套。
   浅色走 Tailwind 的 text-base-content/xx 语义类（模板里直接挂），这里只留深色主题的彩色。 */
/* 深色主题：思维链橙色、工具绿色（提亮保证深底可读）。
   颜色类挂在本组件自己的元素上，用普通选择器；写 :deep 会变成后代选择器，永远不命中。 */
.ecall-activity-reasoning-dark {
  color: #fb923c;
}

.ecall-activity-tool-dark {
  color: #4ade80;
}

.ecall-activity-result-dark {
  color: #54c5d4;
}

.ecall-activity-item-summary {
  /* 单行截断：长命令/路径用 ellipsis 收尾，不允许多行撑开标题。
     完整内容在 <details> 展开后的详情区可见。 */
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* 条目 details 原生开合：chevron 旋转由 details[open] 驱动，不进 Vue 状态。
   口径与 CollapsibleGroup / MultiFileDiffView 一致：收起朝右，展开转 90 度朝下。 */
.ecall-activity-timeline details[open] .ecall-activity-chevron {
  transform: rotate(90deg);
}

/* 思维链滚动区：高度 = 行数 × 1.5 行高，行数由 REASONING_SCROLL_LINES 注入到
   --ecall-reasoning-max-h。规则要落到 OverlayScrollArea 内部的 scroller 上，
   只能从本组件根节点往下穿透。
   font-size 显式钉住 0.75rem：em 的基准是元素自身字号，scroller 若继承到别的字号，
   calc 出来的高度就和正文壳的行高对不上。 */
:deep(.ecall-reasoning-scroll-limit) {
  font-size: 0.75rem;
  max-height: var(--ecall-reasoning-max-h);
}

/* 上下边缘渐隐跟着滚动位置走，类名由 JS 按实测的 scrollTop 挂上来。
   直接对文字做 mask，不用猜气泡底色；内容不够高时三种类都不挂，一个字都不淡。
   渐隐区约一行半，遮住滚出去的那截，中间态两头都收。 */
:deep(.ecall-reasoning-fade-bottom) {
  -webkit-mask-image: linear-gradient(to bottom, #000 calc(100% - 1.5em), transparent 100%);
  mask-image: linear-gradient(to bottom, #000 calc(100% - 1.5em), transparent 100%);
}

:deep(.ecall-reasoning-fade-top) {
  -webkit-mask-image: linear-gradient(to bottom, transparent 0, #000 1.5em);
  mask-image: linear-gradient(to bottom, transparent 0, #000 1.5em);
}

:deep(.ecall-reasoning-fade-both) {
  -webkit-mask-image: linear-gradient(
    to bottom,
    transparent 0,
    #000 1.5em,
    #000 calc(100% - 1.5em),
    transparent 100%
  );
  mask-image: linear-gradient(
    to bottom,
    transparent 0,
    #000 1.5em,
    #000 calc(100% - 1.5em),
    transparent 100%
  );
}

/* 长文本分块：屏外块跳过布局与绘制，滚动条长度用最近一次渲染尺寸估 */
.ecall-reasoning-plain-chunk {
  content-visibility: auto;
  contain-intrinsic-size: auto 1.5em;
}

/* 工具参数区滚动边缘淡出：只做 mask 渐变，底色由 Tailwind 类给，
   跟着参数区那层半透明底走，不硬塞实色 base-200 */
.ecall-activity-fade-top {
  -webkit-mask-image: linear-gradient(to bottom, #000 0%, transparent 100%);
  mask-image: linear-gradient(to bottom, #000 0%, transparent 100%);
}

.ecall-activity-fade-bottom {
  -webkit-mask-image: linear-gradient(to bottom, transparent 0%, #000 100%);
  mask-image: linear-gradient(to bottom, transparent 0%, #000 100%);
}

/* :deep 只留子组件内部那一层。不能再带 .ecall-activity-timeline：那是本组件自己的 ul，
   写成 `[data-v] .ecall-activity-timeline .x` 等于要求它在自己内部，规则永远不命中。 */
:deep(.ecall-plain-markdown-markdown > :first-child) {
  margin-top: 0 !important;
}

/* 正文条目与工具、思维链同一个字号：渲染器根节点自带的是聊天正文那档字号
   （--app-chat-message-text-size），比折叠块里其他条目大一档，这里压回 text-xs。
   必须写死 0.75rem（text-xs 的实际值），不能写 var(--app-text-xs-size)：那个变量随
   界面尺寸缩放，而块内其他文字的 text-xs 不跟缩放，两者会在非 100% 尺寸下对不上。 */
:deep(.ecall-plain-markdown-markdown) {
  font-size: 0.75rem !important;
}

/* 正文条目与思考正文同一档灰，靠 color-mix 淡文字，不连带淡掉里面的代码块底色 */
:deep(.ecall-plain-markdown-markdown),
:deep(.ecall-plain-markdown-markdown p) {
  color: color-mix(in oklab, var(--color-base-content) 56%, transparent) !important;
}
</style>
