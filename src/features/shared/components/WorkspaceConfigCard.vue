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
          :disabled="branchSwitchLocked || !gitRootAvailable || (branchList.length === 0 && !selectedBranch)"
          :teleport="dropdownTeleport"
          :teleport-to="dropdownTeleportTo"
          root-class="min-w-[112px] max-w-[180px]"
          panel-class="w-full"
        >
          <template #trigger="{ toggle, open }">
            <div class="flex h-8 items-center gap-1 bg-transparent pl-3 pr-2">
              <GitBranch class="h-3.5 w-3.5 shrink-0 text-base-content/45" />
              <button
                type="button"
                class="min-w-0 flex-1 cursor-pointer text-left text-xs font-medium text-base-content outline-none disabled:cursor-not-allowed disabled:opacity-60"
                :disabled="branchSwitchLocked || !gitRootAvailable || (branchList.length === 0 && !selectedBranch)"
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
              <button
                v-for="branch in branchList"
                :key="branch"
                type="button"
                class="flex w-full items-center gap-2 rounded-field px-2.5 py-2 text-left text-xs transition-colors"
                :class="branch.toLowerCase() === (selectedBranch || '').toLowerCase()
                  ? 'bg-base-200 font-medium'
                  : 'hover:bg-base-200/70'"
                @click="handleBranchSelect(branch, close)"
              >
                <span class="min-w-0 flex-1 truncate">{{ branch }}</span>
                <Check
                  v-if="branch.toLowerCase() === (selectedBranch || '').toLowerCase()"
                  class="h-3.5 w-3.5 shrink-0 text-primary"
                />
              </button>
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
import { FolderOpen, FolderSearch, FolderPlus, ChevronDown, Check, GitBranch, X } from "@lucide/vue";
import EcallDropdown from "./EcallDropdown.vue";
import OverlayScrollArea from "./OverlayScrollArea.vue";
import type { ShellWorkspaceAccess, ShellWorkMode } from "../../../types/app";
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
  branchList?: string[];
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
  branchList: () => [],
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

function handleBranchSelect(branch: string, close: () => void) {
  close();
  void nextTick(() => emit("update:branch", branch));
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
