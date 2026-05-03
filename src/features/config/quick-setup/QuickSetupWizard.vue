<template>
  <div class="min-h-screen bg-base-100 text-base-content">
    <div class="flex h-screen min-h-0 flex-col overflow-hidden">
      <header class="grid h-10 shrink-0 grid-cols-[1fr_auto_1fr] items-center border-b border-base-300 bg-base-200 px-2 select-none" data-tauri-drag-region>
        <div class="flex justify-self-start">
          <button class="btn btn-ghost btn-xs h-7 min-h-7 w-7 px-0" type="button" :aria-label="t('quickSetup.advancedSettings')" @click.stop="openConfigWindow">
            <SlidersHorizontal class="h-3.5 w-3.5" />
          </button>
        </div>
        <div class="min-w-0 justify-self-center px-8" data-tauri-drag-region>
          <span class="block truncate text-sm font-semibold">{{ t("quickSetup.title") }}</span>
        </div>
        <div class="flex justify-self-end gap-1">
          <button class="btn btn-ghost btn-xs h-7 min-h-7 w-7 px-0" type="button" :aria-label="t('quickSetup.minimize')" @click.stop="minimizeWindow">
            <Minus class="h-3.5 w-3.5" />
          </button>
          <button class="btn btn-ghost btn-xs h-7 min-h-7 w-7 px-0 hover:bg-error" type="button" :aria-label="t('quickSetup.close')" @click.stop="closeWindow">
            <X class="h-3.5 w-3.5" />
          </button>
        </div>
      </header>

      <main v-if="loading" class="flex flex-1 items-center justify-center">
        <span class="loading loading-spinner loading-md"></span>
      </main>

      <main v-else class="flex min-h-0 flex-1 flex-col bg-base-100">
        <div class="shrink-0 border-b border-base-300 bg-base-100 px-4 py-3">
          <div class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3">
            <button class="btn btn-sm" type="button" :disabled="stepIndex === 0 || saving" @click="stepIndex -= 1">{{ t("quickSetup.actions.previous") }}</button>
            <div class="min-w-0">
                <div class="text-[11px] opacity-60">{{ t("quickSetup.stepCounter", { current: stepIndex + 1, total: visibleSteps.length }) }}</div>
                <h1 class="mt-1 text-base font-semibold">{{ currentStep.title }}</h1>
                <p class="mt-1 text-xs opacity-70">{{ currentStep.summary }}</p>
            </div>
            <div class="flex justify-end gap-2">
              <button v-if="currentStep.advanced" class="btn btn-sm" type="button" :disabled="saving" @click="skipAdvancedStep">{{ t("quickSetup.actions.skip") }}</button>
              <button
                v-if="currentStep.id !== 'finish'"
                class="btn btn-sm btn-primary"
                type="button"
                :disabled="saving || (currentStep.id === 'llm' && testingModel)"
                @click="handleNext"
              >
                {{ isLastStep ? t("quickSetup.actions.finishAndOpenChat") : t("quickSetup.actions.next") }}
              </button>
            </div>
          </div>
        </div>

        <section class="min-h-0 flex-1 overflow-y-auto px-4 py-3">
            <div class="grid content-start gap-3">

                <div v-if="errorText" class="alert alert-error py-2 text-sm">{{ errorText }}</div>
                <div v-if="statusText" class="alert alert-info py-2 text-sm">{{ statusText }}</div>

                <template v-if="currentStep.id === 'voice-theme'">
                  <div class="grid gap-4">
                    <div class="grid gap-2">
                      <div class="text-xs font-medium opacity-70">{{ t("appearance.language") }}</div>
                      <div class="grid grid-cols-3 gap-2">
                        <button
                          v-for="option in languageOptions"
                          :key="option.value"
                          class="btn btn-sm"
                          :class="config.uiLanguage === option.value ? 'btn-primary' : 'bg-base-200'"
                          type="button"
                          @click="setUiLanguage(option.value)"
                        >
                          {{ option.label }}
                        </button>
                      </div>
                    </div>
                    <div class="grid gap-2">
                      <div class="text-xs font-medium opacity-70">{{ t("appearance.theme") }}</div>
                      <div class="grid grid-cols-2 gap-2">
                        <button
                          v-for="option in themeOptions"
                          :key="option.value"
                          class="btn btn-sm"
                          :class="themeDraft === option.value ? 'btn-primary' : 'bg-base-200'"
                          type="button"
                          @click="setThemeDraft(option.value)"
                        >
                          {{ option.label }}
                        </button>
                      </div>
                    </div>
                    <div class="grid gap-2 border-t border-base-300 pt-3">
                      <div class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.backgroundWake") }}</div>
                      <div class="grid grid-cols-2 gap-2">
                        <button
                          class="btn btn-sm"
                          :class="config.recordBackgroundWakeEnabled ? 'btn-primary' : 'bg-base-200'"
                          type="button"
                          @click="config.recordBackgroundWakeEnabled = true"
                        >
                          {{ t("quickSetup.actions.enable") }}
                        </button>
                        <button
                          class="btn btn-sm"
                          :class="!config.recordBackgroundWakeEnabled ? 'btn-primary' : 'bg-base-200'"
                          type="button"
                          @click="config.recordBackgroundWakeEnabled = false"
                        >
                          {{ t("quickSetup.actions.disable") }}
                        </button>
                      </div>
                      <div class="text-xs opacity-70">{{ t("quickSetup.hints.backgroundWake") }}</div>
                    </div>
                  </div>
                </template>

                <template v-else-if="currentStep.id === 'llm'">
                  <div class="grid gap-3">
                    <div class="grid gap-2">
                      <div class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.provider") }}</div>
                      <div class="grid grid-cols-4 gap-2">
                        <button
                          v-for="option in providerOptions"
                          :key="option.id"
                          class="btn btn-sm"
                          :class="selectedProviderId === option.id ? 'btn-primary' : 'bg-base-200'"
                          type="button"
                          @click="selectProvider(option.id)"
                        >
                          {{ providerLabel(option) }}
                        </button>
                      </div>
                    </div>
                    <label class="grid gap-2">
                      <span class="text-xs font-medium opacity-70">base_url</span>
                      <input v-model.trim="llmDraft.baseUrl" class="input input-bordered font-mono" />
                    </label>
                    <div class="grid gap-2">
                      <div class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.apiKey") }}</div>
                      <div class="grid grid-cols-[minmax(0,1fr)_auto_auto] gap-2">
                        <input v-model.trim="llmDraft.apiKey" :type="showApiKey ? 'text' : 'password'" class="input input-bordered min-w-0 font-mono" placeholder="sk-..." />
                        <button
                          class="btn bg-base-200 px-3"
                          type="button"
                          :aria-label="showApiKey ? t('quickSetup.actions.hideKey') : t('quickSetup.actions.showKey')"
                          @click="showApiKey = !showApiKey"
                        >
                          <EyeOff v-if="showApiKey" class="h-4 w-4" />
                          <Eye v-else class="h-4 w-4" />
                        </button>
                        <button class="btn bg-base-200" type="button" @click="openProviderKeyUrl">
                          {{ t("quickSetup.actions.getKey") }}
                        </button>
                      </div>
                    </div>
                    <div class="grid gap-2">
                      <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.model") }}</span>
                      <div class="grid grid-cols-[minmax(0,1fr)_auto_auto] gap-2">
                        <input v-model.trim="llmDraft.model" class="input input-bordered min-w-0 font-mono" />
                        <button class="btn bg-base-200" type="button" :disabled="refreshingModels" @click="refreshProviderModels">
                          <span v-if="refreshingModels" class="loading loading-spinner loading-xs"></span>
                          {{ t("quickSetup.actions.refreshModels") }}
                        </button>
                        <button class="btn bg-base-200" type="button" :disabled="testingModel" @click="testProviderConnection">
                          <span v-if="testingModel" class="loading loading-spinner loading-xs"></span>
                          {{ t("quickSetup.actions.testConnection") }}
                        </button>
                      </div>
                      <div v-if="llmStatusText" class="alert alert-info py-2 text-sm">{{ llmStatusText }}</div>
                      <div v-if="modelOptions.length > 0" class="grid grid-cols-2 gap-2">
                        <button
                          v-for="model in modelOptions"
                          :key="model"
                          class="btn btn-sm justify-start font-mono"
                          :class="llmDraft.model === model ? 'btn-primary' : 'bg-base-200'"
                          type="button"
                          @click="llmDraft.model = model"
                        >
                          {{ model }}
                        </button>
                      </div>
                    </div>
                  </div>
                </template>

                <template v-else-if="currentStep.id === 'style'">
                  <div class="grid grid-cols-3 gap-2">
                    <button
                      v-for="style in responseStyleOptions"
                      :key="style.id"
                      class="btn"
                      :class="chatSettings.responseStyleId === style.id ? 'btn-primary' : 'bg-base-200'"
                      type="button"
                      @click="chatSettings.responseStyleId = style.id"
                    >
                      {{ t(`responseStyle.${style.id}`) }}
                    </button>
                  </div>
                </template>

                <template v-else-if="currentStep.id === 'identity'">
                  <div class="grid gap-3">
                    <label class="grid gap-2">
                      <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.userName") }}</span>
                      <input v-model.trim="identityDraft.userAlias" class="input input-bordered" />
                    </label>
                    <label class="grid gap-2">
                      <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.assistantName") }}</span>
                      <input v-model.trim="identityDraft.assistantName" class="input input-bordered" />
                    </label>
                    <label class="grid gap-2">
                      <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.departmentName") }}</span>
                      <input v-model.trim="identityDraft.departmentName" class="input input-bordered" />
                    </label>
                    <div class="grid gap-2 border-t border-base-300 pt-3">
                      <div class="text-sm font-medium">{{ t("quickSetup.fields.avatar") }}</div>
                      <div class="grid grid-cols-2 gap-2">
                        <button class="btn h-auto justify-start bg-base-200 p-3" type="button" @click="openAvatarEditor('user')">
                          <div class="avatar">
                            <div class="w-10 rounded-full">
                              <img v-if="userAvatarUrl" :src="userAvatarUrl" :alt="identityDraft.userAlias" />
                              <div v-else class="grid h-full w-full place-items-center bg-neutral text-neutral-content">{{ avatarInitial(identityDraft.userAlias) }}</div>
                            </div>
                          </div>
                          <span class="min-w-0 truncate">{{ t("quickSetup.actions.chooseUserAvatar") }}</span>
                        </button>
                        <button class="btn h-auto justify-start bg-base-200 p-3" type="button" @click="openAvatarEditor('assistant')">
                          <div class="avatar">
                            <div class="w-10 rounded-full">
                              <img v-if="assistantAvatarUrl" :src="assistantAvatarUrl" :alt="identityDraft.assistantName" />
                              <div v-else class="grid h-full w-full place-items-center bg-neutral text-neutral-content">{{ avatarInitial(identityDraft.assistantName) }}</div>
                            </div>
                          </div>
                          <span class="min-w-0 truncate">{{ t("quickSetup.actions.chooseAssistantAvatar") }}</span>
                        </button>
                      </div>
                      <div class="text-xs opacity-70">{{ t("quickSetup.hints.avatar") }}</div>
                    </div>
                  </div>
                </template>

                <template v-else-if="currentStep.id === 'workspace'">
                  <div class="grid gap-3">
                    <label class="grid gap-2">
                      <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.workspaceName") }}</span>
                      <input v-model.trim="workspaceDraft.name" class="input input-bordered" />
                    </label>
                    <label class="grid gap-2">
                      <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.privateDirectory") }}</span>
                      <input v-model.trim="workspaceDraft.path" class="input input-bordered font-mono" />
                      <button class="btn btn-sm bg-base-200 justify-start" type="button" @click="pickWorkspacePath">{{ t("quickSetup.actions.chooseDirectory") }}</button>
                    </label>
                    <div class="text-xs opacity-70">{{ t("quickSetup.hints.privateDirectory") }}</div>
                  </div>
                </template>

                <template v-else-if="currentStep.id === 'hotkey'">
                  <div class="grid gap-3">
                    <label class="grid gap-2">
                      <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.summonHotkey") }}</span>
                      <input :value="config.hotkey" class="input input-bordered" readonly />
                      <button class="btn btn-sm justify-start" :class="hotkeyCaptureTarget === 'summon' ? 'btn-primary' : 'bg-base-200'" type="button" @click="startHotkeyCapture('summon')">{{ t("quickSetup.actions.record") }}</button>
                    </label>
                    <label class="grid gap-2">
                      <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.recordHotkey") }}</span>
                      <input :value="config.recordHotkey" class="input input-bordered" readonly />
                      <button class="btn btn-sm justify-start" :class="hotkeyCaptureTarget === 'record' ? 'btn-primary' : 'bg-base-200'" type="button" @click="startHotkeyCapture('record')">{{ t("quickSetup.actions.record") }}</button>
                    </label>
                    <div class="text-xs opacity-70">{{ hotkeyCaptureHint }}</div>
                  </div>
                </template>

                <template v-else-if="currentStep.id === 'finish'">
                  <div class="grid gap-3">
                    <div class="alert alert-success">{{ t("quickSetup.hints.readyToOpenChat") }}</div>
                    <button class="btn btn-primary" type="button" :disabled="saving" @click="finishBasicSetup">{{ t("quickSetup.actions.finishAndOpenChat") }}</button>
                    <button class="btn" type="button" @click="enterAdvanced">{{ t("quickSetup.actions.continueAdvanced") }}</button>
                  </div>
                </template>

                <template v-else-if="currentStep.id === 'advanced-rerank'">
                  <div class="grid gap-3">
                    <AdvancedPresetButtons
                      :options="rerankPresetOptions"
                      :selected-id="selectedRerankPresetId"
                      @select="selectRerankPreset"
                    />
                    <AdvancedProviderForm v-model="rerankDraft" />
                  </div>
                </template>

                <template v-else-if="currentStep.id === 'advanced-embedding'">
                  <div class="grid gap-3">
                    <AdvancedPresetButtons
                      :options="embeddingPresetOptions"
                      :selected-id="selectedEmbeddingPresetId"
                      @select="selectEmbeddingPreset"
                    />
                    <AdvancedProviderForm v-model="embeddingDraft" />
                  </div>
                </template>

                <template v-else-if="currentStep.id === 'advanced-stt'">
                  <AdvancedProviderForm v-model="sttDraft" />
                </template>

          </div>
        </section>
      </main>

      <input ref="avatarFileInput" type="file" accept="image/*" class="hidden" @change="onAvatarPicked" />
      <dialog ref="avatarEditorDialog" class="modal">
        <div class="modal-box max-w-sm p-3">
          <h3 class="mb-2 text-sm font-semibold">{{ t("config.persona.editAvatar") }}</h3>
          <div class="rounded border border-base-300 bg-base-100 p-3">
            <div class="flex items-center gap-3">
              <div class="avatar">
                <div class="w-14 rounded-full">
                  <img v-if="avatarEditorAvatarUrl" :src="avatarEditorAvatarUrl" :alt="avatarEditorName" />
                  <div v-else class="grid h-full w-full place-items-center bg-neutral text-neutral-content">{{ avatarInitial(avatarEditorName) }}</div>
                </div>
              </div>
              <div class="min-w-0 text-sm opacity-70 break-all">{{ avatarEditorName }}</div>
            </div>
            <div class="mt-3 flex gap-2">
              <button class="btn btn-sm" type="button" :disabled="avatarSaving || !avatarEditorPersona" @click="openAvatarPickerForEditor">{{ t("config.persona.uploadAvatar") }}</button>
              <button class="btn btn-sm btn-ghost" type="button" :disabled="avatarSaving || !avatarEditorHasAvatar" @click="clearAvatarFromEditor">{{ t("config.persona.clearAvatar") }}</button>
            </div>
            <div v-if="avatarError" class="mt-2 text-sm text-error break-all">{{ avatarError }}</div>
          </div>
          <div class="modal-action mt-2">
            <button class="btn btn-sm btn-ghost" type="button" @click="closeAvatarEditor">{{ t("common.close") }}</button>
          </div>
        </div>
        <form method="dialog" class="modal-backdrop">
          <button aria-label="close">close</button>
        </form>
      </dialog>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, defineComponent, h, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { Eye, EyeOff, Minus, SlidersHorizontal, X } from "lucide-vue-next";
