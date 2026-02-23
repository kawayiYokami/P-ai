import { computed, ref } from "vue";

type SpeechRecognitionResultLike = { isFinal: boolean; 0: { transcript: string } };
type SpeechRecognitionEventLike = { resultIndex: number; results: ArrayLike<SpeechRecognitionResultLike> };
type SpeechRecognitionLike = {
  lang: string;
  interimResults: boolean;
  continuous: boolean;
  onresult: ((event: SpeechRecognitionEventLike) => void) | null;
  onerror: ((event: { error?: string }) => void) | null;
  onend: (() => void) | null;
  start: () => void;
  stop: () => void;
};

type TranslateFn = (key: string, params?: Record<string, unknown>) => string;

type UseSpeechRecordingOptions = {
  t: TranslateFn;
  canStart: () => boolean;
  getLanguage: () => string;
  getMaxRecordSeconds: () => number;
  shouldUseRemoteStt: () => boolean;
  transcribeRemoteStt: (audio: { mime: string; bytesBase64: string }) => Promise<string>;
  appendRecognizedText: (text: string) => void;
  onTranscribed?: (payload: { text: string; source: "local" | "remote" }) => void | Promise<void>;
  setStatus: (text: string) => void;
};

function getSpeechRecognitionCtor():
  | (new () => SpeechRecognitionLike)
  | undefined {
  const w = window as typeof window & {
    SpeechRecognition?: new () => SpeechRecognitionLike;
    webkitSpeechRecognition?: new () => SpeechRecognitionLike;
  };
  return w.SpeechRecognition || w.webkitSpeechRecognition;
}

