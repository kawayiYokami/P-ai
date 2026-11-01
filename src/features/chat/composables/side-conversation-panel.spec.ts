import { describe, expect, it } from "vitest";
import {
  resolveSideConversationPanel,
  resolveSideConversationTabsToClose,
  resolveSideConversationToSelect,
  sideConversationBelongsToPanel,
  sideConversationIdsOfPanel,
} from "./side-conversation-panel";

describe("侧会话的面板归属", () => {
  it("普通追问归追问面板", () => {
    expect(resolveSideConversationPanel("")).toBe("sideChat");
    expect(resolveSideConversationPanel("   ")).toBe("sideChat");
    expect(resolveSideConversationPanel(undefined)).toBe("sideChat");
    expect(resolveSideConversationPanel(null)).toBe("sideChat");
  });

  it("有角色名的会话归队员面板", () => {
    expect(resolveSideConversationPanel("李工")).toBe("teamMember");
    // 前后空格不影响判定：名册写入口径是 trim 后比较
    expect(resolveSideConversationPanel("  李工  ")).toBe("teamMember");
    // 角色名里的空格是名字的一部分，不能当成空角色
    expect(resolveSideConversationPanel("李 工")).toBe("teamMember");
  });

  it("归属判定是双向的：属于 A 面板就不属于 B 面板", () => {
    const cases = ["", "   ", "李工", "李 工", undefined, null];
    for (const teamName of cases) {
      const panel = resolveSideConversationPanel(teamName);
      const other = panel === "sideChat" ? "teamMember" : "sideChat";
      // 追问面板里不出现队员会话，队员面板里也不出现追问会话——同一个判据的两面
      expect(sideConversationBelongsToPanel(teamName, panel)).toBe(true);
      expect(sideConversationBelongsToPanel(teamName, other)).toBe(false);
    }
  });
});

describe("关闭类标签操作的面板作用域", () => {
  // 两个面板混在一起的真实形状：追问与队员交错出现，顺序就是会话列表顺序
  const tabs = [
    { key: "s1", panel: "sideChat" as const },
    { key: "t1", panel: "teamMember" as const },
    { key: "s2", panel: "sideChat" as const },
    { key: "t2", panel: "teamMember" as const },
    { key: "s3", panel: "sideChat" as const },
  ];

  it("关闭其他只关同一面板的标签", () => {
    expect(resolveSideConversationTabsToClose(tabs, "s2", "others")).toEqual(["s1", "s3"]);
    expect(resolveSideConversationTabsToClose(tabs, "t1", "others")).toEqual(["t2"]);
  });

  it("关闭左侧只在同一面板内取左侧", () => {
    // s2 在追问面板里排第二，左侧只有 s1；不能把夹在中间的队员 t1 一起关掉
    expect(resolveSideConversationTabsToClose(tabs, "s2", "left")).toEqual(["s1"]);
    expect(resolveSideConversationTabsToClose(tabs, "t2", "left")).toEqual(["t1"]);
    // 面板内最左一颗没有左侧可关
    expect(resolveSideConversationTabsToClose(tabs, "s1", "left")).toEqual([]);
  });

  it("关闭右侧只在同一面板内取右侧", () => {
    expect(resolveSideConversationTabsToClose(tabs, "s2", "right")).toEqual(["s3"]);
    expect(resolveSideConversationTabsToClose(tabs, "t1", "right")).toEqual(["t2"]);
    expect(resolveSideConversationTabsToClose(tabs, "s3", "right")).toEqual([]);
  });

  it("锚点标签不存在时不做任何关闭", () => {
    for (const scope of ["left", "right", "others"] as const) {
      expect(resolveSideConversationTabsToClose(tabs, "not-exist", scope)).toEqual([]);
      expect(resolveSideConversationTabsToClose(tabs, "", scope)).toEqual([]);
    }
  });
});

describe("父会话切换后选中哪个子会话", () => {
  const children = [
    { conversationId: "t1", teamName: "李工" },
    { conversationId: "s1", teamName: "" },
    { conversationId: "t2", teamName: "王工" },
    { conversationId: "s2", teamName: "" },
  ];

  it("停在不需要分面板的模式时按老规则走", () => {
    // 面板模式是 home / reader / monitor / kanban：沿用上次的，没有就第一个
    expect(resolveSideConversationToSelect(children, "t2", "")).toBe("t2");
    expect(resolveSideConversationToSelect(children, "s2", "")).toBe("s2");
    expect(resolveSideConversationToSelect(children, "not-exist", "")).toBe("t1");
    expect(resolveSideConversationToSelect(children, "", "")).toBe("t1");
  });

  it("停在追问面板时不选中队员会话", () => {
    // 上次打开的是队员：不沿用，改取本面板第一个追问
    expect(resolveSideConversationToSelect(children, "t1", "sideChat")).toBe("s1");
    // 上次打开的就是追问：沿用
    expect(resolveSideConversationToSelect(children, "s2", "sideChat")).toBe("s2");
  });

  it("停在队员面板时不选中追问会话", () => {
    expect(resolveSideConversationToSelect(children, "s1", "teamMember")).toBe("t1");
    expect(resolveSideConversationToSelect(children, "t2", "teamMember")).toBe("t2");
  });

  it("本面板会话为空时兜底回落到全部，不留下无激活状态", () => {
    const onlySideChat = [
      { conversationId: "s1", teamName: "" },
      { conversationId: "s2", teamName: "" },
    ];
    // 队员面板里一个队员都没有：宁可选中一个追问会话，也不能一个都不选
    expect(resolveSideConversationToSelect(onlySideChat, "", "teamMember")).toBe("s1");
    expect(resolveSideConversationToSelect(onlySideChat, "s2", "teamMember")).toBe("s2");
    expect(resolveSideConversationToSelect([], "", "teamMember")).toBe("");
    expect(resolveSideConversationToSelect([], "s1", "sideChat")).toBe("");
  });

  it("按面板取 id 序列时保持原顺序", () => {
    expect(sideConversationIdsOfPanel(children, "sideChat")).toEqual(["s1", "s2"]);
    expect(sideConversationIdsOfPanel(children, "teamMember")).toEqual(["t1", "t2"]);
  });
});
