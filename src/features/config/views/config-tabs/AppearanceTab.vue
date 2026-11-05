<template>
  <ConfigTemplate :model-value="templateValues" :groups="templateGroups">
    <template #row-language>
      <div class="flex min-w-0 flex-wrap items-center justify-between gap-3">
        <span class="text-sm">{{ t("appearance.textLanguage") }}</span>
        <select
          class="select select-bordered select-sm w-52 max-w-full shrink-0"
          :value="props.uiLanguage"
          @change="$emit('update:uiLanguage', ($event.target as HTMLSelectElement).value)"
        >
          <option v-for="opt in props.localeOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
      </div>
    </template>

    <template #row-markdown-font-scale>
      <div class="flex min-w-0 flex-wrap items-center justify-between gap-3">
        <span class="text-sm">{{ t("appearance.textWeight") }}</span>
        <SegmentedControl
          :model-value="markdownFontScale < 1 ? 0 : 1"
          :options="markdownFontScaleOptions"
          :full-width="false"
          class="max-w-full shrink-0"
          @change="setMarkdownFontScale"
        />
      </div>
    </template>

    <template v-if="fontsAvailable" #row-ui-font>
      <div class="flex min-w-0 flex-wrap items-center justify-between gap-3">
        <span class="text-sm">{{ t("appearance.uiFont") }}</span>
        <FontFamilySelect
          :model-value="props.uiFont || 'auto'"
          :options="uiFontOptions"
          :auto-label="t('appearance.fontAuto')"
          :disabled="fontsLoading"
          @update:model-value="$emit('update:uiFont', $event)"
        />
      </div>
    </template>

    <template v-if="fontsAvailable" #row-code-font>
      <div class="flex min-w-0 flex-wrap items-center justify-between gap-3">
        <span class="text-sm">{{ t("appearance.codeFont") }}</span>
        <FontFamilySelect
          :model-value="props.codeFont || 'auto'"
          :options="codeFontOptions"
          :auto-label="t('appearance.fontAutoCode')"
          :disabled="fontsLoading"
          @update:model-value="$emit('update:codeFont', $event)"
        />
      </div>
    </template>

    <template #row-chat-bubble-background>
      <label class="flex min-w-0 cursor-pointer items-center justify-between gap-4">
        <span class="text-sm">{{ t("appearance.chatBubbleBackground") }}</span>
        <input
          :checked="assistantBubbleBackgroundEnabled"
          type="checkbox"
          class="toggle toggle-sm toggle-primary shrink-0"
          @change="setAssistantBubbleBackgroundEnabled(($event.target as HTMLInputElement).checked)"
        />
      </label>
    </template>
    <template #row-chat-process-messages-folded>
      <label class="flex min-w-0 cursor-pointer items-center justify-between gap-4">
        <span class="text-sm">{{ t("appearance.chatProcessMessagesFolded") }}</span>
        <input
          :checked="processMessagesFolded"
          type="checkbox"
          class="toggle toggle-sm toggle-primary shrink-0"
          @change="setProcessMessagesFolded(($event.target as HTMLInputElement).checked)"
        />
      </label>
    </template>
    <template #row-chat-bubble-time>
      <label class="flex min-w-0 cursor-pointer items-center justify-between gap-4">
        <span class="text-sm">{{ t("appearance.chatBubbleFullTime") }}</span>
        <input
          :checked="chatTimeDisplayMode === 'absolute'"
          type="checkbox"
          class="toggle toggle-sm toggle-primary shrink-0"
          @change="setChatTimeDisplayMode(($event.target as HTMLInputElement).checked ? 'absolute' : 'relative')"
        />
      </label>
    </template>
    <template #row-chat-bubble-markdown-layout>
      <div class="flex min-w-0 flex-wrap items-center justify-between gap-3">
        <span class="text-sm">{{ t("appearance.chatBubbleMarkdownLayout") }}</span>
        <SegmentedControl
          :model-value="markdownLayout"
          :options="markdownLayoutOptions"
          size="sm"
          :full-width="false"
          class="max-w-full shrink-0"
          @change="setChatMarkdownLayout"
        />
      </div>
    </template>

    <template #row-input-side-file-tags>
      <label class="flex min-w-0 cursor-pointer items-center justify-between gap-4">
        <span class="text-sm">{{ t("appearance.inputPanelSideFileTags") }}</span>
        <input
          :checked="sideFileTagsEnabled"
          type="checkbox"
          class="toggle toggle-sm toggle-primary shrink-0"
          @change="setSideFileTagsEnabled(($event.target as HTMLInputElement).checked)"
        />
      </label>
    </template>
    <template #row-input-ide-bridge-file-tags>
      <label class="flex min-w-0 cursor-pointer items-center justify-between gap-4">
        <span class="text-sm">{{ t("appearance.inputPanelIdeBridgeFileTags") }}</span>
        <input
          :checked="ideBridgeFileTagsEnabled"
          type="checkbox"
          class="toggle toggle-sm toggle-primary shrink-0"
          @change="setIdeBridgeFileTagsEnabled(($event.target as HTMLInputElement).checked)"
        />
      </label>
    </template>

    <template #row-file-reader-line-wrap>
      <label class="flex min-w-0 cursor-pointer items-center justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm">{{ t("appearance.fileReaderLineWrap") }}</div>
          <div class="mt-1 text-xs text-base-content/60">{{ t("appearance.fileReaderLineWrapHint") }}</div>
        </div>
        <input
          :checked="fileReaderLineWrapEnabled"
          type="checkbox"
          class="toggle toggle-sm toggle-primary shrink-0"
          @change="setFileReaderLineWrapEnabled(($event.target as HTMLInputElement).checked)"
        />
      </label>
    </template>

    <template #row-ui-size-scale>
      <div class="grid min-w-0 gap-2">
        <div class="flex items-center justify-between gap-3">
          <span class="text-sm">{{ t("appearance.uiSizeScale") }}</span>
          <output class="text-sm font-medium">{{ uiSizePresetText }}</output>
        </div>
        <!-- 四档字号，没有中间档：每档是一张手写的整数 px 表，不做百分比缩放 -->
        <div class="grid grid-cols-4 gap-1">
          <button
            v-for="preset in uiSizePresets"
            :key="preset.scale"
            class="btn btn-xs text-caption"
            :class="uiSizeScale === preset.scale ? 'btn-primary' : 'btn-ghost text-base-content/60'"
            :aria-pressed="uiSizeScale === preset.scale"
            type="button"
            @click="$emit('update:uiSizeScale', preset.scale)"
          >
            {{ preset.label }}
          </button>
        </div>
      </div>
    </template>

    <template #row-theme>
      <div class="grid min-w-0 gap-4">
        <!-- 显示模式：明 / 暗 / 自动。与下面的来源 tab 是两个独立维度 -->
        <div class="grid gap-2">
          <span class="text-xs text-base-content/60">{{ t("appearance.displayMode.label") }}</span>
          <SegmentedControl
            :model-value="props.themeMode"
            :options="displayModeOptions"
            @change="onDisplayModeChange"
          />
        </div>

        <div class="tabs tabs-box bg-base-200 p-1">
          <button
            type="button"
            class="tab flex-1 rounded-btn"
            :class="themePanel === 'preset' ? 'tab-active' : ''"
            @click="selectThemePanel('preset')"
          >
            {{ t("appearance.themeTabs.preset") }}
          </button>
          <button
            type="button"
            class="tab flex-1 rounded-btn"
            :class="themePanel === 'generated' ? 'tab-active' : ''"
            @click="selectThemePanel('generated')"
          >
            {{ t("appearance.themeTabs.generated") }}
          </button>
        </div>

        <!-- 预设这一栏原样沿用自动页那块：明暗各挑一个内置主题 -->
        <AutoThemeGrid
          v-if="themePanel === 'preset'"
          :light-themes="lightThemes"
          :dark-themes="darkThemes"
          :auto-light-theme="activeLightCardId"
          :auto-dark-theme="activeDarkCardId"
          :light-custom-tokens="props.generatedLightTokens"
          :dark-custom-tokens="props.generatedDarkTokens"
          @select-light="$emit('setPresetTheme', 'light', $event)"
          @select-dark="$emit('setPresetTheme', 'dark', $event)"
        />

        <GeneratedThemeEditor
          v-else
          :controls="props.generatedThemeControls"
        />
      </div>
    </template>
  </ConfigTemplate>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { canUseTransportSystemFonts, listTransportSystemFonts } from "../../../../services/tauri-api";
