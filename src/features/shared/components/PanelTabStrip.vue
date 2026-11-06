<template>
  <div class="flex shrink-0 items-start gap-2 border-b border-base-300 bg-base-200 p-1">
    <div v-if="$slots.leading" class="flex shrink-0 items-center">
      <slot name="leading" />
    </div>

    <!-- 标签会换行，这层不能再 overflow-hidden，否则第二行被裁掉。
         每行能放几个由固定标签宽度决定，容量估算用的是同一个数。 -->
    <div class="min-w-0 flex-1">
      <div
        ref="tabListRef"
        role="tablist"
        class="ecall-tab-list flex w-full min-w-0 flex-wrap items-center"
        :class="[
          fileVariant ? 'gap-1' : 'gap-0.5',
          fileVariant ? 'ecall-tab-list--file' : '',
        ]"
        :style="{ '--ecall-tab-w': `${effectiveTabWidth}px` }"
        :aria-label="ariaLabel"
      >
        <!-- 可见标签页列表（自适应防压缩） -->
        <div
          v-for="tab in renderedTabs"
          :key="tab.key"
          class="ecall-tab-item group relative flex min-w-0"
          :class="[
            fileVariant ? 'ecall-tab-item--file rounded-lg' : 'overflow-hidden',
            tab.disabled ? 'pointer-events-none opacity-45' : 'cursor-pointer',
            fileVariant && tab.key === activeKey ? 'bg-base-100' : '',
            fileVariant && tab.key !== activeKey ? 'hover:bg-base-300/60' : '',
          ]"
          :title="tab.title || tab.label"
          @auxclick="handleTabAuxClick(tab, $event)"
          @contextmenu="handleTabContextMenu(tab, $event)"
          @pointerdown="startLongPress(tab, $event)"
          @pointermove="trackLongPressMove"
          @pointerup="clearLongPress"
          @pointercancel="clearLongPress"
        >
          <button
            type="button"
            role="tab"
            class="min-w-0 flex-nowrap overflow-hidden"
            :class="[
              fileVariant
                ? 'flex h-auto min-h-7 flex-1 cursor-pointer items-center justify-start gap-1.5 rounded-lg bg-transparent px-2 py-1 text-left'
                : 'btn btn-ghost btn-sm w-full',
              !fileVariant && tab.key === activeKey ? 'bg-base-100' : '',
              fileVariant && tab.key === activeKey ? 'text-base-content' : '',
              fileVariant && tab.key !== activeKey ? 'text-base-content/65 hover:text-base-content' : '',
              !fileVariant && (tab.iconSrc || tab.icon) ? 'gap-1.5' : '',
              // 一律左对齐：图标与标题贴左，右侧那段空位留给运行态转圈，
              // 居中会让标题随文字长短左右漂，同一排的几个 tab 也对不齐
              !fileVariant ? (shouldReserveCloseSpace(tab) ? 'justify-start pr-8' : 'justify-start') : '',
            ]"
            :aria-selected="tab.key === activeKey"
            @click.stop="selectTab(tab)"
          >
            <template v-if="fileVariant">
              <span class="relative flex size-4 shrink-0 items-center justify-center">
                <img
                  v-if="tab.iconSrc"
                  :src="tab.iconSrc"
                  alt=""
                  class="panel-tab-strip-icon absolute size-4 object-contain transition-opacity group-hover:opacity-0"
                />
                <AppIcon
                  v-else-if="tab.icon"
                  :name="tab.icon"
                  class="absolute size-4 transition-opacity group-hover:opacity-0"
                  aria-hidden="true"
                />
                <span
                  v-if="tab.closeable && !tab.disabled"
                  role="button"
                  class="absolute inset-0 flex cursor-pointer items-center justify-center opacity-0 transition-opacity group-hover:opacity-100"
                  :title="closeTitle"
                  @click.stop="closeTab(tab)"
                >
                  <AppIcon name="close" class="size-3.5" />
                </span>
              </span>
            </template>
            <template v-else>
              <img
                v-if="tab.iconSrc"
                :src="tab.iconSrc"
                alt=""
                class="panel-tab-strip-icon size-4 shrink-0 rounded-full object-cover"
              />
              <AppIcon
                v-else-if="tab.icon"
                :name="tab.icon"
                class="size-4 shrink-0"
                aria-hidden="true"
              />
            </template>
            <!-- 会话状态圆点：与徽章/标签并列，不占标题的截断预算。类名由领域侧整套给出（几何+描边+填充），这里不猜 -->
            <span
              v-if="!fileVariant && tab.statusDotClass"
              :class="tab.statusDotClass"
              :title="tab.statusTitle"
              :aria-label="tab.statusTitle"
              role="img"
            ></span>
            <span
              v-if="!fileVariant && tab.badge"
              class="badge badge-xs shrink-0 border-base-300 bg-base-100"
            >{{ tab.badge }}</span>
            <span class="min-w-0 overflow-hidden whitespace-nowrap font-medium" :class="fileVariant ? 'file-tab-label-fade ecall-fade-right' : 'truncate'">{{ tab.label }}</span>
            <!-- 忙碌态：他在跑就把圈转起来。靠标签端部固定一格，标题再长也只截标题，不挤这一格 -->
            <AppLoading
              v-if="!fileVariant && tab.busy"
              size="xs"
              class="ml-auto shrink-0 text-base-content/60"
            />
          </button>
          <button
            v-if="!fileVariant && tab.closeable && shouldShowCloseButton(tab)"
            type="button"
            class="btn btn-ghost btn-xs btn-circle absolute right-1 top-1/2 -translate-y-1/2 border border-base-300 bg-base-100"
            :class="tab.key === activeKey
              ? 'opacity-100'
              : 'pointer-events-none opacity-0 transition-opacity hover:opacity-100 focus:pointer-events-auto focus:opacity-100 group-hover:pointer-events-auto group-hover:opacity-100'"
            :title="closeTitle"
            @click.stop="closeTab(tab)"
          >
            <AppIcon name="close" class="size-3.5" />
          </button>
        </div>

      </div>
    </div>

    <div v-if="$slots.tabTrailing" class="flex shrink-0 items-center pl-1">
      <slot name="tabTrailing" />
    </div>

    <!-- 折叠开关：展开态是「向右折叠」（收成一行），收起态是「向下折叠」（放回多行）。
         只有图标、不带文字——它是个动作，文字会把第一行挤窄。
         与 tabTrailing 一样钉在第一行右侧，所以放在滚动区之外：放进 tablist 会被 4 行滚动区滚走。 -->
    <button
      v-if="showStageToggle"
      type="button"
      class="ecall-tab-pill btn btn-ghost btn-sm btn-circle shrink-0"
      :title="collapsed ? t('chat.moreTabsShort') : t('chat.collapseTabs')"
      :aria-label="collapsed ? t('chat.moreTabsShort') : t('chat.collapseTabs')"
      :aria-expanded="!collapsed"
      @click.stop="toggleCollapsed"
    >
      <AppIcon
        :name="collapsed ? 'chevron-right' : 'chevron-down'"
        class="size-4 shrink-0"
      />
    </button>

    <div v-if="$slots.actions" class="flex shrink-0 items-center gap-1">
      <slot name="actions" />
    </div>

    <!-- 右键上下文菜单 -->
    <div
      v-if="closeMenu"
      class="fixed z-80 menu rounded-box border border-base-300 bg-base-100 p-1 shadow-xl"
      :style="{ left: `${closeMenu.x}px`, top: `${closeMenu.y}px` }"
      @pointerdown.stop
      @contextmenu.prevent.stop
    >
      <template v-if="contextMenuItems.length > 0">
        <button
          v-for="item in contextMenuItems"
          :key="item.label"
          type="button"
          class="btn btn-ghost btn-sm justify-start"
          @click.stop="runContextMenuItem(item)"
        >
          <span aria-hidden="true" class="inline-block size-4 shrink-0"></span>
          <span>{{ item.label }}</span>
        </button>
        <div v-if="currentCloseMenuTab?.closeable" class="my-1 border-t border-base-300"></div>
      </template>
      <button type="button" class="btn btn-ghost btn-sm justify-start" @click.stop="closeMenuTab">
        <AppIcon name="close" class="size-4" />
        <span>{{ closeTitle }}</span>
      </button>
      <button
        v-if="closeMenuCanCloseLeft"
        type="button"
        class="btn btn-ghost btn-sm justify-start"
        @click.stop="closeMenuTabsToLeft"
      >
        <span aria-hidden="true" class="inline-block size-4 shrink-0"></span>
        <span>{{ closeLeftTitle }}</span>
      </button>
      <button
        v-if="closeMenuCanCloseRight"
        type="button"
        class="btn btn-ghost btn-sm justify-start"
        @click.stop="closeMenuTabsToRight"
      >
        <span aria-hidden="true" class="inline-block size-4 shrink-0"></span>
        <span>{{ closeRightTitle }}</span>
      </button>
      <button
        v-if="closeMenuCanCloseOthers"
        type="button"
        class="btn btn-ghost btn-sm justify-start"
        @click.stop="closeMenuOtherTabs"
      >
        <span aria-hidden="true" class="inline-block size-4 shrink-0"></span>
        <span>{{ closeOthersTitle }}</span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, toRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "./AppIcon.vue";
