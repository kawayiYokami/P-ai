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
          <GitBranch class="h-3.5 w-3.5 shrink-0" :class="row.node.data.branch.isCurrent ? 'text-primary' : 'opacity-60'" />
          <span class="min-w-0 flex-1 truncate">{{ row.node.data.branch.name }}</span>
          <span v-if="row.node.data.branch.isCurrent" class="shrink-0 opacity-50">{{ t("gitPanel.current") }}</span>
        </template>
        <!-- 远程分支 -->
        <template v-else-if="row.node.data.kind === 'remote-branch'">
          <Cloud class="h-3 w-3 shrink-0 opacity-60" />
          <span class="min-w-0 flex-1 truncate">{{ row.node.data.branch.name }}</span>
        </template>
      </template>
    </GitTree>
  </div>
</template>

<script setup lang="ts">
/**
 * 分支切换下拉：本地/远程两组，按「末次提交时间倒序 + `/` 折叠分组」排列，选中即切换。
 *
 * 与 Git 面板折叠条上的分支下拉共用同一套排序分组（git-branch-order.ts）与切换流程；
 * 面板与首页卡片墙两处都用它，避免各自维护一份分支列表与切换逻辑。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Cloud, Folder, GitBranch } from "@lucide/vue";
import {
  gitPanelBranchList,
  gitPanelCheckout,
  gitPanelCheckoutCheck,
  type GitPanelBranchEntry,
} from "../../../services/tauri-api";
import { buildBranchTree, sortBranchesForDisplay, type BranchTreeNode } from "../git-branch-order";
import GitTree, { type GitTreeFlatRow, type GitTreeNode } from "./GitTree.vue";

type BranchRow =
  | { kind: "header"; key: string; text: string }
  | { kind: "folder"; key: string; name: string; path: string }
  | { kind: "branch"; key: string; branch: GitPanelBranchEntry }
  | { kind: "remote-branch"; key: string; branch: GitPanelBranchEntry };

const props = withDefaults(defineProps<{
  /** 仓库根路径；为空时不加载 */
  repoRoot?: string;
  /** 外部指定的当前分支；不给则取分支列表里的 HEAD 标记 */
  currentBranch?: string;
}>(), {
  repoRoot: "",
  currentBranch: "",
});

const emit = defineEmits<{
  /** 切换成功；调用方据此刷新自己的数据 */
  (e: "switched", name: string): void;
  /** 切换失败或预检拦截；message 为面向用户的说明 */
  (e: "error", message: string): void;
}>();

const { t } = useI18n();

const branches = ref<GitPanelBranchEntry[]>([]);
const loading = ref(false);
const switching = ref(false);
let loadSeq = 0;

/** 远程分组默认折叠，与 Git 面板一致 */
const collapsedKeys = ["header:remote"];

const localBranches = computed(() => branches.value.filter((branch) => !branch.isRemote));
const remoteBranches = computed(() => branches.value.filter((branch) => branch.isRemote));
const hasAnyBranch = computed(() => branches.value.length > 0);

/** 纯函数树节点 → GitTree 节点；scope 决定行类型与 key 命名空间 */
function toGitTreeNodes(
  nodes: BranchTreeNode<GitPanelBranchEntry>[],
  scope: "local" | "remote",
): GitTreeNode<BranchRow>[] {
  return nodes.map((node) => {
    if (node.kind === "branch") {
      const key = `${scope}:${node.branch.name}`;
      return {
        key,
        data:
          scope === "local"
            ? { kind: "branch" as const, key, branch: node.branch }
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
  const sortedRemote = sortBranchesForDisplay(remoteBranches.value);
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
    const result = await gitPanelBranchList(root);
    if (seq !== loadSeq) return;
    branches.value = result;
  } catch (error) {
    if (seq !== loadSeq) return;
    branches.value = [];
    emit("error", error instanceof Error ? error.message : String(error));
  } finally {
    if (seq === loadSeq) loading.value = false;
  }
}

async function onRowClick(row: GitTreeFlatRow<BranchRow>) {
  const data = row.node.data;
  if (data.kind !== "branch" && data.kind !== "remote-branch") return;
  if (data.branch.isCurrent) return;
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
