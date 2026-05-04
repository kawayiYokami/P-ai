import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, onBeforeUnmount, ref, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { GithubUpdateInfo, UpdateProgressPayload } from "../types/update";
import type { GithubUpdateMethod } from "../../../types/app";

type ViewModeRef = Ref<"chat" | "archives" | "config">;

type UseGithubUpdateOptions = {
  viewMode: ViewModeRef;
  status: Ref<string>;
  updateMethod: Ref<GithubUpdateMethod | undefined>;
};

function formatBytes(value?: number) {
  if (!Number.isFinite(value) || !value || value <= 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let size = value;
  let idx = 0;
  while (size >= 1024 && idx < units.length - 1) {
    size /= 1024;
    idx += 1;
  }
  const digits = idx === 0 ? 0 : size >= 100 ? 0 : size >= 10 ? 1 : 2;
  return `${size.toFixed(digits)} ${units[idx]}`;
}

export function useGithubUpdate(options: UseGithubUpdateOptions) {
  const checkingUpdateRequest = ref(false);
  const updateInProgress = ref(false);
  const updateReadyToRestart = ref(false);
  const updateDialogOpen = ref(false);
  const updateDialogTitle = ref("检查更新");
  const updateDialogBody = ref("");
  const updateDialogKind = ref<"info" | "error">("info");
  const updateDialogReleaseUrl = ref("");
  const updateDialogPrimaryAction = ref<"download" | "force" | "restart" | null>(null);
  const updateProgressPercent = ref<number | null>(null);
  const updateRuntimeKind = ref<"installer" | "portable">("installer");
  const latestCheckResult = ref<GithubUpdateInfo | null>(null);
  const hasAvailableUpdate = computed(() => updateReadyToRestart.value || !!latestCheckResult.value?.hasUpdate);
  const checkingUpdate = computed(() => checkingUpdateRequest.value || updateInProgress.value);
  const updateUiMode = ref<"foreground" | "background" | null>(null);

  let updateProgressUnlisten: UnlistenFn | null = null;
  let dailyCheckTimer: number | null = null;
  let dailyCheckStarted = false;

  function runtimeLabel(kind: "installer" | "portable") {
    return kind === "portable" ? "便携版" : "安装版";
  }

  function closeUpdateDialog() {
    updateDialogOpen.value = false;
  }

  function openUpdateDialog(text: string, kind: "info" | "error", releaseUrl?: string) {
    updateDialogTitle.value = "检查更新";
    updateDialogBody.value = text;
    updateDialogKind.value = kind;
    updateDialogReleaseUrl.value = releaseUrl || "";
    updateDialogPrimaryAction.value = null;
    updateProgressPercent.value = null;
    updateDialogOpen.value = true;
  }

  function openUpdateRelease() {
    const url = updateDialogReleaseUrl.value.trim();
    if (!url) return;
    void invokeTauri("open_external_url", { url });
  }

  function buildCheckDialogBody(result: GithubUpdateInfo) {
    const lines = [
      `当前版本：${result.currentVersion}`,
      `最新版本：${result.latestVersion}`,
      `当前形态：${runtimeLabel(result.runtimeKind)}`,
    ];
    const notes = String(result.releaseNotes || "").trim();
    if (notes) {
      lines.push("");
      lines.push("更新说明：");
      lines.push(notes);
    }
    return lines.join("\n");
  }

  function openCheckResultDialog(result: GithubUpdateInfo) {
    updateReadyToRestart.value = false;
    latestCheckResult.value = result;
    updateRuntimeKind.value = result.runtimeKind;
    updateDialogReleaseUrl.value = result.releaseUrl || "";
    updateDialogBody.value = buildCheckDialogBody(result);
    updateDialogKind.value = "info";
    updateDialogPrimaryAction.value = result.hasUpdate ? "download" : "force";
    updateProgressPercent.value = null;
    updateDialogTitle.value = result.hasUpdate ? "发现可更新版本" : "当前已是最新版本";
    updateDialogOpen.value = true;
  }

  function clearDailyCheckTimer() {
    if (dailyCheckTimer != null) {
      window.clearTimeout(dailyCheckTimer);
      dailyCheckTimer = null;
    }
  }

  function msUntilNextFourAm() {
    const now = new Date();
    const next = new Date(now);
    next.setHours(4, 0, 0, 0);
    if (next.getTime() <= now.getTime()) {
      next.setDate(next.getDate() + 1);
    }
    return Math.max(1000, next.getTime() - now.getTime());
  }

  function scheduleNextDailyCheck() {
    clearDailyCheckTimer();
    dailyCheckTimer = window.setTimeout(() => {
      void checkGithubUpdate(true).finally(() => {
        scheduleNextDailyCheck();
      });
    }, msUntilNextFourAm());
  }

  function currentUpdateMethod(): GithubUpdateMethod {
    const value = options.updateMethod.value;
    return value === "direct" || value === "proxy" ? value : "auto";
  }

  function syncDialogFromProgress(payload: UpdateProgressPayload) {
    const previousUiMode = updateUiMode.value;
    updateRuntimeKind.value = payload.runtimeKind;
    updateDialogReleaseUrl.value = latestCheckResult.value?.releaseUrl || "";
    updateProgressPercent.value = Number.isFinite(payload.percent) ? payload.percent ?? null : null;
    if (payload.stage === "failed") {
      updateInProgress.value = false;
      updateReadyToRestart.value = false;
      updateUiMode.value = null;
      updateDialogPrimaryAction.value = null;
      updateDialogKind.value = "error";
      updateDialogTitle.value = "更新失败";
      updateDialogBody.value = payload.error ? `${payload.message}\n\n${payload.error}` : payload.message;
      if (previousUiMode !== "background") {
        updateDialogOpen.value = true;
      }
      return;
    }
    if (payload.stage === "ready") {
      updateInProgress.value = false;
      updateReadyToRestart.value = true;
      updateUiMode.value = null;
      if (latestCheckResult.value) {
        latestCheckResult.value = {
          ...latestCheckResult.value,
          hasUpdate: true,
        };
      }
      if (previousUiMode !== "background") {
        updateDialogOpen.value = true;
      }
      updateDialogKind.value = "info";
      updateDialogTitle.value = "更新已下载完成";
      updateDialogBody.value = payload.message;
      updateDialogPrimaryAction.value = "restart";
      updateProgressPercent.value = 100;
      return;
    }
    updateDialogKind.value = "info";
    updateDialogTitle.value = payload.stage === "completed" ? "更新完成" : "正在下载更新";
    const progressLine =
      Number.isFinite(payload.downloadedBytes) || Number.isFinite(payload.contentLength)
        ? `\n\n下载进度：${formatBytes(payload.downloadedBytes)} / ${formatBytes(payload.contentLength)}${
            Number.isFinite(payload.percent) ? ` (${Math.max(0, Math.min(100, payload.percent || 0)).toFixed(1)}%)` : ""
          }`
        : "";
    updateDialogBody.value = `${payload.message}\n\n当前形态：${runtimeLabel(payload.runtimeKind)}${progressLine}`;
    if (payload.stage === "completed") {
      updateInProgress.value = false;
      updateReadyToRestart.value = false;
      updateUiMode.value = null;
      updateDialogOpen.value = true;
      updateDialogPrimaryAction.value = null;
      return;
    }
    if (previousUiMode !== "background") {
      updateDialogOpen.value = true;
      updateDialogPrimaryAction.value = null;
    }
  }

  async function checkGithubUpdate(silent: boolean) {
    if (options.viewMode.value !== "config") return;
    if (checkingUpdate.value) return;
    checkingUpdateRequest.value = true;
    try {
      if (!silent) {
        options.status.value = "检查更新中...";
      }
      const result = await invokeTauri<GithubUpdateInfo>("check_github_update", { updateMethod: currentUpdateMethod() });
      latestCheckResult.value = result;
      updateRuntimeKind.value = result.runtimeKind;
      if (!result?.hasUpdate) {
        updateReadyToRestart.value = false;
        if (!silent) {
          options.status.value = `当前已是最新版本 ${result.currentVersion}`;
          openCheckResultDialog(result);
        }
        return result;
      }
      options.status.value = `发现新版本 ${result.latestVersion}（当前 ${result.currentVersion}）`;
      return result;
    } catch (error) {
      if (!silent) {
        options.status.value = `检查更新失败: ${String(error)}`;
        updateDialogPrimaryAction.value = null;
        openUpdateDialog(`检查更新失败：${String(error)}`, "error");
      }
      console.warn("[UPDATE] check_github_update failed:", error);
    } finally {
      checkingUpdateRequest.value = false;
    }
  }

  async function startGithubUpdate(force: boolean, silent: boolean) {
    if (checkingUpdate.value) return;
    updateInProgress.value = true;
    updateReadyToRestart.value = false;
    updateUiMode.value = silent ? "background" : "foreground";
    updateDialogPrimaryAction.value = null;
    updateDialogKind.value = "info";
    updateDialogTitle.value = force ? "准备强制下载更新" : "准备下载更新";
    updateDialogBody.value = force ? "正在准备强制下载更新..." : "正在准备下载更新...";
    updateProgressPercent.value = null;
    options.status.value = force ? "正在准备强制下载更新..." : "正在准备下载更新...";
    if (!silent) {
      updateDialogOpen.value = true;
    }
    try {
      await invokeTauri("start_github_update", { force, updateMethod: currentUpdateMethod() });
    } catch (error) {
      updateInProgress.value = false;
      updateUiMode.value = null;
      updateDialogKind.value = "error";
      updateDialogTitle.value = "更新失败";
      updateDialogBody.value = `启动更新失败：${String(error)}`;
      if (!silent) {
        updateDialogOpen.value = true;
      }
      options.status.value = `更新失败：${String(error)}`;
      console.warn("[UPDATE] start_github_update failed:", error);
    }
  }

  async function applyPreparedGithubUpdate() {
    if (checkingUpdate.value) return;
    updateInProgress.value = true;
    updateUiMode.value = "foreground";
    updateDialogOpen.value = true;
    updateDialogKind.value = "info";
    updateDialogPrimaryAction.value = null;
    updateDialogTitle.value = "更新并重启";
    updateDialogBody.value = "正在应用已下载的更新...";
    updateProgressPercent.value = null;
    options.status.value = "正在应用已下载的更新...";
    try {
      await invokeTauri("apply_prepared_github_update");
    } catch (error) {
      updateInProgress.value = false;
      updateUiMode.value = null;
      updateDialogKind.value = "error";
      updateDialogTitle.value = "更新失败";
      updateDialogBody.value = `应用更新失败：${String(error)}`;
      options.status.value = `更新失败：${String(error)}`;
      console.warn("[UPDATE] apply_prepared_github_update failed:", error);
    }
  }

  function confirmUpdateDialogPrimary() {
    if (updateDialogPrimaryAction.value === "download") {
      void startGithubUpdate(false, false);
      return;
    }
    if (updateDialogPrimaryAction.value === "force") {
      void startGithubUpdate(true, false);
      return;
    }
    if (updateDialogPrimaryAction.value === "restart") {
      void applyPreparedGithubUpdate();
    }
  }

  async function autoCheckGithubUpdate() {
    if (dailyCheckStarted) return;
    dailyCheckStarted = true;
    const result = await checkGithubUpdate(true);
    if (result?.hasUpdate && !updateReadyToRestart.value && !updateInProgress.value) {
      await startGithubUpdate(false, true);
    }
    scheduleNextDailyCheck();
  }

  async function manualCheckGithubUpdate() {
    const result = await checkGithubUpdate(false);
    if (result?.hasUpdate && !updateReadyToRestart.value) {
      await startGithubUpdate(false, false);
    }
  }

  async function triggerUpdateToLatest() {
    if (updateReadyToRestart.value) {
      await applyPreparedGithubUpdate();
      return;
    }
    if (updateInProgress.value || checkingUpdateRequest.value) {
      if (updateUiMode.value !== "background") {
        updateDialogOpen.value = true;
      }
      return;
    }
    if (latestCheckResult.value?.hasUpdate) {
      await startGithubUpdate(false, false);
      return;
    }
    await manualCheckGithubUpdate();
  }

  void listen<UpdateProgressPayload>("easy-call:update-status", (event) => {
    const payload = event.payload;
    if (!payload) return;
    updateInProgress.value = !["failed", "completed", "ready"].includes(payload.stage);
    syncDialogFromProgress(payload);
    options.status.value = payload.error ? payload.error : payload.message;
  })
    .then((unlisten) => {
      updateProgressUnlisten = unlisten;
    })
    .catch((error) => {
      console.warn("[UPDATE] listen easy-call:update-status failed:", error);
    });

  onBeforeUnmount(() => {
    updateProgressUnlisten?.();
    updateProgressUnlisten = null;
    clearDailyCheckTimer();
    dailyCheckStarted = false;
  });

  return {
    checkingUpdate,
    hasAvailableUpdate,
    updateReadyToRestart,
    latestCheckResult,
    updateDialogOpen,
    updateDialogTitle,
    updateDialogBody,
    updateDialogKind,
    updateDialogReleaseUrl,
    updateDialogPrimaryAction,
    updateProgressPercent,
    closeUpdateDialog,
    openUpdateRelease,
    confirmUpdateDialogPrimary,
    autoCheckGithubUpdate,
    manualCheckGithubUpdate,
    triggerUpdateToLatest,
  };
}
