import { describe, expect, it } from "vitest";
import {
  PROCESS_SEGMENT_KEEP_VISIBLE_MIN_CHARS,
  resolveVisibleProcessSegmentIndexes,
  shouldFoldProcessSegments,
} from "./process-segment-fold";

const base = {
  foldEnabled: true,
  pieceCount: 3,
  intent: "auto" as const,
};

describe("process segment fold", () => {
  it("气泡模式流式中也默认折叠", () => {
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: true,
      streaming: true,
    })).toBe(true);
  });

  it("无气泡流式中默认摊开，结束后再折叠", () => {
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: false,
      streaming: true,
    })).toBe(false);
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: false,
      streaming: false,
    })).toBe(true);
  });

  it("手动展开后，流式结束也不再收起", () => {
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: false,
      streaming: false,
      intent: "expanded",
    })).toBe(false);
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: true,
      streaming: false,
      intent: "expanded",
    })).toBe(false);
  });

  it("手动收起后，无气泡流式中也保持折叠", () => {
    expect(shouldFoldProcessSegments({
      ...base,
      bubbleBackground: false,
      streaming: true,
      intent: "collapsed",
    })).toBe(true);
  });

  it("开关关闭或只有一段时不折叠", () => {
    expect(shouldFoldProcessSegments({
      ...base,
      foldEnabled: false,
      bubbleBackground: true,
      streaming: false,
    })).toBe(false);
    expect(shouldFoldProcessSegments({
      ...base,
      pieceCount: 1,
      bubbleBackground: false,
      streaming: false,
    })).toBe(false);
  });
});

const short = (text = "跑了测试") => ({ text });
const long = (minChars = PROCESS_SEGMENT_KEEP_VISIBLE_MIN_CHARS, mark = "长") => ({ text: mark.repeat(minChars) });

describe("resolveVisibleProcessSegmentIndexes", () => {
  it("未折叠时全部可见", () => {
    expect(resolveVisibleProcessSegmentIndexes({
      pieces: [short(), short(), short()],
      folded: false,
    })).toEqual([0, 1, 2]);
  });

  it("折叠时只露长段与最后一段，短段被折起来", () => {
    expect(resolveVisibleProcessSegmentIndexes({
      pieces: [short(), long(), short(), short()],
      folded: true,
    })).toEqual([1, 3]);
  });

  it("恰好 200 字算长段，199 字算短段", () => {
    expect(resolveVisibleProcessSegmentIndexes({
      pieces: [long(PROCESS_SEGMENT_KEEP_VISIBLE_MIN_CHARS - 1), long(), short()],
      folded: true,
    })).toEqual([1, 2]);
  });

  it("全都是长段时不折任何东西，索引仍是全量", () => {
    expect(resolveVisibleProcessSegmentIndexes({
      pieces: [long(), long(), long()],
      folded: true,
    })).toEqual([0, 1, 2]);
  });

  it("最后一段无论多短都展开", () => {
    expect(resolveVisibleProcessSegmentIndexes({
      pieces: [short(), short(), short("好")],
      folded: true,
    })).toEqual([2]);
  });
});
