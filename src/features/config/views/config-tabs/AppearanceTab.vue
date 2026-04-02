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
      <div class="card-body p-4">
        <h3 class="card-title text-base">{{ t("appearance.theme") }}</h3>
        <ThemePreviewGrid
          :light-themes="lightThemes"
          :dark-themes="darkThemes"
          :current-theme="props.currentTheme"
          @select="$emit('setTheme', $event)"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { computed } from "vue";
import { APP_THEMES, DARK_APP_THEMES } from "../../../shell/composables/use-app-theme";
import ThemePreviewGrid from "../../components/ThemePreviewGrid.vue";

const props = defineProps<{
  uiLanguage: "zh-CN" | "en-US" | "zh-TW";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "zh-TW"; label: string }>;
  currentTheme: string;
}>();

defineEmits<{
  (e: "update:uiLanguage", value: string): void;
  (e: "setTheme", value: string): void;
}>();

const { t } = useI18n();
const lightThemes = computed(() => APP_THEMES.filter((theme) => !DARK_APP_THEMES.has(theme)));
const darkThemes = computed(() => APP_THEMES.filter((theme) => DARK_APP_THEMES.has(theme)));
</script>
