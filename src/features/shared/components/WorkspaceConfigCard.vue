<template>
  <div class="flex w-full flex-col gap-2">
    <!-- 文件 + 分支/worktree 同排居中：超宽自动折行居中 -->
    <div class="flex w-full flex-wrap items-center justify-center gap-2">
      <EcallDropdown v-model="mainDropdownOpen" :disabled="availableWorkspaces.length === 0 && !mainPath" :teleport="dropdownTeleport" :teleport-to="dropdownTeleportTo" root-class="min-w-[180px] flex-1" panel-class="w-full">
        <template #trigger="{ toggle, open }">
          <div class="flex h-9 w-full items-center gap-1 rounded-field border border-base-content/10 bg-base-100 pl-3.5 pr-1">
            <FolderOpen class="h-3.5 w-3.5 shrink-0 text-base-content/45" />
            <button
              type="button"
              class="min-w-0 flex-1 cursor-pointer text-left text-xs font-medium text-base-content outline-none"
              :disabled="availableWorkspaces.length === 0 && !mainPath"
              @click="toggle"
            >
              <span class="block w-full truncate" :class="displayMainName ? '' : 'text-base-content/45'">
                {{ displayMainName || t("chat.draftWorkspacePlaceholder") }}
              </span>
            </button>
            <ChevronDown
              class="pointer-events-none h-3.5 w-3.5 shrink-0 text-base-content/45 transition-transform"
              :class="open ? 'rotate-180' : ''"
            />
            <button
              type="button"
              class="btn btn-ghost btn-circle btn-sm shrink-0 text-base-content/55"
              :title="t('common.browse')"
              @click.stop="openMainDirectoryPicker"
            >
              <FolderSearch class="h-3.5 w-3.5" />
            </button>
          </div>
        </template>
        <template #default="{ close }">
          <OverlayScrollArea scroller-class="max-h-60 overscroll-contain" class="p-1">
            <button
              v-for="option in availableWorkspaces"
              :key="option.path"
              type="button"
              class="flex w-full items-center gap-2 rounded-field px-2.5 py-2 text-left text-xs transition-colors"
              :class="isSamePath(option.path, mainPath)
                ? 'bg-base-200 font-medium'
                : 'hover:bg-base-200/70'"
              :title="option.path"
              @click="handleMainSelect(option.path, close)"
            >
              <span class="min-w-0 flex-1 truncate">{{ option.name }}</span>
              <Check
                v-if="isSamePath(option.path, mainPath)"
                class="h-3.5 w-3.5 shrink-0 text-primary"
              />
            </button>
          </OverlayScrollArea>
        </template>
      </EcallDropdown>

      <!-- 工作树与分支一体化双段胶囊：非 git 目录整块不渲染 -->
      <div
        v-if="gitRootAvailable"
        class="flex h-9 shrink-0 items-stretch overflow-hidden rounded-field border border-base-content/10 bg-base-100 transition-colors"
      >
        <!-- 左段：工作树下拉 -->
        <EcallDropdown
          v-model="worktreeDropdownOpen"
          :disabled="worktreeEntries.length === 0 && !selectedWorktreePath"
          :teleport="dropdownTeleport"
          :teleport-to="dropdownTeleportTo"
          :match-trigger-width="false"
          root-class="h-full flex items-center"
          panel-class="w-max max-w-[calc(100vw_-_1.5rem)]"
        >
          <template #trigger="{ toggle, open }">
            <button
              type="button"
              class="flex h-full items-center gap-1.5 px-3 transition-colors hover:bg-base-200/60 disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="worktreeEntries.length === 0 && !selectedWorktreePath"
              :title="t('chat.workspaceWorktreeLabel') + ': ' + (displayWorktreeName || t('chat.workspaceWorktreePlaceholder'))"
              @click="toggle"
            >
              <GitFork class="h-3.5 w-3.5 shrink-0 text-base-content/45" />
              <span class="shrink-0 text-caption font-medium text-base-content/45 select-none">
                {{ t("chat.workspaceWorktreeLabel") }}
              </span>
              <span class="max-w-[100px] truncate text-xs font-medium text-base-content" :class="displayWorktreeName ? '' : 'text-base-content/45'">
                {{ displayWorktreeName || t("chat.workspaceWorktreePlaceholder") }}
              </span>
              <ChevronDown
                class="pointer-events-none h-3 w-3 shrink-0 text-base-content/40 transition-transform"
                :class="open ? 'rotate-180' : ''"
              />
            </button>
          </template>
          <template #default="{ close }">
            <OverlayScrollArea scroller-class="max-h-60 overscroll-contain" class="p-1">
              <button
                v-for="entry in worktreeEntries"
                :key="entry.path"
                type="button"
                class="flex w-full items-center gap-2 rounded-field px-2.5 py-2 text-left text-xs transition-colors"
                :class="isWorktreeEntrySelected(entry)
                  ? 'bg-base-200 font-medium'
                  : 'hover:bg-base-200/70'"
                :title="entry.path"
                @click="handleWorktreeSelect(entry, close)"
              >
                <span class="min-w-0 flex-1 truncate">
                  {{ entry.isMain ? t("chat.worktreeMain") : entry.name }}
                  <span v-if="entry.branch" class="opacity-50">· {{ entry.branch }}</span>
                  <span v-else-if="entry.detached" class="opacity-50">· (detached)</span>
                </span>
                <Check
                  v-if="isWorktreeEntrySelected(entry)"
                  class="h-3.5 w-3.5 shrink-0 text-primary"
                />
              </button>
              <button
                type="button"
                class="flex w-full items-center gap-2 rounded-field px-2.5 py-2 text-left text-xs transition-colors hover:bg-base-200/70"
                @click="handleNewWorktree(close)"
              >
                <FolderPlus class="h-3.5 w-3.5 shrink-0 opacity-60" />
                <span class="min-w-0 flex-1 truncate">{{ t("chat.worktreeNew") }}</span>
              </button>
            </OverlayScrollArea>
          </template>
        </EcallDropdown>

        <!-- 中间细分割线 -->
        <div class="my-auto h-4 w-px bg-base-content/15 shrink-0" />

        <!-- 右段：分支下拉 -->
        <EcallDropdown
          v-model="branchDropdownOpen"
          :disabled="branchSwitchLocked || (branchEntries.length === 0 && !selectedBranch && !isCurrentHeadDetached)"
          :teleport="dropdownTeleport"
          :teleport-to="dropdownTeleportTo"
          :match-trigger-width="false"
          root-class="h-full flex items-center"
          panel-class="w-max max-w-[calc(100vw_-_1.5rem)]"
        >
          <template #trigger="{ toggle, open }">
            <button
              type="button"
              class="flex h-full items-center gap-1.5 px-3 transition-colors hover:bg-base-200/60 disabled:cursor-not-allowed disabled:opacity-60"
              :disabled="branchSwitchLocked || (branchEntries.length === 0 && !selectedBranch && !isCurrentHeadDetached)"
              :title="t('chat.workspaceBranchLabel') + ': ' + (displayBranchName || t('chat.workspaceBranchPlaceholder'))"
              @click="toggle"
            >
              <GitBranch class="h-3.5 w-3.5 shrink-0 text-base-content/45" />
              <span class="shrink-0 text-caption font-medium text-base-content/45 select-none">
                {{ t("chat.workspaceBranchLabel") }}
              </span>
              <span class="max-w-[120px] truncate text-xs font-medium" :class="displayBranchName ? (isCurrentHeadDetached ? 'text-warning font-semibold' : 'text-base-content') : 'text-base-content/45'">
                {{ displayBranchName || (branchLoading ? t("common.loading") : t("chat.workspaceBranchPlaceholder")) }}
              </span>
              <ChevronDown
                class="pointer-events-none h-3 w-3 shrink-0 text-base-content/40 transition-transform"
                :class="open ? 'rotate-180' : ''"
              />
            </button>
          </template>
          <template #default="{ close }">
            <OverlayScrollArea scroller-class="max-h-48 overscroll-contain" class="p-1">
              <GitTree
                v-if="branchTreeNodes.length > 0"
                :nodes="branchTreeNodes"
                default-expanded
                :default-collapsed-keys="['header:remote']"
                @row-click="(row) => onBranchRowClick(row, close)"
              >
                <template #row="{ row }">
                  <template v-if="row.node.data.kind === 'header'">
                    <span class="min-w-0 truncate font-medium opacity-60">{{ row.node.data.text }}</span>
                  </template>
                  <template v-else-if="row.node.data.kind === 'folder'">
                    <Folder class="h-3.5 w-3.5 shrink-0 opacity-60" />
                    <span class="min-w-0 flex-1 truncate font-medium opacity-70">{{ row.node.data.name }}</span>
                  </template>
                  <template v-else>
                    <GitBranch
                      class="h-3.5 w-3.5 shrink-0"
                      :class="row.node.data.isCurrent ? 'text-primary' : 'opacity-60'"
                    />
                    <span class="min-w-0 flex-1 truncate" :class="row.node.data.disabled ? 'opacity-40' : ''">
                      {{ row.node.data.label }}
                    </span>
                    <span
                      v-if="row.node.data.disabled"
                      class="shrink-0 text-caption opacity-50"
                      :title="row.node.data.worktreePath"
                    >
                      {{ t("chat.branchOccupied") }}
                    </span>
                    <span v-else-if="row.node.data.timeText" class="shrink-0 text-caption opacity-50">
                      {{ row.node.data.timeText }}
                    </span>
                    <Check v-if="row.node.data.isSelected" class="h-3.5 w-3.5 shrink-0 text-primary" />
                  </template>
                </template>
              </GitTree>
              <div v-else class="px-2 py-2 text-xs opacity-50">{{ t("gitPanel.noBranches") }}</div>
            </OverlayScrollArea>
          </template>
        </EcallDropdown>
      </div>

    </div>

    <!-- 权限 + 额外目录胶囊 + 添加目录：同排居中，放不下自动折行 -->
    <div class="flex w-full flex-wrap items-center justify-center gap-2">
      <div class="flex shrink-0 items-center rounded-selector border border-base-content/10 bg-base-content/5 p-0.5">
        <button
          v-for="accessOption in ACCESS_OPTIONS"
          :key="accessOption"
          type="button"
          class="rounded-selector px-3.5 py-2 text-xs font-medium leading-none transition-colors"
          :class="access === accessOption
            ? 'bg-base-100 text-base-content'
            : 'text-base-content/55 hover:text-base-content'"
          @click="emit('update:access', accessOption)"
        >
          {{ t(`config.tools.workspaceAccess${ACCESS_LABEL_KEY[accessOption]}`) }}
        </button>
      </div>

      <!-- 额外目录胶囊 + 添加目录：同排，少数同行，放不下自动折到下一行居中 -->
      <div class="flex flex-wrap items-center justify-center gap-1.5">
        <div
          v-for="secPath in secondaryPaths"
          :key="secPath"
          class="group flex max-w-full items-center gap-1 rounded-full border border-base-300 bg-base-100 px-3 py-1.5 text-xs"
          :title="secPath"
        >
          <FolderOpen class="h-3 w-3 shrink-0 text-base-content/45" />
          <span class="max-w-[10rem] truncate font-medium text-base-content">{{ secondaryDisplayName(secPath) }}</span>
          <button
            type="button"
            class="btn btn-ghost btn-xs btn-circle h-5 w-5 min-h-0 p-0 text-base-content/40 hover:text-error"
            :title="t('common.delete')"
            @click="emit('removeSecondary', secPath)"
          >
            <X class="h-3 w-3" />
          </button>
        </div>
        <button
          v-if="!hideAddWorkspace"
          type="button"
          class="flex shrink-0 items-center gap-1 rounded-full border border-dashed border-base-300 bg-base-100 px-3 py-1.5 text-xs font-medium text-base-content/55 transition-colors hover:border-primary/40 hover:bg-primary/10 hover:text-primary"
          :title="t('config.tools.addWorkspace')"
          @click="openSecondaryDirectoryPicker"
        >
          <FolderPlus class="h-3 w-3 shrink-0 opacity-70" />
          <span>{{ t("config.tools.addWorkspace") }}</span>
        </button>
      </div>
    </div>

    <div v-if="gitCheckMessage" class="w-full text-center text-caption leading-tight text-error">
      {{ gitCheckMessage }}
    </div>
    <div v-else-if="checkoutError" class="w-full text-center text-caption leading-tight text-error">
      {{ checkoutError }}
    </div>
    <div v-else-if="branchSwitchLocked" class="w-full text-center text-caption leading-tight text-warning">
      {{ branchLockedReason }}
    </div>

    <!-- 目录选择子弹窗：由主目录浏览或添加额外目录触发 -->
    <WorkspaceDirectoryPickerDialog
      :open="directoryPickerOpen"
      :initial-path="directoryPickerInitialPath"
      @close="directoryPickerOpen = false"
      @select="onDirectoryPicked"
    />

    <!-- 新建工作树子弹窗：基分支 + 名称，确认后立即 git worktree add 并选中新树 -->
    <Teleport :to="dropdownTeleportTo" :disabled="!dropdownTeleport">
      <dialog
        class="modal z-[1100]"
        :class="{ 'modal-open': newWorktreeOpen }"
        :open="newWorktreeOpen"
        @close="newWorktreeOpen = false"
        @cancel.prevent="newWorktreeOpen = false"
        @keydown.esc.prevent="newWorktreeOpen = false"
      >
        <div class="modal-box w-full max-w-md border border-base-content/10 bg-base-100 shadow-2xl">
          <h3 class="text-sm font-semibold">{{ t("chat.worktreeNewTitle") }}</h3>
          <div class="mt-3 flex flex-col gap-3">
            <label class="flex flex-col gap-1">
              <span class="text-xs text-base-content/60">{{ t("chat.worktreeNewBase") }}</span>
              <select v-model="newWorktreeBase" class="select select-bordered select-sm w-full">
                <option v-for="entry in branchEntries.filter((e) => !e.isRemote)" :key="entry.name" :value="entry.name">
                  {{ entry.name }}
                </option>
              </select>
            </label>
            <label class="flex flex-col gap-1">
              <span class="text-xs text-base-content/60">{{ t("chat.worktreeNewName") }}</span>
              <input
                ref="newWorktreeInputRef"
                v-model="newWorktreeName"
                type="text"
                class="input input-bordered input-sm w-full"
                :placeholder="t('chat.worktreeNewNamePlaceholder')"
                @keydown.enter.prevent="confirmNewWorktree"
              />
              <span v-if="newWorktreeName.trim()" class="text-caption text-base-content/40 font-mono truncate block">
                .pai/.worktree/{{ newWorktreeName.trim() }}
              </span>
            </label>
            <div v-if="newWorktreeError" class="rounded-field bg-error/10 px-3 py-2 text-xs text-error">
              {{ newWorktreeError }}
            </div>
          </div>
          <div class="mt-4 flex justify-end gap-2">
            <button class="btn btn-sm btn-ghost" type="button" :disabled="newWorktreeBusy" @click="newWorktreeOpen = false">
              {{ t("common.cancel") }}
            </button>
            <button class="btn btn-sm btn-primary" type="button" :disabled="newWorktreeBusy" @click="confirmNewWorktree">
              {{ newWorktreeBusy ? t("common.saving") : t("common.confirm") }}
            </button>
          </div>
        </div>
        <form method="dialog" class="modal-backdrop">
          <button @click.prevent="newWorktreeOpen = false">close</button>
        </form>
      </dialog>
    </Teleport>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Folder, FolderOpen, FolderSearch, FolderPlus, ChevronDown, Check, GitBranch, GitFork, X } from "@lucide/vue";
