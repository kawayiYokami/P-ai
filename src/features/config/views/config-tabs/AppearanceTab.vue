<template>
  <div class="grid gap-2">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base">{{ t("appearance.language") }}</h3>
        <select
          class="select select-bordered w-full max-w-xs"
          :value="uiLanguage"
          @change="$emit('update:uiLanguage', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="opt in localeOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base">{{ t("appearance.theme") }}</h3>
        <ThemePreviewGrid :themes="themes" :current-theme="currentTheme" @select="$emit('setTheme', $event)" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { APP_THEMES } from "../../../shell/composables/use-app-theme";
import ThemePreviewGrid from "../../components/ThemePreviewGrid.vue";

defineProps<{
  uiLanguage: "zh-CN" | "en-US" | "zh-TW";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "zh-TW"; label: string }>;
  currentTheme: string;
}>();

defineEmits<{
  (e: "update:uiLanguage", value: string): void;
  (e: "setTheme", value: string): void;
}>();

const { t } = useI18n();
const themes = [...APP_THEMES];
</script>