import AppLoading from "./AppLoading.vue";
import { usePanelTabOverflow } from "../composables/use-panel-tab-overflow";
import type {
  PanelTabStripContextMenuItem,
  PanelTabStripItem,
  PanelTabStripVariant,
} from "./panel-tab-strip.types";

export type {
  PanelTabStripContextMenuItem,
  PanelTabStripItem,
  PanelTabStripVariant,
};

const CLOSE_BUTTON_MIN_TAB_WIDTH = 72;

const props = withDefaults(defineProps<{
  tabs: PanelTabStripItem[];
  activeKey?: string;
  ariaLabel?: string;
  closeTitle?: string;
  closeLeftTitle?: string;
  closeRightTitle?: string;
  closeOthersTitle?: string;
  contextMenuItems?: PanelTabStripContextMenuItem[];
  variant?: PanelTabStripVariant;
  /** 标签的换行基准宽（px），默认 104。它只决定一行排得下几个——
 *  标签的最终宽度是把一行按这个容量均分出来的值（见 effectiveTabWidth），视觉上限 160 / file 176。 */
  tabWidth?: number;
}>(), {
  activeKey: "",
  ariaLabel: "",
  closeTitle: "",
  closeLeftTitle: "",
  closeRightTitle: "",
  closeOthersTitle: "",
  contextMenuItems: () => [],
  variant: "default",
  tabWidth: undefined,
});

