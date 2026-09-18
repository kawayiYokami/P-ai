<template>
  <div class="grid gap-3">
    <div v-if="selectedProvider" class="grid gap-3">
      <ConfigTemplate v-model="providerTemplateValues" :groups="providerTemplateGroups" />
      <ApiKeyListCard
        v-if="selectedProvider.providerType !== 'codex'"
        :title="t('config.imageGeneration.apiKeys')"
        :key="selectedProvider.id"
        :model-value="selectedProvider.apiKeys"
        @update:model-value="updateSelectedApiKeys"
      />
      <div v-else class="rounded-box border border-info/30 bg-info/5 px-3 py-2 text-xs text-base-content/70">
        {{ t("config.imageGeneration.codexCredentialHint") }}
      </div>

      <ConfigCard :title="t('config.api.modelCards')">
        <template #actions>
          <button class="btn btn-sm" type="button" :title="t('config.imageGeneration.addModel')" @click="addModel">
            <Plus class="h-4 w-4" />
            <span>{{ t("config.imageGeneration.addModel") }}</span>
          </button>
        </template>

          <div v-if="selectedProvider.models.length" class="divide-y divide-base-200/60">
            <article v-for="model in selectedProvider.models" :key="model.id" class="py-3">
              <div class="flex items-center justify-between gap-3">
                <div class="min-w-0 flex-1 truncate text-base font-semibold">{{ model.name || model.model || model.id }}</div>
                <button class="btn btn-square btn-ghost btn-sm text-error" type="button" :title="t('common.delete')" @click="removeModel(model.id)">
                  <Trash2 class="h-4 w-4" />
                </button>
              </div>
              <label class="mt-3 grid gap-1">
                <span class="text-sm font-medium">{{ t("config.imageGeneration.modelName") }}</span>
                <input v-model="model.name" class="input input-bordered input-sm w-full" :placeholder="t('config.imageGeneration.modelName')" />
              </label>
              <label class="mt-3 grid gap-1">
                <span class="text-sm font-medium">{{ t("config.api.model") }}</span>
                  <div class="join w-full">
                    <input v-model="model.model" class="input input-bordered input-sm join-item flex-1 font-mono" :placeholder="model.id" @blur="syncImageModelIdentifier(model)" @keydown.enter.prevent="syncImageModelIdentifier(model)" />
                    <button class="btn btn-sm join-item bg-base-300" type="button" @click="toggleImageModelPicker(model.id)"><ChevronDown class="h-3.5 w-3.5" /></button>
                  </div>
                  <div v-if="activeImageModelPickerId === model.id" class="rounded-box border border-base-300 bg-base-200/50 p-3">
                    <input v-model="imageModelSearch" class="input input-bordered input-sm mb-2 w-full" :placeholder="t('config.api.searchModel')" />
                    <div class="max-h-48 overflow-auto">
                      <button v-for="option in filteredImageModelOptions" :key="`${model.id}-${option}`" class="btn btn-ghost btn-sm mb-1 mr-1" type="button" @click="selectImageModel(model.id, option)">{{ option }}</button>
                      <div v-if="filteredImageModelOptions.length === 0" class="px-2 py-3 text-sm opacity-50">{{ t("config.api.noModelFound") }}</div>
                    </div>
                  </div>
              </label>
              <div class="mt-2 text-xs text-base-content/60">{{ t("config.api.matchedProtocol", { protocol: providerTypeLabel(selectedProvider.providerType) }) }}</div>
            </article>
          </div>
          <div v-else class="rounded-box border border-dashed border-base-300 py-4 text-center text-xs text-base-content/55">
            {{ t("config.imageGeneration.emptyModels") }}
          </div>
      </ConfigCard>

      <ConfigCard v-if="selectedProvider.providerType === 'comfyui'" :title="t('config.imageGeneration.comfyTitle')">
        <div class="grid gap-4 py-3">
          <label class="grid gap-1">
            <span class="text-sm font-medium">{{ t("config.imageGeneration.workflowJson") }}</span>
            <textarea v-model="selectedProvider.comfyuiWorkflowJson" class="textarea textarea-bordered min-h-64 font-mono text-caption leading-relaxed" :class="{ 'textarea-error': workflowJsonError }" :placeholder="t('config.imageGeneration.workflowPlaceholder')" />
            <span v-if="workflowJsonError" class="mt-1 text-xs text-error">{{ workflowJsonError }}</span>
          </label>
          <div class="grid gap-3 md:grid-cols-2">
            <div v-for="field in comfyMappingFields" :key="field.key" class="rounded-box border border-base-300 bg-base-200/30 p-3">
              <div class="text-sm font-medium">{{ t(field.labelKey) }}</div>
              <label class="mt-2 grid gap-1">
                <span class="text-xs">{{ t("config.imageGeneration.nodeIds") }}</span>
                <input class="input input-bordered input-sm font-mono" :value="selectedProvider.comfyuiMapping[field.key].nodeIds.join(', ')" @input="setMappingNodeIds(field.key, ($event.target as HTMLInputElement).value)" />
              </label>
              <label class="mt-2 grid gap-1">
                <span class="text-xs">{{ t("config.imageGeneration.inputKey") }}</span>
                <input v-model="selectedProvider.comfyuiMapping[field.key].inputKey" class="input input-bordered input-sm font-mono" />
              </label>
            </div>
          </div>
          <label class="grid gap-1">
            <span class="text-sm font-medium">{{ t("config.imageGeneration.outputNodeIds") }}</span>
            <input class="input input-bordered input-sm font-mono text-xs" :value="selectedProvider.comfyuiMapping.outputNodeIds.join(', ')" @input="setOutputNodeIds(($event.target as HTMLInputElement).value)" />
           <span class="text-xs text-base-content/50">{{ t("config.imageGeneration.outputNodeIdsHint") }}</span>
          </label>
        </div>
      </ConfigCard>

      <ConfigCard :title="t('config.imageGeneration.testTitle')">
        <div class="grid gap-4 py-3">
          <div v-if="imageDirty" class="alert alert-warning py-2 text-xs">
            <span>{{ t("config.imageGeneration.testSaveFirst") }}</span>
          </div>

          <!-- 上：参数一排（与 image_generate 工具的可选参数对齐） -->
          <div class="flex flex-wrap gap-3">
            <div class="grid min-w-40 flex-1 content-start gap-1">
              <span class="text-xs font-medium text-base-content/60">{{ t("config.imageGeneration.testModel") }}</span>
              <select v-model="testModelId" class="select select-bordered select-sm w-full">
                <option v-if="testModelOptions.length === 0" value="" disabled>{{ t("config.imageGeneration.emptyModels") }}</option>
                <option v-for="option in testModelOptions" :key="option.value" :value="option.value">{{ option.label }}</option>
              </select>
            </div>
            <div class="grid content-start gap-1">
              <span class="text-xs font-medium text-base-content/60">{{ t("config.imageGeneration.testSize") }}</span>
              <select v-model="testResolution" class="select select-bordered select-sm w-40 font-mono">
                <option value="">{{ t("config.imageGeneration.testOptionModelDefault") }}</option>
                <option v-for="preset in testResolutionPresets" :key="preset" :value="preset">{{ preset }}</option>
              </select>
            </div>
          </div>

          <!-- 中：提示词，标题独立一行 + 全宽多行文本框 -->
          <div class="grid gap-1">
            <span class="text-sm font-medium">{{ t("config.imageGeneration.testPrompt") }}</span>
            <textarea v-model="testPrompt" class="textarea textarea-bordered min-h-24 w-full" :placeholder="t('config.imageGeneration.testPromptPlaceholder')" />
          </div>

          <!-- 下：生成按钮 + 常驻预览 -->
          <button class="btn btn-primary w-full" type="button" :disabled="!canRunImageTest" @click="runImageTest">
            <span v-if="testingImage" class="loading loading-spinner loading-sm" />
            <ImageIcon v-else class="h-4 w-4" />
            {{ testingImage ? t("config.imageGeneration.generating") : t("config.imageGeneration.generateTest") }}
          </button>

          <div v-if="testError" class="rounded-box border border-error/30 bg-error/5 px-3 py-2 text-xs text-error">{{ testError }}</div>

          <div class="overflow-hidden rounded-box border border-base-300 bg-base-200/40">
            <div v-if="testingImage" class="skeleton h-72 w-full rounded-none" />
            <img v-else-if="testPreviewDataUrl" :src="testPreviewDataUrl" :alt="firstTestImage?.revisedPrompt || testPrompt" class="mx-auto block max-h-96 object-contain" />
            <div v-else class="flex h-48 flex-col items-center justify-center gap-2 text-xs text-base-content/45">
              <ImageIcon class="h-8 w-8 opacity-40" />
              <span>{{ t("config.imageGeneration.previewEmpty") }}</span>
            </div>
          </div>

          <template v-if="!testingImage && testResult && firstTestImage">
            <div class="flex flex-wrap items-center gap-2 text-xs">
              <span class="badge badge-ghost badge-sm">{{ testResult.providerName }} · {{ testResult.model }}</span>
              <span class="badge badge-ghost badge-sm font-mono">{{ firstTestImage.width }}×{{ firstTestImage.height }}</span>
              <code class="min-w-0 flex-1 truncate rounded bg-base-200 px-2 py-1 text-caption" :title="firstTestImage.relativePath">{{ firstTestImage.relativePath }}</code>
              <button v-if="localFileSystemAvailable" class="btn btn-xs btn-ghost shrink-0" type="button" :disabled="copyingImage" @click="copyGeneratedImage(firstTestImage.relativePath)">
                <span v-if="copyingImage" class="loading loading-spinner loading-xs" />
                <ImageIcon v-else class="h-3.5 w-3.5" />
                {{ t("config.imageGeneration.copyImage") }}
              </button>
              <button class="btn btn-xs btn-ghost shrink-0" type="button" @click="copyGeneratedMarkdown(firstTestImage.markdown)">
                <Copy class="h-3.5 w-3.5" />
                {{ t("config.imageGeneration.copyMarkdown") }}
              </button>
            </div>
            <div v-if="firstTestImage.revisedPrompt" class="text-xs text-base-content/60">
              <span class="font-medium">{{ t("config.imageGeneration.revisedPrompt") }}：</span>{{ firstTestImage.revisedPrompt }}
            </div>
          </template>
        </div>
      </ConfigCard>
    </div>

    <div v-else class="card border border-dashed border-base-300 bg-base-100">
      <div class="card-body items-center justify-center text-center text-sm text-base-content/55">
        <ImageIcon class="h-10 w-10 opacity-35" />
        <p>{{ t("config.imageGeneration.selectOrAddProvider") }}</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ChevronDown, Copy, Image as ImageIcon, Plus, Trash2 } from "@lucide/vue";
