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
  | "minimax"
  | "moonshot"
  | "nebius"
  | "xai"
  | "zai"
  | "bigmodel"
  | "aliyun"
  | "baidu"
  | "cohere"
  | "ollama"
  | "ollama_cloud"
  | "vertex"
  | "github_copilot"
  | "opencode_go"
  | "bedrock_api"
  | "openai_tts"
  | "openai_stt"
  | "mimo_asr"
  | "openai_embedding"
  | "openai_rerank"
  | "gemini_embedding";

export type CodexAuthMode = "read_local" | "managed_oauth" | "custom_url";

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
  rateLimitResetCreditCount: number;
};

export type CodexConsumeRateLimitResetCreditResult = { outcome: string };

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
  maxConcurrentRequests?: number | null;
  enableText: boolean;
  enableImage: boolean;
  enableAudio: boolean;
  enableVideo?: boolean;
  enableTools: boolean;
  tools: ApiToolItem[];
  baseUrl: string;
  apiKey: string;
  codexAuthMode?: CodexAuthMode;
  codexLocalAuthPath?: string;
  codexCustomUrl?: string;
  codexCustomApiKey?: string;
  codexOriginator?: string;
  codexResidencyRequirement?: string;
  model: string;
  displayName?: string;
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
  displayName?: string;
  deprecated?: boolean;
  enableImage: boolean;
  enableAudio?: boolean;
  enableVideo?: boolean;
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
  deprecated?: boolean;
  requestFormat: ApiRequestFormat;
  allowConcurrentRequests?: boolean;
  maxConcurrentRequests?: number | null;
  enableText: boolean;
  enableImage: boolean;
  enableAudio: boolean;
  enableVideo?: boolean;
  enableTools: boolean;
  tools: ApiToolItem[];
  baseUrl: string;
  codexAuthMode?: CodexAuthMode;
  codexLocalAuthPath?: string;
  codexCustomUrl?: string;
  codexCustomApiKey?: string;
  codexOriginator?: string;
  codexResidencyRequirement?: string;
  apiKeys: string[];
  keyCursor?: number;
  cachedModelOptions: string[];
  models: ApiModelConfigItem[];
  failureRetryCount?: number;
};

export type ImageGenerationProviderKind = "comfyui" | "codex" | "openai" | "xai" | "seedream" | "gemini" | "sensenova";

export type ImageGenerationModelConfigItem = {
  id: string;
  name: string;
  model: string;
  enabled: boolean;
  deprecated?: boolean;
  defaultSize?: string;
  defaultAspectRatio?: string;
  defaultQuality?: string;
};

export type ComfyUiNodeInputMapping = {
  nodeIds: string[];
  inputKey: string;
};

export type ComfyUiWorkflowMapping = {
  prompt: ComfyUiNodeInputMapping;
  negativePrompt: ComfyUiNodeInputMapping;
  model: ComfyUiNodeInputMapping;
  width: ComfyUiNodeInputMapping;
  height: ComfyUiNodeInputMapping;
  seed: ComfyUiNodeInputMapping;
  steps: ComfyUiNodeInputMapping;
  inputImage: ComfyUiNodeInputMapping;
  maskImage: ComfyUiNodeInputMapping;
  outputNodeIds: string[];
};

export type ImageGenerationProviderConfigItem = {
  id: string;
  name: string;
  providerType: ImageGenerationProviderKind;
  enabled: boolean;
  deprecated?: boolean;
  baseUrl: string;
  apiKeys: string[];
  codexApiProviderId?: string;
  keyCursor?: number;
  timeoutSeconds: number;
  watermark: boolean;
  models: ImageGenerationModelConfigItem[];
  comfyuiWorkflowJson: string;
  comfyuiMapping: ComfyUiWorkflowMapping;
};

export type ImageGenerationModelOption = {
  id: string;
  providerId: string;
  providerName: string;
  providerType: ImageGenerationProviderKind;
  modelId: string;
  model: string;
  name: string;
  label: string;
};

export type GeneratedImageAsset = {
  relativePath: string;
  remoteUrl?: string;
  markdown: string;
  mime: string;
  width: number;
  height: number;
  revisedPrompt?: string;
};

export type ImageGenerationResult = {
  providerId: string;
  providerName: string;
  providerType: ImageGenerationProviderKind;
  modelId: string;
  model: string;
  images: GeneratedImageAsset[];
  providerText?: string;
};

export type ShellWorkspaceLevel = "system" | "main" | "secondary";

