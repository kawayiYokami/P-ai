// ==================== 窗口与更新 ====================
include!("config_and_persona/window_and_update.rs");

// ==================== 人格与聊天设置 ====================
include!("config_and_persona/persona_and_chat_settings.rs");

// ==================== 会话快照与API设置 ====================
include!("config_and_persona/conversation_snapshot_api.rs");

// ==================== 会话与归档列表 ====================
include!("config_and_persona/unarchived_conversations.rs");

// ==================== 配置迁移 ====================
include!("config_and_persona/migration.rs");

// ==================== 消息仓库启动迁移 ====================
include!("config_and_persona/message_store_migration.rs");

// ==================== 开机自启 ====================
include!("config_and_persona/autostart.rs");

// ==================== 存储管理 ====================
include!("config_and_persona/storage_usage.rs");

// ==================== 人格表情库管理 ====================
include!("config_and_persona/meme_library.rs");
