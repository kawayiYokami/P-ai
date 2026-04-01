<template>
  <div v-if="props.selectedApiConfig" class="grid gap-3">
    <!-- 能力分组与LLM配置 -->
    <div class="card border border-base-300 bg-base-100">
      <div class="card-body p-4">
        <div class="flex flex-col gap-3">
          <div class="flex w-full flex-col gap-1">
            <div class="flex items-center justify-between py-1">
              <span class="text-sm font-medium">能力分组</span>
            </div>
            <div class="join w-full">
              <button
                v-for="tab in capabilityTabs"
                :key="tab.id"
                class="btn btn-sm join-item flex-1"
                :class="activeCapability === tab.id ? 'btn-primary' : 'bg-base-200'"
                @click="switchCapabilityTab(tab.id)"
              >
                {{ tab.label }}
              </button>
            </div>
          </div>

          <label class="flex w-full flex-col gap-1">
            <div class="flex items-center justify-between py-1">
              <span class="text-sm font-medium">LLM配置</span>
            </div>
            <div class="flex w-full min-w-0 gap-1">
              <select
                :value="activeCapabilitySelectedId"
                class="select select-bordered select-sm min-w-0 flex-1"
                @change="switchCapabilityConfig"
              >
                <option v-for="a in capabilityScopedConfigsWithFallback" :key="a.id" :value="a.id">{{ a.name }}</option>
              </select>
              <button class="btn btn-sm btn-square bg-base-200" :title="t('config.api.addConfig')" @click="handleAddApiConfig">
                <Plus class="h-3.5 w-3.5" />
              </button>
              <button class="btn btn-sm btn-square bg-base-200" :title="t('config.api.removeConfig')" :disabled="props.config.apiConfigs.length <= 1" @click="$emit('removeSelectedApiConfig')">
                <Trash2 class="h-3.5 w-3.5" />
              </button>
              <button
                class="btn btn-sm btn-square"
                :class="props.configDirty ? 'btn-primary' : 'bg-base-200'"
                :disabled="!props.configDirty || props.savingConfig"
                :title="props.savingConfig ? t('config.api.saving') : props.configDirty ? t('config.api.saveConfig') : t('config.api.saved')"
                @click="handleSaveApiConfig"
              >
                <Save v-if="!props.savingConfig" class="h-3.5 w-3.5" />
                <span v-else class="loading loading-spinner loading-sm"></span>
              </button>
            </div>
          </label>
        </div>
      </div>
    </div>

    <!-- API 配置详情 -->
    <div class="card border border-base-300 bg-base-100">
      <div class="card-body p-4">
        <div class="flex flex-col gap-3">
          <div class="flex w-full items-center gap-2">
            <span class="w-24 shrink-0 text-sm font-medium">{{ t("config.api.configName") }}</span>
            <input v-model="props.selectedApiConfig.name" class="input input-bordered input-sm min-w-0 flex-1" :placeholder="t('config.api.configName')" />
          </div>

          <div class="flex w-full items-center gap-2">
            <span class="w-24 shrink-0 text-sm font-medium">{{ t("config.api.requestFormat") }}</span>
            <select v-model="props.selectedApiConfig.requestFormat" class="select select-bordered select-sm min-w-0 flex-1">
              <option v-for="item in currentProtocolOptions" :key="item.value" :value="item.value">{{ item.label }}</option>
            </select>
          </div>

          <div class="flex w-full flex-col gap-1">
            <div class="flex w-full items-center gap-2">
              <span class="w-24 shrink-0 text-sm font-medium">{{ t("config.api.baseUrl") }}</span>
              <div class="flex min-w-0 flex-1 gap-1">
                <input v-model="props.selectedApiConfig.baseUrl" class="input input-bordered input-sm min-w-0 flex-1" :placeholder="props.baseUrlReference" />
                <button class="btn btn-sm btn-square bg-base-200" :title="t('config.api.linkHelper')" @click="baseUrlHelperOpen = !baseUrlHelperOpen">
                  <WandSparkles class="h-3.5 w-3.5" />
                </button>
              </div>
            </div>
            <div v-if="baseUrlHelperOpen" class="mt-1 rounded-box border border-base-300 bg-base-100 p-2">
              <div class="mb-2 text-sm opacity-70">{{ t("config.api.linkHelperHint") }}</div>
              <div class="flex flex-wrap gap-1">
                <div v-for="preset in filteredProviderPresets" :key="preset.id" class="join rounded-btn shadow-sm">
                  <button
                    class="btn btn-sm join-item relative overflow-visible"
                    :class="selectedProviderId === preset.id ? 'btn-primary' : 'bg-base-200'"
                    @click="selectedProviderId = preset.id"
                  >
                    <span
                      v-if="preset.hasFreeQuota"
                      class="badge badge-secondary badge-sm absolute -top-2 left-1 text-[9px] leading-none"
                    >
                      {{ t("config.api.freeBadge") }}
                    </span>
                    <span>{{ preset.name }}</span>
                  </button>
                  <button
                    class="btn btn-sm btn-neutral join-item"
                    :title="t('config.api.openProviderSite')"
                    @click="openProviderSite(preset)"
                  >
                    <ExternalLink class="h-3 w-3" />
                  </button>
                </div>
              </div>
              <label class="mt-2 flex w-full flex-col gap-1">
                <div class="flex items-center justify-between py-0">
                  <span class="text-sm">{{ t("config.api.generatedLink") }}</span>
                </div>
                <div class="flex w-full min-w-0 gap-1">
                  <input :value="generatedBaseUrl" class="input input-bordered input-sm min-w-0 flex-1" readonly />
                  <button class="btn btn-sm btn-primary" :disabled="!generatedBaseUrl" @click="applyGeneratedBaseUrl">
                    <Link class="h-3 w-3" />
                    <span>{{ t("config.api.fillBaseUrl") }}</span>
                  </button>
                </div>
              </label>
            </div>
          </div>

          <div class="flex w-full items-center gap-2">
            <span class="w-24 shrink-0 text-sm font-medium">API Key</span>
            <div class="flex min-w-0 flex-1 gap-1">
              <input
                v-model="props.selectedApiConfig.apiKey"
                :type="showApiKey ? 'text' : 'password'"
                class="input input-bordered input-sm min-w-0 flex-1"
                placeholder="api key"
              />
              <button
                class="btn btn-sm btn-square bg-base-200"
                type="button"
                :title="showApiKey ? t('config.api.hideApiKey') : t('config.api.showApiKey')"
                @click="showApiKey = !showApiKey"
              >
                <EyeOff v-if="showApiKey" class="h-3.5 w-3.5" />
                <Eye v-else class="h-3.5 w-3.5" />
              </button>
            </div>
          </div>

          <div class="flex w-full flex-col gap-1">
            <div class="flex w-full items-center gap-2">
              <span class="w-24 shrink-0 text-sm font-medium">{{ t("config.api.model") }}</span>
              <div class="flex min-w-0 flex-1 gap-1">
                <input v-model="props.selectedApiConfig.model" class="input input-bordered input-sm min-w-0 flex-1" placeholder="model" />
                <div v-if="isTextMode" ref="modelPickerRef" class="dropdown dropdown-end" :class="{ 'dropdown-open': modelPickerOpen }">
                  <button
                    type="button"
                    class="btn btn-sm btn-square"
                    :class="props.modelRefreshOk ? 'btn-primary' : 'bg-base-200'"
                    :disabled="props.modelOptions.length === 0"
                    :title="t('config.api.pickModel')"
                    @click.stop="toggleModelPicker"
                  >
                    <ChevronsUpDown class="h-3.5 w-3.5" />
                  </button>
                  <div v-if="modelPickerOpen" class="dropdown-content z-1 flex max-h-72 min-w-70 flex-col overflow-hidden rounded-box bg-base-100 shadow" @click.stop>
                    <input
                      v-model="modelSearch"
                      type="text"
                      :placeholder="t('config.api.searchModel')"
                      class="input input-bordered input-sm h-8 min-h-8 w-full rounded-none border-x-0 border-t-0 focus:outline-none"
                      @click.stop
                      @keydown.esc.stop.prevent="closeModelPicker()"
                    />
                    <ul class="menu flex-1 flex-col flex-nowrap overflow-auto p-1">
                      <li v-for="modelName in filteredModels" :key="modelName">
                        <button class="wrap-break-word whitespace-normal text-left" @click="selectModel(modelName)">{{ modelName }}</button>
                      </li>
                      <li v-if="filteredModels.length === 0" class="py-2 text-center text-sm opacity-50">{{ t("config.api.noModelFound") }}</li>
                    </ul>
                  </div>
                </div>
                <button v-if="isTextMode" class="btn btn-sm btn-square bg-base-200" :class="{ loading: props.refreshingModels }" :disabled="props.refreshingModels" :title="t('config.api.refreshModels')" @click="$emit('refreshModels')">
                  <RefreshCw class="h-3.5 w-3.5" />
                </button>
              </div>
            </div>
            <div class="flex w-full items-center justify-between pl-26">
              <span class="min-h-4 text-[11px] text-error">{{ props.modelRefreshError || " " }}</span>
            </div>
            <div v-if="modelControlsLocked" class="pl-26 text-[11px] text-warning">
              {{ t("config.api.saveModelFirstHint") }}
            </div>
          </div>

          <div v-if="isTextMode" class="flex w-full items-center gap-2">
            <span class="w-24 shrink-0 text-sm font-medium">{{ t("config.api.temperature") }}</span>
            <div class="min-w-0 flex-1">
              <div class="mb-1 flex items-center justify-end">
                <span class="text-sm opacity-70">{{ Number(props.selectedApiConfig.temperature ?? 1).toFixed(1) }}</span>
              </div>
              <input
                v-model.number="props.selectedApiConfig.temperature"
                :disabled="modelControlsLocked || !props.selectedApiConfig.customTemperatureEnabled"
                type="range"
                min="0"
                max="2"
                step="0.1"
                class="range range-sm w-full"
              />
              <div class="mt-1 flex justify-between text-[10px] opacity-60">
                <span>0.0</span>
                <span>1.0</span>
                <span>2.0</span>
              </div>
            </div>
            <input
              v-model="props.selectedApiConfig.customTemperatureEnabled"
              :disabled="modelControlsLocked"
              type="checkbox"
              class="checkbox checkbox-sm shrink-0"
            />
          </div>

          <div v-if="isTextMode" class="flex w-full items-center gap-2">
            <span class="w-24 shrink-0 text-sm font-medium">{{ t("config.api.contextWindow") }}</span>
            <div class="min-w-0 flex-1">
              <div class="mb-1 flex items-center justify-end">
                <span class="text-sm opacity-70">{{ Math.round(Number(props.selectedApiConfig.contextWindowTokens ?? 128000)) }}</span>
              </div>
              <input
                v-model.number="props.selectedApiConfig.contextWindowTokens"
                :disabled="modelControlsLocked"
                type="range"
                min="16000"
                :max="contextWindowMax"
                step="1000"
                class="range range-sm w-full"
              />
              <div class="mt-1 flex justify-between text-[10px] opacity-60">
                <span>16K</span>
                <span>{{ contextWindowMidLabel }}</span>
                <span>{{ contextWindowMaxLabel }}</span>
              </div>
            </div>
          </div>

          <div v-if="isTextMode" class="flex w-full items-center gap-2">
            <span class="w-24 shrink-0 text-sm font-medium">{{ t("config.api.maxOutputTokens") }}</span>
            <div class="min-w-0 flex-1">
              <div class="mb-1 flex items-center justify-end">
                <span class="text-sm opacity-70">{{ selectedMaxOutputTokens }}</span>
              </div>
              <input
                v-model.number="props.selectedApiConfig.maxOutputTokens"
                :disabled="modelControlsLocked || !maxOutputTokensEnabled"
                type="range"
                min="256"
                :max="maxOutputTokensMax"
                step="256"
                class="range range-sm w-full"
              />
              <div class="mt-1 flex justify-between text-[10px] opacity-60">
                <span>256</span>
                <span>{{ maxOutputTokensMidLabel }}</span>
                <span>{{ maxOutputTokensMaxLabel }}</span>
              </div>
            </div>
            <input
              v-model="props.selectedApiConfig.customMaxOutputTokensEnabled"
              :disabled="modelControlsLocked || maxOutputTokensToggleLocked"
              type="checkbox"
              class="checkbox checkbox-sm shrink-0"
            />
          </div>
        </div>
      </div>
    </div>

    <!-- 能力配置 -->
    <div v-if="isTextMode" class="card border border-base-300 bg-base-100">
      <div class="card-body p-4">
        <h3 class="card-title mb-3 text-base">{{ t("config.api.capabilities") }}</h3>
        <div class="flex w-full gap-2">
          <label class="flex flex-1 cursor-pointer items-center justify-between rounded-md border border-base-300 bg-base-200 px-2 py-1">
            <span class="text-sm">{{ t("config.api.capImage") }}</span>
            <input v-model="props.selectedApiConfig.enableImage" :disabled="modelControlsLocked || !imageToggleAvailable" type="checkbox" class="toggle toggle-sm" />
          </label>
          <label class="flex flex-1 cursor-pointer items-center justify-between rounded-md border border-base-300 bg-base-200 px-2 py-1">
            <span class="text-sm">{{ t("config.api.capTools") }}</span>
            <input v-model="props.selectedApiConfig.enableTools" :disabled="modelControlsLocked || !toolsToggleAvailable" type="checkbox" class="toggle toggle-sm" />
          </label>
        </div>
        <div v-if="!imageToggleAvailable || !toolsToggleAvailable" class="mt-2 text-[11px] opacity-70">
          {{ t("config.api.capabilityLimitedByModelHint") }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronsUpDown, ExternalLink, Eye, EyeOff, Link, Plus, RefreshCw, Save, Trash2, WandSparkles } from "lucide-vue-next";
