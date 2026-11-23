export const FALLBACK_REASONING_EFFORT_OPTIONS = [
  "none",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
] as const;

export type ReasoningCapability = {
  supportsReasoning: boolean;
  reasoningEffortOptions: string[];
};

type ReasoningOptionItem = {
  type?: unknown;
  values?: unknown;
};

type ReasoningCapabilityInput = {
  metadataFound?: unknown;
  fallbackReasoningEffortOptions?: unknown;
  reasoning?: unknown;
  reasoningEffortOptions?: unknown;
  reasoningOptions?: unknown;
};

export type ModelCapabilitySnapshot = {
  fuzzyMatch?: boolean;
  providerName?: string;
  providerApi?: string;
  contextWindowMax?: number;
  maxOutputTokensMax?: number;
  enableImage?: boolean;
  enableVideo?: boolean;
  enableAudio?: boolean;
  enableTools?: boolean;
  documentationUrl?: string;
  reasoning: ReasoningCapability;
};

function normalizeReasoningEffortValue(value: unknown): string {
  return String(value || "").trim().toLowerCase();
}

function normalizeReasoningEffortList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return Array.from(
    new Set(
      value
        .map((item) => normalizeReasoningEffortValue(item))
        .filter(Boolean),
    ),
  );
}

function collectReasoningOptionTypes(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const collected: string[] = [];
  for (const item of value) {
    const entry = item as ReasoningOptionItem;
    const type = normalizeReasoningEffortValue(entry?.type);
    if (!type || collected.includes(type)) continue;
    collected.push(type);
  }
  return collected;
}

function collectReasoningEffortOptions(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  const collected: string[] = [];
  for (const item of value) {
    const entry = item as ReasoningOptionItem;
    const type = normalizeReasoningEffortValue(entry?.type);
    if (type !== "effort") continue;
    collected.push(...normalizeReasoningEffortList(entry?.values));
  }
  return Array.from(new Set(collected));
}

function prependDefaultReasoningOption(options: string[]): string[] {
  return ["default", ...options.filter((item) => item !== "default")];
}

function normalizeDocumentationUrl(value: unknown): string | undefined {
  const normalized = String(value || "").trim();
  return normalized || undefined;
}

export function buildReasoningCapability(input: ReasoningCapabilityInput | null | undefined): ReasoningCapability {
  if (input?.metadataFound === false) {
    return {
      supportsReasoning: true,
      reasoningEffortOptions: prependDefaultReasoningOption(
        normalizeReasoningEffortList(input?.fallbackReasoningEffortOptions).length > 0
          ? normalizeReasoningEffortList(input?.fallbackReasoningEffortOptions)
          : [...FALLBACK_REASONING_EFFORT_OPTIONS],
      ),
    };
  }

  const supportsReasoning = input?.reasoning === true;
  if (!supportsReasoning) {
    return {
      supportsReasoning: false,
      reasoningEffortOptions: [],
    };
  }

  const normalizedReasoningEffortOptions = normalizeReasoningEffortList(input?.reasoningEffortOptions);
  if (normalizedReasoningEffortOptions.length > 0) {
    return {
      supportsReasoning: true,
      reasoningEffortOptions: prependDefaultReasoningOption(normalizedReasoningEffortOptions),
    };
  }

  const collectedReasoningEffortOptions = collectReasoningEffortOptions(input?.reasoningOptions);
  if (collectedReasoningEffortOptions.length > 0) {
    return {
      supportsReasoning: true,
      reasoningEffortOptions: prependDefaultReasoningOption(collectedReasoningEffortOptions),
    };
  }

  const reasoningOptionTypes = collectReasoningOptionTypes(input?.reasoningOptions);
  if (reasoningOptionTypes.includes("toggle") || reasoningOptionTypes.length === 0) {
    return {
      supportsReasoning: true,
      reasoningEffortOptions: ["default"],
    };
  }

  return {
    supportsReasoning: true,
    reasoningEffortOptions: ["default"],
  };
}

export function buildModelCapability(input: {
  metadataFound?: unknown;
  fallbackReasoningEffortOptions?: unknown;
  contextWindowTokens?: unknown;
  maxOutputTokens?: unknown;
  enableImage?: unknown;
  enableVideo?: unknown;
  enableAudio?: unknown;
  enableTools?: unknown;
  documentationUrl?: unknown;
  reasoning?: unknown;
  reasoningEffortOptions?: unknown;
  reasoningOptions?: unknown;
}): ModelCapabilitySnapshot {
  return {
    contextWindowMax: Number.isFinite(Number(input.contextWindowTokens)) ? Number(input.contextWindowTokens) : undefined,
    maxOutputTokensMax: Number.isFinite(Number(input.maxOutputTokens)) ? Number(input.maxOutputTokens) : undefined,
    enableImage: typeof input.enableImage === "boolean" ? input.enableImage : undefined,
    enableVideo: typeof input.enableVideo === "boolean" ? input.enableVideo : undefined,
    enableAudio: typeof input.enableAudio === "boolean" ? input.enableAudio : undefined,
    enableTools: typeof input.enableTools === "boolean" ? input.enableTools : undefined,
    documentationUrl: normalizeDocumentationUrl(input.documentationUrl),
    reasoning: buildReasoningCapability({
      metadataFound: input.metadataFound,
      fallbackReasoningEffortOptions: input.fallbackReasoningEffortOptions,
      reasoning: input.reasoning,
      reasoningEffortOptions: input.reasoningEffortOptions,
      reasoningOptions: input.reasoningOptions,
    }),
  };
}

/**
 * 只有文本能力才有「上下文窗口」这个参数。
 * 嵌入/重排/语音/图像的模型卡不显示它，草稿不该算它、不该夹它、比对也不该看它。
 */
export function capabilityHasContextWindow(capability: string): boolean {
  return capability === "text";
}