export function useSpeechRecording(options: UseSpeechRecordingOptions) {
  const recording = ref(false);
  const recordingMs = ref(0);
  const transcribing = ref(false);
  const supported = computed(() => {
    if (options.shouldUseRemoteStt()) {
      return !!navigator.mediaDevices?.getUserMedia && typeof MediaRecorder !== "undefined";
    }
    return !!getSpeechRecognitionCtor();
  });

  let recognizer: SpeechRecognitionLike | null = null;
  let remoteRecorder: MediaRecorder | null = null;
  let remoteStream: MediaStream | null = null;
  let remoteChunks: BlobPart[] = [];
  let recognizedText = "";
  let discardCurrent = false;
  let startedAt = 0;
  let tickTimer: ReturnType<typeof setInterval> | null = null;
  let maxTimer: ReturnType<typeof setTimeout> | null = null;

  function clearTimers() {
    if (tickTimer) {
      clearInterval(tickTimer);
      tickTimer = null;
    }
    if (maxTimer) {
      clearTimeout(maxTimer);
      maxTimer = null;
    }
  }

  function stopRemoteStream() {
    if (!remoteStream) return;
    for (const track of remoteStream.getTracks()) {
      track.stop();
    }
    remoteStream = null;
  }

  async function readBlobAsDataUrl(blob: Blob): Promise<string> {
    return await new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result || ""));
      reader.onerror = () => reject(reader.error);
      reader.readAsDataURL(blob);
    });
  }

  async function startRemoteRecording() {
    if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") {
      options.setStatus("当前环境不支持录音。");
      return;
    }
    remoteStream = await navigator.mediaDevices.getUserMedia({ audio: true });
    remoteRecorder = new MediaRecorder(remoteStream);
    remoteChunks = [];
    remoteRecorder.ondataavailable = (event: BlobEvent) => {
      if (event.data && event.data.size > 0) {
        remoteChunks.push(event.data);
      }
    };
    remoteRecorder.onerror = () => {
      options.setStatus("录音失败，请重试。");
    };
    remoteRecorder.onstop = async () => {
      recording.value = false;
      clearTimers();
      stopRemoteStream();
      if (discardCurrent) {
        remoteRecorder = null;
        remoteChunks = [];
        return;
      }
      if (remoteChunks.length === 0) {
        options.setStatus(options.t("status.noSpeechText"));
        remoteRecorder = null;
        return;
      }
      const blob = new Blob(remoteChunks, { type: remoteRecorder?.mimeType || "audio/webm" });
      remoteChunks = [];
      try {
        transcribing.value = true;
        options.setStatus("正在转写语音...");
        const dataUrl = await readBlobAsDataUrl(blob);
        const bytesBase64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : "";
        if (!bytesBase64) {
          options.setStatus(options.t("status.noSpeechText"));
          return;
        }
        const text = (await options.transcribeRemoteStt({
          mime: blob.type || "audio/webm",
          bytesBase64,
        }))
          .trim();
        if (text) {
          options.appendRecognizedText(text);
          void options.onTranscribed?.({ text, source: "remote" });
          options.setStatus(options.t("status.recordTranscribed"));
        } else {
          options.setStatus(options.t("status.noSpeechText"));
        }
      } catch (err) {
        options.setStatus(options.t("status.speechFailed", { err: String(err) }));
      } finally {
        transcribing.value = false;
        remoteRecorder = null;
      }
    };
    remoteRecorder.start();
    startedAt = Date.now();
    recording.value = true;
    recordingMs.value = 0;
    tickTimer = setInterval(() => {
      recordingMs.value = Math.max(0, Date.now() - startedAt);
    }, 100);
    maxTimer = setTimeout(() => {
      void stopRecording(false);
      options.setStatus(options.t("status.recordAutoStopped", { seconds: options.getMaxRecordSeconds() }));
    }, options.getMaxRecordSeconds() * 1000);
  }

  async function startRecording() {
    if (recording.value) return;
    if (!options.canStart()) return;
    if (options.shouldUseRemoteStt()) {
      try {
        discardCurrent = false;
        await startRemoteRecording();
      } catch (err) {
        recording.value = false;
        clearTimers();
        stopRemoteStream();
        options.setStatus(options.t("status.recordStartFailed", { err: String(err) }));
      }
      return;
    }
    const SR = getSpeechRecognitionCtor();
    if (!SR) {
      options.setStatus(options.t("status.speechUnsupported"));
      return;
    }
    try {
      discardCurrent = false;
      recognizedText = "";
      recognizer = new SR();
      recognizer.lang = options.getLanguage();
      recognizer.interimResults = true;
      recognizer.continuous = true;
      recognizer.onresult = (event) => {
        for (let i = event.resultIndex; i < event.results.length; i += 1) {
          const item = event.results[i];
          const transcript = (item?.[0]?.transcript || "").trim();
          if (item?.isFinal && transcript) {
            recognizedText += `${transcript}\n`;
          }
        }
      };
      recognizer.onerror = (event) => {
        options.setStatus(options.t("status.speechFailed", { err: event?.error || "unknown" }));
      };
      recognizer.onend = () => {
        recording.value = false;
        clearTimers();
        if (discardCurrent) {
          recognizer = null;
          return;
        }
        transcribing.value = true;
        const text = recognizedText.trim();
        if (text) {
          options.appendRecognizedText(text);
          void options.onTranscribed?.({ text, source: "local" });
          options.setStatus(options.t("status.recordTranscribed"));
        } else {
          options.setStatus(options.t("status.noSpeechText"));
        }
        recognizer = null;
        recognizedText = "";
        transcribing.value = false;
      };
      recognizer.start();
      startedAt = Date.now();
      recording.value = true;
      recordingMs.value = 0;
      tickTimer = setInterval(() => {
        recordingMs.value = Math.max(0, Date.now() - startedAt);
      }, 100);
      maxTimer = setTimeout(() => {
        void stopRecording(false);
        options.setStatus(options.t("status.recordAutoStopped", { seconds: options.getMaxRecordSeconds() }));
      }, options.getMaxRecordSeconds() * 1000);
    } catch (err) {
      recording.value = false;
      clearTimers();
      options.setStatus(options.t("status.recordStartFailed", { err: String(err) }));
    }
  }

  async function stopRecording(discard: boolean) {
    if (!recording.value) return;
    discardCurrent = discard;
    if (options.shouldUseRemoteStt()) {
      if (remoteRecorder && remoteRecorder.state !== "inactive") {
        remoteRecorder.stop();
      } else {
        recording.value = false;
        clearTimers();
        stopRemoteStream();
      }
      return;
    }
    recognizer?.stop();
  }

  function cleanup() {
    clearTimers();
    discardCurrent = true;
    recognizer?.stop();
    recognizer = null;
    if (remoteRecorder && remoteRecorder.state !== "inactive") {
      remoteRecorder.stop();
    }
    remoteRecorder = null;
    remoteChunks = [];
    stopRemoteStream();
    recording.value = false;
    transcribing.value = false;
  }

  return {
    supported,
    recording,
    recordingMs,
    transcribing,
    startRecording,
    stopRecording,
    cleanup,
  };
}
