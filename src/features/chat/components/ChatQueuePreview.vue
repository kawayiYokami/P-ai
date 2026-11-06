<template>
  <div v-if="queueEvents.length > 0" class="mb-2 w-full overflow-hidden rounded-lg bg-base-200/45">
    <div class="divide-y divide-base-content/10">
      <div
        v-for="event in queueEvents"
        :key="event.id"
        class="flex items-center gap-2 px-2 py-1 text-xs"
      >
        <span
          class="badge badge-xs shrink-0"
          :class="event.queueMode === 'guided' ? 'badge-primary' : 'badge-ghost'"
        >
          {{ event.queueMode === "guided" ? t("chat.queue.guiding") : t("chat.queue.queued") }}
        </span>
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
          v-if="event.source === 'user' && event.queueMode !== 'guided'"
          class="btn btn-ghost btn-xs btn-square"
          :title="t('chat.queue.recallToInput')"
          @click="$emit('recallToInput', event)"
        >
          <AppIcon name="undo" class="h-3 w-3" />
        </button>
        <button
          v-if="event.source === 'user'"
          class="btn btn-ghost btn-xs"
          :class="event.queueMode === 'guided' ? 'pointer-events-none text-primary' : ''"
          :title="event.queueMode === 'guided' ? t('chat.queue.guiding') : t('chat.queue.guide')"
          :disabled="event.queueMode === 'guided'"
          @click="event.queueMode === 'guided' ? undefined : $emit('markGuided', event.id)"
        >
          {{ event.queueMode === "guided" ? t("chat.queue.guiding") : t("chat.queue.guide") }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import type { ChatQueueEvent, MainSessionState } from "../composables/use-chat-queue";

const props = defineProps<{
  queueEvents: ChatQueueEvent[];
  sessionState: MainSessionState;
  userPersonaName?: string;
}>();

defineEmits<{
  (e: "recallToInput", event: ChatQueueEvent): void;
  (e: "markGuided", eventId: string): void;
}>();

const { t } = useI18n();

function sourceText(source: string): string {
  switch (source) {
    case "user":
      return String(props.userPersonaName || "").trim() || t("archives.roleUser");
    case "task":
      return t("chat.queue.sourceTask");
    case "delegate":
      return t("chat.queue.sourceDelegate");
    case "system":
      return t("chat.queue.sourceSystem");
    case "remote_im":
      return t("chat.queue.sourceRemoteIm");
    default:
      return source;
  }
}
</script>
