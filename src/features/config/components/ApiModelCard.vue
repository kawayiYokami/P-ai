<template>
  <div :class="embedded ? 'transition' : 'card border border-base-300 bg-base-200/50 transition'">
    <div :class="embedded ? 'py-3' : 'card-body gap-0 p-4'">
      <!-- ========== 头部：标题 + 摘要徽章 + 折叠/删除（同一行） ========== -->
      <div
        class="flex items-center gap-2"
        role="button"
        :aria-expanded="open"
        @click="toggleOpen"
      >
        <div class="min-w-0 flex-1">
          <template v-if="editingTitle">
            <input
              ref="titleInputRef"
              v-model="titleDraft"
              class="input input-bordered input-sm w-full"
              :placeholder="t('config.api.displayNamePlaceholder')"
              @click.stop
              @blur="commitTitle"
              @keydown.enter.prevent="commitTitle"
              @keydown.esc.stop.prevent="cancelTitle"
            />
          </template>
          <template v-else>
            <div class="flex min-w-0 items-center gap-1.5">
              <button
                class="flex min-w-0 items-center gap-1.5 text-left"
                type="button"
                :title="t('config.api.editDisplayName')"
                @click.stop="startEditTitle"
              >
                <span :class="embedded ? 'text-base font-semibold truncate' : 'card-title text-base mb-0 truncate'">{{ displayTitle }}</span>
                <AppIcon name="pencil" class="h-3.5 w-3.5 shrink-0 opacity-50" />
              </button>
              <span v-if="hint" class="min-w-0 truncate text-xs font-normal opacity-60">{{ hint }}</span>
            </div>
          </template>
        </div>
        <div v-if="hasBadges" class="flex shrink-0 items-center gap-1.5 overflow-hidden">
          <span v-for="cap in enabledCapabilities" :key="cap" class="badge badge-sm badge-ghost">
            {{ cap }}
          </span>
          <span v-for="effort in reasoningBadges" :key="effort" class="badge badge-sm badge-outline">
            {{ effort }}
          </span>
          <span
            v-if="contextBadge"
            class="badge badge-sm badge-ghost"
            :title="t('config.api.contextWindow')"
          >
            {{ contextBadge }}
          </span>
          <span
            v-if="temperatureBadge"
            class="badge badge-sm badge-ghost"
            :title="t('config.api.temperature')"
          >
            {{ temperatureBadge }}
          </span>
        </div>
        <div class="flex shrink-0 items-center gap-1">
          <button
            class="btn btn-sm btn-square btn-ghost"
            type="button"
            :title="open ? t('config.api.collapseModelCard') : t('config.api.expandModelCard')"
            @click.stop="toggleOpen"
          >
            <AppIcon name="chevron-down" class="h-4 w-4 transition-transform duration-200" :class="open ? '' : '-rotate-90'" />
          </button>
          <button
            v-if="showDelete"
            class="btn btn-sm btn-square btn-ghost"
            type="button"
            :class="deleteDisabled ? 'text-base-content/30' : 'text-error'"
            :disabled="deleteDisabled"
            @click.stop="emit('remove')"
          >
            <AppIcon name="trash" class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>

      <!-- ========== 展开区 ========== -->
      <div
        class="grid transition-all duration-200"
        :class="open ? 'grid-rows-[1fr] opacity-100' : 'grid-rows-[0fr] opacity-0'"
      >
        <div class="overflow-hidden">
          <div class="mt-3 grid gap-3">
            <label class="relative flex flex-col gap-1">
              <span class="text-sm font-medium">{{ t("config.api.model") }}</span>
              <div class="join">
                <input
                  v-model="card.model"
                  class="input input-bordered input-sm join-item flex-1"
                  placeholder="model"
                  @focus="emit('select')"
                  @blur="emit('sync-metadata')"
                  @keydown.enter.prevent="emit('sync-metadata')"
                />
                <button
                  class="btn btn-sm join-item bg-base-300"
                  type="button"
                  :disabled="filteredModelOptions.length === 0"
                  @click="togglePicker"
                >
                  <AppIcon name="chevron-down" class="h-3.5 w-3.5" />
                </button>
                <button
                  v-if="hasModelInfo"
                  ref="infoButtonRef"
                  class="btn btn-sm join-item bg-base-300"
                  type="button"
                  :title="t('config.api.modelInfoTitle')"
                  @click.stop="toggleModelInfo"
                >
                  <AppIcon name="help-circle" class="h-3.5 w-3.5" />
                </button>
              </div>
              <div v-if="protocolHint" class="mt-1 text-xs opacity-70">
                {{ t("config.api.matchedProtocol", { protocol: protocolHint }) }}
              </div>
              <div
                v-if="modelInfoOpen"
                ref="infoPanelRef"
                class="absolute right-0 top-full z-20 mt-2 w-96 rounded-box border border-base-300 bg-base-100 p-5 shadow-xl"
              >
                <div class="mb-3 flex items-center justify-between">
                  <span class="text-sm font-semibold">{{ t("config.api.modelInfoTitle") }}</span>
                  <button
                    class="btn btn-xs btn-square btn-ghost"
                    type="button"
                    :title="t('common.close')"
                    @click="modelInfoOpen = false"
                  >
                    <AppIcon name="close" class="h-3.5 w-3.5" />
                  </button>
                </div>
                <p v-if="capability?.fuzzyMatch" class="mb-3 text-xs text-warning">
                  {{ t("config.api.modelInfoGuessed") }}
                </p>
                <template v-if="capability?.metadataFound === true">
                  <div class="grid gap-3 text-sm">
                    <div class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2.5">
                      <template v-if="providerText">
                        <span class="opacity-60">{{ t("config.api.modelInfoProvider") }}</span>
                        <span class="text-right">{{ providerText }}</span>
                      </template>
                      <template v-if="protocolText">
                        <span class="opacity-60">{{ t("config.api.modelInfoProtocol") }}</span>
                        <span class="text-right font-mono">{{ protocolText }}</span>
                      </template>
                      <template v-if="contextMaxText">
                        <span class="opacity-60">{{ t("config.api.modelInfoContextMax") }}</span>
                        <span class="text-right font-mono">{{ contextMaxText }}</span>
                      </template>
                      <template v-if="outputMaxText">
                        <span class="opacity-60">{{ t("config.api.modelInfoOutputMax") }}</span>
                        <span class="text-right font-mono">{{ outputMaxText }}</span>
                      </template>
                      <span class="opacity-60">{{ t("config.api.modelInfoReasoning") }}</span>
                      <span class="text-right">{{ reasoningText || t("config.api.modelInfoUnknown") }}</span>
                    </div>
                    <div v-if="supportedCapabilities.length > 0" class="border-t border-base-300 pt-3">
                      <div class="flex flex-wrap gap-2">
                        <span v-for="cap in supportedCapabilities" :key="cap" class="badge badge-sm badge-ghost">
                          {{ cap }}
                        </span>
                      </div>
                    </div>
                  </div>
                </template>
                <p v-else class="text-sm opacity-60">{{ t("config.api.modelInfoNotFound") }}</p>
                <button
                  v-if="documentationUrl"
                  type="button"
                  class="btn btn-sm mt-4 w-full bg-base-200"
                  @click="emit('open-documentation')"
                >
                  <AppIcon name="book-open" class="h-3.5 w-3.5" />
                  {{ t("config.api.modelInfoOpenDocs") }}
                </button>
              </div>
            </label>
            <div v-if="pickerOpen" class="rounded-box border border-base-300 bg-base-200/50 p-3">
              <input
                v-model="modelSearch"
                class="input input-bordered input-sm mb-2 w-full"
                :placeholder="t('config.api.searchModel')"
                @keydown.esc.stop.prevent="closePicker"
              />
              <div class="max-h-48 overflow-auto">
                <button
                  v-for="option in filteredModels"
                  :key="option"
                  class="btn btn-ghost btn-sm mb-1 mr-1"
                  type="button"
                  @click="selectOption(option)"
                >
                  {{ option }}
                </button>
                <div v-if="filteredModels.length === 0" class="px-2 py-3 text-sm opacity-50">{{
                  t("config.api.noModelFound") }}</div>
              </div>
            </div>

            <div v-if="showCapabilityToggles" class="flex flex-wrap gap-3">
              <label
                class="flex min-w-40 flex-1 items-center justify-between rounded-box border border-base-300 bg-base-300 px-3 py-2"
              >
                <span class="text-sm">{{ t("config.api.capImage") }}</span>
                <input v-model="card.enableImage" type="checkbox" class="checkbox checkbox-sm" />
              </label>
              <label
                class="flex min-w-40 flex-1 items-center justify-between rounded-box border border-base-300 bg-base-300 px-3 py-2"
              >
                <span class="text-sm">{{ t("config.api.capAudio") }}</span>
                <input v-model="card.enableAudio" type="checkbox" class="checkbox checkbox-sm" />
              </label>
              <label
                class="flex min-w-40 flex-1 items-center justify-between rounded-box border border-base-300 bg-base-300 px-3 py-2"
              >
                <span class="text-sm">{{ t("config.api.capVideo") }}</span>
                <input v-model="card.enableVideo" type="checkbox" class="checkbox checkbox-sm" />
              </label>
              <label
                class="flex min-w-40 flex-1 items-center justify-between rounded-box border border-base-300 bg-base-300 px-3 py-2"
              >
                <span class="text-sm">{{ t("config.api.temperature") }}</span>
                <input v-model="card.customTemperatureEnabled" type="checkbox" class="checkbox checkbox-sm" />
              </label>
              <label
                class="flex min-w-40 flex-1 items-center justify-between rounded-box border border-base-300 bg-base-300 px-3 py-2"
              >
                <span class="text-sm">{{ t("config.api.maxOutputTokens") }}</span>
                <input
                  v-model="card.customMaxOutputTokensEnabled"
                  type="checkbox"
                  class="checkbox checkbox-sm"
                  @change="emit('toggle-max-output')"
                />
              </label>
            </div>

            <div v-if="showContextWindow" class="grid gap-3">
              <label class="flex flex-col gap-1">
                <span class="text-sm font-medium">{{ t("config.api.contextWindow") }}</span>
                <div class="flex items-center gap-2">
                  <input
                    :value="card.contextWindowTokens"
                    @input="card.contextWindowTokens = Number(($event.target as HTMLInputElement).value)"
                    type="range"
                    :min="SLIDER_CONTEXT_MIN"
                    :max="contextWindowMax"
                    step="1000"
                    class="range range-sm flex-1"
                  />
                  <div class="relative w-28">
                    <input
                      :value="Math.round(Number(card.contextWindowTokens || 0) / 1000)"
                      @input="card.contextWindowTokens = Number(($event.target as HTMLInputElement).value || 0) * 1000"
                      @blur="clampManualContextWindowValue"
                      type="number"
                      :min="Math.round(SLIDER_CONTEXT_MIN / 1000)"
                      :max="2000"
                      step="1"
                      class="input input-bordered input-sm w-full pr-7 text-right font-mono"
                    />
                    <span class="pointer-events-none absolute inset-y-0 right-2 flex items-center text-xs opacity-70">K</span>
                  </div>
                </div>
              </label>

              <div v-if="showReasoning" class="flex flex-col gap-2">
                <div class="flex items-center gap-2">
                  <span class="text-sm font-medium">{{ t("config.api.reasoningEffort") }}</span>
                  <span
                    v-if="reasoningStatus === 'unknown'"
                    class="text-xs opacity-60"
                    :title="t('config.api.reasoningCapabilityUnknown')"
                  >
                    {{ t("config.api.reasoningCapabilityUnknown") }}
                  </span>
                  <span
                    v-else-if="reasoningStatus === 'unsupported'"
                    class="text-xs text-warning"
                    :title="t('config.api.reasoningCapabilityUnsupported')"
                  >
                    {{ t("config.api.reasoningCapabilityUnsupported") }}
                  </span>
                </div>
                <div class="flex flex-wrap gap-x-4 gap-y-2">
                  <label
                    v-for="item in reasoningItems"
                    :key="item.value"
                    class="flex items-center gap-2 text-sm"
                    :class="item.disabled ? 'cursor-not-allowed opacity-50' : ''"
                    :title="item.disabled ? t('config.api.reasoningEffortUnsupported') : undefined"
                  >
                    <input
                      type="checkbox"
                      class="checkbox checkbox-sm"
                      :checked="reasoningCheckedValues.includes(item.value)"
                      :disabled="item.disabled"
                      @change="emit('reasoning-change', { value: item.value, checked: ($event.target as HTMLInputElement).checked })"
                    />
                    <span>{{ item.label }}</span>
                  </label>
                </div>
              </div>

              <label v-if="showTemperature && card.customTemperatureEnabled" class="flex flex-col gap-1">
                <span class="text-sm font-medium">{{ t("config.api.temperature") }}</span>
                <div class="flex items-center gap-2">
                  <input
                    :value="card.temperature"
                    @input="card.temperature = Number(($event.target as HTMLInputElement).value)"
                    type="range"
                    min="0"
                    max="2"
                    step="0.1"
                    class="range range-sm flex-1"
                  />
                  <span class="text-xs font-mono w-8 text-right">{{ Number(card.temperature || 0).toFixed(1) }}</span>
                </div>
              </label>

              <label v-if="showMaxOutputTokens && card.customMaxOutputTokensEnabled" class="flex flex-col gap-1">
                <span class="text-sm font-medium">{{ t("config.api.maxOutputTokens") }}</span>
                <div class="flex items-center gap-2">
                  <input
                    :value="card.maxOutputTokens"
                    @input="card.maxOutputTokens = Number(($event.target as HTMLInputElement).value)"
                    type="range"
                    min="8192"
                    max="128000"
                    step="256"
                    class="range range-sm flex-1"
                  />
                  <input
                    :value="Math.round(Number(card.maxOutputTokens ?? 0))"
                    @input="card.maxOutputTokens = Number(($event.target as HTMLInputElement).value || 0)"
                    type="number"
                    step="1"
                    class="input input-bordered input-sm w-28 text-right font-mono"
                  />
                </div>
              </label>
            </div>

            <div
              v-if="connectionResult"
              class="rounded-box border px-3 py-2 text-xs"
              :class="connectionResult.success ? 'border-success/30 text-success' : 'border-error/30 text-error'"
            >
              {{ connectionResult.success
                ? t('config.api.testConnectionSuccess', { latency: connectionResult.latencyMs })
                : t('config.api.testConnectionFailed', { error: connectionResult.error }) }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <dialog ref="renameDialogRef" class="modal">
      <div class="modal-box max-w-sm">
        <h3 class="text-lg font-semibold">{{ t("config.api.renameDisplayNameTitle") }}</h3>
        <p class="py-3 text-sm opacity-80">{{ t("config.api.renameDisplayNameHint") }}</p>
        <div class="modal-action">
          <button class="btn btn-ghost" type="button" @click="cancelRename">
            {{ t("common.cancel") }}
          </button>
          <button class="btn btn-primary" type="button" @click="confirmRename">
            {{ t("common.confirm") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button>close</button>
      </form>
    </dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import AppIcon from "../../shared/components/AppIcon.vue";
import type { ApiModelConfigItem } from "../../../types/app";
import { reasoningEffortDisplayLabel } from "../utils/api-config-display";
import type { ModelCapabilitySnapshot } from "../utils/model-capability";

const SLIDER_CONTEXT_MIN = 16_000;

export interface ReasoningEffortItem {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface ModelConnectionResult {
  success: boolean;
  latencyMs?: number;
  error?: string;
}

type ModelCapabilityInfo = Partial<ModelCapabilitySnapshot> & { metadataFound?: boolean };

const props = withDefaults(defineProps<{
  card: ApiModelConfigItem;
  title?: string;
  hint?: string;
  modelOptions?: string[];
  showDelete?: boolean;
  deleteDisabled?: boolean;
  showCapabilityToggles?: boolean;
  showContextWindow?: boolean;
  showReasoning?: boolean;
  showTemperature?: boolean;
  showMaxOutputTokens?: boolean;
  reasoningItems?: ReasoningEffortItem[];
  reasoningCheckedValues?: string[];
  reasoningStatus?: "known" | "unknown" | "unsupported";
  protocolHint?: string;
  documentationUrl?: string;
  connectionResult?: ModelConnectionResult | null;
  contextWindowMax?: number;
  defaultOpen?: boolean;
  capability?: ModelCapabilityInfo | null;
  embedded?: boolean;
}>(), {
  title: "",
  hint: "",
  modelOptions: () => [],
  showDelete: true,
  deleteDisabled: false,
  showCapabilityToggles: true,
  showContextWindow: true,
  showReasoning: true,
  showTemperature: true,
  showMaxOutputTokens: true,
  reasoningItems: () => [],
  reasoningCheckedValues: () => [],
  reasoningStatus: "unknown",
  protocolHint: "",
  documentationUrl: "",
  connectionResult: null,
  contextWindowMax: 2_000_000,
  defaultOpen: true,
  capability: null,
  embedded: false,
});

const emit = defineEmits<{
  (event: "select"): void;
  (event: "remove"): void;
  (event: "sync-metadata"): void;
  (event: "select-option", option: string): void;
  (event: "toggle-max-output"): void;
  (event: "reasoning-change", payload: { value: string; checked: boolean }): void;
  (event: "open-documentation"): void;
}>();

const { t } = useI18n();

const open = ref(props.defaultOpen);

function toggleOpen() {
  open.value = !open.value;
  if (!open.value) modelInfoOpen.value = false;
}

// ========== 模型信息弹层 ==========

const modelInfoOpen = ref(false);
const infoButtonRef = ref<HTMLElement | null>(null);
const infoPanelRef = ref<HTMLElement | null>(null);

const hasModelInfo = computed(
  () => props.capability?.metadataFound === true || Boolean(props.documentationUrl),
);

function toggleModelInfo() {
  modelInfoOpen.value = !modelInfoOpen.value;
}

function onDocumentMousedown(event: MouseEvent) {
  const target = event.target as Node;
  if (infoButtonRef.value?.contains(target)) return;
  if (infoPanelRef.value?.contains(target)) return;
  modelInfoOpen.value = false;
}

onMounted(() => document.addEventListener("mousedown", onDocumentMousedown));
onUnmounted(() => document.removeEventListener("mousedown", onDocumentMousedown));

const contextMaxText = computed(() => {
  const tokens = Number(props.capability?.contextWindowMax || 0);
  return tokens > 0 ? `${Math.round(tokens / 1000)}K` : "";
});

const outputMaxText = computed(() => {
  const tokens = Number(props.capability?.maxOutputTokensMax || 0);
  return tokens > 0 ? `${Math.round(tokens / 1000)}K` : "";
});

const providerText = computed(() => String(props.capability?.providerName || "").trim());

const protocolText = computed(() => String(props.capability?.providerApi || "").trim());

const supportedCapabilities = computed<string[]>(() => {
  const capability = props.capability;
  if (!capability || capability.metadataFound !== true) return [];
  const candidates: Array<{ enabled: boolean | undefined; label: string }> = [
    { enabled: capability.enableImage, label: t("config.api.capImage") },
    { enabled: capability.enableAudio, label: t("config.api.capAudio") },
    { enabled: capability.enableVideo, label: t("config.api.capVideo") },
  ];
  return candidates.filter((item) => item.enabled === true).map((item) => item.label);
});

const reasoningText = computed(() => {
  const reasoning = props.capability?.reasoning;
  if (!reasoning) return "";
  if (reasoning.supportsReasoning === false) return t("config.api.modelInfoReasoningUnsupported");
  const options = (reasoning.reasoningEffortOptions || [])
    .filter((value) => String(value || "").trim().toLowerCase() !== "default")
    .map((value) => reasoningEffortDisplayLabel(value, t));
  return options.length > 0 ? options.join("、") : t("config.api.modelInfoReasoningSupported");
});

const pickerOpen = ref(false);
const modelSearch = ref("");

const displayTitle = computed(() => {
  const explicit = String(props.title || "").trim();
  if (explicit) return explicit;
  const displayName = String(props.card.displayName || "").trim();
  if (displayName) return displayName;
  return String(props.card.model || "").trim() || t("config.api.unnamedModel");
});

const editingTitle = ref(false);
const titleDraft = ref("");
const titleInputRef = ref<HTMLInputElement | null>(null);
const renameDialogRef = ref<HTMLDialogElement | null>(null);
const pendingDisplayName = ref("");

function startEditTitle() {
  titleDraft.value = props.card.displayName ?? "";
  editingTitle.value = true;
  void nextTick(() => titleInputRef.value?.focus());
}

function commitTitle() {
  if (!editingTitle.value) return;
  const next = titleDraft.value.trim();
  editingTitle.value = false;
  const current = String(props.card.displayName || "").trim();
  if (next === current) return;
  pendingDisplayName.value = next;
  renameDialogRef.value?.showModal();
}

function confirmRename() {
  props.card.displayName = pendingDisplayName.value;
  pendingDisplayName.value = "";
  renameDialogRef.value?.close();
  emit("sync-metadata");
}

function cancelRename() {
  pendingDisplayName.value = "";
  renameDialogRef.value?.close();
}

// ========== 模型 ID 变更时显示名跟随重置 ==========

watch(
  () => props.card.model,
  (val) => {
    const id = String(val || "").trim();
    if (id) props.card.displayName = id;
  },
);

function cancelTitle() {
  editingTitle.value = false;
}

// ========== 头部徽章 ==========

const enabledCapabilities = computed<string[]>(() => {
  if (!props.showCapabilityToggles) return [];
  const candidates: Array<{ enabled: boolean; label: string }> = [
    { enabled: props.card.enableImage === true, label: t("config.api.capImage") },
    { enabled: props.card.enableAudio === true, label: t("config.api.capAudio") },
    { enabled: props.card.enableVideo === true, label: t("config.api.capVideo") },
  ];
  return candidates.filter((item) => item.enabled).map((item) => item.label);
});

const reasoningBadges = computed(() => {
  if (!props.showReasoning) return [];
  return props.reasoningCheckedValues
    .filter((value) => String(value || "").trim().toLowerCase() !== "default")
    .map((value) => props.reasoningItems.find((item) => item.value === value)?.label || "")
    .filter(Boolean);
});

const contextBadge = computed(() => {
  if (!props.showContextWindow) return "";
  const tokens = Number(props.card.contextWindowTokens || 0);
  return tokens > 0 ? `${Math.round(tokens / 1000)}K` : "";
});

const temperatureBadge = computed(() => {
  if (!props.showTemperature || props.card.customTemperatureEnabled !== true) return "";
  return Number(props.card.temperature || 0).toFixed(1);
});

const hasBadges = computed(
  () =>
    enabledCapabilities.value.length > 0 ||
    reasoningBadges.value.length > 0 ||
    Boolean(contextBadge.value) ||
    Boolean(temperatureBadge.value),
);

// ========== 模型选择 ==========

const filteredModelOptions = computed(() => {
  const options = Array.from(new Set([
    ...(props.modelOptions || []),
    String(props.card.model || "").trim(),
  ].map((item) => String(item || "").trim()).filter(Boolean)));
  return options;
});

const filteredModels = computed(() => {
  const search = modelSearch.value.trim().toLowerCase();
  if (!search) return filteredModelOptions.value;
  return filteredModelOptions.value.filter((item) => item.toLowerCase().includes(search));
});

function togglePicker() {
  pickerOpen.value = !pickerOpen.value;
  modelSearch.value = "";
}

function closePicker() {
  pickerOpen.value = false;
  modelSearch.value = "";
}

function selectOption(option: string) {
  props.card.model = option;
  closePicker();
  emit("select");
  emit("select-option", option);
}

function clampManualContextWindowValue() {
  const nextContext = Math.round(Number(props.card.contextWindowTokens ?? 256_000));
  const clampedContext = Math.max(SLIDER_CONTEXT_MIN, Math.min(2_000_000, nextContext));
  if (!Number.isFinite(nextContext)) {
    props.card.contextWindowTokens = 256_000;
    return;
  }
  if (nextContext !== clampedContext) {
    props.card.contextWindowTokens = clampedContext;
  }
}
</script>
