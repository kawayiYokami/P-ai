<template>
  <GitTree
    ref="treeRef"
    :nodes="treeNodes"
    :mode="mode"
    selectable
    multiple
    default-expanded
    @update:selectedKeys="selectedKeys = $event"
    @select="onSelect"
    @contextmenu="onTreeContextMenu"
  >
    <template #row="{ row, expanded, toggle }">
      <!-- 组头行（树根，行容器样式由 GitTree 统一提供） -->
      <template v-if="row.node.data.kind === 'group'">
        <button
          type="button"
          class="flex min-w-0 flex-1 items-center gap-1 text-left font-medium opacity-70"
          @click.stop="toggle"
        >
          <span class="min-w-0 truncate">{{ title }}</span>
        </button>
        <button
          v-if="entries.length > 0"
          type="button"
          class="btn btn-ghost btn-xs h-5 min-h-5 w-5 px-0"
          :class="{ 'text-primary': selectedCount > 0 }"
          :disabled="busy"
          :title="selectedCount > 0 ? `${actionTitle} (${selectedCount})` : actionTitle"
          @click.stop="runAction(entries.map((entry) => entry.path))"
        >
          <Plus v-if="actionKind === 'stage'" class="h-3 w-3" />
          <Minus v-else class="h-3 w-3" />
        </button>
        <span class="shrink-0 text-caption tabular-nums opacity-50">{{
          totalCount !== undefined && totalCount > 1000 ? "1000+" : (totalCount ?? entries.length)
        }}</span>
      </template>
      <!-- 目录行内容 -->
      <template v-else-if="isDir(row.node.data)">
        <button
          type="button"
          class="flex min-w-0 flex-1 items-center gap-1.5 rounded px-1 py-0.5 text-left font-medium"
          :title="row.node.data.path"
          @click.stop="toggle"
        >
          <img
            :src="resolveFileTreeIcon(row.node.data.path, true, expanded)"
            alt=""
            class="h-4 w-4 shrink-0 object-contain opacity-80"
          />
          <span class="min-w-0 truncate">{{ row.node.data.name }}</span>
        </button>
        <span class="hidden shrink-0 items-center gap-0.5 group-hover:flex focus-within:flex">
          <button
            type="button"
            class="btn btn-ghost btn-xs h-5 min-h-5 w-5 px-0 text-error/70"
            :title="discardTitle"
            :disabled="busy"
            @click.stop="discardSelected(collectDirPaths(row.node.data))"
          >
            <Undo2 class="h-3 w-3" />
          </button>
          <button
            type="button"
            class="btn btn-ghost btn-xs h-5 min-h-5 w-5 px-0"
            :title="actionTitle"
            :disabled="busy"
            @click.stop="runAction(collectDirPaths(row.node.data))"
          >
            <Plus v-if="actionKind === 'stage'" class="h-3 w-3" />
            <Minus v-else class="h-3 w-3" />
          </button>
        </span>
      </template>
      <!-- 文件行内容 -->
      <template v-else>
        <span v-if="mode === 'tree'" class="h-5 w-5 shrink-0"></span>
        <img
          :src="resolveFileTreeIcon(row.node.data.path, false)"
          alt=""
          class="h-4 w-4 shrink-0 object-contain opacity-80"
        />
        <button
          type="button"
          class="flex min-w-0 items-center gap-1.5 text-left"
          :class="mode === 'tree' ? 'flex-1' : 'max-w-[60%]'"
          :title="row.node.data.path"
        >
          <span
            class="min-w-0 truncate"
            :class="isDeletedEntry(row.node.data.entry) ? 'line-through opacity-60' : ''"
          >{{ row.node.data.name }}</span>
        </button>
        <span
          v-if="mode === 'list' && row.node.data.dir"
          class="min-w-0 flex-1 truncate text-caption opacity-40"
          :title="row.node.data.path"
        >{{ row.node.data.dir }}</span>
        <span class="ml-auto flex shrink-0 items-center gap-0.5">
          <span class="hidden items-center gap-0.5 group-hover:flex focus-within:flex">
            <button
              type="button"
              class="btn btn-ghost btn-xs h-5 min-h-5 w-5 px-0 text-error/70"
              :title="discardTitle"
              :disabled="busy"
              @click.stop="discardSelected([row.node.data.path])"
            >
              <Undo2 class="h-3 w-3" />
            </button>
            <button
              type="button"
              class="btn btn-ghost btn-xs h-5 min-h-5 w-5 px-0"
              :title="actionTitle"
              :disabled="busy"
              @click.stop="runAction([row.node.data.path])"
            >
              <Plus v-if="actionKind === 'stage'" class="h-3 w-3" />
              <Minus v-else class="h-3 w-3" />
            </button>
          </span>
          <span
            class="shrink-0 font-mono text-caption font-bold"
            :class="statusClass(row.node.data.entry)"
            :title="statusTitle(row.node.data.entry)"
          >{{ statusLabel(row.node.data.entry) }}</span>
        </span>
      </template>
    </template>
  </GitTree>

  <!-- 右键菜单：折叠全部（仅树状模式） -->
  <div
    v-if="contextMenu.visible && mode === 'tree'"
    class="fixed inset-0 z-50"
    @click="closeContextMenu"
    @contextmenu.prevent="closeContextMenu"
  ></div>
  <div
    v-if="contextMenu.visible && mode === 'tree'"
    class="fixed z-50 w-40 overflow-hidden rounded-lg border border-base-300 bg-base-100 py-1 shadow-xl"
    :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
    @click.stop
    @contextmenu.prevent
  >
    <button
      type="button"
      class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-xs hover:bg-base-300/40"
      @click="collapseAllDirectories"
    >
      <ChevronDown class="h-3.5 w-3.5 opacity-60" />
      {{ collapseAllTitle }}
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { ChevronDown, Minus, Plus, Undo2 } from "@lucide/vue";
import type { GitPanelStatusEntry } from "../../../services/tauri-api";
import { resolveFileTreeIcon } from "../file-tree-icons";
import GitTree, { type GitTreeExpose, type GitTreeFlatRow, type GitTreeNode } from "./GitTree.vue";

