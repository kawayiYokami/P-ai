import type { AgentPermissionControl, PersonaProfile } from "../../../types/app";

/**
 * 取得人格的权限控制块，缺失或字段残缺时就地补齐。
 * 人格可能来自旧数据，字段不一定齐全，这里统一兜底，避免下游到处判空。
 */
export function ensurePermissionControl(persona: PersonaProfile): AgentPermissionControl {
  if (!persona.permissionControl) {
    persona.permissionControl = {
      enabled: false,
      mode: "whitelist",
      builtinToolNames: [],
      skillNames: [],
      mcpToolNames: [],
    };
  }
  const control = persona.permissionControl;
  if (!Array.isArray(control.builtinToolNames)) control.builtinToolNames = [];
  if (!Array.isArray(control.skillNames)) control.skillNames = [];
  if (!Array.isArray(control.mcpToolNames)) control.mcpToolNames = [];
  if (control.mode !== "whitelist" && control.mode !== "blacklist") {
    control.mode = "whitelist";
  }
  return control;
}