const emit = defineEmits<{
  (e: "selectTab", key: string): void;
  (e: "closeTab", key: string): void;
  (e: "closeTabsToLeft", key: string): void;
  (e: "closeTabsToRight", key: string): void;
  (e: "closeOtherTabs", key: string): void;
}>();

const { t } = useI18n();

const tabListRef = ref<HTMLElement | null>(null);
// 可摆标签的宽度：取滚动容器自己的 clientWidth，不是外层 host。
// 竖直滚动条是占位式的（约 15px），只扣在滚动容器的 clientWidth 上；
// 读 host 会把那 15px 也算进容量，临界宽度下就多估一个标签——多估的那个会被挤到下一行白占一行高度。
const availableWidth = ref(0);
const closeMenu = ref<{ key: string; x: number; y: number } | null>(null);
let longPressTimer: ReturnType<typeof setTimeout> | null = null;
let longPressStart: { key: string; x: number; y: number } | null = null;
let suppressNextSelectKey = "";
let tabListResizeObserver: ResizeObserver | null = null;

const fileVariant = computed(() => props.variant === "file");

/** 换行基准宽：只决定「一行排得下几个」，不代表标签最终宽度。 */
const TAB_BREAK_WIDTH = 104;
const breakWidth = computed(() => (
  typeof props.tabWidth === "number" && props.tabWidth > 0 ? props.tabWidth : TAB_BREAK_WIDTH
));

