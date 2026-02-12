import { computed, type Ref } from "vue";
import { emit } from "@tauri-apps/api/event";
import { formatI18nError } from "../../../utils/error";
import { normalizeLocale, type SupportedLocale } from "../../../i18n";
import type { AppConfig } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseAppCoreOptions = {
  t: TrFn;
  config: AppConfig;
  locale: { value: string };
  status: Ref<string>;
  perfDebug: boolean;
};

export function useAppCore(options: UseAppCoreOptions) {
  function perfNow(): number {
    return typeof performance !== "undefined" ? performance.now() : Date.now();
  }

  function perfLog(label: string, startedAt: number) {
    if (!options.perfDebug) return;
    const cost = Math.round((perfNow() - startedAt) * 10) / 10;
    console.log(`[PERF] ${label}: ${cost}ms`);
  }

  function setStatus(text: string) {
    options.status.value = text;
  }

  function setStatusError(key: string, error: unknown) {
    options.status.value = formatI18nError(options.t, key, error);
  }

  const localeOptions = computed<Array<{ value: SupportedLocale; label: string }>>(() => [
    { value: "zh-CN", label: options.t("config.language.zhCN") },
    { value: "en-US", label: options.t("config.language.enUS") },
    { value: "ja-JP", label: options.t("config.language.jaJP") },
    { value: "ko-KR", label: options.t("config.language.koKR") },
  ]);

  function applyUiLanguage(value: string): boolean {
    const lang = normalizeLocale(value);
    if (options.config.uiLanguage === lang && options.locale.value === lang) return false;
    options.config.uiLanguage = lang;
    options.locale.value = lang;
    emit("easy-call:locale-changed", lang);
    return true;
  }

  return {
    perfNow,
    perfLog,
    setStatus,
    setStatusError,
    localeOptions,
    applyUiLanguage,
  };
}
