<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type {
  TrimCompactionPreviewResult,
  TrimPreviewResult,
} from "../../composables/use-conversation-maintenance-dialog";

const props = defineProps<{
  open: boolean;
  loading: boolean;
  running: boolean;
  preview: TrimPreviewResult | null;
  compactionPreview: TrimCompactionPreviewResult | null;
}>();

const emit = defineEmits<{
  close: [];
  confirmCompaction: [];
  confirmArchive: [];
  confirmDelete: [];
}>();

const { t } = useI18n();
const dialogRef = ref<HTMLDialogElement | null>(null);

function onDialogClose() {
  if (props.loading || props.running) {
    const d = dialogRef.value;
    if (d && !d.open && props.open) d.showModal();
    return;
  }
  emit("close");
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

function formatTokens(value: number | undefined): string {
  if (value === undefined || !Number.isFinite(value)) return "—";
  const rounded = Math.round(value);
  if (rounded >= 1000) {
    const k = rounded / 1000;
    const text = Number.isInteger(k) ? String(k) : k.toFixed(1).replace(/\.0$/, "");
    return `~${text}K`;
  }
  return `~${rounded.toLocaleString("en-US")}`;
}

function percentOfWindow(tokens: number | undefined): number {
  const windowTokens = contextWindowTokens.value;
  if (windowTokens <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round((Math.max(0, Number(tokens) || 0) / windowTokens) * 100)));
}

type BreakdownEntry = {
  key: "system" | "tools" | "message" | "reserved" | "available";
  label: string;
  tokens: number | undefined;
  color: string;
};

/** 压缩阈值：超过 82% 触发压缩，18% 为预留给压缩的保留区。 */
const COMPACTION_THRESHOLD_RATIO = 0.82;

const breakdownEntries = computed<BreakdownEntry[]>(() => {
  const breakdown = props.compactionPreview?.tokenBreakdown;
  const entries: BreakdownEntry[] = [
    {
      key: "system",
      label: t("dialogs.trim.breakdownSystem"),
      tokens: breakdown?.systemTokens,
      color: "var(--color-primary)",
    },
    {
      key: "tools",
      label: t("dialogs.trim.breakdownTools"),
      tokens: breakdown?.toolsTokens,
      color: "var(--color-secondary)",
    },
    {
      key: "message",
      label: t("dialogs.trim.breakdownMessage"),
      tokens: breakdown?.messageTokens,
      color: "var(--color-accent)",
    },
  ];
  const windowTokens = Math.max(0, Number(breakdown?.contextWindowTokens) || 0);
  if (windowTokens <= 0) return entries;
  const usedTokens = entries.reduce((sum, entry) => sum + Math.max(0, Number(entry.tokens) || 0), 0);
  const reservedTokens = Math.round(windowTokens * (1 - COMPACTION_THRESHOLD_RATIO));
  entries.push({
    key: "reserved",
    label: t("dialogs.trim.breakdownReserved"),
    tokens: reservedTokens,
    color: "var(--color-warning)",
  });
  // 剩余可用 = 总 - 已用 - 保留
  entries.push({
    key: "available",
    label: t("dialogs.trim.breakdownAvailable"),
    tokens: Math.max(0, windowTokens - usedTokens - reservedTokens),
    color: "var(--color-base-300)",
  });
  return entries;
});

/** 已占用词元合计（不含保留区与剩余可用）。 */
const breakdownUsedTotal = computed(() =>
  breakdownEntries.value
    .filter((entry) => entry.key === "system" || entry.key === "tools" || entry.key === "message")
    .reduce((sum, entry) => sum + Math.max(0, Number(entry.tokens) || 0), 0),
);

/** 饼图整体：上下文窗口总量（含保留区与剩余可用）。 */
const breakdownTotal = computed(() =>
  breakdownEntries.value.reduce((sum, entry) => sum + Math.max(0, Number(entry.tokens) || 0), 0),
);

/** 比例条各分段的宽度：占上下文窗口总量的百分比。 */
function barWidthOf(entry: BreakdownEntry): string {
  const total = breakdownTotal.value;
  const tokens = Math.max(0, Number(entry.tokens) || 0);
  if (total <= 0) return "0%";
  return `${(tokens / total) * 100}%`;
}

const hasAnyBreakdown = computed(() => breakdownTotal.value > 0);

const contextWindowTokens = computed(() =>
  Math.max(0, Number(props.compactionPreview?.tokenBreakdown?.contextWindowTokens) || 0),
);

