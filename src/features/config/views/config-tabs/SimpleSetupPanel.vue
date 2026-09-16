<template>
  <SettingsStickyLayout>
    <template #header>
      <div class="grid gap-2">
        <div v-if="errorText" class="alert alert-error py-2 text-sm">{{ errorText }}</div>
        <div v-if="statusText" class="alert alert-success py-2 text-sm">{{ statusText }}</div>
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0 flex-1">
            <h2 class="text-sm font-semibold text-base-content">{{ t("simpleSetup.welcomeTitle") }}</h2>
            <div class="text-xs leading-relaxed opacity-60 mt-0.5">
              {{ t("simpleSetup.advancedHint") }}
            </div>
          </div>
          <button
            class="btn btn-primary btn-sm shrink-0"
            type="button"
            :disabled="saving || loading"
            @click="handleSave"
          >
            <span v-if="saving" class="loading loading-spinner loading-xs"></span>
            {{ t("simpleSetup.saveAndStart") }}
          </button>
        </div>
      </div>
    </template>

    <div v-if="loading" class="flex min-h-48 items-center justify-center">
      <span class="loading loading-spinner loading-md"></span>
    </div>

    <div v-else class="grid gap-4 pb-6">
      <!-- 界面语言 -->
      <section class="card bg-base-100 border border-base-300">
        <div class="card-body gap-3 p-4">
          <h3 class="text-sm font-semibold">{{ t("simpleSetup.appearance") }}</h3>
          <div class="grid gap-1.5">
            <span class="text-xs font-medium opacity-70">{{ t("appearance.language") }}</span>
            <div class="grid grid-cols-3 gap-2">
              <button
                v-for="option in languageOptions"
                :key="option.value"
                class="btn btn-sm"
                :class="draft.uiLanguage === option.value ? 'btn-primary' : 'bg-base-200'"
                type="button"
                @click="setUiLanguage(option.value)"
              >
                {{ option.label }}
              </button>
            </div>
          </div>
        </div>
      </section>

      <!-- 模型供应商 -->
      <section class="card bg-base-100 border border-base-300">
        <div class="card-body gap-3 p-4">
          <h3 class="text-sm font-semibold">{{ t("simpleSetup.provider") }}</h3>
          <!-- 供应商网格：2 列对称布局 -->
          <div class="grid grid-cols-2 gap-2">
            <button
              v-for="option in providerOptions"
              :key="option.id"
              class="btn btn-sm"
              :class="draft.providerId === option.id ? 'btn-primary' : 'bg-base-200'"
              type="button"
              @click="selectProvider(option.id)"
            >
              {{ option.label }}
            </button>
          </div>

          <!-- 自定义协议与 Base URL -->
          <template v-if="draft.providerId === 'custom'">
            <label class="grid gap-1.5">
              <span class="text-xs font-medium opacity-70">{{ t("simpleSetup.apiProtocol") }}</span>
              <select v-model="draft.customRequestFormat" class="select select-bordered select-sm font-mono">
                <option v-for="option in simpleSetupProtocolOptions" :key="option.value" :value="option.value">
                  {{ option.label }}
                </option>
              </select>
            </label>
            <label class="grid gap-1.5">
              <span class="text-xs font-medium opacity-70">base_url</span>
              <input v-model.trim="draft.customBaseUrl" class="input input-bordered input-sm font-mono" />
            </label>
          </template>

          <div class="divider divider-sm my-0"></div>

          <!-- API Key 与连通性测试 -->
          <div class="grid gap-1.5">
            <div class="flex items-center justify-between">
              <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.apiKey") }}</span>
              <button
                v-if="providerApiKeyUrl"
                class="text-xs text-primary hover:underline"
                type="button"
                @click="openProviderKeyUrl"
              >
                {{ t("quickSetup.actions.getKey") }} ↗
              </button>
            </div>
            <div class="flex items-center gap-2">
              <input
                v-model.trim="draft.apiKey"
                :type="showApiKey ? 'text' : 'password'"
                class="input input-bordered input-sm min-w-0 flex-1 font-mono"
                placeholder="sk-..."
              />
              <button
                class="btn btn-sm btn-square bg-base-200 shrink-0"
                type="button"
                :aria-label="showApiKey ? t('quickSetup.actions.hideKey') : t('quickSetup.actions.showKey')"
                @click="showApiKey = !showApiKey"
              >
                <EyeOff v-if="showApiKey" class="h-3.5 w-3.5" />
                <Eye v-else class="h-3.5 w-3.5" />
              </button>
              <button
                class="btn btn-sm bg-base-200 shrink-0"
                type="button"
                :disabled="testingConnection || !draft.apiKey.trim()"
                @click="testConnection"
              >
                <span v-if="testingConnection" class="loading loading-spinner loading-xs"></span>
                <CheckCircle2 v-else class="h-3.5 w-3.5" />
                {{ testingConnection ? t("simpleSetup.testingConnection") : t("simpleSetup.testConnection") }}
              </button>
            </div>

            <!-- 连通性测试反馈 -->
            <div v-if="connectionTestResult" class="mt-1">
              <div
                class="text-xs flex items-center gap-1.5 rounded px-2.5 py-1.5"
                :class="connectionTestResult.ok ? 'bg-success/15 text-success' : 'bg-error/15 text-error'"
              >
                <CheckCircle2 v-if="connectionTestResult.ok" class="h-3.5 w-3.5 shrink-0" />
                <AlertCircle v-else class="h-3.5 w-3.5 shrink-0" />
                <span class="min-w-0 break-all">{{ connectionTestResult.message }}</span>
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- 模型配置（极简三合一卡片） -->
      <section class="card bg-base-100 border border-base-300">
        <fieldset class="fieldset gap-3 p-4">
          <legend class="fieldset-legend w-full text-sm">
            <span>{{ t("simpleSetup.models") }}</span>
            <button
              v-if="draft.providerId === 'custom'"
              class="btn btn-sm bg-base-200"
              type="button"
              :class="{ loading: refreshingCustomModels }"
              :disabled="refreshingCustomModels"
              @click="refreshCustomModels"
            >
              <RefreshCw class="h-3.5 w-3.5" />
              {{ t("config.api.refreshModels") }}
            </button>
          </legend>

          <div class="grid gap-2.5">
            <div
              v-for="card in modelCards"
              :key="card.id"
              class="rounded-box border border-base-300 bg-base-200/40 p-3 flex flex-col sm:flex-row sm:items-center justify-between gap-2.5"
            >
              <label
                class="label w-full min-w-0 flex-1 cursor-pointer flex-col items-start gap-0.5 whitespace-normal"
                :for="`simple-model-${card.id}`"
              >
                <span class="flex items-center gap-2">
                  <component :is="card.icon" class="h-4 w-4 shrink-0 text-primary" />
                  <span class="text-sm font-semibold text-base-content">{{ card.label }}</span>
                </span>
                <span class="text-xs font-normal">{{ card.hint }}</span>
              </label>
              <div class="shrink-0 sm:w-64">
                <template v-if="draft.providerId === 'custom'">
                  <input
                    :id="`simple-model-${card.id}`"
                    v-model.trim="draft.models[card.id].model"
                    :list="`custom-model-options-${card.id}`"
                    class="input input-bordered input-sm font-mono w-full"
                    :placeholder="t('simpleSetup.modelPlaceholder')"
                  />
                  <datalist :id="`custom-model-options-${card.id}`">
                    <option v-for="opt in draft.customModelOptions" :key="opt" :value="opt"></option>
                  </datalist>
                </template>
                <input
                  v-else
                  :id="`simple-model-${card.id}`"
                  :value="draft.models[card.id].model || card.fallback"
                  class="input input-bordered input-sm font-mono w-full truncate text-center text-base-content/80 sm:text-right"
                  readonly
                />
              </div>
            </div>
          </div>
        </fieldset>
      </section>

      <!-- 快捷键配置 -->
      <section class="card bg-base-100 border border-base-300">
        <div class="card-body gap-3 p-4">
          <h3 class="text-sm font-semibold">{{ t("simpleSetup.hotkeys") }}</h3>
          <div class="grid gap-3">
            <label class="grid gap-1.5">
              <div class="flex items-center justify-between">
                <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.summonHotkey") }}</span>
                <button
                  v-if="draft.hotkey !== 'Alt+·'"
                  class="text-caption text-base-content/60 hover:text-primary transition-colors"
                  type="button"
                  @click="resetHotkeyDefault('summon')"
                >
                  {{ t("simpleSetup.resetDefault") }}
                </button>
              </div>
              <div class="grid grid-cols-[minmax(0,1fr)_auto] gap-2">
                <input :value="draft.hotkey" class="input input-bordered input-sm font-mono" readonly />
                <button
                  class="btn btn-sm"
                  :class="hotkeyCaptureTarget === 'summon' ? 'btn-primary animate-pulse' : 'bg-base-200'"
                  type="button"
                  @click="startHotkeyCapture('summon')"
                >
                  {{ hotkeyCaptureTarget === 'summon' ? t("quickSetup.hotkeyHints.recording") : t("quickSetup.actions.record") }}
                </button>
              </div>
            </label>
            <label class="grid gap-1.5">
              <div class="flex items-center justify-between">
                <span class="text-xs font-medium opacity-70">{{ t("quickSetup.fields.recordHotkey") }}</span>
                <button
                  v-if="draft.recordHotkey !== 'CapsLock'"
                  class="text-caption text-base-content/60 hover:text-primary transition-colors"
                  type="button"
                  @click="resetHotkeyDefault('record')"
                >
                  {{ t("simpleSetup.resetDefault") }}
                </button>
              </div>
              <div class="grid grid-cols-[minmax(0,1fr)_auto] gap-2">
                <input :value="draft.recordHotkey" class="input input-bordered input-sm font-mono" readonly />
                <button
                  class="btn btn-sm"
                  :class="hotkeyCaptureTarget === 'record' ? 'btn-primary animate-pulse' : 'bg-base-200'"
                  type="button"
                  @click="startHotkeyCapture('record')"
                >
                  {{ hotkeyCaptureTarget === 'record' ? t("quickSetup.hotkeyHints.recording") : t("quickSetup.actions.record") }}
                </button>
              </div>
            </label>
            <div v-if="hotkeyCaptureTarget" class="text-xs text-primary font-medium">
              {{ hotkeyCaptureHint }}
            </div>
          </div>
        </div>
      </section>

      <!-- 语音与记忆加速（可选推荐） -->
      <section class="card bg-base-100 border border-base-300">
        <div class="card-body gap-3 p-4">
          <div class="flex items-center justify-between gap-2">
            <div class="flex items-center gap-2">
              <Sparkles class="h-4 w-4 text-warning shrink-0" />
              <h3 class="text-sm font-semibold">{{ t("simpleSetup.accelerationTitle") }}</h3>
            </div>
            <button
              class="text-xs text-primary hover:underline"
              type="button"
              @click="openSiliconFlowKeyUrl"
            >
              {{ t("quickSetup.actions.getKey") }} ↗
            </button>
          </div>
          <div class="text-xs opacity-60 leading-relaxed">
            {{ t("simpleSetup.accelerationDesc") }}
          </div>
          <div class="grid gap-1.5 mt-1">
            <div class="flex items-center gap-2">
              <input
                v-model.trim="draft.siliconFlowKey"
                :type="showSiliconFlowKey ? 'text' : 'password'"
                class="input input-bordered input-sm min-w-0 flex-1 font-mono"
                placeholder="sk-..."
              />
              <button
                class="btn btn-sm btn-square bg-base-200 shrink-0"
                type="button"
                :aria-label="showSiliconFlowKey ? t('quickSetup.actions.hideKey') : t('quickSetup.actions.showKey')"
                @click="showSiliconFlowKey = !showSiliconFlowKey"
              >
                <EyeOff v-if="showSiliconFlowKey" class="h-3.5 w-3.5" />
                <Eye v-else class="h-3.5 w-3.5" />
              </button>
            </div>
          </div>
        </div>
      </section>
    </div>
  </SettingsStickyLayout>
