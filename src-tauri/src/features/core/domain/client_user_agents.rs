// ==================== 供应商客户端请求头预设 ====================
//
// 单一事实源：客户端标识 → 真正发出去的整组请求头。
// 数据源是 magpie 的插件仓（packages/<client>/index.mjs）、
// magpie 内置识别表（internal/agent/*.go 的 UA 前缀）与
// magpie 的 Client 构造（internal/provider/*.go）。
//
// 版本号冻结在采集时的源码值；运行期才拼得出 os/arch 的那些，按 PAI 自己的
// 平台信息拼，映射沿用客户端源码里的写法。会话 id 一类运行期值不写进静态预设，
// 由 client_preset_runtime_headers 在请求时生成。

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientUserAgentPreset {
    id: &'static str,
    name: &'static str,
    headers: Vec<(String, String)>,
}

/// Codex 预设的版本与 User-Agent：逐字对齐 magpie，不自己编。
/// - 版本：magpie/internal/provider/codex_models.go:30 的 codexClientVersion = "0.159.0"。
///   magpie 运行期用 newerVersion(本机 CLI 版本, 请求里的 version) 上调；PAI 拿不到
///   本机 Codex CLI 版本，故取该常量。
/// - UA：magpie/internal/provider/codex_request.go:85-95 的 codexUserAgent .format，
///   Windows 形态样本见 magpie/internal/provider/codex_models_test.go:115。
///   当前固定 Windows 形态，跨平台待办。
const CODEX_PRESET_VERSION: &str = "0.159.0";
const CODEX_PRESET_USER_AGENT: &str =
    "codex_cli_rs/0.159.0 (Windows 10.0.26100; x86_64) WindowsTerminal";

fn client_ua_header(name: &str, value: impl Into<String>) -> (String, String) {
    (name.to_string(), value.into())
}

/// 插件源码把 Node 的 platform/arch 翻成客户端自己的写法；PAI 按同样口径翻。
fn client_ua_rust_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        "x86" => "386",
        other => other,
    }
}

/// AutoClaw 直接用 Node 的 process.arch，不做 Rust 风格改名。
fn client_ua_node_arch() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        "x86" => "ia32",
        other => other,
    }
}

fn client_ua_rust_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macos",
        "windows" => "windows",
        other => other,
    }
}

fn client_ua_autoclaw_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "mac",
        "windows" => "win",
        _ => "linux",
    }
}

