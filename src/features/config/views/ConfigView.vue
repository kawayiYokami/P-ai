<template>
  <div class="h-full min-h-0 flex flex-col gap-2">
    <div class="tabs tabs-boxed tabs-sm shrink-0">
      <a class="tab" :class="{ 'tab-active': configTab === 'hotkey' }" @click="$emit('update:configTab', 'hotkey')">{{ t("config.tabs.hotkey") }}</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'api' }" @click="$emit('update:configTab', 'api')">{{ t("config.tabs.api") }}</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'tools' }" @click="$emit('update:configTab', 'tools')">{{ t("config.tabs.tools") }}</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'persona' }" @click="$emit('update:configTab', 'persona')">{{ t("config.tabs.persona") }}</a>
      <a class="tab" :class="{ 'tab-active': configTab === 'chatSettings' }" @click="$emit('update:configTab', 'chatSettings')">{{ t("config.tabs.chatSettings") }}</a>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto pr-1">
      <div class="grid gap-2">
        <HotkeyTab
          v-if="configTab === 'hotkey'"
          :config="config"
          :ui-language="uiLanguage"
          :locale-options="localeOptions"
          :current-theme="currentTheme"
          :hotkey-test-recording="hotkeyTestRecording"
          :hotkey-test-recording-ms="hotkeyTestRecordingMs"
          :hotkey-test-audio-ready="hotkeyTestAudioReady"
          @update:ui-language="$emit('update:uiLanguage', $event)"
          @toggle-theme="$emit('toggleTheme')"
          @start-hotkey-record-test="$emit('startHotkeyRecordTest')"
          @stop-hotkey-record-test="$emit('stopHotkeyRecordTest')"
          @play-hotkey-record-test="$emit('playHotkeyRecordTest')"
          @capture-hotkey="$emit('captureHotkey', $event)"
        />

        <ApiTab
          v-else-if="configTab === 'api'"
          :config="config"
          :selected-api-config="selectedApiConfig"
          :base-url-reference="baseUrlReference"
          :refreshing-models="refreshingModels"
          :model-options="modelOptions"
          :model-refresh-ok="modelRefreshOk"
          :model-refresh-error="modelRefreshError"
          :config-dirty="configDirty"
          :saving-config="savingConfig"
          @save-api-config="$emit('saveApiConfig')"
          @add-api-config="$emit('addApiConfig')"
          @remove-selected-api-config="$emit('removeSelectedApiConfig')"
          @refresh-models="$emit('refreshModels')"
        />

        <ToolsTab
          v-else-if="configTab === 'tools'"
          :config="config"
          :tool-api-config="toolApiConfig"
          :tool-statuses="toolStatuses"
          @open-memory-viewer="$emit('openMemoryViewer')"
        />

        <PersonaTab
          v-else-if="configTab === 'persona'"
          :personas="personas"
          :assistant-personas="assistantPersonas"
          :persona-editor-id="personaEditorId"
          :selected-persona="selectedPersona"
          :selected-persona-avatar-url="selectedPersonaAvatarUrl"
          :avatar-saving="avatarSaving"
          :avatar-error="avatarError"
          @update:persona-editor-id="$emit('update:personaEditorId', $event)"
          @add-persona="$emit('addPersona')"
          @remove-selected-persona="$emit('removeSelectedPersona')"
          @open-avatar-editor="openAvatarEditorForSelected"
        />

        <ChatSettingsTab
          v-else-if="configTab === 'chatSettings'"
          :config="config"
          :text-capable-api-configs="textCapableApiConfigs"
          :image-capable-api-configs="imageCapableApiConfigs"
          :assistant-personas="assistantPersonas"
          :selected-persona-id="selectedPersonaId"
          :response-style-options="responseStyleOptions"
          :response-style-id="responseStyleId"
          :cache-stats="cacheStats"
          :cache-stats-loading="cacheStatsLoading"
          @update:selected-persona-id="$emit('update:selectedPersonaId', $event)"
          @update:response-style-id="$emit('update:responseStyleId', $event)"
          @open-current-history="$emit('openCurrentHistory')"
          @open-prompt-preview="$emit('openPromptPreview')"
          @open-system-prompt-preview="$emit('openSystemPromptPreview')"
          @refresh-image-cache-stats="$emit('refreshImageCacheStats')"
          @clear-image-cache="$emit('clearImageCache')"
        />
      </div>
    </div>
  </div>

  <input ref="avatarFileInput" type="file" accept="image/*" class="hidden" @change="onAvatarFilePicked" />
  <dialog ref="avatarEditorDialog" class="modal">
    <div class="modal-box p-3 max-w-sm">
      <h3 class="text-sm font-semibold mb-2">{{ t("config.persona.editAvatar") }}</h3>
      <div class="rounded border border-base-300 bg-base-100 p-3">
        <div class="flex items-center gap-3">
          <div v-if="avatarEditorAvatarUrl" class="avatar">
            <div class="w-14 rounded-full">
              <img :src="avatarEditorAvatarUrl" :alt="avatarEditorName" :title="avatarEditorName" />
            </div>
          </div>
          <div v-else class="avatar placeholder">
            <div class="bg-neutral text-neutral-content w-14 rounded-full">
              <span>{{ avatarInitial(avatarEditorName) }}</span>
            </div>
          </div>
          <div class="text-xs opacity-70 break-all">{{ avatarEditorName }}</div>
        </div>
        <div class="mt-3 flex gap-2">
          <button class="btn btn-xs" :disabled="!avatarEditorTargetId || avatarSaving" @click="openAvatarPickerForEditor">{{ t("config.persona.uploadAvatar") }}</button>
          <button class="btn btn-xs btn-ghost" :disabled="!avatarEditorTargetHasAvatar || avatarSaving" @click="clearAvatarFromEditor">{{ t("config.persona.clearAvatar") }}</button>
        </div>
        <div class="mt-2 text-[11px] opacity-60">{{ t("config.persona.pasteImageHint") }}</div>
        <div v-if="avatarError" class="mt-2 text-xs text-error break-all">{{ avatarError }}</div>
      </div>
      <div class="modal-action mt-2">
        <button class="btn btn-sm btn-ghost" @click="closeAvatarEditor">{{ t("common.close") }}</button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button aria-label="close">close</button>
    </form>
  </dialog>
  <dialog ref="cropDialog" class="modal" @close="destroyCropper">
    <div class="modal-box p-3 max-w-md">
      <h3 class="text-sm font-semibold mb-2">{{ t("config.persona.cropAvatar") }}</h3>
      <div class="rounded border border-base-300 bg-base-100 p-2 min-h-64">
        <img ref="cropImageEl" :src="cropSource" alt="crop source" class="max-w-full block" />
      </div>
      <div v-if="localCropError || avatarError" class="mt-2 text-xs text-error break-all">{{ localCropError || avatarError }}</div>
      <div class="modal-action mt-2">
        <button class="btn btn-sm btn-ghost" @click="closeCropDialog">{{ t("common.cancel") }}</button>
        <button class="btn btn-sm btn-primary" :disabled="!cropperReady || avatarSaving" @click="confirmCrop">
          {{ avatarSaving ? t("config.api.saving") : t("config.persona.saveAvatar") }}
        </button>
      </div>
    </div>
    <form method="dialog" class="modal-backdrop">
      <button aria-label="close">close</button>
    </form>
  </dialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, ImageTextCacheStats, PersonaProfile, ResponseStyleOption, ToolLoadStatus } from "../../../types/app";
