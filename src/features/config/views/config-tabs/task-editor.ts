import { inferFixedIntervalFromCronExpression } from "../../../chat/utils/task-schedule-cron";

export type TaskFilter = "" | "active" | "completed";

export type TaskTrigger = {
  run_at?: string;
  cron_expression?: string;
  every_minutes?: number;
  end_at?: string;
  next_run_at?: string;
};

export type TaskProgressNote = {
  atLocal: string;
  note: string;
};

export type TaskEntry = {
  taskId: string;
  conversationId?: string;
  agentId?: string;
  orderIndex: number;
  goal: string;
  why: string;
  todo: string;
  completionState: string;
  completionConclusion: string;
  progressNotes: TaskProgressNote[];
  trigger: TaskTrigger;
  createdAtLocal: string;
  updatedAtLocal: string;
  lastTriggeredAtLocal?: string;
  completedAtLocal?: string;
};

export type TaskRunLogEntry = {
  id: number;
  taskId: string;
  triggeredAtLocal: string;
  outcome: string;
  note: string;
};

export type TaskEditorMode = "create" | "edit";
export type TaskScheduleMode = "once" | "interval";

export type TaskEditorForm = {
  taskId: string;
  agentId: string;
  goal: string;
  why: string;
  todo: string;
  runAt: string;
  scheduleMode: TaskScheduleMode;
  repeatWeeks: string;
  repeatDays: string;
  repeatHours: string;
  preservedEveryMinutes: string;
  preservedCronExpression: string;
  endAt: string;
  completionState: "completed" | "failed_completed";
  completionConclusion: string;
};

const HOURS_PER_DAY = 24;
const HOURS_PER_WEEK = 24 * 7;
const MINUTES_PER_HOUR = 60;

function buildIntervalPartsFromHours(totalHours: number): {
  repeatWeeks: string;
  repeatDays: string;
  repeatHours: string;
} {
  let remainingHours = Math.max(0, Math.floor(totalHours));
  const weeks = Math.floor(remainingHours / HOURS_PER_WEEK);
  remainingHours -= weeks * HOURS_PER_WEEK;
  const days = Math.floor(remainingHours / HOURS_PER_DAY);
  remainingHours -= days * HOURS_PER_DAY;
  return {
    repeatWeeks: String(weeks),
    repeatDays: String(days),
    repeatHours: String(remainingHours),
  };
}

function buildSupportedIntervalFromEveryMinutes(value: number): {
  repeatWeeks: string;
  repeatDays: string;
  repeatHours: string;
} | null {
  if (!Number.isFinite(value) || value <= 0 || value % MINUTES_PER_HOUR !== 0) {
    return null;
  }
  const totalHours = value / MINUTES_PER_HOUR;
  if (totalHours < 1) {
    return null;
  }
  return buildIntervalPartsFromHours(totalHours);
}

function inferSupportedIntervalFromCron(cronExpression: string): {
  repeatWeeks: string;
  repeatDays: string;
  repeatHours: string;
} | null {
  const fixedInterval = inferFixedIntervalFromCronExpression(cronExpression);
  if (!fixedInterval) return null;
  if (fixedInterval.repeatUnit === "minutes") {
    return null;
  }
  const totalHours = fixedInterval.repeatUnit === "days"
    ? fixedInterval.repeatEvery * HOURS_PER_DAY
    : fixedInterval.repeatEvery;
  return buildIntervalPartsFromHours(totalHours);
}

export function createEmptyTaskEditorForm(): TaskEditorForm {
  return {
    taskId: "",
    agentId: "",
    goal: "",
    why: "",
    todo: "",
    runAt: "",
    scheduleMode: "once",
    repeatWeeks: "0",
    repeatDays: "0",
    repeatHours: "0",
    preservedEveryMinutes: "",
    preservedCronExpression: "",
    endAt: "",
    completionState: "completed",
    completionConclusion: "",
  };
}

export function taskEditorFormFromEntry(task: TaskEntry): TaskEditorForm {
  const legacyEveryMinutes = typeof task.trigger.every_minutes === "number" ? task.trigger.every_minutes : NaN;
  const supportedLegacyInterval = Number.isFinite(legacyEveryMinutes)
    ? buildSupportedIntervalFromEveryMinutes(legacyEveryMinutes)
    : null;
  const cronExpression = String(task.trigger.cron_expression || "").trim();
  const supportedCronInterval = cronExpression ? inferSupportedIntervalFromCron(cronExpression) : null;
  const intervalParts = supportedLegacyInterval || supportedCronInterval;
  const hasExistingRecurringSchedule = Number.isFinite(legacyEveryMinutes) || !!cronExpression;
  return {
    taskId: task.taskId,
    agentId: task.agentId || "",
    goal: task.goal || "",
    why: task.why || "",
    todo: task.todo || "",
    runAt: task.trigger.run_at || "",
    scheduleMode: hasExistingRecurringSchedule ? "interval" : "once",
    repeatWeeks: intervalParts?.repeatWeeks || "0",
    repeatDays: intervalParts?.repeatDays || "0",
    repeatHours: intervalParts?.repeatHours || "0",
    preservedEveryMinutes: Number.isFinite(legacyEveryMinutes) ? String(legacyEveryMinutes) : "",
    preservedCronExpression: cronExpression,
    endAt: task.trigger.end_at || "",
    completionState: "completed",
    completionConclusion: task.completionConclusion || "",
  };
}

export function taskEditorSnapshot(form: TaskEditorForm): string {
  const normalized = {
    taskId: String(form.taskId || "").trim(),
    agentId: String(form.agentId || "").trim(),
    goal: String(form.goal || "").trim(),
    why: String(form.why || "").trim(),
    todo: String(form.todo || "").trim(),
    runAt: String(form.runAt || "").trim(),
    scheduleMode: form.scheduleMode === "interval" ? "interval" : "once",
    repeatWeeks: String(form.repeatWeeks || "").trim(),
    repeatDays: String(form.repeatDays || "").trim(),
    repeatHours: String(form.repeatHours || "").trim(),
    preservedEveryMinutes: String(form.preservedEveryMinutes || "").trim(),
    preservedCronExpression: String(form.preservedCronExpression || "").trim(),
    endAt: String(form.endAt || "").trim(),
    completionState:
      String(form.completionState || "").trim() === "failed_completed" ? "failed_completed" : "completed",
    completionConclusion: String(form.completionConclusion || "").trim(),
  };
  return JSON.stringify(normalized);
}

export function taskUpsertEntry(entries: TaskEntry[], next: TaskEntry): TaskEntry[] {
  const list = Array.isArray(entries) ? entries.slice() : [];
  const index = list.findIndex((item) => item.taskId === next.taskId);
  if (index >= 0) {
    list[index] = next;
  } else {
    list.push(next);
  }
  list.sort((a, b) => a.orderIndex - b.orderIndex);
  return list;
}
