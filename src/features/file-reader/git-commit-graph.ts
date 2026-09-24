import type { GitPanelLogEntry } from "../../services/tauri-api";

/**
 * 提交历史图（时间线）计算与渲染。
 * 算法与绘制直接参照 VS Code 内置 SCM 提交历史图：
 * - 泳道分配：每行 inputSwimlanes 继承上一行 outputSwimlanes，
 *   把当前提交替换为 first parent，其余父（merge）追加到泳道尾部；
 * - 每行 SVG 宽度按该行实际泳道数独立计算，多一根才推开一格；
 * - 节点圆、竖线、弧线折线绘制与 VS Code renderSCMHistoryItemGraph 一致。
 */

export const SWIMLANE_HEIGHT = 28; // GitTree 行高 h-7
export const SWIMLANE_WIDTH = 11;
const CURVE_RADIUS = 5;

/** VS Code scmGraph 色板 */
export const GRAPH_COLORS = ["#FFB000", "#DC267F", "#994F00", "#40B0A6", "#B66DFF"] as const;

export function graphColor(index: number): string {
  return GRAPH_COLORS[index % GRAPH_COLORS.length] ?? GRAPH_COLORS[0];
}

/** 泳道元素：一条活跃提交链的标识与颜色 */
export interface SwimlaneNode {
  id: string;
  colorIndex: number;
}

/** 单行上显示的 ref 徽章（颜色与图中该 ref 的线色一致，照抄 VS Code references 推断） */
export interface CommitGraphRef {
  /** 展示名：有本地分支时取本地名，纯远程分支回落到 remote/name，tag 取 tag 名 */
  name: string;
  isTag: boolean;
  /** 该徽章聚合的全部引用名（本地分支 + 各远程跟踪分支），用于 tooltip */
  detail: string[];
  colorIndex: number;
}

/** 单行图数据 */
export interface CommitGraphRow {
  id: string;
  parentIds: string[];
  input: SwimlaneNode[];
  output: SwimlaneNode[];
  circleIndex: number;
  circleColorIndex: number;
  isHead: boolean;
  isMerge: boolean;
  refs: CommitGraphRef[];
}

export interface CommitGraphResult {
  rows: CommitGraphRow[];
  /** 每行 SVG 宽度（像素） */
  widthByRow: number[];
}

/** 解析 %D refs：返回引用列表（含 tag）与 HEAD 信息 */
export interface ParsedRef {
  /** 引用全名（refs/heads/x、refs/remotes/origin/x、refs/tags/v1），作为颜色分配的稳定键 */
  id: string;
  /** 去掉 refs/heads/ 或 refs/remotes/<remote>/ 后的分支名 / tag 名 */
  name: string;
  isTag: boolean;
  /** 远程跟踪分支所属 remote；本地分支与 tag 为 undefined */
  remote?: string;
}

/** 把 git 的 ref 名解析成带类型的引用（兼容 --decorate=full 与 short 两种形式） */
function toParsedRef(raw: string): ParsedRef {
  if (raw.startsWith("refs/tags/")) {
    return { id: raw, name: raw.slice("refs/tags/".length), isTag: true };
  }
  if (raw.startsWith("refs/heads/")) {
    return { id: raw, name: raw.slice("refs/heads/".length), isTag: false };
  }
  if (raw.startsWith("refs/remotes/")) {
    const rest = raw.slice("refs/remotes/".length);
    const slash = rest.indexOf("/");
    if (slash === -1) return { id: raw, name: rest, isTag: false, remote: rest };
    return { id: raw, name: rest.slice(slash + 1), isTag: false, remote: rest.slice(0, slash) };
  }
  // short 形式兜底：无法区分本地/远程，按本地分支处理
  return { id: raw, name: raw, isTag: false };
}

