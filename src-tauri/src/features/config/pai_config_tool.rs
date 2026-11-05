//! PAI config 工具：配置位置说明书 + 校验器 + 生效开关。
//!
//! 工具面只有三个命令：`help` / `check` / `reload`。改配置的路径统一为
//! 「看地图 → 直接编辑文件 → check → reload」，不再提供逐字段的读写命令。

use std::{cell::RefCell, collections::HashSet, fs, path::{Path, PathBuf}};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

thread_local! {
    static OUTPUT_BUFFER: RefCell<String> = const { RefCell::new(String::new()) };
}

// ==================== 域内最小结构 ====================
// 本模块是独立编译单元，看不到主程序经 include! 引入的类型，所以每类文件
// 在这里保留一份只够校验用的最小定义。

fn default_true() -> bool {
    true
}

fn default_memory_recall_mode() -> String {
    "auto".to_string()
}

fn default_global_scope() -> String {
    "global".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ApiToolConfig {
    id: String,
    #[serde(default)]
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    values: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AgentProfile {
    id: String,
    name: String,
    #[serde(alias = "prompt")]
    system_prompt: String,
    #[serde(default)]
    tools: Vec<ApiToolConfig>,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    updated_at: String,
    #[serde(default)]
    avatar_path: Option<String>,
    #[serde(default)]
    avatar_updated_at: Option<String>,
    #[serde(default)]
    is_built_in_user: bool,
    #[serde(default)]
    is_built_in_system: bool,
    #[serde(default)]
    private_memory_enabled: bool,
    #[serde(default = "default_memory_recall_mode")]
    memory_recall_mode: String,
    #[serde(default = "default_global_scope")]
    scope: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    resident_skill_names: Vec<String>,
    #[serde(default)]
    optional_skill_names: Vec<String>,
    #[serde(default)]
    api_config_ids: Vec<String>,
    #[serde(default)]
    api_config_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    permission_control: Option<JsonValue>,
    #[serde(default)]
    child_agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AgentsFile {
    #[serde(default)]
    agents: Vec<AgentProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ShellWorkspaceConfigFile {
    #[serde(default)]
    name: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppConfigSnapshot {
    #[serde(default)]
    shell_workspaces: Vec<ShellWorkspaceConfigFile>,
}

/// `servers/<id>.json` 的真实形态：`{ "mcpServers": { "<id>": { ... } } }`。
/// 字段与 `features/mcp/parser.rs` 里接受的服务直连字段一致。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct McpServerFile {
    #[serde(default)]
    mcp_servers: std::collections::BTreeMap<String, McpServerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct McpServerEntry {
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    env: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "type")]
    transport_type: Option<String>,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    http_headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    env_http_headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    bearer_token_env_var: Option<String>,
    #[serde(default)]
    enabled_tools: Option<Vec<String>>,
    #[serde(default)]
    disabled_tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct McpPolicyFile {
    #[serde(default)]
    server_id: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    tools: Vec<McpToolPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct McpToolPolicy {
    tool_name: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

/// `app_config.toml` 的顶层形状探针。
///
/// 顶层字段与 `features/core/domain/types_config.rs` 的 `AppConfig` 一一对应：
/// 标量按真实类型声明，数组只要求「是数组」，逐元素字段留给应用自己的
/// 反序列化。这样 `check` 报出的必填项与类型错误和 `read_config` 的判定一致。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfigProbe {
    hotkey: String,
    #[serde(default)]
    ui_language: String,
    #[serde(default)]
    ui_font: String,
    #[serde(default)]
    code_font: String,
    #[serde(default, alias = "uiSizePreset", alias = "ui_size_preset")]
    ui_size_scale: u16,
    #[serde(default)]
    web_access_port: u16,
    #[serde(default)]
    web_access_enabled: bool,
    #[serde(default)]
    web_access_password: String,
    #[serde(default)]
    github_update_method: String,
    #[serde(default)]
    skipped_github_update_version: String,
    #[serde(default)]
    record_hotkey: String,
    #[serde(default)]
    record_background_wake_enabled: bool,
    #[serde(default)]
    min_record_seconds: u32,
    #[serde(default)]
    max_record_seconds: u32,
    #[serde(default)]
    tool_max_iterations: u32,
    #[serde(default)]
    llm_round_log_capacity: u32,
    #[serde(default)]
    message_notification_enabled: bool,
    #[serde(default)]
    message_notification_sound_enabled: bool,
    #[serde(default)]
    desktop_operation_notice_enabled: bool,
    #[serde(default)]
    desktop_operate_enabled: bool,
    #[serde(default)]
    selected_api_config_id: String,
    #[serde(default, alias = "chatApiConfigId")]
    expert_api_config_id: String,
    #[serde(default)]
    vision_api_config_id: Option<String>,
    #[serde(default)]
    tool_review_api_config_id: Option<String>,
    #[serde(default)]
    stt_api_config_id: Option<String>,
    #[serde(default)]
    image_generation_model_id: Option<String>,
    #[serde(default)]
    image_to_video_model_id: Option<String>,
    #[serde(default)]
    stt_auto_send: bool,
    #[serde(default)]
    terminal_shell_kind: String,
    #[serde(default)]
    simple_setup_mode: bool,
    #[serde(default)]
    search_mode: String,
    #[serde(default)]
    shell_workspaces: Vec<toml::Value>,
    #[serde(default)]
    mcp_servers: Vec<toml::Value>,
    #[serde(default)]
    remote_im_channels: Vec<toml::Value>,
    #[serde(default)]
    search_engines: Vec<toml::Value>,
    #[serde(default)]
    provider_non_stream_base_urls: Vec<String>,
    #[serde(default)]
    api_providers: Vec<toml::Value>,
    #[serde(default)]
    image_providers: Vec<toml::Value>,
    #[serde(default)]
    api_configs: Vec<toml::Value>,
}

// ==================== 入口与上下文 ====================

#[derive(Debug, Clone)]
pub(crate) struct CliContext {
    config_path: PathBuf,
    data_path: PathBuf,
    workspace_root: PathBuf,
}

impl CliContext {
    fn detect() -> Result<Self, String> {
        if let Some(portable_root) = detect_portable_runtime_root() {
            let config_dir = portable_root.join("config");
            return Ok(Self {
                config_path: config_dir.join("app_config.toml"),
                data_path: config_dir.join("config_mark"),
                workspace_root: portable_root.join("llm-workspace"),
            });
        }

        let config_dir = resolve_standard_config_dir()?;
        Ok(Self {
            config_path: config_dir.join("app_config.toml"),
            data_path: config_dir.join("config_mark"),
            workspace_root: config_dir.join("llm-workspace"),
        })
    }

    fn app_root(&self) -> PathBuf {
        app_root_from_cli_data_path(&self.data_path)
    }

    fn config_dir(&self) -> PathBuf {
        self.config_path
            .parent()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.config_path.clone())
    }
}

#[allow(dead_code)]
pub fn run_cli(args: &[String]) -> Result<String, String> {
    let ctx = CliContext::detect()?;
    run_with_context(&ctx, args)
}

#[allow(dead_code)]
pub fn run_with_paths(
    config_path: PathBuf,
    data_path: PathBuf,
    workspace_root: PathBuf,
    args: &[String],
) -> Result<String, String> {
    let ctx = CliContext {
        config_path,
        data_path,
        workspace_root,
    };
    run_with_context(&ctx, args)
}

#[allow(dead_code)]
pub fn run_command_with_paths(
    config_path: PathBuf,
    data_path: PathBuf,
    workspace_root: PathBuf,
    command: &str,
) -> Result<String, String> {
    let args = split_command_line(command)?;
    run_with_paths(config_path, data_path, workspace_root, &args)
}

fn run_with_context(ctx: &CliContext, args: &[String]) -> Result<String, String> {
    clear_output_buffer();
    let command = args.first().map(String::as_str).unwrap_or("");
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };
    match command {
        "" | "help" | "--help" | "-h" => handle_help(ctx, rest)?,
        "check" => handle_check(ctx, rest)?,
        "reload" => handle_reload(ctx)?,
        // 供应商配置命令当前有意不开放，保持原样。
        "provider" => {
            return Err("provider 命令当前未开放，请不要通过 config 工具修改供应商。".to_string())
        }
        other => return Err(removed_command_hint(other).unwrap_or_else(|| unknown_command_hint(other))),
    }
    Ok(take_output_buffer())
}

fn unknown_command_hint(command: &str) -> String {
    format!(
        "未知命令: {command}\n可用命令: help / check / reload。\n先运行 config \"help\" 看目录地图。"
    )
}

/// 已废除的子命令：一律给迁移提示，不静默成功。
fn removed_command_hint(command: &str) -> Option<String> {
    let hint = match command {
        "agent" => {
            "agent 系列命令已废除。人格定义现在直接改文件：\n\
             1. config \"help\" -agent 看 config/agents.json 的字段清单；\n\
             2. 按字段直接编辑 config/agents.json；\n\
             3. config \"check\" -agent 校验，通过后 config \"reload\" 生效。"
        }
        "mcp" => {
            "mcp 系列命令已废除。MCP 服务现在直接改文件：\n\
             1. config \"help\" -mcp 看字段清单；\n\
             2. 服务定义写 llm-workspace/mcp/servers/<id>.json，启停策略写 llm-workspace/mcp/policies/<id>.json；\n\
             3. config \"check\" -mcp 校验，通过后 config \"reload\" 生效。"
        }
        "skill" => {
            "skill 系列命令已废除。技能现在直接改文件：\n\
             1. config \"help\" -skill 看 frontmatter 字段；\n\
             2. 在 llm-workspace/skills/<name>/SKILL.md 里写 frontmatter 与正文；\n\
             3. config \"check\" -skill 校验，通过后 config \"reload\" 生效。"
        }
        "approot" => {
            "approot 命令已废除。\n\
             直接运行 config \"help\"，目录地图里已给出 app_root 与 config_dir 的绝对路径。"
        }
        "store" => {
            "store 命令已从 config 工具面移除，能力商店的搜索与安装这条路暂时不可用。\n\
             现在只能手工落文件：技能写到 llm-workspace/skills/<name>/SKILL.md，\n\
             MCP 写到 llm-workspace/mcp/servers/<id>.json，再 config \"reload\" 生效。\n\
             字段清单用 config \"help\" -skill 或 config \"help\" -mcp 查。"
        }
        "update" | "delete" | "enable" | "disable" | "new" => {
            "这些顶层命令已废除。\n\
             配置改动一律直接编辑文件；改完 config \"check\" 校验，再 config \"reload\" 生效。\n\
             结构性问题（新建人格、组织父子边、头像路径）也走文件：先看 config \"help\" -agent。"
        }
        _ => return None,
    };
    Some(format!("{hint}\n（原命令: config \"{command}\"）"))
}

// ==================== help：目录地图 ====================

struct LayoutEntry {
    /// 相对 app_root 的路径；以 `/` 结尾表示目录，含 `<...>` 表示占位符。
    path: &'static str,
    desc: &'static str,
}

/// 目录地图的唯一来源，须与 `docs/data-schema-v2.md` 的「布局总表」保持一致。
/// 顺序按顶层目录聚拢，便于渲染成分组结构。
const LAYOUT: &[LayoutEntry] = &[
    LayoutEntry {
        path: "config/app_config.toml",
        desc: "应用主配置：模型供应商、界面、搜索引擎、记忆与工具开关。含明文密钥。",
    },
    LayoutEntry {
        path: "config/agents.json",
        desc: "人格定义与组织关系，父子边写在 childAgentIds 里。",
    },
    LayoutEntry {
        path: "config/llm-workspace/",
        desc: "skills / MCP / tool-output 的实际落点（绑定 config_dir，不随 app_root 上提）。",
    },
    LayoutEntry {
        path: "llm-workspace/downloads/",
        desc: "下载落点，相对 app_root。",
    },
    LayoutEntry {
        path: "state/state.sqlite",
        desc: "全局状态总库：窗口布局、缓存、远程 IM 联系人等。",
    },
    LayoutEntry {
        path: "state/runtime_state.json",
        desc: "旧版全局状态 JSON，仅在数据迁移时读取，不再写入。",
    },
    LayoutEntry {
        path: "chat/chat_metadata.sqlite",
        desc: "会话元数据、消息定位索引、用量统计。",
    },
    LayoutEntry {
        path: "chat/conversations/<id>/",
        desc: "单个会话的正文压缩块。",
    },
    LayoutEntry {
        path: "backups/",
        desc: "迁移与导出的备份，按 backups/<时间戳>/ 组织。",
    },
    LayoutEntry {
        path: "memory/memory_store.db",
        desc: "记忆库。",
    },
    LayoutEntry {
        path: "task/task_store.db",
        desc: "定时任务库。",
    },
    LayoutEntry {
        path: "delegate/delegate_store.db",
        desc: "委托库。",
    },
    LayoutEntry {
        path: "avatars/",
        desc: "人格头像（webp）。",
    },
    LayoutEntry {
        path: "media/",
        desc: "媒体文件（<sha256>.<ext>）。",
    },
    LayoutEntry {
        path: "exports/",
        desc: "导出产物。",
    },
    LayoutEntry {
        path: "auth/",
        desc: "Codex / Grok 等认证落点。",
    },
    LayoutEntry {
        path: "indexes/",
        desc: "聊天历史检索索引。",
    },
    LayoutEntry {
        path: "remote-im/",
        desc: "远程 IM 频道状态。",
    },
    LayoutEntry {
        path: "web-access/",
        desc: "Web 访问桥接令牌。",
    },
    LayoutEntry {
        path: "temp/",
        desc: "临时产物。",
    },
    LayoutEntry {
        path: "tool-review-reports/",
        desc: "工具审查报告。",
    },
    LayoutEntry {
        path: "delegate-conversations/",
        desc: "委托会话存储。",
    },
];

fn handle_help(ctx: &CliContext, args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        push_output(&render_directory_map(ctx));
        return Ok(());
    }
    let requests = parse_topic_flags(args)?;
    let mut blocks = Vec::new();
    for request in &requests {
        blocks.push(render_topic(ctx, request)?);
    }
    push_output(&blocks.join("\n\n"));
    Ok(())
}

fn render_directory_map(ctx: &CliContext) -> String {
    let app_root = ctx.app_root();
    let config_dir = ctx.config_dir();
    let mut out = String::new();
    out.push_str(&format!("app_root:   {}\n", app_root.display()));
    out.push_str(&format!("config_dir: {}\n\n", config_dir.display()));

    let mut current_group = String::new();
    for entry in LAYOUT {
        let (label, group) = match entry.path.split_once('/') {
            Some((group, rest)) if !rest.is_empty() => (rest, Some(group)),
            _ => (entry.path, None),
        };
        match group {
            Some(group) => {
                if group != current_group {
                    out.push_str(&format!("{group}/\n"));
                    current_group = group.to_string();
                }
            }
            None => current_group.clear(),
        }
        let exists = layout_entry_exists(&app_root, entry.path);
        let suffix = if exists { "" } else { "  （尚未创建）" };
        match group {
            Some(_) => out.push_str(&format!("  {:<26} {}{}\n", label, entry.desc, suffix)),
            None => out.push_str(&format!("{:<28} {}{}\n", label, entry.desc, suffix)),
        }
    }

    out.push_str("\n核心文件：config/app_config.toml、config/agents.json\n");
    out.push_str(
        "改法：直接编辑上面的文件，改完 config \"check\" 校验，再 config \"reload\" 让运行中的应用重新加载内存态。\n",
    );
    out.push_str("app_config.toml 按 mtime 自动失效，不需要 reload 就能被读到。\n");
    out.push('\n');
    out.push_str(&available_topics_line());
    out
}

fn layout_entry_exists(app_root: &Path, entry_path: &str) -> bool {
    let trimmed = entry_path.trim_end_matches('/');
    let probe = if trimmed.contains('<') {
        trimmed.rsplit_once('/').map(|(head, _)| head).unwrap_or("")
    } else {
        trimmed
    };
    if probe.is_empty() {
        return true;
    }
    app_root.join(probe).exists()
}

fn available_topics_line() -> String {
    "主题参数（可一次给多个）：\n  -app    app_config.toml\n  -agent  config/agents.json\n  -mcp    llm-workspace/mcp/servers/<id>.json 与 policies/<id>.json\n  -skill  llm-workspace/skills/<name>/SKILL.md 的 frontmatter".to_string()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Topic {
    App,
    Agent,
    Mcp,
    Skill,
}

struct TopicRequest {
    topic: Topic,
    selectors: Vec<String>,
}

fn parse_topic_flags(args: &[String]) -> Result<Vec<TopicRequest>, String> {
    let mut requests: Vec<TopicRequest> = Vec::new();
    for arg in args {
        let topic = match arg.as_str() {
            "-app" | "--app" => Some(Topic::App),
            "-agent" | "--agent" => Some(Topic::Agent),
            "-mcp" | "--mcp" => Some(Topic::Mcp),
            "-skill" | "--skill" => Some(Topic::Skill),
            _ => None,
        };
        match topic {
            Some(topic) => requests.push(TopicRequest {
                topic,
                selectors: Vec::new(),
            }),
            None => {
                if arg.starts_with('-') {
                    return Err(format!(
                        "未知主题: {arg}\n{}",
                        available_topics_line()
                    ));
                }
                match requests.last_mut() {
                    Some(last) => last.selectors.push(arg.clone()),
                    None => {
                        return Err(format!(
                            "主题参数前缺少文件名: {arg}\n{}",
                            available_topics_line()
                        ))
                    }
                }
            }
        }
    }
    if requests.is_empty() {
        return Err(available_topics_line());
    }
    Ok(requests)
}

struct FieldSpec {
    name: &'static str,
    ty: &'static str,
    required: bool,
    desc: &'static str,
}

const APP_FIELDS: &[FieldSpec] = &[
    FieldSpec { name: "hotkey", ty: "string", required: true, desc: "全局呼出热键。" },
    FieldSpec { name: "uiLanguage", ty: "string", required: false, desc: "界面语言。" },
    FieldSpec { name: "uiFont", ty: "string", required: false, desc: "界面字体。" },
    FieldSpec { name: "codeFont", ty: "string", required: false, desc: "代码字体。" },
    FieldSpec { name: "uiSizeScale", ty: "integer", required: false, desc: "界面缩放档位（旧键名 uiSizePreset）。" },
    FieldSpec { name: "webAccessPort", ty: "integer", required: false, desc: "Web 访问端口。" },
    FieldSpec { name: "webAccessEnabled", ty: "boolean", required: false, desc: "是否开启 Web 访问。" },
    FieldSpec { name: "webAccessPassword", ty: "string", required: false, desc: "Web 访问口令。" },
    FieldSpec { name: "githubUpdateMethod", ty: "string", required: false, desc: "GitHub 更新方式。" },
    FieldSpec { name: "skippedGithubUpdateVersion", ty: "string", required: false, desc: "用户跳过提醒的版本号。" },
    FieldSpec { name: "recordHotkey", ty: "string", required: false, desc: "录音热键。" },
    FieldSpec { name: "recordBackgroundWakeEnabled", ty: "boolean", required: false, desc: "是否允许后台唤醒录音。" },
    FieldSpec { name: "minRecordSeconds", ty: "integer", required: false, desc: "最短录音秒数。" },
    FieldSpec { name: "maxRecordSeconds", ty: "integer", required: false, desc: "最长录音秒数。" },
    FieldSpec { name: "toolMaxIterations", ty: "integer", required: false, desc: "单轮工具执行上限。" },
    FieldSpec { name: "llmRoundLogCapacity", ty: "integer", required: false, desc: "轮次日志保留条数。" },
    FieldSpec { name: "messageNotificationEnabled", ty: "boolean", required: false, desc: "消息通知开关。" },
    FieldSpec { name: "messageNotificationSoundEnabled", ty: "boolean", required: false, desc: "消息通知声音开关。" },
    FieldSpec { name: "desktopOperationNoticeEnabled", ty: "boolean", required: false, desc: "桌面操作提示开关。" },
    FieldSpec { name: "desktopOperateEnabled", ty: "boolean", required: false, desc: "桌面操作能力开关。" },
    FieldSpec { name: "selectedApiConfigId", ty: "string", required: false, desc: "默认对话模型 id。" },
    FieldSpec { name: "expertApiConfigId", ty: "string", required: false, desc: "专家模型 id（旧键 chatApiConfigId）。" },
    FieldSpec { name: "visionApiConfigId", ty: "string | null", required: false, desc: "视觉模型 id。" },
    FieldSpec { name: "toolReviewApiConfigId", ty: "string | null", required: false, desc: "工具审查模型 id。" },
    FieldSpec { name: "sttApiConfigId", ty: "string | null", required: false, desc: "语音转写模型 id。" },
    FieldSpec { name: "imageGenerationModelId", ty: "string | null", required: false, desc: "图像生成模型 id。" },
    FieldSpec { name: "imageToVideoModelId", ty: "string | null", required: false, desc: "图生视频模型 id。" },
    FieldSpec { name: "sttAutoSend", ty: "boolean", required: false, desc: "转写完成后自动发送。" },
    FieldSpec { name: "terminalShellKind", ty: "string", required: false, desc: "终端 shell 类型。" },
    FieldSpec { name: "simpleSetupMode", ty: "boolean", required: false, desc: "是否精简设置模式。" },
    FieldSpec { name: "searchMode", ty: "string", required: false, desc: "搜索模式。" },
    FieldSpec { name: "shellWorkspaces", ty: "array<table>", required: false, desc: "工作区列表，元素：name / path / level。" },
    FieldSpec { name: "mcpServers", ty: "array<table>", required: false, desc: "MCP 服务列表。" },
    FieldSpec { name: "remoteImChannels", ty: "array<table>", required: false, desc: "远程 IM 渠道列表。" },
    FieldSpec { name: "searchEngines", ty: "array<table>", required: false, desc: "搜索引擎列表。" },
    FieldSpec { name: "providerNonStreamBaseUrls", ty: "array<string>", required: false, desc: "走非流式接口的 base URL 名单。" },
    FieldSpec { name: "apiProviders", ty: "array<table>", required: false, desc: "模型供应商及其模型列表。" },
    FieldSpec { name: "imageProviders", ty: "array<table>", required: false, desc: "图像供应商列表。" },
    FieldSpec { name: "apiConfigs", ty: "array<table>", required: false, desc: "模型配置列表。" },
];

const AGENT_FIELDS: &[FieldSpec] = &[
    FieldSpec { name: "id", ty: "string", required: true, desc: "人格 id，同名互斥。" },
    FieldSpec { name: "name", ty: "string", required: true, desc: "显示名。" },
    FieldSpec { name: "systemPrompt", ty: "string", required: true, desc: "系统提示词。" },
    FieldSpec { name: "tools", ty: "array<table>", required: false, desc: "人格自带工具，元素：id / command / args / enabled / values。" },
    FieldSpec { name: "createdAt", ty: "string", required: false, desc: "创建时间（RFC3339）。" },
    FieldSpec { name: "updatedAt", ty: "string", required: false, desc: "更新时间（RFC3339）。" },
    FieldSpec { name: "avatarPath", ty: "string | null", required: false, desc: "头像路径，相对 app_root（如 avatars/<id>.webp）。" },
    FieldSpec { name: "avatarUpdatedAt", ty: "string | null", required: false, desc: "头像更新时间。" },
    FieldSpec { name: "isBuiltInUser", ty: "boolean", required: false, desc: "是否内置用户人格。" },
    FieldSpec { name: "isBuiltInSystem", ty: "boolean", required: false, desc: "是否内置系统人格。" },
    FieldSpec { name: "privateMemoryEnabled", ty: "boolean", required: false, desc: "是否启用私有记忆。" },
    FieldSpec { name: "memoryRecallMode", ty: "string", required: false, desc: "记忆召回模式。" },
    FieldSpec { name: "scope", ty: "string", required: false, desc: "可见范围。" },
    FieldSpec { name: "summary", ty: "string", required: false, desc: "人格简介。" },
    FieldSpec { name: "residentSkillNames", ty: "array<string>", required: false, desc: "常驻技能：全文注入系统提示词。" },
    FieldSpec { name: "optionalSkillNames", ty: "array<string>", required: false, desc: "可选技能：只注入名字。" },
    FieldSpec { name: "apiConfigIds", ty: "array<string>", required: false, desc: "备选模型 id 列表。" },
    FieldSpec { name: "apiConfigId", ty: "string", required: false, desc: "主模型 id。" },
    FieldSpec { name: "permissionControl", ty: "object | null", required: false, desc: "人格权限：工具 / 技能 / MCP 可见性。" },
    FieldSpec { name: "childAgentIds", ty: "array<string>", required: false, desc: "下级人格 id：组织树父子边，语义为可直接委托。" },
];

const MCP_FILE_FIELDS: &[FieldSpec] = &[FieldSpec {
    name: "mcpServers",
    ty: "object<服务 id, 服务定义>",
    required: true,
    desc: "服务 id 取自这里的键；文件名只是落地名，不必与 id 相同。",
}];

const MCP_FIELDS: &[FieldSpec] = &[
    FieldSpec { name: "command", ty: "string", required: false, desc: "stdio 方式的启动命令。" },
    FieldSpec { name: "args", ty: "array<string>", required: false, desc: "stdio 方式的参数。" },
    FieldSpec { name: "env", ty: "object<string,string>", required: false, desc: "启动时注入的环境变量。" },
    FieldSpec { name: "cwd", ty: "string", required: false, desc: "stdio 方式的工作目录。" },
    FieldSpec { name: "url", ty: "string", required: false, desc: "HTTP / SSE / streamable-http 方式的地址。" },
    FieldSpec { name: "type", ty: "string", required: false, desc: "传输方式，如 streamable-http、sse、stdio。" },
    FieldSpec { name: "transport", ty: "string", required: false, desc: "传输方式的旧写法，与 type 二选一。" },
    FieldSpec { name: "headers", ty: "object<string,string>", required: false, desc: "HTTP 请求头，常用于 Authorization。" },
    FieldSpec { name: "httpHeaders", ty: "object<string,string>", required: false, desc: "headers 的别名写法。" },
    FieldSpec { name: "envHttpHeaders", ty: "object<string,string>", required: false, desc: "从环境变量取值的请求头。" },
    FieldSpec { name: "bearerTokenEnvVar", ty: "string", required: false, desc: "存放 Bearer token 的环境变量名。" },
    FieldSpec { name: "enabledTools", ty: "array<string>", required: false, desc: "只启用这些工具。" },
    FieldSpec { name: "disabledTools", ty: "array<string>", required: false, desc: "排除这些工具。" },
];

const MCP_POLICY_FIELDS: &[FieldSpec] = &[
    FieldSpec { name: "serverId", ty: "string", required: true, desc: "对应服务的 id，取 mcpServers 的键。" },
    FieldSpec { name: "enabled", ty: "boolean", required: true, desc: "是否启用该服务。" },
    FieldSpec { name: "tools", ty: "array<table>", required: false, desc: "工具级策略，元素：toolName / enabled。" },
];

const SKILL_FIELDS: &[FieldSpec] = &[
    FieldSpec { name: "name", ty: "string", required: true, desc: "技能名，与目录名一致。" },
    FieldSpec { name: "description", ty: "string", required: false, desc: "技能说明：模型据此判断何时使用。" },
];

fn render_topic(ctx: &CliContext, request: &TopicRequest) -> Result<String, String> {
    let mut out = String::new();
    match request.topic {
        Topic::App => {
            out.push_str("app_config.toml — 应用主配置：模型供应商、界面、搜索引擎、记忆与工具开关。\n");
            out.push_str(&format!("路径: {}\n", ctx.config_path.display()));
            out.push_str("顶层字段（TOML，camelCase；未列出的键会被忽略）：\n");
            out.push_str(&render_fields(APP_FIELDS));
            out.push_str("注意：文件里是明文密钥。读文件才能拿到值。\n");
        }
        Topic::Agent => {
            out.push_str("agents.json — 人格定义与组织父子边。\n");
            out.push_str(&format!(
                "路径: {}\n",
                ctx.app_root().join("config").join("agents.json").display()
            ));
            out.push_str("顶层结构: { \"agents\": [ <人格>, ... ] }\n");
            out.push_str("人格字段（JSON，camelCase）：\n");
            out.push_str(&render_fields(AGENT_FIELDS));
        }
        Topic::Mcp => {
            let servers_dir = mcp_servers_dir(ctx);
            let policies_dir = mcp_policies_dir(ctx);
            out.push_str("MCP — 每个服务一份定义文件，启停与工具开关落在对应的 policy 文件。\n");
            match request.selectors.first() {
                Some(id) => {
                    let server = mcp_server_file_for_id(ctx, id);
                    out.push_str(&format!(
                        "定义: {}\n",
                        server
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| format!("未找到服务 {id} 的定义文件"))
                    ));
                    out.push_str(&format!(
                        "策略: {}\n",
                        mcp_policy_file_for_id(ctx, id)
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "尚未创建".to_string())
                    ));
                }
                None => {
                    out.push_str(&format!("定义目录: {}\n", servers_dir.display()));
                    out.push_str(&format!("策略目录: {}\n", policies_dir.display()));
                    out.push_str("每个服务一份 .json；服务 id 是 mcpServers 的键，policy 里的 serverId 与它一致。\n");
                }
            }
            out.push_str("服务文件字段（JSON，camelCase）：\n");
            out.push_str(&render_fields(MCP_FILE_FIELDS));
            out.push_str("服务定义本身的字段（mcpServers 里那层）：\n");
            out.push_str(&render_fields(MCP_FIELDS));
            out.push_str("policy 字段（JSON，camelCase）：\n");
            out.push_str(&render_fields(MCP_POLICY_FIELDS));
        }
        Topic::Skill => {
            let skills_dir = effective_workspace_root(ctx).join("skills");
            out.push_str("Skill — 一个目录一个技能，说明在 SKILL.md 的 YAML frontmatter 里。\n");
            match request.selectors.first() {
                Some(name) => {
                    out.push_str(&format!(
                        "路径: {}\n",
                        skills_dir.join(name).join("SKILL.md").display()
                    ));
                }
                None => {
                    out.push_str(&format!("目录: {}\n", skills_dir.join("<name>").join("SKILL.md").display()));
                }
            }
            out.push_str("frontmatter 字段（写在文件开头的 --- 之间）：\n");
            out.push_str(&render_fields(SKILL_FIELDS));
            out.push_str("--- 之后是 markdown 正文，技能的完整说明。\n");
        }
    }
    Ok(out.trim_end().to_string())
}

fn render_fields(fields: &[FieldSpec]) -> String {
    let mut out = format!("  共 {} 个字段\n", fields.len());
    for field in fields {
        let required = if field.required { "必填" } else { "可选" };
        out.push_str(&format!(
            "  {:<34} {:<18} {:<4} {}\n",
            field.name, field.ty, required, field.desc
        ));
    }
    out
}

// ==================== check：校验 ====================

struct CheckReport {
    checked: Vec<String>,
    errors: Vec<String>,
}

impl CheckReport {
    fn new() -> Self {
        Self {
            checked: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn record(&mut self, path: &Path, result: Result<(), String>) {
        match result {
            Ok(()) => self.checked.push(path.display().to_string()),
            Err(err) => self
                .errors
                .push(format!("{}: {err}", path.display())),
        }
    }

    fn into_result(self) -> Result<String, String> {
        if !self.errors.is_empty() {
            let mut message = String::from("校验未通过，配置未进入生效环节：\n");
            message.push_str(&self.errors.join("\n"));
            return Err(message);
        }
        let mut out = String::from("校验通过：\n");
        if self.checked.is_empty() {
            out.push_str("  （没有找到可校验的配置文件）\n");
        } else {
            for path in &self.checked {
                out.push_str(&format!("  {path}\n"));
            }
        }
        Ok(out.trim_end().to_string())
    }
}

fn handle_check(ctx: &CliContext, args: &[String]) -> Result<(), String> {
    let report = if args.is_empty() {
        check_all(ctx)
    } else {
        let requests = parse_topic_flags(args)?;
        let mut report = CheckReport::new();
        for request in &requests {
            match request.topic {
                Topic::App => check_app_config(ctx, &mut report),
                Topic::Agent => check_agents(ctx, &mut report),
                Topic::Mcp => check_mcp(ctx, request.selectors.first().map(String::as_str), &mut report),
                Topic::Skill => check_skills(
                    ctx,
                    request.selectors.first().map(String::as_str),
                    &mut report,
                ),
            }
        }
        report
    };
    push_output(&report.into_result()?);
    Ok(())
}

fn check_all(ctx: &CliContext) -> CheckReport {
    let mut report = CheckReport::new();
    check_app_config(ctx, &mut report);
    check_agents(ctx, &mut report);
    check_mcp(ctx, None, &mut report);
    check_skills(ctx, None, &mut report);
    report
}

fn check_app_config(ctx: &CliContext, report: &mut CheckReport) {
    let path = ctx.config_path.clone();
    if !path.exists() {
        return;
    }
    report.record(&path, validate_app_config_file(&path));
}

fn validate_app_config_file(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|err| format!("读取失败: {err}"))?;
    // 语法层：toml 的报错带行列信息。
    let _syntax: toml::Value = toml::from_str(&text).map_err(|err| format!("TOML 语法错误: {err}"))?;
    // 类型与必填层：与 read_config 的反序列化判定一致。
    let _probe: AppConfigProbe =
        toml::from_str(&text).map_err(|err| format!("字段校验失败: {err}"))?;
    Ok(())
}

fn check_agents(ctx: &CliContext, report: &mut CheckReport) {
    let path = ctx.app_root().join("config").join("agents.json");
    if !path.exists() {
        return;
    }
    report.record(&path, validate_agents_file(&path));
}

fn validate_agents_file(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|err| format!("读取失败: {err}"))?;
    let file: AgentsFile =
        serde_json::from_str(&text).map_err(|err| format!("JSON 解析失败: {err}"))?;
    let mut seen_ids = HashSet::new();
    for (index, agent) in file.agents.iter().enumerate() {
        let label = format!("agents[{index}]");
        if agent.id.trim().is_empty() {
            return Err(format!("{label}.id 不能为空"));
        }
        if !seen_ids.insert(agent.id.clone()) {
            return Err(format!("{label}.id 重复: {}", agent.id));
        }
        if agent.name.trim().is_empty() {
            return Err(format!("{label}.name 不能为空（id: {}）", agent.id));
        }
        if agent.system_prompt.trim().is_empty() {
            return Err(format!("{label}.systemPrompt 不能为空（id: {}）", agent.id));
        }
    }
    for (index, agent) in file.agents.iter().enumerate() {
        for child_id in &agent.child_agent_ids {
            if !seen_ids.contains(child_id) {
                return Err(format!(
                    "agents[{index}].childAgentIds 指向不存在的人格: {child_id}"
                ));
            }
            if child_id == &agent.id {
                return Err(format!(
                    "agents[{index}].childAgentIds 指向自己: {child_id}"
                ));
            }
        }
    }
    Ok(())
}

fn check_mcp(ctx: &CliContext, selector: Option<&str>, report: &mut CheckReport) {
    let servers_dir = mcp_servers_dir(ctx);
    let policies_dir = mcp_policies_dir(ctx);
    let server_paths = match selector {
        Some(id) => match mcp_server_file_for_id(ctx, id) {
            Some(path) => vec![path],
            None => {
                report
                    .errors
                    .push(format!("MCP 服务 {id}: 在 {} 下找不到定义", servers_dir.display()));
                Vec::new()
            }
        },
        None => json_files(&servers_dir).unwrap_or_default(),
    };
    for path in server_paths.iter().filter(|path| path.is_file()) {
        report.record(path, validate_mcp_server_file(path));
    }

    let policy_paths = match selector {
        Some(id) => mcp_policy_file_for_id(ctx, id).into_iter().collect::<Vec<_>>(),
        None => json_files(&policies_dir).unwrap_or_default(),
    };
    for path in policy_paths {
        report.record(&path, validate_mcp_policy_file(&path));
    }
}

/// 服务 id 是 `mcpServers` 的键，文件名只是落地名，所以先按文件名试、再按键找。
fn mcp_server_file_for_id(ctx: &CliContext, id: &str) -> Option<PathBuf> {
    let dir = mcp_servers_dir(ctx);
    let direct = dir.join(format!("{id}.json"));
    if direct.is_file() {
        return Some(direct);
    }
    json_files(&dir).unwrap_or_default().into_iter().find(|path| {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<McpServerFile>(&text).ok())
            .map(|file| file.mcp_servers.keys().any(|key| key == id))
            .unwrap_or(false)
    })
}

