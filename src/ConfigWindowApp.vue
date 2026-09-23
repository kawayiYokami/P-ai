<template>
  <div class="window-shell text-sm bg-base-200">
    <AppWindowHeader
      v-if="!hideWindowHeader"
      view-mode="config"
      :current-theme="currentTheme"
      :title-text="t('window.configTitle')"
      :chat-usage-percent="0"
      :trimming="false"
      :chatting="false"
      :current-persona-name="String(selectedPersonaEditor?.name || '').trim() || t('archives.roleAssistant')"
      :side-conversation-list-visible="false"
      :tool-review-panel-open-visible="false"
      :chat-side-panel-widths="{ leftWidth: 0, rightWidth: 0 }"
      active-conversation-id=""
      :conversation-items="[]"
      :user-alias="userAlias"
      :user-avatar-url="userAvatarUrl"
      :persona-name-map="chatPersonaNameMap"
      :persona-avatar-url-map="chatPersonaAvatarUrlMap"
      :create-conversation-agent-options="[]"
      default-create-conversation-agent-id=""
      :trim-tip="t('chat.trimTip')"
      :maximized="maximized"
      :window-ready="windowReady"
      :open-settings-title="t('window.configTitle')"
      :close-title="t('common.close')"
      :config-search-query="configSearchQuery"
      :config-search-results="configSearchResults"
      :config-search-placeholder="t('config.search.placeholder')"
      :show-update-to-latest-button="showUpdateToLatestButton"
      :has-available-update="hasAvailableUpdate"
      :checking-update="checkingUpdate"
      :update-to-latest-label="updateToLatestLabel"
      :update-to-latest-title="updateToLatestTitle"
      :window-controls-visible="windowControlsVisible"
      :simple-setup-mode="!!config.simpleSetupMode"
      @start-drag="startDrag"
      @update:config-search-query="updateConfigSearchQuery"
      @select-config-search-result="handleSelectConfigSearchResult"
      @update-to-latest="triggerUpdateToLatest"
      @minimize-window="minimizeWindow"
      @toggle-maximize-window="toggleMaximizeWindow"
      @close-window="handleCloseWindow"
      @update:simple-setup-mode="setSimpleSetupMode"
    />

    <div class="window-content p-0 min-h-0 overflow-hidden">
      <ConfigView
        :config="config"
        :config-tab="configTab"
        :simple-setup-mode="!!config.simpleSetupMode"
        :ui-language="config.uiLanguage"
        :ui-font="config.uiFont"
        :code-font="config.codeFont"
        :locale-options="localeOptions"
        :current-theme="currentTheme"
        :theme-mode="themeMode"
        :auto-light-theme="autoLightTheme"
        :auto-dark-theme="autoDarkTheme"
        :generated-theme-controls="generatedThemeControls"
        :generated-theme-tokens="generatedThemeTokens"
        :generated-light-tokens="generatedLightTokens"
        :generated-dark-tokens="generatedDarkTokens"
        :ui-size-scale="config.uiSizeScale ?? 100"
        :selected-api-config="selectedApiConfig"
        :base-url-reference="baseUrlReference"
        :refreshing-models="refreshingModels"
        :model-options="selectedModelOptions"
        :model-refresh-ok="selectedModelRefreshOk"
        :model-refresh-error="modelRefreshError"
        :tool-statuses="toolStatuses"
        :personas="personas"
        :persona-avatar-url-map="chatPersonaAvatarUrlMap"
        :assistant-personas="assistantPersonas"
        :user-persona="userPersona"
        :persona-editor-id="personaEditorId"
        :assistant-agent-id="assistantAgentId"
        :selected-persona="selectedPersonaEditor"
        :tool-persona="selectedPersonaEditor"
        :selected-persona-avatar-url="selectedPersonaEditorAvatarUrl"
        :user-persona-avatar-url="userPersonaAvatarUrl"
        :response-style-options="responseStyleOptions"
        :response-style-id="selectedResponseStyleId"
        :pdf-read-mode="selectedPdfReadMode"
        :background-voice-screenshot-keywords="backgroundVoiceScreenshotKeywords"
        :background-voice-screenshot-mode="backgroundVoiceScreenshotMode"
        :instruction-presets="instructionPresets"
        :text-capable-api-configs="textCapableApiConfigs"
        :image-capable-api-configs="imageCapableApiConfigs"
        :stt-capable-api-configs="sttCapableApiConfigs"
        :avatar-saving="avatarSaving"
        :avatar-error="avatarError"
        :persona-saving="personaSaving"
        :persona-dirty="personaDirty"
        :config-dirty="configDirty"
        :saving-config="saving"
        :normalize-api-bindings-action="normalizeApiBindingsLocal"
        :hotkey-test-recording="hotkeyTestRecording"
        :hotkey-test-recording-ms="hotkeyTestRecordingMs"
        :hotkey-test-audio-ready="!!hotkeyTestAudio"
        :microphone-permission-state="microphonePermissionState"
        :microphone-permission-requesting="microphonePermissionRequesting"
        :checking-update="checkingUpdate"
        :has-available-update="hasAvailableUpdate"
        :save-config-action="saveConfig"
        :update-record-hotkey-action="updateRecordHotkey"
        :update-record-background-wake-enabled-action="updateRecordBackgroundWakeEnabled"
        :restore-config-action="restoreLastSavedConfigSnapshot"
        :last-saved-config-json="lastSavedConfigJson"
        :set-status-action="setStatus"
        :save-persona-relations="handleSavePersonaRelations"
        @update:config-tab="(value) => { configTab = value; }"
        @update:simple-setup-mode="setSimpleSetupMode"
        @update:ui-language="setUiLanguage"
        @update:persona-editor-id="updatePersonaEditorIdWithNotice"
        @update:response-style-id="(value) => { selectedResponseStyleId = value; }"
        @update:pdf-read-mode="(value) => { selectedPdfReadMode = value; }"
        @update:background-voice-screenshot-keywords="(value) => { backgroundVoiceScreenshotKeywords = String(value || '').replace(/，/g, ','); }"
        @update:background-voice-screenshot-mode="(value) => { backgroundVoiceScreenshotMode = value; }"
        @update:instruction-presets="updateInstructionPresets"
        @patch-conversation-api-settings="patchConversationApiSettings"
        @patch-chat-settings="patchChatSettings"
        @update:ui-size-scale="config.uiSizeScale = setUiSizeScale($event)"
        @update:ui-font="setUiFont($event)"
        @update:code-font="setCodeFont($event)"
        @update:github-update-method="updateGithubUpdateMethod"
        @set-theme="setTheme"
        @set-theme-mode="setThemeMode"
        @set-auto-theme="setAutoTheme"
        @activate-generated-theme="activateGeneratedTheme"
        @update-generated-theme-controls="updateGeneratedThemeControls"
        @reset-generated-theme="resetGeneratedTheme"
        @refresh-models="refreshModels"
        @add-api-config="addApiConfig"
        @remove-selected-api-config="removeSelectedApiConfig"
        @add-persona="addPersona"
        @remove-selected-persona="removeSelectedPersona"
        @reset-personas="loadPersonas"
        @save-personas="savePersonas"
        @convert-private-persona-to-public="convertPrivatePersonaToPublic"
        @import-persona-memories="importPersonaMemories"
        @open-conversation-list="openConversationList"
        @open-prompt-preview="openPromptPreviewFromConfig"
        @open-system-prompt-preview="openSystemPromptPreviewFromConfig"
        @open-memory-viewer="openMemoryViewer"
        @open-runtime-logs="openRuntimeLogs"
        @start-hotkey-record-test="startHotkeyRecordTest"
        @stop-hotkey-record-test="stopHotkeyRecordTest"
        @play-hotkey-record-test="playHotkeyRecordTest"
        @request-microphone-permission="requestMicrophonePermission"
        @capture-hotkey="captureHotkey"
        @save-agent-avatar="saveAgentAvatar"
        @clear-agent-avatar="clearAgentAvatar"
        @check-update="manualCheckGithubUpdate"
        @open-github="openGithubRepository"
        @start-chat="startChat"
      />
    </div>

    <dialog ref="memoryDialog" class="modal">
      <MemoryDialog
        :title="t('memory.title')"
        :empty-text="t('memory.empty')"
        :page-text="t('memory.page', { page: memoryPage, total: memoryPageCount })"
        :prev-page-text="t('memory.prevPage')"
        :next-page-text="t('memory.nextPage')"
        :export-text="t('memory.export')"
        :import-text="t('memory.import')"
        :close-text="t('common.close')"
        :memory-list="memoryList"
        :paged-memories="pagedMemories"
        :memory-page="memoryPage"
        :memory-page-count="memoryPageCount"
        @close="closeMemoryViewer"
        @prev-page="() => { memoryPage -= 1; }"
        @next-page="() => { memoryPage += 1; }"
        @export-memories="exportMemories"
        @trigger-import="triggerMemoryImport"
        @import-file="handleMemoryImportFile"
      />
    </dialog>

    <dialog ref="promptPreviewDialog" class="modal" @close="markPromptPreviewDialogClosed">
      <PromptPreviewDialog
        v-if="promptPreviewDialogOpen"
        :mode="promptPreviewMode"
        :conversation-scope="promptPreviewConversationScope"
        :loading="promptPreviewLoading"
        :title="promptPreviewMode === 'system' ? t('prompt.systemPreview') : t('prompt.requestPreview')"
        :loading-text="t('common.loading')"
        :empty-hint="t('prompt.emptyHint')"
        :chat-text="t('prompt.chat')"
        :compaction-text="t('prompt.compaction')"
        :archive-text="t('prompt.archive')"
        :local-scope-text="t('prompt.local')"
        :remote-scope-text="t('prompt.remote')"
        :delegate-scope-text="t('prompt.delegate')"
        :conversation-text="t('prompt.conversation')"
        :selected-conversation-id="promptPreviewConversationId"
        :conversation-options="promptPreviewConversationOptions"
        :latest-input-length-text="t('prompt.latestInputLength')"
        :images-text="t('prompt.images')"
        :audios-text="t('prompt.audios')"
        :close-text="t('common.close')"
        :latest-user-text="promptPreviewLatestUserText"
        :latest-images="promptPreviewLatestImages"
        :latest-audios="promptPreviewLatestAudios"
        :text="promptPreviewText"
        @select-mode="loadPromptPreview"
        @select-scope="selectPromptPreviewConversationScope"
        @select-conversation="selectPromptPreviewConversation"
        @close="closePromptPreview"
      />
    </dialog>

    <ShellDialogsHost
      :update-dialog-open="updateDialogOpen"
      :update-dialog-title="updateDialogTitle"
      :update-dialog-body="updateDialogBody"
      :update-dialog-kind="updateDialogKind"
      :update-dialog-release-url="updateDialogReleaseUrl"
      :update-dialog-primary-action="updateDialogPrimaryAction"
      :update-progress-percent="updateProgressPercent"
      :update-dialog-skip-version-visible="updateDialogSkipVersionVisible"
      :update-dialog-cancel-update-visible="updateDialogCancelUpdateVisible"
      :update-dialog-cancel-pending="updateCancelPending"
      :portable-pending="portablePending"
      :is-portable-pending="!!portablePending"
      :markdown-is-dark="markdownIsDark"
      :runtime-logs-dialog-open="false"
      :runtime-logs="[]"
      :runtime-logs-loading="false"
      runtime-logs-error=""
      :rewind-confirm-dialog-open="false"
      :rewind-confirm-can-undo-patch="false"
      :branch-from-message-confirm-dialog-open="false"
      :config-save-error-dialog-open="configSaveErrorDialogOpen"
      :config-save-error-dialog-title="configSaveErrorDialogTitle"
      :config-save-error-dialog-body="configSaveErrorDialogBody"
      :config-save-error-dialog-kind="configSaveErrorDialogKind"
      :archive-import-preview-dialog-open="false"
      :archive-import-preview="null"
      :archive-import-running="false"
      :skill-placeholder-dialog-open="false"
      :trim-action-dialog-open="false"
      :trim-preview-loading="false"
      :trim-preview="null"
      :trim-compaction-preview="null"
      :trimming="false"
      @close-update-dialog="closeUpdateDialog"
      @confirm-update-dialog-primary="confirmUpdateDialogPrimary"
      @open-update-release="openUpdateRelease"
      @open-update-repository="openGithubRepository"
      @skip-update-version="skipCurrentUpdateVersion"
      @cancel-update="cancelGithubUpdate"
      @open-portable-pending-dir="openPortablePendingDir"
      @retry-portable-pending="retryPortablePending"
      @dismiss-portable-pending="dismissPortablePending"
      @close-settings-save-error-dialog="closeSettingsSaveErrorDialog"
    />

    <div
      v-if="messageStoreMigration.visible"
      class="fixed inset-0 z-9999 flex items-center justify-center bg-base-300/90 p-6 backdrop-blur"
    >
      <div class="w-full max-w-2xl rounded-box border border-base-content/10 bg-base-100 p-6 shadow-2xl">
        <div class="text-xl font-semibold">{{ t("config.messageStoreMigration.title") }}</div>
        <div class="mt-2 text-sm opacity-70">{{ messageStoreMigration.message }}</div>
        <progress
          v-if="messageStoreMigration.mode === 'migrating' || messageStoreMigration.mode === 'waiting'"
          class="progress progress-primary mt-5 w-full"
          :value="messageStoreMigration.current"
          :max="Math.max(messageStoreMigration.total, 1)"
        />
        <div
          v-if="messageStoreMigration.mode === 'migrating'"
          class="mt-2 text-xs opacity-60"
        >
          {{ messageStoreMigration.current }} / {{ messageStoreMigration.total }}
        </div>
        <div v-if="messageStoreMigration.mode === 'completed'" class="mt-2 text-xs opacity-60">
          {{ t("config.messageStoreMigration.migratedCount") }}: {{ messageStoreMigration.migratedCount }}
          ·
          {{ t("config.messageStoreMigration.discardedCount") }}: {{ messageStoreMigration.discardedCount }}
        </div>
        <div v-if="messageStoreMigration.mode === 'completed'" class="mt-5 flex justify-end">
          <button class="btn btn-primary" @click="confirmMessageStoreMigrationSummary">
            {{ t("config.messageStoreMigration.confirm") }}
          </button>
        </div>
        <div v-if="messageStoreMigration.mode === 'error'" class="mt-5 flex justify-end">
          <button class="btn btn-primary" @click="retryMessageStoreMigration">
            {{ t("config.messageStoreMigration.retry") }}
          </button>
        </div>
      </div>
    </div>

    <StartupOverlay v-if="startupOverlayVisible" />

    <ConfigStatusToast :text="status" :tone="statusTone" />

    <FileLinkContextMenu
      :state="fileLinkMenuState"
      :can-use-local-file-actions="false"
      @close="closeFileLinkMenu"
    />

    <Win10ResizeHandles :enabled="!maximized" />
    <UnsavedConfirmDialog />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import ConfigView from "./features/config/views/ConfigView.vue";
