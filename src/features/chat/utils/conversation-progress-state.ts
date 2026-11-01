// ==================== 会话状态（progressState）事件补丁（纯函数） ====================
//
// 会话状态走独立事件通道（概览项广播对追问会话短路不广播），载荷为 camelCase：
//   { conversationId, progressState, evidence, evidenceUpdatedAt }
//
// 三类会话在同一份概览列表里出现：
//   - 主会话：列表项本身
//   - 追问会话：父项的 childConversations 里，sideConversations 由它派生
//   - 远程联系人：同一个列表项，kind = remote_im_contact
// 远程联系人不做会话状态，后端不下发该字段，行组件也不渲染状态元素，
// 所以打补丁只需覆盖「项本身」与「子项」两层，不必另建覆盖表。

export type ConversationProgressStatePayload = {
  conversationId: string;
  /** 六态 key 或空串（空串＝未设置） */
  progressState: string;
  evidence: string;
  evidenceUpdatedAt: string;
};

/**
 * 从事件载荷里读契约字段。字段缺失或类型不对一律归一成空串，
 * 也就是「未设置」——不要把缺失当成某个默认态。
 */
export function readConversationProgressStatePayload(
  payload?: Record<string, any> | null,
): ConversationProgressStatePayload | null {
  const conversationId = String(payload?.conversationId || "").trim();
  if (!conversationId) return null;
  // 只认契约字段 camelCase：后端已定稿，不存在双格式并存期。
  // 留 snake_case 兜底会把「后端字段名发错」变成静默降级，错误要立刻可见。
  return {
    conversationId,
    progressState: String(payload?.progressState ?? "").trim(),
    evidence: String(payload?.evidence ?? "").trim(),
    evidenceUpdatedAt: String(payload?.evidenceUpdatedAt ?? "").trim(),
  };
}

function patchSingleItem<T extends Record<string, any>>(
  item: T,
  payload: ConversationProgressStatePayload,
): T {
  const sameState = String(item?.progressState || "").trim() === payload.progressState;
  const sameEvidence = String(item?.evidence || "").trim() === payload.evidence;
  const sameEvidenceAt = String(item?.evidenceUpdatedAt || "").trim() === payload.evidenceUpdatedAt;
  // 状态没变但证据变了，同样是一次真实变化，不能丢
  if (sameState && sameEvidence && sameEvidenceAt) return item;
  return {
    ...item,
    progressState: payload.progressState,
    evidence: payload.evidence,
    evidenceUpdatedAt: payload.evidenceUpdatedAt,
  };
}

/**
 * 把状态补丁打进一份会话摘要列表。命中项及其子项会被替换成新对象，
 * 未变化时返回原数组引用（调用方可据此跳过赋值）。
 */
export function patchConversationProgressStateList<T extends Record<string, any>>(
  items: T[],
  payload: ConversationProgressStatePayload,
): { items: T[]; changed: boolean } {
  if (!Array.isArray(items) || items.length === 0) {
    return { items, changed: false };
  }
  let changed = false;
  const nextItems = items.map((item) => {
    let nextItem = item;
    if (String(item?.conversationId || "").trim() === payload.conversationId) {
      const patched = patchSingleItem(item, payload);
      if (patched !== item) {
        changed = true;
        nextItem = patched;
      }
    }
    const children = Array.isArray(item?.childConversations) ? item.childConversations : null;
    if (!children || children.length === 0) return nextItem;
    let childrenChanged = false;
    const nextChildren = children.map((child: Record<string, any>) => {
      if (String(child?.conversationId || "").trim() !== payload.conversationId) return child;
      const patched = patchSingleItem(child, payload);
      if (patched !== child) childrenChanged = true;
      return patched;
    });
    if (!childrenChanged) return nextItem;
    changed = true;
    return { ...nextItem, childConversations: nextChildren };
  });
  return { items: changed ? nextItems : items, changed };
}
