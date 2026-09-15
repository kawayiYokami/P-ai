<template>
  <div class="relative flex h-screen min-h-0 flex-col bg-base-100 text-base-content">
    <header class="flex h-10 shrink-0 items-end gap-2 bg-base-200 px-2" data-tauri-drag-region>
      <button class="btn btn-ghost btn-sm shrink-0" type="button" :title="t('fileReader.openFile')" @click.stop="pickFile">
        <FilePlus class="h-4 w-4" />
      </button>
      <div class="flex min-w-0 flex-1 items-end gap-1 overflow-hidden" data-tauri-drag-region>
        <div
          v-for="tab in fileReaderPanelRef?.tabs || []"
          :key="tab.path"
          class="group flex h-9 min-w-0 max-w-64 flex-1 basis-0 items-center gap-2 overflow-hidden rounded-t-box border border-b-0 px-2 text-sm"
          :class="tab.path === fileReaderPanelRef?.activePath ? 'border-base-300 bg-base-100 text-base-content' : 'border-transparent bg-base-200 text-base-content/65 hover:bg-base-100/70 hover:text-base-content'"
          :title="tab.path"
          role="button"
          tabindex="0"
          :aria-selected="tab.path === fileReaderPanelRef?.activePath"
          @click="fileReaderPanelRef?.setActiveTab(tab.path)"
          @keydown.enter.prevent="fileReaderPanelRef?.setActiveTab(tab.path)"
          @keydown.space.prevent="fileReaderPanelRef?.setActiveTab(tab.path)"
          @contextmenu.prevent.stop="openTabMenu(tab.path, $event.clientX, $event.clientY)"
        >
          <FileText class="h-4 w-4 shrink-0 opacity-70" />
          <span class="min-w-0 flex-1 truncate font-medium">{{ tab.title }}</span>
          <button
            type="button"
            class="btn btn-ghost btn-xs h-5 min-h-5 w-5 p-0 opacity-60 hover:opacity-100"
            :title="t('fileReader.close')"
            @click.stop="fileReaderPanelRef?.closeTab(tab.path)"
          >
            <X class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
      <button class="btn btn-ghost btn-sm shrink-0" type="button" title="最小化" @click.stop="minimizeWindow">
        <Minus class="h-3.5 w-3.5" />
      </button>
      <button class="btn btn-ghost btn-sm shrink-0" type="button" :title="maximized ? '还原窗口' : '最大化'" @click.stop="toggleMaximizeWindow">
        <Square class="h-3.5 w-3.5" />
      </button>
      <button class="btn btn-sm btn-ghost shrink-0 hover:bg-error" type="button" :title="t('fileReader.close')" @click.stop="closeWindow">
        <X class="h-3.5 w-3.5" />
      </button>
    </header>

    <div
      v-if="tabMenu"
      class="fixed z-80 menu rounded-box border border-base-300 bg-base-100 p-1 shadow-xl"
      :style="{ left: `${tabMenu.x}px`, top: `${tabMenu.y}px` }"
      @pointerdown.stop
      @contextmenu.prevent.stop
    >
      <button type="button" class="btn btn-ghost btn-sm justify-start" @click.stop="closeCurrentTabFromMenu">
        <X class="size-4" />
        <span>{{ t('fileReader.close') }}</span>
      </button>
      <button
        v-if="tabMenuCanCloseLeft"
        type="button"
        class="btn btn-ghost btn-sm justify-start"
        @click.stop="closeTabsToLeftFromMenu"
      >
        <span aria-hidden="true" class="inline-block size-4 shrink-0"></span>
        <span>{{ t('fileReader.closeLeft') }}</span>
      </button>
      <button
        v-if="tabMenuCanCloseRight"
        type="button"
        class="btn btn-ghost btn-sm justify-start"
        @click.stop="closeTabsToRightFromMenu"
      >
        <span aria-hidden="true" class="inline-block size-4 shrink-0"></span>
        <span>{{ t('fileReader.closeRight') }}</span>
      </button>
      <button
        v-if="tabMenuCanCloseOthers"
        type="button"
        class="btn btn-ghost btn-sm justify-start"
        @click.stop="closeOtherTabsFromMenu"
      >
        <span aria-hidden="true" class="inline-block size-4 shrink-0"></span>
        <span>{{ t('fileReader.closeOthers') }}</span>
      </button>
    </div>

    <FileReaderPanel
      ref="fileReaderPanelRef"
      class="min-h-0 flex-1"
      :show-tabs="false"
      :show-pick-file-button="false"
      :markdown-is-dark="markdownIsDark"
      custom-markstream-id="file-reader-markstream"
      @add-context-reference="addContextReferenceToChat"
    />

    <FileLinkContextMenu
      :state="fileLinkMenuState"
      :can-open-in-sidebar="false"
      @close="closeFileLinkMenu"
    />

    <Win10ResizeHandles :enabled="!maximized" />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { FilePlus, FileText, Minus, Square, X } from "@lucide/vue";