import UnsavedConfirmDialog from "./features/config/components/UnsavedConfirmDialog.vue";
import { provideUnsavedChangesGuard } from "./features/config/composables/use-unsaved-changes-guard";
import AppWindowHeader from "./features/shell/components/AppWindowHeader.vue";
import ShellDialogsHost from "./features/shell/components/ShellDialogsHost.vue";
import StartupOverlay from "./features/shell/components/StartupOverlay.vue";
import Win10ResizeHandles from "./features/shell/components/Win10ResizeHandles.vue";
import ConfigStatusToast from "./features/config/components/ConfigStatusToast.vue";
import MemoryDialog from "./features/memory/components/dialogs/MemoryDialog.vue";
import PromptPreviewDialog from "./features/chat/components/dialogs/PromptPreviewDialog.vue";
import { getTransportCapabilities, invokeTauri, openTransportWindow } from "./services/tauri-api";
import FileLinkContextMenu from "./features/shared/components/FileLinkContextMenu.vue";
import { useFileLinkContextMenu } from "./features/shared/composables/use-file-link-context-menu";
import type { AppConfig, PromptCommandPreset } from "./types/app";
import { normalizeLocale } from "./i18n";
import { MODEL_ROLE_EXPERT_API_CONFIG_ID, resolveModelRoleApiConfigId } from "./features/config/utils/model-role-options";
import { useWindowShell } from "./features/shell/composables/use-window-shell";
import { useAppTheme, isDarkAppTheme } from "./features/shell/composables/use-app-theme";
import { useAppLifecycle } from "./features/shell/composables/use-app-lifecycle";
import { useAppCore } from "./features/shell/composables/use-app-core";
import { useConfigCore } from "./features/config/composables/use-config-core";
import { useConfigRuntime } from "./features/config/composables/use-config-runtime";
import { useConfigPersistence } from "./features/config/composables/use-config-persistence";
import { useConfigEditors } from "./features/config/composables/use-config-editors";
import { useAppWatchers } from "./features/shell/composables/use-app-watchers";
import { searchConfigTabs, type ConfigSearchTab } from "./features/config/search/config-search";
import { applyCodeFont, applyUiFont, normalizeUiFont } from "./features/shell/composables/use-ui-font";
import { normalizeUiSizeScale, useUiSizeAppearance } from "./features/shell/composables/use-ui-size-appearance";
import { useGithubUpdateMethod } from "./features/shell/composables/use-github-update-method";
import { useGithubUpdateView } from "./features/shell/composables/use-github-update-view";
import { useConfigSaveErrorDialog } from "./features/shell/composables/use-config-save-error-dialog";
import { useWindowActions } from "./features/shell/composables/use-window-actions";
import { useAvatarCache } from "./features/chat/composables/use-avatar-cache";
import { useMemoryViewer } from "./features/memory/composables/use-memory-viewer";
import { usePromptPreview } from "./features/chat/composables/use-prompt-preview";
import { useHotkeyRecordTest } from "./features/shell/composables/use-hotkey-record-test";
import { useChatConfigActionsGlue } from "./features/chat/composables/use-chat-config-actions-glue";
import { useChatConfigDerivedState } from "./features/chat/composables/use-chat-config-derived-state";
import { useChatConfigUiDerivedState } from "./features/chat/composables/use-chat-config-ui-derived-state";
import { useConfigWindowBootstrap } from "./features/config/composables/use-config-window-bootstrap";
import { useMessageStoreMigrationGate } from "./features/shell/composables/use-message-store-migration-gate";
import { formatI18nError } from "./utils/error";

