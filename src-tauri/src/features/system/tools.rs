#[path = "tools/platform/mod.rs"]
mod platform;
include!("tools/types.rs");
include!("tools/image_normalizer_for_llm_request.rs");
include!("tools/xcap_screenshot.rs");
include!("tools/ui_automation.rs");
include!("tools/operate_parser.rs");
include!("tools/operate_blocking.rs");
include!("tools/operate_actions.rs");
include!("tools/operate_runner.rs");
include!("tools/windows_tool.rs");
include!("tools/screenshot_mcp.rs");
include!("tools/macos_tcc.rs");
include!("tools/terminal.rs");
include!("tools/text_codec.rs");
include!("tools/patch.rs");
include!("tools/patch_rewind.rs");
include!("tools/read_file.rs");
include!("tools/todo_mcp.rs");
