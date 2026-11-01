import type { ChatConversationOverviewItem } from "../../../types/app";

/**
 * 会话标题的统一取法（前端唯一入口）。
 *
 * 凡是「要把会话标题显示出来」的地方都走这里，不要在调用侧自己写
 * `item.title || 某个兜底`——那样会漏掉「标题还是占位 id」「只有摘要标题」
 * 「还没起标题但有过消息」这些情况，同一个会话在不同页面显示成不同名字。
 */
export type ConversationTitleLike = Partial<Pick<
  ChatConversationOverviewItem,
  | "conversationId"
  | "kind"
  | "title"
  | "summaryTitle"
  | "remoteContactDisplayName"
  | "updatedAt"
  | "lastMessageAt"
  | "isSystemNotificationConversation"
>> & {
  /** 子会话摘要用 `conversationKind` 表达类型（如 side_chat），与 `kind` 二选一 */
  conversationKind?: string;
  /** 队员在队时的角色名 */
  teamName?: string;
  /** 会话执行人格名 */
  agentName?: string;
};

type ResolveConversationDisplayTitleOptions = {
  locale?: string;
  untitledLabel: string;
  /** 系统通知会话的标题；缺省时回退到中文常量，用于没有 i18n 上下文的调用方 */
  systemNotificationLabel?: string;
};

function normalizedTitlePart(value?: string, conversationId?: string): string {
  const title = String(value || "").trim();
  if (!title) return "";
  const normalizedConversationId = String(conversationId || "").trim();
  if (normalizedConversationId && title === normalizedConversationId) return "";
  return title;
}

export const SYSTEM_NOTIFICATION_DISPLAY_TITLE = "系统通知";

export function formatConversationFallbackTitle(value?: string, locale?: string): string {
  const rawValue = normalizedTitlePart(value);
  if (!rawValue) return "";
  const date = new Date(rawValue);
  if (Number.isNaN(date.getTime())) return rawValue;
  const now = new Date();
  const sameYear = date.getFullYear() === now.getFullYear();
  const dateText = date.toLocaleDateString(locale || undefined, sameYear
    ? {
      month: "2-digit",
      day: "2-digit",
    }
    : {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    });
  const timeText = date.toLocaleTimeString(locale || undefined, {
    hour: "2-digit",
    minute: "2-digit",
  });
  return `${dateText} ${timeText}`.trim();
}

/**
 * 降级链：会话标题 → 摘要标题 → 时间兜底 → 人格名 → 「未命名会话」。
 *
 * - 标题等于会话 id 时视为「还没起标题」，后端在标题为空时会把 id 写进 title。
 * - 时间兜底让人知道这是个「几点的会话」，比一串 id 有用。
 * - 人格名只对人参与的会话有意义（队员在队时带角色名、追问带执行人格名），
 *   它排在时间之后：有时有人的意图仍有价值，但它确实不是标题。
 */
export function resolveConversationDisplayTitle(
  item: ConversationTitleLike,
  options: ResolveConversationDisplayTitleOptions,
): string {
  if (item.isSystemNotificationConversation) {
    return String(options.systemNotificationLabel || "").trim() || SYSTEM_NOTIFICATION_DISPLAY_TITLE;
  }
  const kind = String(item.kind || item.conversationKind || "").trim();
  if (kind === "remote_im_contact") {
    return normalizedTitlePart(item.remoteContactDisplayName)
      || normalizedTitlePart(item.title, item.conversationId)
      || options.untitledLabel;
  }
  return normalizedTitlePart(item.title, item.conversationId)
    || normalizedTitlePart(item.summaryTitle, item.conversationId)
    || formatConversationFallbackTitle(item.lastMessageAt || item.updatedAt, options.locale)
    || String(item.teamName || "").trim()
    || String(item.agentName || "").trim()
    || options.untitledLabel;
}
