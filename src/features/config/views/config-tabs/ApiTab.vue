<template>
  <button class="btn btn-primary btn-sm w-full" :disabled="!props.configDirty || props.savingConfig" @click="$emit('saveApiConfig')">
    {{ props.savingConfig ? t("config.api.saving") : props.configDirty ? t("config.api.saveConfig") : t("config.api.saved") }}
  </button>
  <label class="form-control">
    <div class="label py-1"><span class="label-text text-sm font-medium">{{ t("config.api.editTitle") }}</span></div>
    <div class="flex gap-1">
      <select v-model="props.config.selectedApiConfigId" class="select select-bordered select-sm flex-1">
        <option v-for="a in props.config.apiConfigs" :key="a.id" :value="a.id">{{ a.name }}</option>
      </select>
      <button class="btn btn-sm btn-square btn-ghost bg-base-100" :title="t('config.api.addConfig')" @click="$emit('addApiConfig')">
        <Plus class="h-3.5 w-3.5" />
      </button>
      <button class="btn btn-sm btn-square btn-ghost bg-base-100" :title="t('config.api.removeConfig')" :disabled="props.config.apiConfigs.length <= 1" @click="$emit('removeSelectedApiConfig')">
        <Trash2 class="h-3.5 w-3.5" />
      </button>
    </div>
  </label>

  <div class="divider my-0"></div>

  <div v-if="props.selectedApiConfig" class="grid gap-2">
    <label class="form-control">
      <div class="label py-1"><span class="label-text text-sm font-medium">{{ t("config.api.configName") }}</span></div>
      <input v-model="props.selectedApiConfig.name" class="input input-bordered input-sm" :placeholder="t('config.api.configName')" />
    </label>
    <label class="form-control">
      <div class="label py-1"><span class="label-text text-sm font-medium">{{ t("config.api.requestFormat") }}</span></div>
      <select v-model="props.selectedApiConfig.requestFormat" class="select select-bordered select-sm">
        <option value="openai">OpenAI Compatible</option>
        <option value="gemini">Google Gemini</option>
        <option value="deepseek/kimi">DeepSeek/Kimi</option>
        <option value="anthropic">Anthropic</option>
      </select>
    </label>
    <label class="form-control">
      <div class="label py-1"><span class="label-text text-sm font-medium">{{ t("config.api.baseUrl") }}</span></div>
      <div class="flex gap-1">
        <input v-model="props.selectedApiConfig.baseUrl" class="input input-bordered input-sm flex-1" :placeholder="props.baseUrlReference" />
        <button class="btn btn-sm btn-square btn-ghost bg-base-100" :title="t('config.api.linkHelper')" @click="baseUrlHelperOpen = !baseUrlHelperOpen">
          <WandSparkles class="h-3.5 w-3.5" />
        </button>
      </div>
      <div v-if="baseUrlHelperOpen" class="mt-1 rounded-box border border-base-300 bg-base-100 p-2">
        <div class="mb-2 text-xs opacity-70">{{ t("config.api.linkHelperHint") }}</div>
        <div class="flex flex-wrap gap-1">
          <div v-for="preset in filteredProviderPresets" :key="preset.id" class="join shadow-sm rounded-btn">
            <button
              class="btn btn-sm join-item relative overflow-visible"
              :class="selectedProviderId === preset.id ? 'btn-primary' : 'btn-ghost bg-base-200'"
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
        <label class="form-control mt-2">
          <div class="label py-0"><span class="label-text text-xs">{{ t("config.api.generatedLink") }}</span></div>
          <div class="flex gap-1">
            <input :value="generatedBaseUrl" class="input input-bordered input-xs flex-1" readonly />
            <button class="btn btn-xs btn-primary" :disabled="!generatedBaseUrl" @click="applyGeneratedBaseUrl">
              <Link class="h-3 w-3" />
              <span>{{ t("config.api.fillBaseUrl") }}</span>
            </button>
          </div>
        </label>
      </div>
    </label>
    <label class="form-control">
      <div class="label py-1"><span class="label-text text-sm font-medium">API Key</span></div>
      <input v-model="props.selectedApiConfig.apiKey" type="password" class="input input-bordered input-sm" placeholder="api key" />
    </label>
    <label class="form-control">
      <div class="label py-1">
        <span class="label-text text-sm font-medium">{{ t("config.api.model") }}</span>
        <span class="label-text-alt text-[11px] text-error min-h-4">{{ props.modelRefreshError || " " }}</span>
      </div>
      <div class="flex gap-1">
        <input v-model="props.selectedApiConfig.model" class="input input-bordered input-sm flex-1" placeholder="model" />
        <div class="dropdown dropdown-end">
          <button
            tabindex="0"
            class="btn btn-sm btn-square"
            :class="props.modelRefreshOk ? 'btn-primary' : 'btn-ghost bg-base-100'"
            :disabled="props.modelOptions.length === 0"
            :title="t('config.api.pickModel')"
          >
            <ChevronsUpDown class="h-3.5 w-3.5" />
          </button>
          <ul tabindex="0" class="dropdown-content z-[1] menu p-1 shadow bg-base-100 rounded-box w-52 max-h-56 overflow-auto">
            <li v-for="modelName in props.modelOptions" :key="modelName">
              <button @click="props.selectedApiConfig.model = modelName">{{ modelName }}</button>
            </li>
          </ul>
        </div>
        <button class="btn btn-sm btn-square btn-ghost bg-base-100" :class="{ loading: props.refreshingModels }" :disabled="props.refreshingModels" :title="t('config.api.refreshModels')" @click="$emit('refreshModels')">
          <RefreshCw class="h-3.5 w-3.5" />
        </button>
      </div>
    </label>
    <label class="form-control">
      <div class="label py-1">
        <span class="label-text text-sm font-medium">{{ t("config.api.temperature") }}</span>
        <span class="label-text-alt text-xs opacity-70">{{ Number(props.selectedApiConfig.temperature ?? 1).toFixed(1) }}</span>
      </div>
      <input v-model.number="props.selectedApiConfig.temperature" type="range" min="0" max="2" step="0.1" class="range range-xs" />
      <div class="mt-1 flex justify-between text-[10px] opacity-60">
        <span>0.0</span>
        <span>1.0</span>
        <span>2.0</span>
      </div>
    </label>
    <label class="form-control">
      <div class="label py-1">
        <span class="label-text text-sm font-medium">{{ t("config.api.contextWindow") }}</span>
        <span class="label-text-alt text-xs opacity-70">{{ Math.round(Number(props.selectedApiConfig.contextWindowTokens ?? 128000)) }}</span>
      </div>
      <input v-model.number="props.selectedApiConfig.contextWindowTokens" type="range" min="16000" max="200000" step="1000" class="range range-xs" />
      <div class="mt-1 flex justify-between text-[10px] opacity-60">
        <span>16K</span>
        <span>100K</span>
        <span>200K</span>
      </div>
    </label>
    <div class="form-control">
      <div class="label py-1"><span class="label-text text-sm font-medium">{{ t("config.api.capabilities") }}</span></div>
      <div class="flex gap-2">
        <label class="label cursor-pointer gap-1"><span class="label-text text-xs">{{ t("config.api.capText") }}</span><input v-model="props.selectedApiConfig.enableText" type="checkbox" class="toggle toggle-sm" /></label>
        <label class="label cursor-pointer gap-1"><span class="label-text text-xs">{{ t("config.api.capImage") }}</span><input v-model="props.selectedApiConfig.enableImage" type="checkbox" class="toggle toggle-sm" /></label>
        <label class="label cursor-pointer gap-1"><span class="label-text text-xs">{{ t("config.api.capTools") }}</span><input v-model="props.selectedApiConfig.enableTools" type="checkbox" class="toggle toggle-sm" /></label>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronsUpDown, ExternalLink, Link, Plus, RefreshCw, Trash2, WandSparkles } from "lucide-vue-next";