const { t, locale } = useI18n();
const tr = (key: string, params?: Record<string, unknown>) => t(key, params as never);
const unsavedGuard = provideUnsavedChangesGuard();

// 设置窗口只做配置展示，不提供需要本机文件的动作；文件链接右键只留「复制路径」，
// 复制内容就是链接原文，因此不需要工作区根。
const { state: fileLinkMenuState, close: closeFileLinkMenu } = useFileLinkContextMenu();
const isMacPlatform = /Mac|iPhone|iPad|iPod/i.test(window.navigator.platform || "");
const windowControlsVisible = getTransportCapabilities().windowControls;

/** iframe 嵌入且非 VSCode 宿主时隐藏窗口栏：远程前端模式下由宿主壳层提供 header。 */
const hideWindowHeader = (() => {
  if (window.self === window.top) return false;
  const bridgeWindow = window as Window & { acquireVsCodeApi?: unknown };
  const isVscodeHost =
    typeof bridgeWindow.acquireVsCodeApi === "function"
    || window.location.protocol === "vscode-webview:";
  return !isVscodeHost;
})();

type ConfigTab =
  | "welcome"
  | "hotkey"
  | "api"
  | "mcp"
  | "skill"
  | "catalog"
  | "persona"
  | "demo"
  | "chatSettings"
  | "notification"
  | "networkAccess"
  | "remoteIm"
  | "usage"
  | "memory"
  | "task"
  | "logs"
  | "appearance"
  | "migration"
  | "about";

