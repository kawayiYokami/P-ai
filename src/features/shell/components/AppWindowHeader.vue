<template>
  <div
    class="min-h-10 h-10 overflow-visible select-none"
    :class="headerRootClass"
    :style="viewMode === 'chat' ? chatHeaderGridStyle : undefined"
    data-tauri-drag-region
    @pointerdown="handleTitlebarPointerDown"
    @dblclick="handleTitlebarDoubleClick"
  >
    <div
      v-if="viewMode !== 'chat' && !overlay"
      class="absolute inset-0"
      aria-hidden="true"
      data-tauri-drag-region
    ></div>
    <div
      v-else
      class="absolute inset-0 z-10"
      aria-hidden="true"
      data-tauri-drag-region
    ></div>
    <div
      v-if="viewMode === 'chat'"
      class="relative z-30 flex h-full min-w-0 items-center gap-1 px-2"
      data-tauri-drag-region
    >
      <div v-if="leftHeaderInLayout" class="indicator" @mousedown.stop @dblclick.stop>
        <span
          v-if="conversationUnreadTotal > 0"
          class="indicator-item indicator-top indicator-start z-10 h-2.5 w-2.5 -translate-x-0.5 -translate-y-0.5 rounded-full bg-error"
          aria-hidden="true"
        ></span>
        <button
          class="btn btn-ghost btn-sm h-8 min-h-8 px-2"
          :title="t('chat.conversationList')"
          @click.stop="emit('toggle-side-conversation-list')"
        >
          <AppIcon name="panel-left-close" v-if="sideConversationListVisible" class="h-3.5 w-3.5" />
          <AppIcon name="panel-left" v-else class="h-3.5 w-3.5" />
        </button>
      </div>
    </div>

    <div
      v-if="viewMode === 'chat'"
      class="relative z-30 grid h-full min-w-0 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-1 px-2"
      data-tauri-drag-region
    >
      <div class="relative z-40 flex min-w-0 items-center gap-1" @mousedown.stop @dblclick.stop>
        <div v-if="!leftHeaderInLayout" class="indicator">
          <span
            v-if="conversationUnreadTotal > 0"
            class="indicator-item indicator-top indicator-start z-10 h-2.5 w-2.5 -translate-x-0.5 -translate-y-0.5 rounded-full bg-error"
            aria-hidden="true"
          ></span>
          <button
            class="btn btn-ghost btn-sm h-8 min-h-8 px-2"
            :title="t('chat.conversationList')"
            @click.stop="emit('toggle-side-conversation-list')"
          >
            <AppIcon name="panel-left-close" v-if="sideConversationListVisible" class="h-3.5 w-3.5" />
            <AppIcon name="panel-left" v-else class="h-3.5 w-3.5" />
          </button>
        </div>
        <button
          class="btn btn-ghost btn-sm h-8 min-h-8 px-2"
          :title="t('chat.newConversation')"
          @click.stop="$emit('create-conversation')"
        >
          <AppIcon name="square-pen" class="h-4 w-4" />
        </button>
      </div>

      <div
        class="relative z-30 flex min-w-0 flex-1 self-stretch items-center justify-start gap-1 px-2"
        :title="combinedTitleTooltip"
        data-tauri-drag-region
      >
        <span
          class="truncate text-sm font-semibold text-base-content"
        >{{ combinedTitle }}</span>
      </div>

      <div class="relative z-40 flex min-w-0 items-center justify-end gap-1" @mousedown.stop @dblclick.stop>
      </div>
    </div>

    <div
      v-if="viewMode === 'chat'"
      class="relative z-30 flex h-full min-w-0 flex-nowrap items-center justify-end gap-1 px-2"
      @dblclick.stop
    >
      <button
        v-if="showUpdateToLatestButton"
        class="btn btn-success btn-sm h-8 min-h-8 gap-2 px-3 relative shadow-sm"
        :title="updateToLatestTitle || ''"
        @mousedown.stop
        @click.stop="$emit('update-to-latest')"
      >
        <span
          v-if="hasAvailableUpdate && !checkingUpdate"
          class="absolute right-1.5 top-1.5 h-2 w-2 rounded-full bg-error"
          aria-hidden="true"
        ></span>
        <span v-if="checkingUpdate" class="loading loading-spinner loading-xs"></span>
        <AppIcon name="download" v-else class="h-3.5 w-3.5" />
        <span>{{ updateToLatestLabel }}</span>
      </button>

      <button
        class="btn btn-ghost btn-sm"
        :title="t('chat.rightSidebarToggle')"
        @mousedown.stop
        @click.stop="$emit('toggle-tool-review-panel')"
      >
        <AppIcon name="panel-right-close" v-if="toolReviewPanelOpenVisible" class="h-3.5 w-3.5" />
        <AppIcon name="panel-right" v-else class="h-3.5 w-3.5" />
      </button>

      <button
        v-if="!hideSettings"
        class="btn btn-ghost btn-sm"
        :title="openSettingsTitle || t('common.settings')"
        @mousedown.stop
        @click.stop="$emit('open-settings')"
      >
        <AppIcon name="settings" class="h-3.5 w-3.5" />
      </button>

      <button
        v-if="showWindowControls"
        class="btn btn-ghost btn-sm"
        :title="t('window.minimize')"
        @mousedown.stop
        @click.stop="$emit('minimize-window')"
        :disabled="!windowReady"
      >
        <AppIcon name="minus" class="h-3.5 w-3.5" />
      </button>
      <button
        v-if="showWindowControls"
        class="btn btn-ghost btn-sm"
        :title="maximized ? t('window.restore') : t('window.maximize')"
        @mousedown.stop
        @click.stop="$emit('toggle-maximize-window')"
        :disabled="!windowReady"
      >
        <AppIcon name="square" class="h-3.5 w-3.5" />
      </button>
      <button
        v-if="showWindowControls"
        class="btn btn-sm btn-ghost hover:bg-error"
        :title="closeTitle || t('common.close')"
        @mousedown.stop
        @click.stop="$emit('close-window')"
        :disabled="!windowReady"
      >
        <AppIcon name="close" class="h-3.5 w-3.5" />
      </button>
    </div>

    <div v-if="viewMode !== 'chat' && !overlay" class="relative z-10 min-w-0 justify-self-start flex items-center gap-2" @mousedown.stop @dblclick.stop>
      <button
        v-if="viewMode === 'config' && !simpleSetupMode && showUpdateToLatestButton"
        class="btn btn-success btn-sm h-8 min-h-8 gap-2 px-3 relative shadow-sm"
        :title="updateToLatestTitle || ''"
        @click.stop="$emit('update-to-latest')"
      >
        <span
          v-if="hasAvailableUpdate && !checkingUpdate"
          class="absolute right-1.5 top-1.5 h-2 w-2 rounded-full bg-error"
          aria-hidden="true"
        ></span>
        <span v-if="checkingUpdate" class="loading loading-spinner loading-xs"></span>
        <AppIcon name="download" v-else class="h-3.5 w-3.5" />
        <span>{{ updateToLatestLabel }}</span>
      </button>
    </div>

    <div
      v-if="viewMode !== 'chat' && !overlay"
      class="relative z-10 w-fit min-w-0 flex items-center justify-center justify-self-center"
      @mousedown.stop
      @dblclick.stop
      data-tauri-drag-region
    >
      <div class="pointer-events-none flex items-center px-2">
        <span class="font-semibold text-sm">{{ titleText }}</span>
      </div>
    </div>

    <div
      v-if="viewMode !== 'chat'"
      class="relative z-10 flex shrink-0 flex-nowrap justify-self-end gap-1 px-2"
      :class="overlay ? 'pointer-events-auto' : ''"
      @mousedown.stop
      @dblclick.stop
    >
      <button
        v-if="overlay && !simpleSetupMode && showUpdateToLatestButton"
        class="btn btn-success btn-sm h-8 min-h-8 gap-2 px-3 relative shadow-sm"
        :title="updateToLatestTitle || ''"
        @mousedown.stop
        @click.stop="$emit('update-to-latest')"
      >
        <span
          v-if="hasAvailableUpdate && !checkingUpdate"
          class="absolute right-1.5 top-1.5 h-2 w-2 rounded-full bg-error"
          aria-hidden="true"
        ></span>
        <span v-if="checkingUpdate" class="loading loading-spinner loading-xs"></span>
        <AppIcon name="download" v-else class="h-3.5 w-3.5" />
        <span>{{ updateToLatestLabel }}</span>
      </button>
      <button
        v-if="showWindowControls"
        class="btn btn-ghost btn-sm"
        :title="t('window.minimize')"
        @click.stop="$emit('minimize-window')"
        :disabled="!windowReady"
      >
        <AppIcon name="minus" class="h-3.5 w-3.5" />
      </button>
      <button
        v-if="showWindowControls"
        class="btn btn-ghost btn-sm"
        :title="maximized ? t('window.restore') : t('window.maximize')"
        @click.stop="$emit('toggle-maximize-window')"
        :disabled="!windowReady"
      >
        <AppIcon name="square" class="h-3.5 w-3.5" />
      </button>
      <button
        v-if="showWindowControls"
        class="btn btn-sm btn-ghost hover:bg-error"
        :title="closeTitle || t('common.close')"
        @click.stop="$emit('close-window')"
        :disabled="!windowReady"
      >
        <AppIcon name="close" class="h-3.5 w-3.5" />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { getTransportCapabilities } from "../../../services/tauri-api";
