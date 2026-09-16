// 会话时间线分段：一段 = 一个压缩边界到下一个压缩边界之间的消息，对应服务端一个物理块文件。
// 面板只渲染最后若干段，向上滚到顶再逐段放开，放空了才向后端补一整段。

export type TimelineSegmentMarker = { isCompaction: boolean };

// 每段首节点的下标；首节点恒为一段起点，压缩标记节点另起一段
export function computeTimelineSegmentStarts(entries: ReadonlyArray<TimelineSegmentMarker>): number[] {
  const starts: number[] = [];
  entries.forEach((entry, idx) => {
    if (idx === 0 || entry.isCompaction) starts.push(idx);
  });
  return starts;
}

// 由「已放开的段数」反推可见区起点：段数超出实际段数时退化为全量可见
export function resolveTimelineVisibleStartIndex(starts: ReadonlyArray<number>, revealedSegmentCount: number): number {
  if (starts.length === 0) return 0;
  const count = Math.max(1, Math.floor(revealedSegmentCount) || 1);
  return starts[Math.max(0, starts.length - count)] ?? 0;
}
