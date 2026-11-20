/**
 * 流式内容的「高度渐变跟随」。
 *
 * 流式正文与思考/工具块在流式期间都没有高度上限，内容每长一行，容器就整体上跳一行：
 * 虚拟列表跟着重测一次、滚动条跟着跳一次，观感就是「瞬跳」。这里把容器的高度钉在上限上，
 * 让高度朝内容真实高平滑走过去。
 *
 * 做法：交给 CSS transition，不再逐帧算步长写 max-height。
 * 自己每帧写一次 max-height，等于每帧逼浏览器重排一次；而且步长按像素给（minStep 6 / maxStep 24），
 * 一行文字约 20px，一帧就走到位，看起来和瞬跳没区别。改成写目标值 + transition，
 * 浏览器按时间插值，不管 60Hz 还是 144Hz 观感一致，也有可感知的过渡。
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
  /** 高度过渡时长（毫秒）。交给 CSS transition，不逐帧步进；默认 180 */
  transitionMs?: number;
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
  /** 交回布局：撤掉内联 max-height / transition / contain */
  release(host: HTMLElement): void;
  /** 当前是否钉着高度 */
  readonly pinned: boolean;
  /** 当前钉住的目标高度值 */
  readonly value: number;
};

export function createGradualHeight(options: GradualHeightOptions = {}): GradualHeight {
  const transitionMs = options.transitionMs ?? 180;
  const tolerance = options.tolerance ?? 0.5;
  const settleGraceMs = options.settleGraceMs ?? 160;

  let pinned = false;
  let value = 0;
  /** 最近一次被钉住的元素；元素被 v-if 替换时，闭包状态不能沿用到新元素身上 */
  let hostRef: HTMLElement | null = null;
  /** 最近一次「还在流式」的时刻；流式停下后从这里起算收尾宽限 */
  let liveSeenAt = 0;

  function release(host: HTMLElement): void {
    if (!pinned) return;
    host.style.transition = "";
    host.style.maxHeight = "";
    host.style.contain = "";
    pinned = false;
  }

  function pin(host: HTMLElement, next: number): void {
    value = next;
    if (!pinned) {
      // 钉住的起点就是当前自然高，所以这一帧本身不产生跳动。
      // 用 contain: paint 裁掉超出上限的部分，而不是 overflow: hidden：后者会把这块盒子
      // 变成滚动容器，里面 sticky 的按钮在钉住期间会失效（它会贴着这个不滚动的盒子）。
      host.style.contain = "paint";
      // 过渡交给浏览器：我们只写目标值，插值由 CSS 做，不必每帧插一脚。
      host.style.transition = `max-height ${transitionMs}ms cubic-bezier(0.4, 0, 0.2, 1)`;
      pinned = true;
    }
    host.style.maxHeight = `${next}px`;
  }

  function step(host: HTMLElement, live: boolean): boolean {
    // 元素被替换（如流式前后的 pre-streaming 圆点与气泡互换）：旧元素的高度不能沿用到新元素，
    // 否则会把新元素 max-height 压到内容之下、再慢慢收回，那几帧是在裁正文。
    if (hostRef !== host) {
      hostRef = host;
      pinned = false;
    }
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
    // 目标没变就不用写：流式期间内容不是每帧都在长，多余的样式写入只会换一次样式重算。
    if (pinned && Math.abs(target - value) < tolerance) {
      if (!live) {
        // 收口：把高度交回布局给的精确值，不留残差、末行不会被裁半行
        release(host);
        return false;
      }
      return false;
    }
    if (!pinned && Math.abs(target - rect.height) < tolerance) {
      if (!live) return false;
      // 还在流式：钉在目标上等下一次增长（理由见文件头第 2 条）
      pin(host, target);
      return false;
    }
    pin(host, target);
    // 过渡由 CSS 负责，但收口判定还要看下一帧的目标有没有继续变
    return live;
  }

  return {
    step,
    release,
    get pinned() { return pinned; },
    get value() { return value; },
  };
}