import { i18n, normalizeLocale } from "../../../i18n";
import { invokeTauri } from "../../../services/tauri-api";
import type { ApiProviderConfigItem, ApiRequestFormat, AppBootstrapSnapshot, AppConfig, ChatSettings, PersonaProfile, ResponseStyleOption } from "../../../types/app";
import responseStylesJson from "../../../constants/response-styles.json";
import { useAvatarCache } from "../../chat/composables/use-avatar-cache";
import { isDarkAppTheme, useAppTheme } from "../../shell/composables/use-app-theme";
import { defaultToolBindings } from "../utils/builtin-tools";
import { normalizeApiRequestFormat } from "../utils/api-request-format";
import { hasUsableTextLlm } from "./usable-text-llm";

type StepId = "voice-theme" | "llm" | "style" | "identity" | "workspace" | "hotkey" | "finish" | "advanced-rerank" | "advanced-embedding" | "advanced-stt";
type StepDefinition = { id: StepId; titleKey: string; summaryKey: string; advanced?: boolean };
type StepItem = { id: StepId; title: string; summary: string; advanced?: boolean };
type AdvancedDraft = { name: string; requestFormat: AppConfig["apiProviders"][number]["requestFormat"]; baseUrl: string; apiKey: string; model: string };
type AdvancedPresetId = "siliconflow" | "google";
type AdvancedProviderPreset = AdvancedDraft & { id: AdvancedPresetId; label: string };
type QuickProviderId = "deepseek" | "kimi" | "zai" | "gemini" | "openai" | "openrouter" | "custom";
type QuickProviderPreset = {
  id: QuickProviderId;
  label: string;
  requestFormat: ApiRequestFormat;
  baseUrl: string;
  keyUrl: string;
  defaultModel: string;
};