import AppIcon from "../../shared/components/AppIcon.vue";
import type { ChatConversationOverviewItem } from "../../../types/app";
import { resolveConversationDisplayTitle } from "../../chat/utils/conversation-title";
import { usePipelineStatus } from "../composables/use-pipeline-status";

const props = withDefaults(defineProps<{
  viewMode: "chat" | "archives" | "config";
  currentTheme: string;
  titleText: string;
  currentPersonaName: string;
  sideConversationListVisible: boolean;
  toolReviewPanelOpenVisible: boolean;
  chatSidePanelWidths: { leftWidth: number; rightWidth: number };
  activeConversationId: string;
  conversationItems: ChatConversationOverviewItem[];
  userAlias: string;
  userAvatarUrl: string;
  personaNameMap: Record<string, string>;
  personaAvatarUrlMap: Record<string, string>;
  maximized: boolean;
  windowReady: boolean;
  openSettingsTitle: string;
  /** 隐藏标题栏设置按钮（聊天窗口由侧边栏头像菜单承担入口） */
  hideSettings?: boolean;
  closeTitle?: string;
  simpleSetupMode?: boolean;
  showUpdateToLatestButton?: boolean;
  hasAvailableUpdate?: boolean;
  checkingUpdate?: boolean;
  updateToLatestLabel?: string;
  updateToLatestTitle?: string;
  windowControlsVisible?: boolean;
  pipelineStatusEnabled?: boolean;
  /**
   * 覆盖式标题栏：整条不占布局高度，只留右上角窗口按钮浮在内容之上，标题与左列都不渲染。
   * 用于设置窗口——侧栏与内容直接顶到窗口上沿，面包屑自己占住标题栏那一行。
   */
  overlay?: boolean;
}>(), {
  windowControlsVisible: true,
  pipelineStatusEnabled: true,
  overlay: false,
  simpleSetupMode: false,
  hideSettings: false,
});