import type { ApiConfigItem, ApiRequestFormat, AppConfig } from "../../../../types/app";
import { invokeTauri } from "../../../../services/tauri-api";

type ApiCapability = "text" | "voice" | "embedding";

type ProviderPreset = {
  id: string;
  name: string;
  urls: Partial<Record<ApiRequestFormat, string>>;
  docsUrl: string;
  hasFreeQuota?: boolean;
};

type ProtocolOption = { value: ApiRequestFormat; label: string };

const props = defineProps<{
  config: AppConfig;
  selectedApiConfig: ApiConfigItem | null;
  baseUrlReference: string;
  refreshingModels: boolean;
  modelOptions: string[];
  modelRefreshOk: boolean;
  modelRefreshError: string;
  configDirty: boolean;
  savingConfig: boolean;
  saveApiConfigAction: () => Promise<boolean> | boolean;
}>();

const emit = defineEmits<{
  (e: "saveApiConfig"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "refreshModels"): void;
  (e: "configSwitched"): void;
}>();

const { t } = useI18n();
const CONTEXT_WINDOW_DEFAULT_MAX = 200000;
const CONTEXT_WINDOW_HARD_MAX = 2000000;
const LEGACY_DEFAULT_MAX_OUTPUT_TOKENS = 4096;
const baseUrlHelperOpen = ref(false);
const showApiKey = ref(false);
const selectedProviderId = ref("openai-official");
const modelSearch = ref("");
const modelPickerOpen = ref(false);
const modelPickerRef = ref<HTMLElement | null>(null);
const activeCapability = ref<ApiCapability>("text");
const creatingCapabilityDefault = ref(false);
const applyingModelMetadata = ref(false);
const savedModelSignatureByApiId = ref<Record<string, string>>({});
const metadataModelSignatureByApiId = ref<Record<string, string>>({});
const modelCapabilityByApiId = ref<Record<string, {
  contextWindowMax?: number;
  maxOutputTokensMax?: number;
  enableImage?: boolean;
  enableTools?: boolean;
  enableAudio?: boolean;
}>>({});

