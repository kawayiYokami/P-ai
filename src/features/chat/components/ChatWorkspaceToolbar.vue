<template>
  <div class="rounded-box border border-base-300 bg-base-100/70 px-2 py-1.5 flex items-center justify-between gap-2 text-[11px]">
    <div class="join min-w-0">
      <button
        class="btn btn-sm btn-ghost join-item gap-1.5"
        :disabled="chatting || frozen"
        @click="emit('lockWorkspace')"
      >
        <Folder class="h-3.5 w-3.5" />
        {{ workspaceButtonName || workspaceButtonLabel }}
      </button>
      <button
        type="button"
        class="btn btn-sm join-item gap-1.5"
        :class="supervisionActive ? 'btn-primary' : 'btn-ghost'"
        :disabled="frozen"
        :title="supervisionTitle"
        @click="emit('openSupervisionTask')"
      >
        <Timer class="h-3.5 w-3.5" />
        {{ supervisionActive ? supervisionActiveLabel : supervisionLabel }}
      </button>
    </div>
    <div class="flex min-w-0 items-center justify-end gap-1.5">
      <button
        type="button"
        class="btn btn-sm gap-1.5 shrink-0"
        :class="reviewPanelOpen ? 'btn-primary' : 'btn-ghost'"
        :disabled="!reviewButtonEnabled"
        @click="emit('toggleToolReview')"
      >
        <Shield class="h-3.5 w-3.5" />
        {{ reviewButtonLabel }}
      </button>
      <button
        v-for="persona in personaPresenceChips"
        :key="persona.id"
        type="button"
        class="btn btn-ghost btn-sm btn-circle overflow-visible p-0 shrink-0 border relative"
        :class="personaChipClass(persona)"
        :title="`部门：${persona.departmentName}\n人格：${persona.name}`"
        :disabled="chatting || frozen || !mentionableAgentIds.includes(persona.id)"
        @click="emit('mentionPersona', persona.id)"
      >
        <div class="indicator">
          <span
            v-if="selectedMentionAgentIds.includes(persona.id)"
            class="indicator-item indicator-top indicator-end inline-flex h-4 w-4 translate-x-1/4 -translate-y-1/4 items-center justify-center rounded-full bg-primary text-[9px] font-bold text-primary-content"
          >
            @
          </span>
          <span
            v-if="props.selectedMentionAgentIds.length > 0 && persona.isFrontSpeaking"
            class="indicator-item indicator-top indicator-start inline-flex h-4 w-4 -translate-x-1/4 -translate-y-1/4 items-center justify-center rounded-full bg-base-300 text-[9px] font-bold text-base-content"
          >
            禁
          </span>
          <div class="avatar">
            <div class="w-7 rounded-full">
              <img
                v-if="persona.avatarUrl"
                :src="persona.avatarUrl"
                :alt="persona.name"
                class="w-7 h-7 rounded-full object-cover"
                :class="frontSpeakingMuted(persona) ? 'grayscale opacity-75' : ''"
              />
              <div
                v-else
                class="w-7 h-7 rounded-full flex items-center justify-center text-[10px]"
                :class="frontSpeakingMuted(persona)
                  ? 'bg-base-300 text-base-content/70'
                  : 'bg-neutral text-neutral-content'"
              >
                {{ avatarInitial(persona.name) }}
              </div>
            </div>
          </div>
        </div>
        <span
          v-if="persona.hasBackgroundTask"
          class="absolute right-0.5 top-0.5 inline-block h-2.5 w-2.5 rounded-full bg-error ring-2 ring-base-100"
        ></span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { Folder, Shield, Timer } from "lucide-vue-next";
import type { ChatPersonaPresenceChip } from "../../../types/app";

const props = defineProps<{
  chatting: boolean;
  frozen: boolean;
  workspaceButtonLabel: string;
  workspaceButtonName: string;
  personaPresenceChips: ChatPersonaPresenceChip[];
  mentionableAgentIds: string[];
  selectedMentionAgentIds: string[];
  supervisionActive: boolean;
  supervisionLabel: string;
  supervisionActiveLabel: string;
  supervisionTitle: string;
  reviewButtonLabel: string;
  reviewPanelOpen: boolean;
  reviewButtonEnabled: boolean;
}>();

const emit = defineEmits<{
  (e: "lockWorkspace"): void;
  (e: "openSupervisionTask"): void;
  (e: "toggleToolReview"): void;
  (e: "mentionPersona", agentId: string): void;
}>();

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function personaChipClass(persona: ChatPersonaPresenceChip): string {
  const selected = props.selectedMentionAgentIds.includes(persona.id);
  const muted = frontSpeakingMuted(persona);
  if (selected) {
    return "border-primary/60 bg-primary/10 hover:border-primary hover:bg-primary/15";
  }
  if (muted) {
    return "border-base-300/70 bg-base-200/70 hover:border-base-300 hover:bg-base-200";
  }
  return "border-base-300/70 bg-base-100/70 hover:border-base-300 hover:bg-base-200";
}

function frontSpeakingMuted(persona: ChatPersonaPresenceChip): boolean {
  return props.selectedMentionAgentIds.length > 0 && persona.isFrontSpeaking;
}
</script>
