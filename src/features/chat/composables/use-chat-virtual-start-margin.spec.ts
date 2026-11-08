// @vitest-environment node
import { describe, expect, it } from "vitest";
import { ref, watch } from "vue";
import { computeChatVirtualStartMargin, useChatVirtualStartMargin } from "./use-chat-virtual-start-margin";

type FakeElementOptions = {
  top: number;
  scrollTop?: number;
  clientTop?: number;
  clientHeight?: number;
  isConnected?: boolean;
};

function makeElement(options: FakeElementOptions): HTMLElement {
  const { top, scrollTop = 0, clientTop = 0, clientHeight = 600, isConnected = true } = options;
  return {
    isConnected,
    clientTop,
    clientHeight,
    scrollTop,
    getBoundingClientRect: () => ({ top }) as DOMRect,
  } as unknown as HTMLElement;
}

describe("computeChatVirtualStartMargin", () => {
  it("取列表根元素顶部相对容器内容顶部的距离", () => {
    expect(computeChatVirtualStartMargin({ anchorTop: 112, containerTop: 100, containerScrollTop: 0 })).toBe(12);
  });

  it("滚动后 rect 整体位移，加回 scrollTop 后距离不变", () => {
    expect(computeChatVirtualStartMargin({ anchorTop: 12, containerTop: 100, containerScrollTop: 100 })).toBe(12);
    expect(computeChatVirtualStartMargin({ anchorTop: -88, containerTop: 100, containerScrollTop: 200 })).toBe(12);
  });

  it("容器带边框时减掉 clientTop，不把边框算进前置距离", () => {
    expect(
      computeChatVirtualStartMargin({ anchorTop: 113, containerTop: 100, containerScrollTop: 0, containerClientTop: 1 }),
    ).toBe(12);
  });

  it("负值钳到 0，非有限值回落 0", () => {
    expect(computeChatVirtualStartMargin({ anchorTop: 90, containerTop: 100, containerScrollTop: 0 })).toBe(0);
    expect(computeChatVirtualStartMargin({ anchorTop: Number.NaN, containerTop: 100, containerScrollTop: 0 })).toBe(0);
  });
});

describe("useChatVirtualStartMargin", () => {
  it("挂载即量出前置距离", () => {
    const { startMargin } = useChatVirtualStartMargin({
      scrollContainer: ref(makeElement({ top: 100 })),
      listAnchor: ref(makeElement({ top: 112 })),
    });
    expect(startMargin.value).toBe(12);
  });

  it("几何不变时不重复写入，避免 virtua 无谓重排", () => {
    const container = ref<HTMLElement | null>(makeElement({ top: 100 }));
    const anchor = ref<HTMLElement | null>(makeElement({ top: 112 }));
    const { startMargin, measureStartMargin } = useChatVirtualStartMargin({
      scrollContainer: container,
      listAnchor: anchor,
    });
    const seen: number[] = [];
    watch(startMargin, (value) => seen.push(value));
    measureStartMargin();
    measureStartMargin();
    measureStartMargin();
    expect(seen).toEqual([]);
    expect(startMargin.value).toBe(12);
  });

  it("换成新的列表根元素后重新量取", async () => {
    const container = ref<HTMLElement | null>(makeElement({ top: 100 }));
    const anchor = ref<HTMLElement | null>(makeElement({ top: 112 }));
    const { startMargin } = useChatVirtualStartMargin({ scrollContainer: container, listAnchor: anchor });
    expect(startMargin.value).toBe(12);

    anchor.value = makeElement({ top: 140 });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(startMargin.value).toBe(40);
  });

  it("容器不可见时保留上一次量到的值，不把 0 当成真值", async () => {
    const container = ref<HTMLElement | null>(makeElement({ top: 100 }));
    const anchor = ref<HTMLElement | null>(makeElement({ top: 112 }));
    const { startMargin } = useChatVirtualStartMargin({ scrollContainer: container, listAnchor: anchor });
    expect(startMargin.value).toBe(12);

    container.value = makeElement({ top: 0, clientHeight: 0 });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(startMargin.value).toBe(12);
  });
});

type FakeObserverInstance = { callback: () => void; disconnected: boolean };

class FakeResizeObserver {
  static instances: FakeObserverInstance[] = [];
  private readonly entry: FakeObserverInstance;

  constructor(callback: () => void) {
    this.entry = { callback, disconnected: false };
    FakeResizeObserver.instances.push(this.entry);
  }

  observe() {}

  disconnect() {
    this.entry.disconnected = true;
  }
}

describe("useChatVirtualStartMargin 跟随几何变化", () => {
  function withFakeResizeObserver(run: () => void) {
    const original = (globalThis as { ResizeObserver?: unknown }).ResizeObserver;
    FakeResizeObserver.instances = [];
    (globalThis as { ResizeObserver?: unknown }).ResizeObserver = FakeResizeObserver;
    try {
      run();
    } finally {
      (globalThis as { ResizeObserver?: unknown }).ResizeObserver = original;
    }
  }

  it("内容根尺寸事件触发重量：列表前面多出一段流内内容时距离跟着变", () => {
    withFakeResizeObserver(() => {
      const container = ref<HTMLElement | null>(makeElement({ top: 100 }));
      const anchor = ref<HTMLElement | null>(makeElement({ top: 112 }));
      const prefix = ref<HTMLElement | null>(makeElement({ top: 100 }));
      const { startMargin } = useChatVirtualStartMargin({
        scrollContainer: container,
        listAnchor: anchor,
        prefixContainer: prefix,
      });
      expect(startMargin.value).toBe(12);

      const live = FakeResizeObserver.instances.filter((entry) => !entry.disconnected);
      expect(live).toHaveLength(1);
      // 模拟列表前面插进一段 40px 的流内内容：列表根元素顶部随之下移
      anchor.value = makeElement({ top: 152 });
      live[0].callback();
      expect(startMargin.value).toBe(52);
    });
  });
});
