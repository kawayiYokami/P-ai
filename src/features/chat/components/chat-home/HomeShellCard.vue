<template>
  <CardShell
    :col-span="colSpan"
    :row-span="rowSpan"
    tone="success"
    :icon="SquareTerminal"
    :label="t('chat.homePanel.shellLabel')"
    pulsing
  >
    <template #trailing>
      <button
        v-if="interruptible"
        type="button"
        class="btn btn-ghost btn-xs btn-circle -my-0.5 shrink-0 border border-base-300 bg-base-100 text-base-content/60 hover:text-error"
        :title="t('chat.homePanel.shellInterrupt')"
        :aria-label="t('chat.homePanel.shellInterrupt')"
        @click.stop="emit('interrupt')"
      >
        <Square class="size-3" />
      </button>
      <span class="shrink-0 text-xs text-base-content/45">{{ elapsedText }}</span>
    </template>
    <span class="line-clamp-2 text-sm leading-snug text-base-content/85">{{ description || command }}</span>
    <span class="mt-auto truncate font-mono text-xs text-base-content/35" :title="command">{{ command }}</span>
  </CardShell>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Square, SquareTerminal } from "@lucide/vue";
import CardShell from "./CardShell.vue";

const props = withDefaults(defineProps<{
  description?: string;
  command?: string;
  startedAt?: string;
  /** 运行中允许强制打断；为 true 时右上角常驻打断按钮 */
  interruptible?: boolean;
  /** 由容器统一驱动的时钟，避免每张卡片各起一个定时器 */
  nowMs?: number;
  /** 占几列：宿主 ChatHomePanel 算好下发 */
  colSpan?: number;
  /** 占几行：宿主 ChatHomePanel 算好下发 */
  rowSpan?: number;
}>(), {
  description: "",
  command: "",
  startedAt: "",
  interruptible: false,
  nowMs: 0,
  colSpan: 1,
  rowSpan: 1,
});

const emit = defineEmits<{
  (e: "interrupt"): void;
}>();

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
