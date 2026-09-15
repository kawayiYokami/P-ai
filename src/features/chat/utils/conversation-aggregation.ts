import type { ChatConversationOverviewItem } from "../../../types/app";

// ==================== 聚合会话列表：同人格会话相邻 ====================

export type AggregatedConversationItems = {
  /** 重新排序后的展示序列：每个聚合块的最新会话在前，未聚合会话按原位置保留 */
  reorderedItems: ChatConversationOverviewItem[];
  /** full 会话 id → 紧跟其后的简单条目（同 agentId 的旧会话，按更新时间倒序） */
  simpleFollowers: Record<string, ChatConversationOverviewItem[]>;
};

export function conversationLastUsedMs(item: ChatConversationOverviewItem): number {
  const raw = String(item.lastMessageAt || item.updatedAt || "").trim();
  if (!raw) return 0;
  const timestamp = Date.parse(raw);
  return Number.isFinite(timestamp) ? timestamp : 0;
}

/**
 * 分组内聚合「同一目录 + 同一 agentId」的会话：每组最新会话保留为完整条目（排在组内最新成员的原位置），
 * 其余同目录同 agentId 会话转为简单条目（按更新时间倒序）紧跟其后。agentId 为空时不聚合。
 * 目录以 workspaceRootPath 判定（空路径视为默认目录）。
 * 搜索模式下不聚合，保持原顺序原形态。
 */
export function aggregateConversationItems(
  items: ChatConversationOverviewItem[],
  options: { searchActive: boolean },
): AggregatedConversationItems {
  const noAggregation: AggregatedConversationItems = {
    reorderedItems: items,
    simpleFollowers: {},
  };
  if (options.searchActive) return noAggregation;

  const indexOf = new Map<string, number>();
  const groups = new Map<string, ChatConversationOverviewItem[]>();
  items.forEach((item, index) => {
    indexOf.set(String(item.conversationId || "").trim(), index);
    const agentId = String(item.agentId || "").trim();
    const workspacePath = String(item.workspaceRootPath || "").trim();
    const key = agentId ? `agent:${workspacePath}|${agentId}` : `single:${index}`;
    const group = groups.get(key);
    if (group) group.push(item);
    else groups.set(key, [item]);
  });

  const ordered: Array<{ sortIndex: number; items: ChatConversationOverviewItem[] }> = [];
  for (const groupItems of groups.values()) {
    const sorted = [...groupItems].sort((left, right) => conversationLastUsedMs(right) - conversationLastUsedMs(left));
    const full = sorted[0];
    ordered.push({
      sortIndex: indexOf.get(String(full.conversationId || "").trim()) ?? 0,
      items: sorted,
    });
  }
  ordered.sort((left, right) => left.sortIndex - right.sortIndex);

  const reorderedItems: ChatConversationOverviewItem[] = [];
  const simpleFollowers: Record<string, ChatConversationOverviewItem[]> = {};
  for (const entry of ordered) {
    reorderedItems.push(entry.items[0]);
    if (entry.items.length > 1) {
      simpleFollowers[String(entry.items[0].conversationId || "").trim()] = entry.items.slice(1);
    }
  }
  return { reorderedItems, simpleFollowers };
}
