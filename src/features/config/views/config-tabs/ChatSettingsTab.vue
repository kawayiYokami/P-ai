<template>
  <div class="grid gap-3">
    <!-- 视觉API -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base mb-3">{{ t("config.chatSettings.visionApi") }}</h3>
        <select :value="config.visionApiConfigId ?? ''" class="select select-bordered select-sm" @change="onVisionSelectChange">
          <option value="">{{ t("config.chatSettings.noVision") }}</option>
          <option v-for="a in imageCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
      </div>
    </div>

    <!-- 语音转写（STT） -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base mb-3">语音转写（STT）</h3>
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
      </div>
    </div>

    <!-- 响应风格 -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base mb-3">{{ t("config.chatSettings.responseStyle") }}</h3>
        <div class="join w-full">
          <button
            v-for="style in responseStyleOptions"
            :key="style.id"
            class="btn btn-sm join-item flex-1"
            :class="responseStyleId === style.id ? 'btn-primary' : 'bg-base-200'"
            @click="$emit('update:responseStyleId', style.id)"
          >
            {{ t(`responseStyle.${style.id}`) }}
          </button>
        </div>
      </div>
    </div>

    <!-- PDF读取模式 -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base mb-3">{{ t("config.chatSettings.pdfReadMode") }}</h3>
        <div class="join w-full">
          <button
            class="btn btn-sm join-item flex-1"
            :class="pdfReadMode === 'text' ? 'btn-primary' : 'bg-base-200'"
            @click="$emit('update:pdfReadMode', 'text')"
          >
            {{ t("config.chatSettings.pdfReadModeText") }}
          </button>
          <button
            class="btn btn-sm join-item flex-1"
            :class="pdfReadMode === 'image' ? 'btn-primary' : 'bg-base-200'"
            @click="$emit('update:pdfReadMode', 'image')"
          >
            {{ t("config.chatSettings.pdfReadModeImage") }}
          </button>
        </div>
        <div class="mt-3 text-xs opacity-70">{{ t("config.chatSettings.pdfReadModeHint") }}</div>
      </div>
    </div>

    <!-- 语音截图关键词 -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base mb-3">{{ t("config.chatSettings.backgroundVoiceScreenshotKeywords") }}</h3>
        <div class="flex items-center gap-2">
          <input
            v-model="backgroundVoiceScreenshotKeywordsDraft"
            type="text"
            class="input input-bordered input-sm flex-1"
            :placeholder="t('config.chatSettings.backgroundVoiceScreenshotKeywordsPlaceholder')"
          />
          <button class="btn btn-sm btn-primary shrink-0" :disabled="!backgroundVoiceScreenshotDirty" @click="saveBackgroundVoiceScreenshotSettings">保存</button>
        </div>
        <div class="mt-3 text-xs opacity-70">{{ t("config.chatSettings.backgroundVoiceScreenshotKeywordsHint") }}</div>
      </div>
    </div>

    <!-- 语音截图模式 -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base mb-3">{{ t("config.chatSettings.backgroundVoiceScreenshotMode") }}</h3>
        <div class="join w-full">
          <button
            class="btn btn-sm join-item flex-1"
            :class="backgroundVoiceScreenshotMode === 'desktop' ? 'btn-primary' : 'bg-base-200'"
            @click="onBackgroundVoiceScreenshotModeChange('desktop')"
          >
            {{ t("config.chatSettings.backgroundVoiceScreenshotModeDesktop") }}
          </button>
          <button
            class="btn btn-sm join-item flex-1"
            :class="backgroundVoiceScreenshotMode === 'focused_window' ? 'btn-primary' : 'bg-base-200'"
            @click="onBackgroundVoiceScreenshotModeChange('focused_window')"
          >
            {{ t("config.chatSettings.backgroundVoiceScreenshotModeFocusedWindow") }}
          </button>
        </div>
        <div class="mt-3 text-xs opacity-70">{{ t("config.chatSettings.backgroundVoiceScreenshotModeHint") }}</div>
      </div>
    </div>

    <!-- 快捷操作 -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base mb-3">快捷操作</h3>
        <div class="grid grid-cols-3 gap-2">
          <button class="btn btn-sm bg-base-200 border-base-300 hover:bg-base-300 whitespace-nowrap" @click="$emit('openCurrentHistory')">{{ t("config.chatSettings.openCurrentHistory") }}</button>
          <button class="btn btn-sm bg-base-200 border-base-300 hover:bg-base-300 whitespace-nowrap" @click="$emit('openPromptPreview')">{{ t("config.chatSettings.previewRequest") }}</button>
          <button class="btn btn-sm bg-base-200 border-base-300 hover:bg-base-300 whitespace-nowrap" @click="$emit('openSystemPromptPreview')">{{ t("config.chatSettings.previewSystemPrompt") }}</button>
        </div>
      </div>
    </div>

    <!-- 图片缓存 -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4 text-sm">
        <div class="flex items-center justify-between">
          <span class="font-medium">{{ t("config.chatSettings.imageCacheTitle") }}</span>
          <div class="flex gap-1">
            <button class="btn btn-sm bg-base-200" :class="{ loading: cacheStatsLoading }" @click="$emit('refreshImageCacheStats')">{{ t("common.refresh") }}</button>
            <button class="btn btn-sm bg-base-200" :disabled="cacheStats.entries === 0" @click="$emit('clearImageCache')">{{ t("config.chatSettings.clearCache") }}</button>
          </div>
        </div>
        <div class="mt-1 opacity-80">{{ t("config.chatSettings.cacheEntries", { entries: cacheStats.entries, chars: cacheStats.totalChars }) }}</div>
        <div class="mt-1 opacity-70">{{ t("config.chatSettings.cacheUpdatedAt", { value: cacheStats.latestUpdatedAt || "-" }) }}</div>
        <div class="mt-1 opacity-60">{{ t("config.chatSettings.cacheHint") }}</div>
      </div>
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
