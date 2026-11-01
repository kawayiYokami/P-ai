export type ChatLeftPanelMode = "local" | "contact" | "task";
export type ChatRightPanelMode = "home" | "reader" | "monitor" | "sideChat" | "teamMember" | "kanban";
export type ChatMonitorPanelMode = "delegate" | "tasks" | "tools" | "fastRequests";
export type ChatSidePanelSide = "left" | "right";
export type ChatSidePanelWidths = { leftWidth: number; rightWidth: number };

const CHAT_CONVERSATION_LIST_TAB_STORAGE_KEY = "easy_call.chat_conversation_list_tab.v1";
const CHAT_LEFT_PANEL_MODE_STORAGE_KEY = "easy_call.chat_left_panel_mode.v1";
const CHAT_RIGHT_PANEL_MODE_STORAGE_KEY = "easy_call.chat_right_panel_mode.v1";
const CHAT_RIGHT_PANEL_MODE_BY_CONVERSATION_STORAGE_PREFIX = "easy_call.chat_right_panel_mode.conversation.v1.";
const CHAT_MONITOR_PANEL_MODE_STORAGE_KEY = "easy_call.chat_monitor_panel_mode.v1";
const CHAT_MONITOR_PANEL_MODE_BY_CONVERSATION_STORAGE_PREFIX = "easy_call.chat_monitor_panel_mode.conversation.v1.";
const LEGACY_CHAT_LEFT_PANEL_MODE_STORAGE_KEY = "easy-call.chat.left-panel-mode";
const LEGACY_CHAT_RIGHT_PANEL_MODE_STORAGE_KEY = "easy-call.chat.right-panel-mode";
const CHAT_SIDE_PANEL_VISIBILITY_STORAGE_KEYS = {
  left: "easy_call.chat_left_sidebar_visible.v1",
  right: "easy_call.chat_right_sidebar_visible.v1",
} as const;
const LEGACY_CHAT_SIDE_PANEL_VISIBILITY_STORAGE_KEYS = {
  left: "easy-call.chat.left-sidebar-visible",
  right: "easy-call.chat.right-sidebar-visible",
} as const;
const CHAT_SIDE_PANEL_WIDTH_STORAGE_KEYS = {
  left: "easy_call.chat_left_sidebar_width.v1",
  right: "easy_call.chat_right_sidebar_width.v1",
} as const;
const LEGACY_CHAT_SIDE_PANEL_WIDTH_STORAGE_KEYS = {
  left: "easy-call.chat.left-sidebar-width",
  right: "easy-call.chat.right-sidebar-width",
} as const;

export function normalizeChatLeftPanelMode(value: string): ChatLeftPanelMode {
  if (value === "contact" || value === "task") return value;
  return "local";
}

export function normalizeChatRightPanelMode(value: string, fallback: ChatRightPanelMode = "home"): ChatRightPanelMode {
  if (value === "home" || value === "reader" || value === "monitor" || value === "sideChat" || value === "teamMember" || value === "kanban") return value;
  if (value === "delegate" || value === "tools" || value === "fastRequests" || value === "tasks" || value === "review") return "monitor";
  return fallback;
}

export function normalizeChatMonitorPanelMode(value: string, fallback: ChatMonitorPanelMode = "delegate"): ChatMonitorPanelMode {
  if (value === "delegate" || value === "tasks" || value === "tools" || value === "fastRequests") return value;
  // 概览已迁移到右侧主页，旧存储的 overview 与更早的 backgroundShells 一并回落到委托
  if (value === "overview" || value === "backgroundShells") return "delegate";
  if (value === "review") return "tools";
  return fallback;
}

export function normalizeChatSidePanelWidths(value: Partial<ChatSidePanelWidths> | null | undefined): ChatSidePanelWidths {
  const leftWidth = Number(value?.leftWidth);
  const rightWidth = Number(value?.rightWidth);
  return {
    leftWidth: Number.isFinite(leftWidth) ? leftWidth : 320,
    rightWidth: Number.isFinite(rightWidth) ? rightWidth : 320,
  };
}

export function loadStoredConversationListTab(): ChatLeftPanelMode {
  if (typeof window === "undefined") return "local";
  const stored = String(window.localStorage.getItem(CHAT_CONVERSATION_LIST_TAB_STORAGE_KEY) || "").trim();
  return normalizeChatLeftPanelMode(stored);
}

export function storeConversationListTab(value: ChatLeftPanelMode) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(CHAT_CONVERSATION_LIST_TAB_STORAGE_KEY, normalizeChatLeftPanelMode(value));
}

export function loadStoredChatLeftPanelMode(): ChatLeftPanelMode {
  if (typeof window === "undefined") return loadStoredConversationListTab();
  const stored = String(
    window.localStorage.getItem(CHAT_LEFT_PANEL_MODE_STORAGE_KEY)
    || window.localStorage.getItem(LEGACY_CHAT_LEFT_PANEL_MODE_STORAGE_KEY)
    || "",
  ).trim();
  return stored ? normalizeChatLeftPanelMode(stored) : loadStoredConversationListTab();
}

export function storeChatLeftPanelMode(value: ChatLeftPanelMode) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(CHAT_LEFT_PANEL_MODE_STORAGE_KEY, normalizeChatLeftPanelMode(value));
}

function chatRightPanelModeConversationStorageKey(conversationId: string) {
  const normalizedId = String(conversationId || "").trim();
  return normalizedId ? `${CHAT_RIGHT_PANEL_MODE_BY_CONVERSATION_STORAGE_PREFIX}${normalizedId}` : "";
}

