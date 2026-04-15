import { createI18n } from "vue-i18n";
import zhCN from "./locales/zh-CN.json";
import enUS from "./locales/en-US.json";
import zhTW from "./locales/zh-TW.json";

export const SUPPORTED_LOCALES = ["zh-CN", "en-US", "zh-TW"] as const;
export type SupportedLocale = (typeof SUPPORTED_LOCALES)[number];
export const LOCALE_MESSAGES = {
  "zh-CN": zhCN,
  "en-US": enUS,
  "zh-TW": zhTW,
} as const;

export function normalizeLocale(value?: string | null): SupportedLocale {
  const v = String(value || "").trim();
  if ((SUPPORTED_LOCALES as readonly string[]).includes(v)) return v as SupportedLocale;
  return "zh-CN";
}

export const i18n = createI18n({
  legacy: false,
  locale: "zh-CN",
  fallbackLocale: "zh-CN",
  messages: LOCALE_MESSAGES,
});