const responseStyleOptions = (responseStylesJson as ResponseStyleOption[]).filter((style) => style.id !== "abstract");
const providerOptions: QuickProviderPreset[] = [
  { id: "deepseek", label: "DeepSeek", requestFormat: "deepseek", baseUrl: "https://api.deepseek.com/v1", keyUrl: "https://platform.deepseek.com/api_keys", defaultModel: "deepseek-v4-flash" },
  { id: "kimi", label: "Kimi", requestFormat: "deepseek/kimi", baseUrl: "https://api.moonshot.cn/v1", keyUrl: "https://www.kimi.com/membership/subscription", defaultModel: "kimi" },
  { id: "zai", label: "Zai", requestFormat: "zai", baseUrl: "https://api.z.ai/api/paas/v4", keyUrl: "https://bigmodel.cn/glm-coding", defaultModel: "glm-4.7" },
  { id: "gemini", label: "Gemini", requestFormat: "gemini", baseUrl: "https://generativelanguage.googleapis.com", keyUrl: "https://aistudio.google.com/api-keys", defaultModel: "gemini-2.5-flash" },
  { id: "openai", label: "OpenAI", requestFormat: "openai", baseUrl: "https://api.openai.com/v1", keyUrl: "https://platform.openai.com", defaultModel: "gpt-4o-mini" },
  { id: "openrouter", label: "OpenRouter", requestFormat: "openai", baseUrl: "https://openrouter.ai/api/v1", keyUrl: "https://openrouter.ai/", defaultModel: "openai/gpt-4o-mini" },
  { id: "custom", label: "Custom", requestFormat: "auto", baseUrl: "https://api.openai.com/v1", keyUrl: "https://platform.openai.com", defaultModel: "gpt-4o-mini" },
];
const rerankPresetOptions: AdvancedProviderPreset[] = [
  { id: "siliconflow", label: "硅基流动", name: "SiliconFlow Rerank", requestFormat: "openai_rerank", baseUrl: "https://api.siliconflow.cn/v1", apiKey: "", model: "BAAI/bge-reranker-v2-m3" },
];
const embeddingPresetOptions: AdvancedProviderPreset[] = [
  { id: "siliconflow", label: "硅基流动", name: "SiliconFlow Embedding", requestFormat: "openai_embedding", baseUrl: "https://api.siliconflow.cn/v1", apiKey: "", model: "BAAI/bge-m3" },
  { id: "google", label: "Google", name: "Google Embedding", requestFormat: "gemini_embedding", baseUrl: "https://generativelanguage.googleapis.com", apiKey: "", model: "gemini-embedding-001" },
];
const appWindow = getCurrentWindow();
const { locale, t } = useI18n();
const { currentTheme, restoreThemeFromStorage, setTheme } = useAppTheme();
const languageOptions = computed<Array<{ value: AppConfig["uiLanguage"]; label: string }>>(() => [
  { value: "zh-CN", label: t("quickSetup.languages.zhCN") },
  { value: "zh-TW", label: t("quickSetup.languages.zhTW") },
  { value: "en-US", label: t("quickSetup.languages.enUS") },
]);
const themeOptions = computed<Array<{ value: "corporate" | "dracula"; label: string }>>(() => [
  { value: "corporate", label: t("quickSetup.themeOptions.light") },
  { value: "dracula", label: t("quickSetup.themeOptions.dark") },
]);
const BASIC_STEP_DEFS: StepDefinition[] = [
  { id: "voice-theme", titleKey: "quickSetup.steps.voiceTheme.title", summaryKey: "quickSetup.steps.voiceTheme.summary" },
  { id: "llm", titleKey: "quickSetup.steps.llm.title", summaryKey: "quickSetup.steps.llm.summary" },
  { id: "style", titleKey: "quickSetup.steps.style.title", summaryKey: "quickSetup.steps.style.summary" },
  { id: "identity", titleKey: "quickSetup.steps.identity.title", summaryKey: "quickSetup.steps.identity.summary" },
  { id: "workspace", titleKey: "quickSetup.steps.workspace.title", summaryKey: "quickSetup.steps.workspace.summary" },
  { id: "hotkey", titleKey: "quickSetup.steps.hotkey.title", summaryKey: "quickSetup.steps.hotkey.summary" },
  { id: "finish", titleKey: "quickSetup.steps.finish.title", summaryKey: "quickSetup.steps.finish.summary" },
];
const ADVANCED_STEP_DEFS: StepDefinition[] = [
  { id: "advanced-rerank", titleKey: "quickSetup.steps.advancedRerank.title", summaryKey: "quickSetup.steps.advancedRerank.summary", advanced: true },
  { id: "advanced-embedding", titleKey: "quickSetup.steps.advancedEmbedding.title", summaryKey: "quickSetup.steps.advancedEmbedding.summary", advanced: true },
  { id: "advanced-stt", titleKey: "quickSetup.steps.advancedStt.title", summaryKey: "quickSetup.steps.advancedStt.summary", advanced: true },
];
const QUICK_LLM_PROVIDER_ID = "quick-setup-provider";
const QUICK_LLM_MODEL_ID = "quick-setup-model";
const QUICK_LLM_ENDPOINT_ID = `${QUICK_LLM_PROVIDER_ID}::${QUICK_LLM_MODEL_ID}`;
const QUICK_SETUP_TEST_PROMPT = "这是一个连通性测试。请回复一个OK。";

