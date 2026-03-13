import { ref, type ComputedRef, type Ref } from "vue";
import type { ApiConfigItem } from "../../../types/app";
import { invokeTauri } from "../../../services/tauri-api";

type TrFn = (key: string, params?: Record<string, unknown>) => string;

type UseChatMediaOptions = {
  t: TrFn;
  setStatus: (text: string) => void;
  setChatError: (text: string) => void;
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

type RejectionReason = "imageUnsupported" | "pdfNeedsImage" | "pdfNeedsGemini";
type QueuedLocalFileResult = {
  mime: string;
  fileName: string;
  savedPath: string;
  attachAsMedia: boolean;
  bytesBase64?: string | null;
  textNotice?: string;
};

export function useChatMedia(options: UseChatMediaOptions) {
  const hotkeyTestRecording = ref(false);
  const hotkeyTestRecordingMs = ref(0);
  const hotkeyTestAudio = ref<{ mime: string; bytesBase64: string; durationMs: number } | null>(null);
  const mediaDragActive = ref(false);

  let hotkeyTestRecorder: MediaRecorder | null = null;
  let hotkeyTestStream: MediaStream | null = null;
  let hotkeyTestStartedAt = 0;
  let hotkeyTestTickTimer: ReturnType<typeof setInterval> | null = null;
  let hotkeyTestPlayer: HTMLAudioElement | null = null;
  let dragOverlayHideTimer: ReturnType<typeof setTimeout> | null = null;

  function canAcceptImage(apiConfig: ApiConfigItem): boolean {
    return !!apiConfig.enableImage || options.hasVisionFallback.value;
  }

  function canAcceptPdf(apiConfig: ApiConfigItem): boolean {
    return !!apiConfig.enableImage && apiConfig.requestFormat === "gemini";
  }

  function classifyFileMime(
    mime: string,
    apiConfig: ApiConfigItem,
  ): { kind: "image" | "pdf" | null; reason: RejectionReason | null } {
    const normalized = (mime || "").trim().toLowerCase();
    if (normalized.startsWith("image/")) {
      return canAcceptImage(apiConfig)
        ? { kind: "image", reason: null }
        : { kind: null, reason: "imageUnsupported" };
    }
    if (normalized === "application/pdf") {
      if (!apiConfig.enableImage) return { kind: null, reason: "pdfNeedsImage" };
      if (canAcceptPdf(apiConfig)) return { kind: "pdf", reason: null };
      return { kind: null, reason: "pdfNeedsGemini" };
    }
    return { kind: null, reason: null };
  }

  function inferMimeFromFileName(name: string): string {
    const lower = (name || "").trim().toLowerCase();
    if (lower.endsWith(".pdf")) return "application/pdf";
    if (lower.endsWith(".png")) return "image/png";
    if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg";
    if (lower.endsWith(".gif")) return "image/gif";
    if (lower.endsWith(".webp")) return "image/webp";
    if (lower.endsWith(".heic")) return "image/heic";
    if (lower.endsWith(".heif")) return "image/heif";
    if (lower.endsWith(".svg")) return "image/svg+xml";
    return "";
  }

  function normalizeFileMime(file: File): string {
    const raw = (file.type || "").trim().toLowerCase();
    if (raw) return raw;
    return inferMimeFromFileName(file.name);
  }

  function rejectionMessage(reasons: RejectionReason[], apiConfig: ApiConfigItem | null): string {
    if (reasons.includes("pdfNeedsGemini")) {
      const currentFormat = apiConfig?.requestFormat || "unknown";
      return `当前接口协议为 ${currentFormat}，不是 gemini，PDF 附件暂不支持。`;
    }
    if (reasons.includes("pdfNeedsImage")) {
      return "当前接口未启用图片能力，暂不支持 PDF 附件。";
    }
    return "当前接口不支持图片附件。";
  }

  function notifyRejected(reasons: RejectionReason[]) {
    if (reasons.length === 0) return;
    const text = rejectionMessage(reasons, options.activeChatApiConfig.value);
    options.setStatus(text);
    options.setChatError(text);
  }

  function appendClipboardFile(file: File, mime: string) {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result || "");
      const base64 = result.includes(",") ? result.split(",")[1] : "";
      if (base64) options.clipboardImages.value.push({ mime, bytesBase64: base64 });
    };
    reader.onerror = () => {
      options.setStatusError("status.pasteImageReadFailed", reader.error || "unknown");
    };
    reader.readAsDataURL(file);
  }

  function collectPastedFiles(
    event: ClipboardEvent,
    apiConfig: ApiConfigItem,
  ): { files: Array<{ file: File; mime: string }>; rejected: RejectionReason[] } {
    const data = event.clipboardData;
    if (!data) return { files: [], rejected: [] };
    const items = data.items;
    const filesFromItems =
      items && items.length > 0
        ? Array.from(items)
            .filter((item) => item.kind === "file")
            .map((item) => item.getAsFile())
            .filter((file): file is File => !!file)
        : [];
    const filesFromList = data.files ? Array.from(data.files) : [];
    const sourceFiles = filesFromItems.length > 0 ? filesFromItems : filesFromList;
    if (sourceFiles.length === 0) return { files: [], rejected: [] };
    const files: Array<{ file: File; mime: string }> = [];
    const rejected: RejectionReason[] = [];
    for (const file of sourceFiles) {
      const mime = normalizeFileMime(file);
      if (!mime) continue;
      const classified = classifyFileMime(mime, apiConfig);
      if (!classified.kind) {
        if (classified.reason) rejected.push(classified.reason);
        continue;
      }
      files.push({ file, mime });
    }
    return { files, rejected };
  }

  function parseClipboardFilePaths(event: ClipboardEvent): string[] {
    const data = event.clipboardData;
    if (!data) return [];
    const out: string[] = [];

    const uriList = String(data.getData("text/uri-list") || "").trim();
    if (uriList) {
      for (const raw of uriList.split(/\r?\n/).map((v) => v.trim()).filter(Boolean)) {
        if (raw.startsWith("#")) continue;
        if (raw.startsWith("file://")) {
          try {
            const url = new URL(raw);
            if (url.protocol === "file:") {
              let decoded = decodeURIComponent(url.pathname || "");
              if (!decoded) continue;
              if (url.host) {
                const uncPath = `\\\\${url.host}${decoded.replace(/\//g, "\\")}`;
                out.push(uncPath);
                continue;
              }
              const isWindowsDrivePath = /^\/[a-zA-Z]:\//.test(decoded) || /^[a-zA-Z]:\//.test(decoded);
              if (isWindowsDrivePath) {
                if (/^\/[a-zA-Z]:\//.test(decoded)) decoded = decoded.slice(1);
                out.push(decoded.replace(/\//g, "\\"));
              } else {
                out.push(decoded);
              }
            }
          } catch {
            // ignore invalid uri
          }
        }
      }
    }

    const plain = String(data.getData("text/plain") || "").trim();
    if (plain) {
      for (const raw of plain.split(/\r?\n/).map((v) => v.trim()).filter(Boolean)) {
        const candidate = raw.replace(/^"(.*)"$/, "$1");
        if (/^[a-zA-Z]:\\/.test(candidate) || /^\\\\/.test(candidate)) {
          out.push(candidate);
        }
      }
    }

    const deduped = Array.from(new Set(out));
    return deduped;
  }

  function collectDroppedFiles(
    event: DragEvent,
    apiConfig: ApiConfigItem,
  ): { files: Array<{ file: File; mime: string }>; rejected: RejectionReason[] } {
    const transfer = event.dataTransfer;
    if (!transfer) return { files: [], rejected: [] };
    const fromFiles = transfer.files ? Array.from(transfer.files) : [];
    const fromItems =
      transfer.items && transfer.items.length > 0
        ? Array.from(transfer.items)
            .filter((item) => item.kind === "file")
            .map((item) => item.getAsFile())
            .filter((file): file is File => !!file)
        : [];
    const files = fromFiles.length > 0 ? fromFiles : fromItems;
    if (files.length === 0) return { files: [], rejected: [] };
    const out: Array<{ file: File; mime: string }> = [];
    const rejected: RejectionReason[] = [];
    for (const file of files) {
      const mime = normalizeFileMime(file);
      if (!mime) continue;
      const classified = classifyFileMime(mime, apiConfig);
      if (!classified.kind) {
        if (classified.reason) rejected.push(classified.reason);
        continue;
      }
      out.push({ file, mime });
    }
    return { files: out, rejected };
  }

  function onPaste(event: ClipboardEvent) {
    if (options.viewMode.value !== "chat") return;
    if (options.chatting.value || options.forcingArchive.value) return;
    const apiConfig = options.activeChatApiConfig.value;
    if (!apiConfig) return;
    const collected = collectPastedFiles(event, apiConfig);
    if (collected.files.length > 0) {
      event.preventDefault();
      options.setChatError("");
      for (const item of collected.files) {
        appendClipboardFile(item.file, item.mime);
      }
      return;
    }

    const pastedPaths = parseClipboardFilePaths(event);
    if (pastedPaths.length > 0) {
      event.preventDefault();
      options.setChatError("");
      void onNativeFileDrop(pastedPaths);
      return;
    }

    const text = event.clipboardData?.getData("text/plain") || "";
    if (text && !options.chatInput.value.trim() && apiConfig.enableText) {
      event.preventDefault();
      options.chatInput.value = text;
      options.setChatError("");
      return;
    }

    notifyRejected(collected.rejected);
  }

  function onDragOver(event: DragEvent) {
    if (options.viewMode.value !== "chat") return;
    if (options.chatting.value || options.forcingArchive.value) return;
    const apiConfig = options.activeChatApiConfig.value;
    if (!apiConfig) return;
    if (!event.dataTransfer) return;
    const hasFilePayload = !!event.dataTransfer?.types?.includes("Files");
    if (!hasFilePayload) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
    const collected = collectDroppedFiles(event, apiConfig);
    if (collected.files.length === 0 && collected.rejected.length === 0) return;
    mediaDragActive.value = true;
    if (dragOverlayHideTimer) {
      clearTimeout(dragOverlayHideTimer);
      dragOverlayHideTimer = null;
    }
    dragOverlayHideTimer = setTimeout(() => {
      mediaDragActive.value = false;
      dragOverlayHideTimer = null;
    }, 140);
  }

  function onDrop(event: DragEvent) {
    if (options.viewMode.value !== "chat") return;
    if (options.chatting.value || options.forcingArchive.value) return;
    const apiConfig = options.activeChatApiConfig.value;
    if (!apiConfig) return;
    const collected = collectDroppedFiles(event, apiConfig);
    if (collected.files.length === 0) {
      event.preventDefault();
      notifyRejected(collected.rejected);
      return;
    }
    event.preventDefault();
    options.setChatError("");
    options.setStatus(`收到拖拽文件 ${collected.files.length} 个（DOM）。`);
    mediaDragActive.value = false;
    if (dragOverlayHideTimer) {
      clearTimeout(dragOverlayHideTimer);
      dragOverlayHideTimer = null;
    }
    for (const item of collected.files) {
      appendClipboardFile(item.file, item.mime);
    }
  }

  async function onNativeFileDrop(paths: string[]) {
    if (options.viewMode.value !== "chat") return;
    if (options.chatting.value || options.forcingArchive.value) return;
    const apiConfig = options.activeChatApiConfig.value;
    if (!apiConfig) return;
    if (!Array.isArray(paths) || paths.length === 0) return;
    options.setChatError("");
    options.setStatus(`收到拖拽文件 ${paths.length} 个（Tauri）。`);

    const rejected: RejectionReason[] = [];
    const textNotices: string[] = [];
    for (const path of paths) {
      try {
        const queued = await invokeTauri<QueuedLocalFileResult>("queue_local_file_attachment", {
          input: { path },
        });
        const mime = String(queued.mime || "").trim().toLowerCase();
        const classified = classifyFileMime(mime, apiConfig);
        const canAttachAsMedia =
          !!queued.attachAsMedia &&
          !!String(queued.bytesBase64 || "").trim() &&
          !!classified.kind;
        if (!canAttachAsMedia) {
          const notice = String(queued.textNotice || "").trim();
          if (notice) textNotices.push(notice);
          if (classified.reason) rejected.push(classified.reason);
          continue;
        }
        const base64 = String(queued.bytesBase64 || "").trim();
        if (!base64) continue;
        options.clipboardImages.value.push({ mime, bytesBase64: base64 });
      } catch (error) {
        options.setStatusError("status.pasteImageReadFailed", error);
      }
    }
    if (textNotices.length > 0) {
      const merged = textNotices.join("\n\n");
      options.chatInput.value = options.chatInput.value.trim()
        ? `${options.chatInput.value.trim()}\n\n${merged}`
        : merged;
    }

    notifyRejected(rejected);
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
        try {
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
        } catch (e) {
          options.setStatusError("status.recordTestFailed", e);
        } finally {
          hotkeyTestRecording.value = false;
          clearHotkeyTestTimers();
          stopHotkeyTestStream();
        }
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
    mediaDragActive.value = false;
    if (dragOverlayHideTimer) {
      clearTimeout(dragOverlayHideTimer);
      dragOverlayHideTimer = null;
    }
  }

  return {
    mediaDragActive,
    hotkeyTestRecording,
    hotkeyTestRecordingMs,
    hotkeyTestAudio,
    onPaste,
    onDragOver,
    onDrop,
    onNativeFileDrop,
    removeClipboardImage,
    startHotkeyRecordTest,
    stopHotkeyRecordTest,
    playHotkeyRecordTest,
    cleanupChatMedia,
  };
}