import SegmentedControl from "../../components/SegmentedControl.vue";
import ConfigTemplate from "../../components/ConfigTemplate.vue";
import FontFamilySelect from "../../components/FontFamilySelect.vue";
import type { ConfigTemplateGroup } from "../../components/config-template";
import AutoThemeGrid from "../../components/AutoThemeGrid.vue";
import GeneratedThemeEditor from "../../components/GeneratedThemeEditor.vue";
import {
  APP_THEMES,
  DARK_APP_THEMES,
} from "../../../shell/composables/use-app-theme";
import type { GeneratedThemeControls, GeneratedThemeTokens, ThemeMode, ThemeModeKind, ThemeSourceKind } from "../../../shell/theme/theme-types";
import {
  useMarkdownAppearance,
} from "../../../shell/composables/use-markdown-appearance";
import { useChatMessageAppearance, type ChatMarkdownLayout } from "../../../shell/composables/use-chat-message-appearance";
import { SIDE_FILE_TAGS_AVAILABLE, useChatComposerAppearance } from "../../../shell/composables/use-chat-composer-appearance";
import { useFileReaderAppearance } from "../../../shell/composables/use-file-reader-appearance";

const props = defineProps<{
  uiLanguage: "zh-CN" | "en-US" | "zh-TW";
  uiFont?: string;
  codeFont?: string;
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "zh-TW"; label: string }>;
  currentTheme: string;
  themeMode: ThemeModeKind;
  themeSource: ThemeSourceKind;
  presetLightTheme: string;
  presetDarkTheme: string;
  generatedThemeControls: GeneratedThemeControls;
  generatedLightTokens: GeneratedThemeTokens;
  generatedDarkTokens: GeneratedThemeTokens;
  uiSizeScale: number;
}>();

