<template>
  <div class="grid gap-2">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base">{{ t("appearance.theme") }}</h3>
        <div class="flex items-center gap-2">
          <button
            class="btn"
            :class="{ 'btn-active': currentTheme === 'light' }"
            @click="$emit('toggleTheme')"
          >{{ t("appearance.themeLight") }}</button>
          <button
            class="btn"
            :class="{ 'btn-active': currentTheme === 'dracula' }"
            @click="$emit('toggleTheme')"
          >{{ t("appearance.themeDark") }}</button>
        </div>
      </div>
    </div>

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
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{
  uiLanguage: "zh-CN" | "en-US" | "ja-JP" | "ko-KR";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "ja-JP" | "ko-KR"; label: string }>;
  currentTheme: "light" | "dracula";
}>();

defineEmits<{
  (e: "update:uiLanguage", value: string): void;
  (e: "toggleTheme"): void;
}>();

const { t } = useI18n();
</script>