import type {
  AppConfig,
  ComfyUiWorkflowMapping,
  ImageGenerationModelConfigItem,
  ImageGenerationProviderKind,
  ImageGenerationResult,
} from "../../../../types/app";
import {
  copyTransportChatImageToClipboard,
  getTransportCapabilities,
  invokeTauri,
  readTransportChatImage,
} from "../../../../services/tauri-api";
import ConfigCard from "../../components/ConfigCard.vue";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import ApiKeyListCard from "../../components/ApiKeyListCard.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import {
  createImageGenerationModel,
  createImageGenerationProvider,
  imageGenerationEndpointId,
  imageGenerationProviderTemplate,
  normalizeImageGenerationModelId,
  normalizeImageGenerationProviders,
  resolveSelectedImageProviderId,
} from "../../utils/image-generation-config";

type ComfyInputMappingKey = Exclude<keyof ComfyUiWorkflowMapping, "outputNodeIds">;

const props = defineProps<{
  config: AppConfig;
  selectedProviderId: string;
  savingConfig: boolean;
  saveConfigAction: () => Promise<boolean> | boolean;
  lastSavedConfigJson: string;
  setStatusAction: (text: string) => void;
}>();

const emit = defineEmits<{
  "update:selectedProviderId": [value: string];
}>();

