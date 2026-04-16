<template>
  <aside v-bind="rootAttrs" class="flex h-full min-h-0 flex-col overflow-x-hidden">
    <div class="min-h-0 flex-1 overflow-y-auto overflow-x-hidden">
      <div v-if="errorText" class="mx-4 my-4 rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
        {{ errorText }}
      </div>

      <template v-if="currentBatch">
        <div class="flex min-h-full flex-col">
          <div class="-mx-4 flex flex-col gap-3">
            <ToolReviewItemCard
              v-for="item in currentBatch.items"
              :key="item.callId"
              :item="item"
              :detail="detailMap[item.callId]"
              :loading="detailLoadingCallId === item.callId"
              :reviewing="reviewingCallId === item.callId"
              @load-detail="emit('loadItemDetail', $event)"
              @review="emit('reviewItem', $event)"
            />
          </div>
          <div v-if="batches.length > 1" class="mt-auto px-4 py-3">
            <div class="join flex justify-center">
              <button
                type="button"
                class="join-item btn btn-sm"
                :disabled="!previousBatch"
                @click="previousBatch && emit('selectBatch', previousBatch.batchKey)"
              >
                «
              </button>
              <button
                type="button"
                class="join-item btn btn-sm"
                @click.prevent
              >
                {{ t("chat.toolReview.pageLabel", { current: currentBatchIndex + 1, total: batches.length }) }}
              </button>
              <button
                type="button"
                class="join-item btn btn-sm"
                :disabled="!nextBatch"
                @click="nextBatch && emit('selectBatch', nextBatch.batchKey)"
              >
                »
              </button>
            </div>
          </div>
        </div>
      </template>

      <div v-else class="text-sm text-base-content/65">
        {{ t("chat.toolReview.empty") }}
      </div>
    </div>

    <div class="border-t border-base-300 px-4 py-3">
      <div v-if="currentBatch" class="grid grid-cols-2 gap-3">
        <button
          type="button"
          class="btn btn-sm w-full"
          :disabled="batchReviewing"
          @click="emit('reviewBatch', currentBatch.batchKey)"
        >
          <span v-if="batchReviewing" class="loading loading-spinner loading-xs"></span>
          {{ t("chat.toolReview.evaluateBatchWithCount", { count: currentBatchUnreviewedCount }) }}
        </button>
        <button
          type="button"
          class="btn btn-sm w-full"
          @click="handleReportAction"
        >
          {{ t("chat.toolReview.viewReviewReport") }}
        </button>
      </div>
    </div>
  </aside>

  <dialog class="modal" :class="{ 'modal-open': reportDialogOpen }">
    <div class="modal-box h-[90vh] w-[90vw] max-w-none p-0">
      <div class="flex items-center justify-between border-b border-base-300 px-4 py-3">
        <div class="text-sm">{{ t("chat.toolReview.reportTitle") }}</div>
        <button
          type="button"
          class="btn btn-sm btn-ghost"
          @click="closeReportDialog"
        >
          {{ t("chat.toolReview.closeChanges") }}
        </button>
      </div>
      <div class="assistant-markdown h-[calc(90vh-121px)] overflow-auto px-5 py-4">
        <div v-if="reportErrorText" class="mb-4 rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
          {{ reportErrorText }}
        </div>
        <div v-if="!currentBatch?.report && submitting" class="flex h-full min-h-0 items-center justify-center text-sm text-base-content/70">
          <span class="loading loading-spinner loading-sm mr-2"></span>
          {{ t("chat.toolReview.generatingReviewReport") }}
        </div>
        <div v-else-if="!currentBatch?.report" class="flex h-full min-h-0 items-center justify-center text-sm text-base-content/70">
          {{ t("chat.toolReview.reportUnavailable") }}
        </div>
        <MarkdownRender
          v-else
          class="ecall-markdown-content tool-review-report-markdown max-w-none"
          :nodes="reportMarkdownNodes"
          :is-dark="markdownIsDark"
          :code-block-props="markdownCodeBlockProps"
          :mermaid-props="markdownMermaidProps"
          :typewriter="false"
        />
      </div>
      <div class="flex items-center justify-end gap-3 border-t border-base-300 px-4 py-3">
        <button
          v-if="currentBatch?.report"
          type="button"
          class="btn btn-sm"
          :disabled="submitting || !currentBatch"
          @click="currentBatch && emit('submitBatch', currentBatch.batchKey)"
        >
          <span v-if="submitting" class="loading loading-spinner loading-xs"></span>
          {{ t("chat.toolReview.regenerateReport") }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="closeReportDialog">{{ t("chat.toolReview.closeChanges") }}</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref, useAttrs, watch } from "vue";