fn client_user_agent_presets() -> Vec<ClientUserAgentPreset> {
    let rust_arch = client_ua_rust_arch();
    let rust_platform = client_ua_rust_platform();
    let autoclaw_platform = client_ua_autoclaw_platform();
    let node_arch = client_ua_node_arch();
    let presets: [(&str, &str, String); 45] = [
        // ---- 插件仓：packages/<client>/index.mjs ----
        (
            "cline",
            "Cline",
            "Cline/3.5.54".to_string(),
        ),
        ("workbuddy", "WorkBuddy", "WorkBuddy/5.5.6".to_string()),
        (
            "workbuddy-cli",
            "WorkBuddy CLI",
            "CLI/2.0.0 WorkBuddy/5.5.6".to_string(),
        ),
        ("qoder", "Qoder", "Qoder".to_string()),
        ("minimax-code", "MiniMax Code", "MiniMaxCode".to_string()),
        ("minimax-agent", "MiniMax Agent", "MiniMaxAgent".to_string()),
        (
            "grok",
            "Grok CLI",
            format!("grok-shell/1.0.41 ({rust_platform}; {rust_arch})"),
        ),
        ("factory", "Factory Droid", "factory-cli/0.242.0".to_string()),
        ("zcode", "ZCode", "ZCode/3.16.3".to_string()),
        (
            "zcode-anthropic",
            "ZCode (Anthropic)",
            "ZCode/3.16.3 ai-sdk/anthropic/3.0.81".to_string(),
        ),
        (
            "trae",
            "Trae",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) TRAE-SOLO-CN/0.1.69 Chrome/120.0.6099.291 Electron/28.2.10 Safari/537.36".to_string(),
        ),
        (
            "kiro",
            "Kiro",
            "Kiro-Desktop/0.2.13 (darwin; arm64)".to_string(),
        ),
        (
            "mimo",
            "Xiaomi MiMo",
            "miNative PC/Normal Windows_NT/10.0.26100 SDKV/1.0.0 DEVT/PC DEVS/Windows APP/miaccount_desktop APPV/0.1.0".to_string(),
        ),
        (
            "autoclaw",
            "AutoClaw",
            format!("AutoClaw/1.18.1 ({autoclaw_platform}-{node_arch})"),
        ),
        (
            "autoclaw-cn",
            "AutoClaw CN",
            format!("AutoClaw/2.1.0 ({autoclaw_platform}-{node_arch})"),
        ),
        (
            "zed",
            "Zed",
            format!("Zed/1.23.0 ({rust_platform}; {rust_arch})"),
        ),
        ("commandcode", "Command Code", "cli".to_string()),
        (
            "opendesign",
            "Open Design",
            "opencode-opendesign-auth/0.1.6".to_string(),
        ),
        // ---- magpie 内置：能确定完整形态的用完整值；只剩前缀的逐条注明 ----
        // 完整值来自抓包、客户端源码或 magpie 网关源码；保留项只被 magpie 按前缀识别。
        (
            // magpie/internal/agent/useragent_test.go:12 的 claude-cli 真实样本
            "claude-code",
            "Claude Code",
            "claude-cli/2.1.0 (external, cli)".to_string(),
        ),
        (
            "codex",
            "Codex",
            CODEX_PRESET_USER_AGENT.to_string(),
        ),
        (
            "opencode",
            "OpenCode",
            "opencode/1.19.0 ai-sdk/provider-utils/4.0.23 runtime/bun/1.3.14".to_string(),
        ),
        (
            "gemini-cli",
            "Gemini CLI",
            "GeminiCLI/0.61.0/gemini-2.5-pro (win32; x64; terminal)".to_string(),
        ),
        (
            "deepseek-harness",
            "DeepSeek Harness",
            // deepseek-harness/package.json:3 = 0.2.0-rc.2；
            // packages/llm/llm/src/attribution.ts:43 的完整 URL
            "deepseek-harness/0.2.0-rc.2 (+https://github.com/deepseek-ai/deepseek-harness)"
                .to_string(),
        ),
        ("hermes", "Hermes Agent", "hermes-agent".to_string()), // 仅前缀，未确认真实形态
        ("kimi-code", "Kimi Code", "KimiCLI/1.52.0".to_string()),
        ("qwen-code", "Qwen Code", "qwencode".to_string()), // 仅前缀，未确认真实形态；已知形态为 QwenCode/<version>，版本号未追到
        ("oh-my-pi", "oh-my-pi", "oh-my-pi/1.0".to_string()),
        (
            "muse-code",
            "Muse Code",
            "muse-build/1.4.2 (non-interactive; macos-aarch64; build 0)".to_string(),
        ),
        ("hanako", "OpenHanako", "HanaAgent/1.0".to_string()),
        ("atomcode", "AtomCode", "atomcode/5.2.1".to_string()),
        ("alma", "Alma", "Alma/1.2.0".to_string()),
        ("morph", "Mister Morph", "mistermorph".to_string()), // 仅前缀，未确认真实形态
        ("reasonix", "Reasonix Studio", "reasonix/2.24.0".to_string()),
        ("goose", "Goose", "goose".to_string()), // 仅前缀；客户端 HTTP 库不发 UA，靠 key 识别
        ("crush", "Crush", "crush".to_string()), // 仅前缀，未确认真实形态
        ("copilot", "Copilot CLI", "copilot/1.0.88".to_string()),
        ("stepcode", "StepCode", "stepcode".to_string()), // 仅前缀；客户端不发 UA，靠 key 识别
        ("lody", "Lody", "lody".to_string()), // 仅前缀，未确认真实形态
        ("cindy", "Cindy", "cindy".to_string()), // 仅前缀，未确认真实形态
        ("codebuddy", "CodeBuddy Code", "codebuddy".to_string()), // 仅前缀，未确认真实形态；已知形态为 CLI/<ver> CodeBuddy/<ver>
        ("vscode", "VS Code", "GitHubCopilotChat/0.68.0".to_string()),
        ("mimocode", "MiMo Code", "mimocode/1.2.3".to_string()),
        ("deveco", "DevEco Code", "deveco/0.2.3 ai-sdk/provider-utils/4.0".to_string()),
        ("cursor", "Cursor CLI", "cursor".to_string()), // 仅前缀，未确认真实形态
        // ---- internal/provider 的 Client 构造：整组头，之前漏掉的 Kilo ----
        ("kilo", "Kilo Code", "Kilo-Code/7.8.3".to_string()),
    ];
    presets
        .into_iter()
        .map(|(id, name, user_agent)| {
            let mut headers = vec![client_ua_header("user-agent", user_agent)];
            headers.extend(client_preset_static_headers(id));
            ClientUserAgentPreset { id, name, headers }
        })
        .collect()
}

