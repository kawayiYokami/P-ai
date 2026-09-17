<template>
  <aside class="flex h-full min-h-0 flex-col overflow-x-hidden">
    <div v-if="errorText" class="mx-4 my-4 rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
      {{ errorText }}
    </div>
    <div v-else-if="statuses.length === 0" class="flex min-h-0 flex-1 items-center justify-center px-4 py-8 text-sm text-base-content/65">
      {{ t("chat.toolReview.delegateEmpty") }}
    </div>
    <div v-else class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 py-3">
      <section
        v-for="delegate in statuses"
        :key="delegate.delegateId"
        class="w-full min-w-0 rounded-box border border-base-300 bg-base-200 px-3 py-3"
      >
        <div class="flex min-w-0 items-center justify-between gap-3">
          <div class="min-w-0 truncate text-sm font-medium text-base-content/85">
            {{ delegate.title || delegate.delegateId }}
          </div>
          <div class="badge badge-sm shrink-0 whitespace-nowrap" :class="delegateStatusBadgeClass(delegate.status)">
            {{ formatDelegateStatus(delegate.status) }}
          </div>
        </div>
        <div class="mt-3 flex flex-col gap-2 text-xs text-base-content/70">
          <div class="flex min-w-0 items-center justify-between gap-3">
            <span class="shrink-0">{{ t('chat.delegateStatus.elapsed', { elapsed: formatElapsedMs(delegate.elapsedMs) }) }}</span>
            <span class="shrink-0">{{ t('chat.delegateStatus.steps', { count: delegate.requestCount }) }}</span>
          </div>
          <div class="flex min-w-0 items-center justify-between gap-3">
            <span class="min-w-0 truncate">{{ t('chat.delegateStatus.lastTool', { name: delegate.lastToolName || '-' }) }}</span>
            <span class="shrink-0">{{ t('chat.delegateStatus.usage', { tokens: formatTokenK(delegate.tokenCount) }) }}</span>
          </div>
        </div>
        <div class="mt-3 flex justify-end gap-2">
          <button
            v-if="isDelegateRunning(delegate.status)"
            type="button"
            class="btn btn-sm btn-error btn-outline gap-1.5 font-normal"
            @click="emit('abortDelegate', delegate)"
          >{{ t('chat.delegateStatus.abort') }}</button>
          <button
            type="button"
            class="btn btn-sm gap-1.5 border-base-300 bg-base-100 font-normal hover:bg-base-100"
            @click="openDetailWithTimeline(delegate)"
          >{{ t('chat.delegateStatus.viewDetail') }}</button>
        </div>
      </section>
    </div>
  </aside>
  <dialog ref="detailDialogRef" class="modal" @close="closeDetailDialog" @cancel.prevent="closeDetailDialog">
    <div class="modal-box flex max-h-[80vh] max-w-2xl flex-col overflow-hidden p-0">
      <div class="flex shrink-0 items-center justify-between gap-3 border-b border-base-300 px-5 py-3">
        <div class="min-w-0 truncate text-sm font-semibold text-base-content">{{ detailDialogTitle }}</div>
        <button type="button" class="btn btn-ghost btn-sm gap-1 shrink-0" @click="closeDetailDialog"><span class="text-base leading-none">×</span><span>关闭</span></button>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        <DelegateScheduleTimeline
          v-if="detailDialogConversationId"
          :conversation-id="detailDialogConversationId"
          :auto-refresh-key="detailDialogRefreshKey"
        />
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="closeDetailDialog">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { ConversationDelegateStatusSummary } from "../../../types/app";
import DelegateScheduleTimeline from "./DelegateScheduleTimeline.vue";

defineProps<{
  statuses: ConversationDelegateStatusSummary[];
  errorText: string;
}>();

const emit = defineEmits<{
  (e: "openDelegateDetail", status: ConversationDelegateStatusSummary): void;
  (e: "abortDelegate", status: ConversationDelegateStatusSummary): void;
}>();

const { t } = useI18n();

const detailDialogRef = ref<HTMLDialogElement | null>(null);
const detailDialogOpen = ref(false);

function closeDetailDialog() {
  detailDialogOpen.value = false;
}

function syncDetailDialog() {
  const d = detailDialogRef.value;
  if (!d) return;
  if (detailDialogOpen.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(detailDialogOpen, syncDetailDialog);
watch(detailDialogRef, syncDetailDialog);
const detailDialogTitle = ref("");
const detailDialogConversationId = ref("");
const detailDialogRefreshKey = ref("");

function openDetailWithTimeline(status: ConversationDelegateStatusSummary) {
  const conversationId = String(status?.conversationId || status?.delegateId || "").trim();
  if (!conversationId) return;
  detailDialogTitle.value = String(status?.title || status?.delegateId || "委托详情");
  detailDialogConversationId.value = conversationId;
  detailDialogRefreshKey.value = `${status.status}:${status.updatedAt}`;
  detailDialogOpen.value = true;
}

function formatDelegateStatus(status: string) {
  if (status === "running" || status === "delivered") return t('chat.delegateStatus.statusRunning');
  if (status === "completed") return t('chat.delegateStatus.statusCompleted');
  if (status === "failed") return t('chat.delegateStatus.statusFailed');
  return t('chat.delegateStatus.statusUnknown');
}

function isDelegateRunning(status: string) {
  return status === "running" || status === "delivered";
}

function delegateStatusBadgeClass(status: string) {
  if (status === "completed") return "badge-primary";
  if (status === "failed") return "badge-error";
  if (status === "running" || status === "delivered") return "badge-warning";
  return "badge-ghost";
}

function formatTokenK(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0K";
  const k = value / 1000;
  if (k < 10) return `${k.toFixed(1)}K`;
  return `${Math.round(k)}K`;
}

function formatElapsedMs(value: number) {
  if (!Number.isFinite(value) || value <= 0) return t('chat.delegateStatus.durationZero');
  const totalSeconds = Math.floor(value / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return t('chat.delegateStatus.durationHoursMinutes', { hours, minutes });
  if (minutes > 0) return t('chat.delegateStatus.durationMinutesSeconds', { minutes, seconds });
  return t('chat.delegateStatus.durationSeconds', { seconds });
}
</script>
