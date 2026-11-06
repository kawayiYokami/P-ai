<template>
  <TransitionGroup
    ref="timelineRootRef"
    name="ecall-activity-item"
    tag="ul"
    class="ecall-activity-timeline space-y-2"
    :class="{ 'ecall-activity-dark': props.dark }"
    :appear="false"
  >
    <li
      v-for="(item, itemIndex) in items"
      :key="`activity-${activityItemKey(item)}`"
      v-memo="activityItemMemo(item, itemIndex === items.length - 1)"
      :data-kind="item.kind"
      class="ecall-activity-row relative grid min-w-0 grid-cols-[1rem_minmax(0,1fr)] gap-x-1.5"
    >
      <!-- 左列：图标落在标题行、竖线落在内容行。两列同一套网格行，行高一致才对得上 -->
      <div class="ecall-activity-rail-col relative flex flex-col items-center">
        <div
          class="ecall-fold-node flex h-6 shrink-0 items-center justify-center"
          :class="{
            'is-expanded': activityItemFoldOpen(item),
            'is-collapsible': activityItemFoldable(item),
          }"
        >
          <AppLoading
            v-if="item.kind === 'tool' && item.status === 'doing'"
            size="xs"
          />
          <!-- 工具之间的正文：一个对话气泡，和工具行区分开 -->
          <AppIcon
            v-else-if="item.kind === 'content'"
            name="bubble-chat"
            :stroke-width="TIMELINE_ICON_STROKE"
            class="h-4 w-4"
          />
          <AppIcon
            v-else-if="item.kind === 'reasoning'"
            name="ai-magic"
            :stroke-width="TIMELINE_ICON_STROKE"
            class="ecall-fold-default h-4 w-4"
          />
          <!-- 工具图标按工具名取：扫一眼就知道这一步在干什么，不再全部挂扳手 -->
          <AppIcon
            v-else
            :name="activityToolIconName(item)"
            :stroke-width="TIMELINE_ICON_STROKE"
            class="ecall-fold-default h-4 w-4"
          />
          <AppIcon name="chevron-right" :stroke-width="TIMELINE_ICON_STROKE" class="ecall-fold-hover h-4 w-4" />
          <AppIcon name="chevron-down" :stroke-width="TIMELINE_ICON_STROKE" class="ecall-fold-open h-4 w-4" />
        </div>
        <!-- 竖线：从图标下沿（1.5rem）画到条目底部，负 bottom 吃掉 ul 的 0.5rem 间距 -->
        <span
          v-if="itemIndex !== items.length - 1"
          class="ecall-activity-rail absolute w-px bg-current opacity-30"
        />
      </div>

      <!-- 标题行（思考）：固定「思考」二字，点它开合 -->
      <div
        v-if="item.kind === 'reasoning'"
        class="ecall-activity-title ecall-reasoning-head flex min-h-6 cursor-pointer items-center gap-1.5 pr-1"
        data-selection-ignore="true"
        :class="{ 'ecall-reasoning-stuck': reasoningCollapseOpen(item) }"
        role="button"
        tabindex="0"
        :aria-expanded="reasoningCollapseOpen(item)"
        @click="handleReasoningToggle(item, $event)"
        @keydown.enter.prevent="handleReasoningToggle(item, $event)"
        @keydown.space.prevent="handleReasoningToggle(item, $event)"
      >{{ activityItemTitle(item) }}</div>

      <!-- 内容行（思考）：收起裁四行，展开后全文，不设滚动区 -->
      <div
        v-if="item.kind === 'reasoning' && activityItemHasBody(item)"
        class="ecall-activity-body ecall-reasoning-text min-w-0 pt-2"
        :class="{ 'is-open': reasoningCollapseOpen(item) }"
      >
        <InlineMarkdownText :text="activityItemText(item)" />
      </div>

      <!-- 标题行（工具）：可展开时整行可点，其余情况只作展示 -->
      <div
        v-if="item.kind === 'tool'"
        class="ecall-activity-title flex min-h-6 items-center gap-1.5 pr-1"
        :class="[{ 'cursor-pointer hover:bg-base-200': activityItemCanExpand(item) }, activityItemTitleClass(item)]"
        data-selection-ignore="true"
        :role="activityItemCanExpand(item) ? 'button' : undefined"
        :tabindex="activityItemCanExpand(item) ? 0 : undefined"
        :aria-expanded="activityItemCanExpand(item) ? activityItemExpanded(item) : undefined"
        @click="activityItemCanExpand(item) && toggleToolItem(item)"
        @keydown.enter.prevent="activityItemCanExpand(item) && toggleToolItem(item)"
        @keydown.space.prevent="activityItemCanExpand(item) && toggleToolItem(item)"
      >
              <span
                class="ecall-activity-item-summary min-w-0 inline-flex items-center gap-1.5 overflow-hidden text-ellipsis whitespace-nowrap"
              >
                <template v-if="activityItemSemantic(item)">
                  <span class="font-medium shrink-0 ecall-activity-action">{{ activityItemSemantic(item)!.action }}</span>
                  <span class="opacity-30 shrink-0">·</span>
                  <span class="truncate">{{ activityItemSemantic(item)!.summary }}</span>
                  <span
                    v-if="activityItemSemantic(item)!.lineRange"
                    class="font-mono shrink-0"
                  >{{ activityItemSemantic(item)!.lineRange }}</span>
                  <span
                    v-if="activityItemSemantic(item)!.extra"
                    class="shrink-0 truncate"
                  >{{ activityItemSemantic(item)!.extra }}</span>
                  <span
                    v-if="activityItemStatusSuffix(item)"
                    class="shrink-0 font-normal"
                  >{{ activityItemStatusSuffix(item) }}</span>
                </template>
                <template v-else>
                  <span class="ecall-activity-action">{{ activityItemDisplay(item).text }}</span>
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

      <!-- 内容行（工具）：外壳常驻，高度 0 ↔ auto 交给 CSS 关键字插值，内层 v-if 懒渲染。
           不用 JS 量高：内层由 v-if 插入，Vue 的 DOM 更新在微任务里，同期读 scrollHeight
           必定是 0，要量就得补一次 nextTick，不值当。不支持关键字插值的内核退化成硬跳。
           pt-2 必须放在 overflow 容器里层：留在外壳上会让收起末帧残留 8px 再跳 0。 -->
      <div
        v-if="item.kind === 'tool' && activityItemCanExpand(item)"
        class="ecall-activity-body ecall-tool-body min-w-0 pr-1"
        :class="{ 'is-open': activityItemExpanded(item) }"
        @transitionend="handleToolFoldEnd(item, $event)"
      >
        <div v-if="activityItemExpanded(item) || toolFoldClosing[activityItemKey(item)]" class="relative pt-2">
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
                        class="px-2.5 py-2 font-mono leading-relaxed"
                      />
                    </div>

                    <!-- 卡片二：结果（输出）。默认空态，点一下才把结果取回来 -->
                    <div
                      v-if="toolDetailParts(item).result || (item.contentOmitted && props.state.toolResultOverrides[activityItemKey(item)] === undefined)"
                      :class="['rounded-lg', TOOL_CARD_SURFACE]"
                    >
                      <!-- 结果区：内容色由条目变量给，深色是青 -->
                      <pre
                        v-if="toolDetailParts(item).result"
                        class="m-0 whitespace-pre-wrap break-all px-2.5 py-2 leading-relaxed"
                      ><code>{{ toolDetailParts(item).result }}</code></pre>
                      <div v-else class="px-2.5 py-2">
                        <button
                          v-if="toolResultLoadable(item)"
                          type="button"
                          class="btn btn-ghost btn-xs -ml-1 h-6 min-h-0 justify-start gap-1.5 px-1 font-normal ecall-activity-quiet hover:bg-base-300/60"
                          :disabled="!!props.state.toolResultLoading[activityItemKey(item)]"
                          @click.stop="loadToolResult(item)"
                        >
                          <AppLoading
                            v-if="props.state.toolResultLoading[activityItemKey(item)]"
                            size="xs"
                          />
                          <AppIcon name="chevron-right" v-else :stroke-width="TIMELINE_ICON_STROKE" class="h-3 w-3 shrink-0" />
                          {{ t("chat.toolReview.viewResult") || "查看结果" }}
                        </button>
                        <!-- 缺定位上下文时不给按钮：点了也取不回，只留一句说明 -->
                        <span v-else class="ecall-activity-quiet">
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

      <!-- 标题行（正文）：只露一小截预览，没有内容行 -->
      <div v-if="item.kind === 'content'" class="ecall-activity-title flex min-h-6 items-center pr-1">
        <div class="min-w-0 flex-1 whitespace-pre-wrap wrap-break-word leading-relaxed">
          <PlainMarkdownRenderer :text="activityItemContentText(item)" />
        </div>
      </div>
    </li>
  </TransitionGroup>
