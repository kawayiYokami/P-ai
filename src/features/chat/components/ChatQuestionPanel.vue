<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import DoubleDeckCard from "./input-panel/DoubleDeckCard.vue";
import OverlayScrollArea from "../../shared/components/OverlayScrollArea.vue";
import TerminalApprovalPatchSample from "../../shell/components/TerminalApprovalPatchSample.vue";
import type {
  QuestionAnswer,
  QuestionItem,
  QuestionOption,
  QuestionOptionKind,
} from "./chat-question-panel-types";
import {
  conversationQuestionFingerprints,
  conversationQuestionsGotUpdates,
} from "../utils/conversation-question";
import {
  buildQuestionSubmitPayload,
  buildSelectedQuestionSubmitPayload,
  countAnsweredQuestions,
  nextUnansweredIndex,
  resolveQuestionOptions,
  type QuestionSubmitPayloadItem,
} from "../utils/question-panel-submit";

// 组件对外继续导出这些类型，既有调用方（含 DemoTab）不受影响。
export type { QuestionAnswer, QuestionItem, QuestionOption, QuestionOptionKind };

const props = withDefaults(
  defineProps<{
    items: QuestionItem[];
    modelValue?: Record<string, QuestionAnswer>;
    submitting?: boolean;
    /**
     * 提交节奏。
     * - `all`：全部答完再一起提交（终端审批），行为与历史一致。
     * - `manual`：选项只记录为已选，点右下角「发送 N 题」才把这批已选题一起发出去；
     *   未选的题跳过不拦截，发出的题从列表消失，剩下的可以接着选（会话提问工具）。
     */
    submitMode?: "all" | "manual";
  }>(),
  {
    modelValue: () => ({}),
    submitting: false,
    submitMode: "all",
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: Record<string, QuestionAnswer>];
  submit: [answers: QuestionSubmitPayloadItem[]];
  reject: [number: number];
  cancel: [];
  approveForWorkspace: [requestId: string];
}>();

const { t } = useI18n();
const currentIndex = ref(0);
const confirmStep = ref(false);
const shakeOptionId = ref("");
const optionDrafts = ref<Record<string, string>>({});
const optionInputEls = ref<Record<string, HTMLInputElement | null>>({});
/** 「其他（自己写）」的虚拟 option id：只用于错误高亮，不参与任何选项匹配。 */
const CUSTOM_ANSWER_OPTION_ID = "__custom__";
const customAnswerEditing = ref(false);
const customAnswerDrafts = ref<Record<string, string>>({});
const customAnswerInputEl = ref<HTMLInputElement | null>(null);
/**
 * 搁置折叠：只活在组件内部，不落库、不进任何持久化结构。
 * 切会话时父级用 `:key="conversationQuestionKey"` 重建面板，折叠态随之归零，正好回到展开。
 */
const panelCollapsed = ref(false);
let previousQuestionFingerprints = conversationQuestionFingerprints(props.items);

const total = computed(() => props.items.length);
const currentItem = computed(() => props.items[currentIndex.value] ?? null);
const isSingle = computed(() => total.value <= 1);
/** 手动发送（会话提问工具）：选项只记录为已选，点右下角发送才把这批已选题交出去。 */
const isManualSubmit = computed(() => props.submitMode === "manual");
/** 提问卡露头显隐：前提内容（patch/单行）与确认列表放底座露头，题干与选项收进面卡 */
const hasPreviewText = computed(() => !!String(currentItem.value?.previewText ?? "").trim());
const hasExtra = computed(() => {
  if (total.value === 0) return false;
  if (confirmStep.value) return true;
  return hasPreviewText.value;
});
const answersMap = computed(() => props.modelValue ?? {});
/** 已选且填好的题目条数：发送按钮的文案与可用态都看它。 */
const selectedCount = computed(() => countAnsweredQuestions(props.items, answersMap.value));

const allAnswered = computed(() =>
  total.value > 0 && props.items.every((item) => !!answersMap.value[item.id]),
);

