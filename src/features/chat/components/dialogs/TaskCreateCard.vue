<template>
  <dialog ref="dialogRef" class="modal" @close="onDialogClose" @cancel.prevent="onDialogClose">
    <div class="modal-box w-11/12 max-w-2xl p-0">
      <div class="flex items-center justify-between gap-3 border-b border-base-300/70 px-5 py-4">
        <h3 class="text-base font-semibold">{{ dialogTitle }}</h3>
        <button
          type="button"
          class="btn btn-ghost btn-sm btn-circle"
          :disabled="dialogBusy"
          :title="t('common.close')"
          @click="handleClose"
        >
          <X class="h-4 w-4" />
        </button>
      </div>

      <form @submit.prevent="handleSubmit">
        <div class="space-y-4 px-5 py-4">
          <div
            v-if="errorText"
            class="rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error whitespace-pre-wrap break-all"
          >
            {{ errorText }}
          </div>

          <label class="block space-y-2">
            <span class="flex items-center justify-between gap-2">
              <span class="block text-sm font-medium">{{ t("chat.taskCreate.contentLabel") }}</span>
              <button
                v-if="!isEditMode"
                type="button"
                class="btn btn-ghost btn-xs"
                :disabled="dialogBusy || !content.trim()"
                :title="t('chat.taskCreate.optimizeTitle')"
                @click="handleOptimizeDraft"
              >
                <span v-if="optimizing" class="loading loading-spinner loading-xs"></span>
                <WandSparkles v-else class="h-3.5 w-3.5" />
                {{ t("chat.taskCreate.optimizeAction") }}
              </button>
            </span>
            <textarea
              ref="contentInputRef"
              v-model="content"
              class="textarea textarea-bordered min-h-36 w-full resize-none"
              rows="6"
              :placeholder="t('chat.taskCreate.contentPlaceholder')"
              :disabled="dialogBusy"
            ></textarea>
          </label>

          <label class="block space-y-2">
            <span class="block text-sm font-medium">{{ t("chat.taskCreate.titleLabel") }}</span>
            <input
              v-model="title"
              class="input input-bordered w-full"
              type="text"
              :placeholder="t('chat.taskCreate.titlePlaceholder')"
              :disabled="dialogBusy"
            />
          </label>

          <label class="block space-y-2">
            <span class="block text-sm font-medium">{{ t("config.task.fields.scheduleMode") }}</span>
            <SegmentedControl
              v-model="scheduleMode"
              :options="scheduleModeOptions"
              :disabled="dialogBusy"
              size="sm"
            />
          </label>

          <label class="block space-y-2">
            <span class="block text-sm font-medium">{{ t("config.task.fields.runAt") }}</span>
            <TaskDateTimeInput v-model="runAt" :disabled="dialogBusy" />
          </label>

          <div v-if="scheduleMode === 'interval'" class="space-y-3">
            <label class="block space-y-2">
              <span class="block text-sm font-medium">{{ t("chat.taskCreate.scheduleDetailModeLabel") }}</span>
              <SegmentedControl
                v-model="scheduleDetailMode"
                :options="scheduleDetailModeOptions"
                :disabled="dialogBusy"
                size="sm"
              />
            </label>

            <div v-if="scheduleDetailMode === 'simple'" class="space-y-3">
              <label class="block space-y-2">
                <span class="block text-sm font-medium">{{ t("chat.taskCreate.intervalLabel") }}</span>
                <div class="join w-full">
                  <input
                    v-model="repeatEvery"
                    class="input input-bordered join-item min-w-0 flex-1"
                    type="number"
                    min="1"
                    :max="repeatUnit === 'months' ? 12 : undefined"
                    step="1"
                    :disabled="dialogBusy"
                  />
                  <select
                    v-model="repeatUnit"
                    class="select select-bordered join-item w-32 shrink-0"
                    :disabled="dialogBusy"
                  >
                    <option value="minutes">{{ t("chat.taskCreate.intervalUnits.minutes") }}</option>
                    <option value="hours">{{ t("chat.taskCreate.intervalUnits.hours") }}</option>
                    <option value="days">{{ t("chat.taskCreate.intervalUnits.days") }}</option>
                    <option value="weeks">{{ t("chat.taskCreate.intervalUnits.weeks") }}</option>
                    <option value="months">{{ t("chat.taskCreate.intervalUnits.months") }}</option>
                  </select>
                </div>
              </label>
            </div>

            <div v-else class="space-y-3 rounded-box border border-base-300/70 bg-base-200/30 p-3">
              <CronLight
                v-model="cronExpression"
                v-model:period="cronPeriod"
                class="cron-light-shell"
                theme="ant"
                :locale="locale"
                :custom-locale="cronCustomLocale"
                :periods="cronPeriods"
                :disabled="dialogBusy"
                @error="handleCronError"
              />

              <label class="block space-y-2">
                <span class="block text-sm font-medium">{{ t("chat.taskCreate.cronExpressionLabel") }}</span>
                <input
                  v-model="cronExpression"
                  class="input input-bordered w-full font-mono text-sm"
                  type="text"
                  placeholder="*/5 * * * *"
                  :disabled="dialogBusy"
                />
              </label>

              <div
                class="text-sm"
                :class="
                  simpleModeAllowed
                    ? 'opacity-70'
                    : 'inline-flex rounded-full border border-warning/30 bg-warning/10 px-3 py-1 text-warning'
                "
              >
                {{ simpleModeAllowed ? t("chat.taskCreate.cronAdvancedHint") : t("chat.taskCreate.cronAdvancedLockedHint") }}
              </div>
            </div>

            <label class="block space-y-2">
              <span class="block text-sm font-medium">{{ t("config.task.fields.endAt") }}</span>
              <TaskDateTimeInput v-model="endAt" :disabled="dialogBusy" />
            </label>
          </div>
        </div>

        <div class="border-t border-base-300/70 bg-base-100 px-5 py-4">
          <div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
            <button
              v-if="isEditMode"
              type="button"
              class="btn btn-error btn-outline"
              :disabled="dialogBusy"
              @click="requestDeleteConfirm"
            >
              <Trash2 class="h-4 w-4" />
              {{ t("common.delete") }}
            </button>
            <span v-else class="hidden sm:block"></span>
            <div class="flex items-center justify-end gap-2">
              <button type="button" class="btn btn-ghost" :disabled="dialogBusy || !canRestore" @click="handleRestore">
                {{ t("common.reset") }}
              </button>
              <button type="button" class="btn btn-ghost" :disabled="dialogBusy" @click="handleClose">
                {{ t("common.cancel") }}
              </button>
              <button type="submit" class="btn btn-primary" :disabled="dialogBusy">
                <span v-if="saving" class="loading loading-spinner loading-sm"></span>
                {{ submitButtonLabel }}
              </button>
            </div>
          </div>
        </div>
      </form>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDialogClose">close</button>
    </form>
  </dialog>

  <dialog ref="deleteConfirmDialogRef" class="modal" @close="onDeleteConfirmDialogClose" @cancel.prevent="onDeleteConfirmDialogClose">
    <div class="modal-box max-w-md p-4">
      <h3 class="text-sm font-semibold">{{ t("common.delete") }}</h3>
      <p class="mt-3 whitespace-pre-wrap text-sm">{{ t("config.task.deleteConfirm") }}</p>
      <div class="modal-action mt-4">
        <button class="btn btn-sm btn-ghost" type="button" :disabled="dialogBusy" @click="closeDeleteConfirm">
          {{ t("common.cancel") }}
        </button>
        <button class="btn btn-sm btn-error" type="button" :disabled="dialogBusy" @click="handleDeleteConfirmed">
          <span v-if="saving" class="loading loading-spinner loading-xs"></span>
          {{ t("common.confirm") }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDeleteConfirmDialogClose">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Trash2, WandSparkles, X } from "@lucide/vue";
import { CronLight } from "@vue-js-cron/light";
import "@vue-js-cron/light/dist/light.css";
import { invokeTauri } from "../../../../services/tauri-api";
import { formatDateToLocalRfc3339 } from "../../../../utils/time";
import { toErrorMessage } from "../../../../utils/error";
import SegmentedControl from "../../../config/components/SegmentedControl.vue";
import TaskDateTimeInput from "../../../config/views/config-tabs/TaskDateTimeInput.vue";
import type { TaskEntry, TaskScheduleMode } from "../../../config/views/config-tabs/task-editor";
import { inferCronPeriodFromExpression, inferFixedIntervalFromCronExpression, inferMonthlyIntervalFromCronExpression } from "../../utils/task-schedule-cron";

type RepeatIntervalUnit = "minutes" | "hours" | "days" | "weeks" | "months";
type ScheduleDetailMode = "simple" | "advanced";

type TaskTriggerInputWire = {
  run_at: string;
  everyMinutes?: number;
  cron_expression?: string;
  end_at?: string;
};

type TaskCreateInputWire = {
  conversationId: string;
  agentId?: string;
  targetScope: "desktop";
  goal: string;
  why: string;
  todo: string;
  trigger: TaskTriggerInputWire;
};

type TaskUpdateInputWire = {
  taskId: string;
  conversationId?: string;
  agentId?: string;
  goal: string;
  why: string;
  todo: string;
  trigger: TaskTriggerInputWire;
};

type TaskDeleteInputWire = {
  taskId: string;
};

type TaskOptimizeDraftInputWire = {
  conversationId?: string;
  title: string;
  content: string;
  scheduleMode: TaskScheduleMode;
  runAt: string;
  repeatEvery: string;
  repeatUnit: RepeatIntervalUnit;
  endAt: string;
};

type TaskOptimizeDraftOutputWire = {
  title: string;
  content: string;
  scheduleMode: TaskScheduleMode;
  runAt: string;
  repeatEvery: string;
  repeatUnit: RepeatIntervalUnit;
  endAt: string;
};

const DEFAULT_RUN_AT_DELAY_MINUTES = 10;
const DERIVED_TITLE_LIMIT = 80;

const props = defineProps<{
  open: boolean;
  mode?: "create" | "edit";
  conversationId?: string;
  task?: TaskEntry | null;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "created", task: TaskEntry): void;
  (e: "updated", task: TaskEntry): void;
}>();

