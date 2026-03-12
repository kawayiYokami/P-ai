import { onBeforeUnmount, onMounted, type Ref } from "vue";

type UseAppLifecycleOptions = {
  appBootstrapMount: () => Promise<void>;
  appBootstrapUnmount: () => void;
  restoreThemeFromStorage: () => void;
  onPaste: (event: ClipboardEvent) => void;
  onDragOver: (event: DragEvent) => void;
  onDrop: (event: DragEvent) => void;
  onNativeFileDrop?: (paths: string[]) => Promise<void> | void;
  onNativeDragState?: (active: boolean) => void;
  recordHotkeyMount: () => void;
  recordHotkeyUnmount: () => void;
  refreshAllViewData: () => Promise<void>;
  viewMode: Ref<"chat" | "archives" | "config">;
  syncAlwaysOnTop: () => Promise<void>;
  clearStreamBuffer: () => void;
  stopRecording: (discard: boolean) => Promise<void>;
  cleanupSpeechRecording: () => void;
  cleanupChatMedia: () => Promise<void>;
  afterMountedReady?: () => Promise<void> | void;
};

export function useAppLifecycle(options: UseAppLifecycleOptions) {
  onMounted(async () => {
    try {
      await options.appBootstrapMount();
    } catch (error) {
      console.error("[LIFECYCLE] app bootstrap mount failed:", error);
    }
    options.restoreThemeFromStorage();
    window.addEventListener("paste", options.onPaste);
    window.addEventListener("dragover", options.onDragOver);
    window.addEventListener("drop", options.onDrop);
    options.recordHotkeyMount();
    try {
      await options.refreshAllViewData();
      if (options.viewMode.value === "chat") {
        await options.syncAlwaysOnTop();
      }
      await options.afterMountedReady?.();
    } catch (error) {
      console.error("[LIFECYCLE] mounted async flow failed:", error);
    }
  });

  onBeforeUnmount(() => {
    options.appBootstrapUnmount();
    options.clearStreamBuffer();
    void options.stopRecording(true);
    options.cleanupSpeechRecording();
    options.recordHotkeyUnmount();
    void options.cleanupChatMedia();
    window.removeEventListener("paste", options.onPaste);
    window.removeEventListener("dragover", options.onDragOver);
    window.removeEventListener("drop", options.onDrop);
  });
}
