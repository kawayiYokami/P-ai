<template>
  <CardShell
    layout="tile"
    class="group relative"
    tone="warning"
    :icon="FolderTree"
    :label="t('chat.homePanel.workspace')"
    interactive
    @select="emit('open')"
    @contextmenu.prevent.stop="handleContextMenu"
  >
    <div class="flex w-full min-w-0 flex-col items-center gap-1.5">
      <!-- 工作目录名称 -->
      <span
        class="w-full truncate text-xs text-base-content/50 transition-colors group-hover:text-base-content/75"
        :title="workspaceRootPath"
      >
        {{ name }}
      </span>

      <!-- 居中操作胶囊：在此目录打开各种应用/终端（仅桌面端可见，手机端不可见） -->
      <div
        v-if="showOpenActions"
        class="ecall-home-workspace-actions flex max-w-full items-center justify-center pt-0.5"
        @click.stop
        @keydown.stop
      >
        <EcallDropdown
          v-model="dropdownOpen"
          teleport
          :match-trigger-width="false"
          panel-class="w-56 p-0"
          placement="bottom"
        >
          <template #trigger="{ toggle }">
            <div
              class="inline-flex max-w-full items-center rounded-full border border-base-content/10 bg-base-200/80 p-0.5 text-xs text-base-content/75 shadow-xs transition-colors hover:border-warning/35 hover:bg-base-200"
            >
              <!-- 左侧直接以当前目标打开 -->
              <button
                type="button"
                class="flex min-w-0 items-center gap-1 rounded-l-full py-0.5 pl-2 pr-1.5 text-caption font-medium leading-none transition-colors hover:bg-base-content/10 hover:text-base-content"
                :title="selectedDirectoryOpenTargetTitle"
                :disabled="directoryOpenTargetsLoading"
                @click.stop="handleDirectOpen"
              >
                <img
                  v-if="currentDirectoryOpenTarget.iconDataUrl"
                  :src="currentDirectoryOpenTarget.iconDataUrl"
                  alt=""
                  class="size-3.5 shrink-0 object-contain"
                />
                <SquareTerminal v-else-if="currentDirectoryOpenTarget.type === 'shell'" class="size-3.5 shrink-0 text-warning" />
                <Code2 v-else-if="currentDirectoryOpenTarget.type === 'vscode'" class="size-3.5 shrink-0 text-info" />
                <Folders v-else class="size-3.5 shrink-0 text-warning" />
                <span class="max-w-[62px] truncate">{{ currentDirectoryOpenTarget.label }}</span>
              </button>

              <!-- 细分割线 -->
              <span class="h-3 w-px shrink-0 bg-base-content/15"></span>

              <!-- 右侧弹出完整菜单 -->
              <button
                type="button"
                class="flex shrink-0 items-center rounded-r-full px-1.5 py-0.5 text-base-content/55 transition-colors hover:bg-base-content/10 hover:text-base-content"
                title="切换打开目标"
                :disabled="directoryOpenTargetsLoading"
                @click.stop="toggle"
              >
                <ChevronDown
                  class="size-3 transition-transform duration-150"
                  :class="{ 'rotate-180': dropdownOpen }"
                />
              </button>
            </div>
          </template>
          <template #default="{ close }">
            <DirectoryOpenTargetMenu
              :targets="directoryOpenTargets"
              :selected-kind="selectedDirectoryOpenTargetKind"
              :disabled="directoryOpenTargetsLoading"
              @select="(kind) => handleSelectTarget(kind, close)"
            />
          </template>
        </EcallDropdown>
      </div>
    </div>
  </CardShell>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, Code2, FolderTree, Folders, SquareTerminal } from "@lucide/vue";
import CardShell from "./CardShell.vue";
import EcallDropdown from "../../../shared/components/EcallDropdown.vue";
import DirectoryOpenTargetMenu from "../../../file-reader/components/DirectoryOpenTargetMenu.vue";
import { useDirectoryOpenTargets } from "../../../file-reader/composables/use-directory-open-targets";
import { isMobileTouchViewport } from "../../../shared/utils/mobile-viewport";

const props = withDefaults(defineProps<{
  workspaceRootPath?: string;
}>(), {
  workspaceRootPath: "",
});

const emit = defineEmits<{
  (e: "open"): void;
  (e: "error", message: string): void;
}>();

const { t } = useI18n();

const dropdownOpen = ref(false);
const isMobile = ref(isMobileTouchViewport());

function updateViewportState() {
  isMobile.value = isMobileTouchViewport();
}

const {
  localFileSystemAvailable,
  directoryOpenTargetsLoading,
  selectedDirectoryOpenTargetKind,
  directoryOpenTargets,
  currentDirectoryOpenTarget,
  selectedDirectoryOpenTargetTitle,
  loadDirectoryOpenTargets,
  selectDirectoryOpenTarget,
  openDirectoryWithTarget,
} = useDirectoryOpenTargets();

/** 仅在具有本地文件系统支持、非手机移动端、且工作目录存在时显示外部应用打开胶囊 */
const showOpenActions = computed(() => {
  return localFileSystemAvailable && !isMobile.value && Boolean(props.workspaceRootPath);
});

const name = computed(() => {
  const normalized = String(props.workspaceRootPath || "").replace(/\\/g, "/").replace(/\/+$/, "");
  return normalized.split("/").filter(Boolean).pop() || normalized;
});

async function handleDirectOpen() {
  if (!props.workspaceRootPath) return;
  try {
    await openDirectoryWithTarget(props.workspaceRootPath);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error("[工作目录卡] 打开目录失败", { path: props.workspaceRootPath, error });
    emit("error", message);
  }
}

async function handleSelectTarget(kind: string, close?: () => void) {
  selectDirectoryOpenTarget(kind);
  close?.();
  if (!props.workspaceRootPath) return;
  try {
    await openDirectoryWithTarget(props.workspaceRootPath, kind);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error("[工作目录卡] 打开目录失败", { path: props.workspaceRootPath, error });
    emit("error", message);
  }
}

function handleContextMenu() {
  if (!showOpenActions.value) return;
  dropdownOpen.value = true;
}

onMounted(() => {
  if (typeof window !== "undefined") {
    window.addEventListener("resize", updateViewportState);
  }
  if (!isMobile.value) {
    void loadDirectoryOpenTargets();
  }
});

onBeforeUnmount(() => {
  if (typeof window !== "undefined") {
    window.removeEventListener("resize", updateViewportState);
  }
});
</script>

<style scoped>
/* 样式层兜底：移动端粗指针窄屏下隐藏外部应用打开胶囊 */
@media (pointer: coarse) and (max-width: 768px) {
  .ecall-home-workspace-actions {
    display: none !important;
  }
}
</style>