export type ShellWorkspaceAccess = "approval" | "full_access";
export type ShellWorkMode = "directory" | "worktree";
/** 兼容旧数据的读写宽松类型，迁移后均归一为新枚举 */
export type LegacyShellWorkMode = ShellWorkMode | "isolated_worktree" | "independent_worktree";
export type LegacyShellWorkspaceAccess = ShellWorkspaceAccess | "read_only";
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
  shellWorkMode?: ShellWorkMode;
  shellWorkBranch?: string;
  worktreePath?: string;
  worktreeExists?: boolean;
};

export type IdeContextWorkspaceInput = {
  path: string;
  name?: string;
};

export type IdeContextReferenceItem = {
  id: string;
  workspacePath: string;
  workspaceName: string;
  filePath: string;
  fileName: string;
  relativePath: string;
  startLine?: number;
  endLine?: number;
  displayLabel: string;
  content: string;
  languageId?: string;
  source: string;
  capturedAt: string;
  textBlock: string;
};

export type IdeContextWorkspaceGroup = {
  workspacePath: string;
  workspaceName: string;
  references: IdeContextReferenceItem[];
};

export type IdeContextQueryResult = {
  groups: IdeContextWorkspaceGroup[];
  updatedAt: string;
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
  modelFailureFallbackEnabled: boolean;
  agentIds: string[];
  childDepartmentIds: string[];
  createdAt: string;
  updatedAt: string;
  orderIndex: number;
  isBuiltInAssistant?: boolean;
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
  group?: string;
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
  codeFont: string;
  uiSizeScale?: number;
  webAccessPort?: number;
  webAccessEnabled?: boolean;
  webAccessPassword?: string;
  githubUpdateMethod?: GithubUpdateMethod;
  skippedGithubUpdateVersion?: string;
  recordHotkey: string;
  recordBackgroundWakeEnabled: boolean;
  minRecordSeconds: number;
  maxRecordSeconds: number;
  llmRoundLogCapacity: 1 | 3 | 10;
  messageNotificationEnabled: boolean;
  messageNotificationSoundEnabled: boolean;
  desktopOperationNoticeEnabled: boolean;
  desktopOperateEnabled: boolean;
  selectedApiConfigId: string;
  // Active chat LLM provider config id (kept as legacy key name for storage compatibility).
  assistantDepartmentApiConfigId: string;
  visionApiConfigId?: string;
  imageGenerationModelId?: string;
  toolReviewApiConfigId?: string;
  sttApiConfigId?: string;
  sttAutoSend?: boolean;
  terminalShellKind?: string;
  simpleSetupMode?: boolean;
  shellWorkspaces: ShellWorkspace[];
  mcpServers: McpServerConfig[];
  remoteImChannels: RemoteImChannelConfig[];
  departments: DepartmentConfig[];
  apiProviders: ApiProviderConfigItem[];
  imageProviders: ImageGenerationProviderConfigItem[];
  apiConfigs: ApiConfigItem[];
};

export type RecordHotkeyUpdateResult = {
  recordHotkey: string;
  recordBackgroundWakeEnabled: boolean;
  minRecordSeconds: number;
  maxRecordSeconds: number;
};

export type RemoteImPlatform = "feishu" | "dingtalk" | "onebot_v11" | "weixin_oc";

export type RemoteImChannelConfig = {
  id: string;
  name: string;
  platform: RemoteImPlatform;
  enabled: boolean;
  credentials: Record<string, unknown>;
  receiveFiles: boolean;
  streamingSend: boolean;
  showToolCalls: boolean;
  filterMarkdown: boolean;
  allowSendFiles: boolean;
  behaviorSettings?: RemoteImChannelBehaviorSettings;
};

export type RemoteImGroupReplyPacing = {
  assistantDebounceSeconds: number;
  secretaryInspectionSeconds: number;
  replyCooldownSeconds: number;
  inspectionJitterRatio: number;
  maximumEnergy: number;
  baseReplyEnergyCost: number;
  energyCostPerCharacter: number;
  energyRecoveryPerSecond: number;
  positiveEnergyPhrases: string[];
  negativeEnergyPhrases: string[];
  positiveEnergyDelta: number;
  negativeEnergyDelta: number;
  normalReplyMaxChars: number;
  focusReplyMaxChars: number;
  focusInstructions: string[];
};

