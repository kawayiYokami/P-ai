<template>
  <div :class="delegateOnly ? 'px-0 py-0' : 'rounded-box border border-base-300 bg-base-100 px-3 py-3'">
    <div v-if="!delegateOnly || selectedMessageCount > 0" class="text-xs opacity-70">{{ t("chat.selection.selectedCount", { count: selectedMessageCount }) }}</div>
    <div v-if="!delegateOnly" class="mt-3 flex flex-wrap items-center gap-2">
      <button v-if="!delegateOnly" type="button" class="btn btn-sm" :disabled="selectedMessageCount === 0" @click="emit('selectionActionBranch')">
        {{ t("chat.selection.branch") }}
      </button>
      <button
        v-if="showConversationActions && !delegateOnly"
        type="button"
        class="btn btn-sm"
        :class="{ 'btn-primary': selectionDeliverCardOpen }"
        :disabled="selectedMessageCount === 0 || selectionDeliverTargetOptions.length === 0"
        @click="openSelectionDeliverCard"
      >
        {{ t("chat.selection.forward") }}
      </button>
      <button
        type="button"
        class="btn btn-sm"
        :class="{ 'btn-primary': selectionDelegateCardOpen }"
        :disabled="delegateAgentOptions.length === 0"
        @click="openSelectionDelegateCard"
      >
        {{ t("chat.selection.delegate") }}
      </button>
      <button v-if="!delegateOnly" type="button" class="btn btn-sm" :disabled="selectedMessageCount === 0" @click="emit('selectionActionCopy')">
        {{ t("common.copy") }}
      </button>
      <button v-if="showConversationActions && !delegateOnly" type="button" class="btn btn-sm" :disabled="selectedMessageCount === 0" @click="emit('selectionActionShare', 'copyPng')">
        {{ t("chat.selection.copyImageAsImage") }}
      </button>
      <button v-if="showConversationActions && !delegateOnly" type="button" class="btn btn-sm" :disabled="selectedMessageCount === 0" @click="emit('selectionActionShare', 'png')">
        {{ t("chat.selection.saveAsImage") }}
      </button>
      <button v-if="showConversationActions && !delegateOnly" type="button" class="btn btn-sm" :disabled="selectedMessageCount === 0" @click="emit('selectionActionShare', 'html')">
        {{ t("chat.selection.saveAsHtml") }}
      </button>
      <button type="button" class="btn btn-sm btn-ghost ml-auto" @click="handleExitSelectionMode">
        {{ t("common.cancel") }}
      </button>
    </div>

    <div v-if="showConversationActions && !delegateOnly && selectionDeliverCardOpen" class="mt-3 rounded-box border border-base-300 bg-base-200/50 px-3 py-3">
      <div class="text-sm font-medium">{{ t("chat.selection.forward") }}</div>
      <div class="mt-1 text-xs opacity-70">{{ t("chat.selection.forwardHint") }}</div>
      <select v-model="selectionDeliverTargetKey" class="select select-bordered select-sm mt-3 w-full" :disabled="selectionDeliverTargetOptions.length === 0">
        <option v-for="item in selectionDeliverTargetOptions" :key="item.targetKey" :value="item.targetKey">
          {{ selectionDeliverOptionLabel(item) }}
        </option>
      </select>
      <div class="mt-3 flex items-center justify-end gap-2">
        <button type="button" class="btn btn-sm" @click="closeSelectionDeliverCard">{{ t("common.cancel") }}</button>
        <button type="button" class="btn btn-sm btn-primary" :disabled="!selectionDeliverTargetKey" @click="confirmSelectionDeliver">
          {{ t("chat.selection.confirmForward") }}
        </button>
      </div>
    </div>

    <div
      v-if="selectionDelegateCardOpen"
      :class="[
        delegateOnly && selectedMessageCount > 0 ? 'mt-2' : '',
        !delegateOnly ? 'mt-3' : '',
        'rounded-box border border-base-300 px-3 py-3',
        delegateOnly ? 'bg-base-100' : 'bg-base-200/50',
      ]"
    >
      <div class="flex items-center justify-between gap-3">
        <div>
          <div class="text-sm font-medium">{{ t("chat.selection.asyncDelegate") }}</div>
          <div class="mt-1 text-xs opacity-70">{{ t("chat.selection.delegateHint") }}</div>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <button type="button" class="btn btn-sm btn-ghost" @click="clearSelectionDelegateFields">{{ t("common.clear") }}</button>
        </div>
      </div>
      <div v-if="recentDelegateRequests.length > 0" class="mt-3 flex flex-wrap gap-2">
        <button v-for="item in recentDelegateRequests" :key="item.id" type="button" class="btn btn-xs max-w-full justify-start" :title="item.goal" @click="applyRecentDelegateRequest(item)">
          <span class="max-w-52 truncate">{{ item.label }}</span>
        </button>
      </div>
      <AgentPersonaSelect
        v-model:agent-id="selectionDelegateAgentId"
        class="mt-3"
        :options="delegateAgentOptions"
        :persona-avatar-url-map="personaAvatarUrlMap"
        auto-select-first
      />
      <label class="mt-3 grid gap-1">
        <span class="text-xs opacity-70">{{ t("chat.selection.delegateGoalLabel") }}</span>
        <textarea v-model="selectionDelegateGoal" class="textarea textarea-bordered min-h-24 w-full resize-y text-sm" :placeholder="t('chat.selection.goalPlaceholder')" maxlength="10000"></textarea>
      </label>
      <div class="mt-2 grid grid-cols-2 gap-2">
        <label class="grid min-w-0 content-start gap-1">
          <span class="text-xs opacity-70">{{ t("chat.selection.delegateWhyLabel") }}</span>
          <textarea v-model="selectionDelegateWhy" class="textarea textarea-bordered min-h-20 w-full resize-y text-sm" :placeholder="t('chat.selection.whyPlaceholder')" maxlength="5000"></textarea>
        </label>
        <label class="grid min-w-0 content-start gap-1">
          <span class="text-xs opacity-70">{{ t("chat.selection.delegateTodoLabel") }}</span>
          <textarea v-model="selectionDelegateTodo" class="textarea textarea-bordered min-h-20 w-full resize-y text-sm" :placeholder="t('chat.selection.todoPlaceholder')" maxlength="5000"></textarea>
        </label>
      </div>
      <div class="mt-3 flex items-center justify-end gap-2">
        <button type="button" class="btn btn-sm" @click="cancelSelectionDelegate">{{ t("common.cancel") }}</button>
        <button type="button" class="btn btn-sm btn-primary" :disabled="!canSubmitSelectionDelegate" @click="confirmSelectionDelegate">
          {{ t("chat.selection.delegate") }}
        </button>
      </div>
    </div>

  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { ChatConversationOverviewItem, ConversationForwardTarget, RemoteImContactConversationOption } from "../../../types/app";
