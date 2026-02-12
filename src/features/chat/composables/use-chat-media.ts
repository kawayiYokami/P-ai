import { ref, type ComputedRef, type Ref } from "vue";
import type { ApiConfigItem } from "../../../types/app";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseChatMediaOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setStatusError: (key: string, error: unknown) => void;
  viewMode: Ref<"chat" | "archives" | "config">;
  chatting: Ref<boolean>;
  forcingArchive: Ref<boolean>;
  isRecording: () => boolean;
  activeChatApiConfig: ComputedRef<ApiConfigItem | null>;
  hasVisionFallback: ComputedRef<boolean>;
  chatInput: Ref<string>;
  clipboardImages: Ref<Array<{ mime: string; bytesBase64: string }>>;
};

export function useChatMedia(options: UseChatMediaOptions) {
  const hotkeyTestRecording = ref(false);
  const hotkeyTestRecordingMs = ref(0);
  const hotkeyTestAudio = ref<{ mime: string; bytesBase64: string; durationMs: number } | null>(null);

  let hotkeyTestRecorder: MediaRecorder | null = null;
  let hotkeyTestStream: MediaStream | null = null;
  let hotkeyTestStartedAt = 0;
  let hotkeyTestTickTimer: ReturnType<typeof setInterval> | null = null;
  let hotkeyTestPlayer: HTMLAudioElement | null = null;

  function onPaste(event: ClipboardEvent) {
    if (options.viewMode.value !== "chat") return;
    if (options.chatting.value || options.forcingArchive.value) return;
    const items = event.clipboardData?.items;
    if (!items) return;
    const apiConfig = options.activeChatApiConfig.value;
    if (!apiConfig) return;

    const text = event.clipboardData?.getData("text/plain");
    if (text && !options.chatInput.value.trim() && apiConfig.enableText) {
      options.chatInput.value = text;
    }

    for (const item of Array.from(items)) {
      if (item.type.startsWith("image/")) {
        if (!apiConfig.enableImage && !options.hasVisionFallback.value) {
          event.preventDefault();
          return;
        }
        const file = item.getAsFile();
        if (!file) continue;
        const reader = new FileReader();
        reader.onload = () => {
          const result = String(reader.result || "");
          const base64 = result.includes(",") ? result.split(",")[1] : "";
          if (base64) options.clipboardImages.value.push({ mime: item.type, bytesBase64: base64 });
        };
        reader.onerror = () => {
          options.setStatusError("status.pasteImageReadFailed", reader.error || "unknown");
        };
        reader.readAsDataURL(file);
        event.preventDefault();
      }
    }
  }

  function removeClipboardImage(index: number) {
    if (index < 0 || index >= options.clipboardImages.value.length) return;
    options.clipboardImages.value.splice(index, 1);
  }

  async function readBlobAsDataUrl(blob: Blob): Promise<string> {
    return await new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result || ""));
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(blob);
    });
  }

  function clearHotkeyTestTimers() {
    if (hotkeyTestTickTimer) {
      clearInterval(hotkeyTestTickTimer);
      hotkeyTestTickTimer = null;
    }
  }

  function stopHotkeyTestStream() {
    if (hotkeyTestStream) {
      for (const track of hotkeyTestStream.getTracks()) track.stop();
      hotkeyTestStream = null;
    }
  }

  async function startHotkeyRecordTest() {
    if (hotkeyTestRecording.value) return;
    if (options.isRecording()) return;
    if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") {
      options.setStatus(options.t("status.recordUnsupported"));
      return;
    }
    try {
      hotkeyTestStream = await navigator.mediaDevices.getUserMedia({ audio: true });
      hotkeyTestRecorder = new MediaRecorder(hotkeyTestStream);
      const chunks: BlobPart[] = [];
      hotkeyTestRecorder.ondataavailable = (event: BlobEvent) => {
        if (event.data && event.data.size > 0) chunks.push(event.data);
      };
      hotkeyTestRecorder.onstop = async () => {
        const durationMs = Math.max(0, Date.now() - hotkeyTestStartedAt);
        hotkeyTestRecording.value = false;
        clearHotkeyTestTimers();
        stopHotkeyTestStream();
        if (chunks.length === 0) return;
        const blob = new Blob(chunks, { type: hotkeyTestRecorder?.mimeType || "audio/webm" });
        const dataUrl = await readBlobAsDataUrl(blob);
        const base64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : "";
        if (!base64) return;
        hotkeyTestAudio.value = {
          mime: blob.type || "audio/webm",
          bytesBase64: base64,
          durationMs,
        };
        options.setStatus(
          options.t("status.recordTestDone", { seconds: Math.max(1, Math.round(durationMs / 1000)) }),
        );
      };
      hotkeyTestRecorder.start();
      hotkeyTestStartedAt = Date.now();
      hotkeyTestRecording.value = true;
      hotkeyTestRecordingMs.value = 0;
      clearHotkeyTestTimers();
      hotkeyTestTickTimer = setInterval(() => {
        hotkeyTestRecordingMs.value = Math.max(0, Date.now() - hotkeyTestStartedAt);
      }, 100);
    } catch (e) {
      hotkeyTestRecording.value = false;
      clearHotkeyTestTimers();
      stopHotkeyTestStream();
      options.setStatusError("status.recordTestFailed", e);
    }
  }

  async function stopHotkeyRecordTest() {
    if (!hotkeyTestRecording.value) return;
    if (hotkeyTestRecorder && hotkeyTestRecorder.state !== "inactive") {
      hotkeyTestRecorder.stop();
    } else {
      hotkeyTestRecording.value = false;
      clearHotkeyTestTimers();
      stopHotkeyTestStream();
    }
  }

  function playHotkeyRecordTest() {
    if (!hotkeyTestAudio.value) return;
    if (hotkeyTestPlayer) {
      hotkeyTestPlayer.pause();
      hotkeyTestPlayer.currentTime = 0;
      hotkeyTestPlayer = null;
    }
    const src = `data:${hotkeyTestAudio.value.mime};base64,${hotkeyTestAudio.value.bytesBase64}`;
    hotkeyTestPlayer = new Audio(src);
    void hotkeyTestPlayer.play().catch(() => {
      hotkeyTestPlayer = null;
    });
  }

  async function cleanupChatMedia() {
    await stopHotkeyRecordTest();
    if (hotkeyTestPlayer) {
      hotkeyTestPlayer.pause();
      hotkeyTestPlayer.currentTime = 0;
      hotkeyTestPlayer = null;
    }
    clearHotkeyTestTimers();
    stopHotkeyTestStream();
  }

  return {
    hotkeyTestRecording,
    hotkeyTestRecordingMs,
    hotkeyTestAudio,
    onPaste,
    removeClipboardImage,
    startHotkeyRecordTest,
    stopHotkeyRecordTest,
    playHotkeyRecordTest,
    cleanupChatMedia,
  };
}