// ==================== 一行容量 ====================
// 一行能放几个 = 可摆宽整除「基准宽 + 间距」。可摆宽只读一次滚动容器的 clientWidth，不量内容。
const tabGap = computed(() => (fileVariant.value ? 4 : 2));

const { perRowCapacity } = usePanelTabOverflow({
  tabs: toRef(props, "tabs"),
  activeKey: toRef(props, "activeKey"),
  containerWidth: availableWidth,
  minTabWidth: breakWidth,
  overflowButtonWidth: 32,
  gap: tabGap,
});

// ==================== 标签宽度 ====================
// 宽度 = 把一行按容量均分出来的那个值，所有标签一律等宽。
// 这样第一行是铺满的（红豆要的就是这个），最后一行剩几个就摆几个、不会把单个标签拉宽——
// 用 flex-grow 拉满会让「最后一个标签」比同排的宽一截，内容和关闭按钮被推到两边，中间空一大块。
const tabMaxWidth = computed(() => (fileVariant.value ? 176 : 160));

const effectiveTabWidth = computed(() => {
  if (typeof props.tabWidth === "number" && props.tabWidth > 0) {
    return props.tabWidth;
  }
  const width = availableWidth.value;
  if (width <= 0) return TAB_BREAK_WIDTH;
  const cols = Math.max(1, perRowCapacity.value);
  const gap = tabGap.value;
  return Math.min(tabMaxWidth.value, Math.floor((width - (cols - 1) * gap) / cols));
});

// ==================== 收起 / 展开 ====================
// 纯展示态，组件自己记：默认展开。切会话看到的是新的一组标签，沿用上次的收起态反而会让它莫名少一行。
const collapsed = ref(false);

/** 只有真的会换行（一行装不下）时才给尾部胶囊；单行放得下时它只是噪音。 */
const willWrap = computed(() => props.tabs.length > perRowCapacity.value);
/** 展开态是「收起」、收起态是「更多」——同一位置两种文案。 */
const showStageToggle = computed(() => willWrap.value);

/** 收起态只摆「一行能放下的个数」：胶囊已在滚动区外，不占行内空间，
 *  所以这里按整行容量切，收起后正好一行，不需要 max-height 裁切。 */
const collapsedCapacity = computed(() => Math.max(1, perRowCapacity.value));
const renderedTabs = computed(() => (
  collapsed.value ? props.tabs.slice(0, collapsedCapacity.value) : props.tabs
));

function toggleCollapsed() {
  collapsed.value = !collapsed.value;
}

/** 关闭按钮要不要紧凑，直接看标签实际宽度，不再另算一遍。 */
const compactInactiveClose = computed(() => effectiveTabWidth.value < CLOSE_BUTTON_MIN_TAB_WIDTH);

function shouldShowCloseButton(tab: PanelTabStripItem) {
  if (!tab.closeable || tab.disabled) return false;
  if (tab.key === props.activeKey) return true;
  return !compactInactiveClose.value;
}

function shouldReserveCloseSpace(tab: PanelTabStripItem) {
  return shouldShowCloseButton(tab);
}

function selectTab(tab: PanelTabStripItem) {
  if (tab.disabled) return;
  if (suppressNextSelectKey === tab.key) {
    suppressNextSelectKey = "";
    return;
  }
  closeMenu.value = null;
  emit("selectTab", tab.key);
}

function closeTab(tab: PanelTabStripItem) {
  if (tab.disabled || !tab.closeable) return;
  closeMenu.value = null;
  emit("closeTab", tab.key);
}

function handleTabAuxClick(tab: PanelTabStripItem, event: MouseEvent) {
  if (event.button !== 1) return;
  event.preventDefault();
  closeTab(tab);
}

function runContextMenuItem(item: PanelTabStripContextMenuItem) {
  const tab = currentCloseMenuTab.value;
  closeMenu.value = null;
  if (!tab) return;
  item.onClick(tab.key);
}