const config = reactive<AppConfig>({
  hotkey: "Alt+·",
  uiLanguage: "zh-CN",
  uiFont: "auto",
  codeFont: "auto",
  uiSizeScale: 100,
  webAccessPort: 8429,
  webAccessEnabled: true,
  webAccessPassword: "",
  githubUpdateMethod: "auto",
  skippedGithubUpdateVersion: "",
  recordHotkey: "CapsLock",
  recordBackgroundWakeEnabled: false,
  minRecordSeconds: 1,
  maxRecordSeconds: 60,
  llmRoundLogCapacity: 3,
  messageNotificationEnabled: true,
  messageNotificationSoundEnabled: false,
  desktopOperationNoticeEnabled: true,
  desktopOperateEnabled: true,
  selectedApiConfigId: "",
  expertApiConfigId: "",
  visionApiConfigId: undefined,
  imageGenerationModelId: undefined,
  toolReviewApiConfigId: undefined,
  sttApiConfigId: undefined,
  sttAutoSend: false,
  terminalShellKind: "auto",
  shellWorkspaces: [],
  mcpServers: [],
  remoteImChannels: [],
  apiProviders: [],
  imageProviders: [],
  apiConfigs: [],
});

const personas = ref([] as import("./types/app").PersonaProfile[]);
const assistantAgentId = ref("default-agent");
const personaEditorId = ref("default-agent");
const userAlias = ref(t("archives.roleUser"));
const selectedResponseStyleId = ref("concise");
const selectedPdfReadMode = ref<"text" | "image">("image");
const backgroundVoiceScreenshotKeywords = ref("");
const backgroundVoiceScreenshotMode = ref<"desktop" | "focused_window">("focused_window");
const instructionPresets = ref<PromptCommandPreset[]>([]);
const configTab = ref<ConfigTab>("welcome");
const configSearchQuery = ref("");

const status = ref("");
const suppressAutosave = ref(false);
const loading = ref(false);
const saving = ref(false);
const personaSaving = ref(false);
const lastSavedConfigJson = ref("");
const lastSavedPersonasJson = ref("");

const refreshingModels = ref(false);
const modelRefreshError = ref("");
const modelRefreshOkFlags = ref<Record<string, boolean>>({});
const apiModelOptions = ref<Record<string, string[]>>({});
const checkingToolsStatus = ref(false);
const toolStatuses = ref([] as import("./types/app").ToolLoadStatus[]);
const avatarSaving = ref(false);
const avatarError = ref("");

const viewMode = ref<"chat" | "archives" | "config">("config");
const {
  windowReady,
  maximized,
  initWindow,
  syncWindowControlsState,
  closeWindow,
  startDrag,
  minimizeWindow,
  toggleMaximizeWindow,
} = useWindowShell();

const {
  currentTheme,
  generatedThemeControls,
  generatedThemeTokens,
  generatedThemeTokensByMode,
  themeMode,
  autoLightTheme,
  autoDarkTheme,
  applyTheme,
  setTheme,
  setThemeMode,
  setAutoTheme,
  activateGeneratedTheme,
  updateGeneratedThemeControls,
  resetGeneratedTheme,
  restoreThemeFromStorage,
} = useAppTheme();

