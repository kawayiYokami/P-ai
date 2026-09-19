/**
 * Git 面板分支列表的排序与分组规则（纯函数，无副作用）。
 *
 * 规则：
 * 1. 当前分支永远置顶；
 * 2. 其余按末次提交时间倒序（新的放前面）；
 * 3. 时间相同退化为名称升序，保证刷新顺序稳定；
 * 4. 排序完成后按名字里的 `/` 折叠成目录树，顺序继承排序结果；
 * 5. 当前分支不折进文件夹（取全名当顶层叶子）；
 * 6. 单链目录合并成一行（中间只有一个孩子的目录不摆多层）。
 */

export type BranchLike = {
  name: string;
  isCurrent: boolean;
  /** ISO 8601；解析失败按「最旧」处理 */
  committerDate: string;
};

export type BranchTreeNode<T extends BranchLike> =
  | { kind: "branch"; branch: T; label: string }
  | { kind: "folder"; name: string; path: string; children: BranchTreeNode<T>[] };

/** 分支时间戳；解析失败返回 -Infinity，使其排在最后 */
function branchTime(branch: BranchLike): number {
  const time = Date.parse(branch.committerDate);
  return Number.isNaN(time) ? Number.NEGATIVE_INFINITY : time;
}

/** 当前分支置顶 → 末次提交时间倒序 → 名称升序 */
export function sortBranchesForDisplay<T extends BranchLike>(branches: T[]): T[] {
  return [...branches].sort((a, b) => {
    if (a.isCurrent !== b.isCurrent) return a.isCurrent ? -1 : 1;
    const timeA = branchTime(a);
    const timeB = branchTime(b);
    if (timeA !== timeB) return timeB - timeA;
    if (a.name === b.name) return 0;
    return a.name < b.name ? -1 : 1;
  });
}

/** 构建期的中间节点 */
type BuildNode<T extends BranchLike> =
  | { type: "leaf"; branch: T }
  | { type: "folder"; segment: string; items: BuildNode<T>[] };

/** 把一条分支按 `/` 拆开挂到目录树上；插入顺序决定同级顺序 */
function insertPath<T extends BranchLike>(items: BuildNode<T>[], segments: string[], branch: T): void {
  if (segments.length <= 1) {
    items.push({ type: "leaf", branch });
    return;
  }
  const segment = segments[0];
  let folder = items.find(
    (item): item is Extract<BuildNode<T>, { type: "folder" }> =>
      item.type === "folder" && item.segment === segment,
  );
  if (!folder) {
    folder = { type: "folder", segment, items: [] };
    items.push(folder);
  }
  insertPath(folder.items, segments.slice(1), branch);
}

/** 单链目录合并：文件夹下只有一个子文件夹时，把两段名字拼成一段 */
function compactFolders<T extends BranchLike>(items: BuildNode<T>[]): BuildNode<T>[] {
  return items.map((item) => {
    if (item.type === "leaf") return item;
    let folder: Extract<BuildNode<T>, { type: "folder" }> = {
      type: "folder",
      segment: item.segment,
      items: compactFolders(item.items),
    };
    while (folder.items.length === 1 && folder.items[0].type === "folder") {
      const child = folder.items[0];
      folder = { type: "folder", segment: `${folder.segment}/${child.segment}`, items: child.items };
    }
    return folder;
  });
}

function toTreeNodes<T extends BranchLike>(items: BuildNode<T>[], prefix = ""): BranchTreeNode<T>[] {
  return items.map((item) => {
    if (item.type === "leaf") {
      // 已折进文件夹的分支只显示末段，避免与父级文件夹名重复前缀
      const label = prefix ? item.branch.name.slice(prefix.length + 1) : item.branch.name;
      return { kind: "branch", branch: item.branch, label } as const;
    }
    const path = prefix ? `${prefix}/${item.segment}` : item.segment;
    return {
      kind: "folder",
      name: item.segment,
      path,
      children: toTreeNodes(item.items, path),
    } as const;
  });
}

/**
 * 排序后建树。入参应为 sortBranchesForDisplay 的结果；
 * 当前分支单独提到最前且不参与折叠，其余按 `/` 分组。
 */
export function buildBranchTree<T extends BranchLike>(sortedBranches: T[]): BranchTreeNode<T>[] {
  const current = sortedBranches.find((branch) => branch.isCurrent);
  const rest = sortedBranches.filter((branch) => branch !== current);

  const rootItems: BuildNode<T>[] = [];
  for (const branch of rest) {
    insertPath(rootItems, branch.name.split("/"), branch);
  }

  const nodes: BranchTreeNode<T>[] = current
    ? [{ kind: "branch", branch: current, label: current.name }]
    : [];
  return nodes.concat(toTreeNodes(compactFolders(rootItems)));
}