import type { ApiConfigItem, AppConfig } from "../../../../types/app";
import { invokeTauri } from "../../../../services/tauri-api";

type ProviderPreset = {
  id: string;
  name: string;
  urls: Partial<Record<"openai" | "gemini" | "deepseek/kimi" | "anthropic", string>>;
  docsUrl: string;
  hasFreeQuota?: boolean;
};

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

defineEmits<{
  (e: "saveApiConfig"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "refreshModels"): void;
}>();

const { t } = useI18n();
const baseUrlHelperOpen = ref(false);
const selectedProviderId = ref("openai-official");

const providerPresets: ProviderPreset[] = [
  { id: "openai-official", name: "OpenAI", urls: { openai: "https://api.openai.com/v1" }, docsUrl: "https://platform.openai.com/docs/overview" },
  { id: "anthropic-official", name: "Anthropic", urls: { anthropic: "https://api.anthropic.com" }, docsUrl: "https://docs.anthropic.com/en/api/overview" },
  { id: "google-gemini", name: "Google Gemini", urls: { gemini: "https://generativelanguage.googleapis.com" }, docsUrl: "https://ai.google.dev/gemini-api/docs", hasFreeQuota: true },
  { id: "deepseek", name: "DeepSeek", urls: { openai: "https://api.deepseek.com/v1", "deepseek/kimi": "https://api.deepseek.com/v1" }, docsUrl: "https://api-docs.deepseek.com/" },
  { id: "moonshot-kimi", name: "Moonshot/Kimi", urls: { openai: "https://api.moonshot.cn/v1", "deepseek/kimi": "https://api.moonshot.cn/v1" }, docsUrl: "https://platform.moonshot.cn/docs/api-reference" },
  { id: "zhipu-glm", name: "Zhipu GLM", urls: { openai: "https://open.bigmodel.cn/api/paas/v4", "deepseek/kimi": "https://open.bigmodel.cn/api/paas/v4" }, docsUrl: "https://open.bigmodel.cn/dev/api", hasFreeQuota: true },
  { id: "minimax", name: "MiniMax", urls: { openai: "https://api.minimax.chat/v1", "deepseek/kimi": "https://api.minimax.chat/v1" }, docsUrl: "https://www.minimax.io/platform/document" },
  { id: "siliconflow", name: "SiliconFlow", urls: { openai: "https://api.siliconflow.cn/v1", "deepseek/kimi": "https://api.siliconflow.cn/v1" }, docsUrl: "https://docs.siliconflow.cn/" },
  { id: "iflow", name: "iFlow", urls: { openai: "https://apis.iflow.cn/v1" }, docsUrl: "https://platform.iflow.cn/models", hasFreeQuota: true },
  { id: "modelscope", name: "ModelScope", urls: { openai: "https://api-inference.modelscope.cn/v1" }, docsUrl: "https://modelscope.cn/models", hasFreeQuota: true },
  { id: "nvidia-nim", name: "NVIDIA NIM", urls: { openai: "https://integrate.api.nvidia.com/v1", "deepseek/kimi": "https://integrate.api.nvidia.com/v1" }, docsUrl: "https://docs.api.nvidia.com/nim/", hasFreeQuota: true },
  { id: "openrouter", name: "OpenRouter", urls: { openai: "https://openrouter.ai/api/v1", "deepseek/kimi": "https://openrouter.ai/api/v1" }, docsUrl: "https://openrouter.ai/docs/api-reference/overview", hasFreeQuota: true },
  { id: "cloudflare-gateway", name: "Cloudflare Gateway", urls: { openai: "https://gateway.ai.cloudflare.com/v1/{account_id}/{gateway_id}/{provider}", "deepseek/kimi": "https://gateway.ai.cloudflare.com/v1/{account_id}/{gateway_id}/{provider}" }, docsUrl: "https://developers.cloudflare.com/ai-gateway/" },
  { id: "ollama-local", name: "Ollama (Local)", urls: { openai: "http://localhost:11434/v1", "deepseek/kimi": "http://localhost:11434/v1" }, docsUrl: "https://github.com/ollama/ollama/blob/main/docs/openai.md" },
];

const currentProtocol = computed(() => (props.selectedApiConfig?.requestFormat?.trim() || "openai") as "openai" | "gemini" | "deepseek/kimi" | "anthropic");
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
        DEEPSEEK_KIMI_PROVIDER_IDS.has(p.id) &&
        Boolean(p.urls["deepseek/kimi"]),
    ));
  }
  return sortFreeFirst(providerPresets.filter((p) => Boolean(p.urls[currentProtocol.value])));
});
const selectedProvider = computed(() => providerPresets.find((p) => p.id === selectedProviderId.value) ?? providerPresets[0]);
const generatedBaseUrl = computed(() => {
  const urls = selectedProvider.value.urls;
  return urls[currentProtocol.value] || urls.openai || urls.gemini || urls["deepseek/kimi"] || urls.anthropic || "";
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

async function openProviderSite(preset: ProviderPreset) {
  if (!preset.docsUrl) return;
  try {
    await invokeTauri("open_external_url", { url: preset.docsUrl });
  } catch (error) {
    console.warn("[API] open provider docs failed:", error);
  }
}
</script>