import EcallDropdown from "./EcallDropdown.vue";
import OverlayScrollArea from "./OverlayScrollArea.vue";
import WorkspaceDirectoryPickerDialog from "./WorkspaceDirectoryPickerDialog.vue";
import GitTree, { type GitTreeFlatRow, type GitTreeNode } from "../../file-reader/components/GitTree.vue";
import { buildBranchTree, sortBranchesForDisplay, type BranchTreeNode } from "../../file-reader/git-branch-order";
import { formatRecentRelativeTime } from "../utils/relative-time";
import type { ShellWorkspaceAccess, ShellWorkMode } from "../../../types/app";
import {
  gitPanelBranchList,
  gitPanelCheckout,
  gitPanelCheckoutCheck,
  gitPanelHeadState,
  gitPanelWorktrees,
  gitPanelWorktreeAdd,
  type GitPanelBranchEntry,
  type GitPanelWorktreeEntry,
} from "../../../services/tauri-api";
import {
  defaultWorkspaceNameFromPath,
  normalizeWorkspacePathKey,
  stripExtendedPathPrefix,
} from "../../../utils/shell-workspaces";

type WorkspaceOption = {
  id: string;
  name: string;
  path: string;
  access: ShellWorkspaceAccess;
};

const ACCESS_OPTIONS = ["approval", "full_access"] as const;
const ACCESS_LABEL_KEY: Record<ShellWorkspaceAccess, string> = {
  approval: "Approval",
  full_access: "FullAccess",
};

