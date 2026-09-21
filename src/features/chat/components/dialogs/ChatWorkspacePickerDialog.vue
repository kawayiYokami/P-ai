<template>
  <dialog
    ref="dialogRef"
    class="modal"
    :open="open"
    @close="onDialogClose"
    @cancel.prevent="onDialogClose"
    @keydown.esc.prevent="onDialogClose"
  >
    <div class="modal-box flex max-h-[calc(100dvh-4rem)] w-full max-w-xl flex-col overflow-hidden p-0">
      <div class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
        <WorkspaceConfigCard
          :main-path="mainPath"
          :secondary-paths="secondaryPaths"
          :access="unifiedAccess"
          :work-mode="workMode"
          :selected-branch="selectedBranch"
          :branch-entries="branchEntries"
          :worktree-branch-map="worktreeBranchMap"
          :branch-loading="branchLoading"
          :git-root-available="effectiveGitAvailable"
          :git-check-message="effectiveGitMessage"
          :branch-locked-reason="branchLockedReason"
          :available-workspaces="availableWorkspaceOptions"
          :hide-add-workspace="hideAddWorkspace"
          @update:main-path="onMainPathUpdate"
          @update:access="onAccessUpdate"
          @update:work-mode="onWorkModeUpdate"
          @update:branch="onBranchUpdate"
          @select-worktree="onWorktreeSelect"
          @browse-main="onBrowseMain"
          @add-secondary="onAddSecondary"
          @remove-secondary="onRemoveSecondary"
        />
        <div v-if="validationMessage" class="mt-3 rounded-field bg-error/10 px-3 py-2 text-xs text-error">
          {{ validationMessage }}
        </div>
        <div v-if="checkoutError && !validationMessage" class="mt-3 rounded-field bg-error/10 px-3 py-2 text-xs text-error">
          {{ checkoutError }}
        </div>
      </div>
      <div class="flex shrink-0 items-center justify-between gap-3 border-t border-base-300 px-4 py-3">
        <label
          class="flex cursor-pointer items-center gap-2 text-xs font-medium"
          :title="t('chat.workspacePickerAutonomousHint')"
        >
          <input
            type="checkbox"
            class="checkbox checkbox-primary checkbox-sm"
            :checked="autonomousMode"
            :disabled="saving"
            @change="onAutonomousModeChange"
          />
          <span>{{ t("chat.workspacePickerAutonomous") }}</span>
        </label>
        <div class="flex items-center gap-2">
          <button class="btn btn-sm btn-ghost" type="button" :disabled="saving" @click="emit('close')">
            {{ t("common.cancel") }}
          </button>
          <button class="btn btn-sm btn-primary" type="button" :disabled="saving" @click="emit('save')">
            {{ saving ? t("common.saving") : t("common.save") }}
          </button>
        </div>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDialogClose">close</button>
    </form>
  </dialog>
  <WorkspaceDirectoryPickerDialog
    :open="directoryPickerOpen"
    :initial-path="directoryPickerInitialPath"
    @close="directoryPickerOpen = false"
    @select="onDirectoryPicked"
  />
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import WorkspaceConfigCard from "../../../shared/components/WorkspaceConfigCard.vue";
import WorkspaceDirectoryPickerDialog from "../../../shared/components/WorkspaceDirectoryPickerDialog.vue";
import { gitPanelBranchList, gitPanelCheckout, gitPanelCheckoutCheck, gitPanelHeadState, gitPanelWorktrees, type GitPanelBranchEntry } from "../../../../services/tauri-api";
import type { ChatWorkspaceChoice } from "../../composables/use-chat-workspace";
import type { ShellWorkMode } from "../../../../types/app";
import { normalizeShellWorkMode, normalizeWorkspaceAccess } from "../../../../utils/shell-workspaces";

const props = withDefaults(defineProps<{
  open: boolean;
  saving: boolean;
  workspaces: ChatWorkspaceChoice[];
  autonomousMode: boolean;
  workMode: ShellWorkMode;
  worktreePath?: string;
  worktreeExists?: boolean;
  worktreeAvailable: boolean;
  worktreeCheckMessage?: string;
  validationMessage?: string;
  hideAddWorkspace?: boolean;
  selectedBranch?: string;
  /** 对话框内切换分支成功后按目标目录同步「本会话工作分支」记录 */
  syncWorkspaceBranch?: (workspacePath: string) => Promise<void>;
}>(), {
  hideAddWorkspace: false,
  selectedBranch: "",
  worktreePath: "",
  worktreeExists: false,
});