const emit = defineEmits<{
  (e: "update:uiLanguage", value: string): void;
  (e: "update:uiFont", value: string): void;
  (e: "update:codeFont", value: string): void;
  (e: "update:uiSizeScale", value: number): void;
  (e: "setThemeMode", value: ThemeModeKind): void;
  (e: "setThemeSource", value: ThemeSourceKind): void;
  (e: "setPresetTheme", side: ThemeMode, value: string): void;
  (e: "updateGeneratedThemeControls", value: Partial<GeneratedThemeControls>): void;
  (e: "resetGeneratedTheme"): void;
}>();

const { t } = useI18n();
const templateValues = {};
const templateGroups = computed<ConfigTemplateGroup[]>(() => [
  {
    key: "text",
    title: t("appearance.text"),
    rows: [
      { key: "language", items: [] },
      { key: "markdown-font-scale", items: [] },
      ...(fontsAvailable.value
        ? [
            { key: "ui-font", items: [] },
            { key: "code-font", items: [] },
          ]
        : []),
    ],
  },
  {
    key: "chat-bubble",
    title: t("appearance.chatBubble"),
    rows: [
      { key: "chat-bubble-background", items: [] },
      { key: "chat-process-messages-folded", items: [] },
      { key: "chat-bubble-time", items: [] },
      { key: "chat-bubble-markdown-layout", items: [] },
    ],
  },
  {
    key: "input-panel",
    title: t("appearance.inputPanel"),
    rows: [
      ...(SIDE_FILE_TAGS_AVAILABLE ? [{ key: "input-side-file-tags", items: [] }] : []),
      { key: "input-ide-bridge-file-tags", items: [] },
    ],
  },
  {
    key: "file-reader",
    title: t("appearance.fileReader"),
    rows: [{ key: "file-reader-line-wrap", items: [] }],
  },
  {
    key: "ui-size-scale",
    title: t("appearance.uiSizeScale"),
    rows: [{ key: "ui-size-scale", items: [] }],
  },
  {
    key: "theme",
    title: t("appearance.theme"),
    rows: [{ key: "theme", items: [] }],
  },
]);
const uiSizePresets = computed(() => [
  { scale: 75, label: t("appearance.uiSizeCompact") },
  { scale: 100, label: t("appearance.uiSizeDefault") },
  { scale: 125, label: t("appearance.uiSizeLarge") },
  { scale: 150, label: t("appearance.uiSizeXLarge") },
]);
const uiSizePresetText = computed(() => {
  const preset = uiSizePresets.value.find((item) => item.scale === props.uiSizeScale);
  return (preset ?? uiSizePresets.value[1]).label;
});
const displayModeOptions = computed<Array<{ value: ThemeModeKind; label: string }>>(() => [
  { value: "light", label: t("appearance.displayMode.light") },
  { value: "dark", label: t("appearance.displayMode.dark") },
  { value: "auto", label: t("appearance.displayMode.auto") },
]);
/** 来源直接读 props，不做本地副本：tab 只在点击时上报，状态永远以后端那份为准。 */
function onDisplayModeChange(mode: ThemeModeKind) {
  if (mode === props.themeMode) return;
  emit("setThemeMode", mode);
}

