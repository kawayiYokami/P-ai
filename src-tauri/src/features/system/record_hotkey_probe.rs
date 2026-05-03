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
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
static RECORD_HOTKEY_PROBE_STATE: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<RecordHotkeyProbeState>>> =
    std::sync::OnceLock::new();

fn handle_global_shortcut_probe(app: &AppHandle, shortcut: &Shortcut, state: ShortcutState) {
    if state != ShortcutState::Pressed {
        return;
    }
    let app_state = app.state::<AppState>();
    let config = read_config(&app_state.config_path).unwrap_or_default();
    if is_shortcut_match(shortcut, &config.hotkey) {
        let _ = toggle_window(app, "chat");
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
#[derive(Debug, Clone)]
struct ParsedRecordHotkey {
    modifiers: std::collections::HashSet<String>,
    main: String,
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
#[derive(Debug, Default)]
struct RecordHotkeyProbeState {
    ctrl: bool,
    alt: bool,
    shift: bool,
    meta: bool,
    active: bool,
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
fn modifier_token_from_key(key: rdev::Key) -> Option<&'static str> {
    match key {
        rdev::Key::ControlLeft | rdev::Key::ControlRight => Some("CTRL"),
        rdev::Key::Alt | rdev::Key::AltGr => Some("ALT"),
        rdev::Key::ShiftLeft | rdev::Key::ShiftRight => Some("SHIFT"),
        rdev::Key::MetaLeft | rdev::Key::MetaRight => Some("META"),
        _ => None,
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
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
        _ => return None,
    };
    Some(token.to_string())
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn modifiers_exact(
    state: &RecordHotkeyProbeState,
    required: &std::collections::HashSet<String>,
) -> bool {
    state.ctrl == required.contains("CTRL")
        && state.alt == required.contains("ALT")
        && state.shift == required.contains("SHIFT")
        && state.meta == required.contains("META")
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
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

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn should_stop_on_release(parsed: &ParsedRecordHotkey, released_token: &str) -> bool {
    released_token == parsed.main || parsed.modifiers.contains(released_token)
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
    if let Some(state_arc) = RECORD_HOTKEY_PROBE_STATE.get() {
        if let Ok(mut state) = state_arc.lock() {
            state.ctrl = false;
            state.alt = false;
            state.shift = false;
            state.meta = false;
            state.active = false;
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn set_record_hotkey_probe_chat_window_active(active: bool) {
    let flag = CHAT_WINDOW_ACTIVE.get_or_init(|| std::sync::atomic::AtomicBool::new(false));
    let previous = flag.swap(active, std::sync::atomic::Ordering::AcqRel);
    if previous != active {
        reset_record_hotkey_probe_state();
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
fn is_record_hotkey_probe_chat_window_active() -> bool {
    CHAT_WINDOW_ACTIVE
        .get()
        .map(|flag| flag.load(std::sync::atomic::Ordering::Acquire))
        .unwrap_or(false)
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
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
    let parsed = match parse_record_hotkey(&config.record_hotkey) {
        Some(parsed) => parsed,
        None if config.record_hotkey.trim().is_empty() => {
            started.store(false, std::sync::atomic::Ordering::Release);
            return Ok(());
        }
        None => {
            started.store(false, std::sync::atomic::Ordering::Release);
            return Err(format!(
                "Parse record hotkey failed: {}",
                config.record_hotkey
            ));
        }
    };
    let state = std::sync::Arc::new(std::sync::Mutex::new(RecordHotkeyProbeState::default()));
    let _ = RECORD_HOTKEY_PROBE_STATE.set(state.clone());
    set_record_hotkey_probe_chat_window_active(false);

    std::thread::spawn(move || {
        let state_for_callback = state.clone();
        let callback = move |event: rdev::Event| match event.event_type {
            rdev::EventType::KeyPress(key) => {
                if !is_record_hotkey_probe_background_wake_enabled() {
                    return;
                }
                if is_record_hotkey_probe_chat_window_active() {
                    return;
                }
                let Some(token) = token_from_key(key) else {
                    return;
                };
                let Ok(mut state) = state_for_callback.lock() else {
                    return;
                };
                if let Some(modifier) = modifier_token_from_key(key) {
                    set_modifier_state(&mut state, modifier, true);
                }
                if state.active {
                    return;
                }
                if token != parsed.main {
                    return;
                }
                if !modifiers_exact(&state, &parsed.modifiers) {
                    return;
                }
                state.active = true;
                drop(state);
                emit_record_hotkey_probe_event(&app, "pressed");
            }
            rdev::EventType::KeyRelease(key) => {
                let Some(token) = token_from_key(key) else {
                    return;
                };
                let mut should_emit_release = false;
                let Ok(mut state) = state_for_callback.lock() else {
                    return;
                };
                if state.active && should_stop_on_release(&parsed, &token) {
                    state.active = false;
                    should_emit_release = true;
                }
                if let Some(modifier) = modifier_token_from_key(key) {
                    set_modifier_state(&mut state, modifier, false);
                }
                drop(state);
                if should_emit_release {
                    emit_record_hotkey_probe_event(&app, "released");
                }
            }
            _ => {}
        };
        if let Err(err) = rdev::listen(callback) {
            eprintln!("[RDEV-RECORD-PROBE] listen failed: {err:?}");
        }
    });
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn start_record_hotkey_probe(_app: AppHandle, _config_path: std::path::PathBuf) -> Result<(), String> {
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn set_record_hotkey_probe_background_wake_enabled(_enabled: bool) {}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn set_record_hotkey_probe_chat_window_active(_active: bool) {}
