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

  function matchesRecordHotkey(event: KeyboardEvent): boolean {
    const recordHotkey = options.getRecordHotkey();
    if (recordHotkey === "Alt") return event.key === "Alt";
    if (recordHotkey === "Ctrl") return event.key === "Control";
    if (recordHotkey === "Shift") return event.key === "Shift";
    return false;
  }

  function hasHotkeyConflict(): boolean {
    const summonHotkey = (options.getSummonHotkey() || "").trim().toUpperCase();
    const recordHotkey = (options.getRecordHotkey() || "").trim().toUpperCase();
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
      if (!matchesRecordHotkey(event)) return;
      if (hasHotkeyConflict()) return;
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