const props = withDefaults(defineProps<{
  mainPath: string;
  secondaryPaths?: string[];
  access: ShellWorkspaceAccess;
  workMode?: ShellWorkMode;
  selectedBranch?: string;
  /** 当前选中的工作树路径（主树=仓库根/空串）；空串表示跟随主工作树 */
  selectedWorktreePath?: string;
  availableWorkspaces?: WorkspaceOption[];
  hideAddWorkspace?: boolean;
  dropdownTeleport?: boolean;
  dropdownTeleportTo?: string;
  /** 检出分支后同步「本会话工作分支」记录 */
  syncWorkspaceBranch?: (workspacePath: string) => Promise<void>;
  gitRootCheck?: (path: string) => Promise<boolean>;
}>(), {
  secondaryPaths: () => [],
  workMode: "directory",
  selectedBranch: "",
  selectedWorktreePath: "",
  availableWorkspaces: () => [],
  hideAddWorkspace: false,
  dropdownTeleport: true,
  dropdownTeleportTo: "body",
});

const emit = defineEmits<{
  (e: "update:mainPath", value: string): void;
  (e: "update:access", value: ShellWorkspaceAccess): void;
  (e: "update:workMode", value: ShellWorkMode): void;
  (e: "update:branch", value: string): void;
  /** 选中某个已有工作树（主树或链接树）：调用方据此绑定工作树目录 */
  (e: "update:worktreePath", payload: { worktreePath: string; branch: string; isMain: boolean }): void;
  (e: "browseMain"): void;
  (e: "addSecondary", path: string): void;
  (e: "removeSecondary", path: string): void;
}>();