import { useI18n } from "vue-i18n";
import FileReaderPanel from "../../features/file-reader/components/FileReaderPanel.vue";
import { directoryFromPath } from "../../features/file-reader/utils";
import FileLinkContextMenu from "../../features/shared/components/FileLinkContextMenu.vue";
import { useFileLinkContextMenu } from "../../features/shared/composables/use-file-link-context-menu";
import type { AppThemeState } from "../../features/shell/theme/theme-types";
import { isDarkAppTheme, useAppTheme } from "../../features/shell/composables/use-app-theme";
import Win10ResizeHandles from "../../features/shell/components/Win10ResizeHandles.vue";
import {
  currentTransportWindowIsMaximized,
  emitTransportEventTo,
  hideCurrentTransportWindow,
  invokeTauri,
  getTransportLaunchParameter,
  minimizeCurrentTransportWindow,
  onTransportNotification,
  openTransportFileDialog,
  openTransportWindow,
  toggleCurrentTransportWindowMaximize,
} from "../../services/tauri-api";
import type { IdeContextReferenceItem } from "../../types/app";

const FILE_READER_SESSION_STORAGE_KEY = "easy_call.file_reader_session.v1";
const LEGACY_FILE_READER_SESSION_STORAGE_KEY = "easy-call:file-reader-session:v1";

type FileReaderSessionState = {
  tabs?: string[];
  activePath?: string;
  directoryRootPath?: string;
};

const { applyTheme, currentTheme, restoreThemeFromStorage } = useAppTheme();
const { t } = useI18n();
const maximized = ref(false);
const fileReaderPanelRef = ref<InstanceType<typeof FileReaderPanel> | null>(null);
const markdownIsDark = computed(() => isDarkAppTheme(currentTheme.value));
const tabMenu = ref<{ path: string; x: number; y: number } | null>(null);

// 文件链接右键菜单：本窗口自己就是阅读器，不再提供「在侧边打开」；
// 相对路径以当前打开文件所在目录为基准，与面板内左键打开口径一致。
const { state: fileLinkMenuState, close: closeFileLinkMenu } = useFileLinkContextMenu({
  workspaceRoot: () => directoryFromPath(fileReaderPanelRef.value?.activePath || ""),
});

let unlistenOpenPath: (() => void) | null = null;
let unlistenThemeChanged: (() => void) | null = null;

async function addContextReferenceToChat(reference: IdeContextReferenceItem) {
  try {
    await openTransportWindow("chat");
    await emitTransportEventTo("chat", "fileReader.addToChat", reference);
  } catch (error) {
    console.error("[文件阅读器] 添加选区到聊天失败", error);
  }
}

const currentTabMenuIndex = computed(() => {
  const path = tabMenu.value?.path || "";
  if (!path) return -1;
  return fileReaderPanelRef.value?.tabs.findIndex((tab) => tab.path === path) ?? -1;
});

const tabMenuCanCloseLeft = computed(() => currentTabMenuIndex.value > 0);
const tabMenuCanCloseRight = computed(() => {
  const panel = fileReaderPanelRef.value;
  if (!panel) return false;
  return currentTabMenuIndex.value >= 0 && currentTabMenuIndex.value < panel.tabs.length - 1;
});
const tabMenuCanCloseOthers = computed(() => {
  const panel = fileReaderPanelRef.value;
  return !!panel && currentTabMenuIndex.value >= 0 && panel.tabs.length > 1;
});