import AgentPersonaSelect from "../../shared/components/AgentPersonaSelect.vue";
import type { AgentPersonaOption } from "../../shared/agent-persona-options";
import { resolveConversationDisplayTitle } from "../utils/conversation-title";

type ConversationAgentOption = AgentPersonaOption;

type RecentDelegateRequest = {
  id: string;
  label: string;
  agentId: string;
  presetId: string;
  why: string;
  goal: string;
  todo: string;
};

const props = defineProps<{
  delegateOnly?: boolean;
  showConversationActions?: boolean;
  selectedMessageCount: number;
  activeConversationId: string;
  unarchivedConversationItems: ChatConversationOverviewItem[];
  remoteImContactConversations: RemoteImContactConversationOption[];
  createConversationAgentOptions: ConversationAgentOption[];
  personaAvatarUrlMap?: Record<string, string>;
  activeAgentId?: string;
}>();

const emit = defineEmits<{
  exitSelectionMode: [];
  selectionActionBranch: [];
  selectionActionForward: [target: ConversationForwardTarget];
  selectionActionDelegate: [payload: { agentId: string; presetId: string; why: string; goal: string; todo: string }];
  selectionActionCopy: [];
  selectionActionShare: [format: "html" | "png" | "copyPng"];
}>();

const { t, locale } = useI18n();
const delegateOnly = computed(() => !!props.delegateOnly);
const showConversationActions = computed(() => props.showConversationActions ?? true);
const USER_ASYNC_DELEGATE_RECENT_STORAGE_KEY = "easy_call.user_async_delegate_recent.v1";
const USER_ASYNC_DELEGATE_RECENT_LIMIT = 3;

