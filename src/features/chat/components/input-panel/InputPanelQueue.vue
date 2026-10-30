<template>
  <!-- 队列列表：纯露头内容，无展开动画，显隐由双层卡 extraVisible 统一控制，人格名在前、引导项置顶 -->
  <TransitionGroup
    v-if="orderedQueueEvents.length > 0"
    name="input-panel-queue-move"
    tag="div"
    class="flex flex-col"
  >
    <div
      v-for="event in orderedQueueEvents"
      :key="event.id"
      class="flex items-center gap-2 px-2 py-1 text-xs"
    >
      <span
        class="badge badge-xs shrink-0"
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
        class="btn btn-ghost btn-xs btn-square shrink-0"
        :title="t('chat.queue.recallToInput')"
        @pointerdown.prevent
        @mousedown.prevent
        @click="$emit('recallToInput', event)"
      >
        <Undo2 class="h-3 w-3" />
      </button>
      <button
        v-if="event.source === 'user'"
        class="btn btn-ghost btn-xs shrink-0"
        :class="event.queueMode === 'guided' ? 'pointer-events-none text-primary' : ''"
        :title="event.queueMode === 'guided' ? t('chat.queue.guiding') : t('chat.queue.guide')"
        :disabled="event.queueMode === 'guided'"
        @pointerdown.prevent
        @mousedown.prevent
        @click="event.queueMode === 'guided' ? undefined : $emit('markGuided', event.id)"
      >
        {{ event.queueMode === "guided" ? t("chat.queue.guiding") : t("chat.queue.guide") }}
      </button>
    </div>
  </TransitionGroup>
</template>

<style scoped>
.input-panel-queue-move-move {
  transition: transform 0.3s ease-in-out;
}
</style>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Undo2 } from "@lucide/vue";
import type { ChatQueueEvent } from "../../composables/use-chat-queue";

const props = withDefaults(
  defineProps<{
    queueEvents?: ChatQueueEvent[];
    userPersonaName?: string;
  }>(),
  {
    queueEvents: () => [],
    userPersonaName: "",
  },
);

defineEmits<{
  (e: "recallToInput", event: ChatQueueEvent): void;
  (e: "markGuided", eventId: string): void;
}>();

const { t } = useI18n();

/** 队列发送顺序：被引导的置顶，其余按创建时间正序。 */
const orderedQueueEvents = computed(() => {
  const events = Array.isArray(props.queueEvents) ? [...props.queueEvents] : [];
  const timeOf = (event: ChatQueueEvent): number => {
    const parsed = Date.parse(String(event.createdAt || ""));
    return Number.isFinite(parsed) ? parsed : 0;
  };
  return events.sort((a, b) => {
    const guidedA = a.queueMode === "guided" ? 0 : 1;
    const guidedB = b.queueMode === "guided" ? 0 : 1;
    if (guidedA !== guidedB) return guidedA - guidedB;
    return timeOf(a) - timeOf(b);
  });
});

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
