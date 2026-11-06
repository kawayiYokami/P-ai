<template>
  <div class="flex min-h-0 flex-1 flex-col py-2">
    <div v-if="errorText" class="mx-4 my-3 rounded-box border border-error/30 bg-error/10 px-3 py-2 text-sm text-error">
      {{ errorText }}
    </div>

    <div v-else-if="!normalizedConversationId" class="flex min-h-0 flex-1 items-center justify-center px-4 py-8 text-sm text-base-content/65">
      {{ t("chat.fastRequest.noConversation") }}
    </div>

    <div v-else-if="loading && sortedTurns.length === 0" class="flex min-h-0 flex-1 items-center justify-center px-4 py-8">
      <span class="loading loading-spinner loading-sm text-base-content/45"></span>
    </div>

    <div v-else-if="sortedTurns.length === 0" class="flex min-h-0 flex-1 items-center justify-center px-4 py-8 text-sm text-base-content/65">
      {{ t("chat.fastRequest.empty") }}
    </div>

    <template v-else>
      <CollapsibleGroup
        v-for="section in fastRequestSections"
        :key="section.key"
        :title="section.title"
        :count="section.items.length"
        :model-value="isFastRequestSectionCollapsed(section.key)"
        @update:model-value="toggleFastRequestSection(section.key)"
        @collapse-all="collapseAllFastRequestSections"
      >
        <div v-if="!isFastRequestSectionCollapsed(section.key)">
          <section
            v-for="turn in section.items"
            :key="turnKey(turn)"
            class="last:mb-0 mx-1"
          >
            <div class="group/card rounded-lg px-2 py-2 transition-colors hover:bg-base-100/70">
              <button
                type="button"
                class="flex w-full min-w-0 items-center gap-2 text-left"
                :title="turnItemTitle(turn)"
                @click="openTurnDialog(turn)"
              >
                <span
                  class="flex h-10 w-10 shrink-0 items-center justify-center rounded-full bg-base-100"
                  :class="turn.success ? 'text-success' : 'text-error'"
                >
                  <AppIcon name="check-circle" v-if="turn.success" class="h-4 w-4" />
                  <AppIcon name="x-circle" v-else class="h-4 w-4" />
                </span>
                <div class="min-w-0 flex-1 overflow-hidden">
                  <div class="flex min-w-0 items-start justify-between gap-2">
                    <span class="block min-w-0 flex-1 truncate whitespace-nowrap text-xs font-normal text-base-content">
                      {{ kindLabel(turn.kind) }}
                    </span>
                    <div v-if="timeLabel(turn.createdAt).dateLabel" class="shrink-0 text-right text-xs leading-4 text-base-content/55">
                      {{ timeLabel(turn.createdAt).dateLabel }}
                    </div>
                  </div>
                  <div class="mt-1 flex min-w-0 items-start justify-between gap-2 text-xs text-base-content/65">
                    <div class="min-w-0 flex-1 truncate">
                      {{ turnMetaLabel(turn) }}
                    </div>
                    <div v-if="timeLabel(turn.createdAt).timeLabel" class="shrink-0 text-right leading-4">
                      {{ timeLabel(turn.createdAt).timeLabel }}
                    </div>
                  </div>
                </div>
              </button>
            </div>
          </section>
        </div>
      </CollapsibleGroup>
    </template>
  </div>

  <dialog ref="selectedTurnDialogRef" class="modal" @close="closeTurnDialog" @cancel.prevent="closeTurnDialog">
    <div class="modal-box max-h-[80vh] max-w-2xl overflow-y-auto">
      <div class="mb-3 flex items-start justify-between gap-3">
        <div v-if="selectedTurn" class="min-w-0">
          <div class="truncate text-sm font-semibold text-base-content">{{ kindLabel(selectedTurn.kind) }}</div>
          <div class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-base-content/55">
            <span>{{ turnMetaLabel(selectedTurn) }}</span>
            <span v-if="selectedTurn.createdAt">{{ timeLabel(selectedTurn.createdAt).dateLabel }}</span>
            <span v-if="timeLabel(selectedTurn.createdAt).timeLabel">{{ timeLabel(selectedTurn.createdAt).timeLabel }}</span>
          </div>
        </div>
        <button type="button" class="btn btn-ghost btn-sm h-8 min-h-8 w-8 min-w-8 p-0" @click="closeTurnDialog">×</button>
      </div>

      <div v-if="selectedTurn" class="space-y-3">
        <section class="rounded-lg bg-base-200/70">
          <div class="px-2 py-1.5 text-xs font-medium text-base-content/70">
            {{ t("chat.fastRequest.request") }}
          </div>
          <pre class="max-h-56 overflow-auto whitespace-pre-wrap break-words px-2 pb-2 text-xs leading-5 text-base-content/75">{{ displayText(selectedTurn.requestText) }}</pre>
        </section>
        <section class="rounded-lg bg-base-200/70">
          <div class="px-2 py-1.5 text-xs font-medium text-base-content/70">
            {{ selectedTurn.success ? t("chat.fastRequest.response") : t("chat.fastRequest.error") }}
          </div>
          <pre class="max-h-56 overflow-auto whitespace-pre-wrap break-words px-2 pb-2 text-xs leading-5 text-base-content/75">{{ displayText(selectedTurn.success ? selectedTurn.responseText : (selectedTurn.error || selectedTurn.responseText)) }}</pre>
        </section>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="closeTurnDialog">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { FastRequestTurn } from "../../../types/app";