const props = withDefaults(defineProps<{
  title: string;
  entries: GitPanelStatusEntry[];
  busy?: boolean;
  actionKind: "stage" | "unstage";
  actionTitle: string;
  discardTitle: string;
  expandTitle?: string;
  collapseTitle?: string;
  collapseAllTitle?: string;
  highlightPath?: string;
  /** 展示模式：tree 按目录折叠分组；list 平铺全部文件（VSCode 风格） */
  mode?: "tree" | "list";
  /** 截断前的组内实际数量；不传时回退 entries.length（未截断场景） */
  totalCount?: number;
}>(), {
  busy: false,
  expandTitle: "",
  collapseTitle: "",
  collapseAllTitle: "",
  highlightPath: "",
  mode: "tree",
  totalCount: undefined,
});

const emit = defineEmits<{
  (e: "openDiff", payload: { path: string; staged: boolean; untracked?: boolean }): void;
  (e: "action", paths: string[]): void;
  (e: "discard", paths: string[]): void;
}>();

/** 树节点业务数据：组头 / 目录 / 文件 */
type GitTreeItem =
  | { kind: "group" }
  | { kind: "file"; name: string; path: string; dir: string; entry: GitPanelStatusEntry }
  | { kind: "dir"; name: string; path: string };

function isDir(item: GitTreeItem): item is Extract<GitTreeItem, { kind: "dir" }> {
  return item.kind === "dir";
}

/** 组头作为树根：children 为目录/文件树 */
const treeNodes = computed<GitTreeNode<GitTreeItem>[]>(() => [
  {
    key: "group-root",
    data: { kind: "group" },
    // 空组时 children 为空数组：expandable 兜底保证点击组头仍是展开/折叠语义
    expandable: true,
    children: buildEntries(props.entries),
  },
]);

/** 扁平 status 条目构建为目录树（文件叶子 + 目录中间节点） */
function buildEntries(entries: GitPanelStatusEntry[]): GitTreeNode<GitTreeItem>[] {
  const root: GitTreeNode<GitTreeItem>[] = [];
  // 先按状态分组、组内按路径升序，树与平铺两种模式共用这一份顺序
  const ordered = [...entries].sort((left, right) => {
    const rankDiff = statusSortRank(left) - statusSortRank(right);
    if (rankDiff !== 0) return rankDiff;
    return left.path.localeCompare(right.path);
  });
  for (const entry of ordered) {
    const segments = entry.path.split("/");
    let siblings = root;
    let acc = "";
    for (let i = 0; i < segments.length; i++) {
      const segment = segments[i];
      acc = acc ? `${acc}/${segment}` : segment;
      if (i === segments.length - 1) {
        siblings.push({
          key: acc,
          data: {
            kind: "file",
            name: segment,
            dir: segments.slice(0, -1).join("/"),
            path: acc,
            entry,
          },
        });
        continue;
      }
      const found = siblings.find(
        (item) => item.data.kind === "dir" && item.data.name === segment,
      );
      let dir: GitTreeNode<GitTreeItem> | undefined;
      if (found && found.data.kind === "dir") {
        dir = found;
      }
      if (!dir) {
        dir = { key: acc, data: { kind: "dir", name: segment, path: acc }, children: [] };
        siblings.push(dir);
      }
      siblings = dir.children!;
    }
  }
  return root;
}

