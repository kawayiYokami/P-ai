<template>
  <div class="grid gap-2.5 rounded-xl border border-base-200/80 bg-base-100 p-4">
    <div class="flex items-center justify-between gap-2">
      <div class="text-sm font-medium">{{ t("config.persona.capability.title") }}</div>
      <span v-if="loading" class="text-xs opacity-50">{{ t("config.persona.capability.loading") }}</span>
    </div>
    <div class="grid gap-2 md:grid-cols-3">
      <button
        type="button"
        class="flex min-w-0 flex-col items-start gap-1 rounded-lg border border-base-200/70 bg-base-200/40 p-3 text-left transition-colors hover:border-primary/40 hover:bg-base-200/70"
        @click="emit('open', 'tools')"
      >
        <span class="text-xs opacity-60">{{ t("config.persona.capability.mcp") }}</span>
        <span class="w-full truncate text-sm" :class="mcpSummary === emptyText ? 'opacity-50' : ''">
          {{ mcpSummary }}
        </span>
      </button>
      <button
        type="button"
        class="flex min-w-0 flex-col items-start gap-1 rounded-lg border border-base-200/70 bg-base-200/40 p-3 text-left transition-colors hover:border-primary/40 hover:bg-base-200/70"
        @click="emit('open', 'skills')"
      >
        <span class="text-xs opacity-60">{{ t("config.persona.capability.resident") }}</span>
        <span class="w-full truncate text-sm" :class="residentSummary === emptyText ? 'opacity-50' : ''">
          {{ residentSummary }}
        </span>
      </button>
      <button
        type="button"
        class="flex min-w-0 flex-col items-start gap-1 rounded-lg border border-base-200/70 bg-base-200/40 p-3 text-left transition-colors hover:border-primary/40 hover:bg-base-200/70"
        @click="emit('open', 'skills')"
      >
        <span class="text-xs opacity-60">{{ t("config.persona.capability.optional") }}</span>
        <span class="w-full truncate text-sm" :class="optionalCount === 0 ? 'opacity-50' : ''">
          {{ optionalSummary }}
        </span>
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { PersonaProfile } from "../../../../../types/app";
import { mcpServerIdOf } from "../../../utils/persona-capability";

const props = defineProps<{
  persona: PersonaProfile;
  mcpServerNameById: Record<string, string>;
  loading?: boolean;
}>();

const emit = defineEmits<{
  (e: "open", view: "skills" | "tools"): void;
}>();

const { t } = useI18n();

const emptyText = computed(() => t("config.persona.capability.notEnabled"));
const MCP_SUMMARY_LIMIT = 3;

// MCP 概览按「组」呈现：把 mcpToolNames 里的 serverId 去重后换成组名。
const mcpGroupNames = computed(() => {
  const control = props.persona?.permissionControl;
  const ids = (control?.mcpToolNames || [])
    .map((item) => mcpServerIdOf(item))
    .filter((id) => !!id);
  const seen = new Set<string>();
  const names: string[] = [];
  for (const id of ids) {
    if (seen.has(id)) continue;
    seen.add(id);
    names.push(props.mcpServerNameById[id] || id);
  }
  return names;
});

const mcpSummary = computed(() => {
  const names = mcpGroupNames.value;
  if (names.length === 0) return emptyText.value;
  if (names.length <= MCP_SUMMARY_LIMIT) return names.join("、");
  return `${names.slice(0, MCP_SUMMARY_LIMIT).join("、")} +${names.length - MCP_SUMMARY_LIMIT}`;
});

const residentSummary = computed(() => {
  const names = (props.persona?.residentSkillNames || [])
    .map((item) => String(item || "").trim())
    .filter(Boolean);
  return names.length > 0 ? names.join("、") : emptyText.value;
});

const optionalCount = computed(() =>
  (props.persona?.optionalSkillNames || [])
    .map((item) => String(item || "").trim())
    .filter(Boolean).length,
);

const optionalSummary = computed(() => `${optionalCount.value}`);
</script>
