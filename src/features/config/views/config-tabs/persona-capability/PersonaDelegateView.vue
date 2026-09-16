<template>
  <div class="space-y-3 min-w-0 max-w-full">
    <!-- 候选下属列表 -->
    <ConfigCard
      v-if="candidates.length === 0"
      flush
      class="p-8 text-center text-xs text-base-content/50 min-w-0 max-w-full"
    >
      {{ t("config.persona.delegate.noCandidate") }}
    </ConfigCard>

    <ConfigCard
      v-else
      flush
      class="shadow-xs min-w-0 max-w-full"
    >
      <div class="divide-y divide-base-200/60">
        <button
          v-for="item in candidates"
          :key="item.id"
          type="button"
          class="flex w-full min-w-0 max-w-full items-center gap-3.5 px-4 py-3 text-left transition hover:bg-base-200/40 active:bg-base-200/60 overflow-hidden"
          @click="toggle(item.id)"
        >
          <!-- 头像 -->
          <span
            class="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-full bg-base-200 text-xs font-semibold text-base-content/70 ring-1 ring-base-200"
          >
            <img
              v-if="avatarOf(item.id)"
              :src="avatarOf(item.id)"
              :alt="item.name"
              class="h-full w-full object-cover"
            />
            <span v-else>{{ initialOf(item.name) }}</span>
          </span>

          <!-- 详细信息 -->
          <div class="min-w-0 flex-1 overflow-hidden">
            <div class="flex items-center gap-2 min-w-0">
              <span class="truncate text-sm font-medium text-base-content">{{ item.name }}</span>
              <span v-if="item.isBuiltInSystem" class="badge badge-neutral badge-xs shrink-0">
                {{ t("config.persona.systemTag") }}
              </span>
            </div>
            <div v-if="item.summary" class="mt-0.5 block truncate text-xs text-base-content/50 max-w-full">
              {{ item.summary }}
            </div>
          </div>

          <!-- 勾选状态指示 -->
          <span
            class="flex h-5 w-5 shrink-0 items-center justify-center rounded-selector border transition ml-auto"
            :class="isSelected(item.id)
              ? 'border-primary bg-primary text-primary-content shadow-xs'
              : 'border-base-content/25 bg-transparent text-transparent'"
          >
            <Check class="h-3.5 w-3.5" stroke-width="3" />
          </span>
        </button>
      </div>
    </ConfigCard>

    <!-- 底部微提示 -->
    <p class="px-1 text-xs text-base-content/40 leading-normal">
      {{ t("config.persona.delegate.hint") }}
    </p>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Check } from "@lucide/vue";
import ConfigCard from "../../../components/ConfigCard.vue";
import type { PersonaProfile } from "../../../../../types/app";

const props = withDefaults(defineProps<{
  persona: PersonaProfile;
  personas: PersonaProfile[];
  avatarUrlMap?: Record<string, string>;
  saveRelations?: (updates: { agentId: string; childAgentIds: string[] }[]) => Promise<boolean>;
  setStatusAction?: (message: string) => void;
}>(), {
  avatarUrlMap: () => ({}),
  saveRelations: undefined,
  setStatusAction: undefined,
});

const { t } = useI18n();

const candidates = computed(() =>
  props.personas.filter((item) => String(item.id || "").trim() !== String(props.persona.id || "").trim()),
);

function avatarOf(id: string): string {
  return props.avatarUrlMap[id] || "";
}

function initialOf(name: string): string {
  return String(name || "?").trim().slice(0, 1) || "?";
}

function isSelected(id: string): boolean {
  return (props.persona.childAgentIds || []).includes(id);
}

function toggle(id: string) {
  if (!Array.isArray(props.persona.childAgentIds)) {
    props.persona.childAgentIds = [];
  }
  const current = new Set(
    (props.persona.childAgentIds || []).map((item) => String(item || "").trim()).filter(Boolean),
  );
  if (current.has(id)) {
    current.delete(id);
  } else {
    current.add(id);
  }
  props.persona.childAgentIds = Array.from(current);
}
</script>
