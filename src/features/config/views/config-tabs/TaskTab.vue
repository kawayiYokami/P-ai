<template>
  <div class="space-y-3">
    <div
      v-if="message"
      class="rounded-box border px-3 py-2 text-sm"
      :class="messageError ? 'border-error/40 bg-error/10 text-error' : 'border-base-300 bg-base-200/50'"
    >
      {{ message }}
    </div>

    <div class="overflow-hidden rounded-box border border-base-300 bg-base-100">
      <div class="grid grid-cols-[auto_1fr_auto] items-center gap-2 border-b border-base-300/70 px-3 py-2">
        <div class="font-medium">
          {{ t("config.task.title") }}
          <span v-if="filteredTasks.length">（{{ filteredTasks.length }}）</span>
        </div>
        <div class="flex justify-center">
          <form class="filter" @reset.prevent="resetFilter">
            <input class="btn btn-sm btn-square" type="reset" value="×" :aria-label="t('common.reset')" :title="t('common.reset')" />
            <input class="btn btn-sm" type="radio" name="task-filter" value="active" :checked="filter === 'active'" :aria-label="t('config.task.filters.active')" @change="setFilter('active')" />
            <input class="btn btn-sm" type="radio" name="task-filter" value="completed" :checked="filter === 'completed'" :aria-label="t('config.task.filters.completed')" @change="setFilter('completed')" />
          </form>
        </div>
        <div class="flex items-center justify-end gap-2">
          <button class="btn btn-sm btn-ghost" :disabled="listLoading" @click="loadTasks()">
            <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 21h5v-5"/></svg>
          </button>
        </div>
      </div>

      <div v-if="listLoading && !tasks.length" class="py-8 text-center text-sm opacity-60">{{ t("common.loading") }}</div>

      <div v-else-if="pagedTasks.length" class="divide-y divide-base-300/60">
        <button
          v-for="task in pagedTasks"
          :key="task.taskId"
          class="block w-full px-3 py-3 text-left transition-colors"
          :class="selectedTaskId === task.taskId ? 'bg-primary/10' : 'hover:bg-base-200/50'"
          @click="openEditEditor(task.taskId)"
        >
          <div class="flex items-start gap-3">
            <div class="mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full" :class="task.completionState === 'completed' ? 'bg-success' : (task.completionState === 'failed_completed' ? 'bg-warning' : 'bg-base-300')"></div>
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <div class="font-medium text-sm wrap-break-word">{{ task.goal }}</div>
                <span class="badge badge-ghost">{{ completionStateLabel(task.completionState) }}</span>
              </div>
              <div class="mt-1 text-[11px] opacity-60 line-clamp-2">{{ task.todo || t("config.task.noTodo") }}</div>
              <div class="mt-2 flex flex-wrap items-center gap-2 text-[11px] opacity-50">
                <span>#{{ task.orderIndex }}</span>
                <span v-if="task.trigger.nextRunAtLocal">{{ formatTaskTime(task.trigger.nextRunAtLocal) }}</span>
                <span v-else>{{ formatTaskTime(task.updatedAtLocal) }}</span>
              </div>
            </div>
          </div>
        </button>
      </div>

      <div v-else class="py-6 text-center text-sm opacity-50">{{ t("config.task.empty") }}</div>

      <div v-if="totalPages > 1" class="flex justify-center border-t border-base-300/70 px-3 py-2">
        <div class="join">
          <button class="btn btn-sm join-item" :disabled="page <= 1" @click="page -= 1">‹</button>
          <button class="btn btn-sm join-item btn-active">{{ page }} / {{ totalPages }}</button>
          <button class="btn btn-sm join-item" :disabled="page >= totalPages" @click="page += 1">›</button>
        </div>
      </div>
    </div>

    <dialog ref="editorDialog" class="modal" @cancel.prevent="onEditorDialogCancel">
      <TaskEditorCard
        :mode="editorMode"
        :loading="editorLoading"
        :saving="editorSaving"
        :error-text="editorError"
        :form="editorForm"
        :task="editorTask"
        :logs="runLogs"
        :can-complete="editorCanComplete"
        :editable="editorEditable"
        @close="requestCloseEditor"
        @save="saveEditor"
        @complete="completeEditorTask"
        @delete="deleteEditorTask"
        @reload="reloadEditorTask"
        @refresh-logs="refreshLogs"
      />
      <form method="dialog" class="modal-backdrop">
        <button aria-label="close" @click.prevent="requestCloseEditor">close</button>
      </form>
    </dialog>
    <dialog ref="discardConfirmDialog" class="modal">
      <div class="modal-box max-w-md p-4">
        <h3 class="text-sm font-semibold">{{ t("common.confirm") }}</h3>
        <p class="mt-3 text-sm whitespace-pre-wrap">{{ t("config.task.discardConfirm") }}</p>
        <div class="modal-action mt-4">
          <button class="btn btn-sm btn-ghost" type="button" @click="closeDiscardConfirmDialog(false)">
            {{ t("common.cancel") }}
          </button>
          <button class="btn btn-sm btn-warning" type="button" @click="closeDiscardConfirmDialog(true)">
            {{ t("common.confirm") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button aria-label="close" @click.prevent="closeDiscardConfirmDialog(false)">close</button>
      </form>
    </dialog>
    <dialog ref="deleteConfirmDialog" class="modal">
      <div class="modal-box max-w-md p-4">
        <h3 class="text-sm font-semibold">{{ t("common.delete") }}</h3>
        <p class="mt-3 text-sm whitespace-pre-wrap">{{ t("config.task.deleteConfirm") }}</p>
        <div class="modal-action mt-4">
          <button class="btn btn-sm btn-ghost" type="button" @click="closeDeleteConfirmDialog(false)">
            {{ t("common.cancel") }}
          </button>
          <button class="btn btn-sm btn-error" type="button" @click="closeDeleteConfirmDialog(true)">
            {{ t("common.confirm") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button aria-label="close" @click.prevent="closeDeleteConfirmDialog(false)">close</button>
      </form>
    </dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";
import { formatIsoToLocalDateTime } from "../../../../utils/time";
import TaskEditorCard from "./TaskEditorCard.vue";
import {
  createEmptyTaskEditorForm,
  taskEditorFormFromEntry,
  taskEditorSnapshot,
  taskUpsertEntry,
  type TaskEditorForm,
  type TaskEditorMode,
  type TaskEntry,
  type TaskFilter,
  type TaskRunLogEntry,
} from "./task-editor";

type TaskTriggerInputLocalWire = {
  runAtLocal?: string;
  everyMinutes?: number;
  endAtLocal?: string;
};

type TaskCreateInputWire = {
  goal: string;
  why: string;
  todo: string;
  trigger: TaskTriggerInputLocalWire;
};

type TaskUpdateInputWire = {
  taskId: string;
  goal?: string;
  why?: string;
  todo?: string;
  trigger?: TaskTriggerInputLocalWire;
};

type TaskCompleteInputWire = {
  taskId: string;
  completionState: string;
  completionConclusion: string;
};

type TaskDeleteInputWire = {
  taskId: string;
};

const PAGE_SIZE = 5;

const { t } = useI18n();
const message = ref("");
const messageError = ref(false);
const listLoading = ref(false);
const logsLoading = ref(false);
const tasks = ref<TaskEntry[]>([]);
const runLogs = ref<TaskRunLogEntry[]>([]);
const selectedTaskId = ref("");
const filter = ref<TaskFilter>("active");
const page = ref(1);

const editorDialog = ref<HTMLDialogElement | null>(null);
const discardConfirmDialog = ref<HTMLDialogElement | null>(null);
const deleteConfirmDialog = ref<HTMLDialogElement | null>(null);
const editorOpen = ref(false);
const editorMode = ref<TaskEditorMode>("create");
const editorLoading = ref(false);
const editorSaving = ref(false);
const editorError = ref("");
const editorTask = ref<TaskEntry | null>(null);
const editorForm = ref<TaskEditorForm>(createEmptyTaskEditorForm());
const editorInitialSnapshot = ref(taskEditorSnapshot(editorForm.value));
let resolveDiscardConfirm: ((value: boolean) => void) | null = null;
let resolveDeleteConfirm: ((value: boolean) => void) | null = null;

const filteredTasks = computed(() => {
  if (!filter.value) return tasks.value;
  if (filter.value === "completed") return tasks.value.filter((item) => item.completionState !== "active");
  return tasks.value.filter((item) => item.completionState === "active");
});
const totalPages = computed(() => Math.max(1, Math.ceil(filteredTasks.value.length / PAGE_SIZE)));
const pagedTasks = computed(() => {
  const start = (page.value - 1) * PAGE_SIZE;
  return filteredTasks.value.slice(start, start + PAGE_SIZE);
});
const editorDirty = computed(() => taskEditorSnapshot(editorForm.value) !== editorInitialSnapshot.value);
const editorCanComplete = computed(
  () => editorMode.value === "edit" && !!editorTask.value && editorTask.value.completionState === "active",
);
const editorEditable = computed(
  () => editorMode.value === "create" || (!!editorTask.value && editorTask.value.completionState === "active"),
);

watch(filter, () => {
  page.value = 1;
});

watch(totalPages, (next) => {
  if (page.value > next) {
    page.value = next;
  }
});

function setMessage(text: string, isError = false) {
  message.value = text;
  messageError.value = isError;
}

function clearMessage() {
  message.value = "";
  messageError.value = false;
}

function describeError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error || "");
}

function formatTaskTime(value?: string | null): string {
  return formatIsoToLocalDateTime(value, "-");
}

function completionStateLabel(value: string): string {
  if (value === "completed") return t("config.task.completionStates.completed");
  if (value === "failed_completed") return t("config.task.completionStates.failedCompleted");
  if (value === "active") return t("config.task.filters.active");
  return value || "-";
}

function resetFilter() {
  filter.value = "";
}

function setFilter(value: Exclude<TaskFilter, "">) {
  filter.value = value;
}

function resetEditorForm(mode: TaskEditorMode, task: TaskEntry | null) {
  editorMode.value = mode;
  editorTask.value = task;
  editorForm.value = task ? taskEditorFormFromEntry(task) : createEmptyTaskEditorForm();
  editorInitialSnapshot.value = taskEditorSnapshot(editorForm.value);
}

async function ensureEditorDialogOpen() {
  await nextTick();
  if (editorDialog.value && !editorDialog.value.open) {
    editorDialog.value.showModal();
  }
  editorOpen.value = true;
}

function closeEditorDialogDirect() {
  if (editorDialog.value?.open) {
    editorDialog.value.close();
  }
  editorOpen.value = false;
  editorLoading.value = false;
  editorSaving.value = false;
  editorError.value = "";
}

async function confirmDiscardIfNeeded(): Promise<boolean> {
  if (!editorOpen.value || editorSaving.value || !editorDirty.value) {
    return true;
  }
  return requestDiscardConfirmDialog();
}

function requestDiscardConfirmDialog(): Promise<boolean> {
  const dialog = discardConfirmDialog.value;
  if (!dialog) return Promise.resolve(false);
  return new Promise<boolean>((resolve) => {
    resolveDiscardConfirm = resolve;
    dialog.showModal();
  });
}

function closeDiscardConfirmDialog(value: boolean) {
  const dialog = discardConfirmDialog.value;
  if (dialog?.open) {
    dialog.close();
  }
  resolveDiscardConfirm?.(value);
  resolveDiscardConfirm = null;
}

function buildTriggerInputFromForm(): TaskTriggerInputLocalWire | null {
  const everyMinutesText = String(editorForm.value.everyMinutesText || "").trim();
  let everyMinutes: number | undefined;
  if (everyMinutesText) {
    const parsed = Number(everyMinutesText);
    if (!Number.isFinite(parsed) || parsed <= 0) {
      editorError.value = t("config.task.validation.everyMinutesPositive");
      return null;
    }
    everyMinutes = parsed;
  }
  const runAtLocal = String(editorForm.value.runAtLocal || "").trim();
  const endAtLocal = String(editorForm.value.endAtLocal || "").trim();
  return {
    runAtLocal: runAtLocal || undefined,
    everyMinutes,
    endAtLocal: endAtLocal || undefined,
  };
}

function editorCreatePayload(): TaskCreateInputWire | null {
  if (!String(editorForm.value.goal || "").trim()) {
    editorError.value = t("config.task.validation.goalRequired");
    return null;
  }
  const trigger = buildTriggerInputFromForm();
  if (!trigger) return null;
  return {
    goal: editorForm.value.goal.trim(),
    why: editorForm.value.why.trim(),
    todo: editorForm.value.todo.trim(),
    trigger,
  };
}

function editorUpdatePayload(): TaskUpdateInputWire | null {
  if (!String(editorForm.value.taskId || "").trim()) {
    editorError.value = t("config.task.detailLoadFailed");
    return null;
  }
  if (!String(editorForm.value.goal || "").trim()) {
    editorError.value = t("config.task.validation.goalRequired");
    return null;
  }
  const trigger = buildTriggerInputFromForm();
  if (!trigger) return null;
  return {
    taskId: editorForm.value.taskId.trim(),
    goal: editorForm.value.goal.trim(),
    why: editorForm.value.why.trim(),
    todo: editorForm.value.todo.trim(),
    trigger,
  };
}

function editorCompletePayload(): TaskCompleteInputWire | null {
  if (!String(editorForm.value.taskId || "").trim()) {
    editorError.value = t("config.task.detailLoadFailed");
    return null;
  }
  return {
    taskId: editorForm.value.taskId.trim(),
    completionState: editorForm.value.completionState,
    completionConclusion: editorForm.value.completionConclusion.trim(),
  };
}

function editorDeletePayload(): TaskDeleteInputWire | null {
  if (!String(editorForm.value.taskId || "").trim()) {
    editorError.value = t("config.task.detailLoadFailed");
    return null;
  }
  return {
    taskId: editorForm.value.taskId.trim(),
  };
}

async function loadRunLogs(taskId = selectedTaskId.value, silent = false) {
  const normalizedTaskId = String(taskId || "").trim();
  if (!normalizedTaskId) {
    runLogs.value = [];
    return;
  }
  logsLoading.value = true;
  try {
    runLogs.value = await invokeTauri<TaskRunLogEntry[]>("task_list_run_logs", {
      input: { taskId: normalizedTaskId, limit: 50 },
    });
  } catch (error) {
    if (!silent) {
      setMessage(`${t("config.task.runLogsLoadFailed")}: ${describeError(error)}`, true);
    }
  } finally {
    logsLoading.value = false;
  }
}

async function loadTasks(options: { preferredTaskId?: string; keepMessage?: boolean } = {}) {
  listLoading.value = true;
  if (!options.keepMessage) {
    clearMessage();
  }
  try {
    const nextTasks = await invokeTauri<TaskEntry[]>("task_list_tasks");
    tasks.value = nextTasks;
    const preferredTaskId = String(options.preferredTaskId || selectedTaskId.value || "").trim();
    if (preferredTaskId && nextTasks.some((item) => item.taskId === preferredTaskId)) {
      selectedTaskId.value = preferredTaskId;
    } else if (nextTasks.length > 0) {
      selectedTaskId.value = nextTasks[0].taskId;
    } else {
      selectedTaskId.value = "";
    }
  } catch (error) {
    setMessage(`${t("config.task.listLoadFailed")}: ${describeError(error)}`, true);
  } finally {
    listLoading.value = false;
  }
}

async function loadEditorTask(taskId = selectedTaskId.value) {
  const normalizedTaskId = String(taskId || "").trim();
  if (!normalizedTaskId) return;
  editorLoading.value = true;
  editorError.value = "";
  try {
    const detail = await invokeTauri<TaskEntry>("task_get_task", { input: { taskId: normalizedTaskId } });
    tasks.value = taskUpsertEntry(tasks.value, detail);
    selectedTaskId.value = detail.taskId;
    resetEditorForm("edit", detail);
    await loadRunLogs(detail.taskId, true);
  } catch (error) {
    editorTask.value = null;
    editorError.value = `${t("config.task.detailLoadFailed")}: ${describeError(error)}`;
  } finally {
    editorLoading.value = false;
  }
}

async function openEditEditor(taskId: string) {
  if (!(await confirmDiscardIfNeeded())) return;
  selectedTaskId.value = taskId;
  editorError.value = "";
  editorTask.value = null;
  editorLoading.value = true;
  editorMode.value = "edit";
  editorForm.value = createEmptyTaskEditorForm();
  editorInitialSnapshot.value = taskEditorSnapshot(editorForm.value);
  await ensureEditorDialogOpen();
  await loadEditorTask(taskId);
}

async function requestCloseEditor() {
  if (!(await confirmDiscardIfNeeded())) return;
  closeEditorDialogDirect();
}

function onEditorDialogCancel(event: Event) {
  event.preventDefault();
  void requestCloseEditor();
}

async function saveEditor() {
  if (editorSaving.value) return;
  if (!editorEditable.value && editorMode.value === "edit") return;
  editorError.value = "";
  editorSaving.value = true;
  try {
    if (editorMode.value === "create") {
      const payload = editorCreatePayload();
      if (!payload) return;
      const created = await invokeTauri<TaskEntry>("task_create_task", { input: payload });
      tasks.value = taskUpsertEntry(tasks.value, created);
      selectedTaskId.value = created.taskId;
      closeEditorDialogDirect();
      setMessage(t("config.task.created"));
      await loadTasks({ preferredTaskId: created.taskId, keepMessage: true });
      return;
    }

    const payload = editorUpdatePayload();
    if (!payload) return;
    const updated = await invokeTauri<TaskEntry>("task_update_task", { input: payload });
    tasks.value = taskUpsertEntry(tasks.value, updated);
    selectedTaskId.value = updated.taskId;
    closeEditorDialogDirect();
    setMessage(t("config.task.updated"));
    await loadTasks({ preferredTaskId: updated.taskId, keepMessage: true });
  } catch (error) {
    editorError.value = `${editorMode.value === "create" ? t("config.task.createFailed") : t("config.task.updateFailed")}: ${describeError(error)}`;
  } finally {
    editorSaving.value = false;
  }
}

async function completeEditorTask() {
  if (editorSaving.value) return;
  editorError.value = "";
  editorSaving.value = true;
  try {
    const payload = editorCompletePayload();
    if (!payload) return;
    const completed = await invokeTauri<TaskEntry>("task_complete_task", { input: payload });
    tasks.value = taskUpsertEntry(tasks.value, completed);
    selectedTaskId.value = completed.taskId;
    closeEditorDialogDirect();
    setMessage(t("config.task.completed"));
    await loadTasks({ preferredTaskId: completed.taskId, keepMessage: true });
  } catch (error) {
    editorError.value = `${t("config.task.completeFailed")}: ${describeError(error)}`;
  } finally {
    editorSaving.value = false;
  }
}

async function deleteEditorTask() {
  if (editorSaving.value || editorMode.value !== "edit") return;
  const payload = editorDeletePayload();
  if (!payload) return;
  const confirmed = await requestDeleteConfirmDialog();
  if (!confirmed) {
    return;
  }

  editorError.value = "";
  editorSaving.value = true;
  try {
    await invokeTauri("task_delete_task", { input: payload });
    tasks.value = tasks.value.filter((item) => item.taskId !== payload.taskId);
    if (selectedTaskId.value === payload.taskId) {
      selectedTaskId.value = "";
    }
    closeEditorDialogDirect();
    setMessage(t("config.task.deleted"));
    await loadTasks({ keepMessage: true });
  } catch (error) {
    editorError.value = `${t("config.task.deleteFailed")}: ${describeError(error)}`;
  } finally {
    editorSaving.value = false;
  }
}

function requestDeleteConfirmDialog(): Promise<boolean> {
  const dialog = deleteConfirmDialog.value;
  if (!dialog) return Promise.resolve(false);
  return new Promise<boolean>((resolve) => {
    resolveDeleteConfirm = resolve;
    dialog.showModal();
  });
}

function closeDeleteConfirmDialog(value: boolean) {
  const dialog = deleteConfirmDialog.value;
  if (dialog?.open) {
    dialog.close();
  }
  resolveDeleteConfirm?.(value);
  resolveDeleteConfirm = null;
}

async function reloadEditorTask() {
  if (editorMode.value !== "edit") return;
  await loadEditorTask(editorForm.value.taskId || selectedTaskId.value);
}

async function refreshLogs() {
  await loadRunLogs();
}

onMounted(() => {
  void loadTasks();
});
</script>
