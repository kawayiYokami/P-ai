<template>
  <aside v-bind="rootAttrs" class="w-full flex h-full min-h-0 flex-col bg-base-200">
    <div ref="contentScroller" class="ecall-chat-scroll-container flex min-h-0 flex-1 flex-col overflow-y-auto">
      <div v-if="errorText" class="mx-4 my-4 rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
        {{ errorText }}
      </div>

      <template v-if="activeTab === 'tools' && currentBatch">
        <DiffFlowView
          ref="diffFlowRef"
          class="min-h-0 flex-1"
          :items="diffFlowItems"
          @open-file="emit('openFile', $event)"
        >
          <!-- 非文件类工具调用：没有 diff 可铺，仍按工具名折叠在文件改动之后 -->
          <div v-if="nonFileReviewGroups.length > 0" class="flex flex-col py-2">
            <CollapsibleGroup
              v-for="group in nonFileReviewGroups"
              :key="group.key"
              :title="group.title"
              :count="group.items.length"
              :model-value="isToolAssessmentSectionCollapsed(group.key)"
              @update:model-value="toggleToolAssessmentSection(group.key)"
              @collapse-all="collapseAllToolAssessmentSections"
            >
              <div v-if="!isToolAssessmentSectionCollapsed(group.key)">
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
              </div>
            </CollapsibleGroup>
          </div>
        </DiffFlowView>
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

      <template v-else-if="activeTab === 'timers'">
        <div v-if="timerErrorText" class="mx-4 my-4 rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
          {{ timerErrorText }}
        </div>
        <div v-else-if="timerLoading && sortedTimers.length === 0" class="flex min-h-0 flex-1 items-center justify-center px-4 py-8">
          <AppLoading size="sm" class="text-base-content/45" />
        </div>
        <div v-else-if="sortedTimers.length === 0" class="flex min-h-0 flex-1 items-center justify-center px-4 py-8 text-sm text-base-content/65">
          {{ t("chat.toolReview.timerEmpty") }}
        </div>
        <div v-else>
          <TimerListItem
            v-for="timer in sortedTimers"
            :key="timer.id"
            :timer="timer"
            @click="openTimerEditor(timer)"
          />
        </div>
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
      <div class="flex items-center justify-center gap-3">
        <button
          type="button"
          class="btn btn-sm bg-base-100 text-xs hover:bg-base-100"
          :title="diffFlowRef?.allCollapsed ? t('chat.toolReview.expandAll') : t('chat.toolReview.collapseAll')"
          @click="diffFlowRef?.toggleAll()"
        >
          {{ diffFlowRef?.allCollapsed ? t("chat.toolReview.expandAll") : t("chat.toolReview.collapseAll") }}
        </button>
        <div class="join">
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
    </div>
    <FloatingScrollbar :target="contentScroller" />
  </aside>

  <dialog ref="delegateResultDialogRef" class="modal" @close="closeDelegateResultDialog" @cancel.prevent="closeDelegateResultDialog">
    <div class="modal-box flex max-h-[80vh] max-w-2xl flex-col overflow-hidden p-0">
      <div class="flex shrink-0 items-center justify-between gap-3 border-b border-base-300 px-5 py-3">
        <div class="min-w-0 truncate text-sm font-semibold text-base-content">{{ delegateResultTitle }}</div>
        <button type="button" class="btn btn-ghost btn-sm gap-1 shrink-0" @click="closeDelegateResultDialog"><span class="text-base leading-none">×</span><span>关闭</span></button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto bg-base-200 px-5 py-4">
        <DelegateScheduleTimeline
          v-if="delegateResultConversationId"
          :conversation-id="delegateResultConversationId"
          :auto-refresh-key="delegateResultTimelineKey"
          :status-snapshot="delegateResultStatusSnapshot"
        />
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="closeDelegateResultDialog">close</button>
    </form>
  </dialog>

  <TimerCreateCard
    :open="timerEditorOpen"
    mode="edit"
    :timer="timerEditorTimer"
    @close="closeTimerEditor"
    @saved="handleTimerSaved"
    @enabled-change="handleTimerEnabledChange"
    @deleted="handleTimerDeleted"
  />

