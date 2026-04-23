<template>
  <div class="space-y-4">
    <section>
      <div class="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-4">
        <button
          v-for="theme in lightThemes"
          :key="theme"
          type="button"
          class="overflow-hidden rounded-box border text-left transition-all"
          :class="currentTheme === theme ? 'border-base-content shadow-sm ring-1 ring-base-content/30' : 'border-base-300 hover:border-base-content/30'"
          :data-theme="theme"
          @click="$emit('select', theme)"
        >
          <div class="flex">
            <div class="h-16 w-8 bg-base-200"></div>
            <div class="h-16 w-8 bg-base-300"></div>
            <div class="flex-1 bg-base-100 px-3 py-2">
              <div class="text-base font-semibold leading-tight text-base-content">{{ themeLabel(theme) }}</div>
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
    </section>

    <section>
      <div class="grid grid-cols-2 gap-2 md:grid-cols-3 xl:grid-cols-4">
        <button
          v-for="theme in darkThemes"
          :key="theme"
          type="button"
          class="overflow-hidden rounded-box border text-left transition-all"
          :class="currentTheme === theme ? 'border-base-content shadow-sm ring-1 ring-base-content/30' : 'border-base-300 hover:border-base-content/30'"
          :data-theme="theme"
          @click="$emit('select', theme)"
        >
          <div class="flex">
            <div class="h-16 w-8 bg-base-200"></div>
            <div class="h-16 w-8 bg-base-300"></div>
            <div class="flex-1 bg-base-100 px-3 py-2">
              <div class="text-base font-semibold leading-tight text-base-content">{{ themeLabel(theme) }}</div>
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
    </section>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{
  lightThemes: string[];
  darkThemes: string[];
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