const AdvancedPresetButtons = defineComponent({
  props: {
    options: { type: Array, required: true },
    selectedId: { type: String, required: true },
  },
  emits: ["select"],
  setup(props, { emit }) {
    return () => h("div", { class: "grid gap-2" }, [
      h("div", { class: "text-xs font-medium opacity-70" }, t("quickSetup.fields.provider")),
      h("div", { class: "grid grid-cols-2 gap-2" }, (props.options as AdvancedProviderPreset[]).map((option) =>
        h("button", {
          key: option.id,
          class: ["btn btn-sm", props.selectedId === option.id ? "btn-primary" : "bg-base-200"],
          type: "button",
          onClick: () => emit("select", option.id),
        }, option.label),
      )),
    ]);
  },
});

const AdvancedProviderForm = defineComponent({
  props: {
    modelValue: { type: Object, required: true },
  },
  emits: ["update:modelValue"],
  setup(props, { emit }) {
    const showKey = ref(false);
    function patch(key: keyof AdvancedDraft, value: string) {
      emit("update:modelValue", { ...(props.modelValue as AdvancedDraft), [key]: value });
    }
    return () => h("div", { class: "grid gap-3" }, [
      h("label", { class: "grid gap-2" }, [
        h("span", { class: "text-xs font-medium opacity-70" }, t("quickSetup.fields.providerName")),
        h("input", { class: "input input-bordered", value: (props.modelValue as AdvancedDraft).name, onInput: (event: Event) => patch("name", (event.target as HTMLInputElement).value) }),
      ]),
      h("label", { class: "grid gap-2" }, [
        h("span", { class: "text-xs font-medium opacity-70" }, "base_url"),
        h("input", { class: "input input-bordered font-mono", value: (props.modelValue as AdvancedDraft).baseUrl, onInput: (event: Event) => patch("baseUrl", (event.target as HTMLInputElement).value) }),
      ]),
      h("div", { class: "grid gap-2" }, [
        h("span", { class: "text-xs font-medium opacity-70" }, t("quickSetup.fields.apiKey")),
        h("div", { class: "grid grid-cols-[minmax(0,1fr)_auto] gap-2" }, [
          h("input", { type: showKey.value ? "text" : "password", class: "input input-bordered min-w-0 font-mono", value: (props.modelValue as AdvancedDraft).apiKey, onInput: (event: Event) => patch("apiKey", (event.target as HTMLInputElement).value) }),
          h("button", { class: "btn bg-base-200 px-3", type: "button", ariaLabel: showKey.value ? t("quickSetup.actions.hideKey") : t("quickSetup.actions.showKey"), onClick: () => { showKey.value = !showKey.value; } }, [
            h(showKey.value ? EyeOff : Eye, { class: "h-4 w-4" }),
          ]),
        ]),
      ]),
      h("label", { class: "grid gap-2" }, [
        h("span", { class: "text-xs font-medium opacity-70" }, t("quickSetup.fields.model")),
        h("input", { class: "input input-bordered font-mono", value: (props.modelValue as AdvancedDraft).model, onInput: (event: Event) => patch("model", (event.target as HTMLInputElement).value) }),
      ]),
    ]);
  },
});

const isMacPlatform = typeof navigator !== "undefined" && /mac/i.test(navigator.platform || "");
const loading = ref(true);
const saving = ref(false);
const errorText = ref("");
const statusText = ref("");
const advancedMode = ref(false);
const stepIndex = ref(0);
const themeDraft = ref<"corporate" | "dracula">("corporate");
const hotkeyCaptureTarget = ref<"summon" | "record" | null>(null);
const hotkeyCaptureHint = ref(t("quickSetup.hotkeyHints.idle"));
const selectedProviderId = ref<QuickProviderId>("deepseek");
const refreshingModels = ref(false);
const testingModel = ref(false);
const showApiKey = ref(false);
const modelOptions = ref<string[]>([]);
const llmStatusText = ref("");
const llmConnectionTestSignature = ref("");
const llmConnectionBypassSignature = ref("");
const selectedRerankPresetId = ref<AdvancedPresetId>("siliconflow");
const selectedEmbeddingPresetId = ref<AdvancedPresetId>("siliconflow");
const avatarFileInput = ref<HTMLInputElement | null>(null);
const avatarEditorDialog = ref<HTMLDialogElement | null>(null);
const avatarEditorTarget = ref<"user" | "assistant" | null>(null);
const avatarSaving = ref(false);
const avatarError = ref("");
let hotkeyCaptureHandler: ((event: KeyboardEvent) => void) | null = null;

const config = reactive<AppConfig>(defaultConfig());
const chatSettings = reactive<ChatSettings>(defaultChatSettings());
const personas = ref<PersonaProfile[]>([]);
const { resolveAvatarUrl, ensureAvatarCached, preloadPersonaAvatars } = useAvatarCache({ personas });
const identityDraft = reactive({ userAlias: "用户", assistantName: "派师傅", departmentName: "助理部门" });
const workspaceDraft = reactive({ name: "easy_call_ai", path: "" });
const llmDraft = reactive<AdvancedDraft>({
  name: "DeepSeek",
  requestFormat: "deepseek",
  baseUrl: "https://api.deepseek.com/v1",
  apiKey: "",
  model: "deepseek-v4-flash",
});
const rerankDraft = ref<AdvancedDraft>({ name: "SiliconFlow Rerank", requestFormat: "openai_rerank", baseUrl: "https://api.siliconflow.cn/v1", apiKey: "", model: "BAAI/bge-reranker-v2-m3" });
const embeddingDraft = ref<AdvancedDraft>({ name: "SiliconFlow Embedding", requestFormat: "openai_embedding", baseUrl: "https://api.siliconflow.cn/v1", apiKey: "", model: "BAAI/bge-m3" });
const sttDraft = ref<AdvancedDraft>({ name: "SiliconFlow STT", requestFormat: "openai_stt", baseUrl: "https://api.siliconflow.cn/v1", apiKey: "", model: "FunAudioLLM/SenseVoiceSmall" });

