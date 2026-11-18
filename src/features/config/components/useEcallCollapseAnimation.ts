/**
 * 行内展开动画：JS 逐帧算高度 + opacity + translateY，180ms cubic-bezier(0.22, 1, 0.36, 1)。
 *
 * 用途只有两处，都是「一块内容就地展开/收起」，**不是折叠卡装置**：
 *   - 联系人设置区的行内展开（`RemoteImTab.vue` 里联系人行的详情块）
 *   - `PipelineScheduleTimeline` 的时间轴明细
 * 不要拿它去实现卡片折叠——卡片折叠要么用 daisyUI `collapse`，要么干脆不做。

 * 这套参数在 PipelineScheduleTimeline 里跑过 144Hz 实测（不掉帧），现在抽出来给所有折叠卡复用。
 * `prefers-reduced-motion: reduce` 时直接瞬时完成，不做逐帧动画。
 *
 * 取消路径：动画被 Vue 取消（连点折叠）时 `cleanupCollapseAnimation` 会摘掉 transitionend、
 * 清掉兜底 timer，并让在途的 `done` 失效——否则迟到的 done 会把过渡状态错当成完成。
 */

const COLLAPSE_DURATION_MS = 180;
const COLLAPSE_FALLBACK_MS = 400;
const COLLAPSE_TRANSITION = [
  `height ${COLLAPSE_DURATION_MS}ms cubic-bezier(0.22, 1, 0.36, 1)`,
  "opacity 140ms ease-out",
  `transform ${COLLAPSE_DURATION_MS}ms cubic-bezier(0.22, 1, 0.36, 1)`,
].join(", ");

type PendingAnimation = {
  onEnd: (event: TransitionEvent) => void;
  timer: number;
};

/** 每个元素当前在途的动画句柄；取消时一并清理 */
const pendingByElement = new WeakMap<Element, PendingAnimation>();

function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** 取消在途动画：摘监听、清 timer，并释放句柄（迟到的 done 不再生效） */
function dropPending(el: Element) {
  const pending = pendingByElement.get(el);
  if (!pending) return;
  el.removeEventListener("transitionend", pending.onEnd as EventListener);
  window.clearTimeout(pending.timer);
  pendingByElement.delete(el);
}

export function cleanupCollapseAnimation(element: Element) {
  dropPending(element);
  const el = element as HTMLElement;
  el.style.height = "";
  el.style.opacity = "";
  el.style.transform = "";
  el.style.overflow = "";
  el.style.willChange = "";
  el.style.transition = "";
}

function finishAnimation(el: HTMLElement, done: () => void) {
  if (el.dataset.ecallCollapseFinished === "1") return;
  el.dataset.ecallCollapseFinished = "1";
  dropPending(el);
  el.style.height = "";
  el.style.opacity = "";
  el.style.transform = "";
  el.style.overflow = "";
  el.style.willChange = "";
  el.style.transition = "";
  done();
}

export function animateCollapseEnter(element: Element, done: () => void) {
  const el = element as HTMLElement;
  cleanupCollapseAnimation(el);
  delete el.dataset.ecallCollapseFinished;
  if (prefersReducedMotion()) {
    done();
    return;
  }
  el.style.height = "0px";
  el.style.opacity = "0";
  el.style.transform = "translateY(-6px)";
  el.style.overflow = "hidden";
  el.style.willChange = "height, opacity, transform";
  void el.offsetHeight;
  const onEnd = (event: TransitionEvent) => {
    if (event.target !== el || event.propertyName !== "height") return;
    finishAnimation(el, done);
  };
  el.addEventListener("transitionend", onEnd);
  const timer = window.setTimeout(() => finishAnimation(el, done), COLLAPSE_FALLBACK_MS);
  pendingByElement.set(el, { onEnd, timer });
  el.style.transition = COLLAPSE_TRANSITION;
  requestAnimationFrame(() => {
    el.style.height = `${el.scrollHeight}px`;
    el.style.opacity = "1";
    el.style.transform = "translateY(0)";
  });
}

export function animateCollapseLeave(element: Element, done: () => void) {
  const el = element as HTMLElement;
  cleanupCollapseAnimation(el);
  delete el.dataset.ecallCollapseFinished;
  if (prefersReducedMotion()) {
    done();
    return;
  }
  el.style.height = `${el.scrollHeight}px`;
  el.style.opacity = "1";
  el.style.transform = "translateY(0)";
  el.style.overflow = "hidden";
  el.style.willChange = "height, opacity, transform";
  void el.offsetHeight;
  const onEnd = (event: TransitionEvent) => {
    if (event.target !== el || event.propertyName !== "height") return;
    finishAnimation(el, done);
  };
  el.addEventListener("transitionend", onEnd);
  const timer = window.setTimeout(() => finishAnimation(el, done), COLLAPSE_FALLBACK_MS);
  pendingByElement.set(el, { onEnd, timer });
  el.style.transition = COLLAPSE_TRANSITION;
  requestAnimationFrame(() => {
    el.style.height = "0px";
    el.style.opacity = "0";
    el.style.transform = "translateY(-6px)";
  });
}

/** 卡片折叠/展开的动画三件套，直接绑到 `<Transition :css="false">` 上 */
export function useEcallCollapseAnimation() {
  return {
    animateCollapseEnter,
    animateCollapseLeave,
    cleanupCollapseAnimation,
  };
}

/** 仅供测试：当前元素是否还有在途动画 */
export function hasPendingCollapseAnimation(element: Element): boolean {
  return pendingByElement.has(element);
}