const { t } = useI18n();
// 选中项归父级页面持有：详情组件挂在 v-if 分支下，概览卡片点击时它还没挂载，状态留在组件内部会丢
const selectedProviderId = computed({
  get: () => props.selectedProviderId,
  set: (value: string) => emit("update:selectedProviderId", value),
});
const testPrompt = ref("");
const localFileSystemAvailable = getTransportCapabilities().localFileSystem;
// 与 AI 工具 image_generate 的可选参数对齐：仅 resolution，留空表示用模型默认值
const testResolution = ref("");
const testResolutionPresets = ["512x512", "1024x1024", "1536x1024", "1024x1536", "2K", "4K"];
const testModelId = ref("");
const testingImage = ref(false);
const copyingImage = ref(false);
const testError = ref("");
const testResult = ref<ImageGenerationResult | null>(null);
const testPreviewDataUrl = ref("");
const firstTestImage = computed(() => testResult.value?.images[0] || null);
const activeImageModelPickerId = ref("");
const imageModelSearch = ref("");
let localSeed = 0;

const providerTypeOptions: Array<{ value: ImageGenerationProviderKind; label: string }> = [
  { value: "comfyui", label: "ComfyUI" },
  { value: "codex", label: "OpenAI Codex" },
  { value: "openai", label: "OpenAI" },
  { value: "xai", label: "xAI Grok Imagine" },
  { value: "seedream", label: "Seedance / Seedream" },
  { value: "gemini", label: "Gemini Nano Banana 2" },
  { value: "sensenova", label: "商汤科技 · SenseNova" },
];