import { toErrorMessage } from "../../../utils/error";
import { formatConversationListTimeWithMinuteDetails } from "../utils/conversation-time";
import CollapsibleGroup from "./CollapsibleGroup.vue";

const props = defineProps<{
  conversationId: string;
  active?: boolean;
}>();

const { t, locale } = useI18n();
const turns = ref<FastRequestTurn[]>([]);
const loading = ref(false);
const errorText = ref("");
const collapsedFastRequestSectionKeys = ref<Record<string, boolean>>({});
const selectedTurnDialogRef = ref<HTMLDialogElement | null>(null);
const selectedTurn = ref<FastRequestTurn | null>(null);

function syncSelectedTurnDialog() {
  const d = selectedTurnDialogRef.value;
  if (!d) return;
  if (selectedTurn.value) {
    if (!d.open) d.showModal();
  } else if (d.open) d.close();
}

watch(selectedTurn, syncSelectedTurnDialog);
watch(selectedTurnDialogRef, syncSelectedTurnDialog);
let requestSeq = 0;

type FastRequestSection = {
  key: string;
  title: string;
  items: FastRequestTurn[];
  order: number;
};

const normalizedConversationId = computed(() => String(props.conversationId || "").trim());

const sortedTurns = computed(() =>
  turns.value
    .slice()
    .sort((left, right) => timestamp(right.createdAt) - timestamp(left.createdAt)),
);

const fastRequestSections = computed<FastRequestSection[]>(() => {
  const sections = new Map<string, FastRequestSection>();
  for (const turn of sortedTurns.value) {
    const key = fastRequestSectionKey(turn.kind);
    const section = sections.get(key) || {
      key,
      title: kindLabel(turn.kind),
      items: [],
      order: fastRequestKindOrder(turn.kind),
    };
    section.items.push(turn);
    sections.set(key, section);
  }
  return Array.from(sections.values()).sort((left, right) => {
    if (left.order !== right.order) return left.order - right.order;
    return left.title.localeCompare(right.title, locale.value);
  });
});

async function loadTurns() {
  const conversationId = normalizedConversationId.value;
  const seq = ++requestSeq;
  errorText.value = "";
  if (!conversationId) {
    turns.value = [];
    return;
  }
  loading.value = true;
  try {
    const result = await invokeTauri<FastRequestTurn[]>("conversation.fastRequestTurns", { conversationId }, 10000);
    if (seq !== requestSeq) return;
    turns.value = Array.isArray(result) ? result.map(normalizeTurn) : [];
  } catch (error) {
    if (seq !== requestSeq) return;
    errorText.value = t("chat.fastRequest.loadFailed", { error: toErrorMessage(error) });
  } finally {
    if (seq === requestSeq) {
      loading.value = false;
    }
  }
}

