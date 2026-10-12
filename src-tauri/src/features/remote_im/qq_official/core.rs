// ==================== QQ 官方机器人：常量与凭证模型 ====================

/// QQ 开放平台正式环境 OpenAPI 基地址。
const QQ_OFFICIAL_API_BASE_URL: &str = "https://api.sgroup.qq.com";
/// QQ 开放平台沙箱环境 OpenAPI 基地址。
const QQ_OFFICIAL_SANDBOX_API_BASE_URL: &str = "https://sandbox.api.sgroup.qq.com";
/// 调用凭证接口（正式环境与沙箱环境共用）。
const QQ_OFFICIAL_TOKEN_URL: &str = "https://bots.qq.com/app/getAppAccessToken";

/// 单聊 + 群聊事件意图 GROUP_AND_C2C_EVENT (1 << 25)。
const QQ_OFFICIAL_INTENT_GROUP_AND_C2C: i64 = 1 << 25;

/// 调用凭证提前刷新余量（秒）。
const QQ_OFFICIAL_TOKEN_REFRESH_AHEAD_SECS: i64 = 60;
/// 调用凭证兜底有效期（官方默认 7200 秒）。
const QQ_OFFICIAL_DEFAULT_TOKEN_TTL_SECS: i64 = 7200;
/// 默认 HTTP 超时（毫秒）。
const QQ_OFFICIAL_DEFAULT_HTTP_TIMEOUT_MS: u64 = 15_000;
/// 重连退避下限与上限（秒）。
const QQ_OFFICIAL_RECONNECT_MIN_SECS: u64 = 2;
const QQ_OFFICIAL_RECONNECT_MAX_SECS: u64 = 60;

/// 网关操作码。
const QQ_OFFICIAL_OP_DISPATCH: i64 = 0;
const QQ_OFFICIAL_OP_HEARTBEAT: i64 = 1;
const QQ_OFFICIAL_OP_IDENTIFY: i64 = 2;
const QQ_OFFICIAL_OP_RESUME: i64 = 6;
const QQ_OFFICIAL_OP_RECONNECT: i64 = 7;
const QQ_OFFICIAL_OP_INVALID_SESSION: i64 = 9;
const QQ_OFFICIAL_OP_HELLO: i64 = 10;
const QQ_OFFICIAL_OP_HEARTBEAT_ACK: i64 = 11;

/// 网关事件名。
const QQ_OFFICIAL_EVENT_READY: &str = "READY";
const QQ_OFFICIAL_EVENT_C2C_MESSAGE_CREATE: &str = "C2C_MESSAGE_CREATE";
const QQ_OFFICIAL_EVENT_GROUP_AT_MESSAGE_CREATE: &str = "GROUP_AT_MESSAGE_CREATE";
const QQ_OFFICIAL_EVENT_GROUP_MESSAGE_CREATE: &str = "GROUP_MESSAGE_CREATE";

/// 入站消息类型（官方事件字段 `message_type`）。
const QQ_OFFICIAL_MESSAGE_TYPE_PLAIN: i64 = 0;
const QQ_OFFICIAL_MESSAGE_TYPE_QUOTE: i64 = 103;

/// 出站消息类型。
const QQ_OFFICIAL_MSG_TYPE_TEXT: i64 = 0;
const QQ_OFFICIAL_MSG_TYPE_INPUT_NOTIFY: i64 = 6;
const QQ_OFFICIAL_MSG_TYPE_MEDIA: i64 = 7;

/// 「输入中」状态持续时长（秒），官方上限 60 秒。
const QQ_OFFICIAL_INPUT_NOTIFY_SECONDS: i64 = 60;

/// 被动回复窗口与次数上限（官方口径：单聊 60 分钟 / 4 次，群聊 5 分钟 / 5 次）。
const QQ_OFFICIAL_C2C_PASSIVE_WINDOW_SECS: i64 = 60 * 60;
const QQ_OFFICIAL_C2C_PASSIVE_MAX_REPLIES: u32 = 4;
const QQ_OFFICIAL_GROUP_PASSIVE_WINDOW_SECS: i64 = 5 * 60;
const QQ_OFFICIAL_GROUP_PASSIVE_MAX_REPLIES: u32 = 5;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QqOfficialCredentials {
    #[serde(default)]
    app_id: String,
    #[serde(default)]
    app_secret: String,
    #[serde(default)]
    sandbox: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    api_timeout_ms: Option<u64>,
}

impl QqOfficialCredentials {
    fn from_value(value: &Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }

    fn normalized_app_id(&self) -> String {
        self.app_id.trim().to_string()
    }

    fn normalized_app_secret(&self) -> String {
        self.app_secret.trim().to_string()
    }

    fn normalized_api_timeout_ms(&self) -> u64 {
        self.api_timeout_ms
            .filter(|value| *value > 0)
            .unwrap_or(QQ_OFFICIAL_DEFAULT_HTTP_TIMEOUT_MS)
    }

    fn api_base_url(&self) -> &'static str {
        if self.sandbox {
            QQ_OFFICIAL_SANDBOX_API_BASE_URL
        } else {
            QQ_OFFICIAL_API_BASE_URL
        }
    }
}

/// 兼容数字与字符串两种 JSON 形态的整数解析（官方对 `expires_in` 存在字符串返回）。
fn qq_official_json_i64(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    value
        .as_str()
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .and_then(|raw| raw.parse::<i64>().ok())
}
