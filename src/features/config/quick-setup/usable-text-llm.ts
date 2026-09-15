import type { ApiConfigItem, AppConfig } from "../../../types/app";
import { MODEL_ROLE_EXPERT_API_CONFIG_ID, resolveModelRoleApiConfigId } from "../utils/model-role-options";

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
  const directId = String(config.expertApiConfigId || "").trim();
  if (directId && usableIds.has(directId)) return true;
  // 主助理人格未显式指定模型时按「专家」模型角色解析，与后端 agent_api_config_ids 的默认一致。
  const assistantId = resolveModelRoleApiConfigId(MODEL_ROLE_EXPERT_API_CONFIG_ID, config);
  return usableIds.has(assistantId);
}
