import type {
  RemoteImChannelBehaviorSettings,
  RemoteImContact,
  RemoteImGroupReplyPacing,
} from "../../../../../types/app";

export const DEFAULT_REMOTE_IM_GROUP_REPLY_PACING: RemoteImGroupReplyPacing = {
  assistantDebounceSeconds: 1,
  secretaryInspectionSeconds: 60,
  replyCooldownSeconds: 10,
  inspectionJitterRatio: 0.2,
  maximumEnergy: 100,
  baseReplyEnergyCost: 14,
  energyCostPerCharacter: 0.12,
  energyRecoveryPerSecond: 0.6,
  positiveEnergyPhrases: ["厉害", "像人"],
  negativeEnergyPhrases: ["够了", "烦", "串了"],
  positiveEnergyDelta: 6,
  negativeEnergyDelta: -15,
  normalReplyMaxChars: 20,
  focusReplyMaxChars: 200,
  focusInstructions: ["分析", "总结", "好好想想", "为什么", "到底"],
};

export const DEFAULT_REMOTE_IM_CHANNEL_BEHAVIOR_SETTINGS: RemoteImChannelBehaviorSettings = {
  responseGuidance: "",
  blockedMessagePrefixes: ["#", "/", "%"],
  muteKeywords: ["闭嘴"],
  unmuteKeywords: ["张嘴"],
  patienceSeconds: 60,
  muteDurationSeconds: 600,
  groupReplyPacing: DEFAULT_REMOTE_IM_GROUP_REPLY_PACING,
};

type PartialChannelBehaviorSettings = Omit<
  Partial<RemoteImChannelBehaviorSettings>,
  "groupReplyPacing"
> & {
  groupReplyPacing?: Partial<RemoteImGroupReplyPacing> | null;
};

export function normalizeGroupReplyPacing(
  value?: Partial<RemoteImGroupReplyPacing> | null,
): RemoteImGroupReplyPacing {
  const defaults = DEFAULT_REMOTE_IM_GROUP_REPLY_PACING;
  const numberValue = (candidate: unknown, fallback: number) => {
    const parsed = Number(candidate);
    return Number.isFinite(parsed) ? parsed : fallback;
  };
  return {
    assistantDebounceSeconds: Math.max(1, Math.round(numberValue(value?.assistantDebounceSeconds, defaults.assistantDebounceSeconds))),
    secretaryInspectionSeconds: Math.max(1, Math.round(numberValue(value?.secretaryInspectionSeconds, defaults.secretaryInspectionSeconds))),
    replyCooldownSeconds: Math.max(0, Math.round(numberValue(value?.replyCooldownSeconds, defaults.replyCooldownSeconds))),
    inspectionJitterRatio: Math.min(1, Math.max(0, numberValue(value?.inspectionJitterRatio, defaults.inspectionJitterRatio))),
    maximumEnergy: Math.max(0.01, numberValue(value?.maximumEnergy, defaults.maximumEnergy)),
    baseReplyEnergyCost: Math.max(0, numberValue(value?.baseReplyEnergyCost, defaults.baseReplyEnergyCost)),
    energyCostPerCharacter: Math.max(0, numberValue(value?.energyCostPerCharacter, defaults.energyCostPerCharacter)),
    energyRecoveryPerSecond: Math.max(0, numberValue(value?.energyRecoveryPerSecond, defaults.energyRecoveryPerSecond)),
    positiveEnergyPhrases: Array.isArray(value?.positiveEnergyPhrases)
      ? [...value.positiveEnergyPhrases]
      : [...defaults.positiveEnergyPhrases],
    negativeEnergyPhrases: Array.isArray(value?.negativeEnergyPhrases)
      ? [...value.negativeEnergyPhrases]
      : [...defaults.negativeEnergyPhrases],
    positiveEnergyDelta: Math.max(0, numberValue(value?.positiveEnergyDelta, defaults.positiveEnergyDelta)),
    negativeEnergyDelta: Math.min(0, numberValue(value?.negativeEnergyDelta, defaults.negativeEnergyDelta)),
    normalReplyMaxChars: Math.max(1, Math.round(numberValue(value?.normalReplyMaxChars, defaults.normalReplyMaxChars))),
    focusReplyMaxChars: Math.max(1, Math.round(numberValue(value?.focusReplyMaxChars, defaults.focusReplyMaxChars))),
    focusInstructions: Array.isArray(value?.focusInstructions)
      ? [...value.focusInstructions]
      : [...defaults.focusInstructions],
  };
}