const { t } = useI18n();

const mainDropdownOpen = ref(false);
const worktreeDropdownOpen = ref(false);
const branchDropdownOpen = ref(false);

const gitRootAvailable = ref(false);
const gitCheckMessage = ref("");
const checkoutError = ref("");
const branchLoading = ref(false);
const rebaseInProgress = ref(false);
const worktreeEntries = ref<GitPanelWorktreeEntry[]>([]);
const branchEntries = ref<GitPanelBranchEntry[]>([]);
const worktreeBranchMap = ref<Record<string, string>>({});

function isSamePath(a?: string, b?: string): boolean {
  const normA = normalizeWorkspacePathKey(a);
  const normB = normalizeWorkspacePathKey(b);
  if (!normA || !normB) return false;
  return normA === normB;
}

/** 分支操作的目标目录：选中的工作树（未选或为主树时回退到主目录） */
const effectiveTargetWorktreePath = computed(() => {
  const selected = stripExtendedPathPrefix(String(props.selectedWorktreePath || "").trim());
  const main = stripExtendedPathPrefix(String(props.mainPath || "").trim());
  if (selected && !isSamePath(selected, main)) {
    return selected;
  }
  return main;
});

/** 当前工作树是否处于 detached HEAD（游离 HEAD）状态 */
const isCurrentHeadDetached = computed(() => {
  const target = effectiveTargetWorktreePath.value;
  if (!target || worktreeEntries.value.length === 0) return false;
  const entry = worktreeEntries.value.find((e) => isSamePath(e.path, target));
  if (entry) {
    return Boolean(entry.detached);
  }
  return branchEntries.value.length > 0 && !branchEntries.value.some((b) => b.isCurrent);
});

