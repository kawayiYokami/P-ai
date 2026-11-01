/**
 * 侧会话归属：一个会话只属于一个面板，追问是追问，队员是队员。
 *
 * 判据只有一条 —— 角色名（teamName）有没有值：有值的是队员，没有的是普通追问。
 * 编号已经不下发，不能拿它当判据。
 */
export type SideConversationPanel = "sideChat" | "teamMember";

export function resolveSideConversationPanel(teamName: string | null | undefined): SideConversationPanel {
  return String(teamName || "").trim() !== "" ? "teamMember" : "sideChat";
}

/** 归属判定是对称的：某个会话属于 A 面板，就必然不属于 B 面板。 */
export function sideConversationBelongsToPanel(
  teamName: string | null | undefined,
  panel: SideConversationPanel,
): boolean {
  return resolveSideConversationPanel(teamName) === panel;
}

export type SideConversationTabLike = {
  key: string;
  panel: SideConversationPanel;
};

export type SideConversationCandidate = {
  conversationId: string;
  teamName?: string;
};

/** 某个面板自己的子会话 id 序列，顺序与列表一致 */
export function sideConversationIdsOfPanel(
  candidates: SideConversationCandidate[],
  panel: SideConversationPanel,
): string[] {
  return candidates
    .filter((item) => sideConversationBelongsToPanel(item.teamName, panel))
    .map((item) => String(item.conversationId || "").trim())
    .filter(Boolean);
}

/**
 * 父会话切换后该选中哪个子会话。
 *
 * panel 传空串表示当前父会话停在不需要分面板的模式（首页 / 阅读器 / 看板 / 监控），
 * 这时按老规则走：沿用上次打开的会话，没有就取第一个。
 *
 * 停在这两个面板时，候选先按面板过滤：上次打开的会话若属于另一个面板就不沿用，
 * 改取本面板第一个——否则追问面板会选中一名队员，那一格永远是空的。
 */
export function resolveSideConversationToSelect(
  candidates: SideConversationCandidate[],
  rememberedId: string,
  panel: SideConversationPanel | "",
): string {
  const remembered = String(rememberedId || "").trim();
  const ids = candidates.map((item) => String(item.conversationId || "").trim()).filter(Boolean);
  if (panel === "") {
    return (ids.includes(remembered) ? remembered : "") || ids[0] || "";
  }
  const scoped = sideConversationIdsOfPanel(candidates, panel);
  if (scoped.includes(remembered)) return remembered;
  if (scoped.length > 0) return scoped[0];
  // 兜底是有意为之：本面板一个会话都没有时回落到全部子会话。
  // 宁可选中一个不属于当前面板的会话（该面板显示自己的空态），
  // 也不能一个都不选、让会话停在无激活状态。
  return (ids.includes(remembered) ? remembered : "") || ids[0] || "";
}

/**
 * 「关闭左侧全部 / 右侧全部 / 关闭其他」该关哪些标签。
 *
 * 作用域按被点击那颗标签自己的面板算：两个面板各有一组标签，
 * 若拿另一组的列表去关，用户会在自己看不见的那一栏里丢东西。
 * 锚点标签不存在时返回空数组，调用方据此不做任何关闭动作。
 */
export function resolveSideConversationTabsToClose<T extends SideConversationTabLike>(
  tabs: T[],
  anchorKey: string,
  scope: "left" | "right" | "others",
): string[] {
  const key = String(anchorKey || "").trim();
  const anchor = tabs.find((tab) => tab.key === key);
  if (!anchor) return [];
  const scoped = tabs.filter((tab) => tab.panel === anchor.panel);
  const index = scoped.findIndex((tab) => tab.key === key);
  if (index < 0) return [];
  if (scope === "others") {
    return scoped.filter((tab) => tab.key !== key).map((tab) => tab.key);
  }
  if (scope === "left") {
    return scoped.slice(0, index).map((tab) => tab.key);
  }
  return scoped.slice(index + 1).map((tab) => tab.key);
}
