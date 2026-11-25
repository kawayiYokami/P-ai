// @vitest-environment happy-dom
import { beforeEach, describe, expect, it } from "vitest";
import { nextTick, ref } from "vue";
import {
  rememberedSideConversationInHistory,
  useChatRightPanelHistory,
  type ChatRightPanelTab,
} from "./use-chat-right-panel-history";

/** 把 composable 接到一个可手动改的「当前标签」上，模拟真实面板的切换。 */
function mountHistory(options?: { unopenable?: string[]; scope?: string }) {
  const current = ref<ChatRightPanelTab>({ kind: "home", id: "" });
  const scope = ref(options?.scope ?? "parent-1");
  const unopenable = new Set(options?.unopenable || []);
  const history = useChatRightPanelHistory({
    scope: () => scope.value,
    currentTab: () => current.value,
    openTab: (tab) => {
      if (tab.id && unopenable.has(tab.id)) return false;
      current.value = { ...tab };
      return true;
    },
  });
  async function goTo(tab: ChatRightPanelTab) {
    current.value = { ...tab };
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
  return { current, scope, history, goTo, back, forward };
}

const HOME: ChatRightPanelTab = { kind: "home", id: "" };
const FILE_A: ChatRightPanelTab = { kind: "file", id: "E:/repo/src/a.ts" };
const FILE_B: ChatRightPanelTab = { kind: "file", id: "E:/repo/src/b.ts" };
const SIDE_A: ChatRightPanelTab = { kind: "sideChat", id: "conv-a" };
const SIDE_B: ChatRightPanelTab = { kind: "sideChat", id: "conv-b" };
const TEAM_A: ChatRightPanelTab = { kind: "teamMember", id: "team-a" };

beforeEach(() => {
  window.localStorage.clear();
});

describe("右面板浏览历史", () => {
  it("刚打开时两边都退不了", async () => {
    const { history } = mountHistory();
    await nextTick();
    expect(history.canGoBack.value).toBe(false);
    expect(history.canGoForward.value).toBe(false);
  });

  it("从队伍对话点进文件，后退回队伍对话，前进再回那个文件", async () => {
    const { current, history, goTo, back, forward } = mountHistory();
    await nextTick();
    await goTo(TEAM_A);
    await goTo(FILE_A);
    expect(history.canGoBack.value).toBe(true);

    await back();
    expect(current.value).toEqual(TEAM_A);
    expect(history.canGoForward.value).toBe(true);

    await forward();
    expect(current.value).toEqual(FILE_A);
    expect(history.canGoForward.value).toBe(false);
  });

  it("两个文件是两个标签，后退能回到上一个文件而不只是「文件页」", async () => {
    const { current, goTo, back } = mountHistory();
    await nextTick();
    await goTo(FILE_A);
    await goTo(FILE_B);

    await back();
    expect(current.value).toEqual(FILE_A);
  });

  it("同一路径的文件算同一个标签，重复切不堆历史", async () => {
    const { current, history, goTo, back } = mountHistory();
    await nextTick();
    await goTo(FILE_A);
    await goTo(FILE_A);
    await goTo(FILE_A);
    // 起点是首页，三次切同一个文件只该多出一条：退一次就回到首页，再退不了
    await back();
    expect(current.value).toEqual(HOME);
    expect(history.canGoBack.value).toBe(false);
  });

  it("退到一半又切去新标签时，前进分支作废", async () => {
    const { current, history, goTo, back } = mountHistory();
    await nextTick();
    await goTo(SIDE_A);
    await goTo(FILE_A);
    await back();
    expect(current.value).toEqual(SIDE_A);

    await goTo(SIDE_B);
    expect(history.canGoForward.value).toBe(false);
    // 后退仍然回到上一页，而不是回到被作废的文件
    await back();
    expect(current.value).toEqual(SIDE_A);
  });

  it("历史里那个标签已经打不开时跳过它继续退", async () => {
    const { current, goTo, back } = mountHistory({ unopenable: ["conv-b"] });
    await nextTick();
    await goTo(SIDE_A);
    await goTo(SIDE_B);
    await goTo(FILE_A);

    // conv-b 已打不开：退一次直接落到 conv-a，而不是卡在原地
    await back();
    expect(current.value).toEqual(SIDE_A);
  });

  it("还没定位到具体对象的标签不进历史", async () => {
    const { history, goTo } = mountHistory();
    await nextTick();
    // 新建追问页（会话 id 为空）、文件面板还没恢复完（路径为空）都不算一次浏览
    await goTo({ kind: "sideChat", id: "" });
    await goTo({ kind: "file", id: "" });
    expect(history.canGoBack.value).toBe(false);
  });

  it("按父会话分片：切走再切回来各自接着用，不互相带标签", async () => {
    const { current, scope, history, goTo, back } = mountHistory();
    await nextTick();
    await goTo(SIDE_A);
    await goTo(FILE_A);

    // 切到另一个父会话：换上它自己的历史，这里还是空的
    scope.value = "parent-2";
    await nextTick();
    expect(history.canGoBack.value).toBe(false);
    await goTo(FILE_B);

    // 切回来：原来的两个标签还在，后退能回到 conv-a
    scope.value = "parent-1";
    await nextTick();
    expect(history.canGoBack.value).toBe(true);
    await back();
    expect(current.value).toEqual(SIDE_A);
  });

  it("栈上限 30 页，超出时丢掉最早的那一页", async () => {
    const { history, goTo, back } = mountHistory();
    await nextTick();
    for (let index = 0; index < 40; index += 1) {
      await goTo({ kind: "file", id: `E:/repo/src/f${index}.ts` });
    }
    // 一路退到底：30 页最多退 29 次
    let steps = 0;
    while (history.canGoBack.value && steps < 100) {
      await back();
      steps += 1;
    }
    expect(steps).toBe(29);
  });

  it("能从历史里找回最近看过的那个会话标签", async () => {
    const { goTo } = mountHistory({ scope: "parent-recall" });
    await nextTick();
    await goTo(SIDE_A);
    await goTo(TEAM_A);
    await goTo(FILE_A);

    // 最后停在文件页，但往回找会话标签应该找到 team-a
    expect(rememberedSideConversationInHistory("parent-recall")).toBe("team-a");
    expect(rememberedSideConversationInHistory("parent-none")).toBe("");
  });

  it("首页和工具审查没有标识，各算一个标签", async () => {
    const { current, goTo, back } = mountHistory();
    await nextTick();
    await goTo({ kind: "monitor", id: "" });
    await goTo(HOME);

    await back();
    expect(current.value).toEqual({ kind: "monitor", id: "" });
  });
});
