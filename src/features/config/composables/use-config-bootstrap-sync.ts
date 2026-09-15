import type { Ref } from "vue";
import { applyUiSizeScale } from "../../shell/composables/use-ui-size-appearance";
import type { AppConfig, PromptCommandPreset } from "../../../types/app";
import {
  normalizeImageGenerationModelId,
  normalizeImageGenerationProviders,
} from "../utils/image-generation-config";

type RuntimeNumberNormalizer = (
  minValue: unknown,
  maxValue: unknown,
  fallback?: { minRecordSeconds?: number; maxRecordSeconds?: number },
) => { minRecordSeconds: number; maxRecordSeconds: number };

function normalizeInstructionPresets(value: unknown): PromptCommandPreset[] {
  return Array.isArray(value)
    ? value
      .map((item) => ({
        id: String(item?.id || "").trim(),
        name: String(item?.prompt || item?.name || "").trim(),
        prompt: String(item?.prompt || item?.name || "").trim(),
      }))
      .filter((item) => !!item.id && !!item.prompt)
    : [];
}

function normalizeWebAccessPort(value: unknown): number {
  const parsed = Math.round(Number(value));
  if (Number.isFinite(parsed) && parsed >= 1024 && parsed <= 65535) {
    return parsed;
  }
  return 8429;
}

export function applyConversationApiBootstrapUpdate(bindings: {
  config: AppConfig;
}, payload: Record<string, unknown>) {
  bindings.config.expertApiConfigId = String(payload.expertApiConfigId ?? "").trim();
  bindings.config.visionApiConfigId = payload.visionApiConfigId as string | undefined;
  bindings.config.toolReviewApiConfigId = payload.toolReviewApiConfigId as string | undefined;
  bindings.config.sttApiConfigId = payload.sttApiConfigId as string | undefined;
  if ("sttAutoSend" in payload) bindings.config.sttAutoSend = !!payload.sttAutoSend;
}

export function applyChatSettingsBootstrapUpdate(bindings: {
  assistantAgentId: Ref<string>;
  personaEditorId: Ref<string>;
  userAlias: Ref<string>;
  selectedResponseStyleId: Ref<string>;
  selectedPdfReadMode: Ref<"text" | "image">;
  backgroundVoiceScreenshotKeywords: Ref<string>;
  backgroundVoiceScreenshotMode: Ref<"desktop" | "focused_window">;
  instructionPresets: Ref<PromptCommandPreset[]>;
}, payload: Record<string, unknown>) {
  if ("assistantAgentId" in payload) {
    const nextAgentId = String(payload.assistantAgentId ?? "").trim();
    bindings.assistantAgentId.value = nextAgentId;
    if (bindings.personaEditorId.value !== nextAgentId) bindings.personaEditorId.value = nextAgentId;
  }
  if ("userAlias" in payload) bindings.userAlias.value = String(payload.userAlias ?? "");
  if ("responseStyleId" in payload) bindings.selectedResponseStyleId.value = String(payload.responseStyleId ?? "").trim();
  if (payload.pdfReadMode === "text" || payload.pdfReadMode === "image") {
    bindings.selectedPdfReadMode.value = payload.pdfReadMode;
  }
  if ("backgroundVoiceScreenshotKeywords" in payload) {
    bindings.backgroundVoiceScreenshotKeywords.value = String(payload.backgroundVoiceScreenshotKeywords ?? "");
  }
  if (payload.backgroundVoiceScreenshotMode === "desktop" || payload.backgroundVoiceScreenshotMode === "focused_window") {
    bindings.backgroundVoiceScreenshotMode.value = payload.backgroundVoiceScreenshotMode;
  }
  if (Array.isArray(payload.instructionPresets)) {
    bindings.instructionPresets.value = normalizeInstructionPresets(payload.instructionPresets);
  }
}

