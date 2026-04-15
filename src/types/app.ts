export type ApiRequestFormat =
  | "openai"
  | "openai_responses"
  | "codex"
  | "openai_tts"
  | "openai_stt"
  | "openai_embedding"
  | "openai_rerank"
  | "gemini"
  | "gemini_embedding"
  | "anthropic";

export type CodexAuthMode = "read_local" | "managed_oauth";

export type CodexAuthStatus = {
  providerId: string;
  authMode: CodexAuthMode;
  authenticated: boolean;
  status: string;
  message: string;
  email: string;
  accountId: string;
  accessTokenPreview: string;
  localAuthPath: string;
  managedAuthPath: string;
  expiresAt: string;
};

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
  codexAuthMode?: CodexAuthMode;
  codexLocalAuthPath?: string;
  model: string;
  reasoningEffort?: string;
  temperature: number;
  customTemperatureEnabled?: boolean;
  contextWindowTokens: number;
  customMaxOutputTokensEnabled?: boolean;
  maxOutputTokens?: number;
};

export type ApiModelConfigItem = {
  id: string;
  model: string;
  enableImage: boolean;
  enableTools: boolean;
  reasoningEffort?: string;
  temperature: number;
  customTemperatureEnabled?: boolean;
  contextWindowTokens: number;
  customMaxOutputTokensEnabled?: boolean;
  maxOutputTokens?: number;
};

export type ApiProviderConfigItem = {
  id: string;
  name: string;
  requestFormat: ApiRequestFormat;
  enableText: boolean;
  enableImage: boolean;
  enableAudio: boolean;
  enableTools: boolean;
  tools: ApiToolItem[];
  baseUrl: string;
  codexAuthMode?: CodexAuthMode;
  codexLocalAuthPath?: string;
  apiKeys: string[];
  keyCursor?: number;
  cachedModelOptions: string[];
  models: ApiModelConfigItem[];
  failureRetryCount?: number;
};

export type ShellWorkspaceLevel = "system" | "main" | "secondary";

export type ShellWorkspaceAccess = "approval" | "full_access" | "read_only";

export type ShellWorkspace = {
  id: string;
  name: string;
  path: string;
  level: ShellWorkspaceLevel;
  access: ShellWorkspaceAccess;
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
  selectedApiConfigId: string;
  // Active chat LLM provider config id (kept as legacy key name for storage compatibility).
  assistantDepartmentApiConfigId: string;
  visionApiConfigId?: string;
  sttApiConfigId?: string;
  sttAutoSend?: boolean;
  terminalShellKind?: string;
  shellWorkspaces: ShellWorkspace[];
  mcpServers: McpServerConfig[];
  remoteImChannels: RemoteImChannelConfig[];
  departments: DepartmentConfig[];
  apiProviders: ApiProviderConfigItem[];
  apiConfigs: ApiConfigItem[];
};

export type RemoteImPlatform = "feishu" | "dingtalk" | "onebot_v11" | "weixin_oc";

export type RemoteImChannelConfig = {
  id: string;
  name: string;
  platform: RemoteImPlatform;
  enabled: boolean;
  credentials: Record<string, unknown>;
  activateAssistant: boolean;
  receiveFiles: boolean;
  streamingSend: boolean;
  showToolCalls: boolean;
  allowSendFiles: boolean;
};

export type RemoteImContact = {
  id: string;
  channelId: string;
  platform: RemoteImPlatform;
  remoteContactType: string;
  remoteContactId: string;
  remoteContactName: string;
  remarkName: string;
  allowSend: boolean;
  allowSendFiles: boolean;
  allowReceive: boolean;
  activationMode: "always" | "never" | "keyword";
  activationKeywords: string[];
  activationCooldownSeconds: number;
  routeMode?: "main_session" | "dedicated_contact_conversation";
  boundDepartmentId?: string;
  boundConversationId?: string;
  processingMode?: "qa" | "continuous";
  lastActivatedAt?: string;
  lastMessageAt?: string;
  dingtalkSessionWebhook?: string;
  dingtalkSessionWebhookExpiredTime?: number;
};