</template>

<script setup lang="ts">
// 思考与工具时间线：消息内与委托卡共用同一套渲染与语义。
//
// 只负责「怎么画一条条活动」，不负责顶层折叠面板的开合：
// 面板的 details 元素、summary 文案与展开态由调用方持有，
// 这样消息里的气泡头与委托卡里的「执行过程」标题各自保留自己的外壳。
import { onBeforeUnmount, onMounted, onUpdated, reactive, ref } from "vue";
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
// 时间轴图标的线宽：默认 2 在 16px 下偏粗，压到 1.5 后笔画与 12px 文字齐平
const TIMELINE_ICON_STROKE = 1.5;
import InlineMarkdownText from "../markdown/InlineMarkdownText.vue";
import type { AppIconName } from "../../shared/icons/icon-map";
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

// ==================== 思维链折叠 ====================

// 折叠的开合是纯展示态，不进外部 state：它只决定这一条思考露 4 行还是全文，
// 与工具条目的展开态无关。
const reasoningCollapseOverrides = reactive<Record<string, boolean>>({});

function reasoningCollapseOpen(item: ChatActivityItem): boolean {
  const override = reasoningCollapseOverrides[activityItemKey(item)];
  if (typeof override === "boolean") return override;
  // 没有记录 = 这一条是本次流式新长出来的，直接全文可见；新内容长出来也是展开的
  return true;
}

