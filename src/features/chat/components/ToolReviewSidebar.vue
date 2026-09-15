<template>
  <aside v-bind="rootAttrs" class="w-full flex h-full min-h-0 flex-col bg-base-200">
    <div ref="contentScroller" class="ecall-chat-scroll-container flex min-h-0 flex-1 flex-col overflow-y-auto p-1">
      <div v-if="errorText" class="mx-4 my-4 rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
        {{ errorText }}
      </div>

      <template v-if="activeTab === 'tools' && currentBatch">
        <div class="flex min-h-full flex-col">
          <div class="flex min-h-0 flex-1 flex-col py-2">
            <CollapsibleGroup
              v-for="group in reviewGroups"
              :key="group.key"
              :title="group.title"
              :count="group.segments.length || group.items.length"
              :model-value="isToolAssessmentSectionCollapsed(group.key)"
              @update:model-value="toggleToolAssessmentSection(group.key)"
              @collapse-all="collapseAllToolAssessmentSections"
            >
              <div v-if="!isToolAssessmentSectionCollapsed(group.key)">
                <template v-if="group.segments.length > 0">
                  <div v-for="(segment, idx) in group.segments" :key="`${group.key}:segment:${idx}`" class="mx-1 mb-1">
                    <TerminalApprovalPatchSample
                      v-if="segment.diffLines.length > 0"
                      :lines="segment.diffLines"
                      :diff-only="true"
                      :lang="resolveShikiLanguage(extensionFromPath(segment.path))"
                      :title="segmentActionLabel(segment)"
                      :collapsed="isSegmentCollapsed(group.key, idx)"
                      class="mt-1 rounded-lg"
                      @update:collapsed="setSegmentCollapsed(group.key, idx, $event)"
                      @collapse-all="collapseAllSegmentsInGroup(group.key)"
                    />
                  </div>
                </template>
                <template v-else>
                  <ToolAssessmentCard
                    v-for="item in group.items"
                    :key="`${group.key}:${item.callId}`"
                    :item="item"
                    :detail="detailMap[item.callId]"
                    :loading="detailLoadingCallId === item.callId"
                    :reviewing="reviewingCallId === item.callId"
                    :is-dark="markdownIsDark"
                    @load-detail="emit('loadItemDetail', $event)"
                    @review="emit('reviewItem', $event)"
                  />
                </template>
              </div>
            </CollapsibleGroup>
          </div>
        </div>
      </template>

      <div v-else-if="activeTab === 'tools'" class="flex flex-1 items-center justify-center px-4 py-8 text-sm text-base-content/65">
        {{ t("chat.toolReview.empty") }}
      </div>

      <template v-else-if="activeTab === 'delegates'">
        <div v-if="delegateStatusesErrorText" class="mx-4 my-4 rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
          {{ delegateStatusesErrorText }}
        </div>
        <div v-else-if="delegateStatuses.length === 0" class="flex min-h-0 flex-1 items-center justify-center px-4 py-8 text-sm text-base-content/65">
          {{ t("chat.toolReview.delegateEmpty") }}
        </div>
        <CollapsibleGroup
          v-for="section in delegateStatusSections"
          :key="section.key"
          :title="section.title"
          :count="section.items.length"
          :model-value="isDelegateSectionCollapsed(section.key)"
          @update:model-value="toggleDelegateSection(section.key)"
          @collapse-all="collapseAllDelegateSections"
        >
          <div v-if="!isDelegateSectionCollapsed(section.key)">
            <section v-for="delegate in section.items" :key="delegate.delegateId" class="mx-1">
              <DelegateCard
                :title="delegate.title || delegate.delegateId"
                :running="isDelegateRunning(delegate)"
                :elapsed-ms="delegate.elapsedMs"
                :request-count="delegate.requestCount"
                :token-count="delegate.tokenCount"
                :last-tool-name="delegate.lastToolName"
                :show-result="canShowDelegateResult(delegate)"
                :time-date-label="delegateTimeLabel(delegate).dateLabel"
                :time-minute-label="delegateTimeLabel(delegate).timeLabel"
                :started-label="delegateStartedLabel(delegate)"
                :avatar-url="personaAvatarUrlMap[delegate.targetAgentId || ''] || ''"
                @abort="emit('abortDelegate', delegate)"
                @open-detail="openDelegateResult(delegate)"
              />
            </section>
          </div>
        </CollapsibleGroup>
      </template>

      <template v-else-if="activeTab === 'tasks'">
        <div v-if="taskErrorText" class="mx-4 my-4 rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
          {{ taskErrorText }}
        </div>
        <div v-else-if="taskLoading && taskSections.length === 0" class="flex min-h-0 flex-1 items-center justify-center px-4 py-8">
          <span class="loading loading-spinner loading-sm text-base-content/45"></span>
        </div>
        <div v-else-if="taskSections.length === 0" class="flex min-h-0 flex-1 items-center justify-center px-4 py-8 text-sm text-base-content/65">
          {{ t("chat.toolReview.taskEmpty") }}
        </div>
        <CollapsibleGroup
          v-for="section in taskSections"
          :key="section.key"
          :title="section.title"
          :count="section.items.length"
          :model-value="isTaskSectionCollapsed(section.key)"
          @update:model-value="toggleTaskSection(section.key)"
          @collapse-all="collapseAllTaskSections"
        >
          <div v-if="!isTaskSectionCollapsed(section.key)">
            <TaskListItem
              v-for="task in section.items"
              :key="task.taskId"
              :label="taskListTitle(task)"
              :description="taskListTodo(task)"
              :title="taskListTitle(task)"
              :time-date-label="taskListTime(task).dateLabel"
              :time-minute-label="taskListTime(task).timeLabel"
              :disabled="!canEditTaskInSidebar"
              @click="openTaskEditor(task)"
            />
          </div>
        </CollapsibleGroup>
      </template>

      <FastRequestTurnsPanel
        v-else-if="activeTab === 'fastRequests'"
        :conversation-id="activeConversationId"
        active
      />
    </div>
    <div
      v-if="activeTab === 'tools' && currentBatch && props.batches.length > 1"
      class="shrink-0 border-t border-base-300 bg-base-200 px-4 py-3"
    >
      <div class="join flex justify-center">
        <button
          type="button"
          class="join-item btn btn-sm bg-base-100 hover:bg-base-100"
          :disabled="!previousBatch"
          @click="previousBatch && emit('selectBatch', previousBatch.batchKey)"
        >
          «
        </button>
        <button
          type="button"
          class="join-item btn btn-sm bg-base-100 hover:bg-base-100"
          @click.prevent
        >
          {{ t("chat.toolReview.pageLabel", { current: currentBatchIndex + 1, total: props.batches.length }) }}
        </button>
        <button
          type="button"
          class="join-item btn btn-sm bg-base-100 hover:bg-base-100"
          :disabled="!nextBatch"
          @click="nextBatch && emit('selectBatch', nextBatch.batchKey)"
        >
          »
        </button>
      </div>
    </div>
    <FloatingScrollbar :target="contentScroller" />
  </aside>

  <dialog ref="delegateResultDialogRef" class="modal" @close="closeDelegateResultDialog" @cancel.prevent="closeDelegateResultDialog">
    <div class="modal-box flex max-h-[80vh] max-w-2xl flex-col overflow-hidden p-0">
      <div class="flex shrink-0 items-center justify-between gap-3 border-b border-base-200 px-5 py-3">
        <div class="min-w-0 truncate text-sm font-semibold text-base-content">{{ delegateResultTitle }}</div>
        <button type="button" class="btn btn-ghost btn-sm gap-1 shrink-0" @click="closeDelegateResultDialog"><span class="text-base leading-none">×</span><span>关闭</span></button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        <DelegateScheduleTimeline
          v-if="delegateResultConversationId"
          :conversation-id="delegateResultConversationId"
          :auto-refresh-key="delegateResultTimelineKey"
        />
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="closeDelegateResultDialog">close</button>
    </form>
  </dialog>

  <TaskCreateCard
    :open="taskEditorOpen"
    mode="edit"
    :conversation-id="activeConversationId"
    :task="taskEditorTask"
    @close="closeTaskEditor"
    @created="handleTaskMutated"
    @updated="handleTaskMutated"
  />

