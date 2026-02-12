<template>
  <label class="form-control">
    <div class="label py-1"><span class="label-text text-xs">{{ t("config.chatSettings.chatApi") }}</span></div>
    <select v-model="config.chatApiConfigId" class="select select-bordered select-sm">
      <option v-for="a in textCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
    </select>
  </label>
  <label class="form-control">
    <div class="label py-1"><span class="label-text text-xs">{{ t("config.chatSettings.visionApi") }}</span></div>
    <select :value="config.visionApiConfigId ?? ''" class="select select-bordered select-sm" @change="config.visionApiConfigId = (($event.target as HTMLSelectElement).value || undefined)">
      <option value="">{{ t("config.chatSettings.noVision") }}</option>
      <option v-for="a in imageCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
    </select>
  </label>
  <label class="form-control">
    <div class="label py-1"><span class="label-text text-xs">{{ t("config.chatSettings.assistantPersona") }}</span></div>
    <select :value="selectedPersonaId" class="select select-bordered select-sm" @change="$emit('update:selectedPersonaId', ($event.target as HTMLSelectElement).value)">
      <option v-for="p in assistantPersonas" :key="p.id" :value="p.id">{{ p.name }}</option>
    </select>
  </label>
  <div class="form-control">
    <div class="label py-1"><span class="label-text text-xs">{{ t("config.chatSettings.responseStyle") }}</span></div>
    <div class="join w-full">
      <button
        v-for="style in responseStyleOptions"
        :key="style.id"
        class="btn btn-sm join-item flex-1"
        :class="responseStyleId === style.id ? 'btn-primary' : 'btn-ghost bg-base-100'"
        @click="$emit('update:responseStyleId', style.id)"
      >
        {{ t(`responseStyle.${style.id}`) }}
      </button>
    </div>
  </div>
  <div class="grid grid-cols-3 gap-1 min-w-0">
    <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 px-2 min-w-0" @click="$emit('openCurrentHistory')">{{ t("config.chatSettings.openCurrentHistory") }}</button>
    <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 px-2 min-w-0" @click="$emit('openPromptPreview')">{{ t("config.chatSettings.previewRequest") }}</button>
    <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 px-2 min-w-0" @click="$emit('openSystemPromptPreview')">{{ t("config.chatSettings.previewSystemPrompt") }}</button>
  </div>
  <div class="rounded border border-base-300 bg-base-100 p-2 text-xs">
    <div class="flex items-center justify-between">
      <span class="font-medium">{{ t("config.chatSettings.imageCacheTitle") }}</span>
      <div class="flex gap-1">
        <button class="btn btn-xs btn-ghost" :class="{ loading: cacheStatsLoading }" @click="$emit('refreshImageCacheStats')">{{ t("common.refresh") }}</button>
        <button class="btn btn-xs btn-ghost" :disabled="cacheStats.entries === 0" @click="$emit('clearImageCache')">{{ t("config.chatSettings.clearCache") }}</button>
      </div>
    </div>
    <div class="mt-1 opacity-80">{{ t("config.chatSettings.cacheEntries", { entries: cacheStats.entries, chars: cacheStats.totalChars }) }}</div>
    <div class="mt-1 opacity-70">{{ t("config.chatSettings.cacheUpdatedAt", { value: cacheStats.latestUpdatedAt || "-" }) }}</div>
    <div class="mt-1 opacity-60">{{ t("config.chatSettings.cacheHint") }}</div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, ImageTextCacheStats, PersonaProfile, ResponseStyleOption } from "../../../../types/app";

defineProps<{
  config: AppConfig;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  assistantPersonas: PersonaProfile[];
  selectedPersonaId: string;
  responseStyleOptions: ResponseStyleOption[];
  responseStyleId: string;
  cacheStats: ImageTextCacheStats;
  cacheStatsLoading: boolean;
}>();

defineEmits<{
  (e: "update:selectedPersonaId", value: string): void;
  (e: "update:responseStyleId", value: string): void;
  (e: "openCurrentHistory"): void;
  (e: "openPromptPreview"): void;
  (e: "openSystemPromptPreview"): void;
  (e: "refreshImageCacheStats"): void;
  (e: "clearImageCache"): void;
}>();

const { t } = useI18n();
</script>