const pipelineStatus = props.pipelineStatusEnabled
  ? usePipelineStatus({
    activeConversationId: computed(() => String(props.activeConversationId || "").trim()),
  })
  : null;

const emit = defineEmits<{
  (e: "open-settings"): void;
  (e: "open-archives"): void;
  (e: "toggle-side-conversation-list"): void;
  (e: "toggle-tool-review-panel"): void;
  (e: "minimize-window"): void;
  (e: "toggle-maximize-window"): void;
  (e: "switch-conversation", payload: { conversationId: string; kind?: "local_unarchived" | "remote_im_contact" | "acp_session"; remoteContactId?: string }): void;
  (e: "rename-conversation", payload: { conversationId: string; title: string }): void;
  (e: "toggle-pin-conversation", conversationId: string): void;
  (e: "archive-conversation", conversationId: string): void;
  (e: "delete-conversation", conversationId: string): void;
  (e: "create-conversation", payload?: { workspaceRootPath?: string }): void;
  (e: "trimConversation"): void;
  (e: "startDrag"): void;
  (e: "close-window"): void;
  (e: "update-to-latest"): void;
  (e: "update:simple-setup-mode", value: boolean): void;
}>();

const { t, locale } = useI18n();
const transportCapabilities = getTransportCapabilities();
const localPathPickerAvailable = transportCapabilities.localPathPicker;

