<template>
  <ul :class="depth === 0 ? 'menu menu-xs w-full flex-nowrap p-0' : 'w-full'" :style="depth === 0 ? 'width: 100%' : undefined">
    <li
      v-for="row in visibleRows"
      :key="row.key"
      @contextmenu="onRowContextMenu(row, $event)"
    >
      <details
        v-if="hasChildren(row.node) || row.node.expandable"
        class="git-tree-details"
        :data-flat="indent === 0 ? true : undefined"
        :open="expandedSet.has(row.key)"
        @toggle="onDetailsToggle(row.node, $event)"
      >
        <summary
          class="git-tree-row group"
          :class="row.node.rowClass"
          :title="row.node.title"
        >
          <slot
            name="row"
            :row="row"
            :depth="row.depth"
            :expanded="expandedSet.has(row.key)"
            :toggle="() => toggleRow(row)"
            :selected="selectedSet.has(row.key)"
            :select="(event?: MouseEvent) => selectRow(row, event)"
            :has-children="hasChildren(row.node)"
          />
        </summary>
        <!-- 递归子树复用同一 #row slot（转发父级 slot props，避免业务方重复注入） -->
        <GitTree
          :nodes="row.node.children || []"
          :mode="mode"
          :default-expanded="defaultExpanded"
          :default-collapsed-keys="defaultCollapsedKeys"
          :selectable="selectable"
          :multiple="multiple"
          :indent="indent"
          :depth="row.depth + 1"
          @select="emit('select', $event)"
          @expand="emit('expand', $event)"
          @update:selected-keys="emit('update:selectedKeys', $event)"
          @row-click="(r, e) => emit('rowClick', r, e)"
          @contextmenu="(r, e) => emit('contextmenu', r, e)"
        >
          <template #row="slotProps">
            <slot name="row" v-bind="slotProps" />
          </template>
        </GitTree>
      </details>
      <a
        v-else-if="row.node.interactive !== false"
        class="git-tree-row group"
        :class="[leafClass(row), row.node.rowClass]"
        :title="row.node.title"
        @click="onLeafClick(row, $event)"
      >
        <slot
          name="row"
          :row="row"
          :depth="row.depth"
          :expanded="false"
          :toggle="() => toggleRow(row)"
          :selected="selectedSet.has(row.key)"
          :select="(event?: MouseEvent) => selectRow(row, event)"
          :has-children="false"
        />
      </a>
      <div
        v-else
        class="git-tree-row git-tree-inert"
        :class="row.node.rowClass"
      >
        <slot
          name="row"
          :row="row"
          :depth="row.depth"
          :expanded="false"
          :toggle="() => toggleRow(row)"
          :selected="false"
          :select="() => undefined"
          :has-children="false"
        />
      </div>
    </li>
  </ul>
</template>

<script setup lang="ts" generic="T">
import { computed, inject, provide, ref, watch, type Ref } from "vue";

/** 通用树节点：key 唯一标识，data 携带业务数据，children 可选 */
export interface GitTreeNode<T = unknown> {
  key: string;
  data: T;
  children?: GitTreeNode<T>[];
  /** 无 children 但可展开（懒加载占位，展开时触发 expand 事件由调用方填充） */
  expandable?: boolean;
  /** 行悬停提示 */
  title?: string;
  /** 不可交互行：无 hover 高亮、不响应点击/右键（如分组标题） */
  interactive?: boolean;
  /** 数据声明的行级高亮类（如当前分支），追加到行容器 */
  rowClass?: string;
}

/** 拍平后的行：depth 为层级深度（list 模式根为 0、叶子为 1） */
export interface GitTreeFlatRow<T = unknown> {
  key: string;
  depth: number;
  node: GitTreeNode<T>;
}

/** #row slot 的透传 props（递归子树转发时复用，避免模板内联类型自引用） */
export type GitTreeRowSlot<T = unknown> = {
  row: GitTreeFlatRow<T>;
  depth: number;
  expanded: boolean;
  toggle: () => void;
  selected: boolean;
  select: (event?: MouseEvent) => void;
  hasChildren: boolean;
};

/** GitTree 暴露给调用方的方法（供 ref 调用） */
export interface GitTreeExpose {
  clearSelection(): void;
  /** 折叠所有可展开节点；keepRoots 为 true 时保留根节点展开（根常为分组头） */
  collapseAll(keepRoots?: boolean): void;
  expandAll(): void;
}