const selectionDeliverCardOpen = ref(false);
const selectionDeliverTargetKey = ref("");
const selectionDelegateCardOpen = ref(false);
const selectionDelegateAgentId = ref("");
const selectionDelegatePresetId = ref("review");
const selectionDelegateWhy = ref("");
const selectionDelegateGoal = ref("");
const selectionDelegateTodo = ref("");
const recentDelegateRequests = ref<RecentDelegateRequest[]>([]);

const selectionDeliverTargetOptions = computed(() => {
  const activeConversationId = String(props.activeConversationId || "").trim();
  const localTargets = (Array.isArray(props.unarchivedConversationItems) ? props.unarchivedConversationItems : [])
    .filter((item) => String(item.conversationId || "").trim() !== activeConversationId)
    .filter((item) => !item.isSystemNotificationConversation)
    .filter((item) => String(item.kind || "local_unarchived").trim() !== "remote_im_contact")
    .map((item) => ({
      targetKey: `local:${String(item.conversationId || "").trim()}`,
      target: {
        kind: "local_unarchived" as const,
        conversationId: String(item.conversationId || "").trim(),
      },
      title: resolveConversationDisplayTitle(item, {
        locale: locale.value,
        untitledLabel: t("chat.untitledConversation"),
      }),
      runtimeState: item.runtimeState,
    }))
    .filter((item) => !!item.target.conversationId);
  const remoteTargets = (Array.isArray(props.remoteImContactConversations) ? props.remoteImContactConversations : [])
    .filter((item) => String(item.conversationId || "").trim() !== activeConversationId)
    .map((item) => ({
      targetKey: `remote:${String(item.contactId || "").trim()}:${String(item.conversationId || "").trim()}`,
      target: {
        kind: "remote_im_contact" as const,
        conversationId: String(item.conversationId || "").trim(),
        remoteContactId: String(item.contactId || "").trim(),
      },
      title: String(item.title || "").trim() || String(item.contactDisplayName || "").trim(),
      remoteContactName: String(item.contactDisplayName || "").trim() || undefined,
      channelName: String(item.channelName || "").trim() || undefined,
    }))
    .filter((item) => !!item.target.conversationId && !!item.target.remoteContactId);
  return [...localTargets, ...remoteTargets];
});

const delegateAgentOptions = computed(() => {
  const sourceAgentId = String(props.activeAgentId || "").trim();
  // 用户主动发起异步委托不受 AI delegate 工具的“直接下级人格”限制，
  // 但禁止委托给自己：异步委托给同一人格只能走同步路径。
  return (Array.isArray(props.createConversationAgentOptions) ? props.createConversationAgentOptions : [])
    .map((item) => ({
      id: String(item.id || "").trim(),
      agentId: String(item.agentId || "").trim(),
      agentName: String(item.agentName || "").trim(),
      label: String(item.label || "").trim(),
      name: String(item.name || "").trim() || String(item.id || "").trim(),
      ownerAgentId: String(item.ownerAgentId || item.agentId || "").trim(),
      ownerName: String(item.ownerName || "").trim(),
      providerName: String(item.providerName || "").trim() || undefined,
      modelName: String(item.modelName || "").trim() || undefined,
      apiConfigId: String(item.apiConfigId || "").trim() || undefined,
      modelMissing: !!item.modelMissing,
      personaMissing: !!item.personaMissing,
      unavailable: !!item.unavailable,
      childAgentIds: Array.isArray(item.childAgentIds) ? item.childAgentIds : [],
    }))
    .filter((item) => !!item.id && !!item.agentId)
    .filter((item) => !sourceAgentId || String(item.agentId || "").trim() !== sourceAgentId);
});