fn mcp_policy_file_for_id(ctx: &CliContext, id: &str) -> Option<PathBuf> {
    let dir = mcp_policies_dir(ctx);
    let direct = dir.join(format!("{id}.json"));
    if direct.is_file() {
        return Some(direct);
    }
    json_files(&dir).unwrap_or_default().into_iter().find(|path| {
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<McpPolicyFile>(&text).ok())
            .map(|policy| policy.server_id == id)
            .unwrap_or(false)
    })
}

fn validate_mcp_server_file(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|err| format!("读取失败: {err}"))?;
    let file: McpServerFile =
        serde_json::from_str(&text).map_err(|err| format!("JSON 解析失败: {err}"))?;
    if file.mcp_servers.is_empty() {
        return Err("mcpServers 为空，至少要有一个服务".to_string());
    }
    // 服务 id 取自 mcpServers 的键，文件名只是落地名（应用自己生成的文件两者并不相等），
    // 所以这里只校验每条定义本身，不对文件名做约束。
    for (name, server) in &file.mcp_servers {
        validate_mcp_server(server).map_err(|err| format!("mcpServers.{name}: {err}"))?;
    }
    Ok(())
}

fn validate_mcp_server(server: &McpServerEntry) -> Result<(), String> {
    let has_command = server
        .command
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    let has_url = server
        .url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some();
    if has_command && has_url {
        return Err("command 与 url 只能给一个".to_string());
    }
    if has_command || has_url {
        return Ok(());
    }
    Err("服务至少要给 command（stdio）或 url（http / sse / streamable-http）".to_string())
}