function stepFromDefinition(step: StepDefinition): StepItem {
  return {
    id: step.id,
    title: t(step.titleKey),
    summary: t(step.summaryKey),
    advanced: step.advanced,
  };
}

const visibleSteps = computed(() => (advancedMode.value ? [...BASIC_STEP_DEFS, ...ADVANCED_STEP_DEFS] : BASIC_STEP_DEFS).map(stepFromDefinition));
const currentStep = computed(() => visibleSteps.value[stepIndex.value] || visibleSteps.value[0]);
const isLastStep = computed(() => stepIndex.value >= visibleSteps.value.length - 1);
const selectedProvider = computed(() => providerOptions.find((item) => item.id === selectedProviderId.value) || providerOptions[0]);
const userAvatarUrl = computed(() => {
  const user = userPersona();
  return resolveAvatarUrl(user?.avatarPath, user?.avatarUpdatedAt);
});
const assistantAvatarUrl = computed(() => {
  const assistant = assistantPersona();
  return resolveAvatarUrl(assistant?.avatarPath, assistant?.avatarUpdatedAt);
});
const avatarEditorPersona = computed(() => {
  if (avatarEditorTarget.value === "user") return userPersona() || null;
  if (avatarEditorTarget.value === "assistant") return assistantPersona() || null;
  return null;
});
const avatarEditorName = computed(() => {
  if (avatarEditorTarget.value === "user") return identityDraft.userAlias || t("quickSetup.fields.userName");
  if (avatarEditorTarget.value === "assistant") return identityDraft.assistantName || t("quickSetup.fields.assistantName");
  return t("config.persona.avatarFallbackName");
});
const avatarEditorAvatarUrl = computed(() => {
  if (avatarEditorTarget.value === "user") return userAvatarUrl.value;
  if (avatarEditorTarget.value === "assistant") return assistantAvatarUrl.value;
  return "";
});
const avatarEditorHasAvatar = computed(() => !!avatarEditorPersona.value?.avatarPath);
const currentLlmConnectionSignature = computed(() => [
  selectedProviderId.value,
  llmDraft.requestFormat,
  llmDraft.baseUrl.trim(),
  llmDraft.apiKey.trim(),
  llmDraft.model.trim(),
].join("\n"));

watch(() => config.uiLanguage, (value) => {
  applyUiLanguage(value);
});

watch(currentLlmConnectionSignature, (value) => {
  if (llmConnectionTestSignature.value && value !== llmConnectionTestSignature.value) {
    llmStatusText.value = "";
  }
});

onMounted(async () => {
  try {
    restoreThemeFromStorage();
    syncThemeDraftFromCurrentTheme();
    const snapshot = await invokeTauri<AppBootstrapSnapshot>("load_app_bootstrap_snapshot");
    applySnapshot(snapshot);
    await preloadPersonaAvatars();
  } catch (error) {
    errorText.value = `加载配置失败：${String(error ?? "unknown")}`;
  } finally {
    loading.value = false;
  }
});

onBeforeUnmount(() => stopHotkeyCapture());

function defaultConfig(): AppConfig {
  return {
    hotkey: "Alt+·",
    uiLanguage: "zh-CN",
    uiFont: "auto",
    recordHotkey: isMacPlatform ? "Option+Space" : "Alt",
    recordBackgroundWakeEnabled: true,
    minRecordSeconds: 1,
    maxRecordSeconds: 60,
    llmRoundLogCapacity: 3,
    selectedApiConfigId: "",
    assistantDepartmentApiConfigId: "",
    sttAutoSend: false,
    terminalShellKind: "auto",
    shellWorkspaces: [],
    mcpServers: [],
    remoteImChannels: [],
    departments: [],
    apiProviders: [],
    apiConfigs: [],
  };
}

function defaultChatSettings(): ChatSettings {
  return {
    assistantDepartmentAgentId: "default-agent",
    userAlias: "用户",
    responseStyleId: "concise",
    pdfReadMode: "text",
    backgroundVoiceScreenshotKeywords: "",
    backgroundVoiceScreenshotMode: "desktop",
    instructionPresets: [],
  };
}

function applySnapshot(snapshot: AppBootstrapSnapshot) {
  Object.assign(config, defaultConfig(), snapshot.config || {});
  Object.assign(chatSettings, defaultChatSettings(), snapshot.chatSettings || {});
  if (!responseStyleOptions.some((style) => style.id === chatSettings.responseStyleId)) {
    chatSettings.responseStyleId = "concise";
  }
  personas.value = Array.isArray(snapshot.agents) ? snapshot.agents : [];
  identityDraft.userAlias = String(chatSettings.userAlias || "用户").trim() || "用户";
  const assistantAgent = assistantPersona();
  identityDraft.assistantName = String(assistantAgent?.name || "派师傅").trim() || "派师傅";
  identityDraft.departmentName = String(assistantDepartment()?.name || "助理部门").trim() || "助理部门";
  const workspace = config.shellWorkspaces?.[0];
  workspaceDraft.name = String(workspace?.name || "默认工作空间").trim() || "默认工作空间";
  workspaceDraft.path = String(workspace?.path || "").trim();
  const existing = findQuickLlmConfig() || (config.apiConfigs || []).find(isUsableTextLlmConfig);
  if (existing) {
    const preset = providerPresetFromConfig(existing.requestFormat, existing.baseUrl);
    selectedProviderId.value = preset.id;
    llmDraft.name = preset.id === "custom" ? (existing.name || providerLabel(preset)) : providerLabel(preset);
    llmDraft.requestFormat = preset.requestFormat;
    llmDraft.baseUrl = preset.baseUrl;
    llmDraft.apiKey = existing.apiKey;
    llmDraft.model = existing.model || preset.defaultModel;
  }
}

function findQuickLlmConfig() {
  return (config.apiConfigs || []).find((api) => api.id === QUICK_LLM_ENDPOINT_ID || api.id.startsWith(`${QUICK_LLM_PROVIDER_ID}::`));
}

function isUsableTextLlmConfig(api: AppConfig["apiConfigs"][number]): boolean {
  return !!api.enableText
    && !["openai_stt", "openai_embedding", "openai_rerank", "gemini_embedding"].includes(api.requestFormat)
    && !!String(api.baseUrl || "").trim()
    && !!String(api.apiKey || "").trim()
    && !!String(api.model || "").trim();
}

function assistantDepartment() {
  return (config.departments || []).find((item) => item.id === "assistant-department" || item.isBuiltInAssistant);
}

function assistantPersona() {
  const assistantId = String(chatSettings.assistantDepartmentAgentId || "").trim();
  return personas.value.find((item) => item.id === assistantId)
    || personas.value.find((item) => !item.isBuiltInUser && !item.isBuiltInSystem);
}

