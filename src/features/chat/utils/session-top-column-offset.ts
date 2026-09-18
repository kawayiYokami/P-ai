/**
 * 会话悬浮操作区上排竖列（回到底部 + 时间线）的位移计算。
 *
 * 上排与下排（工作区操作条）互斥出现，上排的时间线按钮需要与下排操作条里的
 * 那个位置重合，切换状态时按钮才不跳动。偏移只能运行时测：操作条内容换行、
 * 工作区名变长都会移动按钮位置。
 */

export type TimelineAlignRect = {
  right: number;
  top: number;
  bottom: number;
};

export type TimelineAlignOffset = {
  x: number;
  y: number;
};

/**
 * 上排时间线按钮在 h-10 容器内垂直居中，容器的右边与中心即按钮的右边与中心。
 * `box` 是当前已经带上一次位移的 rect，先还原成未位移位置再求差，避免误差累积。
 */
export function computeSessionTopColumnOffset(
  target: TimelineAlignRect,
  box: TimelineAlignRect,
  applied: TimelineAlignOffset,
): TimelineAlignOffset {
  return {
    x: target.right - (box.right - applied.x),
    y: (target.top + target.bottom) / 2 - ((box.top + box.bottom) / 2 - applied.y),
  };
}

/** 两次位移差不足半像素时视为已对齐，避免浮点抖动反复触发重排 */
export function isSessionTopColumnOffsetSettled(
  next: TimelineAlignOffset,
  applied: TimelineAlignOffset,
): boolean {
  return Math.abs(next.x - applied.x) < 0.5 && Math.abs(next.y - applied.y) < 0.5;
}
