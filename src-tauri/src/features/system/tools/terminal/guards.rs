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

fn terminal_git_dangerous_block_reason(command: &str) -> Option<&'static str> {
    for simple in terminal_split_simple_commands(command) {
        let Some(first) = simple.argv.first() else {
            continue;
        };
        let base_cmd = terminal_unquote_token(first).to_ascii_lowercase();
        if base_cmd != "git" {
            continue;
        }
        let second = simple
            .argv
            .get(1)
            .map(|item| terminal_unquote_token(item).to_ascii_lowercase())
            .unwrap_or_default();
        let has_force_flag = simple.argv.iter().skip(2).any(|item| {
            let token = terminal_unquote_token(item).to_ascii_lowercase();
            matches!(token.as_str(), "-f" | "--force")
        });

        match second.as_str() {
            "push" if has_force_flag => {
                return Some("git push --force/-f is especially dangerous and is blocked");
            }
            "pull" if has_force_flag => {
                return Some("git pull --force/-f is especially dangerous and is blocked");
            }
            "push" => return Some("git push is blocked"),
            "pull" => return Some("git pull is blocked"),
            "fetch" => return Some("git fetch is blocked"),
            "commit" => return Some("git commit is blocked"),
            "merge" => return Some("git merge is blocked"),
            "rebase" => return Some("git rebase is blocked"),
            "reset" => return Some("git reset is blocked"),
            "checkout" => return Some("git checkout is blocked"),
            "switch" => return Some("git switch is blocked"),
            "restore" => return Some("git restore is blocked"),
            "clean" => return Some("git clean is blocked"),
            "stash" => return Some("git stash is blocked"),
            "apply" => return Some("git apply is blocked"),
            _ => {}
        }
    }
    None
}

fn terminal_command_block_reason(command: &str) -> Option<&'static str> {
    if terminal_is_powershell_encoded_command(command) {
        return Some("encoded command is blocked");
    }
    if let Some(reason) = terminal_git_dangerous_block_reason(command) {
        return Some(reason);
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

fn terminal_decode_with_encoding(
    bytes: &[u8],
    encoding: &'static encoding_rs::Encoding,
) -> Option<String> {
    let (decoded, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        return None;
    }
    Some(decoded.into_owned())
}

#[cfg(target_os = "windows")]
fn terminal_windows_system_encoding() -> Option<&'static encoding_rs::Encoding> {
    use windows_sys::Win32::Globalization::GetACP;

    let label = match unsafe { GetACP() } {
        936 => b"gbk".as_slice(),
        65001 => b"utf-8".as_slice(),
        1250 => b"windows-1250".as_slice(),
        1251 => b"windows-1251".as_slice(),
        1252 => b"windows-1252".as_slice(),
        1253 => b"windows-1253".as_slice(),
        1254 => b"windows-1254".as_slice(),
        1255 => b"windows-1255".as_slice(),
        1256 => b"windows-1256".as_slice(),
        1257 => b"windows-1257".as_slice(),
        1258 => b"windows-1258".as_slice(),
        874 => b"windows-874".as_slice(),
        932 => b"shift_jis".as_slice(),
        949 => b"euc-kr".as_slice(),
        950 => b"big5".as_slice(),
        866 => b"ibm866".as_slice(),
        437 => b"ibm437".as_slice(),
        850 => b"ibm850".as_slice(),
        _ => return None,
    };
    encoding_rs::Encoding::for_label(label)
}

fn terminal_detect_output_encoding(bytes: &[u8]) -> Option<&'static encoding_rs::Encoding> {
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    let (encoding, _) = detector.guess_assess(None, true);
    Some(encoding)
}

fn terminal_decode_output_bytes(bytes: &[u8]) -> String {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(encoding) = terminal_windows_system_encoding() {
            if let Some(decoded) = terminal_decode_with_encoding(bytes, encoding) {
                return decoded;
            }
        }
    }

    if let Some(encoding) = terminal_detect_output_encoding(bytes) {
        if let Some(decoded) = terminal_decode_with_encoding(bytes, encoding) {
            return decoded;
        }
    }

    #[cfg(target_os = "windows")]
    if let Some(decoded) = terminal_decode_with_encoding(bytes, encoding_rs::GBK) {
        return decoded;
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

    #[test]
    fn detect_windows_1252_punctuation_should_not_become_garbled() {
        let bytes = [0x93, b'H', b'e', b'l', b'l', b'o', 0x94];
        assert_eq!(terminal_decode_output_bytes(&bytes), "“Hello”");
    }

    #[test]
    fn git_push_force_should_be_blocked_as_high_risk() {
        assert_eq!(
            terminal_command_block_reason("git push --force origin main"),
            Some("git push --force/-f is especially dangerous and is blocked")
        );
    }

    #[test]
    fn git_pull_should_be_blocked() {
        assert_eq!(
            terminal_command_block_reason("git pull origin main"),
            Some("git pull is blocked")
        );
    }
}
