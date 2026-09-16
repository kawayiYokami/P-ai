import type { AgentPermissionControl, PersonaProfile } from "../../../types/app";

function normalizePermissionControl(control?: AgentPermissionControl | null) {
  if (!control) {
    return {
      enabled: false,
      mode: "whitelist",
      builtinToolNames: [],
      skillNames: [],
      mcpToolNames: [],
    };
  }
  return {
    enabled: !!control.enabled,
    mode: control.mode || "whitelist",
    builtinToolNames: [...(control.builtinToolNames || [])].map((s) => String(s || "").trim()).filter(Boolean).sort(),
    skillNames: [...(control.skillNames || [])].map((s) => String(s || "").trim()).filter(Boolean).sort(),
    mcpToolNames: [...(control.mcpToolNames || [])].map((s) => String(s || "").trim()).filter(Boolean).sort(),
  };
}

export function buildPersonasSnapshotJson(personas: PersonaProfile[]) {
  return JSON.stringify(
    personas.map((item) => ({
      id: item.id,
      name: item.name,
      systemPrompt: item.systemPrompt,
      residentSkillNames: (item.residentSkillNames || []).map((s) => String(s || "").trim()).filter(Boolean),
      childAgentIds: (item.childAgentIds || []).map((id) => String(id || "").trim()).filter(Boolean).sort(),
      permissionControl: normalizePermissionControl(item.permissionControl),
      privateMemoryEnabled: !!item.privateMemoryEnabled,
      memoryRecallMode: item.memoryRecallMode || "auto",
      avatarPath: item.avatarPath || "",
      avatarUpdatedAt: item.avatarUpdatedAt || "",
      isBuiltInUser: !!item.isBuiltInUser,
      isBuiltInSystem: !!item.isBuiltInSystem,
      source: item.source || "",
      scope: item.scope || "",
      tools: (item.tools || []).map((tool) => ({
        id: tool.id,
        enabled: !!tool.enabled,
        command: tool.command || "",
        args: Array.isArray(tool.args) ? [...tool.args] : [],
        values: tool.values ?? null,
      })),
    })),
  );
}

export function useChatWindowMessageHelpers(bindings: Record<string, any>) {
  function syncUserAliasFromPersona() {
    const next = (bindings.userPersona.value?.name || "").trim() || bindings.t("archives.roleUser");
    if (bindings.userAlias.value !== next) {
      bindings.userAlias.value = next;
    }
  }

  return {
    syncUserAliasFromPersona,
  };
}