/** 当前分支名称展示：游离 HEAD 明确标出 (detached) */
const displayBranchName = computed(() => {
  if (isCurrentHeadDetached.value) {
    return "(detached)";
  }
  return String(props.selectedBranch || "").trim();
});

/** 工作树下拉触发器显示：主树显示「主工作树」，链接树显示目录名 */
const displayWorktreeName = computed(() => {
  const selected = stripExtendedPathPrefix(String(props.selectedWorktreePath || "").trim());
  const main = stripExtendedPathPrefix(String(props.mainPath || "").trim());
  const entries = worktreeEntries.value || [];
  if (entries.length === 0) return "";
  
  if (!selected || isSamePath(selected, main)) {
    const mainEntry = entries.find((e) => e.isMain || isSamePath(e.path, main));
    return mainEntry?.isMain ? t("chat.worktreeMain") : (mainEntry?.name || t("chat.worktreeMain"));
  }
  
  const entry = entries.find((item) => isSamePath(item.path, selected));
  if (entry) {
    return entry.isMain ? t("chat.worktreeMain") : entry.name;
  }
  return defaultWorkspaceNameFromPath(selected) || selected;
});

function isWorktreeEntrySelected(entry: GitPanelWorktreeEntry): boolean {
  const selected = stripExtendedPathPrefix(String(props.selectedWorktreePath || "").trim());
  const main = stripExtendedPathPrefix(String(props.mainPath || "").trim());
  if (!selected || isSamePath(selected, main)) {
    return entry.isMain || isSamePath(entry.path, main);
  }
  return isSamePath(entry.path, selected);
}

function handleWorktreeSelect(entry: GitPanelWorktreeEntry, close: () => void) {
  close();
  checkoutError.value = "";
  const isMain = Boolean(entry.isMain || isSamePath(entry.path, props.mainPath));
  const worktreePath = isMain ? "" : entry.path;
  const workMode: ShellWorkMode = isMain ? "directory" : "worktree";
  const branch = String(entry.branch || "").trim();

  emit("update:worktreePath", {
    worktreePath: entry.path,
    branch,
    isMain,
  });
  emit("update:workMode", workMode);
  if (branch) {
    emit("update:branch", branch);
  } else {
    emit("update:branch", "");
  }
  void nextTick(() => {
    void loadBranches(entry.path);
  });
}

const displayMainName = computed(() => {
  const path = stripExtendedPathPrefix(String(props.mainPath || "").trim());
  if (!path) return "";
  const found = props.availableWorkspaces.find((opt) => isSamePath(opt.path, path));
  if (found) return found.name;
  return defaultWorkspaceNameFromPath(path) || path;
});