/** 跨层共享的树状态：递归层级复用同一份展开/选中状态与全局叶序 */
interface TreeState {
  expandedSet: Ref<Set<string>>;
  userToggledKeys: Ref<Set<string>>;
  selectedSet: Ref<Set<string>>;
  selectionAnchor: Ref<string>;
  leafKeys: Ref<string[]>;
}

const stateKey = Symbol("git-tree-state");

const props = withDefaults(defineProps<{
  nodes: GitTreeNode<T>[];
  /** tree 按层级缩进展示；list 平铺所有叶子节点（忽略层级） */
  mode?: "tree" | "list";
  /** 初始是否展开所有可展开节点 */
  defaultExpanded?: boolean;
  /** defaultExpanded 时仍保持折叠的节点 key（如远程分组默认收起） */
  defaultCollapsedKeys?: string[];
  /** 是否允许选中行（选中集由组件维护，通过 update:selectedKeys 同步） */
  selectable?: boolean;
  /** selectable 时是否支持 Shift 范围多选 */
  multiple?: boolean;
  /** 每级缩进像素（list/子树缩进由 menu 嵌套天然提供；indent=0 关闭子树缩进，泳道对齐场景用） */
  indent?: number;
  /** 内部：当前渲染层级（根为 0，递归子层 +1） */
  depth?: number;
}>(), {
  mode: "tree",
  defaultExpanded: false,
  defaultCollapsedKeys: () => [],
  selectable: false,
  multiple: false,
  indent: 14,
  depth: 0,
});

const emit = defineEmits<{
  /** 普通点击（非 Shift）触发；multiple 的 Shift 范围选择不触发 */
  (e: "select", key: string): void;
  /** 懒加载节点被展开且尚无 children 时触发，调用方异步填充 children */
  (e: "expand", key: string): void;
  (e: "update:selectedKeys", keys: string[]): void;
  /** 叶子节点（不可展开且非 selectable）被点击时触发 */
  (e: "rowClick", row: GitTreeFlatRow<T>, event: MouseEvent): void;
  /** 行右键（interactive 节点） */
  (e: "contextmenu", row: GitTreeFlatRow<T>, event: MouseEvent): void;
}>();

/** #row 具名 slot 的运行时 props（与 GitTreeRowSlot 一致，供递归转发与使用方注入） */
defineSlots<{
  row: (props: GitTreeRowSlot<T>) => unknown;
}>();

function hasChildren(node: GitTreeNode<T>): boolean {
  return (node.children?.length ?? 0) > 0;
}

// ==================== 共享状态：根层创建，递归子层注入 ====================
const injected = inject<TreeState | undefined>(stateKey, undefined);
const state: TreeState = injected ?? {
  expandedSet: ref<Set<string>>(new Set()),
  userToggledKeys: ref<Set<string>>(new Set()),
  selectedSet: ref<Set<string>>(new Set()),
  selectionAnchor: ref(""),
  leafKeys: ref<string[]>([]),
};
if (!injected) provide(stateKey, state);

const { expandedSet, userToggledKeys, selectedSet, selectionAnchor, leafKeys } = state;

const isRoot = computed(() => props.depth === 0);

/**
 * 全树拍平（仅根层计算）：tree 模式含中间层行；list 模式根显示、其余只收叶子。
 * 供 Shift 范围选择的全局叶序使用；子层注入共享结果。
 */
function flattenAll(items: GitTreeNode<T>[], listMode: boolean): GitTreeFlatRow<T>[] {
  const rows: GitTreeFlatRow<T>[] = [];
  const walk = (nodes: GitTreeNode<T>[], depth: number) => {
    for (const node of nodes) {
      if (depth === 0 || !listMode) {
        rows.push({ key: node.key, depth, node });
        if (hasChildren(node) && expandedSet.value.has(node.key)) {
          walk(node.children!, depth + 1);
        }
      } else if (hasChildren(node)) {
        walk(node.children!, depth + 1);
      } else {
        rows.push({ key: node.key, depth: 1, node });
      }
    }
  };
  walk(items, 0);
  return rows;
}

watch(
  () => [props.nodes, expandedSet.value] as const,
  () => {
    if (isRoot.value) leafKeys.value = flattenAll(props.nodes, props.mode === "list")
      .filter((row) => !hasChildren(row.node))
      .map((row) => row.key);
  },
  { immediate: true },
);

