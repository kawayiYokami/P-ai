<template>
  <section class="last:mb-0 mx-1">
    <div
      class="group/card relative flex items-center gap-2 rounded-lg px-2 py-2 transition-colors hover:bg-base-100/70"
      @contextmenu.prevent="openContextMenu"
    >
      <button
        type="button"
        class="flex min-w-0 flex-1 items-center gap-2 text-left"
        :disabled="loading"
        @click="openChanges"
      >
        <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-base-100 text-xs font-semibold text-base-content/65">
          #{{ item.orderIndex }}
        </div>
        <div class="min-w-0 flex-1 overflow-hidden">
          <div class="flex min-w-0 items-start justify-between gap-2">
            <span class="block min-w-0 flex-1 truncate whitespace-nowrap text-xs font-normal text-base-content">{{ title }}</span>
            <div v-if="timeDateLabel" class="shrink-0 text-right text-xs leading-4 text-base-content/55">{{ timeDateLabel }}</div>
          </div>
          <div class="mt-1 flex min-w-0 items-start justify-between gap-2 text-xs text-base-content/65">
            <div class="min-w-0 flex-1 truncate">{{ reviewOpinionText }}</div>
            <div v-if="timeMinuteLabel" class="shrink-0 text-right leading-4">
              {{ timeMinuteLabel }}
            </div>
          </div>
        </div>
        <AppLoading v-if="loading" size="xs" class="shrink-0 text-base-content/55" />
      </button>
      <div v-if="item.isSuccess === false" class="shrink-0">
        <span v-if="isDenied" class="badge badge-warning badge-xs font-normal">{{ t("chat.toolReview.denied") }}</span>
        <span v-else class="badge badge-error badge-xs font-normal">{{ t("chat.toolReview.failed") }}</span>
      </div>
      <div v-else-if="item.hasReview" class="shrink-0 text-xs leading-5 text-base-content/45">
        {{ t("chat.toolReview.evaluated") }}
      </div>
      <div
        v-else
        class="shrink-0 text-xs leading-5 text-base-content/45"
      >
        {{ t("chat.toolReview.unevaluated") }}
      </div>

      <div
        v-if="contextMenuOpen"
        class="fixed z-50 w-36 rounded-box border border-base-300 bg-base-100 p-1 shadow-xl"
        :style="{ left: `${contextMenuPosition.x}px`, top: `${contextMenuPosition.y}px` }"
        @pointerdown.stop
      >
        <button
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          @click.stop="handleViewAction"
        >
          {{ t("chat.toolReview.view") }}
        </button>
        <button
          type="button"
          class="btn btn-ghost btn-sm h-8 w-full justify-start px-2 text-sm font-normal"
          :disabled="item.hasReview || item.isSuccess === false || loading || reviewing"
          @click.stop="handleReviewAction"
        >
          <AppLoading v-if="reviewing" size="xs" />
          <span v-else>{{ t("chat.toolReview.evaluate") }}</span>
        </button>
      </div>
    </div>
  </section>

  <ToolReviewChangesDialog
    ref="changesDialogRef"
    :title="title"
    :subtitle="`#${item.orderIndex}`"
    :show-preview="!!detail"
    :preview-mode="detail?.previewKind === 'patch' ? 'patch' : 'plain'"
    :preview-text="detail ? detail.previewText || detail.resultText : ''"
    :review-opinion="dialogReviewOpinion"
    :review-allow="detail?.review?.allow"
    :review-model-name="detail?.review?.modelName || ''"
    :is-dark="isDark"
  />
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppLoading from "../../shared/components/AppLoading.vue";
import type { ToolReviewItemDetail, ToolReviewItemSummary } from "../composables/use-chat-tool-review";
import ToolReviewChangesDialog from "./ToolReviewChangesDialog.vue";
import { formatConversationListTimeWithMinuteDetails } from "../utils/conversation-time";

