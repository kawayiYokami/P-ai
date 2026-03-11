<template>
  <div class="flex flex-col gap-6 pb-20 [&_div]:[transition:background-color_200ms,border-color_200ms,box-shadow_200ms,border-radius_200ms_ease-out]">
    <!-- 欢迎主卡片（全宽） -->
    <div class="card bg-base-100 card-border border-base-300 from-base-content/5 bg-gradient-to-bl to-50% card-sm overflow-hidden">
      <div class="card-body gap-6">
        <!-- 标题区域 -->
        <div class="flex items-start justify-between">
          <div class="flex-1">
            <h2 class="flex items-center gap-3 text-2xl font-bold">
              <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="size-7 opacity-30">
                <path stroke-linecap="round" stroke-linejoin="round" d="M9.813 15.904 9 18.75l-.813-2.846a4.5 4.5 0 0 0-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 0 0 3.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 0 0 3.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 0 0-3.09 3.09ZM18.259 8.715 18 9.75l-.259-1.035a3.375 3.375 0 0 0-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 0 0 2.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 0 0 2.456 2.456L21.75 6l-1.035.259a3.375 3.375 0 0 0-2.456 2.456ZM16.894 20.567 16.5 21.75l-.394-1.183a2.25 2.25 0 0 0-1.423-1.423L13.5 18.75l1.183-.394a2.25 2.25 0 0 0 1.423-1.423l.394-1.183.394 1.183a2.25 2.25 0 0 0 1.423 1.423l1.183.394-1.183.394a2.25 2.25 0 0 0-1.423 1.423Z" />
              </svg>
              {{ t("config.welcome.title") }}
            </h2>
            <p class="mt-2 text-base opacity-80">{{ t("config.welcome.subtitle") }}</p>
          </div>
          <!-- 配置完成度 -->
          <div class="radial-progress shrink-0 text-neutral" :style="`--value:${completionRate};--size:5rem`" role="progressbar">
            {{ completionRate }}%
          </div>
        </div>
      </div>
      <!-- 下栏深色背景 -->
      <div class="bg-base-300">
        <div class="flex items-center gap-2 p-4">
          <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="size-5 opacity-60">
            <path stroke-linecap="round" stroke-linejoin="round" d="m11.25 11.25.041-.02a.75.75 0 0 1 1.063.852l-.708 2.836a.75.75 0 0 0 1.063.853l.041-.021M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9-3.75h.008v.008H12V8.25Z" />
          </svg>
          <span class="flex-1">{{ t("config.welcome.askLlmHint") }}</span>
        </div>
      </div>
    </div>

    <!-- 配置卡片网格（2x2） -->
    <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
      <div
        v-for="card in cards"
        :key="card.id"
        class="card bg-base-100 card-border border-base-300 from-base-content/5 bg-gradient-to-bl to-50% card-sm overflow-hidden"
      >
        <div class="card-body gap-4">
          <!-- 标题和状态 -->
          <div class="flex items-center justify-between border-base-300 border-b pb-3">
            <div class="flex items-center gap-2">
              <svg
                v-if="card.ok"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 16 16"
                fill="currentColor"
                class="size-5 text-success"
              >
                <path fill-rule="evenodd" d="M8 15A7 7 0 1 0 8 1a7 7 0 0 0 0 14Zm3.844-8.791a.75.75 0 0 0-1.188-.918l-3.7 4.79-1.649-1.833a.75.75 0 1 0-1.114 1.004l2.25 2.5a.75.75 0 0 0 1.15-.043l4.25-5.5Z" clip-rule="evenodd" />
              </svg>
              <svg
                v-else-if="card.required"
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 16 16"
                fill="currentColor"
                class="size-5 text-error"
              >
                <path fill-rule="evenodd" d="M8 15A7 7 0 1 0 8 1a7 7 0 0 0 0 14ZM8 4a.75.75 0 0 1 .75.75v3a.75.75 0 0 1-1.5 0v-3A.75.75 0 0 1 8 4Zm0 8a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z" clip-rule="evenodd" />
              </svg>
              <svg
                v-else
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 16 16"
                fill="currentColor"
                class="size-5 text-warning"
              >
                <path fill-rule="evenodd" d="M8 15A7 7 0 1 0 8 1a7 7 0 0 0 0 14ZM8 4a.75.75 0 0 1 .75.75v3a.75.75 0 0 1-1.5 0v-3A.75.75 0 0 1 8 4Zm0 8a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z" clip-rule="evenodd" />
              </svg>
              <span class="font-semibold">{{ card.title }}</span>
            </div>
            <div class="flex items-center gap-1.5">
              <span class="badge" :class="card.required ? 'badge-primary' : 'badge-secondary'">
                {{ card.required ? t("config.welcome.requiredBadge") : t("config.welcome.recommendedBadge") }}
              </span>
            </div>
          </div>

          <!-- 描述 -->
          <p class="text-sm opacity-85">{{ card.summary }}</p>

          <!-- 当前状态 -->
          <div class="rounded-box bg-base-200/30 px-3 py-2.5 text-sm">
            <div class="flex items-start gap-2">
              <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="currentColor" class="size-4 mt-0.5 shrink-0 opacity-50">
                <path fill-rule="evenodd" d="M15 8A7 7 0 1 1 1 8a7 7 0 0 1 14 0Zm-6 3.5a1 1 0 1 1-2 0 1 1 0 0 1 2 0ZM7.293 5.293a1 1 0 1 1 .99 1.667c-.459.134-.765.653-.765 1.165v.75a.75.75 0 0 0 1.5 0v-.75a2.5 2.5 0 1 0-1.725-4.332Z" clip-rule="evenodd" />
              </svg>
              <div class="flex-1">
                <span class="font-medium opacity-70">{{ t("config.welcome.currentState") }}</span>
                {{ card.current }}
              </div>
            </div>
          </div>

          <!-- 操作按钮 -->
          <div class="card-actions justify-end">
            <button class="btn btn-sm btn-primary" @click="$emit('jump', card.targetTab)">
              {{ card.action }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig } from "../../../../types/app";

