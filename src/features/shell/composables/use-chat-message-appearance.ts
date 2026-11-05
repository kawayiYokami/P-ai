import { ref } from "vue";
import { emitTransportEvent, onTransportNotification } from "../../../services/tauri-api";

const CHAT_BUBBLE_BACKGROUND_STORAGE_KEY = "easy-call.chat.bubble-background.v1";
const CHAT_PROCESS_MESSAGES_FOLDED_STORAGE_KEY = "easy-call.chat.process-messages-folded.v1";
const CHAT_SEGMENTED_MARKDOWN_STORAGE_KEY = "easy-call.chat.segmented-markdown.v1";
const CHAT_TIME_DISPLAY_MODE_STORAGE_KEY = "easy-call.chat.time-display-mode.v1";
const CHAT_MARKDOWN_LAYOUT_STORAGE_KEY = "easy-call.chat.markdown-layout.v1";
const CHAT_MESSAGE_FONT_SIZE_STORAGE_KEY = "easy-call.chat.message-font-size.v1";

/** 聊天气泡正文字号档位。与全局「UI 界面尺寸」是两个维度：这里只改消息正文。 */
export type ChatMessageFontSize = "compact" | "default" | "large" | "xlarge";

const CHAT_MESSAGE_FONT_SIZE_PX: Record<ChatMessageFontSize, string> = {
  compact: "13px",
  // default 不写死 px：移除覆盖，回到 --app-text-sm-size，跟随全局界面尺寸
  default: "",
  large: "16px",
  xlarge: "18px",
};

function normalizeChatMessageFontSize(value: unknown): ChatMessageFontSize {
  return value === "compact" || value === "large" || value === "xlarge" ? value : "default";
}

function readChatMessageFontSizePreference(): ChatMessageFontSize {
  if (typeof window === "undefined") return "default";
  return normalizeChatMessageFontSize(window.localStorage.getItem(CHAT_MESSAGE_FONT_SIZE_STORAGE_KEY));
}

function persistChatMessageFontSize(size: ChatMessageFontSize) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(CHAT_MESSAGE_FONT_SIZE_STORAGE_KEY, size);
}

/**
 * 应用到根元素：只覆盖 --app-chat-message-text-size，不动字号 token 本身。
 * default 档必须移除覆盖而不是写回原值——style.css 里它默认等于 --app-text-sm-size，
 * 移除后才会跟随全局界面尺寸档位。
 */
function applyChatMessageFontSize(size: ChatMessageFontSize) {
  if (typeof document === "undefined") return;
  const root = document.documentElement.style;
  const px = CHAT_MESSAGE_FONT_SIZE_PX[size];
  if (px) {
    root.setProperty("--app-chat-message-text-size", px);
  } else {
    root.removeProperty("--app-chat-message-text-size");
  }
}

type ChatTimeDisplayMode = "relative" | "absolute";

/** 对话 Markdown 布局档位：紧凑（极简）/ 适中（知乎）/ 宽松（Tailwind Prose） */
export type ChatMarkdownLayout = "compact" | "comfortable" | "relaxed";

const CHAT_MARKDOWN_LAYOUT_CLASSES: ChatMarkdownLayout[] = ["compact", "comfortable", "relaxed"];

type ChatMessageAppearancePayload = {
  assistantBubbleBackgroundEnabled?: boolean;
  processMessagesFolded?: boolean;
  segmentedMarkdownEnabled?: boolean;
  chatTimeDisplayMode?: ChatTimeDisplayMode;
  markdownLayout?: ChatMarkdownLayout;
  messageFontSize?: ChatMessageFontSize;
};

const assistantBubbleBackgroundEnabled = ref(readBooleanPreferenceDefault(CHAT_BUBBLE_BACKGROUND_STORAGE_KEY, true));
const processMessagesFolded = ref(readBooleanPreferenceDefault(CHAT_PROCESS_MESSAGES_FOLDED_STORAGE_KEY, true));
/** 分段入口已隐藏，运行时固定开启；关闭路径仍保留在渲染层 */
const segmentedMarkdownEnabled = ref(true);
const chatTimeDisplayMode = ref<ChatTimeDisplayMode>(readChatTimeDisplayModePreference());
const markdownLayout = ref<ChatMarkdownLayout>(readMarkdownLayoutPreference());
const messageFontSize = ref<ChatMessageFontSize>(readChatMessageFontSizePreference());
let initialized = false;
let eventUnlisten: (() => void) | null = null;