fn validate_mcp_policy_file(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|err| format!("读取失败: {err}"))?;
    let policy: McpPolicyFile =
        serde_json::from_str(&text).map_err(|err| format!("JSON 解析失败: {err}"))?;
    if policy.server_id.trim().is_empty() {
        return Err("policy.serverId 不能为空".to_string());
    }
    let mut seen = HashSet::new();
    for tool in &policy.tools {
        if tool.tool_name.trim().is_empty() {
            return Err("policy.tools[].toolName 不能为空".to_string());
        }
        if !seen.insert(tool.tool_name.clone()) {
            return Err(format!("policy.tools[].toolName 重复: {}", tool.tool_name));
        }
    }
    Ok(())
}

fn check_skills(ctx: &CliContext, selector: Option<&str>, report: &mut CheckReport) {
    let skills_dir = effective_workspace_root(ctx).join("skills");
    let paths = match selector {
        Some(name) => {
            let path = skills_dir.join(name).join("SKILL.md");
            if !path.is_file() {
                report.errors.push(format!("{}: 文件不存在", path.display()));
                return;
            }
            vec![path]
        }
        None => skill_md_files(&skills_dir),
    };
    for path in paths {
        report.record(&path, validate_skill_file(&path));
    }
}

fn skill_md_files(skills_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(skills_dir) else {
        return out;
    };
    for entry in entries.filter_map(Result::ok) {
        let skill_md = entry.path().join("SKILL.md");
        if skill_md.is_file() {
            out.push(skill_md);
        }
    }
    out.sort();
    out
}

