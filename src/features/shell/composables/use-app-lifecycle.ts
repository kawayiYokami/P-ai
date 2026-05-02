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
  beforeRefreshData?: () => Promise<void> | void;
  afterSafetyGateReady?: () => Promise<void> | void;
  refreshAllViewData: () => Promise<void>;
  afterRefreshData?: () => Promise<void> | void;
  viewMode: Ref<"chat" | "archives" | "config">;
  syncWindowControlsState: () => Promise<void>;
  stopRecording: (discard: boolean) => Promise<void>;
  cleanupSpeechRecording: () => void;
  cleanupChatMedia: () => Promise<void>;
  afterMountedReady?: () => Promise<void> | void;
  onStartupStepFailed?: (label: string, error: unknown) => void;
};

const STARTUP_STEP_TIMEOUT_MS = 10_000;

function startupTimeoutError(label: string): Error {
  return new Error(`启动步骤超时：${label} 超过 ${STARTUP_STEP_TIMEOUT_MS / 1000} 秒未完成，已跳过。`);
}

async function runStartupStep(
  label: string,
  task: () => Promise<void> | void,
  onFailed?: (label: string, error: unknown) => void,
): Promise<boolean> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  try {
    await Promise.race([
      Promise.resolve().then(task),
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(startupTimeoutError(label)), STARTUP_STEP_TIMEOUT_MS);
      }),
    ]);
    return true;
  } catch (error) {
    console.error(`[LIFECYCLE] startup step failed: ${label}`, error);
    onFailed?.(label, error);
    return false;
  } finally {
    if (timer) clearTimeout(timer);
  }
}

export function useAppLifecycle(options: UseAppLifecycleOptions) {
  onMounted(async () => {
    const bootstrapMounted = await runStartupStep(
      "appBootstrapMount",
      () => options.appBootstrapMount(),
      options.onStartupStepFailed,
    );
    if (!bootstrapMounted) return;
    options.restoreThemeFromStorage();
    window.addEventListener("paste", options.onPaste);
    window.addEventListener("dragover", options.onDragOver);
    window.addEventListener("drop", options.onDrop);
    options.recordHotkeyMount();
    try {
      await options.beforeRefreshData?.();
    } catch (error) {
      console.error("[LIFECYCLE] startup safety gate failed: beforeRefreshData", error);
      options.onStartupStepFailed?.("beforeRefreshData", error);
      return;
    }
    const backendReadyNotified = await runStartupStep(
      "afterSafetyGateReady",
      () => options.afterSafetyGateReady?.(),
      options.onStartupStepFailed,
    );
    if (!backendReadyNotified) return;
    try {
      await options.refreshAllViewData();
    } catch (error) {
      console.error("[LIFECYCLE] startup refresh failed: refreshAllViewData", error);
      options.onStartupStepFailed?.("refreshAllViewData", error);
      return;
    }
    const afterRefreshCompleted = await runStartupStep(
      "afterRefreshData",
      () => options.afterRefreshData?.(),
      options.onStartupStepFailed,
    );
    if (!afterRefreshCompleted) return;
    if (options.viewMode.value === "chat") {
      const windowControlsSynced = await runStartupStep(
        "syncWindowControlsState",
        () => options.syncWindowControlsState(),
        options.onStartupStepFailed,
      );
      if (!windowControlsSynced) return;
    }
    await runStartupStep(
      "afterMountedReady",
      () => options.afterMountedReady?.(),
      options.onStartupStepFailed,
    );
  });

  onBeforeUnmount(() => {
    options.appBootstrapUnmount();
    void options.stopRecording(true);
    options.cleanupSpeechRecording();
    options.recordHotkeyUnmount();
    void options.cleanupChatMedia();
    window.removeEventListener("paste", options.onPaste);
    window.removeEventListener("dragover", options.onDragOver);
    window.removeEventListener("drop", options.onDrop);
  });
}
