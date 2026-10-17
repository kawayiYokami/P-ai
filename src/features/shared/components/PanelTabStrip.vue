<template>
  <div class="flex shrink-0 items-center gap-2 border-b border-base-300 bg-base-200 p-1">
    <div v-if="$slots.leading" class="flex shrink-0 items-center">
      <slot name="leading" />
    </div>

    <div ref="tabListHostRef" class="min-w-0 flex-1" :class="fileVariant ? '' : 'overflow-hidden'">
      <div
        role="tablist"
        class="flex w-full min-w-0 items-center gap-0"
        :class="fileVariant ? 'gap-1' : 'gap-0'"
        :aria-label="ariaLabel"
      >
        <div
          v-for="(tab, tabIndex) in tabs"
          :key="tab.key"
          class="group relative flex min-w-8 max-w-40 flex-1 basis-0"
          :class="[
            fileVariant ? 'rounded-lg' : 'overflow-hidden',
            tab.disabled ? 'pointer-events-none opacity-45' : 'cursor-pointer',
            tabBorderClass(tab.key, tabIndex),
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
              !fileVariant ? (shouldReserveCloseSpace(tab) ? 'justify-start pr-8' : 'justify-center') : '',
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
                <component
                  :is="tab.icon"
                  v-else-if="tab.icon"
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
                  <X class="size-3.5" />
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
              <component
                :is="tab.icon"
                v-else-if="tab.icon"
                class="size-4 shrink-0"
                aria-hidden="true"
              />
            </template>
            <span
              v-if="!fileVariant && tab.badge"
              class="badge badge-xs shrink-0 border-base-300 bg-base-100"
            >{{ tab.badge }}</span>
            <span class="min-w-0 overflow-hidden whitespace-nowrap font-medium" :class="fileVariant ? 'file-tab-label-fade' : 'truncate'">{{ tab.label }}</span>
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
            <X class="size-3.5" />
          </button>
        </div>
        <div v-if="$slots.tabTrailing" class="flex shrink-0 items-center pl-1">
          <slot name="tabTrailing" />
        </div>
      </div>
    </div>

    <div v-if="$slots.actions" class="flex shrink-0 items-center gap-1">
      <slot name="actions" />
    </div>

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
        <X class="size-4" />
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
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import { X } from "@lucide/vue";

type PanelTabStripItem = {
  key: string;
  label: string;
  title?: string;
  badge?: string;
  iconSrc?: string;
  icon?: Component;
  closeable?: boolean;
  disabled?: boolean;
};

type PanelTabStripContextMenuItem = {
  label: string;
  onClick: (key: string) => void;
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
  showTabBorders?: boolean;
  variant?: "default" | "file";
}>(), {
  activeKey: "",
  ariaLabel: "",
  closeTitle: "",
  closeLeftTitle: "",
  closeRightTitle: "",
  closeOthersTitle: "",
  contextMenuItems: () => [],
  showTabBorders: true,
  variant: "default",
});

const emit = defineEmits<{
  (e: "selectTab", key: string): void;
  (e: "closeTab", key: string): void;
  (e: "closeTabsToLeft", key: string): void;
  (e: "closeTabsToRight", key: string): void;
  (e: "closeOtherTabs", key: string): void;
}>();

const tabListHostRef = ref<HTMLElement | null>(null);
const hostWidth = ref(0);
const closeMenu = ref<{ key: string; x: number; y: number } | null>(null);
let longPressTimer: ReturnType<typeof setTimeout> | null = null;
let longPressStart: { key: string; x: number; y: number } | null = null;
let suppressNextSelectKey = "";
let hostResizeObserver: ResizeObserver | null = null;

const fileVariant = computed(() => props.variant === "file");

const estimatedTabWidth = computed(() => {
  const tabCount = Math.max(props.tabs.length, 1);
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

function tabBorderClass(tabKey: string, tabIndex: number) {
  if (fileVariant.value) return "";
  if (!props.showTabBorders || tabIndex <= 0) return "";
  const prevTab = props.tabs[tabIndex - 1];
  if (!prevTab) return "";
  if (tabKey === props.activeKey || prevTab.key === props.activeKey) return "";
  return "border-l border-base-100";
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
  if (event.key === "Escape") closeFloatingMenu();
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
  -webkit-mask-image: linear-gradient(to right, #000 0, #000 calc(100% - 1rem), transparent 100%);
  mask-image: linear-gradient(to right, #000 0, #000 calc(100% - 1rem), transparent 100%);
}
</style>