type FetchModelMetadataOutput = {
  found: boolean;
  matchedModelId?: string | null;
  contextWindowTokens?: number | null;
  maxOutputTokens?: number | null;
  enableImage?: boolean | null;
  enableTools?: boolean | null;
  enableAudio?: boolean | null;
};

const capabilityTabs: Array<{ id: ApiCapability; label: string }> = [
  { id: "text", label: "文本" },
  { id: "voice", label: "语音（朗读/转写）" },
  { id: "embedding", label: "嵌入（嵌入/重排）" },
];

const protocolOptionsByCapability: Record<ApiCapability, ProtocolOption[]> = {
  text: [
    { value: "openai", label: "OpenAI Compatible" },
    { value: "openai_responses", label: "OpenAI Responses" },
    { value: "gemini", label: "Google Gemini" },
    { value: "anthropic", label: "Anthropic" },
  ],
  voice: [
    { value: "openai_stt", label: "OpenAI STT" },
    { value: "openai_tts", label: "OpenAI TTS" },
  ],
  embedding: [
    { value: "openai_embedding", label: "OpenAI Embedding" },
    { value: "gemini_embedding", label: "Gemini Embedding" },
    { value: "openai_rerank", label: "OpenAI Rerank" },
  ],
};

const capabilityDefaultProtocol: Record<ApiCapability, ApiRequestFormat> = {
  text: "openai",
  voice: "openai_stt",
  embedding: "openai_embedding",
};

