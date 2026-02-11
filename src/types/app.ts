export type ApiToolItem = {
  id: string;
  command: string;
  args: string[];
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
};

export type AppConfig = {
  hotkey: string;
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
};

export type MessagePart =
  | { type: "text"; text: string }
  | { type: "image"; mime: string; bytesBase64: string }
  | { type: "audio"; mime: string; bytesBase64: string };

export type ChatMessage = { id: string; role: string; parts: MessagePart[] };

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
};

export type ArchiveSummary = { archiveId: string; archivedAt: string; title: string };

export type ChatSettings = { selectedPersonaId: string; userAlias: string };

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