fn validate_skill_file(path: &Path) -> Result<(), String> {
    let (name, _description, _body) = parse_skill_frontmatter(path)?;
    if name.trim().is_empty() {
        return Err("frontmatter 缺少 name".to_string());
    }
    Ok(())
}

/// 读 SKILL.md 的 frontmatter，取 name / description 与正文。
/// 本模块是独立编译单元，用不到主程序的解析函数，这里保留最小实现。
fn parse_skill_frontmatter(skill_md_path: &Path) -> Result<(String, String, String), String> {
    let content = fs::read_to_string(skill_md_path)
        .map_err(|err| format!("读取失败: {err}"))?;
    let mut lines = content.lines();
    let first = lines
        .next()
        .unwrap_or_default()
        .trim_start_matches('\u{feff}')
        .trim();
    if first != "---" {
        return Err("SKILL.md 必须以 YAML frontmatter（---）开头".to_string());
    }
    let mut name = String::new();
    let mut description = String::new();
    let mut closed = false;
    for line in lines.by_ref() {
        let trimmed = line.trim();
        if trimmed == "---" {
            closed = true;
            break;
        }
        let Some((key, raw_value)) = trimmed.split_once(':') else {
            continue;
        };
        let mut value = raw_value.trim().to_string();
        if value.len() >= 2 {
            let bytes = value.as_bytes();
            let first_byte = bytes[0];
            let last_byte = bytes[value.len() - 1];
            if (first_byte == b'"' && last_byte == b'"')
                || (first_byte == b'\'' && last_byte == b'\'')
            {
                value = value[1..value.len() - 1].to_string();
            }
        }
        match key.trim() {
            "name" => name = value,
            "description" => description = value,
            _ => {}
        }
    }
    if !closed {
        return Err("frontmatter 没有闭合的 ---".to_string());
    }
    if name.trim().is_empty() {
        name = skill_md_path
            .parent()
            .and_then(|path| path.file_name())
            .and_then(|value| value.to_str())
            .unwrap_or("skill")
            .to_string();
    }
    let body = content
        .split_once("\n---")
        .map(|(_, rest)| rest.trim_start_matches(['\r', '\n']).trim().to_string())
        .unwrap_or_default();
    Ok((name, description, body))
}

