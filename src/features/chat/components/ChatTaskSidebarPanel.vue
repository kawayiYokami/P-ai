<template>
  <div class="conversation-tab-panel">
    <div v-if="loading && groupedTaskSections.length === 0" class="flex justify-center px-3 py-6">
      <AppLoading size="sm" class="text-base-content/45" />
    </div>
    <div v-else-if="errorText" class="px-3 py-4 text-center text-sm text-error">
      {{ errorText }}
    </div>
    <template v-else>
      <CollapsibleGroup
        head-opaque
        v-for="section in groupedTaskSections"
        :key="section.key"
        :title="section.title"
        :count="section.items.length"
        :model-value="isTaskSectionCollapsed(section.key)"
        @update:model-value="toggleTaskSection(section.key)"
        @collapse-all="collapseAllTaskSections"
        @after-enter="emit('layoutChange')"
        @after-leave="emit('layoutChange')"
      >
        <TaskListItem
          v-for="task in section.items"
          :key="task.taskId"
          :label="taskTitle(task)"
          :description="taskTodo(task)"
          :title="taskItemTitle(task, section.title)"
          :time-label="taskTimeLabel(task)"
          @click="emit('editTask', task)"
        />
      </CollapsibleGroup>
      <div v-if="groupedTaskSections.length === 0" class="px-3 py-4 text-center text-sm text-base-content/60">
        {{ normalizedSearchQuery ? t("chat.taskSidebar.searchEmpty") : t("chat.taskSidebar.empty") }}
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import AppLoading from "../../shared/components/AppLoading.vue";
import { invokeTauri, onTransportNotification, onTransportRecovered } from "../../../services/tauri-api";
import type { ChatConversationOverviewItem } from "../../../types/app";
import type { TaskEntry } from "../../config/views/config-tabs/task-editor";
import { toErrorMessage } from "../../../utils/error";
import CollapsibleGroup from "./CollapsibleGroup.vue";
import TaskListItem from "./TaskListItem.vue";
import { formatConversationListTime } from "../utils/conversation-time";
import { resolveConversationDisplayTitle } from "../utils/conversation-title";

const SYSTEM_NOTIFICATION_CONVERSATION_ID = "system-notification-conversation";

type TaskSection = {
  key: string;
  title: string;
  items: TaskEntry[];
  order: number;
};

const props = defineProps<{
  conversationItems: ChatConversationOverviewItem[];
  searchQuery: string;
}>();

const emit = defineEmits<{
  (e: "editTask", task: TaskEntry): void;
  (e: "layoutChange"): void;
}>();

const { t, locale } = useI18n();
const tasks = ref<TaskEntry[]>([]);
const loading = ref(false);
const errorText = ref("");
const collapsedTaskSectionKeys = ref<Record<string, boolean>>({});

const normalizedSearchQuery = computed(() =>
  String(props.searchQuery || "").trim().toLocaleLowerCase(),
);

const conversationTitleById = computed(() => {
  const map = new Map<string, string>();
  for (const item of props.conversationItems || []) {
    const conversationId = String(item.conversationId || "").trim();
    if (!conversationId) continue;
    map.set(conversationId, resolveConversationDisplayTitle(item, {
      locale: locale.value,
      untitledLabel: t("chat.untitledConversation"),
      systemNotificationLabel: t("chat.systemPersona"),
    }));
  }
  return map;
});

const conversationOrderById = computed(() => {
  const map = new Map<string, number>();
  (props.conversationItems || []).forEach((item, index) => {
    const conversationId = String(item.conversationId || "").trim();
    if (conversationId && !map.has(conversationId)) {
      map.set(conversationId, index);
    }
  });
  return map;
});

