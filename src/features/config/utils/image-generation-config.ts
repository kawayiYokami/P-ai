import type {
  AppConfig,
  ComfyUiNodeInputMapping,
  ComfyUiWorkflowMapping,
  ImageGenerationModelConfigItem,
  ImageGenerationModelOption,
  ImageGenerationProviderConfigItem,
  ImageGenerationProviderKind,
} from "../../../types/app";

export const CODEX_IMAGE_MAIN_MODEL = "gpt-5.6-luna";

type ProviderTemplate = {
  name: string;
  baseUrl: string;
  model: Omit<ImageGenerationModelConfigItem, "id" | "enabled" | "deprecated">;
};

const PROVIDER_TEMPLATES: Record<ImageGenerationProviderKind, ProviderTemplate> = {
  comfyui: {
    name: "Local ComfyUI",
    baseUrl: "http://127.0.0.1:8188",
    model: {
      name: "ComfyUI Workflow",
      model: "",
      defaultSize: "512x512",
      defaultAspectRatio: "1:1",
    },
  },
  codex: {
    name: "OpenAI Codex Image Generation",
    baseUrl: "https://chatgpt.com/backend-api/codex",
    model: {
      name: "Codex Image Generation",
      model: CODEX_IMAGE_MAIN_MODEL,
      defaultSize: "512x512",
      defaultAspectRatio: "1:1",
      defaultQuality: "medium",
    },
  },
  openai: {
    name: "OpenAI Images",
    baseUrl: "https://api.openai.com/v1",
    model: {
      name: "GPT Image 2",
      model: "gpt-image-2",
      defaultSize: "512x512",
      defaultAspectRatio: "1:1",
      defaultQuality: "medium",
    },
  },
  xai: {
    name: "xAI Grok Imagine",
    baseUrl: "https://api.x.ai/v1",
    model: {
      name: "Grok Imagine Image Quality",
      model: "grok-imagine-image-quality",
      defaultSize: "512x512",
      defaultAspectRatio: "1:1",
    },
  },
  seedream: {
    name: "Seedance / Seedream",
    baseUrl: "https://ark.cn-beijing.volces.com/api/v3",
    model: {
      name: "Seedream 5.0 Pro",
      model: "doubao-seedream-5-0-pro-260628",
      defaultSize: "512x512",
      defaultAspectRatio: "1:1",
      defaultQuality: "standard",
    },
  },
  gemini: {
    name: "Gemini Nano Banana",
    baseUrl: "https://generativelanguage.googleapis.com/v1beta",
    model: {
      name: "Nano Banana 2",
      model: "gemini-3.1-flash-image",
      defaultSize: "512x512",
      defaultAspectRatio: "1:1",
    },
  },
  sensenova: {
    name: "商汤科技 · SenseNova",
    baseUrl: "https://token.sensenova.cn/v1",
    model: {
      name: "SenseNova U1 Fast",
      model: "sensenova-u1-fast",
      defaultSize: "1024x1024",
      defaultAspectRatio: "1:1",
    },
  },
};

const PROVIDER_KINDS = new Set<ImageGenerationProviderKind>([
  "comfyui",
  "codex",
  "openai",
  "xai",
  "seedream",
  "gemini",
  "sensenova",
]);

function normalizedOptionalText(value: unknown): string | undefined {
  const text = String(value ?? "").trim();
  return text || undefined;
}

function normalizeProviderKind(value: unknown): ImageGenerationProviderKind {
  const kind = String(value ?? "").trim().toLowerCase() as ImageGenerationProviderKind;
  return PROVIDER_KINDS.has(kind) ? kind : "openai";
}

function normalizeUniqueStrings(value: unknown, caseInsensitive = false): string[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  const result: string[] = [];
  for (const item of value) {
    const text = String(item ?? "").trim();
    if (!text) continue;
    const key = caseInsensitive ? text.toLowerCase() : text;
    if (seen.has(key)) continue;
    seen.add(key);
    result.push(text);
  }
  return result;
}