const imageModelOptions = computed(() => {
  const provider = selectedProvider.value;
  if (!provider) return [];
  const defaults: Record<ImageGenerationProviderKind, string[]> = {
    comfyui: [],
    codex: ["gpt-image-2"],
    openai: ["gpt-image-2"],
    xai: ["grok-imagine-image-quality", "grok-imagine-image"],
    seedream: ["doubao-seedream-5-0-pro-260628"],
    gemini: ["gemini-3.1-flash-image"],
    sensenova: ["sensenova-u1-fast", "sensenova-u1.5-lite"],
  };
  return Array.from(new Set([...defaults[provider.providerType], ...provider.models.map((model) => model.model || model.id)].filter(Boolean)));
});

const filteredImageModelOptions = computed(() => {
  const query = imageModelSearch.value.trim().toLowerCase();
  return imageModelOptions.value.filter((option) => !query || option.toLowerCase().includes(query));
});

const comfyMappingFields: Array<{ key: ComfyInputMappingKey; labelKey: string }> = [
  { key: "prompt", labelKey: "config.imageGeneration.mappingPrompt" },
  { key: "negativePrompt", labelKey: "config.imageGeneration.mappingNegativePrompt" },
  { key: "model", labelKey: "config.imageGeneration.mappingModel" },
  { key: "width", labelKey: "config.imageGeneration.mappingWidth" },
  { key: "height", labelKey: "config.imageGeneration.mappingHeight" },
  { key: "seed", labelKey: "config.imageGeneration.mappingSeed" },
  { key: "steps", labelKey: "config.imageGeneration.mappingSteps" },
  { key: "inputImage", labelKey: "config.imageGeneration.mappingInputImage" },
  { key: "maskImage", labelKey: "config.imageGeneration.mappingMaskImage" },
];

const providers = computed(() => props.config.imageProviders || []);
const selectedProvider = computed(() => (
  providers.value.find((provider) => provider.id === selectedProviderId.value) || null
));
const codexApiProviders = computed(() => (
  (props.config.apiProviders || []).filter((provider) => provider.requestFormat === "codex" && !provider.deprecated)
));

const testModelOptions = computed(() => {
  const provider = selectedProvider.value;
  if (!provider) return [];
  return provider.models
    .filter((model) => !model.deprecated)
    .map((model) => {
      const endpointId = imageGenerationEndpointId(provider.id, model.id);
      const display = String(model.name || "").trim();
      const modelName = String(model.model || model.id || "").trim();
      const label = display && display !== modelName ? `${display} (${modelName})` : (modelName || display);
      return { value: endpointId, label };
    });
});

function ensureSensenovaModels(provider: { models: Array<{ id: string; name: string; model: string; enabled: boolean; deprecated?: boolean; defaultSize?: string; defaultAspectRatio?: string; defaultQuality?: string }> }) {
  const presets = [
    { id: "sensenova-u1-fast", name: "SenseNova U1 Fast", model: "sensenova-u1-fast" },
    { id: "sensenova-u1.5-lite", name: "SenseNova U1.5 Lite", model: "sensenova-u1.5-lite" },
  ];
  const existing = new Set(provider.models.map((m: { model: string; id: string }) => m.model || m.id));
  let added = false;
  for (const preset of presets) {
    if (existing.has(preset.model)) continue;
    provider.models.push({
      id: preset.id,
      name: preset.name,
      model: preset.model,
      enabled: true,
      deprecated: false,
      defaultSize: (provider.models[0] as { defaultSize?: string } | undefined)?.defaultSize,
      defaultAspectRatio: (provider.models[0] as { defaultAspectRatio?: string } | undefined)?.defaultAspectRatio,
      defaultQuality: (provider.models[0] as { defaultQuality?: string } | undefined)?.defaultQuality,
    });
    existing.add(preset.model);
    added = true;
  }
  if (provider.models.length === 0) {
    provider.models = presets.map((p) => ({ id: p.id, name: p.name, model: p.model, enabled: true, deprecated: false }));
  }
  void added;
}

