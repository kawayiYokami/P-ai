export type ApiToolItem = {
  id: string;
  command: string;
  args: string[];
  enabled: boolean;
  values: Record<string, unknown>;
};

export type ApiConfigItem = {
  id: string;
  name: string;
  requestFormat: string;
  enableText: boolean;
  enableImage: boolean;
  enableAudio: boolean;
  enableTools: boolean;
  tools: ApiToolItem[];
  baseUrl: string;
  apiKey: string;
  model: string;
  temperature: number;
  contextWindowTokens: number;
};

export type AppConfig = {
  hotkey: string;
  uiLanguage: "zh-CN" | "en-US" | "ja-JP" | "ko-KR";
  recordHotkey: string;
  minRecordSeconds: number;
  maxRecordSeconds: number;
  toolMaxIterations: number;
  selectedApiConfigId: string;
  chatApiConfigId: string;
  visionApiConfigId?: string;
  apiConfigs: ApiConfigItem[];
};

export type PersonaProfile = {
  id: string;
  name: string;
  systemPrompt: string;
  createdAt: string;
  updatedAt: string;
  avatarPath?: string;
  avatarUpdatedAt?: string;
  isBuiltInUser?: boolean;
};

export type MessagePart =
  | { type: "text"; text: string }
  | { type: "image"; mime: string; bytesBase64: string }
  | { type: "audio"; mime: string; bytesBase64: string };

export type ChatRole = "user" | "assistant" | "tool" | "system";

export type ToolCallFunction = {
  name: string;
  arguments?: string;
};

export type ToolCallItem = {
  function?: ToolCallFunction;
};

export type ToolCallMessage = {
  role: "assistant" | "tool";
  tool_calls?: ToolCallItem[];
};

export type ChatMessage = {
  id: string;
  role: ChatRole;
  createdAt?: string;
  parts: MessagePart[];
  extraTextBlocks?: string[];
  providerMeta?: {
    reasoningStandard?: string;
    reasoningInline?: string;
    [key: string]: unknown;
  };
  toolCall?: ToolCallMessage[];
};

export type ChatSnapshot = {
  conversationId: string;
  latestUser?: ChatMessage;
  latestAssistant?: ChatMessage;
  activeMessageCount: number;
};

export type ChatTurn = {
  id: string;
  userText: string;
  userImages: Array<{ mime: string; bytesBase64: string }>;
  userAudios: Array<{ mime: string; bytesBase64: string }>;
  assistantText: string;
  assistantReasoningStandard: string;
  assistantReasoningInline: string;
};

export type ArchiveSummary = {
  archiveId: string;
  archivedAt: string;
  title: string;
  messageCount?: number;
};

export type ResponseStyleOption = {
  id: string;
  name: string;
  prompt: string;
};

export type ChatSettings = { selectedPersonaId: string; userAlias: string; responseStyleId: string };

export type ToolLoadStatus = {
  id: string;
  status: "loaded" | "failed" | "timeout" | "disabled";
  detail: string;
};

export type ImageTextCacheStats = {
  entries: number;
  totalChars: number;
  latestUpdatedAt?: string;
};
