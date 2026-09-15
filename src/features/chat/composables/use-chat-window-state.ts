import { reactive, ref, shallowRef } from "vue";
import type { TerminalApprovalRequestPayload } from "../../shell/composables/use-terminal-approval";
import type {
  AppConfig,
  ChatMessage,
  ChatTodoItem,
  PersonaProfile,
  PromptCommandPreset,
  ToolLoadStatus,
} from "../../../types/app";
import type { ContextUsageUpdatePayload } from "./use-chat-flow-events";

type UseChatWindowStateOptions = {
  isMacPlatform: boolean;
  t: (key: string) => string;
};

export function useChatWindowState(options: UseChatWindowStateOptions) {
  const BACKGROUND_CONVERSATION_CACHE_LIMIT = 10;
  const FOREGROUND_SNAPSHOT_RECENT_LIMIT = 4;
  const OLDER_HISTORY_PAGE_SIZE = 4;
  type BackgroundConversationBadgeState = "completed" | "failed";

  const viewMode = ref<"chat" | "archives" | "config">("config");
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
  const recordHotkeyProbeLastSeq = ref(0);
  const recordHotkeyProbeDown = ref(false);
  const chatWindowActiveSynced = ref<boolean | null>(null);
  const chatWindowEventUnlisteners: Record<string, (() => void) | null> = {
    chatHistoryFlushed: null,
    chatRoundStarted: null,
    chatRoundCompleted: null,
    chatRoundFailed: null,
    chatAssistantDelta: null,
    chatStreamRebindRequired: null,
    chatConversationMessagesAfterSynced: null,
    chatConversationMessageAppended: null,
    chatConversationTodosUpdated: null,
    chatConversationPinUpdated: null,
    chatConversationRuntimeStateUpdated: null,
    chatConversationOverviewUpdated: null,
  };
  const currentChatConversationId = ref("");
  const currentChatPreferredApiConfigId = ref("");
  const personas = ref<PersonaProfile[]>([]);
  const assistantAgentId = ref("default-agent");
  const personaEditorId = ref("default-agent");
  const userAlias = ref(options.t("archives.roleUser"));
  const selectedResponseStyleId = ref("concise");
  const selectedPdfReadMode = ref<"text" | "image">("image");
  const backgroundVoiceScreenshotKeywords = ref("");
  const backgroundVoiceScreenshotMode = ref<"desktop" | "focused_window">("focused_window");
  const instructionPresets = ref<PromptCommandPreset[]>([]);
  const conversationForegroundSyncing = ref(false);
  const lastOverviewSyncAt = ref("");
  const backgroundConversationBadgeMap = ref<Record<string, BackgroundConversationBadgeState>>({});
  const conversationMessageCache = ref<Record<string, ChatMessage[]>>({});
  const latestUserText = ref("");
  const latestUserImages = ref<Array<{ mime: string; bytesBase64: string }>>([]);
  const latestOwnMessageAlignRequest = ref(0);
  const latestContextUsagePreview = ref<ContextUsageUpdatePayload | null>(null);
  const clipboardImages = ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>([]);
  const queuedAttachmentNotices = ref<Array<{ id: string; fileName: string; path: string; mime: string }>>([]);
  const allMessages = shallowRef<ChatMessage[]>([]);
  const status = ref("Ready.");
  const terminalApprovalQueue = ref<TerminalApprovalRequestPayload[]>([]);
  const terminalApprovalResolving = ref(false);
  const loading = ref(false);
  const saving = ref(false);
  const startupDataReady = ref(false);
  const startupOverlayVisible = ref(true);
  const chatting = ref(false);
  const trimming = ref(false);
  const compactingConversation = ref(false);
  const trimmingConversationId = ref("");
  const compactingConversationId = ref("");
  const suppressNextCompactionReload = ref(false);
  const branchingConversation = ref(false);
  const forwardingConversationSelection = ref(false);
  const hasMoreBackendHistory = ref(false);
  const loadingOlderConversationHistory = ref(false);
  const refreshingModels = ref(false);
  const modelRefreshError = ref("");
  const modelRefreshOkFlags = ref<Record<string, boolean>>({});
  const checkingToolsStatus = ref(false);
  const toolStatuses = ref<ToolLoadStatus[]>([]);
  const avatarSaving = ref(false);
  const avatarError = ref("");
  const personaSaving = ref(false);
  const apiModelOptions = ref<Record<string, string[]>>({});
  const suppressAutosave = ref(false);
  const lastSavedConfigJson = ref("");
  const lastSavedPersonasJson = ref("");
  const PERF_DEBUG = import.meta.env.DEV;
  const CHAT_STREAM_DEBUG = typeof window !== "undefined"
    && window.localStorage.getItem("easy-call.debug.chat-stream") === "1";
  const toolReviewRefreshTick = ref(0);
  const currentChatTodos = ref<ChatTodoItem[]>([]);
  const foregroundTailLatestReady = ref(true);

  return {
    BACKGROUND_CONVERSATION_CACHE_LIMIT,
    FOREGROUND_SNAPSHOT_RECENT_LIMIT,
    OLDER_HISTORY_PAGE_SIZE,
    viewMode,
    config,
    recordHotkeyProbeLastSeq,
    recordHotkeyProbeDown,
    chatWindowActiveSynced,
    chatWindowEventUnlisteners,
    currentChatConversationId,
    currentChatPreferredApiConfigId,
    personas,
    assistantAgentId,
    personaEditorId,
    userAlias,
    selectedResponseStyleId,
    selectedPdfReadMode,
    backgroundVoiceScreenshotKeywords,
    backgroundVoiceScreenshotMode,
    instructionPresets,
    conversationForegroundSyncing,
    lastOverviewSyncAt,
    backgroundConversationBadgeMap,
    conversationMessageCache,
    latestUserText,
    latestUserImages,
    latestOwnMessageAlignRequest,
    latestContextUsagePreview,
    clipboardImages,
    queuedAttachmentNotices,
    allMessages,
    status,
    terminalApprovalQueue,
    terminalApprovalResolving,
    loading,
    saving,
    startupDataReady,
    startupOverlayVisible,
    chatting,
    trimming,
    compactingConversation,
    trimmingConversationId,
    compactingConversationId,
    suppressNextCompactionReload,
    branchingConversation,
    forwardingConversationSelection,
    hasMoreBackendHistory,
    loadingOlderConversationHistory,
    refreshingModels,
    modelRefreshError,
    modelRefreshOkFlags,
    checkingToolsStatus,
    toolStatuses,
    avatarSaving,
    avatarError,
    personaSaving,
    apiModelOptions,
    suppressAutosave,
    lastSavedConfigJson,
    lastSavedPersonasJson,
    PERF_DEBUG,
    CHAT_STREAM_DEBUG,
    toolReviewRefreshTick,
    currentChatTodos,
    foregroundTailLatestReady,
  };
}