const providerTemplateValues = computed<Record<string, unknown>>({
  get: () => {
    const provider = selectedProvider.value;
    if (!provider) return {};
    return {
      providerName: provider.name,
      providerType: provider.providerType,
      baseUrl: provider.baseUrl,
      codexApiProviderId: provider.codexApiProviderId || "",
      timeoutSeconds: provider.timeoutSeconds,
      watermark: provider.watermark,
    };
  },
  set: (values) => {
    const provider = selectedProvider.value;
    if (!provider) return;
    if (typeof values.providerType === "string" && providerTypeOptions.some((item) => item.value === values.providerType)) {
      const nextType = values.providerType as ImageGenerationProviderKind;
      const prevType = provider.providerType;
      if (nextType !== prevType) {
        const template = imageGenerationProviderTemplate(nextType);
        provider.providerType = nextType;
        // 自动填充 URL 与名称：仅当为空或仍为旧模板默认值时覆盖，保留用户手改
        if (!provider.baseUrl || provider.baseUrl === imageGenerationProviderTemplate(prevType).baseUrl) {
          provider.baseUrl = template.baseUrl;
        }
        if (!provider.name || provider.name === imageGenerationProviderTemplate(prevType).name) {
          provider.name = template.name;
        }
        if (nextType === "sensenova") {
          ensureSensenovaModels(provider as any);
          if (!props.config.imageGenerationModelId) {
            const first = provider.models[0];
            if (first) props.config.imageGenerationModelId = imageGenerationEndpointId(provider.id, first.id);
          }
        } else if (prevType === "sensenova" && provider.models.length === 0) {
          const model = createImageGenerationModel(nextType, nextSeed());
          provider.models = [model];
        }
        if (nextType === "codex" && !provider.codexApiProviderId) provider.codexApiProviderId = codexApiProviders.value[0]?.id;
      } else {
        provider.providerType = nextType;
        if (provider.providerType === "codex" && !provider.codexApiProviderId) provider.codexApiProviderId = codexApiProviders.value[0]?.id;
      }
    }
    if (typeof values.providerName === "string") provider.name = values.providerName;
    if (typeof values.baseUrl === "string") provider.baseUrl = values.baseUrl;
    if (typeof values.codexApiProviderId === "string") provider.codexApiProviderId = values.codexApiProviderId;
    if (typeof values.timeoutSeconds === "number" && Number.isFinite(values.timeoutSeconds)) provider.timeoutSeconds = values.timeoutSeconds;
    if (typeof values.watermark === "boolean") provider.watermark = values.watermark;
  },
});

const providerTemplateGroups = computed<ConfigTemplateGroup[]>(() => {
  const provider = selectedProvider.value;
  if (!provider) return [];
  const providerTypeField = {
    key: "providerType",
    label: t("config.imageGeneration.providerType"),
    type: "select" as const,
    options: providerTypeOptions.map((item) => ({ value: item.value, label: item.label })),
  };
  const providerNameField = {
    key: "providerName",
    label: t("config.imageGeneration.providerName"),
    type: "text" as const,
  };
  const endpointField = provider.providerType === "codex"
    ? { key: "codexApiProviderId", label: t("config.imageGeneration.codexApiProvider"), description: t("config.imageGeneration.codexApiProviderHint"), type: "select" as const, options: [{ value: "", label: t("config.imageGeneration.codexApiProviderMissing") }, ...codexApiProviders.value.map((item) => ({ value: item.id, label: item.name }))] }
    : { key: "baseUrl", label: t("config.imageGeneration.baseUrl"), type: "text" as const, stacked: true };
  const timeoutField = {
    key: "timeoutSeconds",
    label: t("config.imageGeneration.timeoutSeconds"),
    type: "number" as const,
    min: 10,
    max: 600,
  };
  const rows: ConfigTemplateGroup["rows"] = [
    { items: [providerTypeField] },
    { items: [providerNameField] },
    { items: [timeoutField] },
    { items: [endpointField] },
  ];
  if (provider.providerType === "seedream") {
    rows.push({
      items: [{
        key: "watermark",
        label: t("config.imageGeneration.watermark"),
        description: t("config.imageGeneration.watermarkHint"),
        type: "toggle",
      }],
    });
  }
  return [{ title: t("config.imageGeneration.providerSettings"), rows }];
});