const preferredDelegateAgentId = computed(() => String(delegateAgentOptions.value[0]?.id || "").trim());
const canSubmitSelectionDelegate = computed(() =>
  delegateAgentOptions.value.some((agent) =>
    agent.agentId === String(selectionDelegateAgentId.value || "").trim()
    && !agent.personaMissing
  )
  && !!String(selectionDelegateGoal.value || "").trim(),
);

function selectionDeliverOptionLabel(item: {
  target: ConversationForwardTarget;
  title: string;
  runtimeState?: ChatConversationOverviewItem["runtimeState"];
  remoteContactName?: string;
  channelName?: string;
}): string {
  const parts = [String(item.title || "").trim() || t('chat.selection.unnamedConversation')];
  if (item.target.kind === "remote_im_contact") {
    const remoteContactName = String(item.remoteContactName || "").trim();
    const channelName = String(item.channelName || "").trim();
    if (remoteContactName) parts.push(remoteContactName);
    if (channelName) parts.push(channelName);
  } else {
    if (item.runtimeState === "assistant_streaming") parts.push(t('chat.selection.streaming'));
    if (item.runtimeState === "organizing_context") parts.push(t('chat.selection.organizing'));
  }
  return parts.join(" / ");
}

function openSelectionDeliverCard() {
  if (delegateOnly.value) return;
  if (selectionDeliverTargetOptions.value.length === 0) return;
  closeSelectionDelegateCard();
  const currentTargetKey = String(selectionDeliverTargetKey.value || "").trim();
  const hasValidTarget = selectionDeliverTargetOptions.value.some((item) => item.targetKey === currentTargetKey);
  if (!currentTargetKey || !hasValidTarget) {
    selectionDeliverTargetKey.value = selectionDeliverTargetOptions.value[0]?.targetKey || "";
  }
  selectionDeliverCardOpen.value = true;
}

function closeSelectionDeliverCard() {
  selectionDeliverCardOpen.value = false;
}

function confirmSelectionDeliver() {
  const targetKey = String(selectionDeliverTargetKey.value || "").trim();
  const target = selectionDeliverTargetOptions.value.find((item) => item.targetKey === targetKey)?.target;
  if (!target?.conversationId) return;
  closeSelectionDeliverCard();
  emit("selectionActionForward", target);
}

function normalizeRecentDelegateRequest(raw: unknown): RecentDelegateRequest | null {
  const item = raw as (Partial<RecentDelegateRequest> & {
    background?: string;
    question?: string;
    focus?: string;
  }) | null;
  if (!item) return null;
  const agentId = String(item.agentId || "").trim();
  const goal = String(item.goal || item.question || "").trim();
  const todo = String(item.todo || item.focus || "").trim();
  if (!agentId || !goal) return null;
  const presetId = String(item.presetId || "review").trim() || "review";
  const label = String(item.label || goal).trim() || goal;
  return {
    id: String(item.id || `${agentId}:${presetId}:${goal}`).trim(),
    label,
    agentId,
    presetId,
    why: String(item.why || item.background || "").trim(),
    goal,
    todo,
  };
}

function saveRecentDelegateRequests() {
  try {
    window.localStorage.setItem(USER_ASYNC_DELEGATE_RECENT_STORAGE_KEY, JSON.stringify(recentDelegateRequests.value));
  } catch {
    // ignore persistence failures
  }
}