// ==================== 选中与批量操作 ====================
const treeRef = ref<GitTreeExpose | null>(null);
/** GitTree 内部选中集的镜像（操作按钮优先作用于选中集） */
const selectedKeys = ref<string[]>([]);

const selectedCount = computed(() => selectedKeys.value.length);

function onSelect(key: string) {
  // 普通点击文件行：打开 diff（选中集已由 GitTree 维护）
  // 未跟踪文件（??）git diff 不输出内容，标记 untracked 由上层直接打开文件
  const entry = props.entries.find((item) => item.path === key);
  const untracked =
    !!entry && entry.stagedStatus.trim() === "?" && entry.unstagedStatus.trim() === "?";
  emit("openDiff", { path: key, staged: props.actionKind === "unstage", untracked });
}

/** 批量操作：有选中集时作用于全部选中文件，否则作用于传入路径 */
function runAction(paths: string[]) {
  const targets = selectedKeys.value.length > 0 ? [...selectedKeys.value] : paths;
  if (targets.length === 0) return;
  emit("action", targets);
  treeRef.value?.clearSelection();
}

function discardSelected(paths: string[]) {
  const targets = selectedKeys.value.length > 0 ? [...selectedKeys.value] : paths;
  if (targets.length === 0) return;
  emit("discard", targets);
  treeRef.value?.clearSelection();
}

// ==================== 右键菜单 ====================
const contextMenu = ref<{ visible: boolean; x: number; y: number }>({ visible: false, x: 0, y: 0 });

/** GitTree 行右键转发：树状模式打开折叠菜单；原生右键菜单已由全局 contextmenu guard 屏蔽 */
function onTreeContextMenu(_row: GitTreeFlatRow<GitTreeItem>, event: MouseEvent) {
  if (props.mode !== "tree") return;
  event.preventDefault();
  contextMenu.value = { visible: true, x: event.clientX, y: event.clientY };
}

function closeContextMenu() {
  contextMenu.value.visible = false;
}

// 一键折叠全部目录：交给 GitTree 维护的展开状态（保留组头根节点展开）
function collapseAllDirectories() {
  treeRef.value?.collapseAll(true);
  closeContextMenu();
}

// 收集目录下所有文件路径（含子目录），供文件夹级暂存/取消暂存使用
// 直接按路径前缀过滤 entries，不依赖树结构，任意层级目录都有效
function collectDirPaths(dir: Extract<GitTreeItem, { kind: "dir" }>): string[] {
  const prefix = `${dir.path}/`;
  return props.entries
    .filter((entry) => entry.path.startsWith(prefix))
    .map((entry) => entry.path);
}

function statusLabel(entry: GitPanelStatusEntry) {
  const staged = entry.stagedStatus.trim();
  const unstaged = entry.unstagedStatus.trim();
  if (staged === "?" && unstaged === "?") return "U";
  const code = props.actionKind === "unstage" ? staged : unstaged;
  return code || "M";
}

/** 状态码悬停提示：M 修改 / A 新增 / D 删除 / U 未跟踪 / R 重命名 */
function statusTitle(entry: GitPanelStatusEntry) {
  const label = statusLabel(entry);
  const names: Record<string, string> = {
    M: "Modified",
    A: "Added",
    D: "Deleted",
    U: "Untracked",
    R: "Renamed",
  };
  return names[label] || label;
}

function statusClass(entry: GitPanelStatusEntry) {
  const label = statusLabel(entry);
  if (label === "A" || label === "U") return "text-success";
  if (label === "D") return "text-error";
  return "text-warning";
}

/** 删除状态：文件名划线、整体降透明度，和其他状态的条目一眼区分开 */
function isDeletedEntry(entry: GitPanelStatusEntry) {
  return statusLabel(entry) === "D";
}

/** 组内状态排序权重：新增 → 删除 → 修改 → 重命名 → 未跟踪，未知码排最后 */
const STATUS_SORT_ORDER = ["A", "D", "M", "R", "U"];

function statusSortRank(entry: GitPanelStatusEntry) {
  const index = STATUS_SORT_ORDER.indexOf(statusLabel(entry));
  return index === -1 ? STATUS_SORT_ORDER.length : index;
}
</script>
