type UseRecordHotkeyOptions = {
  isActive: () => boolean;
  getSummonHotkey: () => string;
  getRecordHotkey: () => string;
  onConflict: () => void;
  onStartRecording: () => void | Promise<void>;
  onStopRecording: (discard: boolean) => void | Promise<void>;
  startDelayMs?: number;
};

export function useRecordHotkey(options: UseRecordHotkeyOptions) {
  let keydownHandler: ((event: KeyboardEvent) => void) | null = null;
  let keyupHandler: ((event: KeyboardEvent) => void) | null = null;
  let startTimer: ReturnType<typeof setTimeout> | null = null;
  let hotkeyPressed = false;
  let suppressUntil = 0;
  let blockUntilRelease = false;
  let conflictHintShown = false;

  const startDelayMs = options.startDelayMs ?? 180;

  function clearStartTimer() {
    if (startTimer) {
      clearTimeout(startTimer);
      startTimer = null;
    }
  }

  function isModifierToken(token: string): boolean {
    return token === "CTRL" || token === "ALT" || token === "SHIFT" || token === "META";
  }

  function eventMainToken(event: KeyboardEvent): string {
    const code = (event.code || "").toUpperCase();
    if (code === "BACKQUOTE") return "·";
    if (code.startsWith("KEY") && code.length === 4) return code.slice(3);
    if (code.startsWith("DIGIT") && code.length === 6) return code.slice(5);
    if (/^F\d{1,2}$/.test(code)) return code;
    if (code === "SPACE") return "SPACE";
    if (code === "MINUS") return "-";
    if (code === "EQUAL") return "=";
    if (code === "BRACKETLEFT") return "[";
    if (code === "BRACKETRIGHT") return "]";
    if (code === "BACKSLASH") return "\\";
    if (code === "SEMICOLON") return ";";
    if (code === "QUOTE") return "'";
    if (code === "COMMA") return ",";
    if (code === "PERIOD") return ".";
    if (code === "SLASH") return "/";
    const key = (event.key || "").trim();
    return key.length === 1 ? key.toUpperCase() : key.toUpperCase();
  }

  function parseHotkey(raw: string): { modifiers: Set<string>; main: string | null } {
    const tokens = String(raw || "")
      .split("+")
      .map((token) => token.trim().toUpperCase())
      .filter((token) => !!token);
    const modifiers = new Set<string>();
    let main: string | null = null;
    for (const token of tokens) {
      if (isModifierToken(token)) {
        modifiers.add(token);
      } else if (!main) {
        main = token;
      }
    }
    if (!main && tokens.length === 1 && isModifierToken(tokens[0])) {
      main = tokens[0];
    }
    return { modifiers, main };
  }

  function normalizeHotkey(raw: string): string {
    const { modifiers, main } = parseHotkey(raw);
    const orderedMods = ["CTRL", "ALT", "SHIFT", "META"].filter((mod) => modifiers.has(mod));
    const parts = main ? [...orderedMods, main] : orderedMods;
    return parts.join("+");
  }

  function modifiersExactlyMatch(mods: Set<string>, event: KeyboardEvent): boolean {
    return (
      event.ctrlKey === mods.has("CTRL")
      && event.altKey === mods.has("ALT")
      && event.shiftKey === mods.has("SHIFT")
      && event.metaKey === mods.has("META")
    );
  }

  function isModifierKeyEvent(event: KeyboardEvent, token: string): boolean {
    if (token === "CTRL") return event.key === "Control";
    if (token === "ALT") return event.key === "Alt";
    if (token === "SHIFT") return event.key === "Shift";
    if (token === "META") return event.key === "Meta";
    return false;
  }

  function matchesRecordHotkey(event: KeyboardEvent): boolean {
    const parsed = parseHotkey(options.getRecordHotkey());
    if (!parsed.main) return false;
    if (!modifiersExactlyMatch(parsed.modifiers, event)) return false;
    if (isModifierToken(parsed.main)) {
      return isModifierKeyEvent(event, parsed.main);
    }
    return eventMainToken(event) === parsed.main;
  }

  function isRelevantRecordHotkeyKey(event: KeyboardEvent): boolean {
    const parsed = parseHotkey(options.getRecordHotkey());
    if (!parsed.main) return false;
    if (isModifierToken(parsed.main) && isModifierKeyEvent(event, parsed.main)) {
      return true;
    }
    if (!isModifierToken(parsed.main) && eventMainToken(event) === parsed.main) {
      return true;
    }
    if (parsed.modifiers.has("CTRL") && event.key === "Control") return true;
    if (parsed.modifiers.has("ALT") && event.key === "Alt") return true;
    if (parsed.modifiers.has("SHIFT") && event.key === "Shift") return true;
    if (parsed.modifiers.has("META") && event.key === "Meta") return true;
    return false;
  }

  function hasHotkeyConflict(): boolean {
    const summonHotkey = normalizeHotkey(options.getSummonHotkey());
    const recordHotkey = normalizeHotkey(options.getRecordHotkey());
    if (!summonHotkey || !recordHotkey) return false;
    return summonHotkey === recordHotkey;
  }

  function mount() {
    if (keydownHandler || keyupHandler) return;

    keydownHandler = (event: KeyboardEvent) => {
      if (!options.isActive()) return;
      if (!matchesRecordHotkey(event)) return;
      if (hasHotkeyConflict()) {
        if (!conflictHintShown) {
          options.onConflict();
          conflictHintShown = true;
        }
        return;
      }
      conflictHintShown = false;
      if (event.repeat) return;
      if (Date.now() < suppressUntil) return;
      if (blockUntilRelease) return;
      event.preventDefault();
      hotkeyPressed = true;
      clearStartTimer();
      startTimer = setTimeout(() => {
        if (!hotkeyPressed) return;
        if (Date.now() < suppressUntil) return;
        void options.onStartRecording();
      }, startDelayMs);
    };

    keyupHandler = (event: KeyboardEvent) => {
      if (!options.isActive()) return;
      if (hasHotkeyConflict()) return;
      if (!isRelevantRecordHotkeyKey(event)) return;
      if (blockUntilRelease) {
        blockUntilRelease = false;
        hotkeyPressed = false;
        clearStartTimer();
        return;
      }
      event.preventDefault();
      hotkeyPressed = false;
      clearStartTimer();
      void options.onStopRecording(false);
    };

    window.addEventListener("keydown", keydownHandler);
    window.addEventListener("keyup", keyupHandler);
  }

  function suppressAfterPopup(durationMs: number) {
    suppressUntil = Date.now() + durationMs;
    blockUntilRelease = true;
    hotkeyPressed = false;
    clearStartTimer();
  }

  function unmount() {
    clearStartTimer();
    if (keydownHandler) {
      window.removeEventListener("keydown", keydownHandler);
      keydownHandler = null;
    }
    if (keyupHandler) {
      window.removeEventListener("keyup", keyupHandler);
      keyupHandler = null;
    }
  }

  return {
    mount,
    unmount,
    suppressAfterPopup,
  };
}
