<template>
  <div class="grid grid-cols-2 md:grid-cols-3 xl:grid-cols-4 gap-2">
    <button
      v-for="theme in themes"
      :key="theme"
      type="button"
      class="rounded-box border transition-all text-left overflow-hidden"
      :class="currentTheme === theme ? 'border-base-content shadow-sm ring-1 ring-base-content/30' : 'border-base-300 hover:border-base-content/30'"
      :data-theme="theme"
      @click="$emit('select', theme)"
    >
      <div class="flex">
        <div class="w-8 h-16 bg-base-200"></div>
        <div class="w-8 h-16 bg-base-300"></div>
        <div class="flex-1 px-3 py-2 bg-base-100">
          <div class="font-semibold text-base-content text-base leading-tight">{{ themeLabel(theme) }}</div>
          <div class="mt-2 flex items-center gap-1">
            <span class="badge badge-sm badge-primary">A</span>
            <span class="badge badge-sm badge-secondary">A</span>
            <span class="badge badge-sm badge-accent">A</span>
            <span class="badge badge-sm badge-neutral">A</span>
          </div>
        </div>
      </div>
    </button>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{
  themes: string[];
  currentTheme: string;
}>();

defineEmits<{
  (e: "select", value: string): void;
}>();

const { t, te } = useI18n();

function themeLabel(theme: string): string {
  const key = `appearance.themeNames.${theme}`;
  return te(key) ? t(key) : theme;
}
</script>