export function normalizeChannelBehaviorSettings(
  value?: PartialChannelBehaviorSettings | null,
): RemoteImChannelBehaviorSettings {
  const defaults = DEFAULT_REMOTE_IM_CHANNEL_BEHAVIOR_SETTINGS;
  const numberValue = (candidate: unknown, fallback: number) => {
    const parsed = Number(candidate);
    return Number.isFinite(parsed) ? parsed : fallback;
  };
  const normalizeList = (candidate: unknown, fallback: string[]) => {
    const values = Array.isArray(candidate) ? candidate : fallback;
    return parseSpaceSeparatedList(values.map((item) => String(item || "")).join(" "));
  };
  return {
    responseGuidance: String(value?.responseGuidance || "").trim(),
    blockedMessagePrefixes: normalizeList(value?.blockedMessagePrefixes, defaults.blockedMessagePrefixes),
    muteKeywords: normalizeList(value?.muteKeywords, defaults.muteKeywords),
    unmuteKeywords: normalizeList(value?.unmuteKeywords, defaults.unmuteKeywords),
    patienceSeconds: Math.max(0, Math.round(numberValue(value?.patienceSeconds, defaults.patienceSeconds))),
    muteDurationSeconds: Math.max(0, Math.round(numberValue(value?.muteDurationSeconds, defaults.muteDurationSeconds))),
    groupReplyPacing: normalizeGroupReplyPacing(value?.groupReplyPacing),
  };
}

export function cloneChannelBehaviorSettings(
  value?: PartialChannelBehaviorSettings | null,
): RemoteImChannelBehaviorSettings {
  return normalizeChannelBehaviorSettings(value);
}

export function parseSpaceSeparatedList(raw: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const value of String(raw || "").split(/\s+/)) {
    const item = value.trim();
    if (!item || seen.has(item)) continue;
    seen.add(item);
    out.push(item);
  }
  return out;
}

export function resolveBehaviorDraftSave<T>(
  currentDraft: T,
  previousSnapshot: string,
  submittedSnapshot: string,
  serverDraft: T | null,
  saveError?: unknown,
): { draft: T; savedSnapshot: string; error: string } {
  if (saveError !== undefined) {
    return {
      draft: currentDraft,
      savedSnapshot: previousSnapshot,
      error: String(saveError),
    };
  }
  if (!serverDraft) {
    return { draft: currentDraft, savedSnapshot: previousSnapshot, error: "" };
  }
  const serverSnapshot = JSON.stringify(serverDraft);
  return {
    draft: JSON.stringify(currentDraft) === submittedSnapshot ? serverDraft : currentDraft,
    savedSnapshot: serverSnapshot,
    error: "",
  };
}

export function platformLabelOf(platform: string): string {
  const value = String(platform || "").trim().toLowerCase();
  if (value === "feishu") return "Feishu";
  if (value === "dingtalk") return "DingTalk";
  if (value === "weixin_oc") return "个人微信";
  return "OneBot v11";
}

export function normalizeActivationMode(value: string): RemoteImContact["activationMode"] {
  const mode = String(value || "").trim().toLowerCase();
  if (mode === "always" || mode === "keyword") return mode;
  return "never";
}

export function normalizeResponseStrategy(
  value?: string,
): NonNullable<RemoteImContact["responseStrategy"]> {
  return value === "always_reply" ? "always_reply" : "smart_judge";
}

export function normalizeProcessingMode(value?: string): "qa" | "continuous" {
  return value === "qa" ? "qa" : "continuous";
}

export function contactRouteLabel(_item: RemoteImContact): string {
  return "联系人独立会话";
}

export function contactRoutingHint(_item: RemoteImContact): string {
  return "人格将在该联系人的独立会话中处理消息";
}

export function processingModeHint(item: RemoteImContact): string {
  return normalizeProcessingMode(item.processingMode) === "qa"
    ? "当前为无上下文模式（问答模式）"
    : "当前为有上下文模式（会话模式）";
}

export function contactActivationHint(item: RemoteImContact): string {
  const mode = normalizeActivationMode(item.activationMode);
  if (mode === "always") return "始终入场：消息入库后，总是开始本批巡检。";
  if (mode === "keyword") return "点名入场：消息命中点名词时，才允许进入巡检流程。";
  return "不入场：消息只入库，不开始巡检。";
}

export function contactResponseStrategyHint(item: RemoteImContact): string {
  const mode = normalizeResponseStrategy(item.responseStrategy);
  if (mode === "smart_judge") {
    return "智能判断：先用快速模型检查这批消息是否真的需要回复。";
  }
  return "始终回复：一旦允许入场，就直接交给处理人格回复。";
}

export function contactCommunicationToggleEnabled(item: Pick<RemoteImContact, "allowReceive" | "allowSend">): boolean {
  return !!item.allowSend;
}

export function contactCommunicationToggleClass(item: Pick<RemoteImContact, "allowReceive" | "allowSend">): string {
  return contactCommunicationToggleEnabled(item) ? "toggle-success" : "";
}

export function parseKeywordList(raw: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const item of String(raw || "").split(/[,\n，]/)) {
    const keyword = item.trim();
    if (!keyword || seen.has(keyword)) continue;
    seen.add(keyword);
    out.push(keyword);
  }
  return out;
}

export function parseBlockedMessagePrefixes(raw: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const item of String(raw || "").split(/\s+/)) {
    const prefix = item.trim();
    if (!prefix || seen.has(prefix)) continue;
    seen.add(prefix);
    out.push(prefix);
  }
  return out;
}

export function parseActivationKeywords(raw: string): string[] {
  return parseKeywordList(raw);
}

export function formatLogTime(timestamp: string): string {
  const d = new Date(timestamp);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}:${String(d.getSeconds()).padStart(2, "0")}`;
}