// ==================== reload：生效 ====================

/// 先校验再声明生效范围。校验不过直接返回 Err，调用方不会进入重载环节。
fn handle_reload(ctx: &CliContext) -> Result<(), String> {
    let report = check_all(ctx);
    report.into_result()?;
    print_json(&serde_json::json!({
        "type": "workspaceReloadRequest",
        "scope": ["mcp", "skills", "privateAgents", "agents"],
    }))
}

// ==================== 路径解析 ====================

fn app_root_from_cli_data_path(data_path: &Path) -> PathBuf {
    let parent = data_path
        .parent()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| PathBuf::from("."));
    let is_config_dir = parent
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("config"))
        .unwrap_or(false);
    if is_config_dir {
        if let Some(root) = parent.parent() {
            return root.to_path_buf();
        }
    }
    parent
}

fn load_config_doc(ctx: &CliContext) -> Result<toml::Value, String> {
    let text = fs::read_to_string(&ctx.config_path)
        .map_err(|err| format!("读取配置失败 ({}): {err}", ctx.config_path.display()))?;
    toml::from_str(&text).map_err(|err| format!("解析配置失败: {err}"))
}

fn snapshot_from_doc(doc: &toml::Value) -> Result<AppConfigSnapshot, String> {
    doc.clone()
        .try_into::<AppConfigSnapshot>()
        .map_err(|err| format!("配置结构不匹配: {err}"))
}