const groupedTaskSections = computed<TaskSection[]>(() => {
  const query = normalizedSearchQuery.value;
  const sections = new Map<string, TaskSection>();
  for (const task of tasks.value) {
    if (String(task.completionState || "").trim() !== "active") continue;
    const rawConversationId = String(task.conversationId || "").trim();
    const conversationId = rawConversationId || SYSTEM_NOTIFICATION_CONVERSATION_ID;
    const isSystemTask = conversationId === SYSTEM_NOTIFICATION_CONVERSATION_ID;
    const sectionKey = `conversation:${conversationId}`;
    const title = isSystemTask
      ? t("chat.taskSidebar.systemConversation")
      : conversationTitleById.value.get(conversationId) || t("chat.taskSidebar.unknownConversation");
    if (query && !taskMatchesSearch(task, title, query)) continue;
    if (!sections.has(sectionKey)) {
      sections.set(sectionKey, {
        key: sectionKey,
        title,
        items: [],
        order: isSystemTask ? -1 : conversationOrderById.value.get(conversationId) ?? 1_000_000,
      });
    }
    sections.get(sectionKey)?.items.push(task);
  }
  return Array.from(sections.values())
    .map((section) => ({
      ...section,
      items: section.items.slice().sort(compareTaskForList),
    }))
    .sort((left, right) => left.order - right.order || left.title.localeCompare(right.title, locale.value));
});

function taskMatchesSearch(task: TaskEntry, sectionTitle: string, query: string): boolean {
  return [
    sectionTitle,
    task.goal,
    task.todo,
    task.taskId,
  ].some((value) => String(value || "").toLocaleLowerCase().includes(query));
}

function taskSortTime(task: TaskEntry): number {
  const raw = String(task.trigger?.next_run_at || task.trigger?.run_at || task.updatedAtLocal || "").trim();
  if (!raw) return Number.POSITIVE_INFINITY;
  const time = new Date(raw).getTime();
  return Number.isFinite(time) ? time : Number.POSITIVE_INFINITY;
}

function compareTaskForList(left: TaskEntry, right: TaskEntry): number {
  const timeDiff = taskSortTime(left) - taskSortTime(right);
  if (timeDiff !== 0) return timeDiff;
  return Number(left.orderIndex || 0) - Number(right.orderIndex || 0);
}

function taskTitle(task: TaskEntry): string {
  return String(task.goal || "").trim() || t("config.task.noTodo");
}

function taskTodo(task: TaskEntry): string {
  return String(task.todo || "").trim() || t("config.task.noTodo");
}

function taskTimeLabel(task: TaskEntry): string {
  const raw = String(task.trigger?.next_run_at || task.trigger?.run_at || "").trim();
  return raw ? formatConversationListTime(raw, locale.value) : "";
}

function taskItemTitle(task: TaskEntry, sectionTitle: string): string {
  return `${sectionTitle}\n${taskTitle(task)}`;
}

function isTaskSectionCollapsed(key: string): boolean {
  return !!collapsedTaskSectionKeys.value[key];
}

function toggleTaskSection(key: string) {
  collapsedTaskSectionKeys.value = {
    ...collapsedTaskSectionKeys.value,
    [key]: !collapsedTaskSectionKeys.value[key],
  };
  emit("layoutChange");
}

function collapseAllTaskSections() {
  collapsedTaskSectionKeys.value = groupedTaskSections.value.reduce((next, section) => {
    next[section.key] = true;
    return next;
  }, { ...collapsedTaskSectionKeys.value } as Record<string, boolean>);
  emit("layoutChange");
}

async function loadTasks() {
  loading.value = true;
  errorText.value = "";
  try {
    tasks.value = await invokeTauri<TaskEntry[]>("task.list", {});
    console.info("[Sidebar任务列表] 完成", {
      total: tasks.value.length,
      active: tasks.value.filter((task) => String(task.completionState || "").trim() === "active").length,
    });
  } catch (error) {
    errorText.value = `${t("config.task.listLoadFailed")}: ${toErrorMessage(error)}`;
  } finally {
    loading.value = false;
    emit("layoutChange");
  }
}

type TaskChangedMonitorPayload = { domain?: string };

function handleTaskRefreshEvent(payload: TaskChangedMonitorPayload | null | undefined) {
  if (String(payload?.domain || "").trim() !== "task") return;
  void loadTasks();
}

let unlistenTaskChanged: (() => void) | null = null;
let unlistenTaskRecovered: (() => void) | null = null;

onMounted(() => {
  void loadTasks();
  unlistenTaskChanged = onTransportNotification("monitor.changed", handleTaskRefreshEvent);
  unlistenTaskRecovered = onTransportRecovered(() => {
    void loadTasks();
  });
});

onBeforeUnmount(() => {
  unlistenTaskChanged?.();
  unlistenTaskChanged = null;
  unlistenTaskRecovered?.();
  unlistenTaskRecovered = null;
});
</script>
