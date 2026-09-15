import type { ApiConfigItem, PersonaProfile } from "../../types/app";
import { MODEL_ROLE_EXPERT_API_CONFIG_ID, resolveModelRoleApiConfigId } from "../config/utils/model-role-options";

export type AgentPersonaOption = {
  id: string;
  agentId: string;
  agentName: string;
  label: string;
  name: string;
  ownerAgentId: string;
  ownerName: string;
  providerName?: string;
  modelName?: string;
  apiConfigId?: string;
  modelMissing?: boolean;
  personaMissing?: boolean;
  childAgentIds?: string[];
  unavailable?: boolean;
};

type BuildAgentPersonaOptionsInput = {
  personas: PersonaProfile[] | null | undefined;
  apiConfigs: ApiConfigItem[] | null | undefined;
  expertApiConfigId?: string;
  toolReviewApiConfigId?: string | null;
};

function trimText(value: unknown): string {
  return String(value || "").trim();
}

function agentPrimaryApiConfigId(persona: PersonaProfile | null | undefined): string {
  const ids = Array.isArray(persona?.apiConfigIds)
    ? persona.apiConfigIds.map(trimText).filter(Boolean)
    : [];
  if (ids.length > 0) return ids[0];
  return trimText((persona as { apiConfigId?: unknown } | null | undefined)?.apiConfigId);
}

function agentConversationApiConfigId(
  persona: PersonaProfile,
  input: BuildAgentPersonaOptionsInput,
): string {
  const directId = agentPrimaryApiConfigId(persona);
  // 人格未显式指定模型时回退到「专家」模型角色，与后端 agent_api_config_ids 的默认一致。
  const rawId = directId || MODEL_ROLE_EXPERT_API_CONFIG_ID;
  return resolveModelRoleApiConfigId(rawId, {
    expertApiConfigId: trimText(input.expertApiConfigId),
    toolReviewApiConfigId: trimText(input.toolReviewApiConfigId),
  });
}

/** 选项 id 即人格 id：组织已经扁平为「人格即目标」这一层。 */
export function agentPersonaOptionId(agentId: string): string {
  return trimText(agentId);
}

export function buildAgentPersonaOptions(input: BuildAgentPersonaOptionsInput): AgentPersonaOption[] {
  const apiConfigs = new Map(
    (input.apiConfigs || [])
      .map((api) => [trimText(api.id), api] as const)
      .filter(([id, api]) => !!id && !!api.enableText),
  );
  const options: AgentPersonaOption[] = [];
  for (const persona of input.personas || []) {
    const agentId = trimText(persona.id);
    if (!agentId) continue;
    if (persona.isBuiltInUser || agentId === "user-persona") continue;
    const apiConfigId = agentConversationApiConfigId(persona, input);
    const apiConfig = apiConfigId ? apiConfigs.get(apiConfigId) : null;
    const agentName = trimText(persona.name) || agentId;
    options.push({
      id: agentPersonaOptionId(agentId),
      agentId,
      agentName,
      label: agentName,
      name: agentName,
      ownerAgentId: agentId,
      ownerName: agentName,
      providerName: apiConfig ? trimText(apiConfig.name || apiConfig.id) || undefined : undefined,
      modelName: apiConfig ? trimText(apiConfig.displayName || apiConfig.model) || undefined : undefined,
      apiConfigId: apiConfigId || undefined,
      modelMissing: !apiConfig,
      personaMissing: false,
      childAgentIds: Array.isArray(persona.childAgentIds)
        ? persona.childAgentIds.map(trimText).filter(Boolean)
        : [],
    });
  }
  return options;
}

export function findAgentPersonaOption(
  options: AgentPersonaOption[],
  agentId?: string | null,
): AgentPersonaOption | null {
  const aid = trimText(agentId);
  if (!aid) return null;
  return options.find((option) => option.agentId === aid) || null;
}
