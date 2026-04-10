<template>
  <div class="rounded-box border border-base-300 bg-base-100/70 px-2 py-1.5 flex items-center gap-2 text-[11px]">
    <div class="join">
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
    <div class="ml-auto flex items-center gap-1.5 overflow-hidden">
      <button
        v-for="persona in personaPresenceChips"
        :key="persona.id"
        type="button"
        class="btn btn-ghost btn-sm btn-circle p-0 shrink-0 border relative"
        :class="persona.isFrontSpeaking ? 'border-primary/60 bg-primary/10' : 'border-base-300/70 bg-base-100/70'"
        :title="`部门：${persona.departmentName}\n人格：${persona.name}`"
        disabled
        @click.prevent
      >
        <div class="avatar">
          <div
            class="w-7 rounded-full"
            :class="persona.isFrontSpeaking ? 'ring-2 ring-primary ring-offset-2 ring-offset-base-100' : ''"
          >
            <img
              v-if="persona.avatarUrl"
              :src="persona.avatarUrl"
              :alt="persona.name"
              class="w-7 h-7 rounded-full object-cover"
            />
            <div v-else class="bg-neutral text-neutral-content w-7 h-7 rounded-full flex items-center justify-center text-[10px]">
              {{ avatarInitial(persona.name) }}
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
import { Folder, Timer } from "lucide-vue-next";
import type { ChatPersonaPresenceChip } from "../../../types/app";

defineProps<{
  chatting: boolean;
  frozen: boolean;
  workspaceButtonLabel: string;
  workspaceButtonName: string;
  personaPresenceChips: ChatPersonaPresenceChip[];
  supervisionActive: boolean;
  supervisionLabel: string;
  supervisionActiveLabel: string;
  supervisionTitle: string;
}>();

const emit = defineEmits<{
  (e: "lockWorkspace"): void;
  (e: "openSupervisionTask"): void;
}>();

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}
</script>