function imageConfigSnapshot(config: Partial<AppConfig>) {
  const imageProviders = normalizeImageGenerationProviders(config.imageProviders);
  return {
    imageGenerationModelId: normalizeImageGenerationModelId(config.imageGenerationModelId, imageProviders),
    imageProviders,
  };
}

const savedImageSnapshot = computed(() => {
  try {
    return imageConfigSnapshot(JSON.parse(String(props.lastSavedConfigJson || "{}")) as Partial<AppConfig>);
  } catch {
    return imageConfigSnapshot({ imageProviders: [] });
  }
});
const currentImageSnapshot = computed(() => imageConfigSnapshot(props.config));
const imageDirty = computed(() => (
  JSON.stringify(currentImageSnapshot.value) !== JSON.stringify(savedImageSnapshot.value)
));
const canRunImageTest = computed(() => (
  !imageDirty.value
  && !testingImage.value
  && testPrompt.value.trim().length > 0
  && !!testModelId.value
  && testModelOptions.value.some((option) => option.value === testModelId.value)
));

const workflowJsonError = computed(() => {
  const provider = selectedProvider.value;
  if (!provider || provider.providerType !== "comfyui") return "";
  const raw = provider.comfyuiWorkflowJson.trim();
  if (!raw) return t("config.imageGeneration.workflowRequired");
  try {
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return t("config.imageGeneration.workflowObjectRequired");
    }
    return "";
  } catch (error) {
    return t("config.imageGeneration.workflowInvalid", { error: error instanceof Error ? error.message : String(error) });
  }
});

const enabledWorkflowError = computed(() => {
  for (const provider of providers.value) {
    if (!provider.enabled || provider.deprecated || provider.providerType !== "comfyui") continue;
    if (!provider.comfyuiMapping.prompt.nodeIds.length || !provider.comfyuiMapping.prompt.inputKey.trim()) {
      return t("config.imageGeneration.promptMappingRequiredFor", { name: provider.name });
    }
    const raw = provider.comfyuiWorkflowJson.trim();
    if (!raw) return t("config.imageGeneration.workflowRequiredFor", { name: provider.name });
    try {
      const parsed = JSON.parse(raw);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        return t("config.imageGeneration.workflowRequiredFor", { name: provider.name });
      }
    } catch {
      return t("config.imageGeneration.workflowRequiredFor", { name: provider.name });
    }
  }
  for (const provider of providers.value) {
    if (!provider.enabled || provider.deprecated || provider.providerType !== "codex") continue;
    const linked = String(provider.codexApiProviderId || "").trim();
    if (!linked || !codexApiProviders.value.some((item) => item.id === linked)) {
      return t("config.imageGeneration.codexProviderRequired", { name: provider.name });
    }
  }
  return "";
});

const toolbarState = computed(() => ({
  providers: providers.value.map((provider) => ({
    id: provider.id,
    label: `${provider.name || provider.id}（${providerTypeLabel(provider.providerType)}）`,
  })),
  selectedProviderId: selectedProviderId.value,
  dirty: imageDirty.value,
  saving: props.savingConfig,
  removeDisabled: !selectedProvider.value || providers.value.length <= 1,
  restoreDisabled: !imageDirty.value || props.savingConfig,
  saveDisabled: !imageDirty.value || props.savingConfig || !!enabledWorkflowError.value,
}));

watch(
  providers,
  (value) => {
    const resolved = resolveSelectedImageProviderId(value, selectedProviderId.value);
    if (resolved === selectedProviderId.value) return;
    selectedProviderId.value = resolved;
  },
  { immediate: true },
);

watch(
  () => testModelOptions.value.map((option) => option.value).join("|"),
  () => {
    const options = testModelOptions.value.map((option) => option.value);
    if (options.length === 0) {
      testModelId.value = "";
      return;
    }
    if (testModelId.value && options.includes(testModelId.value)) return;
    const global = String(props.config.imageGenerationModelId || "").trim();
    if (global && options.includes(global)) {
      testModelId.value = global;
      return;
    }
    testModelId.value = options[0] || "";
  },
  { immediate: true },
);

