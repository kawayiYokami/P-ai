<template>
  <div class="space-y-4">
    <div
      class="w-full overflow-hidden rounded-box border border-primary text-left shadow-sm ring-1 ring-primary/35 transition-all"
      :style="previewStyle"
      :data-theme="GENERATED_THEME_NAME"
    >
      <div class="flex">
        <div class="h-20 w-10 bg-base-200"></div>
        <div class="h-20 w-10 bg-base-300"></div>
        <div class="flex-1 bg-base-100 px-4 py-3">
          <div class="flex items-center justify-between gap-3">
            <div>
              <div class="text-base font-semibold text-base-content">{{ t("appearance.customThemeTitle") }}</div>
            </div>
          </div>
          <div class="mt-3 flex flex-wrap items-center gap-1.5">
            <span class="badge badge-sm badge-primary">A</span>
            <span class="badge badge-sm badge-secondary">A</span>
            <span class="badge badge-sm badge-accent">A</span>
            <span class="badge badge-sm badge-neutral">A</span>
            <span class="badge badge-sm badge-ghost">{{ sizePresetLabel }}</span>
          </div>
        </div>
      </div>
    </div>

    <div class="space-y-4">
      <div class="mb-4 flex items-center justify-between gap-3">
        <div>
          <h4 class="text-sm font-semibold text-base-content">{{ t("appearance.customThemeControls") }}</h4>
        </div>
        <button class="btn btn-xs" type="button" @click="$emit('reset')">{{ t("appearance.resetGeneratedTheme") }}</button>
      </div>

      <div class="grid gap-4">
        <div class="space-y-2">
          <div class="join w-full">
            <button
              type="button"
              class="btn join-item flex-1"
              :class="props.controls.mode === 'light' ? 'btn-primary' : 'border-base-300 bg-base-200 hover:bg-base-300'"
              @click="patchControls({ mode: 'light' })"
            >
              {{ t("appearance.modeOptions.light") }}
            </button>
            <button
              type="button"
              class="btn join-item flex-1"
              :class="props.controls.mode === 'dark' ? 'btn-primary' : 'border-base-300 bg-base-200 hover:bg-base-300'"
              @click="patchControls({ mode: 'dark' })"
            >
              {{ t("appearance.modeOptions.dark") }}
            </button>
          </div>
        </div>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.themeHue") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ Math.round(props.controls.themeHue) }}deg</span>
          </div>
          <input
            class="range range-primary w-full"
            type="range"
            min="1"
            max="360"
            step="1"
            :value="props.controls.themeHue"
            @input="patchSlider('themeHue', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.contrast") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ props.controls.contrast }}</span>
          </div>
          <input
            class="range range-secondary w-full"
            type="range"
            min="10"
            max="100"
            step="1"
            :value="props.controls.contrast"
            @input="patchSlider('contrast', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.brightness") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ props.controls.brightness }}</span>
          </div>
          <input
            class="range range-info w-full"
            type="range"
            min="0"
            max="100"
            step="1"
            :value="props.controls.brightness"
            @input="patchSlider('brightness', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.tint") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ props.controls.tint }}</span>
          </div>
          <input
            class="range range-success w-full"
            type="range"
            min="0"
            max="100"
            step="1"
            :value="props.controls.tint"
            @input="patchSlider('tint', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.tone") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ props.controls.tone }}</span>
          </div>
          <input
            class="range range-accent w-full"
            type="range"
            min="30"
            max="100"
            step="1"
            :value="props.controls.tone"
            @input="patchSlider('tone', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.radius") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ props.controls.radius }}px</span>
          </div>
          <input
            class="range w-full"
            type="range"
            min="0"
            max="28"
            step="1"
            :value="props.controls.radius"
            @input="patchSlider('radius', $event)"
          />
        </label>

        <div class="space-y-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.uiSizePreset") }}</span>
            <span class="text-xs text-base-content/65">{{ sizePresetLabel }}</span>
          </div>
          <div class="join w-full">
            <button
              v-for="preset in sizePresets"
              :key="preset"
              type="button"
              class="btn join-item flex-1"
              :class="props.controls.uiSizePreset === preset ? 'btn-primary' : 'border-base-300 bg-base-200 hover:bg-base-300'"
              @click="patchControls({ uiSizePreset: preset })"
            >
              {{ t(`appearance.sizePresets.${preset}`) }}
            </button>
          </div>
        </div>

        <div class="grid gap-2 md:grid-cols-2">
          <label class="label cursor-pointer justify-start gap-3 rounded-box border border-base-300 bg-base-200 px-3 py-2">
            <input
              class="toggle toggle-sm"
              type="checkbox"
              :checked="props.controls.depthEnabled"
              @change="patchBoolean('depthEnabled', $event)"
            />
            <span class="label-text text-base-content">{{ t("appearance.depthEnabled") }}</span>
          </label>
          <label class="label cursor-pointer justify-start gap-3 rounded-box border border-base-300 bg-base-200 px-3 py-2">
            <input
              class="toggle toggle-sm"
              type="checkbox"
              :checked="props.controls.noiseEnabled"
              @change="patchBoolean('noiseEnabled', $event)"
            />
            <span class="label-text text-base-content">{{ t("appearance.noiseEnabled") }}</span>
          </label>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  GENERATED_THEME_NAME,
  generatedThemeTokensToCssVariables,
} from "../../shell/theme/theme-generator";
import type {
  GeneratedThemeControls,
  GeneratedThemeTokens,
  GeneratedUiSizePreset,
} from "../../shell/theme/theme-types";

const props = defineProps<{
  controls: GeneratedThemeControls;
  tokens: GeneratedThemeTokens;
}>();

const emit = defineEmits<{
  (e: "updateControls", value: Partial<GeneratedThemeControls>): void;
  (e: "reset"): void;
}>();

const { t } = useI18n();

const sizePresets: GeneratedUiSizePreset[] = ["compact", "default", "comfortable"];

const previewStyle = computed(() => generatedThemeTokensToCssVariables(props.tokens));
const sizePresetLabel = computed(() => t(`appearance.sizePresets.${props.controls.uiSizePreset}`));

function patchControls(patch: Partial<GeneratedThemeControls>) {
  emit("updateControls", patch);
}

function patchSlider(key: "themeHue" | "contrast" | "brightness" | "tint" | "tone" | "radius", event: Event) {
  const nextValue = Number((event.target as HTMLInputElement).value || 0);
  patchControls({ [key]: nextValue } as Partial<GeneratedThemeControls>);
}

function patchBoolean(key: "depthEnabled" | "noiseEnabled", event: Event) {
  patchControls({ [key]: (event.target as HTMLInputElement).checked } as Partial<GeneratedThemeControls>);
}
</script>