/** 分支锁定原因非空时，分支下拉整体禁用并显示原因 */
const branchSwitchLocked = computed(() => Boolean(String(branchLockedReason.value || "").trim()));
const branchLockedReason = computed(() => (rebaseInProgress.value ? t("chat.workspaceBranchRebasing") : ""));

function secondaryDisplayName(path: string): string {
  const normalized = stripExtendedPathPrefix(String(path || "").trim());
  const found = props.availableWorkspaces.find((opt) => isSamePath(opt.path, normalized));
  if (found) return found.name;
  return defaultWorkspaceNameFromPath(normalized) || normalized;
}

function handleMainSelect(path: string, close: () => void) {
  close();
  void nextTick(() => emit("update:mainPath", path));
}

// ========== Git 状态加载与切换 ==========

async function refreshRebaseState(path: string) {
  const normalized = stripExtendedPathPrefix(String(path || "").trim());
  if (!normalized) {
    rebaseInProgress.value = false;
    return;
  }
  try {
    const state = await gitPanelHeadState(normalized);
    rebaseInProgress.value = Boolean(state?.rebaseInProgress);
  } catch {
    rebaseInProgress.value = false;
  }
}

async function loadWorktrees(repoRoot: string) {
  const normalized = stripExtendedPathPrefix(String(repoRoot || "").trim());
  if (!normalized) {
    worktreeEntries.value = [];
    worktreeBranchMap.value = {};
    return;
  }
  try {
    const result = await gitPanelWorktrees(normalized, normalized);
    const entries = result.worktrees || [];
    worktreeEntries.value = entries;
    const map: Record<string, string> = {};
    for (const entry of entries) {
      // 主树占用的分支同样算占用，避免在链接工作树内切成主树当前分支引发冲突
      const branch = String(entry.branch || "").trim().toLowerCase();
      const worktreePath = String(entry.path || "").trim();
      if (!branch || !worktreePath) continue;
      map[branch] = worktreePath;
    }
    worktreeBranchMap.value = map;
  } catch (error) {
    console.warn("[工作区] 获取工作树列表失败:", { path: normalized, error });
    worktreeEntries.value = [];
    worktreeBranchMap.value = {};
  }
}

let branchSeq = 0;
async function loadBranches(targetPath: string) {
  const seq = ++branchSeq;
  const normalized = stripExtendedPathPrefix(String(targetPath || "").trim());
  if (!normalized) {
    branchEntries.value = [];
    checkoutError.value = "";
    return;
  }
  branchLoading.value = true;
  await refreshRebaseState(normalized);
  try {
    const entries = await gitPanelBranchList(normalized);
    if (seq !== branchSeq) return;
    branchEntries.value = entries;
    const current = entries.find((e) => e.isCurrent)?.name;
    const currentName = String(current || "").trim();
    if (currentName) {
      if (currentName.toLowerCase() !== String(props.selectedBranch || "").trim().toLowerCase()) {
        emit("update:branch", currentName);
      }
    } else if (entries.some((e) => e.isCurrent === false)) {
      // 游离 HEAD：没有 current 分支，清空选中分支名避免残留旧分支
      if (String(props.selectedBranch || "").trim()) {
        emit("update:branch", "");
      }
    }
  } catch (error) {
    if (seq !== branchSeq) return;
    branchEntries.value = [];
    console.warn("[工作区] 加载分支失败:", { path: normalized, error });
  } finally {
    if (seq === branchSeq) branchLoading.value = false;
  }
}

let gitCheckSeq = 0;
async function runGitRootCheck(main: string) {
  const seq = ++gitCheckSeq;
  const normalized = stripExtendedPathPrefix(String(main || "").trim());
  if (!normalized) {
    gitRootAvailable.value = false;
    gitCheckMessage.value = "";
    checkoutError.value = "";
    worktreeEntries.value = [];
    branchEntries.value = [];
    return;
  }
  checkoutError.value = "";
  let available = false;
  try {
    if (props.gitRootCheck) {
      available = await props.gitRootCheck(normalized);
    } else {
      const entries = await gitPanelBranchList(normalized);
      available = entries.length > 0;
    }
  } catch {
    available = false;
  }
  if (seq !== gitCheckSeq) return;
  gitRootAvailable.value = available;
  if (available) {
    await loadWorktrees(normalized);
    await loadBranches(effectiveTargetWorktreePath.value);
  } else {
    worktreeEntries.value = [];
    branchEntries.value = [];
    worktreeBranchMap.value = {};
  }
}

// ========== 分支树节点构建 ==========

type BranchRowData =
  | { kind: "header"; text: string }
  | { kind: "folder"; name: string }
  | {
      kind: "branch";
      name: string;
      label: string;
      isCurrent: boolean;
      timeText: string;
      worktreePath: string;
      disabled: boolean;
      isSelected: boolean;
    };

