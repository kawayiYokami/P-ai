export type TaskFilter = "" | "active" | "tracked" | "completed";

export type TaskTrigger = {
  runAtLocal?: string;
  endAtLocal?: string;
  everyMinutes?: number;
  nextRunAtLocal?: string;
};

export type TaskProgressNote = {
  atLocal: string;
  note: string;
};

export type TaskEntry = {
  taskId: string;
  conversationId?: string;
  orderIndex: number;
  title: string;
  cause: string;
  goal: string;
  flow: string;
  todos: string[];
  statusSummary: string;
  completionState: string;
  completionConclusion: string;
  progressNotes: TaskProgressNote[];
  stageKey: string;
  stageUpdatedAtLocal?: string;
  trigger: TaskTrigger;
  createdAtLocal: string;
  updatedAtLocal: string;
  lastTriggeredAtLocal?: string;
  completedAtLocal?: string;
  currentTracked: boolean;
};

export type TaskRunLogEntry = {
  id: number;
  taskId: string;
  triggeredAtLocal: string;
  outcome: string;
  note: string;
};

export type TaskEditorMode = "create" | "edit";

export type TaskEditorForm = {
  taskId: string;
  title: string;
  cause: string;
  goal: string;
  flow: string;
  statusSummary: string;
  todosText: string;
  stageKey: string;
  appendNote: string;
  runAtLocal: string;
  everyMinutesText: string;
  endAtLocal: string;
  completionState: "completed" | "failed_completed";
  completionConclusion: string;
};

export function createEmptyTaskEditorForm(): TaskEditorForm {
  return {
    taskId: "",
    title: "",
    cause: "",
    goal: "",
    flow: "",
    statusSummary: "",
    todosText: "",
    stageKey: "",
    appendNote: "",
    runAtLocal: "",
    everyMinutesText: "",
    endAtLocal: "",
    completionState: "completed",
    completionConclusion: "",
  };
}

export function taskEditorFormFromEntry(task: TaskEntry): TaskEditorForm {
  return {
    taskId: task.taskId,
    title: task.title || "",
    cause: task.cause || "",
    goal: task.goal || "",
    flow: task.flow || "",
    statusSummary: task.statusSummary || "",
    todosText: (task.todos || []).join("；"),
    stageKey: task.stageKey || "",
    appendNote: "",
    runAtLocal: task.trigger.runAtLocal || "",
    everyMinutesText:
      typeof task.trigger.everyMinutes === "number" && Number.isFinite(task.trigger.everyMinutes)
        ? String(task.trigger.everyMinutes)
        : "",
    endAtLocal: task.trigger.endAtLocal || "",
    completionState: "completed",
    completionConclusion: task.completionConclusion || "",
  };
}

export function taskEditorTodosFromText(value: string): string[] {
  return String(value || "")
    .split(/[\r\n；;|]+/)
    .map((item) => item.trim())
    .filter(Boolean);
}

export function taskEditorSnapshot(form: TaskEditorForm): string {
  const normalized = {
    taskId: String(form.taskId || "").trim(),
    title: String(form.title || "").trim(),
    cause: String(form.cause || "").trim(),
    goal: String(form.goal || "").trim(),
    flow: String(form.flow || "").trim(),
    statusSummary: String(form.statusSummary || "").trim(),
    todos: taskEditorTodosFromText(form.todosText),
    stageKey: String(form.stageKey || "").trim(),
    appendNote: String(form.appendNote || "").trim(),
    runAtLocal: String(form.runAtLocal || "").trim(),
    everyMinutesText: String(form.everyMinutesText || "").trim(),
    endAtLocal: String(form.endAtLocal || "").trim(),
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