function clearLongPress() {
  if (longPressTimer) {
    clearTimeout(longPressTimer);
    longPressTimer = null;
  }
  longPressStart = null;
}

function syncAvailableWidth() {
  availableWidth.value = tabListRef.value?.clientWidth ?? 0;
}

function observeTabListSize() {
  tabListResizeObserver?.disconnect();
  tabListResizeObserver = null;
  const tabList = tabListRef.value;
  if (!tabList) {
    availableWidth.value = 0;
    return;
  }
  syncAvailableWidth();
  if (typeof ResizeObserver === "undefined") return;
  // 观察滚动容器本身：竖直滚动条出现/消失会改变它的内容宽（content box），
  // 所以这一步也顺带把「滚动条挤掉 15px」这件事收进容量估算，不需要额外监听。
  tabListResizeObserver = new ResizeObserver(() => {
    syncAvailableWidth();
  });
  tabListResizeObserver.observe(tabList);
}

function menuPosition(x: number, y: number) {
  const menuWidth = 208;
  const menuItemCount = props.contextMenuItems.length
    + (currentCloseMenuTab.value?.closeable ? 1 : 0)
    + Number(closeMenuCanCloseLeft.value)
    + Number(closeMenuCanCloseRight.value)
    + Number(closeMenuCanCloseOthers.value);
  const menuHeight = 16 + menuItemCount * 40 + (props.contextMenuItems.length > 0 && currentCloseMenuTab.value?.closeable ? 9 : 0);
  const padding = 8;
  return {
    x: Math.min(Math.max(padding, x), Math.max(padding, window.innerWidth - menuWidth - padding)),
    y: Math.min(Math.max(padding, y), Math.max(padding, window.innerHeight - menuHeight - padding)),
  };
}

function openCloseMenu(tab: PanelTabStripItem, x: number, y: number) {
  if (tab.disabled || !tab.closeable) return;
  const position = menuPosition(x, y);
  closeMenu.value = { key: tab.key, ...position };
}

function handleTabContextMenu(tab: PanelTabStripItem, event: MouseEvent) {
  if (tab.disabled || !tab.closeable) return;
  event.preventDefault();
  event.stopPropagation();
  openCloseMenu(tab, event.clientX, event.clientY);
}

function startLongPress(tab: PanelTabStripItem, event: PointerEvent) {
  if (tab.disabled || !tab.closeable || event.pointerType === "mouse") return;
  clearLongPress();
  longPressStart = { key: tab.key, x: event.clientX, y: event.clientY };
  longPressTimer = setTimeout(() => {
    if (!longPressStart || longPressStart.key !== tab.key) return;
    suppressNextSelectKey = tab.key;
    openCloseMenu(tab, longPressStart.x, longPressStart.y);
    clearLongPress();
  }, 560);
}

function trackLongPressMove(event: PointerEvent) {
  if (!longPressStart) return;
  if (Math.abs(event.clientX - longPressStart.x) > 8 || Math.abs(event.clientY - longPressStart.y) > 8) {
    clearLongPress();
  }
}

function closeMenuTab() {
  const tab = currentCloseMenuTab.value;
  if (!tab) {
    closeMenu.value = null;
    return;
  }
  closeTab(tab);
}

const currentCloseMenuTab = computed(() => {
  const key = closeMenu.value?.key || "";
  return props.tabs.find((item) => item.key === key) || null;
});

const currentCloseMenuIndex = computed(() => {
  const key = currentCloseMenuTab.value?.key || "";
  return props.tabs.findIndex((item) => item.key === key);
});

const closeableTabs = computed(() => props.tabs.filter((item) => item.closeable && !item.disabled));

const closeMenuCanCloseLeft = computed(() => {
  const index = currentCloseMenuIndex.value;
  if (index <= 0) return false;
  return props.tabs.slice(0, index).some((item) => item.closeable && !item.disabled);
});

const closeMenuCanCloseRight = computed(() => {
  const index = currentCloseMenuIndex.value;
  if (index < 0) return false;
  return props.tabs.slice(index + 1).some((item) => item.closeable && !item.disabled);
});

