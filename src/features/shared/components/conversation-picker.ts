import type { UnarchivedConversationSummary } from "../../../types/app";
import { resolveConversationDisplayTitle } from "../../chat/utils/conversation-title";

export type IndexedConversation = {
  /** 会话 ID */
  id: string;
  /** 倒序序号：最近更新的会话是 1 */
  index: number;
  /** 显示标题，统一由 `resolveConversationDisplayTitle` 产出 */
  title: string;
};

export type IndexConversationsOptions = {
  /** 一条会话都取不到标题时的兜底文案，调用方传 `chat.untitledConversation` */
  untitledLabel?: string;
  locale?: string;
};

/** 没有 i18n 上下文时的兜底，与 `conversation-title.ts` 里系统通知标题的处理方式一致 */
const DEFAULT_UNTITLED_LABEL = "未命名会话";

function conversationSortKey(item: UnarchivedConversationSummary): string {
  return String(item.updatedAt || item.lastMessageAt || "").trim();
}

/**
 * 把会话列表整成「会话 #N」需要的索引：按更新时间倒序，最近的排最前拿 1 号。
 *
 * 标题不在调用侧自己拼，一律走 `resolveConversationDisplayTitle`，
 * 否则同一个会话在选择器和侧边栏会显示成两个名字。
 */
export function indexConversations(
  list: UnarchivedConversationSummary[] | null | undefined,
  options: IndexConversationsOptions = {},
): IndexedConversation[] {
  const items = Array.isArray(list) ? list : [];
  const titleOptions = {
    untitledLabel: String(options.untitledLabel || "").trim() || DEFAULT_UNTITLED_LABEL,
    locale: options.locale,
  };
  return [...items]
    .sort((left, right) => conversationSortKey(right).localeCompare(conversationSortKey(left)))
    .map((item, position) => ({
      id: String(item?.conversationId || "").trim(),
      index: position + 1,
      title: resolveConversationDisplayTitle(item, titleOptions),
    }))
    .filter((item) => !!item.id);
}