const generatedLightTokens = computed(() => generatedThemeTokensByMode.value.light);
const generatedDarkTokens = computed(() => generatedThemeTokensByMode.value.dark);

const markdownIsDark = computed(() => isDarkAppTheme(currentTheme.value));

const {
  setStatus,
  setStatusError,
  statusTone,
  localeOptions,
  applyUiLanguage,
} = useAppCore({
  t: tr,
  config,
  locale,
  status,
  perfDebug: false,
});

const {
  messageStoreMigration,
  ensureMessageStoreMigrationGate,
  confirmMessageStoreMigrationSummary,
  retryMessageStoreMigration,
} = useMessageStoreMigrationGate({
  formatRequestFailed: (error) => formatI18nError(tr, "status.requestFailed", error),
  t: tr,
});
const {
  hotkeyTestRecording,
  hotkeyTestRecordingMs,
  hotkeyTestAudio,
  microphonePermissionState,
  microphonePermissionRequesting,
  startHotkeyRecordTest,
  stopHotkeyRecordTest,
  playHotkeyRecordTest,
  requestMicrophonePermission,
  cleanupHotkeyRecordTest,
} = useHotkeyRecordTest({
  t: tr,
  setStatus,
  setStatusError,
});
const startupOverlayVisible = ref(true);
const { setUiSizeScale, uiSizeScale } = useUiSizeAppearance();
const { updateGithubUpdateMethod } = useGithubUpdateMethod(config, setStatusError);

const {
  selectedApiConfig,
  selectedApiProvider,
  textCapableApiConfigs,
  imageCapableApiConfigs,
  sttCapableApiConfigs,
  normalizeRuntimeConfigNumbers,
} = useChatConfigDerivedState(config);

const userPersona = computed(() => personas.value.find((p) => p.isBuiltInUser || p.id === "user-persona") ?? null);
const assistantPersonas = computed(() =>
  personas.value.filter((p) => !p.isBuiltInUser && !p.isBuiltInSystem && p.id !== "user-persona" && p.id !== "system-persona"),
);
const selectedPersonaEditor = computed(() => personas.value.find((p) => p.id === personaEditorId.value) ?? null);
const toolApiConfig = computed(() => {
  const persona = personas.value.find(
    (item) => String(item.id || "").trim() === String(assistantAgentId.value || "").trim(),
  ) ?? null;
  const ids = Array.isArray(persona?.apiConfigIds)
    ? persona.apiConfigIds.map((id) => String(id || "").trim()).filter(Boolean)
    : [];
  // 人格未显式指定模型时按「专家」模型角色解析，与后端 agent_api_config_ids 的默认一致。
  const resolved = resolveModelRoleApiConfigId(ids[0] || MODEL_ROLE_EXPERT_API_CONFIG_ID, config);
  return config.apiConfigs.find((a) => a.id === resolved) ?? null;
});

const { resolveAvatarUrl, resolveBrandAvatarUrl, ensureAvatarCached, preloadPersonaAvatars } = useAvatarCache({ personas });
const userAvatarUrl = computed(() => resolveAvatarUrl(userPersona.value?.avatarPath, userPersona.value?.avatarUpdatedAt));
const userPersonaAvatarUrl = computed(() => userAvatarUrl.value);
const selectedPersonaEditorAvatarUrl = computed(() => resolveAvatarUrl(selectedPersonaEditor.value?.avatarPath, selectedPersonaEditor.value?.avatarUpdatedAt));
const chatPersonaNameMap = computed<Record<string, string>>(() => {
  const next: Record<string, string> = {};
  for (const persona of personas.value) {
    const id = String(persona.id || "").trim();
    if (!id) continue;
    next[id] = String(persona.name || "").trim() || id;
  }
  return next;
});
const chatPersonaAvatarUrlMap = computed<Record<string, string>>(() => {
  const next: Record<string, string> = {};
  for (const persona of personas.value) {
    const id = String(persona.id || "").trim();
    if (!id) continue;
    const url = resolveAvatarUrl(persona.avatarPath, persona.avatarUpdatedAt);
    if (url) {
      next[id] = url;
    } else if (!persona.isBuiltInUser && persona.id !== "user-persona") {
      next[id] = resolveBrandAvatarUrl();
    }
  }
  return next;
});

const configSearchResults = computed(() => searchConfigTabs(configSearchQuery.value, normalizeLocale(config.uiLanguage)));

const {
  defaultApiTools,
  createApiProvider,
  createApiConfig,
  normalizeApiBindingsLocal,
  buildConfigPayload,
  buildConfigSnapshotJson,
} = useConfigCore({
  config,
  textCapableApiConfigs,
  t,
});

const {
  selectedModelOptions,
  selectedModelRefreshOk,
  responseStyleOptions,
  baseUrlReference,
  configDirty,
  personaDirty,
  responseStyleIds,
} = useChatConfigUiDerivedState({
  config,
  apiModelOptions,
  modelRefreshOkFlags,
  selectedApiConfig,
  personas,
  lastSavedConfigJson,
  lastSavedPersonasJson,
  buildConfigSnapshotJson,
  t: tr,
});

const {
  syncTrayIcon,
  saveAgentAvatar,
  clearAgentAvatar,
  refreshModels,
  refreshToolsStatus,
} = useConfigRuntime({
  t: tr,
  setStatus,
  setStatusError,
  personas,
  assistantAgentId,
  toolAgentId: assistantAgentId,
  avatarSaving,
  avatarError,
  selectedApiConfig,
  selectedApiProvider,
  refreshingModels,
  modelRefreshError,
  apiModelOptions,
  modelRefreshOkFlags,
  toolApiConfig,
  checkingToolsStatus,
  toolStatuses,
  ensureAvatarCached,
});