function capabilityFromConfig(config: ApiConfigItem): ApiCapability {
  const format = config.requestFormat;
  const normalized = String(format || "").trim().toLowerCase();
  if (
    normalized === "openai_stt"
    || normalized === "openai_tts"
    || normalized === "openai-stt"
    || normalized === "openai-tts"
    || normalized === "stt"
    || normalized === "tts"
  ) {
    return "voice";
  }
  if (
    normalized === "openai_embedding"
    || normalized === "gemini_embedding"
    || normalized === "openai_rerank"
    || normalized === "openai-embedding"
    || normalized === "openai-rerank"
    || normalized === "embedding"
    || normalized === "rerank"
  ) {
    return "embedding";
  }
  return "text";
}

const currentProtocol = computed<ApiRequestFormat>(() => props.selectedApiConfig?.requestFormat || "openai");
const isTextMode = computed(() => activeCapability.value === "text");
const selectedApiId = computed(() => String(props.selectedApiConfig?.id || "").trim());
const selectedModelSignature = computed(() => {
  const cfg = props.selectedApiConfig;
  if (!cfg) return "";
  return `${String(cfg.model || "").trim()}`;
});
const modelChangedButUnsaved = computed(() => {
  const id = selectedApiId.value;
  if (!id || !isTextMode.value) return false;
  const savedSignature = savedModelSignatureByApiId.value[id];
  if (savedSignature === undefined) return false;
  return selectedModelSignature.value !== savedSignature;
});
const modelControlsLocked = computed(() => isTextMode.value && (modelChangedButUnsaved.value || applyingModelMetadata.value));
const selectedModelCapability = computed(() => {
  const id = selectedApiId.value;
  if (!id) return null;
  return modelCapabilityByApiId.value[id] ?? null;
});
const contextWindowMax = computed(() => {
  const raw = Number(selectedModelCapability.value?.contextWindowMax ?? CONTEXT_WINDOW_DEFAULT_MAX);
  if (!Number.isFinite(raw)) return CONTEXT_WINDOW_DEFAULT_MAX;
  return Math.max(16000, Math.min(CONTEXT_WINDOW_HARD_MAX, Math.round(raw)));
});
const maxOutputTokensMax = computed(() => {
  const raw = Number(selectedModelCapability.value?.maxOutputTokensMax ?? 32768);
  if (!Number.isFinite(raw)) return 32768;
  return Math.max(256, Math.min(32768, Math.round(raw)));
});
const contextWindowMaxLabel = computed(() => `${Math.round(contextWindowMax.value / 1000)}K`);
const contextWindowMidLabel = computed(() => formatTokenLabel(Math.round(((16000 + contextWindowMax.value) / 2) / 1000) * 1000));
const maxOutputTokensMaxLabel = computed(() => `${Math.round(maxOutputTokensMax.value / 1000)}K`);
const maxOutputTokensMidLabel = computed(() => formatTokenLabel(Math.round(((256 + maxOutputTokensMax.value) / 2) / 256) * 256));
const maxOutputTokensToggleLocked = computed(() => currentProtocol.value === "anthropic");
const maxOutputTokensEnabled = computed(() => maxOutputTokensToggleLocked.value || !!props.selectedApiConfig?.customMaxOutputTokensEnabled);
const selectedMaxOutputTokens = computed(() => {
  const raw = Number(props.selectedApiConfig?.maxOutputTokens);
  if (!Number.isFinite(raw)) return maxOutputTokensMax.value;
  return Math.max(256, Math.min(maxOutputTokensMax.value, Math.round(raw)));
});
const selectedModelMetadataReady = computed(() => {
  const id = selectedApiId.value;
  if (!id) return false;
  return metadataModelSignatureByApiId.value[id] === selectedModelSignature.value;
});
const imageToggleAvailable = computed(() => {
  const value = selectedModelCapability.value?.enableImage;
  return value === undefined ? true : !!value;
});
const toolsToggleAvailable = computed(() => {
  const value = selectedModelCapability.value?.enableTools;
  return value === undefined ? true : !!value;
});
const currentProtocolOptions = computed(() => {
  return protocolOptionsByCapability[activeCapability.value];
});
const capabilityScopedConfigs = computed(() =>
  props.config.apiConfigs.filter(
    (cfg) => capabilityFromConfig(cfg) === activeCapability.value,
  ),
);
const capabilityScopedConfigsWithFallback = computed(() => {
  const items = [...capabilityScopedConfigs.value];
  if (activeCapability.value !== "voice") return items;
  const sttId = String(props.config.sttApiConfigId || "").trim();
  if (!sttId) return items;
  if (items.some((item) => item.id === sttId)) return items;
  const sttConfig = props.config.apiConfigs.find((item) => item.id === sttId);
  if (!sttConfig) return items;
  return [sttConfig, ...items];
});
const activeCapabilitySelectedId = computed(() => {
  const selected = props.config.selectedApiConfigId;
  if (capabilityScopedConfigsWithFallback.value.some((item) => item.id === selected)) {
    return selected;
  }
  return capabilityScopedConfigsWithFallback.value[0]?.id ?? "";
});