function userPersona() {
  return personas.value.find((item) => item.id === "user-persona" || item.isBuiltInUser);
}

function avatarInitial(name: string): string {
  const text = String(name || "").trim();
  return text ? text[0].toUpperCase() : "?";
}

function syncThemeDraftFromCurrentTheme() {
  if (currentTheme.value === "corporate" || currentTheme.value === "dracula") {
    themeDraft.value = currentTheme.value;
    return;
  }
  themeDraft.value = isDarkAppTheme(currentTheme.value) ? "dracula" : "corporate";
}

function applyUiLanguage(value: string) {
  const lang = normalizeLocale(value);
  document.documentElement.lang = lang;
  config.uiLanguage = lang;
  locale.value = lang;
  i18n.global.locale.value = lang;
  if (!hotkeyCaptureTarget.value) {
    hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.idle");
  }
}

function setUiLanguage(value: AppConfig["uiLanguage"]) {
  applyUiLanguage(value);
  void emit("easy-call:locale-changed", normalizeLocale(value)).catch((error) => {
    console.warn("[LOCALE] emit easy-call:locale-changed failed:", error);
  });
}

function setThemeDraft(value: "corporate" | "dracula") {
  themeDraft.value = value;
  setTheme(value);
}

function providerPresetFromConfig(requestFormat: ApiRequestFormat, baseUrl: string): QuickProviderPreset {
  const normalizedBaseUrl = String(baseUrl || "").toLowerCase();
  const matchedPreset = providerOptions
    .filter((preset) => preset.id !== "custom")
    .find((preset) => {
      const presetHost = new URL(preset.baseUrl).host.toLowerCase();
      return preset.requestFormat === requestFormat && normalizedBaseUrl.includes(presetHost);
    });
  if (matchedPreset) return matchedPreset;
  const customPreset = providerOptions.find((preset) => preset.id === "custom") || providerOptions[0];
  return {
    ...customPreset,
    requestFormat,
    baseUrl: String(baseUrl || "").trim(),
  };
}

function providerLabel(option: QuickProviderPreset): string {
  return option.id === "custom" ? t("quickSetup.providers.custom") : option.label;
}

function applyAdvancedPreset(draft: AdvancedDraft, preset: AdvancedProviderPreset) {
  const apiKey = draft.apiKey;
  draft.name = preset.name;
  draft.requestFormat = preset.requestFormat;
  draft.baseUrl = preset.baseUrl;
  draft.model = preset.model;
  draft.apiKey = apiKey;
}

function selectRerankPreset(presetId: AdvancedPresetId) {
  const preset = rerankPresetOptions.find((item) => item.id === presetId);
  if (!preset) return;
  selectedRerankPresetId.value = presetId;
  applyAdvancedPreset(rerankDraft.value, preset);
}

function selectEmbeddingPreset(presetId: AdvancedPresetId) {
  const preset = embeddingPresetOptions.find((item) => item.id === presetId);
  if (!preset) return;
  selectedEmbeddingPresetId.value = presetId;
  applyAdvancedPreset(embeddingDraft.value, preset);
}

function selectProvider(providerId: QuickProviderId) {
  selectedProviderId.value = providerId;
  const preset = selectedProvider.value;
  llmDraft.name = providerLabel(preset);
  llmDraft.requestFormat = preset.requestFormat;
  llmDraft.baseUrl = preset.baseUrl;
  llmDraft.model = preset.defaultModel;
  modelOptions.value = [];
  llmStatusText.value = "";
  llmConnectionTestSignature.value = "";
  llmConnectionBypassSignature.value = "";
  statusText.value = "";
  errorText.value = "";
}

function openProviderKeyUrl() {
  void invokeTauri("open_external_url", { url: selectedProvider.value.keyUrl });
}

async function refreshProviderModels() {
  const apiKey = llmDraft.apiKey.trim();
  if (!apiKey) {
    errorText.value = t("quickSetup.errors.apiKeyRequiredForRefresh");
    return;
  }
  refreshingModels.value = true;
  errorText.value = "";
  llmStatusText.value = "";
  try {
    const models = await invokeTauri<string[]>("refresh_models", {
      input: {
        baseUrl: llmDraft.baseUrl.trim(),
        apiKey,
        requestFormat: llmDraft.requestFormat,
        providerId: null,
        codexAuthMode: "read_local",
        codexLocalAuthPath: "~/.codex/auth.json",
      },
    });
    const normalized = models.map((item) => item.trim()).filter(Boolean);
    modelOptions.value = normalized;
    if (normalized.length > 0 && !normalized.includes(llmDraft.model.trim())) {
      llmDraft.model = normalized[0];
    }
    llmStatusText.value = t("quickSetup.status.refreshModelsSuccess", { count: normalized.length });
  } catch (error) {
    errorText.value = t("quickSetup.errors.refreshModelsFailed", { error: String(error ?? "unknown") });
  } finally {
    refreshingModels.value = false;
  }
}

async function testProviderConnection() {
  const apiKey = llmDraft.apiKey.trim();
  const baseUrl = llmDraft.baseUrl.trim();
  const model = llmDraft.model.trim();
  if (!baseUrl || !apiKey || !model) {
    errorText.value = t("quickSetup.errors.connectionFieldsRequired");
    return;
  }
  testingModel.value = true;
  errorText.value = "";
  llmStatusText.value = t("quickSetup.status.testingConnection");
  llmConnectionTestSignature.value = "";
  llmConnectionBypassSignature.value = "";
  statusText.value = "";
  try {
    const reply = await invokeTauri<string>("quick_genai_chat", {
      input: {
        baseUrl,
        apiKey,
        requestFormat: llmDraft.requestFormat,
        model,
        prompt: QUICK_SETUP_TEST_PROMPT,
        providerId: selectedProviderId.value,
      },
    });
    llmConnectionTestSignature.value = currentLlmConnectionSignature.value;
    llmConnectionBypassSignature.value = "";
    llmStatusText.value = t("quickSetup.status.connectionOk", { reply: reply.trim() || "OK" });
  } catch (error) {
    llmConnectionTestSignature.value = "";
    llmConnectionBypassSignature.value = "";
    errorText.value = t("quickSetup.errors.connectionFailed", { error: String(error ?? "unknown") });
    llmStatusText.value = "";
  } finally {
    testingModel.value = false;
  }
}

function validateCurrentStep(): boolean {
  errorText.value = "";
  if (currentStep.value.id === "llm") {
    if (!llmDraft.baseUrl.trim() || !llmDraft.model.trim() || !llmDraft.apiKey.trim()) {
      errorText.value = "LLM 供应商需要 base_url、模型和 API Key。";
      return false;
    }
  }
  if (currentStep.value.id === "identity") {
    if (!identityDraft.userAlias.trim() || !identityDraft.assistantName.trim() || !identityDraft.departmentName.trim()) {
      errorText.value = "用户、助理、主部门名称都不能为空。";
      return false;
    }
  }
  if (currentStep.value.id === "workspace" && !workspaceDraft.path.trim()) {
    errorText.value = "Shell 工作目录不能为空。";
    return false;
  }
  if (currentStep.value.id === "hotkey" && (!config.hotkey.trim() || !config.recordHotkey.trim())) {
    errorText.value = "呼唤热键和录音键不能为空。";
    return false;
  }
  return true;
}