const headerRootClass = computed(() => {
  if (props.viewMode === "chat") return "shrink-0 relative z-40 grid items-center";
  if (props.overlay) {
    // 不占布局：绝对定位浮在内容之上，只有按钮组可交互，其余穿透给页面。
    // 注意不能同时留 relative/shrink-0：定位类互斥，留下的那个会把它重新塞回布局流。
    return "pointer-events-none absolute inset-x-0 top-0 z-50 flex items-center justify-end gap-1 px-2";
  }
  return "shrink-0 relative z-40 grid grid-cols-[1fr_auto_1fr] items-center px-2";
});

const showWindowControls = computed(() =>
  props.windowControlsVisible !== false && transportCapabilities.windowControls,
);

// ========== responsive header pane layout ==========

const windowWidth = ref(typeof window === "undefined" ? 0 : window.innerWidth);

// Tauri 原生拖动（data-tauri-drag-region）为首选，JS 拖动仅 WEB 回退
const TITLEBAR_DRAG_START_DISTANCE = 8;

let pendingTitlebarDrag: { x: number; y: number } | null = null;
let lastToggleMaximizeAt = 0;

function clearPendingTitlebarDrag() {
  pendingTitlebarDrag = null;
}

function handleWindowMouseMove(event: MouseEvent) {
  if (transportCapabilities.windowControls) return;
  const start = pendingTitlebarDrag;
  if (!start) return;
  if ((event.buttons & 1) === 0) {
    clearPendingTitlebarDrag();
    return;
  }
  if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < TITLEBAR_DRAG_START_DISTANCE) return;
  clearPendingTitlebarDrag();
  emit("startDrag");
}

function updateWindowWidth() {
  windowWidth.value = typeof window === "undefined" ? 0 : window.innerWidth;
}

function isInteractiveTitlebarTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return !!target.closest("button,input,textarea,select,a,label,summary,details,[role='button'],[contenteditable='true']");
}

function handleTitlebarDoubleClick(event: MouseEvent) {
  if (!props.windowReady || !showWindowControls.value) return;
  if (event.button !== 0) return;
  // 按钮容器已 @dblclick.stop，冒泡到这里说明点在拖动区
  if (isInteractiveTitlebarTarget(event.target)) return;
  // 避免单击按钮后极短时间内的双击误判
  if (Date.now() - lastToggleMaximizeAt < 300) return;
  lastToggleMaximizeAt = Date.now();
  event.preventDefault();
  event.stopPropagation();
  emit("toggle-maximize-window");
}

function handleTitlebarPointerDown(event: PointerEvent) {
  if (!props.windowReady || !showWindowControls.value) return;
  if (event.button !== 0) return;
  if (isInteractiveTitlebarTarget(event.target)) return;
  // Tauri 原生拖动区接管（windowControls 代理原生能力），无需 JS 起拖
  if (transportCapabilities.windowControls) return;
  if (event.detail >= 2) {
    clearPendingTitlebarDrag();
    event.preventDefault();
    event.stopPropagation();
    return;
  }
  pendingTitlebarDrag = { x: event.clientX, y: event.clientY };
}

// 兼容旧 @mousedown 绑定（若残留）
function handleTitlebarMouseDown(event: MouseEvent) {
  handleTitlebarPointerDown(event as unknown as PointerEvent);
}

