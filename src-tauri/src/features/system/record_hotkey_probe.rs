fn is_shortcut_match(shortcut: &Shortcut, raw_hotkey: &str) -> bool {
    match parse_hotkey(raw_hotkey) {
        Ok(parsed) => parsed == *shortcut,
        Err(_) => false,
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
static RECORD_HOTKEY_PROBE_STARTED: std::sync::OnceLock<std::sync::atomic::AtomicBool> =
    std::sync::OnceLock::new();
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
static RECORD_BACKGROUND_WAKE_ENABLED: std::sync::OnceLock<std::sync::atomic::AtomicBool> =
    std::sync::OnceLock::new();
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
static RECORD_HOTKEY_PROBE_EVENT_SEQ: std::sync::OnceLock<std::sync::atomic::AtomicU64> =
    std::sync::OnceLock::new();
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
static CHAT_WINDOW_ACTIVE: std::sync::OnceLock<std::sync::atomic::AtomicBool> =
    std::sync::OnceLock::new();
#[cfg(any(target_os = "macos", target_os = "linux"))]
static RECORD_HOTKEY_PROBE_STATE: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<RecordHotkeyProbeState>>> =
    std::sync::OnceLock::new();
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
static RECORD_HOTKEY_PROBE_PARSED: std::sync::OnceLock<
    std::sync::Arc<std::sync::Mutex<Option<ParsedRecordHotkey>>>,
> = std::sync::OnceLock::new();

fn handle_global_shortcut_probe(app: &AppHandle, shortcut: &Shortcut, state: ShortcutState) {
    if state != ShortcutState::Pressed {
        return;
    }
    let app_state = app.state::<AppState>();
    let config = match state_read_config_cached(app_state.inner()) {
        Ok(config) => config,
        Err(err) => {
            runtime_log_error(format!("[快捷键] 读取配置失败：error={err}"));
            return;
        }
    };
    if is_shortcut_match(shortcut, &config.hotkey) {
        let app_handle = app.clone();
        if let Err(err) = std::thread::Builder::new()
            .name("global-shortcut-chat-toggle".to_string())
            .spawn(move || {
                runtime_log_info(format!("[快捷键] 收到召唤热键，开始切换聊天窗口"));
                match toggle_window(&app_handle, "chat") {
                    Ok(()) => runtime_log_info(format!("[快捷键] 聊天窗口切换完成")),
                    Err(err) => runtime_log_error(format!("[快捷键] 聊天窗口切换失败：error={err}")),
                }
            })
        {
            runtime_log_error(format!("[快捷键] 调度聊天窗口切换失败：error={err}"));
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
#[derive(Debug, Clone)]
struct ParsedRecordHotkey {
    modifiers: std::collections::HashSet<String>,
    main: String,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[derive(Debug, Default)]
struct RecordHotkeyProbeState {
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
    active: bool,
    /// 主键是否处于按下状态（与 active 无关）：窗口不活跃、后台唤醒关闭时仍会记录，
    /// 用于保证「切屏后松手」也能上报 released。
    main_down: bool,
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn normalize_record_hotkey_text(raw: &str) -> String {
    let mut text = raw.trim().to_string();
    text = text.replace('＋', "+").replace('`', "·");
    text
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn parse_record_hotkey(raw: &str) -> Option<ParsedRecordHotkey> {
    let text = normalize_record_hotkey_text(raw);
    let tokens: Vec<String> = text
        .split('+')
        .map(|token| match token.trim().to_uppercase().as_str() {
            "OPTION" => "ALT".to_string(),
            "COMMAND" => "META".to_string(),
            other => other.to_string(),
        })
        .filter(|token| !token.is_empty())
        .collect();
    if tokens.is_empty() {
        return None;
    }
    let mut modifiers = std::collections::HashSet::<String>::new();
    let mut main: Option<String> = None;
    for token in &tokens {
        if token == "CTRL" || token == "ALT" || token == "SHIFT" || token == "META" {
            modifiers.insert(token.clone());
        } else if main.is_none() {
            main = Some(token.clone());
        }
    }
    if main.is_none() && tokens.len() == 1 {
        main = Some(tokens[0].clone());
    }
    Some(ParsedRecordHotkey {
        modifiers,
        main: main?,
    })
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn record_hotkey_modifier_display(token: &str) -> &'static str {
    match token {
        "CTRL" => "Ctrl",
        "ALT" => "Alt",
        "SHIFT" => "Shift",
        "META" => "Meta",
        _ => "",
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn record_hotkey_main_display(token: &str) -> String {
    match token {
        "CTRL" | "ALT" | "SHIFT" | "META" => record_hotkey_modifier_display(token).to_string(),
        "SPACE" => "Space".to_string(),
        "ENTER" => "Enter".to_string(),
        "TAB" => "Tab".to_string(),
        other => other.to_string(),
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn normalize_record_hotkey_label(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let parsed = parse_record_hotkey(trimmed)
        .ok_or_else(|| format!("录音热键格式无效：{trimmed}"))?;
    if parsed.modifiers.len() == 1 && parsed.modifiers.contains(&parsed.main) {
        return Ok(record_hotkey_main_display(&parsed.main));
    }
    let mut parts: Vec<String> = Vec::new();
    for token in ["CTRL", "ALT", "SHIFT", "META"] {
        if parsed.modifiers.contains(token) {
            parts.push(record_hotkey_modifier_display(token).to_string());
        }
    }
    parts.push(record_hotkey_main_display(&parsed.main));
    Ok(parts.join("+"))
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn record_hotkey_signature(raw: &str) -> Option<String> {
    let parsed = parse_record_hotkey(raw)?;
    if parsed.modifiers.len() == 1 && parsed.modifiers.contains(&parsed.main) {
        return Some(parsed.main);
    }
    let mut parts: Vec<String> = Vec::new();
    for token in ["CTRL", "ALT", "SHIFT", "META"] {
        if parsed.modifiers.contains(token) {
            parts.push(token.to_string());
        }
    }
    parts.push(parsed.main);
    Some(parts.join("+"))
}

#[cfg(target_os = "linux")]
fn modifier_token_from_key(key: rdev::Key) -> Option<&'static str> {
    match key {
        rdev::Key::ControlLeft | rdev::Key::ControlRight => Some("CTRL"),
        rdev::Key::Alt | rdev::Key::AltGr => Some("ALT"),
        rdev::Key::ShiftLeft | rdev::Key::ShiftRight => Some("SHIFT"),
        rdev::Key::MetaLeft | rdev::Key::MetaRight => Some("META"),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn token_from_key(key: rdev::Key) -> Option<String> {
    if let Some(token) = modifier_token_from_key(key) {
        return Some(token.to_string());
    }
    let token = match key {
        rdev::Key::BackQuote => "·",
        rdev::Key::KeyA => "A",
        rdev::Key::KeyB => "B",
        rdev::Key::KeyC => "C",
        rdev::Key::KeyD => "D",
        rdev::Key::KeyE => "E",
        rdev::Key::KeyF => "F",
        rdev::Key::KeyG => "G",
        rdev::Key::KeyH => "H",
        rdev::Key::KeyI => "I",
        rdev::Key::KeyJ => "J",
        rdev::Key::KeyK => "K",
        rdev::Key::KeyL => "L",
        rdev::Key::KeyM => "M",
        rdev::Key::KeyN => "N",
        rdev::Key::KeyO => "O",
        rdev::Key::KeyP => "P",
        rdev::Key::KeyQ => "Q",
        rdev::Key::KeyR => "R",
        rdev::Key::KeyS => "S",
        rdev::Key::KeyT => "T",
        rdev::Key::KeyU => "U",
        rdev::Key::KeyV => "V",
        rdev::Key::KeyW => "W",
        rdev::Key::KeyX => "X",
        rdev::Key::KeyY => "Y",
        rdev::Key::KeyZ => "Z",
        rdev::Key::Num0 => "0",
        rdev::Key::Num1 => "1",
        rdev::Key::Num2 => "2",
        rdev::Key::Num3 => "3",
        rdev::Key::Num4 => "4",
        rdev::Key::Num5 => "5",
        rdev::Key::Num6 => "6",
        rdev::Key::Num7 => "7",
        rdev::Key::Num8 => "8",
        rdev::Key::Num9 => "9",
        rdev::Key::F1 => "F1",
        rdev::Key::F2 => "F2",
        rdev::Key::F3 => "F3",
        rdev::Key::F4 => "F4",
        rdev::Key::F5 => "F5",
        rdev::Key::F6 => "F6",
        rdev::Key::F7 => "F7",
        rdev::Key::F8 => "F8",
        rdev::Key::F9 => "F9",
        rdev::Key::F10 => "F10",
        rdev::Key::F11 => "F11",
        rdev::Key::F12 => "F12",
        rdev::Key::Space => "SPACE",
        rdev::Key::Minus => "-",
        rdev::Key::Equal => "=",
        rdev::Key::LeftBracket => "[",
        rdev::Key::RightBracket => "]",
        rdev::Key::BackSlash => "\\",
        rdev::Key::SemiColon => ";",
        rdev::Key::Quote => "'",
        rdev::Key::Comma => ",",
        rdev::Key::Dot => ".",
        rdev::Key::Slash => "/",
        rdev::Key::Return => "ENTER",
        rdev::Key::Tab => "TAB",
        rdev::Key::CapsLock => "CAPSLOCK",
        _ => return None,
    };
    Some(token.to_string())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn modifiers_exact(
    state: &RecordHotkeyProbeState,
    required: &std::collections::HashSet<String>,
) -> bool {
    state.ctrl == required.contains("CTRL")
        && state.alt == required.contains("ALT")
        && state.shift == required.contains("SHIFT")
        && state.meta == required.contains("META")
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn set_modifier_state(state: &mut RecordHotkeyProbeState, token: &str, value: bool) {
    if token == "CTRL" {
        state.ctrl = value;
    } else if token == "ALT" {
        state.alt = value;
    } else if token == "SHIFT" {
        state.shift = value;
    } else if token == "META" {
        state.meta = value;
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn should_stop_on_release(parsed: &ParsedRecordHotkey, released_token: &str) -> bool {
    released_token == parsed.main || parsed.modifiers.contains(released_token)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn handle_record_hotkey_probe_key(
    app: &AppHandle,
    state_arc: &std::sync::Arc<std::sync::Mutex<RecordHotkeyProbeState>>,
    parsed_state: &std::sync::Arc<std::sync::Mutex<Option<ParsedRecordHotkey>>>,
    token: String,
    pressed: bool,
) {
    let parsed = match parsed_state.lock() {
        Ok(slot) => slot.clone(),
        Err(_) => return,
    };
    let Some(parsed) = parsed else { return };
    let Ok(mut state) = state_arc.lock() else { return };
    let modifier = ["CTRL", "ALT", "SHIFT", "META"]
        .iter()
        .find(|item| **item == token)
        .copied();
    let is_main = token == parsed.main;
    // 物理按键状态先记录，与「是否激活」解耦：即使窗口不活跃、后台唤醒关闭，
    // 主键松开时也要上报 released，让「切屏后松手」的录音同样能停下来。
    if let Some(modifier) = modifier {
        set_modifier_state(&mut state, modifier, pressed);
    }
    if pressed {
        if is_main {
            state.main_down = true;
        }
        if !is_record_hotkey_probe_background_wake_enabled() || is_record_hotkey_probe_chat_window_active() {
            return;
        }
        if state.active || !is_main || !modifiers_exact(&state, &parsed.modifiers) {
            return;
        }
        state.active = true;
        drop(state);
        emit_record_hotkey_probe_event(app, "pressed");
        return;
    }
    let should_emit_release =
        (state.active || state.main_down) && should_stop_on_release(&parsed, &token);
    if should_emit_release {
        state.active = false;
        state.main_down = false;
    }
    drop(state);
    if should_emit_release {
        emit_record_hotkey_probe_event(app, "released");
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
#[derive(Serialize, Clone)]
struct RecordHotkeyProbeEventPayload {
    state: &'static str,
    seq: u64,
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn emit_record_hotkey_probe_event(app: &AppHandle, state: &'static str) {
    let seq_counter = RECORD_HOTKEY_PROBE_EVENT_SEQ
        .get_or_init(|| std::sync::atomic::AtomicU64::new(0));
    let seq = seq_counter.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1;
    runtime_log_debug(format!("[录音热键] 上报 {state}，seq={seq}"));
    let payload = RecordHotkeyProbeEventPayload { state, seq };
    let _ = app.emit("easy-call:record-hotkey-probe", payload);
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn set_record_hotkey_probe_background_wake_enabled(enabled: bool) {
    let flag = RECORD_BACKGROUND_WAKE_ENABLED
        .get_or_init(|| std::sync::atomic::AtomicBool::new(enabled));
    flag.store(enabled, std::sync::atomic::Ordering::Release);
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn is_record_hotkey_probe_background_wake_enabled() -> bool {
    RECORD_BACKGROUND_WAKE_ENABLED
        .get()
        .map(|flag| flag.load(std::sync::atomic::Ordering::Acquire))
        .unwrap_or(false)
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn reset_record_hotkey_probe_state() {
    // Windows 走轮询，没有跨线程缓存的按键状态，无需复位。
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    if let Some(state_arc) = RECORD_HOTKEY_PROBE_STATE.get() {
        if let Ok(mut state) = state_arc.lock() {
            state.ctrl = false;
            state.alt = false;
            state.shift = false;
            state.meta = false;
            state.active = false;
            state.main_down = false;
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn reset_record_hotkey_probe_modifiers() {
    // Windows 走轮询，没有跨线程缓存的按键状态，无需复位。
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    if let Some(state_arc) = RECORD_HOTKEY_PROBE_STATE.get() {
        if let Ok(mut state) = state_arc.lock() {
            state.ctrl = false;
            state.alt = false;
            state.shift = false;
            state.meta = false;
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn set_record_hotkey_probe_hotkey(raw_hotkey: &str) -> Result<(), String> {
    let parsed = if raw_hotkey.trim().is_empty() {
        None
    } else {
        Some(
            parse_record_hotkey(raw_hotkey)
                .ok_or_else(|| format!("Parse record hotkey failed: {}", raw_hotkey.trim()))?,
        )
    };
    let parsed_slot = RECORD_HOTKEY_PROBE_PARSED
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)));
    let mut slot = parsed_slot
        .lock()
        .map_err(|_| "Lock record hotkey parsed state failed".to_string())?;
    *slot = parsed;
    drop(slot);
    reset_record_hotkey_probe_state();
    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn set_record_hotkey_probe_chat_window_active(active: bool) {
    let flag = CHAT_WINDOW_ACTIVE.get_or_init(|| std::sync::atomic::AtomicBool::new(false));
    let previous = flag.swap(active, std::sync::atomic::Ordering::AcqRel);
    if previous != active {
        reset_record_hotkey_probe_modifiers();
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn is_record_hotkey_probe_chat_window_active() -> bool {
    CHAT_WINDOW_ACTIVE
        .get()
        .map(|flag| flag.load(std::sync::atomic::Ordering::Acquire))
        .unwrap_or(false)
}

// ==================== Windows 录音热键轮询 ====================
// Windows 不使用 rdev 的低级钩子：钩子会偶发丢失 KeyRelease，而「同一个事实有两个来源」
// 正是「松手了还在录」的温床。这里按 global-hotkey 与同类 push-to-talk 项目的通行做法，
// 直接周期采样 GetAsyncKeyState，物理按键状态本身就是唯一真相。

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

#[cfg(target_os = "windows")]
const RECORD_HOTKEY_POLL_INTERVAL_MS: u64 = 10;

/// 录音热键 token → Win32 虚拟键码；未知 token 返回 None（不参与物理判断，避免误判为松开）。
#[cfg(target_os = "windows")]
fn record_hotkey_token_vk_code(token: &str) -> Option<i32> {
    match token {
        "CTRL" => Some(0x11),
        "ALT" => Some(0x12),
        "SHIFT" => Some(0x10),
        "META" => Some(0x5B),
        "SPACE" => Some(0x20),
        "ENTER" => Some(0x0D),
        "TAB" => Some(0x09),
        "CAPSLOCK" => Some(0x14),
        "-" => Some(0xBD),
        "=" => Some(0xBB),
        "[" => Some(0xDB),
        "]" => Some(0xDD),
        "\\" => Some(0xDC),
        ";" => Some(0xBA),
        "'" => Some(0xDE),
        "," => Some(0xBC),
        "." => Some(0xBE),
        "/" => Some(0xBF),
        "·" => Some(0xC0),
        other => {
            let bytes = other.as_bytes();
            if bytes.len() == 1 {
                let ch = bytes[0];
                if ch.is_ascii_uppercase() || ch.is_ascii_digit() {
                    return Some(ch as i32);
                }
            }
            if let Some(rest) = other.strip_prefix('F') {
                if let Ok(index) = rest.parse::<u8>() {
                    if (1..=12).contains(&index) {
                        return Some(0x6F + index as i32);
                    }
                }
            }
            None
        }
    }
}

#[cfg(target_os = "windows")]
fn vk_is_physically_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

#[cfg(target_os = "windows")]
fn is_token_physically_down(token: &str) -> bool {
    // Meta 需左右分别查询；其余按虚拟键码查询。
    if token == "META" {
        return vk_is_physically_down(0x5B) || vk_is_physically_down(0x5C);
    }
    // 未知按键一律按「仍按下」处理，宁可漏补一次也不误停一次。
    record_hotkey_token_vk_code(token)
        .map(vk_is_physically_down)
        .unwrap_or(true)
}

/// 严格匹配：热键要求的按键全部物理按下，且没有多余的修饰键被按下。
/// 查询方式以闭包注入，便于对匹配规则本身做单测。
#[cfg(target_os = "windows")]
fn record_hotkey_matches_physical_state(
    parsed: &ParsedRecordHotkey,
    is_down: &dyn Fn(&str) -> bool,
) -> bool {
    if !is_down(parsed.main.as_str()) {
        return false;
    }
    for token in &parsed.modifiers {
        if !is_down(token.as_str()) {
            return false;
        }
    }
    // 多余的修饰键按下时不算命中（与事件路径原有的严格匹配语义一致）；
    // 主键本身若是修饰键则跳过，避免把它当成多余修饰键。
    for token in ["CTRL", "ALT", "SHIFT", "META"] {
        if parsed.modifiers.contains(token) || token == parsed.main {
            continue;
        }
        if is_down(token) {
            return false;
        }
    }
    true
}

#[cfg(target_os = "windows")]
fn is_record_hotkey_physically_down(parsed: &ParsedRecordHotkey) -> bool {
    record_hotkey_matches_physical_state(parsed, &is_token_physically_down)
}

#[cfg(target_os = "windows")]
fn start_record_hotkey_probe(app: AppHandle, config_path: std::path::PathBuf) -> Result<(), String> {
    let started = RECORD_HOTKEY_PROBE_STARTED
        .get_or_init(|| std::sync::atomic::AtomicBool::new(false));
    let swapped = started.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    );
    if swapped.is_err() {
        return Ok(());
    }

    let config = read_config(&config_path).unwrap_or_default();
    set_record_hotkey_probe_background_wake_enabled(config.record_background_wake_enabled);
    if let Err(err) = set_record_hotkey_probe_hotkey(&config.record_hotkey) {
        started.store(false, std::sync::atomic::Ordering::Release);
        return Err(err);
    }
    if config.record_hotkey.trim().is_empty() {
        started.store(false, std::sync::atomic::Ordering::Release);
        return Ok(());
    }
    let parsed_state = RECORD_HOTKEY_PROBE_PARSED
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)))
        .clone();
    set_record_hotkey_probe_chat_window_active(false);

    if let Err(err) = std::thread::Builder::new()
        .name("record-hotkey-poll".to_string())
        .spawn(move || run_record_hotkey_poll_loop(app, parsed_state))
    {
        started.store(false, std::sync::atomic::Ordering::Release);
        return Err(format!("启动录音热键轮询失败：{err}"));
    }
    Ok(())
}

/// Windows 的唯一输入源：每 `RECORD_HOTKEY_POLL_INTERVAL_MS` 采样一次物理按键状态，
/// 只在状态发生边沿变化时上报，避免重复事件。
#[cfg(target_os = "windows")]
fn run_record_hotkey_poll_loop(app: AppHandle, parsed_state: std::sync::Arc<std::sync::Mutex<Option<ParsedRecordHotkey>>>) {
    let mut prev_was_down = false;
    loop {
        std::thread::sleep(std::time::Duration::from_millis(RECORD_HOTKEY_POLL_INTERVAL_MS));
        let parsed = match parsed_state.lock() {
            Ok(slot) => slot.clone(),
            Err(_) => continue,
        };
        let Some(parsed) = parsed else {
            // 没有配置热键时保持静默，并把上一轮状态清掉。
            prev_was_down = false;
            continue;
        };
        let is_down = is_record_hotkey_physically_down(&parsed);
        if is_down == prev_was_down {
            continue;
        }
        prev_was_down = is_down;
        if is_down {
            // 上升沿：仅在后台唤醒开启且聊天窗口不活跃时激活，与事件路径语义一致。
            if is_record_hotkey_probe_background_wake_enabled()
                && !is_record_hotkey_probe_chat_window_active()
            {
                emit_record_hotkey_probe_event(&app, "pressed");
            }
        } else {
            // 下降沿：无条件上报松开。无论按下时是否被抑制，松手都必须让前端有机会停止录音。
            emit_record_hotkey_probe_event(&app, "released");
        }
    }
}

#[cfg(target_os = "linux")]
fn start_record_hotkey_probe(app: AppHandle, config_path: std::path::PathBuf) -> Result<(), String> {
    let started = RECORD_HOTKEY_PROBE_STARTED
        .get_or_init(|| std::sync::atomic::AtomicBool::new(false));
    let swapped = started.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    );
    if swapped.is_err() {
        return Ok(());
    }

    let config = read_config(&config_path).unwrap_or_default();
    set_record_hotkey_probe_background_wake_enabled(config.record_background_wake_enabled);
    if let Err(err) = set_record_hotkey_probe_hotkey(&config.record_hotkey) {
        started.store(false, std::sync::atomic::Ordering::Release);
        return Err(err);
    }
    if config.record_hotkey.trim().is_empty() {
        started.store(false, std::sync::atomic::Ordering::Release);
        return Ok(());
    }
    let state = std::sync::Arc::new(std::sync::Mutex::new(RecordHotkeyProbeState::default()));
    let _ = RECORD_HOTKEY_PROBE_STATE.set(state.clone());
    let parsed_state = RECORD_HOTKEY_PROBE_PARSED
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None)))
        .clone();
    set_record_hotkey_probe_chat_window_active(false);

    std::thread::spawn(move || {
        let state_for_callback = state.clone();
        let callback = move |event: rdev::Event| match event.event_type {
            rdev::EventType::KeyPress(key) => {
                if let Some(token) = token_from_key(key) {
                    handle_record_hotkey_probe_key(&app, &state_for_callback, &parsed_state, token, true);
                }
            }
            rdev::EventType::KeyRelease(key) => {
                if let Some(token) = token_from_key(key) {
                    handle_record_hotkey_probe_key(&app, &state_for_callback, &parsed_state, token, false);
                }
            }
            _ => {}
        };
        if let Err(err) = rdev::listen(callback) {
            runtime_log_error(format!("[RDEV-RECORD-PROBE] listen failed: {err:?}"));
        }
    });
    Ok(())
}

#[cfg(target_os = "macos")]
fn start_record_hotkey_probe(app: AppHandle, config_path: std::path::PathBuf) -> Result<(), String> {
    use core_foundation::runloop::CFRunLoop;
    use core_graphics::event::{
        CallbackResult, CGEventTap, CGEventTapLocation, CGEventTapOptions,
        CGEventTapPlacement, CGEventType, EventField,
    };

    let started = RECORD_HOTKEY_PROBE_STARTED.get_or_init(|| std::sync::atomic::AtomicBool::new(false));
    if started.compare_exchange(false, true, std::sync::atomic::Ordering::AcqRel, std::sync::atomic::Ordering::Acquire).is_err() {
        return Ok(());
    }
    let config = read_config(&config_path).unwrap_or_default();
    set_record_hotkey_probe_background_wake_enabled(config.record_background_wake_enabled);
    set_record_hotkey_probe_hotkey(&config.record_hotkey)?;
    if config.record_hotkey.trim().is_empty() {
        return Ok(());
    }
    let state = std::sync::Arc::new(std::sync::Mutex::new(RecordHotkeyProbeState::default()));
    let _ = RECORD_HOTKEY_PROBE_STATE.set(state.clone());
    let parsed_state = RECORD_HOTKEY_PROBE_PARSED.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(None))).clone();
    std::thread::Builder::new().name("macos-record-hotkey-probe".to_string()).spawn(move || {
        let install = CGEventTap::with_enabled(CGEventTapLocation::HID, CGEventTapPlacement::HeadInsertEventTap, CGEventTapOptions::ListenOnly, vec![CGEventType::KeyDown, CGEventType::KeyUp, CGEventType::FlagsChanged], move |_proxy, event_type, event| {
            if matches!(event_type, CGEventType::FlagsChanged) {
                handle_macos_record_hotkey_modifier_flags(&app, &state, &parsed_state, event.get_flags());
                return CallbackResult::Keep;
            }
            if !matches!(event_type, CGEventType::KeyDown | CGEventType::KeyUp) {
                return CallbackResult::Keep;
            }
            let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
            let token = macos_key_token(keycode);
            if let Some(token) = token {
                handle_record_hotkey_probe_key(
                    &app,
                    &state,
                    &parsed_state,
                    token,
                    matches!(event_type, CGEventType::KeyDown),
                );
            }
            CallbackResult::Keep
        }, CFRunLoop::run_current);
        if install.is_err() { runtime_log_error("[录音热键] macOS Event Tap 创建失败，请授予辅助功能权限".to_string()); }
    }).map_err(|err| format!("启动 macOS 录音热键监听失败：{err}"))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn handle_macos_record_hotkey_modifier_flags(
    app: &AppHandle,
    state_arc: &std::sync::Arc<std::sync::Mutex<RecordHotkeyProbeState>>,
    parsed_state: &std::sync::Arc<std::sync::Mutex<Option<ParsedRecordHotkey>>>,
    flags: core_graphics::event::CGEventFlags,
) {
    use core_graphics::event::CGEventFlags;

    let parsed = match parsed_state.lock() {
        Ok(slot) => slot.clone(),
        Err(_) => return,
    };
    let Some(parsed) = parsed else { return };
    let Ok(mut state) = state_arc.lock() else { return };
    let was_active = state.active;
    let was_exact = modifiers_exact(&state, &parsed.modifiers);
    state.ctrl = flags.contains(CGEventFlags::CGEventFlagControl);
    state.alt = flags.contains(CGEventFlags::CGEventFlagAlternate);
    state.shift = flags.contains(CGEventFlags::CGEventFlagShift);
    state.meta = flags.contains(CGEventFlags::CGEventFlagCommand);
    let is_exact = modifiers_exact(&state, &parsed.modifiers);
    let is_modifier_only = parsed.modifiers.contains(&parsed.main);
    let can_activate = is_record_hotkey_probe_background_wake_enabled()
        && !is_record_hotkey_probe_chat_window_active();
    let should_emit_pressed = can_activate && !was_active && !was_exact && is_exact && is_modifier_only;
    let should_emit_released = was_active && !is_exact;
    if should_emit_pressed {
        state.active = true;
    } else if should_emit_released {
        state.active = false;
    }
    drop(state);
    if should_emit_pressed {
        emit_record_hotkey_probe_event(app, "pressed");
    } else if should_emit_released {
        emit_record_hotkey_probe_event(app, "released");
    }
}

#[cfg(target_os = "macos")]
fn macos_key_token(keycode: u16) -> Option<String> {
    use core_graphics::event::KeyCode;

    let token = match keycode {
        KeyCode::ANSI_A => "A",
        KeyCode::ANSI_B => "B",
        KeyCode::ANSI_C => "C",
        KeyCode::ANSI_D => "D",
        KeyCode::ANSI_E => "E",
        KeyCode::ANSI_F => "F",
        KeyCode::ANSI_G => "G",
        KeyCode::ANSI_H => "H",
        KeyCode::ANSI_I => "I",
        KeyCode::ANSI_J => "J",
        KeyCode::ANSI_K => "K",
        KeyCode::ANSI_L => "L",
        KeyCode::ANSI_M => "M",
        KeyCode::ANSI_N => "N",
        KeyCode::ANSI_O => "O",
        KeyCode::ANSI_P => "P",
        KeyCode::ANSI_Q => "Q",
        KeyCode::ANSI_R => "R",
        KeyCode::ANSI_S => "S",
        KeyCode::ANSI_T => "T",
        KeyCode::ANSI_U => "U",
        KeyCode::ANSI_V => "V",
        KeyCode::ANSI_W => "W",
        KeyCode::ANSI_X => "X",
        KeyCode::ANSI_Y => "Y",
        KeyCode::ANSI_Z => "Z",
        KeyCode::ANSI_0 => "0",
        KeyCode::ANSI_1 => "1",
        KeyCode::ANSI_2 => "2",
        KeyCode::ANSI_3 => "3",
        KeyCode::ANSI_4 => "4",
        KeyCode::ANSI_5 => "5",
        KeyCode::ANSI_6 => "6",
        KeyCode::ANSI_7 => "7",
        KeyCode::ANSI_8 => "8",
        KeyCode::ANSI_9 => "9",
        KeyCode::ANSI_MINUS => "-",
        KeyCode::ANSI_EQUAL => "=",
        KeyCode::ANSI_LEFT_BRACKET => "[",
        KeyCode::ANSI_RIGHT_BRACKET => "]",
        KeyCode::ANSI_BACKSLASH => "\\",
        KeyCode::ANSI_SEMICOLON => ";",
        KeyCode::ANSI_QUOTE => "'",
        KeyCode::ANSI_COMMA => ",",
        KeyCode::ANSI_PERIOD => ".",
        KeyCode::ANSI_SLASH => "/",
        KeyCode::ANSI_GRAVE => "·",
        KeyCode::CAPS_LOCK => "CAPSLOCK",
        KeyCode::SPACE => "SPACE",
        KeyCode::RETURN => "ENTER",
        KeyCode::TAB => "TAB",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        _ => return None,
    };
    Some(token.to_string())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn start_record_hotkey_probe(_app: AppHandle, _config_path: std::path::PathBuf) -> Result<(), String> {
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn set_record_hotkey_probe_background_wake_enabled(_enabled: bool) {}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn set_record_hotkey_probe_chat_window_active(_active: bool) {}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn set_record_hotkey_probe_hotkey(_raw_hotkey: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod record_hotkey_probe_tests {
    use super::*;

    #[test]
    fn normalizes_modifier_only_record_hotkey() {
        assert_eq!(normalize_record_hotkey_label("option"), Ok("Alt".to_string()));
        assert_eq!(normalize_record_hotkey_label("Command"), Ok("Meta".to_string()));
    }

    #[test]
    fn normalizes_combined_record_hotkey() {
        assert_eq!(
            normalize_record_hotkey_label(" command + shift + k "),
            Ok("Shift+Meta+K".to_string())
        );
        assert_eq!(
            record_hotkey_signature("Shift+Meta+K"),
            Some("SHIFT+META+K".to_string())
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn maps_record_hotkey_tokens_to_win32_vk_codes() {
        assert_eq!(record_hotkey_token_vk_code("CAPSLOCK"), Some(0x14));
        assert_eq!(record_hotkey_token_vk_code("A"), Some(0x41));
        assert_eq!(record_hotkey_token_vk_code("Z"), Some(0x5A));
        assert_eq!(record_hotkey_token_vk_code("0"), Some(0x30));
        assert_eq!(record_hotkey_token_vk_code("9"), Some(0x39));
        assert_eq!(record_hotkey_token_vk_code("F1"), Some(0x70));
        assert_eq!(record_hotkey_token_vk_code("F12"), Some(0x7B));
        assert_eq!(record_hotkey_token_vk_code("SPACE"), Some(0x20));
        assert_eq!(record_hotkey_token_vk_code("CTRL"), Some(0x11));
        assert_eq!(record_hotkey_token_vk_code("·"), Some(0xC0));
        // 未知 token 不参与物理判断，避免把「无法识别」误判成已松开
        assert_eq!(record_hotkey_token_vk_code("UNKNOWN"), None);
        assert_eq!(record_hotkey_token_vk_code("F13"), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn unknown_token_is_treated_as_still_pressed() {
        assert!(is_token_physically_down("NOT_A_REAL_KEY"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn physical_match_requires_main_and_blocks_extra_modifiers() {
        let parsed = parse_record_hotkey("Ctrl+K").expect("parse record hotkey");
        // 主键未按下：不命中
        assert!(!record_hotkey_matches_physical_state(&parsed, &|_| false));
        // 要求的按键全按下、但没有多余修饰键：命中
        assert!(record_hotkey_matches_physical_state(&parsed, &|token| {
            token == "CTRL" || token == "K"
        }));
        // 多按了一个未要求的修饰键（ALT）：严格匹配拒绝
        assert!(!record_hotkey_matches_physical_state(&parsed, &|token| {
            token == "CTRL" || token == "K" || token == "ALT"
        }));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn physical_match_handles_single_key_hotkey() {
        let parsed = parse_record_hotkey("CapsLock").expect("parse record hotkey");
        assert!(record_hotkey_matches_physical_state(&parsed, &|token| {
            token == "CAPSLOCK"
        }));
        // 单键热键下多按 ctrl 同样不算命中
        assert!(!record_hotkey_matches_physical_state(&parsed, &|token| {
            token == "CAPSLOCK" || token == "CTRL"
        }));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn maps_macos_function_and_punctuation_keys() {
        assert_eq!(macos_key_token(0x7A), Some("F1".to_string()));
        assert_eq!(macos_key_token(0x60), Some("F5".to_string()));
        assert_eq!(macos_key_token(0x2F), Some(".".to_string()));
        assert_eq!(macos_key_token(0x1B), Some("-".to_string()));
    }
}
