use std::{
    fs,
    path::{Path, PathBuf},
};

use easy_call_ai::pai_config_tool;
use serde_json::Value;
use uuid::Uuid;

fn test_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("pai-config-test-{}", Uuid::new_v4()));
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
apiKeys = []
cachedModelOptions = ["gpt-4o-mini", "gpt-4.1-mini"]

[[apiProviders.models]]
id = "model-a"
model = "gpt-4o-mini"
enableTools = true
reasoningEffort = "medium"
temperature = 1.0
contextWindowTokens = 128000
maxOutputTokens = 4096

[[apiProviders.models]]
id = "model-b"
model = "gpt-4.1-mini"
enableTools = true
reasoningEffort = "medium"
temperature = 1.0
contextWindowTokens = 128000
maxOutputTokens = 4096

[[apiProviders]]
id = "provider-b"
name = "Provider B"
requestFormat = "openai"
enableText = true
enableTools = true
baseUrl = "https://api.openai.com/v1"
apiKeys = []
cachedModelOptions = ["o4-mini"]

[[apiProviders.models]]
id = "model-c"
model = "o4-mini"
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
    },
    {
      "id": "agent-b",
      "name": "Agent B",
      "systemPrompt": "Prompt B",
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

fn run_cli(root: &Path, args: &[&str]) -> String {
    let args = args.iter().map(|value| (*value).to_string()).collect::<Vec<_>>();
    pai_config_tool::run_with_paths(
        root.to_path_buf(),
        root.join("app_config.toml"),
        root.join("config_mark"),
        root.join("llm-workspace"),
        &args,
    )
    .unwrap_or_else(|err| panic!("command failed: {args:?}\nerror={err}"))
}

fn run_cli_err(root: &Path, args: &[&str]) -> String {
    let args = args.iter().map(|value| (*value).to_string()).collect::<Vec<_>>();
    pai_config_tool::run_with_paths(
        root.to_path_buf(),
        root.join("app_config.toml"),
        root.join("config_mark"),
        root.join("llm-workspace"),
        &args,
    )
    .unwrap_err()
}

fn write_png(path: &Path) {
    let png: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8,
        6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 15, 4, 0, 9,
        251, 3, 253, 160, 109, 203, 182, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    fs::write(path, png).expect("write png");
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read json")).expect("parse json")
}

#[test]
fn agent_acceptance_commands_should_work() {
    let root = test_root();
    seed_app(&root);

    let new_output = run_cli(
        &root,
        &["agent", "new", "Example Agent", "你是一个直接可靠的助手。"],
    );
    let agent_file = root.join("agent.json");
    fs::write(&agent_file, &new_output).expect("write agent file");

    let _ = run_cli(&root, &["agent", "example"]);
    let _ = run_cli(&root, &["agent", "check", agent_file.to_str().unwrap()]);
    let _ = run_cli(
        &root,
        &["agent", "update", "agent-a", agent_file.to_str().unwrap()],
    );

    let avatar = root.join("avatar.png");
    write_png(&avatar);
    let _ = run_cli(
        &root,
        &["agent", "avatar", "agent-a", avatar.to_str().unwrap()],
    );

    let agents = read_json(&root.join("config").join("agents.json"));
    assert_eq!(agents["agents"][0]["name"], "Example Agent");
    assert!(agents["agents"][0]["avatarPath"].as_str().unwrap().contains("avatars"));
}

#[test]
fn mcp_acceptance_commands_should_work() {
    let root = test_root();
    seed_app(&root);

    let example_output = run_cli(&root, &["mcp", "example"]);
    let mcp_file = root.join("mcp.json");
    fs::write(&mcp_file, example_output).expect("write mcp file");

    let _ = run_cli(&root, &["mcp", "check", mcp_file.to_str().unwrap()]);
    let _ = run_cli(
        &root,
        &["mcp", "add", "playwright", "--", "npx", "@playwright/mcp@latest"],
    );
    let _ = run_cli(&root, &["mcp", "example"]);
    let _ = run_cli(&root, &["mcp", "enable", "playwright"]);
    let enabled_policy = fs::read_to_string(
        root.join("llm-workspace")
            .join("mcp")
            .join("policies")
            .join("playwright.json"),
    )
    .expect("read enabled policy");
    assert!(enabled_policy.contains("\"enabled\": true"));
    let _ = run_cli(
        &root,
        &["mcp", "update", "playwright", mcp_file.to_str().unwrap()],
    );
    let _ = run_cli(&root, &["mcp", "test", "playwright"]);
    let _ = run_cli(&root, &["mcp", "disable", "playwright"]);
    let disabled_policy = fs::read_to_string(
        root.join("llm-workspace")
            .join("mcp")
            .join("policies")
            .join("playwright.json"),
    )
    .expect("read disabled policy");
    assert!(disabled_policy.contains("\"enabled\": false"));

    let delete_err = run_cli_err(&root, &["mcp", "delete", "playwright"]);
    assert!(delete_err.contains("--confirmed"));
    let _ = run_cli(&root, &["mcp", "delete", "playwright", "--confirmed"]);
    assert!(
        !root
            .join("llm-workspace")
            .join("mcp")
            .join("servers")
            .join("playwright.json")
            .exists()
    );
    assert!(
        !root
            .join("llm-workspace")
            .join("mcp")
            .join("policies")
            .join("playwright.json")
            .exists()
    );
}
