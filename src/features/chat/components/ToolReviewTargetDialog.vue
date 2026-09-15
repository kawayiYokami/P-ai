<template>
  <dialog ref="dialogRef" class="modal !items-start overflow-y-auto overflow-x-hidden pt-[max(1rem,env(safe-area-inset-top))] pb-[max(1rem,env(safe-area-inset-bottom))] sm:!items-center sm:py-6" @close="onDialogClose" @cancel.prevent="onDialogClose">
    <div class="modal-box mx-auto flex max-h-[calc(100dvh-max(2rem,env(safe-area-inset-top)+env(safe-area-inset-bottom)))] w-[88vw] max-w-4xl flex-col overflow-hidden p-0">
      <div class="shrink-0 border-b border-base-300 px-5 py-4">
        <div class="text-base font-semibold">{{ t("chat.toolReview.generateReviewReport") }}</div>
      </div>
      <div class="relative z-20 shrink-0 overflow-visible px-5 pt-4">
        <div class="mb-4 grid gap-1.5">
          <div class="text-xs font-medium text-base-content/60">{{ t("chat.toolReview.agentLabel") }}</div>
          <AgentPersonaSelect
            v-model:agent-id="selectedAgentId"
            :options="agentSelectOptions"
            :persona-avatar-url-map="personaAvatarUrlMap"
            auto-select-first
          />
        </div>
        <div role="tablist" class="tabs tabs-border flex-wrap">
          <button type="button" role="tab" class="tab" :class="{ 'tab-active': scope === 'commit' }" @click="setScope('commit')">{{ t("chat.toolReview.scopeCommit") }}</button>
          <button type="button" role="tab" class="tab" :class="{ 'tab-active': scope === 'main' }" @click="setScope('main')">{{ t("chat.toolReview.scopeMain") }}</button>
          <button type="button" role="tab" class="tab" :class="{ 'tab-active': scope === 'uncommitted' }" @click="setScope('uncommitted')">{{ t("chat.toolReview.scopeUncommitted") }}</button>
          <button type="button" role="tab" class="tab" :class="{ 'tab-active': scope === 'custom' }" @click="setScope('custom')">{{ t("chat.toolReview.scopeCustom") }}</button>
        </div>
      </div>
      <div class="relative z-0 min-h-0 flex-1 px-5 py-4" :class="scope === 'commit' ? 'flex flex-col overflow-hidden' : 'overflow-y-auto'">
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
        <div v-if="errorText" class="mt-3 rounded border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
          {{ errorText }}
        </div>
      </div>
      <div class="flex shrink-0 items-center justify-end gap-3 border-t border-base-300 px-5 py-4 pb-[max(1rem,env(safe-area-inset-bottom))]">
        <button type="button" class="btn" :disabled="submitting" @click="close">{{ t("common.cancel") }}</button>
        <button type="button" class="btn btn-primary" :disabled="!canConfirm" @click="confirm">{{ t("common.confirm") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDialogClose">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AgentPersonaSelect from "../../shared/components/AgentPersonaSelect.vue";
import type { ToolReviewCodeReviewScope, ToolReviewCommitOption } from "../composables/use-chat-tool-review";
import type { AgentPersonaOption } from "../../shared/agent-persona-options";

type AgentOption = AgentPersonaOption;

const props = defineProps<{
  open: boolean;
  submitting: boolean;
  errorText: string;
  currentAgentId: string;
  agentOptions: AgentOption[];
  personaAvatarUrlMap?: Record<string, string>;
  commitOptions: ToolReviewCommitOption[];
  commitOptionsLoading: boolean;
  commitTotal: number;
  commitPage: number;
  commitPageSize: number;
}>();

const emit = defineEmits<{
  close: [];
  pickCommitReview: [page: number];
  reviewCode: [input: { scope: ToolReviewCodeReviewScope; target?: string; agentId: string }];
}>();

const { t } = useI18n();
const dialogRef = ref<HTMLDialogElement | null>(null);

function onDialogClose() {
  if (props.submitting) {
    const d = dialogRef.value;
    if (d && !d.open && props.open) d.showModal();
    return;
  }
  close();
}

function syncDialog() {
  const d = dialogRef.value;
  if (!d) return;
  if (props.open) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(() => props.open, syncDialog);
watch(dialogRef, syncDialog);

const selectedAgentId = ref("");
const selectedCommitHashes = ref<string[]>([]);
const customTargetText = ref("");
const scope = ref<ToolReviewCodeReviewScope>("main");

const agentSelectOptions = computed<AgentPersonaOption[]>(() => {
  const seen = new Set<string>();
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
      seen.add(item.id);
      return true;
    });
});

const validSelectionOption = computed<AgentPersonaOption | null>(() => {
  const selectedAgentIdValue = String(selectedAgentId.value || "").trim();
  const selected = agentSelectOptions.value.find((item) => item.agentId === selectedAgentIdValue);
  if (selected) return selected;
  const currentAgentId = String(props.currentAgentId || "").trim();
  const currentOption = agentSelectOptions.value.find((item) => item.agentId === currentAgentId);
  if (currentOption) return currentOption;
  return agentSelectOptions.value[0] || null;
});

const commitTotalPages = computed(() => Math.max(1, Math.ceil(props.commitTotal / Math.max(1, props.commitPageSize))));

const canConfirm = computed(() => {
  if (props.submitting || !validSelectionOption.value) return false;
  if (scope.value === "commit") return selectedCommitHashes.value.length > 0;
  if (scope.value === "custom") return !!customTargetText.value.trim();
  return true;
});

watch(
  () => [props.currentAgentId, agentSelectOptions.value.map((item) => item.id).join("|")] as const,
  () => {
    const selectedOption = validSelectionOption.value;
    selectedAgentId.value = String(selectedOption?.agentId || "").trim();
  },
  { immediate: true },
);

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    const selectedOption = validSelectionOption.value;
    selectedAgentId.value = String(selectedOption?.agentId || "").trim();
  },
);

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
  emit("close");
}

function confirm() {
  const selection = validSelectionOption.value;
  const agentId = String(selection?.agentId || "").trim();
  if (!agentId) return;
  if (scope.value === "commit") {
    if (selectedCommitHashes.value.length === 0) return;
    emit("reviewCode", { scope: "commit", target: selectedCommitHashes.value.join("\n"), agentId });
    close();
    return;
  }
  if (scope.value === "custom") {
    const target = customTargetText.value.trim();
    if (!target) return;
    emit("reviewCode", { scope: "custom", target, agentId });
    close();
    return;
  }
  emit("reviewCode", { scope: scope.value, target: "", agentId });
  close();
}
</script>
