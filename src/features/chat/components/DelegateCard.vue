<template>
  <section class="last:mb-0 mx-1">
    <div class="group/card flex w-full items-center gap-2 rounded-lg px-2 py-2 transition-colors hover:bg-base-100/70">
      <button
        type="button"
        class="flex min-w-0 flex-1 items-center gap-2 text-left disabled:cursor-default"
        :disabled="!showResult"
        :title="showResult ? '查看结果' : undefined"
        @click="showResult && emit('openDetail')"
      >
        <img v-if="avatarUrl" :src="avatarUrl" class="h-10 w-10 shrink-0 rounded-full object-cover" />
        <div class="min-w-0 flex-1 overflow-hidden">
          <div class="flex min-w-0 items-start justify-between gap-2">
            <span class="block min-w-0 flex-1 truncate whitespace-nowrap text-xs font-normal text-base-content">{{ title }}</span>
            <div v-if="timeDateLabel" class="shrink-0 text-right text-xs leading-4 text-base-content/55">{{ timeDateLabel }}</div>
          </div>
          <div class="mt-1 flex min-w-0 items-start justify-between gap-2 text-xs text-base-content/65">
            <DelegateProgressLine
              class="min-w-0 flex-1 truncate"
              :running="running"
              :elapsed-ms="elapsedMs"
              :request-count="requestCount"
              :token-count="tokenCount"
              :last-tool-name="lastToolName"
              :text="text"
              :started-label="startedLabel"
            />
            <div v-if="timeMinuteLabel" class="shrink-0 text-right leading-4">
              {{ timeMinuteLabel }}
            </div>
          </div>
        </div>
      </button>
      <div class="flex shrink-0 items-center gap-1">
        <button
          v-if="running"
          type="button"
          class="btn btn-ghost btn-sm h-8 min-h-8 shrink-0 gap-1.5 px-2 font-normal hover:bg-warning hover:text-warning-content"
          title="打断委托"
          @click.stop="emit('abort')"
        >
          <AppIcon name="close" class="size-3.5" aria-hidden="true" />
          <span>打断</span>
        </button>
      </div>
    </div>
  </section>
</template>

<script setup lang="ts">
import AppIcon from "../../shared/components/AppIcon.vue";
import DelegateProgressLine from "./DelegateProgressLine.vue";

defineProps<{
  title: string;
  avatarUrl?: string;
  running?: boolean;
  elapsedMs?: number;
  requestCount?: number;
  tokenCount?: number;
  lastToolName?: string;
  text?: string;
  showResult?: boolean;
  timeDateLabel?: string;
  timeMinuteLabel?: string;
  startedLabel?: string;
}>();

const emit = defineEmits<{
  abort: [];
  openDetail: [];
}>();
</script>
