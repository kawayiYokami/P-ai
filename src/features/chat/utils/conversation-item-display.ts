import type { ChatConversationOverviewItem, ConversationPreviewMessage, ConversationProgressState } from "../../../types/app";

// ==================== 会话项展示判定（纯函数，供 ChatConversationItem 与 Sidebar 共用） ====================

export type ConversationItemLevel = "full" | "sim" | "mini";

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

// ==================== 会话状态（progressState）展示 ====================
//
// 会话状态是第三层信息，与下面两层互不相干，取色也绝不借用它们的类：
//   - 运行生命周期：runtimeState / MainSessionState（idle / assistant_streaming / organizing_context）
//   - 列表活动态：ConversationListItemState.activity（idle / busy / completed / failed）
// 三者会同时出现在同一行：「已交付（done）但仍在执行工具」是合法画面，
// 所以状态标记与忙碌点各管各的，不做「有状态就隐藏忙碌点」这类联动。

export type ConversationProgressVisual = {
  /** 归一化后的六态 key；未设置或无法识别的值统一为空串 */
  key: ConversationProgressState | "";
  /** 状态名文案的 i18n key */
  labelKey: string;
  /** 圆点旁短标签的 i18n key；未设置为空串（没有状态就不占文字位置） */
  shortLabelKey: string;
  /** 圆点填充色 */
  dotClass: string;
  /**
   * 胶囊的表面处理，一整套给完：颜色 + 前景色。形态统一为实心，只有色差。
   * 需要带文字时用这一套，不要裸写 text-success / text-warning：
   * 徽章自带 *-content 前景色，明暗主题下都可读。
   */
  chipClass: string;
};

/**
 * 圆点的几何与描边。描边用中性色，只负责「这里有个点」的存在性；
 * 填充色只负责区分状态。存在性与状态分离——否则浅色主题下
 * 未设置（base-content/25）会淡到看不出，读者连「有没有状态」都判不了。
 * 1px 用 ring 而不是 border：ring 不占布局，6px 的圆点不会被撑形。
 */
export const CONVERSATION_PROGRESS_DOT_CLASS =
  "h-1.5 w-1.5 shrink-0 rounded-full ring-1 ring-base-content/40";

/** 圆点整套类名（几何 + 描边 + 填充），供不该反向依赖本模块的组件整套取用 */
export function conversationProgressDotClass(visual: ConversationProgressVisual): string {
  return `${CONVERSATION_PROGRESS_DOT_CLASS} ${visual.dotClass}`;
}

/**
 * 六态 + 未设置的唯一取色来源。颜色只用 DaisyUI 语义 token，不写死调色板值，
 * 因为项目挂了 12 套主题，只有 light / dark 有自定义覆盖，其余走 DaisyUI 内置色板。
 *
 * 两条禁令，任何人都不要「优化」回去：
 *   1. 不用 info（#0ea5e9）——它与 primary（#1d5cc2）同属蓝系，小圆点上分不出来。
 *   2. 不用 secondary（#219090）——它与 success（#10b981）同为 teal-green 系，
 *      而「待办」与「完成」恰好是两个极端语义，撞色会让用户直接读反。
 * 中性色交给 base-content 用透明度分两档：未设置是没有状态，待办是有状态但没开始。
 */
const PROGRESS_STATE_VISUALS: Record<ConversationProgressState | "", ConversationProgressVisual> = {
  "": {
    key: "",
    labelKey: "chat.conversationStatus.unset",
    // 空状态不占文字位置：只有灰点，没有标签
    shortLabelKey: "",
    dotClass: "bg-base-content/25",
    chipClass: "badge-ghost",
  },
  todo: {
    key: "todo",
    labelKey: "chat.conversationStatus.todo",
    shortLabelKey: "chat.conversationStatus.short.todo",
    dotClass: "bg-base-content/45",
    /*
     * 中性灰填充。不用 badge-ghost：它在浅色下就是 base-200，与列表底色同值，
     * 胶囊只剩两个浮着的字，和其余实心胶囊不成体系；深色下同理（base-200 == 行底）。
     * 用 base-content 的透明度做填充，明暗两侧都真的有一块底色。
     */
    chipClass: "bg-base-content/15 text-base-content",
  },
  planning: {
    key: "planning",
    labelKey: "chat.conversationStatus.planning",
    shortLabelKey: "chat.conversationStatus.short.planning",
    dotClass: "bg-accent",
    chipClass: "badge-accent",
  },
  in_progress: {
    key: "in_progress",
    labelKey: "chat.conversationStatus.in_progress",
    shortLabelKey: "chat.conversationStatus.short.inProgress",
    dotClass: "bg-primary",
    chipClass: "badge-primary",
  },
  in_review: {
    key: "in_review",
    labelKey: "chat.conversationStatus.in_review",
    shortLabelKey: "chat.conversationStatus.short.inReview",
    dotClass: "bg-warning",
    chipClass: "badge-warning",
  },
  blocked: {
    key: "blocked",
    labelKey: "chat.conversationStatus.blocked",
    shortLabelKey: "chat.conversationStatus.short.blocked",
    dotClass: "bg-error",
    chipClass: "badge-error",
  },
  done: {
    key: "done",
    labelKey: "chat.conversationStatus.done",
    shortLabelKey: "chat.conversationStatus.short.done",
    dotClass: "bg-success",
    chipClass: "badge-success",
  },
};

export const CONVERSATION_PROGRESS_STATE_KEYS: ConversationProgressState[] = [
  "planning",
  "todo",
  "in_progress",
  "in_review",
  "blocked",
  "done",
];

/**
 * 进度状态的排序名次：直接取上面数组的下标，按推进顺序排。
 * 未设置与未知值统一排在最后——「没有状态」意味着连这一步都还没开始记录，
 * 不该插在正在推进的会话中间；也不单独给它们编一组序号，避免多一套顺序定义。
 */
export function conversationProgressSortRank(progressState?: string | null): number {
  const key = String(progressState || "").trim();
  const index = (CONVERSATION_PROGRESS_STATE_KEYS as string[]).indexOf(key);
  return index === -1 ? CONVERSATION_PROGRESS_STATE_KEYS.length : index;
}

/**
 * 状态 key → 视觉。无法识别的值（例如后端加了第七态但前端还没跟上）
 * 一律退回「未设置」，绝不复用某个已知状态的配色，避免报错误信息。
 */
export function conversationProgressVisual(progressState?: string | null): ConversationProgressVisual {
  const key = String(progressState || "").trim();
  const visual = (PROGRESS_STATE_VISUALS as Record<string, ConversationProgressVisual | undefined>)[key];
  return visual || PROGRESS_STATE_VISUALS[""];
}

/** 是否是可识别的六态之一（未设置与未知值都返回 false） */
export function isKnownProgressState(progressState?: string | null): progressState is ConversationProgressState {
  const key = String(progressState || "").trim();
  return (CONVERSATION_PROGRESS_STATE_KEYS as string[]).includes(key);
}

/**
 * 这类会话是否有「会话状态」这个概念。白名单，不是黑名单：
 * 只有追问会话（side_chat）有状态——主会话管的是议题怎么流转，用看板；
 * 追问会话单线程单任务，用它汇报这条线做到哪一步。队员会话也是 side_chat，走同一档。
 * 系统通知、委托、远程联系人等将来新增的类型都不会自动获得状态标记——
 * 黑名单写法（不等于 remote_im_contact）会在新增类型时静默放行，没人会发现。
 */
export function conversationShowsProgressState(conversationKind?: string | null): boolean {
  const kind = String(conversationKind || "").trim();
  return kind === "side_chat";
}

