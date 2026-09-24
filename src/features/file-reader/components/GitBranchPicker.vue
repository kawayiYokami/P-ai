<template>
  <div class="ecall-branch-picker">
    <div v-if="loading" class="px-2 py-2 text-xs opacity-50">{{ t("gitPanel.loading") }}</div>
    <div v-else-if="!hasAnyBranch" class="px-2 py-2 text-xs opacity-50">{{ t("gitPanel.noBranches") }}</div>
    <GitTree
      v-else
      :nodes="treeNodes"
      default-expanded
      :default-collapsed-keys="collapsedKeys"
      @row-click="onRowClick"
    >
      <template #row="{ row }">
        <!-- 分组头（树根：本地分支/远程分支） -->
        <template v-if="row.node.data.kind === 'header'">
          <span class="min-w-0 truncate font-medium opacity-60">{{ row.node.data.text }}</span>
        </template>
        <!-- 目录（名字里按 / 折叠出的分组） -->
        <template v-else-if="row.node.data.kind === 'folder'">
          <Folder class="h-3.5 w-3.5 shrink-0 opacity-60" />
          <span class="min-w-0 flex-1 truncate font-medium opacity-70">{{ row.node.data.name }}</span>
        </template>
        <!-- 本地分支 -->
        <template v-else-if="row.node.data.kind === 'branch'">
          <GitBranch
            class="h-3.5 w-3.5 shrink-0"
            :class="isSelectMode
              ? (isSelected(row.node.data.branch.name) ? 'text-primary' : 'opacity-60')
              : (row.node.data.branch.isCurrent ? 'text-primary' : (row.node.data.worktreePath ? 'opacity-30' : 'opacity-60'))"
          />
          <span
            class="min-w-0 flex-1 truncate"
            :class="isSelectMode
              ? (isSelected(row.node.data.branch.name) ? 'font-medium text-primary' : '')
              : (row.node.data.worktreePath ? 'opacity-40' : '')"
          >
            {{ row.node.data.branch.name }}
          </span>
          <!-- 取值模式：只表达“选中”，占用与当前分支对基分支无意义 -->
          <template v-if="isSelectMode">
            <Check v-if="isSelected(row.node.data.branch.name)" class="h-3.5 w-3.5 shrink-0 text-primary" />
          </template>
          <template v-else>
            <span
              v-if="row.node.data.worktreePath"
              class="shrink-0 rounded-full bg-base-content/10 px-1.5 py-0.5 text-caption opacity-60"
              :title="row.node.data.worktreePath"
            >
              {{ t("gitPanel.worktreeOccupied") }}
            </span>
            <span v-else-if="row.node.data.branch.isCurrent" class="shrink-0 opacity-50">{{ t("gitPanel.current") }}</span>
          </template>
        </template>
        <!-- 远程分支 -->
        <template v-else-if="row.node.data.kind === 'remote-branch'">
          <Cloud
            class="h-3 w-3 shrink-0"
            :class="isSelectMode && isSelected(row.node.data.branch.name) ? 'text-primary' : 'opacity-60'"
          />
          <span
            class="min-w-0 flex-1 truncate"
            :class="isSelectMode && isSelected(row.node.data.branch.name) ? 'font-medium text-primary' : ''"
          >
            {{ row.node.data.branch.name }}
          </span>
          <Check v-if="isSelectMode && isSelected(row.node.data.branch.name)" class="h-3.5 w-3.5 shrink-0 text-primary" />
        </template>
      </template>
    </GitTree>
  </div>
</template>