// ==================== 展开状态 ====================
/** 用户主动 toggle 过的节点：之后不再受 defaultExpanded 自动展开影响 */
function syncDefaultExpanded(items: GitTreeNode<T>[]) {
  if (!props.defaultExpanded) return;
  const collapsed = new Set(props.defaultCollapsedKeys);
  const next = new Set(expandedSet.value);
  const collect = (nodes: GitTreeNode<T>[]) => {
    for (const node of nodes) {
      if (
        (hasChildren(node) || node.expandable) &&
        !collapsed.has(node.key) &&
        !userToggledKeys.value.has(node.key)
      ) {
        next.add(node.key);
      }
      if (hasChildren(node)) collect(node.children!);
    }
  };
  collect(items);
  expandedSet.value = next;
}

watch(() => props.nodes, (nodes) => syncDefaultExpanded(nodes), { immediate: true });

// ==================== 当前层可见行 ====================
/**
 * menu 嵌套语义：本层 ul 只渲染本层 nodes；展开的子行由 details 内的
 * 子 GitTree 递归渲染。list 模式子层跳过目录中间层只收叶子（VSCode 风格）。
 */
const visibleRows = computed<GitTreeFlatRow<T>[]>(() => {
  if (props.mode === "list" && !isRoot.value) {
    const rows: GitTreeFlatRow<T>[] = [];
    const walk = (nodes: GitTreeNode<T>[], depth: number) => {
      for (const node of nodes) {
        if (hasChildren(node)) {
          walk(node.children!, depth);
        } else {
          rows.push({ key: node.key, depth: 1, node });
        }
      }
    };
    walk(props.nodes, 0);
    return rows;
  }
  return props.nodes.map((node) => ({ key: node.key, depth: props.depth, node }));
});

// ==================== 展开/折叠 ====================
function setExpanded(key: string, open: boolean) {
  const next = new Set(expandedSet.value);
  if (open) next.add(key);
  else next.delete(key);
  expandedSet.value = next;
}

function toggleRow(row: GitTreeFlatRow<T>) {
  // 用户主动操作过的节点，之后不受 defaultExpanded 自动展开影响
  userToggledKeys.value.add(row.key);
  const isExpanded = expandedSet.value.has(row.key);
  if (!isExpanded && !hasChildren(row.node) && row.node.expandable) {
    // 懒加载占位节点：展开时通知调用方填充子节点
    emit("expand", row.key);
  }
  setExpanded(row.key, !isExpanded);
}

/** details 原生切换：同步到共享展开状态（受控 :open + @toggle 双向一致） */
function onDetailsToggle(node: GitTreeNode<T>, event: Event) {
  if (!(event.target instanceof HTMLDetailsElement)) return;
  userToggledKeys.value.add(node.key);
  if (event.target.open && !hasChildren(node) && node.expandable) {
    emit("expand", node.key);
  }
  setExpanded(node.key, event.target.open);
}

// ==================== 行视觉（对齐 daisyUI menu） ====================
/** 叶子行：选中高亮（与文件树一致）；其余 hover 由 menu 类承担 */
function leafClass(row: GitTreeFlatRow<T>) {
  if (props.selectable && selectedSet.value.has(row.key)) {
    return "bg-primary/10 text-primary";
  }
  return "";
}

function onLeafClick(row: GitTreeFlatRow<T>, event: MouseEvent) {
  if (row.node.interactive === false) return;
  if (props.selectable) {
    // 叶子 + 可选中：整行点击 = 选中（select 内部处理 Shift 范围）
    selectRow(row, event);
  } else {
    // 叶子 + 不可选中：交给使用方
    emit("rowClick", row, event);
  }
}

function onRowContextMenu(row: GitTreeFlatRow<T>, event: MouseEvent) {
  if (row.node.interactive === false) return;
  // 统一阻止原生浏览器菜单，避免与使用方自定义菜单同时出现
  event.preventDefault();
  emit("contextmenu", row, event);
}