type ConfigTab = "welcome" | "hotkey" | "api" | "tools" | "mcp" | "skill" | "persona" | "department" | "chatSettings" | "memory" | "task" | "logs" | "appearance" | "about";

const props = defineProps<{
  config: AppConfig;
}>();

defineEmits<{
  (e: "jump", value: ConfigTab): void;
}>();

const { t } = useI18n();

function firstTextModel(apiConfigs: ApiConfigItem[]) {
  return apiConfigs.find((item) => item.enableText);
}

function firstMultimodalTextModel(apiConfigs: ApiConfigItem[]) {
  return apiConfigs.find((item) => item.enableText && item.enableImage);
}

function firstSttModel(apiConfigs: ApiConfigItem[]) {
  return apiConfigs.find((item) => item.requestFormat === "openai_stt");
}

function firstEmbeddingModel(apiConfigs: ApiConfigItem[]) {
  return apiConfigs.find((item) => item.requestFormat === "openai_embedding" || item.requestFormat === "gemini_embedding");
}

const assistantDepartment = computed(() =>
  props.config.departments.find((item) => item.id === "assistant-department" || item.isBuiltInAssistant),
);

const cards = computed(() => {
  const apiConfigs = props.config.apiConfigs || [];
  const textModel = firstTextModel(apiConfigs);
  const multimodalModel = firstMultimodalTextModel(apiConfigs);
  const sttModel = firstSttModel(apiConfigs);
  const embeddingModel = firstEmbeddingModel(apiConfigs);
  const assistant = assistantDepartment.value;
  const assistantModelIds = Array.isArray(assistant?.apiConfigIds) && assistant?.apiConfigIds.length
    ? assistant.apiConfigIds
    : (assistant?.apiConfigId ? [assistant.apiConfigId] : []);
  const assistantModels = assistantModelIds
    .map((id) => apiConfigs.find((api) => api.id === id && api.enableText))
    .filter((item): item is ApiConfigItem => !!item);

  return [
    {
      id: "text-model",
      title: t("config.welcome.cards.textModel.title"),
      required: true,
      ok: !!textModel,
      summary: t("config.welcome.cards.textModel.summary"),
      current: textModel
        ? (multimodalModel
            ? t("config.welcome.cards.textModel.currentOkMultimodal", { name: multimodalModel.name })
            : t("config.welcome.cards.textModel.currentOk", { name: textModel.name }))
        : t("config.welcome.cards.textModel.currentMissing"),
      action: t("config.welcome.cards.textModel.action"),
      targetTab: "api" as ConfigTab,
    },
    {
      id: "assistant-department-model",
      title: t("config.welcome.cards.assistantDepartment.title"),
      required: true,
      ok: assistantModels.length > 0,
      summary: t("config.welcome.cards.assistantDepartment.summary"),
      current: assistantModels.length > 0
        ? t("config.welcome.cards.assistantDepartment.currentOk", { names: assistantModels.map((item) => item.name).join(" -> ") })
        : t("config.welcome.cards.assistantDepartment.currentMissing"),
      action: t("config.welcome.cards.assistantDepartment.action"),
      targetTab: "department" as ConfigTab,
    },
    {
      id: "voice",
      title: t("config.welcome.cards.voice.title"),
      required: false,
      ok: !!sttModel,
      summary: t("config.welcome.cards.voice.summary"),
      current: sttModel
        ? t("config.welcome.cards.voice.currentOk", { name: sttModel.name })
        : t("config.welcome.cards.voice.currentMissing"),
      action: t("config.welcome.cards.voice.action"),
      targetTab: "api" as ConfigTab,
    },
    {
      id: "embedding",
      title: t("config.welcome.cards.embedding.title"),
      required: false,
      ok: !!embeddingModel,
      summary: t("config.welcome.cards.embedding.summary"),
      current: embeddingModel
        ? t("config.welcome.cards.embedding.currentOk", { name: embeddingModel.name })
        : t("config.welcome.cards.embedding.currentMissing"),
      action: t("config.welcome.cards.embedding.action"),
      targetTab: "api" as ConfigTab,
    },
  ];
});

// 计算配置完成率
const completionRate = computed(() => {
  const total = cards.value.length;
  const completed = cards.value.filter(card => card.ok).length;
  return Math.round((completed / total) * 100);
});
</script>
