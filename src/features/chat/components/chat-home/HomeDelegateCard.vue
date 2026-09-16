<template>
  <CardShell
    tone="primary"
    :icon="Network"
    :label="t('chat.homePanel.delegateLabel')"
    interactive
    :pulsing="pulsing"
    @select="emit('open')"
  >
    <template #trailing>
      <span class="ecall-home-num shrink-0 text-xs text-base-content/45">{{ elapsedText }}</span>
    </template>
    <span class="line-clamp-2 text-xs leading-snug text-base-content/85">{{ title }}</span>
    <ArrowUpRight class="mt-auto size-3.5 self-end text-base-content/25" aria-hidden="true" />
  </CardShell>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowUpRight, Network } from "@lucide/vue";
import CardShell from "./CardShell.vue";

const props = withDefaults(defineProps<{
  title: string;
  elapsedMs?: number;
  requestCount?: number;
  tokenCount?: number;
  lastToolName?: string;
  /** 右上角自定义标签：给了就用它（历史委托显示状态），否则按 elapsedMs 显示时长 */
  metaLabel?: string;
  /** 运行中的委托卡显示脉冲点；已结束的委托卡不显示 */
  pulsing?: boolean;
}>(), {
  elapsedMs: 0,
  requestCount: 0,
  tokenCount: 0,
  lastToolName: "",
  metaLabel: "",
  pulsing: false,
});

const emit = defineEmits<{
  (e: "open"): void;
}>();

const { t } = useI18n();

const elapsedText = computed(() => props.metaLabel || formatElapsedMs(props.elapsedMs || 0));

function formatElapsedMs(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "";
  const totalSeconds = Math.floor(value / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}时${minutes}分`;
  if (minutes > 0) return `${minutes}分${seconds}秒`;
  return `${seconds}秒`;
}
</script>

<style scoped>
.ecall-home-num {
  font-variant-numeric: tabular-nums;
}
</style>
