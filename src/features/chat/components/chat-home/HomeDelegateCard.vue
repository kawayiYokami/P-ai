<template>
  <CardShell
    :col-span="colSpan"
    :row-span="rowSpan"
    tone="primary"
    icon="network"
    :label="starterDisplayName"
    interactive
    @select="emit('open')"
  >
    <template #leading>
      <div class="flex min-w-0 flex-1 items-center gap-1.5">
        <img
          v-if="starterAvatarUrl"
          :src="starterAvatarUrl"
          :alt="starterDisplayName"
          :title="starterDisplayName"
          class="size-4 shrink-0 rounded-full object-cover ring-1 ring-base-content/10"
        />
        <span
          v-else
          class="flex size-4 shrink-0 items-center justify-center rounded-full bg-primary/10 text-primary"
        >
          <AppIcon name="network" class="size-2.5" />
        </span>
        <span class="truncate text-xs font-semibold text-base-content/80" :title="starterDisplayName">
          {{ starterDisplayName }}
        </span>
      </div>
    </template>

    <template #trailing>
      <div class="flex shrink-0 items-center gap-1.5">
        <span class="ecall-home-num font-mono text-xs text-base-content/45" :title="elapsedTitle">
          {{ elapsedText }}
        </span>
        <span v-if="pulsing" class="relative flex size-2 shrink-0 items-center justify-center" title="运行中">
          <span class="absolute inline-flex size-full animate-ping rounded-full bg-primary/75 opacity-75" />
          <span class="relative inline-flex size-1.5 rounded-full bg-primary" />
        </span>
        <span
          v-else
          class="inline-flex size-1.5 shrink-0 rounded-full"
          :class="statusDotClass"
          :title="metaLabel || '已结束'"
        />
      </div>
    </template>

    <div class="my-auto min-w-0 py-1">
      <span class="line-clamp-3 text-xs leading-relaxed font-medium text-base-content/85" :title="title">
        {{ title }}
      </span>
    </div>

    <div class="mt-auto flex items-center justify-between gap-1 border-t border-base-content/5 pt-1.5 text-[11px] text-base-content/45">
      <div class="flex min-w-0 items-center gap-1.5 font-mono">
        <span v-if="toolCallCount > 0" class="flex items-center gap-0.5 shrink-0" :title="`${toolCallCount} 次工具调用`">
          <AppIcon name="wrench" class="size-3 text-base-content/40" aria-hidden="true" />
          <span class="ecall-home-num">{{ toolCallCount }}</span>
        </span>
        <span v-if="toolCallCount > 0 && tokenText" class="text-base-content/20 shrink-0">·</span>
        <span v-if="tokenText" class="ecall-home-num truncate" :title="tokenTitle">
          {{ tokenText }}
        </span>
        <span v-if="toolCallCount === 0 && !tokenText" class="text-base-content/30 font-sans">
          {{ pulsing ? '运行中' : (metaLabel || '已结束') }}
        </span>
      </div>
      <AppIcon
        name="arrow-up-right"
        class="size-3.5 shrink-0 text-base-content/25 transition-transform duration-150 group-hover:translate-x-0.5 group-hover:-translate-y-0.5 group-hover:text-base-content/60"
        aria-hidden="true"
      />
    </div>
  </CardShell>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../../shared/components/AppIcon.vue";
import CardShell from "./CardShell.vue";

const props = withDefaults(defineProps<{
  title: string;
  elapsedMs?: number;
  requestCount?: number;
  toolCallCount?: number;
  tokenCount?: number;
  lastToolName?: string;
  /** 右上角自定义标签：给了就用它（历史委托显示状态），否则按 elapsedMs 显示时长 */
  metaLabel?: string;
  /** 运行中的委托卡显示脉冲点；已结束的委托卡不显示 */
  pulsing?: boolean;
  /** 占几列：宿主 ChatHomePanel 算好下发 */
  colSpan?: number;
  /** 占几行：宿主 ChatHomePanel 算好下发 */
  rowSpan?: number;
  /** 发起这次委托的人格头像（不是被派的那位）；没有就不渲染 */
  starterAvatarUrl?: string;
  /** 发起者角色名，只用于头像的 hover 提示与 alt */
  starterName?: string;
}>(), {
  elapsedMs: 0,
  requestCount: 0,
  toolCallCount: 0,
  tokenCount: 0,
  lastToolName: "",
  metaLabel: "",
  pulsing: false,
  colSpan: 1,
  rowSpan: 1,
  starterAvatarUrl: "",
  starterName: "",
});

const emit = defineEmits<{
  (e: "open"): void;
}>();

const { t } = useI18n();

const starterDisplayName = computed(() => props.starterName || t("chat.homePanel.delegateLabel"));

const elapsedText = computed(() => props.metaLabel || formatElapsedTimer(props.elapsedMs || 0));

const elapsedTitle = computed(() => {
  if (props.metaLabel && props.elapsedMs) {
    return `${props.metaLabel} · 耗时 ${formatElapsedChinese(props.elapsedMs)}`;
  }
  return props.elapsedMs ? `耗时 ${formatElapsedChinese(props.elapsedMs)}` : "";
});

const statusDotClass = computed(() => {
  if (props.metaLabel === "已失败") return "bg-error/80";
  if (props.metaLabel === "被中断") return "bg-warning/80";
  return "bg-success/80";
});

const tokenText = computed(() => {
  if (!props.tokenCount || props.tokenCount <= 0) return "";
  return `${formatTokenK(props.tokenCount)} tok`;
});

const tokenTitle = computed(() => {
  if (!props.tokenCount || props.tokenCount <= 0) return "";
  return `${props.tokenCount.toLocaleString()} Tokens`;
});

function formatTokenK(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0k";
  const k = value / 1000;
  if (k < 10) return `${k.toFixed(1)}k`;
  return `${Math.round(k)}k`;
}

/** 计时器格式：02:18 / 01:02:18，整洁等宽不占位 */
function formatElapsedTimer(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "00:00";
  const totalSeconds = Math.floor(value / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  const mm = String(minutes).padStart(2, "0");
  const ss = String(seconds).padStart(2, "0");
  if (hours > 0) {
    return `${String(hours).padStart(2, "0")}:${mm}:${ss}`;
  }
  return `${mm}:${ss}`;
}

function formatElapsedChinese(value: number) {
  if (!Number.isFinite(value) || value <= 0) return "0秒";
  const totalSeconds = Math.floor(value / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  if (hours > 0) return `${hours}小时${minutes}分${seconds}秒`;
  if (minutes > 0) return `${minutes}分${seconds}秒`;
  return `${seconds}秒`;
}
</script>

<style scoped>
.ecall-home-num {
  font-variant-numeric: tabular-nums;
}
</style>
