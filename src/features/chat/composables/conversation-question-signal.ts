import { reactive } from "vue";
import type { ConversationQuestionView } from "../utils/conversation-question";

/**
 * 会话问题的窗口级最新快照。
 *
 * 视图层不再只靠自己的 `question.itemsUpdated` 订阅拿这份数据：那条订阅在组件层的
 * 注册时机一旦落进「运行时还没就绪」的窗口期，就会静默降级成本地总线，桌面广播再也
 * 命中不了它，而这次降级连日志都写不出来（探针本身要靠 invoke）。
 *
 * 窗口层那份同名订阅是好的——会话行的待答角标就是它刷新的。所以改由窗口层把事件里的
 * items 按会话存下来，视图层只读自己这个会话的那一份：零额外请求，也不会把用户已经
 * 勾好的选项冲掉。
 */
const latestItemsByConversation = reactive<Record<string, ConversationQuestionView[]>>({});

/** 窗口层收到问题变更后写入该会话的最新条目。 */
export function publishConversationQuestionItems(
  conversationId: string,
  items: ConversationQuestionView[],
): void {
  const id = String(conversationId || "").trim();
  if (!id) return;
  latestItemsByConversation[id] = Array.isArray(items) ? items : [];
}

/**
 * 读某个会话的最新条目；没收到过该会话的推送时返回 undefined。
 *
 * 在 watch / computed 的取值函数里读即建立依赖，推送换了新数组就会触发。
 */
export function peekConversationQuestionItems(
  conversationId: string,
): ConversationQuestionView[] | undefined {
  return latestItemsByConversation[String(conversationId || "").trim()];
}