function normalizeNodeMapping(value: unknown, inputKey: string): ComfyUiNodeInputMapping {
  const mapping = value && typeof value === "object"
    ? value as Partial<ComfyUiNodeInputMapping>
    : {};
  return {
    nodeIds: normalizeUniqueStrings(mapping.nodeIds, true),
    inputKey: normalizedOptionalText(mapping.inputKey) || inputKey,
  };
}

export function createDefaultComfyUiMapping(): ComfyUiWorkflowMapping {
  return {
    prompt: { nodeIds: [], inputKey: "text" },
    negativePrompt: { nodeIds: [], inputKey: "text" },
    model: { nodeIds: [], inputKey: "ckpt_name" },
    width: { nodeIds: [], inputKey: "width" },
    height: { nodeIds: [], inputKey: "height" },
    seed: { nodeIds: [], inputKey: "seed" },
    steps: { nodeIds: [], inputKey: "steps" },
    inputImage: { nodeIds: [], inputKey: "image" },
    maskImage: { nodeIds: [], inputKey: "image" },
    outputNodeIds: [],
  };
}

export function normalizeComfyUiMapping(value: unknown): ComfyUiWorkflowMapping {
  const mapping = value && typeof value === "object"
    ? value as Partial<ComfyUiWorkflowMapping>
    : {};
  return {
    prompt: normalizeNodeMapping(mapping.prompt, "text"),
    negativePrompt: normalizeNodeMapping(mapping.negativePrompt, "text"),
    model: normalizeNodeMapping(mapping.model, "ckpt_name"),
    width: normalizeNodeMapping(mapping.width, "width"),
    height: normalizeNodeMapping(mapping.height, "height"),
    seed: normalizeNodeMapping(mapping.seed, "seed"),
    steps: normalizeNodeMapping(mapping.steps, "steps"),
    inputImage: normalizeNodeMapping(mapping.inputImage, "image"),
    maskImage: normalizeNodeMapping(mapping.maskImage, "image"),
    outputNodeIds: normalizeUniqueStrings(mapping.outputNodeIds, true),
  };
}

function defaultModelId(kind: ImageGenerationProviderKind, seed: string): string {
  const model = PROVIDER_TEMPLATES[kind].model.model;
  if (model) return model;
  return `comfyui-workflow-${seed}`;
}

export function createImageGenerationProvider(
  providerType: ImageGenerationProviderKind = "openai",
  seed = Date.now().toString(),
): ImageGenerationProviderConfigItem {
  const template = PROVIDER_TEMPLATES[providerType];
  const baseProvider: ImageGenerationProviderConfigItem = {
    id: `image-provider-${providerType}-${seed}`,
    name: template.name,
    providerType,
    enabled: true,
    deprecated: false,
    baseUrl: template.baseUrl,
    apiKeys: [],
    codexApiProviderId: undefined,
    keyCursor: 0,
    timeoutSeconds: 600,
    watermark: false,
    models: [{
      id: defaultModelId(providerType, seed),
      name: template.model.name,
      model: template.model.model,
      enabled: true,
      deprecated: false,
      defaultSize: template.model.defaultSize,
      defaultAspectRatio: template.model.defaultAspectRatio,
      defaultQuality: template.model.defaultQuality,
    }],
    comfyuiWorkflowJson: "",
    comfyuiMapping: createDefaultComfyUiMapping(),
  };
  if (providerType === "sensenova") {
    baseProvider.models = [
      {
        id: "sensenova-u1-fast",
        name: "SenseNova U1 Fast",
        model: "sensenova-u1-fast",
        enabled: true,
        deprecated: false,
        defaultSize: template.model.defaultSize,
        defaultAspectRatio: template.model.defaultAspectRatio,
        defaultQuality: template.model.defaultQuality,
      },
      {
        id: "sensenova-u1.5-lite",
        name: "SenseNova U1.5 Lite",
        model: "sensenova-u1.5-lite",
        enabled: true,
        deprecated: false,
        defaultSize: template.model.defaultSize,
        defaultAspectRatio: template.model.defaultAspectRatio,
        defaultQuality: template.model.defaultQuality,
      },
    ];
  }
  return baseProvider;
}