/** 收起档露四行：把「四行 + pt-2」算成像素，只用来判断这条思考有没有得展开、
 *  要不要套收起高度与底部淡出，不当过渡终点（终点一律实测）。 */
function reasoningCollapsedLimit(el: HTMLElement): number {
  const style = getComputedStyle(el);
  const lineHeight = Number.parseFloat(style.lineHeight) || 21;
  const paddingTop = Number.parseFloat(style.paddingTop) || 0;
  return lineHeight * 4 + paddingTop;
}

/**
 * 四行装得下就不切换收起态。
 *
 * 展开态会挂上吸附、换底色，这些在短思考上是看得见的副作用，所以不能只靠「没箭头
 * 就别点」来拦。判据是正文比收起档高多少，与当前收起还是展开无关，读一次不挂监听。
 */
function handleReasoningToggle(item: ChatActivityItem, event?: Event): void {
  const head = event?.currentTarget;
  if (!(head instanceof HTMLElement)) return;
  const body = head.closest(".ecall-activity-row")?.querySelector<HTMLElement>(".ecall-reasoning-text");
  if (!body) return;
  if (body.scrollHeight <= reasoningCollapsedLimit(body) + 1) return;
  const open = reasoningCollapseOpen(item);
  // class 与响应式状态一起改：class 直接落在 DOM 上，收起方向才量得到真实的收起高度
  const apply = () => {
    body.classList.toggle("is-open", !open);
    reasoningCollapseOverrides[activityItemKey(item)] = !open;
  };
  if (open) collapseFold(body, apply);
  else expandFold(body, apply);
}

// ===== 折叠过渡 =====
// 只做两件事：把当前高度钉成像素，再过渡到布局给出的目标高度。读一次像素不是监听：
// 过渡期间不再读，也不挂 observer。终点一律实测，不按「行高 × 4」自己算——
// 段落间距会被算漏，收起落定那一帧就会弹开（实测差 26px）。
const FOLD_MS = 200; // 与 src/style.css 的 --duration-fold 同值

/** 过渡结束清内联高度，交回布局。只认 transitionend，不用定时器。 */
function settleHeight(el: HTMLElement): void {
  el.addEventListener("transitionend", () => {
    el.style.transition = "";
    el.style.maxHeight = "";
    el.style.height = "";
  }, { once: true });
}