</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useAttrs, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppLoading from "../../shared/components/AppLoading.vue";
import { invokeTauri, onTransportNotification, onTransportRecovered } from "../../../services/tauri-api";
import type { ArchiveBlockPage, BackgroundShellTaskSummary, ChatMessage, ConversationDelegateStatusSummary, ShellWorkspace } from "../../../types/app";
import { toErrorMessage } from "../../../utils/error";
import { defaultWorkspaceNameFromPath, inferWorkspaceName, isLegacyGenericWorkspaceName, normalizeWorkspaceLevel } from "../../../utils/shell-workspaces";
import type { ToolReviewBatchSummary, ToolReviewItemDetail, ToolReviewItemSummary, ToolReviewSegment } from "../composables/use-chat-tool-review";
import { groupSegmentsByFile, segmentDiffStats } from "../composables/tool-review-segments";
import { formatConversationListTimeWithMinuteDetails } from "../utils/conversation-time";
import { AppMarkdownRenderer, initKatex } from "../markdown";
import ToolAssessmentCard from "./ToolAssessmentCard.vue";
import DelegateCard from "./DelegateCard.vue";
import DelegateScheduleTimeline from "./DelegateScheduleTimeline.vue";
import FloatingScrollbar from "../../shell/components/FloatingScrollbar.vue";
import CollapsibleGroup from "./CollapsibleGroup.vue";
import DiffFlowView from "./DiffFlowView.vue";
import type { DiffFlowItem } from "./diff-flow-types";
import TimerListItem from "./TimerListItem.vue";
import TimerCreateCard from "./dialogs/TimerCreateCard.vue";
import FastRequestTurnsPanel from "./FastRequestTurnsPanel.vue";
import type { TimerEntry } from "../utils/timer-entry";

initKatex();

type ToolReviewSidebarTab = "tools" | "delegates" | "timers" | "fastRequests";

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
  (e: "openFile", path: string): void;
  (e: "timerSaved"): void;
}>();

const { t, locale } = useI18n();
const contentScroller = ref<HTMLElement | null>(null);
/** 变更列表的折叠开关由 DiffFlowView 自己维护，这里只借它的整体切换按钮 */
const diffFlowRef = ref<InstanceType<typeof DiffFlowView> | null>(null);
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
const collapsedDelegateSectionKeys = ref<Record<string, boolean>>({
  running: false,
  completed: true,
  interrupted: true,
  failed: true,
});
const delegateResultConversationId = ref("");
const delegateResultTimelineKey = ref("");
const delegateResultStatusSnapshot = ref<import("../../../types/app").ConversationDelegateStatusSummary | null>(null);
const timerEntries = ref<TimerEntry[]>([]);
const timerLoading = ref(false);
const timerErrorText = ref("");
const timerEditorOpen = ref(false);
const timerEditorTimer = ref<TimerEntry | null>(null);

