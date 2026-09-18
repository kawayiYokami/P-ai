import { describe, expect, it } from "vitest";
import {
  computeSessionTopColumnOffset,
  isSessionTopColumnOffsetSettled,
  type TimelineAlignRect,
} from "./session-top-column-offset";

/** 下排操作条里的时间线按钮：btn-sm btn-circle，高 2rem */
function target(right: number, bottom: number, size = 32): TimelineAlignRect {
  return { right, top: bottom - size, bottom };
}

/** 上排的 h-10 锚点容器：按钮在其中垂直居中，故容器中心即按钮中心 */
function box(right: number, bottom: number, size = 40): TimelineAlignRect {
  return { right, top: bottom - size, bottom };
}

describe("上排竖列位移计算", () => {
  it("首次计算取右边缘差与中心差", () => {
    // 下排按钮：右边 584、中心 924；上排锚点：右边 592、中心 916
    const offset = computeSessionTopColumnOffset(target(584, 940), box(592, 936), { x: 0, y: 0 });
    expect(offset).toEqual({ x: -8, y: 8 });
  });

  it("已应用位移后重算结果稳定，不会累积", () => {
    const t = target(584, 940);
    const b = box(592, 936);
    const first = computeSessionTopColumnOffset(t, b, { x: 0, y: 0 });
    // 位移生效后 rect 已跟着移动，此时再算必须得到同一个值
    const shifted = box(b.right + first.x, b.bottom + first.y);
    expect(computeSessionTopColumnOffset(t, shifted, first)).toEqual(first);
  });

  it("连续多次重算保持收敛", () => {
    const t = target(584, 940);
    let boxRect = box(592, 936);
    let applied = { x: 0, y: 0 };
    for (let i = 0; i < 3; i += 1) {
      const next = computeSessionTopColumnOffset(t, boxRect, applied);
      if (isSessionTopColumnOffsetSettled(next, applied)) break;
      boxRect = box(boxRect.right + (next.x - applied.x), boxRect.bottom + (next.y - applied.y));
      applied = next;
    }
    expect(applied).toEqual({ x: -8, y: 8 });
  });

  it("下排操作条换行导致按钮下移时跟随更新", () => {
    const t = target(584, 940);
    const b = box(592, 936);
    const first = computeSessionTopColumnOffset(t, b, { x: 0, y: 0 });
    const shifted = box(b.right + first.x, b.bottom + first.y);
    // 操作条换成两行后，下排按钮整体下移 40px
    const next = computeSessionTopColumnOffset(target(584, 980), shifted, first);
    expect(next).toEqual({ x: -8, y: 48 });
  });

  it("两侧右边一致时只做垂直位移", () => {
    const offset = computeSessionTopColumnOffset(target(300, 940), box(300, 936), { x: 0, y: 0 });
    expect(offset).toEqual({ x: 0, y: 8 });
  });

  it("半像素以内视为已对齐", () => {
    expect(isSessionTopColumnOffsetSettled({ x: 8, y: 3 }, { x: 8.4, y: 3.4 })).toBe(true);
    expect(isSessionTopColumnOffsetSettled({ x: 8, y: 3 }, { x: 8.6, y: 3 })).toBe(false);
    expect(isSessionTopColumnOffsetSettled({ x: 8, y: 3 }, { x: 8, y: 3.6 })).toBe(false);
  });
});
