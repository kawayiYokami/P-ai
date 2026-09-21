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
              @click.stop="emit('browseMain')"
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
              :class="option.path.toLowerCase() === mainPath.toLowerCase()
                ? 'bg-base-200 font-medium'
                : 'hover:bg-base-200/70'"
              :title="option.path"
              @click="handleMainSelect(option.path, close)"
            >
              <span class="min-w-0 flex-1 truncate">{{ option.name }}</span>
              <Check
                v-if="option.path.toLowerCase() === mainPath.toLowerCase()"
                class="h-3.5 w-3.5 shrink-0 text-primary"
              />
            </button>
          </OverlayScrollArea>
        </template>
      </EcallDropdown>

      <!-- 分支 + worktree：join 在一起为一组，共享外边框，中间细分隔；不可用时禁用而非隐藏，避免切目录时整行跳动 -->
      <div
        class="flex shrink-0 items-center overflow-hidden rounded-field border bg-base-100 transition-colors"
        :class="gitRootAvailable ? 'border-base-content/10' : 'border-base-content/10 opacity-60'"
      >
        <EcallDropdown
          v-model="branchDropdownOpen"
          :disabled="branchSwitchLocked || !gitRootAvailable || (branchEntries.length === 0 && !selectedBranch)"
          :teleport="dropdownTeleport"
          :teleport-to="dropdownTeleportTo"
          :match-trigger-width="false"
          root-class="min-w-[112px] max-w-[180px]"
          panel-class="w-max max-w-[calc(100vw_-_1.5rem)]"
        >
          <template #trigger="{ toggle, open }">
            <div class="flex h-8 items-center gap-1 bg-transparent pl-3 pr-2">
              <GitBranch class="h-3.5 w-3.5 shrink-0 text-base-content/45" />
              <button
                type="button"
                class="min-w-0 flex-1 cursor-pointer text-left text-xs font-medium text-base-content outline-none disabled:cursor-not-allowed disabled:opacity-60"
                :disabled="branchSwitchLocked || !gitRootAvailable || (branchEntries.length === 0 && !selectedBranch)"
                @click="toggle"
              >
                <span class="block w-full truncate" :class="displayBranchName ? '' : 'text-base-content/45'">
                  {{ displayBranchName || (branchLoading ? t("common.loading") : (gitRootAvailable ? t("chat.workspaceBranchPlaceholder") : "非 Git 目录")) }}
                </span>
              </button>
              <ChevronDown
                class="pointer-events-none h-3.5 w-3.5 shrink-0 text-base-content/45 transition-transform"
                :class="open ? 'rotate-180' : ''"
              />
            </div>
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
                    <span class="min-w-0 flex-1 truncate">{{ row.node.data.label }}</span>
                    <span
                      v-if="row.node.data.worktreePath"
                      class="shrink-0 rounded-full bg-primary/10 px-1.5 py-0.5 text-caption text-primary"
                      :title="row.node.data.worktreePath"
                    >
                      {{ t("chat.draftWorkModeWorktree") }}
                    </span>
                    <span v-if="row.node.data.timeText" class="shrink-0 text-caption opacity-50">
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
        <div class="h-5 w-px shrink-0 bg-base-content/10"></div>
        <label class="flex h-8 shrink-0 select-none items-center gap-1.5 bg-transparent px-2.5" :class="gitRootAvailable ? 'cursor-pointer' : 'cursor-not-allowed opacity-60'">
          <input
            type="checkbox"
            class="checkbox checkbox-primary checkbox-xs h-3.5 w-3.5 rounded-[4px] disabled:cursor-not-allowed"
            :checked="workMode === 'worktree'"
            :disabled="!gitRootAvailable"
            @change="handleWorktreeChecked"
          />
          <span class="text-xs font-medium leading-none" :class="gitRootAvailable ? 'text-base-content/80' : 'text-base-content/50'">{{ t("chat.draftWorkModeWorktree") }}</span>
        </label>
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
          @click="emit('addSecondary')"
        >
          <FolderPlus class="h-3 w-3 shrink-0 opacity-70" />
          <span>{{ t("config.tools.addWorkspace") }}</span>
        </button>
      </div>
    </div>

    <div v-if="gitCheckMessage" class="w-full text-center text-caption leading-tight text-error">
      {{ gitCheckMessage }}
    </div>
    <div v-else-if="branchSwitchLocked" class="w-full text-center text-caption leading-tight text-warning">
      {{ branchLockedReason }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Folder, FolderOpen, FolderSearch, FolderPlus, ChevronDown, Check, GitBranch, X } from "@lucide/vue";
