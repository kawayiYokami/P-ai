<template>
  <dialog ref="dialogRef" class="modal !items-start overflow-y-auto overflow-x-hidden pt-[max(1rem,env(safe-area-inset-top))] pb-[max(1rem,env(safe-area-inset-bottom))] sm:!items-center sm:py-6" @close="onDialogClose" @cancel.prevent="onDialogClose">
    <div class="modal-box mx-auto flex max-h-[calc(100dvh-max(2rem,env(safe-area-inset-top)+env(safe-area-inset-bottom)))] w-[88vw] max-w-4xl flex-col overflow-hidden p-0">
      <div class="relative z-20 shrink-0 border-b border-base-300 px-5 py-3">
        <SegmentedControl
          :model-value="panel"
          :options="launchPanelOptions"
          size="sm"
          :full-width="false"
          @update:model-value="setPanel"
        />
      </div>
      <div class="relative z-20 shrink-0 overflow-visible px-5 pt-4">
        <div class="grid gap-1.5">
          <div class="text-xs font-medium text-base-content/60">{{ t("chat.toolReview.agentLabel") }}</div>
          <AgentPersonaSelect
            v-model:agent-id="selectedAgentId"
            :api-config-id="selectedApiConfigId"
            :options="agentSelectOptions"
            :api-configs="apiConfigs"
            :persona-avatar-url-map="personaAvatarUrlMap"
            show-model
            auto-select-first
            @update:api-config-id="setSelectedApiConfigId"
          />
        </div>
      </div>
      <div class="relative z-0 min-h-0 flex-1 px-5 py-4" :class="panel === 'review' && scope === 'commit' ? 'flex flex-col overflow-hidden' : 'overflow-y-auto'">
        <template v-if="panel === 'delegate'">
          <div v-if="recentGoals.length > 0" class="mb-3 flex flex-wrap gap-2">
            <button v-for="item in recentGoals" :key="item.id" type="button" class="btn btn-xs max-w-full justify-start" :title="item.goal" @click="applyRecentGoal(item)">
              <span class="max-w-52 truncate">{{ item.goal }}</span>
            </button>
          </div>
          <label class="grid gap-1">
            <span class="text-xs font-medium text-base-content/60">{{ t("chat.selection.delegateGoalLabel") }}</span>
            <textarea v-model="goalText" class="textarea textarea-bordered min-h-32 w-full resize-y text-sm" :placeholder="t('chat.selection.goalPlaceholder')" maxlength="10000"></textarea>
          </label>
        </template>
        <template v-else>
          <div role="tablist" class="tabs tabs-border mb-4 flex-wrap">
            <button type="button" role="tab" class="tab" :class="{ 'tab-active': scope === 'commit' }" @click="setScope('commit')">{{ t("chat.toolReview.scopeCommit") }}</button>
            <button type="button" role="tab" class="tab" :class="{ 'tab-active': scope === 'main' }" @click="setScope('main')">{{ t("chat.toolReview.scopeMain") }}</button>
            <button type="button" role="tab" class="tab" :class="{ 'tab-active': scope === 'uncommitted' }" @click="setScope('uncommitted')">{{ t("chat.toolReview.scopeUncommitted") }}</button>
            <button type="button" role="tab" class="tab" :class="{ 'tab-active': scope === 'custom' }" @click="setScope('custom')">{{ t("chat.toolReview.scopeCustom") }}</button>
          </div>
          <div v-if="scope === 'commit'" class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-box border border-base-300 bg-base-100">
            <div class="flex shrink-0 items-center justify-between gap-3 border-b border-base-300 bg-base-100 px-4 py-3 text-sm">
              <button type="button" class="btn btn-sm shrink-0" :disabled="commitOptionsLoading || commitPage <= 1" @click="requestCommitPage(commitPage - 1)">上一页</button>
              <span class="min-w-0 flex-1 text-center text-base-content/70">第 {{ commitPage }} 页 / 共 {{ commitTotalPages }} 页</span>
              <button type="button" class="btn btn-sm shrink-0" :disabled="commitOptionsLoading || commitPage >= commitTotalPages" @click="requestCommitPage(commitPage + 1)">下一页</button>
            </div>
            <div class="min-h-0 flex-1 overflow-y-auto overscroll-contain">
              <div v-if="commitOptionsLoading" class="px-4 py-3 text-sm text-base-content/70">{{ t("chat.toolReview.commitPickerLoading") }}</div>
              <div v-else-if="commitOptions.length === 0" class="px-4 py-3 text-sm text-base-content/70">{{ t("chat.toolReview.commitPickerEmpty") }}</div>
              <button
                v-for="item in commitOptions"
                :key="item.hash"
                type="button"
                class="flex w-full items-start gap-3 border-b border-base-300 px-4 py-3 text-left last:border-b-0 hover:bg-base-200"
                @click="toggleCommitSelection(item.hash)"
              >
                <input type="checkbox" class="checkbox checkbox-sm mt-1" :checked="selectedCommitHashes.includes(item.hash)" tabindex="-1">
                <div class="min-w-0 flex-1 text-sm text-base-content">{{ item.subject }}</div>
              </button>
            </div>
          </div>
          <div v-else-if="scope === 'custom'">
            <textarea
              v-model="customTargetText"
              class="textarea textarea-bordered h-40 w-full"
              :placeholder="t('chat.toolReview.customDialogPlaceholder')"
            ></textarea>
          </div>
          <div v-else class="rounded-box border border-base-300 px-4 py-3 text-sm text-base-content/70">
            {{ scope === 'main' ? t('chat.toolReview.scopeMain') : t('chat.toolReview.scopeUncommitted') }}
          </div>
        </template>
        <div v-if="errorText" class="mt-3 rounded border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
          {{ errorText }}
        </div>
      </div>
      <div class="flex shrink-0 items-center justify-end gap-3 border-t border-base-300 px-5 py-4 pb-[max(1rem,env(safe-area-inset-bottom))]">
        <button type="button" class="btn" :disabled="submitting || delegating" @click="close">{{ t("common.cancel") }}</button>
        <button type="button" class="btn btn-primary" :disabled="!canConfirm" @click="confirm">{{ t("common.confirm") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDialogClose">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem } from "../../../types/app";
import AgentPersonaSelect from "../../shared/components/AgentPersonaSelect.vue";
import SegmentedControl, { type SegmentedControlOption } from "../../config/components/SegmentedControl.vue";
import type { ToolReviewCodeReviewScope, ToolReviewCommitOption } from "../composables/use-chat-tool-review";
import type { AgentPersonaOption } from "../../shared/agent-persona-options";
import { isModelRoleApiConfigId } from "../../config/utils/model-role-options";

type LaunchPanel = "delegate" | "review";

type RecentGoal = {
  id: string;
  agentId: string;
  goal: string;
};

const RECENT_STORAGE_KEY = "easy_call.user_async_delegate_recent.v1";
const RECENT_LIMIT = 3;

const props = defineProps<{
  open: boolean;
  initialPanel: LaunchPanel;
  submitting: boolean;
  delegating?: boolean;
  errorText: string;
  currentAgentId: string;
  /** 当前会话正在用的模型：打开对话卡片时模型选择器的默认值 */
  currentApiConfigId?: string;
  agentOptions: AgentPersonaOption[];
  apiConfigs?: ApiConfigItem[];
  personaAvatarUrlMap?: Record<string, string>;
  commitOptions: ToolReviewCommitOption[];
  commitOptionsLoading: boolean;
  commitTotal: number;
  commitPage: number;
  commitPageSize: number;
  /** 会话或工作树一变，旧 commit 就不能再留 */
  commitSourceKey?: string;
}>();

const emit = defineEmits<{
  close: [];
  pickCommitReview: [page: number];
  panelChange: [panel: LaunchPanel];
  reviewCode: [input: { scope: ToolReviewCodeReviewScope; target?: string; agentId: string; apiConfigId?: string }];
  delegate: [input: { agentId: string; goal: string; apiConfigId?: string }];
}>();

const { t } = useI18n();
const dialogRef = ref<HTMLDialogElement | null>(null);
const panel = ref<LaunchPanel>("review");

const launchPanelOptions = computed<Array<SegmentedControlOption<LaunchPanel>>>(() => [
  { value: "delegate", label: t("chat.conversationMenu.groupDelegate") },
  { value: "review", label: t("chat.toolReview.generateReviewReport") },
]);

/** 面板切换同时上报，由宿主记住最后一次选择，下次打开沿用 */
function setPanel(value: LaunchPanel) {
  if (panel.value === value) return;
  panel.value = value;
  emit("panelChange", value);
}
const selectedAgentId = ref("");
const selectedApiConfigId = ref("");
const selectedCommitHashes = ref<string[]>([]);
const customTargetText = ref("");
const goalText = ref("");
const scope = ref<ToolReviewCodeReviewScope>("main");
const recentGoals = ref<RecentGoal[]>([]);

const apiConfigs = computed(() => (Array.isArray(props.apiConfigs) ? props.apiConfigs : []));

/** 卡片默认模型：当前会话正在用的模型。哨兵值不是模型，不能出现在选择器里。 */
function conversationDefaultApiConfigId(): string {
  const current = String(props.currentApiConfigId || "").trim();
  return isModelRoleApiConfigId(current) ? "" : current;
}

/** 任何来源（人格自动选择、最近目标）写回的模型都先过一遍哨兵拦截 */
function setSelectedApiConfigId(value: string) {
  const next = String(value || "").trim();
  selectedApiConfigId.value = isModelRoleApiConfigId(next) ? "" : next;
}

function onDialogClose() {
  if (props.submitting || props.delegating) {
    const dialog = dialogRef.value;
    if (dialog && !dialog.open && props.open) dialog.showModal();
    return;
  }
  close();
}

function syncDialog() {
  const dialog = dialogRef.value;
  if (!dialog) return;
  if (props.open) {
    if (!dialog.open) dialog.showModal();
  } else if (dialog.open) dialog.close();
}

watch(() => props.open, syncDialog);
watch(dialogRef, syncDialog);

const agentSelectOptions = computed<AgentPersonaOption[]>(() => {
  const seen = new Set<string>();
  const excludeSelf = panel.value === "delegate" ? String(props.currentAgentId || "").trim() : "";
  return (Array.isArray(props.agentOptions) ? props.agentOptions : [])
    .map((item) => {
      const agentId = String(item.agentId || "").trim();
      return {
        ...item,
        agentId,
        id: String(item.id || agentId).trim(),
      };
    })
    .filter((item) => {
      if (!item.agentId || !item.id || seen.has(item.id)) return false;
      if (excludeSelf && item.agentId === excludeSelf) return false;
      seen.add(item.id);
      return true;
    });
});

const validSelectionOption = computed<AgentPersonaOption | null>(() => {
  const selectedAgentIdValue = String(selectedAgentId.value || "").trim();
  const selected = agentSelectOptions.value.find((item) => item.agentId === selectedAgentIdValue);
  if (selected) return selected;
  if (panel.value !== "delegate") {
    const currentAgentId = String(props.currentAgentId || "").trim();
    const currentOption = agentSelectOptions.value.find((item) => item.agentId === currentAgentId);
    if (currentOption) return currentOption;
  }
  return agentSelectOptions.value[0] || null;
});

const commitTotalPages = computed(() => Math.max(1, Math.ceil(props.commitTotal / Math.max(1, props.commitPageSize))));

const canConfirm = computed(() => {
  if (props.submitting || props.delegating || !validSelectionOption.value) return false;
  if (panel.value === "delegate") return !!goalText.value.trim();
  if (scope.value === "commit") return selectedCommitHashes.value.length > 0;
  if (scope.value === "custom") return !!customTargetText.value.trim();
  return true;
});

watch(
  () => [props.currentAgentId, panel.value, agentSelectOptions.value.map((item) => item.id).join("|")] as const,
  () => {
    const selectedOption = validSelectionOption.value;
    const nextAgentId = String(selectedOption?.agentId || "").trim();
    if (selectedAgentId.value !== nextAgentId) {
      selectedAgentId.value = nextAgentId;
    }
    if (!selectedApiConfigId.value) {
      selectedApiConfigId.value = conversationDefaultApiConfigId();
    }
  },
  { immediate: true },
);

watch(
  () => [props.open, String(props.currentApiConfigId || "").trim()] as const,
  ([open], previous) => {
    if (!open) return;
    const [wasOpen] = previous || [false];
    // 每次打开都以当前会话模型为默认；已打开期间会话模型变化不打断用户选择
    if (!wasOpen) {
      selectedApiConfigId.value = conversationDefaultApiConfigId();
      return;
    }
    if (!selectedApiConfigId.value) {
      selectedApiConfigId.value = conversationDefaultApiConfigId();
    }
  },
);

watch(
  () => props.open,
  (open, wasOpen) => {
    if (!open) return;
    panel.value = props.initialPanel === "delegate" ? "delegate" : "review";
    if (wasOpen) return;
    goalText.value = "";
    selectedCommitHashes.value = [];
    customTargetText.value = "";
    scope.value = "main";
  },
);

watch(
  () => props.initialPanel,
  (value) => {
    if (!props.open) return;
    panel.value = value === "delegate" ? "delegate" : "review";
  },
);

watch(
  () => props.commitSourceKey,
  (next, previous) => {
    if (next === previous) return;
    selectedCommitHashes.value = [];
    if (!props.open || scope.value !== "commit") return;
    emit("pickCommitReview", 1);
  },
);

function loadRecentGoals() {
  try {
    const raw = window.localStorage.getItem(RECENT_STORAGE_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return;
    recentGoals.value = parsed
      .map((item) => {
        const record = item as { id?: string; agentId?: string; goal?: string; question?: string };
        const goal = String(record?.goal || record?.question || "").trim();
        const agentId = String(record?.agentId || "").trim();
        if (!goal || !agentId) return null;
        return {
          id: String(record?.id || `${agentId}:${goal}`).trim(),
          agentId,
          goal,
        };
      })
      .filter((item): item is RecentGoal => !!item)
      .slice(0, RECENT_LIMIT);
  } catch {
    recentGoals.value = [];
  }
}

function rememberGoal(agentId: string, goal: string) {
  const next = {
    id: `${Date.now()}:${agentId}`,
    agentId,
    goal,
    presetId: "custom",
    why: "",
    todo: "",
    label: goal,
  };
  const kept = recentGoals.value.filter((item) => !(item.agentId === agentId && item.goal === goal));
  recentGoals.value = [{ id: next.id, agentId, goal }, ...kept].slice(0, RECENT_LIMIT);
  try {
    const raw = window.localStorage.getItem(RECENT_STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) as unknown : [];
    const previous = Array.isArray(parsed) ? parsed : [];
    const stored = [
      next,
      ...previous.filter((item) => {
        const record = item as { agentId?: string; goal?: string };
        return !(String(record?.agentId || "") === agentId && String(record?.goal || "") === goal);
      }),
    ].slice(0, RECENT_LIMIT);
    window.localStorage.setItem(RECENT_STORAGE_KEY, JSON.stringify(stored));
  } catch {
    // 最近目标写不进去不影响这次提交
  }
}

function applyRecentGoal(item: RecentGoal) {
  goalText.value = item.goal;
  if (agentSelectOptions.value.some((option) => option.agentId === item.agentId)) {
    selectedAgentId.value = item.agentId;
    if (!selectedApiConfigId.value) {
      selectedApiConfigId.value = conversationDefaultApiConfigId();
    }
  }
}

function setScope(nextScope: ToolReviewCodeReviewScope) {
  scope.value = nextScope;
  if (nextScope === "commit" && !props.commitOptionsLoading && props.commitOptions.length === 0) {
    emit("pickCommitReview", 1);
  }
}

function requestCommitPage(page: number) {
  const normalizedPage = Math.min(Math.max(1, page), commitTotalPages.value);
  emit("pickCommitReview", normalizedPage);
}

function toggleCommitSelection(hash: string) {
  const normalizedHash = String(hash || "").trim();
  if (!normalizedHash) return;
  selectedCommitHashes.value = selectedCommitHashes.value.includes(normalizedHash)
    ? selectedCommitHashes.value.filter((item) => item !== normalizedHash)
    : [...selectedCommitHashes.value, normalizedHash];
}

function close() {
  selectedCommitHashes.value = [];
  customTargetText.value = "";
  goalText.value = "";
  emit("close");
}

function selectedModelId(): string {
  return String(selectedApiConfigId.value || "").trim();
}

/** 由宿主在提交成功后调用：把这次目标写进最近列表（失败不写） */
function rememberCurrentDelegateGoal() {
  const agentId = String(validSelectionOption.value?.agentId || "").trim();
  const goal = String(goalText.value || "").trim();
  if (!agentId || !goal) return;
  rememberGoal(agentId, goal);
}

function confirm() {
  const selection = validSelectionOption.value;
  const agentId = String(selection?.agentId || "").trim();
  if (!agentId) return;
  const apiConfigId = selectedModelId() || undefined;
  if (panel.value === "delegate") {
    const goal = goalText.value.trim().slice(0, 10000);
    if (!goal) return;
    // 提交是异步的：这里只发出请求，由宿主拿到结果后再决定关闭或报错，
    // 失败时保留已填目标，也不把失败的目标记进最近列表。
    emit("delegate", { agentId, goal, apiConfigId });
    return;
  }
  if (scope.value === "commit") {
    if (selectedCommitHashes.value.length === 0) return;
    emit("reviewCode", { scope: "commit", target: selectedCommitHashes.value.join("\n"), agentId, apiConfigId });
    return;
  }
  if (scope.value === "custom") {
    const target = customTargetText.value.trim();
    if (!target) return;
    emit("reviewCode", { scope: "custom", target, agentId, apiConfigId });
    return;
  }
  emit("reviewCode", { scope: scope.value, target: "", agentId, apiConfigId });
}

onMounted(loadRecentGoals);

defineExpose({
  /** 提交成功后由宿主调用，把本次目标记入最近列表 */
  rememberCurrentDelegateGoal,
});
</script>