fn load_snapshot(ctx: &CliContext) -> Result<AppConfigSnapshot, String> {
    snapshot_from_doc(&load_config_doc(ctx)?)
}

fn effective_workspace_root(ctx: &CliContext) -> PathBuf {
    load_snapshot(ctx)
        .ok()
        .and_then(|snapshot| {
            snapshot.shell_workspaces.into_iter().find_map(|workspace| {
                if workspace.level.trim() != "system" {
                    return None;
                }
                let path = workspace.path.trim();
                if path.is_empty() {
                    return None;
                }
                let candidate = PathBuf::from(normalize_path_text(path));
                if candidate.is_absolute() {
                    Some(candidate)
                } else {
                    Some(ctx.workspace_root.join(candidate))
                }
            })
        })
        .unwrap_or_else(|| ctx.workspace_root.clone())
}

fn normalize_path_text(value: &str) -> String {
    let mut out = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    if let Some(rest) = out.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            out = home.join(rest).to_string_lossy().to_string();
        }
    }
    out
}

fn mcp_servers_dir(ctx: &CliContext) -> PathBuf {
    effective_workspace_root(ctx).join("mcp").join("servers")
}

fn mcp_policies_dir(ctx: &CliContext) -> PathBuf {
    effective_workspace_root(ctx).join("mcp").join("policies")
}

