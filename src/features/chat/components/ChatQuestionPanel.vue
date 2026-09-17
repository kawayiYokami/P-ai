<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import DoubleDeckCard from "./input-panel/DoubleDeckCard.vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import TerminalApprovalPatchSample from "../../shell/components/TerminalApprovalPatchSample.vue";

export type QuestionOptionKind = "direct" | "withInput";

export type QuestionOption = {
  id: string;
  label: string;
  kind: QuestionOptionKind;
  placeholder?: string;
  inputRequired?: boolean;
};

export type QuestionItem = {
  id: string;
  title: string;
  description?: string;
  previewText?: string;
  options?: QuestionOption[];
  canRememberWorkspace?: boolean;
  workspaceLabel?: string;
};

export type QuestionAnswer = {
  optionId: string;
  label: string;
  comment: string;
};

const props = withDefaults(
  defineProps<{
    items: QuestionItem[];
    modelValue?: Record<string, QuestionAnswer>;
    submitting?: boolean;
  }>(),
  {
    modelValue: () => ({}),
    submitting: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: Record<string, QuestionAnswer>];
  submit: [answers: Array<{ id: string; optionId: string; label: string; comment: string }>];
  cancel: [];
  approveForWorkspace: [requestId: string];
}>();

const { t } = useI18n();
const currentIndex = ref(0);
const confirmStep = ref(false);
const shakeOptionId = ref("");
const optionDrafts = ref<Record<string, string>>({});
const optionInputEls = ref<Record<string, HTMLInputElement | null>>({});

const total = computed(() => props.items.length);
const currentItem = computed(() => props.items[currentIndex.value] ?? null);
const isSingle = computed(() => total.value <= 1);
/** 提问卡露头显隐：前提内容（patch/单行）与确认列表放底座露头，题干与选项收进面卡 */
const hasPreviewText = computed(() => !!String(currentItem.value?.previewText ?? "").trim());
const hasExtra = computed(() => {
  if (total.value === 0) return false;
  if (confirmStep.value) return true;
  return hasPreviewText.value;
});
const answersMap = computed(() => props.modelValue ?? {});

const allAnswered = computed(() =>
  total.value > 0 && props.items.every((item) => !!answersMap.value[item.id]),
);

const currentOptions = computed<QuestionOption[]>(() => {
  const item = currentItem.value;
  if (!item) return [];
  if (Array.isArray(item.options) && item.options.length > 0) return item.options;
  return [
    { id: "deny", label: "拒绝", kind: "withInput", placeholder: "补充说明（拒绝必填）", inputRequired: true },
    { id: "approve", label: "同意", kind: "direct" },
  ];
});

const selectedOptionIdForCurrent = computed(() => {
  const id = currentItem.value?.id ?? "";
  return id ? answersMap.value[id]?.optionId ?? "" : "";
});

const previewLines = computed(() => {
  const text = String(currentItem.value?.previewText ?? "");
  if (!text.trim()) return [] as string[];
  return text.replace(/\r/g, "").split("\n");
});

const isPreviewPatch = computed(() => {
  const text = String(currentItem.value?.previewText ?? "");
  return text.includes("*** Begin Patch") || text.includes("*** Update File:") || text.includes("*** Add File:") || text.includes("*** Delete File:") || text.includes("@@");
});

const canRememberWorkspaceForCurrent = computed(() => !!currentItem.value?.canRememberWorkspace);
const workspaceLabelForCurrent = computed(() => String(currentItem.value?.workspaceLabel || "").trim());

function handleWorkspaceRemember() {
  const item = currentItem.value;
  if (!item || props.submitting || !item.canRememberWorkspace) return;
  emit("approveForWorkspace", item.id);
}

function draftKey(questionId: string, optionId: string): string {
  return `${questionId}::${optionId}`;
}

function getDraft(questionId: string, optionId: string): string {
  return optionDrafts.value[draftKey(questionId, optionId)] ?? "";
}

function setDraft(questionId: string, optionId: string, value: string) {
  optionDrafts.value[draftKey(questionId, optionId)] = value;
}

function setInputEl(optionId: string, el: unknown) {
  optionInputEls.value[optionId] = el as HTMLInputElement | null;
}

watch(currentItem, () => {
  shakeOptionId.value = "";
});

watch(() => props.modelValue, () => {
  // keep drafts if user navigates back: prefill draft from saved answer comment for withInput
  const item = currentItem.value;
  if (!item) return;
  const ans = answersMap.value[item.id];
  if (!ans) return;
  const opts = currentOptions.value;
  const matched = opts.find((o) => o.id === ans.optionId);
  if (matched?.kind === "withInput") {
    const key = draftKey(item.id, matched.id);
    if (!optionDrafts.value[key]) {
      optionDrafts.value[key] = ans.comment ?? "";
    }
  }
});

