<template>
  <ChatConversationFloatingScroll class="h-full">
    <div ref="contentRef" class="flex flex-col gap-4 p-4 sm:flex-row sm:flex-wrap sm:gap-x-2 sm:gap-y-4">
      <div
        class="relative w-full min-w-0 flex flex-col gap-1 rounded-xl border-x border-t border-base-300 bg-base-100 p-2 transition-colors sm:w-auto sm:min-w-[16rem]"
      >
        <div
          v-if="options.every((option) => option.personaMissing)"
          class="w-full px-2 py-4 text-center text-xs text-base-content/60"
        >
          {{ t("chat.noAvailableAgent") }}
        </div>
        <div v-else class="grid grid-cols-3 gap-1 sm:flex sm:flex-wrap sm:justify-center">
          <button
            v-for="option in options"
            :key="option.id"
            type="button"
            class="flex min-w-0 w-full flex-col items-center gap-1 rounded-lg px-1 py-1 transition-colors hover:bg-base-200 sm:w-20"
            :class="selectedId === option.id ? 'bg-primary/10' : ''"
            @click="emit('select', option)"
          >
            <div class="avatar">
              <div
                class="h-10 w-10 rounded-full transition-shadow"
                :class="selectedId === option.id ? 'ring-2 ring-primary ring-offset-1 ring-offset-base-100' : ''"
              >
                <img
                  v-if="resolveAvatarUrl(option.agentId)"
                  :src="resolveAvatarUrl(option.agentId)"
                  :alt="option.agentName"
                  class="h-10 w-10 rounded-full object-cover"
                />
                <div
                  v-else
                  class="flex h-10 w-10 items-center justify-center rounded-full bg-primary text-sm font-semibold text-primary-content"
                >
                  {{ agentInitials(option.agentName) }}
                </div>
              </div>
            </div>
            <span class="max-w-full truncate text-center text-xs leading-tight">
              {{ option.agentName }}
            </span>
            <span v-if="option.personaMissing" class="text-caption leading-none text-warning">
              {{ t("chat.personaRemoved") }}
            </span>
            <span v-else-if="option.modelMissing" class="text-caption leading-none text-warning">
              {{ t("chat.personaModelNotConfigured") }}
            </span>
            <span v-else-if="option.unavailable" class="text-caption leading-none text-warning">
              {{ t("chat.personaRemoved") }}
            </span>
          </button>
        </div>
      </div>
    </div>
  </ChatConversationFloatingScroll>
</template>

<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import ChatConversationFloatingScroll from "../../chat/components/ChatConversationFloatingScroll.vue";
import type { AgentPersonaOption } from "../agent-persona-options";

const props = withDefaults(defineProps<{
  options?: AgentPersonaOption[];
  selectedId?: string;
  avatarUrlMap?: Record<string, string>;
}>(), {
  options: () => [],
  selectedId: "",
  avatarUrlMap: () => ({}),
});

const emit = defineEmits<{
  select: [option: AgentPersonaOption];
}>();

const { t } = useI18n();

const contentRef = ref<HTMLElement | null>(null);
defineExpose({ contentRef });

function resolveAvatarUrl(agentId: string): string {
  return props.avatarUrlMap?.[agentId] || "";
}

function agentInitials(name: string): string {
  const text = String(name || "").trim();
  if (!text) return "?";
  const firstTwo = text.slice(0, 2);
  if (/^[A-Za-z]{2}/.test(firstTwo)) {
    return firstTwo.toUpperCase();
  }
  return text.charAt(0).toUpperCase();
}
</script>