</template>

<script setup lang="ts">
import { computed, onMounted } from "vue";
import { useI18n } from "vue-i18n";
import {
  AlertCircle,
  Brain,
  CheckCircle2,
  Eye,
  EyeOff,
  RefreshCw,
  Sparkles,
  Zap,
} from "@lucide/vue";
import SettingsStickyLayout from "../../components/SettingsStickyLayout.vue";
import { openTransportWindow, hideCurrentTransportWindow } from "../../../../services/tauri-api";
import {
  clearSimpleSetupDraft,
  saveSimpleSetupDraft,
  simpleProviderOptions,
  simpleSetupProtocolOptions,
  useSimpleSetup,
} from "../../quick-setup/use-simple-setup";
import type { SimpleModelCard } from "../../quick-setup/use-simple-setup";

const { t } = useI18n();

const {
  loading,
  saving,
  errorText,
  statusText,
  showApiKey,
  showSiliconFlowKey,
  hotkeyCaptureTarget,
  hotkeyCaptureHint,
  refreshingCustomModels,
  testingConnection,
  connectionTestResult,
  draft,
  languageOptions,
  providerApiKeyUrl,
  selectedProvider,
  loadSnapshot,
  selectProvider,
  testConnection,
  resetHotkeyDefault,
  refreshCustomModels,
  openProviderKeyUrl,
  openSiliconFlowKeyUrl,
  setUiLanguage,
  startHotkeyCapture,
  saveAll,
} = useSimpleSetup();

const providerOptions = simpleProviderOptions.filter((option) => option.id !== "opencode");

const modelCards = computed(() => {
  const preset = selectedProvider.value;
  const defaultModel = preset.defaultModel;
  const visionModel = preset.visionModel ?? preset.defaultModel;
  return [
    { id: "quick" as SimpleModelCard, label: t("simpleSetup.modelQuick"), hint: t("simpleSetup.modelQuickHint"), icon: Zap, fallback: defaultModel },
    { id: "expert" as SimpleModelCard, label: t("simpleSetup.modelExpert"), hint: t("simpleSetup.modelExpertHint"), icon: Brain, fallback: defaultModel },
    { id: "vision" as SimpleModelCard, label: t("simpleSetup.modelVision"), hint: t("simpleSetup.modelVisionHint"), icon: Eye, fallback: visionModel },
  ];
});

onMounted(async () => {
  await loadSnapshot();
});

async function handleSave() {
  saveSimpleSetupDraft({ ...draft });
  await saveAll();
  if (!errorText.value) {
    clearSimpleSetupDraft();
    await openTransportWindow("chat");
    await hideCurrentTransportWindow();
  }
}
</script>