function confirmUntestedLlmConnection(): boolean {
  const signature = currentLlmConnectionSignature.value;
  if (llmConnectionTestSignature.value === signature || llmConnectionBypassSignature.value === signature) {
    return true;
  }
  const confirmed = window.confirm(t("quickSetup.confirmations.untestedConnection"));
  if (confirmed) {
    llmConnectionBypassSignature.value = signature;
  }
  return confirmed;
}

async function handleNext() {
  if (!validateCurrentStep()) return;
  if (currentStep.value.id === "llm" && !confirmUntestedLlmConnection()) return;
  try {
    if (currentStep.value.id === "llm") applyLlmDraft();
    if (currentStep.value.id === "style") await saveChatSettingsOnly();
    if (currentStep.value.id === "identity") await saveIdentityStep();
    if (currentStep.value.id === "workspace") applyWorkspaceDraft();
    if (currentStep.value.advanced) await saveAdvancedCurrentStep();
    if (isLastStep.value) {
      await finishBasicSetup();
      return;
    }
    stepIndex.value += 1;
  } catch (error) {
    errorText.value = String(error ?? "unknown");
  }
}

function enterAdvanced() {
  advancedMode.value = true;
  stepIndex.value = BASIC_STEP_DEFS.length;
  errorText.value = "";
}

async function skipAdvancedStep() {
  if (isLastStep.value) {
    await finishBasicSetup();
  } else {
    stepIndex.value += 1;
  }
}

function upsertProvider(draft: AdvancedDraft, stableIds?: { providerId: string; modelId: string }): string {
  const seed = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const providerId = stableIds?.providerId || `api-provider-${seed}`;
  const modelId = stableIds?.modelId || `api-model-${seed}`;
  const endpointId = `${providerId}::${modelId}`;
  const provider: ApiProviderConfigItem = {
    id: providerId,
    name: draft.name.trim(),
    requestFormat: normalizeApiRequestFormat(draft.requestFormat),
    allowConcurrentRequests: false,
    enableText: draft.requestFormat !== "openai_stt" && draft.requestFormat !== "openai_embedding" && draft.requestFormat !== "openai_rerank" && draft.requestFormat !== "gemini_embedding",
    enableImage: draft.requestFormat !== "openai_stt" && draft.requestFormat !== "openai_embedding" && draft.requestFormat !== "openai_rerank" && draft.requestFormat !== "gemini_embedding",
    enableAudio: draft.requestFormat === "openai_stt",
    enableTools: draft.requestFormat !== "openai_stt" && draft.requestFormat !== "openai_embedding" && draft.requestFormat !== "openai_rerank" && draft.requestFormat !== "gemini_embedding",
    tools: defaultToolBindings(),
    baseUrl: draft.baseUrl.trim(),
    codexAuthMode: "read_local",
    codexLocalAuthPath: "~/.codex/auth.json",
    apiKeys: draft.apiKey.trim() ? [draft.apiKey.trim()] : [],
    keyCursor: 0,
    cachedModelOptions: [draft.model.trim()],
    models: [{
      id: modelId,
      model: draft.model.trim(),
      enableImage: draft.requestFormat !== "openai_stt" && draft.requestFormat !== "openai_embedding" && draft.requestFormat !== "openai_rerank" && draft.requestFormat !== "gemini_embedding",
      enableTools: draft.requestFormat !== "openai_stt" && draft.requestFormat !== "openai_embedding" && draft.requestFormat !== "openai_rerank" && draft.requestFormat !== "gemini_embedding",
      reasoningEffort: "medium",
      temperature: 1,
      customTemperatureEnabled: false,
      contextWindowTokens: 128000,
      customMaxOutputTokensEnabled: false,
      maxOutputTokens: 4096,
    }],
    failureRetryCount: 0,
  };
  const nextProviders = stableIds
    ? (config.apiProviders || []).filter((item) => item.id !== providerId)
    : (config.apiProviders || []).filter((item) => item.id !== providerId && item.name !== provider.name);
  config.apiProviders = [...nextProviders, provider];
  config.apiConfigs = [...(config.apiConfigs || []).filter((item) => !item.id.startsWith(`${providerId}::`)), {
    id: endpointId,
    name: `${provider.name}/${draft.model.trim()}`,
    requestFormat: provider.requestFormat,
    allowConcurrentRequests: false,
    enableText: provider.enableText,
    enableImage: provider.models[0].enableImage,
    enableAudio: provider.enableAudio,
    enableTools: provider.models[0].enableTools,
    tools: defaultToolBindings(),
    baseUrl: provider.baseUrl,
    apiKey: draft.apiKey.trim(),
    codexAuthMode: "read_local",
    codexLocalAuthPath: "~/.codex/auth.json",
    model: draft.model.trim(),
    reasoningEffort: "medium",
    temperature: 1,
    customTemperatureEnabled: false,
    contextWindowTokens: 128000,
    customMaxOutputTokensEnabled: false,
    maxOutputTokens: 4096,
  }];
  return endpointId;
}

function applyLlmDraft() {
  const preset = selectedProvider.value;
  if (preset.id !== "custom") {
    llmDraft.name = providerLabel(preset);
    llmDraft.requestFormat = preset.requestFormat;
  } else if (!llmDraft.name.trim()) {
    llmDraft.name = providerLabel(preset);
  }
  const endpointId = upsertProvider(llmDraft, { providerId: QUICK_LLM_PROVIDER_ID, modelId: QUICK_LLM_MODEL_ID });
  config.selectedApiConfigId = endpointId;
  config.assistantDepartmentApiConfigId = endpointId;
  const department = assistantDepartment();
  if (department) {
    department.name = identityDraft.departmentName.trim() || department.name;
    department.apiConfigId = endpointId;
    department.apiConfigIds = [endpointId];
    department.updatedAt = new Date().toISOString();
  }
}

function applyWorkspaceDraft() {
  config.shellWorkspaces = [{
    id: "system-workspace",
    name: workspaceDraft.name.trim() || "默认工作空间",
    path: workspaceDraft.path.trim(),
    level: "system",
    access: "full_access",
    builtIn: true,
  }];
}

async function saveIdentityStep() {
  chatSettings.userAlias = identityDraft.userAlias.trim();
  const user = userPersona();
  if (user) user.name = identityDraft.userAlias.trim();
  const assistant = assistantPersona();
  if (assistant) {
    assistant.name = identityDraft.assistantName.trim();
    chatSettings.assistantDepartmentAgentId = assistant.id;
  }
  const department = assistantDepartment();
  if (department) {
    department.name = identityDraft.departmentName.trim();
    if (assistant && !department.agentIds.includes(assistant.id)) department.agentIds = [assistant.id];
    department.updatedAt = new Date().toISOString();
  }
  await savePersonasIfNeeded();
  await saveChatSettingsOnly();
}

async function savePersonasIfNeeded() {
  if (personas.value.length === 0) return;
  await invokeTauri<PersonaProfile[]>("save_agents", { input: { agents: personas.value } });
}

async function saveChatSettingsOnly() {
  const saved = await invokeTauri<ChatSettings>("patch_chat_settings", {
    input: {
      assistantDepartmentAgentId: chatSettings.assistantDepartmentAgentId,
      userAlias: chatSettings.userAlias,
      responseStyleId: chatSettings.responseStyleId,
    },
  });
  Object.assign(chatSettings, saved);
}

