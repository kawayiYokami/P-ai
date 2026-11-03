import type { ChatConversationOverviewItem, ConversationPreviewMessage } from "../../../types/app";

// ==================== 会话项展示判定（纯函数，供 ChatConversationItem 与 Sidebar 共用） ====================

/**
 * 会话项档位：
 *   full   完整项——头像 + 标题 + 摘要两行，本地列表默认形态
 *   sim    简单两行——无头像，标题 + 摘要
 *   mini   简单一行——无头像，只有标题
 *   remote 远程视图——负责人头像 + 标题，单行，不带摘要
 */
export type ConversationItemLevel = "full" | "sim" | "mini" | "remote";

/**
 * 天起点：凌晨 4 点。当前时刻未到 4 点时，天起点回退到前一天 4 点。
 * 与「最近会话」计数使用同一套「凌晨 4 点区分天」的口径。
 */
export function activityDayStartMs(now: number = Date.now()): number {
  const dayStart = new Date(now);
  dayStart.setHours(4, 0, 0, 0);
  if (dayStart.getTime() > now) {
    dayStart.setDate(dayStart.getDate() - 1);
  }
  return dayStart.getTime();
}

/** 未读，或今天（凌晨 4 点起）有过活动 → sim（摘要常显）；否则 → mini（摘要折叠、hover 展开） */
export function simpleConversationItemLevel(
  item: ChatConversationOverviewItem,
  now: number = Date.now(),
): "sim" | "mini" {
  return hasUnreadOrRecentActivity(item, now) ? "sim" : "mini";
}

export function hasUnreadOrRecentActivity(
  item: ChatConversationOverviewItem,
  now: number = Date.now(),
): boolean {
  if (Number(item.unreadCount || 0) > 0) return true;
  const raw = String(item.lastMessageAt || item.updatedAt || "").trim();
  if (!raw) return false;
  const time = Date.parse(raw);
  if (!Number.isFinite(time)) return false;
  return time >= activityDayStartMs(now);
}

/** 未读角标：当前会话不显示；0 不显示；超过 99 显示 99+ */
export function conversationUnreadBadge(
  item: ChatConversationOverviewItem,
  activeConversationId: string,
): string {
  if (String(item.conversationId || "").trim() === String(activeConversationId || "").trim()) {
    return "";
  }
  const unreadCount = Math.max(0, Number(item.unreadCount || 0));
  if (unreadCount <= 0) return "";
  return unreadCount > 99 ? "99+" : String(unreadCount);
}

/**
 * 待答问题数角标：与未读是两个字段，不合并、也不互相压制。
 * 它讲的是「这个会话里还有几个问题等你回答」，属于提醒而非警报，
 * 所以取中性色，绝不借用 error 或任何状态色。
 *
 * 与未读的两处有意差异：
 *   - 0 或字段缺失一律不渲染，不给每行加一个恒为 0 的空壳；
 *   - 当前会话照样显示：你正看着它，不代表问题已经回答过。
 */
export function conversationPendingQuestionBadge(item: ChatConversationOverviewItem): string {
  const raw = Number(item.questionCount ?? 0);
  if (!Number.isFinite(raw) || raw <= 0) return "";
  const count = Math.floor(raw);
  return count > 99 ? "99+" : String(count);
}

/** 运行时是否忙碌（流式/整理上下文/归档/压缩） */
export function conversationRuntimeBusy(runtimeState?: ChatConversationOverviewItem["runtimeState"]): boolean {
  return runtimeState === "assistant_streaming"
    || runtimeState === "organizing_context"
    || runtimeState === "archiving"
    || runtimeState === "compacting";
}
