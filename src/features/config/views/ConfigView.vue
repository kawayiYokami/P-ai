<template>
  <div class="h-full min-h-0 flex gap-3">
    <div class="w-36 shrink-0">
      <ul class="menu bg-base-200 rounded-box gap-1 [&>li>a]:min-w-30 [&>li>a]:w-full">
        <li>
          <a :class="{ 'active': props.configTab === 'welcome', 'menu-active': props.configTab === 'welcome', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('welcome')">{{ t("config.tabs.welcome") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'hotkey', 'menu-active': props.configTab === 'hotkey', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('hotkey')">{{ t("config.tabs.hotkey") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'api', 'menu-active': props.configTab === 'api', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('api')">{{ t("config.tabs.api") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'tools', 'menu-active': props.configTab === 'tools', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('tools')">{{ t("config.tabs.tools") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'mcp', 'menu-active': props.configTab === 'mcp', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('mcp')">MCP</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'skill', 'menu-active': props.configTab === 'skill', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('skill')">{{ t("config.tabs.skill") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'persona', 'menu-active': props.configTab === 'persona', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('persona')">{{ t("config.tabs.persona") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'department', 'menu-active': props.configTab === 'department', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('department')">{{ t("config.tabs.department") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'chatSettings', 'menu-active': props.configTab === 'chatSettings', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('chatSettings')">{{ t("config.tabs.chatSettings") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'remoteIm', 'menu-active': props.configTab === 'remoteIm', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('remoteIm')">{{ t("config.tabs.remoteIm") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'memory', 'menu-active': props.configTab === 'memory' }" @click="requestTabChange('memory')">{{ t("config.tabs.memory") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'task', 'menu-active': props.configTab === 'task', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('task')">{{ t("config.tabs.task") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'logs', 'menu-active': props.configTab === 'logs', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('logs')">{{ t("config.tabs.logs") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'appearance', 'menu-active': props.configTab === 'appearance', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('appearance')">{{ t("config.tabs.appearance") }}</a>
        </li>
        <li>
          <a :class="{ 'active': props.configTab === 'about', 'menu-active': props.configTab === 'about', 'opacity-50 pointer-events-none': memorySyncLocked }" @click="requestTabChange('about')">{{ t("config.tabs.about") }}</a>
        </li>
      </ul>
    </div>

    <div
      class="flex-1 min-w-0"
      :class="props.configTab === 'api' ? 'flex min-h-0 flex-col overflow-hidden' : 'overflow-y-auto scrollbar-gutter-stable'"
    >
      <div v-if="props.configTab === 'api'" class="flex-1 min-h-0">
        <ApiTab
          :config="config"
          :base-url-reference="baseUrlReference"
          :refreshing-models="refreshingModels"
          :model-options="modelOptions"
          :model-refresh-ok="modelRefreshOk"
          :model-refresh-error="modelRefreshError"
          :config-dirty="configDirty"
          :saving-config="savingConfig"
          :save-api-config-action="props.saveConfigAction"
          :restore-api-config-action="props.restoreConfigAction"
          :normalize-api-bindings-action="normalizeApiBindingsAction"
          :last-saved-config-json="props.lastSavedConfigJson"
          @refresh-models="$emit('refreshModels')"
        />
      </div>

      <SettingsContentContainer v-else>
        <WelcomeTab
          v-if="props.configTab === 'welcome'"
          :config="config"
          :personas="personas"
          @jump="$emit('update:configTab', $event)"
        />

        <HotkeyTab
          v-else-if="props.configTab === 'hotkey'"
          :config="config"
          :hotkey-test-recording="hotkeyTestRecording"
          :hotkey-test-recording-ms="hotkeyTestRecordingMs"
          :hotkey-test-audio-ready="hotkeyTestAudioReady"
          @start-hotkey-record-test="$emit('startHotkeyRecordTest')"
          @stop-hotkey-record-test="$emit('stopHotkeyRecordTest')"
          @play-hotkey-record-test="$emit('playHotkeyRecordTest')"
          @capture-hotkey="$emit('captureHotkey', $event)"
          @summon-chat-now="$emit('summonChatNow')"
          @update:record-hotkey="onRecordHotkeyChanged"
          @update:record-background-wake-enabled="onRecordBackgroundWakeChanged"
          @update:min-record-seconds="onMinRecordSecondsChanged"
          @update:max-record-seconds="onMaxRecordSecondsChanged"
        />

        <ToolsTab
          v-else-if="props.configTab === 'tools'"
          :config="config"
          :personas="assistantPersonas"
          :persona-editor-id="personaEditorId"
          :selected-persona="toolPersona"
          :tool-api-config="toolApiConfig"
          :tool-statuses="toolStatuses"
          :saving-config="savingConfig"
          @update:persona-editor-id="$emit('update:personaEditorId', $event)"
          @tool-switch-changed="$emit('toolSwitchChanged')"
          @save-api-config="onSaveToolsConfig"
          @save-personas="$emit('savePersonas')"
          @open-memory-viewer="$emit('update:configTab', 'memory')"
        />
        <McpTab
          v-else-if="props.configTab === 'mcp'"
        />
        <SkillTab
          v-else-if="props.configTab === 'skill'"
        />

        <PersonaTab
          v-else-if="props.configTab === 'persona'"
          :personas="personas"
          :assistant-personas="assistantPersonas"
          :persona-editor-id="personaEditorId"
          :selected-persona="selectedPersona"
          :selected-persona-avatar-url="selectedPersonaAvatarUrl"
          :avatar-saving="avatarSaving"
          :avatar-error="avatarError"
          :persona-saving="personaSaving"
          :persona-dirty="personaDirty"
          @update:persona-editor-id="$emit('update:personaEditorId', $event)"
          @add-persona="$emit('addPersona')"
          @remove-selected-persona="$emit('removeSelectedPersona')"
          @open-avatar-editor="openAvatarEditorForSelected"
          @import-persona-memories="$emit('importPersonaMemories', $event)"
          @save-personas="$emit('savePersonas')"
        />

        <ChatSettingsTab
          v-else-if="props.configTab === 'chatSettings'"
          :config="config"
          :image-capable-api-configs="imageCapableApiConfigs"
          :stt-capable-api-configs="sttCapableApiConfigs"
          :response-style-options="responseStyleOptions"
          :response-style-id="responseStyleId"
          :pdf-read-mode="pdfReadMode"
          :background-voice-screenshot-keywords="backgroundVoiceScreenshotKeywords"
          :background-voice-screenshot-mode="backgroundVoiceScreenshotMode"
          :cache-stats="cacheStats"
          :cache-stats-loading="cacheStatsLoading"
          @update:response-style-id="$emit('update:responseStyleId', $event)"
          @update:pdf-read-mode="$emit('update:pdfReadMode', $event)"
          @update:background-voice-screenshot-keywords="$emit('update:backgroundVoiceScreenshotKeywords', $event)"
          @update:background-voice-screenshot-mode="$emit('update:backgroundVoiceScreenshotMode', $event)"
          @save-chat-settings="$emit('saveChatSettings')"
          @open-current-history="$emit('openCurrentHistory')"
          @open-prompt-preview="$emit('openPromptPreview')"
          @open-system-prompt-preview="$emit('openSystemPromptPreview')"
          @refresh-image-cache-stats="$emit('refreshImageCacheStats')"
          @clear-image-cache="$emit('clearImageCache')"
        />
        <RemoteImTab
          v-else-if="props.configTab === 'remoteIm'"
          :config="config"
          :save-config-action="saveConfigAction"
          :set-status-action="setStatusAction"
        />

        <DepartmentTab
          v-else-if="props.configTab === 'department'"
          :config="config"
          :api-configs="config.apiConfigs"
          :personas="assistantPersonas"
          :assistant-department-agent-id="assistantDepartmentAgentId"
          :saving-config="savingConfig"
          :save-config-action="saveConfigAction"
          :set-status-action="setStatusAction"
          @update:assistant-department-assignee-id="$emit('update:assistantDepartmentAgentId', $event)"
        />

        <MemoryTab
          v-else-if="props.configTab === 'memory'"
          :sync-locked="memorySyncLocked"
          @sync-lock-change="onMemorySyncLockChange"
        />

        <TaskTab
          v-else-if="props.configTab === 'task'"
        />

        <LogTab
          v-else-if="props.configTab === 'logs'"
          :open-runtime-logs="() => $emit('openRuntimeLogs')"
        />

        <AppearanceTab
          v-else-if="props.configTab === 'appearance'"
          :ui-language="uiLanguage"
          :locale-options="localeOptions"
          :current-theme="currentTheme"
          @update:ui-language="$emit('update:uiLanguage', $event)"
          @set-theme="$emit('setTheme', $event)"
        />

        <AboutTab
          v-else-if="props.configTab === 'about'"
          :checking-update="checkingUpdate"
          @check-update="$emit('checkUpdate')"
          @open-github="$emit('openGithub')"
        />
      </SettingsContentContainer>
    </div>

    <!-- Dialogs -->

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
          <div class="text-sm opacity-70 break-all">{{ avatarEditorName }}</div>
        </div>
        <div class="mt-3 flex gap-2">
          <button class="btn btn-sm" :disabled="!avatarEditorTargetId || avatarSaving" @click="openAvatarPickerForEditor">{{ t("config.persona.uploadAvatar") }}</button>
          <button class="btn btn-sm btn-ghost" :disabled="!avatarEditorTargetHasAvatar || avatarSaving" @click="clearAvatarFromEditor">{{ t("config.persona.clearAvatar") }}</button>
        </div>
        <div class="mt-2 text-[11px] opacity-60">{{ t("config.persona.pasteImageHint") }}</div>
        <div v-if="avatarError" class="mt-2 text-sm text-error break-all">{{ avatarError }}</div>
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
      <div v-if="localCropError || avatarError" class="mt-2 text-sm text-error break-all">{{ localCropError || avatarError }}</div>
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
    <dialog ref="migrateWorkspaceDialog" class="modal">
      <div class="modal-box max-w-lg p-4">
        <h3 class="text-sm font-semibold">{{ t("config.tools.migrateWorkspaceTitle") }}</h3>
        <p v-if="workspaceMigrationUi.mode === 'confirm'" class="mt-3 text-sm whitespace-pre-wrap">
          {{ t("config.tools.migrateWorkspaceConfirm", { oldPath: pendingWorkspaceMigration.oldPath, newPath: pendingWorkspaceMigration.newPath }) }}
        </p>
        <div v-else class="mt-3 grid gap-3">
          <div class="text-sm">{{ workspaceMigrationUi.message || t("config.tools.migrateWorkspacePreparing") }}</div>
          <progress class="progress progress-primary w-full" :value="workspaceMigrationProgressValue" max="100"></progress>
          <div class="flex items-center justify-between text-xs opacity-70">
            <span>{{ workspaceMigrationStageLabel }}</span>
            <span>{{ workspaceMigrationUi.processed }}/{{ workspaceMigrationUi.total }}</span>
          </div>
          <div v-if="workspaceMigrationUi.currentPath" class="text-xs font-mono break-all opacity-70">
            {{ workspaceMigrationUi.currentPath }}
          </div>
          <div v-if="workspaceMigrationUi.error" class="rounded bg-error/10 px-3 py-2 text-sm text-error whitespace-pre-wrap break-all">
            {{ workspaceMigrationUi.error }}
          </div>
        </div>
        <div class="modal-action mt-4">
          <button
            class="btn btn-sm btn-ghost"
            type="button"
            :disabled="workspaceMigrationUi.mode === 'running'"
            @click="cancelWorkspaceMigration"
          >
            {{ workspaceMigrationUi.mode === 'error' ? t("common.close") : t("common.cancel") }}
          </button>
          <button
            v-if="workspaceMigrationUi.mode === 'confirm'"
            class="btn btn-sm btn-outline"
            type="button"
            @click="skipWorkspaceMigration"
          >
            {{ t("config.tools.migrateWorkspaceSkip") }}
          </button>
          <button
            v-if="workspaceMigrationUi.mode === 'confirm'"
            class="btn btn-sm btn-primary"
            type="button"
            @click="confirmWorkspaceMigration"
          >
            {{ t("common.confirm") }}
          </button>
        </div>
      </div>
      <form method="dialog" class="modal-backdrop">
        <button aria-label="close" @click="cancelWorkspaceMigration">close</button>
      </form>
    </dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ApiConfigItem, AppConfig, ImageTextCacheStats, PersonaProfile, ResponseStyleOption, ToolLoadStatus } from "../../../types/app";
import Cropper from "cropperjs";
import SettingsContentContainer from "../components/SettingsContentContainer.vue";
import WelcomeTab from "./config-tabs/WelcomeTab.vue";
import HotkeyTab from "./config-tabs/HotkeyTab.vue";
import ApiTab from "./config-tabs/ApiTab.vue";
import ToolsTab from "./config-tabs/ToolsTab.vue";
import McpTab from "./config-tabs/McpTab.vue";
import SkillTab from "./config-tabs/SkillTab.vue";
import PersonaTab from "./config-tabs/PersonaTab.vue";
import DepartmentTab from "./config-tabs/DepartmentTab.vue";
import ChatSettingsTab from "./config-tabs/ChatSettingsTab.vue";
import RemoteImTab from "./config-tabs/RemoteImTab.vue";
import MemoryTab from "./config-tabs/MemoryTab.vue";
import TaskTab from "./config-tabs/TaskTab.vue";
import LogTab from "./config-tabs/LogTab.vue";
import AppearanceTab from "./config-tabs/AppearanceTab.vue";
import AboutTab from "./config-tabs/AboutTab.vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invokeTauri } from "../../../services/tauri-api";
import { toErrorMessage } from "../../../utils/error";

type ConfigTab = "welcome" | "hotkey" | "api" | "tools" | "mcp" | "skill" | "persona" | "department" | "chatSettings" | "remoteIm" | "memory" | "task" | "logs" | "appearance" | "about";
type AvatarTarget = { agentId: string };

const props = defineProps<{
  config: AppConfig;
  configTab: ConfigTab;
  uiLanguage: "zh-CN" | "en-US" | "zh-TW";
  localeOptions: Array<{ value: "zh-CN" | "en-US" | "zh-TW"; label: string }>;
  currentTheme: string;
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
  assistantDepartmentAgentId: string;
  responseStyleOptions: ResponseStyleOption[];
  responseStyleId: string;
  pdfReadMode: "text" | "image";
  backgroundVoiceScreenshotKeywords: string;
  backgroundVoiceScreenshotMode: "desktop" | "focused_window";
  selectedPersona: PersonaProfile | null;
  toolPersona: PersonaProfile | null;
  selectedPersonaAvatarUrl: string;
  userPersonaAvatarUrl: string;
  textCapableApiConfigs: ApiConfigItem[];
  imageCapableApiConfigs: ApiConfigItem[];
  sttCapableApiConfigs: ApiConfigItem[];
  cacheStats: ImageTextCacheStats;
  cacheStatsLoading: boolean;
  avatarSaving: boolean;
  avatarError: string;
  personaSaving: boolean;
  personaDirty: boolean;
  configDirty: boolean;
  savingConfig: boolean;
  normalizeApiBindingsAction: () => void;
  hotkeyTestRecording: boolean;
  hotkeyTestRecordingMs: number;
  hotkeyTestAudioReady: boolean;
  checkingUpdate: boolean;
  saveConfigAction: () => Promise<boolean> | boolean;
  restoreConfigAction: () => boolean;
  lastSavedConfigJson: string;
  setStatusAction: (text: string) => void;
}>();

const emit = defineEmits<{
  (e: "update:configTab", value: ConfigTab): void;
  (e: "update:uiLanguage", value: string): void;
  (e: "update:personaEditorId", value: string): void;
  (e: "update:assistantDepartmentAgentId", value: string): void;
  (e: "update:responseStyleId", value: string): void;
  (e: "update:pdfReadMode", value: "text" | "image"): void;
  (e: "update:backgroundVoiceScreenshotKeywords", value: string): void;
  (e: "update:backgroundVoiceScreenshotMode", value: "desktop" | "focused_window"): void;
  (e: "saveChatSettings"): void;
  (e: "setTheme", value: string): void;
  (e: "refreshModels"): void;
  (e: "toolSwitchChanged"): void;
  (e: "openMemoryViewer"): void;
  (e: "addApiConfig"): void;
  (e: "removeSelectedApiConfig"): void;
  (e: "saveApiConfig"): void;
  (e: "addPersona"): void;
  (e: "removeSelectedPersona"): void;
  (e: "savePersonas"): void;
  (e: "importPersonaMemories", value: { agentId: string; file: File }): void;
  (e: "openCurrentHistory"): void;
  (e: "openPromptPreview"): void;
  (e: "openSystemPromptPreview"): void;
  (e: "refreshImageCacheStats"): void;
  (e: "clearImageCache"): void;
  (e: "openRuntimeLogs"): void;
  (e: "startHotkeyRecordTest"): void;
  (e: "stopHotkeyRecordTest"): void;
  (e: "playHotkeyRecordTest"): void;
  (e: "captureHotkey", value: string): void;
  (e: "summonChatNow"): void;
  (e: "saveAgentAvatar", value: { agentId: string; mime: string; bytesBase64: string }): void;
  (e: "clearAgentAvatar", value: { agentId: string }): void;
  (e: "checkUpdate"): void;
  (e: "openGithub"): void;
}>();

const { t } = useI18n();

const avatarFileInput = ref<HTMLInputElement | null>(null);
const avatarEditorDialog = ref<HTMLDialogElement | null>(null);
const cropDialog = ref<HTMLDialogElement | null>(null);
const migrateWorkspaceDialog = ref<HTMLDialogElement | null>(null);
const cropImageEl = ref<HTMLImageElement | null>(null);
const cropSource = ref("");
const pendingWorkspaceMigration = ref({ oldPath: "", newPath: "" });
const workspaceMigrationUi = ref({
  taskId: "",
  mode: "confirm" as "confirm" | "running" | "error",
  stage: "idle",
  message: "",
  processed: 0,
  total: 0,
  currentPath: "",
  error: "",
});
type WorkspaceMigrationDecision = "migrate" | "skip" | "cancel";
let resolveWorkspaceMigrationConfirm: ((value: WorkspaceMigrationDecision) => void) | null = null;
let workspaceMigrationProgressUnlisten: UnlistenFn | null = null;
const cropperReady = ref(false);
const localCropError = ref("");
const avatarEditorTargetId = ref("");
const memorySyncLocked = ref(false);
const savingToolsConfig = ref(false);
let cropper: Cropper | null = null;
let cropTarget: AvatarTarget | null = null;
const MIN_RECORD_SECONDS = 1;
const MAX_MIN_RECORD_SECONDS = 30;
const MAX_RECORD_SECONDS = 600;
const workspaceMigrationProgressValue = computed(() => {
  if (workspaceMigrationUi.value.total <= 0) return 0;
  return Math.max(0, Math.min(100, Math.round((workspaceMigrationUi.value.processed / workspaceMigrationUi.value.total) * 100)));
});
const workspaceMigrationStageLabel = computed(() => {
  const stage = workspaceMigrationUi.value.stage;
  if (stage === "scanning") return t("config.tools.migrateWorkspaceStageScanning");
  if (stage === "copying") return t("config.tools.migrateWorkspaceStageCopying");
  if (stage === "deleting") return t("config.tools.migrateWorkspaceStageDeleting");
  if (stage === "completed") return t("config.tools.migrateWorkspaceStageCompleted");
  if (stage === "failed") return t("config.tools.migrateWorkspaceStageFailed");
  return t("config.tools.migrateWorkspacePreparing");
});

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

// `config` is a shared reactive object from the root app state.
// Direct mutation here is intentional and immediately reflected upstream.
async function onRecordHotkeyChanged(value: string) {
  const next = String(value || "").trim();
  if (!next) return;
  if (props.config.recordHotkey === next) return;
  props.config.recordHotkey = next;
}

function onRecordBackgroundWakeChanged(value: boolean) {
  const next = !!value;
  if (!!props.config.recordBackgroundWakeEnabled === next) return;
  props.config.recordBackgroundWakeEnabled = next;
}

function onMinRecordSecondsChanged(value: number) {
  const next = Math.max(MIN_RECORD_SECONDS, Math.min(MAX_MIN_RECORD_SECONDS, Math.round(Number(value) || MIN_RECORD_SECONDS)));
  props.config.minRecordSeconds = next;
  if (props.config.maxRecordSeconds < next) {
    props.config.maxRecordSeconds = next;
  }
}

function onMaxRecordSecondsChanged(value: number) {
  const next = Math.max(
    props.config.minRecordSeconds,
    Math.min(MAX_RECORD_SECONDS, Math.round(Number(value) || props.config.minRecordSeconds)),
  );
  props.config.maxRecordSeconds = next;
}

function requestTabChange(nextTab: ConfigTab) {
  if (memorySyncLocked.value && nextTab !== "memory") {
    return;
  }
  emit("update:configTab", nextTab);
}

function onMemorySyncLockChange(locked: boolean) {
  memorySyncLocked.value = !!locked;
}

async function onSaveToolsConfig() {
  if (savingToolsConfig.value) return;
  savingToolsConfig.value = true;
  const previousShellWorkspaces = Array.isArray(props.config.shellWorkspaces)
    ? props.config.shellWorkspaces.map((item) => ({
      name: String(item.name || ""),
      path: String(item.path || ""),
      builtIn: !!item.builtIn,
    }))
    : [];
  const previousTerminalShellKind = String(props.config.terminalShellKind || "auto").trim() || "auto";
  try {
    let savedSnapshot: { shellWorkspaces?: Array<{ path?: string }> } = {};
    try {
      savedSnapshot = JSON.parse(props.lastSavedConfigJson || "{}") as { shellWorkspaces?: Array<{ path?: string }> };
    } catch {
      savedSnapshot = {};
    }
    const previousSavedPath = String(savedSnapshot.shellWorkspaces?.[0]?.path || "").trim();
    const nextPath = String(props.config.shellWorkspaces?.[0]?.path || "").trim();
    const normalizedPreviousSavedPath = previousSavedPath.replace(/[\\/]+$/g, "");
    const normalizedNextPath = nextPath.replace(/[\\/]+$/g, "");
    if (
      normalizedPreviousSavedPath
      && normalizedNextPath
      && normalizedPreviousSavedPath !== normalizedNextPath
    ) {
      const decision = await requestWorkspaceMigrationConfirm(previousSavedPath, nextPath);
      if (decision === "cancel") {
        props.config.shellWorkspaces = previousShellWorkspaces;
        props.config.terminalShellKind = previousTerminalShellKind;
        return;
      }
      if (decision === "migrate") {
        const migratedPath = await migrateWorkspaceWithProgress(previousSavedPath, nextPath);
        props.setStatusAction(t("config.tools.migrateWorkspaceDone", { path: migratedPath }));
      }
    }
    const saved = await Promise.resolve(props.saveConfigAction());
    if (!saved) {
      props.config.shellWorkspaces = previousShellWorkspaces;
      props.config.terminalShellKind = previousTerminalShellKind;
      props.setStatusAction(t("status.saveConfigFailed", { err: "tools settings save rejected" }));
    }
  } catch (error) {
    props.config.shellWorkspaces = previousShellWorkspaces;
    props.config.terminalShellKind = previousTerminalShellKind;
    props.setStatusAction(t("status.saveConfigFailed", { err: toErrorMessage(error) }));
    throw error;
  } finally {
    savingToolsConfig.value = false;
  }
}

function requestWorkspaceMigrationConfirm(oldPath: string, newPath: string): Promise<WorkspaceMigrationDecision> {
  const dialog = migrateWorkspaceDialog.value;
  if (!dialog) return Promise.resolve("cancel");
  pendingWorkspaceMigration.value = { oldPath, newPath };
  workspaceMigrationUi.value = {
    taskId: "",
    mode: "confirm",
    stage: "idle",
    message: "",
    processed: 0,
    total: 0,
    currentPath: "",
    error: "",
  };
  return new Promise<WorkspaceMigrationDecision>((resolve) => {
    resolveWorkspaceMigrationConfirm = resolve;
    dialog.showModal();
  });
}

function finishWorkspaceMigrationConfirm(value: WorkspaceMigrationDecision) {
  const dialog = migrateWorkspaceDialog.value;
  if (dialog?.open && value !== "migrate") {
    dialog.close();
  }
  resolveWorkspaceMigrationConfirm?.(value);
  resolveWorkspaceMigrationConfirm = null;
}

function confirmWorkspaceMigration() {
  workspaceMigrationUi.value.mode = "running";
  workspaceMigrationUi.value.stage = "scanning";
  workspaceMigrationUi.value.message = t("config.tools.migrateWorkspacePreparing");
  finishWorkspaceMigrationConfirm("migrate");
}

function skipWorkspaceMigration() {
  finishWorkspaceMigrationConfirm("skip");
}

function cancelWorkspaceMigration() {
  if (workspaceMigrationUi.value.mode === "running") return;
  finishWorkspaceMigrationConfirm("cancel");
}

async function ensureWorkspaceMigrationListener() {
  if (workspaceMigrationProgressUnlisten) return;
  workspaceMigrationProgressUnlisten = await listen<{
    taskId: string;
    stage: string;
    processed: number;
    total: number;
    currentPath?: string | null;
    message: string;
    done: boolean;
    error?: string | null;
  }>("easy-call:workspace-migration-progress", (event) => {
    const payload = event.payload;
    if (!payload || payload.taskId !== workspaceMigrationUi.value.taskId) return;
    workspaceMigrationUi.value.stage = String(payload.stage || "");
    workspaceMigrationUi.value.message = String(payload.message || "");
    workspaceMigrationUi.value.processed = Number(payload.processed || 0);
    workspaceMigrationUi.value.total = Number(payload.total || 0);
    workspaceMigrationUi.value.currentPath = String(payload.currentPath || "");
    workspaceMigrationUi.value.error = String(payload.error || "");
    if (payload.error) {
      workspaceMigrationUi.value.mode = "error";
    }
  });
}

async function migrateWorkspaceWithProgress(oldPath: string, newPath: string): Promise<string> {
  await ensureWorkspaceMigrationListener();
  const taskId = `workspace-migration-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  workspaceMigrationUi.value.taskId = taskId;
  workspaceMigrationUi.value.mode = "running";
  workspaceMigrationUi.value.stage = "scanning";
  workspaceMigrationUi.value.message = t("config.tools.migrateWorkspacePreparing");
  workspaceMigrationUi.value.processed = 0;
  workspaceMigrationUi.value.total = 0;
  workspaceMigrationUi.value.currentPath = oldPath;
  workspaceMigrationUi.value.error = "";
  try {
    const migratedPath = await invokeTauri<string>("migrate_shell_workspace_directory", {
      input: {
        oldPath,
        newPath,
        taskId,
      },
    });
    const dialog = migrateWorkspaceDialog.value;
    if (dialog?.open) {
      dialog.close();
    }
    return migratedPath;
  } catch (error) {
    workspaceMigrationUi.value.mode = "error";
    workspaceMigrationUi.value.stage = "failed";
    workspaceMigrationUi.value.message = t("config.tools.migrateWorkspaceFailed");
    workspaceMigrationUi.value.error = toErrorMessage(error);
    throw error;
  }
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
  if (idx < 0) {
    localCropError.value = t("config.persona.avatarSaveEncodeFailed");
    return;
  }
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
  workspaceMigrationProgressUnlisten?.();
  destroyCropper();
});
</script>

<style scoped>
.scrollbar-gutter-stable {
  scrollbar-gutter: stable;
}
</style>