function normalizePath(path: string) {
  return String(path || "")
    .trim()
    .replace(/^\\\\\?\\/, "")
    .replace(/^\/\/\?\//, "")
    .replace(/^\/\?\//, "")
    .replace(/^\?\//, "")
    .replace(/^\?\\/, "")
    .replace(/\\/g, "/");
}

function readFileReaderSessionState(): FileReaderSessionState {
  try {
    return JSON.parse(localStorage.getItem(FILE_READER_SESSION_STORAGE_KEY) || localStorage.getItem(LEGACY_FILE_READER_SESSION_STORAGE_KEY) || "{}") as FileReaderSessionState;
  } catch {
    return {};
  }
}

function persistFileReaderSession() {
  const panel = fileReaderPanelRef.value;
  if (!panel) return;
  const uniqueTabs = Array.from(new Set(panel.tabs.map((tab) => normalizePath(tab.path)).filter(Boolean)));
  const state: FileReaderSessionState = {
    tabs: uniqueTabs,
    activePath: normalizePath(panel.activePath),
    directoryRootPath: normalizePath(panel.directoryRootPath),
  };
  localStorage.setItem(FILE_READER_SESSION_STORAGE_KEY, JSON.stringify(state));
}

function menuPosition(x: number, y: number) {
  const menuWidth = 132;
  const menuHeight = 164;
  const padding = 8;
  return {
    x: Math.min(Math.max(padding, x), Math.max(padding, window.innerWidth - menuWidth - padding)),
    y: Math.min(Math.max(padding, y), Math.max(padding, window.innerHeight - menuHeight - padding)),
  };
}

function openTabMenu(path: string, x: number, y: number) {
  const panel = fileReaderPanelRef.value;
  if (!panel?.tabs.some((tab) => tab.path === path)) return;
  tabMenu.value = { path, ...menuPosition(x, y) };
}

function closeTabMenu() {
  tabMenu.value = null;
}

function closeCurrentTabFromMenu() {
  const path = tabMenu.value?.path || "";
  if (!path) return closeTabMenu();
  fileReaderPanelRef.value?.closeTab(path);
  closeTabMenu();
}

function closeTabsToLeftFromMenu() {
  const path = tabMenu.value?.path || "";
  if (!path) return closeTabMenu();
  fileReaderPanelRef.value?.closeTabsToLeftOf(path);
  closeTabMenu();
}

function closeTabsToRightFromMenu() {
  const path = tabMenu.value?.path || "";
  if (!path) return closeTabMenu();
  fileReaderPanelRef.value?.closeTabsToRightOf(path);
  closeTabMenu();
}

function closeOtherTabsFromMenu() {
  const path = tabMenu.value?.path || "";
  if (!path) return closeTabMenu();
  fileReaderPanelRef.value?.closeOtherTabs(path);
  closeTabMenu();
}

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") closeTabMenu();
}

async function restoreFileReaderSession(loadActiveTab: boolean) {
  const panel = fileReaderPanelRef.value;
  if (!panel) return;
  const state = readFileReaderSessionState();
  const restoredTabs = Array.from(new Set((state.tabs || []).map((path) => normalizePath(path)).filter(Boolean)));

  for (const path of restoredTabs) {
    await panel.openPath(path);
  }

  if (restoredTabs.length > 0) {
    const restoredActivePath = normalizePath(state.activePath || "");
    panel.setActiveTab(restoredTabs.includes(restoredActivePath) ? restoredActivePath : restoredTabs[0]);
  }

  const restoredDirectoryRoot = normalizePath(state.directoryRootPath || "");
  if (restoredDirectoryRoot) {
    await panel.openDirectoryTree(restoredDirectoryRoot);
  }
}

async function pickFile() {
  const picked = await openTransportFileDialog({ multiple: false, directory: false, title: "打开文件" });
  if (!picked || Array.isArray(picked)) return;
  await fileReaderPanelRef.value?.openPath(String(picked));
}

async function syncWindowState() {
  try {
    maximized.value = await currentTransportWindowIsMaximized();
  } catch {
    maximized.value = false;
  }
}

async function minimizeWindow() {
  await minimizeCurrentTransportWindow();
}

async function toggleMaximizeWindow() {
  await toggleCurrentTransportWindowMaximize();
  await syncWindowState();
}

async function closeWindow() {
  persistFileReaderSession();
  await hideCurrentTransportWindow();
}

onMounted(async () => {
  window.addEventListener("pointerdown", closeTabMenu);
  window.addEventListener("keydown", handleWindowKeydown);
  restoreThemeFromStorage();
  try {
    unlistenThemeChanged = onTransportNotification<AppThemeState>("theme.changed", (payload) => {
      applyTheme(payload);
    });
  } catch (error) {
    console.error("[文件阅读窗口] 监听主题变化失败", error);
  }
  void syncWindowState();
  const path = getTransportLaunchParameter("path");
  await restoreFileReaderSession(!path);
  if (path) {
    void fileReaderPanelRef.value?.openPath(path, { revealInDirectoryTree: true });
  }
  try {
    unlistenOpenPath = onTransportNotification<{ path?: string }>("fileReader.openPath", (payload) => {
      void fileReaderPanelRef.value?.openPath(payload?.path || "", { revealInDirectoryTree: true });
    });
  } catch (error) {
    console.error("[文件阅读窗口] 监听打开文件事件失败", error);
  }
});

onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", closeTabMenu);
  window.removeEventListener("keydown", handleWindowKeydown);
  unlistenOpenPath?.();
  unlistenThemeChanged?.();
});
</script>
