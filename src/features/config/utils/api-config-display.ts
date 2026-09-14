import type { ApiConfigItem } from "../../../types/app";

export const LEGAL_REASONING_EFFORTS = [
  "default",
  "none",
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
] as const;

export type LegalReasoningEffort = (typeof LEGAL_REASONING_EFFORTS)[number];

/** codex 供应商固定支持的思考等级：保存时按「模型 × 全部等级」落盘，不提供界面配置 */
export const CODEX_REASONING_EFFORTS = ["low", "medium", "high", "xhigh", "max"] as const;

type TranslateFn = (key: string, params?: Record<string, unknown>) => string;
type ApiConfigDisplayOptions = {
  providerMaxCharacters?: number;
};

const REASONING_SUFFIX_PATTERN = /\s*·\s*(默认|不思考|最小|低|中|高|极高|最大|Default|Off|Minimal|Low|Medium|High|Extra High|XHigh|Max)$/i;

function compactText(value: string, maxCharacters?: number): string {
  if (!maxCharacters || maxCharacters < 1) return value;
  return Array.from(value).slice(0, maxCharacters).join("");
}

function providerModelDisplayName(
  providerName: string,
  modelValue: string,
  providerMaxCharacters?: number,
): string {
  const provider = compactText(String(providerName || "").trim(), providerMaxCharacters);
  const model = String(modelValue || "").trim();
  return provider && model ? `${provider} · ${model}` : (provider || model);
}

function formatProviderModelDisplayName(
  name: string,
  modelValue: string,
  providerMaxCharacters?: number,
): string {
  const base = String(name || "").trim();
  const model = String(modelValue || "").trim();
  if (!base || !model || base === model) return base;

  const displaySuffix = ` · ${model}`;
  if (base.endsWith(displaySuffix)) {
    return providerModelDisplayName(base.slice(0, -displaySuffix.length), model, providerMaxCharacters);
  }

  // 兼容历史配置中以“供应商/模型”保存的名称。
  const legacySuffix = `/${model}`;
  return base.endsWith(legacySuffix)
    ? providerModelDisplayName(base.slice(0, -legacySuffix.length), model, providerMaxCharacters)
    : base;
}

export function normalizeReasoningEffortValue(value: unknown): string {
  return String(value || "").trim().toLowerCase();
}

export function isLegalReasoningEffort(value: unknown): value is LegalReasoningEffort {
  const normalized = normalizeReasoningEffortValue(value);
  return (LEGAL_REASONING_EFFORTS as readonly string[]).includes(normalized);
}

/** 固定档位顺序：default → none → minimal → low → medium → high → xhigh → max；未知值置后保持相对顺序。 */
export function sortReasoningEffortValues(values: Iterable<string>): string[] {
  const unique: string[] = [];
  for (const raw of values) {
    const value = normalizeReasoningEffortValue(raw);
    if (!value || unique.includes(value)) continue;
    unique.push(value);
  }
  const rank = new Map<string, number>(
    LEGAL_REASONING_EFFORTS.map((item, index) => [item, index]),
  );
  return unique.sort((left, right) => {
    const leftRank = rank.get(left);
    const rightRank = rank.get(right);
    if (leftRank != null && rightRank != null) return leftRank - rightRank;
    if (leftRank != null) return -1;
    if (rightRank != null) return 1;
    return left.localeCompare(right);
  });
}

export function reasoningEffortDisplayLabel(
  value: unknown,
  t?: TranslateFn,
): string {
  const normalized = normalizeReasoningEffortValue(value);
  if (normalized === "default") {
    return t ? t("config.api.reasoningDefault") : "默认";
  }
  if (normalized === "none") {
    return t ? t("config.api.reasoningOff") : "不思考";
  }
  if (normalized === "minimal") {
    return t ? t("config.api.reasoningMinimal") : "最小";
  }
  if (normalized === "low") {
    return t ? t("config.api.reasoningLow") : "低";
  }
  if (normalized === "medium") {
    return t ? t("config.api.reasoningMedium") : "中";
  }
  if (normalized === "high") {
    return t ? t("config.api.reasoningHigh") : "高";
  }
  if (normalized === "xhigh") {
    return t ? t("config.api.reasoningXHigh") : "极高";
  }
  if (normalized === "max") {
    return t ? t("config.api.reasoningMax") : "最大";
  }
  return "";
}

export function stripReasoningEffortDisplaySuffix(name: string): string {
  return String(name || "").replace(REASONING_SUFFIX_PATTERN, "").trim();
}

export function apiConfigDisplayName(
  providerName: string,
  modelValue: string,
  reasoningEffort: unknown,
  t?: TranslateFn,
): string {
  const provider = String(providerName || "").trim();
  const model = String(modelValue || "").trim();
  const base = provider && model
    ? `${provider}/${model}`
    : (provider || model);
  const label = reasoningEffortDisplayLabel(reasoningEffort, t);
  if (!base) return label;
  return label ? `${base} · ${label}` : base;
}

/** 聊天下拉/按钮优先按 reasoningEffort 现算，避免历史 name 偶发缺后缀。 */
export function formatApiConfigOptionLabel(
  item: Pick<ApiConfigItem, "name" | "model" | "displayName" | "reasoningEffort"> | null | undefined,
  t?: TranslateFn,
  options?: ApiConfigDisplayOptions,
): string {
  if (!item) return "";
  const model = String(item.displayName || item.model || "").trim();
  const rawName = String(item.name || "").trim();
  const baseFromName = stripReasoningEffortDisplaySuffix(rawName);
  let base = baseFromName;
  if (model) {
    if (!base) {
      base = model;
    } else if (!base.includes("/") && base !== model) {
      // name 异常时尽量保住 model
      base = model;
    } else if (base.endsWith(`/${model}`) || base === model) {
      // already good
    } else if (!base.includes(model)) {
      // name 与 model 不一致时，以 provider/model 形态优先
      const provider = base.includes("/") ? base.slice(0, base.lastIndexOf("/")) : base;
      base = provider ? `${provider}/${model}` : model;
    }
  }
  if (!base) base = rawName || model;
  const label = reasoningEffortDisplayLabel(item.reasoningEffort, t);
  const displayBase = formatProviderModelDisplayName(
    base,
    model,
    options?.providerMaxCharacters,
  );
  return label ? `${displayBase} · ${label}` : displayBase;
}

/** 卡片概览展示端点：去掉默认的 https:// 前缀降噪，保留 http:// 以暴露非加密端点。 */
export function formatEndpointDisplay(url: unknown): string {
  return String(url || "").trim().replace(/^https:\/\//i, "");
}
