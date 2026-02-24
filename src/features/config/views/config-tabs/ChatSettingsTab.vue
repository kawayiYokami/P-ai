<template>
  <label class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-xs">{{ t("config.chatSettings.chatLlmProvider") }}</span></div>
    <select v-model="config.chatApiConfigId" class="select select-bordered select-sm">
      <option v-for="a in textCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
    </select>
  </label>
  <label class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-xs">{{ t("config.chatSettings.visionApi") }}</span></div>
    <select :value="config.visionApiConfigId ?? ''" class="select select-bordered select-sm" @change="config.visionApiConfigId = (($event.target as HTMLSelectElement).value || undefined)">
      <option value="">{{ t("config.chatSettings.noVision") }}</option>
      <option v-for="a in imageCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
    </select>
  </label>
  <label class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-xs">语音转写（STT）</span></div>
    <div class="flex items-center gap-2">
      <select :value="config.sttApiConfigId ?? ''" class="select select-bordered select-sm flex-1" @change="onSttSelectChange">
        <option value="">本地（Web Speech）</option>
        <option v-for="a in sttCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
      </select>
      <label class="inline-flex cursor-pointer items-center gap-1 py-0">
        <span class="text-xs">完成后发送</span>
        <input
          :checked="!!config.sttAutoSend"
          type="checkbox"
          class="toggle toggle-sm"
          :disabled="!config.sttApiConfigId"
          @change="onSttAutoSendChange"
        />
      </label>
    </div>
  </label>
  <label class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-xs">{{ t("config.chatSettings.assistantPersona") }}</span></div>
    <select :value="selectedPersonaId" class="select select-bordered select-sm" @change="$emit('update:selectedPersonaId', ($event.target as HTMLSelectElement).value)">
      <option v-for="p in assistantPersonas" :key="p.id" :value="p.id">{{ p.name }}</option>
    </select>
  </label>
  <div class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-xs">{{ t("config.chatSettings.responseStyle") }}</span></div>
    <div class="join w-full">
      <button
        v-for="style in responseStyleOptions"
        :key="style.id"
        class="btn btn-sm join-item flex-1"
        :class="responseStyleId === style.id ? 'btn-primary' : 'bg-base-100'"
        @click="$emit('update:responseStyleId', style.id)"
      >
        {{ t(`responseStyle.${style.id}`) }}
      </button>
    </div>
  </div>
  <div class="grid grid-cols-3 gap-2 mb-3">
    <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 whitespace-nowrap" @click="$emit('openCurrentHistory')">{{ t("config.chatSettings.openCurrentHistory") }}</button>
    <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 whitespace-nowrap" @click="$emit('openPromptPreview')">{{ t("config.chatSettings.previewRequest") }}</button>
    <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 whitespace-nowrap" @click="$emit('openSystemPromptPreview')">{{ t("config.chatSettings.previewSystemPrompt") }}</button>
  </div>
  <div class="card bg-base-100 border border-base-300">
    <div class="card-body p-3 text-xs">
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
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, ImageTextCacheStats, PersonaProfile, ResponseStyleOption } from "../../../../types/app";

const props = defineProps<{
  config: AppConfig;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  sttCapableApiConfigs: ApiConfigItem[];
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

function onSttSelectChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value || undefined;
  props.config.sttApiConfigId = value;
  if (!value) {
    props.config.sttAutoSend = false;
  }
}

function onSttAutoSendChange(event: Event) {
  if (!props.config.sttApiConfigId) {
    props.config.sttAutoSend = false;
    return;
  }
  props.config.sttAutoSend = (event.target as HTMLInputElement).checked;
}
</script>

