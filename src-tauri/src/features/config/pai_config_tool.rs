use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

thread_local! {
    static OUTPUT_BUFFER: RefCell<String> = const { RefCell::new(String::new()) };
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
    #[serde(default = "default_main_source")]
    source: String,
    #[serde(default = "default_global_scope")]
    scope: String,
    /// 人格简介：组织清单里对目标人格的说明。
    #[serde(default)]
    summary: String,
    /// 常驻 skill：说明全文注入该人格系统提示词。
    #[serde(default)]
    resident_skill_names: Vec<String>,
    /// 可选 skill：只注入引用名字。
    #[serde(default)]
    optional_skill_names: Vec<String>,
    /// 人格驱动模型（会话基线，可被会话级首选覆盖）。
    #[serde(default)]
    api_config_ids: Vec<String>,
    #[serde(default)]
    api_config_id: String,
    /// 人格权限：强制约束该人格所有会话的工具/技能/MCP 可见性。
    /// 结构由 GUI/私有组织定义，CLI 只做透传与挂摘，不在这里重复建模。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    permission_control: Option<JsonValue>,
    /// 下级人格 id：组织树的父子边，语义为「可直接委托」。
    #[serde(default)]
    child_agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AgentsFile {
    #[serde(default)]
    agents: Vec<AgentProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrivatePersonaFile {
    id: String,
    name: String,
    #[serde(alias = "prompt", alias = "systemPrompt")]
    system_prompt: String,
    #[serde(default)]
    tools: Vec<ApiToolConfig>,
    #[serde(default)]
    avatar_path: Option<String>,
    // 以下字段由 GUI/私有组织合并流程写入；CLI 保存时仅做透传，避免覆盖已有配置。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    resident_skill_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    optional_skill_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    api_config_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    child_agent_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    permission_control: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentListItem {
    id: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ApiModelConfig {
    id: String,
    model: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    enable_image: bool,
    #[serde(default)]
    enable_audio: bool,
    #[serde(default)]
    enable_video: bool,
    #[serde(default = "default_true")]
    enable_tools: bool,
    #[serde(default = "default_reasoning_effort")]
    reasoning_effort: String,
    #[serde(default = "default_temperature")]
    temperature: f64,
    #[serde(default)]
    custom_temperature_enabled: bool,
    #[serde(default = "default_context_window_tokens")]
    context_window_tokens: u32,
    #[serde(default = "default_max_output_tokens")]
    max_output_tokens: u32,
    #[serde(default)]
    custom_max_output_tokens_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ApiProviderConfig {
    id: String,
    name: String,
    #[serde(default = "default_request_format")]
    request_format: String,
    #[serde(default)]
    allow_concurrent_requests: bool,
    #[serde(default)]
    max_concurrent_requests: Option<u32>,
    #[serde(default = "default_true")]
    enable_text: bool,
    #[serde(default)]
    enable_image: bool,
    #[serde(default)]
    enable_audio: bool,
    #[serde(default)]
    enable_video: bool,
    #[serde(default = "default_true")]
    enable_tools: bool,
    #[serde(default)]
    tools: Vec<ApiToolConfig>,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    codex_auth_mode: String,
    #[serde(default)]
    codex_local_auth_path: String,
    #[serde(default)]
    codex_custom_url: Option<String>,
    #[serde(default)]
    codex_custom_api_key: Option<String>,
    #[serde(default)]
    codex_originator: String,
    #[serde(default)]
    codex_residency_requirement: Option<String>,
    #[serde(default)]
    api_keys: Vec<String>,
    #[serde(default)]
    key_cursor: u32,
    #[serde(default)]
    cached_model_options: Vec<String>,
    #[serde(default)]
    models: Vec<ApiModelConfig>,
    #[serde(default)]
    failure_retry_count: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderListItem {
    id: String,
    name: String,
    request_format: String,
    base_url: String,
    enabled: ProviderCapabilitySummary,
    model_count: usize,
    key_count: usize,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderCapabilitySummary {
    text: bool,
    image: bool,
    audio: bool,
    video: bool,
    tools: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppConfigSnapshot {
    #[serde(default, alias = "chatApiConfigId")]
    selected_api_config_id: String,
    #[serde(default)]
    api_providers: Vec<ApiProviderConfig>,
    #[serde(default)]
    shell_workspaces: Vec<ShellWorkspaceConfigFile>,
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
struct McpServerFile {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone)]
struct McpServerEntry {
    id: String,
    file: McpServerFile,
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

#[derive(Debug, Clone)]
pub(crate) struct CliContext {
    app_root: PathBuf,
    config_path: PathBuf,
    data_path: PathBuf,
    workspace_root: PathBuf,
}

#[allow(dead_code)]
pub fn run_cli(args: &[String]) -> Result<String, String> {
    let ctx = CliContext::detect()?;
    run_with_context(&ctx, args)
}

#[allow(dead_code)]
pub fn run_with_paths(
    app_root: PathBuf,
    config_path: PathBuf,
    data_path: PathBuf,
    workspace_root: PathBuf,
    args: &[String],
) -> Result<String, String> {
    let ctx = CliContext {
        app_root,
        config_path,
        data_path,
        workspace_root,
    };
    run_with_context(&ctx, args)
}

#[allow(dead_code)]
pub fn run_command_with_paths(
    app_root: PathBuf,
    config_path: PathBuf,
    data_path: PathBuf,
    workspace_root: PathBuf,
    command: &str,
) -> Result<String, String> {
    let args = split_command_line(command)?;
    run_with_paths(app_root, config_path, data_path, workspace_root, &args)
}

fn run_with_context(ctx: &CliContext, args: &[String]) -> Result<String, String> {
    clear_output_buffer();
    if args.is_empty() {
        print_help()?;
        return Ok(take_output_buffer());
    }
    match args[0].as_str() {
        "agent" => handle_agent(ctx, &args[1..]),
        "mcp" => handle_mcp(ctx, &args[1..]),
        "skill" => handle_skill(ctx, &args[1..]),
        "approot" => handle_approot(ctx),
        "help" | "--help" | "-h" => print_help(),
        "provider" => Err("provider 命令当前未开放，请不要通过 config 工具修改供应商。".to_string()),
        other => Err(format!("未知顶级命令: {other}")),
    }?;
    Ok(take_output_buffer())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkillListItem {
    name: String,
    description: String,
    enabled: bool,
    path: String,
}

/// skill 组只开放最小面：列出现有技能供挂摘时选择（结论 22）。
fn handle_skill(ctx: &CliContext, args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(String::as_str).unwrap_or("ls");
    match cmd {
        "ls" | "" => {
            let items = skill_list_items(ctx)?;
            print_json(&items)
        }
        _ => Err("用法: skill ls".to_string()),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppRootInfo {
    app_root: String,
    config_path: String,
    workspace_root: String,
}

/// 输出应用数据目录。安装版与便携版的落盘位置没有统一规律，只能由应用自身报出。
fn handle_approot(ctx: &CliContext) -> Result<(), String> {
    print_json(&AppRootInfo {
        app_root: app_root_from_cli_data_path(&ctx.data_path)
            .display()
            .to_string(),
        config_path: ctx.config_path.display().to_string(),
        workspace_root: effective_workspace_root(ctx).display().to_string(),
    })
}

fn skill_list_items(ctx: &CliContext) -> Result<Vec<SkillListItem>, String> {
    let workspace_root = effective_workspace_root(ctx);
    let skills_dir = workspace_root.join("skills");
    if !skills_dir.exists() {
        return Ok(Vec::new());
    }
    let enabled_map = load_cli_skill_enabled_map(&workspace_root);
    let mut dirs = fs::read_dir(&skills_dir)
        .map_err(|err| format!("读取技能目录失败 ({}): {err}", skills_dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    let mut items = Vec::new();
    for dir in dirs {
        let skill_md = dir.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let Ok((name, description, _)) = parse_skill_frontmatter(&skill_md) else {
            continue;
        };
        let enabled = enabled_map
            .iter()
            .find(|(key, _)| key == &name)
            .map(|(_, value)| *value)
            .unwrap_or(true);
        items.push(SkillListItem {
            name,
            description,
            enabled,
            path: skill_md.to_string_lossy().to_string(),
        });
    }
    Ok(items)
}

fn load_cli_skill_enabled_map(workspace_root: &Path) -> Vec<(String, bool)> {
    let dir = workspace_root.join("skill-policies");
    let mut map = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return map;
    };
    for path in entries.filter_map(|entry| entry.ok().map(|entry| entry.path())) {
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(file) = read_json_file::<SkillPolicyEntry>(&path) else {
            continue;
        };
        let name = file.skill_name.trim().to_string();
        if !name.is_empty() {
            map.push((name, file.enabled));
        }
    }
    map
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SkillPolicyEntry {
    #[serde(default)]
    skill_name: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

/// 读 SKILL.md 的 frontmatter，取 name 与 description。
/// config 工具是独立 crate 模块，不能用主程序的解析函数，这里保留最小实现。
fn parse_skill_frontmatter(skill_md_path: &Path) -> Result<(String, String, String), String> {
    let content = fs::read_to_string(skill_md_path)
        .map_err(|err| format!("读取 SKILL.md 失败 ({}): {err}", skill_md_path.display()))?;
    let mut lines = content.lines();
    let first = lines
        .next()
        .unwrap_or_default()
        .trim_start_matches('\u{feff}')
        .trim();
    if first != "---" {
        return Err("SKILL.md 必须以 YAML frontmatter 开头".to_string());
    }
    let mut name = String::new();
    let mut description = String::new();
    for line in lines.by_ref() {
        let trimmed = line.trim();
        if trimmed == "---" {
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
            if (first_byte == b'"' && last_byte == b'"') || (first_byte == b'\'' && last_byte == b'\'') {
                value = value[1..value.len() - 1].to_string();
            }
        }
        match key.trim() {
            "name" => name = value,
            "description" => description = value,
            _ => {}
        }
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

/// 内置人格 id 名单：这些人格的常驻/可选 skill 引用锁定（结论 25）。
const CLI_BUILT_IN_ORGANIZATION_AGENT_IDS: [&str; 5] = [
    "default-agent",
    "deputy-agent",
    "reviewer",
    "saddler",
    "support",
];

fn is_built_in_organization_agent_id(agent_id: &str) -> bool {
    CLI_BUILT_IN_ORGANIZATION_AGENT_IDS.contains(&agent_id.trim())
}

/// 模型角色端点：与主程序 `MODEL_ROLE_*` 常量同值，config 工具是独立 crate 模块，这里保留本地副本。
const CLI_MODEL_ROLE_EXPERT_ENDPOINT: &str = "role:expert";
const CLI_MODEL_ROLE_QUICK_ENDPOINT: &str = "role:quick";

impl CliContext {
    #[allow(dead_code)]
    fn detect() -> Result<Self, String> {
        if let Ok(root) = std::env::var("PAI_APP_ROOT") {
            let app_root = PathBuf::from(root);
            return Ok(Self {
                config_path: app_root.join("app_config.toml"),
                data_path: app_root.join("config_mark"),
                workspace_root: app_root.join("llm-workspace"),
                app_root,
            });
        }

        if let Some(portable_root) = detect_portable_runtime_root() {
            let config_dir = portable_root.join("config");
            return Ok(Self {
                config_path: config_dir.join("app_config.toml"),
                data_path: config_dir.join("config_mark"),
                workspace_root: portable_root.join("llm-workspace"),
                app_root: portable_root,
            });
        }

        let config_dir = resolve_standard_config_dir()?;
        let app_root = config_dir.clone();
        Ok(Self {
            config_path: config_dir.join("app_config.toml"),
            data_path: config_dir.join("config_mark"),
            workspace_root: app_root.join("llm-workspace"),
            app_root,
        })
    }
}

fn print_help() -> Result<(), String> {
    push_output(
        r#"PAI config

Description:
  This is the PAI configuration tool for LLM agents.
  Use it when the user asks to modify PAI settings, including agents/personas, agent tree, or MCP.

Usage:
  config "<command>"

Rules:
  - Start with config "help" when you need the command guide.
  - One tool call executes one command.
  - Use ls/get/example before writing when you do not know the current shape.
  - Use check to verify an edited file, diff to preview the plan, then update to apply it.
  - agent new / mcp add create and persist immediately.
  - Do not edit PAI config source files directly.
  - Delete commands are destructive and must only be used after the user explicitly agrees.
  - Delete commands require --confirmed, for example: mcp delete playwright --confirmed.
  - If an argument contains spaces, quote it.
  - When no config command covers the case, run config "approot" first, then edit files under that data directory by hand.

Agent/persona:
  agent ls
  agent get <name-or-id>
  agent tree
  agent example
  agent new <name> <persona> [<avatar-file>]
  agent export <name-or-id> <file>
  agent check <file>
  agent diff <name-or-id> <file>
  agent update <name-or-id> <file>
  agent avatar <name-or-id> <image-file>

Agent organization (tree edges are edited here, not through files):
  agent parent <name-or-id>
  agent children <name-or-id>
  agent set-parent <child> <parent> [<parent>...]
  agent clear-parent <child>

Agent model and skills:
  agent set-model-class <name-or-id> <expert|fast>
  agent set-provider <name-or-id> <provider-id>
  agent set-model <name-or-id> <model>
  agent set-resident <name-or-id> <skill-name> [<skill-name>...]
  agent unset-resident <name-or-id> <skill-name> [<skill-name>...]
  agent set-optional <name-or-id> <skill-name> [<skill-name>...]
  agent unset-optional <name-or-id> <skill-name> [<skill-name>...>

Skill:
  skill ls

Location:
  approot  # data directory: config, state, conversations, workspace

MCP:
  mcp ls
  mcp get <name-or-id>
  mcp example
  mcp add <name> -- <command> [args...]
  mcp new <name>
  mcp export <name-or-id> <file>
  mcp check <file>
  mcp diff <name-or-id> <file>
  mcp update <name-or-id> <file>
  mcp enable <name-or-id>
  mcp disable <name-or-id>
  mcp delete <name-or-id> --confirmed
  mcp test <name-or-id>
  mcp tools <name-or-id>

Common flows:
  config "agent get <name-or-id>"
  config "agent export <name-or-id> <file>"
  edit the exported JSON with file tools
  config "agent check <file>"
  config "agent diff <name-or-id> <file>"
  config "agent update <name-or-id> <file>"

  config "mcp add playwright -- npx @playwright/mcp@latest"
  config "mcp enable playwright""#
    );
    Ok(())
}

fn handle_agent(ctx: &CliContext, args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    match cmd {
        "ls" => {
            let agents = visible_agents(&load_agents(ctx)?);
            let items = agents.iter().map(agent_list_item).collect::<Vec<_>>();
            print_json(&items)
        }
        "get" => {
            let selector = required_arg(args, 1, "agent get <name-or-id>")?;
            let agents = visible_agents(&load_agents(ctx)?);
            let agent = find_agent(&agents, selector)?;
            print_json(agent)
        }
        "tree" => {
            let agents = load_agents(ctx)?;
            print_json(&build_agent_tree(&agents))
        }
        "example" => print_json(&agent_example()),
        "new" => {
            let name = required_arg(args, 1, "agent new <name> <persona> [<avatar-file>]")?;
            let persona = required_arg(args, 2, "agent new <name> <persona> [<avatar-file>]")?;
            let avatar = args.get(3).cloned();
            let mut agents = load_main_agents(ctx)?;
            let existing_agents = load_agents(ctx)?;
            let mut next = build_new_agent(name, persona, avatar);
            next.id = unique_slugified_id(
                name,
                "agent",
                existing_agents.iter().map(|item| item.id.as_str()),
            );
            validate_agent(&next)?;
            agents.push(next.clone());
            save_main_agents(ctx, &agents)?;
            print_json(&next)
        }
        "export" => {
            let selector = required_arg(args, 1, "agent export <name-or-id> <file>")?;
            let file = required_arg(args, 2, "agent export <name-or-id> <file>")?;
            let agents = visible_agents(&load_agents(ctx)?);
            let agent = find_agent(&agents, selector)?;
            write_json_file(Path::new(file), agent)?;
            print_output_path(file)
        }
        "check" => {
            let file = required_arg(args, 1, "agent check <file>")?;
            let agent = read_json_file::<AgentProfile>(Path::new(file))?;
            validate_agent(&agent)?;
            print_ok_preview(&agent)
        }
        "diff" => {
            let selector = required_arg(args, 1, "agent diff <name-or-id> <file>")?;
            let file = required_arg(args, 2, "agent diff <name-or-id> <file>")?;
            let agents = visible_agents(&load_agents(ctx)?);
            let current = find_agent(&agents, selector)?;
            let next = read_json_file::<AgentProfile>(Path::new(file))?;
            validate_agent(&next)?;
            print_json(&build_named_diff(current, &next))
        }
        "update" => {
            let selector = required_arg(args, 1, "agent update <name-or-id> <file>")?;
            let file = required_arg(args, 2, "agent update <name-or-id> <file>")?;
            let mut agents = load_agents(ctx)?;
            let idx = find_agent_index(&agents, selector)?;
            ensure_agent_writable(&agents[idx])?;
            let mut next = read_json_file::<AgentProfile>(Path::new(file))?;
            validate_agent(&next)?;
            next.id = agents[idx].id.clone();
            next.created_at = keep_or_now(&agents[idx].created_at);
            next.updated_at = now_iso();
            next.source = agents[idx].source.clone();
            next.scope = agents[idx].scope.clone();
            // 组织边只走树上的即时命令，update 不改边。
            next.child_agent_ids = agents[idx].child_agent_ids.clone();
            // 内置人格的常驻/可选引用锁定，update 不得改动。
            if is_built_in_organization_agent_id(&agents[idx].id) {
                next.resident_skill_names = agents[idx].resident_skill_names.clone();
                next.optional_skill_names = agents[idx].optional_skill_names.clone();
            }
            agents[idx] = next.clone();
            save_agent(ctx, &next)?;
            print_json(&next)
        }
        "avatar" => {
            let selector = required_arg(args, 1, "agent avatar <name-or-id> <image-file>")?;
            let image = required_arg(args, 2, "agent avatar <name-or-id> <image-file>")?;
            let mut agents = load_agents(ctx)?;
            let idx = find_agent_index(&agents, selector)?;
            ensure_agent_writable(&agents[idx])?;
            let avatar_path = save_avatar_file(ctx, &agents[idx].id, Path::new(image))?;
            let avatar_file_name = avatar_path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| "头像文件名无效".to_string())?;
            // 与运行态保持一致：存相对数据根的路径。
            agents[idx].avatar_path = Some(format!("avatars/{avatar_file_name}"));
            agents[idx].avatar_updated_at = Some(now_iso());
            agents[idx].updated_at = now_iso();
            save_agent(ctx, &agents[idx])?;
            print_json(&agents[idx])
        }
        "parent" => {
            let selector = required_arg(args, 1, "agent parent <name-or-id>")?;
            let agents = visible_agents(&load_agents(ctx)?);
            let agent = find_agent(&agents, selector)?;
            print_json(&agent_parent_items(&agents, &agent.id))
        }
        "children" => {
            let selector = required_arg(args, 1, "agent children <name-or-id>")?;
            let agents = visible_agents(&load_agents(ctx)?);
            let agent = find_agent(&agents, selector)?;
            print_json(&agent_child_items(&agents, agent))
        }
        "set-parent" => {
            let usage = "agent set-parent <child> <parent> [<parent>...]";
            let child = required_arg(args, 1, usage)?;
            let parent_selectors = args.get(2..).unwrap_or_default();
            if parent_selectors.is_empty() {
                return Err(format!("用法: {usage}"));
            }
            let mut agents = load_agents(ctx)?;
            let child_idx = find_agent_index(&agents, child)?;
            ensure_agent_writable(&agents[child_idx])?;
            let child_id = agents[child_idx].id.clone();
            let mut parent_ids = Vec::<String>::new();
            for selector in parent_selectors {
                let parent_idx = find_agent_index(&agents, selector)?;
                let parent_id = agents[parent_idx].id.clone();
                if parent_id == child_id {
                    return Err(format!("组织树不能自指: {child_id}"));
                }
                if !parent_ids.contains(&parent_id) {
                    parent_ids.push(parent_id);
                }
            }
            // set 语义：父集合整体覆盖，先摘掉这个人格在所有父上的旧边。
            for agent in agents.iter_mut() {
                agent.child_agent_ids.retain(|id| id.trim() != child_id);
            }
            let mut changed = Vec::<AgentProfile>::new();
            for parent_id in &parent_ids {
                let idx = find_agent_index(&agents, parent_id)?;
                if !agents[idx].child_agent_ids.iter().any(|id| id.trim() == child_id) {
                    agents[idx].child_agent_ids.push(child_id.clone());
                    agents[idx].updated_at = now_iso();
                    changed.push(agents[idx].clone());
                }
            }
            for agent in &changed {
                save_agent(ctx, agent)?;
            }
            print_json(&serde_json::json!({ "child": child_id, "parents": parent_ids }))
        }
        "clear-parent" => {
            let usage = "agent clear-parent <child>";
            let child = required_arg(args, 1, usage)?;
            let mut agents = load_agents(ctx)?;
            let child_idx = find_agent_index(&agents, child)?;
            ensure_agent_writable(&agents[child_idx])?;
            let child_id = agents[child_idx].id.clone();
            let mut changed = Vec::<AgentProfile>::new();
            for agent in agents.iter_mut() {
                let before = agent.child_agent_ids.len();
                agent.child_agent_ids.retain(|id| id.trim() != child_id);
                if agent.child_agent_ids.len() != before {
                    agent.updated_at = now_iso();
                    changed.push(agent.clone());
                }
            }
            for agent in &changed {
                save_agent(ctx, agent)?;
            }
            print_json(&serde_json::json!({ "child": child_id, "parents": Vec::<String>::new() }))
        }
        "set-model-class" => {
            let usage = "agent set-model-class <name-or-id> <expert|fast>";
            let selector = required_arg(args, 1, usage)?;
            let model_class = required_arg(args, 2, usage)?;
            let endpoint = match model_class {
                "expert" => CLI_MODEL_ROLE_EXPERT_ENDPOINT.to_string(),
                "fast" => CLI_MODEL_ROLE_QUICK_ENDPOINT.to_string(),
                _ => return Err("model-class 只能是 expert 或 fast".to_string()),
            };
            mutate_agent(ctx, selector, |agent, _| {
                agent.api_config_ids = vec![endpoint.clone()];
                agent.api_config_id = endpoint.clone();
                Ok(())
            })
        }
        "set-provider" => {
            let usage = "agent set-provider <name-or-id> <provider-id>";
            let selector = required_arg(args, 1, usage)?;
            let provider_selector = required_arg(args, 2, usage)?;
            mutate_agent(ctx, selector, |agent, snapshot| {
                let provider = snapshot
                    .api_providers
                    .iter()
                    .find(|item| matches_selector(&item.id, &item.name, provider_selector))
                    .ok_or_else(|| format!("provider not found: {provider_selector}"))?;
                let endpoint = provider_first_endpoint(provider)
                    .ok_or_else(|| format!("provider has no models: {provider_selector}"))?;
                agent.api_config_ids = vec![endpoint.clone()];
                agent.api_config_id = endpoint;
                Ok(())
            })
        }
        "set-model" => {
            let usage = "agent set-model <name-or-id> <model>";
            let selector = required_arg(args, 1, usage)?;
            let model = required_arg(args, 2, usage)?;
            mutate_agent(ctx, selector, |agent, snapshot| {
                let current = agent_primary_endpoint(agent);
                let (provider_id, _) = split_endpoint_id(&current)
                    .ok_or_else(|| "当前人格没有可用 provider，先执行 agent set-provider".to_string())?;
                let provider = snapshot
                    .api_providers
                    .iter()
                    .find(|item| item.id == provider_id)
                    .ok_or_else(|| format!("provider not found: {provider_id}"))?;
                let endpoint = provider_model_endpoint(provider, model)
                    .ok_or_else(|| format!("model not found under provider {provider_id}: {model}"))?;
                agent.api_config_ids = vec![endpoint.clone()];
                agent.api_config_id = endpoint;
                Ok(())
            })
        }
        "set-resident" => {
            let usage = "agent set-resident <name-or-id> <skill-name> [<skill-name>...]";
            let selector = required_arg(args, 1, usage)?;
            let skills = args.get(2..).unwrap_or_default().to_vec();
            if skills.is_empty() {
                return Err(format!("用法: {usage}"));
            }
            mutate_agent_skills(ctx, selector, |agent, _| {
                for skill in &skills {
                    let skill = skill.trim();
                    if skill.is_empty() {
                        continue;
                    }
                    if !agent.resident_skill_names.iter().any(|item| item == skill) {
                        agent.resident_skill_names.push(skill.to_string());
                    }
                    // 常驻与可选互斥：同一个 skill 不能同时挂两份。
                    agent.optional_skill_names.retain(|item| item != skill);
                }
                Ok(())
            })
        }
        "unset-resident" => {
            let usage = "agent unset-resident <name-or-id> <skill-name> [<skill-name>...]";
            let selector = required_arg(args, 1, usage)?;
            let skills = args.get(2..).unwrap_or_default().to_vec();
            if skills.is_empty() {
                return Err(format!("用法: {usage}"));
            }
            mutate_agent_skills(ctx, selector, |agent, _| {
                agent
                    .resident_skill_names
                    .retain(|item| !skills.iter().any(|skill| skill.trim() == item.trim()));
                Ok(())
            })
        }
        "set-optional" => {
            let usage = "agent set-optional <name-or-id> <skill-name> [<skill-name>...]";
            let selector = required_arg(args, 1, usage)?;
            let skills = args.get(2..).unwrap_or_default().to_vec();
            if skills.is_empty() {
                return Err(format!("用法: {usage}"));
            }
            mutate_agent_skills(ctx, selector, |agent, _| {
                for skill in &skills {
                    let skill = skill.trim();
                    if skill.is_empty() {
                        continue;
                    }
                    if !agent.optional_skill_names.iter().any(|item| item == skill) {
                        agent.optional_skill_names.push(skill.to_string());
                    }
                    agent.resident_skill_names.retain(|item| item != skill);
                }
                Ok(())
            })
        }
        "unset-optional" => {
            let usage = "agent unset-optional <name-or-id> <skill-name> [<skill-name>...]";
            let selector = required_arg(args, 1, usage)?;
            let skills = args.get(2..).unwrap_or_default().to_vec();
            if skills.is_empty() {
                return Err(format!("用法: {usage}"));
            }
            mutate_agent_skills(ctx, selector, |agent, _| {
                agent
                    .optional_skill_names
                    .retain(|item| !skills.iter().any(|skill| skill.trim() == item.trim()));
                Ok(())
            })
        }
        _ => Err(
            "用法: agent ls|get|tree|example|new|export|check|diff|update|avatar|parent|children|set-parent|clear-parent|set-model-class|set-provider|set-model|set-resident|unset-resident|set-optional|unset-optional"
                .to_string(),
        ),
    }
}

/// 组织树上的父人格：所有把该人格列进 `childAgentIds` 的人格。
fn agent_parent_items(agents: &[AgentProfile], child_id: &str) -> Vec<AgentListItem> {
    let mut parents = agents
        .iter()
        .filter(|agent| {
            agent.id.trim() != child_id
                && agent
                    .child_agent_ids
                    .iter()
                    .any(|id| id.trim() == child_id)
        })
        .map(agent_list_item)
        .collect::<Vec<_>>();
    parents.sort_by(|a, b| a.id.cmp(&b.id));
    parents
}

/// 组织树上的直接下级人格。
fn agent_child_items(agents: &[AgentProfile], agent: &AgentProfile) -> Vec<AgentListItem> {
    let mut children = agent
        .child_agent_ids
        .iter()
        .filter_map(|child_id| {
            agents
                .iter()
                .find(|item| item.id.trim() == child_id.trim())
                .map(agent_list_item)
        })
        .collect::<Vec<_>>();
    children.sort_by(|a, b| a.id.cmp(&b.id));
    children
}

fn agent_primary_endpoint(agent: &AgentProfile) -> String {
    agent
        .api_config_ids
        .first()
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| agent.api_config_id.clone())
}

/// 读改写单个人格：先校验可写，再落盘，最后打印新值。
fn mutate_agent(
    ctx: &CliContext,
    selector: &str,
    mutate: impl FnOnce(&mut AgentProfile, &AppConfigSnapshot) -> Result<(), String>,
) -> Result<(), String> {
    mutate_agent_with_check(ctx, selector, false, mutate)
}

/// 只改两个 skill 列表时额外校验：内置人格的常驻/可选引用锁定。
fn mutate_agent_skills(
    ctx: &CliContext,
    selector: &str,
    mutate: impl FnOnce(&mut AgentProfile, &AppConfigSnapshot) -> Result<(), String>,
) -> Result<(), String> {
    mutate_agent_with_check(ctx, selector, true, mutate)
}

fn mutate_agent_with_check(
    ctx: &CliContext,
    selector: &str,
    lock_skills: bool,
    mutate: impl FnOnce(&mut AgentProfile, &AppConfigSnapshot) -> Result<(), String>,
) -> Result<(), String> {
    let mut agents = load_agents(ctx)?;
    let idx = find_agent_index(&agents, selector)?;
    ensure_agent_writable(&agents[idx])?;
    if lock_skills {
        ensure_agent_skill_editable(&agents[idx])?;
    }
    let snapshot = load_snapshot(ctx)?;
    mutate(&mut agents[idx], &snapshot)?;
    agents[idx].updated_at = now_iso();
    save_agent(ctx, &agents[idx])?;
    print_json(&agents[idx])
}

fn provider_first_endpoint(provider: &ApiProviderConfig) -> Option<String> {
    provider
        .models
        .first()
        .map(|model| format!("{}::{}", provider.id, model.id))
}

fn provider_model_endpoint(provider: &ApiProviderConfig, model_selector: &str) -> Option<String> {
    provider.models.iter().find_map(|model| {
        if model.id.eq_ignore_ascii_case(model_selector)
            || model.model.eq_ignore_ascii_case(model_selector)
        {
            Some(format!("{}::{}", provider.id, model.id))
        } else {
            None
        }
    })
}

fn split_endpoint_id(value: &str) -> Option<(String, String)> {
    let (provider_id, model_id) = value.split_once("::")?;
    let provider_id = provider_id.trim();
    let model_id = model_id.trim();
    if provider_id.is_empty() || model_id.is_empty() {
        None
    } else {
        Some((provider_id.to_string(), model_id.to_string()))
    }
}

fn build_agent_tree(agents: &[AgentProfile]) -> JsonValue {
    use std::collections::{HashMap, HashSet};
    let by_id = agents
        .iter()
        .filter(|agent| !agent.is_built_in_user)
        .map(|agent| (agent.id.trim().to_string(), agent.clone()))
        .collect::<HashMap<String, AgentProfile>>();
    let mut child_ids = HashSet::<String>::new();
    for agent in by_id.values() {
        for child in &agent.child_agent_ids {
            let child = child.trim();
            if by_id.contains_key(child) && child != agent.id.trim() {
                child_ids.insert(child.to_string());
            }
        }
    }
    let mut roots = by_id
        .values()
        .filter(|agent| !child_ids.contains(agent.id.trim()))
        .map(|agent| agent.id.trim().to_string())
        .collect::<Vec<_>>();
    roots.sort();
    let mut visited = HashSet::<String>::new();
    let nodes = roots
        .iter()
        .map(|id| agent_tree_node(&by_id, id, &mut visited))
        .collect::<Vec<_>>();
    serde_json::json!({ "roots": nodes })
}

fn agent_tree_node(
    by_id: &std::collections::HashMap<String, AgentProfile>,
    id: &str,
    visited: &mut std::collections::HashSet<String>,
) -> JsonValue {
    let Some(agent) = by_id.get(id) else {
        return serde_json::json!({ "id": id });
    };
    if !visited.insert(id.to_string()) {
        return serde_json::json!({ "id": id, "cycle": true });
    }
    let mut children = agent
        .child_agent_ids
        .iter()
        .map(|child| child.trim().to_string())
        .filter(|child| !child.is_empty() && child != id && by_id.contains_key(child))
        .collect::<Vec<_>>();
    children.sort();
    children.dedup();
    let child_nodes = children
        .iter()
        .map(|child| agent_tree_node(by_id, child, visited))
        .collect::<Vec<_>>();
    serde_json::json!({
        "id": agent.id.trim(),
        "name": agent.name.trim(),
        "children": child_nodes,
    })
}

#[allow(dead_code)]
fn handle_provider(ctx: &CliContext, args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    match cmd {
        "ls" => {
            let snapshot = load_snapshot(ctx)?;
            let items = snapshot
                .api_providers
                .iter()
                .map(provider_list_item)
                .collect::<Vec<_>>();
            print_json(&items)
        }
        "get" => {
            let selector = required_arg(args, 1, "provider get <name-or-id>")?;
            let snapshot = load_snapshot(ctx)?;
            let provider = find_provider(&snapshot.api_providers, selector)?;
            print_json(&redacted_provider(provider))
        }
        "example" => print_json(&provider_example()),
        "new" => {
            let name = required_arg(args, 1, "provider new <name>")?;
            print_json(&build_new_provider(name))
        }
        "export" => {
            let selector = required_arg(args, 1, "provider export <name-or-id> <file>")?;
            let file = required_arg(args, 2, "provider export <name-or-id> <file>")?;
            let snapshot = load_snapshot(ctx)?;
            let provider = find_provider(&snapshot.api_providers, selector)?;
            write_json_file(Path::new(file), &redacted_provider(provider))?;
            print_output_path(file)
        }
        "check" => {
            let file = required_arg(args, 1, "provider check <file>")?;
            let provider = read_json_file::<ApiProviderConfig>(Path::new(file))?;
            validate_provider(&provider)?;
            print_ok_preview(&redacted_provider(&provider))
        }
        "diff" => {
            let selector = required_arg(args, 1, "provider diff <name-or-id> <file>")?;
            let file = required_arg(args, 2, "provider diff <name-or-id> <file>")?;
            let snapshot = load_snapshot(ctx)?;
            let current = find_provider(&snapshot.api_providers, selector)?;
            let mut next = read_json_file::<ApiProviderConfig>(Path::new(file))?;
            restore_redacted_provider_secrets(&mut next, current);
            validate_provider(&next)?;
            print_json(&build_named_diff(current, &next))
        }
        "update" => {
            let selector = required_arg(args, 1, "provider update <name-or-id> <file>")?;
            let file = required_arg(args, 2, "provider update <name-or-id> <file>")?;
            let mut doc = load_config_doc(ctx)?;
            let mut snapshot = snapshot_from_doc(&doc)?;
            let idx = find_provider_index(&snapshot.api_providers, selector)?;
            let mut next = read_json_file::<ApiProviderConfig>(Path::new(file))?;
            restore_redacted_provider_secrets(&mut next, &snapshot.api_providers[idx]);
            validate_provider(&next)?;
            next.id = snapshot.api_providers[idx].id.clone();
            snapshot.api_providers[idx] = next.clone();
            write_providers_to_doc(&mut doc, &snapshot.api_providers)?;
            save_config_doc(ctx, &doc)?;
            print_json(&redacted_provider(&next))
        }
        "test" => {
            let selector = required_arg(args, 1, "provider test <name-or-id>")?;
            let snapshot = load_snapshot(ctx)?;
            let provider = find_provider(&snapshot.api_providers, selector)?;
            validate_provider(provider)?;
            print_json(&serde_json::json!({
                "ok": true,
                "message": "provider config is structurally valid",
                "diagnostics": {
                    "providerId": provider.id,
                    "modelCount": provider.models.len(),
                    "baseUrl": provider.base_url
                }
            }))
        }
        "models" => {
            let selector = required_arg(args, 1, "provider models <name-or-id>")?;
            let snapshot = load_snapshot(ctx)?;
            let provider = find_provider(&snapshot.api_providers, selector)?;
            print_json(&provider.models)
        }
        _ => Err("用法: provider ls|get|example|new|export|check|diff|update|test|models".to_string()),
    }
}

fn handle_mcp(ctx: &CliContext, args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    match cmd {
        "ls" => {
            let servers = list_mcp_servers(ctx)?;
            print_json(&servers)
        }
        "get" => {
            let selector = required_arg(args, 1, "mcp get <name-or-id>")?;
            let server = find_mcp_server(ctx, selector)?;
            print_json(&server)
        }
        "example" => print_json(&mcp_example()),
        "add" => {
            let name = required_arg(args, 1, "mcp add <name> -- <command> [args...]")?;
            let sep = args
                .iter()
                .position(|item| item == "--")
                .ok_or_else(|| "mcp add 需要使用 -- 分隔命令".to_string())?;
            if sep + 1 >= args.len() {
                return Err("mcp add 需要提供 command".to_string());
            }
            let command = &args[sep + 1];
            let command_args = args[sep + 2..].to_vec();
            let file = McpServerFile {
                name: Some(name.to_string()),
                transport: Some("stdio".to_string()),
                command: Some(command.to_string()),
                args: Some(command_args),
                url: None,
                env: None,
            };
            validate_mcp_server(&file)?;
            save_mcp_server(ctx, name, &file)?;
            ensure_mcp_policy(ctx, name, false)?;
            print_json(&file)
        }
        "new" => {
            let name = required_arg(args, 1, "mcp new <name>")?;
            let mut sample = mcp_example();
            sample.name = Some(name.to_string());
            print_json(&sample)
        }
        "export" => {
            let selector = required_arg(args, 1, "mcp export <name-or-id> <file>")?;
            let file = required_arg(args, 2, "mcp export <name-or-id> <file>")?;
            let server = find_mcp_server(ctx, selector)?;
            write_json_file(Path::new(file), &server)?;
            print_output_path(file)
        }
        "check" => {
            let file = required_arg(args, 1, "mcp check <file>")?;
            let server = read_json_file::<McpServerFile>(Path::new(file))?;
            validate_mcp_server(&server)?;
            print_ok_preview(&server)
        }
        "diff" => {
            let selector = required_arg(args, 1, "mcp diff <name-or-id> <file>")?;
            let file = required_arg(args, 2, "mcp diff <name-or-id> <file>")?;
            let current = find_mcp_server(ctx, selector)?;
            let next = read_json_file::<McpServerFile>(Path::new(file))?;
            validate_mcp_server(&next)?;
            print_json(&build_named_diff(&current, &next))
        }
        "update" => {
            let selector = required_arg(args, 1, "mcp update <name-or-id> <file>")?;
            let file = required_arg(args, 2, "mcp update <name-or-id> <file>")?;
            let next = read_json_file::<McpServerFile>(Path::new(file))?;
            validate_mcp_server(&next)?;
            let server_id = find_mcp_server_id(ctx, selector)?;
            save_mcp_server(ctx, &server_id, &next)?;
            print_json(&next)
        }
        "enable" => {
            let selector = required_arg(args, 1, "mcp enable <name-or-id>")?;
            let id = find_mcp_server_id(ctx, selector)?;
            ensure_mcp_policy(ctx, &id, true)?;
            print_json(&serde_json::json!({"serverId": id, "enabled": true}))
        }
        "disable" => {
            let selector = required_arg(args, 1, "mcp disable <name-or-id>")?;
            let id = find_mcp_server_id(ctx, selector)?;
            ensure_mcp_policy(ctx, &id, false)?;
            print_json(&serde_json::json!({"serverId": id, "enabled": false}))
        }
        "delete" => {
            let selector = required_arg(args, 1, "mcp delete <name-or-id> --confirmed")?;
            require_delete_confirmed(args, "mcp delete <name-or-id> --confirmed")?;
            let id = delete_mcp_server(ctx, selector)?;
            print_json(&serde_json::json!({"ok": true, "serverId": id}))
        }
        "test" => {
            let selector = required_arg(args, 1, "mcp test <name-or-id>")?;
            let server = find_mcp_server(ctx, selector)?;
            validate_mcp_server(&server)?;
            print_json(&serde_json::json!({
                "ok": true,
                "message": "mcp definition is structurally valid",
                "diagnostics": {
                    "transport": server.transport,
                    "command": server.command,
                    "url": server.url
                }
            }))
        }
        "tools" => {
            let selector = required_arg(args, 1, "mcp tools <name-or-id>")?;
            let id = find_mcp_server_id(ctx, selector)?;
            let policy = load_mcp_policy(ctx, &id)?;
            print_json(&policy.tools)
        }
        _ => Err("用法: mcp ls|get|example|add|new|export|check|diff|update|enable|disable|delete|test|tools".to_string()),
    }
}

fn load_agents(ctx: &CliContext) -> Result<Vec<AgentProfile>, String> {
    let mut agents = load_main_agents(ctx)?;
    let mut seen_ids = agents
        .iter()
        .map(|agent| agent.id.trim().to_ascii_lowercase())
        .filter(|id| !id.is_empty())
        .collect::<BTreeSet<_>>();
    for agent in load_private_agents(ctx)? {
        let key = agent.id.trim().to_ascii_lowercase();
        if !key.is_empty() && seen_ids.insert(key) {
            agents.push(agent);
        }
    }
    Ok(agents)
}

fn visible_agents(agents: &[AgentProfile]) -> Vec<AgentProfile> {
    agents
        .iter()
        .filter(|agent| !is_preset_agent(agent))
        .cloned()
        .collect::<Vec<_>>()
}

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

fn main_agents_shard_path(ctx: &CliContext) -> PathBuf {
    app_root_from_cli_data_path(&ctx.data_path)
        .join("config")
        .join("agents.json")
}

fn load_main_agents(ctx: &CliContext) -> Result<Vec<AgentProfile>, String> {
    let shard = main_agents_shard_path(ctx);
    if shard.exists() {
        return Ok(read_json_file::<AgentsFile>(&shard)?.agents);
    }
    Ok(Vec::new())
}

fn save_main_agents(ctx: &CliContext, agents: &[AgentProfile]) -> Result<(), String> {
    let shard = main_agents_shard_path(ctx);
    if let Some(parent) = shard.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建 agents 目录失败 ({}): {err}", parent.display()))?;
    }
    write_json_file(&shard, &AgentsFile { agents: agents.to_vec() })
}

fn save_agent(ctx: &CliContext, agent: &AgentProfile) -> Result<(), String> {
    if is_private_agent(agent) {
        return save_private_agent(ctx, agent);
    }
    let mut agents = load_main_agents(ctx)?;
    let idx = find_agent_index(&agents, &agent.id)?;
    agents[idx] = agent.clone();
    save_main_agents(ctx, &agents)
}

fn load_private_agents(ctx: &CliContext) -> Result<Vec<AgentProfile>, String> {
    let dir = private_personas_dir(ctx);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(&dir)
        .map_err(|err| format!("读取私有人格目录失败 ({}): {err}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    let mut agents = Vec::new();
    for path in paths {
        let file = read_json_file::<PrivatePersonaFile>(&path)?;
        let id = file.id.trim().to_string();
        if id.is_empty() || reserved_agent_id(&id) {
            continue;
        }
        let name = file.name.trim().to_string();
        let system_prompt = file.system_prompt.trim().to_string();
        if name.is_empty() || system_prompt.is_empty() {
            continue;
        }
        let now = now_iso();
        agents.push(AgentProfile {
            id,
            name,
            system_prompt,
            tools: file.tools,
            created_at: now.clone(),
            updated_at: now,
            avatar_path: file.avatar_path,
            avatar_updated_at: None,
            is_built_in_user: false,
            is_built_in_system: false,
            private_memory_enabled: false,
            memory_recall_mode: default_memory_recall_mode(),
            source: default_private_workspace_source(),
            scope: default_assistant_private_scope(),
            summary: file.summary,
            resident_skill_names: file.resident_skill_names,
            optional_skill_names: file.optional_skill_names,
            api_config_ids: file.api_config_ids,
            api_config_id: String::new(),
            permission_control: file.permission_control,
            child_agent_ids: file.child_agent_ids,
        });
    }
    Ok(agents)
}

fn save_private_agent(ctx: &CliContext, agent: &AgentProfile) -> Result<(), String> {
    let dir = private_personas_dir(ctx);
    fs::create_dir_all(&dir)
        .map_err(|err| format!("创建私有人格目录失败 ({}): {err}", dir.display()))?;
    let existing = find_private_agent_path(ctx, &agent.id)?;
    let path = existing
        .clone()
        .unwrap_or_else(|| dir.join(format!("{}.json", sanitize_file_id(&agent.id))));
    // 人格字段本身已由 load_private_agents 从 JSON 读全，这里直接回写，
    // 保证 agent update / set-parent 等命令改动的字段真的落盘。
    let file = PrivatePersonaFile {
        id: agent.id.clone(),
        name: agent.name.clone(),
        system_prompt: agent.system_prompt.clone(),
        tools: agent.tools.clone(),
        avatar_path: agent.avatar_path.clone(),
        summary: agent.summary.clone(),
        resident_skill_names: agent.resident_skill_names.clone(),
        optional_skill_names: agent.optional_skill_names.clone(),
        api_config_ids: agent.api_config_ids.clone(),
        child_agent_ids: agent.child_agent_ids.clone(),
        permission_control: agent.permission_control.clone(),
    };
    write_json_file(&path, &file)
}

fn find_private_agent_path(ctx: &CliContext, agent_id: &str) -> Result<Option<PathBuf>, String> {
    let dir = private_personas_dir(ctx);
    if !dir.exists() {
        return Ok(None);
    }
    for entry in fs::read_dir(&dir)
        .map_err(|err| format!("读取私有人格目录失败 ({}): {err}", dir.display()))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let Ok(file) = read_json_file::<PrivatePersonaFile>(&path) else {
            continue;
        };
        if file.id.eq_ignore_ascii_case(agent_id) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn private_personas_dir(ctx: &CliContext) -> PathBuf {
    effective_workspace_root(ctx)
        .join("private-organization")
        .join("personas")
}

fn is_private_agent(agent: &AgentProfile) -> bool {
    agent.source.trim() == default_private_workspace_source()
}

fn reserved_agent_id(id: &str) -> bool {
    matches!(id, "default" | "user" | "system")
}

fn is_preset_agent(agent: &AgentProfile) -> bool {
    agent.is_built_in_user || agent.is_built_in_system || reserved_agent_id(&agent.id)
}

fn ensure_agent_writable(agent: &AgentProfile) -> Result<(), String> {
    if is_preset_agent(agent) {
        return Err("预设人格是只读的，不能通过 config 工具修改。".to_string());
    }
    Ok(())
}

/// 内置人格的常驻/可选 skill 引用锁定（结论 25）：引用不可摘、不可改。
fn ensure_agent_skill_editable(agent: &AgentProfile) -> Result<(), String> {
    if is_built_in_organization_agent_id(&agent.id) {
        return Err(format!(
            "内置人格的常驻/可选 skill 引用是锁定的，不能通过 config 工具修改：{}",
            agent.id
        ));
    }
    Ok(())
}

fn load_config_doc(ctx: &CliContext) -> Result<toml::Value, String> {
    if !ctx.config_path.exists() {
        let snapshot = AppConfigSnapshot::default();
        return toml::Value::try_from(snapshot).map_err(|err| format!("初始化配置失败: {err}"));
    }
    let raw = fs::read_to_string(&ctx.config_path)
        .map_err(|err| format!("读取配置失败 ({}): {err}", ctx.config_path.display()))?;
    toml::from_str::<toml::Value>(&raw)
        .map_err(|err| format!("解析配置失败 ({}): {err}", ctx.config_path.display()))
}

fn save_config_doc(ctx: &CliContext, doc: &toml::Value) -> Result<(), String> {
    if let Some(parent) = ctx.config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建配置目录失败 ({}): {err}", parent.display()))?;
    }
    let text = toml::to_string_pretty(doc).map_err(|err| format!("序列化配置失败: {err}"))?;
    fs::write(&ctx.config_path, text)
        .map_err(|err| format!("写入配置失败 ({}): {err}", ctx.config_path.display()))
}

fn snapshot_from_doc(doc: &toml::Value) -> Result<AppConfigSnapshot, String> {
    doc.clone()
        .try_into()
        .map_err(|err| format!("读取配置快照失败: {err}"))
}

fn load_snapshot(ctx: &CliContext) -> Result<AppConfigSnapshot, String> {
    snapshot_from_doc(&load_config_doc(ctx)?)
}

#[allow(dead_code)]
fn write_providers_to_doc(doc: &mut toml::Value, providers: &[ApiProviderConfig]) -> Result<(), String> {
    let providers_value = toml::Value::try_from(providers.to_vec())
        .map_err(|err| format!("序列化 apiProviders 失败: {err}"))?;
    ensure_doc_table(doc)?
        .insert("apiProviders".to_string(), providers_value);
    Ok(())
}

fn ensure_doc_table(doc: &mut toml::Value) -> Result<&mut toml::map::Map<String, toml::Value>, String> {
    doc.as_table_mut()
        .ok_or_else(|| "配置根节点必须是 TOML table".to_string())
}

fn find_agent<'a>(agents: &'a [AgentProfile], selector: &str) -> Result<&'a AgentProfile, String> {
    let idx = find_agent_index(agents, selector)?;
    Ok(&agents[idx])
}

fn find_agent_index(agents: &[AgentProfile], selector: &str) -> Result<usize, String> {
    agents
        .iter()
        .position(|item| matches_selector(&item.id, &item.name, selector))
        .ok_or_else(|| format!("agent not found: {selector}"))
}

#[allow(dead_code)]
fn find_provider<'a>(providers: &'a [ApiProviderConfig], selector: &str) -> Result<&'a ApiProviderConfig, String> {
    let idx = find_provider_index(providers, selector)?;
    Ok(&providers[idx])
}

#[allow(dead_code)]
fn find_provider_index(providers: &[ApiProviderConfig], selector: &str) -> Result<usize, String> {
    providers
        .iter()
        .position(|item| matches_selector(&item.id, &item.name, selector))
        .ok_or_else(|| format!("provider not found: {selector}"))
}

fn matches_selector(id: &str, name: &str, selector: &str) -> bool {
    let selector = selector.trim();
    !selector.is_empty()
        && (id.eq_ignore_ascii_case(selector) || name.eq_ignore_ascii_case(selector))
}

fn agent_example() -> AgentProfile {
    build_new_agent(
        "example-agent",
        "你是一个可靠、直接、会先给结论再补说明的助手。",
        None,
    )
}

fn build_new_agent(name: &str, persona: &str, avatar: Option<String>) -> AgentProfile {
    let now = now_iso();
    AgentProfile {
        id: slugify_with_fallback(name, "agent"),
        name: name.to_string(),
        system_prompt: persona.to_string(),
        tools: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
        avatar_path: avatar,
        avatar_updated_at: None,
        is_built_in_user: false,
        is_built_in_system: false,
        private_memory_enabled: false,
        memory_recall_mode: default_memory_recall_mode(),
        source: default_main_source(),
        scope: default_global_scope(),
        summary: String::new(),
        resident_skill_names: Vec::new(),
        optional_skill_names: Vec::new(),
        api_config_ids: Vec::new(),
        api_config_id: String::new(),
        permission_control: None,
        child_agent_ids: Vec::new(),
    }
}

fn validate_agent(agent: &AgentProfile) -> Result<(), String> {
    if agent.name.trim().is_empty() {
        return Err("agent.name 不能为空".to_string());
    }
    if agent.system_prompt.trim().is_empty() {
        return Err("agent.systemPrompt 不能为空".to_string());
    }
    Ok(())
}

fn agent_list_item(agent: &AgentProfile) -> AgentListItem {
    AgentListItem {
        id: agent.id.clone(),
        name: agent.name.clone(),
    }
}

#[allow(dead_code)]
fn provider_list_item(provider: &ApiProviderConfig) -> ProviderListItem {
    ProviderListItem {
        id: provider.id.clone(),
        name: provider.name.clone(),
        request_format: provider.request_format.clone(),
        base_url: provider.base_url.clone(),
        enabled: ProviderCapabilitySummary {
            text: provider.enable_text,
            image: provider.enable_image,
            audio: provider.enable_audio,
            video: provider.enable_video,
            tools: provider.enable_tools,
        },
        model_count: provider.models.len(),
        key_count: provider
            .api_keys
            .iter()
            .filter(|key| !key.trim().is_empty())
            .count(),
    }
}

#[allow(dead_code)]
fn redacted_provider(provider: &ApiProviderConfig) -> ApiProviderConfig {
    let mut out = provider.clone();
    out.api_keys = out
        .api_keys
        .iter()
        .map(|key| redacted_secret_placeholder(key))
        .collect();
    out.codex_custom_api_key = out
        .codex_custom_api_key
        .as_ref()
        .map(|key| redacted_secret_placeholder(key));
    out
}

#[allow(dead_code)]
fn redacted_secret_placeholder(secret: &str) -> String {
    let len = secret.chars().count();
    if len == 0 {
        return String::new();
    }
    format!("__PAI_SECRET_REDACTED_LEN_{len}__")
}

#[allow(dead_code)]
fn is_redacted_secret_placeholder(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("__PAI_SECRET_REDACTED_LEN_") && value.ends_with("__")
}

#[allow(dead_code)]
fn restore_redacted_provider_secrets(next: &mut ApiProviderConfig, current: &ApiProviderConfig) {
    for (idx, key) in next.api_keys.iter_mut().enumerate() {
        if is_redacted_secret_placeholder(key) {
            if let Some(current_key) = current.api_keys.get(idx) {
                *key = current_key.clone();
            }
        }
    }
    if next
        .codex_custom_api_key
        .as_deref()
        .map(is_redacted_secret_placeholder)
        .unwrap_or(false)
    {
        next.codex_custom_api_key = current.codex_custom_api_key.clone();
    }
}

#[allow(dead_code)]
fn provider_example() -> ApiProviderConfig {
    build_new_provider("example-provider")
}

#[allow(dead_code)]
fn build_new_provider(name: &str) -> ApiProviderConfig {
    ApiProviderConfig {
        id: slugify_with_fallback(name, "provider"),
        name: name.to_string(),
        request_format: default_request_format(),
        allow_concurrent_requests: false,
        max_concurrent_requests: None,
        enable_text: true,
        enable_image: false,
        enable_audio: false,
        enable_video: false,
        enable_tools: true,
        tools: Vec::new(),
        base_url: "https://api.openai.com/v1".to_string(),
        codex_auth_mode: "read_local".to_string(),
        codex_local_auth_path: "~/.codex/auth.json".to_string(),
        codex_custom_url: None,
        codex_custom_api_key: None,
        codex_originator: "codex-tui".to_string(),
        codex_residency_requirement: None,
        api_keys: Vec::new(),
        key_cursor: 0,
        cached_model_options: vec!["gpt-4o-mini".to_string()],
        models: vec![ApiModelConfig {
            id: "default-model".to_string(),
            model: "gpt-4o-mini".to_string(),
            display_name: String::new(),
            enable_image: false,
            enable_audio: false,
            enable_video: false,
            enable_tools: true,
            reasoning_effort: default_reasoning_effort(),
            temperature: default_temperature(),
            custom_temperature_enabled: false,
            context_window_tokens: default_context_window_tokens(),
            max_output_tokens: default_max_output_tokens(),
            custom_max_output_tokens_enabled: false,
        }],
        failure_retry_count: 0,
    }
}

#[allow(dead_code)]
fn validate_provider(provider: &ApiProviderConfig) -> Result<(), String> {
    if provider.id.trim().is_empty() {
        return Err("provider.id 不能为空".to_string());
    }
    if provider.name.trim().is_empty() {
        return Err("provider.name 不能为空".to_string());
    }
    if provider.base_url.trim().is_empty() {
        return Err("provider.baseUrl 不能为空".to_string());
    }
    if provider.models.is_empty() {
        return Err("provider.models 至少需要一个模型".to_string());
    }
    if provider.models.iter().any(|item| item.id.trim().is_empty() || item.model.trim().is_empty()) {
        return Err("provider.models 中的 id/model 不能为空".to_string());
    }
    Ok(())
}

fn list_mcp_servers(ctx: &CliContext) -> Result<Vec<McpServerSummary>, String> {
    let mut out = Vec::new();
    let dir = mcp_servers_dir(ctx);
    if !dir.exists() {
        return Ok(out);
    }
    for path in json_files(&dir)? {
        let entry = read_mcp_server_entry(&path)?;
        out.push(McpServerSummary {
            id: entry.id,
            name: entry.file.name.clone().unwrap_or_default(),
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct McpServerSummary {
    id: String,
    name: String,
}

fn find_mcp_server(ctx: &CliContext, selector: &str) -> Result<McpServerFile, String> {
    let id = find_mcp_server_id(ctx, selector)?;
    let path = find_mcp_server_path(ctx, &id)?
        .ok_or_else(|| format!("mcp server not found: {selector}"))?;
    Ok(read_mcp_server_entry(&path)?.file)
}

fn find_mcp_server_id(ctx: &CliContext, selector: &str) -> Result<String, String> {
    for server in list_mcp_servers(ctx)? {
        if matches_selector(&server.id, &server.name, selector) {
            return Ok(server.id);
        }
    }
    Err(format!("mcp server not found: {selector}"))
}

fn find_mcp_server_path(ctx: &CliContext, id: &str) -> Result<Option<PathBuf>, String> {
    let dir = mcp_servers_dir(ctx);
    if !dir.exists() {
        return Ok(None);
    }
    for path in json_files(&dir)? {
        let entry = read_mcp_server_entry(&path)?;
        if entry.id == id {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn read_mcp_server_entry(path: &Path) -> Result<McpServerEntry, String> {
    let id = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("读取 MCP 失败 ({}): {err}", path.display()))?;
    let value = serde_json::from_str::<JsonValue>(&raw)
        .map_err(|err| format!("解析 MCP JSON 失败 ({}): {err}", path.display()))?;
    let definition_value = value
        .get("definitionJson")
        .or_else(|| value.get("definition_json"))
        .and_then(JsonValue::as_str)
        .and_then(|text| serde_json::from_str::<JsonValue>(text).ok())
        .unwrap_or_else(|| value.clone());
    let file = mcp_server_file_from_definition(&id, &definition_value)?;
    Ok(McpServerEntry { id, file })
}

fn mcp_server_file_from_definition(id: &str, value: &JsonValue) -> Result<McpServerFile, String> {
    if let Some(servers) = value.get("mcpServers").and_then(JsonValue::as_object) {
        let (name, server_value) = servers
            .iter()
            .next()
            .ok_or_else(|| "mcpServers 不能为空".to_string())?;
        let mut file = mcp_server_file_from_object(server_value)?;
        if file.name.as_deref().map(str::trim).unwrap_or("").is_empty() {
            file.name = Some(name.to_string());
        }
        return Ok(file);
    }
    let mut file = mcp_server_file_from_object(value)?;
    if file.name.as_deref().map(str::trim).unwrap_or("").is_empty() {
        file.name = Some(id.to_string());
    }
    Ok(file)
}

fn mcp_server_file_from_object(value: &JsonValue) -> Result<McpServerFile, String> {
    let mut file = serde_json::from_value::<McpServerFile>(value.clone())
        .map_err(|err| format!("解析 MCP 定义失败: {err}"))?;
    if file.transport.as_deref().map(str::trim).unwrap_or("").is_empty() {
        if file.command.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_some() {
            file.transport = Some("stdio".to_string());
        } else if file.url.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_some() {
            file.transport = Some("streamableHttp".to_string());
        }
    }
    Ok(file)
}

fn mcp_server_definition_json(id: &str, server: &McpServerFile) -> JsonValue {
    let name = server
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(id);
    let mut server_obj = serde_json::Map::new();
    if let Some(transport) = server.transport.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        server_obj.insert("transport".to_string(), JsonValue::String(transport.to_string()));
    }
    if let Some(command) = server.command.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        server_obj.insert("command".to_string(), JsonValue::String(command.to_string()));
    }
    if let Some(args) = &server.args {
        server_obj.insert(
            "args".to_string(),
            JsonValue::Array(args.iter().map(|arg| JsonValue::String(arg.clone())).collect()),
        );
    }
    if let Some(url) = server.url.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        server_obj.insert("url".to_string(), JsonValue::String(url.to_string()));
    }
    if let Some(env) = &server.env {
        server_obj.insert(
            "env".to_string(),
            serde_json::to_value(env).unwrap_or_else(|_| JsonValue::Object(serde_json::Map::new())),
        );
    }
    let mut servers = serde_json::Map::new();
    servers.insert(name.to_string(), JsonValue::Object(server_obj));
    let mut root = serde_json::Map::new();
    root.insert("mcpServers".to_string(), JsonValue::Object(servers));
    JsonValue::Object(root)
}

fn save_mcp_server(ctx: &CliContext, id: &str, server: &McpServerFile) -> Result<(), String> {
    let dir = mcp_servers_dir(ctx);
    fs::create_dir_all(&dir)
        .map_err(|err| format!("创建 MCP 目录失败 ({}): {err}", dir.display()))?;
    write_json_file(&dir.join(format!("{}.json", sanitize_file_id(id))), &mcp_server_definition_json(id, server))
}

fn delete_mcp_server(ctx: &CliContext, selector: &str) -> Result<String, String> {
    let id = find_mcp_server_id(ctx, selector)?;
    let server_path = find_mcp_server_path(ctx, &id)?
        .ok_or_else(|| format!("mcp server not found: {selector}"))?;
    fs::remove_file(&server_path)
        .map_err(|err| format!("删除 MCP 失败 ({}): {err}", server_path.display()))?;
    let policy_path = mcp_policies_dir(ctx).join(format!("{}.json", sanitize_file_id(&id)));
    if policy_path.exists() {
        fs::remove_file(&policy_path)
            .map_err(|err| format!("删除 MCP policy 失败 ({}): {err}", policy_path.display()))?;
    }
    Ok(id)
}

fn ensure_mcp_policy(ctx: &CliContext, id: &str, enabled: bool) -> Result<(), String> {
    let mut policy = load_mcp_policy(ctx, id).unwrap_or_default();
    policy.server_id = id.to_string();
    policy.enabled = enabled;
    let dir = mcp_policies_dir(ctx);
    fs::create_dir_all(&dir)
        .map_err(|err| format!("创建 MCP policy 目录失败 ({}): {err}", dir.display()))?;
    write_json_file(&dir.join(format!("{}.json", sanitize_file_id(id))), &policy)
}

fn load_mcp_policy(ctx: &CliContext, id: &str) -> Result<McpPolicyFile, String> {
    let path = mcp_policies_dir(ctx).join(format!("{}.json", sanitize_file_id(id)));
    if path.exists() {
        read_json_file(&path)
    } else {
        Ok(McpPolicyFile {
            server_id: id.to_string(),
            enabled: false,
            tools: Vec::new(),
        })
    }
}

fn validate_mcp_server(server: &McpServerFile) -> Result<(), String> {
    let transport = server.transport.as_deref().unwrap_or_default().trim().to_ascii_lowercase();
    let has_command = server.command.as_deref().map(str::trim).filter(|v| !v.is_empty()).is_some();
    let has_url = server.url.as_deref().map(str::trim).filter(|v| !v.is_empty()).is_some();
    if transport == "stdio" || (transport.is_empty() && has_command) {
        if !has_command {
            return Err("stdio MCP 必须提供 command".to_string());
        }
        return Ok(());
    }
    if transport == "http" || transport == "sse" || transport == "streamablehttp" || has_url {
        if !has_url {
            return Err("HTTP/SSE MCP 必须提供 url".to_string());
        }
        return Ok(());
    }
    if has_command || has_url {
        return Ok(());
    }
    Err("MCP 至少需要 command 或 url".to_string())
}

fn mcp_example() -> McpServerFile {
    McpServerFile {
        name: Some("example-mcp".to_string()),
        transport: Some("stdio".to_string()),
        command: Some("npx".to_string()),
        args: Some(vec!["@playwright/mcp@latest".to_string()]),
        url: None,
        env: None,
    }
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
    let mut out = value.trim().trim_matches('"').trim_matches('\'').to_string();
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
        .ok_or_else(|| "无法定位标准配置目录".to_string())?;
    let path = dirs.config_dir().to_path_buf();
    fs::create_dir_all(&path)
        .map_err(|err| format!("创建标准配置目录失败 ({}): {err}", path.display()))?;
    Ok(path)
}

fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in text.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch == '-' || ch == '_' || ch.is_whitespace() {
            Some('-')
        } else {
            None
        };
        if let Some(ch) = mapped {
            if ch == '-' {
                if !last_dash && !out.is_empty() {
                    out.push(ch);
                }
                last_dash = true;
            } else {
                out.push(ch);
                last_dash = false;
            }
        }
    }
    out.trim_matches('-').to_string()
}

fn slugify_with_fallback(text: &str, prefix: &str) -> String {
    let out = slugify(text);
    if out.is_empty() {
        format!("{}-{}", prefix.trim(), Utc::now().timestamp())
    } else {
        out
    }
}

fn unique_slugified_id<'a>(
    text: &str,
    prefix: &str,
    existing_ids: impl IntoIterator<Item = &'a str>,
) -> String {
    let base = slugify_with_fallback(text, prefix);
    let existing = existing_ids
        .into_iter()
        .map(|item| item.trim().to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect::<BTreeSet<_>>();
    if !existing.contains(&base.to_ascii_lowercase()) {
        return base;
    }
    for index in 2..=10_000 {
        let candidate = format!("{base}-{index}");
        if !existing.contains(&candidate.to_ascii_lowercase()) {
            return candidate;
        }
    }
    format!("{}-{}", base, Utc::now().timestamp_millis())
}

fn sanitize_file_id(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed
    }
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn keep_or_now(value: &str) -> String {
    if value.trim().is_empty() {
        now_iso()
    } else {
        value.to_string()
    }
}

fn required_arg<'a>(args: &'a [String], index: usize, usage: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("参数不足，用法: {usage}"))
}

fn require_delete_confirmed(args: &[String], usage: &str) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--confirmed") {
        Ok(())
    } else {
        Err(format!(
            "删除命令必须先获得用户明确同意，并在命令中加入 --confirmed。用法: {usage}"
        ))
    }
}

fn save_avatar_file(ctx: &CliContext, agent_id: &str, image_path: &Path) -> Result<PathBuf, String> {
    let ext = image_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    if ext != "png" && ext != "webp" {
        return Err("头像目前只支持 .png 或 .webp".to_string());
    }
    let avatars_dir = ctx.app_root.join("avatars");
    fs::create_dir_all(&avatars_dir)
        .map_err(|err| format!("创建头像目录失败 ({}): {err}", avatars_dir.display()))?;
    let target = avatars_dir.join(format!("agent-{}.{}", sanitize_file_id(agent_id), ext));
    fs::copy(image_path, &target).map_err(|err| {
        format!(
            "复制头像失败 ({} -> {}): {err}",
            image_path.display(),
            target.display()
        )
    })?;
    Ok(target)
}

fn print_ok_preview<T: Serialize>(value: &T) -> Result<(), String> {
    print_json(&serde_json::json!({
        "ok": true,
        "blockingIssues": [],
        "warnings": [],
        "normalizedPreview": value
    }))
}

fn build_named_diff<T: Serialize>(current: &T, next: &T) -> serde_json::Value {
    let current_value = serde_json::to_value(current).unwrap_or(JsonValue::Null);
    let next_value = serde_json::to_value(next).unwrap_or(JsonValue::Null);
    let changed_fields = diff_fields("", &current_value, &next_value);
    serde_json::json!({
        "planSummary": format!("{} 个字段将变化", changed_fields.len()),
        "planItems": changed_fields,
        "changedFields": changed_fields,
        "affectedResources": []
    })
}

fn diff_fields(prefix: &str, current: &JsonValue, next: &JsonValue) -> Vec<String> {
    if current == next {
        return Vec::new();
    }
    match (current, next) {
        (JsonValue::Object(left), JsonValue::Object(right)) => {
            let keys = left.keys().chain(right.keys()).cloned().collect::<BTreeSet<_>>();
            let mut out = Vec::new();
            for key in keys.iter() {
                let path = if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{prefix}.{key}")
                };
                let left_value = left.get(key).unwrap_or(&JsonValue::Null);
                let right_value = right.get(key).unwrap_or(&JsonValue::Null);
                let nested = diff_fields(&path, left_value, right_value);
                if nested.is_empty() && left_value != right_value {
                    out.push(path);
                } else {
                    out.extend(nested);
                }
            }
            out
        }
        _ => vec![prefix.to_string()],
    }
}

fn read_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let raw = fs::read_to_string(path)
        .map_err(|err| format!("读取 JSON 失败 ({}): {err}", path.display()))?;
    serde_json::from_str(&raw).map_err(|err| format!("解析 JSON 失败 ({}): {err}", path.display()))
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("创建目录失败 ({}): {err}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|err| format!("序列化 JSON 失败: {err}"))?;
    fs::write(path, text).map_err(|err| format!("写入文件失败 ({}): {err}", path.display()))
}

fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|err| format!("序列化输出失败: {err}"))?;
    push_output(&text);
    Ok(())
}

fn print_output_path(path: &str) -> Result<(), String> {
    print_json(&serde_json::json!({
        "path": path
    }))
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

fn default_true() -> bool {
    true
}

fn default_request_format() -> String {
    "openai".to_string()
}

fn default_reasoning_effort() -> String {
    "medium".to_string()
}

fn default_temperature() -> f64 {
    1.0
}

fn default_context_window_tokens() -> u32 {
    128_000
}

fn default_max_output_tokens() -> u32 {
    4_096
}

fn default_memory_recall_mode() -> String {
    "auto".to_string()
}

fn default_main_source() -> String {
    "main_config".to_string()
}

fn default_global_scope() -> String {
    "global".to_string()
}

fn default_private_workspace_source() -> String {
    "private_workspace".to_string()
}

fn default_assistant_private_scope() -> String {
    "assistant_private".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("pai-config-module-test-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("config")).expect("create config dir");
        fs::create_dir_all(root.join("llm-workspace")).expect("create workspace dir");
        root
    }

    fn seed_app(root: &Path) {
        fs::write(root.join("app_config.toml"), sample_config_toml()).expect("write app_config");
        fs::write(root.join("config").join("agents.json"), sample_agents_json()).expect("write agents");
    }

    fn sample_config_toml() -> &'static str {
        r#"
selectedApiConfigId = "provider-a::model-a"
assistantDepartmentApiConfigId = "provider-a::model-a"

[[departments]]
id = "dept-a"
name = "Dept A"
summary = "A"
guide = "GA"
apiConfigIds = ["provider-a::model-a"]
apiConfigId = "provider-a::model-a"
agentIds = ["agent-a"]
childDepartmentIds = []
createdAt = "2026-01-01T00:00:00Z"
updatedAt = "2026-01-01T00:00:00Z"
orderIndex = 1
isBuiltInAssistant = false
source = "main_config"
scope = "global"

[[departments]]
id = "dept-b"
name = "Dept B"
summary = "B"
guide = "GB"
apiConfigIds = ["provider-a::model-a"]
apiConfigId = "provider-a::model-a"
agentIds = ["agent-b"]
childDepartmentIds = []
createdAt = "2026-01-01T00:00:00Z"
updatedAt = "2026-01-01T00:00:00Z"
orderIndex = 2
isBuiltInAssistant = false
source = "main_config"
scope = "global"

[[apiProviders]]
id = "provider-a"
name = "Provider A"
requestFormat = "openai"
enableText = true
enableTools = true
baseUrl = "https://api.openai.com/v1"
apiKeys = ["sk-test-secret"]
cachedModelOptions = ["gpt-4o-mini"]

[[apiProviders.models]]
id = "model-a"
model = "gpt-4o-mini"
enableTools = true
reasoningEffort = "medium"
temperature = 1.0
contextWindowTokens = 128000
maxOutputTokens = 4096
"#
    }

    fn sample_agents_json() -> &'static str {
        r#"
{
  "agents": [
    {
      "id": "agent-a",
      "name": "Agent A",
      "systemPrompt": "Prompt A",
      "tools": [],
      "createdAt": "2026-01-01T00:00:00Z",
      "updatedAt": "2026-01-01T00:00:00Z",
      "privateMemoryEnabled": false,
      "memoryRecallMode": "auto",
      "source": "main_config",
      "scope": "global"
    }
  ]
}
"#
    }

    fn append_preset_agent_and_department(root: &Path) {
        fs::write(
            root.join("config").join("agents.json"),
            r#"
{
  "agents": [
    {
      "id": "agent-a",
      "name": "Agent A",
      "systemPrompt": "Prompt A",
      "tools": [],
      "createdAt": "2026-01-01T00:00:00Z",
      "updatedAt": "2026-01-01T00:00:00Z",
      "privateMemoryEnabled": false,
      "memoryRecallMode": "auto",
      "source": "main_config",
      "scope": "global"
    },
    {
      "id": "user",
      "name": "User Persona",
      "systemPrompt": "Built-in user",
      "tools": [],
      "createdAt": "2026-01-01T00:00:00Z",
      "updatedAt": "2026-01-01T00:00:00Z",
      "isBuiltInUser": true,
      "privateMemoryEnabled": false,
      "memoryRecallMode": "auto",
      "source": "main_config",
      "scope": "global"
    }
  ]
}
"#,
        )
        .expect("write agents with preset");
        let mut config = fs::read_to_string(root.join("app_config.toml")).expect("read config");
        config.push_str(
            r#"

[[departments]]
id = "assistant-department"
name = "助理部门"
summary = "built-in"
guide = "built-in"
apiConfigIds = ["provider-a::model-a"]
apiConfigId = "provider-a::model-a"
agentIds = ["agent-a"]
childDepartmentIds = []
createdAt = "2026-01-01T00:00:00Z"
updatedAt = "2026-01-01T00:00:00Z"
orderIndex = 99
isBuiltInAssistant = true
source = "main_config"
scope = "global"
"#,
        );
        fs::write(root.join("app_config.toml"), config).expect("write config with preset department");
    }

    #[test]
    fn approot_should_report_data_directory_and_key_paths() {
        let root = test_root();
        seed_app(&root);
        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "approot",
        )
        .expect("run approot");
        let value: JsonValue = serde_json::from_str(&output).expect("parse output");
        assert_eq!(value["appRoot"], root.display().to_string());
        assert_eq!(
            value["configPath"],
            root.join("app_config.toml").display().to_string()
        );
        assert_eq!(
            value["workspaceRoot"],
            root.join("llm-workspace").display().to_string()
        );
    }

    #[test]
    fn approot_should_lift_app_root_when_data_path_sits_under_config_dir() {
        let root = test_root();
        seed_app(&root);
        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config").join("config_mark"),
            root.join("llm-workspace"),
            "approot",
        )
        .expect("run approot");
        let value: JsonValue = serde_json::from_str(&output).expect("parse output");
        assert_eq!(value["appRoot"], root.display().to_string());
    }

    #[test]
    fn run_command_with_paths_should_support_agent_ls() {
        let root = test_root();
        seed_app(&root);
        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "agent ls",
        )
        .expect("run agent ls");
        let value: JsonValue = serde_json::from_str(&output).expect("parse output");
        assert_eq!(value.as_array().expect("agents array").len(), 1);
        assert_eq!(value[0]["id"], "agent-a");
        assert_eq!(value[0]["name"], "Agent A");
        assert!(value[0].get("systemPrompt").is_none());
    }

    #[test]
    fn agent_new_should_persist_and_use_agent_prefix_for_non_ascii_name() {
        let root = test_root();
        seed_app(&root);
        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "agent new 测试人格 你是一个可靠助手",
        )
        .expect("create agent");
        let value: JsonValue = serde_json::from_str(&output).expect("parse created agent");
        let agent_id = value["id"].as_str().expect("agent id");
        assert!(agent_id.starts_with("agent-"));

        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "agent ls",
        )
        .expect("run agent ls");
        let value: JsonValue = serde_json::from_str(&output).expect("parse output");
        let agents = value.as_array().expect("agents array");
        assert!(agents.iter().any(|item| item["id"] == agent_id));
        assert!(agents.iter().any(|item| item["name"] == "测试人格"));
    }

    #[test]
    fn agent_new_should_write_agents_shard_next_to_data_path_not_workspace_root() {
        let root = test_root();
        seed_app(&root);
        let actual_data_root = root.join("runtime-data");
        let wrong_app_root = root.join("workspace-owner");
        fs::create_dir_all(actual_data_root.join("config")).expect("create actual config dir");
        fs::create_dir_all(wrong_app_root.join("llm-workspace")).expect("create wrong workspace dir");
        fs::copy(root.join("app_config.toml"), actual_data_root.join("app_config.toml")).expect("copy config");
        fs::copy(root.join("config_mark"), actual_data_root.join("config_mark")).ok();

        let output = run_command_with_paths(
            wrong_app_root.clone(),
            actual_data_root.join("app_config.toml"),
            actual_data_root.join("config_mark"),
            wrong_app_root.join("llm-workspace"),
            "agent new TestAgent Prompt",
        )
        .expect("create agent");
        let value: JsonValue = serde_json::from_str(&output).expect("parse created agent");
        assert_eq!(value["id"], "testagent");

        let actual_agents_shard = actual_data_root.join("config").join("agents.json");
        let wrong_agents_shard = wrong_app_root.join("config").join("agents.json");
        assert!(actual_agents_shard.exists());
        assert!(!wrong_agents_shard.exists());
    }

    #[test]
    fn agent_ls_and_get_should_hide_preset_agents() {
        let root = test_root();
        seed_app(&root);
        append_preset_agent_and_department(&root);

        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "agent ls",
        )
        .expect("run agent ls");
        let value: JsonValue = serde_json::from_str(&output).expect("parse output");
        let agents = value.as_array().expect("agents array");
        assert!(agents.iter().all(|item| item["id"] != "user"));

        let err = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "agent get user",
        )
        .expect_err("preset agent should be hidden");
        assert!(err.contains("agent not found: user"));
    }

    #[test]
    fn agent_export_should_return_structured_path() {
        let root = test_root();
        seed_app(&root);
        let export_path = root.join("agent-export.json");
        let output = run_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            &[
                "agent".to_string(),
                "export".to_string(),
                "agent-a".to_string(),
                export_path.display().to_string(),
            ],
        )
        .expect("export agent");
        let value: JsonValue = serde_json::from_str(&output).expect("parse export output");
        assert_eq!(value["path"], export_path.to_string_lossy().to_string());
    }

    #[test]
    fn split_command_line_should_keep_windows_backslashes_in_export_path() {
        let root = test_root();
        seed_app(&root);
        let export_path = root.join(".tmp").join("test-agent-v2.json");
        let command = format!("agent export agent-a {}", export_path.display());

        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            &command,
        )
        .expect("export agent with windows path");
        let value: JsonValue = serde_json::from_str(&output).expect("parse export output");
        assert_eq!(value["path"], export_path.to_string_lossy().to_string());
        assert!(export_path.exists());
    }

    #[test]
    fn agent_diff_should_detect_system_prompt_change() {
        let root = test_root();
        seed_app(&root);
        let export_path = root.join("agent-diff.json");
        run_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            &[
                "agent".to_string(),
                "export".to_string(),
                "agent-a".to_string(),
                export_path.display().to_string(),
            ],
        )
        .expect("export agent");
        let mut agent = read_json_file::<AgentProfile>(&export_path).expect("read exported agent");
        agent.system_prompt = "你是一个完整的新人格设定。".to_string();
        write_json_file(&export_path, &agent).expect("write updated agent");

        let output = run_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            &[
                "agent".to_string(),
                "diff".to_string(),
                "agent-a".to_string(),
                export_path.display().to_string(),
            ],
        )
        .expect("diff agent");
        let value: JsonValue = serde_json::from_str(&output).expect("parse diff output");
        let changed_fields = value["changedFields"]
            .as_array()
            .expect("changedFields array");
        assert!(changed_fields.iter().any(|item| item == "systemPrompt"));
    }

    #[allow(dead_code)]
    fn provider_update_should_preserve_redacted_keys_when_reenabled() {
        let root = test_root();
        seed_app(&root);
        let exported = root.join("provider.json");
        run_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            &[
                "provider".to_string(),
                "export".to_string(),
                "provider-a".to_string(),
                exported.display().to_string(),
            ],
        )
        .expect("export provider");
        let mut provider = read_json_file::<ApiProviderConfig>(&exported).expect("read exported provider");
        provider.name = "Provider Renamed".to_string();
        write_json_file(&exported, &provider).expect("write exported provider");
        let output = run_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            &[
                "provider".to_string(),
                "update".to_string(),
                "provider-a".to_string(),
                exported.display().to_string(),
            ],
        )
        .expect("update provider");
        assert!(!output.contains("sk-test-secret"));
        let config = fs::read_to_string(root.join("app_config.toml")).expect("read config");
        assert!(config.contains("Provider Renamed"));
        assert!(config.contains("sk-test-secret"));
    }

    #[test]
    fn provider_command_should_be_blocked_from_config_parser() {
        let root = test_root();
        seed_app(&root);
        let err = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "provider ls",
        )
        .expect_err("provider command should be blocked");
        assert!(err.contains("provider 命令当前未开放"));
    }

    #[test]
    fn mcp_ls_should_read_workspace_mcp_servers() {
        let root = test_root();
        seed_app(&root);
        let servers_dir = root.join("llm-workspace").join("mcp").join("servers");
        fs::create_dir_all(&servers_dir).expect("create mcp servers dir");
        fs::write(
            servers_dir.join("playwright.json"),
            r#"{
  "mcpServers": {
    "playwright": {
      "command": "npx",
      "args": ["@playwright/mcp@latest"]
    }
  }
}"#,
        )
        .expect("write mcp server");

        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "mcp ls",
        )
        .expect("run mcp ls");
        let value: JsonValue = serde_json::from_str(&output).expect("parse output");
        assert_eq!(value.as_array().expect("mcp array").len(), 1);
        assert_eq!(value[0]["id"], "playwright");
        assert_eq!(value[0]["name"], "playwright");
        assert!(value[0].get("transport").is_none());
        assert!(value[0].get("command").is_none());
    }

    #[test]
    fn mcp_ls_should_use_configured_system_workspace() {
        let root = test_root();
        seed_app(&root);
        let custom_workspace = root.join("custom-workspace");
        let mut config = fs::read_to_string(root.join("app_config.toml")).expect("read config");
        config.push_str(&format!(
            r#"

[[shellWorkspaces]]
name = "custom"
path = "{}"
level = "system"
"#,
            custom_workspace.to_string_lossy().replace('\\', "\\\\")
        ));
        fs::write(root.join("app_config.toml"), config).expect("write config");
        let servers_dir = custom_workspace.join("mcp").join("servers");
        fs::create_dir_all(&servers_dir).expect("create mcp servers dir");
        fs::write(
            servers_dir.join("browser.json"),
            r#"{
  "mcpServers": {
    "browser": {
      "command": "npx",
      "args": ["@browser/mcp@latest"]
    }
  }
}"#,
        )
        .expect("write mcp server");

        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "mcp ls",
        )
        .expect("run mcp ls");
        let value: JsonValue = serde_json::from_str(&output).expect("parse output");
        assert_eq!(value.as_array().expect("mcp array").len(), 1);
        assert_eq!(value[0]["id"], "browser");
        assert_eq!(value[0]["name"], "browser");
    }

    /// 造一个带两个人格（agent-a / agent-b）与内置 leader 的测试根目录。
    fn seed_two_agents(root: &Path) {
        seed_app(root);
        fs::write(
            root.join("config").join("agents.json"),
            r#"
{
  "agents": [
    {
      "id": "agent-a",
      "name": "Agent A",
      "systemPrompt": "Prompt A",
      "summary": "A 的简介",
      "source": "main_config",
      "scope": "global"
    },
    {
      "id": "agent-b",
      "name": "Agent B",
      "systemPrompt": "Prompt B",
      "source": "main_config",
      "scope": "global"
    },
    {
      "id": "support",
      "name": "support",
      "systemPrompt": "内置 support",
      "residentSkillNames": ["support"],
      "source": "main_config",
      "scope": "global"
    }
  ]
}
"#,
        )
        .expect("write agents");
    }

    fn read_agents_json(root: &Path) -> JsonValue {
        let raw = fs::read_to_string(root.join("config").join("agents.json")).expect("read agents");
        serde_json::from_str(&raw).expect("parse agents")
    }

    #[test]
    fn set_parent_and_clear_parent_should_edit_tree_edges() {
        let root = test_root();
        seed_two_agents(&root);
        let run = |command: &str| {
            run_command_with_paths(
                root.clone(),
                root.join("app_config.toml"),
                root.join("config_mark"),
                root.join("llm-workspace"),
                command,
            )
        };

        run("agent set-parent agent-b agent-a").expect("set-parent");
        let agents = read_agents_json(&root);
        let children = agents["agents"]
            .as_array()
            .expect("array")
            .iter()
            .find(|agent| agent["id"] == "agent-a")
            .expect("agent-a")["childAgentIds"]
            .as_array()
            .expect("childAgentIds")
            .iter()
            .map(|value| value.as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert_eq!(children, vec!["agent-b".to_string()]);

        let parents_output = run("agent parent agent-b").expect("parent");
        let parents: JsonValue = serde_json::from_str(&parents_output).expect("parse parents");
        assert_eq!(parents[0]["id"], "agent-a");

        let children_output = run("agent children agent-a").expect("children");
        let listed: JsonValue = serde_json::from_str(&children_output).expect("parse children");
        assert_eq!(listed[0]["id"], "agent-b");

        run("agent clear-parent agent-b").expect("clear-parent");
        let agents = read_agents_json(&root);
        assert!(agents["agents"]
            .as_array()
            .expect("array")
            .iter()
            .find(|agent| agent["id"] == "agent-a")
            .expect("agent-a")["childAgentIds"]
            .as_array()
            .expect("childAgentIds")
            .is_empty());
    }

    #[test]
    fn set_parent_should_reject_self_reference() {
        let root = test_root();
        seed_two_agents(&root);
        let err = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "agent set-parent agent-a agent-a",
        )
        .expect_err("self parent must fail");
        assert!(err.contains("不能自指"), "unexpected: {err}");
    }

    #[test]
    fn set_model_class_should_write_role_endpoint() {
        let root = test_root();
        seed_two_agents(&root);
        run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "agent set-model-class agent-a fast",
        )
        .expect("set-model-class");
        let agents = read_agents_json(&root);
        let ids = agents["agents"]
            .as_array()
            .expect("array")
            .iter()
            .find(|agent| agent["id"] == "agent-a")
            .expect("agent-a")["apiConfigIds"]
            .as_array()
            .expect("apiConfigIds")
            .iter()
            .map(|value| value.as_str().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["role:quick".to_string()]);
    }

    #[test]
    fn set_resident_should_be_locked_for_built_in_persona() {
        let root = test_root();
        seed_two_agents(&root);
        let err = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "agent set-resident support hr",
        )
        .expect_err("built-in persona skills are locked");
        assert!(err.contains("锁定的"), "unexpected: {err}");
    }

    #[test]
    fn set_resident_should_move_optional_to_resident() {
        let root = test_root();
        seed_two_agents(&root);
        let run = |command: &str| {
            run_command_with_paths(
                root.clone(),
                root.join("app_config.toml"),
                root.join("config_mark"),
                root.join("llm-workspace"),
                command,
            )
        };
        run("agent set-optional agent-a hr").expect("set-optional");
        run("agent set-resident agent-a hr").expect("set-resident");
        let agents = read_agents_json(&root);
        let agent = agents["agents"]
            .as_array()
            .expect("array")
            .iter()
            .find(|agent| agent["id"] == "agent-a")
            .expect("agent-a");
        assert_eq!(agent["residentSkillNames"][0], "hr");
        assert!(agent["optionalSkillNames"]
            .as_array()
            .map(|items| items.is_empty())
            .unwrap_or(true));
    }

    #[test]
    fn skill_ls_should_list_workspace_skills() {
        let root = test_root();
        seed_app(&root);
        let skill_dir = root.join("llm-workspace").join("skills").join("demo-skill");
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: demo-skill\ndescription: 示例技能\n---\n\n正文\n",
        )
        .expect("write SKILL.md");
        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "skill ls",
        )
        .expect("run skill ls");
        let value: JsonValue = serde_json::from_str(&output).expect("parse output");
        let items = value.as_array().expect("skill array");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["name"], "demo-skill");
        assert_eq!(items[0]["enabled"], true);
    }

    #[test]
    fn help_should_explain_config_tool_commands() {
        let root = test_root();
        seed_app(&root);
        let output = run_command_with_paths(
            root.clone(),
            root.join("app_config.toml"),
            root.join("config_mark"),
            root.join("llm-workspace"),
            "help",
        )
        .expect("run help");
        assert!(output.contains("PAI config"));
        assert!(output.contains("Use it when the user asks to modify PAI settings"));
        assert!(output.contains("agent update <name-or-id> <file>"));
        assert!(output.contains("agent tree"));
        assert!(output.contains("agent set-parent <child> <parent> [<parent>...]"));
        assert!(output.contains("agent set-model-class <name-or-id> <expert|fast>"));
        assert!(output.contains("agent set-resident <name-or-id> <skill-name> [<skill-name>...]"));
        assert!(output.contains("skill ls"));
        assert!(output.contains("mcp add <name> -- <command> [args...]"));
        assert!(output.contains("Delete commands are destructive"));
        assert!(output.contains("mcp delete <name-or-id> --confirmed"));
        assert!(!output.contains("skill update <name-or-id> <dir>"));
        assert!(!output.contains("skill delete <name-or-id> --confirmed"));
        assert!(!output.contains("provider ls"));
        assert!(!output.contains("Provider:"));
    }
}
