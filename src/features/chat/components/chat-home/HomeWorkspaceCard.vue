<template>
  <CardShell
    layout="tile"
    class="group relative"
    :tone="currentTone"
    :icon="currentIcon"
    :label="currentLabel"
    interactive
    @select="handleCardSelect"
    @contextmenu.prevent.stop="handleContextMenu"
  >
    <!-- 右上角切换胶囊：工作树模式下在工作目录和工作树所在目录之间切换 -->
    <button
      v-if="canToggleWorktree"
      type="button"
      class="ecall-home-workspace-toggle absolute right-1.5 top-1.5 z-10 flex h-5 max-w-[80px] items-center gap-1 rounded-full border border-base-content/10 bg-base-200/90 px-1.5 text-caption font-medium text-base-content/70 shadow-2xs backdrop-blur-xs transition-all hover:border-base-content/25 hover:bg-base-300 hover:text-base-content active:scale-95"
      :title="switchTooltip"
      @click.stop="toggleTarget"
      @keydown.stop
    >
      <ArrowLeftRight class="size-2.5 shrink-0 opacity-70" />
      <span class="truncate">{{ nextTargetButtonText }}</span>
    </button>

    <div class="flex w-full min-w-0 flex-col items-center gap-1.5">
      <!-- 工作目录 / 工作树名称 -->
      <span
        class="w-full truncate text-xs text-base-content/50 transition-colors group-hover:text-base-content/75"
        :title="currentPath"
      >
        {{ currentName }}
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
              class="inline-flex max-w-full items-center rounded-full border border-base-content/10 bg-base-200/80 p-0.5 text-xs text-base-content/75 shadow-xs transition-colors hover:bg-base-200"
              :class="isWorktreeTarget ? 'hover:border-secondary/35' : 'hover:border-warning/35'"
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
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowLeftRight, ChevronDown, Code2, FolderTree, Folders, GitFork, SquareTerminal } from "@lucide/vue";
import type { ShellWorkMode } from "../../../../types/app";
import CardShell from "./CardShell.vue";
import EcallDropdown from "../../../shared/components/EcallDropdown.vue";
import DirectoryOpenTargetMenu from "../../../file-reader/components/DirectoryOpenTargetMenu.vue";
import { useDirectoryOpenTargets } from "../../../file-reader/composables/use-directory-open-targets";
import { isMobileTouchViewport } from "../../../shared/utils/mobile-viewport";

const props = withDefaults(defineProps<{
  workspaceRootPath?: string;
  worktreePath?: string;
  worktreeBranch?: string;
  workMode?: ShellWorkMode;
}>(), {
  workspaceRootPath: "",
  worktreePath: "",
  worktreeBranch: "",
  workMode: "directory",
});

const emit = defineEmits<{
  (e: "open", path?: string): void;
  (e: "error", message: string): void;
}>();

const { t } = useI18n();

const dropdownOpen = ref(false);
const isMobile = ref(isMobileTouchViewport());

function updateViewportState() {
  isMobile.value = isMobileTouchViewport();
}

function normalizePath(p?: string) {
  return String(p || "").trim().replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
}

/** 仅在处于工作树模式且工作树路径有效且与主工作区不同时，才允许切换 */
const canToggleWorktree = computed(() => {
  if (props.workMode !== "worktree") return false;
  const main = normalizePath(props.workspaceRootPath);
  const wt = normalizePath(props.worktreePath);
  return Boolean(main && wt && main !== wt);
});

type TargetKind = "workspace" | "worktree";
const activeTarget = ref<TargetKind>("worktree");

watch(
  canToggleWorktree,
  (canToggle) => {
    if (canToggle) {
      activeTarget.value = "worktree";
    }
  },
  { immediate: true },
);

const effectiveTarget = computed<TargetKind>(() => {
  return canToggleWorktree.value ? activeTarget.value : "workspace";
});

const isWorktreeTarget = computed(() => effectiveTarget.value === "worktree");

function toggleTarget() {
  activeTarget.value = isWorktreeTarget.value ? "workspace" : "worktree";
}

const workspaceName = computed(() => {
  const normalized = String(props.workspaceRootPath || "").replace(/\\/g, "/").replace(/\/+$/, "");
  return normalized.split("/").filter(Boolean).pop() || normalized;
});

const worktreeName = computed(() => {
  if (props.worktreeBranch) {
    return props.worktreeBranch;
  }
  const normalized = String(props.worktreePath || "").replace(/\\/g, "/").replace(/\/+$/, "");
  return normalized.split("/").filter(Boolean).pop() || normalized;
});

const currentPath = computed(() => {
  return isWorktreeTarget.value ? (props.worktreePath || props.workspaceRootPath) : props.workspaceRootPath;
});

const currentName = computed(() => {
  return isWorktreeTarget.value ? worktreeName.value : workspaceName.value;
});

const currentLabel = computed(() => {
  return isWorktreeTarget.value ? t("chat.homePanel.worktree") : t("chat.homePanel.workspace");
});

const currentIcon = computed(() => {
  return isWorktreeTarget.value ? GitFork : FolderTree;
});

const currentTone = computed<"warning" | "secondary">(() => {
  return isWorktreeTarget.value ? "secondary" : "warning";
});

const nextTargetButtonText = computed(() => {
  return isWorktreeTarget.value ? t("chat.homePanel.workspace") : t("chat.homePanel.worktree");
});

const switchTooltip = computed(() => {
  return isWorktreeTarget.value
    ? t("chat.homePanel.switchToWorkspace", { name: workspaceName.value })
    : t("chat.homePanel.switchToWorktree", { name: worktreeName.value });
});

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

/** 仅在具有本地文件系统支持、非手机移动端、且当前目录存在时显示外部应用打开胶囊 */
const showOpenActions = computed(() => {
  return localFileSystemAvailable && !isMobile.value && Boolean(currentPath.value);
});

function handleCardSelect() {
  emit("open", currentPath.value);
}

async function handleDirectOpen() {
  if (!currentPath.value) return;
  try {
    await openDirectoryWithTarget(currentPath.value);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error("[工作目录卡] 打开目录失败", { path: currentPath.value, error });
    emit("error", message);
  }
}

async function handleSelectTarget(kind: string, close?: () => void) {
  selectDirectoryOpenTarget(kind);
  close?.();
  if (!currentPath.value) return;
  try {
    await openDirectoryWithTarget(currentPath.value, kind);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error("[工作目录卡] 打开目录失败", { path: currentPath.value, error });
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