export function createImageGenerationModel(
  providerType: ImageGenerationProviderKind,
  seed = Date.now().toString(),
): ImageGenerationModelConfigItem {
  const template = PROVIDER_TEMPLATES[providerType].model;
  return {
    id: `${defaultModelId(providerType, seed)}-${seed}`,
    name: template.name,
    model: template.model,
    enabled: true,
    deprecated: false,
    defaultSize: template.defaultSize,
    defaultAspectRatio: template.defaultAspectRatio,
    defaultQuality: template.defaultQuality,
  };
}

function normalizeModel(
  value: unknown,
  providerType: ImageGenerationProviderKind,
  fallbackId: string,
): ImageGenerationModelConfigItem | null {
  const model = value && typeof value === "object"
    ? value as Partial<ImageGenerationModelConfigItem>
    : {};
  const id = String(model.id ?? fallbackId).trim();
  if (!id || id.includes("::")) return null;
  const modelValue = providerType === "codex"
    ? CODEX_IMAGE_MAIN_MODEL
    : String(model.model ?? "").trim();
  const fallbackName = modelValue || id;
  return {
    id,
    name: String(model.name ?? "").trim() || fallbackName,
    model: modelValue || (providerType === "comfyui" ? "" : id),
    enabled: model.enabled !== false,
    deprecated: !!model.deprecated,
    defaultSize: normalizedOptionalText(model.defaultSize),
    defaultAspectRatio: normalizedOptionalText(model.defaultAspectRatio),
    defaultQuality: normalizedOptionalText(model.defaultQuality),
  };
}

export function normalizeImageGenerationProvider(
  value: unknown,
  index = 0,
): ImageGenerationProviderConfigItem | null {
  const source = value && typeof value === "object"
    ? value as Partial<ImageGenerationProviderConfigItem>
    : {};
  const providerType = normalizeProviderKind(source.providerType);
  const template = PROVIDER_TEMPLATES[providerType];
  const id = String(source.id ?? `image-provider-${providerType}-${index + 1}`).trim();
  if (!id || id.includes("::")) return null;
  const seenModels = new Set<string>();
  const models = (Array.isArray(source.models) ? source.models : [])
    .map((model, modelIndex) => normalizeModel(model, providerType, `image-model-${modelIndex + 1}`))
    .filter((model): model is ImageGenerationModelConfigItem => {
      if (!model) return false;
      const key = model.id.toLowerCase();
      if (seenModels.has(key)) return false;
      seenModels.add(key);
      return true;
    });
  const timeout = Math.round(Number(source.timeoutSeconds ?? 600));
  return {
    id,
    name: String(source.name ?? "").trim() || template.name,
    providerType,
    enabled: source.enabled !== false,
    deprecated: !!source.deprecated,
    baseUrl: String(source.baseUrl ?? "").trim().replace(/\/+$/, "") || template.baseUrl,
    apiKeys: normalizeUniqueStrings(source.apiKeys),
    codexApiProviderId: normalizedOptionalText(source.codexApiProviderId),
    keyCursor: Math.max(0, Math.min(1_000_000, Math.round(Number(source.keyCursor ?? 0)) || 0)),
    timeoutSeconds: Number.isFinite(timeout) && timeout > 0
      ? Math.max(10, Math.min(600, timeout))
      : 600,
    watermark: !!source.watermark,
    models,
    comfyuiWorkflowJson: String(source.comfyuiWorkflowJson ?? "").trim(),
    comfyuiMapping: normalizeComfyUiMapping(source.comfyuiMapping),
  };
}

