import type { ApiConfigItem, AppConfig } from "../../../types/app";

export function isTextRequestFormat(value: unknown): boolean {
  return ![
    "openai_tts",
    "openai_stt",
    "openai_embedding",
    "openai_rerank",
    "gemini_embedding",
  ].includes(String(value || "").trim());
}

export function chatApiHasRequiredAuth(api: ApiConfigItem): boolean {
  const format = String(api.requestFormat || "").trim();
  const authMode = String(api.codexAuthMode || "read_local").trim();
  if (format === "codex" && (authMode === "read_local" || authMode === "managed_oauth")) {
    return true;
  }
  return !!String(api.apiKey || "").trim();
}

export function isUsableTextLlmApi(api: ApiConfigItem): boolean {
  return !!api.enableText
    && isTextRequestFormat(api.requestFormat)
    && !!String(api.baseUrl || "").trim()
    && !!String(api.model || "").trim()
    && chatApiHasRequiredAuth(api);
}

export function hasUsableTextLlm(config: AppConfig): boolean {
  const usableIds = new Set(
    (config.apiConfigs || [])
      .filter(isUsableTextLlmApi)
      .map((api) => String(api.id || "").trim())
      .filter(Boolean),
  );
  if (usableIds.size === 0) return false;
  const directId = String(config.assistantDepartmentApiConfigId || "").trim();
  if (directId && usableIds.has(directId)) return true;
  const assistant = (config.departments || []).find((item) => item.id === "assistant-department" || item.isBuiltInAssistant);
  const ids = Array.isArray(assistant?.apiConfigIds) && assistant?.apiConfigIds.length
    ? assistant.apiConfigIds
    : (assistant?.apiConfigId ? [assistant.apiConfigId] : []);
  return ids.some((id) => usableIds.has(String(id || "").trim()));
}
