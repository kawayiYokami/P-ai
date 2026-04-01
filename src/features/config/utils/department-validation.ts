import type { ApiConfigItem, AppConfig } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

function textCapableApiIds(apiConfigs: ApiConfigItem[]): Set<string> {
  return new Set(
    (apiConfigs || [])
      .filter((api) => !!api.enableText && ["openai", "openai_responses", "gemini", "anthropic"].includes(api.requestFormat))
      .map((api) => api.id),
  );
}

export function validateDepartmentConfig(
  config: AppConfig,
  apiConfigs: ApiConfigItem[],
  t: TrFn,
): string {
  const departments = config.departments || [];
  const validApiIds = textCapableApiIds(apiConfigs);

  for (const department of departments) {
    const name = String(department.name || "").trim();
    if (!name) {
      return t("config.department.validation.emptyName");
    }
  }

  const seenNames = new Set<string>();
  for (const department of departments) {
    const nameKey = String(department.name || "").trim().toLocaleLowerCase();
    if (!nameKey) continue;
    if (seenNames.has(nameKey)) {
      return t("config.department.validation.duplicateName");
    }
    seenNames.add(nameKey);
  }

  for (const department of departments) {
    const ids = Array.isArray(department.apiConfigIds) && department.apiConfigIds.length > 0
      ? department.apiConfigIds
      : (department.apiConfigId ? [department.apiConfigId] : []);
    for (const id of ids.map((item) => String(item || "").trim()).filter(Boolean)) {
      if (!validApiIds.has(id)) {
        return t("config.department.validation.invalidModel", { name: department.name || department.id });
      }
    }
  }

  const assistant = departments.find((item) => item.id === "assistant-department" || item.isBuiltInAssistant);
  if (!assistant) {
    return t("config.department.validation.missingAssistant");
  }
  const assistantIds = Array.isArray(assistant.apiConfigIds) && assistant.apiConfigIds.length > 0
    ? assistant.apiConfigIds
    : (assistant.apiConfigId ? [assistant.apiConfigId] : []);
  if (assistantIds.map((item) => String(item || "").trim()).filter(Boolean).length === 0) {
    return t("config.department.validation.assistantNeedsModel");
  }

  return "";
}
