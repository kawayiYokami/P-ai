<template>
  <div class="grid gap-3">
    <div class="text-xs leading-snug text-base-content/60">{{ t("config.persona.capability.skillHint") }}</div>

    <div v-if="loading" class="flex items-center gap-2 text-sm opacity-60">
      <span class="loading loading-spinner loading-xs"></span>
      <span>{{ t("config.persona.capability.loading") }}</span>
    </div>

    <div v-else-if="skills.length === 0" class="rounded-xl border border-dashed border-base-300 bg-base-100 py-10 text-center text-sm opacity-60">
      {{ t("config.persona.capability.skillEmpty") }}
    </div>

    <div v-else class="overflow-hidden rounded-xl border border-base-200/80 bg-base-100">
      <div
        v-for="skill in skills"
        :key="skill.path || skill.name"
        class="flex flex-wrap items-center justify-between gap-3 border-b border-base-200/60 px-4 py-3 last:border-b-0"
      >
        <div class="min-w-0 flex-1">
          <div class="flex items-center gap-2">
            <span class="truncate text-sm font-medium">{{ skill.name }}</span>
            <span v-if="skill.isBuiltin" class="badge badge-ghost badge-xs shrink-0">
              {{ t("config.persona.capability.builtinTag") }}
            </span>
          </div>
          <div class="mt-0.5 truncate text-xs opacity-60">{{ skill.description }}</div>
        </div>
        <SegmentedControl
          :model-value="stateOf(skill.name)"
          :options="stateOptions"
          size="sm"
          class="shrink-0"
          surface-class="bg-base-200"
          @change="(value) => changeState(skill.name, value)"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { PersonaProfile, SkillSummaryItem } from "../../../../../types/app";
import SegmentedControl, { type SegmentedControlOption } from "../../../components/SegmentedControl.vue";
import { personaSkillState, setPersonaSkillState, type PersonaSkillState } from "../../../utils/persona-capability";

const props = defineProps<{
  persona: PersonaProfile;
  skills: SkillSummaryItem[];
  loading?: boolean;
}>();

const { t } = useI18n();

const stateOptions = computed<SegmentedControlOption<PersonaSkillState>[]>(() => [
  { value: "off", label: t("config.persona.capability.stateOff") },
  { value: "optional", label: t("config.persona.capability.stateOptional") },
  { value: "resident", label: t("config.persona.capability.stateResident") },
]);

function stateOf(name: string): PersonaSkillState {
  return personaSkillState(props.persona, name);
}

function changeState(name: string, value: PersonaSkillState) {
  setPersonaSkillState(props.persona, name, value);
}
</script>