export function parseRefs(refs: string): { branches: ParsedRef[]; isHead: boolean } {
  const branches: ParsedRef[] = [];
  let isHead = false;
  if (!refs.trim()) return { branches, isHead };

  for (const item of refs.split(",").map((s) => s.trim()).filter(Boolean)) {
    if (item === "HEAD") {
      isHead = true;
      continue;
    }
    if (item.startsWith("HEAD -> ")) {
      isHead = true;
      branches.push(toParsedRef(item.slice("HEAD -> ".length).trim()));
      continue;
    }
    if (item.startsWith("tag: ")) {
      branches.push(toParsedRef(item.slice("tag: ".length).trim()));
      continue;
    }
    const parsed = toParsedRef(item);
    // refs/remotes/<remote>/HEAD 只是 remote 的默认分支指针，不承载信息（VS Code 同样跳过）
    if (parsed.remote && parsed.name === "HEAD") continue;
    branches.push(parsed);
  }
  return { branches, isHead };
}

/** 同行同类型引用合并为一个徽章：同名本地分支与各远程跟踪分支聚合到一项 */
function mergeRefs(refs: ParsedRef[], colorOf: (ref: ParsedRef) => number): CommitGraphRef[] {
  const merged: CommitGraphRef[] = [];
  const indexByKey = new Map<string, number>();

  for (const ref of refs) {
    const label = ref.remote ? `${ref.remote}/${ref.name}` : ref.name;
    const key = `${ref.isTag ? "tag" : "branch"}:${ref.name}`;
    let index = indexByKey.get(key);
    if (index === undefined) {
      index = merged.length;
      indexByKey.set(key, index);
      merged.push({ name: label, isTag: ref.isTag, detail: [], colorIndex: colorOf(ref) });
    } else if (!ref.isTag && !ref.remote) {
      // 本地分支比远程跟踪分支更能代表这条分支，展示名让给本地名
      merged[index].name = label;
    }
    merged[index].detail.push(label);
  }

  return merged;
}

/** 单行内联展示的 ref 徽章上限，超出部分折叠成 +N */
export const MAX_INLINE_REFS = 3;

/** 按上限切分 ref 徽章：inline 直接展示，overflow 折叠为 +N */
export function splitInlineRefs(refs: CommitGraphRef[]): {
  inline: CommitGraphRef[];
  overflow: CommitGraphRef[];
} {
  if (refs.length <= MAX_INLINE_REFS) return { inline: refs, overflow: [] };
  return { inline: refs.slice(0, MAX_INLINE_REFS), overflow: refs.slice(MAX_INLINE_REFS) };
}

/**
 * 计算提交图布局（VS Code 泳道替换算法）。
 * @param entries 提交列表（新 → 旧，即 git log 顺序）
 */