function nextSeed(): string {
  localSeed += 1;
  return `${Date.now()}-${localSeed}`;
}

function providerTypeLabel(kind: ImageGenerationProviderKind): string {
  return providerTypeOptions.find((option) => option.value === kind)?.label || kind;
}

function clearInvalidDefaultModel() {
  props.config.imageGenerationModelId = normalizeImageGenerationModelId(
    props.config.imageGenerationModelId,
    providers.value,
  );
}

function addProvider() {
  const provider = createImageGenerationProvider("openai", nextSeed());
  if (provider.providerType === "codex") {
    provider.codexApiProviderId = codexApiProviders.value[0]?.id;
  }
  props.config.imageProviders.push(provider);
  selectedProviderId.value = provider.id;
  if (!props.config.imageGenerationModelId && provider.models[0]) {
    props.config.imageGenerationModelId = imageGenerationEndpointId(provider.id, provider.models[0].id);
  }
}

function removeSelectedProvider() {
  const provider = selectedProvider.value;
  if (!provider) return;
  if (!window.confirm(t("config.imageGeneration.confirmRemoveProvider", { name: provider.name }))) return;
  const index = props.config.imageProviders.findIndex((item) => item.id === provider.id);
  if (index >= 0) props.config.imageProviders.splice(index, 1);
  clearInvalidDefaultModel();
}

function updateSelectedApiKeys(apiKeys: string[]) {
  const provider = selectedProvider.value;
  if (!provider) return;
  provider.apiKeys = apiKeys;
}

function addModel() {
  const provider = selectedProvider.value;
  if (!provider) return;
  const model = createImageGenerationModel(provider.providerType, nextSeed());
  provider.models.push(model);
  if (!props.config.imageGenerationModelId && provider.enabled) {
    props.config.imageGenerationModelId = imageGenerationEndpointId(provider.id, model.id);
  }
}

function removeModel(modelId: string) {
  const provider = selectedProvider.value;
  if (!provider) return;
  const model = provider.models.find((item) => item.id === modelId);
  if (!model) return;
  if (!window.confirm(t("config.imageGeneration.confirmRemoveModel", { name: model.name || model.model || model.id }))) return;
  const index = provider.models.findIndex((item) => item.id === modelId);
  if (index >= 0) provider.models.splice(index, 1);
  clearInvalidDefaultModel();
}

function syncImageModelIdentifier(target: ImageGenerationModelConfigItem) {
  const provider = selectedProvider.value;
  if (!provider || !target) return;
  if (provider.providerType === "codex") {
    const forced = "gpt-5.6-luna";
    const prevEndpointForced = imageGenerationEndpointId(provider.id, target.id);
    target.model = forced;
    if (target.id !== forced && !provider.models.some((m) => m !== target && m.id.toLowerCase() === forced.toLowerCase())) {
      target.id = forced;
      if (props.config.imageGenerationModelId === prevEndpointForced) {
        props.config.imageGenerationModelId = imageGenerationEndpointId(provider.id, forced);
      }
    }
    return;
  }
  const raw = String(target.model || "").trim();
  if (!raw) return;
  if (raw.includes("::")) {
    props.setStatusAction(t("config.imageGeneration.invalidModelId"));
    return;
  }
  if (target.id === raw) return;
  if (provider.models.some((m) => m !== target && m.id.toLowerCase() === raw.toLowerCase())) {
    props.setStatusAction(t("config.imageGeneration.invalidModelId"));
    return;
  }
  const previousEndpointId = imageGenerationEndpointId(provider.id, target.id);
  target.id = raw;
  if (props.config.imageGenerationModelId === previousEndpointId) {
    props.config.imageGenerationModelId = imageGenerationEndpointId(provider.id, raw);
  }
}

function toggleImageModelPicker(modelId: string) {
  activeImageModelPickerId.value = activeImageModelPickerId.value === modelId ? "" : modelId;
  imageModelSearch.value = "";
}

