<template>
  <div class="space-y-3">
    <!-- 当前追踪任务 -->
    <div v-if="trackedTask" class="card bg-base-100 border border-primary/30">
      <div class="card-body p-3 space-y-2">
        <div class="flex items-center justify-between">
          <span class="text-sm font-medium text-primary">{{ t("config.task.currentTracked") }}</span>
        </div>
        <div class="text-lg font-semibold wrap-break-word">{{ trackedTask.title }}</div>
        <div v-if="trackedTask.statusSummary" class="text-sm opacity-70 whitespace-pre-wrap wrap-break-word">
          {{ trackedTask.statusSummary }}
        </div>
      </div>
    </div>

    <!-- 任务列表 -->
    <div class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
      <div class="flex items-center gap-2 px-3 py-2 border-b border-base-300/70">
        <div class="font-medium">{{ t("config.task.title") }}<span v-if="filteredTasks.length">（{{ filteredTasks.length }}）</span></div>
        <div class="ml-auto flex items-center gap-2">
          <div class="join">
            <button class="btn btn-xs join-item" :class="filter === 'active' ? 'btn-primary' : 'btn-ghost'" @click="filter = 'active'">
              {{ t("config.task.filters.active") }}
            </button>
            <button class="btn btn-xs join-item" :class="filter === 'tracked' ? 'btn-primary' : 'btn-ghost'" @click="filter = 'tracked'">
              {{ t("config.task.filters.tracked") }}
            </button>
            <button class="btn btn-xs join-item" :class="filter === 'completed' ? 'btn-primary' : 'btn-ghost'" @click="filter = 'completed'">
              {{ t("config.task.filters.completed") }}
            </button>
            <button class="btn btn-xs join-item" :class="filter === 'all' ? 'btn-primary' : 'btn-ghost'" @click="filter = 'all'">
              {{ t("config.task.filters.all") }}
            </button>
          </div>
          <button class="btn btn-sm" :disabled="loading" @click="loadTasks">
            <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 21h5v-5"/></svg>
          </button>
        </div>
      </div>

      <div v-if="pagedTasks.length" class="divide-y divide-base-300/60">
        <div
          v-for="task in pagedTasks"
          :key="task.taskId"
          class="px-3 py-2 cursor-pointer transition-colors"
          :class="selectedTaskId === task.taskId ? 'bg-primary/10' : 'hover:bg-base-200/50'"
          @click="selectTask(task.taskId)"
        >
          <div class="flex items-start gap-3">
            <div class="w-2.5 h-2.5 rounded-full shrink-0 mt-1.5" :class="task.currentTracked ? 'bg-primary' : (task.completionState === 'completed' ? 'bg-success' : 'bg-base-300')"></div>
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <div class="font-medium text-sm wrap-break-word">{{ task.title }}</div>
                <span v-if="task.currentTracked" class="badge badge-xs badge-primary">{{ t("config.task.trackedShort") }}</span>
              </div>
              <div class="text-[11px] opacity-60 line-clamp-1 mt-0.5">{{ task.statusSummary || t("config.task.noStatus") }}</div>
              <div class="flex items-center gap-2 mt-1 text-[11px] opacity-50">
                <span>#{{ task.orderIndex }}</span>
                <span>{{ task.completionState }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="text-sm opacity-50 text-center py-4">{{ t("config.task.empty") }}</div>

      <!-- 分页 -->
      <div v-if="totalPages > 1" class="flex justify-center border-t border-base-300/70 px-3 py-2">
        <div class="join">
          <button class="btn btn-xs join-item" :disabled="page <= 1" @click="page--">‹</button>
          <button class="btn btn-xs join-item btn-active">{{ page }} / {{ totalPages }}</button>
          <button class="btn btn-xs join-item" :disabled="page >= totalPages" @click="page++">›</button>
        </div>
      </div>
    </div>

    <!-- 任务详情 -->
    <div class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
      <div class="flex items-center gap-2 px-3 py-2 border-b border-base-300/70">
        <div class="font-medium">{{ t("config.task.detail") }}</div>
        <div class="ml-auto flex items-center gap-2">
          <span class="text-xs opacity-50">{{ t("config.task.readonlyHint") }}</span>
          <button v-if="selectedTask" class="btn btn-sm" :disabled="loading" @click="reloadSelected">
            <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 21h5v-5"/></svg>
          </button>
        </div>
      </div>

      <div v-if="message" class="text-sm px-3 py-2 border-b border-base-300/70" :class="messageError ? 'bg-error/10 text-error' : 'bg-base-200/50'">
        {{ message }}
      </div>

      <div v-if="selectedTask">
        <!-- 标题行 -->
        <div class="px-3 py-2 border-b border-base-300/70">
          <div class="text-lg font-semibold wrap-break-word">{{ selectedTask.title }}</div>
          <div class="flex flex-wrap gap-2 mt-1">
            <span class="badge badge-xs" :class="selectedTask.currentTracked ? 'badge-primary' : 'badge-ghost'">
              {{ selectedTask.currentTracked ? t("config.task.currentTracked") : selectedTask.completionState }}
            </span>
            <span v-if="selectedTask.stageKey" class="badge badge-xs badge-outline">{{ selectedTask.stageKey }}</span>
          </div>
        </div>

        <!-- 内容列表 -->
        <div class="divide-y divide-base-300/60">
          <!-- 状态摘要 -->
          <div class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.task.fields.statusSummary") }}</div>
            <div class="text-sm whitespace-pre-wrap wrap-break-word">{{ selectedTask.statusSummary || '-' }}</div>
          </div>

          <!-- 目标 -->
          <div class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.task.fields.goal") }}</div>
            <div class="text-sm whitespace-pre-wrap wrap-break-word">{{ selectedTask.goal || '-' }}</div>
          </div>

          <!-- 起因 -->
          <div class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.task.fields.cause") }}</div>
            <div class="text-sm whitespace-pre-wrap wrap-break-word">{{ selectedTask.cause || '-' }}</div>
          </div>

          <!-- 流程 -->
          <div class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.task.fields.flow") }}</div>
            <div class="text-sm whitespace-pre-wrap wrap-break-word">{{ selectedTask.flow || '-' }}</div>
          </div>

          <!-- Todo 列表 -->
          <div class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.task.fields.todos") }}</div>
            <div v-if="selectedTask.todos.length" class="space-y-1">
              <div v-for="(todo, idx) in selectedTask.todos" :key="idx" class="text-sm wrap-break-word">- {{ todo }}</div>
            </div>
            <div v-else class="text-sm opacity-50">-</div>
          </div>

          <!-- 完成结论 -->
          <div v-if="selectedTask.completionConclusion" class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.task.fields.completionConclusion") }}</div>
            <div class="text-sm whitespace-pre-wrap wrap-break-word">{{ selectedTask.completionConclusion }}</div>
          </div>

          <!-- 进度笔记 -->
          <div v-if="selectedTask.progressNotes.length" class="px-3 py-2">
            <div class="text-[11px] opacity-50 uppercase tracking-wide mb-1">{{ t("config.task.notes") }}</div>
            <div class="space-y-2 max-h-48 overflow-y-auto">
              <div
                v-for="note in selectedTask.progressNotes.slice().reverse()"
                :key="`${note.at}-${note.note}`"
                class="bg-base-200/50 rounded px-2 py-1.5 text-sm"
              >
                <div class="text-[10px] opacity-50 mb-0.5">{{ formatTaskTime(note.at) }}</div>
                <div class="whitespace-pre-wrap wrap-break-word">{{ note.note }}</div>
              </div>
            </div>
          </div>

          <!-- 元信息 -->
          <div class="px-3 py-2 bg-base-200/30">
            <div class="grid grid-cols-2 md:grid-cols-3 gap-x-4 gap-y-1 text-xs">
              <div><span class="opacity-50">{{ t("config.task.fields.runAt") }}:</span> {{ formatTaskTime(selectedTask.trigger.runAt) }}</div>
              <div><span class="opacity-50">{{ t("config.task.fields.endAt") }}:</span> {{ formatTaskTime(selectedTask.trigger.endAt) }}</div>
              <div><span class="opacity-50">{{ t("config.task.fields.nextRunAt") }}:</span> {{ formatTaskTime(selectedTask.trigger.nextRunAt) }}</div>
              <div><span class="opacity-50">{{ t("config.task.fields.everyMinutes") }}:</span> {{ selectedTask.trigger.everyMinutes ?? '-' }}</div>
              <div><span class="opacity-50">{{ t("config.task.fields.updatedAt") }}:</span> {{ formatTaskTime(selectedTask.updatedAt) }}</div>
              <div class="col-span-2"><span class="opacity-50">ID:</span> <span class="font-mono">{{ selectedTask.taskId }}</span></div>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="text-sm opacity-50 text-center py-8">{{ t("config.task.selectHint") }}</div>
    </div>

    <!-- 运行日志 -->
    <div class="border border-base-300 rounded-box bg-base-100 overflow-hidden">
      <div class="flex items-center gap-2 px-3 py-2 border-b border-base-300/70">
        <div class="font-medium">{{ t("config.task.runLogs") }}</div>
        <div class="ml-auto">
          <button class="btn btn-sm" :disabled="loading" @click="refreshLogs">
            <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 21h5v-5"/></svg>
          </button>
        </div>
      </div>

      <div v-if="runLogs.length" class="divide-y divide-base-300/60 max-h-48 overflow-y-auto">
        <div
          v-for="log in runLogs"
          :key="log.id"
          class="px-3 py-2"
        >
          <div class="flex items-center justify-between gap-2">
            <span class="badge badge-xs" :class="runLogBadgeClass(log.outcome)">{{ runLogLabel(log.outcome) }}</span>
            <span class="text-[11px] opacity-50">{{ formatTaskTime(log.triggeredAt) }}</span>
          </div>
          <div v-if="log.note" class="text-sm whitespace-pre-wrap wrap-break-word opacity-70 mt-1">{{ log.note }}</div>
        </div>
      </div>

      <div v-else class="text-sm opacity-50 text-center py-4">{{ t("config.task.noLogs") }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";

type TaskTrigger = {
  runAt?: string;
  endAt?: string;
  everyMinutes?: number;
  nextRunAt?: string;
};

type TaskProgressNote = {
  at: string;
  note: string;
};

type TaskEntry = {
  taskId: string;
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
  stageUpdatedAt?: string;
  trigger: TaskTrigger;
  createdAt: string;
  updatedAt: string;
  lastTriggeredAt?: string;
  completedAt?: string;
  currentTracked: boolean;
};

type TaskRunLogEntry = {
  id: number;
  taskId: string;
  triggeredAt: string;
  outcome: string;
  note: string;
};

const PAGE_SIZE = 10;

const { t } = useI18n();
const loading = ref(false);
const message = ref("");
const messageError = ref(false);
const tasks = ref<TaskEntry[]>([]);
const runLogs = ref<TaskRunLogEntry[]>([]);
const selectedTaskId = ref("");
const filter = ref<"active" | "tracked" | "completed" | "all">("active");
const page = ref(1);

const trackedTask = computed(() => tasks.value.find((item) => item.currentTracked) ?? null);
const selectedTask = computed(() => tasks.value.find((item) => item.taskId === selectedTaskId.value) ?? null);
const filteredTasks = computed(() => {
  if (filter.value === "all") return tasks.value;
  if (filter.value === "tracked") return tasks.value.filter((item) => item.currentTracked);
  if (filter.value === "completed") return tasks.value.filter((item) => item.completionState !== "active");
  return tasks.value.filter((item) => item.completionState === "active");
});
const totalPages = computed(() => Math.max(1, Math.ceil(filteredTasks.value.length / PAGE_SIZE)));
const pagedTasks = computed(() => {
  const start = (page.value - 1) * PAGE_SIZE;
  return filteredTasks.value.slice(start, start + PAGE_SIZE);
});

// 过滤器变化时重置页码
watch(filter, () => {
  page.value = 1;
});

function setMessage(text: string, isError = false) {
  message.value = text;
  messageError.value = isError;
}

function formatTaskTime(value?: string | null): string {
  const raw = String(value || '').trim();
  if (!raw) return '-';
  const parsed = new Date(raw);
  if (Number.isNaN(parsed.getTime())) return raw;
  const parts = new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).formatToParts(parsed);
  const pick = (type: string) => parts.find((part) => part.type === type)?.value || '00';
  return `${pick('year')}-${pick('month')}-${pick('day')} ${pick('hour')}:${pick('minute')}:${pick('second')}`;
}

function runLogLabel(outcome: string): string {
  if (outcome === "sent") return "已发送";
  if (outcome === "queued") return "已排队";
  if (outcome === "dequeued") return "已恢复";
  if (outcome === "failed") return "失败";
  return outcome || "-";
}

function runLogBadgeClass(outcome: string): string {
  if (outcome === "sent") return "badge-success";
  if (outcome === "queued") return "badge-warning";
  if (outcome === "dequeued") return "badge-info";
  if (outcome === "failed") return "badge-error";
  return "badge-ghost";
}

async function loadRunLogs(taskId?: string) {
  try {
    runLogs.value = await invokeTauri<TaskRunLogEntry[]>("task_list_run_logs", {
      input: { taskId: (taskId ?? selectedTaskId.value) || undefined, limit: 50 },
    });
  } catch (error) {
    setMessage(String(error), true);
  }
}

async function loadTasks() {
  loading.value = true;
  try {
    tasks.value = await invokeTauri<TaskEntry[]>("task_list_tasks");
    if (!selectedTaskId.value && tasks.value.length > 0) {
      selectedTaskId.value = (trackedTask.value ?? tasks.value[0]).taskId;
    }
    await loadRunLogs();
  } catch (error) {
    setMessage(String(error), true);
  } finally {
    loading.value = false;
  }
}

async function selectTask(taskId: string) {
  selectedTaskId.value = taskId;
  await reloadSelected();
}

async function reloadSelected() {
  if (!selectedTaskId.value) return;
  loading.value = true;
  try {
    const detail = await invokeTauri<TaskEntry>("task_get_task", { input: { taskId: selectedTaskId.value } });
    tasks.value = tasks.value.map((item) => (item.taskId === detail.taskId ? detail : item));
    if (!tasks.value.some((item) => item.taskId === detail.taskId)) {
      tasks.value.unshift(detail);
    }
    await loadRunLogs(detail.taskId);
  } catch (error) {
    setMessage(String(error), true);
  } finally {
    loading.value = false;
  }
}

async function refreshLogs() {
  await loadRunLogs();
}

onMounted(() => {
  void loadTasks();
});
</script>
