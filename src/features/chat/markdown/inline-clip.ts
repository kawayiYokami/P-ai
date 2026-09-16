import type { InlineSegment } from "./parse-markdown";

/** 一个行内段在屏幕上占用的可见长度 */
function segmentLength(segment: InlineSegment): number {
  if ("children" in segment) {
    return segment.children.reduce((sum, child) => sum + segmentLength(child), 0);
  }
  switch (segment.type) {
    case "text":
    case "code":
    case "math":
    case "link":
      return segment.text.length;
    case "image":
    case "imageLink":
      return segment.alt.length;
    case "html_br":
      return 1;
    default:
      return 0;
  }
}

export function inlineSegmentsLength(segments: InlineSegment[]): number {
  return segments.reduce((sum, segment) => sum + segmentLength(segment), 0);
}

/** 可以按字切开而不破坏标记的段：只有纯文本 */
function isSplittable(segment: InlineSegment): segment is Extract<InlineSegment, { type: "text" }> {
  return segment.type === "text";
}

/**
 * 按「前 headRatio + 后 (1 - headRatio)」截断行内段，中间省略号连接。
 *
 * 切点规则：
 * - 纯文本段按字切开，切哪儿都不会破坏标记；
 * - 带标记的段（行内代码、数学、链接、粗体/斜体等）**整体保留或整体丢弃**，
 *   绝不切开，所以不会露出半截反引号或成对的 `**`。
 *
 * 代价：正好跨在切点上的标记段会整块不显示，实际字数只可能少于上限，不会超。
 */
export function clipInlineSegments(
  segments: InlineSegment[],
  limit: number,
  headRatio: number,
): InlineSegment[] {
  const budget = Math.floor(Number(limit) || 0);
  if (budget <= 0) return segments;
  if (inlineSegmentsLength(segments) <= budget) return segments;

  const ratio = Number.isFinite(headRatio) ? Math.min(Math.max(headRatio, 0), 1) : 0.3;
  const headBudget = Math.round(budget * ratio);
  const tailBudget = budget - headBudget;

  const head: InlineSegment[] = [];
  let headUsed = 0;
  let headSplit: InlineSegment | null = null;
  let headSplitLength = 0;
  for (const segment of segments) {
    if (headUsed >= headBudget) break;
    const length = segmentLength(segment);
    const remain = headBudget - headUsed;
    if (length > remain) {
      // 跨过头部预算：纯文本切一刀，带标记的整段丢弃（连同它之后的所有段）
      if (isSplittable(segment)) {
        head.push({ type: "text", text: segment.text.slice(0, remain) });
        headSplit = segment;
        headSplitLength = remain;
        headUsed += remain;
      }
      break;
    }
    head.push(segment);
    headUsed += length;
  }

  const tail: InlineSegment[] = [];
  let tailUsed = 0;
  let tailSplit: InlineSegment | null = null;
  let tailSplitLength = 0;
  for (let index = segments.length - 1; index >= 0; index -= 1) {
    if (tailUsed >= tailBudget) break;
    const segment = segments[index];
    const length = segmentLength(segment);
    const remain = tailBudget - tailUsed;
    if (length > remain) {
      if (isSplittable(segment)) {
        tail.unshift({ type: "text", text: segment.text.slice(segment.text.length - remain) });
        tailSplit = segment;
        tailSplitLength = remain;
        tailUsed += remain;
      }
      break;
    }
    tail.unshift(segment);
    tailUsed += length;
  }

  // 同一段被头尾同时整段纳入：放弃截断，避免内容重复
  if (head.some((segment) => tail.includes(segment))) return segments;

  // 同一段被头尾各切走一部分：切出的两截盖住整段时同样放弃截断
  if (
    headSplit &&
    headSplit === tailSplit &&
    headSplit.type === "text" &&
    headSplitLength + tailSplitLength > headSplit.text.length
  ) {
    return segments;
  }

  if (head.length === 0 && tail.length === 0) return segments;

  return [...head, { type: "text", text: "…" }, ...tail];
}
