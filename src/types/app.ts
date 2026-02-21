export type ApiRequestFormat =
  | "openai"
  | "openai_tts"
  | "openai_stt"
  | "openai_embedding"
  | "openai_rerank"
  | "gemini"
  | "gemini_embedding"
  | "deepseek/kimi"
  | "anthropic";

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
  requestFormat: ApiRequestFormat;
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
  // Active chat LLM provider config id (kept as legacy key name for storage compatibility).
  chatApiConfigId: string;
  visionApiConfigId?: string;
  sttApiConfigId?: string;
  sttAutoSend?: boolean;
  terminalProjectRoots: string[];
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
  assistantToolCallCount: number;
  assistantLastToolName: string;
};

export type ArchiveSummary = {
  archiveId: string;
  archivedAt: string;
  title: string;
  messageCount?: number;
};

export type UnarchivedConversationSummary = {
  conversationId: string;
  title: string;
  updatedAt: string;
  lastMessageAt?: string;
  messageCount: number;
  agentId: string;
  apiConfigId: string;
};

export type ResponseStyleOption = {
  id: string;
  name: string;
  prompt: string;
};

export type ChatSettings = { selectedPersonaId: string; userAlias: string; responseStyleId: string };

export type ToolLoadStatus = {
  id: string;
  status: "loaded" | "failed" | "timeout" | "disabled" | "unavailable";
  detail: string;
};

export type ImageTextCacheStats = {
  entries: number;
  totalChars: number;
  latestUpdatedAt?: string;
};
