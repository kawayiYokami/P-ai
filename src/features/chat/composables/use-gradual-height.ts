/**
 * 流式内容的「高度渐变跟随」。
 *
 * 流式正文与思考/工具块在流式期间都没有高度上限，内容每长一行，容器就整体上跳一行：
 * 虚拟列表跟着重测一次、滚动条跟着跳一次，观感就是「瞬跳」。这里把容器的高度钉在上限上，
 * 每帧朝内容真实高走一部分，让高度是渐变而不是一次到位。
 *
 * 三条边界（踩过的坑，不要改）：
 * 1. 用 max-height 而不是 height。内容比上限矮时元素自然高就是内容高，钉住不会留空白；
 *    而钉住期间 `scrollHeight` 仍然返回内容真实高度，插值目标一次读数就够，不必反复量。
 * 2. 与目标差小于半像素、且这一块已经不再流式，才撤掉内联 max-height 交回布局给的精确值。
 *    流式期间不能撤：撤了下一帧又得从自然高重新起跳，每次增长都变成一次瞬跳。
 * 3. 用户手动折叠/展开期间由调用方让开（不要在这里判断），同一时刻只允许一处代码写高度。
 *
 * 每帧的读写顺序由调用方保证：先读完这一帧需要的所有量，再调用 step 写样式。
 */
export type GradualHeightOptions = {
  /** 每帧走完差额的比例，越小越慢越好追不上；默认 0.12 */
  rate?: number;
  /** 单帧最少走多少像素，避免小差额拖成黏滞感；默认 6 */
  minStep?: number;
  /** 单帧最多走多少像素 */
  maxStep?: number;
  /** 差额大时希望用几帧走完；默认 10 */
  framesBudget?: number;
  /** 判定收敛的像素差；默认 0.5 */
  tolerance?: number;
  /**
   * 流式停止后还允许渐变收敛多久（毫秒），超过就直接落到精值；默认 160。
   *
   * 为什么要有上限：宿主（ChatView 的贴底跟随）在流式结束那一刻就不再跟滚了，
   * 这时候再慢慢追，追出来的那几十像素会留在视口外，用户得自己往下滚。
   */
  settleGraceMs?: number;
};

export type GradualHeight = {
  /** 走一帧。返回 true 表示还需要下一帧。live=false 时只做收尾（追上目标后交回精确高度）。 */
  step(host: HTMLElement, live: boolean): boolean;
  /** 交回布局：撤掉内联 max-height / overflow */
  release(host: HTMLElement): void;
  /** 当前是否钉着高度 */
  readonly pinned: boolean;
  /** 当前钉住的高度值 */
  readonly value: number;
};

export function createGradualHeight(options: GradualHeightOptions = {}): GradualHeight {
  const rate = options.rate ?? 0.12;
  const minStep = options.minStep ?? 6;
  const maxStep = options.maxStep ?? 24;
  const framesBudget = options.framesBudget ?? 10;
  const tolerance = options.tolerance ?? 0.5;
  const settleGraceMs = options.settleGraceMs ?? 160;

  let pinned = false;
  let value = 0;
  /** 最近一次「还在流式」的时刻；流式停下后从这里起算收尾宽限 */
  let liveSeenAt = 0;

  function release(host: HTMLElement): void {
    if (!pinned) return;
    host.style.maxHeight = "";
    host.style.overflow = "";
    pinned = false;
  }

  function pin(host: HTMLElement, next: number): void {
    value = next;
    if (!pinned) {
      // 钉住的起点就是当前自然高，所以这一帧本身不产生跳动
      host.style.overflow = "hidden";
      pinned = true;
    }
    host.style.maxHeight = `${next}px`;
  }

  function step(host: HTMLElement, live: boolean): boolean {
    // 不在流式、也没在跟随的块，不必每帧来算
    if (!live && !pinned) return false;
    if (live) liveSeenAt = performance.now();
    // 流式已经停下、又过了宽限：直接交回精值，别把最后一截留在视口外面慢慢追
    if (!live && performance.now() > liveSeenAt + settleGraceMs) {
      release(host);
      return false;
    }
    const rect = host.getBoundingClientRect();
    // 不在视口里的块直接给终值：看不见的渐变没人看，白付每帧重排的代价
    if (rect.bottom <= 0 || rect.top >= window.innerHeight) {
      release(host);
      return false;
    }
    const target = host.scrollHeight;
    const current = pinned ? value : rect.height;
    const diff = target - current;
    if (Math.abs(diff) < tolerance) {
      if (!live) {
        // 收口：把高度交回布局给的精确值，不留残差、末行不会被裁半行
        release(host);
        return false;
      }
      // 还在流式：钉在目标上等下一次增长（理由见文件头第 2 条）
      pin(host, target);
      return false;
    }
    // 差额很大时按「framesBudget 帧走完」给一个步长下限，避免大块内容框跟不上、长时间被裁
    const budgetStep = Math.abs(diff) / framesBudget;
    const stepSize = Math.min(maxStep, Math.max(minStep, Math.max(Math.abs(diff) * rate, budgetStep)));
    // 最后一帧直接落到目标上：别在几百毫秒之后还差一两像素
    const next = Math.abs(diff) > stepSize ? current + Math.sign(diff) * stepSize : target;
    pin(host, next);
    return true;
  }

  return {
    step,
    release,
    get pinned() { return pinned; },
    get value() { return value; },
  };
}
