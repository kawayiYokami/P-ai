const APP_DATA_SCHEMA_VERSION: u32 = 1;

// ========== 数据迁移版本门禁 ==========
//
// 版本语义：
//   - DATA_MIGRATION_CURRENT_VERSION：当前数据迁移版本，启动期写回 runtime_state。
//   - V2/V3：data_migration_steps() 注册的版本化迁移步骤，按版本号递增执行。
//
// 新增迁移（v2+）的接入流程：
//   1. 在 app_data_layout.rs 的 data_migration_steps() 注册一个 DataMigrationStep；
//   2. 在此处新增 DATA_MIGRATION_VERSION_V2 常量，并把 CURRENT_VERSION 提到它。
const DATA_MIGRATION_VERSION_V2_ASSISTANT_WORKSPACE_FOR_EMPTY_SHELL_WORKSPACES: u32 = 2;
/// 消息存储迁移（会话分片 V2→V3→V4）自己的版本。
/// 它记在独立的 kv `message_store_migration_version`，**不得**与全局 `DATA_MIGRATION_CURRENT_VERSION` 共用一个常量：
/// 否则任何一次纯应用数据迁移（如 V5 部门转人格）都会把已完成消息存储迁移的老用户重新拦在门闩外。
const MESSAGE_STORE_MIGRATION_CURRENT_VERSION: u32 = 4;
/// V5：把「部门」承载的权责（模型/权限/提示词/下级）迁到人格身上，部门退场。
/// 旧结构 `AppConfig.departments` 只允许在 `agent_org_migration.rs` 内读取。
const DATA_MIGRATION_VERSION_V5_DEPARTMENTS_TO_AGENT_ORGANIZATION: u32 = 5;
/// V6：头像路径从绝对路径改为相对数据根的相对路径，让数据根搬迁 / 导入导出后仍可解析。
const DATA_MIGRATION_VERSION_V6_AVATAR_PATH_RELATIVE: u32 = 6;
const DATA_MIGRATION_CURRENT_VERSION: u32 = DATA_MIGRATION_VERSION_V6_AVATAR_PATH_RELATIVE;
const MAX_MULTIMODAL_BYTES: usize = 10 * 1024 * 1024;
const DEFAULT_AGENT_ID: &str = "default-agent";
const DEPUTY_AGENT_ID: &str = "deputy-agent";
/// 内置人格 id：原内置部门在新组织里的身份。
/// `assistants`（显示名）复用 `default-agent`、`explorer`（显示名）复用 `deputy-agent`，
/// 故这两个不新增常量；其余 5 个是新建的人格 id。
const LEADER_AGENT_ID: &str = "leader";
const REVIEWER_AGENT_ID: &str = "reviewer";
const SADDLER_AGENT_ID: &str = "saddler";
const SUPPORT_AGENT_ID: &str = "support";
const HR_AGENT_ID: &str = "hr";
const USER_PERSONA_ID: &str = "user-persona";
const SYSTEM_PERSONA_ID: &str = "system-persona";
const DELEGATE_TOOL_KIND_DELEGATE: &str = "delegate";
const DELEGATE_TOOL_KIND_USER_MENTION: &str = "user_async_delegate";
/// 深度回忆委托：仅此类委托会话挂载 deeprecall_search / deeprecall_context。
const DELEGATE_TOOL_KIND_DEEP_RECALL: &str = "deeprecall";
const SYSTEM_NOTIFICATION_CONVERSATION_ID: &str = "system-notification-conversation";
const CONVERSATION_KIND_CHAT: &str = "chat";
const CONVERSATION_KIND_SIDE_CHAT: &str = "side_chat";
const CONVERSATION_KIND_SYSTEM_NOTIFICATION: &str = "system_notification";
const CONVERSATION_KIND_DELEGATE: &str = "delegate";
const CONVERSATION_KIND_REMOTE_IM_CONTACT: &str = "remote_im_contact";
const DEFAULT_RESPONSE_STYLE_ID: &str = "concise";
const DEFAULT_PDF_READ_MODE: &str = "image";
const DEFAULT_BACKGROUND_VOICE_SCREENSHOT_MODE: &str = "focused_window";
const CHAT_ABORTED_BY_USER_ERROR: &str = "CHAT_ABORTED_BY_USER";
const CHAT_DISPATCH_RESTART_AFTER_COMPACTION: &str = "CHAT_DISPATCH_RESTART_AFTER_COMPACTION";
const APP_HTTP_ORIGINATOR: &str = "p_ai_desktop";