function isDestructiveOption(opt: QuestionOption): boolean {
  if (opt.inputRequired) return true;
  const text = `${opt.id} ${opt.label}`.toLowerCase();
  return text.includes("deny") || text.includes("reject") || text.includes("拒绝");
}

function optionBtnClass(opt: QuestionOption): string {
  const selected = selectedOptionIdForCurrent.value === opt.id;
  const destructive = isDestructiveOption(opt);
  if (destructive) return selected ? "btn-error" : "btn-error btn-soft";
  return selected ? "btn-primary" : "btn-primary btn-soft";
}

function goTo(index: number) {
  if (total.value === 0) return;
  const clamped = Math.max(0, Math.min(index, total.value - 1));
  currentIndex.value = clamped;
  confirmStep.value = false;
  shakeOptionId.value = "";
  nextTick(() => {});
}

function handleOptionSelect(option: QuestionOption) {
  const item = currentItem.value;
  if (!item || props.submitting) return;
  let comment = "";
  if (option.kind === "withInput") {
    comment = getDraft(item.id, option.id).trim();
    if (option.inputRequired && !comment) {
      shakeOptionId.value = option.id;
      window.setTimeout(() => {
        if (shakeOptionId.value === option.id) shakeOptionId.value = "";
      }, 420);
      optionInputEls.value[option.id]?.focus();
      return;
    }
  }
  shakeOptionId.value = "";
  const next = {
    ...answersMap.value,
    [item.id]: { optionId: option.id, label: option.label, comment },
  };
  emit("update:modelValue", next);

  if (isSingle.value) {
    handleSubmitAll(next);
    return;
  }
  const isLast = currentIndex.value === total.value - 1;
  if (isLast) {
    confirmStep.value = true;
    return;
  }
  goTo(currentIndex.value + 1);
}

function handleSubmitAll(overrideMap?: Record<string, QuestionAnswer>) {
  const map = overrideMap ?? answersMap.value;
  if (props.submitting) return;
  if (!props.items.every((item) => !!map[item.id])) return;
  for (const item of props.items) {
    const ans = map[item.id];
    if (!ans) return;
    const opts = Array.isArray(item.options) && item.options.length > 0
      ? item.options
      : [
          { id: "deny", label: "拒绝", kind: "withInput" as const, inputRequired: true },
          { id: "approve", label: "同意", kind: "direct" as const },
        ];
    const opt = opts.find((o) => o.id === ans.optionId);
    if (opt?.kind === "withInput" && opt.inputRequired && !String(ans.comment ?? "").trim()) return;
  }
  const payload = props.items.map((item) => {
    const ans = map[item.id]!;
    return { id: item.id, optionId: ans.optionId, label: ans.label, comment: ans.comment };
  });
  emit("submit", payload);
}
</script>