import EcallDropdown from "./EcallDropdown.vue";
import OverlayScrollArea from "./OverlayScrollArea.vue";
import GitTree, { type GitTreeFlatRow, type GitTreeNode } from "../../file-reader/components/GitTree.vue";
import { buildBranchTree, sortBranchesForDisplay, type BranchTreeNode } from "../../file-reader/git-branch-order";
import { formatRecentRelativeTime } from "../utils/relative-time";
import type { ShellWorkspaceAccess, ShellWorkMode } from "../../../types/app";
import type { GitPanelBranchEntry } from "../../../services/tauri-api";
import { defaultWorkspaceNameFromPath } from "../../../utils/shell-workspaces";

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
  workMode: ShellWorkMode;
  selectedBranch?: string;
  /** 完整分支数据：含当前标记与末次提交时间，用于分类排序与时间展示 */
  branchEntries?: GitPanelBranchEntry[];
  /** 分支名（小写）→ 该分支已检出的工作树路径；命中表示该分支已被工作树占用 */
  worktreeBranchMap?: Record<string, string>;
  branchLoading?: boolean;
  gitRootAvailable?: boolean;
  gitCheckMessage?: string;
  /** 非空表示当前不允许切换分支，内容为禁用原因（如变基进行中） */
  branchLockedReason?: string;
  availableWorkspaces?: WorkspaceOption[];
  hideAddWorkspace?: boolean;
  dropdownTeleport?: boolean;
  dropdownTeleportTo?: string;
}>(), {
  secondaryPaths: () => [],
  selectedBranch: "",
  branchEntries: () => [],
  worktreeBranchMap: () => ({}),
  branchLoading: false,
  gitRootAvailable: false,
  gitCheckMessage: "",
  branchLockedReason: "",
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
  /** 选中已被工作树检出的分支：调用方据此把会话直接绑定到该工作树目录 */
  (e: "selectWorktree", payload: { branch: string; worktreePath: string }): void;
  (e: "browseMain"): void;
  (e: "addSecondary"): void;
  (e: "removeSecondary", path: string): void;
}>();

const { t } = useI18n();

const mainDropdownOpen = ref(false);
const branchDropdownOpen = ref(false);

const displayMainName = computed(() => {
  const path = String(props.mainPath || "").trim();
  if (!path) return "";
  const found = props.availableWorkspaces.find((opt) => opt.path.toLowerCase() === path.toLowerCase());
  if (found) return found.name;
  return defaultWorkspaceNameFromPath(path) || path;
});

const displayBranchName = computed(() => {
  return String(props.selectedBranch || "").trim();
});

/** 分支锁定原因非空时，分支下拉整体禁用并显示原因 */
const branchSwitchLocked = computed(() => Boolean(String(props.branchLockedReason || "").trim()));

function secondaryDisplayName(path: string): string {
  const normalized = String(path || "").trim();
  const found = props.availableWorkspaces.find((opt) => opt.path.toLowerCase() === normalized.toLowerCase());
  if (found) return found.name;
  return defaultWorkspaceNameFromPath(normalized) || normalized;
}

function handleMainSelect(path: string, close: () => void) {
  // 先关闭下拉（让固定定位面板开始离场），再在下一帧更新文本，避免触发宽度变化时面板重新锚定到新的文本位置而瞬移到右边
  close();
  void nextTick(() => emit("update:mainPath", path));
}

/** 分支行数据：树里三种行共用一个联合类型 */
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
      isSelected: boolean;
    };

/** 分支名（小写）→ 已检出的工作树路径 */
function worktreePathOf(branchName: string): string {
  const map = props.worktreeBranchMap || {};
  return String(map[String(branchName || "").trim().toLowerCase()] || "");
}

/** 分类算法输出 → GitTree 节点；scope 决定 key 命名空间，避免本地/远程同名冲突 */
function toGitTreeNodes(
  nodes: BranchTreeNode<GitPanelBranchEntry>[],
  scope: "local" | "remote",
  nowMs: number,
): GitTreeNode<BranchRowData>[] {
  const selected = String(props.selectedBranch || "").trim().toLowerCase();
  return nodes.map((node) => {
    if (node.kind === "branch") {
      const entry = node.branch;
      return {
        key: `${scope}:${entry.name}`,
        title: entry.name,
        data: {
          kind: "branch" as const,
          name: entry.name,
          label: node.label,
          isCurrent: Boolean(entry.isCurrent),
          timeText: formatRecentRelativeTime(entry.committerDate, nowMs, t),
          worktreePath: worktreePathOf(entry.name),
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

/** 分支下拉内容：本地/远程两组，组内按「时间倒序 + `/` 折树」，与 Git 面板同一套算法 */
const branchTreeNodes = computed<GitTreeNode<BranchRowData>[]>(() => {
  const entries = props.branchEntries || [];
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

function onBranchRowClick(row: GitTreeFlatRow<BranchRowData>, close: () => void) {
  const data = row.node.data;
  if (data.kind !== "branch") return;
  close();
  void nextTick(() => {
    emit("update:branch", data.name);
    // 该分支已被工作树检出：交给调用方直接绑定那个工作树目录，不再新建
    if (data.worktreePath) emit("selectWorktree", { branch: data.name, worktreePath: data.worktreePath });
  });
}

function handleWorktreeChecked(event: Event) {
  const checked = Boolean((event.target as HTMLInputElement | null)?.checked);
  emit("update:workMode", checked ? "worktree" : "directory");
}

watch(() => props.mainPath, () => {
  mainDropdownOpen.value = false;
});
watch(() => props.selectedBranch, () => {
  branchDropdownOpen.value = false;
});
</script>