const emit = defineEmits<{
  (e: "close"): void;
  (e: "addWorkspace"): void;
  (e: "setMain", workspaceId: string): void;
  (e: "setAccess", workspaceId: string, access: ChatWorkspaceChoice["access"]): void;
  (e: "setAccessUnified", access: ChatWorkspaceChoice["access"]): void;
  (e: "setAutonomousMode", enabled: boolean): void;
  (e: "setWorkMode", mode: ShellWorkMode): void;
  (e: "setBranch", branch: string): void;
  /** 选中已被工作树检出的分支：把会话直接绑到该工作树目录 */
  (e: "setWorktree", payload: { branch: string; worktreePath: string }): void;
  (e: "removeWorkspace", workspaceId: string): void;
  (e: "openDir", workspaceId: string): void;
  (e: "save"): void;
  (e: "addSecondary", path: string): void;
  (e: "removeSecondary", path: string): void;
  (e: "updateMainPath", path: string): void;
}>();

const { t } = useI18n();
const dialogRef = ref<HTMLDialogElement | null>(null);

const branchEntries = ref<GitPanelBranchEntry[]>([]);
/** 分支名（小写）→ 该分支已检出的工作树路径；命中表示该分支已被工作树占用 */
const worktreeBranchMap = ref<Record<string, string>>({});
const branchLoading = ref(false);
const checkoutError = ref("");
/** 变基进行中：HEAD 游离，切分支会破坏变基现场，此时锁住分支下拉 */
const rebaseInProgress = ref(false);
const localGitAvailable = ref<boolean | null>(null);
const localGitMessage = ref("");
let branchSeq = 0;

const effectiveGitAvailable = computed(() => {
  if (localGitAvailable.value !== null) return localGitAvailable.value;
  return props.worktreeAvailable;
});
const effectiveGitMessage = computed(() => {
  if (checkoutError.value) return checkoutError.value;
  if (localGitMessage.value) return localGitMessage.value;
  return String(props.worktreeCheckMessage || "").trim();
});

const directoryPickerOpen = ref(false);
const directoryPickerMode = ref<"main" | "secondary">("main");
const directoryPickerInitialPath = ref("");

const mainPath = computed(() => {
  const main = props.workspaces.find((w) => w.level === "main");
  if (main) return String(main.path || "").trim();
  return String(props.workspaces[0]?.path || "").trim();
});

const secondaryPaths = computed(() => {
  return props.workspaces
    .filter((w) => w.level === "secondary")
    .map((w) => String(w.path || "").trim())
    .filter(Boolean);
});

const unifiedAccess = computed<ChatWorkspaceChoice["access"]>(() => {
  const main = props.workspaces.find((w) => w.level === "main");
  const raw = String(main?.access || props.workspaces[0]?.access || "approval").trim();
  return normalizeWorkspaceAccess(raw) as ChatWorkspaceChoice["access"];
});

const selectedBranch = computed(() => String(props.selectedBranch || "").trim());

/** 分支下拉的锁定原因；为空表示可正常切换 */
const branchLockedReason = computed(() => (rebaseInProgress.value ? t("chat.workspaceBranchRebasing") : ""));

const availableWorkspaceOptions = computed(() => {
  return props.workspaces.map((w) => ({
    id: w.id,
    name: w.name,
    path: w.path,
    access: w.access,
  }));
});

function onDialogClose() {
  // :open 绑定已接管显隐，不再调用 showModal/close 避免进入 top layer；仅处理关闭意图
  if (props.saving) return;
  emit("close");
}

// 使用 :open 绑定而非 showModal，避免进入 top layer 导致 teleport 到 body 的下拉被遮挡
// 保留一个空的 sync 占位以兼容热更新，不再操作 dialog 方法
function syncDialog() {}

const effectiveWorktreeExists = computed(() => {
  const rawPath = String(props.worktreePath || "").trim();
  if (props.workMode !== "worktree" || !props.worktreeExists || !rawPath) return false;
  const normalizedWorktree = rawPath.replace(/\\/g, "/");
  const marker = "/.pai/.worktree/";
  const idx = normalizedWorktree.toLowerCase().indexOf(marker);
  if (idx < 0) return false;
  const worktreeRoot = normalizedWorktree.slice(0, idx).replace(/\/+$/, "");
  const normalizedMain = String(mainPath.value || "").trim().replace(/\\/g, "/").replace(/\/+$/, "");
  if (!worktreeRoot || !normalizedMain) return false;
  return worktreeRoot.toLowerCase() === normalizedMain.toLowerCase();
});

function resolveBranchTargetPath(): string {
  const worktreePath = String(props.worktreePath || "").trim();
  if (props.workMode === "worktree" && effectiveWorktreeExists.value && worktreePath) {
    return worktreePath;
  }
  return String(mainPath.value || "").trim();
}

