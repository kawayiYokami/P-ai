<template>
  <div class="flex shrink-0 items-center gap-2 border-b border-base-300 bg-base-200 p-1">
    <div v-if="$slots.leading" class="flex shrink-0 items-center">
      <slot name="leading" />
    </div>

    <div ref="tabListHostRef" class="min-w-0 flex-1" :class="fileVariant ? '' : 'overflow-hidden'">
      <div
        role="tablist"
        class="flex w-full min-w-0 items-center"
        :class="fileVariant ? 'gap-1' : 'gap-0.5'"
        :aria-label="ariaLabel"
      >
        <!-- 可见标签页列表（自适应防压缩） -->
        <div
          v-for="tab in visibleTabs"
          :key="tab.key"
          class="group relative flex min-w-0 flex-1 basis-0"
          :class="[
            fileVariant ? 'max-w-44 rounded-lg' : 'max-w-40 overflow-hidden',
            tab.disabled ? 'pointer-events-none opacity-45' : 'cursor-pointer',
            fileVariant && tab.key === activeKey ? 'bg-base-100 shadow-sm' : '',
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
              !fileVariant && tab.key === activeKey ? 'bg-base-100/60' : '',
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
            <span
              v-if="!fileVariant && tab.busy"
              class="loading loading-spinner loading-xs ml-auto shrink-0 text-base-content/60"
              aria-hidden="true"
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

        <!-- 溢出“...”更多标签页下拉按钮 -->
        <div v-if="hasOverflow" class="flex shrink-0 items-center pl-0.5">
          <EcallDropdown
            v-model="overflowDropdownOpen"
            :open-on-hover="openMoreOnHover"
            teleport
            :match-trigger-width="false"
            panel-class="min-w-[190px] max-w-[280px] p-1 shadow-2xl border border-base-300 bg-base-100"
          >
            <template #trigger="{ toggle, open }">
              <button
                type="button"
                class="btn btn-ghost btn-xs size-7 min-h-7 rounded-lg px-0 transition-colors"
                :class="[
                  open ? 'bg-base-300 text-base-content' : 'text-base-content/70 hover:bg-base-300/60 hover:text-base-content',
                  activeInOverflow ? 'bg-base-100/80 font-semibold shadow-xs ring-1 ring-base-content/15' : '',
                ]"
                :title="overflowButtonTitle"
                @click.stop="toggle"
              >
                <AppIcon name="more-horizontal" class="size-4" />
              </button>
            </template>
            <template #default>
              <div class="flex flex-col gap-0.5">
                <div class="px-2 py-1 text-micro font-medium text-base-content/50 select-none">
                  {{ t("chat.moreTabs") }} ({{ overflowTabs.length }})
                </div>
                <div
                  v-for="tab in overflowTabs"
                  :key="tab.key"
                  class="group/more-tab flex items-center justify-between gap-2 rounded-lg px-2 py-1.5 text-xs transition-colors cursor-pointer select-none"
                  :class="[
                    tab.key === activeKey ? 'bg-base-200/80 font-semibold text-base-content' : 'text-base-content/80 hover:bg-base-200/50',
                    tab.disabled ? 'pointer-events-none opacity-45' : '',
                  ]"
                  :title="tab.title || tab.label"
                  @click="selectOverflowTab(tab)"
                  @contextmenu="handleTabContextMenu(tab, $event)"
                >
                  <div class="flex min-w-0 items-center gap-2">
                    <img
                      v-if="tab.iconSrc"
                      :src="tab.iconSrc"
                      alt=""
                      class="size-4 shrink-0 rounded-full object-cover"
                    />
                    <AppIcon
                      v-else-if="tab.icon"
                      :name="tab.icon"
                      class="size-4 shrink-0"
                      aria-hidden="true"
                    />
                    <span
                      v-if="tab.statusDotClass"
                      :class="tab.statusDotClass"
                      :title="tab.statusTitle"
                      class="size-2 shrink-0 rounded-full"
                    />
                    <span class="truncate">{{ tab.label }}</span>
                  </div>
                  <button
                    v-if="tab.closeable && !tab.disabled"
                    type="button"
                    class="btn btn-ghost btn-xs btn-circle size-5 min-h-5 opacity-60 hover:opacity-100"
                    :title="closeTitle"
                    @click.stop="closeOverflowTab(tab)"
                  >
                    <AppIcon name="close" class="size-3" />
                  </button>
                </div>
              </div>
            </template>
          </EcallDropdown>
        </div>

        <div v-if="$slots.tabTrailing" class="flex shrink-0 items-center pl-1">
          <slot name="tabTrailing" />
        </div>
      </div>
    </div>

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
import EcallDropdown from "./EcallDropdown.vue";
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
  /** 单个卡片最小宽度保护（防止过分挤压），默认 default: 104px, file: 92px */
  minTabWidth?: number;
  /** 更多按钮下拉菜单是否悬停触发，默认 true */
  openMoreOnHover?: boolean;
}>(), {
  activeKey: "",
  ariaLabel: "",
  closeTitle: "",
  closeLeftTitle: "",
  closeRightTitle: "",
  closeOthersTitle: "",
  contextMenuItems: () => [],
  variant: "default",
  minTabWidth: undefined,
  openMoreOnHover: true,
});

