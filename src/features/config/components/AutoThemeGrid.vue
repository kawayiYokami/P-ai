<template>
  <div class="space-y-4">
    <section>
      <h4 class="mb-2 text-sm font-semibold text-base-content/70">{{ t("appearance.autoThemeLight") }}</h4>
      <div class="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-4">
        <button
          v-for="card in lightCards"
          :key="card.id"
          type="button"
          class="overflow-hidden rounded-box border text-left transition-all"
          :class="selectedClass(props.autoLightTheme, card.id)"
          :data-theme="card.custom ? GENERATED_THEME_NAME : card.id"
          :style="card.custom ? customStyle(props.lightCustomTokens) : undefined"
          @click="$emit('selectLight', card.id)"
        >
          <div class="flex">
            <div class="w-8 bg-base-200"></div>
            <div class="w-8 bg-base-300"></div>
            <div class="flex-1 bg-base-100 px-3 py-2">
              <div class="flex items-center justify-between gap-1">
                <div class="text-base font-semibold leading-tight text-base-content">{{ themeLabel(card) }}</div>
                <span
                  v-if="props.autoLightTheme === card.id"
                  class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-base-content text-base-100"
                >
                  <AppIcon name="check" class="h-3 w-3" />
                </span>
              </div>
              <div class="mt-2 flex flex-wrap items-center gap-1">
                <span class="badge badge-sm badge-primary">A</span>
                <span class="badge badge-sm badge-secondary">A</span>
                <span class="badge badge-sm badge-accent">A</span>
                <span class="badge badge-sm badge-neutral">A</span>
              </div>
              <!-- 四大状态色：卡片不只要看品牌色，也要看出成功/警告/错误/信息在该主题下的表现 -->
              <div class="mt-1 flex flex-wrap items-center gap-1">
                <span class="badge badge-sm badge-success">A</span>
                <span class="badge badge-sm badge-warning">A</span>
                <span class="badge badge-sm badge-error">A</span>
                <span class="badge badge-sm badge-info">A</span>
              </div>
            </div>
          </div>
        </button>
      </div>
    </section>

    <section>
      <h4 class="mb-2 text-sm font-semibold text-base-content/70">{{ t("appearance.autoThemeDark") }}</h4>
      <div class="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-4">
        <button
          v-for="card in darkCards"
          :key="card.id"
          type="button"
          class="overflow-hidden rounded-box border text-left transition-all"
          :class="selectedClass(props.autoDarkTheme, card.id)"
          :data-theme="card.custom ? GENERATED_THEME_NAME : card.id"
          :style="card.custom ? customStyle(props.darkCustomTokens) : undefined"
          @click="$emit('selectDark', card.id)"
        >
          <div class="flex">
            <div class="w-8 bg-base-200"></div>
            <div class="w-8 bg-base-300"></div>
            <div class="flex-1 bg-base-100 px-3 py-2">
              <div class="flex items-center justify-between gap-1">
                <div class="text-base font-semibold leading-tight text-base-content">{{ themeLabel(card) }}</div>
                <span
                  v-if="props.autoDarkTheme === card.id"
                  class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-base-content text-base-100"
                >
                  <AppIcon name="check" class="h-3 w-3" />
                </span>
              </div>
              <div class="mt-2 flex flex-wrap items-center gap-1">
                <span class="badge badge-sm badge-primary">A</span>
                <span class="badge badge-sm badge-secondary">A</span>
                <span class="badge badge-sm badge-accent">A</span>
                <span class="badge badge-sm badge-neutral">A</span>
              </div>
              <!-- 四大状态色：卡片不只要看品牌色，也要看出成功/警告/错误/信息在该主题下的表现 -->
              <div class="mt-1 flex flex-wrap items-center gap-1">
                <span class="badge badge-sm badge-success">A</span>
                <span class="badge badge-sm badge-warning">A</span>
                <span class="badge badge-sm badge-error">A</span>
                <span class="badge badge-sm badge-info">A</span>
              </div>
            </div>
          </div>
        </button>
      </div>
    </section>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import {
  GENERATED_THEME_DARK_ID,
  GENERATED_THEME_LIGHT_ID,
  GENERATED_THEME_NAME,
  generatedThemeTokensToCssVariables,
} from "../../shell/theme/theme-generator";
import type { GeneratedThemeTokens } from "../../shell/theme/theme-types";

const props = defineProps<{
  lightThemes: string[];
  darkThemes: string[];
  autoLightTheme: string;
  autoDarkTheme: string;
  lightCustomTokens: GeneratedThemeTokens;
  darkCustomTokens: GeneratedThemeTokens;
}>();

defineEmits<{
  (e: "selectLight", value: string): void;
  (e: "selectDark", value: string): void;
}>();

const { t, te } = useI18n();

type AutoThemeCard = { id: string; custom: boolean };

const lightCards = computed<AutoThemeCard[]>(() => [
  ...props.lightThemes.map((id) => ({ id, custom: false })),
  { id: GENERATED_THEME_LIGHT_ID, custom: true },
]);
const darkCards = computed<AutoThemeCard[]>(() => [
  ...props.darkThemes.map((id) => ({ id, custom: false })),
  { id: GENERATED_THEME_DARK_ID, custom: true },
]);

function selectedClass(current: string, cardId: string): string {
  return current === cardId
    ? "border-base-content shadow-sm ring-1 ring-base-content/30"
    : "border-base-300 hover:border-base-content/30";
}

function customStyle(tokens: GeneratedThemeTokens) {
  return generatedThemeTokensToCssVariables(tokens);
}

function themeLabel(card: AutoThemeCard): string {
  const key = `appearance.themeNames.${card.id}`;
  return te(key) ? t(key) : card.id;
}
</script>
