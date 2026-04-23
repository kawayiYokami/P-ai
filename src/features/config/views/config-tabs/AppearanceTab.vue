<template>
  <div class="grid gap-2">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <div class="grid grid-cols-1 gap-3">
          <div class="space-y-2">
            <h3 class="card-title text-base">{{ t("appearance.language") }}</h3>
            <select
              class="select select-bordered w-full"
              :value="props.uiLanguage"
              @change="$emit('update:uiLanguage', ($event.target as HTMLSelectElement).value)"
            >
              <option v-for="opt in props.localeOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
            </select>
          </div>
        </div>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body gap-4 p-4">
        <h3 class="card-title text-base">{{ t("appearance.theme") }}</h3>

        <div class="tabs tabs-box bg-base-200 p-1">
          <button
            type="button"
            class="tab flex-1 rounded-btn"
            :class="activeTab === 'preset' ? 'tab-active' : ''"
            @click="activeTab = 'preset'"
          >
            {{ t("appearance.themeTabs.preset") }}
          </button>
          <button
            type="button"
            class="tab flex-1 rounded-btn"
            :class="activeTab === 'generated' ? 'tab-active' : ''"
            @click="activateGeneratedTab"
          >
            {{ t("appearance.themeTabs.generated") }}
          </button>
        </div>

        <ThemePreviewGrid
          v-if="activeTab === 'preset'"
          :light-themes="lightThemes"
          :dark-themes="darkThemes"
          :current-theme="props.currentTheme"
          @select="$emit('setTheme', $event)"
        />

        <GeneratedThemeEditor
          v-else
          :controls="props.generatedThemeControls"
          :tokens="props.generatedThemeTokens"
          @update-controls="$emit('updateGeneratedThemeControls', $event)"
          @reset="$emit('resetGeneratedTheme')"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import ThemePreviewGrid from "../../components/ThemePreviewGrid.vue";
import GeneratedThemeEditor from "../../components/GeneratedThemeEditor.vue";
import {
  APP_THEMES,
  DARK_APP_THEMES,
} from "../../../shell/composables/use-app-theme";
import type { GeneratedThemeControls, GeneratedThemeTokens } from "../../../shell/theme/theme-types";
import {
  GENERATED_THEME_DARK_ID,
  GENERATED_THEME_LIGHT_ID,
} from "../../../shell/theme/theme-generator";

const props = defineProps<{
  uiLanguage: "zh-CN" | "en-US" | "zh-TW";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "zh-TW"; label: string }>;
  currentTheme: string;
  generatedThemeControls: GeneratedThemeControls;
  generatedThemeTokens: GeneratedThemeTokens;
}>();

const emit = defineEmits<{
  (e: "update:uiLanguage", value: string): void;
  (e: "setTheme", value: string): void;
  (e: "activateGeneratedTheme"): void;
  (e: "updateGeneratedThemeControls", value: Partial<GeneratedThemeControls>): void;
  (e: "resetGeneratedTheme"): void;
}>();

const { t } = useI18n();
const activeTab = ref<"preset" | "generated">("generated");
const lightThemes = computed(() => APP_THEMES.filter((theme) => !DARK_APP_THEMES.has(theme)));
const darkThemes = computed(() => APP_THEMES.filter((theme) => DARK_APP_THEMES.has(theme)));

function isGeneratedTheme(theme: string) {
  return theme === GENERATED_THEME_LIGHT_ID || theme === GENERATED_THEME_DARK_ID;
}

function activateGeneratedTab() {
  activeTab.value = "generated";
  if (!isGeneratedTheme(props.currentTheme)) {
    emit("activateGeneratedTheme");
  }
}

watch(
  () => props.currentTheme,
  (theme) => {
    activeTab.value = isGeneratedTheme(theme) ? "generated" : "preset";
  },
  { immediate: true },
);
</script>
