<template>
  <div v-if="props.selectedApiConfig" class="grid w-full gap-2">
    <div class="flex w-full flex-col gap-1">
      <div class="flex items-center justify-between py-1"><span class="text-sm font-medium">能力分组</span></div>
      <div class="join w-full">
        <button
          v-for="tab in capabilityTabs"
          :key="tab.id"
          class="btn btn-sm join-item flex-1"
          :class="activeCapability === tab.id ? 'btn-primary' : 'bg-base-100'"
          @click="switchCapabilityTab(tab.id)"
        >
          {{ tab.label }}
        </button>
      </div>
    </div>

    <label class="flex w-full flex-col gap-1">
      <div class="flex items-center justify-between py-1"><span class="text-sm font-medium">LLM配置</span></div>
      <div class="flex w-full min-w-0 gap-1">
        <select
          :value="activeCapabilitySelectedId"
          class="select select-bordered select-sm flex-1 min-w-0"
          @change="switchCapabilityConfig"
        >
          <option v-for="a in capabilityScopedConfigsWithFallback" :key="a.id" :value="a.id">{{ a.name }}</option>
        </select>
        <button class="btn btn-sm btn-square bg-base-100" :title="t('config.api.addConfig')" @click="handleAddApiConfig">
          <Plus class="h-3.5 w-3.5" />
        </button>
        <button class="btn btn-sm btn-square bg-base-100" :title="t('config.api.removeConfig')" :disabled="props.config.apiConfigs.length <= 1" @click="$emit('removeSelectedApiConfig')">
          <Trash2 class="h-3.5 w-3.5" />
        </button>
        <button
          class="btn btn-sm btn-square"
          :class="props.configDirty ? 'btn-primary' : 'bg-base-100'"
          :disabled="!props.configDirty || props.savingConfig"
          :title="props.savingConfig ? t('config.api.saving') : props.configDirty ? t('config.api.saveConfig') : t('config.api.saved')"
          @click="$emit('saveApiConfig')"
        >
          <Save v-if="!props.savingConfig" class="h-3.5 w-3.5" />
          <span v-else class="loading loading-spinner loading-xs"></span>
        </button>
      </div>
    </label>

    <div class="divider my-0"></div>

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
          <button class="btn btn-sm btn-square bg-base-100" :title="t('config.api.linkHelper')" @click="baseUrlHelperOpen = !baseUrlHelperOpen">
            <WandSparkles class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
      <div v-if="baseUrlHelperOpen" class="mt-1 rounded-box border border-base-300 bg-base-100 p-2">
        <div class="mb-2 text-xs opacity-70">{{ t("config.api.linkHelperHint") }}</div>
        <div class="flex flex-wrap gap-1">
          <div v-for="preset in filteredProviderPresets" :key="preset.id" class="join shadow-sm rounded-btn">
            <button
              class="btn btn-sm join-item relative overflow-visible"
              :class="selectedProviderId === preset.id ? 'btn-primary' : 'bg-base-200'"
              @click="selectedProviderId = preset.id"
            >
              <span
                v-if="preset.hasFreeQuota"
                class="badge badge-secondary badge-xs text-[9px] leading-none absolute -top-2 left-1"
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
          <div class="flex items-center justify-between py-0"><span class="text-xs">{{ t("config.api.generatedLink") }}</span></div>
          <div class="flex w-full min-w-0 gap-1">
            <input :value="generatedBaseUrl" class="input input-bordered input-xs flex-1 min-w-0" readonly />
            <button class="btn btn-xs btn-primary" :disabled="!generatedBaseUrl" @click="applyGeneratedBaseUrl">
              <Link class="h-3 w-3" />
              <span>{{ t("config.api.fillBaseUrl") }}</span>
            </button>
          </div>
        </label>
      </div>
    </div>

    <div class="flex w-full items-center gap-2">
      <span class="w-24 shrink-0 text-sm font-medium">API Key</span>
      <input v-model="props.selectedApiConfig.apiKey" type="password" class="input input-bordered input-sm min-w-0 flex-1" placeholder="api key" />
    </div>

    <div class="flex w-full flex-col gap-1">
      <div class="flex w-full items-center gap-2">
        <span class="w-24 shrink-0 text-sm font-medium">{{ t("config.api.model") }}</span>
        <div class="flex min-w-0 flex-1 gap-1">
          <input v-model="props.selectedApiConfig.model" class="input input-bordered input-sm min-w-0 flex-1" placeholder="model" />
          <div v-if="isTextMode" class="dropdown dropdown-end">
            <button
              tabindex="0"
              class="btn btn-sm btn-square"
              :class="props.modelRefreshOk ? 'btn-primary' : 'bg-base-100'"
              :disabled="props.modelOptions.length === 0"
              :title="t('config.api.pickModel')"
            >
              <ChevronsUpDown class="h-3.5 w-3.5" />
            </button>
            <div tabindex="0" class="dropdown-content z-[1] flex flex-col shadow bg-base-100 rounded-box min-w-[280px] max-h-72 overflow-hidden">
              <input
                v-model="modelSearch"
                type="text"
                :placeholder="t('config.api.searchModel')"
                class="input input-sm input-bordered h-8 min-h-8 w-full rounded-none border-x-0 border-t-0 focus:outline-none"
                @click.stop
              />
              <ul class="menu flex-col flex-nowrap flex-1 min-h-0 overflow-auto p-1">
                <li v-for="modelName in filteredModels" :key="modelName">
                  <button class="whitespace-normal break-words text-left" @click="selectModel(modelName)">{{ modelName }}</button>
                </li>
                <li v-if="filteredModels.length === 0" class="text-center text-xs opacity-50 py-2">{{ t("config.api.noModelFound") }}</li>
              </ul>
            </div>
          </div>
          <button v-if="isTextMode" class="btn btn-sm btn-square bg-base-100" :class="{ loading: props.refreshingModels }" :disabled="props.refreshingModels" :title="t('config.api.refreshModels')" @click="$emit('refreshModels')">
            <RefreshCw class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
      <div class="flex w-full items-center justify-between pl-[6.5rem]">
        <span class="text-[11px] text-error min-h-4">{{ props.modelRefreshError || " " }}</span>
      </div>
    </div>

    <div v-if="isTextMode" class="flex w-full items-center gap-2">
      <span class="w-24 shrink-0 text-sm font-medium">{{ t("config.api.temperature") }}</span>
      <div class="min-w-0 flex-1">
        <div class="mb-1 flex items-center justify-end">
        <span class="text-xs opacity-70">{{ Number(props.selectedApiConfig.temperature ?? 1).toFixed(1) }}</span>
        </div>
        <input v-model.number="props.selectedApiConfig.temperature" type="range" min="0" max="2" step="0.1" class="range range-xs w-full" />
        <div class="mt-1 flex justify-between text-[10px] opacity-60">
          <span>0.0</span>
          <span>1.0</span>
          <span>2.0</span>
        </div>
      </div>
    </div>

    <div v-if="isTextMode" class="flex w-full items-center gap-2">
      <span class="w-24 shrink-0 text-sm font-medium">{{ t("config.api.contextWindow") }}</span>
      <div class="min-w-0 flex-1">
        <div class="mb-1 flex items-center justify-end">
        <span class="text-xs opacity-70">{{ Math.round(Number(props.selectedApiConfig.contextWindowTokens ?? 128000)) }}</span>
        </div>
        <input v-model.number="props.selectedApiConfig.contextWindowTokens" type="range" min="16000" max="200000" step="1000" class="range range-xs w-full" />
        <div class="mt-1 flex justify-between text-[10px] opacity-60">
          <span>16K</span>
          <span>100K</span>
          <span>200K</span>
        </div>
      </div>
    </div>

    <div v-if="isTextMode" class="flex w-full flex-col gap-1">
      <div class="flex items-center justify-between py-1"><span class="text-sm font-medium">{{ t("config.api.capabilities") }}</span></div>
      <div class="flex w-full gap-2">
        <label class="flex flex-1 cursor-pointer items-center justify-between rounded-md border border-base-300 bg-base-100 px-2 py-1"><span class="text-xs">{{ t("config.api.capImage") }}</span><input v-model="props.selectedApiConfig.enableImage" type="checkbox" class="toggle toggle-sm" /></label>
        <label class="flex flex-1 cursor-pointer items-center justify-between rounded-md border border-base-300 bg-base-100 px-2 py-1"><span class="text-xs">{{ t("config.api.capTools") }}</span><input v-model="props.selectedApiConfig.enableTools" type="checkbox" class="toggle toggle-sm" /></label>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronsUpDown, ExternalLink, Link, Plus, RefreshCw, Save, Trash2, WandSparkles } from "lucide-vue-next";
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
}>();