function occupiedWorktreePathOf(branchName: string): string {
  const map = worktreeBranchMap.value || {};
  const occupied = String(map[String(branchName || "").trim().toLowerCase()] || "");
  if (!occupied) return "";
  // 当前生效的目标工作树检出该分支是正常状态，不算占用
  const currentTarget = effectiveTargetWorktreePath.value;
  if (isSamePath(occupied, currentTarget)) return "";
  return occupied;
}

function toGitTreeNodes(
  nodes: BranchTreeNode<GitPanelBranchEntry>[],
  scope: "local" | "remote",
  nowMs: number,
): GitTreeNode<BranchRowData>[] {
  const selected = String(props.selectedBranch || "").trim().toLowerCase();
  return nodes.map((node) => {
    if (node.kind === "branch") {
      const entry = node.branch;
      const occupiedPath = occupiedWorktreePathOf(entry.name);
      return {
        key: `${scope}:${entry.name}`,
        title: entry.name,
        interactive: !occupiedPath,
        data: {
          kind: "branch" as const,
          name: entry.name,
          label: node.label,
          isCurrent: Boolean(entry.isCurrent),
          timeText: formatRecentRelativeTime(entry.committerDate, nowMs, t),
          worktreePath: occupiedPath,
          disabled: Boolean(occupiedPath),
          isSelected: entry.name.trim().toLowerCase() === selected,
        },
      };
    }
    return {
      key: `folder:${scope}:${node.path}`,
      interactive: false,
      data: { kind: "folder" as const, name: node.name },
      children: toGitTreeNodes(node.children, scope, nowMs),
    };
  });
}

const branchTreeNodes = computed<GitTreeNode<BranchRowData>[]>(() => {
  const entries = branchEntries.value || [];
  if (entries.length === 0) return [];
  const nowMs = Date.now();
  const roots: GitTreeNode<BranchRowData>[] = [];
  const local = entries.filter((entry) => !entry.isRemote);
  const remote = entries.filter((entry) => entry.isRemote);
  if (local.length > 0) {
    roots.push({
      key: "header:local",
      interactive: false,
      data: { kind: "header", text: t("gitPanel.localBranches") },
      children: toGitTreeNodes(buildBranchTree(sortBranchesForDisplay(local)), "local", nowMs),
    });
  }
  if (remote.length > 0) {
    roots.push({
      key: "header:remote",
      interactive: false,
      data: { kind: "header", text: t("gitPanel.remoteBranches") },
      children: toGitTreeNodes(buildBranchTree(sortBranchesForDisplay(remote)), "remote", nowMs),
    });
  }
  return roots;
});

async function onBranchRowClick(row: GitTreeFlatRow<BranchRowData>, close: () => void) {
  const data = row.node.data;
  if (data.kind !== "branch" || data.disabled) return;
  close();
  const normalized = String(data.name || "").trim();
  if (!normalized) return;
  if (normalized.toLowerCase() === String(props.selectedBranch || "").trim().toLowerCase()) return;

  const target = effectiveTargetWorktreePath.value;
  if (!gitRootAvailable.value || !target) {
    emit("update:branch", normalized);
    return;
  }

  branchLoading.value = true;
  checkoutError.value = "";
  try {
    const check = await gitPanelCheckoutCheck(target, normalized);
    const dirtyPaths: string[] = (check as unknown as { dirtyPaths: string[] }).dirtyPaths || [];
    if (Array.isArray(dirtyPaths) && dirtyPaths.length > 0) {
      const preview = dirtyPaths.slice(0, 3).join(", ");
      const more = dirtyPaths.length > 3 ? t("chat.workspaceBranchDirtyMore", { count: dirtyPaths.length - 3 }) : "";
      const detail = preview ? t("chat.workspaceBranchDirtyDetail", { preview, more }) : "";
      checkoutError.value = t("chat.workspaceBranchDirtyBlocked", { detail });
      return;
    }
    const result = await gitPanelCheckout(target, normalized);
    if (result.exitCode !== 0) {
      const message = (result.stderr || result.stdout || "").trim() || "切换分支失败";
      checkoutError.value = t("chat.workspaceBranchCheckoutFailed", { message });
      return;
    }
    // 会话内自己切的分支：按目标目录同步记录
    await props.syncWorkspaceBranch?.(target);
    emit("update:branch", normalized);
    // 重新获取分支列表与工作树列表
    await loadBranches(target);
    await loadWorktrees(props.mainPath);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    checkoutError.value = t("chat.workspaceBranchCheckFailed", { message });
  } finally {
    branchLoading.value = false;
  }
}