/// 客户端源码里写死的随行头；会话 id 一类运行期值不在这里。
fn client_preset_static_headers(id: &str) -> Vec<(String, String)> {
    match id {
        // magpie/internal/provider/cline.go:25-31 的 clineHeaders
        "cline" => vec![
            client_ua_header("HTTP-Referer", "https://cline.bot"),
            client_ua_header("X-Title", "Cline"),
            client_ua_header("X-IS-MULTIROOT", "false"),
            client_ua_header("X-CLIENT-TYPE", "cline-desktop"),
            client_ua_header("X-CLIENT-VERSION", "3.5.54"),
        ],
        // magpie/internal/provider/opencode_client.go:44-55
        "opencode" => vec![
            client_ua_header("x-opencode-client", "cli"),
            client_ua_header("x-opencode-project", "global"),
        ],
        // magpie/internal/provider/kilo.go:33-48
        "kilo" => vec![
            client_ua_header("HTTP-Referer", "https://kilocode.ai"),
            client_ua_header("X-Title", "Kilo Code"),
            client_ua_header("X-KILOCODE-EDITORNAME", "Kilo CLI 7.8.3"),
            client_ua_header("x-kilocode-mode", "code"),
        ],
        // plugins/packages/grok/index.mjs:303-313 的 sign()
        "grok" => vec![
            client_ua_header("x-grok-client-version", "1.0.41"),
            client_ua_header("x-xai-token-auth", "xai-grok-cli"),
            client_ua_header("x-grok-client-identifier", "grok-shell"),
            client_ua_header("x-grok-client-mode", "headless"),
        ],
        // magpie/internal/provider/codex_request.go:41-52 的 codexSign。
        // Accept 与 session-id/thread-id 在 body == nil 时不发（即非流式请求），
        // 因此不进静态预设，由流式调用点按实际流式与否注入。
        "codex" => vec![
            client_ua_header("OpenAI-Beta", "responses=experimental"),
            client_ua_header("version", CODEX_PRESET_VERSION),
            client_ua_header("originator", "codex_cli_rs"),
        ],
        _ => Vec::new(),
    }
}

/// magpie 的 id 形状（internal/provider/opencode_client.go:210-260）：
/// 前缀_ + 12 位十六进制时间计数 + 14 位 base62。ses_ 的低 3 位十六进制固定 fff，
/// 满足 openCodeRealSession 的「低 12 位是小计数（按位取反）」检查。
const OPENCODE_BASE62: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
static OPENCODE_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn opencode_base62_suffix(mut value: u64) -> String {
    let mut suffix = String::with_capacity(14);
    for _ in 0..14 {
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        suffix.push(OPENCODE_BASE62[(value % 62) as usize] as char);
        value = value.wrapping_mul(0x2545f4914f6cdd1d);
    }
    suffix
}

/// 同一会话标识得到同一个 ses_ id；对应 magpie 的 openCodeSession 按 seed 记忆。
fn opencode_session_id(seed: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in seed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("ses_{:09x}fff{}", hash & 0xf_ffff_ffff, opencode_base62_suffix(hash))
}

/// 每次请求唯一的 msg_ id，形状同 magpie 的 openCodeNewID("msg", false, now)。
fn opencode_request_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let counter = OPENCODE_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let current = (now.as_millis() as u64)
        .wrapping_mul(0x1000)
        .wrapping_add(counter % 0x1000);
    format!(
        "msg_{:012x}{}",
        current & 0xffff_ffff_ffff,
        opencode_base62_suffix(current ^ counter)
    )
}

/// 只有请求时才知道的随行头：会话标识、每次请求唯一的消息 id。
/// 静态预设里不写假值；这里按 PAI 当前会话生成，来源写在各分支注释里。
fn client_preset_runtime_headers(
    preset_id: Option<&str>,
    session_key: &str,
) -> Vec<(String, String)> {
    let Some(id) = preset_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };
    match id {
        // magpie/internal/provider/opencode_client.go:44-55 的 OpenCodeClient
        "opencode" => {
            let session = opencode_session_id(session_key);
            vec![
                client_ua_header("x-opencode-session", session.clone()),
                client_ua_header("x-opencode-session-id", session),
                client_ua_header("x-opencode-request", opencode_request_id()),
            ]
        }
        // magpie/internal/provider/kilo.go:33-48 的 KiloClient
        "kilo" => {
            let task = opencode_session_id(session_key);
            vec![
                client_ua_header("X-KILOCODE-TASKID", task.clone()),
                client_ua_header("x-session-affinity", task.clone()),
                client_ua_header("X-Session-Id", task),
            ]
        }
        // 插件 grok 的 x-grok-conv-id（index.mjs:989）
        "grok" => vec![
            client_ua_header("x-grok-conv-id", session_key),
            client_ua_header("x-grok-session-id", session_key),
        ],
        // codex 的 session-id/thread-id 沿用 provider_session_headers 的既有通道，
        // 且只在流式请求发（codexSign 的 body == nil 判据），不在这里重复注入。
        _ => Vec::new(),
    }
}

/// 预设 id 是否存在于表里；用于「Codex 供应商没选或选了未知 id 时回落 codex 预设」。
fn client_user_agent_preset_exists(id: &str) -> bool {
    let id = id.trim();
    !id.is_empty() && client_user_agent_presets().iter().any(|preset| preset.id == id)
}

/// 只有表里存在的 id 才生效；空值、未知 id 一律维持默认 P-ai。
fn resolve_client_preset_headers(preset_id: Option<&str>) -> Option<Vec<(String, String)>> {
    let preset_id = preset_id?.trim();
    if preset_id.is_empty() {
        return None;
    }
    client_user_agent_presets()
        .into_iter()
        .find(|preset| preset.id == preset_id)
        .map(|preset| preset.headers)
}

#[tauri::command]
fn list_client_user_agents() -> Vec<ClientUserAgentPreset> {
    client_user_agent_presets()
}