function syncUserAliasFromPersona() {
  const next = (userPersona.value?.name || "").trim() || t("archives.roleUser");
  if (userAlias.value !== next) {
    userAlias.value = next;
  }
}

const {
  configSaveErrorDialogOpen,
  configSaveErrorDialogTitle,
  configSaveErrorDialogBody,
  configSaveErrorDialogKind,
  closeSettingsSaveErrorDialog,
  openSettingsSaveErrorDialog,
} = useConfigSaveErrorDialog({
  t: tr,
  configTab,
});

const {
  buildPersonasSnapshotJson,
  setUiLanguage,
  importPersonaMemories,
  handleToolsChanged,
} = useChatConfigActionsGlue({
  t: tr,
  config,
  locale,
  personas,
  configTab,
  lastSavedConfigJson,
  normalizeLocale,
  applyUiLanguage,
  buildConfigSnapshotJson,
  refreshToolsStatus,
  setStatus,
  setStatusError,
});

const configPersistence = useConfigPersistence({
  t: tr,
  setStatus,
  setStatusError,
  onSaveConfigError: openSettingsSaveErrorDialog,
  config,
  locale,
  normalizeLocale,
  suppressAutosave,
  loading,
  saving,
  savingPersonas: personaSaving,
  personas,
  assistantPersonas,
  assistantAgentId,
  personaEditorId,
  userAlias,
  selectedResponseStyleId,
  selectedPdfReadMode,
  backgroundVoiceScreenshotKeywords,
  backgroundVoiceScreenshotMode,
  instructionPresets,
  responseStyleIds,
  createApiConfig,
  normalizeApiBindingsLocal,
  buildConfigPayload,
  buildConfigSnapshotJson,
  buildPersonasSnapshotJson,
  lastSavedConfigJson,
  lastSavedPersonasJson,
  syncUserAliasFromPersona,
  preloadPersonaAvatars,
  syncTrayIcon,
});

const {
  loadBootstrapSnapshot,
  saveConfig,
  captureHotkey,
  updateRecordHotkey,
  updateRecordBackgroundWakeEnabled,
  loadPersonas,
  savePersonas,
  saveChatPreferences,
  convertPrivatePersonaToPublic,
  patchChatSettings,
  patchConversationApiSettings,
  restoreLastSavedConfigSnapshot,
  restoreLastSavedPersonasSnapshot,
} = configPersistence;

const {
  addApiConfig,
  removeSelectedApiConfig,
  addPersona,
  setPersonaChildAgentsBatch,
  removeSelectedPersona,
} = useConfigEditors({
  t: tr,
  config,
  personas,
  assistantPersonas,
  assistantAgentId,
  personaEditorId,
  selectedPersonaEditor,
  createApiConfig,
  createApiProvider,
  normalizeApiBindingsLocal,
  savePersonas,
  saveChatPreferences,
  saveConfig,
});

const {
  checkingUpdate,
  hasAvailableUpdate,
  latestCheckResult,
  updateReadyToRestart,
  updateCancelPending,
  updateDialogOpen,
  updateDialogTitle,
  updateDialogBody,
  updateDialogKind,
  updateDialogReleaseUrl,
  updateDialogPrimaryAction,
  updateProgressPercent,
  updateDialogSkipVersionVisible,
  updateDialogCancelUpdateVisible,
  closeUpdateDialog,
  openUpdateRelease,
  confirmUpdateDialogPrimary,
  refreshGithubUpdateState,
  manualCheckGithubUpdate,
  triggerUpdateToLatest,
  cancelGithubUpdate,
  skipCurrentUpdateVersion,
  showUpdateToLatestButton,
  updateToLatestLabel,
  updateToLatestTitle,
  portablePending,
  openPortablePendingDir,
  retryPortablePending,
  dismissPortablePending,
} = useGithubUpdateView({
  t: tr,
  viewMode,
  status,
  updateMethod: computed(() => config.githubUpdateMethod || "auto"),
  skippedVersion: computed(() => config.skippedGithubUpdateVersion || ""),
  onSkippedVersionSaved: (saved) => {
    config.skippedGithubUpdateVersion = saved.skippedGithubUpdateVersion || "";
  },
});

const { openGithubRepository } = useWindowActions({
  closeWindow,
  minimizeWindow,
  freezeForegroundConversation: () => undefined,
});

const {
  memoryDialog,
  memoryList,
  memoryPage,
  memoryPageCount,
  pagedMemories,
  openMemoryViewer,
  closeMemoryViewer,
  exportMemories,
  triggerMemoryImport,
  handleMemoryImportFile,
} = useMemoryViewer({
  t: tr,
  setStatus,
  setStatusError,
});

const promptPreviewCurrentConversationId = ref("");
const promptPreviewLocalConversations = computed(() => [] as import("./types/app").UnarchivedConversationSummary[]);
const promptPreviewRemoteConversations = computed(() => [] as import("./types/app").RemoteImContactConversationSummary[]);
const promptPreviewDelegateConversations = computed(() => [] as import("./types/app").DelegateConversationSummary[]);
const {
  promptPreviewDialog,
  promptPreviewDialogOpen,
  markPromptPreviewDialogClosed,
  promptPreviewLoading,
  promptPreviewText,
  promptPreviewLatestUserText,
  promptPreviewLatestImages,
  promptPreviewLatestAudios,
  promptPreviewMode,
  promptPreviewConversationScope,
  promptPreviewConversationId,
  promptPreviewConversationOptions,
  loadPromptPreview,
  openPromptPreview,
  openSystemPromptPreview,
  selectPromptPreviewConversationScope,
  selectPromptPreviewConversation,
  closePromptPreview,
} = usePromptPreview({
  t: tr,
  currentConversationId: promptPreviewCurrentConversationId,
  localConversations: promptPreviewLocalConversations,
  remoteConversations: promptPreviewRemoteConversations,
  delegateConversations: promptPreviewDelegateConversations,
});