export type RemoteImContactConversationSummary = {
  contactId: string;
  conversationId: string;
  title: string;
  updatedAt: string;
  lastMessageAt?: string;
  messageCount: number;
  channelId: string;
  platform: RemoteImPlatform;
  contactDisplayName: string;
  boundDepartmentId?: string;
  processingMode: "qa" | "continuous";
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

export type LlmRoundLogStage = {
  stage: string;
  elapsedMs: number;
  sincePrevMs: number;
};

export type LlmRoundLogEntry = {
  id: string;
  createdAt: string;
  traceId?: string;
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
  timeline?: LlmRoundLogStage[];
  success: boolean;
};

export type RuntimeLogEntry = {
  id: string;
  createdAt: string;
  level: string;
  message: string;
  repeat: number;
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
  | { type: "image"; mime: string; bytesBase64: string; name?: string; compressed?: boolean }
  | { type: "audio"; mime: string; bytesBase64: string };

export type ChatRole = "user" | "assistant" | "tool" | "system";

export type ToolCallFunction = {
  name: string;
  arguments?: unknown;
};

export type ToolCallItem = {
  id?: string;
  type?: string;
  call_id?: string;
  function?: ToolCallFunction;
};

export type ToolCallMessage = {
  role: "assistant" | "tool";
  content?: string | null;
  reasoning_content?: string;
  tool_call_id?: string;
  tool_calls?: ToolCallItem[];
};

export type TaskTriggerMessageCard = {
  taskId?: string;
  goal: string;
  why?: string;
  todo?: string;
  runAtLocal?: string;
  endAtLocal?: string;
  nextRunAtLocal?: string;
  everyMinutes?: number;
};

export type PlanMessageCard = {
  action: "present" | "complete";
  context: string;
};

export type ChatTodoItem = {
  content: string;
  status: "pending" | "in_progress" | "completed";
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
    attachments?: Array<{ fileName: string; relativePath: string; mime?: string }>;
    taskTrigger?: TaskTriggerMessageCard;
    planCard?: PlanMessageCard;
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
  sourceMessageId?: string;
  isExtraTextBlock?: boolean;
  role: ChatRole;
  isStreaming?: boolean;
  streamSegments?: string[];
  streamTail?: string;
  streamAnimatedDelta?: string;
  speakerAgentId?: string;
  createdAt?: string;
  providerMeta?: ChatMessage["providerMeta"];
  text: string;
  images: Array<{ mime: string; bytesBase64?: string; mediaRef?: string }>;
  audios: Array<{ mime: string; bytesBase64: string }>;
  attachmentFiles: Array<{ fileName: string; relativePath: string }>;
  taskTrigger?: TaskTriggerMessageCard;
  planCard?: PlanMessageCard;
  remoteImOrigin?: {
    senderName: string;
    remoteContactName?: string;
    remoteContactType: string;
    channelId: string;
    contactId: string;
  };
  reasoningStandard: string;
  reasoningInline: string;
  toolCallCount: number;
  lastToolName: string;
  toolCalls: Array<{ name: string; argsText: string; status?: "doing" | "done" }>;
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
  unreadCount: number;
  agentId: string;
  departmentId: string;
  departmentName: string;
  apiConfigId?: string;
  workspaceLabel?: string;
  isActive?: boolean;
  isMainConversation?: boolean;
  runtimeState?: "idle" | "assistant_streaming" | "organizing_context";
  currentTodo?: string;
  planModeEnabled?: boolean;
  currentTodos?: ChatTodoItem[];
  previewMessages?: ConversationPreviewMessage[];
};

export type ConversationPreviewMessage = {
  messageId: string;
  role: ChatRole;
  speakerAgentId?: string;
  createdAt?: string;
  textPreview?: string;
  hasImage?: boolean;
  hasPdf?: boolean;
  hasAudio?: boolean;
  hasAttachment?: boolean;
};

export type ChatConversationOverviewItem = {
  conversationId: string;
  title: string;
  messageCount: number;
  unreadCount?: number;
  agentId?: string;
  departmentId?: string;
  departmentName?: string;
  updatedAt?: string;
  workspaceLabel?: string;
  isActive?: boolean;
  isMainConversation?: boolean;
  runtimeState?: "idle" | "assistant_streaming" | "organizing_context";
  currentTodo?: string;
  currentTodos?: ChatTodoItem[];
  color?: string;
  canCreateNew?: boolean;
  backgroundStatus?: "completed" | "failed";
  previewMessages?: ConversationPreviewMessage[];
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

export type PdfReadMode = "text" | "image";

export type PromptCommandPreset = {
  id: string;
  name: string;
  prompt: string;
};

export type ChatSettings = {
  assistantDepartmentAgentId: string;
  userAlias: string;
  responseStyleId: string;
  pdfReadMode: PdfReadMode;
  backgroundVoiceScreenshotKeywords: string;
  backgroundVoiceScreenshotMode: "desktop" | "focused_window";
  instructionPresets: PromptCommandPreset[];
};

export type ChatSettingsPatch = {
  assistantDepartmentAgentId?: string;
  userAlias?: string;
  responseStyleId?: string;
  pdfReadMode?: PdfReadMode;
  backgroundVoiceScreenshotKeywords?: string;
  backgroundVoiceScreenshotMode?: "desktop" | "focused_window";
  instructionPresets?: PromptCommandPreset[];
};

export type ConversationApiSettings = {
  assistantDepartmentApiConfigId: string;
  visionApiConfigId?: string;
  sttApiConfigId?: string;
  sttAutoSend?: boolean;
};

export type ConversationApiSettingsPatch = {
  assistantDepartmentApiConfigId?: string;
  visionApiConfigId?: string | null;
  sttApiConfigId?: string | null;
  sttAutoSend?: boolean;
};

export type AppBootstrapSnapshot = {
  config: AppConfig;
  agents: PersonaProfile[];
  chatSettings: ChatSettings;
};

export type ToolLoadStatus = {
  id: string;
  status: "loaded" | "failed" | "timeout" | "disabled" | "unavailable";
  detail: string;
};

export type FrontendToolFunctionDefinition = {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
};

export type FrontendToolDefinition = {
  type: string;
  function: FrontendToolFunctionDefinition;
};

export type ImageTextCacheStats = {
  entries: number;
  totalChars: number;
  latestUpdatedAt?: string;
};
