use std::path::Path;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;

const TERMINAL_MAX_OUTPUT_BYTES: usize = 256 * 1024;
const TERMINAL_DEFAULT_TIMEOUT_MS: u64 = 20_000;
const TERMINAL_MAX_TIMEOUT_MS: u64 = 120_000;

include!("terminal/runtime.rs");

include!("terminal/workspace.rs");

include!("terminal/approval.rs");

include!("terminal/guards.rs");

include!("terminal/exec.rs");