/** 收起 → 展开：钉住当前高，读全文高，切到展开态，再过渡过去。 */
function expandFold(el: HTMLElement, applyOpen: () => void): void {
  const start = el.getBoundingClientRect().height;
  el.style.height = `${start}px`;
  const target = el.scrollHeight;
  applyOpen();
  // 高度没变就没有过渡可等，内联高度也别留着
  if (Math.abs(target - start) < 1) {
    el.style.height = "";
    return;
  }
  el.style.transition = `height ${FOLD_MS}ms ease`;
  void el.offsetHeight;
  el.style.height = `${target}px`;
  settleHeight(el);
}

/**
 * 展开 → 收起：钉住当前高，过渡到收起档。
 *
 * 收起档就是样式里那条 max-height 的数值，两边同一个算法，落定值与过渡终点一致，不会弹。
 * 但上限会把起点（当前全文高）压扁成四行，过渡就没起点——过渡期间先摘掉它，落定再交回去。
 */
function collapseFold(el: HTMLElement, applyClosed: () => void): void {
  const current = el.getBoundingClientRect().height;
  // 短思考的自然高比收起档还矮，取小的那个，落定后不会被撑到四行
  const target = Math.min(reasoningCollapsedLimit(el), el.scrollHeight);
  if (Math.abs(target - current) < 1) {
    applyClosed();
    return;
  }
  el.style.height = `${current}px`;
  el.style.maxHeight = "none";
  applyClosed();
  el.style.transition = `height ${FOLD_MS}ms ease`;
  void el.offsetHeight;
  el.style.height = `${target}px`;
  settleHeight(el);
}

// ===== 收起态底部淡出 =====
// 只在正文比收起档高时才淡出：短思考如果也加遮罩，最后两行会整片变透明。
// 用 data 属性而不是 class 绑定，Vue 不管理它，避免每帧比对 class 字符串。
const timelineRootRef = ref<unknown>(null);

/** TransitionGroup 的 ref 传回来可能是元素、组件实例或 Fragment 数组，取到真正的 DOM 为止。 */
function resolveTimelineRoot(): HTMLElement | null {
  const raw = Array.isArray(timelineRootRef.value) ? timelineRootRef.value[0] : timelineRootRef.value;
  if (raw instanceof HTMLElement) return raw;
  const inner = (raw as { $el?: unknown } | null | undefined)?.$el;
  if (inner instanceof HTMLElement) return inner;
  if (Array.isArray(inner)) {
    for (const node of inner) if (node instanceof HTMLElement) return node;
  }
  return null;
}

function syncReasoningFade(): void {
  const root = resolveTimelineRoot();
  if (!root || typeof root.querySelectorAll !== "function") return;
  root.querySelectorAll<HTMLElement>(".ecall-reasoning-text").forEach((el) => {
    // 和收起档比，不和当前盒子比：展开态盒子本来就装得下，按盒子比会把这标记清掉。
    if (el.scrollHeight > reasoningCollapsedLimit(el) + 1) el.dataset.faded = "1";
    else delete el.dataset.faded;
  });
}

// 更新可能每帧都来（流式），用 rAF 合并，避免每次 patch 都强制一次布局
let fadeRaf = 0;

/** 挪到下一帧再量：挂载那一帧布局还没落定，量出来的高度不作数。 */
function scheduleSyncFade(): void {
  if (fadeRaf) return;
  fadeRaf = requestAnimationFrame(() => {
    fadeRaf = 0;
    syncReasoningFade();
  });
}

onMounted(() => {
  // 容器每次重新打开都会重新挂载本组件：此刻还在列表里、且已经跑完的思考是上一轮留下的，
  // 先收成四行。还在流式的那条不收，之后新长出来的条目也不在这个循环里——
  // 这两者保持展开，新长出来的内容直接可见。
  for (const item of props.items) {
    if (item.kind === "reasoning" && item.running !== true) {
      reasoningCollapseOverrides[activityItemKey(item)] = false;
    }
  }
  scheduleSyncFade();
});

onUpdated(scheduleSyncFade);
onBeforeUnmount(() => {
  if (fadeRaf) cancelAnimationFrame(fadeRaf);
});

// ==================== 条目展开态 ====================

function activityItemKey(item: ChatActivityItem): string {
  return `${item.kind}:${String(item.id || "")}`;
}

function activityItemExpanded(item: ChatActivityItem): boolean {
  return props.state.expandedOverrides[activityItemKey(item)] === true;
}

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