export type RemoteImChannelBehaviorSettings = {
  responseGuidance: string;
  blockedMessagePrefixes: string[];
  muteKeywords: string[];
  unmuteKeywords: string[];
  patienceSeconds: number;
  muteDurationSeconds: number;
  groupReplyPacing: RemoteImGroupReplyPacing;
};

export type RemoteImContact = {
  id: string;
  channelId: string;
  platform: RemoteImPlatform;
  remoteContactType: string;
  remoteContactId: string;
  remoteContactName: string;
  avatarUrl?: string;
  remarkName: string;
  allowSend: boolean;
  allowSendFiles: boolean;
  allowReceive: boolean;
  activationMode: "always" | "never" | "keyword";
  activationKeywords: string[];
  muteKeywords: string[];
  unmuteKeywords: string[];
  patienceSeconds: number;
  muteDurationSeconds: number;
  activationCooldownSeconds: number;
  responseStrategy?: "always_reply" | "smart_judge";
  blockedMessagePrefixes: string[];
  groupReplyPacing?: RemoteImGroupReplyPacing;
  routeMode?: "main_session" | "dedicated_contact_conversation";
  boundDepartmentId?: string;
  boundAgentId?: string;
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
  runtimeState?: "idle" | "assistant_streaming" | "organizing_context" | "archiving" | "compacting";
  channelId: string;
  channelName?: string;
  channelEnabled?: boolean;
  platform: RemoteImPlatform;
  contactDisplayName: string;
  boundDepartmentId?: string;
  boundAgentId?: string;
  processingMode: "qa" | "continuous";
  previewMessages?: ConversationPreviewMessage[];
};

export type RemoteImContactConversationOption = {
  contactId: string;
  conversationId: string;
  title: string;
  contactDisplayName: string;
  channelName?: string;
  channelEnabled?: boolean;
};

export type RemoteImContactDashboardSnapshot = {
  contactId: string;
  energy: number;
  maximumEnergy: number;
  energyPercent: number;
  energyRecoveryPerSecond: number;
  presence: "away" | "present";
  lastPresenceAt?: string;
  watermark: string;
  updatedAt: string;
};

export type RemoteImContactDashboardSyncResult = {
  snapshot: RemoteImContactDashboardSnapshot;
  changed: boolean;
};

export type McpValidationIssue = {
  code: string;
  message: string;
  serverName?: string;
  field?: string;
  params?: Record<string, string>;
};

export type McpDefinitionValidateResult = {
  ok: boolean;
  transport?: string;
  serverName?: string;
  message: string;
  schemaVersion?: string;
  errorCode?: string;
  details?: string[];
  issues?: McpValidationIssue[];
  migratedDefinitionJson?: string;
};

export type McpFixDefinitionResult = {
  ok: boolean;
  fixedDefinitionJson?: string;
  message: string;
  issues: McpValidationIssue[];
  modelName?: string;
};

export type McpToolDescriptor = {
  toolName: string;
  description: string;
  enabled: boolean;
  compatibilityError?: string;
  parameters: Record<string, unknown>;
};

export type McpListServerToolsResult = {
  serverId: string;
  tools: McpToolDescriptor[];
  elapsedMs: number;
};

export type SkillFileItem = {
  name: string;
  relativePath: string;
  sizeBytes: number;
};

export type SkillSummaryItem = {
  name: string;
  description: string;
  content: string;
  path: string;
  additionalFiles?: SkillFileItem[];
  isBuiltin?: boolean;
};

export type SkillListResult = {
  skills: SkillSummaryItem[];
  errors: WorkspaceLoadError[];
};

export type WorkspaceLoadError = {
  item: string;
  error: string;
  hint?: string;
  skipped?: boolean;
};

export type WorkspaceLoadedGroup = {
  kind: string;
  label: string;
  count: number;
  items: string[];
};

export type WorkspaceFailedGroup = {
  kind: string;
  label: string;
  count: number;
  items: WorkspaceLoadError[];
};

export type RefreshMcpAndSkillsResult = {
  ok?: boolean;
  status?: string;
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
  loadedGroups: WorkspaceLoadedGroup[];
  failedGroups: WorkspaceFailedGroup[];
  totalLoaded: number;
  totalFailed: number;
  loadedSummary: string;
  failedSummary: string;
  repairSummary?: string;
  repairItems?: WorkspaceLoadError[];
  needsRepair: boolean;
};

export type LlmRoundLogHeader = {
  name: string;
  value: string;
};