</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useAttrs, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri, onTransportNotification, onTransportRecovered } from "../../../services/tauri-api";
import type { ArchiveBlockPage, BackgroundShellTaskSummary, ChatMessage, ConversationDelegateStatusSummary, ShellWorkspace } from "../../../types/app";
import { toErrorMessage } from "../../../utils/error";
import { defaultWorkspaceNameFromPath, inferWorkspaceName, isLegacyGenericWorkspaceName, normalizeWorkspaceLevel } from "../../../utils/shell-workspaces";
import type { ToolReviewBatchSummary, ToolReviewItemDetail, ToolReviewItemSummary, ToolReviewSegment } from "../composables/use-chat-tool-review";
import { groupSegmentsByFile } from "../composables/tool-review-segments";
import { formatConversationListTimeWithMinuteDetails } from "../utils/conversation-time";
import { AppMarkdownRenderer, initKatex } from "../markdown";
import ToolAssessmentCard from "./ToolAssessmentCard.vue";
import DelegateCard from "./DelegateCard.vue";
import DelegateScheduleTimeline from "./DelegateScheduleTimeline.vue";
import FloatingScrollbar from "../../shell/components/FloatingScrollbar.vue";
import CollapsibleGroup from "./CollapsibleGroup.vue";
import TerminalApprovalPatchSample from "../../shell/components/TerminalApprovalPatchSample.vue";
import { resolveShikiLanguage, extensionFromPath } from "../../file-reader/utils";
import TaskListItem from "./TaskListItem.vue";
import TaskCreateCard from "./dialogs/TaskCreateCard.vue";
import FastRequestTurnsPanel from "./FastRequestTurnsPanel.vue";
import type { TaskEntry } from "../../config/views/config-tabs/task-editor";