function readBooleanPreference(storageKey: string): boolean {
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem(storageKey) === "1";
}

/** 读布尔偏好：无存储记录时返回默认值（区别于显式存 "0"） */
function readBooleanPreferenceDefault(storageKey: string, defaultValue: boolean): boolean {
  if (typeof window === "undefined") return defaultValue;
  const stored = window.localStorage.getItem(storageKey);
  return stored === null ? defaultValue : stored === "1";
}

function persistBooleanPreference(storageKey: string, enabled: boolean) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(storageKey, enabled ? "1" : "0");
}

function readChatTimeDisplayModePreference(): ChatTimeDisplayMode {
  if (typeof window === "undefined") return "relative";
  return window.localStorage.getItem(CHAT_TIME_DISPLAY_MODE_STORAGE_KEY) === "absolute" ? "absolute" : "relative";
}

function persistChatTimeDisplayMode(mode: ChatTimeDisplayMode) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(CHAT_TIME_DISPLAY_MODE_STORAGE_KEY, mode);
}

function readMarkdownLayoutPreference(): ChatMarkdownLayout {
  if (typeof window === "undefined") return "comfortable";
  const stored = window.localStorage.getItem(CHAT_MARKDOWN_LAYOUT_STORAGE_KEY);
  return stored === "compact" || stored === "comfortable" || stored === "relaxed" ? stored : "comfortable";
}

function persistMarkdownLayout(layout: ChatMarkdownLayout) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(CHAT_MARKDOWN_LAYOUT_STORAGE_KEY, layout);
}

/** 布局档位通过根元素 class 生效，CSS 覆盖规则以 `html.ecall-md-layout-*` 为前缀 */
function applyMarkdownLayoutClass(layout: ChatMarkdownLayout) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  for (const candidate of CHAT_MARKDOWN_LAYOUT_CLASSES) {
    root.classList.toggle(`ecall-md-layout-${candidate}`, candidate === layout);
  }
}

function applyPayload(payload: ChatMessageAppearancePayload | undefined) {
  if (typeof payload?.assistantBubbleBackgroundEnabled === "boolean") {
    assistantBubbleBackgroundEnabled.value = payload.assistantBubbleBackgroundEnabled;
  }
  if (typeof payload?.processMessagesFolded === "boolean") {
    processMessagesFolded.value = payload.processMessagesFolded;
  }
  // 忽略外部传入的关闭值，避免旧窗口把分段关掉
  segmentedMarkdownEnabled.value = true;
  if (payload?.chatTimeDisplayMode === "absolute" || payload?.chatTimeDisplayMode === "relative") {
    chatTimeDisplayMode.value = payload.chatTimeDisplayMode;
  }
  if (payload?.markdownLayout === "compact" || payload?.markdownLayout === "comfortable" || payload?.markdownLayout === "relaxed") {
    markdownLayout.value = payload.markdownLayout;
    applyMarkdownLayoutClass(payload.markdownLayout);
  }
  if (payload?.messageFontSize) {
    const next = normalizeChatMessageFontSize(payload.messageFontSize);
    messageFontSize.value = next;
    applyChatMessageFontSize(next);
  }
}

function restoreFromStorage() {
  assistantBubbleBackgroundEnabled.value = readBooleanPreferenceDefault(CHAT_BUBBLE_BACKGROUND_STORAGE_KEY, true);
  processMessagesFolded.value = readBooleanPreferenceDefault(CHAT_PROCESS_MESSAGES_FOLDED_STORAGE_KEY, true);
  segmentedMarkdownEnabled.value = true;
  persistBooleanPreference(CHAT_SEGMENTED_MARKDOWN_STORAGE_KEY, true);
  chatTimeDisplayMode.value = readChatTimeDisplayModePreference();
  markdownLayout.value = readMarkdownLayoutPreference();
  applyMarkdownLayoutClass(markdownLayout.value);
  messageFontSize.value = readChatMessageFontSizePreference();
  applyChatMessageFontSize(messageFontSize.value);
}

