import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  animateCollapseEnter,
  animateCollapseLeave,
  cleanupCollapseAnimation,
  hasPendingCollapseAnimation,
} from "./useEcallCollapseAnimation";

/** 最小 HTMLElement 替身：只提供动画用到的字段 */
function fakeElement() {
  const listeners = new Set<(event: unknown) => void>();
  const el = {
    style: {} as Record<string, string>,
    dataset: {} as Record<string, string>,
    offsetHeight: 120,
    scrollHeight: 240,
    addEventListener: (_type: string, handler: (event: unknown) => void) => listeners.add(handler),
    removeEventListener: (_type: string, handler: (event: unknown) => void) => listeners.delete(handler),
    listenerCount: () => listeners.size,
  };
  return el as unknown as HTMLElement & { listenerCount: () => number };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("window", {
    setTimeout: (handler: () => void, timeout?: number) => setTimeout(handler, timeout),
    clearTimeout: (id: number) => clearTimeout(id),
    matchMedia: () => ({ matches: false }),
  });
  vi.stubGlobal("requestAnimationFrame", (cb: () => void) => setTimeout(cb, 0));
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("useEcallCollapseAnimation 的取消路径", () => {
  it("cleanup 后迟到的 done 不再触发，监听与兜底 timer 一并清掉", () => {
    const el = fakeElement();
    const done = vi.fn();

    animateCollapseLeave(el, done);
    expect(hasPendingCollapseAnimation(el)).toBe(true);
    expect(el.listenerCount()).toBe(1);

    // 连点折叠：Vue 取消上一次过渡
    cleanupCollapseAnimation(el);
    expect(hasPendingCollapseAnimation(el)).toBe(false);
    expect(el.listenerCount()).toBe(0);

    // 兜底 timer 已被清掉，推进时间也不会调用 done
    vi.advanceTimersByTime(1000);
    expect(done).not.toHaveBeenCalled();
  });

  it("正常结束时 done 只调用一次，并清空在途句柄", () => {
    const el = fakeElement();
    const done = vi.fn();

    animateCollapseEnter(el, done);
    vi.advanceTimersByTime(500);
    expect(done).toHaveBeenCalledTimes(1);
    expect(hasPendingCollapseAnimation(el)).toBe(false);

    // 再次推进不重复回调
    vi.advanceTimersByTime(500);
    expect(done).toHaveBeenCalledTimes(1);
  });

  it("reduce-motion 下直接完成，不留在途动画", () => {
    vi.stubGlobal("window", {
      setTimeout: (handler: () => void, timeout?: number) => setTimeout(handler, timeout),
      clearTimeout: (id: number) => clearTimeout(id),
      matchMedia: () => ({ matches: true }),
    });
    const el = fakeElement();
    const done = vi.fn();

    animateCollapseLeave(el, done);
    expect(done).toHaveBeenCalledTimes(1);
    expect(hasPendingCollapseAnimation(el)).toBe(false);
  });
});