function chatMonitorPanelModeConversationStorageKey(conversationId: string) {
  const normalizedId = String(conversationId || "").trim();
  return normalizedId ? `${CHAT_MONITOR_PANEL_MODE_BY_CONVERSATION_STORAGE_PREFIX}${normalizedId}` : "";
}

export function loadStoredChatRightPanelMode(fallback: ChatRightPanelMode = "home", conversationId = ""): ChatRightPanelMode {
  if (typeof window === "undefined") return fallback;
  const conversationKey = chatRightPanelModeConversationStorageKey(conversationId);
  if (conversationKey) {
    const storedForConversation = String(window.localStorage.getItem(conversationKey) || "").trim();
    return storedForConversation ? normalizeChatRightPanelMode(storedForConversation, fallback) : fallback;
  }
  const stored = String(
    window.localStorage.getItem(CHAT_RIGHT_PANEL_MODE_STORAGE_KEY)
    || window.localStorage.getItem(LEGACY_CHAT_RIGHT_PANEL_MODE_STORAGE_KEY)
    || "",
  ).trim();
  return normalizeChatRightPanelMode(stored, fallback);
}

export function storeChatRightPanelMode(value: ChatRightPanelMode, conversationId = "") {
  if (typeof window === "undefined") return;
  const normalized = normalizeChatRightPanelMode(value);
  const conversationKey = chatRightPanelModeConversationStorageKey(conversationId);
  if (conversationKey) {
    window.localStorage.setItem(conversationKey, normalized);
    return;
  }
  window.localStorage.setItem(CHAT_RIGHT_PANEL_MODE_STORAGE_KEY, normalized);
}

export function loadStoredChatMonitorPanelMode(fallback: ChatMonitorPanelMode = "delegate", conversationId = ""): ChatMonitorPanelMode {
  if (typeof window === "undefined") return fallback;
  const monitorConversationKey = chatMonitorPanelModeConversationStorageKey(conversationId);
  const legacyConversationKey = chatRightPanelModeConversationStorageKey(conversationId);
  if (monitorConversationKey) {
    const storedForConversation = String(
      window.localStorage.getItem(monitorConversationKey)
      || window.localStorage.getItem(legacyConversationKey)
      || "",
    ).trim();
    return normalizeChatMonitorPanelMode(storedForConversation, fallback);
  }
  const stored = String(
    window.localStorage.getItem(CHAT_MONITOR_PANEL_MODE_STORAGE_KEY)
    || window.localStorage.getItem(CHAT_RIGHT_PANEL_MODE_STORAGE_KEY)
    || window.localStorage.getItem(LEGACY_CHAT_RIGHT_PANEL_MODE_STORAGE_KEY)
    || "",
  ).trim();
  return normalizeChatMonitorPanelMode(stored, fallback);
}

export function storeChatMonitorPanelMode(value: ChatMonitorPanelMode, conversationId = "") {
  if (typeof window === "undefined") return;
  const normalized = normalizeChatMonitorPanelMode(value);
  const conversationKey = chatMonitorPanelModeConversationStorageKey(conversationId);
  if (conversationKey) {
    window.localStorage.setItem(conversationKey, normalized);
    return;
  }
  window.localStorage.setItem(CHAT_MONITOR_PANEL_MODE_STORAGE_KEY, normalized);
}

export function loadStoredChatSidePanelVisibility(side: ChatSidePanelSide): boolean {
  if (typeof window === "undefined") return false;
  const stored = window.localStorage.getItem(CHAT_SIDE_PANEL_VISIBILITY_STORAGE_KEYS[side])
    ?? window.localStorage.getItem(LEGACY_CHAT_SIDE_PANEL_VISIBILITY_STORAGE_KEYS[side]);
  return stored === "true";
}

export function storeChatSidePanelVisibility(side: ChatSidePanelSide, visible: boolean) {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(CHAT_SIDE_PANEL_VISIBILITY_STORAGE_KEYS[side], visible ? "true" : "false");
}

export function loadStoredChatSidePanelWidths(): ChatSidePanelWidths {
  if (typeof window === "undefined") {
    return { leftWidth: 320, rightWidth: 320 };
  }
  const leftWidth = Number(
    window.localStorage.getItem(CHAT_SIDE_PANEL_WIDTH_STORAGE_KEYS.left)
    ?? window.localStorage.getItem(LEGACY_CHAT_SIDE_PANEL_WIDTH_STORAGE_KEYS.left),
  );
  const rightWidth = Number(
    window.localStorage.getItem(CHAT_SIDE_PANEL_WIDTH_STORAGE_KEYS.right)
    ?? window.localStorage.getItem(LEGACY_CHAT_SIDE_PANEL_WIDTH_STORAGE_KEYS.right),
  );
  return normalizeChatSidePanelWidths({ leftWidth, rightWidth });
}

export function storeChatSidePanelWidths(value: Partial<ChatSidePanelWidths> | null | undefined) {
  if (typeof window === "undefined") return;
  const widths = normalizeChatSidePanelWidths(value);
  window.localStorage.setItem(CHAT_SIDE_PANEL_WIDTH_STORAGE_KEYS.left, String(widths.leftWidth));
  window.localStorage.setItem(CHAT_SIDE_PANEL_WIDTH_STORAGE_KEYS.right, String(widths.rightWidth));
}