function handleStorageEvent(event: StorageEvent) {
  if (
    event.key !== CHAT_BUBBLE_BACKGROUND_STORAGE_KEY
    && event.key !== CHAT_PROCESS_MESSAGES_FOLDED_STORAGE_KEY
    && event.key !== CHAT_SEGMENTED_MARKDOWN_STORAGE_KEY
    && event.key !== CHAT_TIME_DISPLAY_MODE_STORAGE_KEY
    && event.key !== CHAT_MARKDOWN_LAYOUT_STORAGE_KEY
    && event.key !== CHAT_MESSAGE_FONT_SIZE_STORAGE_KEY
  ) return;
  restoreFromStorage();
}

export function initChatMessageAppearance() {
  if (initialized) return;
  initialized = true;
  restoreFromStorage();
  if (typeof window !== "undefined") {
    window.addEventListener("storage", handleStorageEvent);
  }
  eventUnlisten = onTransportNotification<ChatMessageAppearancePayload>("chatMessageAppearance.changed", (payload) => {
    applyPayload(payload);
  });
}

function emitAppearanceChanged() {
  void emitTransportEvent("chatMessageAppearance.changed", {
    assistantBubbleBackgroundEnabled: assistantBubbleBackgroundEnabled.value,
    processMessagesFolded: processMessagesFolded.value,
    segmentedMarkdownEnabled: segmentedMarkdownEnabled.value,
    chatTimeDisplayMode: chatTimeDisplayMode.value,
    markdownLayout: markdownLayout.value,
    messageFontSize: messageFontSize.value,
  } satisfies ChatMessageAppearancePayload).catch((error) => {
    console.warn("[聊天外观] 同步消息外观变化失败", error);
  });
}

export function useChatMessageAppearance() {
  initChatMessageAppearance();

  function setAssistantBubbleBackgroundEnabled(enabled: boolean) {
    assistantBubbleBackgroundEnabled.value = enabled;
    persistBooleanPreference(CHAT_BUBBLE_BACKGROUND_STORAGE_KEY, enabled);
    emitAppearanceChanged();
  }

  function setProcessMessagesFolded(enabled: boolean) {
    processMessagesFolded.value = enabled;
    persistBooleanPreference(CHAT_PROCESS_MESSAGES_FOLDED_STORAGE_KEY, enabled);
    emitAppearanceChanged();
  }

  function setSegmentedMarkdownEnabled(_enabled: boolean) {
    segmentedMarkdownEnabled.value = true;
    persistBooleanPreference(CHAT_SEGMENTED_MARKDOWN_STORAGE_KEY, true);
    emitAppearanceChanged();
  }

  function setChatTimeDisplayMode(mode: ChatTimeDisplayMode) {
    chatTimeDisplayMode.value = mode;
    persistChatTimeDisplayMode(mode);
    emitAppearanceChanged();
  }

  function setChatMarkdownLayout(layout: ChatMarkdownLayout) {
    markdownLayout.value = layout;
    persistMarkdownLayout(layout);
    applyMarkdownLayoutClass(layout);
    emitAppearanceChanged();
  }

  function setChatMessageFontSize(size: ChatMessageFontSize) {
    const next = normalizeChatMessageFontSize(size);
    messageFontSize.value = next;
    persistChatMessageFontSize(next);
    applyChatMessageFontSize(next);
    emitAppearanceChanged();
  }

  return {
    assistantBubbleBackgroundEnabled,
    processMessagesFolded,
    segmentedMarkdownEnabled,
    chatTimeDisplayMode,
    markdownLayout,
    messageFontSize,
    setAssistantBubbleBackgroundEnabled,
    setProcessMessagesFolded,
    setSegmentedMarkdownEnabled,
    setChatTimeDisplayMode,
    setChatMarkdownLayout,
    setChatMessageFontSize,
  };
}