import Cropper from "cropperjs";
import HotkeyTab from "./config-tabs/HotkeyTab.vue";
import ApiTab from "./config-tabs/ApiTab.vue";
import ToolsTab from "./config-tabs/ToolsTab.vue";
import PersonaTab from "./config-tabs/PersonaTab.vue";
import ChatSettingsTab from "./config-tabs/ChatSettingsTab.vue";

type ConfigTab = "hotkey" | "api" | "tools" | "persona" | "chatSettings";
type AvatarTarget = { agentId: string };

const props = defineProps<{
  config: AppConfig;
  configTab: ConfigTab;
  uiLanguage: "zh-CN" | "en-US" | "ja-JP" | "ko-KR";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "ja-JP" | "ko-KR"; label: string }>;
  currentTheme: "light" | "forest";
  selectedApiConfig: ApiConfigItem | null;
  toolApiConfig: ApiConfigItem | null;
  baseUrlReference: string;
  refreshingModels: boolean;
  modelOptions: string[];
  modelRefreshOk: boolean;
  modelRefreshError: string;
  toolStatuses: ToolLoadStatus[];
  personas: PersonaProfile[];
  assistantPersonas: PersonaProfile[];
  userPersona: PersonaProfile | null;
  personaEditorId: string;
  selectedPersonaId: string;
  responseStyleOptions: ResponseStyleOption[];
  responseStyleId: string;
  selectedPersona: PersonaProfile | null;
  selectedPersonaAvatarUrl: string;
  userPersonaAvatarUrl: string;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  cacheStats: ImageTextCacheStats;
  cacheStatsLoading: boolean;
  avatarSaving: boolean;
  avatarError: string;
  configDirty: boolean;
  savingConfig: boolean;
  hotkeyTestRecording: boolean;
  hotkeyTestRecordingMs: number;
  hotkeyTestAudioReady: boolean;
}>();

