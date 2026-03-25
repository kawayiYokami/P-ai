fn terminal_is_powershell_encoded_command(command: &str) -> bool {
    let tokens = terminal_tokenize(command);
    if tokens.is_empty() {
        return false;
    }

    let mut saw_powershell = false;
    let mut saw_encoded_flag = false;
    for token in tokens {
        let unquoted = terminal_unquote_token(&token);
        let exe_name = unquoted
            .rsplit(['\\', '/'])
            .next()
            .unwrap_or(unquoted.as_str());
        let lower = exe_name.to_ascii_lowercase();
        let lower_full = unquoted.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
        ) {
            saw_powershell = true;
        }
        if matches!(lower_full.as_str(), "-encodedcommand" | "-enc" | "-e")
            || lower_full.starts_with("-encodedcommand:")
            || lower_full.starts_with("-enc:")
            || lower_full.starts_with("-e:")
        {
            saw_encoded_flag = true;
        }
    }
    saw_powershell && saw_encoded_flag
}

fn terminal_command_block_reason(command: &str) -> Option<&'static str> {
    if terminal_is_powershell_encoded_command(command) {
        return Some("encoded command is blocked");
    }
    let lower = command.to_ascii_lowercase();
    if lower.contains("invoke-expression") || lower.contains("iex ") || lower.contains("iex(") {
        return Some("Invoke-Expression/iex is blocked");
    }
    if lower.contains("start-process")
        && (lower.contains("powershell")
            || lower.contains("pwsh")
            || lower.contains("cmd.exe")
            || lower.contains("/bin/sh")
            || lower.contains("/bin/bash"))
    {
        return Some("spawning nested shells is blocked");
    }
    None
}

fn terminal_decode_output_bytes(bytes: &[u8]) -> String {
    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
        return text;
    }
    #[cfg(target_os = "windows")]
    {
        let (decoded, _, had_errors) = encoding_rs::GBK.decode(bytes);
        if !had_errors {
            return decoded.into_owned();
        }
    }
    String::from_utf8_lossy(bytes).to_string()
}

fn truncate_terminal_output(bytes: &[u8]) -> (String, bool) {
    if bytes.len() <= TERMINAL_MAX_OUTPUT_BYTES {
        return (terminal_decode_output_bytes(bytes), false);
    }
    (
        terminal_decode_output_bytes(&bytes[..TERMINAL_MAX_OUTPUT_BYTES]),
        true,
    )
}

fn terminal_is_timeout_error(err: &str) -> bool {
    err.to_ascii_lowercase().contains("timed out after")
}

#[cfg(test)]
mod terminal_output_decode_tests {
    use super::*;

    #[test]
    fn decode_utf8_output_should_keep_utf8_text() {
        assert_eq!(terminal_decode_output_bytes("中文".as_bytes()), "中文");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn decode_windows_gbk_output_should_fallback_to_gbk() {
        let bytes = [0xd6, 0xd0, 0xce, 0xc4];
        assert_eq!(terminal_decode_output_bytes(&bytes), "中文");
    }
}