const closeMenuCanCloseOthers = computed(() => {
  const currentKey = currentCloseMenuTab.value?.key || "";
  if (!currentKey) return false;
  return closeableTabs.value.some((item) => item.key !== currentKey);
});

function closeMenuTabsToLeft() {
  const tab = currentCloseMenuTab.value;
  if (!tab || !closeMenuCanCloseLeft.value) {
    closeMenu.value = null;
    return;
  }
  closeMenu.value = null;
  emit("closeTabsToLeft", tab.key);
}

function closeMenuTabsToRight() {
  const tab = currentCloseMenuTab.value;
  if (!tab || !closeMenuCanCloseRight.value) {
    closeMenu.value = null;
    return;
  }
  closeMenu.value = null;
  emit("closeTabsToRight", tab.key);
}

function closeMenuOtherTabs() {
  const tab = currentCloseMenuTab.value;
  if (!tab || !closeMenuCanCloseOthers.value) {
    closeMenu.value = null;
    return;
  }
  closeMenu.value = null;
  emit("closeOtherTabs", tab.key);
}

function closeFloatingMenu() {
  closeMenu.value = null;
}

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeFloatingMenu();
  }
}

watch(
  () => props.tabs.length,
  () => {
    void nextTick(syncAvailableWidth);
  },
);

onMounted(() => {
  window.addEventListener("pointerdown", closeFloatingMenu);
  window.addEventListener("keydown", handleWindowKeydown);
  void nextTick(observeTabListSize);
});

onBeforeUnmount(() => {
  clearLongPress();
  tabListResizeObserver?.disconnect();
  tabListResizeObserver = null;
  window.removeEventListener("pointerdown", closeFloatingMenu);
  window.removeEventListener("keydown", handleWindowKeydown);
});
</script>

<style scoped>
/* ===== 换行、4 行滚动区与标签宽度 =====
   所有标签等宽：宽度由 JS 按「一行容量均分」算出来后用 --ecall-tab-w 下发，
   所以第一行是铺满的，而最后一行剩几个就摆几个——不会把末尾单个标签拉宽。
   （用 flex-grow 拉满会让末尾那个比同排宽一截，内容和关闭按钮被推到两边，中间空一大块。）

   展开态高度 = 行高 × 4 + 行间距 × 3，写死算式，不量内容。超过 4 行在标签区内纵向滚动，
   不会把整块面板顶穿。滚动用原生 overflow，不走 OverlayScrollArea：见下方注释。 */
.ecall-tab-list {
  row-gap: 2px;
  max-height: calc(32px * 4 + 2px * 3);
  overflow-x: hidden;
  overflow-y: auto;
}

/* file 档标签实测也是 32px 高（label 行高 24 + 上下 padding 8），只是行间距宽一点。
   不要再按 28px 算——那样 4 行装不下，第 4 行会被裁掉一半。 */
.ecall-tab-list--file {
  row-gap: 4px;
  max-height: calc(32px * 4 + 4px * 3);
}

/* 宽度 = 按一行容量均分出来的固定值（由 --ecall-tab-w 下发），所有标签等宽。
   不用 flex-grow 拉满：那样最后一行只剩一两个标签时会被拉宽，和同排的宽度不一致。 */
.ecall-tab-item {
  flex: 0 0 auto;
  width: var(--ecall-tab-w, 104px);
  max-width: 160px;
}

.ecall-tab-item--file {
  max-width: 176px;
}

/* 收起态不再用 max-height 裁切：标签按「一行容量」切片渲染，收起后自然只有一行。
   裁切会把被藏的标签留在 DOM 里，Tab 键还能走到看不见的标签上。 */

.panel-tab-strip-icon {
  filter:
    drop-shadow(0 0 0.35px rgb(255 255 255 / 0.45))
    drop-shadow(0 0 0.45px rgb(15 23 42 / 0.22));
}

.file-tab-label-fade {
  flex: 1 1 0%;
  white-space: nowrap;
  overflow: hidden;
}
</style>
