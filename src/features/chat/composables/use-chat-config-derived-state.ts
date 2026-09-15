import { computed } from "vue";
import type { ApiRequestFormat, AppConfig } from "../../../types/app";
import { isModelRoleApiConfigId, MODEL_ROLE_EXPERT_API_CONFIG_ID, resolveModelRoleApiConfigId } from "../../config/utils/model-role-options";

export function useChatConfigDerivedState(config: AppConfig) {
  const TEXT_REQUEST_FORMATS = new Set<ApiRequestFormat>([
    "auto",
    "openai",
    "deepseek",
    "openai_responses",
    "codex",
    "gemini",
    "anthropic",
    "fireworks",
    "together",
    "groq",
    "mimo",
    "minimax",
    "moonshot",
    "nebius",
    "xai",
    "zai",
    "bigmodel",
    "aliyun",
    "baidu",
    "cohere",
    "ollama",
    "ollama_cloud",
    "vertex",
    "github_copilot",
    "opencode_go",
    "bedrock_api",
  ]);

  function isTextRequestFormat(format: string): boolean {
    const normalized = String(format || "").trim().toLowerCase();
    return normalized === "deepseek/kimi" || TEXT_REQUEST_FORMATS.has(normalized as ApiRequestFormat);
  }

  const selectedApiConfig = computed(() => config.apiConfigs.find((a) => a.id === config.selectedApiConfigId) ?? null);
  const selectedApiProvider = computed(() => {
    const [providerId] = String(config.selectedApiConfigId || "").split("::");
    return config.apiProviders.find((provider) => provider.id === providerId) ?? null;
  });
  const textCapableApiConfigs = computed(() =>
    config.apiConfigs.filter((a) => a.enableText && isTextRequestFormat(a.requestFormat)),
  );
  const imageCapableApiConfigs = computed(() =>
    config.apiConfigs.filter((a) =>
      a.enableImage || a.enableAudio || a.enableVideo,
    ),
  );
  const sttCapableApiConfigs = computed(() =>
    config.apiConfigs.filter((a) => a.requestFormat === "openai_stt" || a.requestFormat === "mimo_asr"),
  );

  function agentPrimaryApiConfigId(
    agent?: { apiConfigId?: string; apiConfigIds?: string[] } | null,
  ): string {
    const ids = Array.isArray(agent?.apiConfigIds)
      ? agent.apiConfigIds.map((id) => String(id || "").trim()).filter(Boolean)
      : [];
    if (ids.length > 0) return ids[0];
    return String(agent?.apiConfigId || "").trim();
  }

  function agentOrderedApiConfigIds(
    agent?: { apiConfigId?: string; apiConfigIds?: string[] } | null,
  ): string[] {
    const ordered = Array.from(new Set([
      ...((Array.isArray(agent?.apiConfigIds) ? agent.apiConfigIds : []).map((id) => String(id || "").trim()).filter(Boolean)),
      String(agent?.apiConfigId || "").trim(),
    ].filter(Boolean)));
    // 模型失败自动降级已停用：只保留主模型。
    return ordered.slice(0, 1);
  }

  function agentConversationApiConfigId(
    agent?: { id?: string; isBuiltInAssistant?: boolean; apiConfigId?: string; apiConfigIds?: string[] } | null,
  ): string {
    const directId = agentPrimaryApiConfigId(agent);
    // 人格未显式指定模型时回退到「专家」模型角色，与后端 agent_api_config_ids 的默认一致。
    const rawId = directId || MODEL_ROLE_EXPERT_API_CONFIG_ID;
    return resolveModelRoleApiConfigId(rawId, config);
  }

  function applyAgentPrimaryApiConfigLocally(
    agent: { id?: string; isBuiltInAssistant?: boolean; apiConfigId?: string; apiConfigIds?: string[]; updatedAt?: string } | null | undefined,
    apiConfigId: string,
  ): boolean {
    if (!agent) return false;
    const nextId = String(apiConfigId || "").trim();
    if (!nextId) return false;
    const next = agentOrderedApiConfigIds(agent);
    if ((next[0] || "") === nextId) {
      if (!isModelRoleApiConfigId(nextId)) {
        config.selectedApiConfigId = nextId;
      }
      return false;
    }
    const filtered: string[] = [];
    filtered.unshift(nextId);
    const deduped = Array.from(new Set(filtered.filter(Boolean)));
    agent.apiConfigIds = deduped;
    agent.apiConfigId = deduped[0] || "";
    agent.updatedAt = new Date().toISOString();
    if (!isModelRoleApiConfigId(nextId)) {
      config.selectedApiConfigId = nextId;
    }
    return true;
  }

  function normalizeRuntimeConfigNumbers(
    minValue: unknown,
    maxValue: unknown,
    fallback?: {
      minRecordSeconds?: number;
      maxRecordSeconds?: number;
    },
  ): { minRecordSeconds: number; maxRecordSeconds: number } {
    const MIN_RECORD_SECONDS = 1;
    const MAX_MIN_RECORD_SECONDS = 30;
    const DEFAULT_MAX_RECORD_SECONDS = 60;
    const MAX_RECORD_SECONDS = 600;
    const fallbackMin = Number(fallback?.minRecordSeconds);
    const fallbackMax = Number(fallback?.maxRecordSeconds);
    const nextMin = Number(minValue);
    const nextMax = Number(maxValue);
    const resolvedMin = Number.isFinite(nextMin)
      ? nextMin
      : (Number.isFinite(fallbackMin) ? fallbackMin : MIN_RECORD_SECONDS);
    const minRecordSeconds = Math.max(
      MIN_RECORD_SECONDS,
      Math.min(MAX_MIN_RECORD_SECONDS, Math.round(resolvedMin)),
    );
    const resolvedMax = Number.isFinite(nextMax)
      ? nextMax
      : (Number.isFinite(fallbackMax) ? fallbackMax : DEFAULT_MAX_RECORD_SECONDS);
    const maxRecordSeconds = Math.max(
      minRecordSeconds,
      Math.min(MAX_RECORD_SECONDS, Math.round(resolvedMax)),
    );
    return { minRecordSeconds, maxRecordSeconds };
  }

  const expertApiConfigId = computed(
    () => String(config.expertApiConfigId || "").trim(),
  );
  const assistantAgentApiConfig = computed(
    () => config.apiConfigs.find((a) => a.id === expertApiConfigId.value) ?? null,
  );
  const hasVisionFallback = computed(() =>
    !!config.visionApiConfigId
    && config.apiConfigs.some((a) =>
      a.id === config.visionApiConfigId
      && (a.enableImage || a.enableAudio || a.enableVideo),
    ),
  );
  const activeSttApiConfig = computed(
    () => sttCapableApiConfigs.value.find((a) => a.id === config.sttApiConfigId) ?? null,
  );
  const shouldUseRemoteStt = computed(() => {
    const cfg = activeSttApiConfig.value;
    if (!cfg) return false;
    return !!cfg.model.trim() && !!cfg.baseUrl.trim() && !!cfg.apiKey.trim();
  });

  return {
    TEXT_REQUEST_FORMATS,
    isTextRequestFormat,
    selectedApiConfig,
    selectedApiProvider,
    textCapableApiConfigs,
    imageCapableApiConfigs,
    sttCapableApiConfigs,
    agentPrimaryApiConfigId,
    agentOrderedApiConfigIds,
    agentConversationApiConfigId,
    applyAgentPrimaryApiConfigLocally,
    normalizeRuntimeConfigNumbers,
    expertApiConfigId,
    assistantAgentApiConfig,
    hasVisionFallback,
    activeSttApiConfig,
    shouldUseRemoteStt,
  };
}
