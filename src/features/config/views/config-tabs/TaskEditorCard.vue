<template>
  <div class="modal-box w-[90vw] max-w-none h-[90vh] flex min-h-0 flex-col overflow-hidden p-0">
    <div v-if="errorText" class="mx-5 mt-5 rounded-box border px-3 py-2 text-sm" :class="showLoadError ? 'border-error/40 bg-error/10 text-error' : 'border-warning/40 bg-warning/10 text-warning-content'">
      {{ errorText }}
    </div>

    <div v-if="!loading && !editable && mode === 'edit' && task" class="mx-5 mt-5 rounded-box border border-base-300/70 bg-base-200/60 px-3 py-2 text-sm opacity-80">
      {{ t("config.task.completedReadonlyHint") }}
    </div>

    <div v-if="loading" class="flex flex-1 items-center justify-center text-sm opacity-70">
      {{ t("common.loading") }}
    </div>

    <div v-else-if="showLoadError" class="flex flex-1 items-center justify-center px-6">
      <div class="w-full max-w-md rounded-box border border-base-300 bg-base-100 p-5 text-center shadow-sm">
        <div class="text-base font-medium">{{ t("config.task.detailLoadFailed") }}</div>
        <div class="mt-2 text-sm opacity-70">{{ errorText }}</div>
        <div class="mt-4 flex justify-center gap-2">
          <button class="btn btn-sm" :disabled="saving" @click="$emit('reload')">{{ t("config.task.retry") }}</button>
          <button class="btn btn-sm btn-ghost" :disabled="saving" @click="$emit('close')">{{ t("common.close") }}</button>
        </div>
      </div>
    </div>

    <div v-else class="min-h-0 flex-1 overflow-y-auto pt-5">
      <div class="space-y-4 p-2">
        <details class="collapse collapse-arrow border border-base-300 bg-base-100" :name="accordionName" open>
          <summary class="collapse-title flex items-center justify-between gap-3 pr-10 text-base font-semibold">
            <span>{{ mode === "create" ? t("config.task.editorCreateTitle") : (form.goal || t("config.task.editorEditTitle")) }}</span>
            <span v-if="task" class="text-sm font-normal">#{{ task.orderIndex }}</span>
          </summary>
          <div class="collapse-content pt-1">
            <div class="space-y-5">
              <label class="block space-y-2">
                <span class="block text-sm font-medium">{{ t("config.task.fields.goal") }}</span>
                <input v-model="form.goal" class="input input-bordered w-full" type="text" :disabled="!editable || saving" />
              </label>

              <label class="block space-y-2">
                <span class="block text-sm font-medium">{{ t("config.task.fields.why") }}</span>
                <input v-model="form.why" class="input input-bordered w-full" type="text" :disabled="!editable || saving" />
              </label>

              <label class="block space-y-2">
                <span class="block text-sm font-medium">{{ t("config.task.fields.todo") }}</span>
                <input v-model="form.todo" class="input input-bordered w-full" type="text" :placeholder="t('config.task.todoPlaceholder')" :disabled="!editable || saving" />
              </label>

              <div class="divider my-1"></div>

              <div class="text-sm">
                {{ t("config.task.scheduleTitle") }}
              </div>

              <label class="block space-y-2">
                <span class="block text-sm font-medium">{{ t("config.task.fields.runAt") }}</span>
                <TaskDateTimeInput
                  v-model="form.runAtLocal"
                  :disabled="!editable || saving"
                />
              </label>

              <label class="block space-y-2">
                <span class="block text-sm font-medium">{{ t("config.task.fields.everyMinutes") }}</span>
                <input v-model="form.everyMinutesText" class="input input-bordered w-full" type="number" min="0.1" step="0.1" :disabled="!editable || saving" />
              </label>

              <label class="block space-y-2">
                <span class="block text-sm font-medium">{{ t("config.task.fields.endAt") }}</span>
                <TaskDateTimeInput
                  v-model="form.endAtLocal"
                  :disabled="!editable || saving"
                />
              </label>

              <template v-if="mode === 'edit' && canComplete">
                <div class="divider my-1"></div>

                <div class="text-sm">
                  {{ t("config.task.completeTitle") }}
                </div>

                <label class="block space-y-2">
                  <span class="block text-sm font-medium">{{ t("config.task.fields.completionState") }}</span>
                  <select v-model="form.completionState" class="select select-bordered w-full" :disabled="!editable || saving">
                    <option value="completed">{{ t("config.task.completionStates.completed") }}</option>
                    <option value="failed_completed">{{ t("config.task.completionStates.failedCompleted") }}</option>
                  </select>
                </label>

                <label class="block space-y-2">
                  <span class="block text-sm font-medium">{{ t("config.task.fields.completionConclusion") }}</span>
                  <input v-model="form.completionConclusion" class="input input-bordered w-full" type="text" :disabled="!editable || saving" />
                </label>
              </template>

              <div class="divider my-1"></div>

              <div class="text-sm">
                {{ t("config.task.metaTitle") }}
              </div>

              <div class="space-y-2 text-sm">
                <div v-if="task"><span class="font-medium">ID:</span> <span class="font-mono text-xs">{{ task.taskId }}</span></div>
                <div v-if="task"><span class="font-medium">#</span>{{ task.orderIndex }}</div>
                <div v-if="task?.completionState"><span class="font-medium">{{ t("config.task.fields.completionState") }}:</span> {{ completionStateLabel(task.completionState) }}</div>
                <div v-if="task"><span class="font-medium">{{ t("config.task.fields.updatedAt") }}:</span> {{ formatTaskTime(task.updatedAtLocal) }}</div>
                <div v-if="task?.createdAtLocal"><span class="font-medium">{{ t("config.task.createdAt") }}:</span> {{ formatTaskTime(task.createdAtLocal) }}</div>
                <div v-if="task?.trigger.nextRunAtLocal"><span class="font-medium">{{ t("config.task.fields.nextRunAt") }}:</span> {{ formatTaskTime(task.trigger.nextRunAtLocal) }}</div>
                <div v-if="task?.lastTriggeredAtLocal"><span class="font-medium">{{ t("config.task.lastTriggeredAt") }}:</span> {{ formatTaskTime(task.lastTriggeredAtLocal) }}</div>
                <div v-if="task?.completedAtLocal"><span class="font-medium">{{ t("config.task.completedAt") }}:</span> {{ formatTaskTime(task.completedAtLocal) }}</div>
              </div>
            </div>
          </div>
        </details>

        <details class="collapse collapse-arrow border border-base-300 bg-base-100" :name="accordionName">
          <summary class="collapse-title text-base font-semibold">
            {{ t("config.task.runLogs") }}
          </summary>
          <div class="collapse-content pt-1">
            <div v-if="mode === 'edit'" class="mb-3 flex justify-end">
              <button class="btn btn-xs btn-ghost" :disabled="loading || saving" @click="$emit('refreshLogs')">
                {{ t("config.task.refresh") }}
              </button>
            </div>
            <div v-if="logs.length" class="divide-y divide-base-300/60">
              <div v-for="log in logs" :key="log.id" class="py-3 text-sm first:pt-0 last:pb-0">
                <div class="flex items-center justify-between gap-2">
                  <span class="badge badge-sm" :class="runLogBadgeClass(log.outcome)">{{ runLogLabel(log.outcome) }}</span>
                  <span class="text-[11px]">{{ formatTaskTime(log.triggeredAtLocal) }}</span>
                </div>
                <div v-if="log.note" class="mt-1 whitespace-pre-wrap wrap-break-word">{{ log.note }}</div>
              </div>
            </div>
            <div v-else class="text-sm">
              {{ t("config.task.noLogs") }}
            </div>
          </div>
        </details>
      </div>
    </div>

    <div class="border-t border-base-300/70 bg-base-100 px-5 py-4 shrink-0">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div>
          <button v-if="mode === 'edit'" class="btn btn-ghost" :disabled="saving || loading" @click="$emit('delete')">
            {{ t("common.delete") }}
          </button>
        </div>
        <div class="flex flex-wrap items-center justify-end gap-2">
          <button class="btn btn-ghost" :disabled="saving" @click="$emit('close')">{{ t("common.close") }}</button>
          <button
            v-if="mode === 'edit' && canComplete"
            class="btn btn-warning"
            :disabled="saving || loading"
            @click="$emit('complete')"
          >
            {{ t("config.task.completeAction") }}
          </button>
          <button class="btn btn-primary" :disabled="saving || loading || !editable" @click="$emit('save')">
            {{ saving ? t("config.task.saving") : (mode === "create" ? t("config.task.createAction") : t("config.task.saveUpdate")) }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { TaskEditorForm, TaskEditorMode, TaskEntry, TaskRunLogEntry } from "./task-editor";
import { formatIsoToLocalDateTime } from "../../../../utils/time";
import TaskDateTimeInput from "./TaskDateTimeInput.vue";

const props = defineProps<{
  mode: TaskEditorMode;
  loading: boolean;
  saving: boolean;
  errorText: string;
  form: TaskEditorForm;
  task: TaskEntry | null;
  logs: TaskRunLogEntry[];
  canComplete: boolean;
  editable: boolean;
}>();

defineEmits<{
  close: [];
  save: [];
  complete: [];
  delete: [];
  reload: [];
  refreshLogs: [];
}>();

const { t } = useI18n();
const accordionName = "task-editor-accordion";

const showLoadError = computed(() => props.mode === "edit" && !props.loading && !!props.errorText && !props.task);

function formatTaskTime(value?: string | null): string {
  return formatIsoToLocalDateTime(value, "-");
}

function runLogLabel(outcome: string): string {
  if (outcome === "sent") return t("config.task.runLogOutcomes.sent");
  if (outcome === "queued") return t("config.task.runLogOutcomes.queued");
  if (outcome === "dequeued") return t("config.task.runLogOutcomes.dequeued");
  if (outcome === "failed") return t("config.task.runLogOutcomes.failed");
  return outcome || "-";
}

function runLogBadgeClass(outcome: string): string {
  if (outcome === "sent") return "badge-success";
  if (outcome === "queued") return "badge-warning";
  if (outcome === "dequeued") return "badge-info";
  if (outcome === "failed") return "badge-error";
  return "badge-ghost";
}

function completionStateLabel(value: string): string {
  if (value === "completed") return t("config.task.completionStates.completed");
  if (value === "failed_completed") return t("config.task.completionStates.failedCompleted");
  if (value === "active") return t("config.task.filters.active");
  return value || "-";
}
</script>