export function normalizeImageGenerationProviders(value: unknown): ImageGenerationProviderConfigItem[] {
  if (!Array.isArray(value)) return [];
  const seen = new Set<string>();
  return value
    .map((provider, index) => normalizeImageGenerationProvider(provider, index))
    .filter((provider): provider is ImageGenerationProviderConfigItem => {
      if (!provider) return false;
      const key = provider.id.toLowerCase();
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
}

export function cloneImageGenerationProvider(
  provider: ImageGenerationProviderConfigItem,
): ImageGenerationProviderConfigItem {
  return {
    ...provider,
    apiKeys: [...provider.apiKeys],
    models: provider.models.map((model) => ({ ...model })),
    comfyuiMapping: {
      prompt: { ...provider.comfyuiMapping.prompt, nodeIds: [...provider.comfyuiMapping.prompt.nodeIds] },
      negativePrompt: { ...provider.comfyuiMapping.negativePrompt, nodeIds: [...provider.comfyuiMapping.negativePrompt.nodeIds] },
      model: { ...provider.comfyuiMapping.model, nodeIds: [...provider.comfyuiMapping.model.nodeIds] },
      width: { ...provider.comfyuiMapping.width, nodeIds: [...provider.comfyuiMapping.width.nodeIds] },
      height: { ...provider.comfyuiMapping.height, nodeIds: [...provider.comfyuiMapping.height.nodeIds] },
      seed: { ...provider.comfyuiMapping.seed, nodeIds: [...provider.comfyuiMapping.seed.nodeIds] },
      steps: { ...provider.comfyuiMapping.steps, nodeIds: [...provider.comfyuiMapping.steps.nodeIds] },
      inputImage: { ...provider.comfyuiMapping.inputImage, nodeIds: [...provider.comfyuiMapping.inputImage.nodeIds] },
      maskImage: { ...provider.comfyuiMapping.maskImage, nodeIds: [...provider.comfyuiMapping.maskImage.nodeIds] },
      outputNodeIds: [...provider.comfyuiMapping.outputNodeIds],
    },
  };
}

export function imageGenerationEndpointId(providerId: string, modelId: string): string {
  return `${providerId.trim()}::${modelId.trim()}`;
}

export function deriveImageGenerationModelOptions(
  providers: ImageGenerationProviderConfigItem[],
): ImageGenerationModelOption[] {
  const options: ImageGenerationModelOption[] = [];
  for (const provider of providers) {
    if (!provider.enabled || provider.deprecated) continue;
    for (const model of provider.models) {
      if (!model.enabled || model.deprecated) continue;
      const id = imageGenerationEndpointId(provider.id, model.id);
      options.push({
        id,
        providerId: provider.id,
        providerName: provider.name,
        providerType: provider.providerType,
        modelId: model.id,
        model: model.model,
        name: model.name,
        label: `${provider.name} / ${model.name}`,
      });
    }
  }
  return options;
}

export function normalizeImageGenerationModelId(
  value: unknown,
  providers: ImageGenerationProviderConfigItem[],
): string | undefined {
  const modelId = String(value ?? "").trim();
  if (!modelId) return undefined;
  return deriveImageGenerationModelOptions(providers).some((option) => option.id === modelId)
    ? modelId
    : undefined;
}

// 选中项解析：目标仍在列表里就沿用，否则回退首项；列表为空则清空
export function resolveSelectedImageProviderId(
  providers: ImageGenerationProviderConfigItem[],
  currentId: string,
): string {
  if (providers.some((provider) => provider.id === currentId)) return currentId;
  return providers[0]?.id || "";
}

export function normalizeImageGenerationConfig(config: Pick<AppConfig, "imageGenerationModelId" | "imageProviders">): void {
  config.imageProviders = normalizeImageGenerationProviders(config.imageProviders);
  config.imageGenerationModelId = normalizeImageGenerationModelId(
    config.imageGenerationModelId,
    config.imageProviders,
  );
}

export function imageGenerationProviderTemplate(
  providerType: ImageGenerationProviderKind,
): ProviderTemplate {
  return PROVIDER_TEMPLATES[providerType];
}
