/** 流式气泡只增不减的尺寸锁。锁只属于当前正在输出的那一段。 */

export type StreamingSizeLockState = {
  pieceIndex: number;
  height: number;
  width: number;
};

export function createStreamingSizeLock(): StreamingSizeLockState {
  return { pieceIndex: -1, height: 0, width: 0 };
}

/** 换段后的那一帧旧高度不能套上新段，否则折叠收起的上一段会把新气泡撑出空白。 */
export function streamingSizeLockStyle(
  state: StreamingSizeLockState,
  pieceIndex: number,
): { minHeight?: string; minWidth?: string } | undefined {
  if (pieceIndex < 0 || state.pieceIndex !== pieceIndex) return undefined;
  const styles: { minHeight?: string; minWidth?: string } = {};
  if (state.height > 0) styles.minHeight = `${state.height}px`;
  if (state.width > 0) styles.minWidth = `min(${state.width}px, 100%)`;
  return styles.minHeight || styles.minWidth ? styles : undefined;
}

/**
 * 同一段只升不降。pieceIndex 变了说明上一段已经收起或被工具切开，
 * 旧高度作废，改记新段自己的尺寸。
 */
export function observeStreamingSize(
  state: StreamingSizeLockState,
  pieceIndex: number,
  height: number,
  width: number,
  parentWidth: number,
): StreamingSizeLockState {
  const nextHeight = Math.max(0, height);
  const cappedWidth = parentWidth > 0 ? Math.min(Math.max(0, width), parentWidth) : Math.max(0, width);
  if (state.pieceIndex !== pieceIndex) {
    return { pieceIndex, height: nextHeight, width: cappedWidth };
  }
  const widthLocked = parentWidth > 0 && state.width > parentWidth ? parentWidth : state.width;
  const heightLocked = Math.max(state.height, nextHeight);
  const widthNext = Math.max(widthLocked, cappedWidth);
  if (heightLocked === state.height && widthNext === state.width) return state;
  return { pieceIndex, height: heightLocked, width: widthNext };
}

/** 尺寸锁 watch 的关键值元组：消息、流式开关、当前段下标、观测目标元素。 */
export type StreamingLockWatchKey = readonly [string, boolean, number, HTMLElement | null];

/**
 * 消息根层的高度锁：只增不减。
 *
 * 段级锁只管当前那一段，换段那一刻旧段锁作废、新段从零开始，中间还夹着一次折叠，
 * 消息根会在这一下真的变矮，它下面的内容跟着上移，滚动区随即反弹。
 * 这一层锁的是整条消息，撑住的是「消息容器不许缩」。
 */
export function nextMessageRootLockHeight(locked: number, height: number): number {
  const next = Math.max(0, Math.round(height));
  return next > locked ? next : locked;
}

/**
 * 判断尺寸锁的 watch 是否该继续往下走。
 *
 * 每触发一次就要 teardown + 重装一次 ResizeObserver，重装期间丢掉的尺寸回调会让
 * 只增不减的锁彻底追不上真实高度：实测锁停在 2186px，气泡已经长到 3010px，
 * 中间态回落的一行（-22px）因此挡不住。所以四项关键值都没变时，必须在回调入口返回。
 *
 * 不要删掉调用点：删了不会报错，只会让高度锁重新变成摆设。
 */
export function streamingLockWatchChanged(
  prev: StreamingLockWatchKey | undefined,
  next: StreamingLockWatchKey,
): boolean {
  if (!prev) return true;
  return prev[0] !== next[0] || prev[1] !== next[1] || prev[2] !== next[2] || prev[3] !== next[3];
}
