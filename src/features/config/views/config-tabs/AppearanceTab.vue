<template>
  <div class="grid gap-2">
    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
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
          <div class="space-y-2">
            <h3 class="card-title text-base">{{ t("appearance.font") }}</h3>
            <select
              class="select select-bordered w-full"
              :value="props.uiFont"
              @change="$emit('update:uiFont', ($event.target as HTMLSelectElement).value)"
            >
              <option v-for="opt in fontOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
            </select>
          </div>
        </div>
      </div>
    </div>

    <div class="card bg-base-100 border border-base-300">
      <div class="card-body p-4">
        <h3 class="card-title text-base">{{ t("appearance.theme") }}</h3>
        <ThemePreviewGrid :themes="themes" :current-theme="props.currentTheme" @select="$emit('setTheme', $event)" />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { invokeTauri } from "../../../../services/tauri-api";
import { APP_THEMES } from "../../../shell/composables/use-app-theme";
import ThemePreviewGrid from "../../components/ThemePreviewGrid.vue";

const props = defineProps<{
  uiLanguage: "zh-CN" | "en-US" | "zh-TW";
  uiFont: string;
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "zh-TW"; label: string }>;
  currentTheme: string;
}>();

defineEmits<{
  (e: "update:uiLanguage", value: string): void;
  (e: "update:uiFont", value: string): void;
  (e: "setTheme", value: string): void;
}>();

const { t } = useI18n();
const themes = [...APP_THEMES];
const systemFonts = ref<string[]>([]);

const fontOptions = computed(() => {
  const base = [{ value: "auto", label: t("appearance.fontAuto") }];
  const extras = systemFonts.value.map((name) => ({ value: name, label: name }));
  const current = String(props.uiFont || "").trim();
  if (current && current !== "auto" && !extras.some((v) => v.value === current)) {
    extras.unshift({ value: current, label: current });
  }
  return [...base, ...extras];
});

onMounted(async () => {
  try {
    const fonts = await invokeTauri<string[]>("list_system_fonts");
    if (Array.isArray(fonts)) {
      systemFonts.value = fonts.map((v) => String(v || "").trim()).filter(Boolean);
    }
  } catch (error) {
    console.warn("[APPEARANCE] list_system_fonts failed:", error);
    systemFonts.value = [];
  }
});
</script>