<script setup lang="ts">
/**
 * 分支树展示组件：本地/远程两组，按「末次提交时间倒序 + `/` 折叠分组」排列。
 *
 * 两种用法，展示层完全复用，差别只在点击行为：
 * - mode="checkout"（默认）：选中即切换分支（预检冲突 → 确认 → checkout），
 *   被其他工作树占用的分支不可点。Git 面板折叠条与首页环境卡用它。
 * - mode="select"：纯取值，点击只抛 select 事件，不预检、不确认、不落盘；
 *   占用与「当前」标记不适用，因此不渲染。新建工作树的基分支选择用它。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Check, Cloud, Folder, GitBranch } from "@lucide/vue";
import {
  gitPanelBranchList,
  gitPanelCheckout,
  gitPanelCheckoutCheck,
  gitPanelWorktrees,
  type GitPanelBranchEntry,
} from "../../../services/tauri-api";
import { buildBranchTree, sortBranchesForDisplay, type BranchTreeNode } from "../git-branch-order";
import GitTree, { type GitTreeFlatRow, type GitTreeNode } from "./GitTree.vue";

type BranchRow =
  | { kind: "header"; key: string; text: string }
  | { kind: "folder"; key: string; name: string; path: string }
  | { kind: "branch"; key: string; branch: GitPanelBranchEntry; worktreePath: string }
  | { kind: "remote-branch"; key: string; branch: GitPanelBranchEntry };

const props = withDefaults(defineProps<{
  /** 仓库根路径；为空时不加载 */
  repoRoot?: string;
  /** 外部指定的当前分支；不给则取分支列表里的 HEAD 标记 */
  currentBranch?: string;
  /** 点击语义：checkout=切换分支，select=仅取值 */
  mode?: "checkout" | "select";
  /** mode="select" 时高亮并打勾的分支名 */
  selected?: string;
  /** 是否展示远程分支分组 */
  showRemote?: boolean;
}>(), {
  repoRoot: "",
  currentBranch: "",
  mode: "checkout",
  selected: "",
  showRemote: true,
});

const emit = defineEmits<{
  /** 切换成功；调用方据此刷新自己的数据 */
  (e: "switched", name: string): void;
  /** 切换失败或预检拦截；message 为面向用户的说明 */
  (e: "error", message: string): void;
  /** mode="select"：用户点选了某个分支 */
  (e: "select", name: string): void;
}>();

const { t } = useI18n();

const isSelectMode = computed(() => props.mode === "select");

const branches = ref<GitPanelBranchEntry[]>([]);
/** 分支名（小写）→ 该分支已检出的工作树路径；命中表示分支被工作树占用，不能切换 */
const worktreeBranchMap = ref<Record<string, string>>({});
const loading = ref(false);
const switching = ref(false);
let loadSeq = 0;

/** 远程分组默认折叠，与 Git 面板一致 */
const collapsedKeys = ["header:remote"];

const localBranches = computed(() => branches.value.filter((branch) => !branch.isRemote));
const remoteBranches = computed(() => branches.value.filter((branch) => branch.isRemote));
const hasAnyBranch = computed(() =>
  localBranches.value.length > 0 || (props.showRemote && remoteBranches.value.length > 0),
);

/** 行内判断：取值模式下该分支是否即当前选中项 */
function isSelected(name: string): boolean {
  return String(props.selected || "").trim() === name;
}

/** 分支名（小写）→ 已检出的工作树路径 */
function worktreePathOf(branchName: string): string {
  return String(worktreeBranchMap.value[String(branchName || "").trim().toLowerCase()] || "");
}

/** 纯函数树节点 → GitTree 节点；scope 决定行类型与 key 命名空间 */
function toGitTreeNodes(
  nodes: BranchTreeNode<GitPanelBranchEntry>[],
  scope: "local" | "remote",
): GitTreeNode<BranchRow>[] {
  return nodes.map((node) => {
    if (node.kind === "branch") {
      const key = `${scope}:${node.branch.name}`;
      const worktreePath = scope === "local" ? worktreePathOf(node.branch.name) : "";
      return {
        key,
        data:
          scope === "local"
            ? { kind: "branch" as const, key, branch: node.branch, worktreePath }
            : { kind: "remote-branch" as const, key, branch: node.branch },
        title: node.branch.name,
      };
    }
    const key = `folder:${scope}:${node.path}`;
    return {
      key,
      data: { kind: "folder" as const, key, name: node.name, path: node.path },
      children: toGitTreeNodes(node.children, scope),
    };
  });
}

