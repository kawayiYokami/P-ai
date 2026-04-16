import { computed, ref, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import { formatDateToLocalRfc3339 } from "../../../utils/time";
import { toErrorMessage } from "../../../utils/error";
import type { TaskEntry } from "../../config/views/config-tabs/task-editor";

const SUPERVISION_TASK_GOAL_PREFIX = "督工任务：";
const SUPERVISION_TASK_HISTORY_STORAGE_KEY = "chat-supervision-task-history";
const SUPERVISION_TASK_HISTORY_LIMIT = 3;

export type ActiveSupervisionTaskSummary = {
  taskId: string;
  goal: string;
  why: string;
  todo: string;
  endAtLocal: string;
  remainingHours: number;
};

export type SupervisionTaskHistoryEntry = {
  goal: string;
  why: string;
  todo: string;
  durationHours: number;
};

type UseSupervisionTaskOptions = {
  t: (key: string, params?: Record<string, unknown>) => string;
  currentConversationId: Ref<string>;
  setStatus: (message: string) => void;
};

export function useSupervisionTask(options: UseSupervisionTaskOptions) {
  const supervisionTaskDialogOpen = ref(false);
  const supervisionTaskSaving = ref(false);
  const supervisionTaskError = ref("");
  const activeSupervisionTask = ref<ActiveSupervisionTaskSummary | null>(null);
  const recentSupervisionTaskHistory = ref<SupervisionTaskHistoryEntry[]>([]);
  let supervisionTaskPollTimer = 0;

  function clearSupervisionTaskPollTimer() {
    if (supervisionTaskPollTimer) {
      window.clearInterval(supervisionTaskPollTimer);
      supervisionTaskPollTimer = 0;
    }
  }

  function normalizeSupervisionGoal(goal: string): string {
    const text = String(goal || "").trim();
    if (!text) return SUPERVISION_TASK_GOAL_PREFIX;
    if (text.startsWith(SUPERVISION_TASK_GOAL_PREFIX)) {
      return text;
    }
    return `${SUPERVISION_TASK_GOAL_PREFIX}${text}`;
  }

  function stripSupervisionGoalPrefix(goal: string): string {
    const text = String(goal || "").trim();
    if (!text.startsWith(SUPERVISION_TASK_GOAL_PREFIX)) {
      return text;
    }
    return text.slice(SUPERVISION_TASK_GOAL_PREFIX.length).trim();
  }

  function parseTaskTime(value?: string | null): Date | null {
    const raw = String(value || "").trim();
    if (!raw) return null;
    const parsed = new Date(raw);
    return Number.isNaN(parsed.getTime()) ? null : parsed;
  }

  function normalizeSupervisionTaskHistoryEntry(entry: Partial<SupervisionTaskHistoryEntry>): SupervisionTaskHistoryEntry | null {
    const goal = String(entry.goal || "").trim();
    const why = String(entry.why || "").trim();
    const todo = String(entry.todo || "").trim();
    const durationHours = Math.min(24, Math.max(1, Number(entry.durationHours || 1)));
    if (!goal || !todo) return null;
    return {
      goal,
      why,
      todo,
      durationHours,
    };
  }

  function loadRecentSupervisionTaskHistory() {
    try {
      const raw = window.localStorage.getItem(SUPERVISION_TASK_HISTORY_STORAGE_KEY);
      if (!raw) {
        recentSupervisionTaskHistory.value = [];
        return;
      }
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) {
        recentSupervisionTaskHistory.value = [];
        return;
      }
      const normalized: SupervisionTaskHistoryEntry[] = [];
      const seen = new Set<string>();
      for (const item of parsed) {
        const entry = normalizeSupervisionTaskHistoryEntry(
          (item || {}) as Partial<SupervisionTaskHistoryEntry>,
        );
        if (!entry) continue;
        const dedupeKey = JSON.stringify(entry);
        if (seen.has(dedupeKey)) continue;
        seen.add(dedupeKey);
        normalized.push(entry);
        if (normalized.length >= SUPERVISION_TASK_HISTORY_LIMIT) break;
      }
      recentSupervisionTaskHistory.value = normalized;
    } catch {
      recentSupervisionTaskHistory.value = [];
    }
  }

  function saveRecentSupervisionTaskHistory() {
    try {
      window.localStorage.setItem(
        SUPERVISION_TASK_HISTORY_STORAGE_KEY,
        JSON.stringify(recentSupervisionTaskHistory.value),
      );
    } catch {
      // ignore persistence failures
    }
  }

  function pushRecentSupervisionTaskHistory(entry: Partial<SupervisionTaskHistoryEntry>) {
    const normalized = normalizeSupervisionTaskHistoryEntry(entry);
    if (!normalized) return;
    const dedupeKey = JSON.stringify(normalized);
    recentSupervisionTaskHistory.value = [
      normalized,
      ...recentSupervisionTaskHistory.value.filter((item) => JSON.stringify(item) !== dedupeKey),
    ].slice(0, SUPERVISION_TASK_HISTORY_LIMIT);
    saveRecentSupervisionTaskHistory();
  }

  function supervisionTaskIsActive(task: TaskEntry, conversationId: string): boolean {
    if (String(task.completionState || "").trim() !== "active") return false;
    if (String(task.conversationId || "").trim() !== conversationId) return false;
    if (!String(task.goal || "").trim().startsWith(SUPERVISION_TASK_GOAL_PREFIX)) return false;
    const runAt = parseTaskTime(task.trigger?.runAtLocal);
    const endAt = parseTaskTime(task.trigger?.endAtLocal);
    if (!endAt) return false;
    const now = new Date();
    if (runAt) {
      return now >= runAt && now <= endAt;
    }
    return now <= endAt;
  }

  function activeSupervisionTaskFromEntry(task: TaskEntry): ActiveSupervisionTaskSummary {
    const endAt = parseTaskTime(task.trigger?.endAtLocal);
    const remainingHours = endAt
      ? Math.min(24, Math.max(1, Math.ceil((endAt.getTime() - Date.now()) / 3_600_000)))
      : 1;
    return {
      taskId: String(task.taskId || "").trim(),
      goal: stripSupervisionGoalPrefix(task.goal),
      why: String(task.why || "").trim(),
      todo: String(task.todo || "").trim(),
      endAtLocal: String(task.trigger?.endAtLocal || "").trim(),
      remainingHours,
    };
  }

  const chatSupervisionActive = computed(() => !!activeSupervisionTask.value);
  const chatSupervisionTitle = computed(() => {
    const task = activeSupervisionTask.value;
    if (!task) {
      return options.t("chat.supervision.buttonHint");
    }
    return options.t("chat.supervision.activeHintShort", { endAt: task.endAtLocal });
  });

  async function refreshActiveSupervisionTask(params: { silent?: boolean } = {}) {
    const conversationId = String(options.currentConversationId.value || "").trim();
    if (!conversationId) {
      activeSupervisionTask.value = null;
      return;
    }
    try {
      const tasks = await invokeTauri<TaskEntry[]>("task_list_tasks");
      const nextTask = tasks
        .filter((task) => supervisionTaskIsActive(task, conversationId))
        .sort((left, right) => {
          const leftTime = parseTaskTime(left.updatedAtLocal)?.getTime() ?? 0;
          const rightTime = parseTaskTime(right.updatedAtLocal)?.getTime() ?? 0;
          return rightTime - leftTime;
        })[0];
      activeSupervisionTask.value = nextTask ? activeSupervisionTaskFromEntry(nextTask) : null;
    } catch (error) {
      activeSupervisionTask.value = null;
      if (!params.silent) {
        console.warn("[督工] 读取当前会话督工任务失败", error);
      }
    }
  }

  function openSupervisionTaskDialog() {
    if (!String(options.currentConversationId.value || "").trim()) {
      options.setStatus(options.t("chat.supervision.noConversation"));
      return;
    }
    supervisionTaskError.value = "";
    supervisionTaskDialogOpen.value = true;
  }

  function closeSupervisionTaskDialog() {
    if (supervisionTaskSaving.value) return;
    supervisionTaskDialogOpen.value = false;
    supervisionTaskError.value = "";
  }

  async function saveSupervisionTask(payload: {
    durationHours: number;
    goal: string;
    why: string;
    todo: string;
  }) {
    if (supervisionTaskSaving.value) return;
    const conversationId = String(options.currentConversationId.value || "").trim();
    if (!conversationId) {
      supervisionTaskError.value = options.t("chat.supervision.noConversation");
      return;
    }
    supervisionTaskSaving.value = true;
    supervisionTaskError.value = "";
    try {
      const now = new Date();
      now.setSeconds(0, 0);
      const endAt = new Date(now.getTime() + payload.durationHours * 3_600_000);
      const trigger = {
        runAtLocal: formatDateToLocalRfc3339(now),
        everyMinutes: 0.1,
        endAtLocal: formatDateToLocalRfc3339(endAt),
      };
      let taskId = "";
      if (activeSupervisionTask.value?.taskId) {
        const updated = await invokeTauri<TaskEntry>("task_update_task", {
          input: {
            taskId: activeSupervisionTask.value.taskId,
            conversationId,
            targetScope: "desktop",
            goal: normalizeSupervisionGoal(payload.goal),
            why: payload.why,
            todo: payload.todo,
            trigger,
          },
        });
        taskId = String(updated.taskId || "").trim();
        options.setStatus(
          options.t("chat.supervision.updatedStatus", {
            hours: payload.durationHours,
          }),
        );
      } else {
        const created = await invokeTauri<TaskEntry>("task_create_task", {
          input: {
            conversationId,
            targetScope: "desktop",
            goal: normalizeSupervisionGoal(payload.goal),
            why: payload.why,
            todo: payload.todo,
            trigger,
          },
        });
        taskId = String(created.taskId || "").trim();
        options.setStatus(
          options.t("chat.supervision.createdStatus", {
            hours: payload.durationHours,
          }),
        );
      }
      if (taskId) {
        try {
          await invokeTauri<boolean>("task_dispatch_task_now", { input: { taskId } });
        } catch (dispatchError) {
          console.warn("[督工] 首次触发失败", dispatchError);
        }
      }
      pushRecentSupervisionTaskHistory(payload);
      supervisionTaskDialogOpen.value = false;
      await refreshActiveSupervisionTask({ silent: true });
    } catch (error) {
      supervisionTaskError.value = `${options.t("chat.supervision.saveFailed")}: ${toErrorMessage(error)}`;
    } finally {
      supervisionTaskSaving.value = false;
    }
  }

  function startSupervisionTaskPolling() {
    clearSupervisionTaskPollTimer();
    supervisionTaskPollTimer = window.setInterval(() => {
      void refreshActiveSupervisionTask({ silent: true });
    }, 30_000);
  }

  function handleConversationChanged() {
    supervisionTaskDialogOpen.value = false;
    supervisionTaskError.value = "";
    void refreshActiveSupervisionTask({ silent: true });
  }

  loadRecentSupervisionTaskHistory();

  return {
    supervisionTaskDialogOpen,
    supervisionTaskSaving,
    supervisionTaskError,
    activeSupervisionTask,
    recentSupervisionTaskHistory,
    chatSupervisionActive,
    chatSupervisionTitle,
    openSupervisionTaskDialog,
    closeSupervisionTaskDialog,
    saveSupervisionTask,
    refreshActiveSupervisionTask,
    startSupervisionTaskPolling,
    clearSupervisionTaskPollTimer,
    handleConversationChanged,
  };
}
