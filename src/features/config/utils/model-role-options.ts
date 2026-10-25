export const MODEL_ROLE_EXPERT_API_CONFIG_ID = "role:expert";
export const MODEL_ROLE_QUICK_API_CONFIG_ID = "role:quick";
/** 「跟随会话」委托哨兵：它本身不是可提交的模型，必须由会话上下文解析。 */
export const MODEL_ROLE_SESSION_API_CONFIG_ID = "role:session";

export function isModelRoleApiConfigId(value: unknown): boolean {
  const normalized = String(value || "").trim();
  return normalized === MODEL_ROLE_EXPERT_API_CONFIG_ID
    || normalized === MODEL_ROLE_QUICK_API_CONFIG_ID
    || normalized === MODEL_ROLE_SESSION_API_CONFIG_ID;
}

export function resolveModelRoleApiConfigId(
  value: unknown,
  config: { expertApiConfigId?: string; toolReviewApiConfigId?: string | null },
): string {
  const normalized = String(value || "").trim();
  if (normalized === MODEL_ROLE_EXPERT_API_CONFIG_ID) {
    return String(config.expertApiConfigId || "").trim();
  }
  if (normalized === MODEL_ROLE_QUICK_API_CONFIG_ID) {
    return String(config.toolReviewApiConfigId || "").trim();
  }
  return normalized;
}