watch(
  () => [mainPath.value, props.workMode, String(props.worktreePath || "").trim(), Boolean(props.worktreeExists), effectiveWorktreeExists.value] as const,
  () => {
    const target = resolveBranchTargetPath();
    if (!target) {
      branchEntries.value = [];
      worktreeBranchMap.value = {};
      branchLoading.value = false;
      // 无目标时不保留旧的本地探测结果，避免旧错误残留
      localGitAvailable.value = false;
      localGitMessage.value = "";
      // checkoutError 保留给分支切换错误，这里仅在目标为空时清空
      checkoutError.value = "";
      return;
    }
    // 不再以 worktreeAvailable 门控：始终尝试取真值，失败则显错（Web/桌面同一 Rust 文案）
    void loadBranches(target);
  },
  { immediate: true },
);

/**
 * 取一次 HEAD 状态：变基进行中 HEAD 游离，切分支会破坏变基现场，命中就锁住分支下拉。
 * 取不到状态时按「不锁」处理，保持分支切换原有可用性。
 */
async function refreshRebaseState(path: string) {
  const normalized = String(path || "").trim();
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

async function loadBranches(path: string) {
  const seq = ++branchSeq;  const normalized = String(path || "").trim();
  if (!normalized) {
    branchEntries.value = [];
    worktreeBranchMap.value = {};
    localGitAvailable.value = false;
    localGitMessage.value = "";
    rebaseInProgress.value = false;
    return;
  }
  branchLoading.value = true;
  // 每次加载前清空上一次的显错，成功后会置为可用
  checkoutError.value = "";
  localGitMessage.value = "";
  await refreshRebaseState(normalized);
  try {
    const entries = await gitPanelBranchList(normalized);
    if (seq !== branchSeq) return;
    localGitAvailable.value = true;
    localGitMessage.value = "";
    branchEntries.value = entries;
    void loadWorktreeBranchMap(normalized);
    const current = entries.find((e) => e.isCurrent)?.name;
    const currentName = String(current || "").trim();
    if (!currentName) return;
    const selected = String(selectedBranch.value || "").trim();
    // 真值模型：显示永远等于当前真值
    // directory：永远用项目当前分支
    // worktree 已创建且草稿主目录与工作树所属一致：用工作树当前分支
    // worktree 未创建或草稿已切新目录：没有可信真值，保留意图，不自动覆盖
    const isWorktree = props.workMode === "worktree";
    const worktreeExists = Boolean(effectiveWorktreeExists.value);
    const shouldSyncToTruth = !isWorktree || worktreeExists;
    if (shouldSyncToTruth && currentName.toLowerCase() !== selected.toLowerCase()) {
      // 延迟到下一 tick 再 emit，避免在 load 过程中同步触发 watcher 循环
      emit("setBranch", currentName);
    }
  } catch (error) {
    if (seq !== branchSeq) return;
    branchEntries.value = [];
    worktreeBranchMap.value = {};
    const message = error instanceof Error ? String(error.message || error) : String(error);
    const normalizedMessage = message.trim() || "加载分支失败";
    localGitAvailable.value = false;
    localGitMessage.value = normalizedMessage;
    checkoutError.value = normalizedMessage;
    // 关键：不可静默。Web 端 F12 必须可见，卡片通过 effectiveGitMessage 显错
    console.warn("[工作区] 加载分支失败:", { path: normalized, error: normalizedMessage });
  } finally {
    if (seq === branchSeq) branchLoading.value = false;
  }
}

/**
 * 取当前仓库已有工作树，按分支名建映射（排除主工作树，它就是仓库根）。
 * 命中表示该分支已被某个工作树检出，选中时直接绑定那个目录而不是新建。
 */
async function loadWorktreeBranchMap(path: string) {
  const normalized = String(path || "").trim();
  const repoRoot = String(mainPath.value || "").trim();
  if (!normalized || !repoRoot) {
    worktreeBranchMap.value = {};
    return;
  }
  try {
    const result = await gitPanelWorktrees(normalized, repoRoot);
    const map: Record<string, string> = {};
    for (const entry of result.worktrees || []) {
      if (entry.isMain) continue;
      const branch = String(entry.branch || "").trim().toLowerCase();
      const worktreePath = String(entry.path || "").trim();
      if (!branch || !worktreePath) continue;
      map[branch] = worktreePath;
    }
    worktreeBranchMap.value = map;
  } catch (error) {
    // 工作树列表取不到不阻塞分支选择，仅退化为「全部按新建处理」
    console.warn("[工作区] 获取工作树列表失败:", { path: normalized, error });
    worktreeBranchMap.value = {};
  }
}

/** 选中已被工作树检出的分支：交给上层把会话直接绑到该工作树目录 */
function onWorktreeSelect(payload: { branch: string; worktreePath: string }) {
  emit("setWorktree", payload);
}

// 每次打开都重取真值：变基结束后要能自动解锁，分支列表也可能已在外部变化
watch(
  () => props.open,
  (isOpen) => {
    if (!isOpen) return;
    const target = resolveBranchTargetPath();
    if (target) void loadBranches(target);
  },
);

function onMainPathUpdate(path: string) {
  const normalized = String(path || "").trim();
  if (!normalized) return;
  // 通过 workspaces 匹配 id 触发 setMain，否则视为新增浏览
  const matched = props.workspaces.find((w) => w.path.toLowerCase() === normalized.toLowerCase());
  if (matched) {
    emit("setMain", matched.id);
  } else {
    // 新路径：先通过 addWorkspace + setMain 模拟，或直接触发 add+切换
    // 简化：触发 addSecondary 后切换为主（picker 内添加后需提升为主）
    emit("updateMainPath", normalized);
  }
}

function onAccessUpdate(access: ChatWorkspaceChoice["access"]) {
  const normalized = normalizeWorkspaceAccess(String(access || ""));
  // 统一权限：忽略 workspaceId，更新全部
  emit("setAccessUnified", normalized as ChatWorkspaceChoice["access"]);
  // 兼容旧路径
  const main = props.workspaces.find((w) => w.level === "main") || props.workspaces[0];
  if (main) emit("setAccess", main.id, normalized as ChatWorkspaceChoice["access"]);
}

async function onWorkModeUpdate(mode: ShellWorkMode) {
  const normalized = normalizeShellWorkMode(String(mode || "")) as ShellWorkMode;
  checkoutError.value = "";
  // 真值模型：切换代表意图，显示由 watcher 按 git 真值自动回填；持久化延迟到保存
  emit("setWorkMode", normalized);
}

async function onBranchUpdate(branch: string) {
  const normalized = String(branch || "").trim();
  if (!normalized) return;
  if (normalized.toLowerCase() === String(selectedBranch.value || "").trim().toLowerCase()) return;
  // worktree 未创建或草稿已切新目录导致 effective 不存在：没有可信真值可 checkout，仅改草稿意图
  if (props.workMode === "worktree" && !effectiveWorktreeExists.value) {
    checkoutError.value = "";
    emit("setBranch", normalized);
    return;
  }
  // directory 或 worktree 已创建：立即对 git 真值执行 checkout，显示跟着真值走
  const target = resolveBranchTargetPath();
  if (!target || !props.worktreeAvailable) {
    checkoutError.value = "";
    emit("setBranch", normalized);
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
    try {
      await gitPanelCheckout(target, normalized);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      checkoutError.value = t("chat.workspaceBranchCheckoutFailed", { message });
      return;
    }
    // 会话内自己切的分支：按目标目录同步记录，避免之后被守卫当成「在别处切过」而拦一次
    await props.syncWorkspaceBranch?.(target);
    try {
      const entries = await gitPanelBranchList(target);
      branchEntries.value = entries;
    } catch {
      // ignore
    }
    emit("setBranch", normalized);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    checkoutError.value = t("chat.workspaceBranchCheckFailed", { message });
  } finally {
    branchLoading.value = false;
  }
}

function onBrowseMain() {
  directoryPickerMode.value = "main";
  directoryPickerInitialPath.value = String(mainPath.value || "").trim();
  directoryPickerOpen.value = true;
}

function onAddSecondary() {
  directoryPickerMode.value = "secondary";
  // 初始路径取主目录的父级或当前主目录，便于同级选择
  directoryPickerInitialPath.value = String(mainPath.value || "").trim();
  directoryPickerOpen.value = true;
}

function onDirectoryPicked(path: string) {
  const nextPath = String(path || "").trim();
  directoryPickerOpen.value = false;
  if (!nextPath) return;
  if (directoryPickerMode.value === "main") {
    emit("updateMainPath", nextPath);
  } else {
    emit("addSecondary", nextPath);
    emit("addWorkspace");
  }
}

function onRemoveSecondary(path: string) {
  const normalized = String(path || "").trim();
  const matched = props.workspaces.find((w) => w.path.toLowerCase() === normalized.toLowerCase());
  if (matched) emit("removeWorkspace", matched.id);
  emit("removeSecondary", normalized);
}

function onAutonomousModeChange(event: Event) {
  emit("setAutonomousMode", Boolean((event.target as HTMLInputElement | null)?.checked));
}
</script>