const emit = defineEmits<{
  (e: "update:configTab", value: ConfigTab): void;
  (e: "update:uiLanguage", value: string): void;
  (e: "update:personaEditorId", value: string): void;
  (e: "update:selectedPersonaId", value: string): void;
  (e: "update:responseStyleId", value: string): void;
  (e: "toggleTheme"): void;
  (e: "refreshModels"): void;
  (e: "openMemoryViewer"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "saveApiConfig"): void;
  (e: "addPersona"): void;
  (e: "removeSelectedPersona"): void;
  (e: "openCurrentHistory"): void;
  (e: "openPromptPreview"): void;
  (e: "openSystemPromptPreview"): void;
  (e: "refreshImageCacheStats"): void;
  (e: "clearImageCache"): void;
  (e: "startHotkeyRecordTest"): void;
  (e: "stopHotkeyRecordTest"): void;
  (e: "playHotkeyRecordTest"): void;
  (e: "captureHotkey", value: string): void;
  (e: "saveAgentAvatar", value: { agentId: string; mime: string; bytesBase64: string }): void;
  (e: "clearAgentAvatar", value: { agentId: string }): void;
}>();

const { t } = useI18n();

const avatarFileInput = ref<HTMLInputElement | null>(null);
const avatarEditorDialog = ref<HTMLDialogElement | null>(null);
const cropDialog = ref<HTMLDialogElement | null>(null);
const cropImageEl = ref<HTMLImageElement | null>(null);
const cropSource = ref("");
const cropperReady = ref(false);
const localCropError = ref("");
const avatarEditorTargetId = ref("");
let cropper: Cropper | null = null;
let cropTarget: AvatarTarget | null = null;

function avatarInitial(name: string): string {
  const text = (name || "").trim();
  if (!text) return "?";
  return text[0].toUpperCase();
}

function openAvatarPicker(target: AvatarTarget) {
  cropTarget = target;
  if (avatarFileInput.value) {
    avatarFileInput.value.value = "";
    avatarFileInput.value.click();
  }
}

function openAvatarEditorForSelected() {
  if (!props.selectedPersona) return;
  avatarEditorTargetId.value = props.selectedPersona.id;
  cropTarget = { agentId: props.selectedPersona.id };
  avatarEditorDialog.value?.showModal();
}

function closeAvatarEditor() {
  avatarEditorDialog.value?.close();
}

function openAvatarPickerForEditor() {
  if (!avatarEditorTargetId.value) return;
  openAvatarPicker({ agentId: avatarEditorTargetId.value });
}

function ensureEditorCropTarget() {
  if (cropTarget || !avatarEditorTargetId.value) return;
  cropTarget = { agentId: avatarEditorTargetId.value };
}

function clearAvatarFromEditor() {
  if (!avatarEditorTargetId.value) return;
  emit("clearAgentAvatar", { agentId: avatarEditorTargetId.value });
}

function avatarById(id: string): PersonaProfile | null {
  return props.personas.find((p) => p.id === id) ?? null;
}

const avatarEditorTarget = () => avatarById(avatarEditorTargetId.value);

