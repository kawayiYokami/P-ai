/**
 * 折叠面板 JS 高度过渡：供 <Transition :css="false"> 的 enter/leave 钩子使用。
 * 拉出：height 0→内容高 + opacity + translateY(-6px)；收起反向。
 */
export function useCollapseTransition() {
  function cleanupAnimation(element: Element) {
    const el = element as HTMLElement;
    el.style.height = "";
    el.style.opacity = "";
    el.style.transform = "";
    el.style.overflow = "";
    el.style.willChange = "";
    el.style.transition = "";
    el.style.paddingTop = "";
    el.style.paddingBottom = "";
  }

  function finishAnimation(el: HTMLElement, onEnd: (e: TransitionEvent) => void, done: () => void) {
    if (el.dataset.ecallCollapseFinished === "1") return;
    el.dataset.ecallCollapseFinished = "1";
    el.removeEventListener("transitionend", onEnd);
    cleanupAnimation(el);
    done();
  }

  function bindTransitionEnd(el: HTMLElement, done: () => void) {
    const onEnd = (e: TransitionEvent) => {
      if (e.target !== el || e.propertyName !== "height") return;
      finishAnimation(el, onEnd, done);
    };
    el.addEventListener("transitionend", onEnd);
    return onEnd;
  }

  function animateEnter(element: Element, done: () => void) {
    const el = element as HTMLElement;
    cleanupAnimation(el);
    delete el.dataset.ecallCollapseFinished;
    el.style.height = "0px";
    el.style.opacity = "0";
    el.style.transform = "translateY(-6px)";
    el.style.overflow = "hidden";
    el.style.willChange = "height, opacity, transform";
    const onEnd = bindTransitionEnd(el, done);
    // 只展开到露出为止：内容比一屏还高时，动画走过整段 height 会在同样 180ms 里
    // 冲过上千像素，看上去就是直接跳出来。以「面板顶部到视口底部」为终点，
    // 超出屏幕的部分不参与动画，速度与短内容一致。
    const visibleHeight = Math.max(0, window.innerHeight - el.getBoundingClientRect().top);
    const targetHeight = Math.min(el.scrollHeight, visibleHeight);
    requestAnimationFrame(() => {
      el.style.transition = [
        "height 180ms cubic-bezier(0.22, 1, 0.36, 1)",
        "opacity 140ms ease-out",
        "transform 180ms cubic-bezier(0.22, 1, 0.36, 1)",
      ].join(", ");
      el.style.height = `${targetHeight}px`;
      el.style.opacity = "1";
      el.style.transform = "translateY(0)";
    });
    window.setTimeout(() => finishAnimation(el, onEnd, done), 400);
  }

  function animateLeave(element: Element, done: () => void) {
    const el = element as HTMLElement;
    cleanupAnimation(el);
    delete el.dataset.ecallCollapseFinished;
    el.style.height = `${el.scrollHeight}px`;
    el.style.opacity = "1";
    el.style.transform = "translateY(0)";
    el.style.overflow = "hidden";
    el.style.willChange = "height, opacity, transform";
    const onEnd = bindTransitionEnd(el, done);
    requestAnimationFrame(() => {
      el.style.transition = [
        "height 180ms cubic-bezier(0.22, 1, 0.36, 1)",
        "opacity 140ms ease-out",
        "transform 180ms cubic-bezier(0.22, 1, 0.36, 1)",
      ].join(", ");
      el.style.height = "0px";
      el.style.opacity = "0";
      el.style.transform = "translateY(-6px)";
      // 内层带 pb-1：border-box 下 height 归 0 也压不住 padding，元素卸载前会一直
      // 剩一条 3.5px 的尾巴。收起时把上下内边距一起归零，落定就真的是 0。
      el.style.paddingTop = "0px";
      el.style.paddingBottom = "0px";
    });
    window.setTimeout(() => finishAnimation(el, onEnd, done), 400);
  }

  return { animateEnter, animateLeave, cleanupAnimation };
}
