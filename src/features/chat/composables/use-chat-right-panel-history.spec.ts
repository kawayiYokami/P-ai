import { describe, expect, it } from "vitest";
import { nextTick, ref } from "vue";
import { useChatRightPanelHistory, type ChatRightPanelView } from "./use-chat-right-panel-history";

/** 把 composable 接到一个可手动改的「当前视图」上，模拟真实面板的切换。 */
function mountHistory(options?: { unopenable?: string[] }) {
  const current = ref<ChatRightPanelView>({ mode: "home", conversationId: "" });
  const unopenable = new Set(options?.unopenable || []);
  const history = useChatRightPanelHistory({
    currentView: () => current.value,
    applyView: (view) => {
      if (view.conversationId && unopenable.has(view.conversationId)) return false;
      current.value = { ...view };
      return true;
    },
  });
  async function goTo(view: ChatRightPanelView) {
    current.value = { ...view };
    await nextTick();
  }
  async function back() {
    history.goBack();
    await nextTick();
  }
  async function forward() {
    history.goForward();
    await nextTick();
  }
  return { current, history, goTo, back, forward };
}

const TEAM_A: ChatRightPanelView = { mode: "teamMember", conversationId: "conv-a" };
const TEAM_B: ChatRightPanelView = { mode: "teamMember", conversationId: "conv-b" };
const READER: ChatRightPanelView = { mode: "reader", conversationId: "" };
const HOME: ChatRightPanelView = { mode: "home", conversationId: "" };

describe("右面板浏览历史", () => {
  it("刚打开时两边都退不了", async () => {
    const { history } = mountHistory();
    await nextTick();
    expect(history.canGoBack.value).toBe(false);
    expect(history.canGoForward.value).toBe(false);
  });

  it("从队伍对话点进文件页，后退回队伍对话，前进再回文件页", async () => {
    const { current, history, goTo, back, forward } = mountHistory();
    await nextTick();
    await goTo(TEAM_A);
    await goTo(READER);
    expect(history.canGoBack.value).toBe(true);

    await back();
    expect(current.value).toEqual(TEAM_A);
    expect(history.canGoForward.value).toBe(true);

    await forward();
    expect(current.value).toEqual(READER);
    expect(history.canGoForward.value).toBe(false);
  });

  it("退到一半又切去新页面时，前进分支作废", async () => {
    const { current, history, goTo, back } = mountHistory();
    await nextTick();
    await goTo(TEAM_A);
    await goTo(READER);
    await back();
    expect(current.value).toEqual(TEAM_A);

    await goTo(TEAM_B);
    expect(history.canGoForward.value).toBe(false);
    // 后退仍然回到上一页，而不是回到被作废的 reader
    await back();
    expect(current.value).toEqual(TEAM_A);
  });

  it("历史里那一页已经打不开时跳过它继续退", async () => {
    const { current, goTo, back } = mountHistory({ unopenable: ["conv-b"] });
    await nextTick();
    await goTo(TEAM_A);
    await goTo(TEAM_B);
    await goTo(READER);

    // conv-b 已打不开：退一次直接落到 conv-a，而不是卡在原地
    await back();
    expect(current.value).toEqual(TEAM_A);
  });

  it("同一页连续切换不会在历史里堆重复项", async () => {
    const { history, goTo, back, current } = mountHistory();
    await nextTick();
    await goTo(HOME);
    await goTo(TEAM_A);
    await goTo(TEAM_A);
    await goTo(TEAM_A);
    await back();
    expect(current.value).toEqual(HOME);
    expect(history.canGoBack.value).toBe(false);
  });

  it("只有模式的页面不带会话 id，切会话不算换页", async () => {
    const { history, goTo } = mountHistory();
    await nextTick();
    await goTo(READER);
    await goTo({ mode: "reader", conversationId: "" });
    // 两次是同一页：仍然只有一条历史，退不回去
    expect(history.canGoBack.value).toBe(true);
  });
});
