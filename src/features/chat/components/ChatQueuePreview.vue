<template>
  <div v-if="queueEvents.length > 0" class="border-t border-base-300 bg-base-100/50 p-2">
    <div class="text-[11px] opacity-60 mb-1.5 flex items-center gap-2">
      <span>队列中 ({{ queueEvents.length }})</span>
      <span v-if="sessionState !== 'idle'" class="badge badge-xs badge-warning">{{ sessionStateText }}</span>
    </div>
    <div class="space-y-1">
      <div
        v-for="event in queueEvents"
        :key="event.id"
        class="flex items-center gap-2 rounded bg-base-200/60 px-2 py-1.5 text-xs"
      >
        <span
          class="badge badge-xs"
          :class="{
            'badge-primary': event.source === 'user',
            'badge-info': event.source === 'task',
            'badge-secondary': event.source === 'delegate',
            'badge-neutral': event.source === 'system',
            'badge-accent': event.source === 'remote_im',
          }"
        >
          {{ sourceText(event.source) }}
        </span>
        <span class="flex-1 truncate opacity-80">{{ event.messagePreview }}</span>
        <button
          v-if="event.source === 'user'"
          class="btn btn-ghost btn-xs btn-square"
          :title="t('chat.queue.recallToInput')"
          @click="$emit('recallToInput', event)"
        >
          <Undo2 class="h-3 w-3" />
        </button>
        <button
          class="btn btn-ghost btn-xs btn-square"
          :title="t('chat.queue.remove')"
          @click="$emit('removeFromQueue', event.id)"
        >
          <X class="h-3 w-3" />
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { X, Undo2 } from "lucide-vue-next";
import type { ChatQueueEvent, MainSessionState } from "../composables/use-chat-queue";

const props = defineProps<{
  queueEvents: ChatQueueEvent[];
  sessionState: MainSessionState;
}>();

defineEmits<{
  (e: "recallToInput", event: ChatQueueEvent): void;
  (e: "removeFromQueue", eventId: string): void;
}>();

const { t } = useI18n();

const sessionStateText = computed(() => {
  switch (props.sessionState) {
    case "assistant_streaming":
      return "助理回复中";
    case "organizing_context":
      return "整理上下文";
    default:
      return "空闲";
  }
});

function sourceText(source: string): string {
  switch (source) {
    case "user":
      return "用户";
    case "task":
      return "任务";
    case "delegate":
      return "委托";
    case "system":
      return "系统";
    case "remote_im":
      return "远程";
    default:
      return source;
  }
}
</script>
