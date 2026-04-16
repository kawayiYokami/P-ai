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

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base mb-3">{{ t("config.chatSettings.toolReviewApi") }}</h3>
        <select :value="config.toolReviewApiConfigId ?? ''" class="select select-bordered select-sm" @change="onToolReviewSelectChange">
          <option value="">{{ t("config.chatSettings.noToolReview") }}</option>
          <option v-for="a in textCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
        <div class="mt-3 text-xs opacity-70 whitespace-pre-line">{{ t("config.chatSettings.toolReviewApiHint") }}</div>
      </div>
    </div>

    <!-- 语音转写（STT） -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base mb-3">{{ t("config.chatSettings.sttTitle") }}</h3>
        <div class="flex items-center gap-2">
          <select :value="config.sttApiConfigId ?? ''" class="select select-bordered select-sm flex-1" @change="onSttSelectChange">
            <option value="">{{ t("config.chatSettings.sttLocalWebSpeech") }}</option>
            <option v-for="a in sttCapableApiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
          </select>
          <label class="inline-flex cursor-pointer items-center gap-1 py-0">
            <span class="text-sm">{{ t("config.chatSettings.sttAutoSend") }}</span>
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
            @click="onResponseStyleChange(style.id)"
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
            @click="onPdfReadModeChange('text')"
          >
            {{ t("config.chatSettings.pdfReadModeText") }}
          </button>
          <button
            class="btn btn-sm join-item flex-1"
            :class="pdfReadMode === 'image' ? 'btn-primary' : 'bg-base-200'"
            @click="onPdfReadModeChange('image')"
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

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4 gap-3">
        <div class="flex items-center justify-between gap-2">
          <div>
            <h3 class="card-title text-base">{{ t("config.chatSettings.instructionPresetsTitle") }}</h3>
            <div class="text-xs opacity-70 mt-1">{{ t("config.chatSettings.instructionPresetsHint") }}</div>
          </div>
          <button class="btn btn-sm btn-ghost shrink-0" @click="addInstructionPreset">
            <Plus class="h-4 w-4" />
            <span>{{ t("config.chatSettings.addInstructionPreset") }}</span>
          </button>
        </div>
        <div v-if="instructionPresetsDraft.length === 0" class="text-sm opacity-60">{{ t("config.chatSettings.noInstructionPresets") }}</div>
        <div v-else class="overflow-hidden rounded-box border border-base-300 bg-base-200/20">
          <div v-for="item in instructionPresetsDraft" :key="item.id" class="flex items-center gap-2 border-b border-base-300 px-3 py-2 last:border-b-0">
            <input
              v-model="item.prompt"
              type="text"
              class="input input-ghost input-sm flex-1"
              :placeholder="t('config.chatSettings.instructionPresetPlaceholder')"
            />
            <button class="btn btn-sm btn-ghost btn-square shrink-0" @click="removeInstructionPreset(item.id)">
              <Trash2 class="h-4 w-4" />
            </button>
          </div>
        </div>
        <div class="flex justify-end">
          <button class="btn btn-sm btn-primary" :disabled="!instructionPresetsDirty" @click="saveInstructionPresets">{{ t("config.chatSettings.saveInstructionPresets") }}</button>
        </div>
      </div>
    </div>

    <!-- 快捷操作 -->
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base mb-3">{{ t("config.chatSettings.quickActionsTitle") }}</h3>
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
import { Plus, Trash2 } from "lucide-vue-next";
import type { AppConfig, ApiConfigItem, ChatSettingsPatch, ConversationApiSettingsPatch, ImageTextCacheStats, PromptCommandPreset, ResponseStyleOption } from "../../../../types/app";

const props = defineProps<{
  config: AppConfig;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  sttCapableApiConfigs: ApiConfigItem[];
  responseStyleOptions: ResponseStyleOption[];
  responseStyleId: string;
  pdfReadMode: "text" | "image";
  backgroundVoiceScreenshotKeywords: string;
  backgroundVoiceScreenshotMode: "desktop" | "focused_window";
  instructionPresets: PromptCommandPreset[];
  cacheStats: ImageTextCacheStats;
  cacheStatsLoading: boolean;
}>();

