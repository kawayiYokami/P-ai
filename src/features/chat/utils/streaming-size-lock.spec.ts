import { describe, expect, it } from "vitest";
import {
  createStreamingSizeLock,
  nextMessageRootLockHeight,
  observeStreamingSize,
  streamingLockWatchChanged,
  streamingSizeLockStyle,
  type StreamingLockWatchKey,
} from "./streaming-size-lock";

describe("streaming size lock", () => {
  it("同一段只升不降", () => {
    const grown = observeStreamingSize(createStreamingSizeLock(), 0, 80, 120, 400);
    const shrunk = observeStreamingSize(grown, 0, 40, 80, 400);
    expect(shrunk).toBe(grown);
    expect(streamingSizeLockStyle(shrunk, 0)?.minHeight).toBe("80px");
  });

  it("换段后不沿用上一段高度", () => {
    const previous = observeStreamingSize(createStreamingSizeLock(), 0, 200, 320, 400);
    const next = observeStreamingSize(previous, 1, 48, 90, 400);
    expect(next.height).toBe(48);
    expect(next.width).toBe(90);
    expect(streamingSizeLockStyle(previous, 1)).toBeUndefined();
    expect(streamingSizeLockStyle(next, 1)?.minHeight).toBe("48px");
  });

  it("窗口变窄时宽度锁跟着收，不超过父容器", () => {
    const wide = observeStreamingSize(createStreamingSizeLock(), 0, 40, 320, 400);
    const narrowed = observeStreamingSize(wide, 0, 40, 320, 180);
    expect(narrowed.width).toBe(180);
  });
});

describe("流式尺寸锁的 watch 守卫", () => {
  const el = {} as HTMLElement;
  const base: StreamingLockWatchKey = ["msg-1", true, 0, null];

  it("首次触发（没有上一份关键值）必须放行", () => {
    expect(streamingLockWatchChanged(undefined, base)).toBe(true);
  });

  it("四项关键值都没变时不许放行", () => {
    // 这是本次回归的核心：放行一次就 teardown + 重装一次 ResizeObserver，
    // 流式期间每帧都触发，高度锁会彻底追不上真实高度。
    expect(streamingLockWatchChanged(["msg-1", true, 0, null], ["msg-1", true, 0, null])).toBe(false);
    expect(streamingLockWatchChanged(["msg-1", true, 0, el], ["msg-1", true, 0, el])).toBe(false);
  });

  it("任何一项关键值变化都要放行", () => {
    expect(streamingLockWatchChanged(base, ["msg-2", true, 0, null])).toBe(true);
    expect(streamingLockWatchChanged(base, ["msg-1", false, 0, null])).toBe(true);
    expect(streamingLockWatchChanged(base, ["msg-1", true, 1, null])).toBe(true);
    expect(streamingLockWatchChanged(base, ["msg-1", true, 0, el])).toBe(true);
  });
});

describe("message root size lock", () => {
  it("首次观察到高度就记下来", () => {
    expect(nextMessageRootLockHeight(0, 320)).toBe(320);
  });

  it("消息变矮时保持锁定高度，不跟着降", () => {
    // 换段那一刻旧段锁作废、新段从零开始，整条消息会矮一块；这一层不许它矮。
    expect(nextMessageRootLockHeight(320, 298)).toBe(320);
    expect(nextMessageRootLockHeight(320, 0)).toBe(320);
  });

  it("消息真实涨高时更新到新高度", () => {
    expect(nextMessageRootLockHeight(320, 512)).toBe(512);
  });

  it("高度取整，亚像素抖动不反复写样式", () => {
    expect(nextMessageRootLockHeight(320, 320.6)).toBe(321);
    expect(nextMessageRootLockHeight(320, 319.4)).toBe(320);
  });
});
