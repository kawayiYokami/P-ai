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
  appendRecognizedText: (text: string) => void;
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
  const supported = computed(() => !!getSpeechRecognitionCtor());

  let recognizer: SpeechRecognitionLike | null = null;
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

  async function startRecording() {
    if (recording.value) return;
    if (!options.canStart()) return;
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
        const text = recognizedText.trim();
        if (text) {
          options.appendRecognizedText(text);
          options.setStatus(options.t("status.recordTranscribed"));
        } else {
          options.setStatus(options.t("status.noSpeechText"));
        }
        recognizer = null;
        recognizedText = "";
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
    recognizer?.stop();
  }

  function cleanup() {
    clearTimers();
    discardCurrent = true;
    recognizer?.stop();
    recognizer = null;
    recording.value = false;
  }

  return {
    supported,
    recording,
    recordingMs,
    startRecording,
    stopRecording,
    cleanup,
  };
}

