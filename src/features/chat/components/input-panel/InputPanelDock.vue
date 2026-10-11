<template>
  <!-- 输入面板独立底座：直接套双层卡，底卡包面卡永远两层，队列为露头、输入为面卡，手机桌面共用单布局 -->
  <DoubleDeckCard :extra-visible="hasExtra" :is-rounded="!!isRounded" :bg="dockBg">
    <template #extra>
      <InputPanelQueue
        :queue-events="queueEvents"
        :user-persona-name="userPersonaName"
        @recall-to-input="emit('recallToInput', $event)"
        @mark-guided="emit('markGuided', $event)"
      />
      <div v-if="deckExtraVisible" :class="{ 'border-t border-base-300': hasQueue }">
        <slot name="deck-extra" />
      </div>
    </template>
    <template #main>
      <div v-if="$slots.attachments">
        <slot name="attachments" />
      </div>
      <div>
        <slot name="body" />
      </div>
      <div class="mt-2 pt-0">
        <slot name="footer" />
      </div>
    </template>
  </DoubleDeckCard>
</template>

<script setup lang="ts">
import { computed } from "vue";
import DoubleDeckCard from "./DoubleDeckCard.vue";
import InputPanelQueue from "./InputPanelQueue.vue";
import type { ChatQueueEvent } from "../../composables/use-chat-queue";

const props = withDefaults(defineProps<{
  queueEvents?: ChatQueueEvent[];
  userPersonaName?: string;
  queueVisible?: boolean;
  deckExtraVisible?: boolean;
  isRounded?: boolean;
  dockBg?: "base-100" | "base-200" | "base-300";
}>(), {
  queueEvents: () => [],
  userPersonaName: "",
  queueVisible: true,
  deckExtraVisible: false,
  isRounded: false,
  dockBg: "base-200",
});

/** 有队列露头时，建议条与队列之间才需要分割线 */
const hasQueue = computed(
  () => !!props.queueVisible && (props.queueEvents?.length ?? 0) > 0,
);

/** 露头显隐：底座内容为空时只见底卡包面卡，有队列或额外露头内容时露头在面卡上方 */
const hasExtra = computed(() => hasQueue.value || !!props.deckExtraVisible);

const emit = defineEmits<{
  (e: "recallToInput", event: ChatQueueEvent): void;
  (e: "markGuided", eventId: string): void;
}>();
</script>