const { t, locale } = useI18n();
const dialogRef = ref<HTMLDialogElement | null>(null);
const deleteConfirmDialogRef = ref<HTMLDialogElement | null>(null);
const contentInputRef = ref<HTMLTextAreaElement | null>(null);
const saving = ref(false);
const optimizing = ref(false);
const deleteConfirmOpen = ref(false);

function onDialogClose() {
  if (dialogBusy.value) {
    const d = dialogRef.value;
    if (d && !d.open && props.open) d.showModal();
    return;
  }
  handleClose();
}

function syncDialog() {
  const d = dialogRef.value;
  if (!d) return;
  if (props.open) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

function onDeleteConfirmDialogClose() {
  if (dialogBusy.value) {
    const d = deleteConfirmDialogRef.value;
    if (d && !d.open && deleteConfirmOpen.value) d.showModal();
    return;
  }
  closeDeleteConfirm();
}

function syncDeleteConfirmDialog() {
  const d = deleteConfirmDialogRef.value;
  if (!d) return;
  if (deleteConfirmOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(() => props.open, syncDialog);
watch(dialogRef, syncDialog);
watch(deleteConfirmOpen, syncDeleteConfirmDialog);
watch(deleteConfirmDialogRef, syncDeleteConfirmDialog);
const errorText = ref("");
const title = ref("");
const content = ref("");
const runAt = ref("");
const scheduleMode = ref<TaskScheduleMode>("once");
const scheduleDetailMode = ref<ScheduleDetailMode>("simple");
const repeatEvery = ref("1");
const repeatUnit = ref<RepeatIntervalUnit>("hours");
const endAt = ref("");
const cronExpression = ref("");
const cronPeriod = ref("");
const cronErrorText = ref("");
const preservedCronExpression = ref("");
const initialScheduleSnapshot = ref("");
const lastScheduleDetailMode = ref<ScheduleDetailMode>("simple");
const dialogMode = computed(() => props.mode === "edit" ? "edit" : "create");
const isEditMode = computed(() => dialogMode.value === "edit");
const dialogBusy = computed(() => saving.value || optimizing.value);
const existingRecurringTask = computed(() =>
  isEditMode.value
  && (
    !!String(props.task?.trigger?.cron_expression || "").trim()
    || (Number.isFinite(Number(props.task?.trigger?.every_minutes)) && Number(props.task?.trigger?.every_minutes) > 0)
  ),
);
const simpleModeAllowed = computed(() => {
  const existingEveryMinutes = Number(props.task?.trigger?.every_minutes);
  if (Number.isFinite(existingEveryMinutes) && existingEveryMinutes > 0) {
    return true;
  }
  const cron = String(cronExpression.value || preservedCronExpression.value || "").trim();
  if (!cron) return true;
  if (inferMonthlyIntervalFromCronExpression(cron)) return true;
  return !!inferFixedIntervalFromCronExpression(cron);
});
const scheduleModeOptions = computed(() => [
  { value: "once" as const, label: t("config.task.scheduleModes.once"), disabled: existingRecurringTask.value },
  { value: "interval" as const, label: t("config.task.scheduleModes.interval") },
]);
const scheduleDetailModeOptions = computed(() => [
  {
    value: "simple" as const,
    label: t("chat.taskCreate.scheduleDetailModes.simple"),
  },
  { value: "advanced" as const, label: t("chat.taskCreate.scheduleDetailModes.advanced") },
]);
const cronPeriods = computed(() => [
  { id: "minute", value: ["minute"], text: t("chat.taskCreate.cronPeriods.minute") },
  { id: "hour", value: ["minute", "hour"], text: t("chat.taskCreate.cronPeriods.hour") },
  { id: "day", value: ["hour", "minute"], text: t("chat.taskCreate.cronPeriods.day") },
  { id: "week", value: ["dayOfWeek", "hour", "minute"], text: t("chat.taskCreate.cronPeriods.week") },
  { id: "month", value: ["day", "dayOfWeek", "hour", "minute"], text: t("chat.taskCreate.cronPeriods.month") },
  { id: "year", value: ["month", "day", "dayOfWeek", "hour", "minute"], text: t("chat.taskCreate.cronPeriods.year") },
]);
const cronCustomLocale = computed(() => {
  const currentLocale = String(locale.value || "").trim();
  if (currentLocale === "en-US") {
    return undefined;
  }
  const isTraditionalChinese = currentLocale === "zh-TW";
  const periodPrefix = isTraditionalChinese ? "週期級別" : "周期级别";
  return {
    minute: { prefix: periodPrefix, text: isTraditionalChinese ? "分鐘" : "分钟" },
    hour: {
      prefix: periodPrefix,
      text: isTraditionalChinese ? "小時" : "小时",
      minute: { "*": { prefix: isTraditionalChinese ? "在" : "在", suffix: isTraditionalChinese ? "分" : "分" }, any: { text: isTraditionalChinese ? "每" : "每" } },
    },
    day: { prefix: periodPrefix, text: isTraditionalChinese ? "天" : "天" },
    week: { prefix: periodPrefix, text: isTraditionalChinese ? "週" : "周" },
    month: {
      prefix: periodPrefix,
      text: isTraditionalChinese ? "月" : "月",
      dayOfWeek: { "*": { prefix: isTraditionalChinese ? "且" : "且" } },
    },
    year: {
      prefix: periodPrefix,
      text: isTraditionalChinese ? "年" : "年",
      dayOfWeek: { "*": { prefix: isTraditionalChinese ? "且" : "且" } },
    },
    "q-second": { text: isTraditionalChinese ? "秒" : "秒" },
    "q-minute": {
      text: isTraditionalChinese ? "分鐘" : "分钟",
      second: { "*": { prefix: isTraditionalChinese ? "在" : "在", suffix: isTraditionalChinese ? "秒" : "秒" }, any: { text: isTraditionalChinese ? "每" : "每" } },
    },
    "q-hour": { text: isTraditionalChinese ? "小時" : "小时", minute: { "*": { prefix: isTraditionalChinese ? "在" : "在" } } },
    "*": {
      prefix: isTraditionalChinese ? "每" : "每",
      suffix: "",
      text: isTraditionalChinese ? "未知" : "未知",
      "*": {
        value: { text: "{{value.text}}" },
        range: { text: "{{start.text}}-{{end.text}}" },
        step: { text: isTraditionalChinese ? "每 {{step.value}}" : "每 {{step.value}}" },
        rangeStep: { text: "{{start.text}}-{{end.text}}/{{step.value}}" },
        stepFrom: { text: "{{start.text}}/{{step.value}}" },
      },
      month: {
        "*": { prefix: isTraditionalChinese ? "在" : "在" },
        any: { text: isTraditionalChinese ? "每月" : "每月" },
        value: { text: "{{value.alt}}" },
        range: { text: "{{start.alt}}-{{end.alt}}" },
        rangeStep: { text: "{{start.alt}}-{{end.alt}}/{{step.value}}" },
        stepFrom: { text: "{{start.alt}}/{{step.value}}" },
      },
      day: {
        "*": { prefix: isTraditionalChinese ? "在" : "在" },
        any: { text: isTraditionalChinese ? "每天" : "每天" },
        noSpecific: { text: isTraditionalChinese ? "不指定日期" : "不指定日期" },
      },
      dayOfWeek: {
        "*": { prefix: isTraditionalChinese ? "在" : "在" },
        any: { text: isTraditionalChinese ? "每週每天" : "每周每天" },
        value: { text: "{{value.alt}}" },
        range: { text: "{{start.alt}}-{{end.alt}}" },
        rangeStep: { text: "{{start.alt}}-{{end.alt}}/{{step.value}}" },
        stepFrom: { text: "{{start.alt}}/{{step.value}}" },
        noSpecific: { text: isTraditionalChinese ? "不指定星期" : "不指定星期" },
      },
      hour: {
        "*": { prefix: isTraditionalChinese ? "在" : "在" },
        any: { text: isTraditionalChinese ? "每小時" : "每小时" },
      },
      minute: {
        "*": { prefix: ":" },
        any: { text: isTraditionalChinese ? "每分鐘" : "每分钟" },
        step: { text: isTraditionalChinese ? "每 {{step.value}} 分鐘" : "每 {{step.value}} 分钟" },
      },
      second: {
        "*": { prefix: ":" },
        any: { text: isTraditionalChinese ? "每秒" : "每秒" },
      },
    },
  };
});
const dialogTitle = computed(() => isEditMode.value ? t("config.task.editorEditTitle") : t("chat.taskCreate.title"));
const submitButtonLabel = computed(() => {
  if (saving.value) return t("config.task.saving");
  return isEditMode.value ? t("config.task.saveUpdate") : t("config.task.createAction");
});
const initialFormSnapshot = ref("");
const canRestore = computed(() => initialFormSnapshot.value !== formInputSnapshot());

function defaultRunAt(): string {
  const next = new Date(Date.now() + DEFAULT_RUN_AT_DELAY_MINUTES * 60_000);
  next.setSeconds(0, 0);
  return formatDateToLocalRfc3339(next);
}

function resetForm() {
  title.value = "";
  content.value = "";
  runAt.value = defaultRunAt();
  scheduleMode.value = "once";
  scheduleDetailMode.value = "simple";
  lastScheduleDetailMode.value = "simple";
  repeatEvery.value = "1";
  repeatUnit.value = "hours";
  endAt.value = "";
  cronExpression.value = "";
  cronPeriod.value = "minute";
  cronErrorText.value = "";
  preservedCronExpression.value = "";
  initialScheduleSnapshot.value = scheduleInputSnapshot();
  initialFormSnapshot.value = formInputSnapshot();
  errorText.value = "";
}

function resetFormFromTask(task: TaskEntry) {
  title.value = String(task.goal || "").trim();
  content.value = String(task.todo || "").trim();
  runAt.value = String(task.trigger?.run_at || task.trigger?.next_run_at || "").trim() || defaultRunAt();
  endAt.value = String(task.trigger?.end_at || "").trim();
  preservedCronExpression.value = String(task.trigger?.cron_expression || "").trim();
  cronExpression.value = preservedCronExpression.value;
  cronPeriod.value = preservedCronExpression.value
    ? inferCronPeriodFromExpression(preservedCronExpression.value)
    : "minute";
  cronErrorText.value = "";
  const everyMinutes = Number(task.trigger?.every_minutes);
  if (Number.isFinite(everyMinutes) && everyMinutes > 0) {
    scheduleMode.value = "interval";
    scheduleDetailMode.value = "simple";
    lastScheduleDetailMode.value = "simple";
    setRepeatFromEveryMinutes(everyMinutes);
  } else if (preservedCronExpression.value) {
    scheduleMode.value = "interval";
    scheduleDetailMode.value = "advanced";
    lastScheduleDetailMode.value = "advanced";
    const monthlyInterval = inferMonthlyIntervalFromCronExpression(preservedCronExpression.value);
    if (monthlyInterval) {
      repeatEvery.value = String(monthlyInterval);
      repeatUnit.value = "months";
    } else {
      const fixedInterval = inferFixedIntervalFromCronExpression(preservedCronExpression.value);
      if (fixedInterval) {
        repeatEvery.value = String(fixedInterval.repeatEvery);
        repeatUnit.value = fixedInterval.repeatUnit;
      } else {
        repeatEvery.value = "1";
        repeatUnit.value = "hours";
      }
    }
  } else {
    scheduleMode.value = "once";
    scheduleDetailMode.value = "simple";
    lastScheduleDetailMode.value = "simple";
    repeatEvery.value = "1";
    repeatUnit.value = "hours";
  }
  initialScheduleSnapshot.value = scheduleInputSnapshot();
  initialFormSnapshot.value = formInputSnapshot();
  errorText.value = "";
}

function deriveTitleFromContent(value: string): string {
  const firstLine = value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => !!line);
  const compact = String(firstLine || value).replace(/\s+/g, " ").trim();
  if (compact.length <= DERIVED_TITLE_LIMIT) return compact;
  return `${compact.slice(0, DERIVED_TITLE_LIMIT)}...`;
}

function buildMonthlyCronExpression(runAtDate: Date, repeatEveryValue: number): string {
  const minute = runAtDate.getMinutes();
  const hour = runAtDate.getHours();
  const day = runAtDate.getDate();
  if (repeatEveryValue === 1) {
    return `${minute} ${hour} ${day} * *`;
  }
  const startMonth = runAtDate.getMonth() + 1;
  const months: number[] = [];
  let current = startMonth;
  for (let index = 0; index < 12; index += 1) {
    if (months.includes(current)) break;
    months.push(current);
    current += repeatEveryValue;
    while (current > 12) {
      current -= 12;
    }
  }
  return `${minute} ${hour} ${day} ${months.sort((left, right) => left - right).join(",")} *`;
}

function buildSimpleCronExpression(runAtDate: Date, repeatEveryValue: number, unit: RepeatIntervalUnit): string | null {
  const minute = runAtDate.getMinutes();
  const hour = runAtDate.getHours();
  if (unit === "months") {
    return buildMonthlyCronExpression(runAtDate, repeatEveryValue);
  }
  if (unit === "minutes") {
    if (repeatEveryValue === 1) return "* * * * *";
    return `*/${repeatEveryValue} * * * *`;
  }
  if (unit === "hours") {
    if (repeatEveryValue === 1) return `${minute} * * * *`;
    return `${minute} */${repeatEveryValue} * * *`;
  }
  if (unit === "days") {
    if (repeatEveryValue === 1) return `${minute} ${hour} * * *`;
    return null;
  }
  return null;
}

function setRepeatFromEveryMinutes(value: number) {
  const minuteValue = Math.max(1, Math.floor(value));
  const unitToMinutes: Record<RepeatIntervalUnit, number> = {
    minutes: 1,
    hours: 60,
    days: 24 * 60,
    weeks: 7 * 24 * 60,
    months: 30 * 24 * 60,
  };
  const preferredUnits: RepeatIntervalUnit[] = ["weeks", "days", "hours", "minutes"];
  const matchedUnit = preferredUnits.find((unit) => minuteValue % unitToMinutes[unit] === 0) || "minutes";
  repeatUnit.value = matchedUnit;
  repeatEvery.value = String(Math.max(1, Math.floor(minuteValue / unitToMinutes[matchedUnit])));
}

function normalizeOptimizedScheduleMode(value: string): TaskScheduleMode | null {
  return value === "once" || value === "interval" ? value : null;
}

function normalizeOptimizedRepeatUnit(value: string): RepeatIntervalUnit | null {
  const normalized = String(value || "").trim();
  if (
    normalized === "minutes"
    || normalized === "hours"
    || normalized === "days"
    || normalized === "weeks"
    || normalized === "months"
  ) {
    return normalized;
  }
  return null;
}

function normalizeOptimizedRepeatEvery(value: string, unit: RepeatIntervalUnit): string | null {
  const parsed = Number.parseInt(String(value || "").trim(), 10);
  if (!Number.isInteger(parsed) || parsed <= 0) return null;
  if (unit === "months" && ![1, 2, 3, 4, 6, 12].includes(parsed)) return null;
  return String(parsed);
}

function normalizeOptimizedDateTime(value: string): string | null {
  const normalized = String(value || "").trim();
  if (!normalized) return null;
  const parsed = new Date(normalized);
  return Number.isFinite(parsed.getTime()) ? normalized : null;
}

function scheduleInputSnapshot(): string {
  return JSON.stringify({
    runAt: String(runAt.value || "").trim(),
    scheduleMode: scheduleMode.value,
    scheduleDetailMode: scheduleDetailMode.value,
    repeatEvery: String(repeatEvery.value || "").trim(),
    repeatUnit: repeatUnit.value,
    cronExpression: String(cronExpression.value || "").trim(),
    endAt: String(endAt.value || "").trim(),
  });
}

function formInputSnapshot(): string {
  return JSON.stringify({
    title: String(title.value || "").trim(),
    content: String(content.value || "").trim(),
    schedule: scheduleInputSnapshot(),
  });
}

function handleCronError(message: string) {
  cronErrorText.value = String(message || "").trim();
}

function syncCronPeriodFromExpression() {
  const normalizedCron = String(cronExpression.value || "").trim();
  cronPeriod.value = normalizedCron ? inferCronPeriodFromExpression(normalizedCron) : "minute";
}

function buildPayload(): TaskCreateInputWire | TaskUpdateInputWire | null {
  const normalizedContent = content.value.trim();
  if (!normalizedContent) {
    errorText.value = t("chat.taskCreate.validation.contentRequired");
    void nextTick(() => contentInputRef.value?.focus());
    return null;
  }

  const normalizedRunAt = runAt.value.trim();
  if (!normalizedRunAt) {
    errorText.value = t("chat.taskCreate.validation.runAtRequired");
    return null;
  }
  const runAtDate = new Date(normalizedRunAt);
  if (!Number.isFinite(runAtDate.getTime())) {
    errorText.value = t("chat.taskCreate.validation.runAtInvalid");
    return null;
  }
  if (!isEditMode.value && runAtDate.getTime() < Date.now() - 60_000) {
    errorText.value = t("chat.taskCreate.validation.runAtPast");
    return null;
  }

  const trigger: TaskTriggerInputWire = {
    run_at: normalizedRunAt,
  };
  if (existingRecurringTask.value && scheduleMode.value !== "interval") {
    errorText.value = t("config.task.validation.recurringToOnceNotAllowed");
    return null;
  }
  if (scheduleMode.value === "interval") {
    const existingEveryMinutes = Number(props.task?.trigger?.every_minutes);
    if (
      isEditMode.value
      && preservedCronExpression.value
      && !Number.isFinite(existingEveryMinutes)
      && initialScheduleSnapshot.value === scheduleInputSnapshot()
    ) {
      trigger.cron_expression = preservedCronExpression.value;
    } else if (scheduleDetailMode.value === "advanced") {
      const normalizedCronExpression = String(cronExpression.value || "").trim();
      if (!normalizedCronExpression) {
        errorText.value = t("chat.taskCreate.validation.cronRequired");
        return null;
      }
      if (cronErrorText.value) {
        errorText.value = `${t("chat.taskCreate.validation.cronInvalid")} ${cronErrorText.value}`;
        return null;
      }
      trigger.cron_expression = normalizedCronExpression;
    } else {
      const repeatEveryValue = Number(String(repeatEvery.value || "1").trim() || "1");
      if (!Number.isInteger(repeatEveryValue) || repeatEveryValue <= 0) {
        errorText.value = t("chat.taskCreate.validation.intervalInvalid");
        return null;
      }
      if (repeatUnit.value === "months") {
        if (repeatEveryValue > 12 || (repeatEveryValue > 1 && 12 % repeatEveryValue !== 0)) {
          errorText.value = t("chat.taskCreate.validation.monthIntervalInvalid");
          return null;
        }
      }
      const simpleCronExpression = buildSimpleCronExpression(runAtDate, repeatEveryValue, repeatUnit.value);
      if (simpleCronExpression) {
        trigger.cron_expression = simpleCronExpression;
      } else {
        const unitToMinutes: Record<"minutes" | "hours" | "days" | "weeks", number> = {
          minutes: 1,
          hours: 60,
          days: 24 * 60,
          weeks: 7 * 24 * 60,
        };
        const repeatUnitValue = repeatUnit.value;
        if (repeatUnitValue === "months") {
          errorText.value = t("chat.taskCreate.validation.monthIntervalInvalid");
          return null;
        }
        trigger.everyMinutes = repeatEveryValue * unitToMinutes[repeatUnitValue];
      }
    }

    const normalizedEndAt = endAt.value.trim();
    if (normalizedEndAt) {
      const endAtDate = new Date(normalizedEndAt);
      if (!Number.isFinite(endAtDate.getTime())) {
        errorText.value = t("chat.taskCreate.validation.endAtInvalid");
        return null;
      }
      if (endAtDate.getTime() <= runAtDate.getTime()) {
        errorText.value = t("chat.taskCreate.validation.endAtNotAfterRunAt");
        return null;
      }
      trigger.end_at = normalizedEndAt;
    }
  }

  const basePayload: TaskCreateInputWire = {
    conversationId: String(props.conversationId || props.task?.conversationId || "").trim(),
    targetScope: "desktop",
    goal: title.value.trim() || deriveTitleFromContent(normalizedContent),
    why: "",
    todo: normalizedContent,
    trigger,
  };
  if (!basePayload.conversationId) {
    errorText.value = t("chat.taskCreate.validation.noConversation");
    return null;
  }
  if (!isEditMode.value) return basePayload;
  const taskId = String(props.task?.taskId || "").trim();
  if (!taskId) {
    errorText.value = t("config.task.detailLoadFailed");
    return null;
  }
  const conversationId = String(props.task?.conversationId || "").trim();
  return {
    taskId,
    ...(conversationId ? { conversationId } : {}),
    goal: basePayload.goal,
    why: basePayload.why,
    todo: basePayload.todo,
    trigger,
  };
}

async function requestTaskCreate(payload: TaskCreateInputWire): Promise<TaskEntry> {
  return invokeTauri<TaskEntry>("task.create", payload);
}

async function requestTaskUpdate(payload: TaskUpdateInputWire): Promise<TaskEntry> {
  return invokeTauri<TaskEntry>("task.update", payload);
}

async function requestTaskDelete(payload: TaskDeleteInputWire): Promise<void> {
  await invokeTauri("task.delete", payload);
}

async function requestTaskOptimize(payload: TaskOptimizeDraftInputWire): Promise<TaskOptimizeDraftOutputWire> {
  return invokeTauri<TaskOptimizeDraftOutputWire>("task.optimizeDraft", payload, 60_000);
}

async function handleSubmit() {
  if (dialogBusy.value) return;
  const payload = buildPayload();
  if (!payload) return;

  saving.value = true;
  errorText.value = "";
  try {
    if (isEditMode.value) {
      const updated = await requestTaskUpdate(payload as TaskUpdateInputWire);
      emit("updated", updated);
      emit("close");
      return;
    }
    const created = await requestTaskCreate(payload as TaskCreateInputWire);
    emit("created", created);
    emit("close");
  } catch (error) {
    errorText.value = `${isEditMode.value ? t("config.task.updateFailed") : t("config.task.createFailed")}: ${toErrorMessage(error)}`;
  } finally {
    saving.value = false;
  }
}

async function handleOptimizeDraft() {
  if (dialogBusy.value || isEditMode.value) return;
  const normalizedContent = content.value.trim();
  if (!normalizedContent) {
    errorText.value = t("chat.taskCreate.validation.contentRequired");
    void nextTick(() => contentInputRef.value?.focus());
    return;
  }
  const payload: TaskOptimizeDraftInputWire = {
    conversationId: String(props.conversationId || "").trim(),
    title: title.value.trim(),
    content: normalizedContent,
    scheduleMode: scheduleMode.value,
    runAt: runAt.value.trim(),
    repeatEvery: repeatEvery.value.trim(),
    repeatUnit: repeatUnit.value,
    endAt: endAt.value.trim(),
  };
  optimizing.value = true;
  errorText.value = "";
  try {
    const optimized = await requestTaskOptimize(payload);
    const nextContent = String(optimized.content || "").trim();
    const nextTitle = String(optimized.title || "").trim();
    if (nextContent) content.value = nextContent;
    if (nextTitle) title.value = nextTitle;
    const nextScheduleMode = normalizeOptimizedScheduleMode(String(optimized.scheduleMode || ""));
    if (nextScheduleMode) scheduleMode.value = nextScheduleMode;
    const nextRunAt = normalizeOptimizedDateTime(String(optimized.runAt || ""));
    if (nextRunAt) runAt.value = nextRunAt;
    const nextRepeatUnit = normalizeOptimizedRepeatUnit(String(optimized.repeatUnit || ""));
    if (nextRepeatUnit) repeatUnit.value = nextRepeatUnit;
    const nextRepeatEvery = normalizeOptimizedRepeatEvery(String(optimized.repeatEvery || ""), repeatUnit.value);
    if (nextRepeatEvery) repeatEvery.value = nextRepeatEvery;
    const nextEndAt = normalizeOptimizedDateTime(String(optimized.endAt || ""));
    endAt.value = scheduleMode.value === "interval" && nextEndAt ? nextEndAt : "";
  } catch (error) {
    errorText.value = `${t("chat.taskCreate.optimizeFailed")}: ${toErrorMessage(error)}`;
  } finally {
    optimizing.value = false;
  }
}

function requestDeleteConfirm() {
  if (dialogBusy.value || !isEditMode.value) return;
  deleteConfirmOpen.value = true;
}

function closeDeleteConfirm() {
  if (dialogBusy.value) return;
  deleteConfirmOpen.value = false;
}

async function handleDeleteConfirmed() {
  if (dialogBusy.value || !isEditMode.value) return;
  const taskId = String(props.task?.taskId || "").trim();
  if (!taskId) {
    deleteConfirmOpen.value = false;
    errorText.value = t("config.task.detailLoadFailed");
    return;
  }
  const payload: TaskDeleteInputWire = { taskId };
  saving.value = true;
  errorText.value = "";
  try {
    await requestTaskDelete(payload);
    deleteConfirmOpen.value = false;
    emit("close");
  } catch (error) {
    deleteConfirmOpen.value = false;
    errorText.value = `${t("config.task.deleteFailed")}: ${toErrorMessage(error)}`;
  } finally {
    saving.value = false;
  }
}

function handleClose() {
  if (dialogBusy.value) return;
  deleteConfirmOpen.value = false;
  emit("close");
}

function handleRestore() {
  if (dialogBusy.value || !canRestore.value) return;
  errorText.value = "";
  if (isEditMode.value && props.task) {
    resetFormFromTask(props.task);
    return;
  }
  resetForm();
}

watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    if (isEditMode.value && props.task) {
      resetFormFromTask(props.task);
    } else {
      resetForm();
    }
    await nextTick();
    contentInputRef.value?.focus();
  },
  { immediate: true },
);