const emit = defineEmits<{
  (e: "selectTab", key: string): void;
  (e: "closeTab", key: string): void;
  (e: "closeTabsToLeft", key: string): void;
  (e: "closeTabsToRight", key: string): void;
  (e: "closeOtherTabs", key: string): void;
}>();

const { t } = useI18n();

const tabListHostRef = ref<HTMLElement | null>(null);
const hostWidth = ref(0);
const overflowDropdownOpen = ref(false);
const closeMenu = ref<{ key: string; x: number; y: number } | null>(null);
let longPressTimer: ReturnType<typeof setTimeout> | null = null;
let longPressStart: { key: string; x: number; y: number } | null = null;
let suppressNextSelectKey = "";
let hostResizeObserver: ResizeObserver | null = null;

const fileVariant = computed(() => props.variant === "file");

const effectiveMinTabWidth = computed(() => {
  if (typeof props.minTabWidth === "number" && props.minTabWidth > 0) {
    return props.minTabWidth;
  }
  return fileVariant.value ? 92 : 104;
});

// ==================== 自适应溢出计算 ====================
const { visibleTabs, overflowTabs, hasOverflow, activeInOverflow } = usePanelTabOverflow({
  tabs: toRef(props, "tabs"),
  activeKey: toRef(props, "activeKey"),
  containerWidth: hostWidth,
  minTabWidth: effectiveMinTabWidth,
  overflowButtonWidth: 32,
  gap: computed(() => (fileVariant.value ? 4 : 2)),
});

const overflowButtonTitle = computed(() => {
  return `${t("chat.moreTabs")} (${overflowTabs.value.length})`;
});

const estimatedTabWidth = computed(() => {
  const tabCount = Math.max(visibleTabs.value.length, 1);
  const available = Math.max(hostWidth.value, 0);
  if (available <= 0) return 160;
  return Math.min(160, available / tabCount);
});

const compactInactiveClose = computed(() => estimatedTabWidth.value < CLOSE_BUTTON_MIN_TAB_WIDTH);

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

function selectOverflowTab(tab: PanelTabStripItem) {
  overflowDropdownOpen.value = false;
  selectTab(tab);
}

function closeTab(tab: PanelTabStripItem) {
  if (tab.disabled || !tab.closeable) return;
  closeMenu.value = null;
  emit("closeTab", tab.key);
}

function closeOverflowTab(tab: PanelTabStripItem) {
  closeTab(tab);
  if (overflowTabs.value.length <= 1) {
    overflowDropdownOpen.value = false;
  }
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

function syncHostWidth() {
  hostWidth.value = tabListHostRef.value?.clientWidth ?? 0;
}

function observeTabListHost() {
  hostResizeObserver?.disconnect();
  hostResizeObserver = null;
  const host = tabListHostRef.value;
  if (!host) {
    hostWidth.value = 0;
    return;
  }
  syncHostWidth();
  if (typeof ResizeObserver === "undefined") return;
  hostResizeObserver = new ResizeObserver(() => {
    syncHostWidth();
  });
  hostResizeObserver.observe(host);
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
    overflowDropdownOpen.value = false;
  }
}

watch(
  () => props.tabs.length,
  () => {
    void nextTick(syncHostWidth);
  },
);

onMounted(() => {
  window.addEventListener("pointerdown", closeFloatingMenu);
  window.addEventListener("keydown", handleWindowKeydown);
  void nextTick(observeTabListHost);
});

onBeforeUnmount(() => {
  clearLongPress();
  hostResizeObserver?.disconnect();
  hostResizeObserver = null;
  window.removeEventListener("pointerdown", closeFloatingMenu);
  window.removeEventListener("keydown", handleWindowKeydown);
});
</script>

<style scoped>
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
