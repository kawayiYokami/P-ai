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
});