export function computeCommitGraph(entries: GitPanelLogEntry[]): CommitGraphResult {
  // ref 全名 → 颜色：新 ref 出现时轮转分配，同 ref 复用
  const refColorMap = new Map<string, number>();
  let colorIndex = -1;
  const nextColor = (): number => {
    colorIndex = (colorIndex + 1) % GRAPH_COLORS.length;
    return colorIndex;
  };

  // 预扫描全部提交的 refs 并分配颜色：merge 的额外父可能在后续行才带 ref，
  // 若等轮到它才分配，merge 弧线会先拿到一个新色、该 ref 又被分配另一个色，
  // 导致弧线与父提交行/徽章颜色不一致。预先分配后颜色顺序与逐行分配完全一致。
  const parsedRefs = new Map(entries.map((entry) => [entry.hash, parseRefs(entry.refs)]));
  for (const { branches } of parsedRefs.values()) {
    for (const ref of branches) {
      if (!refColorMap.has(ref.id)) {
        refColorMap.set(ref.id, nextColor());
      }
    }
  }

  const rows: CommitGraphRow[] = [];

  for (const entry of entries) {
    const { branches, isHead } = parsedRefs.get(entry.hash)!;
    const refColorIndex = branches
      .map((r) => refColorMap.get(r.id))
      .find((c) => c !== undefined);

    const input = rows.length > 0 ? rows[rows.length - 1].output.map((n) => ({ ...n })) : [];
    const output: SwimlaneNode[] = [];
    let firstParentAdded = false;

    // 第一父：替换当前提交在泳道中的位置
    if (entry.parents.length > 0) {
      for (const node of input) {
        if (node.id === entry.hash) {
          if (!firstParentAdded) {
            // 当前提交有引用时用引用色，否则沿用原泳道色
            output.push({
              id: entry.parents[0],
              colorIndex: refColorIndex ?? node.colorIndex,
            });
            firstParentAdded = true;
          }
          continue;
        }
        output.push({ ...node });
      }
    }

    // 其余父（merge 提交的第二父等）：追加到泳道尾部
    for (let i = firstParentAdded ? 1 : 0; i < entry.parents.length; i++) {
      // i === 0（first parent 未被泳道替换添加）：VS Code 取当前提交的 ref 色；
      // i > 0（merge 额外父）：优先取该父提交自身的 ref 色，颜色与父提交行/徽章一致
      const colorIndexForParent = i === 0
        ? refColorIndex
        : (parsedRefs.get(entry.parents[i])?.branches ?? [])
            .map((r) => refColorMap.get(r.id))
            .find((c) => c !== undefined);
      output.push({ id: entry.parents[i], colorIndex: colorIndexForParent ?? nextColor() });
    }

    const inputIndex = input.findIndex((n) => n.id === entry.hash);
    const circleIndex = inputIndex !== -1 ? inputIndex : input.length;
    const circleColorIndex =
      circleIndex < output.length
        ? output[circleIndex].colorIndex
        : circleIndex < input.length
          ? input[circleIndex].colorIndex
          : 0;

    rows.push({
      id: entry.hash,
      parentIds: entry.parents,
      input,
      output,
      circleIndex,
      circleColorIndex,
      isHead,
      isMerge: entry.parents.length > 1,
      // refs 颜色：colorMap 已分配则用其线色，否则按 VS Code fallback 用节点圆颜色；
      // 同名本地/远程分支合并为一个徽章后再交给渲染层
      refs: mergeRefs(branches, (ref) => refColorMap.get(ref.id) ?? circleColorIndex),
    });
  }

  const widthByRow = rows.map(
    (r) => SWIMLANE_WIDTH * (Math.max(r.input.length, r.output.length, 1) + 1),
  );
  return { rows, widthByRow };
}

/** 泳道 X 坐标：VS Code 从 index+1 起算（左边预留一格） */
export function laneX(index: number): number {
  return SWIMLANE_WIDTH * (index + 1);
}

/** 子节点行延续竖线参数：位置与颜色取自父提交行 */
export function laneLine(laneIndex: number, colorIndex: number): { x: number; color: string } {
  return { x: laneX(laneIndex), color: graphColor(colorIndex) };
}

/**
 * 生成单行图的 SVG 字符串（照抄 VS Code renderSCMHistoryItemGraph 绘制逻辑）。
 * 所有值均为内部计算（泳道索引、色板常量），无外部输入注入风险。
 */
