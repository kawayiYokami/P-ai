<template>
  <details class="collapse collapse-arrow w-full rounded-none border-x-0 border-y border-base-300 bg-base-100" @toggle="handleToggle">
    <summary class="collapse-title min-h-0 px-4 py-3 pr-10">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <div class="truncate text-sm">{{ item.toolName }}</div>
          <div class="text-xs text-base-content/60">#{{ item.orderIndex }}</div>
        </div>
        <div class="badge badge-ghost badge-sm">
          {{ item.hasReview ? t("chat.toolReview.reviewed") : t("chat.toolReview.unreviewed") }}
        </div>
      </div>
    </summary>
    <div class="collapse-content flex flex-col gap-3 px-4">
      <div v-if="loading" class="flex items-center gap-2 text-sm text-base-content/65">
        <span class="loading loading-spinner loading-sm"></span>
        <span>{{ t("chat.loadMore") }}</span>
      </div>

      <template v-else-if="detail">
        <div class="whitespace-pre-wrap break-words text-sm leading-7">
          {{ detail.review?.reviewOpinion || t("chat.toolReview.reviewUnavailable") }}
        </div>
        <div class="flex items-center justify-end gap-3">
          <button
            type="button"
            class="btn btn-sm gap-1.5 font-normal"
            @click.prevent.stop="openChangesDialog"
          >
            <Eye class="h-4 w-4" />
            {{ t("chat.toolReview.viewChanges") }}
          </button>
          <button
            type="button"
            class="btn btn-sm gap-1.5 font-normal"
            :disabled="reviewing"
            @click.prevent.stop="$emit('review', item.callId)"
          >
            <span v-if="reviewing" class="loading loading-spinner loading-xs"></span>
            <RotateCcw v-else class="h-4 w-4" />
            {{ item.hasReview ? t("chat.toolReview.evaluateAgain") : t("chat.toolReview.evaluateNow") }}
          </button>
        </div>
      </template>
    </div>
  </details>
  <dialog ref="changesDialogRef" class="modal">
    <div class="modal-box h-[90vh] w-[90vw] max-w-none p-0">
      <div class="flex items-center justify-between border-b border-base-300 px-4 py-3">
        <div class="min-w-0">
          <div class="truncate text-sm">{{ item.toolName }}</div>
          <div class="text-xs text-base-content/60">#{{ item.orderIndex }}</div>
        </div>
        <button
          type="button"
          class="btn btn-sm btn-ghost"
          @click="closeChangesDialog"
        >
          {{ t("chat.toolReview.closeChanges") }}
        </button>
      </div>
      <div class="flex h-[calc(90vh-61px)] min-h-0 flex-col overflow-hidden">
        <ToolReviewCodePreview
          v-if="detail"
          :mode="detail.previewKind === 'patch' ? 'patch' : 'plain'"
          :title="detail.previewKind === 'patch' ? '' : t('chat.toolReview.commandPreview')"
          :code="detail.previewText || detail.resultText"
        />
        <ToolReviewCodePreview
          v-if="detail?.review?.rawContent"
          mode="plain"
          :title="t('chat.toolReview.rawReview')"
          :code="detail.review.rawContent"
        />
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button>{{ t("chat.toolReview.closeChanges") }}</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, RotateCcw } from "lucide-vue-next";
import type { ToolReviewItemDetail, ToolReviewItemSummary } from "../composables/use-chat-tool-review";
import ToolReviewCodePreview from "./ToolReviewCodePreview.vue";

const props = defineProps<{
  item: ToolReviewItemSummary;
  detail?: ToolReviewItemDetail;
  loading: boolean;
  reviewing: boolean;
}>();

const emit = defineEmits<{
  (e: "loadDetail", callId: string): void;
  (e: "review", callId: string): void;
}>();

const { t } = useI18n();
const changesDialogRef = ref<HTMLDialogElement | null>(null);

function handleToggle(event: Event) {
  const target = event.currentTarget as HTMLDetailsElement | null;
  if (!target?.open) return;
  emit("loadDetail", props.item.callId);
}

function openChangesDialog() {
  changesDialogRef.value?.showModal();
}

function closeChangesDialog() {
  changesDialogRef.value?.close();
}
</script>
