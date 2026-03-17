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
  clipboardImages: Ref<Array<{ mime: string; bytesBase64: string; savedPath?: string }>>;
  queuedAttachmentNotices: Ref<Array<{ id: string; fileName: string; relativePath: string; mime: string }>>;
};

type QueuedLocalFileResult = {
  mime: string;
  fileName: string;
  savedPath: string;
  attachAsMedia: boolean;
  bytesBase64?: string | null;
  textNotice?: string;
};

type QueueInlineFileAttachmentInput = {
  fileName: string;
  mime?: string;
  bytesBase64: string;
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

  function classifyFileMime(
    mime: string,
    apiConfig: ApiConfigItem,
  ): { kind: "image" | "pdf" | null; reason: "imageUnsupported" | null } {
    const normalized = (mime || "").trim().toLowerCase();
    if (normalized.startsWith("image/")) {
      return canAcceptImage(apiConfig)
        ? { kind: "image", reason: null }
        : { kind: null, reason: "imageUnsupported" };
    }
    if (normalized === "application/pdf") {
      // PDF 不再走多模态直发，统一入队为普通附件，交由后端阅读链路处理。
      return { kind: null, reason: null };
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

  function collectPastedFiles(
    event: ClipboardEvent,
  ): Array<{ file: File; mime: string }> {
    const data = event.clipboardData;
    if (!data) return [];
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
    if (sourceFiles.length === 0) return [];
    const files: Array<{ file: File; mime: string }> = [];
    for (const file of sourceFiles) {
      const mime = normalizeFileMime(file);
      files.push({ file, mime });
    }
    return files;
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
  ): Array<{ file: File; mime: string }> {
    const transfer = event.dataTransfer;
    if (!transfer) return [];
    const fromFiles = transfer.files ? Array.from(transfer.files) : [];
    const fromItems =
      transfer.items && transfer.items.length > 0
        ? Array.from(transfer.items)
            .filter((item) => item.kind === "file")
            .map((item) => item.getAsFile())
            .filter((file): file is File => !!file)
        : [];
    const files = fromFiles.length > 0 ? fromFiles : fromItems;
    if (files.length === 0) return [];
    const out: Array<{ file: File; mime: string }> = [];
    for (const file of files) {
      const mime = normalizeFileMime(file);
      out.push({ file, mime });
    }
    return out;
  }

  function applyQueuedAttachmentResult(queued: QueuedLocalFileResult, apiConfig: ApiConfigItem) {
    const mime = String(queued.mime || "").trim().toLowerCase();
    const classified = classifyFileMime(mime, apiConfig);
    const canAttachAsMedia =
      !!queued.attachAsMedia &&
      !!String(queued.bytesBase64 || "").trim() &&
      !!classified.kind;

    if (!canAttachAsMedia) {
      const savedPath = String(queued.savedPath || "").trim();
      const relativePath = savedPath.replace(/\\/g, "/").replace(/^.*\/downloads\//, "downloads/");
      const fileName = String(queued.fileName || "").trim() || relativePath.split("/").pop() || "attachment";
      const id = `${relativePath || fileName}::${mime}`;
      if (!options.queuedAttachmentNotices.value.some((item) => item.id === id)) {
        options.queuedAttachmentNotices.value.push({
          id,
          fileName,
          relativePath: relativePath || savedPath || fileName,
          mime,
        });
      }
      return;
    }

    options.clipboardImages.value.push({
      mime,
      bytesBase64: String(queued.bytesBase64 || "").trim(),
    });
  }

  async function queueInlineBrowserFile(file: File, mime: string): Promise<QueuedLocalFileResult | null> {
    const dataUrl = await readBlobAsDataUrl(file);
    const bytesBase64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : "";
    if (!bytesBase64) return null;
    return await invokeTauri<QueuedLocalFileResult>("queue_inline_file_attachment", {
      input: {
        fileName: String(file.name || "").trim() || "attachment",
        mime,
        bytesBase64,
      } as QueueInlineFileAttachmentInput,
    });
  }

  function onPaste(event: ClipboardEvent) {
    if (options.viewMode.value !== "chat") return;
    if (options.chatting.value || options.forcingArchive.value) return;
    const apiConfig = options.activeChatApiConfig.value;
    if (!apiConfig) return;
    const collected = collectPastedFiles(event);
    if (collected.length > 0) {
      event.preventDefault();
      options.setChatError("");
      void (async () => {
        for (const item of collected) {
          try {
            const queued = await queueInlineBrowserFile(item.file, item.mime);
            if (!queued) continue;
            applyQueuedAttachmentResult(queued, apiConfig);
          } catch (error) {
            options.setStatusError("status.pasteImageReadFailed", error);
          }
        }
      })();
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
    const collected = collectDroppedFiles(event);
    if (collected.length === 0) return;
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
    const collected = collectDroppedFiles(event);
    if (collected.length === 0) return;
    event.preventDefault();
    options.setChatError("");
    options.setStatus(`收到拖拽文件 ${collected.length} 个（DOM）。`);
    mediaDragActive.value = false;
    if (dragOverlayHideTimer) {
      clearTimeout(dragOverlayHideTimer);
      dragOverlayHideTimer = null;
    }
    void (async () => {
      for (const item of collected) {
        try {
          const queued = await queueInlineBrowserFile(item.file, item.mime);
          if (!queued) continue;
          applyQueuedAttachmentResult(queued, apiConfig);
        } catch (error) {
          options.setStatusError("status.pasteImageReadFailed", error);
        }
      }
    })();
  }

  async function onNativeFileDrop(paths: string[]) {
    if (options.viewMode.value !== "chat") return;
    if (options.chatting.value || options.forcingArchive.value) return;
    const apiConfig = options.activeChatApiConfig.value;
    if (!apiConfig) return;
    if (!Array.isArray(paths) || paths.length === 0) return;
    options.setChatError("");
    options.setStatus(`收到拖拽文件 ${paths.length} 个（Tauri）。`);

    for (const path of paths) {
      try {
        const queued = await invokeTauri<QueuedLocalFileResult>("queue_local_file_attachment", {
          input: { path },
        });
        applyQueuedAttachmentResult(queued, apiConfig);
      } catch (error) {
        options.setStatusError("status.pasteImageReadFailed", error);
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