const { t } = useI18n();
const emit = defineEmits<{
  (e: "update:responseStyleId", value: string): void;
  (e: "update:pdfReadMode", value: "text" | "image"): void;
  (e: "update:backgroundVoiceScreenshotKeywords", value: string): void;
  (e: "update:backgroundVoiceScreenshotMode", value: "desktop" | "focused_window"): void;
  (e: "update:instructionPresets", value: PromptCommandPreset[]): void;
  (e: "patchConversationApiSettings", value: ConversationApiSettingsPatch): void;
  (e: "patchChatSettings", value: ChatSettingsPatch): void;
  (e: "openCurrentHistory"): void;
  (e: "openPromptPreview"): void;
  (e: "openSystemPromptPreview"): void;
  (e: "refreshImageCacheStats"): void;
  (e: "clearImageCache"): void;
}>();

function onVisionSelectChange(event: Event) {
  props.config.visionApiConfigId = ((event.target as HTMLSelectElement).value || undefined);
  emit("patchConversationApiSettings", {
    visionApiConfigId: props.config.visionApiConfigId ?? null,
  });
}

function onToolReviewSelectChange(event: Event) {
  props.config.toolReviewApiConfigId = ((event.target as HTMLSelectElement).value || undefined);
  emit("patchConversationApiSettings", {
    toolReviewApiConfigId: props.config.toolReviewApiConfigId ?? null,
  });
}

function onResponseStyleChange(value: string) {
  emit("update:responseStyleId", value);
  emit("patchChatSettings", {
    responseStyleId: value,
  });
}

function onPdfReadModeChange(value: "text" | "image") {
  emit("update:pdfReadMode", value);
  emit("patchChatSettings", {
    pdfReadMode: value,
  });
}

function onSttSelectChange(event: Event) {
  const value = (event.target as HTMLSelectElement).value || undefined;
  props.config.sttApiConfigId = value;
  if (!value) {
    props.config.sttAutoSend = false;
  }
  emit("patchConversationApiSettings", {
    sttApiConfigId: props.config.sttApiConfigId ?? null,
    sttAutoSend: !!props.config.sttAutoSend,
  });
}

function onSttAutoSendChange(event: Event) {
  if (!props.config.sttApiConfigId) {
    props.config.sttAutoSend = false;
    emit("patchConversationApiSettings", {
      sttApiConfigId: null,
      sttAutoSend: false,
    });
    return;
  }
  props.config.sttAutoSend = (event.target as HTMLInputElement).checked;
  emit("patchConversationApiSettings", {
    sttAutoSend: !!props.config.sttAutoSend,
  });
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
  emit("patchChatSettings", {
    backgroundVoiceScreenshotKeywords: backgroundVoiceScreenshotKeywordsDraft.value,
  });
}

function onBackgroundVoiceScreenshotModeChange(value: "desktop" | "focused_window") {
  emit("update:backgroundVoiceScreenshotMode", value);
  emit("patchChatSettings", {
    backgroundVoiceScreenshotMode: value,
  });
}

function randomInstructionPresetId(): string {
  return `instruction-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function normalizeInstructionPresets(value: PromptCommandPreset[]): PromptCommandPreset[] {
  return (Array.isArray(value) ? value : [])
    .map((item) => ({
      id: String(item?.id || "").trim() || randomInstructionPresetId(),
      name: String(item?.prompt || item?.name || "").trim(),
      prompt: String(item?.prompt || item?.name || "").trim(),
    }))
    .filter((item) => !!item.prompt);
}

const instructionPresetsDraft = ref<PromptCommandPreset[]>(normalizeInstructionPresets(props.instructionPresets));

watch(
  () => props.instructionPresets,
  (value) => {
    instructionPresetsDraft.value = normalizeInstructionPresets(value);
  },
  { deep: true },
);

const instructionPresetsDirty = computed(() =>
  JSON.stringify(instructionPresetsDraft.value) !== JSON.stringify(normalizeInstructionPresets(props.instructionPresets)),
);

function addInstructionPreset() {
  instructionPresetsDraft.value = [
    ...instructionPresetsDraft.value,
    {
      id: randomInstructionPresetId(),
      name: "",
      prompt: "",
    },
  ];
}

function removeInstructionPreset(id: string) {
  instructionPresetsDraft.value = instructionPresetsDraft.value.filter((item) => item.id !== id);
}

function saveInstructionPresets() {
  const normalized = instructionPresetsDraft.value
    .map((item) => ({
      id: String(item.id || "").trim() || randomInstructionPresetId(),
      name: String(item.prompt || item.name || "").trim(),
      prompt: String(item.prompt || item.name || "").trim(),
    }))
    .filter((item) => !!item.prompt);
  instructionPresetsDraft.value = normalized;
  emit("update:instructionPresets", normalized);
  emit("patchChatSettings", {
    instructionPresets: normalized,
  });
}
</script>