export function applyConfigBootstrapUpdate(bindings: {
  config: AppConfig;
  createApiConfig: (id: string) => AppConfig["apiConfigs"][number];
  buildConfigSnapshotJson: () => string;
  lastSavedConfigJson: Ref<string>;
  normalizeUiSizeScale: (value: unknown) => AppConfig["uiSizeScale"];
  updateGithubUpdateMethod: (value: unknown) => void;
  normalizeRuntimeConfigNumbers?: RuntimeNumberNormalizer;
}, payload: Record<string, unknown>) {
  if (!payload || typeof payload !== "object") return;
  if ("hotkey" in payload) bindings.config.hotkey = String(payload.hotkey ?? "").trim();
  if ("uiFont" in payload) bindings.config.uiFont = String(payload.uiFont ?? "");
  if ("codeFont" in payload) bindings.config.codeFont = String(payload.codeFont ?? "");
  if ("uiSizeScale" in payload) {
    bindings.config.uiSizeScale = bindings.normalizeUiSizeScale(payload.uiSizeScale);
    applyUiSizeScale(bindings.config.uiSizeScale);
  }
  if ("webAccessPort" in payload) bindings.config.webAccessPort = normalizeWebAccessPort(payload.webAccessPort);
  if ("webAccessEnabled" in payload) bindings.config.webAccessEnabled = payload.webAccessEnabled !== false;
  if ("webAccessPassword" in payload) bindings.config.webAccessPassword = String(payload.webAccessPassword || "").trim();
  if ("githubUpdateMethod" in payload) bindings.updateGithubUpdateMethod(payload.githubUpdateMethod);
  if ("recordHotkey" in payload) bindings.config.recordHotkey = String(payload.recordHotkey ?? "");
  if ("recordBackgroundWakeEnabled" in payload) bindings.config.recordBackgroundWakeEnabled = !!payload.recordBackgroundWakeEnabled;
  if (bindings.normalizeRuntimeConfigNumbers && ("minRecordSeconds" in payload || "maxRecordSeconds" in payload)) {
    const normalized = bindings.normalizeRuntimeConfigNumbers(
      "minRecordSeconds" in payload ? payload.minRecordSeconds : bindings.config.minRecordSeconds,
      "maxRecordSeconds" in payload ? payload.maxRecordSeconds : bindings.config.maxRecordSeconds,
      {
        minRecordSeconds: bindings.config.minRecordSeconds,
        maxRecordSeconds: bindings.config.maxRecordSeconds,
      },
    );
    bindings.config.minRecordSeconds = normalized.minRecordSeconds;
    bindings.config.maxRecordSeconds = normalized.maxRecordSeconds;
  }
  if ("selectedApiConfigId" in payload) bindings.config.selectedApiConfigId = String(payload.selectedApiConfigId ?? "").trim();
  if ("expertApiConfigId" in payload) bindings.config.expertApiConfigId = String(payload.expertApiConfigId ?? "").trim();
  if ("visionApiConfigId" in payload) bindings.config.visionApiConfigId = payload.visionApiConfigId as string | undefined;
  if ("imageProviders" in payload) {
    bindings.config.imageProviders = normalizeImageGenerationProviders(payload.imageProviders);
  }
  if ("imageGenerationModelId" in payload || "imageProviders" in payload) {
    bindings.config.imageGenerationModelId = normalizeImageGenerationModelId(
      "imageGenerationModelId" in payload
        ? payload.imageGenerationModelId
        : bindings.config.imageGenerationModelId,
      bindings.config.imageProviders,
    );
  }
  if ("toolReviewApiConfigId" in payload) bindings.config.toolReviewApiConfigId = payload.toolReviewApiConfigId as string | undefined;
  if ("sttApiConfigId" in payload) bindings.config.sttApiConfigId = payload.sttApiConfigId as string | undefined;
  if ("sttAutoSend" in payload) bindings.config.sttAutoSend = !!payload.sttAutoSend;
  if ("terminalShellKind" in payload) bindings.config.terminalShellKind = String(payload.terminalShellKind ?? "");
  if ("apiProviders" in payload) {
    bindings.config.apiProviders = Array.isArray(payload.apiProviders)
      ? payload.apiProviders.map((provider: any) => ({
        ...provider,
        deprecated: !!provider.deprecated,
        apiKeys: Array.isArray(provider.apiKeys) ? [...provider.apiKeys] : [],
        cachedModelOptions: Array.isArray(provider.cachedModelOptions) ? [...provider.cachedModelOptions] : [],
        models: Array.isArray(provider.models) ? provider.models.map((model: any) => ({ ...model, deprecated: !!model.deprecated })) : [],
        tools: Array.isArray(provider.tools)
          ? provider.tools.map((tool: any) => ({
            ...tool,
            args: Array.isArray(tool.args) ? [...tool.args] : [],
            values: { ...((tool.values || {}) as Record<string, unknown>) },
          }))
          : [],
      }))
      : [];
  }
  bindings.config.apiConfigs.splice(
    0,
    bindings.config.apiConfigs.length,
    ...((Array.isArray(payload.apiConfigs) && payload.apiConfigs.length > 0)
      ? payload.apiConfigs.map((item: any) => ({
        ...item,
        tools: Array.isArray(item.tools)
          ? item.tools.map((tool: any) => ({
            ...tool,
            args: Array.isArray(tool.args) ? [...tool.args] : [],
            values: { ...((tool.values || {}) as Record<string, unknown>) },
          }))
          : [],
      }))
      : [bindings.createApiConfig("default")]),
  );
  bindings.lastSavedConfigJson.value = bindings.buildConfigSnapshotJson();
}
