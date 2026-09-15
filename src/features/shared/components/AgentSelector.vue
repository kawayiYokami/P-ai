<template>
  <AgentPersonaSelect
    :agent-id="agentId"
    :api-config-id="apiConfigId"
    :options="options"
    :persona-avatar-url-map="personaAvatarUrlMap"
    :api-configs="apiConfigs"
    :auto-select-first="autoSelectFirst"
    @update:agent-id="emit('update:agentId', $event)"
    @update:api-config-id="emit('update:apiConfigId', $event)"
  />
</template>

<script setup lang="ts">
import type { ApiConfigItem } from "../../../types/app";
import AgentPersonaSelect from "./AgentPersonaSelect.vue";
import type { AgentPersonaOption } from "../agent-persona-options";

defineProps<{
  agentId?: string;
  apiConfigId?: string;
  options?: AgentPersonaOption[];
  personaAvatarUrlMap?: Record<string, string>;
  apiConfigs?: ApiConfigItem[];
  autoSelectFirst?: boolean;
}>();

const emit = defineEmits<{
  "update:agentId": [value: string];
  "update:apiConfigId": [value: string];
}>();
</script>