onMounted(() => {
  savedModelSignatureByApiId.value = Object.fromEntries(
    props.config.apiConfigs.map((item) => [item.id, String(item.model || "").trim()]),
  );
  document.addEventListener("mousedown", handleDocumentMouseDown);
  const selected = props.selectedApiConfig;
  if (!selected) return;
  activeCapability.value = capabilityFromConfig(selected);
  ensureCapabilityConfig(activeCapability.value);
});

onBeforeUnmount(() => {
  document.removeEventListener("mousedown", handleDocumentMouseDown);
});

watch(
  () => props.config.apiConfigs,
  (list) => {
    const snapshot = { ...savedModelSignatureByApiId.value };
    for (const item of list) {
      if (!(item.id in snapshot)) {
        snapshot[item.id] = String(item.model || "").trim();
      }
    }
    savedModelSignatureByApiId.value = snapshot;
  },
  { deep: true },
);

watch(activeCapability, (capability) => {
  closeModelPicker(false);
  if (!ensureCapabilityConfig(capability)) {
    return;
  }
  const selected = props.config.selectedApiConfigId;
  if (capabilityScopedConfigsWithFallback.value.some((item) => item.id === selected)) return;
  const nextId = capabilityScopedConfigsWithFallback.value[0]?.id;
  if (!nextId) return;
  props.config.selectedApiConfigId = nextId;
});

