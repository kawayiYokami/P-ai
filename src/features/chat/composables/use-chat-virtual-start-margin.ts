import { onBeforeUnmount, ref, watch, type Ref } from "vue";

/**
 * 虚拟列表前置偏移（virtua 的 `startMargin`）测量。
 *
 * `startMargin` 的语义是「列表起点离滚动容器内容顶部的距离」。不传时 virtua 当作 0，
 * 于是 `scrollToIndex` 的落点、`findItemIndex` 的结果都会系统性少算这段距离（误差恒等于
 * 前置高度，实测见 `.pai/skills/virtua/references/behavior.md` 第 6 节）。这段距离在
 * 聊天区里来自 `chatContentRoot` 的 `py-3` 上留白，未来前置内容变化时也会跟着变，所以
 * 必须实测，不能写死。
 */

type UseChatVirtualStartMarginOptions = {
  /** 滚动容器本身（`overflow-y: auto` 的那个元素） */
  scrollContainer: Ref<HTMLElement | null | undefined>;
  /** 虚拟列表根元素；它顶部就是列表坐标系的 0 */
  listAnchor: Ref<HTMLElement | null | undefined>;
  /**
   * 列表所在的内容根（`chatContentRoot`）：它包住列表前面的一切流内内容。
   * 观测它的 border-box，任何前置内容增减、留白变化都会让它改变尺寸，从而触发重量。
   */
  prefixContainer?: Ref<HTMLElement | null | undefined>;
};

/**
 * 由两边元素相对视口的 top 反推「列表起点在滚动内容坐标里的位置」。
 *
 * `getBoundingClientRect()` 的 top 会随滚动整体位移，所以要把 `scrollTop` 加回去才能得到
 * 与滚动无关的距离；容器有边框时 rect 差里还包含 `clientTop`，一并减掉。
 * 结果取整到 px：亚像素抖动没有意义，反而会让 virtua 反复重排。
 */
export function computeChatVirtualStartMargin(input: {
  anchorTop: number;
  containerTop: number;
  containerScrollTop: number;
  containerClientTop?: number;
}): number {
  const raw =
    input.anchorTop - input.containerTop - (input.containerClientTop || 0) + input.containerScrollTop;
  if (!Number.isFinite(raw)) return 0;
  return Math.max(0, Math.round(raw));
}

export function useChatVirtualStartMargin(options: UseChatVirtualStartMarginOptions) {
  const startMargin = ref(0);
  let resizeObserver: ResizeObserver | null = null;

  function readStartMargin(): number | null {
    const container = options.scrollContainer.value;
    const anchor = options.listAnchor.value;
    if (!container || !anchor) return null;
    if (!container.isConnected || !anchor.isConnected) return null;
    // 容器不可见（display:none、尚未布局）时 rect 全是 0，此时量出的距离没有意义
    if (container.clientHeight === 0) return null;
    const containerRect = container.getBoundingClientRect();
    const anchorRect = anchor.getBoundingClientRect();
    return computeChatVirtualStartMargin({
      anchorTop: anchorRect.top,
      containerTop: containerRect.top,
      containerScrollTop: container.scrollTop,
      containerClientTop: container.clientTop,
    });
  }

  function measure() {
    const next = readStartMargin();
    if (next === null || next === startMargin.value) return;
    startMargin.value = next;
  }

  function stopObserving() {
    resizeObserver?.disconnect();
    resizeObserver = null;
  }

  // 触发时机有三类，都不做定时轮询：
  // 1. 容器尺寸变化（窗口宽度变化 → 前置留白/卡片换行高度跟着变）；
  // 2. 内容根（`chatContentRoot`）的 border-box 变化 —— 它包住列表前面的一切流内内容，
  //    高度在流式输出时每帧都会变，但这里只读顶部 rect 并做等值比较，几何没变就不写回，
  //    不会引起 virtua 重排；代价是每次尺寸事件两次 getBoundingClientRect；
  // 3. 容器或列表根元素挂载/换实例（`virtuaKey` 变化），由下面的 watch 触发。
  // 草稿收件人卡片是 `absolute inset-0` 覆盖层、不占流内空间（DraftRecipientCard.vue:2），
  // 它的挂载与高度变化本就不该进 startMargin。
  function syncObservation() {
    stopObserving();
    const container = options.scrollContainer.value;
    const anchor = options.listAnchor.value;
    if (!container || !anchor || typeof ResizeObserver === "undefined") {
      measure();
      return;
    }
    const observer = new ResizeObserver(() => measure());
    observer.observe(container);
    const prefix = options.prefixContainer?.value;
    if (prefix) observer.observe(prefix, { box: "border-box" });
    // observe() 会立刻投递一次初始回调，首帧的量取由它完成
    resizeObserver = observer;
    measure();
  }

  watch(
    () => [options.scrollContainer.value, options.listAnchor.value, options.prefixContainer?.value] as const,
    () => syncObservation(),
    { immediate: true, flush: "post" },
  );

  onBeforeUnmount(() => {
    stopObserving();
  });

  return { startMargin, measureStartMargin: measure };
}