function selectImageModel(previousId: string, value: string) {
  const provider = selectedProvider.value;
  const model = provider?.models.find((item) => item.id === previousId);
  if (!provider || !model || !value) return;
  const trimmed = String(value).trim();
  if (!trimmed || trimmed.includes("::")) {
    props.setStatusAction(t("config.imageGeneration.invalidModelId"));
    return;
  }
  if (provider.models.some((m) => m !== model && m.id.toLowerCase() === trimmed.toLowerCase())) {
    props.setStatusAction(t("config.imageGeneration.invalidModelId"));
    return;
  }
  const previousEndpointId = imageGenerationEndpointId(provider.id, model.id);
  model.model = trimmed;
  model.id = trimmed;
  if (props.config.imageGenerationModelId === previousEndpointId) {
    props.config.imageGenerationModelId = imageGenerationEndpointId(provider.id, trimmed);
  }
  activeImageModelPickerId.value = "";
  imageModelSearch.value = "";
}

function splitNodeIds(value: string): string[] {
  return value.split(/[，,\s]+/).map((item) => item.trim()).filter(Boolean);
}

function setMappingNodeIds(key: ComfyInputMappingKey, value: string) {
  if (!selectedProvider.value) return;
  selectedProvider.value.comfyuiMapping[key].nodeIds = splitNodeIds(value);
}

function setOutputNodeIds(value: string) {
  if (!selectedProvider.value) return;
  selectedProvider.value.comfyuiMapping.outputNodeIds = splitNodeIds(value);
}

function restoreImageConfig() {
  props.config.imageProviders = normalizeImageGenerationProviders(savedImageSnapshot.value.imageProviders);
  props.config.imageGenerationModelId = savedImageSnapshot.value.imageGenerationModelId;
  props.setStatusAction(t("config.imageGeneration.restored"));
}

async function saveImageConfig() {
  if (!imageDirty.value) return;
  if (enabledWorkflowError.value) {
    props.setStatusAction(enabledWorkflowError.value);
    return;
  }
  const saved = await Promise.resolve(props.saveConfigAction());
  if (!saved) return;
  props.setStatusAction(t("config.imageGeneration.saved"));
}

defineExpose({
  toolbarState,
  addProvider,
  removeSelectedProvider,
  restoreImageConfig,
  saveImageConfig,
});

async function runImageTest() {
  if (!canRunImageTest.value) return;
  testingImage.value = true;
  testError.value = "";
  testResult.value = null;
  testPreviewDataUrl.value = "";
  const request: Record<string, unknown> = {
    prompt: testPrompt.value.trim(),
    n: 1,
    modelId: testModelId.value,
  };
  const resolution = testResolution.value.trim();
  if (resolution) request.size = resolution;
  try {
    const result = await invokeTauri<ImageGenerationResult>("generate_image", { request });
    testResult.value = result;
    const firstImage = result.images?.[0];
    if (firstImage?.relativePath) {
      // 后端预览命令只认 {Assistant Space} 前缀，裸相对路径会按进程工作目录解析导致找不到文件
      const preview = await readTransportChatImage({
        path: `{Assistant Space}/${firstImage.relativePath}`,
        maxEdge: 768,
      });
      testPreviewDataUrl.value = String(preview?.dataUrl || "");
    }
    props.setStatusAction(t("config.imageGeneration.testCompleted"));
  } catch (error) {
    testError.value = String(error instanceof Error ? error.message : error || t("config.imageGeneration.testFailed"));
    props.setStatusAction(t("config.imageGeneration.testFailed"));
  } finally {
    testingImage.value = false;
  }
}

async function copyGeneratedMarkdown(markdown: string) {
  const value = String(markdown || "").trim();
  if (!value) return;
  try {
    await navigator.clipboard.writeText(value);
    props.setStatusAction(t("config.imageGeneration.markdownCopied"));
  } catch (error) {
    testError.value = String(error instanceof Error ? error.message : error || t("config.imageGeneration.copyFailed"));
  }
}

async function copyGeneratedImage(relativePath: string) {
  const value = String(relativePath || "").trim();
  if (!value || copyingImage.value) return;
  copyingImage.value = true;
  try {
    await copyTransportChatImageToClipboard(`{Assistant Space}/${value}`);
    props.setStatusAction(t("config.imageGeneration.imageCopied"));
  } catch (error) {
    testError.value = String(error instanceof Error ? error.message : error || t("config.imageGeneration.copyFailed"));
  } finally {
    copyingImage.value = false;
  }
}
</script>