// ========== 新建工作树子弹窗 ==========

const newWorktreeOpen = ref(false);
const newWorktreeInputRef = ref<HTMLInputElement | null>(null);
const newWorktreeName = ref("");
const newWorktreeBase = ref("");
const newWorktreeBusy = ref(false);
const newWorktreeError = ref("");

function handleNewWorktree(close: () => void) {
  close();
  newWorktreeName.value = "";
  newWorktreeBase.value = String(props.selectedBranch || "").trim() || (branchEntries.value.find((b) => !b.isRemote)?.name || "");
  newWorktreeError.value = "";
  newWorktreeOpen.value = true;
  void nextTick(() => {
    newWorktreeInputRef.value?.focus();
  });
}

function resolveMainRepoRoot(path: string): string {
  const normalized = stripExtendedPathPrefix(String(path || "").trim()).replace(/[\\/]+$/, "");
  if (!normalized) return "";
  const lower = normalized.replace(/\\/g, "/").toLowerCase();
  for (const marker of ["/.pai/.worktree/", "/.pai/worktree/"]) {
    const idx = lower.indexOf(marker);
    if (idx !== -1) {
      return normalized.slice(0, idx);
    }
  }
  return normalized;
}

function newWorktreeTargetPath(name: string): string {
  const rawMain = stripExtendedPathPrefix(String(props.mainPath || "").trim());
  const main = resolveMainRepoRoot(rawMain);
  if (!main) return "";
  const sep = main.includes("\\") ? "\\" : "/";
  const suffix = String(name || "").trim();
  return `${main}${sep}.pai${sep}.worktree${sep}${suffix}`;
}

async function confirmNewWorktree() {
  const name = String(newWorktreeName.value || "").trim();
  const base = String(newWorktreeBase.value || "").trim();
  if (!name || !base) {
    newWorktreeError.value = t("chat.worktreeNewMissing");
    return;
  }
  const repoRoot = resolveMainRepoRoot(stripExtendedPathPrefix(String(props.mainPath || "").trim()));
  const path = newWorktreeTargetPath(name);
  if (!repoRoot || !path) {
    newWorktreeError.value = t("chat.worktreeNewFailed", { message: "无法推导工作树路径" });
    return;
  }
  newWorktreeBusy.value = true;
  newWorktreeError.value = "";
  try {
    const result = await gitPanelWorktreeAdd({
      repoRoot,
      path,
      branch: name,
      baseBranch: base,
      checkoutExisting: false,
    });
    newWorktreeOpen.value = false;
    await loadWorktrees(repoRoot);
    emit("update:worktreePath", {
      worktreePath: result.path,
      branch: result.branch,
      isMain: false,
    });
    emit("update:workMode", "worktree");
    emit("update:branch", result.branch);
    void loadBranches(result.path);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    newWorktreeError.value = t("chat.worktreeNewFailed", { message });
  } finally {
    newWorktreeBusy.value = false;
  }
}

// ========== 目录选择子弹窗 ==========

const directoryPickerOpen = ref(false);
const directoryPickerMode = ref<"main" | "secondary">("main");
const directoryPickerInitialPath = ref("");

function openMainDirectoryPicker() {
  emit("browseMain");
  directoryPickerMode.value = "main";
  directoryPickerInitialPath.value = stripExtendedPathPrefix(String(props.mainPath || "").trim());
  directoryPickerOpen.value = true;
}

function openSecondaryDirectoryPicker() {
  directoryPickerMode.value = "secondary";
  directoryPickerInitialPath.value = stripExtendedPathPrefix(String(props.mainPath || "").trim());
  directoryPickerOpen.value = true;
}

function onDirectoryPicked(pickedPath: string) {
  const path = stripExtendedPathPrefix(String(pickedPath || "").trim());
  directoryPickerOpen.value = false;
  if (!path) return;
  if (directoryPickerMode.value === "main") {
    handleMainSelect(path, () => {});
  } else {
    emit("addSecondary", path);
  }
}

// ========== 监听 ==========

watch(
  () => props.mainPath,
  (next) => {
    mainDropdownOpen.value = false;
    worktreeDropdownOpen.value = false;
    branchDropdownOpen.value = false;
    void runGitRootCheck(next);
  },
  { immediate: true },
);

watch(
  () => effectiveTargetWorktreePath.value,
  (nextTarget, prevTarget) => {
    worktreeDropdownOpen.value = false;
    if (gitRootAvailable.value && nextTarget && !isSamePath(nextTarget, prevTarget)) {
      void loadBranches(nextTarget);
    }
  },
);

watch(
  () => props.selectedBranch,
  () => {
    branchDropdownOpen.value = false;
  },
);
</script>