import { useI18n } from "vue-i18n";
import MarkdownRender, { enableKatex, enableMermaid, getMarkdown, parseMarkdownToStructure } from "markstream-vue";
import type { ToolReviewBatchSummary, ToolReviewItemDetail } from "../composables/use-chat-tool-review";
import { registerChatMarkstreamComponents } from "../markdown/register-chat-markstream";
import ToolReviewItemCard from "./ToolReviewItemCard.vue";

enableMermaid();
enableKatex();
registerChatMarkstreamComponents();

const markstreamMarkdown = getMarkdown();
const markdownCodeBlockProps = {
  showHeader: true,
  showCopyButton: true,
  showPreviewButton: false,
  showExpandButton: true,
  showCollapseButton: true,
  showFontSizeButtons: false,
  enableFontSizeControl: false,
  isShowPreview: false,
};
const markdownMermaidProps = {
  showHeader: true,
  showCopyButton: true,
  showExportButton: false,
  showFullscreenButton: true,
  showCollapseButton: false,
  showZoomControls: true,
  showModeToggle: false,
  enableWheelZoom: true,
};

const props = defineProps<{
  batches: ToolReviewBatchSummary[];
  currentBatchKey: string;
  detailMap: Record<string, ToolReviewItemDetail>;
  detailLoadingCallId: string;
  reviewingCallId: string;
  batchReviewingKey: string;
  submittingBatchKey: string;
  errorText: string;
  reportErrorText: string;
  markdownIsDark: boolean;
}>();

const emit = defineEmits<{
  (e: "selectBatch", batchKey: string): void;
  (e: "loadItemDetail", callId: string): void;
  (e: "reviewItem", callId: string): void;
  (e: "reviewBatch", batchKey: string): void;
  (e: "submitBatch", batchKey: string): void;
}>();

const { t } = useI18n();
const reportDialogOpen = ref(false);
const rootAttrs = useAttrs();

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

const batchReviewing = computed(() =>
  !!currentBatch.value && props.batchReviewingKey === currentBatch.value.batchKey
);

const submitting = computed(() =>
  !!currentBatch.value && props.submittingBatchKey === currentBatch.value.batchKey
);

const currentBatchUnreviewedCount = computed(() =>
  currentBatch.value?.items.filter((item) => !item.hasReview).length ?? 0
);

const reportMarkdownNodes = computed(() =>
  parseMarkdownToStructure(
    currentBatch.value?.report?.reportText || "",
    markstreamMarkdown,
    { final: true },
  )
);

watch(() => props.currentBatchKey, () => {
  reportDialogOpen.value = false;
});

function handleReportAction() {
  if (!currentBatch.value) return;
  reportDialogOpen.value = true;
  if (!currentBatch.value.report) {
    emit("submitBatch", currentBatch.value.batchKey);
  }
}

function closeReportDialog() {
  reportDialogOpen.value = false;
}
</script>

