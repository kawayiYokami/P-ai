import { ref, watch, type Ref } from "vue";
import type { AppConfig } from "../../../types/app";
import { appendTransportProbeLog } from "../../../services/tauri-api";
import { useRecordHotkey } from "./use-record-hotkey";

type RecordingActivationSource = "foreground" | "background";

type UseChatWindowRecordingOrchestratorOptions = {
  viewMode: Ref<"chat" | "archives" | "config">;
  config: AppConfig;
  recording: Ref<boolean>;
  recordHotkeyProbeLastSeq: Ref<number>;
  recordHotkeyProbeDown: Ref<boolean>;
  startSpeechRecording: () => Promise<unknown>;
  stopSpeechRecording: (discard: boolean) => Promise<unknown>;
  prewarmMicrophone: () => Promise<unknown>;
};

export function useChatWindowRecordingOrchestrator(options: UseChatWindowRecordingOrchestratorOptions) {
  const CHAT_WINDOW_MIC_PREWARM_DEBOUNCE_MS = 260;
  const foregroundRecordingActive = ref(false);
  let chatMicPrewarmTimer: ReturnType<typeof setTimeout> | null = null;

  function clearChatMicPrewarmTimer() {
    if (!chatMicPrewarmTimer) return;
    clearTimeout(chatMicPrewarmTimer);
    chatMicPrewarmTimer = null;
  }

  async function tryPrewarmChatMic(reason: string) {
    if (options.viewMode.value !== "chat") return;
    if (document.visibilityState === "hidden") return;
    if (!document.hasFocus()) return;
    void reason;
    await options.prewarmMicrophone();
  }

  function isChatWindowActiveNow(): boolean {
    return options.viewMode.value === "chat" && document.visibilityState === "visible" && document.hasFocus();
  }

  async function startRecording(source: RecordingActivationSource = "foreground") {
    appendTransportProbeLog("[录音]", "收到开始", {
      source,
      recording: options.recording.value,
      chatActive: isChatWindowActiveNow(),
    });
    if (!options.recording.value) {
      foregroundRecordingActive.value = source === "foreground" && isChatWindowActiveNow();
    }
    await options.startSpeechRecording();
    if (!options.recording.value) {
      foregroundRecordingActive.value = false;
    }
  }

  async function stopRecording(discard: boolean) {
    appendTransportProbeLog("[录音]", "收到停止", {
      discard,
      recording: options.recording.value,
      foregroundActive: foregroundRecordingActive.value,
    });
    foregroundRecordingActive.value = false;
    await options.stopSpeechRecording(discard);
  }

  const recordHotkey = useRecordHotkey({
    isActive: () => isChatWindowActiveNow(),
    getRecordHotkey: () => options.config.recordHotkey,
    onStartRecording: (source) => startRecording(source),
    onStopRecording: (discard) => stopRecording(discard),
    startDelayMs: 0,
  });

  function cancelForegroundRecordingOnBackground(reason: string) {
    appendTransportProbeLog("[录音]", "失焦取消检查", {
      reason,
      foregroundActive: foregroundRecordingActive.value,
      recording: options.recording.value,
    });
    if (!foregroundRecordingActive.value) return;
    foregroundRecordingActive.value = false;
    recordHotkey.resetPressedState();
    void options.stopSpeechRecording(true);
  }

  watch(options.recording, (active) => {
    if (!active) {
      foregroundRecordingActive.value = false;
    }
  });

  function clearRecordHotkeyProbeState() {
    options.recordHotkeyProbeDown.value = false;
    options.recordHotkeyProbeLastSeq.value = 0;
  }

  function scheduleChatMicPrewarm(reason: string, delayMs = 0) {
    clearChatMicPrewarmTimer();
    if (delayMs <= 0) {
      void tryPrewarmChatMic(reason);
      return;
    }
    chatMicPrewarmTimer = setTimeout(() => {
      chatMicPrewarmTimer = null;
      void tryPrewarmChatMic(reason);
    }, delayMs);
  }

  function handleWindowFocusForMicPrewarm() {
    scheduleChatMicPrewarm("focus", CHAT_WINDOW_MIC_PREWARM_DEBOUNCE_MS);
  }

  function handleVisibilityForMicPrewarm() {
    if (document.visibilityState !== "visible") return;
    scheduleChatMicPrewarm("visibility_visible", CHAT_WINDOW_MIC_PREWARM_DEBOUNCE_MS);
  }

  function cleanupChatForegroundRecording() {
    clearRecordHotkeyProbeState();
  }

  return {
    recordHotkey,
    foregroundRecordingActive,
    clearChatMicPrewarmTimer,
    isChatWindowActiveNow,
    startRecording,
    stopRecording,
    cancelForegroundRecordingOnBackground,
    clearRecordHotkeyProbeState,
    scheduleChatMicPrewarm,
    cleanupChatForegroundRecording,
    handleWindowFocusForMicPrewarm,
    handleVisibilityForMicPrewarm,
  };
}