watch(
  [activeCapability, capabilityScopedConfigs],
  () => {
    closeModelPicker(false);
    if (!ensureCapabilityConfig(activeCapability.value)) {
      return;
    }
    const selected = props.config.selectedApiConfigId;
    if (capabilityScopedConfigsWithFallback.value.some((item) => item.id === selected)) return;
    const nextId = capabilityScopedConfigsWithFallback.value[0]?.id;
    if (!nextId) return;
    props.config.selectedApiConfigId = nextId;
  },
  { immediate: true },
);

watch(
  () => props.selectedApiConfig,
  (cfg) => {
    if (!cfg) return;
    if (capabilityFromConfig(cfg) === "text" && !cfg.enableText) {
      cfg.enableText = true;
    }
    cfg.customMaxOutputTokensEnabled = cfg.requestFormat === "anthropic"
      ? true
      : !!cfg.customMaxOutputTokensEnabled;
  },
  { immediate: true, deep: true },
);

watch(
  () => props.selectedApiConfig?.contextWindowTokens,
  (value) => {
    const cfg = props.selectedApiConfig;
    if (!cfg || !isTextMode.value) return;
    const next = Math.round(Number(value ?? 128000));
    const clamped = Math.max(16000, Math.min(contextWindowMax.value, next));
    if (next !== clamped) {
      cfg.contextWindowTokens = clamped;
    }
  },
);

watch(
  () => props.selectedApiConfig?.maxOutputTokens,
  (value) => {
    const cfg = props.selectedApiConfig;
    if (!cfg || !isTextMode.value) return;
    const next = Math.round(Number(value ?? maxOutputTokensMax.value));
    const clamped = Math.max(256, Math.min(maxOutputTokensMax.value, next));
    if (next !== clamped) {
      cfg.maxOutputTokens = clamped;
    }
  },
);

watch(
  [selectedApiId, selectedModelSignature, activeCapability],
  () => {
    void syncSelectedModelMetadataIfNeeded();
  },
  { immediate: true },
);

const filteredModels = computed(() => {
  const search = modelSearch.value.trim().toLowerCase();
  if (!search) return props.modelOptions;
  return props.modelOptions.filter((m) => m.toLowerCase().includes(search));
});

function selectModel(modelName: string) {
  if (props.selectedApiConfig) {
    props.selectedApiConfig.model = modelName;
  }
  closeModelPicker();
}

function formatTokenLabel(value: number): string {
  if (value >= 1000) {
    return `${Math.round(value / 1000)}K`;
  }
  return String(Math.round(value));
}

function closeModelPicker(resetSearch = true) {
  modelPickerOpen.value = false;
  if (resetSearch) {
    modelSearch.value = "";
  }
}

function toggleModelPicker() {
  if (props.modelOptions.length === 0) return;
  if (modelPickerOpen.value) {
    closeModelPicker();
    return;
  }
  modelPickerOpen.value = true;
}

function handleDocumentMouseDown(event: MouseEvent) {
  const root = modelPickerRef.value;
  const target = event.target;
  if (!root || !(target instanceof Node)) return;
  if (!root.contains(target)) {
    closeModelPicker(false);
  }
}

async function applySavedModelMetadata(target: ApiConfigItem) {
  const model = String(target.model || "").trim();
  if (!model) return;
  const hadMetadataForCurrentModel = metadataModelSignatureByApiId.value[target.id] === model;
  const metadata = await invokeTauri<FetchModelMetadataOutput>("fetch_model_metadata", {
    input: {
      requestFormat: target.requestFormat,
      model,
    },
  });
  if (!metadata?.found) return;
  const rawContextMax = Number(metadata.contextWindowTokens ?? target.contextWindowTokens ?? 128000);
  const rawOutputMax = Number(metadata.maxOutputTokens ?? target.maxOutputTokens ?? 4096);
  const contextMax = Math.max(16000, Math.min(CONTEXT_WINDOW_HARD_MAX, Math.round(rawContextMax)));
  const outputMax = Math.max(256, Math.min(32768, Math.round(rawOutputMax)));
  modelCapabilityByApiId.value = {
    ...modelCapabilityByApiId.value,
    [target.id]: {
      contextWindowMax: contextMax,
      maxOutputTokensMax: outputMax,
      enableImage: typeof metadata.enableImage === "boolean" ? metadata.enableImage : undefined,
      enableTools: typeof metadata.enableTools === "boolean" ? metadata.enableTools : undefined,
      enableAudio: typeof metadata.enableAudio === "boolean" ? metadata.enableAudio : undefined,
    },
  };
  metadataModelSignatureByApiId.value = {
    ...metadataModelSignatureByApiId.value,
    [target.id]: model,
  };
  const currentContext = Math.round(Number(target.contextWindowTokens ?? contextMax));
  if (!Number.isFinite(currentContext) || currentContext < 16000 || currentContext > contextMax) {
    target.contextWindowTokens = contextMax;
  }
  const rawCurrentOutput = Number(target.maxOutputTokens);
  const currentOutput = Math.round(rawCurrentOutput);
  const shouldAdoptMetadataDefault = !Number.isFinite(rawCurrentOutput)
    || (!hadMetadataForCurrentModel && currentOutput === LEGACY_DEFAULT_MAX_OUTPUT_TOKENS);
  if (shouldAdoptMetadataDefault) {
    target.maxOutputTokens = outputMax;
  } else if (currentOutput < 256 || currentOutput > outputMax) {
    target.maxOutputTokens = Math.max(256, Math.min(outputMax, currentOutput));
  }
  if (typeof metadata.enableImage === "boolean" && !metadata.enableImage) {
    target.enableImage = false;
  }
  if (typeof metadata.enableTools === "boolean" && !metadata.enableTools) {
    target.enableTools = false;
  }
}