<style scoped>
.assistant-markdown :deep(.ecall-markdown-content.prose) {
  --tw-prose-body: currentColor;
  --tw-prose-headings: currentColor;
  --tw-prose-lead: currentColor;
  --tw-prose-links: hsl(var(--bc));
  --tw-prose-bold: currentColor;
  --tw-prose-counters: currentColor;
  --tw-prose-bullets: hsl(var(--bc) / 0.5);
  --tw-prose-hr: hsl(var(--bc) / 0.15);
  --tw-prose-quotes: currentColor;
  --tw-prose-quote-borders: hsl(var(--bc) / 0.2);
  --tw-prose-captions: hsl(var(--bc) / 0.75);
  --tw-prose-code: currentColor;
  --tw-prose-pre-code: currentColor;
  --tw-prose-pre-bg: hsl(var(--b2));
  --tw-prose-th-borders: hsl(var(--bc) / 0.2);
  --tw-prose-td-borders: hsl(var(--bc) / 0.15);
}

.assistant-markdown :deep(.ecall-markdown-content) {
  min-width: 0;
  max-width: 100%;
  overflow-x: hidden;
  font-size: 0.9rem;
  line-height: 1.5;
}

.assistant-markdown :deep(.ecall-markdown-content.markdown-renderer) {
  content-visibility: visible !important;
  contain: none !important;
  contain-intrinsic-size: auto !important;
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

.assistant-markdown :deep(.ecall-markdown-content :where(p,ul,ol,blockquote,pre,table,figure)) {
  margin-top: 0.25rem;
  margin-bottom: 0.25rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(h1,h2,h3,h4)) {
  margin-top: 0.7rem;
  margin-bottom: 0.32rem;
  font-weight: 600;
  line-height: 1.5;
  letter-spacing: -0.015em;
}

.assistant-markdown :deep(.ecall-markdown-content h1) {
  font-size: 1.12rem;
}

.assistant-markdown :deep(.ecall-markdown-content h2) {
  font-size: 1.04rem;
}

.assistant-markdown :deep(.ecall-markdown-content h3) {
  font-size: 0.98rem;
}

.assistant-markdown :deep(.ecall-markdown-content h4) {
  font-size: 0.94rem;
}

.assistant-markdown :deep(.ecall-markdown-content :where(ul,ol)) {
  padding-left: 1.05rem;
}

.assistant-markdown :deep(.ecall-markdown-content li) {
  margin: 0.12rem 0;
}

.assistant-markdown :deep(.ecall-markdown-content li > :where(p,ul,ol)) {
  margin-top: 0.16rem;
  margin-bottom: 0.16rem;
}

.assistant-markdown :deep(.ecall-markdown-content a) {
  text-decoration: underline;
  text-underline-offset: 0.18em;
  text-decoration-color: hsl(var(--bc) / 0.28);
}

.assistant-markdown :deep(.ecall-markdown-content a:hover) {
  text-decoration-color: hsl(var(--bc) / 0.5);
}

.assistant-markdown :deep(.ecall-markdown-content strong) {
  font-weight: 600;
}

.assistant-markdown :deep(.ecall-markdown-content blockquote) {
  border-left: 3px solid hsl(var(--bc) / 0.16);
  padding-left: 0.68rem;
  color: hsl(var(--bc) / 0.82);
}

.assistant-markdown :deep(.ecall-markdown-content hr) {
  border: 0;
  border-top: 1px solid hsl(var(--bc) / 0.14);
  margin: 0.65rem 0;
}

.assistant-markdown :deep(.ecall-markdown-content :not(pre) > code) {
  border: 1px solid hsl(var(--bc) / 0.12);
  background: hsl(var(--b2));
  border-radius: 0.4rem;
  padding: 0.08rem 0.3rem;
  font-size: 0.86em;
  font-weight: 500;
}

.assistant-markdown :deep(.ecall-markdown-content table) {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9rem;
}

.assistant-markdown :deep(.ecall-markdown-content th) {
  border-bottom: 1px solid hsl(var(--bc) / 0.16);
  padding: 0.36rem 0.5rem;
  text-align: left;
  font-weight: 600;
}

.assistant-markdown :deep(.ecall-markdown-content td) {
  border-bottom: 1px solid hsl(var(--bc) / 0.1);
  padding: 0.34rem 0.5rem;
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