const treeNodes = computed<GitTreeNode<BranchRow>[]>(() => {
  const roots: GitTreeNode<BranchRow>[] = [];
  const sortedLocal = sortBranchesForDisplay(localBranches.value);
  const sortedRemote = props.showRemote ? sortBranchesForDisplay(remoteBranches.value) : [];
  if (sortedLocal.length > 0) {
    roots.push({
      key: "header:local",
      data: { kind: "header", key: "header:local", text: t("gitPanel.localBranches") },
      children: toGitTreeNodes(buildBranchTree(sortedLocal), "local"),
    });
  }
  if (sortedRemote.length > 0) {
    roots.push({
      key: "header:remote",
      data: { kind: "header", key: "header:remote", text: t("gitPanel.remoteBranches") },
      children: toGitTreeNodes(buildBranchTree(sortedRemote), "remote"),
    });
  }
  return roots;
});

async function load() {
  const root = String(props.repoRoot || "").trim();
  if (!root) {
    branches.value = [];
    return;
  }
  const seq = ++loadSeq;
  loading.value = true;
  try {
    // 取值模式不切换分支，用不到「分支是否被工作树占用」，省一次 worktree list
    const [result, worktrees] = await Promise.all([
      gitPanelBranchList(root),
      isSelectMode.value ? Promise.resolve(null) : gitPanelWorktrees(root, root).catch(() => null),
    ]);
    if (seq !== loadSeq) return;
    branches.value = result;
    const map: Record<string, string> = {};
    for (const entry of worktrees?.worktrees || []) {
      // 主工作树就是仓库根，它检出的当前分支仍可正常选择
      if (entry.isMain) continue;
      const branch = String(entry.branch || "").trim().toLowerCase();
      const worktreePath = String(entry.path || "").trim();
      if (!branch || !worktreePath) continue;
      map[branch] = worktreePath;
    }
    worktreeBranchMap.value = map;
  } catch (error) {
    if (seq !== loadSeq) return;
    branches.value = [];
    worktreeBranchMap.value = {};
    emit("error", error instanceof Error ? error.message : String(error));
  } finally {
    if (seq === loadSeq) loading.value = false;
  }
}

async function onRowClick(row: GitTreeFlatRow<BranchRow>) {
  const data = row.node.data;
  if (data.kind !== "branch" && data.kind !== "remote-branch") return;
  // 取值模式：选中即抛事件，由调用方决定落点，不做任何预检或切换
  if (isSelectMode.value) {
    emit("select", data.branch.name);
    return;
  }
  if (data.branch.isCurrent) return;
  // 分支已被其他工作树检出：checkout 必然失败，拦在点击之前
  if (data.kind === "branch" && data.worktreePath) return;
  await switchBranch(data.branch.name);
}

/** 与 Git 面板一致：预检冲突 → 确认 → 切换 */
async function switchBranch(name: string) {
  const root = String(props.repoRoot || "").trim();
  if (!root || switching.value) return;
  switching.value = true;
  try {
    const check = await gitPanelCheckoutCheck(root, name);
    if (check.conflictingPaths.length > 0) {
      emit("error", t("gitPanel.checkoutBlocked", { name, paths: check.conflictingPaths.join("\n") }));
      return;
    }
    if (!window.confirm(t("gitPanel.checkoutConfirm", { name }))) return;
    const result = await gitPanelCheckout(root, name);
    if (result.exitCode !== 0) {
      emit("error", result.stderr || result.stdout || `checkout ${name} failed`);
      return;
    }
    emit("switched", name);
    await load();
  } catch (error) {
    emit("error", error instanceof Error ? error.message : String(error));
  } finally {
    switching.value = false;
  }
}

onMounted(() => {
  void load();
});

onBeforeUnmount(() => {
  loadSeq += 1;
});
</script>