async function syncSelectedModelMetadataIfNeeded() {
  const target = props.selectedApiConfig;
  if (!target || capabilityFromConfig(target) !== "text") return;
  if (applyingModelMetadata.value || modelChangedButUnsaved.value || selectedModelMetadataReady.value) return;
  if (!String(target.id || "").trim() || !String(target.model || "").trim()) return;
  applyingModelMetadata.value = true;
  try {
    await applySavedModelMetadata(target);
  } catch (error) {
    console.warn("[API] fetch_model_metadata failed:", error);
  } finally {
    applyingModelMetadata.value = false;
  }
}

async function handleSaveApiConfig() {
  const target = props.selectedApiConfig;
  const saved = await Promise.resolve(props.saveApiConfigAction());
  if (!saved || !target) return;
  const signature = String(target.model || "").trim();
  savedModelSignatureByApiId.value = {
    ...savedModelSignatureByApiId.value,
    [target.id]: signature,
  };
  if (!isTextMode.value) return;
  applyingModelMetadata.value = true;
  try {
    await applySavedModelMetadata(target);
  } catch (error) {
    console.warn("[API] fetch_model_metadata failed:", error);
  } finally {
    applyingModelMetadata.value = false;
  }
}

const providerPresets: ProviderPreset[] = [
  { id: "openai-official", name: "OpenAI", urls: { openai: "https://api.openai.com/v1", openai_responses: "https://api.openai.com/v1", openai_stt: "https://api.openai.com/v1", openai_tts: "https://api.openai.com/v1/audio/speech", openai_embedding: "https://api.openai.com/v1", openai_rerank: "https://api.openai.com/v1" }, docsUrl: "https://platform.openai.com/docs/overview" },
  { id: "anthropic-official", name: "Anthropic", urls: { anthropic: "https://api.anthropic.com" }, docsUrl: "https://docs.anthropic.com/en/api/overview" },
  { id: "google-gemini", name: "Google Gemini", urls: { gemini: "https://generativelanguage.googleapis.com", gemini_embedding: "https://generativelanguage.googleapis.com" }, docsUrl: "https://ai.google.dev/gemini-api/docs", hasFreeQuota: true },
  { id: "deepseek", name: "DeepSeek", urls: { openai: "https://api.deepseek.com/v1", openai_responses: "https://api.deepseek.com/v1" }, docsUrl: "https://api-docs.deepseek.com/" },
  { id: "moonshot-kimi", name: "Moonshot/Kimi", urls: { openai: "https://api.moonshot.cn/v1", openai_responses: "https://api.moonshot.cn/v1" }, docsUrl: "https://platform.moonshot.cn/docs/api-reference" },
  { id: "zhipu-glm", name: "Zhipu GLM", urls: { openai: "https://open.bigmodel.cn/api/paas/v4", openai_responses: "https://open.bigmodel.cn/api/paas/v4" }, docsUrl: "https://open.bigmodel.cn/dev/api", hasFreeQuota: true },
  { id: "minimax", name: "MiniMax", urls: { openai: "https://api.minimax.chat/v1", openai_responses: "https://api.minimax.chat/v1" }, docsUrl: "https://www.minimax.io/platform/document" },
  { id: "siliconflow", name: "SiliconFlow", urls: { openai: "https://api.siliconflow.cn/v1", openai_responses: "https://api.siliconflow.cn/v1", openai_stt: "https://api.siliconflow.cn/v1", openai_embedding: "https://api.siliconflow.cn/v1", openai_rerank: "https://api.siliconflow.cn/v1" }, docsUrl: "https://docs.siliconflow.cn/", hasFreeQuota: true },
  { id: "iflow", name: "iFlow", urls: { openai: "https://apis.iflow.cn/v1", openai_responses: "https://apis.iflow.cn/v1" }, docsUrl: "https://platform.iflow.cn/models", hasFreeQuota: true },
  { id: "modelscope", name: "ModelScope", urls: { openai: "https://api-inference.modelscope.cn/v1", openai_responses: "https://api-inference.modelscope.cn/v1" }, docsUrl: "https://modelscope.cn/models", hasFreeQuota: true },
  { id: "nvidia-nim", name: "NVIDIA NIM", urls: { openai: "https://integrate.api.nvidia.com/v1", openai_responses: "https://integrate.api.nvidia.com/v1" }, docsUrl: "https://docs.api.nvidia.com/nim/", hasFreeQuota: true },
  { id: "openrouter", name: "OpenRouter", urls: { openai: "https://openrouter.ai/api/v1", openai_responses: "https://openrouter.ai/api/v1" }, docsUrl: "https://openrouter.ai/docs/api-reference/overview", hasFreeQuota: true },
  { id: "cloudflare-gateway", name: "Cloudflare Gateway", urls: { openai: "https://gateway.ai.cloudflare.com/v1/{account_id}/{gateway_id}/{provider}", openai_responses: "https://gateway.ai.cloudflare.com/v1/{account_id}/{gateway_id}/{provider}" }, docsUrl: "https://developers.cloudflare.com/ai-gateway/" },
  { id: "ollama-local", name: "Ollama (Local)", urls: { openai: "http://localhost:11434/v1", openai_responses: "http://localhost:11434/v1" }, docsUrl: "https://github.com/ollama/ollama/blob/main/docs/openai.md" },
];