async function saveConfigOnly() {
  const saved = await invokeTauri<AppConfig>("save_config", { config: { ...config } });
  Object.assign(config, saved);
}

async function saveAdvancedCurrentStep() {
  if (currentStep.value.id === "advanced-rerank") {
    await saveAdvancedProvider(rerankDraft.value, "rerank");
  } else if (currentStep.value.id === "advanced-embedding") {
    await saveAdvancedProvider(embeddingDraft.value, "embedding");
  } else if (currentStep.value.id === "advanced-stt") {
    await saveAdvancedProvider(sttDraft.value, "stt");
  }
}

async function saveAdvancedProvider(draft: AdvancedDraft, kind: "rerank" | "embedding" | "stt") {
  if (!draft.apiKey.trim() || !draft.baseUrl.trim() || !draft.model.trim()) {
    throw new Error("高级模型需要填写 base_url、API Key 和模型；不需要可点跳过。");
  }
  const endpointId = upsertProvider(draft);
  if (kind === "stt") {
    config.sttApiConfigId = endpointId;
    await saveConfigOnly();
    return;
  }
  await saveConfigOnly();
  if (kind === "rerank") {
    await invokeTauri("save_memory_rerank_binding", { input: { apiConfigId: endpointId, modelName: draft.model } });
  } else {
    await invokeTauri("save_memory_embedding_binding", { input: { apiConfigId: endpointId, modelName: draft.model, batchSize: 64 } });
  }
}

async function finishBasicSetup() {
  saving.value = true;
  errorText.value = "";
  try {
    applyLlmDraft();
    applyWorkspaceDraft();
    await saveIdentityStep();
    await saveChatSettingsOnly();
    await saveConfigOnly();
    if (!hasUsableTextLlm(config)) {
      throw new Error("配置已保存，但仍未检测到可用文本 LLM。请检查 API Key、base_url、模型和主部门绑定。");
    }
    await invokeTauri("complete_quick_setup_and_open_chat");
  } catch (error) {
    errorText.value = String(error ?? "unknown");
  } finally {
    saving.value = false;
  }
}

async function pickWorkspacePath() {
  const picked = await open({ directory: true, multiple: false, defaultPath: workspaceDraft.path || undefined });
  if (!picked || Array.isArray(picked)) return;
  workspaceDraft.path = String(picked);
  if (!workspaceDraft.name.trim()) {
    workspaceDraft.name = picked.replace(/\\/g, "/").split("/").filter(Boolean).pop() || "默认工作空间";
  }
}

async function openConfigWindow() {
  await invokeTauri("show_main_window");
}

function minimizeWindow() {
  void appWindow.minimize();
}

function closeWindow() {
  void appWindow.hide();
}

async function openAvatarEditor(target: "user" | "assistant") {
  avatarEditorTarget.value = target;
  avatarError.value = "";
  const persona = target === "user" ? userPersona() : assistantPersona();
  if (persona?.avatarPath) {
    await ensureAvatarCached(persona.avatarPath, persona.avatarUpdatedAt);
  }
  avatarEditorDialog.value?.showModal();
}

function closeAvatarEditor() {
  avatarEditorDialog.value?.close();
}

function openAvatarPickerForEditor() {
  if (!avatarEditorPersona.value) return;
  if (avatarFileInput.value) {
    avatarFileInput.value.value = "";
    avatarFileInput.value.click();
  }
}

async function clearAvatarFromEditor() {
  const agent = avatarEditorPersona.value;
  if (!agent) return;
  avatarSaving.value = true;
  avatarError.value = "";
  try {
    await invokeTauri("clear_agent_avatar", { input: { agentId: agent.id } });
    agent.avatarPath = undefined;
    agent.avatarUpdatedAt = undefined;
    agent.updatedAt = new Date().toISOString();
    statusText.value = t("status.avatarCleared");
  } catch (error) {
    avatarError.value = String(error ?? "unknown");
  } finally {
    avatarSaving.value = false;
  }
}

async function onAvatarPicked(event: Event) {
  const input = event.target as HTMLInputElement | null;
  const file = input?.files?.[0];
  if (!file) return;
  const agent = avatarEditorPersona.value;
  if (!agent) return;
  avatarSaving.value = true;
  avatarError.value = "";
  try {
    const bytesBase64 = await fileToBase64(file);
    const result = await invokeTauri<{ path: string; updatedAt: string }>("save_agent_avatar", {
      input: { agentId: agent.id, mime: file.type || "image/png", bytesBase64 },
    });
    agent.avatarPath = result.path;
    agent.avatarUpdatedAt = result.updatedAt;
    agent.updatedAt = new Date().toISOString();
    await ensureAvatarCached(result.path, result.updatedAt);
    statusText.value = t("status.avatarSaved");
  } catch (error) {
    avatarError.value = String(error ?? "unknown");
  } finally {
    avatarSaving.value = false;
  }
  if (input) input.value = "";
}

function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || "").split(",")[1] || "");
    reader.onerror = () => reject(reader.error || new Error("读取文件失败"));
    reader.readAsDataURL(file);
  });
}

function startHotkeyCapture(target: "summon" | "record") {
  stopHotkeyCapture();
  hotkeyCaptureTarget.value = target;
  hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.recording");
  hotkeyCaptureHandler = (event: KeyboardEvent) => {
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      if (target === "record") {
        config.recordHotkey = "";
        hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.cleared");
        stopHotkeyCapture();
        return;
      }
      hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.cancelled");
      stopHotkeyCapture();
      return;
    }
    const combo = keyboardEventToHotkey(event, target === "summon");
    if (!combo) {
      hotkeyCaptureHint.value = target === "summon"
        ? t("quickSetup.hotkeyHints.summonNeedsModifier")
        : t("quickSetup.hotkeyHints.unrecognized");
      return;
    }
    if (target === "summon") config.hotkey = combo;
    else config.recordHotkey = combo;
    hotkeyCaptureHint.value = t("quickSetup.hotkeyHints.recorded", { combo });
    stopHotkeyCapture();
  };
  window.addEventListener("keydown", hotkeyCaptureHandler, true);
}

function stopHotkeyCapture() {
  if (hotkeyCaptureHandler) {
    window.removeEventListener("keydown", hotkeyCaptureHandler, true);
    hotkeyCaptureHandler = null;
  }
  hotkeyCaptureTarget.value = null;
}

function keyboardEventToHotkey(event: KeyboardEvent, requireModifier: boolean): string {
  const modifiers: string[] = [];
  if (event.ctrlKey) modifiers.push("Ctrl");
  if (event.altKey) modifiers.push("Alt");
  if (event.shiftKey) modifiers.push("Shift");
  if (event.metaKey) modifiers.push("Meta");
  const raw = event.key;
  const lower = raw.toLowerCase();
  const modifierOnly: Record<string, string> = { control: "Ctrl", alt: "Alt", shift: "Shift", meta: "Meta" };
  if (modifierOnly[lower]) return requireModifier ? "" : modifierOnly[lower];
  const main = lower === " " ? "Space" : lower === "`" ? "·" : raw.length === 1 ? raw.toUpperCase() : raw;
  if (requireModifier && modifiers.length === 0) return "";
  return [...modifiers, main].join("+");
}
</script>
