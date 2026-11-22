import type { ChatConversationOverviewItem } from "../../../types/app";
import { canonicalWorkspaceRootForComparison, workspaceNameFromPath } from "./conversation-sections";

// ==================== 最近会话按来源聚块 ====================

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

export type RecentSourceBlockDivider = {
  /** 该块的第一个会话 id，渲染时据此插入分割线 */
  conversationId: string;
  /** 分割线文案：本地会话取工作区名，远程联系人取频道名 */
  label: string;
  /** 本地会话的工作区路径，用于点击后跳转到对应项目分组；远程联系人为空 */
  workspaceRootPath: string;
};

/**
 * 最近会话按来源（工作区 / 频道）聚块：块内按最近使用倒序，块之间按各自最新会话的最近使用倒序。
 * 仅用于「最近会话」分组在非人格分组依据下的展示，时间顺序让位于来源聚块。
 */
export function groupRecentItemsBySource(
  items: ChatConversationOverviewItem[],
  options: { defaultLabel: string },
): { orderedItems: ChatConversationOverviewItem[]; dividers: RecentSourceBlockDivider[] } {
  if (items.length === 0) return { orderedItems: [], dividers: [] };
  const blocks = new Map<string, { label: string; workspaceRootPath: string; items: ChatConversationOverviewItem[] }>();
  for (const item of items) {
    const isRemote = item.kind === "remote_im_contact";
    const workspacePath = String(item.workspaceRootPath || "").trim();
    const label = (isRemote
      ? String(item.channelName || item.remoteContactDisplayName || options.defaultLabel).trim()
      : String(item.workspaceLabel || workspaceNameFromPath(workspacePath) || options.defaultLabel).trim()
    ) || options.defaultLabel;
    const key = isRemote
      ? `channel:${label}`
      : `workspace:${canonicalWorkspaceRootForComparison(workspacePath) || "__default__"}`;
    const existing = blocks.get(key);
    if (existing) existing.items.push(item);
    else blocks.set(key, { label, workspaceRootPath: isRemote ? "" : workspacePath, items: [item] });
  }

  const orderedBlocks = Array.from(blocks.values())
    .map((block) => ({
      ...block,
      sortedItems: [...block.items].sort((left, right) => conversationLastUsedMs(right) - conversationLastUsedMs(left)),
      recency: block.items.reduce((max, item) => Math.max(max, conversationLastUsedMs(item)), 0),
    }))
    .sort((left, right) => right.recency - left.recency);

  const orderedItems: ChatConversationOverviewItem[] = [];
  const dividers: RecentSourceBlockDivider[] = [];
  for (const block of orderedBlocks) {
    const first = block.sortedItems[0];
    const conversationId = String(first?.conversationId || "").trim();
    if (conversationId) {
      dividers.push({ conversationId, label: block.label, workspaceRootPath: block.workspaceRootPath });
    }
    orderedItems.push(...block.sortedItems);
  }
  return { orderedItems, dividers };
}

/**
 * 分组内聚合「同一目录 + 同一 agentId」的会话：每组最新会话保留为完整条目（排在组内最新成员的原位置），
 * 其余同目录同 agentId 会话转为简单条目（按更新时间倒序）紧跟其后。agentId 为空时不聚合。
 * 目录以 workspaceRootPath 判定（空路径视为默认目录）。
 * 搜索模式下不聚合，保持原顺序原形态。
 * aggregate 传 false 时整段不聚合：联系人页每一行是一个联系人或一个会话，它不是「同一实体的一批会话」，
 * 折叠后这些条目没有渲染位置，等于把人藏起来。
 */
export function aggregateConversationItems(
  items: ChatConversationOverviewItem[],
  options: { searchActive: boolean; aggregate?: boolean },
): AggregatedConversationItems {
  const noAggregation: AggregatedConversationItems = {
    reorderedItems: items,
    simpleFollowers: {},
  };
  if (options.searchActive || options.aggregate === false) return noAggregation;

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
