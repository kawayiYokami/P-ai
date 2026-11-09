/** 过程段折叠意图。auto 走默认；expanded / collapsed 是用户点过之后的选择。 */
export type ProcessSegmentFoldIntent = "auto" | "expanded" | "collapsed";

/**
 * 折叠时保持展开的长段门槛（字符数）。
 *
 * 短的过程提示折起来只是省地方，长段（分析、结论、报错细节）一折进去就没法读了，
 * 所以折叠只作用于短段。
 */
export const PROCESS_SEGMENT_KEEP_VISIBLE_MIN_CHARS = 200;

export function shouldFoldProcessSegments(input: {
  foldEnabled: boolean;
  pieceCount: number;
  bubbleBackground: boolean;
  streaming: boolean;
  intent: ProcessSegmentFoldIntent;
}): boolean {
  if (!input.foldEnabled || input.pieceCount <= 1) return false;
  if (input.intent === "expanded") return false;
  if (input.intent === "collapsed") return true;
  // 无气泡流式先全部摊开，输出结束再收成只留最后一段。气泡模式始终默认折叠。
  if (!input.bubbleBackground && input.streaming) return false;
  return true;
}

function processSegmentTextLength(text: string): number {
  return Array.from(String(text || "").trim()).length;
}

/**
 * 折叠后仍然露出来的段下标。
 *
 * 规则：正文达到 {@link PROCESS_SEGMENT_KEEP_VISIBLE_MIN_CHARS} 的长段始终展开，
 * 最后一段永远展开，其余短段折进「上 N 条消息」。未折叠时全部可见。
 */
export function resolveVisibleProcessSegmentIndexes(input: {
  pieces: Array<{ text: string }>;
  folded: boolean;
}): number[] {
  const total = input.pieces.length;
  if (total === 0) return [];
  if (!input.folded) return input.pieces.map((_, index) => index);
  const visible: number[] = [];
  input.pieces.forEach((piece, index) => {
    const isLast = index === total - 1;
    const isLongEnough = processSegmentTextLength(piece.text) >= PROCESS_SEGMENT_KEEP_VISIBLE_MIN_CHARS;
    if (isLast || isLongEnough) visible.push(index);
  });
  return visible;
}
