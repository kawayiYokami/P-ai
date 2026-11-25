<template>
  <div class="group relative mx-1" :class="{ 'opacity-60': !timer.enabled }">
    <button
      type="button"
      class="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left transition-colors hover:bg-base-100/70 disabled:cursor-default disabled:hover:bg-transparent"
      :disabled="disabled"
      :title="itemTitle"
      @click="$emit('click')"
    >
      <!-- 状态只用一个图标：还会再跑 / 已经结束。
           依赖状态的文字（停用、单次、cron 原文）都不铺在行里，避免双行。 -->
      <span
        class="shrink-0"
        :title="finished ? t('config.timer.finished') : t('config.timer.running')"
      >
        <AppIcon
          :name="finished ? 'check-circle' : 'clock'"
          class="h-4 w-4"
          :class="finished ? 'text-base-content/35' : 'text-success'"
        />
      </span>
      <span class="min-w-0 flex-1 truncate text-sm font-medium">{{ message }}</span>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import {
  formatTimerTime,
  timerIsFinished,
  timerScheduleDescription,
  type TimerEntry,
} from "../utils/timer-entry";

const props = defineProps<{
  timer: TimerEntry;
  disabled?: boolean;
}>();

defineEmits<{
  click: [];
}>();

const { t } = useI18n();

/** 定时器只有一句话，行首行尾都用它；这里只是收敛空白，不做二次加工 */
const message = computed(() => String(props.timer.message || "").trim());

const finished = computed(() => timerIsFinished(props.timer));

/**
 * 行里只留一句话，其余信息退到 title：悬停能看到频率人话描述与下次触发，
 * 但 cron 原文不进任何一处可见文本。
 */
const itemTitle = computed(() => [
  message.value,
  timerScheduleDescription(props.timer) || t("config.timer.singleRun"),
  formatTimerTime(props.timer.nextRunAt)
    ? `${t("config.timer.nextRun")}: ${formatTimerTime(props.timer.nextRunAt)}`
    : "",
  finished.value ? t("config.timer.finished") : t("config.timer.running"),
].filter(Boolean).join("\n"));
</script>