fn json_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = fs::read_dir(dir)
        .map_err(|err| format!("读取目录失败 ({}): {err}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.is_file())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

#[allow(dead_code)]
fn detect_portable_runtime_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent()?;
    let marker = exe_dir.join("PORTABLE");
    marker.exists().then(|| exe_dir.join("data"))
}

#[allow(dead_code)]
fn resolve_standard_config_dir() -> Result<PathBuf, String> {
    let dirs = ProjectDirs::from("ai", "easycall", "p-ai")
        .ok_or_else(|| "无法解析应用配置目录".to_string())?;
    Ok(dirs.config_dir().to_path_buf())
}

// ==================== 输出 ====================

fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| format!("序列化输出失败: {err}"))?;
    push_output(&text);
    Ok(())
}

fn clear_output_buffer() {
    OUTPUT_BUFFER.with(|buffer| buffer.borrow_mut().clear());
}

fn take_output_buffer() -> String {
    OUTPUT_BUFFER.with(|buffer| std::mem::take(&mut *buffer.borrow_mut()))
}

fn push_output(text: &str) {
    OUTPUT_BUFFER.with(|buffer| {
        let mut buffer = buffer.borrow_mut();
        if !buffer.is_empty() {
            buffer.push('\n');
        }
        buffer.push_str(text);
    });
}

