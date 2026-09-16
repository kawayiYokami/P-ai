import { computed, ref } from "vue";
import { appendTransportProbeLog } from "../../../services/tauri-api";

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
  getMinRecordSeconds: () => number;
  getMaxRecordSeconds: () => number;
  shouldUseRemoteStt: () => boolean;
  prepareRemoteAudio?: (blob: Blob) => Promise<{ mime: string; bytesBase64: string }>;
  transcribeRemoteStt: (audio: { mime: string; bytesBase64: string }) => Promise<string>;
  appendRecognizedText: (text: string) => void;
  onTranscribed?: (payload: { text: string; source: "local" | "remote" }) => void | Promise<void>;
  setStatus: (text: string) => void;
};

const REMOTE_STT_MIN_RMS_DBFS = -52;

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
  let prewarmInFlight: Promise<boolean> | null = null;
  let lastPrewarmAt = 0;
  // 启动尚未落地（getUserMedia 未返回、录音器还没 start）时收到的停止请求，
  // 若直接返回就会永久丢失，录音会一直录到上限；因此先挂起，等真正开始的那一刻立刻执行。
  let startInFlight = false;
  let pendingStop: { discard: boolean } | null = null;

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

  function pcm16ToWavBytes(audioBuffer: AudioBuffer): Uint8Array {
    const channels = Math.max(1, audioBuffer.numberOfChannels);
    const sampleRate = audioBuffer.sampleRate;
    const frames = audioBuffer.length;
    const bitsPerSample = 16;
    const blockAlign = channels * (bitsPerSample / 8);
    const byteRate = sampleRate * blockAlign;
    const dataSize = frames * blockAlign;
    const totalSize = 44 + dataSize;
    const bytes = new Uint8Array(totalSize);
    const view = new DataView(bytes.buffer);

    const writeAscii = (offset: number, text: string) => {
      for (let i = 0; i < text.length; i += 1) {
        view.setUint8(offset + i, text.charCodeAt(i));
      }
    };

    writeAscii(0, "RIFF");
    view.setUint32(4, 36 + dataSize, true);
    writeAscii(8, "WAVE");
    writeAscii(12, "fmt ");
    view.setUint32(16, 16, true);
    view.setUint16(20, 1, true);
    view.setUint16(22, channels, true);
    view.setUint32(24, sampleRate, true);
    view.setUint32(28, byteRate, true);
    view.setUint16(32, blockAlign, true);
    view.setUint16(34, bitsPerSample, true);
    writeAscii(36, "data");
    view.setUint32(40, dataSize, true);

    let offset = 44;
    for (let i = 0; i < frames; i += 1) {
      for (let c = 0; c < channels; c += 1) {
        const sample = audioBuffer.getChannelData(c)[i] ?? 0;
        const clamped = Math.max(-1, Math.min(1, sample));
        const int16 = clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff;
        view.setInt16(offset, int16, true);
        offset += 2;
      }
    }
    return bytes;
  }

  function bytesToBase64(bytes: Uint8Array): string {
    let binary = "";
    const chunkSize = 0x8000;
    for (let i = 0; i < bytes.length; i += chunkSize) {
      const chunk = bytes.subarray(i, i + chunkSize);
      binary += String.fromCharCode(...chunk);
    }
    return btoa(binary);
  }

  async function blobToWavBase64(blob: Blob): Promise<{ mime: string; bytesBase64: string }> {
    const AudioCtx = (window.AudioContext || (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext);
    if (!AudioCtx) {
      throw new Error("AudioContext is not supported.");
    }
    let ctx: AudioContext | null = null;
    try {
      ctx = new AudioCtx();
      const arrayBuffer = await blob.arrayBuffer();
      const audioBuffer = await ctx.decodeAudioData(arrayBuffer.slice(0));
      const wavBytes = pcm16ToWavBytes(audioBuffer);
      return {
        mime: "audio/wav",
        bytesBase64: bytesToBase64(wavBytes),
      };
    } finally {
      if (ctx) {
        try {
          await ctx.close();
        } catch {
          // ignore close error
        }
      }
    }
  }

  async function computeAudioRmsDbfs(blob: Blob): Promise<number | null> {
    const AudioCtx = (window.AudioContext || (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext);
    if (!AudioCtx) return null;
    let ctx: AudioContext | null = null;
    try {
      ctx = new AudioCtx();
      const arrayBuffer = await blob.arrayBuffer();
      const audioBuffer = await ctx.decodeAudioData(arrayBuffer.slice(0));
      const channels = audioBuffer.numberOfChannels;
      const frames = audioBuffer.length;
      if (channels <= 0 || frames <= 0) return null;
      let sumSquares = 0;
      let count = 0;
      for (let c = 0; c < channels; c += 1) {
        const data = audioBuffer.getChannelData(c);
        for (let i = 0; i < data.length; i += 1) {
          const s = data[i];
          sumSquares += s * s;
        }
        count += data.length;
      }
      if (count <= 0) return null;
      const rms = Math.sqrt(sumSquares / count);
      if (!Number.isFinite(rms) || rms <= 0) return -120;
      return 20 * Math.log10(rms);
    } catch {
      return null;
    } finally {
      if (ctx) {
        try {
          await ctx.close();
        } catch {
          // ignore close error
        }
      }
    }
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
      const finishEarly = (statusText?: string) => {
        if (statusText) {
          options.setStatus(statusText);
        }
        remoteRecorder = null;
        remoteChunks = [];
      };
      recording.value = false;
      clearTimers();
      stopRemoteStream();
      const elapsedMs = Math.max(0, Date.now() - startedAt);
      if (discardCurrent) {
        finishEarly();
        return;
      }
      const minMs = Math.max(1, options.getMinRecordSeconds()) * 1000;
      if (elapsedMs < minMs) {
        finishEarly(options.t("status.noSpeechText"));
        return;
      }
      if (remoteChunks.length === 0) {
        finishEarly(options.t("status.noSpeechText"));
        return;
      }
      const blob = new Blob(remoteChunks, { type: remoteRecorder?.mimeType || "audio/webm" });
      remoteChunks = [];
      const rmsDbfs = await computeAudioRmsDbfs(blob);
      if (rmsDbfs !== null && rmsDbfs < REMOTE_STT_MIN_RMS_DBFS) {
        finishEarly(options.t("status.noSpeechText"));
        return;
      }
      try {
        transcribing.value = true;
        options.setStatus("正在转写语音...");
        const preparedAudio = options.prepareRemoteAudio
          ? await options.prepareRemoteAudio(blob)
          : await (async () => {
            const dataUrl = await readBlobAsDataUrl(blob);
            return {
              mime: blob.type || "audio/webm",
              bytesBase64: dataUrl.includes(",") ? dataUrl.split(",")[1] : "",
            };
          })();
        if (!preparedAudio.bytesBase64) {
          options.setStatus(options.t("status.noSpeechText"));
          return;
        }
        const text = (await options.transcribeRemoteStt({
          mime: preparedAudio.mime,
          bytesBase64: preparedAudio.bytesBase64,
        }))
          .trim();
        if (text) {
          options.appendRecognizedText(text);
          void options.onTranscribed?.({ text, source: "remote" });
          options.setStatus("");
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

  async function prewarmMicrophone(): Promise<boolean> {
    // Only prewarm when remote STT path uses MediaRecorder/getUserMedia permission.
    if (!options.shouldUseRemoteStt()) return false;
    if (!navigator.mediaDevices?.getUserMedia || typeof MediaRecorder === "undefined") return false;
    if (recording.value || transcribing.value || startInFlight) return false;
    const now = Date.now();
    if (now - lastPrewarmAt < 15_000) return false;
    if (prewarmInFlight) return prewarmInFlight;
    prewarmInFlight = (async () => {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        for (const track of stream.getTracks()) {
          track.stop();
        }
        lastPrewarmAt = Date.now();
        return true;
      } catch {
        // Keep this silent: prewarm is best-effort and should not pollute UX.
        return false;
      } finally {
        prewarmInFlight = null;
      }
    })();
    return prewarmInFlight;
  }

  async function startRecording() {
    if (recording.value) return;
    if (!options.canStart()) return;
    // 启动到「真正开始录音」之间有异步空隙（远程 STT 要等 getUserMedia 返回），
    // 期间到达的停止请求必须先挂起，否则会被 stopRecording 的早退丢掉，录音只能等上限自动停。
    startInFlight = true;
    pendingStop = null;
    try {
      await startRecordingInner();
    } finally {
      startInFlight = false;
      flushPendingStop();
    }
  }

  function flushPendingStop() {
    const pending = pendingStop;
    pendingStop = null;
    if (!pending) return;
    appendTransportProbeLog("[录音]", "启动结束，补执行挂起的停止请求", { discard: pending.discard });
    void stopRecording(pending.discard);
  }

  async function startRecordingInner() {
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
      let recognitionFailed = false;
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
        recognitionFailed = true;
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
          options.setStatus("");
        } else if (!recognitionFailed) {
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
    if (!recording.value) {
      // 启动尚未落地：记下意图，交给 startRecording 的 finally 结算，避免这次停止被丢掉。
      if (startInFlight && !pendingStop) {
        pendingStop = { discard };
        appendTransportProbeLog("[录音]", "启动未落地，挂起停止请求", { discard });
      }
      return;
    }
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
    startInFlight = false;
    pendingStop = null;
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
    prewarmMicrophone,
    cleanup,
    blobToWavBase64,
  };
}
