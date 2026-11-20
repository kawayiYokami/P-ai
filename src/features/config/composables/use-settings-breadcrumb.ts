import { inject, provide, ref, type InjectionKey, type Ref } from "vue";
import type { SettingsBreadcrumbItem } from "../components/settings-breadcrumb-types";

/**
 * 设置页面包屑上下文。
 *
 * 面包屑那一层**永远存在**：由设置页外壳（`ConfigView`）统一渲染在右栏顶端，
 * 子页只是往上补自己的层级。子页不定义面包屑时，外壳用当前一级 tab 名兜底，
 * 所以任何设置页（包括不用 `SettingsPageShell` 的页面）都有这一行。
 */
export type SettingsBreadcrumbContext = {
  /** 子页上报的层级；空数组表示「本级没有更细的层级」。 */
  items: Ref<SettingsBreadcrumbItem[]>;
  set(next: SettingsBreadcrumbItem[]): void;
  reset(): void;
};

const SETTINGS_BREADCRUMB_KEY: InjectionKey<SettingsBreadcrumbContext> = Symbol(
  "settings-breadcrumb",
);

export function provideSettingsBreadcrumb(): SettingsBreadcrumbContext {
  const items = ref<SettingsBreadcrumbItem[]>([]);
  const context: SettingsBreadcrumbContext = {
    items,
    set(next) {
      items.value = next;
    },
    reset() {
      items.value = [];
    },
  };
  provide(SETTINGS_BREADCRUMB_KEY, context);
  return context;
}

export function useSettingsBreadcrumbContext(): SettingsBreadcrumbContext | null {
  return inject(SETTINGS_BREADCRUMB_KEY, null);
}
