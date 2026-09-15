import { computed } from "vue";
import type { ChatMentionEntry } from "../../../types/app";
import { resolveModelRoleApiConfigId } from "../../config/utils/model-role-options";
import { buildAgentPersonaOptions } from "../../shared/agent-persona-options";

export function useChatPersonaConversationDerivedState(bindings: Record<string, any>) {
  const userPersona = computed(
    () => bindings.personas.value.find((p: any) => p.isBuiltInUser || p.id === "user-persona") ?? null,
  );
  const assistantPersonas = computed(() =>
    bindings.personas.value.filter((p: any) =>
      !p.isBuiltInUser && !p.isBuiltInSystem && p.id !== "user-persona" && p.id !== "system-persona",
    ),
  );
  const assistantPersona = computed(
    () =>
      assistantPersonas.value.find((p: any) => p.id === bindings.assistantAgentId.value)
      ?? assistantPersonas.value[0]
      ?? null,
  );
  const currentForegroundConversationSummary = computed(() => {
    const currentConversationId = String(bindings.currentChatConversationId.value || "").trim();
    if (currentConversationId) {
      const matched = bindings.chatConversationItems.value.find(
        (item: any) => String(item.conversationId || "").trim() === currentConversationId,
      );
      if (matched) return matched;
    }
    return (
      bindings.unarchivedConversations.value.find((item: any) => !!item.isSystemNotificationConversation)
      || bindings.unarchivedConversations.value[0]
      || null
    );
  });
  const currentForegroundAgentId = computed(
    () => String(currentForegroundConversationSummary.value?.agentId || "").trim(),
  );
  const currentForegroundPersona = computed(
    () =>
      bindings.personas.value.find((p: any) => p.id === currentForegroundAgentId.value)
      ?? assistantPersona.value
      ?? assistantPersonas.value[0]
      ?? null,
  );
  function resolveForegroundTextApiConfigId(apiConfigId: string): string {
    const resolvedId = resolveModelRoleApiConfigId(apiConfigId, bindings.config);
    return bindings.config.apiConfigs.some((item: any) => item.id === resolvedId && item.enableText)
      ? resolvedId
      : "";
  }
  const currentConversationPreferredApiConfigId = computed(() => {
    const apiConfigId = String(bindings.currentChatPreferredApiConfigId?.value || "").trim();
    if (!apiConfigId) return "";
    return resolveForegroundTextApiConfigId(apiConfigId);
  });
  const currentForegroundApiConfigIds = computed(() => {
    const agentIds = bindings.agentOrderedApiConfigIds(currentForegroundPersona.value);
    return Array.from(new Set([
      currentConversationPreferredApiConfigId.value,
      ...agentIds,
    ].map((item: string) => resolveForegroundTextApiConfigId(String(item || "").trim())).filter(Boolean)));
  });
  const currentForegroundApiConfigId = computed(
    () => {
      return currentForegroundApiConfigIds.value[0]
        || resolveForegroundTextApiConfigId(bindings.agentConversationApiConfigId(currentForegroundPersona.value));
    },
  );
  const currentForegroundApiConfig = computed(
    () => {
      const resolvedId = resolveModelRoleApiConfigId(currentForegroundApiConfigId.value, bindings.config);
      return bindings.config.apiConfigs.find((a: any) => a.id === resolvedId) ?? null;
    },
  );
  const selectedPersonaEditor = computed(
    () => bindings.personas.value.find((p: any) => p.id === bindings.personaEditorId.value) ?? null,
  );
  const toolPersona = computed(() =>
    bindings.personas.value.find((p: any) => p.id === bindings.assistantAgentId.value)
    ?? assistantPersona.value
    ?? null,
  );
  const toolApiConfig = computed(() => {
    const resolvedId = bindings.agentConversationApiConfigId(toolPersona.value);
    if (!resolvedId) return null;
    return bindings.config.apiConfigs.find((a: any) => a.id === resolvedId) ?? null;
  });
  const userAvatarUrl = computed(
    () => bindings.resolveAvatarUrl(userPersona.value?.avatarPath, userPersona.value?.avatarUpdatedAt),
  );
  const userPersonaAvatarUrl = computed(() => userAvatarUrl.value);
  const selectedPersonaAvatarUrl = computed(
    () => bindings.resolveAvatarUrl(assistantPersona.value?.avatarPath, assistantPersona.value?.avatarUpdatedAt),
  );
  const currentForegroundPersonaAvatarUrl = computed(
    () => bindings.resolveAvatarUrl(currentForegroundPersona.value?.avatarPath, currentForegroundPersona.value?.avatarUpdatedAt),
  );
  const selectedPersonaEditorAvatarUrl = computed(
    () => bindings.resolveAvatarUrl(selectedPersonaEditor.value?.avatarPath, selectedPersonaEditor.value?.avatarUpdatedAt),
  );
  const chatPersonaNameMap = computed<Record<string, string>>(() => {
    const next: Record<string, string> = {};
    for (const persona of bindings.personas.value) {
      const id = String(persona.id || "").trim();
      if (!id) continue;
      const name = String(persona.name || "").trim();
      next[id] = name || id;
    }
    return next;
  });
  const chatPersonaAvatarUrlMap = computed<Record<string, string>>(() => {
    const next: Record<string, string> = {};
    for (const persona of bindings.personas.value) {
      const id = String(persona.id || "").trim();
      if (!id) continue;
      const url = bindings.resolveAvatarUrl(persona.avatarPath, persona.avatarUpdatedAt);
      if (url) {
        next[id] = url;
      } else if (!persona.isBuiltInUser && persona.id !== "user-persona") {
        next[id] = bindings.resolveBrandAvatarUrl();
      }
    }
    return next;
  });
  const chatMentionEntries = computed<ChatMentionEntry[]>(() => {
    const localeName = bindings.config.uiLanguage === "en-US" ? "en" : "zh-CN";
    const currentAgentId = String(currentForegroundAgentId.value || "").trim();
    const textCapableApiIds = new Set(
      (bindings.config.apiConfigs || [])
        .filter((api: any) => !!api.enableText && bindings.isTextRequestFormat(api.requestFormat))
        .map((api: any) => String(api.id || "").trim())
        .filter(Boolean),
    );
    const items: ChatMentionEntry[] = [];
    for (const persona of bindings.personas.value) {
      if (persona.isBuiltInSystem || persona.id === "system-persona") continue;
      const agentId = String(persona.id || "").trim();
      if (!agentId) continue;
      const agentName = String(persona.name || "").trim() || agentId;
      const avatarUrl = String(chatPersonaAvatarUrlMap.value[agentId] || "").trim() || undefined;
      const backgroundTaskCount = bindings.agentWorkPresence.activeWorkCountForAgent(
        String(bindings.currentChatConversationId.value || "").trim(),
        agentId,
      );
      const isUserPersona = agentId === "user-persona" || persona.isBuiltInUser;
      const isCurrentRuntimeAgent = agentId === currentAgentId;
      const hasTextModel = bindings.agentOrderedApiConfigIds(persona).some((apiConfigId: string) => {
        const resolvedId = resolveModelRoleApiConfigId(apiConfigId, bindings.config);
        return textCapableApiIds.has(resolvedId);
      });
      let mentionable = true;
      let unavailableReason = "";
      let hidden = false;
      if (isUserPersona) {
        mentionable = false;
        hidden = true;
        unavailableReason = bindings.t("chat.mentionUnavailableUserPersona");
      } else if (isCurrentRuntimeAgent) {
        mentionable = false;
        hidden = true;
        unavailableReason = bindings.t("chat.mentionUnavailableSelf");
      } else if (!hasTextModel) {
        mentionable = false;
        unavailableReason = bindings.t("chat.mentionUnavailableNoModel");
      }
      items.push({
        agentId,
        agentName,
        avatarUrl,
        isFrontSpeaking: isCurrentRuntimeAgent,
        hasBackgroundTask: backgroundTaskCount > 0,
        mentionable,
        hidden,
        unavailableReason: unavailableReason || undefined,
      });
    }
    return items.sort((left, right) => {
      if (left.isFrontSpeaking !== right.isFrontSpeaking) return left.isFrontSpeaking ? -1 : 1;
      if (left.mentionable !== right.mentionable) return left.mentionable ? -1 : 1;
      if (left.hasBackgroundTask !== right.hasBackgroundTask) return left.hasBackgroundTask ? -1 : 1;
      if (left.agentId === "user-persona" && right.agentId !== "user-persona") return -1;
      if (right.agentId === "user-persona" && left.agentId !== "user-persona") return 1;
      return left.agentName.localeCompare(right.agentName, localeName);
    });
  });
  const createConversationAgentOptions = computed(() =>
    buildAgentPersonaOptions({
      personas: bindings.personas.value || [],
      apiConfigs: bindings.config.apiConfigs || [],
      expertApiConfigId: bindings.config.expertApiConfigId,
      toolReviewApiConfigId: bindings.config.toolReviewApiConfigId,
    }),
  );
  return {
    userPersona,
    assistantPersonas,
    assistantPersona,
    currentForegroundConversationSummary,
    currentForegroundAgentId,
    currentConversationPreferredApiConfigId,
    currentForegroundApiConfigIds,
    currentForegroundApiConfigId,
    currentForegroundApiConfig,
    currentForegroundPersona,
    selectedPersonaEditor,
    toolPersona,
    toolApiConfig,
    userAvatarUrl,
    userPersonaAvatarUrl,
    selectedPersonaAvatarUrl,
    currentForegroundPersonaAvatarUrl,
    selectedPersonaEditorAvatarUrl,
    chatPersonaNameMap,
    chatPersonaAvatarUrlMap,
    chatMentionEntries,
    createConversationAgentOptions,
  };
}