function loadRecentDelegateRequests() {
  try {
    const raw = window.localStorage.getItem(USER_ASYNC_DELEGATE_RECENT_STORAGE_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return;
    recentDelegateRequests.value = parsed
      .map((item) => normalizeRecentDelegateRequest(item))
      .filter((item): item is RecentDelegateRequest => !!item)
      .slice(0, USER_ASYNC_DELEGATE_RECENT_LIMIT);
  } catch {
    recentDelegateRequests.value = [];
  }
}

function rememberDelegateRequest(raw: Omit<RecentDelegateRequest, "id" | "label">) {
  const request = normalizeRecentDelegateRequest({ ...raw, id: `${Date.now()}:${raw.agentId}`, label: raw.goal });
  if (!request) return;
  const key = `${request.agentId}\n${request.presetId}\n${request.why}\n${request.goal}\n${request.todo}`;
  recentDelegateRequests.value = [
    request,
    ...recentDelegateRequests.value.filter((item) => `${item.agentId}\n${item.presetId}\n${item.why}\n${item.goal}\n${item.todo}` !== key),
  ].slice(0, USER_ASYNC_DELEGATE_RECENT_LIMIT);
  saveRecentDelegateRequests();
}

function clearSelectionDelegateFields() {
  selectionDelegatePresetId.value = "review";
  selectionDelegateWhy.value = "";
  selectionDelegateGoal.value = "";
  selectionDelegateTodo.value = "";
}

function applyRecentDelegateRequest(item: RecentDelegateRequest) {
  const optionStillExists = delegateAgentOptions.value.some((agent) =>
    agent.agentId === item.agentId
  );
  if (optionStillExists) {
    selectionDelegateAgentId.value = item.agentId;
  }
  selectionDelegatePresetId.value = item.presetId || "review";
  selectionDelegateWhy.value = item.why;
  selectionDelegateGoal.value = item.goal;
  selectionDelegateTodo.value = item.todo;
}

function openSelectionDelegateCard() {
  closeSelectionDeliverCard();
  const preferredOption = delegateAgentOptions.value.find((option) => option.id === preferredDelegateAgentId.value)
    || delegateAgentOptions.value[0];
  if (preferredOption) {
    selectionDelegateAgentId.value = preferredOption.agentId;
  }
  selectionDelegateCardOpen.value = true;
}

function closeSelectionDelegateCard() {
  selectionDelegateCardOpen.value = false;
}

function cancelSelectionDelegate() {
  if (delegateOnly.value) {
    handleExitSelectionMode();
    return;
  }
  closeSelectionDelegateCard();
}

function confirmSelectionDelegate() {
  if (!canSubmitSelectionDelegate.value) return;

  // 限制每个字段的最大长度，防止崩溃
  const MAX_GOAL_LENGTH = 10000;
  const MAX_WHY_LENGTH = 5000;
  const MAX_TODO_LENGTH = 5000;

  const rawGoal = String(selectionDelegateGoal.value || "").trim();
  const rawWhy = String(selectionDelegateWhy.value || "").trim();
  const rawTodo = String(selectionDelegateTodo.value || "").trim();

  const payload = {
    agentId: String(selectionDelegateAgentId.value || "").trim(),
    presetId: String(selectionDelegatePresetId.value || "review").trim() || "review",
    why: rawWhy.length > MAX_WHY_LENGTH ? rawWhy.slice(0, MAX_WHY_LENGTH) : rawWhy,
    goal: rawGoal.length > MAX_GOAL_LENGTH ? rawGoal.slice(0, MAX_GOAL_LENGTH) : rawGoal,
    todo: rawTodo.length > MAX_TODO_LENGTH ? rawTodo.slice(0, MAX_TODO_LENGTH) : rawTodo,
  };
  rememberDelegateRequest(payload);
  closeSelectionDelegateCard();
  emit("selectionActionDelegate", payload);
}

function handleExitSelectionMode() {
  closeSelectionDeliverCard();
  closeSelectionDelegateCard();
  emit("exitSelectionMode");
}

function syncDelegateOnlyPanel() {
  if (delegateOnly.value) {
    openSelectionDelegateCard();
  }
}

onMounted(() => {
  loadRecentDelegateRequests();
  syncDelegateOnlyPanel();
});

watch(delegateOnly, syncDelegateOnlyPanel);
</script>
