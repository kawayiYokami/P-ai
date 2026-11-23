import { computed, ref, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import { toErrorMessage } from "../../../utils/error";
import type { ConversationGoalState } from "../../../types/app";
import { clearNativeTextSelection } from "../../../utils/native-selection";

const GOAL_TASK_HISTORY_STORAGE_KEY = "chat-goal-task-history";
const GOAL_TASK_HISTORY_LIMIT = 3;

export type ActiveGoalTaskSummary = {
  taskId: string;
  goal: string;
  why: string;
  todo: string;
  endAtLocal: string;
  remainingHours: number;
};

export type GoalTaskHistoryEntry = {
  goal: string;
  why: string;
  todo: string;
  durationHours: number;
};

type GoalMutationOutput = {
  conversationId: string;
  goal: ConversationGoalState;
};

type GoalUpdatedPayload = {
  conversationId?: string;
  goal?: ConversationGoalState | null;
};

type UseGoalTaskOptions = {
  t: (key: string, params?: Record<string, unknown>) => string;
  currentConversationId: Ref<string>;
  setStatus: (message: string) => void;
};

function goalIsActive(goal?: ConversationGoalState | null): goal is ConversationGoalState {
  return String(goal?.status || "").trim() === "active";
}

function activeGoalTaskFromGoal(goal: ConversationGoalState): ActiveGoalTaskSummary {
  return {
    taskId: String(goal.goalId || "").trim(),
    goal: String(goal.objective || "").trim(),
    why: "",
    todo: "",
    endAtLocal: String(goal.startedAt || "").trim(),
    remainingHours: 0,
  };
}

export function useGoalTask(options: UseGoalTaskOptions) {
  const goalDialogOpen = ref(false);
  const goalSaving = ref(false);
  const goalError = ref("");
  const activeGoalTask = ref<ActiveGoalTaskSummary | null>(null);
  const recentGoalTaskHistory = ref<GoalTaskHistoryEntry[]>([]);

  function clearGoalTaskPollTimer() {
    // Goal 状态由会话快照和 goalUpdated 事件驱动，不再轮询 task。
  }

  function normalizeGoalTaskHistoryEntry(entry: Partial<GoalTaskHistoryEntry>): GoalTaskHistoryEntry | null {
    const goal = String(entry.goal || "").trim();
    const why = String(entry.why || "").trim();
    const todo = String(entry.todo || "").trim();
    const durationHours = Math.min(24, Math.max(1, Number(entry.durationHours || 1)));
    if (!goal) return null;
    return {
      goal,
      why,
      todo,
      durationHours,
    };
  }

  function loadRecentGoalTaskHistory() {
    try {
      const raw = window.localStorage.getItem(GOAL_TASK_HISTORY_STORAGE_KEY);
      if (!raw) {
        recentGoalTaskHistory.value = [];
        return;
      }
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) {
        recentGoalTaskHistory.value = [];
        return;
      }
      const normalized: GoalTaskHistoryEntry[] = [];
      const seen = new Set<string>();
      for (const item of parsed) {
        const entry = normalizeGoalTaskHistoryEntry(
          (item || {}) as Partial<GoalTaskHistoryEntry>,
        );
        if (!entry) continue;
        const dedupeKey = JSON.stringify(entry);
        if (seen.has(dedupeKey)) continue;
        seen.add(dedupeKey);
        normalized.push(entry);
        if (normalized.length >= GOAL_TASK_HISTORY_LIMIT) break;
      }
      recentGoalTaskHistory.value = normalized;
    } catch {
      recentGoalTaskHistory.value = [];
    }
  }

  function saveRecentGoalTaskHistory() {
    try {
      window.localStorage.setItem(
        GOAL_TASK_HISTORY_STORAGE_KEY,
        JSON.stringify(recentGoalTaskHistory.value),
      );
    } catch {
      // ignore persistence failures
    }
  }

  function pushRecentGoalTaskHistory(entry: Partial<GoalTaskHistoryEntry>) {
    const normalized = normalizeGoalTaskHistoryEntry(entry);
    if (!normalized) return;
    const dedupeKey = JSON.stringify(normalized);
    recentGoalTaskHistory.value = [
      normalized,
      ...recentGoalTaskHistory.value.filter((item) => JSON.stringify(item) !== dedupeKey),
    ].slice(0, GOAL_TASK_HISTORY_LIMIT);
    saveRecentGoalTaskHistory();
  }

  const chatGoalActive = computed(() => !!activeGoalTask.value);
  const chatGoalTitle = computed(() => {
    const task = activeGoalTask.value;
    if (!task) {
      return options.t("chat.goal.buttonHint");
    }
    return options.t("chat.goal.activeHintShort", { goal: task.goal });
  });

  async function refreshActiveGoalTask(params: { silent?: boolean } = {}) {
    const conversationId = String(options.currentConversationId.value || "").trim();
    if (!conversationId) {
      activeGoalTask.value = null;
      return;
    }
    try {
      const goal = await invokeTauri<ConversationGoalState | null>("goal.current", {
        conversationId,
      });
      activeGoalTask.value = goalIsActive(goal) ? activeGoalTaskFromGoal(goal) : null;
    } catch (error) {
      activeGoalTask.value = null;
      if (!params.silent) {
        console.warn("[目标] 读取当前会话 goal 失败", error);
      }
    }
  }

  function applyConversationGoalUpdated(payload: GoalUpdatedPayload) {
    const conversationId = String(payload?.conversationId || "").trim();
    if (conversationId !== String(options.currentConversationId.value || "").trim()) return;
    const goal = payload?.goal || null;
    activeGoalTask.value = goalIsActive(goal) ? activeGoalTaskFromGoal(goal) : null;
  }

  function openGoalTaskDialog() {
    if (!String(options.currentConversationId.value || "").trim()) {
      options.setStatus(options.t("chat.goal.noConversation"));
      return;
    }
    clearNativeTextSelection();
    goalError.value = "";
    goalDialogOpen.value = true;
  }

  function closeGoalTaskDialog() {
    if (goalSaving.value) return;
    goalDialogOpen.value = false;
    goalError.value = "";
  }

  async function saveGoalTask(payload: {
    durationHours: number;
    goal: string;
    why: string;
    todo: string;
  }) {
    if (goalSaving.value) return;
    const conversationId = String(options.currentConversationId.value || "").trim();
    const objective = String(payload.goal || "").trim();
    if (!conversationId) {
      goalError.value = options.t("chat.goal.noConversation");
      return;
    }
    if (!objective) {
      goalError.value = options.t("chat.goal.goalPlaceholder");
      return;
    }
    goalSaving.value = true;
    goalError.value = "";
    try {
      const hadActiveGoal = !!activeGoalTask.value;
      if (hadActiveGoal) {
        await invokeTauri<GoalMutationOutput>("goal.cancel", {
          input: { conversationId },
        });
      }
      const created = await invokeTauri<GoalMutationOutput>("goal.create", {
        input: {
          conversationId,
          objective,
        },
      });
      activeGoalTask.value = goalIsActive(created.goal)
        ? activeGoalTaskFromGoal(created.goal)
        : null;
      options.setStatus(
        options.t(hadActiveGoal ? "chat.goal.updatedStatus" : "chat.goal.createdStatus"),
      );
      pushRecentGoalTaskHistory(payload);
      goalDialogOpen.value = false;
    } catch (error) {
      goalError.value = `${options.t("chat.goal.saveFailed")}: ${toErrorMessage(error)}`;
    } finally {
      goalSaving.value = false;
    }
  }

  async function stopGoalTask() {
    if (goalSaving.value) return;
    const conversationId = String(options.currentConversationId.value || "").trim();
    if (!conversationId || !activeGoalTask.value) {
      goalError.value = options.t("chat.goal.noActiveTask");
      return;
    }
    goalSaving.value = true;
    goalError.value = "";
    try {
      await invokeTauri<GoalMutationOutput>("goal.cancel", {
        input: { conversationId },
      });
      options.setStatus(options.t("chat.goal.stoppedStatus"));
      activeGoalTask.value = null;
      goalDialogOpen.value = false;
      await refreshActiveGoalTask({ silent: true });
    } catch (error) {
      goalError.value = `${options.t("chat.goal.stopFailed")}: ${toErrorMessage(error)}`;
    } finally {
      goalSaving.value = false;
    }
  }

  /** 模式面板里直接填目标内容启动，不经过完整目标弹窗。 */
  async function startGoalTaskWithObjective(objective: string) {
    if (goalSaving.value) return;
    const conversationId = String(options.currentConversationId.value || "").trim();
    const goal = String(objective || "").trim();
    if (!conversationId) {
      options.setStatus(options.t("chat.goal.noConversation"));
      return;
    }
    if (!goal) return;
    goalSaving.value = true;
    goalError.value = "";
    try {
      const hadActiveGoal = !!activeGoalTask.value;
      if (hadActiveGoal) {
        await invokeTauri<GoalMutationOutput>("goal.cancel", {
          input: { conversationId },
        });
      }
      const created = await invokeTauri<GoalMutationOutput>("goal.create", {
        input: { conversationId, objective: goal },
      });
      activeGoalTask.value = goalIsActive(created.goal)
        ? activeGoalTaskFromGoal(created.goal)
        : null;
      options.setStatus(
        options.t(hadActiveGoal ? "chat.goal.updatedStatus" : "chat.goal.createdStatus"),
      );
    } catch (error) {
      options.setStatus(`${options.t("chat.goal.saveFailed")}: ${toErrorMessage(error)}`);
    } finally {
      goalSaving.value = false;
    }
  }

  function startGoalTaskPolling() {
    void refreshActiveGoalTask({ silent: true });
  }  function handleConversationChanged() {
    goalDialogOpen.value = false;
    goalError.value = "";
    void refreshActiveGoalTask({ silent: true });
  }

  loadRecentGoalTaskHistory();

  return {
    goalDialogOpen,
    goalSaving,
    goalError,
    activeGoalTask,
    recentGoalTaskHistory,
    chatGoalActive,
    chatGoalTitle,
    openGoalTaskDialog,
    closeGoalTaskDialog,
    saveGoalTask,
    startGoalTaskWithObjective,
    stopGoalTask,
    refreshActiveGoalTask,
    startGoalTaskPolling,
    clearGoalTaskPollTimer,
    handleConversationChanged,
    applyConversationGoalUpdated,
  };
}