const filteredProviderPresets = computed(() => {
  const sortFreeFirst = (list: ProviderPreset[]) =>
    [...list].sort((a, b) => Number(Boolean(b.hasFreeQuota)) - Number(Boolean(a.hasFreeQuota)));
  return sortFreeFirst(providerPresets.filter((p) => Boolean(p.urls[currentProtocol.value])));
});

const selectedProvider = computed(() => providerPresets.find((p) => p.id === selectedProviderId.value) ?? providerPresets[0]);
const generatedBaseUrl = computed(() => {
  const urls = selectedProvider.value.urls;
  return urls[currentProtocol.value] || urls.openai || "";
});

watch(
  filteredProviderPresets,
  (list) => {
    if (!list.length) return;
    if (!list.some((item) => item.id === selectedProviderId.value)) {
      selectedProviderId.value = list[0].id;
    }
  },
  { immediate: true },
);

function applyGeneratedBaseUrl() {
  if (!props.selectedApiConfig || !generatedBaseUrl.value) return;
  props.selectedApiConfig.baseUrl = generatedBaseUrl.value;
  baseUrlHelperOpen.value = false;
}

function switchCapabilityConfig(event: Event) {
  const id = (event.target as HTMLSelectElement).value;
  if (!id) return;
  props.config.selectedApiConfigId = id;
}

function switchCapabilityTab(capability: ApiCapability) {
  activeCapability.value = capability;
  if (!ensureCapabilityConfig(capability)) {
    return;
  }
  const selected = props.config.selectedApiConfigId;
  if (capabilityScopedConfigsWithFallback.value.some((item) => item.id === selected)) return;
  const nextId = capabilityScopedConfigsWithFallback.value[0]?.id;
  if (!nextId) return;
  props.config.selectedApiConfigId = nextId;
}

function handleAddApiConfig() {
  createConfigForCapability(activeCapability.value);
}

function ensureCapabilityConfig(capability: ApiCapability): boolean {
  if (capabilityScopedConfigsWithFallback.value.length > 0) return true;
  if (creatingCapabilityDefault.value) return false;
  createConfigForCapability(capability);
  return false;
}

function createConfigForCapability(capability: ApiCapability) {
  if (creatingCapabilityDefault.value) return;
  creatingCapabilityDefault.value = true;
  const prevIds = new Set(props.config.apiConfigs.map((item) => item.id));
  const defaultFormat = capabilityDefaultProtocol[capability];
  const wantedTextMode = capability === "text";
  const wantedVoiceMode = capability === "voice";
  emit("addApiConfig");
  queueMicrotask(() => {
    const created = props.config.apiConfigs.find((item) => !prevIds.has(item.id));
    if (!created) return;
    created.requestFormat = defaultFormat;
    created.enableText = wantedTextMode;
    created.enableImage = wantedTextMode ? created.enableImage : false;
    created.enableTools = wantedTextMode ? created.enableTools : false;
    if (wantedVoiceMode || capability === "embedding") {
      created.enableText = false;
    }
    props.config.selectedApiConfigId = created.id;
    creatingCapabilityDefault.value = false;
  });
  queueMicrotask(() => {
    if (creatingCapabilityDefault.value) {
      creatingCapabilityDefault.value = false;
    }
  });
}

async function openProviderSite(preset: ProviderPreset) {
  if (!preset.docsUrl) return;
  try {
    await invokeTauri("open_external_url", { url: preset.docsUrl });
  } catch (error) {
    console.warn("[API] open provider docs failed:", error);
  }
}
</script>