/**
 * 「预设 / 自定义」只是切视图：这里不写回主题来源，也不重落主题。
 * 到底生效哪一套，由卡片上的勾决定（勾预设卡片 = 用预设，勾自定义卡片 = 用自定义）。
 */
const themePanel = ref<ThemeSourceKind>(props.themeSource);

function selectThemePanel(source: ThemeSourceKind) {
  themePanel.value = source;
}

/** 明暗两侧各记一张卡：勾就画在各自那张卡上，互不牵连 */
const activeLightCardId = computed(() => props.presetLightTheme);
const activeDarkCardId = computed(() => props.presetDarkTheme);
const markdownFontScaleOptions = computed(() => [
  { value: 0, label: t("appearance.markdownFontScaleLight") },
  { value: 1, label: t("appearance.markdownFontScaleHeavy") },
]);
const markdownLayoutOptions = computed<Array<{ value: ChatMarkdownLayout; label: string }>>(() => [
  { value: "compact", label: t("appearance.markdownLayoutCompact") },
  { value: "comfortable", label: t("appearance.markdownLayoutComfortable") },
  { value: "relaxed", label: t("appearance.markdownLayoutRelaxed") },
]);
const lightThemes = computed(() => APP_THEMES.filter((theme) => !DARK_APP_THEMES.has(theme)));
const darkThemes = computed(() => APP_THEMES.filter((theme) => DARK_APP_THEMES.has(theme)));
const {
  markdownFontScale,
  setMarkdownFontScale,
} = useMarkdownAppearance();
const {
  assistantBubbleBackgroundEnabled,
  processMessagesFolded,
  chatTimeDisplayMode,
  markdownLayout,
  setAssistantBubbleBackgroundEnabled,
  setProcessMessagesFolded,
  setChatTimeDisplayMode,
  setChatMarkdownLayout,
} = useChatMessageAppearance();
const {
  sideFileTagsEnabled,
  ideBridgeFileTagsEnabled,
  setSideFileTagsEnabled,
  setIdeBridgeFileTagsEnabled,
} = useChatComposerAppearance();
const {
  fileReaderLineWrapEnabled,
  setFileReaderLineWrapEnabled,
} = useFileReaderAppearance();

const fontsAvailable = computed(() => canUseTransportSystemFonts());
const systemFonts = ref<string[]>([]);
const monospaceFonts = ref<Set<string>>(new Set());
const fontsLoading = ref(false);

const uiFontOptions = computed(() => {
  const current = String(props.uiFont || "").trim();
  const set = new Set<string>();
  if (current && current !== "auto" && !monospaceFonts.value.has(current)) set.add(current);
  for (const name of systemFonts.value) {
    if (!monospaceFonts.value.has(name)) set.add(name);
  }
  return [...set];
});

const codeFontOptions = computed(() => {
  const current = String(props.codeFont || "").trim();
  const set = new Set<string>();
  // 无条件保留用户已保存的代码字体为候选项：即使系统字体枚举失败、或后端未将其标记为 monospace，
  // 下拉列表也必须能对应上当前选中的代码字体
  if (current && current !== "auto") set.add(current);
  for (const name of systemFonts.value) {
    if (monospaceFonts.value.has(name)) set.add(name);
  }
  return [...set];
});

onMounted(async () => {
  if (!fontsAvailable.value) return;
  fontsLoading.value = true;
  try {
    const fonts = await listTransportSystemFonts<{ family: string; monospace: boolean }[]>();
    if (Array.isArray(fonts)) {
      const families: string[] = [];
      const mono = new Set<string>();
      for (const item of fonts) {
        if (!item || typeof item.family !== "string" || !item.family.trim()) continue;
        families.push(item.family.trim());
        if (item.monospace) mono.add(item.family.trim());
      }
      systemFonts.value = families;
      monospaceFonts.value = mono;
    }
  } catch (error) {
    console.warn("[APPEARANCE] list_system_fonts failed:", error);
    systemFonts.value = [];
  } finally {
    fontsLoading.value = false;
  }
});

</script>
