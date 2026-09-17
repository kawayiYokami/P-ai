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

/** 在名单里加入或移除某个名称，返回去空后的新数组。 */
export function setListMembership(list: string[], name: string, member: boolean): string[] {
  const target = String(name || "").trim();
  if (!target) return list;
  const set = new Set(list.map((item) => String(item || "").trim()).filter(Boolean));
  if (member) {
    set.add(target);
  } else {
    set.delete(target);
  }
  return Array.from(set);
}

/** MCP 工具全名是 `serverId::toolName`，取回 serverId；旧数据可能只存裸工具名，返回空串。 */
export function mcpServerIdOf(fullName: string): string {
  const raw = String(fullName || "").trim();
  const idx = raw.indexOf("::");
  return idx >= 0 ? raw.slice(0, idx).trim() : "";
}

export type PersonaSkillState = "off" | "optional" | "resident";

/** 某个 skill 相对某个人格的三态：随身 / 备用 / 未启用。 */
export function personaSkillState(persona: PersonaProfile, name: string): PersonaSkillState {
  const target = String(name || "").trim();
  if ((persona.residentSkillNames || []).some((item) => String(item || "").trim() === target)) {
    return "resident";
  }
  if ((persona.optionalSkillNames || []).some((item) => String(item || "").trim() === target)) {
    return "optional";
  }
  return "off";
}

/**
 * 改写某个人格下一个 skill 的三态。
 * 随身与备用互斥，同名的两份不会并存；未启用则两边都摘掉。
 */
export function setPersonaSkillState(
  persona: PersonaProfile,
  name: string,
  state: PersonaSkillState,
): void {
  const target = String(name || "").trim();
  if (!target) return;
  const resident = new Set(
    (persona.residentSkillNames || []).map((item) => String(item || "").trim()).filter(Boolean),
  );
  const optional = new Set(
    (persona.optionalSkillNames || []).map((item) => String(item || "").trim()).filter(Boolean),
  );
  resident.delete(target);
  optional.delete(target);
  if (state === "resident") resident.add(target);
  if (state === "optional") optional.add(target);
  persona.residentSkillNames = Array.from(resident);
  persona.optionalSkillNames = Array.from(optional);
}