function normalizeTurn(turn: FastRequestTurn): FastRequestTurn {
  return {
    id: String(turn?.id || ""),
    kind: String(turn?.kind || ""),
    requestText: String(turn?.requestText || ""),
    responseText: String(turn?.responseText || ""),
    success: !!turn?.success,
    error: turn?.error ? String(turn.error) : null,
    modelName: turn?.modelName ? String(turn.modelName) : null,
    durationMs: turn?.durationMs === null || turn?.durationMs === undefined
      ? null
      : (Number.isFinite(Number(turn.durationMs)) ? Number(turn.durationMs) : null),
    createdAt: String(turn?.createdAt || ""),
  };
}

function kindLabel(kind: string) {
  const normalized = normalizeKind(kind);
  if (normalized === "remote_im") return t("chat.fastRequest.kindRemoteIm");
  if (normalized === "remote_im_reply_decision") return t("chat.fastRequest.kindRemoteImReplyDecision");
  if (normalized === "remote_im_reply_rewrite") return t("chat.fastRequest.kindRemoteImReplyRewrite");
  if (normalized === "title_generation") return t("chat.fastRequest.kindTitleGeneration");
  if (normalized === "task_optimization") return t("chat.fastRequest.kindTaskOptimization");
  if (normalized === "tool_review") return t("chat.fastRequest.kindToolReview");
  if (normalized === "vision_image_description") return t("chat.fastRequest.kindVisionImageDescription");
  return normalized || t("chat.fastRequest.unknownKind");
}

function normalizeKind(kind: string) {
  return String(kind || "").trim();
}

function fastRequestSectionKey(kind: string) {
  return `kind:${normalizeKind(kind) || "unknown"}`;
}

function fastRequestKindOrder(kind: string) {
  const normalized = normalizeKind(kind);
  if (normalized === "remote_im_reply_decision") return 0;
  if (normalized === "remote_im_reply_rewrite") return 1;
  if (normalized === "remote_im") return 2;
  if (normalized === "title_generation") return 3;
  if (normalized === "task_optimization") return 4;
  if (normalized === "tool_review") return 5;
  return 99;
}

function isFastRequestSectionCollapsed(key: string) {
  return !!collapsedFastRequestSectionKeys.value[key];
}

function toggleFastRequestSection(key: string) {
  collapsedFastRequestSectionKeys.value = {
    ...collapsedFastRequestSectionKeys.value,
    [key]: !collapsedFastRequestSectionKeys.value[key],
  };
}

function collapseAllFastRequestSections() {
  collapsedFastRequestSectionKeys.value = fastRequestSections.value.reduce((next, section) => {
    next[section.key] = true;
    return next;
  }, { ...collapsedFastRequestSectionKeys.value } as Record<string, boolean>);
}

function turnKey(turn: FastRequestTurn) {
  return turn.id || `${turn.createdAt}:${turn.kind}:${String(turn.requestText || "").slice(0, 32)}`;
}

function openTurnDialog(turn: FastRequestTurn) {
  selectedTurn.value = turn;
}

function closeTurnDialog() {
  selectedTurn.value = null;
}

function durationLabel(value: number | null | undefined) {
  const ms = Number(value);
  if (!Number.isFinite(ms) || ms < 0) return "";
  return t("chat.fastRequest.durationMs", { ms: Math.round(ms) });
}

function turnMetaLabel(turn: FastRequestTurn) {
  const parts = [
    turn.success ? t("chat.fastRequest.success") : t("chat.fastRequest.failed"),
    durationLabel(turn.durationMs),
    String(turn.modelName || "").trim(),
  ].filter((item) => String(item || "").trim());
  return parts.join(" · ");
}

function turnItemTitle(turn: FastRequestTurn) {
  return `${kindLabel(turn.kind)}\n${turnMetaLabel(turn)}`;
}

function timeLabel(raw: string) {
  const value = String(raw || "").trim();
  return value ? formatConversationListTimeWithMinuteDetails(value, locale.value) : { dateLabel: "", timeLabel: "" };
}

function timestamp(raw: string) {
  const time = new Date(String(raw || "")).getTime();
  return Number.isFinite(time) ? time : 0;
}

function displayText(text: string | null | undefined) {
  const value = String(text || "").trim();
  return value || "-";
}

watch(
  () => [props.active !== false, normalizedConversationId.value] as const,
  ([active]) => {
    selectedTurn.value = null;
    if (!active) return;
    void loadTurns();
  },
  { immediate: true },
);
</script>
