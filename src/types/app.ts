export type ApiConfigItem = {
  id: string;
  name: string;
  requestFormat: string;
  enableText: boolean;
  enableImage: boolean;
  enableAudio: boolean;
  baseUrl: string;
  apiKey: string;
  model: string;
};

export type AppConfig = {
  hotkey: string;
  selectedApiConfigId: string;
  apiConfigs: ApiConfigItem[];
};

export type AgentProfile = {
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

export type ArchiveSummary = { archiveId: string; archivedAt: string; title: string };

export type ChatSettings = { selectedAgentId: string; userAlias: string };