export function renderGraphRowSVG(row: CommitGraphRow): string {
  const W = SWIMLANE_WIDTH;
  const H = SWIMLANE_HEIGHT;
  const midY = H / 2;
  const R = CURVE_RADIUS;
  const parts: string[] = [];

  let outputSwimlaneIndex = 0;
  for (let index = 0; index < row.input.length; index++) {
    const color = graphColor(row.input[index].colorIndex);

    // 当前提交
    if (row.input[index].id === row.id) {
      if (index !== row.circleIndex) {
        // 基础提交：/ 弧线 + - 横线 连到节点泳道
        const d = [
          `M ${W * (index + 1)} 0`,
          `A ${W} ${W} 0 0 1 ${W * index} ${W}`,
          `H ${W * (row.circleIndex + 1)}`,
        ].join(" ");
        parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="1" stroke-linecap="round"/>`);
      } else {
        outputSwimlaneIndex++;
      }
    } else if (
      outputSwimlaneIndex < row.output.length &&
      row.input[index].id === row.output[outputSwimlaneIndex].id
    ) {
      // 非当前提交：保留泳道
      if (index === outputSwimlaneIndex) {
        // 直线 | 贯穿
        parts.push(`<path d="M ${W * (index + 1)} 0 V ${H}" fill="none" stroke="${color}" stroke-width="1" stroke-linecap="round"/>`);
      } else {
        // 泳道移动：| 竖线 + / 弧线 + - 横线 + / 弧线 + | 竖线
        const d = [
          `M ${W * (index + 1)} 0`,
          `V 6`,
          `A ${R} ${R} 0 0 1 ${W * (index + 1) - R} ${midY}`,
          `H ${W * (outputSwimlaneIndex + 1) + R}`,
          `A ${R} ${R} 0 0 0 ${W * (outputSwimlaneIndex + 1)} ${midY + R}`,
          `V ${H}`,
        ].join(" ");
        parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="1" stroke-linecap="round"/>`);
      }
      outputSwimlaneIndex++;
    }
  }

  // merge 提交的其余父：从节点泳道分叉出去的弧线
  for (let i = 1; i < row.parentIds.length; i++) {
    const parentOutputIndex = findLastIndex(row.output, row.parentIds[i]);
    if (parentOutputIndex === -1) continue;
    const color = graphColor(row.output[parentOutputIndex].colorIndex);
    const d = [
      `M ${W * parentOutputIndex} ${midY}`,
      `A ${W} ${W} 0 0 1 ${W * (parentOutputIndex + 1)} ${H}`,
      `M ${W * parentOutputIndex} ${midY}`,
      `H ${W * (row.circleIndex + 1)} `,
    ].join(" ");
    parts.push(`<path d="${d}" fill="none" stroke="${color}" stroke-width="1" stroke-linecap="round"/>`);
  }

  // 节点上方竖线（| 到 *）
  const inputIndex = row.input.findIndex((n) => n.id === row.id);
  if (inputIndex !== -1) {
    const color = graphColor(row.input[inputIndex].colorIndex);
    parts.push(`<path d="M ${W * (row.circleIndex + 1)} 0 V ${midY}" fill="none" stroke="${color}" stroke-width="1" stroke-linecap="round"/>`);
  }

  // 节点下方竖线（| 从 * 出发）
  if (row.parentIds.length > 0) {
    const color = graphColor(row.circleColorIndex);
    parts.push(`<path d="M ${W * (row.circleIndex + 1)} ${midY} V ${H}" fill="none" stroke="${color}" stroke-width="1" stroke-linecap="round"/>`);
  }

  // 节点圆
  const circleColor = graphColor(row.circleColorIndex);
  const cx = W * (row.circleIndex + 1);
  if (row.isHead) {
    // HEAD：大实心圆 + 白色中心点
    parts.push(`<circle cx="${cx}" cy="${midY}" r="7" fill="${circleColor}"/>`);
    parts.push(`<circle cx="${cx}" cy="${midY}" r="2" fill="#fff"/>`);
  } else if (row.isMerge) {
    // merge：双环
    parts.push(`<circle cx="${cx}" cy="${midY}" r="6" fill="${circleColor}"/>`);
    parts.push(`<circle cx="${cx}" cy="${midY}" r="3" fill="#fff"/>`);
  } else {
    // 普通提交
    parts.push(`<circle cx="${cx}" cy="${midY}" r="5" fill="${circleColor}"/>`);
  }

  const width = W * (Math.max(row.input.length, row.output.length, 1) + 1);
  return `<svg width="${width}" height="${H}" class="block shrink-0" style="overflow: visible">${parts.join("")}</svg>`;
}

function findLastIndex(nodes: SwimlaneNode[], id: string): number {
  for (let i = nodes.length - 1; i >= 0; i--) {
    if (nodes[i].id === id) return i;
  }
  return -1;
}
