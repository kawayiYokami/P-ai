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
    requestAnimationFrame(() => {
      el.style.transition = [
        "height 180ms cubic-bezier(0.22, 1, 0.36, 1)",
        "opacity 140ms ease-out",
        "transform 180ms cubic-bezier(0.22, 1, 0.36, 1)",
      ].join(", ");
      el.style.height = `${el.scrollHeight}px`;
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
    });
    window.setTimeout(() => finishAnimation(el, onEnd, done), 400);
  }

  return { animateEnter, animateLeave, cleanupAnimation };
}
