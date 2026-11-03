/**
 * 工具参数解析：把 JSON 参数摊平成「字段行」列表，供 ToolArgsView 渲染。
 *
 * 输出不保留 JSON 语法字符（大括号、方括号、键名引号、逗号），只留
 * 「键名 + 值 + 层级」。非 JSON 输入返回 null，调用方退回纯文本渲染。
 */

export type ToolArgLine = {
  /** 层级，渲染时按级换算缩进 */
  depth: number;
  /** 行首前缀，数组项用 "-" */
  prefix?: string;
  /** 左侧标签；数组项没有键名时为空 */
  label?: string;
  /** 右侧值；容器行为空 */
  value?: string;
  /** 弱化提示行（截断标注），不参与键值排版 */
  hint?: boolean;
};

/**
 * 单个字符串值的显示上限。
 *
 * 这个数字管的是「看一眼够不够判断这坨东西是什么」，不是「能把内容读全」——
 * 要读全的走结果区或原文，不靠参数区。540px 宽的气泡里 500 字会占十三行，
 * 把一条工具条目撑成半屏，所以压到 240。
 */
export const TOOL_ARG_VALUE_MAX_CHARS = 240;

/** 空对象 / 空数组的占位。用中文括号而不是破折号：破折号会和数组项的 "-" 混淆。 */
export const TOOL_ARG_EMPTY_MARK = "（空）";

const INDENT_DEPTH_LIMIT = 12;

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isScalar(value: unknown): boolean {
  return value === null || typeof value !== "object";
}

/** 字符串按码点截断，避免把代理对切一半。 */
function truncate(value: string): { text: string; total: number } {
  const chars = Array.from(value);
  if (chars.length <= TOOL_ARG_VALUE_MAX_CHARS) return { text: value, total: 0 };
  return { text: chars.slice(0, TOOL_ARG_VALUE_MAX_CHARS).join(""), total: chars.length };
}

/** 截断标注另起一行：和值放同一行会把值挤断在首行右侧。 */
function pushTruncateHint(out: ToolArgLine[], depth: number, total: number) {
  if (!total) return;
  out.push({ depth, value: `…共 ${total} 字符未显示`, hint: true });
}

function pushScalar(out: ToolArgLine[], depth: number, label: string | undefined, value: unknown, prefix?: string) {
  if (typeof value === "string") {
    const { text, total } = truncate(value);
    out.push({ depth, prefix, label, value: text });
    pushTruncateHint(out, depth + 1, total);
    return;
  }
  const text = value === null ? "null" : String(value);
  out.push({ depth, prefix, label, value: text });
}

function pushEmpty(out: ToolArgLine[], depth: number, label: string | undefined) {
  out.push({ depth, label, value: TOOL_ARG_EMPTY_MARK });
}

function walkArray(out: ToolArgLine[], depth: number, label: string | undefined, items: unknown[]) {
  if (items.length === 0) {
    pushEmpty(out, depth, label);
    return;
  }
  // 标量数组并成一行，省掉一列纯占位的破折号。
  if (items.every(isScalar)) {
    // 合并成一行的数组仍然按整行截断，标注落在行尾。
    const joined = items.map((item) => (typeof item === "string" ? truncate(item).text : String(item))).join(", ");
    const { text, total } = truncate(joined);
    out.push({ depth, label, value: text });
    pushTruncateHint(out, depth + 1, total);
    return;
  }
  // 含容器的数组：先出一行键名，再逐项展开。
  out.push({ depth, label, value: undefined });
  items.forEach((item) => {
    if (isPlainObject(item)) {
      const entries = Object.entries(item);
      if (entries.length === 0) {
        out.push({ depth: depth + 1, prefix: "-", value: TOOL_ARG_EMPTY_MARK });
        return;
      }
      entries.forEach(([key, child], index) => {
        const lineDepth = index === 0 ? depth + 1 : depth + 2;
        walkValue(out, lineDepth, key, child, index === 0 ? "-" : undefined);
      });
      return;
    }
    if (Array.isArray(item)) {
      walkArray(out, depth + 1, undefined, item);
      return;
    }
    pushScalar(out, depth + 1, undefined, item, "-");
  });
}

function walkObject(out: ToolArgLine[], depth: number, entries: [string, unknown][]) {
  entries.forEach(([key, value]) => {
    walkValue(out, depth, key, value);
  });
}

function walkValue(out: ToolArgLine[], depth: number, label: string | undefined, value: unknown, prefix?: string) {
  const safeDepth = Math.min(depth, INDENT_DEPTH_LIMIT);
  if (Array.isArray(value)) {
    walkArray(out, safeDepth, label, value);
    return;
  }
  if (isPlainObject(value)) {
    const entries = Object.entries(value);
    if (entries.length === 0) {
      pushEmpty(out, safeDepth, label);
      return;
    }
    out.push({ depth: safeDepth, label, value: undefined });
    walkObject(out, safeDepth + 1, entries);
    return;
  }
  pushScalar(out, safeDepth, label, value, prefix);
}

/**
 * 解析参数文本。返回 null 表示不是可解析的 JSON，调用方应退回纯文本。
 */
export function parseToolArgs(text: string): ToolArgLine[] | null {
  const raw = String(text ?? "");
  const trimmed = raw.trim();
  if (!trimmed) return null;
  const head = trimmed[0];
  if (head !== "{" && head !== "[") return null;

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch {
    return null;
  }

  const out: ToolArgLine[] = [];
  if (Array.isArray(parsed)) {
    walkArray(out, 0, undefined, parsed);
  } else if (isPlainObject(parsed)) {
    walkObject(out, 0, Object.entries(parsed));
  } else {
    return null;
  }
  return out.length > 0 ? out : null;
}