function updateConfigSearchQuery(value: string) {
  configSearchQuery.value = String(value || "");
}

async function handleCloseWindow() {
  const allow = await unsavedGuard.confirmLeaveIfDirty();
  if (!allow) return;
  closeWindow();
}

async function setSimpleSetupMode(value: boolean) {
  const next = !!value;
  if (!!config.simpleSetupMode === next) return;
  const allow = await unsavedGuard.confirmLeaveIfDirty();
  if (!allow) return;
  config.simpleSetupMode = next;
}

function setUiFont(value: string) {
  const next = normalizeUiFont(value);
  if (config.uiFont === next) return;
  config.uiFont = next;
  void saveConfig();
}

function setCodeFont(value: string) {
  const next = normalizeUiFont(value);
  if (config.codeFont === next) return;
  config.codeFont = next;
  void saveConfig();
}

function startChat() {
  void openTransportWindow("chat");
}

async function handleSelectConfigSearchResult(tab: ConfigSearchTab) {
  if (tab === configTab.value) {
    configSearchQuery.value = "";
    return;
  }
  const allow = await unsavedGuard.confirmLeaveIfDirty();
  if (!allow) return;
  configTab.value = tab;
  configSearchQuery.value = "";
}

async function updatePersonaEditorIdWithNotice(value: string) {
  const nextId = String(value || "").trim();
  if (!nextId || nextId === personaEditorId.value) return;
  if (personaDirty.value) {
    const currentName = String(selectedPersonaEditor.value?.name || personaEditorId.value || "").trim() || t("config.persona.title");
    const allow = await unsavedGuard.confirmLeaveIfDirty({
      title: t("config.unsavedConfirm.title"),
      message: t("status.personaUnsavedSwitchHint", { name: currentName }),
      canSaveAndLeave: true,
      saveAndLeaveText: t("config.unsavedConfirm.saveAndLeave"),
    });
    if (!allow) return;
  }
  personaEditorId.value = nextId;
}

/** 人格上下级关系的批量保存：把有变更的下级一次性写回，返回是否成功。 */
async function handleSavePersonaRelations(
  updates: { agentId: string; childAgentIds: string[] }[],
): Promise<boolean> {
  const ok = await setPersonaChildAgentsBatch({ updates });
  if (!ok) {
    setStatus(tr("config.persona.saveRelationsFailed"));
    return false;
  }
  setStatus(tr("config.persona.saveRelationsSuccess"));
  return true;
}

function updateInstructionPresets(value: PromptCommandPreset[]) {
  instructionPresets.value = Array.isArray(value)
    ? value
      .map((item) => ({
        id: String(item?.id || "").trim(),
        name: String(item?.prompt || item?.name || "").trim(),
        prompt: String(item?.prompt || item?.name || "").trim(),
      }))
      .filter((item) => !!item.id && !!item.prompt)
    : [];
}

async function openConversationList() {
  try {
    await openTransportWindow("archives");
  } catch (error) {
    setStatusError("status.requestFailed", error);
  }
}

async function openPromptPreviewFromConfig() {
  const apiConfigId = String(config.expertApiConfigId || config.selectedApiConfigId || "").trim();
  const agentId = String(assistantAgentId.value || "").trim();
  if (!apiConfigId || !agentId) return;
  await openPromptPreview(apiConfigId, agentId);
}

async function openSystemPromptPreviewFromConfig() {
  const apiConfigId = String(config.expertApiConfigId || config.selectedApiConfigId || "").trim();
  const agentId = String(assistantAgentId.value || "").trim();
  if (!apiConfigId || !agentId) return;
  await openSystemPromptPreview(apiConfigId, agentId);
}

function openRuntimeLogs() {
  void openTransportWindow("runtimeLogs").catch((error) => {
    console.warn("[运行日志] 打开日志窗口失败", error);
  });
}

async function refreshAllViewData() {
  await loadBootstrapSnapshot();
}

const appBootstrap = useConfigWindowBootstrap({
  viewMode,
  initWindow,
  applyTheme,
  normalizeLocale,
  config,
  locale,
  assistantAgentId,
  personaEditorId,
  userAlias,
  selectedResponseStyleId,
  selectedPdfReadMode,
  backgroundVoiceScreenshotKeywords,
  backgroundVoiceScreenshotMode,
  instructionPresets,
  createApiConfig,
  buildConfigSnapshotJson,
  lastSavedConfigJson,
  normalizeUiSizeScale,
  updateGithubUpdateMethod,
  normalizeRuntimeConfigNumbers,
});

useAppLifecycle({
  appBootstrapMount: appBootstrap.mount,
  appBootstrapUnmount: appBootstrap.unmount,
  restoreThemeFromStorage,
  onPaste: () => undefined,
  onDragOver: (event) => { event.preventDefault(); },
  onDrop: (event) => { event.preventDefault(); },
  recordHotkeyMount: () => undefined,
  recordHotkeyUnmount: () => undefined,
  prepareInitialData: ensureMessageStoreMigrationGate,
  refreshAllViewData,
  viewMode,
  syncWindowControlsState,
  stopRecording: async () => undefined,
  cleanupSpeechRecording: () => undefined,
  cleanupChatMedia: cleanupHotkeyRecordTest,
  onBackendReadyChange: (ready) => {
    startupOverlayVisible.value = !ready;
  },
  afterMountedReady: async () => {
    await refreshGithubUpdateState();
  },
});

useAppWatchers({
  config,
  configTab,
  viewMode,
  personas,
  userPersona,
  assistantPersonas,
  assistantAgentId,
  personaEditorId,
  selectedApiConfig,
  toolApiConfig,
  modelRefreshError,
  toolStatuses,
  defaultApiTools,
  t: tr,
  normalizeApiBindingsLocal,
  syncUserAliasFromPersona,
  syncTrayIcon,
  refreshToolsStatus,
});

