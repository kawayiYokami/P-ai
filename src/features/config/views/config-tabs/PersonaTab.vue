<template>
  <label class="form-control">
    <div class="label py-1"><span class="label-text text-xs">{{ t("config.persona.title") }}</span></div>
    <div class="flex gap-1">
      <select :value="personaEditorId" class="select select-bordered select-sm flex-1" @change="$emit('update:personaEditorId', ($event.target as HTMLSelectElement).value)">
        <option v-for="p in personas" :key="p.id" :value="p.id">{{ p.name }}{{ p.isBuiltInUser ? `（${t("config.persona.userTag")}）` : "" }}</option>
      </select>
      <button class="btn btn-sm btn-square text-primary bg-base-100" :title="t('config.persona.add')" @click="$emit('addPersona')">
        <Plus class="h-3.5 w-3.5" />
      </button>
      <button
        class="btn btn-sm btn-square text-error bg-base-100"
        :title="t('config.persona.remove')"
        :disabled="!selectedPersona || selectedPersona.isBuiltInUser || assistantPersonas.length <= 1"
        @click="$emit('removeSelectedPersona')"
      >
        <Trash2 class="h-3.5 w-3.5" />
      </button>
    </div>
  </label>
  <div class="divider my-0"></div>

  <div v-if="selectedPersona" class="grid gap-2">
    <label class="form-control">
      <div class="label py-1"><span class="label-text text-xs">{{ t("config.persona.name") }}</span></div>
      <div class="flex items-center gap-2">
        <input v-model="selectedPersona.name" class="input input-bordered input-sm flex-1" :placeholder="t('config.persona.name')" />
        <button
          class="btn btn-ghost btn-circle p-0 min-h-0 h-auto w-auto"
          :disabled="avatarSaving"
          :title="avatarSaving ? t('config.persona.avatarSaving') : t('config.persona.editAvatar')"
          @click="$emit('openAvatarEditor')"
        >
          <div v-if="selectedPersonaAvatarUrl" class="avatar">
            <div class="w-10 rounded-full">
              <img :src="selectedPersonaAvatarUrl" :alt="selectedPersona.name" :title="selectedPersona.name" />
            </div>
          </div>
          <div v-else class="avatar placeholder">
            <div class="bg-neutral text-neutral-content w-10 rounded-full">
              <span>{{ avatarInitial(selectedPersona.name) }}</span>
            </div>
          </div>
        </button>
      </div>
      <div v-if="avatarError" class="label py-1"><span class="label-text-alt text-error break-all">{{ avatarError }}</span></div>
    </label>
    <label class="form-control">
      <div class="label py-1"><span class="label-text text-xs">{{ t("config.persona.prompt") }}</span></div>
      <textarea
        v-model="selectedPersona.systemPrompt"
        class="textarea textarea-bordered textarea-sm"
        rows="4"
        :placeholder="selectedPersona.isBuiltInUser ? t('config.persona.userPlaceholder') : t('config.persona.assistantPlaceholder')"
      ></textarea>
    </label>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Plus, Trash2 } from "lucide-vue-next";
import type { PersonaProfile } from "../../../../types/app";

defineProps<{
  personas: PersonaProfile[];
  assistantPersonas: PersonaProfile[];
  personaEditorId: string;
  selectedPersona: PersonaProfile | null;
  selectedPersonaAvatarUrl: string;
  avatarSaving: boolean;
  avatarError: string;
}>();

defineEmits<{
  (e: "update:personaEditorId", value: string): void;
  (e: "addPersona"): void;
  (e: "removeSelectedPersona"): void;
  (e: "openAvatarEditor"): void;
}>();

const { t } = useI18n();

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}
</script>