initKatex();

type ToolReviewSidebarTab = "tools" | "delegates" | "tasks" | "fastRequests";

const props = defineProps<{
  activeTab: ToolReviewSidebarTab;
  batches: ToolReviewBatchSummary[];
  currentBatchKey: string;
  detailMap: Record<string, ToolReviewItemDetail>;
  segmentMap: Record<string, ToolReviewSegment[]>;
  detailLoadingCallId: string;
  reviewingCallId: string;
  batchReviewingKey: string;
  errorText: string;
  markdownIsDark: boolean;
  activeConversationId: string;
  currentWorkspaceName: string;
  currentWorkspaceRootPath: string;
  workspaces: ShellWorkspace[];
  agentOptions: Array<{ id: string; name: string; ownerName: string; providerName?: string; modelName?: string }>;
  delegateStatuses: ConversationDelegateStatusSummary[];
  delegateStatusesErrorText: string;
  personaAvatarUrlMap: Record<string, string>;
}>();

const emit = defineEmits<{
  (e: "selectBatch", batchKey: string): void;
  (e: "loadItemDetail", callId: string): void;
  (e: "reviewItem", callId: string): void;
  (e: "reviewBatch", batchKey: string): void;
  (e: "openDelegateDetail", status: ConversationDelegateStatusSummary): void;
  (e: "abortDelegate", status: ConversationDelegateStatusSummary): void;
  (e: "assistantLinkClick", event: MouseEvent): void;
}>();

const { t, locale } = useI18n();
const contentScroller = ref<HTMLElement | null>(null);
const delegateResultDialogRef = ref<HTMLDialogElement | null>(null);
const delegateResultDialogOpen = ref(false);

function closeDelegateResultDialog() {
  delegateResultDialogOpen.value = false;
}

