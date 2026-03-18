<template>
  <label class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-sm">{{ t("config.chatSettings.visionApi") }}</span></div>
    <select :value="config.visionApiConfigId ?? ''" class="select select-bordered select-sm" @change="onVisionSelectChange">
      <option value="">{{ t("config.chatSettings.noVision") }}</option>
      <option v-for="a in imageCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
    </select>
  </label>
  <label class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-sm">语音转写（STT）</span></div>
    <div class="flex items-center gap-2">
      <select :value="config.sttApiConfigId ?? ''" class="select select-bordered select-sm flex-1" @change="onSttSelectChange">
        <option value="">本地（Web Speech）</option>
        <option v-for="a in sttCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
      </select>
      <label class="inline-flex cursor-pointer items-center gap-1 py-0">
        <span class="text-sm">完成后发送</span>
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
  <div class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-sm">{{ t("config.chatSettings.responseStyle") }}</span></div>
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
  <div class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-sm">{{ t("config.chatSettings.pdfReadMode") }}</span></div>
    <div class="join w-full">
      <button
        class="btn btn-sm join-item flex-1"
        :class="pdfReadMode === 'text' ? 'btn-primary' : 'bg-base-100'"
        @click="$emit('update:pdfReadMode', 'text')"
      >
        {{ t("config.chatSettings.pdfReadModeText") }}
      </button>
      <button
        class="btn btn-sm join-item flex-1"
        :class="pdfReadMode === 'image' ? 'btn-primary' : 'bg-base-100'"
        @click="$emit('update:pdfReadMode', 'image')"
      >
        {{ t("config.chatSettings.pdfReadModeImage") }}
      </button>
    </div>
    <div class="mt-1 text-xs opacity-70">{{ t("config.chatSettings.pdfReadModeHint") }}</div>
  </div>
  <div class="my-3 border-t border-base-300"></div>
  <div class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-sm">{{ t("config.chatSettings.backgroundVoiceScreenshotKeywords") }}</span></div>
    <div class="flex items-center gap-2">
      <input
        v-model="backgroundVoiceScreenshotKeywordsDraft"
        type="text"
        class="input input-bordered input-sm flex-1"
        :placeholder="t('config.chatSettings.backgroundVoiceScreenshotKeywordsPlaceholder')"
      />
      <button class="btn btn-sm btn-primary shrink-0" :disabled="!backgroundVoiceScreenshotDirty" @click="saveBackgroundVoiceScreenshotSettings">保存</button>
    </div>
    <div class="mt-1 text-xs opacity-70">{{ t("config.chatSettings.backgroundVoiceScreenshotKeywordsHint") }}</div>
  </div>
  <div class="mb-3 flex w-full flex-col gap-1">
    <div class="flex items-center justify-between py-1"><span class="text-sm">{{ t("config.chatSettings.backgroundVoiceScreenshotMode") }}</span></div>
    <div class="join w-full">
      <button
        class="btn btn-sm join-item flex-1"
        :class="backgroundVoiceScreenshotMode === 'desktop' ? 'btn-primary' : 'bg-base-100'"
        @click="onBackgroundVoiceScreenshotModeChange('desktop')"
      >
        {{ t("config.chatSettings.backgroundVoiceScreenshotModeDesktop") }}
      </button>
      <button
        class="btn btn-sm join-item flex-1"
        :class="backgroundVoiceScreenshotMode === 'focused_window' ? 'btn-primary' : 'bg-base-100'"
        @click="onBackgroundVoiceScreenshotModeChange('focused_window')"
      >
        {{ t("config.chatSettings.backgroundVoiceScreenshotModeFocusedWindow") }}
      </button>
    </div>
    <div class="mt-1 text-xs opacity-70">{{ t("config.chatSettings.backgroundVoiceScreenshotModeHint") }}</div>
  </div>
  <div class="grid grid-cols-3 gap-2 mb-3">
    <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 whitespace-nowrap" @click="$emit('openCurrentHistory')">{{ t("config.chatSettings.openCurrentHistory") }}</button>
    <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 whitespace-nowrap" @click="$emit('openPromptPreview')">{{ t("config.chatSettings.previewRequest") }}</button>
    <button class="btn btn-sm bg-base-100 border-base-300 hover:bg-base-200 whitespace-nowrap" @click="$emit('openSystemPromptPreview')">{{ t("config.chatSettings.previewSystemPrompt") }}</button>
  </div>
  <div class="card bg-base-100 border border-base-300">
    <div class="card-body p-3 text-sm">
      <div class="flex items-center justify-between">
        <span class="font-medium">{{ t("config.chatSettings.imageCacheTitle") }}</span>
        <div class="flex gap-1">
          <button class="btn btn-sm btn-ghost" :class="{ loading: cacheStatsLoading }" @click="$emit('refreshImageCacheStats')">{{ t("common.refresh") }}</button>
          <button class="btn btn-sm btn-ghost" :disabled="cacheStats.entries === 0" @click="$emit('clearImageCache')">{{ t("config.chatSettings.clearCache") }}</button>
        </div>
      </div>
      <div class="mt-1 opacity-80">{{ t("config.chatSettings.cacheEntries", { entries: cacheStats.entries, chars: cacheStats.totalChars }) }}</div>
      <div class="mt-1 opacity-70">{{ t("config.chatSettings.cacheUpdatedAt", { value: cacheStats.latestUpdatedAt || "-" }) }}</div>
      <div class="mt-1 opacity-60">{{ t("config.chatSettings.cacheHint") }}</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { AppConfig, ApiConfigItem, ImageTextCacheStats, ResponseStyleOption } from "../../../../types/app";