const currentOptions = computed<QuestionOption[]>(() => {
  const item = currentItem.value;
  if (!item) return [];
  return resolveQuestionOptions(item);
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

function getCustomAnswerDraft(questionId: string): string {
  return customAnswerDrafts.value[questionId] ?? "";
}

function setCustomAnswerDraft(questionId: string, value: string) {
  customAnswerDrafts.value[questionId] = value;
}

function setCustomAnswerInputEl(el: unknown) {
  customAnswerInputEl.value = el as HTMLInputElement | null;
}

/** 展开「其他（自己写）」：这一题已经写过答案时预填进来，改着方便。 */
function handleStartCustomAnswer() {
  const item = currentItem.value;
  if (!item || props.submitting) return;
  if (!getCustomAnswerDraft(item.id)) {
    customAnswerDrafts.value[item.id] = String(answersMap.value[item.id]?.customText ?? "");
  }
  customAnswerEditing.value = true;
  nextTick(() => customAnswerInputEl.value?.focus());
}

/**
 * 把自己写的内容记为这一题的答案。与点选项一致：这里只勾选，
 * 真正提交要等右下角「发送 N 题」；空内容不记，只抖一下提示。
 */
function handleCustomAnswerConfirm() {
  const item = currentItem.value;
  if (!item || props.submitting) return;
  const text = getCustomAnswerDraft(item.id).trim();
  if (!text) {
    shakeOptionId.value = CUSTOM_ANSWER_OPTION_ID;
    window.setTimeout(() => {
      if (shakeOptionId.value === CUSTOM_ANSWER_OPTION_ID) shakeOptionId.value = "";
    }, 420);
    customAnswerInputEl.value?.focus();
    return;
  }
  shakeOptionId.value = "";
  const next = {
    ...answersMap.value,
    [item.id]: { optionId: "", label: text, comment: "", customText: text },
  };
  emit("update:modelValue", next);
  // 自己写也是答完一题：多题时同样跳到下一道未答（单题没有去处，保持只记录）。
  if (!isSingle.value) advanceAfterAnswer(next);
}

watch(currentItem, () => {
  shakeOptionId.value = "";
  // 切到一题时，已经自己写过答案的直接展开并预填，省得再点一次入口。
  const item = currentItem.value;
  const existing = item ? String(answersMap.value[item.id]?.customText ?? "").trim() : "";
  customAnswerEditing.value = existing.length > 0;
  if (item && existing && !getCustomAnswerDraft(item.id)) {
    customAnswerDrafts.value[item.id] = existing;
  }
});

/**
 * 答完一题后题目被移出列表，停留位置取「原索引 clamp 到新长度内」：
 * 同一位置现在是下一道题，答到最后一道时回落到新的最后一道，不会弹回第一题。
 * 逐题提交依赖父级不重建面板（key 只跟会话走），这里才留得住用户挑的位置。
 */
watch(
  () => props.items.length,
  (length) => {
    if (length === 0) {
      currentIndex.value = 0;
      return;
    }
    if (currentIndex.value > length - 1) currentIndex.value = length - 1;
  },
);

/**
 * 折叠着的时候，题目有更新才自己展开：新题进来、或已有题被改。
 * 只是减少（答掉一题、拒掉一题）不展开——那是用户刚做完的动作，再弹回来就是打扰。
 * 父级每次刷新都会给一份新数组，所以比的是内容指纹，不是引用。
 */
watch(
  () => props.items,
  (next) => {
    if (conversationQuestionsGotUpdates(previousQuestionFingerprints, next)) {
      panelCollapsed.value = false;
    }
    previousQuestionFingerprints = conversationQuestionFingerprints(next);
  },
);

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

/**
 * 答完一题后跳到下一道**未答**的题，不是机械的下一格：用户会跳着答，中间留空很正常。
 * 全部答完时终端审批进全量确认视图；会话提问停在原地等右下角发送，不替用户按下发送。
 */
function advanceAfterAnswer(nextAnswers: Record<string, QuestionAnswer>) {
  const nextIndex = nextUnansweredIndex(props.items, nextAnswers, currentIndex.value);
  if (nextIndex >= 0) {
    goTo(nextIndex);
    return;
  }
  if (!isManualSubmit.value) confirmStep.value = true;
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
  // 改选给定选项就等于放弃自己写的那份答案：收起输入行，草稿留着可再展开。
  customAnswerEditing.value = false;
  const next = {
    ...answersMap.value,
    [item.id]: { optionId: option.id, label: option.label, comment },
  };
  emit("update:modelValue", next);

  // 单题没有下一道可跳：终端审批直接提交，会话提问只记录、等右下角发送。两条都是原行为。
  if (isSingle.value) {
    if (!isManualSubmit.value) handleSubmitAll(next);
    return;
  }

  advanceAfterAnswer(next);
}

/** 手动发送：把这批已选题一次性交出去，未选的题不参与也不拦截。 */
function handleSubmitSelected() {
  if (props.submitting) return;
  const payload = buildSelectedQuestionSubmitPayload(props.items, answersMap.value);
  if (!payload) return;
  // 发完停留剩下的第一题（全发完就是空态），不跨轮记忆位置。
  currentIndex.value = 0;
  confirmStep.value = false;
  emit("submit", payload);
}

function handleSubmitAll(overrideMap?: Record<string, QuestionAnswer>) {
  if (props.submitting) return;
  const payload = buildQuestionSubmitPayload(props.items, overrideMap ?? answersMap.value);
  if (!payload) return;
  emit("submit", payload);
}

/**
 * 拒绝回答：单题即时生效，不需要先选选项，也不参与批量发送。
 * 只允许手动提交模式（会话提问）出现——终端审批那套的选项是批准/拒绝执行，题目本身不能拒。
 */
function handleReject() {
  const item = currentItem.value;
  if (!item || props.submitting || !isManualSubmit.value) return;
  const number = Number(item.id);
  if (!Number.isInteger(number) || number < 1) return;
  emit("reject", number);
}
</script>

<template>
  <div class="mx-auto w-full max-w-3xl">
    <!-- 搁置窄条：收起后只剩这一条。数字取当前列表里的题数，答掉的题会从列表消失 -->
    <button
      v-if="panelCollapsed"
      type="button"
      class="flex w-full items-center gap-2 rounded-box border border-base-content/15 bg-base-200/60 px-3 py-1.5 text-xs text-base-content/70 transition-colors hover:border-base-content/25 hover:text-base-content"
      @click="panelCollapsed = false"
    >
      <AppIcon name="help-circle" class="h-3.5 w-3.5 shrink-0" aria-hidden="true" />
      <span class="min-w-0 truncate">{{ t("chat.questionPanel.pendingCount", { count: total }) }}</span>
      <AppIcon name="chevron-up" class="ml-auto h-3.5 w-3.5 shrink-0" aria-hidden="true" />
    </button>
    <!-- 提问卡：双层卡，前提内容放底卡露头，题干加选项放面卡。
         收起不卸载卡片，走最大高度与透明度过渡，展开回来草稿与选中态都还在。 -->
    <DoubleDeckCard
      class="ecall-question-fold"
      :class="panelCollapsed ? 'is-collapsed' : ''"
      :extra-visible="hasExtra"
      is-rounded
      frameless
      bg="base-200"
    >
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
          {{ t("chat.questionPanel.empty") }}
        </div>

        <template v-else>
          <!-- header: 题号切换条。两种节奏都渲染：它的职责是挑题，不是走向导。 -->
          <nav v-if="!isSingle" class="flex flex-wrap items-center gap-1 px-2 pb-1 text-xs" aria-label="breadcrumb">
            <template v-for="(item, idx) in items" :key="item.id">
              <button
                type="button"
                class="rounded px-1.5 py-0.5 transition"
                :class="[
                  idx === currentIndex && !confirmStep ? 'bg-primary text-primary-content' : answersMap[item.id] ? 'bg-success text-success-content font-medium' : 'text-base-content/45 hover:bg-base-200',
                  confirmStep ? (answersMap[item.id] ? 'bg-success text-success-content font-medium' : 'text-base-content/45') : '',
                ]"
                @click="goTo(idx)"
              >
                {{ idx + 1 }}
              </button>
              <span v-if="idx < items.length - 1" class="text-base-content/25">/</span>
            </template>
            <span v-if="confirmStep" class="ml-1 text-base-content/35">· {{ t("chat.questionPanel.confirmSuffix") }}</span>
          </nav>

          <div v-if="confirmStep" class="flex justify-end gap-2 px-2 py-1">
            <button type="button" class="btn btn-ghost btn-sm" :disabled="submitting" @click="confirmStep = false">
              {{ t("chat.questionPanel.backToEdit") }}
            </button>
            <button type="button" class="btn btn-primary btn-sm" :disabled="submitting || !allAnswered" @click="handleSubmitAll()">
              {{ submitting ? t("chat.questionPanel.submitting") : t("chat.questionPanel.confirmSubmit", { count: total }) }}
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
              <button
                type="button"
                class="btn btn-ghost btn-xs h-5 min-h-0 shrink-0 gap-1 px-1.5 text-xs font-normal text-base-content/50 hover:text-base-content"
                :title="t('chat.questionPanel.collapseHint')"
                @click="panelCollapsed = true"
              >
                {{ t("chat.questionPanel.collapse") }}
                <AppIcon name="chevron-down" class="h-3 w-3" aria-hidden="true" />
              </button>
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
              class="btn btn-sm ecall-question-option w-full justify-center"
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
                :placeholder="opt.placeholder ?? (opt.inputRequired ? t('chat.questionPanel.optionPlaceholderRequired') : t('chat.questionPanel.optionPlaceholderOptional'))"
                class="input input-bordered input-sm join-item min-w-0 flex-1 text-sm"
                :class="shakeOptionId === opt.id ? 'input-error animate-[shake_0.42s_ease]' : ''"
                @input="setDraft(currentItem!.id, opt.id, ($event.target as HTMLInputElement).value)"
                @keydown.enter.prevent="handleOptionSelect(opt)"
              >
              <button
                type="button"
                class="btn btn-sm ecall-question-option join-item shrink-0"
                :class="optionBtnClass(opt)"
                :disabled="submitting"
                @click="handleOptionSelect(opt)"
              >
                {{ opt.label }}
              </button>
            </div>
          </template>
          <!-- 其他：自己写答案。只在会话提问出现，终端审批那套的选项是批准/拒绝执行，不改 -->
          <div v-if="isManualSubmit" class="w-full">
            <button
              v-if="!customAnswerEditing"
              type="button"
              class="btn btn-sm ecall-question-option w-full justify-center btn-primary btn-soft"
              :disabled="submitting"
              @click="handleStartCustomAnswer"
            >
              {{ t("chat.questionPanel.customAnswer") }}
            </button>
            <div v-else class="join w-full">
              <input
                :ref="setCustomAnswerInputEl"
                :value="getCustomAnswerDraft(currentItem!.id)"
                type="text"
                :placeholder="t('chat.questionPanel.customAnswerPlaceholder')"
                class="input input-bordered input-sm join-item min-w-0 flex-1 text-sm"
                :class="shakeOptionId === CUSTOM_ANSWER_OPTION_ID ? 'input-error animate-[shake_0.42s_ease]' : ''"
                @input="setCustomAnswerDraft(currentItem!.id, ($event.target as HTMLInputElement).value)"
                @keydown.enter.prevent="handleCustomAnswerConfirm"
              >
              <button
                type="button"
                class="btn btn-sm ecall-question-option join-item shrink-0 btn-primary"
                :disabled="submitting"
                @click="handleCustomAnswerConfirm"
              >
                {{ t("chat.questionPanel.confirm") }}
              </button>
            </div>
          </div>
          <div v-if="shakeOptionId" class="text-xs text-error">
            {{ shakeOptionId === CUSTOM_ANSWER_OPTION_ID ? t("chat.questionPanel.customAnswerEmpty") : t("chat.questionPanel.optionRequiredEmpty") }}
          </div>
          </div>

          <!-- 手动发送：放弃此问题在左下角、发送在右下角，未选时发送是灰态，位置不跳 -->
          <div v-if="isManualSubmit" class="flex items-center justify-between gap-2 px-2 pb-1 pt-2">
            <button
              type="button"
              class="btn btn-ghost btn-sm font-normal text-base-content/60 hover:text-base-content"
              :disabled="submitting"
              @click="handleReject"
            >
              {{ t("chat.questionPanel.reject") }}
            </button>
            <button
              type="button"
              class="btn btn-primary btn-sm"
              :disabled="submitting || selectedCount === 0"
              @click="handleSubmitSelected"
            >
              {{ submitting ? t("chat.questionPanel.sending") : t("chat.questionPanel.sendSelected", { count: selectedCount }) }}
            </button>
          </div>
        </template>
      </template>
    </DoubleDeckCard>
  </div>
</template>

<style scoped>
/* 收起/展开：卡片不卸载，靠最大高度与透明度过渡（时长由卡片根的 transition-all 提供） */
.ecall-question-fold {
  overflow: hidden;
  max-height: 2000px;
  opacity: 1;
}

.ecall-question-fold.is-collapsed {
  max-height: 0;
  opacity: 0;
  pointer-events: none;
}

/* 选项文字沿项目既有混色口径（同 style.css 的 --ecall-md-link-color）：
   主色掺入正文色——浅色主题把主色压深、深色主题把主色提亮。
   浅深两档主色都已单独过对比度，这里只是让文字比实心按钮更收敛，
   不动全局 btn 样式；只作用于 soft 态，选中后的实心 primary 仍用 primary-content。 */
.ecall-question-option.btn-soft {
  color: color-mix(in oklab, var(--color-primary) 70%, var(--color-base-content));
}

@keyframes shake {
  0%, 100% { transform: translateX(0); }
  20% { transform: translateX(-3px); }
  40% { transform: translateX(3px); }
  60% { transform: translateX(-2px); }
  80% { transform: translateX(2px); }
}
</style>