watch(
  () => props.task,
  (task) => {
    if (!props.open || !isEditMode.value || !task) return;
    resetFormFromTask(task);
  },
);

watch(
  [scheduleMode, scheduleDetailMode, runAt, repeatEvery, repeatUnit],
  () => {
    if (scheduleMode.value !== "interval" || scheduleDetailMode.value !== "advanced") return;
    if (String(cronExpression.value || "").trim()) return;
    const runAtDate = new Date(String(runAt.value || "").trim());
    const repeatEveryValue = Number(String(repeatEvery.value || "1").trim() || "1");
    if (!Number.isFinite(runAtDate.getTime()) || !Number.isInteger(repeatEveryValue) || repeatEveryValue <= 0) {
      return;
    }
    const nextCronExpression = buildSimpleCronExpression(runAtDate, repeatEveryValue, repeatUnit.value);
    if (nextCronExpression) {
      cronExpression.value = nextCronExpression;
    }
  },
  { immediate: true },
);

watch(
  scheduleDetailMode,
  (nextMode, prevMode) => {
    if (nextMode === prevMode) return;
    if (nextMode !== "simple" || simpleModeAllowed.value) {
      if (nextMode === "advanced") {
        syncCronPeriodFromExpression();
      }
      lastScheduleDetailMode.value = nextMode;
      return;
    }
    const confirmed = typeof window !== "undefined"
      ? window.confirm(t("chat.taskCreate.confirmSwitchToSimple"))
      : false;
    if (!confirmed) {
      scheduleDetailMode.value = (prevMode || lastScheduleDetailMode.value || "advanced");
      return;
    }
    cronExpression.value = "";
    cronErrorText.value = "";
    preservedCronExpression.value = "";
    lastScheduleDetailMode.value = "simple";
  },
);

watch(
  () => props.open,
  (open) => {
    if (!open || scheduleDetailMode.value !== "advanced") return;
    syncCronPeriodFromExpression();
  },
);
</script>

<style scoped>
.cron-light-shell:deep(.cl-btn) {
  min-height: 2.4rem;
  border-radius: 0.75rem;
  border: 1px solid color-mix(in srgb, var(--color-base-content) 14%, transparent);
  background: var(--color-base-100);
  padding: 0.35rem 0.75rem;
}

.cron-light-shell:deep(.cl-menu) {
  border-radius: 0.9rem;
  border: 1px solid color-mix(in srgb, var(--color-base-content) 12%, transparent);
  background: var(--color-base-100);
}

.cron-light-shell:deep(.cl-col.selected),
.cron-light-shell:deep(.cl-col.selected:hover) {
  background: color-mix(in srgb, var(--color-primary) 16%, white);
  color: var(--color-base-content);
}
</style>