const props = defineProps<{
  config: AppConfig;
  imageCapableApiConfigs: ApiConfigItem[];
  sttCapableApiConfigs: ApiConfigItem[];
  responseStyleOptions: ResponseStyleOption[];
  responseStyleId: string;
  pdfReadMode: "text" | "image";
  backgroundVoiceScreenshotKeywords: string;
  backgroundVoiceScreenshotMode: "desktop" | "focused_window";
  cacheStats: ImageTextCacheStats;
  cacheStatsLoading: boolean;
}>();

const { t } = useI18n();
const emit = defineEmits<{
  (e: "update:responseStyleId", value: string): void;
  (e: "update:pdfReadMode", value: "text" | "image"): void;
  (e: "update:backgroundVoiceScreenshotKeywords", value: string): void;
  (e: "update:backgroundVoiceScreenshotMode", value: "desktop" | "focused_window"): void;
  (e: "saveChatSettings"): void;
  (e: "openCurrentHistory"): void;
  (e: "openPromptPreview"): void;
  (e: "openSystemPromptPreview"): void;
  (e: "refreshImageCacheStats"): void;
  (e: "clearImageCache"): void;
}>();

function onVisionSelectChange(event: Event) {
  props.config.visionApiConfigId = ((event.target as HTMLSelectElement).value || undefined);
  emit("saveChatSettings");
}

function onSttSelectChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value || undefined;
  props.config.sttApiConfigId = value;
  if (!value) {
    props.config.sttAutoSend = false;
  }
  emit("saveChatSettings");
}

function onSttAutoSendChange(event: Event) {
  if (!props.config.sttApiConfigId) {
    props.config.sttAutoSend = false;
    emit("saveChatSettings");
    return;
  }
  props.config.sttAutoSend = (event.target as HTMLInputElement).checked;
  emit("saveChatSettings");
}

const backgroundVoiceScreenshotKeywordsDraft = ref(String(props.backgroundVoiceScreenshotKeywords || ""));

watch(
  () => props.backgroundVoiceScreenshotKeywords,
  (value) => {
    backgroundVoiceScreenshotKeywordsDraft.value = String(value || "");
  },
);

const backgroundVoiceScreenshotDirty = computed(
  () => backgroundVoiceScreenshotKeywordsDraft.value !== String(props.backgroundVoiceScreenshotKeywords || ""),
);

function saveBackgroundVoiceScreenshotSettings() {
  emit("update:backgroundVoiceScreenshotKeywords", backgroundVoiceScreenshotKeywordsDraft.value);
  emit("saveChatSettings");
}

function onBackgroundVoiceScreenshotModeChange(value: "desktop" | "focused_window") {
  emit("update:backgroundVoiceScreenshotMode", value);
  emit("saveChatSettings");
}
</script>