function headerPaneWidth(side: "left" | "right"): number {
  const raw = side === "left"
    ? Number(props.chatSidePanelWidths?.leftWidth || 0)
    : Number(props.chatSidePanelWidths?.rightWidth || 0);
  const min = 260;
  return Math.max(min, Number.isFinite(raw) && raw > 0 ? Math.round(raw) : min);
}

function headerCanFit(leftW: number, rightW: number): boolean {
  return windowWidth.value <= 0 || leftW + 300 + rightW <= windowWidth.value;
}

const leftHeaderInLayout = computed(() => {
  if (props.viewMode !== "chat" || !props.sideConversationListVisible) return false;
  return headerCanFit(headerPaneWidth("left"), 0);
});

const rightHeaderInLayout = computed(() => {
  if (props.viewMode !== "chat" || !props.toolReviewPanelOpenVisible) return false;
  const rightW = headerPaneWidth("right");
  return leftHeaderInLayout.value
    ? headerCanFit(headerPaneWidth("left"), rightW)
    : headerCanFit(0, rightW);
});

const chatHeaderGridStyle = computed(() => {
  const leftColumn = leftHeaderInLayout.value
    ? `${headerPaneWidth("left")}px`
    : "0px";
  const rightColumn = rightHeaderInLayout.value
    ? `${headerPaneWidth("right")}px`
    : "max-content";
  return {
    gridTemplateColumns: `${leftColumn} minmax(0, 1fr) ${rightColumn}`,
  };
});


const currentConversationItem = computed(() => {
  const activeId = String(props.activeConversationId || "").trim();
  if (!activeId) return null;
  return props.conversationItems.find((i) => i.conversationId === activeId) || null;
});

const currentConversationTitle = computed(() => {
  const item = currentConversationItem.value;
  if (!item) return "";
  return resolveConversationDisplayTitle(item, {
    locale: locale.value,
    untitledLabel: t("chat.untitledConversation"),
    systemNotificationLabel: t("chat.systemPersona"),
  });
});

const conversationUnreadTotal = computed(() =>
  props.conversationItems.reduce((total, item) => total + Math.max(0, Number(item.unreadCount || 0)), 0),
);

const combinedTitle = computed(() => {
  const title = currentConversationTitle.value || props.currentPersonaName;
  return title;
});

const combinedTitleTooltip = computed(() => {
  return combinedTitle.value;
});

watch(
  () => props.activeConversationId,
  (conversationId) => pipelineStatus?.markConversationRead(conversationId),
  { immediate: true },
);
function isInteractiveHeaderTarget(target: HTMLElement): boolean {
  return Boolean(
    target.closest("button, input, textarea, select, option, a, label, summary, [role='button'], [contenteditable='true'], [data-no-drag]"),
  );
}

function handleOpenDraftConversationEvent(event: Event) {
  const detail = (event as CustomEvent<{ workspaceRootPath?: string }>).detail || {};
  emit("create-conversation", {
    workspaceRootPath: String(detail.workspaceRootPath || "").trim() || undefined,
  });
}

onMounted(() => {
  window.addEventListener("mousemove", handleWindowMouseMove);
  window.addEventListener("mouseup", clearPendingTitlebarDrag);
  window.addEventListener("blur", clearPendingTitlebarDrag);
  // 侧栏分节 + 与 composer「新会话」按钮通过全局事件请求打开会话草稿
  window.addEventListener("easy-call:open-draft-conversation", handleOpenDraftConversationEvent);
  updateWindowWidth();
  window.addEventListener("resize", updateWindowWidth, { passive: true });
});

onBeforeUnmount(() => {
  window.removeEventListener("mousemove", handleWindowMouseMove);
  window.removeEventListener("mouseup", clearPendingTitlebarDrag);
  window.removeEventListener("blur", clearPendingTitlebarDrag);
  window.removeEventListener("easy-call:open-draft-conversation", handleOpenDraftConversationEvent);
  window.removeEventListener("resize", updateWindowWidth);
});
</script>

<style scoped>
</style>