const emit = defineEmits<{
  (e: "saveApiConfig"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "refreshModels"): void;
  (e: "configSwitched"): void;
}>();

const { t } = useI18n();
const baseUrlHelperOpen = ref(false);
const selectedProviderId = ref("openai-official");
const modelSearch = ref("");
const activeCapability = ref<ApiCapability>("text");
const creatingCapabilityDefault = ref(false);

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
    { value: "deepseek/kimi", label: "DeepSeek/Kimi" },
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
const currentProtocolOptions = computed(() => protocolOptionsByCapability[activeCapability.value]);
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
  const selected = props.selectedApiConfig;
  if (!selected) return;
  activeCapability.value = capabilityFromConfig(selected);
  ensureCapabilityConfig(activeCapability.value);
});

watch(activeCapability, (capability) => {
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
  },
  { immediate: true, deep: true },
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
  modelSearch.value = "";
}

const providerPresets: ProviderPreset[] = [
  { id: "openai-official", name: "OpenAI", urls: { openai: "https://api.openai.com/v1", openai_responses: "https://api.openai.com/v1", openai_stt: "https://api.openai.com/v1", openai_tts: "https://api.openai.com/v1/audio/speech", openai_embedding: "https://api.openai.com/v1", openai_rerank: "https://api.openai.com/v1" }, docsUrl: "https://platform.openai.com/docs/overview" },
  { id: "anthropic-official", name: "Anthropic", urls: { anthropic: "https://api.anthropic.com" }, docsUrl: "https://docs.anthropic.com/en/api/overview" },
  { id: "google-gemini", name: "Google Gemini", urls: { gemini: "https://generativelanguage.googleapis.com", gemini_embedding: "https://generativelanguage.googleapis.com" }, docsUrl: "https://ai.google.dev/gemini-api/docs", hasFreeQuota: true },
  { id: "deepseek", name: "DeepSeek", urls: { openai: "https://api.deepseek.com/v1", openai_responses: "https://api.deepseek.com/v1", "deepseek/kimi": "https://api.deepseek.com/v1" }, docsUrl: "https://api-docs.deepseek.com/" },
  { id: "moonshot-kimi", name: "Moonshot/Kimi", urls: { openai: "https://api.moonshot.cn/v1", openai_responses: "https://api.moonshot.cn/v1", "deepseek/kimi": "https://api.moonshot.cn/v1" }, docsUrl: "https://platform.moonshot.cn/docs/api-reference" },
  { id: "zhipu-glm", name: "Zhipu GLM", urls: { openai: "https://open.bigmodel.cn/api/paas/v4", openai_responses: "https://open.bigmodel.cn/api/paas/v4", "deepseek/kimi": "https://open.bigmodel.cn/api/paas/v4" }, docsUrl: "https://open.bigmodel.cn/dev/api", hasFreeQuota: true },
  { id: "minimax", name: "MiniMax", urls: { openai: "https://api.minimax.chat/v1", openai_responses: "https://api.minimax.chat/v1", "deepseek/kimi": "https://api.minimax.chat/v1" }, docsUrl: "https://www.minimax.io/platform/document" },
  { id: "siliconflow", name: "SiliconFlow", urls: { openai: "https://api.siliconflow.cn/v1", openai_responses: "https://api.siliconflow.cn/v1", openai_stt: "https://api.siliconflow.cn/v1", openai_embedding: "https://api.siliconflow.cn/v1", openai_rerank: "https://api.siliconflow.cn/v1", "deepseek/kimi": "https://api.siliconflow.cn/v1" }, docsUrl: "https://docs.siliconflow.cn/", hasFreeQuota: true },
  { id: "iflow", name: "iFlow", urls: { openai: "https://apis.iflow.cn/v1", openai_responses: "https://apis.iflow.cn/v1" }, docsUrl: "https://platform.iflow.cn/models", hasFreeQuota: true },
  { id: "modelscope", name: "ModelScope", urls: { openai: "https://api-inference.modelscope.cn/v1", openai_responses: "https://api-inference.modelscope.cn/v1" }, docsUrl: "https://modelscope.cn/models", hasFreeQuota: true },
  { id: "nvidia-nim", name: "NVIDIA NIM", urls: { openai: "https://integrate.api.nvidia.com/v1", openai_responses: "https://integrate.api.nvidia.com/v1", "deepseek/kimi": "https://integrate.api.nvidia.com/v1" }, docsUrl: "https://docs.api.nvidia.com/nim/", hasFreeQuota: true },
  { id: "openrouter", name: "OpenRouter", urls: { openai: "https://openrouter.ai/api/v1", openai_responses: "https://openrouter.ai/api/v1", "deepseek/kimi": "https://openrouter.ai/api/v1" }, docsUrl: "https://openrouter.ai/docs/api-reference/overview", hasFreeQuota: true },
  { id: "cloudflare-gateway", name: "Cloudflare Gateway", urls: { openai: "https://gateway.ai.cloudflare.com/v1/{account_id}/{gateway_id}/{provider}", openai_responses: "https://gateway.ai.cloudflare.com/v1/{account_id}/{gateway_id}/{provider}", "deepseek/kimi": "https://gateway.ai.cloudflare.com/v1/{account_id}/{gateway_id}/{provider}" }, docsUrl: "https://developers.cloudflare.com/ai-gateway/" },
  { id: "ollama-local", name: "Ollama (Local)", urls: { openai: "http://localhost:11434/v1", openai_responses: "http://localhost:11434/v1", "deepseek/kimi": "http://localhost:11434/v1" }, docsUrl: "https://github.com/ollama/ollama/blob/main/docs/openai.md" },
];

const DEEPSEEK_KIMI_PROVIDER_IDS = new Set<string>([
  "deepseek",
  "moonshot-kimi",
  "cloudflare-gateway",
]);

const filteredProviderPresets = computed(() => {
  const sortFreeFirst = (list: ProviderPreset[]) =>
    [...list].sort((a, b) => Number(Boolean(b.hasFreeQuota)) - Number(Boolean(a.hasFreeQuota)));

  if (currentProtocol.value === "deepseek/kimi") {
    return sortFreeFirst(providerPresets.filter(
      (p) =>
        DEEPSEEK_KIMI_PROVIDER_IDS.has(p.id)
        && Boolean(p.urls["deepseek/kimi"]),
    ));
  }
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

