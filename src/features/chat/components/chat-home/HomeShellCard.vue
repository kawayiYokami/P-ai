<template>
  <CardShell
    tone="success"
    :icon="SquareTerminal"
    :label="t('chat.homePanel.shellLabel')"
    pulsing
  >
    <template #trailing>
      <span class="shrink-0 text-xs text-base-content/45">{{ elapsedText }}</span>
    </template>
    <span class="line-clamp-2 text-sm leading-snug text-base-content/85">{{ description || command }}</span>
    <span class="mt-auto truncate font-mono text-xs text-base-content/35" :title="command">{{ command }}</span>
  </CardShell>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { SquareTerminal } from "@lucide/vue";
import CardShell from "./CardShell.vue";

const props = withDefaults(defineProps<{
  description?: string;
  command?: string;
  startedAt?: string;
  /** 由容器统一驱动的时钟，避免每张卡片各起一个定时器 */
  nowMs?: number;
}>(), {
  description: "",
  command: "",
  startedAt: "",
  nowMs: 0,
});

const { t } = useI18n();

const elapsedText = computed(() => {
  const startedMs = Date.parse(String(props.startedAt || ""));
  const nowMs = Number(props.nowMs || 0);
  if (!Number.isFinite(startedMs) || startedMs <= 0 || !nowMs) return "";
  return formatDurationMs(nowMs - startedMs);
});

function formatDurationMs(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0秒";
  const totalSeconds = Math.floor(value / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}时${minutes}分`;
  if (minutes > 0) return `${minutes}分${seconds}秒`;
  return `${seconds}秒`;
}
</script>
