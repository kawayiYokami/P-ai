export type ApiRequestFormat =
  | "openai"
  | "openai_responses"
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
  maxOutputTokens?: number;
  failureRetryCount?: number;
};

export type ShellWorkspace = {
  name: string;
  path: string;
  builtIn?: boolean;
};

export type McpToolPolicy = {
  toolName: string;
  enabled: boolean;
};

export type McpCachedTool = {
  toolName: string;
  description: string;
};

export type McpServerConfig = {
  id: string;
  name: string;
  enabled: boolean;
  definitionJson: string;
  toolPolicies: McpToolPolicy[];
  cachedTools?: McpCachedTool[];
  lastStatus?: string;
  lastError?: string;
  updatedAt?: string;
};

export type DepartmentConfig = {
  id: string;
  name: string;
  summary: string;
  guide: string;
  apiConfigId: string;
  apiConfigIds: string[];
  agentIds: string[];
  createdAt: string;
  updatedAt: string;
  orderIndex: number;
  isBuiltInAssistant?: boolean;
  source?: string;
  scope?: string;
};

export type AppConfig = {
  hotkey: string;
  uiLanguage: "zh-CN" | "en-US" | "zh-TW";
  uiFont: string;
  recordHotkey: string;
  recordBackgroundWakeEnabled: boolean;
  minRecordSeconds: number;
  maxRecordSeconds: number;
  toolMaxIterations: number;
  selectedApiConfigId: string;
  // Active chat LLM provider config id (kept as legacy key name for storage compatibility).
  assistantDepartmentApiConfigId: string;
  visionApiConfigId?: string;
  sttApiConfigId?: string;
  sttAutoSend?: boolean;
  shellWorkspaces: ShellWorkspace[];
  mcpServers: McpServerConfig[];
  departments: DepartmentConfig[];
  apiConfigs: ApiConfigItem[];
};

export type McpDefinitionValidateResult = {
  ok: boolean;
  transport?: string;
  serverName?: string;
  message: string;
  schemaVersion?: string;
  errorCode?: string;
  details?: string[];
  migratedDefinitionJson?: string;
};

export type McpToolDescriptor = {
  toolName: string;
  description: string;
  enabled: boolean;
};

export type McpListServerToolsResult = {
  serverId: string;
  tools: McpToolDescriptor[];
  elapsedMs: number;
};

export type SkillSummaryItem = {
  name: string;
  description: string;
  content: string;
  path: string;
};

export type SkillListResult = {
  skills: SkillSummaryItem[];
  errors: WorkspaceLoadError[];
};

export type WorkspaceLoadError = {
  item: string;
  error: string;
};

export type RefreshMcpAndSkillsResult = {
  mcpLoaded: string[];
  mcpFailed: WorkspaceLoadError[];
  skillsLoaded: string[];
  skillsFailed: WorkspaceLoadError[];
  skills: SkillSummaryItem[];
  skillSummary: string;
  privateAgentsLoaded: string[];
  privateAgentsFailed: WorkspaceLoadError[];
  privateDepartmentsLoaded: string[];
  privateDepartmentsFailed: WorkspaceLoadError[];
};

export type LlmRoundLogHeader = {
  name: string;
  value: string;
};

export type LlmRoundLogEntry = {
  id: string;
  createdAt: string;
  scene: string;
  requestFormat: string;
  provider: string;
  model: string;
  baseUrl: string;
  headers: LlmRoundLogHeader[];
  tools?: unknown;
  request: unknown;
  response?: unknown;
  error?: string;
  elapsedMs: number;
  success: boolean;
};

export type PersonaProfile = {
  id: string;
  name: string;
  systemPrompt: string;
  tools: ApiToolItem[];
  privateMemoryEnabled?: boolean;
  createdAt: string;
  updatedAt: string;
  avatarPath?: string;
  avatarUpdatedAt?: string;
  isBuiltInUser?: boolean;
  isBuiltInSystem?: boolean;
  source?: string;
  scope?: string;
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

export type TaskTriggerMessageCard = {
  taskId?: string;
  title: string;
  cause?: string;
  goal?: string;
  flow?: string;
  statusSummary?: string;
  todos: string[];
  runAt?: string;
  endAt?: string;
  nextRunAt?: string;
  everyMinutes?: number;
};

export type ChatMessage = {
  id: string;
  role: ChatRole;
  createdAt?: string;
  speakerAgentId?: string;
  parts: MessagePart[];
  extraTextBlocks?: string[];
  providerMeta?: {
    reasoningStandard?: string;
    reasoningInline?: string;
    messageKind?: string;
    hiddenPromptText?: string;
    taskTrigger?: TaskTriggerMessageCard;
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

export type ChatMessageBlock = {
  id: string;
  role: ChatRole;
  speakerAgentId?: string;
  createdAt?: string;
  text: string;
  images: Array<{ mime: string; bytesBase64: string }>;
  audios: Array<{ mime: string; bytesBase64: string }>;
  taskTrigger?: TaskTriggerMessageCard;
  reasoningStandard: string;
  reasoningInline: string;
  toolCallCount: number;
  lastToolName: string;
};

export type ChatPersonaPresenceChip = {
  id: string;
  name: string;
  avatarUrl: string;
  departmentName: string;
  isFrontSpeaking: boolean;
  hasBackgroundTask: boolean;
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

export type DelegateConversationSummary = {
  conversationId: string;
  title: string;
  updatedAt: string;
  lastMessageAt?: string;
  messageCount: number;
  agentId: string;
  apiConfigId: string;
  delegateId?: string;
  rootConversationId?: string;
  archivedAt?: string;
};

export type AgentWorkSignalPayload = {
  agentId: string;
  delegateId: string;
};

export type ResponseStyleOption = {
  id: string;
  name: string;
  prompt: string;
};

export type ChatSettings = { assistantDepartmentAgentId: string; userAlias: string; responseStyleId: string };

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


