import { computed, onBeforeUnmount, onMounted, ref, type Ref } from "vue";
import { invokeTauri, onTransportNotification } from "../../../services/tauri-api";

export type KanbanIssue = {
  number: number;
  title: string;
  status: string;
  assignee: string;
  note: string;
  /** 挂在这条议题上的计划文件路径（磁盘绝对路径），空表示没挂。 */
  planPath?: string;
  createdAt?: string;
  updatedAt?: string;
  version?: number;
};

export type KanbanBoardView = {
  conversationId: string;
  issues: KanbanIssue[];
};

/** 需要用户参与的四类，顺序按产品口径定（待审 → 待办 → 阻塞 → 待议），不由紧迫度推导。
    磁贴泳道与看板「决策」视图共用这一份，两边不再各写一套集合。 */
export const KANBAN_ATTENTION_ORDER = ["in_review", "todo", "blocked", "discussing"] as const;

/** 磁贴的两个泳道与看板时间线共用同一份状态集合：参与栏（KANBAN_ATTENTION_ORDER）放
    需要用户参与的四类，进展栏放解决中、已完成、已关闭这三类，两份合起来覆盖全部七态。
    磁贴里的条目是滚动看的，某一类条数变多不影响其它栏。 */
export const KANBAN_PROGRESS_ORDER = ["in_progress", "done", "closed"] as const;

/** 看板页时间线自上而下的阶段顺序：全部七态，主链走完再到已关闭。
    blocked 不占主链位置——它可以从任何阶段进入、也可以回到任何阶段，放进中间会被读成
    「流程中的一站」。放在线首，读作「偏离主链的汇总区」：先看清哪里卡住了，再往下看推进。
    这里展示全貌，与磁贴两个泳道不是同一份集合，不要互相推导。 */
export const KANBAN_STAGE_ORDER = [
  "blocked",
  "discussing",
  "todo",
  "in_progress",
  "in_review",
  "done",
  "closed",
] as const;

export type KanbanStageStatus = (typeof KANBAN_STAGE_ORDER)[number];

export function kanbanIssuesByStatus(
  issues: KanbanIssue[],
  status: string,
): KanbanIssue[] {
  return issues.filter((issue) => String(issue.status || "").trim() === status);
}

/** 磁贴的参与栏：只取这四类，其余状态不上磁贴。 */
export function kanbanAttentionIssues(issues: KanbanIssue[]): KanbanIssue[] {
  return kanbanIssuesInOrder(issues, KANBAN_ATTENTION_ORDER);
}

/** 按给定顺序取子集并排序：不在顺序里的状态直接丢弃，同状态按编号升序。
    磁贴两个泳道与看板时间线都从这里取序列，避免各写一份过滤+排序。 */
export function kanbanIssuesInOrder(
  issues: KanbanIssue[],
  order: readonly string[],
): KanbanIssue[] {
  const rankOf = (issue: KanbanIssue) => order.indexOf(String(issue.status || "").trim());
  return issues
    .map((issue) => ({ issue, rank: rankOf(issue) }))
    .filter((entry) => entry.rank >= 0)
    .sort((a, b) => a.rank - b.rank || Number(a.issue.number || 0) - Number(b.issue.number || 0))
    .map((entry) => entry.issue);
}

export function useKanbanBoard(conversationId: Ref<string>) {
  const issues = ref<KanbanIssue[]>([]);
  const loading = ref(false);
  /** 成功读到过看板就为 true：磁贴靠它决定要不要出现。
      非主会话调用会直接报错，这里就不会置位，磁贴也就不显示。 */
  const loaded = ref(false);
  const errorText = ref("");
  const approvingNumber = ref<number | null>(null);

  async function reload() {
    const target = String(conversationId.value || "").trim();
    if (!target) {
      issues.value = [];
      loaded.value = false;
      return;
    }
    loading.value = true;
    try {
      const view = await invokeTauri<KanbanBoardView>("kanban_get_board", {
        input: { conversationId: target },
      });
      // 会话已经切走时丢弃这次结果，避免旧看板覆盖新看板
      if (String(conversationId.value || "").trim() !== target) return;
      issues.value = Array.isArray(view?.issues) ? view.issues : [];
      errorText.value = "";
      loaded.value = true;
    } catch (error: any) {
      if (String(conversationId.value || "").trim() !== target) return;
      issues.value = [];
      errorText.value = String(error?.message || error || "读取议题看板失败");
    } finally {
      loading.value = false;
    }
  }

  async function approve(number: number, note = "") {
    const target = String(conversationId.value || "").trim();
    if (!target) return;
    approvingNumber.value = number;
    try {
      await invokeTauri<KanbanIssue>("kanban_approve_issue", {
        input: { conversationId: target, number, note },
      });
      await reload();
    } finally {
      approvingNumber.value = null;
    }
  }

  // 队长用工具改看板时靠这条广播刷新；用户点通过自己会 reload，不依赖它。
  let stopUpdatedListener: (() => void) | null = null;
  onMounted(() => {
    stopUpdatedListener = onTransportNotification<any>("kanban.updated", (payload) => {
      const target = String(conversationId.value || "").trim();
      if (!target) return;
      const changed = String(payload?.conversationId || "").trim();
      if (changed && changed !== target) return;
      void reload();
    });
  });
  onBeforeUnmount(() => {
    stopUpdatedListener?.();
    stopUpdatedListener = null;
  });

  const attentionIssues = computed(() => kanbanAttentionIssues(issues.value));

  return {
    issues,
    attentionIssues,
    loading,
    loaded,
    errorText,
    approvingNumber,
    reload,
    approve,
  };
}