#[allow(dead_code)]
pub fn split_command_line(command: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::<String>::new();
    let mut current = String::new();
    let mut quote = None::<char>;

    for ch in command.chars() {
        match quote {
            Some(active_quote) if ch == active_quote => {
                quote = None;
            }
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
            }
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if quote.is_some() {
        return Err("config command 解析失败，请检查引号是否闭合".to_string());
    }
    if !current.is_empty() {
        out.push(current);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use uuid::Uuid;

    fn test_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("pai-config-tool-test-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("config")).expect("create config dir");
        fs::create_dir_all(root.join("llm-workspace")).expect("create workspace dir");
        root
    }

    fn run(root: &Path, command: &str) -> Result<String, String> {
        run_command_with_paths(
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            command,
        )
    }

    fn run_portable(root: &Path, command: &str) -> Result<String, String> {
        run_command_with_paths(
            root.join("config").join("app_config.toml"),
            root.join("config").join("config_mark"),
            root.join("config").join("llm-workspace"),
            command,
        )
    }

    fn valid_app_config() -> &'static str {
        "hotkey = \"Alt+·\"\nselectedApiConfigId = \"p::m\"\n\n[[apiProviders]]\nid = \"p\"\n"
    }

    fn seed_valid(root: &Path) {
        fs::write(root.join("app_config.toml"), valid_app_config()).expect("write app config");
        fs::write(
            root.join("config").join("agents.json"),
            "{\"agents\":[{\"id\":\"a\",\"name\":\"A\",\"systemPrompt\":\"p\"}]}",
        )
        .expect("write agents");
    }

    #[test]
    fn directory_map_shows_standard_and_portable_roots() {
        // Windows 标准安装：config_dir 末段就是 config，app_root 上提一级。
        let standard = test_root();
        let output = run_portable(&standard, "help").expect("run help");
        assert!(
            output.contains(&format!("app_root:   {}", standard.display())),
            "{output}"
        );
        assert!(
            output.contains(&format!("config_dir: {}", standard.join("config").display())),
            "{output}"
        );

        // 便携模式：数据根 data/ 下同样是 config/ + llm-workspace/。
        let portable = test_root();
        let output = run_portable(&portable, "help").expect("run help portable");
        assert!(
            output.contains(&format!("app_root:   {}", portable.display())),
            "{output}"
        );
        assert!(
            output.contains(&format!("config_dir: {}", portable.join("config").display())),
            "{output}"
        );

        // Linux / macOS 标准安装：config_dir 就是业务根本身，app_root 与它重合。
        let flat = std::env::temp_dir().join(format!("pai-config-flat-{}", Uuid::new_v4()));
        fs::create_dir_all(&flat).expect("create flat root");
        let output = run(&flat, "help").expect("run help flat");
        assert!(
            output.contains(&format!("app_root:   {}", flat.display())),
            "{output}"
        );
        assert!(
            output.contains(&format!("config_dir: {}", flat.display())),
            "{output}"
        );
    }

    #[test]
    fn directory_map_marks_missing_dirs_and_lists_topics() {
        let root = test_root();
        let output = run(&root, "help").expect("run help");
        assert!(output.contains("config/app_config.toml") || output.contains("app_config.toml"), "{output}");
        // config/ 已建、state/ 未建：只有后者带「尚未创建」。
        assert!(output.contains("agents.json"), "{output}");
        assert!(output.contains("state.sqlite"), "{output}");
        assert!(output.contains("尚未创建"), "{output}");
        assert!(output.contains("-app") && output.contains("-skill"), "{output}");
    }

    #[test]
    fn directory_map_matches_docs_layout_table() {
        let docs = include_str!("../../../../docs/data-schema-v2.md");
        let mut in_section = false;
        let mut documented = BTreeSet::new();
        for line in docs.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with("### 布局总表") {
                in_section = true;
                continue;
            }
            if !in_section {
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            if !line.starts_with('|') {
                break;
            }
            let Some(start) = line.find('`') else { continue };
            let rest = &line[start + 1..];
            let Some(end) = rest.find('`') else { continue };
            let path = rest[..end].trim().trim_end_matches('/').to_string();
            if path.is_empty() || path == "路径" {
                continue;
            }
            documented.insert(path);
        }
        assert!(!documented.is_empty(), "没有解析到布局总表");
        let ours = LAYOUT
            .iter()
            .map(|entry| entry.path.trim_end_matches('/').to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            ours, documented,
            "目录地图与 docs/data-schema-v2.md 的布局总表不一致"
        );
    }

    #[test]
    fn topic_field_lists_match_structs() {
        // agents.json：字段清单与 AgentProfile 的反序列化字段集合一致。
        let probe: AgentProfile =
            serde_json::from_str("{\"id\":\"a\",\"name\":\"A\",\"systemPrompt\":\"p\",\"permissionControl\":{}}")
                .expect("parse agent");
        let keys = serde_json::to_value(&probe)
            .expect("serialize agent")
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        let declared = AGENT_FIELDS
            .iter()
            .map(|field| field.name.to_string())
            .collect::<HashSet<_>>();
        assert_eq!(keys, declared, "agents.json 字段清单与 AgentProfile 不一致");

        // app_config.toml：字段清单与 AppConfigProbe 的字段集合一致。
        let probe: AppConfigProbe =
            toml::from_str("hotkey = \"Alt+·\"").expect("parse app config probe");
        let keys = serde_json::to_value(&probe)
            .expect("serialize app probe")
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        let declared = APP_FIELDS
            .iter()
            .map(|field| field.name.to_string())
            .collect::<HashSet<_>>();
        assert_eq!(keys, declared, "app_config.toml 字段清单与 AppConfigProbe 不一致");

        // MCP：文件外壳与服务定义各一份，policy 一份。
        let keys = serde_json::to_value(McpServerFile::default())
            .expect("serialize mcp file")
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        let declared = MCP_FILE_FIELDS
            .iter()
            .map(|field| field.name.to_string())
            .collect::<HashSet<_>>();
        assert_eq!(keys, declared, "MCP 文件外壳字段清单与 McpServerFile 不一致");

        let keys = serde_json::to_value(McpServerEntry::default())
            .expect("serialize mcp entry")
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        let declared = MCP_FIELDS
            .iter()
            .map(|field| field.name.to_string())
            .collect::<HashSet<_>>();
        assert_eq!(keys, declared, "MCP 服务定义字段清单与 McpServerEntry 不一致");

        let policy = McpPolicyFile {
            server_id: "s".to_string(),
            enabled: true,
            tools: Vec::new(),
        };
        let keys = serde_json::to_value(&policy)
            .expect("serialize policy")
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect::<HashSet<_>>();
        let declared = MCP_POLICY_FIELDS
            .iter()
            .map(|field| field.name.to_string())
            .collect::<HashSet<_>>();
        assert_eq!(keys, declared, "MCP policy 字段清单与 McpPolicyFile 不一致");

        // skill：frontmatter 只认 name / description，与解析实现一致。
        let declared = SKILL_FIELDS
            .iter()
            .map(|field| field.name.to_string())
            .collect::<HashSet<_>>();
        assert_eq!(
            declared,
            HashSet::from(["name".to_string(), "description".to_string()]),
            "skill 字段清单与 frontmatter 解析字段不一致"
        );
    }

    #[test]
    fn app_config_field_list_matches_appconfig_struct() {
        let source = include_str!("../core/domain/types_config.rs");
        let marker = "struct AppConfig {";
        let start = source.find(marker).expect("找不到 struct AppConfig");
        let body = &source[start + marker.len()..];
        let end = body.find("\n}\n").expect("找不到 struct AppConfig 结尾");
        let mut expected = BTreeSet::new();
        for line in body[..end].lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("#[") {
                continue;
            }
            let Some((name, _)) = trimmed.split_once(':') else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() || name.contains(' ') || name.contains('=') {
                continue;
            }
            expected.insert(to_camel_case(name));
        }
        let declared = APP_FIELDS
            .iter()
            .map(|field| field.name.to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            declared, expected,
            "help -app 的字段清单与 AppConfig 的字段集合不一致"
        );
    }

    fn to_camel_case(snake: &str) -> String {
        let mut out = String::new();
        let mut upper_next = false;
        for ch in snake.chars() {
            if ch == '_' {
                upper_next = true;
                continue;
            }
            if upper_next {
                out.extend(ch.to_uppercase());
                upper_next = false;
            } else {
                out.push(ch);
            }
        }
        out
    }

    #[test]
    fn unknown_topic_lists_available_topics() {
        let root = test_root();
        let err = run(&root, "help -foo").expect_err("unknown topic should fail");
        assert!(err.contains("未知主题"), "{err}");
        assert!(err.contains("-app") && err.contains("-agent") && err.contains("-mcp") && err.contains("-skill"), "{err}");
    }

    #[test]
    fn check_reports_syntax_type_and_required_errors() {
        let root = test_root();
        // 一、TOML 语法错：报错带行列。
        fs::write(root.join("app_config.toml"), "hotkey = \"Alt+·\"\nbroken = [\n").expect("write");
        let err = run(&root, "check -app").expect_err("syntax error");
        assert!(err.contains("TOML 语法错误"), "{err}");
        assert!(err.contains("line"), "{err}");

        // 二、字段类型错：能定位到字段名。
        fs::write(
            root.join("app_config.toml"),
            "hotkey = \"Alt+·\"\nwebAccessPort = \"abc\"\n",
        )
        .expect("write");
        let err = run(&root, "check -app").expect_err("type error");
        assert!(err.contains("webAccessPort"), "{err}");

        // 三、缺必填字段：hotkey 没有 default，read_config 会直接 Err。
        fs::write(root.join("app_config.toml"), "uiLanguage = \"zh-CN\"\n").expect("write");
        let err = run(&root, "check -app").expect_err("missing required");
        assert!(err.contains("hotkey"), "{err}");
    }

    #[test]
    fn check_passes_on_valid_files() {
        let root = test_root();
        seed_valid(&root);
        let output = run(&root, "check").expect("check should pass");
        assert!(output.contains("校验通过"), "{output}");
        assert!(output.contains("app_config.toml"), "{output}");
        assert!(output.contains("agents.json"), "{output}");
    }

    #[test]
    fn check_failure_blocks_reload() {
        let root = test_root();
        fs::write(root.join("app_config.toml"), "hotkey = \"Alt+·\"\nbroken = [\n").expect("write");
        let err = run(&root, "reload").expect_err("reload must fail on invalid config");
        assert!(err.contains("校验未通过"), "{err}");
        assert!(!err.contains("workspaceReloadRequest"), "{err}");
    }

    #[test]
    fn reload_scope_covers_mcp_skills_agents() {
        let root = test_root();
        seed_valid(&root);
        let output = run(&root, "reload").expect("reload should pass");
        assert!(output.contains("workspaceReloadRequest"), "{output}");
        assert!(output.contains("\"mcp\"") && output.contains("\"skills\""), "{output}");
        assert!(output.contains("\"privateAgents\"") && output.contains("\"agents\""), "{output}");
    }

    #[test]
    fn check_mcp_resolves_service_id_from_mcp_servers_key() {
        let root = test_root();
        seed_valid(&root);
        let servers = root.join("llm-workspace").join("mcp").join("servers");
        fs::create_dir_all(&servers).expect("create servers dir");
        // 文件名与应用生成的落地名一致，不等于服务 id；id 取 mcpServers 的键。
        fs::write(
            servers.join("mcp-1700000000000.json"),
            "{\"mcpServers\":{\"demo\":{\"type\":\"streamable-http\",\"url\":\"https://example.com/mcp\"}}}",
        )
        .expect("write server");
        let output = run(&root, "check -mcp demo").expect("check by id should pass");
        assert!(output.contains("mcp-1700000000000.json"), "{output}");

        let err = run(&root, "check -mcp nope").expect_err("unknown id");
        assert!(err.contains("找不到定义"), "{err}");
    }

    #[test]
    fn removed_commands_return_migration_hint() {
        let root = test_root();
        seed_valid(&root);
        for command in [
            "agent ls",
            "mcp ls",
            "skill ls",
            "store search pdf",
            "approot",
            "update",
            "delete",
            "enable",
            "disable",
            "new",
        ] {
            let err = run(&root, command).expect_err(command);
            assert!(err.contains("已废除") || err.contains("已从 config 工具面移除"), "{command}: {err}");
            assert!(err.contains("help"), "{command}: {err}");
        }
        let err = run(&root, "wat").expect_err("unknown command");
        assert!(err.contains("未知命令"), "{err}");
    }
}
