import { describe, expect, it } from "vitest";
import { computeTimelineSegmentStarts, resolveTimelineVisibleStartIndex } from "./timeline-segments";

describe("computeTimelineSegmentStarts", () => {
  it("空列表没有段起点", () => {
    expect(computeTimelineSegmentStarts([])).toEqual([]);
  });

  it("没有压缩标记时整段只有起点 0", () => {
    expect(computeTimelineSegmentStarts([{ isCompaction: false }, { isCompaction: false }])).toEqual([0]);
  });

  it("首节点是压缩标记时不重复计入起点", () => {
    expect(computeTimelineSegmentStarts([{ isCompaction: true }, { isCompaction: false }])).toEqual([0]);
  });

  it("每个压缩标记各自另起一段", () => {
    const starts = computeTimelineSegmentStarts([
      { isCompaction: false },
      { isCompaction: true },
      { isCompaction: false },
      { isCompaction: true },
    ]);
    expect(starts).toEqual([0, 1, 3]);
  });
});

describe("resolveTimelineVisibleStartIndex", () => {
  it("无段时返回 0", () => {
    expect(resolveTimelineVisibleStartIndex([], 3)).toBe(0);
  });

  it("默认放开最后一段", () => {
    expect(resolveTimelineVisibleStartIndex([0, 10, 25], 1)).toBe(25);
  });

  it("放开段数等于总段数时从 0 开始", () => {
    expect(resolveTimelineVisibleStartIndex([0, 10, 25], 3)).toBe(0);
  });

  it("放开段数超过总段数时退化为全量可见", () => {
    expect(resolveTimelineVisibleStartIndex([0, 10, 25], 9)).toBe(0);
  });

  it("段数为 0 或非数时按 1 段处理", () => {
    expect(resolveTimelineVisibleStartIndex([0, 10, 25], 0)).toBe(25);
    expect(resolveTimelineVisibleStartIndex([0, 10, 25], Number.NaN)).toBe(25);
  });
});