<template>
  <div class="mx-auto w-full max-w-3xl">
    <!-- 提问卡：双层卡，前提内容放底卡露头，题干加选项放面卡 -->
    <DoubleDeckCard :extra-visible="hasExtra" :is-rounded="true" bg="base-200">
      <template #extra>
        <div v-if="total > 0" class="space-y-3 px-4 pb-1 pt-4">
          <!-- confirm overview: N>1 all answered，列表放露头 -->
          <ul v-if="confirmStep" class="flex flex-col gap-2">
            <li
              v-for="(item, idx) in items"
              :key="item.id"
              class="flex cursor-pointer items-start justify-between gap-3 rounded-box border border-base-300 bg-base-100/80 px-3 py-2.5 hover:border-base-300"
              @click="goTo(idx)"
            >
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <span class="badge badge-ghost badge-sm">{{ idx + 1 }}</span>
                  <span class="truncate text-sm">{{ item.title }}</span>
                </div>
                <div class="mt-1 flex flex-wrap items-center gap-1.5">
                  <span class="badge badge-sm" :class="answersMap[item.id]?.label.includes('拒绝') || answersMap[item.id]?.optionId === 'deny' ? 'badge-error badge-outline' : 'badge-success badge-outline'">
                    {{ answersMap[item.id]?.label ?? answersMap[item.id]?.optionId }}
                  </span>
                  <span
                    v-if="answersMap[item.id]?.comment"
                    class="line-clamp-2 text-xs text-base-content/60"
                  >
                    {{ answersMap[item.id]?.comment }}
                  </span>
                </div>
              </div>
            </li>
          </ul>

          <!-- 前提内容：直接铺在底座上，用项目统一滚动区 -->
          <OverlayScrollArea
            v-else-if="currentItem && hasPreviewText"
            scroller-class="max-h-36 overscroll-contain"
          >
            <TerminalApprovalPatchSample
              v-if="isPreviewPatch"
              :lines="previewLines"
              :diff-only="false"
              :show-prefixes="true"
              :collapsed="false"
              :hide-header="true"
              :embedded="true"
            />
            <pre v-else class="whitespace-pre-wrap break-words text-xs leading-5 text-base-content/70">{{ currentItem.previewText }}</pre>
          </OverlayScrollArea>
        </div>
      </template>
      <template #main>
        <div v-if="total === 0" class="px-2 py-8 text-center text-sm text-base-content/50">
          暂无问题
        </div>

        <template v-else>
          <!-- header: breadcrumb -->
          <nav v-if="!isSingle" class="flex flex-wrap items-center gap-1 px-2 pb-1 text-xs" aria-label="breadcrumb">
            <template v-for="(item, idx) in items" :key="item.id">
              <button
                type="button"
                class="rounded px-1.5 py-0.5 transition"
                :class="[
                  idx === currentIndex && !confirmStep ? 'bg-primary text-primary-content' : answersMap[item.id] ? 'bg-success/15 text-success' : 'text-base-content/45 hover:bg-base-200',
                  confirmStep ? (answersMap[item.id] ? 'bg-success/15 text-success' : 'text-base-content/45') : '',
                ]"
                @click="goTo(idx)"
              >
                {{ idx + 1 }}
              </button>
              <span v-if="idx < items.length - 1" class="text-base-content/25">/</span>
            </template>
            <span v-if="confirmStep" class="ml-1 text-base-content/35">· 确认</span>
          </nav>

          <div v-if="confirmStep" class="flex justify-end gap-2 px-2 py-1">
            <button type="button" class="btn btn-ghost btn-sm" :disabled="submitting" @click="confirmStep = false">
              返回修改
            </button>
            <button type="button" class="btn btn-primary btn-sm" :disabled="submitting || !allAnswered" @click="handleSubmitAll()">
              {{ submitting ? "提交中..." : `确认提交 ${total} 题` }}
            </button>
          </div>

          <div v-else-if="currentItem" class="space-y-2 px-2 py-1">
            <!-- 题干：QuestionItem.title（审批 summary）+ description（审批 message/reason），目录全权随题干进面卡 -->
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0 flex-1">
                <div class="text-sm font-semibold leading-6 whitespace-pre-wrap break-words">{{ currentItem.title }}</div>
                <div v-if="currentItem.description" class="mt-1 whitespace-pre-wrap break-words text-xs leading-5 text-base-content/60">
                  {{ currentItem.description }}
                </div>
              </div>
              <label
                v-if="canRememberWorkspaceForCurrent"
                class="flex shrink-0 cursor-pointer items-center gap-2 btn btn-ghost font-normal"
                :class="submitting ? 'pointer-events-none opacity-60' : ''"
                :title="workspaceLabelForCurrent || t('terminalApproval.rememberWorkspace')"
              >
                <input
                  type="checkbox"
                  class="checkbox checkbox-sm"
                  :disabled="submitting"
                  @change="handleWorkspaceRemember"
                >
                <span>{{ t("terminalApproval.rememberWorkspace") }}</span>
              </label>
            </div>
          <template
            v-for="opt in [...currentOptions].sort((a, b) => {
              const aDeny = a.kind === 'withInput' && a.inputRequired ? 1 : 0;
              const bDeny = b.kind === 'withInput' && b.inputRequired ? 1 : 0;
              return aDeny - bDeny;
            })"
            :key="opt.id"
          >
            <button
              v-if="opt.kind === 'direct'"
              type="button"
              class="btn btn-sm w-full justify-center"
              :class="optionBtnClass(opt)"
              :disabled="submitting"
              @click="handleOptionSelect(opt)"
            >
              {{ opt.label }}
            </button>
            <div v-else class="join w-full">
              <input
                :ref="(el) => setInputEl(opt.id, el)"
                :value="getDraft(currentItem!.id, opt.id)"
                type="text"
                :placeholder="opt.placeholder ?? (opt.inputRequired ? '补充说明（必填）' : '补充说明（可选）')"
                class="input input-bordered input-sm join-item min-w-0 flex-1 text-sm"
                :class="shakeOptionId === opt.id ? 'input-error animate-[shake_0.42s_ease]' : ''"
                @input="setDraft(currentItem!.id, opt.id, ($event.target as HTMLInputElement).value)"
                @keydown.enter.prevent="handleOptionSelect(opt)"
              >
              <button
                type="button"
                class="btn btn-sm join-item shrink-0"
                :class="optionBtnClass(opt)"
                :disabled="submitting"
                @click="handleOptionSelect(opt)"
              >
                {{ opt.label }}
              </button>
            </div>
          </template>
          <div v-if="shakeOptionId" class="text-xs text-error">请填写必填的补充说明</div>
          </div>
        </template>
      </template>
    </DoubleDeckCard>
  </div>
</template>

<style scoped>
@keyframes shake {
  0%, 100% { transform: translateX(0); }
  20% { transform: translateX(-3px); }
  40% { transform: translateX(3px); }
  60% { transform: translateX(-2px); }
  80% { transform: translateX(2px); }
}
</style>