const avatarEditorName = computed(() => avatarEditorTarget()?.name || t("config.persona.avatarFallbackName"));
const avatarEditorAvatarUrl = computed(() => {
  const target = avatarEditorTarget();
  if (!target) return "";
  if (target.id === props.userPersona?.id) return props.userPersonaAvatarUrl;
  if (target.id === props.selectedPersona?.id) return props.selectedPersonaAvatarUrl;
  return "";
});
const avatarEditorTargetHasAvatar = computed(() => !!avatarEditorTarget()?.avatarPath);

async function readFileAsDataUrl(file: File): Promise<string> {
  return await new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

async function loadImage(dataUrl: string): Promise<HTMLImageElement> {
  return await new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("load image failed"));
    img.src = dataUrl;
  });
}

async function downscaleDataUrl(dataUrl: string, maxSide = 1024): Promise<string> {
  const img = await loadImage(dataUrl);
  const w = img.naturalWidth || img.width;
  const h = img.naturalHeight || img.height;
  if (w <= maxSide && h <= maxSide) return dataUrl;
  const scale = Math.min(1, maxSide / Math.max(w, h));
  const targetW = Math.max(1, Math.round(w * scale));
  const targetH = Math.max(1, Math.round(h * scale));
  const canvas = document.createElement("canvas");
  canvas.width = targetW;
  canvas.height = targetH;
  const ctx = canvas.getContext("2d");
  if (!ctx) return dataUrl;
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(img, 0, 0, targetW, targetH);
  return canvas.toDataURL("image/webp", 0.9);
}

function destroyCropper() {
  if (cropper) {
    cropper.destroy();
    cropper = null;
  }
  cropperReady.value = false;
}

function closeCropDialog() {
  cropDialog.value?.close();
  cropSource.value = "";
  cropTarget = null;
  localCropError.value = "";
}

async function onAvatarFilePicked(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  void processAvatarFile(file);
}

async function processAvatarFile(file: File) {
  ensureEditorCropTarget();
  if (!cropTarget) return;
  localCropError.value = "";
  try {
    const dataUrl = await readFileAsDataUrl(file);
    cropSource.value = await downscaleDataUrl(dataUrl, 1024);
    await nextTick();
    destroyCropper();
    if (!cropImageEl.value) {
      localCropError.value = t("config.persona.cropInitFailed");
      return;
    }
    cropper = new Cropper(cropImageEl.value, {
      aspectRatio: 1,
      viewMode: 1,
      dragMode: "move",
      autoCropArea: 1,
      background: false,
      guides: false,
    });
    cropperReady.value = true;
    cropDialog.value?.showModal();
  } catch (e) {
    localCropError.value = t("config.persona.avatarReadFailed", { err: String(e) });
  }
}

function handleAvatarPaste(event: ClipboardEvent) {
  if (!avatarEditorDialog.value?.open) return;
  const items = event.clipboardData?.items;
  if (!items || items.length === 0) return;
  const imageItem = Array.from(items).find((item) => item.type.startsWith("image/"));
  if (!imageItem) {
    localCropError.value = t("config.persona.pasteNoImage");
    return;
  }
  const file = imageItem.getAsFile();
  if (!file) {
    localCropError.value = t("config.persona.pasteReadFailed");
    return;
  }
  event.preventDefault();
  event.stopPropagation();
  void processAvatarFile(file);
}

onMounted(() => {
  window.addEventListener("paste", handleAvatarPaste);
});

function confirmCrop() {
  if (!cropTarget) {
    localCropError.value = t("config.persona.cropMissingTarget");
    return;
  }
  if (!cropper) {
    localCropError.value = t("config.persona.cropperNotReady");
    return;
  }
  localCropError.value = "";
  const canvas = cropper.getCroppedCanvas({
    width: 128,
    height: 128,
    imageSmoothingEnabled: true,
    imageSmoothingQuality: "high",
  });
  const dataUrl = canvas.toDataURL("image/webp", 0.8);
  const marker = "base64,";
  const idx = dataUrl.indexOf(marker);
  if (idx < 0) return;
  const bytesBase64 = dataUrl.slice(idx + marker.length);
  emit("saveAgentAvatar", {
    agentId: cropTarget.agentId,
    mime: "image/webp",
    bytesBase64,
  });
  closeCropDialog();
}

onBeforeUnmount(() => {
  window.removeEventListener("paste", handleAvatarPaste);
  destroyCropper();
});
</script>
