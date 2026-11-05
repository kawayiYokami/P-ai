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
          </div>
          <!-- 四大状态色：与主题卡片一致，自定义主题也要能看出状态色表现 -->
          <div class="mt-1.5 flex flex-wrap items-center gap-1.5">
            <span class="badge badge-sm badge-success">A</span>
            <span class="badge badge-sm badge-warning">A</span>
            <span class="badge badge-sm badge-error">A</span>
            <span class="badge badge-sm badge-info">A</span>
          </div>
        </div>
      </div>
    </div>

    <div class="space-y-4">
      <div class="mb-4 flex items-center justify-between gap-3">
        <div>
          <h4 class="text-sm font-semibold text-base-content">{{ t("appearance.customThemeControls") }}</h4>
        </div>
        <button class="btn btn-xs" type="button" @click="resetEditingMode">{{ t("appearance.resetGeneratedTheme") }}</button>
      </div>

      <div class="grid gap-4">
        <div class="space-y-2">
          <SegmentedControl
            :model-value="editControls.mode"
            :options="modeOptions"
            @change="patchMode"
          />
        </div>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.themeHue") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ Math.round(editControls.themeHue) }}deg</span>
          </div>
          <input
            class="range range-primary w-full"
            type="range"
            min="1"
            max="360"
            step="1"
            :value="editControls.themeHue"
            @input="patchSlider('themeHue', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.contrast") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ editControls.contrast }}</span>
          </div>
          <input
            class="range range-secondary w-full"
            type="range"
            min="1"
            max="30"
            step="1"
            :value="editControls.contrast"
            @input="patchSlider('contrast', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ brightnessLabel }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ editControls.brightness }}</span>
          </div>
          <input
            class="range range-info w-full"
            type="range"
            min="0"
            max="100"
            step="1"
            :value="editControls.brightness"
            @input="patchSlider('brightness', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.tint") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ editControls.tint }}</span>
          </div>
          <input
            class="range range-success w-full"
            type="range"
            min="0"
            max="100"
            step="1"
            :value="editControls.tint"
            @input="patchSlider('tint', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.tone") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ editControls.tone }}</span>
          </div>
          <input
            class="range range-accent w-full"
            type="range"
            min="30"
            max="100"
            step="1"
            :value="editControls.tone"
            @input="patchSlider('tone', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.textStrength") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ editControls.textStrength }}</span>
          </div>
          <input
            class="range range-warning w-full"
            type="range"
            min="55"
            max="100"
            step="1"
            :value="editControls.textStrength"
            @input="patchSlider('textStrength', $event)"
          />
        </label>

        <label class="grid gap-2">
          <div class="flex items-center justify-between gap-3 text-sm">
            <span>{{ t("appearance.radius") }}</span>
            <span class="font-mono text-xs text-base-content/65">{{ editControls.radius }}px</span>
          </div>
          <input
            class="range w-full"
            type="range"
            min="0"
            max="28"
            step="1"
            :value="editControls.radius"
            @input="patchSlider('radius', $event)"
          />
        </label>

        <div class="grid gap-2 md:grid-cols-2">
          <label class="label cursor-pointer justify-start gap-3 rounded-box border border-base-300 bg-base-200 px-3 py-2">
            <input
              class="toggle toggle-sm"
              type="checkbox"
              :checked="editControls.depthEnabled"
              @change="patchBoolean('depthEnabled', $event)"
            />
            <span class="text-sm text-base-content">{{ t("appearance.depthEnabled") }}</span>
          </label>
          <label class="label cursor-pointer justify-start gap-3 rounded-box border border-base-300 bg-base-200 px-3 py-2">
            <input
              class="toggle toggle-sm"
              type="checkbox"
              :checked="editControls.noiseEnabled"
              @change="patchBoolean('noiseEnabled', $event)"
            />
            <span class="text-sm text-base-content">{{ t("appearance.noiseEnabled") }}</span>
          </label>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import SegmentedControl from "./SegmentedControl.vue";
import {
  GENERATED_THEME_NAME,
  generateGeneratedThemeTokens,
  generatedThemeTokensToCssVariables,
  getGeneratedThemeDefaultControls,
} from "../../shell/theme/theme-generator";
import type {
  GeneratedThemeControls,
  ThemeMode,
} from "../../shell/theme/theme-types";
import { useAppTheme } from "../../shell/composables/use-app-theme";

const props = defineProps<{
  controls: GeneratedThemeControls;
}>();

const { t } = useI18n();
const { generatedThemeControlsByMode, updateGeneratedThemeControls } = useAppTheme();

/**
 * 「我在编辑哪一侧」是编辑器自己的状态：只由这里的浅色/深色切换决定。
 * 不能从应用当前生效的明暗派生——那样上面选了深色，这里就只能编辑深色，
 * 点上浅色也会被主题重新施加时改回去。
 */
const editMode = ref<ThemeMode>(props.controls.mode);
const editControls = computed(() => generatedThemeControlsByMode.value[editMode.value]);

const previewStyle = computed(() => generatedThemeTokensToCssVariables(generateGeneratedThemeTokens(editControls.value)));
const modeOptions = computed(() => [
  { value: "light" as const, label: t("appearance.modeOptions.light") },
  { value: "dark" as const, label: t("appearance.modeOptions.dark") },
]);
const brightnessLabel = computed(() =>
  editMode.value === "dark" ? t("appearance.darkness") : t("appearance.brightness"),
);

function patchControls(patch: Partial<GeneratedThemeControls>) {
  updateGeneratedThemeControls({ ...patch, mode: editMode.value });
}

function patchMode(mode: ThemeMode) {
  editMode.value = mode;
}

function resetEditingMode() {
  updateGeneratedThemeControls(getGeneratedThemeDefaultControls(editMode.value));
}

function patchSlider(key: "themeHue" | "contrast" | "brightness" | "tint" | "tone" | "textStrength" | "radius", event: Event) {
  const nextValue = Number((event.target as HTMLInputElement).value || 0);
  patchControls({ [key]: nextValue } as Partial<GeneratedThemeControls>);
}

function patchBoolean(key: "depthEnabled" | "noiseEnabled", event: Event) {
  patchControls({ [key]: (event.target as HTMLInputElement).checked } as Partial<GeneratedThemeControls>);
}
</script>
