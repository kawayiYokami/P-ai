import { describe, expect, it } from "vitest";
import { migratePersistedThemePreferences } from "./use-app-theme";
import { DEFAULT_GENERATED_THEME_CONTROLS } from "../theme/theme-generator";

/** v3 存档的骨架：mode 只有 manual / auto，主题只记一个 activeState。 */
function legacyPreferences(overrides: Record<string, unknown>) {
  return {
    version: 3,
    mode: "manual",
    activeState: { kind: "preset", name: "light" },
    autoLightTheme: "light",
    autoDarkTheme: "dark",
    generatedControls: { ...DEFAULT_GENERATED_THEME_CONTROLS },
    ...overrides,
  };
}

describe("migratePersistedThemePreferences", () => {
  it("manual + 浅色预设：显示模式落 light，来源落 preset，主题名进浅色槽", () => {
    const migrated = migratePersistedThemePreferences(
      legacyPreferences({ activeState: { kind: "preset", name: "cupcake" } }),
    );

    expect(migrated).not.toBeNull();
    expect(migrated!.version).toBe(4);
    expect(migrated!.mode).toBe("light");
    expect(migrated!.source).toBe("preset");
    expect(migrated!.presetLightTheme).toBe("cupcake");
    // 深色槽保留迁移前那套自动深色主题，不被清空
    expect(migrated!.presetDarkTheme).toBe("dark");
  });

  it("manual + 深色预设：显示模式落 dark，来源落 preset，主题名进深色槽", () => {
    const migrated = migratePersistedThemePreferences(
      legacyPreferences({ activeState: { kind: "preset", name: "dracula" } }),
    );

    expect(migrated!.mode).toBe("dark");
    expect(migrated!.source).toBe("preset");
    expect(migrated!.presetDarkTheme).toBe("dracula");
    expect(migrated!.presetLightTheme).toBe("light");
  });

  it("manual + 自定义：来源落 generated，显示模式按当时实际生效那一侧", () => {
    const controls = {
      ...DEFAULT_GENERATED_THEME_CONTROLS,
      mode: "dark" as const,
      themeHue: 210,
    };
    const migrated = migratePersistedThemePreferences(
      legacyPreferences({ activeState: { kind: "generated", controls } }),
    );

    expect(migrated!.source).toBe("generated");
    expect(migrated!.mode).toBe("dark");
    // 那一侧的自定义参数要跟着搬进 byMode 对应槽位
    expect(migrated!.generatedControlsByMode.dark.themeHue).toBe(210);
  });

  it("auto + 任意来源：显示模式固定落 auto，其余维度照常迁移", () => {
    const migrated = migratePersistedThemePreferences(
      legacyPreferences({ mode: "auto", activeState: { kind: "preset", name: "cupcake" } }),
    );

    expect(migrated!.mode).toBe("auto");
    expect(migrated!.source).toBe("preset");
    expect(migrated!.presetLightTheme).toBe("cupcake");
  });

  it("已是 v4 的存档原样放行，只做兜底归一", () => {
    const migrated = migratePersistedThemePreferences({
      version: 4,
      mode: "dark",
      source: "generated",
      presetLightTheme: "cupcake",
      presetDarkTheme: "dim",
      generatedControlsByMode: null,
    });

    expect(migrated!.mode).toBe("dark");
    expect(migrated!.source).toBe("generated");
    expect(migrated!.presetLightTheme).toBe("cupcake");
    expect(migrated!.presetDarkTheme).toBe("dim");
  });

  it("读不出结构时返回 null，由调用方回退默认值", () => {
    expect(migratePersistedThemePreferences(null)).toBeNull();
    expect(migratePersistedThemePreferences("not an object")).toBeNull();
  });
});