const props = withDefaults(defineProps<{
  item: ToolReviewItemSummary;
  detail?: ToolReviewItemDetail;
  loading: boolean;
  reviewing?: boolean;
  isDark?: boolean;
}>(), {
  detail: undefined,
  reviewing: false,
  isDark: false,
});

const emit = defineEmits<{
  (e: "loadDetail", callId: string): void;
  (e: "review", callId: string): void;
}>();

const { t, locale } = useI18n();
const changesDialogRef = ref<{ openChangesDialog: () => void; closeChangesDialog: () => void } | null>(null);
const pendingOpen = ref(false);
const contextMenuOpen = ref(false);
const contextMenuPosition = ref({ x: 0, y: 0 });

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

const title = computed(() => {
  if (isTerminalTool(props.item.toolName)) {
    return String(props.item.command || "").trim() || t("chat.toolReview.terminalCommand");
  }
  if (isFileChangeTool(props.item.toolName)) {
    const fileName = patchFileName();
    const operation = patchOperationLabel(props.item.patchOperation);
    return fileName ? `${operation} ${fileName}` : operation;
  }
  return props.item.toolName;
});

const isDenied = computed(() => {
  if (props.item.isDenied) return true;
  if (props.item.isSuccess === false) {
    const reason = String(props.item.blockedReason || "").toLowerCase();
    return reason.includes("denied") || reason.includes("rejected") || reason.includes("refused") || reason === "user_denied";
  }
  return false;
});

const reviewOpinionText = computed(() => {
  if (props.item.isSuccess === false) {
    if (isDenied.value) {
      return props.item.blockedReason === "user_denied_apply_patch"
        ? "用户拒绝了本次变更应用"
        : (props.item.blockedReason === "user_denied_command"
          ? "用户拒绝了本次命令执行"
          : t("chat.toolReview.denied"));
    }
    return props.item.blockedReason || t("chat.toolReview.failed");
  }
  const direct = props.detail?.review?.reviewOpinion;
  if (direct && direct.trim()) return direct;
  const summaryOpinion = String(props.item.reviewOpinion || "").trim();
  if (summaryOpinion) return summaryOpinion;
  if (props.item.hasReview && !props.detail) return t("chat.toolReview.evaluated");
  return props.item.hasReview ? t("chat.toolReview.reviewUnavailable") : t("chat.toolReview.unevaluated");
});

const dialogReviewOpinion = computed(() => String(props.detail?.review?.reviewOpinion || props.item.reviewOpinion || "").trim());
const timeParts = computed(() => {
  const raw = String(props.item.finishedAt || "").trim();
  return raw ? formatConversationListTimeWithMinuteDetails(raw, locale.value) : null;
});
const timeDateLabel = computed(() => timeParts.value?.dateLabel || "");
const timeMinuteLabel = computed(() => timeParts.value?.timeLabel || "");

watch(() => props.detail, (detail) => {
  if (!pendingOpen.value || !detail) return;
  pendingOpen.value = false;
  changesDialogRef.value?.openChangesDialog();
});

function openChanges() {
  closeContextMenu();
  if (props.detail) {
    changesDialogRef.value?.openChangesDialog();
    return;
  }
  pendingOpen.value = true;
  emit("loadDetail", props.item.callId);
}

function openContextMenu(event: MouseEvent) {
  contextMenuPosition.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 160)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 96)),
  };
  contextMenuOpen.value = true;
}

function closeContextMenu() {
  contextMenuOpen.value = false;
}

function handleViewAction() {
  closeContextMenu();
  openChanges();
}

function handleReviewAction() {
  closeContextMenu();
  if (props.item.hasReview || props.loading || props.reviewing) return;
  emit("review", props.item.callId);
}

function handleGlobalPointerDown() {
  closeContextMenu();
}

function handleGlobalEscape(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeContextMenu();
  }
}

onMounted(() => {
  window.addEventListener("pointerdown", handleGlobalPointerDown);
  window.addEventListener("keydown", handleGlobalEscape);
});

onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", handleGlobalPointerDown);
  window.removeEventListener("keydown", handleGlobalEscape);
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
</script>
