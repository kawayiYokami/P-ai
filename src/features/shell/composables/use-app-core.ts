import { computed, ref, type Ref } from "vue";
import { formatI18nError } from "../../../utils/error";
import { normalizeLocale, type SupportedLocale } from "../../../i18n";
import type { AppConfig } from "../../../types/app";
import { emitTransportEvent } from "../../../services/tauri-api";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseAppCoreOptions = {
  t: TrFn;
  config: AppConfig;
  locale: { value: string };
  status: Ref<string>;
  perfDebug: boolean;
};

export type StatusTone = "default" | "error" | "success";

export function useAppCore(options: UseAppCoreOptions) {
  function perfNow(): number {
    return typeof performance !== "undefined" ? performance.now() : Date.now();
  }

  function perfLog(label: string, startedAt: number) {
    if (!options.perfDebug) return;
    const cost = Math.round((perfNow() - startedAt) * 10) / 10;
    console.log(`[性能] ${label}: ${cost}ms`);
  }

  const statusTone = ref<StatusTone>("default");
  // 每次 setStatus 自增，让「内容相同」的重复提示也能重新弹出
  const statusNonce = ref(0);
  // sticky 提示不自动消失，需用户手动关闭（用于连接测试结果这类）
  const statusSticky = ref(false);

  function setStatus(text: string, tone: StatusTone = "default", sticky = false) {
    options.status.value = text;
    statusTone.value = tone;
    statusSticky.value = sticky;
    statusNonce.value += 1;
  }

  function setStatusError(key: string, error: unknown) {
    options.status.value = formatI18nError(options.t, key, error);
    statusTone.value = "error";
    statusSticky.value = false;
    statusNonce.value += 1;
  }

  const localeOptions = computed<Array<{ value: SupportedLocale; label: string }>>(() => [
    { value: "zh-CN", label: "简体中文" },
    { value: "en-US", label: "English" },
    { value: "zh-TW", label: "繁體中文" },
  ]);

  function applyUiLanguage(value: string): boolean {
    const lang = normalizeLocale(value);
    if (options.config.uiLanguage === lang && options.locale.value === lang) return false;
    options.config.uiLanguage = lang;
    options.locale.value = lang;
    void emitTransportEvent("locale.changed", lang).catch((error) => {
      console.warn("[语言] 同步语言变化失败", error);
    });
    return true;
  }

  return {
    perfNow,
    perfLog,
    setStatus,
    setStatusError,
    statusTone,
    statusNonce,
    statusSticky,
    localeOptions,
    applyUiLanguage,
  };
}