export type LlmRoundLogStage = {
  stage: string;
  elapsedMs: number;
  sincePrevMs: number;
  detail?: Record<string, unknown>;
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

export type MemoryRecallMode = "auto" | "manual" | "off";

/** 保存配置时后端归一化自动补上的内容（自修复记录），由保存结果显式回报。 */
export type ConfigRepairNotice = {
  kind: string;
  departmentId: string;
  departmentName: string;
  agentId: string;
};

export type PersonaProfile = {
  id: string;
  name: string;
  systemPrompt: string;
  tools: ApiToolItem[];
  privateMemoryEnabled?: boolean;
  memoryRecallMode?: MemoryRecallMode;
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
  | { type: "text"; text: string; reasoningContent?: string; reasoning_content?: string }
  | { type: "image"; mime: string; bytesBase64: string; name?: string; compressed?: boolean }
  | { type: "audio"; mime: string; bytesBase64: string }
  | { type: "attachment"; path: string; mime: string; name: string };

export type ChatIngressPart =
  | { type: "text"; text: string }
  | {
    type: "attachment";
    path?: string;
    bytesBase64?: string;
    mime: string;
    name: string;
  };

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
  metadata?: Record<string, unknown>;
};

export type TaskTriggerMessageCard = {
  taskId?: string;
  goal: string;
  why?: string;
  todo?: string;
  runAt?: string;
  cronExpression?: string;
  endAt?: string;
  nextRunAt?: string;
};

export type PlanMessageCard = {
  action: "present" | "complete";
  path: string;
  context?: string;
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

export type MemeAnnotation = { meme: string; path: string; };

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
  /** 助理消息的正式内容源；流式快照直接写入，完成/停止不得重建。 */
  contentBlocks?: AssistantStreamBlock[];
  extraTextBlocks?: string[];
  providerMeta?: {
    dispatchElapsedMs?: number;
    messageKind?: string;
    hiddenPromptText?: string;
    /** 旧消息只读兼容；新消息不再写入附件清单。 */
    attachments?: Array<{ fileName: string; relativePath: string; mime?: string }>;
    taskTrigger?: TaskTriggerMessageCard;
    planCard?: PlanMessageCard;
    [key: string]: unknown;
  };
  toolCall?: ToolCallMessage[];
  activityItems?: ChatActivityItem[];
  memeAnnotations?: MemeAnnotation[];
};

export type ChatActivityStatus = "idle" | "requesting" | "thinking" | "running_tool" | "complete";

export type ChatActivityItem =
  | {
    kind: "reasoning";
    id: string;
    text: string;
    running?: boolean;
  }
  | {
    kind: "content";
    id: string;
    text: string;
    running?: boolean;
  }
  | {
    kind: "tool";
    id: string;
    toolCallId?: string;
    name: string;
    argsText: string;
    resultText?: string;
    status?: "doing" | "done";
  };

export type AssistantStreamToolBlock = {
  toolCallId: string;
  name: string;
  argsText: string;
  resultText?: string;
  resultMetadata?: Record<string, unknown>;
  status?: "doing" | "done";
};

export type AssistantStreamBlock = {
  reasoning?: string;
  reasoningCharCount?: number;
  text?: string;
  tools?: AssistantStreamToolBlock[];
  pendingTextBreak?: boolean;
};

export type ChatSnapshot = {
  conversationId: string;
  latestUser?: ChatMessage;
  latestAssistant?: ChatMessage;
  activeMessageCount: number;
};

export type ChatMessageBlock = {
  id: string;
  /** 仅供开发诊断使用：生成展示块时对应的原始 ChatMessage。 */
  rawMessage?: ChatMessage;
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
  contentBlocks?: AssistantStreamBlock[];
  mentions?: ChatMentionTarget[];
  text: string;
  images: Array<{ mime: string; bytesBase64?: string; mediaRef?: string; name?: string }>;
  audios: Array<{ mime: string; bytesBase64?: string; mediaRef?: string; name?: string }>;
  attachmentFiles: Array<{ fileName: string; path: string }>;
  extraTextReferences?: Array<{ label: string; text: string }>;
  taskTrigger?: TaskTriggerMessageCard;
  planCard?: PlanMessageCard;
  remoteImOrigin?: {
    senderName: string;
    remoteContactName?: string;
    remoteContactType: string;
    channelId: string;
    contactId: string;
  };
  dispatchElapsedMs?: number;
  frontendDispatchElapsedMs?: number;
  toolCallCount: number;
  lastToolName: string;
  toolCalls: Array<{ toolCallId?: string; name: string; argsText: string; status?: "doing" | "done" }>;
  activityItems: ChatActivityItem[];
  activityReasoningCharCount: number;
  activityToolCountsByName: Record<string, number>;
  activityRunning: boolean;
  activityStatus: ChatActivityStatus;
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

export type ChatMentionEntry = {
  agentId: string;
  agentName: string;
  avatarUrl?: string;
  departmentId?: string;
  departmentName: string;
  departmentNames: string[];
  isFrontSpeaking: boolean;
  hasBackgroundTask: boolean;
  mentionable: boolean;
  /** 完全隐藏：不进入候选面板（如 @自己、用户人格），区别于灰显的 mentionable=false */
  hidden?: boolean;
  /** @deprecated 改用 selectedChatMentionKeys */
  unavailableReason?: string;
  selected?: boolean;
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

export type ConversationGoalUsage = {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
};

export type ConversationGoalState = {
  goalId: string;
  status: "active" | "complete" | "blocked" | "cancelled_by_user" | string;
  objective: string;
  startedAt: string;
  endedAt?: string | null;
  usageStart?: ConversationGoalUsage;
  usageEnd?: ConversationGoalUsage | null;
};

export type FastRequestTurn = {
  id: string;
  kind: string;
  requestText: string;
  responseText: string;
  success: boolean;
  error?: string | null;
  modelName?: string | null;
  durationMs?: number | null;
  createdAt: string;
};

export type UnarchivedConversationSummary = {
  conversationId: string;
  title: string;
  summaryTitle?: string;
  updatedAt: string;
  lastMessageAt?: string;
  messageCount: number;
  bodyMessageCount?: number;
  bodyTextLength?: number;
  hasAssistantReply?: boolean;
  unreadCount: number;
  agentId: string;
  departmentId: string;
  departmentName: string;
  conversationKind?: string;
  childConversationIds?: string[];
  childConversations?: ChildConversationSummary[];
  parentConversationId?: string;
  forkMessageCursor?: string;
  apiConfigId?: string;
  workspaceLabel?: string;
  workspaceRootPath?: string;
  isActive?: boolean;
  isSystemNotificationConversation?: boolean;
  isMainConversation?: boolean;
  isPinned?: boolean;
  isDraft?: boolean;
  pinIndex?: number;
  runtimeState?: "idle" | "assistant_streaming" | "organizing_context" | "archiving" | "compacting";
  currentTodo?: string;
  planModeEnabled?: boolean;
  autoPushRemoteContactId?: string;
  activeGoal?: ConversationGoalState | null;
  currentTodos?: ChatTodoItem[];
  lastError?: string;
  detachedWindowOpen?: boolean;
  detachedWindowLabel?: string;
  previewMessages?: ConversationPreviewMessage[];
  state?: ConversationListItemState;
};

export type ChildConversationSummary = {
  conversationId: string;
  title: string;
  status: string;
  conversationKind: string;
  parentConversationId?: string;
  updatedAt: string;
};

export type ConversationListItemState = {
  activity: "idle" | "busy" | "completed" | "failed";
  runtimeState: "idle" | "assistant_streaming" | "organizing_context";
  unreadCount: number;
  openState: "closed" | "open";
  openViewerId?: string;
  currentViewerId?: string;
  openedBy?: "main" | "detached" | "vscode";
  disabledReason?: "organizing_context";
  failedMessage?: string;
  completedAt?: string;
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
  summaryTitle?: string;
  kind?: "local_unarchived" | "remote_im_contact";
  conversationKind?: "chat" | "side_chat" | string;
  childConversationIds?: string[];
  remoteContactId?: string;
  remoteContactDisplayName?: string;
  channelId?: string;
  channelName?: string;
  messageCount: number;
  bodyMessageCount?: number;
  bodyTextLength?: number;
  hasAssistantReply?: boolean;
  unreadCount?: number;
  agentId?: string;
  departmentId?: string;
  departmentName?: string;
  parentConversationId?: string;
  forkMessageCursor?: string;
  updatedAt?: string;
  lastMessageAt?: string;
  workspaceLabel?: string;
  workspaceRootPath?: string;
  isActive?: boolean;
  isSystemNotificationConversation?: boolean;
  isMainConversation?: boolean;
  isPinned?: boolean;
  isDraft?: boolean;
  pinIndex?: number;
  runtimeState?: "idle" | "assistant_streaming" | "organizing_context" | "archiving" | "compacting";
  currentTodo?: string;
  autoPushRemoteContactId?: string;
  activeGoal?: ConversationGoalState | null;
  currentTodos?: ChatTodoItem[];
  detachedWindowOpen?: boolean;
  detachedWindowLabel?: string;
  color?: string;
  canCreateNew?: boolean;
  backgroundStatus?: "completed" | "failed";
  previewMessages?: ConversationPreviewMessage[];
  state?: ConversationListItemState;
};

export type ConversationForwardTarget = {
  kind: "local_unarchived" | "remote_im_contact";
  conversationId: string;
  remoteContactId?: string;
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
  kind: string;
  conversationId: string;
  rootConversationId: string;
  title: string;
  status: string;
  active: boolean;
  startedAt: string;
  updatedAt: string;
  completedAt?: string;
  archivedAt?: string;
  elapsedMs: number;
  requestCount: number;
  toolCallCount: number;
  lastToolName: string;
  tokenCount: number;
  inputTokenCount?: number;
  outputTokenCount?: number;
  cacheReadTokenCount?: number;
  cacheWriteTokenCount?: number;
  targetAgentId?: string;
};

export type BackgroundShellTaskSummary = {
  id: string;
  kind: string;
  status: string;
  exitCode: number | null;
  description: string;
  command: string;
  cwd: string;
  startedAt: string;
  timeoutMs: number | null;
  log: string;
  outputTail: string;
};

export type ScheduleEvent = {
  id: string;
  runId: string;
  conversationId: string;
  delegateId?: string;
  rootConversationId?: string;
  phase: string;
  createdAt: string;
  elapsedMs: number;
  success?: boolean;
  detail: Record<string, unknown>;
};

export type ScheduleRun = {
  runId: string;
  conversationId: string;
  delegateId?: string;
  rootConversationId?: string;
  traceId?: string;
  scene?: string;
  requestFormat?: string;
  provider?: string;
  model?: string;
  baseUrl?: string;
  headers?: Array<{ name: string; value: string }>;
  tools?: unknown;
  status: string;
  startedAt: string;
  updatedAt: string;
  elapsedMs: number;
  requestCount: number;
  toolCallCount: number;
  lastToolName?: string;
  lastModelName?: string;
  events: ScheduleEvent[];
};

export type AgentWorkSignalPayload = {
  conversationId: string;
  agentId: string;
  delegateId: string;
};

/** 会话撤回完成事件：后端截断会话后广播给所有已打开该会话的端。 */
export type ChatRewindCompletedPayload = {
  conversationId: string;
  /** 被撤回的目标消息 ID（该消息及其之后均已被后端删除）。 */
  targetMessageId: string;
  /** 截断后保留的最后一条消息 ID；为空表示后端未能给出（正常不会发生）。 */
  remainingLastMessageId?: string;
  removedCount: number;
  remainingCount: number;
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

export type UsageOverviewTotals = {
  conversationCount: number;
  archivedConversationCount: number;
  activeConversationCount: number;
  delegateConversationCount: number;
  withUsageConversationCount: number;
  weightedTokens: number;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  reasoningTokens: number;
};

export type UsageAggregateItem = {
  key: string;
  label: string;
  conversationCount: number;
  weightedTokens: number;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  reasoningTokens: number;
};

export type UsageProviderModelAggregateItem = {
  key: string;
  providerKey: string;
  providerLabel: string;
  modelName: string;
  conversationCount: number;
  weightedTokens: number;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  reasoningTokens: number;
};

export type UsageConversationItem = {
  conversationId: string;
  title: string;
  summaryTitle?: string;
  updatedAt: string;
  archivedAt?: string | null;
  agentId: string;
  agentName: string;
  departmentId: string;
  departmentName: string;
  avatarPath?: string;
  avatarUpdatedAt?: string;
  apiConfigId: string;
  apiConfigName: string;
  modelName: string;
  conversationKind: string;
  isDelegate: boolean;
  isSystemNotificationConversation: boolean;
  messageCount: number;
  weightedTokens: number;
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  cacheReadTokens: number;
  cacheWriteTokens: number;
  reasoningTokens: number;
};

export type UsageOverview = {
  generatedAt: string;
  totals: UsageOverviewTotals;
  conversations: UsageConversationItem[];
  byProviderModel: UsageProviderModelAggregateItem[];
  byModel: UsageAggregateItem[];
  byApiConfig: UsageAggregateItem[];
  byAgent: UsageAggregateItem[];
  byDepartment: UsageAggregateItem[];
  byKind: UsageAggregateItem[];
};