// ==================== 选择 ====================
function selectRow(row: GitTreeFlatRow<T>, event?: MouseEvent) {
  if (!props.selectable) {
    emit("select", row.key);
    return;
  }
  if (!props.multiple || !event?.shiftKey) {
    // 普通点击：单选并作为 Shift 范围锚点
    selectedSet.value = new Set([row.key]);
    selectionAnchor.value = row.key;
    emit("update:selectedKeys", [...selectedSet.value]);
    emit("select", row.key);
    return;
  }
  // Shift：以最后一次普通点击为锚点做范围选择
  const keys = leafKeys.value;
  const currentIndex = keys.indexOf(row.key);
  if (currentIndex < 0) return;
  const anchorIndex = selectionAnchor.value ? keys.indexOf(selectionAnchor.value) : -1;
  const start = anchorIndex >= 0 ? Math.min(anchorIndex, currentIndex) : currentIndex;
  const end = Math.max(anchorIndex, currentIndex);
  const range = keys.slice(start, end + 1);
  selectedSet.value = new Set(range);
  if (selectionAnchor.value === "") selectionAnchor.value = row.key;
  emit("update:selectedKeys", [...selectedSet.value]);
}

// ==================== 展开/折叠全部 ====================
function clearSelection() {
  selectedSet.value = new Set();
  selectionAnchor.value = "";
  emit("update:selectedKeys", []);
}

function collapseAll(keepRoots = false) {
  const next = new Set(expandedSet.value);
  const collect = (items: GitTreeNode<T>[], depth: number) => {
    for (const node of items) {
      if (hasChildren(node)) {
        if (!(keepRoots && depth === 0)) {
          next.delete(node.key);
          // 折叠全部视为用户主动折叠，后续 nodes 刷新不再自动展开
          userToggledKeys.value.add(node.key);
        }
        collect(node.children!, depth + 1);
      }
    }
  };
  collect(props.nodes, 0);
  expandedSet.value = next;
}

function expandAll() {
  const next = new Set(expandedSet.value);
  const collect = (items: GitTreeNode<T>[]) => {
    for (const node of items) {
      if (hasChildren(node) || node.expandable) next.add(node.key);
      if (hasChildren(node)) collect(node.children!);
    }
  };
  collect(props.nodes);
  expandedSet.value = next;
}

defineExpose(
  isRoot.value
    ? { clearSelection, collapseAll, expandAll }
    : undefined,
);
</script>

<style scoped>
/* 根 ul：daisyUI .menu 自带 width:fit-content（内容撑多宽就多宽），
   必须强制占满容器。!important 是为了压过 utilities 层内的任何后续声明，
   否则行内 max-content 会把 ul 撑到几千像素
   同时用内联 style 兜底，避免 scoped hash 失效或层叠顺序导致的覆盖丢失 */
.menu {
  width: 100% !important;
}
/* 阻断 min-width:auto 传递：daisyUI li/details 是 flex item，nowrap 内容的
   min-content 会沿链把 li 撑出容器（横向滚动条 + 按钮推出屏外），全链断开
   额外加 width/max-width 兜底，避免 fit-content 链路下被内容撑宽 */
li,
.git-tree-details,
.git-tree-row {
  min-width: 0;
}
li,
.git-tree-details {
  width: 100%;
}
/* daisyUI menu 行容器覆盖：行高保持 Git 域约定 28px（SWIMLANE_HEIGHT 依赖），
   内容自定义 slot 用 flex 布局接管 menu 的 grid，hover/圆角/箭头仍由 menu 提供 */
.git-tree-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  height: 1.75rem;
  min-height: 1.75rem;
  width: 100%;
  padding-block: 0;
  overflow: hidden;
}
/* summary 的 menu 箭头在 flex 布局下失去 justify-self，手动推到行尾 */
.git-tree-row::after {
  margin-inline-start: auto;
}
/* 嵌套子层：menu 语义缩进（tree 模式默认）；indent=0 时关闭缩进与引导竖线（泳道对齐场景）
   注意：ul 已带 w-full，缩进必须用 margin/width 联动，否则右缘溢出父容器 */
.git-tree-details > ul {
  margin-inline-start: 1rem;
  padding-inline-start: 0.5rem;
  width: calc(100% - 1rem);
}
.git-tree-details[data-flat] > ul {
  margin-inline-start: 0;
  padding-inline-start: 0;
}
.git-tree-details[data-flat] > ul::before {
  display: none;
}
/* 不可交互行：不响应 hover 与点击 */
.git-tree-inert {
  pointer-events: none;
}
</style>