watch(
  () => ({ uiFont: config.uiFont, codeFont: config.codeFont, uiLanguage: config.uiLanguage, uiSizeScale: config.uiSizeScale }),
  ({ uiFont, codeFont, uiLanguage, uiSizeScale }) => {
    applyUiFont(uiFont, uiLanguage);
    applyCodeFont(codeFont);
    config.uiFont = normalizeUiFont(uiFont);
    config.codeFont = normalizeUiFont(codeFont);
    config.uiSizeScale = setUiSizeScale(uiSizeScale);
  },
  { immediate: true },
);

// Ctrl/Meta + 滑轮会改全局 uiSizeScale，需回写设置页当前值。
watch(uiSizeScale, (scale) => {
  if (config.uiSizeScale !== scale) {
    config.uiSizeScale = scale;
  }
});

onMounted(() => {
  unsavedGuard.registerDirtyChecker("global-config", {
    isDirty: () => {
      const dirty = configDirty.value || personaDirty.value;
      if (dirty) {
        // 排障探针：diff 出具体哪些字段不一致，定位「用户没改却弹未保存」的根源。
        try {
          const cur = JSON.parse(buildConfigSnapshotJson()) as Record<string, unknown>;
          const saved = JSON.parse(lastSavedConfigJson.value) as Record<string, unknown>;
          const diffKeys = [...new Set([...Object.keys(cur), ...Object.keys(saved)])]
            .filter((key) => JSON.stringify(cur[key]) !== JSON.stringify(saved[key]));
          const lines: string[] = [
            `[unsaved-guard] global-config dirty: configDirty=${configDirty.value} personaDirty=${personaDirty.value} diffKeys=${JSON.stringify(diffKeys)}`,
          ];
          // apiProviders 按 provider.id 拆字段级 diff
          if (diffKeys.includes("apiProviders")) {
            const savedProviders = Array.isArray(saved.apiProviders) ? saved.apiProviders as Array<Record<string, unknown>> : [];
            const curProviders = Array.isArray(cur.apiProviders) ? cur.apiProviders as Array<Record<string, unknown>> : [];
            for (const curP of curProviders) {
              const savedP = savedProviders.find((p) => p?.id === curP?.id);
              if (!savedP) {
                lines.push(`  added provider ${curP.id} (${curP.name})`);
                continue;
              }
              const pDiffKeys = [...new Set([...Object.keys(savedP), ...Object.keys(curP)])]
                .filter((k) => JSON.stringify(savedP[k]) !== JSON.stringify(curP[k]));
              if (pDiffKeys.length === 0) continue;
              lines.push(`  provider ${curP.id} (${curP.name}) diffKeys=${JSON.stringify(pDiffKeys)}`);
              for (const k of pDiffKeys) {
                // models 数组再拆一层，按 model.id 找字段差异
                if (k === "models" && Array.isArray(savedP[k]) && Array.isArray(curP[k])) {
                  const savedModels = savedP[k] as Array<Record<string, unknown>>;
                  const curModels = curP[k] as Array<Record<string, unknown>>;
                  for (const cm of curModels) {
                    const sm = savedModels.find((m) => m?.id === cm?.id);
                    if (!sm) {
                      lines.push(`    added model ${cm.id} (${cm.model})`);
                      continue;
                    }
                    const mDiffKeys = [...new Set([...Object.keys(sm), ...Object.keys(cm)])]
                      .filter((mk) => JSON.stringify(sm[mk]) !== JSON.stringify(cm[mk]));
                    if (mDiffKeys.length === 0) continue;
                    lines.push(`    model ${cm.id} (${cm.model}) diffKeys=${JSON.stringify(mDiffKeys)}`);
                    for (const mk of mDiffKeys) {
                      lines.push(`      ${mk}: saved=${JSON.stringify(sm[mk])} current=${JSON.stringify(cm[mk])}`);
                    }
                  }
                  for (const sm of savedModels) {
                    if (!curModels.find((m) => m?.id === sm?.id)) {
                      lines.push(`    removed model ${sm.id} (${sm.model})`);
                    }
                  }
                } else {
                  lines.push(`    ${k}: saved=${JSON.stringify(savedP[k])?.slice(0, 200)} current=${JSON.stringify(curP[k])?.slice(0, 200)}`);
                }
              }
            }
            for (const savedP of savedProviders) {
              if (!curProviders.find((p) => p?.id === savedP?.id)) {
                lines.push(`  removed provider ${savedP.id} (${savedP.name})`);
              }
            }
          }
          void invokeTauri("append_runtime_log_probe", {
            message: lines.join("\n"),
            level: "debug",
          }).catch(() => {});
        } catch {
          /* 探针失败不影响主流程 */
        }
      }
      return dirty;
    },
    title: t("config.unsavedConfirm.title"),
    message: t("config.unsavedConfirm.message"),
    onDiscard: () => {
      restoreLastSavedConfigSnapshot();
      restoreLastSavedPersonasSnapshot();
    },
    onSave: async () => {
      // 任一保存失败都立即返回 false，让 guard 中止离开；全部成功才返回 true。
      if (configDirty.value) {
        const ok = await saveConfig();
        if (!ok) return false;
      }
      if (personaDirty.value) {
        const ok = await savePersonas();
        if (!ok) return false;
      }
      return true;
    },
  });

  const handleBeforeUnload = (event: BeforeUnloadEvent) => {
    if (unsavedGuard.hasDirtyState()) {
      event.preventDefault();
      event.returnValue = "";
    }
  };
  window.addEventListener("beforeunload", handleBeforeUnload);
  onBeforeUnmount(() => {
    window.removeEventListener("beforeunload", handleBeforeUnload);
  });
});
</script>