type DelegateStatusSection = {
  key: string;
  title: string;
  items: ConversationDelegateStatusSummary[];
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

/** 监控面板只列本会话的定时器，筛选在后端完成（timer.list 带 conversationId） */
const sortedTimers = computed(() => timerEntries.value.slice().sort((left, right) => {
  const leftTime = Date.parse(String(left.nextRunAt || "").trim());
  const rightTime = Date.parse(String(right.nextRunAt || "").trim());
  const leftValue = Number.isFinite(leftTime) ? leftTime : Number.POSITIVE_INFINITY;
  const rightValue = Number.isFinite(rightTime) ? rightTime : Number.POSITIVE_INFINITY;
  if (leftValue !== rightValue) return leftValue - rightValue;
  return String(left.createdAt || "").localeCompare(String(right.createdAt || ""));
}));
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
  delegateResultStatusSnapshot.value = status;
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

/**
 * 有 diff 可铺的改动段：交给 DiffFlowView 平铺，一段一项、按工具调用顺序排。
 * 不按文件聚合——同一文件改了几次就平铺几项，序号与实际改动顺序对得上。
 */
const diffFlowItems = computed<DiffFlowItem[]>(() =>
  (props.segmentMap[props.currentBatchKey] ?? [])
    .filter((segment) => (segment.diffLines ?? []).length > 0)
    .map((segment, index) => {
      const stats = segmentDiffStats(segment);
      return {
        key: `${index}:${segment.path}`,
        path: formatPatchGroupTitle(segment.path),
        added: stats.add,
        removed: stats.remove,
        lines: segment.diffLines,
        sourcePath: segment.path,
      };
    }),
);

/** 没有 diff 的工具调用：仍按工具名折叠在文件改动之后 */
const nonFileReviewGroups = computed(() =>
  reviewGroups.value.filter((group) => group.segments.length === 0),
);

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

/** 只取本会话的定时器；筛选在后端做，切会话即换一份列表 */
async function loadTimers() {
  timerLoading.value = true;
  timerErrorText.value = "";
  try {
    timerEntries.value = await invokeTauri<TimerEntry[]>("timer.list", {
      conversationId: String(props.activeConversationId || "").trim(),
    });
  } catch (error) {
    timerErrorText.value = `${t("config.timer.listLoadFailed")}: ${toErrorMessage(error)}`;
  } finally {
    timerLoading.value = false;
  }
}

function openTimerEditor(timer: TimerEntry) {
  if (!String(timer.id || "").trim()) return;
  timerEditorTimer.value = timer;
  timerEditorOpen.value = true;
}

function closeTimerEditor() {
  timerEditorOpen.value = false;
  timerEditorTimer.value = null;
}

type TimerChangedMonitorPayload = { domain?: string };

function handleTimerRefreshEvent(payload: TimerChangedMonitorPayload | null | undefined) {
  if (String(payload?.domain || "").trim() !== "timer") return;
  void loadTimers();
}

function handleTimerSaved() {
  timerEditorOpen.value = false;
  timerEditorTimer.value = null;
  emit("timerSaved");
  void loadTimers();
}

/** 只拨了启用开关：刷新这一条，弹窗继续开着 */
function handleTimerEnabledChange(updated: TimerEntry) {
  timerEditorTimer.value = updated;
  emit("timerSaved");
  void loadTimers();
}

function handleTimerDeleted() {
  timerEditorOpen.value = false;
  timerEditorTimer.value = null;
  emit("timerSaved");
  void loadTimers();
}

function isDelegateRunning(delegate: ConversationDelegateStatusSummary) {
  const status = String(delegate.status || "").trim();
  return delegate.active && (status === "running" || status === "delivered");
}

function canShowDelegateResult(delegate: ConversationDelegateStatusSummary) {
  // 运行中也能点开：弹窗里的时间线自己会在运行期刷新，这正是「委托跑到哪一步」的入口。
  const conversationId = String(delegate?.conversationId || delegate?.delegateId || "").trim();
  return !!conversationId;
}

let unlistenTimerChanged: (() => void) | null = null;
let unlistenTimerRecovered: (() => void) | null = null;

onMounted(() => {
  unlistenTimerChanged = onTransportNotification("monitor.changed", handleTimerRefreshEvent);
  unlistenTimerRecovered = onTransportRecovered(() => {
    void loadTimers();
  });
});

// 定时器列表懒加载：切到 timers 标签才拉取，避免监控面板挂载时白读全局定时器；
// 切换会话后本会话的定时器也变了，需要重新拉。
watch(
  () => [props.activeTab, props.activeConversationId] as const,
  ([tab]) => {
    if (tab === "timers") {
      void loadTimers();
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  unlistenTimerChanged?.();
  unlistenTimerChanged = null;
  unlistenTimerRecovered?.();
  unlistenTimerRecovered = null;
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
