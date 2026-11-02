/**
 * 内置人格 id。
 *
 * 与后端 `src-tauri/src/features/core/domain/constants.rs` 的 `USER_PERSONA_ID` /
 * `SYSTEM_PERSONA_ID` 一一对应；前端需要判断「这是不是用户」「这是不是系统」时统一从这里取，
 * 不要在各组件里重新写字面量。
 */
export const USER_PERSONA_ID = "user-persona";
export const SYSTEM_PERSONA_ID = "system-persona";

/** 说话人是否内置系统人格。 */
export function isSystemSpeakerId(speakerAgentId: unknown): boolean {
  return String(speakerAgentId || "").trim() === SYSTEM_PERSONA_ID;
}