/** 这一条能不能折叠：思考行与有正文的工具行可以，正文行不行。 */
function activityItemFoldable(item: ChatActivityItem): boolean {
  if (item.kind === "reasoning") return true;
  if (item.kind === "tool") return item.status !== "doing" && activityItemCanExpand(item);
  return false;
}

/** 折叠节点当前是否展开，左列图标显示哪个箭头跟着它走。 */
function activityItemFoldOpen(item: ChatActivityItem): boolean {
  if (item.kind === "reasoning") return reasoningCollapseOpen(item);
  if (item.kind === "tool") return activityItemExpanded(item);
  return false;
}

/**
 * 工具详情收起时先留着内层内容：v-if 和 class 在同一帧变化，内容一撤，外壳的
 * auto 起始高度当场就是 0，过渡就没有起点了。等高度过渡结束（transitionend）再卸载。
 */
const toolFoldClosing = reactive<Record<string, boolean>>({});

function toggleToolItem(item: ChatActivityItem): void {
  const key = activityItemKey(item);
  if (!activityItemExpanded(item)) {
    delete toolFoldClosing[key];
    applyExpandedChange(item, true);
    return;
  }
  toolFoldClosing[key] = true;
  applyExpandedChange(item, false);
}

function handleToolFoldEnd(item: ChatActivityItem, event: TransitionEvent): void {
  // 只认外壳自己的 height 过渡：内层按钮的 hover 过渡也会冒泡上来
  if (event.target !== event.currentTarget || event.propertyName !== "height") return;
  delete toolFoldClosing[activityItemKey(item)];
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


/** 生长中的正文只看长度和末尾字符，避免每个新字都扫描全文。 */
function liveActivityTextSignature(text: string): string {
  const length = text.length;
  return `${length}:${length > 0 ? text.charCodeAt(length - 1) : 0}`;
}

/** 已闭合条目只保留身份。正在生长的条目用常量级签名，不哈希全文。 */
function activityItemMemo(item: ChatActivityItem, isLast: boolean): unknown[] {
  const expanded = activityItemExpanded(item);
  // isLast 必须进依赖：末条追加新条目后会被翻转成非末条，要补上竖线与末尾留白行
  if (item.kind === "tool") {
    // expanded 必须进依赖：它决定内容行渲不渲染、左列图标换成上还是下折箭头。
    // 漏了它，点开之后子树命中旧缓存，箭头停在默认图标上，只有 hover 的 CSS 还能动。
    // closing 也要进：收起过渡结束后靠它把内层内容撤掉，不进就永远撤不掉。
    const foldState = expanded ? (toolFoldClosing[activityItemKey(item)] ? "closing" : "open") : "closed";
    return [activityItemKey(item), item.status || "", foldState, textContentSignature(item.argsText), textContentSignature(item.resultText), isLast];
  }
  // 开合也要进依赖：它决定这一条思考露四行还是全文，不进依赖会停在旧形态。
  const collapseOpenKey = item.kind === "reasoning" ? (reasoningCollapseOpen(item) ? "1" : "0") : "";
  if (!item.running) return [activityItemKey(item), expanded, collapseOpenKey, isLast];
  return [activityItemKey(item), expanded, collapseOpenKey, liveActivityTextSignature(activityItemText(item)), isLast];
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
  // 思维链走自己的折叠入口，不从这里开；正文条目只露头一小截，同样不给
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

/** 标题行只补状态色：常态颜色由 data-kind 映射给的变量决定，这里不写普通档位。 */
function activityItemTitleClass(item: ChatActivityItem): string {
  if (item.kind === "tool" && item.resultText) {
    const status = parseToolCallResultStatus(item.resultText);
    if (status.isDenied) return "ecall-activity-state-warning";
    if (status.isFailed) return "ecall-activity-state-error";
  }
  return "";
}

/**
 * 工具名 → 图标。同一类共用同一个图标：扫一眼知道这一步在干什么就够，花样多了反而看不出类别。
 * 只按小写全名精确匹配；未命中（第三方 / MCP 工具名带命名空间前缀）统一挂网络图标。
 */
const TOOL_ICON_BY_NAME: Record<string, AppIconName> = {
  // 读文件与目录
  read: "file-text",
  read_file: "file-text",
  read_dir: "folder-open",
  list_dir: "folder-open",
  find: "search",
  // 写改与文件操作
  write: "pencil-line",
  write_file: "pencil-line",
  create_file: "pencil-line",
  append_text: "pencil-line",
  update: "pencil-line",
  apply_patch: "file-diff",
  move: "arrow-right-left",
  move_file: "arrow-right-left",
  rename_file: "arrow-right-left",
  delete: "trash",
  delete_file: "trash",
  // 命令与后台进程
  exec: "square-terminal",
  shell_exec: "square-terminal",
  background: "play",
  // 网络与抓取
  fetch: "cloud",
  websearch: "search",
  // 记忆与回忆：与思考节点同一族，都用气泡
  remember: "thought-bubble",
  recall: "thought-bubble",
  deeprecall: "thought-bubble",
  deeprecall_search: "thought-bubble",
  deeprecall_context: "thought-bubble",
  memory: "thought-bubble",
  // 通讯：委托、会话、队伍、联系人
  delegate: "users",
  get_session: "message-square",
  inform_session: "message-square",
  contact_reply: "send",
  contact_no_reply: "send",
  contact_send_files: "send",
  team_info: "users",
  team_read: "users",
  team_recruit: "users",
  team_send: "users",
  team_report: "users",
  team_share_model: "users",
  // 计划、待办与看板
  plan: "clipboard-list",
  todo: "list-todo",
  kanban: "layout-grid",
  kanban_set_plan: "layout-grid",
  create_goal: "target",
  update_goal: "target",
  task: "timer",
  wait: "clock",
  // 配置
  config: "settings",
  app_config: "settings",
  pai_config: "settings",
  configure_pai: "settings",
  // 图像与媒体
  image_generate: "image",
  image_edit: "image",
  image_to_video: "image",
  read_media: "image",
  // 交互
  question: "help-circle",
  operate: "keyboard",
  windows: "app-window",
  meme: "image",
  // 工具目录
  get_tool: "puzzle",
};

/** 未命中的第三方 / MCP 工具统一挂网络图标：表示「接了个外部能力」，不去猜它具体干什么。 */
const FALLBACK_TOOL_ICON: AppIconName = "network";

function activityToolIconName(item: ChatActivityItem): AppIconName {
  if (item.kind !== "tool") return FALLBACK_TOOL_ICON;
  return TOOL_ICON_BY_NAME[String(item.name || "").trim().toLowerCase()] || FALLBACK_TOOL_ICON;
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
/* ===== 块内字号：跟随消息正文字号档位 =====
   折叠块里原来是 8 处 text-xs（写死 0.75rem），用户把消息字号调大时整块不跟。
   这里在根上定一档，比正文小一档、跟着正文走（6/7 = 12/14，默认档正好 12px，
   与改前的视觉一致），块内所有文字继承它，不再各处单独写档位。 */
.ecall-activity-timeline {
  font-size: calc(var(--app-chat-message-text-size, 0.875rem) * 6 / 7);
}

/* ===== 条目配色：一张表，两套皮肤 =====
   三种条目 × 两个位置 = 6 个色值，别处不再单独写颜色：
     UI  = 图标、竖线、动作名（工具名 / 「思考」）
     内容 = 摘要行（summary / 行号 / extra）、说明文字、参数、结果、思维链正文
   条目类型只作为数据写在 li 的 data-kind 上，颜色全部在这里按类型映射，
   元素自己只引用 var(--ecall-ui) / var(--ecall-body)，不各自压档。 */
.ecall-activity-row {
  --ecall-ui: color-mix(in oklab, var(--color-base-content) 80%, transparent);
  --ecall-body: color-mix(in oklab, var(--color-base-content) 65%, transparent);
}

/* 深色：正文保持中性灰，思考走橙，工具 UI 绿 / 内容青 */
.ecall-activity-dark .ecall-activity-row {
  --ecall-ui: color-mix(in oklab, var(--color-base-content) 80%, transparent);
  --ecall-body: color-mix(in oklab, var(--color-base-content) 56%, transparent);
}

.ecall-activity-dark .ecall-activity-row[data-kind="reasoning"] {
  --ecall-ui: #fb923c;
  --ecall-body: #fb923c;
}

.ecall-activity-dark .ecall-activity-row[data-kind="tool"] {
  --ecall-ui: #4ade80;
  --ecall-body: #54c5d4;
}

/* 图标与竖线：UI 色。竖线再叠 opacity-30 做细线 */
.ecall-fold-node,
.ecall-activity-rail {
  color: var(--ecall-ui);
}

/* 标题行：默认走内容色（摘要文字在这一行），只有动作名单独用 UI 色 */
.ecall-activity-title {
  color: var(--ecall-body);
}

/* 动作名：条目的 UI 色（工具名 / 「思考」） */
.ecall-activity-action,
.ecall-activity-row[data-kind="reasoning"] .ecall-activity-title {
  color: var(--ecall-ui);
}

/* 失败 / 拒绝压过类型色：动作名也要跟着变，否则一行里绿字配红字 */
.ecall-activity-title.ecall-activity-state-error,
.ecall-activity-title.ecall-activity-state-error .ecall-activity-action {
  color: var(--color-error);
}

.ecall-activity-title.ecall-activity-state-warning,
.ecall-activity-title.ecall-activity-state-warning .ecall-activity-action {
  color: var(--color-warning);
}

/* 内容行：内容色 */
.ecall-activity-body,
.ecall-activity-quiet {
  color: var(--ecall-body);
}

/* 「查看结果」那个按钮带 btn-xs，会把字号又钉死回 0.75rem，
   块内统一档位要在这里拉回来；同一类用在纯文本上时 inherit 本来就是原值，不会有多余影响。 */
.ecall-activity-quiet {
  font-size: inherit;
}

/* 工具图标不套圆底，图标裸显、只取前景色的淡档：整行不染色，一屏工具才不会花成一片。 */

/* 折叠节点图标：所有可折叠条目共用一套表现——平时显示自己的图标，
   整行悬停换成右折箭头提示可展开，展开后换成下折箭头提示可收起。
   三个图标叠在同一格内，只显示当前那一个，不额外占位。
   思维链节点与工具条目都挂这张表，只认 .is-collapsible 标记。 */
.ecall-fold-hover,
.ecall-fold-open {
  display: none;
}
.ecall-activity-row:hover .ecall-fold-node.is-collapsible:not(.is-expanded) .ecall-fold-default {
  display: none;
}
.ecall-activity-row:hover .ecall-fold-node.is-collapsible:not(.is-expanded) .ecall-fold-hover {
  display: inline;
}
.ecall-fold-node.is-collapsible.is-expanded .ecall-fold-default,
.ecall-fold-node.is-collapsible.is-expanded .ecall-fold-hover {
  display: none;
}
.ecall-fold-node.is-collapsible.is-expanded .ecall-fold-open {
  display: inline;
}

/* 思考正文装得下收起档（四行）时，展开没有意义：
   悬停不换成折箭头、标题不给手型，免得点一下什么都没变。
   `data-faded` 由脚本量出「比收起档高」才写，量不出来就是没得展开。
   注意这条必须在上面三条之后：它要多一层 :has() 才能压过悬停规则。 */
.ecall-activity-row:has(.ecall-reasoning-text:not([data-faded="1"])):hover
  .ecall-fold-node.is-collapsible:not(.is-expanded)
  .ecall-fold-default {
  display: inline;
}
.ecall-activity-row:has(.ecall-reasoning-text:not([data-faded="1"])):hover
  .ecall-fold-node.is-collapsible:not(.is-expanded)
  .ecall-fold-hover {
  display: none;
}
.ecall-activity-row:has(.ecall-reasoning-text:not([data-faded="1"])) .ecall-reasoning-head {
  cursor: default;
}

.ecall-activity-item-summary {
  /* 单行截断：长命令/路径用 ellipsis 收尾，不允许多行撑开标题。
     完整内容在内容行展开后可见。 */
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ===== 两行网格 =====
   每个条目是两行：左列图标 / 右列标题行在上，左列竖线 / 右列内容行在下。
   两列共用同一套网格行，图标与标题行天然同高，不靠各写各的 padding 对齐。
   左列跨两行，竖线从图标下沿一直画到条目底部，负 bottom 吃掉 ul 的 0.5rem 间距。 */
.ecall-activity-rail-col {
  grid-column: 1;
  grid-row: 1 / span 2;
}

.ecall-activity-title {
  grid-column: 2;
  grid-row: 1;
}

.ecall-activity-body {
  grid-column: 2;
  grid-row: 2;
}

.ecall-activity-rail {
  top: 1.5rem;
  bottom: -0.5rem;
  left: 50%;
  transform: translateX(-50%);
}

/* ===== 思考折叠块 =====
   标题行 + 正文。收起时正文裁四行并淡出，展开后全文，不设滚动区。
   点击入口就是标题行本身；展开后标题行吸附在滚动区顶部。
   颜色不在这里定：由上面那张 6 色表按 data-kind 给。 */
/* 标题行：颜色由 .ecall-activity-title 的变量给，这里只管行高与手型 */
.ecall-reasoning-head {
  line-height: 1.5;
  cursor: pointer;
}

/* 思维链展开后标题吸附：内容再长也能随时点回收起。
   吸附范围被这一条 li 框住，滚到条目末尾自动脱离，不会压住后面的工具行。
   底色是固定的聊天区底色，和工具详情区那两条渐隐同一档。
   标题下方不留渐变：那条渐隐只能跟着展开态一起出现，无法只在真的贴住时出现，
   于是没贴顶的时候也压在正文上，看着像无端发白。内容从标题底下穿过时，
   标题自身的不透明底色已经挡住了。 */
.ecall-reasoning-head.ecall-reasoning-stuck {
  position: sticky;
  top: 0;
  z-index: 12;
  background-color: var(--color-base-200);
}

/* 图标那一格跟着一起吸附，否则标题吸住了、图标还跟着内容往上跑，看着像标题悬空。
   top 与标题行保持一致，两格才不会一上一下。 */
.ecall-activity-row:has(.ecall-reasoning-stuck) .ecall-fold-node {
  position: sticky;
  top: 0;
  z-index: 12;
  background-color: var(--color-base-200);
}

/* 内容行与标题行相隔 8px，和其它条目之间的一致；间距只由列的 pt-2 出，不再叠 margin */
.ecall-reasoning-text {
  line-height: 1.5;
  white-space: pre-wrap;
  word-break: break-word;
  overflow: hidden;
}

/* 收起档：最多露四行（四行 + pt-2 的 8px），多出来的裁掉。
   用 max-height 而不是 line-clamp：clamp 的实际高度会把段落间距算进去，
   和「行高 × 4」对不上，收起落定那一帧就会弹开；max-height 只是上限，
   装得下四行的短思考按自然高度显示，底下不会多出空白。 */
.ecall-reasoning-text:not(.is-open) {
  max-height: calc(1.5em * 4 + 8px);
}

/* 底部 2rem 淡出只在收起且真的被裁掉时加，短思考不加，否则最后两行会整片透明。
   用 mask 而不是盖一层渐变：气泡底色不固定，实色遮罩会在深色或自定义气泡里露馅。 */
.ecall-reasoning-text:not(.is-open)[data-faded="1"] {
  -webkit-mask-image: linear-gradient(to bottom, #000 calc(100% - 2rem), transparent 100%);
  mask-image: linear-gradient(to bottom, #000 calc(100% - 2rem), transparent 100%);
}

/* 工具详情：外壳常驻、高度 0 起，靠关键字插值在 0 与 auto 之间过渡。
   不支持 interpolate-size 的内核直接硬跳，不补 JS。 */
.ecall-tool-body {
  height: 0;
  overflow: hidden;
}

.ecall-tool-body.is-open {
  height: auto;
}

.ecall-tool-body,
.ecall-tool-body.is-open {
  interpolate-size: allow-keywords;
  transition: height var(--duration-fold) ease;
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

/* 正文条目的 markdown 渲染器根节点自带聊天正文那档字号，比折叠块其他条目大一档，
   这里拉回块内那一档。写 inherit 而不是某个固定值：块内字号已经统一由
   .ecall-activity-timeline 定义，跟着消息字号档位走，这里跟它对齐即可。 */
:deep(.ecall-plain-markdown-markdown) {
  font-size: inherit !important;
}

/* 正文条目用条目内容色（同一张表的 --ecall-body），靠 color-mix 淡文字，
   不连带淡掉里面的代码块底色。深浅色跟着 data-kind 表走，不再单独写第二条覆盖。 */
:deep(.ecall-plain-markdown-markdown),
:deep(.ecall-plain-markdown-markdown p) {
  color: var(--ecall-body) !important;
}
</style>