function syncDelegateResultDialog() {
  const d = delegateResultDialogRef.value;
  if (!d) return;
  if (delegateResultDialogOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(delegateResultDialogOpen, syncDelegateResultDialog);
watch(delegateResultDialogRef, syncDelegateResultDialog);
const delegateResultTitle = ref("");
const rootAttrs = useAttrs();
const collapsedToolAssessmentSectionKeys = ref<Record<string, boolean>>({});
const collapsedSegmentKeys = ref<Record<string, boolean>>({});
const collapsedDelegateSectionKeys = ref<Record<string, boolean>>({
  running: false,
  completed: true,
  interrupted: true,
  failed: true,
});
const delegateResultConversationId = ref("");
const delegateResultTimelineKey = ref("");
const collapsedTaskSectionKeys = ref<Record<string, boolean>>({
  active: false,
  completed: true,
  failed_completed: true,
});
const taskEntries = ref<TaskEntry[]>([]);
const taskLoading = ref(false);
const taskErrorText = ref("");
const taskEditorOpen = ref(false);
const taskEditorTask = ref<TaskEntry | null>(null);

type DelegateStatusSection = {
  key: string;
  title: string;
  items: ConversationDelegateStatusSummary[];
};

type TaskSectionKey = "active" | "completed" | "failed_completed";

type TaskSection = {
  key: TaskSectionKey;
  title: string;
  items: TaskEntry[];
  order: number;
};

const delegateStatusSections = computed<DelegateStatusSection[]>(() => {
  const sections: DelegateStatusSection[] = [
    { key: "running", title: "正在运行中", items: [] },
    { key: "completed", title: "已完成", items: [] },
    { key: "interrupted", title: "被中断", items: [] },
    { key: "failed", title: "已失败", items: [] },
  ];
  for (const delegate of props.delegateStatuses) {
    sections[delegateStatusSectionIndex(delegate)].items.push(delegate);
  }
  return sections.filter((section) => section.items.length > 0);
});

const canEditTaskInSidebar = computed(() => true);

const currentConversationTasks = computed(() => {
  const conversationId = String(props.activeConversationId || "").trim();
  if (!conversationId) return [];
  return taskEntries.value.filter((task) => String(task.conversationId || "").trim() === conversationId);
});
const taskSections = computed<TaskSection[]>(() => {
  const sections: TaskSection[] = [
    { key: "active", title: t("config.task.filters.active"), items: [], order: 0 },
    { key: "completed", title: t("config.task.completionStates.completed"), items: [], order: 1 },
    { key: "failed_completed", title: t("config.task.completionStates.failedCompleted"), items: [], order: 2 },
  ];
  for (const task of currentConversationTasks.value) {
    const section = sections.find((item) => item.key === normalizeTaskSectionKey(task.completionState));
    section?.items.push(task);
  }
  return sections
    .map((section) => ({
      ...section,
      items: section.items.slice().sort(compareTaskForSidebar),
    }))
    .filter((section) => section.items.length > 0)
    .sort((left, right) => left.order - right.order);
});
function delegateStatusSectionIndex(delegate: ConversationDelegateStatusSummary) {
  const status = String(delegate.status || "").trim();
  if (status === "failed") return 3;
  if ((status === "running" || status === "delivered") && delegate.active) return 0;
  if (status === "running" || status === "delivered") return 2;
  return 1;
}

function isToolAssessmentSectionCollapsed(key: string) {
  return !!collapsedToolAssessmentSectionKeys.value[key];
}

function toggleToolAssessmentSection(key: string) {
  const nextCollapsed = !collapsedToolAssessmentSectionKeys.value[key];
  const next = { ...collapsedToolAssessmentSectionKeys.value, [key]: nextCollapsed };
  if (!nextCollapsed) {
    // 手风琴：展开当前分组时，折叠其余所有分组
    for (const section of reviewGroups.value) {
      if (section.key !== key) next[section.key] = true;
    }
  }
  collapsedToolAssessmentSectionKeys.value = next;
}

function collapseAllToolAssessmentSections() {
  collapsedToolAssessmentSectionKeys.value = reviewGroups.value.reduce((next, section) => {
    next[section.key] = true;
    return next;
  }, { ...collapsedToolAssessmentSectionKeys.value } as Record<string, boolean>);
}

function segmentCollapseKey(groupKey: string, idx: number) {
  return `${groupKey}:segment:${idx}`;
}

function isSegmentCollapsed(groupKey: string, idx: number) {
  return !!collapsedSegmentKeys.value[segmentCollapseKey(groupKey, idx)];
}

function setSegmentCollapsed(groupKey: string, idx: number, value: boolean) {
  const key = segmentCollapseKey(groupKey, idx);
  if (!!collapsedSegmentKeys.value[key] === value) return;
  collapsedSegmentKeys.value = {
    ...collapsedSegmentKeys.value,
    [key]: value,
  };
}

function collapseAllSegmentsInGroup(groupKey: string) {
  const group = reviewGroups.value.find((item) => item.key === groupKey);
  if (!group) return;
  const next = { ...collapsedSegmentKeys.value };
  group.segments.forEach((_, idx) => {
    next[segmentCollapseKey(groupKey, idx)] = true;
  });
  collapsedSegmentKeys.value = next;
}

function isDelegateSectionCollapsed(key: string) {
  return !!collapsedDelegateSectionKeys.value[key];
}

function toggleDelegateSection(key: string) {
  collapsedDelegateSectionKeys.value = {
    ...collapsedDelegateSectionKeys.value,
    [key]: !collapsedDelegateSectionKeys.value[key],
  };
}

function collapseAllDelegateSections() {
  collapsedDelegateSectionKeys.value = delegateStatusSections.value.reduce((next, section) => {
    next[section.key] = true;
    return next;
  }, { ...collapsedDelegateSectionKeys.value } as Record<string, boolean>);
}

function normalizeTaskSectionKey(value: string): TaskSectionKey {
  return value === "completed" || value === "failed_completed" ? value : "active";
}

function isTaskSectionCollapsed(key: TaskSectionKey) {
  return !!collapsedTaskSectionKeys.value[key];
}

function toggleTaskSection(key: TaskSectionKey) {
  collapsedTaskSectionKeys.value = {
    ...collapsedTaskSectionKeys.value,
    [key]: !collapsedTaskSectionKeys.value[key],
  };
}

function collapseAllTaskSections() {
  collapsedTaskSectionKeys.value = taskSections.value.reduce((next, section) => {
    next[section.key] = true;
    return next;
  }, { ...collapsedTaskSectionKeys.value } as Record<string, boolean>);
}

function taskListTitle(task: TaskEntry) {
  return String(task.goal || "").trim() || t("config.task.noTodo");
}

function taskListTodo(task: TaskEntry) {
  return String(task.todo || "").trim() || t("config.task.noTodo");
}

function taskListTime(task: TaskEntry) {
  const raw = String(task.completedAtLocal || task.updatedAtLocal || "").trim();
  return raw ? formatConversationListTimeWithMinuteDetails(raw, locale.value) : { dateLabel: "", timeLabel: "" };
}

function compareTaskForSidebar(left: TaskEntry, right: TaskEntry) {
  const leftRaw = String(left.completedAtLocal || left.updatedAtLocal || "").trim();
  const rightRaw = String(right.completedAtLocal || right.updatedAtLocal || "").trim();
  const leftTime = leftRaw ? new Date(leftRaw).getTime() : Number.POSITIVE_INFINITY;
  const rightTime = rightRaw ? new Date(rightRaw).getTime() : Number.POSITIVE_INFINITY;
  if (leftTime !== rightTime) return leftTime - rightTime;
  return Number(left.orderIndex || 0) - Number(right.orderIndex || 0);
}

function delegateTimeLabel(delegate: ConversationDelegateStatusSummary) {
  const raw = String(delegate.completedAt || delegate.archivedAt || delegate.updatedAt || "").trim();
  return raw ? formatConversationListTimeWithMinuteDetails(raw, locale.value) : { dateLabel: "", timeLabel: "" };
}

function delegateStartedLabel(delegate: ConversationDelegateStatusSummary) {
  const raw = String(delegate.startedAt || "").trim();
  if (!raw) return "";
  const parts = formatConversationListTimeWithMinuteDetails(raw, locale.value);
  return parts.timeLabel ? `${parts.dateLabel} ${parts.timeLabel}` : parts.dateLabel;
}

const currentBatchIndex = computed(() => {
  const currentKey = String(props.currentBatchKey || "").trim();
  if (!currentKey) return -1;
  return props.batches.findIndex((batch) => batch.batchKey === currentKey);
});

const currentBatch = computed(() => {
  const currentKey = String(props.currentBatchKey || "").trim();
  if (!currentKey) return null;
  return props.batches.find((batch) => batch.batchKey === currentKey) || null;
});

const previousBatch = computed(() => {
  const index = currentBatchIndex.value;
  if (index < 0) {
    return props.batches[props.batches.length - 1] || null;
  }
  if (index <= 0) return null;
  return props.batches[index - 1] || null;
});

const nextBatch = computed(() => {
  const index = currentBatchIndex.value;
  if (index < 0 || index >= props.batches.length - 1) return null;
  return props.batches[index + 1] || null;
});

function openDelegateResult(status: import("../../../types/app").ConversationDelegateStatusSummary) {
  const conversationId = String(status?.conversationId || status?.delegateId || "").trim();
  if (!conversationId) return;
  delegateResultTitle.value = String(status?.title || status?.delegateId || "委托结果");
  delegateResultConversationId.value = conversationId;
  delegateResultTimelineKey.value = `${status.status}:${status.updatedAt}`;
  delegateResultDialogOpen.value = true;
}

type ToolReviewGroup = {
  key: string;
  title: string;
  firstOrderIndex: number;
  items: ToolReviewItemSummary[];
  segments: ToolReviewSegment[];
};

function isTerminalTool(toolName: string) {
  const normalized = String(toolName || "").trim();
  return normalized === "shell_exec" || normalized === "exec";
}

function isFileChangeTool(toolName: string) {
  const normalized = String(toolName || "").trim();
  return normalized === "apply_patch"
    || normalized === "write"
    || normalized === "delete"
    || normalized === "update"
    || normalized === "move";
}

const reviewGroups = computed<ToolReviewGroup[]>(() => {
  const terminalItems = [] as ToolReviewItemSummary[];
  const otherGroups = new Map<string, ToolReviewGroup>();
  for (const item of currentBatch.value?.items ?? []) {
    if (isTerminalTool(item.toolName)) {
      terminalItems.push(item);
      continue;
    }
    if (!isFileChangeTool(item.toolName) || item.isSuccess === false) {
      const toolName = String(item.toolName || "").trim() || t("chat.toolReview.otherGroup");
      const groupKey = `other:${toolName}`;
      const group = otherGroups.get(groupKey) || {
        key: groupKey,
        title: toolName,
        firstOrderIndex: Number(item.orderIndex || 0),
        items: [],
        segments: [],
      };
      group.firstOrderIndex = Math.min(group.firstOrderIndex, Number(item.orderIndex || 0));
      group.items.push(item);
      otherGroups.set(groupKey, group);
      continue;
    }
  }

  const segments = Array.isArray(props.segmentMap[props.currentBatchKey]) ? props.segmentMap[props.currentBatchKey] : [];
  const fileGroups = new Map<string, ToolReviewGroup>();
  groupSegmentsByFile(segments).forEach((fileGroup, index) => {
    const key = `file:${fileGroup.path || "__unknown__"}`;
    fileGroups.set(key, {
      key,
      title: formatPatchGroupTitle(fileGroup.path),
      firstOrderIndex: index,
      items: [],
      segments: fileGroup.segments,
    });
  });

  const groups = [] as ToolReviewGroup[];
  if (terminalItems.length > 0) {
    groups.push({
      key: "terminal",
      title: t("chat.toolReview.terminalGroup"),
      firstOrderIndex: Math.min(...terminalItems.map((item) => Number(item.orderIndex || 0))),
      items: terminalItems.sort(sortByOrderIndex),
      segments: [],
    });
  }
  groups.push(
    ...Array.from(fileGroups.values())
      .sort((a, b) => a.firstOrderIndex - b.firstOrderIndex)
  );
  groups.push(
    ...Array.from(otherGroups.values())
      .map((group) => ({ ...group, items: group.items.sort(sortByOrderIndex) }))
      .sort((a, b) => a.firstOrderIndex - b.firstOrderIndex)
  );
  return groups;
});

function sortByOrderIndex(left: ToolReviewItemSummary, right: ToolReviewItemSummary) {
  return Number(left.orderIndex || 0) - Number(right.orderIndex || 0);
}

function segmentActionLabel(segment: ToolReviewSegment) {
  const action = String(segment.action || "").trim();
  if (action === "add") return t("chat.toolReview.patchOperationAdd");
  if (action === "delete") return t("chat.toolReview.patchOperationDelete");
  if (action === "move") return t("chat.toolReview.patchOperationMove");
  return t("chat.toolReview.patchOperationUpdate");
}

function formatPatchGroupTitle(path: string) {
  const normalized = String(path || "").replace(/\\/g, "/").trim();
  if (!normalized) return t("chat.toolReview.patchUnknownFileGroup");
  return compactPathByWorkspace(normalized);
}

function compactPathByWorkspace(path: string) {
  const normalizedPath = normalizePathForDisplay(path);
  const matches = workspacePathDisplayCandidates.value
    .map((candidate) => {
      const root = candidate.root;
      if (!root) return null;
      if (isSameNormalizedPath(normalizedPath, root)) {
        return { root, name: candidate.name, rest: "" };
      }
      if (!isPathUnderWorkspace(normalizedPath, root)) return null;
      return {
        root,
        name: candidate.name,
        rest: normalizedPath.slice(root.length + 1),
      };
    })
    .filter((item): item is { root: string; name: string; rest: string } => !!item)
    .sort((left, right) => right.root.length - left.root.length);
  const matched = matches[0];
  if (!matched) return normalizedPath;
  return matched.rest ? `${matched.name}/${matched.rest}` : matched.name;
}

const workspacePathDisplayCandidates = computed(() =>
  [currentWorkspaceCandidate.value, ...workspaceListCandidates.value]
    .filter((item): item is { root: string; name: string } => !!item)
    .sort((left, right) => right.root.length - left.root.length)
);

const currentWorkspaceCandidate = computed(() => {
  const root = normalizePathForDisplay(props.currentWorkspaceRootPath);
  if (!root) return null;
  const matchedWorkspace = (Array.isArray(props.workspaces) ? props.workspaces : []).find((workspace) =>
    isSameNormalizedPath(root, normalizePathForDisplay(workspace.path))
  );
  const currentName = String(props.currentWorkspaceName || "").trim();
  const matchedName = currentName || (matchedWorkspace ? workspaceDisplayName(matchedWorkspace, root, 0) : "");
  return {
    root,
    name: matchedName || defaultWorkspaceNameFromPath(root) || root,
  };
});

const workspaceListCandidates = computed(() =>
  (Array.isArray(props.workspaces) ? props.workspaces : [])
    .map((workspace, index) => {
      const root = normalizePathForDisplay(workspace.path);
      if (!root) return null;
      return {
        root,
        name: workspaceDisplayName(workspace, root, index),
      };
    })
    .filter((item): item is { root: string; name: string } => !!item)
);

function normalizePathForDisplay(path: string) {
  return String(path || "")
    .replace(/^\\\\\?\\/, "")
    .replace(/^\/\/\?\//, "")
    .replace(/\\/g, "/")
    .replace(/\/+$/, "")
    .trim();
}

function normalizePathForCompare(path: string) {
  return normalizePathForDisplay(path).toLowerCase();
}

function isSameNormalizedPath(path: string, root: string) {
  return normalizePathForCompare(path) === normalizePathForCompare(root);
}

function isPathUnderWorkspace(path: string, root: string) {
  const normalizedPath = normalizePathForCompare(path);
  const normalizedRoot = normalizePathForCompare(root);
  return normalizedPath.startsWith(`${normalizedRoot}/`);
}

function workspaceDisplayName(workspace: ShellWorkspace, root: string, index: number) {
  const level = normalizeWorkspaceLevel(String(workspace.level || ""));
  const rawName = String(workspace.name || "").trim();
  if (!isLegacyGenericWorkspaceName(level, rawName)) {
    return rawName;
  }
  return inferWorkspaceName(level, root, index) || defaultWorkspaceNameFromPath(root) || root;
}

async function requestTaskList() {
  return invokeTauri<TaskEntry[]>("task.list", {});
}

async function loadConversationTasks() {
  taskLoading.value = true;
  taskErrorText.value = "";
  try {
    taskEntries.value = await requestTaskList();
  } catch (error) {
    taskErrorText.value = `${t("config.task.listLoadFailed")}: ${toErrorMessage(error)}`;
  } finally {
    taskLoading.value = false;
  }
}

function openTaskEditor(task: TaskEntry) {
  if (!canEditTaskInSidebar.value) return;
  if (!String(task.taskId || "").trim()) return;
  taskEditorTask.value = task;
  taskEditorOpen.value = true;
}

function closeTaskEditor() {
  taskEditorOpen.value = false;
  taskEditorTask.value = null;
}

type TaskChangedMonitorPayload = { domain?: string };

function handleTaskRefreshEvent(payload: TaskChangedMonitorPayload | null | undefined) {
  if (String(payload?.domain || "").trim() !== "task") return;
  void loadConversationTasks();
}

function handleTaskMutated(task: TaskEntry) {
  taskEditorOpen.value = false;
  taskEditorTask.value = task;
  void loadConversationTasks();
}

function isDelegateRunning(delegate: ConversationDelegateStatusSummary) {
  const status = String(delegate.status || "").trim();
  return delegate.active && (status === "running" || status === "delivered");
}

function canShowDelegateResult(delegate: ConversationDelegateStatusSummary) {
  const status = String(delegate.status || "").trim();
  if (status === "running" || status === "delivered") return false;
  return true;
}

let unlistenTaskChanged: (() => void) | null = null;
let unlistenTaskRecovered: (() => void) | null = null;

onMounted(() => {
  unlistenTaskChanged = onTransportNotification("monitor.changed", handleTaskRefreshEvent);
  unlistenTaskRecovered = onTransportRecovered(() => {
    void loadConversationTasks();
  });
});

// 任务列表懒加载：切到 tasks 标签才拉取，避免监控面板挂载时白读全局任务
watch(
  () => props.activeTab,
  (tab) => {
    if (tab === "tasks") {
      void loadConversationTasks();
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  unlistenTaskChanged?.();
  unlistenTaskChanged = null;
  unlistenTaskRecovered?.();
  unlistenTaskRecovered = null;
});
</script>

<style scoped>

.assistant-markdown :deep(.ecall-markdown-content.prose) {
  --tw-prose-body: currentColor;
  --tw-prose-headings: currentColor;
  --tw-prose-lead: currentColor;
  --tw-prose-links: var(--color-base-content);
  --tw-prose-bold: currentColor;
  --tw-prose-counters: currentColor;
  --tw-prose-bullets: color-mix(in srgb, var(--color-base-content) 50%, transparent);
  --tw-prose-hr: color-mix(in srgb, var(--color-base-content) 15%, transparent);
  --tw-prose-quotes: currentColor;
  --tw-prose-quote-borders: color-mix(in srgb, var(--color-base-content) 20%, transparent);
  --tw-prose-captions: color-mix(in srgb, var(--color-base-content) 75%, transparent);
  --tw-prose-code: currentColor;
  --tw-prose-pre-code: currentColor;
  --tw-prose-pre-bg: var(--color-base-200);
  --tw-prose-th-borders: color-mix(in srgb, var(--color-base-content) 20%, transparent);
  --tw-prose-td-borders: color-mix(in srgb, var(--color-base-content) 15%, transparent);
}

.assistant-markdown :deep(.ecall-markdown-content) {
  min-width: 0;
  max-width: 100%;
  overflow-x: hidden;
  font-family: inherit;
  font-size: var(--app-text-sm-size);
  line-height: 1.5;
}

.assistant-markdown :deep(.ecall-markdown-content .paragraph-node),
.assistant-markdown :deep(.ecall-markdown-content .heading-node),
.assistant-markdown :deep(.ecall-markdown-content .list-node),
.assistant-markdown :deep(.ecall-markdown-content .list-item),
.assistant-markdown :deep(.ecall-markdown-content .blockquote),
.assistant-markdown :deep(.ecall-markdown-content .link-node),
.assistant-markdown :deep(.ecall-markdown-content .strong-node),
.assistant-markdown :deep(.ecall-markdown-content .inline-code),
.assistant-markdown :deep(.ecall-markdown-content .table-node-wrapper),
.assistant-markdown :deep(.ecall-markdown-content .hr-node) {
  font-size: inherit;
  line-height: inherit;
}

.assistant-markdown :deep(.ecall-markdown-content.markdown-renderer) {
  content-visibility: visible !important;
  contain: none !important;
  contain-intrinsic-size: auto !important;
}

.assistant-markdown :deep(.ecall-markdown-content .markdown-renderer),
.assistant-markdown :deep(.ecall-markdown-content .node-slot),
.assistant-markdown :deep(.ecall-markdown-content .node-content),
.assistant-markdown :deep(.ecall-markdown-content .text-node) {
  font-size: inherit;
  line-height: inherit;
}

.assistant-markdown :deep(.ecall-markdown-content .code-block-container),
.assistant-markdown :deep(.ecall-markdown-content ._mermaid) {
  content-visibility: visible !important;
  contain: none !important;
  contain-intrinsic-size: auto !important;
}

.assistant-markdown :deep(.ecall-markdown-content > :first-child) {
  margin-top: 0;
}

.assistant-markdown :deep(.ecall-markdown-content > :last-child) {
  margin-bottom: 0;
}

.assistant-markdown :deep(.ecall-markdown-content :where(p,ul,ol,blockquote,pre,table,figure,.paragraph-node,.list-node,.blockquote,.table-node-wrapper,.code-block-container,._mermaid,.vmr-container)) {
  margin-top: 0.25rem;
  margin-bottom: 0.25rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(h1,h2,h3,h4,.heading-node)) {
  margin-top: 0.7rem;
  margin-bottom: 0.32rem;
  line-height: 1.5;
}

.assistant-markdown :deep(.ecall-markdown-content :where(h1,.heading-node.heading-1)) {
  font-size: var(--app-text-markdown-heading-1-size);
}

.assistant-markdown :deep(.ecall-markdown-content :where(h2,.heading-node.heading-2)) {
  font-size: var(--app-text-markdown-heading-2-size);
}

.assistant-markdown :deep(.ecall-markdown-content :where(h3,.heading-node.heading-3)) {
  font-size: var(--app-text-markdown-heading-3-size);
}

.assistant-markdown :deep(.ecall-markdown-content :where(h4,.heading-node.heading-4)) {
  font-size: var(--app-text-markdown-heading-4-size);
}

.assistant-markdown :deep(.ecall-markdown-content :where(ul,ol,.list-node)) {
  padding-left: 1.05rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(li,.list-item)) {
  margin: 0.12rem 0;
  padding-left: 0;
}

.assistant-markdown :deep(.ecall-markdown-content :where(li,.list-item) > :where(p,ul,ol,.paragraph-node,.list-node)) {
  margin-top: 0.16rem;
  margin-bottom: 0.16rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(blockquote,.blockquote)) {
  padding: 0.5rem 0.68rem 0.5rem 0.82rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(blockquote,.blockquote) .markdown-renderer),
.assistant-markdown :deep(.ecall-markdown-content :where(ul,ol,.list-node,li,.list-item) .markdown-renderer) {
  font-size: inherit;
  line-height: inherit;
}

.assistant-markdown :deep(.ecall-markdown-content :where(hr,.hr-node)) {
  margin: 0.65rem 0;
}

.assistant-markdown :deep(.ecall-markdown-content :where(:not(pre) > code,.inline-code):not(.code-block-container *)) {
  font-size: var(--app-text-xs-size);
}

.assistant-markdown :deep(.ecall-markdown-content :where(table,.table-node)) {
  font-size: var(--app-text-sm-size);
}

.assistant-markdown :deep(.ecall-markdown-content ._mermaid) {
  width: 100%;
}

.tool-review-report-markdown:deep(.code-block-container),
.tool-review-report-markdown:deep(._mermaid) {
  margin: 1rem 0;
}

.tool-review-report-markdown:deep(> :first-child) {
  margin-top: 0;
}

.tool-review-report-markdown:deep(> :last-child) {
  margin-bottom: 0;
}
</style>
