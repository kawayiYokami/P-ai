export type TaskFilter = "" | "active" | "completed";

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

export type TaskEditorForm = {
  taskId: string;
  goal: string;
  why: string;
  todo: string;
  runAtLocal: string;
  everyMinutesText: string;
  endAtLocal: string;
  completionState: "completed" | "failed_completed";
  completionConclusion: string;
};

export function createEmptyTaskEditorForm(): TaskEditorForm {
  return {
    taskId: "",
    goal: "",
    why: "",
    todo: "",
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
    goal: task.goal || "",
    why: task.why || "",
    todo: task.todo || "",
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

export function taskEditorSnapshot(form: TaskEditorForm): string {
  const normalized = {
    taskId: String(form.taskId || "").trim(),
    goal: String(form.goal || "").trim(),
    why: String(form.why || "").trim(),
    todo: String(form.todo || "").trim(),
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
