export type ApiRequestFormat =
  | "auto"
  | "openai"
  | "deepseek"
  | "deepseek/kimi"
  | "openai_responses"
  | "codex"
  | "gemini"
  | "anthropic"
  | "fireworks"
  | "together"
  | "groq"
  | "mimo"
  | "nebius"
  | "xai"
  | "zai"
  | "bigmodel"
  | "aliyun"
  | "cohere"
  | "ollama"
  | "ollama_cloud"
  | "vertex"
  | "github_copilot"
  | "openai_tts"
  | "openai_stt"
  | "openai_embedding"
  | "openai_rerank"
  | "gemini_embedding";

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

export type CodexRateLimitWindow = {
  usedPercent: number;
  windowDurationMins?: number | null;
  resetsAt?: number | null;
};

export type CodexCreditsSnapshot = {
  hasCredits: boolean;
  unlimited: boolean;
  balance?: string | null;
};

export type CodexRateLimitSnapshot = {
  limitId: string;
  limitName: string;
  primary?: CodexRateLimitWindow | null;
  secondary?: CodexRateLimitWindow | null;
  credits?: CodexCreditsSnapshot | null;
  planType: string;
  rateLimitReachedType: string;
};

export type CodexRateLimitQueryResult = {
  usageUrl: string;
  preferredSnapshot?: CodexRateLimitSnapshot | null;
  snapshots: CodexRateLimitSnapshot[];
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
  allowConcurrentRequests?: boolean;
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
  allowConcurrentRequests?: boolean;
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
export type GithubUpdateMethod = "auto" | "direct" | "proxy";

export type ShellWorkspace = {
  id: string;
  name: string;
  path: string;
  level: ShellWorkspaceLevel;
  access: ShellWorkspaceAccess;
  builtIn?: boolean;
};

export type ChatShellWorkspaceState = {
  sessionId: string;
  workspaceName: string;
  rootPath: string;
  workspaces?: ShellWorkspace[];
  autonomousMode?: boolean;
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
  isDeputy?: boolean;
  source?: string;
  scope?: string;
  permissionControl?: DepartmentPermissionControl;
};

export type DepartmentPermissionMode = "whitelist" | "blacklist";

export type DepartmentPermissionControl = {
  enabled: boolean;
  mode: DepartmentPermissionMode;
  builtinToolNames: string[];
  skillNames: string[];
  mcpToolNames: string[];
};

export type DepartmentPermissionCatalogItem = {
  name: string;
  description: string;
};

export type DepartmentPermissionCatalog = {
  builtinTools: DepartmentPermissionCatalogItem[];
  skills: DepartmentPermissionCatalogItem[];
  mcpTools: DepartmentPermissionCatalogItem[];
};

export type AppConfig = {
  hotkey: string;
  uiLanguage: "zh-CN" | "en-US" | "zh-TW";
  uiFont: string;
  webviewZoomPercent?: number;
  githubUpdateMethod?: GithubUpdateMethod;
  recordHotkey: string;
  recordBackgroundWakeEnabled: boolean;
  minRecordSeconds: number;
  maxRecordSeconds: number;
  llmRoundLogCapacity: 1 | 3 | 10;
  selectedApiConfigId: string;
  // Active chat LLM provider config id (kept as legacy key name for storage compatibility).
  assistantDepartmentApiConfigId: string;
  visionApiConfigId?: string;
  toolReviewApiConfigId?: string;
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
  patienceSeconds: number;
  activationCooldownSeconds: number;
  routeMode?: "main_session" | "dedicated_contact_conversation";
  boundDepartmentId?: string;
  boundConversationId?: string;
  processingMode?: "qa" | "continuous";
  lastActivatedAt?: string;
  lastMessageAt?: string;
  dingtalkSessionWebhook?: string;
  dingtalkSessionWebhookExpiredTime?: number;
  shellWorkspaces?: ShellWorkspace[];
};

export type RemoteImContactConversationSummary = {
  contactId: string;
  conversationId: string;
  title: string;
  updatedAt: string;
  lastMessageAt?: string;
  messageCount: number;
  channelId: string;
  channelName?: string;
  platform: RemoteImPlatform;
  contactDisplayName: string;
  boundDepartmentId?: string;
  processingMode: "qa" | "continuous";
  previewMessages?: ConversationPreviewMessage[];
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
  parameters: Record<string, unknown>;
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
  response?: unknown;
  error?: string;
  elapsedMs: number;
  timeline?: LlmRoundLogStage[];
  roundCount?: number;
  toolCallCount?: number;
  rounds?: LlmRoundLogEntry[];
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

export type MemeMessageSegment =
  | { type: "text"; text: string }
  | {
    type: "meme";
    name: string;
    category: string;
    mime: string;
    relativePath: string;
    bytesBase64: string;
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
    dispatchElapsedMs?: number;
    messageKind?: string;
    hiddenPromptText?: string;
    attachments?: Array<{ fileName: string; relativePath: string; mime?: string }>;
    taskTrigger?: TaskTriggerMessageCard;
    planCard?: PlanMessageCard;
    memeSegments?: MemeMessageSegment[];
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
  dividerKind?: "plan_started";
  isStreaming?: boolean;
  streamSegments?: string[];
  streamTail?: string;
  streamAnimatedDelta?: string;
  speakerAgentId?: string;
  createdAt?: string;
  providerMeta?: ChatMessage["providerMeta"];
  mentions?: ChatMentionTarget[];
  text: string;
  images: Array<{ mime: string; bytesBase64?: string; mediaRef?: string }>;
  audios: Array<{ mime: string; bytesBase64: string }>;
  attachmentFiles: Array<{ fileName: string; relativePath: string }>;
  memeSegments?: MemeMessageSegment[];
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
  dispatchElapsedMs?: number;
  frontendDispatchElapsedMs?: number;
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

export type ChatMentionTarget = {
  agentId: string;
  agentName: string;
  departmentId: string;
  departmentName: string;
  avatarUrl?: string;
};

export type ArchiveSummary = {
  archiveId: string;
  archivedAt: string;
  title: string;
  messageCount?: number;
};

export type ConversationBlockSummary = {
  blockId: number;
  messageCount: number;
  firstMessageId: string;
  lastMessageId: string;
  firstCreatedAt?: string;
  lastCreatedAt?: string;
  isLatest: boolean;
};

export type ArchiveBlockPage = {
  blocks: ConversationBlockSummary[];
  selectedBlockId: number;
  messages: ChatMessage[];
  hasPrevBlock: boolean;
  hasNextBlock: boolean;
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
  parentConversationId?: string;
  forkMessageCursor?: string;
  apiConfigId?: string;
  workspaceLabel?: string;
  isActive?: boolean;
  isMainConversation?: boolean;
  isPinned?: boolean;
  pinIndex?: number;
  runtimeState?: "idle" | "assistant_streaming" | "organizing_context";
  currentTodo?: string;
  planModeEnabled?: boolean;
  currentTodos?: ChatTodoItem[];
  detachedWindowOpen?: boolean;
  detachedWindowLabel?: string;
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
  kind?: "local_unarchived" | "remote_im_contact";
  remoteContactId?: string;
  remoteContactDisplayName?: string;
  messageCount: number;
  unreadCount?: number;
  agentId?: string;
  departmentId?: string;
  departmentName?: string;
  parentConversationId?: string;
  forkMessageCursor?: string;
  updatedAt?: string;
  lastMessageAt?: string;
  workspaceLabel?: string;
  isActive?: boolean;
  isMainConversation?: boolean;
  isPinned?: boolean;
  pinIndex?: number;
  runtimeState?: "idle" | "assistant_streaming" | "organizing_context";
  currentTodo?: string;
  currentTodos?: ChatTodoItem[];
  detachedWindowOpen?: boolean;
  detachedWindowLabel?: string;
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

export type ConversationDelegateStatusSummary = {
  delegateId: string;
  conversationId: string;
  rootConversationId: string;
  title: string;
  status: string;
  startedAt: string;
  updatedAt: string;
  completedAt?: string;
  archivedAt?: string;
  elapsedMs: number;
  requestCount: number;
  toolCallCount: number;
  lastToolName: string;
  tokenCount: number;
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
  toolReviewApiConfigId?: string;
  sttApiConfigId?: string;
  sttAutoSend?: boolean;
};

export type ConversationApiSettingsPatch = {
  assistantDepartmentApiConfigId?: string;
  visionApiConfigId?: string | null;
  toolReviewApiConfigId?: string | null;
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