const compressionEstimate = computed(() => {
  const breakdown = props.compactionPreview?.tokenBreakdown;
  const systemTokens = Math.max(0, Number(breakdown?.systemTokens) || 0);
  const toolsTokens = Math.max(0, Number(breakdown?.toolsTokens) || 0);
  const messageTokens = Math.max(0, Number(breakdown?.messageTokens) || 0);
  const usedTotal = systemTokens + toolsTokens + messageTokens;
  if (usedTotal <= 0) return null;
  // 压缩只释放正文；系统提示词与工具 schema 是固定成本。
  const released = messageTokens;
  const windowTokens = Math.max(0, Number(breakdown?.contextWindowTokens) || 0);
  // 压缩后占用占比 = 固定成本 / 上下文窗口（保留区不算已用，不参与释放）。
  const afterRatio =
    windowTokens > 0 ? (systemTokens + toolsTokens) / windowTokens : usedTotal > 0 ? 1 : 0;
  return { released, afterRatio };
});

/** 压缩后预计占用占比（%）——固定成本占上下文窗口的比例。 */
const compressionEstimatePercent = computed(() => {
  const estimate = compressionEstimate.value;
  if (!estimate) return null;
  return Math.min(100, Math.max(0, Math.round(estimate.afterRatio * 100)));
});
</script>

<template>
  <dialog ref="dialogRef" class="modal" @close="onDialogClose" @cancel.prevent="onDialogClose">
    <div v-if="open" class="modal-box w-[min(80vw,40rem)] max-w-[40rem]">
      <div class="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between sm:gap-4">
        <h3 class="font-semibold text-base">{{ t("dialogs.trim.title") }}</h3>
        <div class="flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs opacity-60">
          <span>{{ t("dialogs.trim.messageCount", { count: preview?.messageCount ?? 0 }) }}</span>
          <span>{{ t("dialogs.trim.contextUsage", { percent: compactionPreview?.contextUsagePercent ?? 0 }) }}</span>
        </div>
      </div>

      <div v-if="loading" class="mt-4 text-sm opacity-70">{{ t("dialogs.trim.loading") }}</div>

      <!-- 词元账单：比例条 + 明细 + 压缩预期，直接平铺 -->
      <div v-else-if="hasAnyBreakdown" class="mt-4 space-y-3">
        <div class="flex h-3 w-full overflow-hidden rounded-full bg-base-300/30">
          <span
            v-for="entry in breakdownEntries"
            :key="entry.key"
            class="block h-full"
            :style="{ width: barWidthOf(entry), backgroundColor: entry.color }"
          />
        </div>
        <div class="space-y-1.5 text-sm">
          <div
            v-for="entry in breakdownEntries"
            :key="entry.key"
            class="grid grid-cols-[minmax(0,1fr)_5rem_3.5rem] items-center gap-3"
          >
            <span class="flex min-w-0 items-center gap-2">
              <span class="h-2.5 w-2.5 shrink-0 rounded-full" :style="{ backgroundColor: entry.color }" />
              <span class="truncate">{{ entry.label }}</span>
            </span>
            <span class="text-right tabular-nums text-base-content/80">{{ formatTokens(entry.tokens) }}</span>
            <span class="text-right tabular-nums text-base-content/60">
              <template v-if="contextWindowTokens > 0 && entry.tokens">{{ percentOfWindow(entry.tokens) }}%</template>
            </span>
          </div>
          <div class="grid grid-cols-[minmax(0,1fr)_5rem_3.5rem] items-center gap-3 border-t border-base-300 pt-1.5 text-xs text-base-content/60">
            <span>{{ t("dialogs.trim.breakdownUsed") }}</span>
            <span class="text-right tabular-nums">{{ formatTokens(breakdownUsedTotal) }}</span>
            <span />
          </div>
          <div
            v-if="contextWindowTokens > 0"
            class="grid grid-cols-[minmax(0,1fr)_5rem_3.5rem] items-center gap-3 text-xs text-base-content/60"
          >
            <span>{{ t("dialogs.trim.breakdownContextWindow") }}</span>
            <span class="text-right tabular-nums">{{ formatTokens(contextWindowTokens) }}</span>
            <span />
          </div>
        </div>
      </div>

      <div v-else class="mt-4 text-sm opacity-70">{{ t("dialogs.trim.breakdownEmpty") }}</div>

      <div
        v-if="compressionEstimate"
        class="mt-3 rounded bg-primary/10 px-3 py-2 text-xs text-primary"
      >
        {{ t("dialogs.trim.compressionEstimate", {
          percent: compressionEstimatePercent ?? 0,
          tokens: formatTokens(compressionEstimate.released),
        }) }}
      </div>
      <div
        v-else-if="!loading && compactionPreview?.compactionDisabledReason"
        class="mt-3 rounded bg-warning/10 px-3 py-2 text-sm text-warning"
      >
        {{ compactionPreview.compactionDisabledReason }}
      </div>

      <div class="mt-5 flex items-center justify-end gap-2">
        <button
          class="btn btn-sm btn-primary"
          :disabled="loading || !compactionPreview?.canCompact || running"
          @click="emit('confirmCompaction')"
        >
          {{ t("dialogs.trim.compactTitle") }}
        </button>
        <button class="btn btn-sm" :disabled="loading || running" @click="emit('close')">
          {{ t("common.cancel") }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button @click.prevent="onDialogClose">close</button>
    </form>
  </dialog>
</template>
