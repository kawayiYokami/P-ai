// @vitest-environment node
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ref } from "vue";
import { useChatScrollLayout } from "./use-chat-scroll-layout";

function makeLayout() {
  return useChatScrollLayout({
    activeConversationId: ref("conversation-a"),
    chatting: ref(false),
    busy: ref(false),
    frozen: ref(false),
    timelineItemCount: ref(0),
    onReachedBottom: () => {},
    focusComposerInput: () => {},
  });
}

function makeScroller(initial: { scrollTop: number; scrollHeight: number; clientHeight: number }) {
  return { ...initial } as unknown as HTMLElement & { scrollTop: number };
}

beforeEach(() => {
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: {
      setTimeout: (handler: () => void, timeoutMs: number) => setTimeout(handler, timeoutMs),
      clearTimeout: (timer: unknown) => clearTimeout(timer as ReturnType<typeof setTimeout>),
      getComputedStyle: () => ({ paddingTop: "0", paddingBottom: "0" }),
    },
  });
});

afterEach(() => {
  Object.defineProperty(globalThis, "window", {
    configurable: true,
    value: undefined,
  });
});

describe("followBottom intent", () => {
  it("用户主动滚到底部：进入跟随", () => {
    const layout = makeLayout();
    layout.scrollContainer.value = makeScroller({ scrollTop: 900, scrollHeight: 1000, clientHeight: 100 });
    layout.noteWheelScrollIntent();
    layout.onScroll();
    expect(layout.followBottom.value).toBe(true);
  });

  it("跟随中向上滚离底部：退出跟随", () => {
    const layout = makeLayout();
    const scroller = makeScroller({ scrollTop: 900, scrollHeight: 1000, clientHeight: 100 });
    layout.scrollContainer.value = scroller;
    layout.noteWheelScrollIntent();
    layout.onScroll();
    expect(layout.followBottom.value).toBe(true);

    (scroller as { scrollTop: number }).scrollTop = 400;
    layout.noteWheelScrollIntent();
    layout.onScroll();
    expect(layout.followBottom.value).toBe(false);
  });

  it("程序化滚动（无用户意图）落在底部也不进入跟随", () => {
    const layout = makeLayout();
    layout.scrollContainer.value = makeScroller({ scrollTop: 900, scrollHeight: 1000, clientHeight: 100 });
    layout.onScroll();
    expect(layout.followBottom.value).toBe(false);
  });

  it("点击回到底部：显式进入跟随", () => {
    const layout = makeLayout();
    layout.startFollowBottom();
    expect(layout.followBottom.value).toBe(true);
  });

  it("未贴底时朝底部方向滚动：有滚到最下的意图即解锁，不必真正滚到底", () => {
    const layout = makeLayout();
    const scroller = makeScroller({ scrollTop: 1200, scrollHeight: 4000, clientHeight: 100 });
    layout.scrollContainer.value = scroller;
    layout.stopFollowBottom();
    expect(layout.followBottom.value).toBe(false);

    (scroller as { scrollTop: number }).scrollTop = 1260;
    layout.noteWheelScrollIntent();
    layout.onScroll();
    // 距底仍有 2740px
    expect(layout.followBottom.value).toBe(true);
  });

  it("跟随中朝历史方向滚动：锁定跟随", () => {
    const layout = makeLayout();
    const scroller = makeScroller({ scrollTop: 900, scrollHeight: 4000, clientHeight: 100 });
    layout.scrollContainer.value = scroller;
    layout.startFollowBottom();
    layout.noteWheelScrollIntent();
    layout.onScroll();
    expect(layout.followBottom.value).toBe(true);

    (scroller as { scrollTop: number }).scrollTop = 700;
    layout.noteWheelScrollIntent();
    layout.onScroll();
    expect(layout.followBottom.value).toBe(false);
  });

  it("显式跳转（时间线、跳转到用户消息、发送后自动上推）：锁定跟随", () => {
    const layout = makeLayout();
    layout.startFollowBottom();
    layout.stopFollowBottom();
    expect(layout.followBottom.value).toBe(false);
  });

  it("补载历史期间的程序化抬高：不进入跟随", () => {
    const layout = makeLayout();
    const scroller = makeScroller({ scrollTop: 0, scrollHeight: 1599, clientHeight: 874 });
    layout.scrollContainer.value = scroller;
    layout.noteWheelScrollIntent();

    (scroller as { scrollTop: number }).scrollTop = 1830;
    layout.onScroll({ suppressFollowIntent: true });
    expect(layout.followBottom.value).toBe(false);
  });

  it("抑制结束后朝底部方向滚动：恢复解锁", () => {
    const layout = makeLayout();
    const scroller = makeScroller({ scrollTop: 0, scrollHeight: 4000, clientHeight: 874 });
    layout.scrollContainer.value = scroller;
    layout.noteWheelScrollIntent();

    (scroller as { scrollTop: number }).scrollTop = 1830;
    layout.onScroll({ suppressFollowIntent: true });
    expect(layout.followBottom.value).toBe(false);

    layout.noteWheelScrollIntent();
    (scroller as { scrollTop: number }).scrollTop = 1900;
    layout.onScroll();
    expect(layout.followBottom.value).toBe(true);
  });
});
