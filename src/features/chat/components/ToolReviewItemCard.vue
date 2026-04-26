<template>
  <details class="collapse collapse-arrow w-full rounded-box border border-base-300 bg-base-200" @toggle="handleToggle">
    <summary class="collapse-title px-3 py-3 pr-10 min-h-0">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0 flex items-center gap-2">
          <div class="shrink-0 text-sm text-base-content/60">#{{ item.orderIndex }}</div>
          <div class="truncate text-sm">{{ title }}</div>
        </div>
        <div
          class="badge badge-sm min-w-14 shrink-0 justify-center whitespace-nowrap"
          :class="item.hasReview ? 'badge-primary' : 'badge-warning'"
        >
          {{ item.hasReview ? t("chat.toolReview.evaluated") : t("chat.toolReview.unevaluated") }}
        </div>
      </div>
    </summary>
    <div class="collapse-content flex flex-col gap-3 px-3 pb-3">
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
            class="btn btn-sm gap-1.5 border-base-300 bg-base-100 font-normal hover:bg-base-100"
            @click.prevent.stop="openChangesDialog"
          >
            <Eye class="h-4 w-4" />
            {{ t("chat.toolReview.viewChanges") }}
          </button>
          <button
            type="button"
            class="btn btn-sm gap-1.5 border-base-300 bg-base-100 font-normal hover:bg-base-100"
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
  <ToolReviewChangesDialog
    ref="changesDialogRef"
    :title="title"
    :subtitle="`#${item.orderIndex}`"
    :show-preview="!!detail"
    :preview-mode="detail?.previewKind === 'patch' ? 'patch' : 'plain'"
    :preview-text="detail ? detail.previewText || detail.resultText : ''"
    :raw-review="detail?.review?.rawContent || ''"
  />
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, RotateCcw } from "lucide-vue-next";
import type { ToolReviewItemDetail, ToolReviewItemSummary } from "../composables/use-chat-tool-review";
import ToolReviewChangesDialog from "./ToolReviewChangesDialog.vue";

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
const changesDialogRef = ref<{ openChangesDialog: () => void; closeChangesDialog: () => void } | null>(null);

const title = computed(() => {
  if (props.item.toolName === "shell_exec") {
    return String(props.item.command || "").trim() || t("chat.toolReview.terminalCommand");
  }
  if (props.item.toolName === "apply_patch") {
    const fileName = patchFileName();
    const operation = patchOperationLabel(props.item.patchOperation);
    return fileName ? `${operation} ${fileName}` : operation;
  }
  return props.item.toolName;
});

function patchFileName() {
  const paths = Array.isArray(props.item.affectedPaths) ? props.item.affectedPaths : [];
  if (paths.length !== 1) return "";
  const normalized = String(paths[0] || "").replace(/\\/g, "/").trim();
  return normalized.split("/").filter(Boolean).pop() || "";
}

function patchOperationLabel(operation?: string) {
  if (operation === "add") return t("chat.toolReview.patchOperationAdd");
  if (operation === "delete") return t("chat.toolReview.patchOperationDelete");
  if (operation === "mixed") return t("chat.toolReview.patchOperationMixed");
  return t("chat.toolReview.patchOperationUpdate");
}

function handleToggle(event: Event) {
  const target = event.currentTarget as HTMLDetailsElement | null;
  if (!target?.open) return;
  emit("loadDetail", props.item.callId);
}

function openChangesDialog() {
  changesDialogRef.value?.openChangesDialog();
}
</script>
