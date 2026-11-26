import { computed, onBeforeUnmount, onMounted, ref, type Ref } from "vue";
import { i18n } from "../../../i18n";
import {
  applyPreparedTransportGithubUpdate,
  canUseTransportGithubUpdate,
  canUseTransportGithubUpdateCheck,
  cancelTransportGithubUpdate,
  checkTransportGithubUpdate,
  dismissPortablePendingManualReplace,
  getPortablePendingManualReplace,
  getTransportGithubUpdateState,
  invokeTauri,
  isDesktopTauriHost,
  onTransportNotification,
  openPortablePendingDir,
  openTransportExternalUrl,
  retryPortablePendingManualReplace,
  startTransportGithubUpdate,
} from "../../../services/tauri-api";
import type { PortablePendingManualReplace } from "../../../services/tauri-api";
import type { GithubUpdateInfo, GithubUpdateState, UpdateProgressPayload } from "../types/update";
import type { GithubUpdateMethod } from "../../../types/app";

const t = i18n.global.t;

type ViewModeRef = Ref<"chat" | "archives" | "config">;

type UseGithubUpdateOptions = {
  viewMode: ViewModeRef;
  status: Ref<string>;
  updateMethod: Ref<GithubUpdateMethod | undefined>;
  skippedVersion: Ref<string | undefined>;
  onSkippedVersionSaved: (config: { skippedGithubUpdateVersion?: string }) => void;
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

function normalizeSkippedVersion(value: string | undefined) {
  return String(value || "").trim();
}

function isCancellableUpdateStage(stage: string | null | undefined) {
  return ["checking", "downloading", "verifying", "preparing", "replacing"].includes(String(stage || ""));
}

/** 更新触发的后端重启会切断 Web 连接，此时请求以断连错误结束属于预期结果。 */
function isUpdateConnectionDroppedError(error: unknown) {
  const message = String((error as { message?: unknown })?.message || error || "");
  return message.includes("连接已断开") || message.includes("PAI 未运行") || message.includes("请求超时");
}

export function useGithubUpdate(options: UseGithubUpdateOptions) {
  const checkingUpdateRequest = ref(false);
  const updateInProgress = ref(false);
  const updateCancelPending = ref(false);
  const updateStage = ref<string | null>(null);
  const updateReadyToRestart = ref(false);
  const updateDialogOpen = ref(false);
  const updateDialogTitle = ref(t("about.dialogTitleCheck"));
  const updateDialogBody = ref("");
  const updateDialogKind = ref<"info" | "error">("info");
  const updateDialogReleaseUrl = ref("");
  const updateDialogPrimaryAction = ref<"download" | "force" | "restart" | null>(null);
  const updateProgressPercent = ref<number | null>(null);
  const updateRuntimeKind = ref<"installer" | "portable">("installer");
  const latestCheckResult = ref<GithubUpdateInfo | null>(null);
  const currentUpdateState = ref<GithubUpdateState | null>(null);
  const checkingUpdate = computed(() => checkingUpdateRequest.value || updateInProgress.value);
  const updateUiMode = ref<"foreground" | "background" | null>(null);
  const skippedVersion = computed(() => normalizeSkippedVersion(options.skippedVersion.value));

  // 应用更新由后端执行下载与替换，桌面与 Web/VS Code 宿主走同一份实现。
  function canUseGithubUpdate() {
    return canUseTransportGithubUpdate();
  }

  /** 检查远端版本两端都能做。 */
  function canCheckGithubUpdate() {
    return canUseTransportGithubUpdateCheck();
  }

  const updateSuppressedBySkip = computed(() => {
    const latestVersion = String(
      currentUpdateState.value?.latestVersion || latestCheckResult.value?.latestVersion || "",
    ).trim();
    return !!latestVersion && !!skippedVersion.value && latestVersion === skippedVersion.value;
  });
  const shouldShowUpdateAction = computed(() => {
    if (updateReadyToRestart.value) return !updateSuppressedBySkip.value;
    return !!currentUpdateState.value?.hasVisibleUpdate && !updateSuppressedBySkip.value;
  });
  const hasAvailableUpdate = computed(() => shouldShowUpdateAction.value);
  const showUpdateToLatestButton = computed(() => shouldShowUpdateAction.value);
  const latestUpdateVersion = computed(() =>
    String(currentUpdateState.value?.latestVersion || latestCheckResult.value?.latestVersion || "").trim(),
  );
  const updateDialogSkipVersionVisible = computed(() =>
    updateDialogOpen.value
    && (updateDialogPrimaryAction.value === "download" || updateDialogPrimaryAction.value === "restart")
    && !!latestUpdateVersion.value
    && !updateInProgress.value
  );
  const updateDialogCancelUpdateVisible = computed(() =>
    updateInProgress.value
    && !updateCancelPending.value
    && isCancellableUpdateStage(updateStage.value),
  );

  let updateProgressUnlisten: (() => void) | null = null;
  function runtimeLabel(kind: "installer" | "portable") {
    return kind === "portable" ? t("about.runtimePortable") : t("about.runtimeInstaller");
  }

  function closeUpdateDialog() {
    updateDialogOpen.value = false;
  }

  function openUpdateDialog(text: string, kind: "info" | "error", releaseUrl?: string) {
    updateDialogTitle.value = t("about.dialogTitleCheck");
    updateDialogBody.value = text;
    updateDialogKind.value = kind;
    updateDialogReleaseUrl.value = releaseUrl || "";
    updateDialogPrimaryAction.value = null;
    updateProgressPercent.value = null;
    updateStage.value = null;
    updateDialogOpen.value = true;
  }

  function openUpdateRelease() {
    const url = String(updateDialogReleaseUrl.value || latestCheckResult.value?.releaseUrl || "").trim();
    if (!url) return;
    void openTransportExternalUrl(url);
  }

  function buildCheckDialogBody(result: GithubUpdateInfo) {
    const lines = [
      t("about.currentVersion", { version: result.currentVersion }),
      t("about.latestVersion", { version: result.latestVersion }),
      t("about.currentRuntime", { kind: runtimeLabel(result.runtimeKind) }),
    ];
    const notes = String(result.releaseNotes || "").trim();
    if (notes) {
      lines.push("");
      lines.push(t("about.releaseNotes"));
      lines.push(notes);
    }
    return lines.join("\n");
  }

  function openCheckResultDialog(result: GithubUpdateInfo) {
    updateDialogReleaseUrl.value = result.releaseUrl || "";
    updateDialogBody.value = buildCheckDialogBody(result);
    updateDialogKind.value = "info";
    // 下载与安装都由后端执行，两端给出同一组操作
    updateDialogPrimaryAction.value = result.hasUpdate ? "download" : "force";
    updateProgressPercent.value = null;
    updateStage.value = null;
    updateDialogTitle.value = result.hasUpdate ? t("about.foundUpdate") : t("about.alreadyLatest");
    updateDialogOpen.value = true;
  }

  function openPreparedUpdateDialog() {
    const latestVersion = latestUpdateVersion.value;
    const currentVersion = String(
      currentUpdateState.value?.currentVersion || latestCheckResult.value?.currentVersion || "",
    ).trim();
    const runtimeKind = currentUpdateState.value?.runtimeKind || latestCheckResult.value?.runtimeKind || "installer";
    const releaseNotes = String(
      currentUpdateState.value?.releaseNotes || latestCheckResult.value?.releaseNotes || "",
    ).trim();
    updateDialogReleaseUrl.value = currentUpdateState.value?.releaseUrl || latestCheckResult.value?.releaseUrl || "";
    updateDialogKind.value = "info";
    updateDialogPrimaryAction.value = "restart";
    updateProgressPercent.value = null;
    updateStage.value = "ready";
    updateDialogTitle.value = t("about.updateDownloaded");
    const lines = [
      t("about.currentVersion", { version: currentVersion || envVersionFallback() }),
      t("about.latestVersion", { version: latestVersion }),
      t("about.currentRuntime", { kind: runtimeLabel(runtimeKind) }),
    ];
    if (releaseNotes) {
      lines.push("");
      lines.push(t("about.releaseNotes"));
      lines.push(releaseNotes);
    }
    updateDialogBody.value = withWebRestartHint(lines.join("\n"));
    updateDialogOpen.value = true;
  }

  function envVersionFallback() {
    return latestCheckResult.value?.currentVersion || currentUpdateState.value?.currentVersion || "";
  }

  /** Web/VS Code 宿主跑在浏览器里，更新会重启后端进程，当前页面连接必然断开。 */
  function withWebRestartHint(body: string) {
    if (isDesktopTauriHost()) return body;
    return `${body}\n\n${t("about.webRestartDisconnectHint")}`;
  }

  function currentUpdateMethod(): GithubUpdateMethod {
    const value = options.updateMethod.value;
    return value === "direct" || value === "proxy" ? value : "auto";
  }

  function applySkippedVersion(version: string) {
    options.onSkippedVersionSaved({
      skippedGithubUpdateVersion: String(version || "").trim(),
    });
  }

  async function saveSkippedVersion(version: string) {
    const saved = await invokeTauri<{ skippedGithubUpdateVersion?: string }>("set_skipped_github_update_version", { version });
    applySkippedVersion(saved.skippedGithubUpdateVersion || "");
    if (currentUpdateState.value) {
      currentUpdateState.value = {
        ...currentUpdateState.value,
        skippedVersion: saved.skippedGithubUpdateVersion || "",
        hasVisibleUpdate: false,
      };
    }
  }

  function syncCurrentUpdateState(state: GithubUpdateState | null | undefined) {
    if (!state) return;
    currentUpdateState.value = state;
    updateRuntimeKind.value = state.runtimeKind;
    updateReadyToRestart.value = !!state.hasPreparedUpdate;
    if (state.releaseUrl) updateDialogReleaseUrl.value = state.releaseUrl;
    if (state.hasPreparedUpdate) {
      latestCheckResult.value = {
        currentVersion: state.currentVersion,
        latestVersion: state.latestVersion,
        hasUpdate: true,
        releaseUrl: state.releaseUrl,
        updateSource: "github",
        accessMode: "direct",
        releaseNotes: state.releaseNotes,
        publishedAt: state.publishedAt,
        runtimeKind: state.runtimeKind,
        canForceUpdate: true,
      };
    }
  }

  async function refreshGithubUpdateState() {
    if (!canUseGithubUpdate()) return null;
    try {
      const state = await getTransportGithubUpdateState<GithubUpdateState>();
      syncCurrentUpdateState(state);
      return state;
    } catch (error) {
      console.warn("[自动更新] 读取更新状态失败", error);
      return null;
    }
  }

  function syncDialogFromProgress(payload: UpdateProgressPayload) {
    const previousUiMode = updateUiMode.value;
    updateStage.value = payload.stage;
    updateRuntimeKind.value = payload.runtimeKind;
    updateDialogReleaseUrl.value = latestCheckResult.value?.releaseUrl || "";
    updateProgressPercent.value = Number.isFinite(payload.percent) ? payload.percent ?? null : null;
    if (payload.stage === "failed") {
      updateInProgress.value = false;
      updateCancelPending.value = false;
      updateReadyToRestart.value = false;
      updateUiMode.value = null;
      updateDialogPrimaryAction.value = null;
      updateDialogKind.value = "error";
      updateDialogTitle.value = t("about.updateFailed");
      updateDialogBody.value = payload.error ? `${payload.message}\n\n${payload.error}` : payload.message;
      if (previousUiMode === "foreground") {
        updateDialogOpen.value = true;
      }
      return;
    }
    if (payload.stage === "cancelled") {
      updateInProgress.value = false;
      updateCancelPending.value = false;
      updateReadyToRestart.value = false;
      updateUiMode.value = null;
      updateDialogPrimaryAction.value = null;
      updateProgressPercent.value = null;
      updateDialogOpen.value = false;
      return;
    }
    if (payload.stage === "ready") {
      updateInProgress.value = false;
      updateCancelPending.value = false;
      updateReadyToRestart.value = true;
      updateUiMode.value = null;
      if (latestCheckResult.value) {
        latestCheckResult.value = {
          ...latestCheckResult.value,
          hasUpdate: true,
        };
      }
      if (previousUiMode === "foreground") {
        updateDialogOpen.value = true;
      }
      updateDialogKind.value = "info";
      updateDialogTitle.value = t("about.updateDownloaded");
      updateDialogBody.value = withWebRestartHint(String(payload.message || ""));
      updateDialogPrimaryAction.value = "restart";
      updateProgressPercent.value = 100;
      return;
    }
    updateDialogKind.value = "info";
    updateDialogTitle.value =
      payload.stage === "completed"
        ? t("about.updateCompleted")
        : payload.stage === "checking"
          ? t("about.checking")
          : t("about.downloading");
    const progressLine =
      Number.isFinite(payload.downloadedBytes) || Number.isFinite(payload.contentLength)
        ? `\n\n${t("about.downloadProgress", { current: formatBytes(payload.downloadedBytes), total: formatBytes(payload.contentLength) })}${
            Number.isFinite(payload.percent) ? ` (${Math.max(0, Math.min(100, payload.percent || 0)).toFixed(1)}%)` : ""
          }`
        : "";
    updateDialogBody.value = `${payload.message}\n\n${t("about.currentRuntime", { kind: runtimeLabel(payload.runtimeKind) })}${progressLine}`;
    if (payload.stage === "completed") {
      updateInProgress.value = false;
      updateCancelPending.value = false;
      updateReadyToRestart.value = false;
      updateUiMode.value = null;
      updateDialogOpen.value = true;
      updateDialogPrimaryAction.value = null;
      return;
    }
    if (previousUiMode === "foreground") {
      updateDialogOpen.value = true;
      updateDialogPrimaryAction.value = null;
    }
  }

  function isUpdateProgressPayload(payload: unknown): payload is UpdateProgressPayload {
    return !!payload && typeof payload === "object" && typeof (payload as UpdateProgressPayload).stage === "string";
  }

  function handleUpdateProgressPayload(payload: UpdateProgressPayload | null | undefined) {
    if (!payload) return;
    updateInProgress.value = !["failed", "completed", "ready", "cancelled"].includes(payload.stage);
    syncDialogFromProgress(payload);
    options.status.value = payload.error ? payload.error : payload.message;
    void refreshGithubUpdateState();
  }

  async function checkGithubUpdate(silent: boolean, respectCooldown = false) {
    if (!canCheckGithubUpdate()) return null;
    if (options.viewMode.value === "archives") return;
    if (checkingUpdate.value) return;
    checkingUpdateRequest.value = true;
    try {
      if (!silent) {
        options.status.value = t("about.checking");
      }
      const result = await checkTransportGithubUpdate<GithubUpdateInfo>({
        updateMethod: currentUpdateMethod(),
        respectCooldown,
      });
      latestCheckResult.value = result;
      updateRuntimeKind.value = result.runtimeKind;
      updateDialogReleaseUrl.value = result.releaseUrl || "";
      // 更新状态（是否已下载、待重启）由后端统一维护，两端同一份
      const latestState = await refreshGithubUpdateState();
      if (!result?.hasUpdate) {
        updateReadyToRestart.value = false;
        if (!silent) {
          options.status.value = t("about.alreadyLatestWithVersion", { version: result.currentVersion });
          openCheckResultDialog(result);
        }
        return result;
      }
      options.status.value = t("about.foundNewVersion", { latest: result.latestVersion, current: result.currentVersion });
      if (!silent || !updateSuppressedBySkip.value) {
        if (latestState?.hasPreparedUpdate && latestState.latestVersion === result.latestVersion) {
          openPreparedUpdateDialog();
        } else {
          openCheckResultDialog(result);
        }
      }
      return result;
    } catch (error) {
      if (!silent) {
        options.status.value = t("about.checkFailed", { error: String(error) });
        updateDialogPrimaryAction.value = null;
        openUpdateDialog(t("about.checkFailedDialog", { error: String(error) }), "error");
      }
      console.warn("[自动更新] 检查更新失败", error);
    } finally {
      checkingUpdateRequest.value = false;
    }
  }

  async function startGithubUpdate(force: boolean, silent: boolean) {
    if (checkingUpdate.value) return;
    updateInProgress.value = true;
    updateCancelPending.value = false;
    updateStage.value = "checking";
    updateReadyToRestart.value = false;
    updateUiMode.value = silent ? "background" : "foreground";
    updateDialogPrimaryAction.value = null;
    updateDialogKind.value = "info";
    updateDialogTitle.value = force ? t("about.prepareForceDownload") : t("about.prepareDownload");
    updateDialogBody.value = force ? t("about.preparingForceDownload") : t("about.preparingDownload");
    updateProgressPercent.value = null;
    options.status.value = force ? t("about.preparingForceDownload") : t("about.preparingDownload");
    if (!silent) {
      updateDialogOpen.value = true;
    }
    try {
      await startTransportGithubUpdate({ force, updateMethod: currentUpdateMethod() });
    } catch (error) {
      if (String(error || "").includes("用户已取消更新")) {
        updateInProgress.value = false;
        updateCancelPending.value = false;
        updateStage.value = null;
        updateUiMode.value = null;
        updateDialogOpen.value = false;
        updateProgressPercent.value = null;
        options.status.value = t("about.cancellingUpdate");
        return;
      }
      updateInProgress.value = false;
      updateCancelPending.value = false;
      updateStage.value = null;
      updateUiMode.value = null;
      updateDialogKind.value = "error";
      updateDialogTitle.value = t("about.updateFailed");
      updateDialogBody.value = t("about.startUpdateFailed", { error: String(error) });
      if (!silent) {
        updateDialogOpen.value = true;
      }
      options.status.value = t("about.startUpdateFailedStatus", { error: String(error) });
      console.warn("[自动更新] 启动更新失败", error);
    }
  }

  async function cancelGithubUpdate() {
    if (!updateInProgress.value || updateCancelPending.value || !isCancellableUpdateStage(updateStage.value)) return;
    updateCancelPending.value = true;
    options.status.value = t("about.cancellingUpdate");
    try {
      await cancelTransportGithubUpdate();
    } catch (error) {
      updateCancelPending.value = false;
      options.status.value = t("about.cancelUpdateFailed", { error: String(error) });
      openUpdateDialog(t("about.cancelUpdateFailed", { error: String(error) }), "error");
    }
  }

  async function applyPreparedGithubUpdate() {
    if (checkingUpdate.value) return;
    updateInProgress.value = true;
    updateCancelPending.value = false;
    updateStage.value = "installing";
    updateUiMode.value = "foreground";
    updateDialogOpen.value = true;
    updateDialogKind.value = "info";
    updateDialogPrimaryAction.value = null;
    updateDialogTitle.value = t("about.updateAndRestartTitle");
    updateDialogBody.value = withWebRestartHint(t("about.applyingUpdate"));
    updateProgressPercent.value = null;
    options.status.value = t("about.applyingUpdate");
    try {
      await applyPreparedTransportGithubUpdate();
    } catch (error) {
      if (!isDesktopTauriHost() && isUpdateConnectionDroppedError(error)) {
        // 后端已在重启，页面连接被切断是预期行为，不能当成更新失败
        updateInProgress.value = false;
        updateCancelPending.value = false;
        updateStage.value = "completed";
        updateUiMode.value = null;
        updateDialogKind.value = "info";
        updateDialogTitle.value = t("about.updateCompleted");
        updateDialogBody.value = t("about.webRestartDisconnectHint");
        updateDialogPrimaryAction.value = null;
        updateProgressPercent.value = 100;
        options.status.value = t("about.webRestartDisconnectHint");
        return;
      }
      updateInProgress.value = false;
      updateCancelPending.value = false;
      updateStage.value = null;
      updateUiMode.value = null;
      updateDialogKind.value = "error";
      updateDialogTitle.value = t("about.updateFailed");
      updateDialogBody.value = t("about.applyUpdateFailed", { error: String(error) });
      options.status.value = t("about.applyUpdateFailedStatus", { error: String(error) });
      console.warn("[自动更新] 应用已下载更新失败", error);
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

  async function skipCurrentUpdateVersion() {
    const version = latestUpdateVersion.value;
    if (!version) return;
    await saveSkippedVersion(version);
    updateDialogOpen.value = false;
    options.status.value = t("about.skipVersionSaved", { version });
  }

  async function manualCheckGithubUpdate() {
    await checkGithubUpdate(false);
  }

  async function triggerUpdateToLatest() {
    if (updateReadyToRestart.value) {
      openPreparedUpdateDialog();
      return;
    }
    if (updateInProgress.value || checkingUpdateRequest.value) {
      if (updateUiMode.value === "foreground") {
        updateDialogOpen.value = true;
      }
      return;
    }
    if (currentUpdateState.value?.hasVisibleUpdate) {
      openPreparedUpdateDialog();
      return;
    }
    const state = await refreshGithubUpdateState();
    if (updateReadyToRestart.value || state?.hasVisibleUpdate) {
      openPreparedUpdateDialog();
      return;
    }
    // 状态里没有可更新版本时也要给出结果，不能让点击静默结束
    await checkGithubUpdate(false);
  }

  const portablePending = ref<PortablePendingManualReplace | null>(null);
  async function refreshPortablePending() {
    if (!canUseGithubUpdate()) return null;
    try {
      const pending = await getPortablePendingManualReplace();
      portablePending.value = pending;
      return pending;
    } catch {
      return null;
    }
  }
  function buildPortablePendingDialogBody(_pending: PortablePendingManualReplace): string {
    // 弹窗正文与卡片会重复，pending 场景下正文留空，细节由 ShellDialogsHost 的卡片展示
    return "";
  }
  async function openPortablePendingDialog() {
    const pending = portablePending.value || (await refreshPortablePending());
    if (!pending) return;
    updateDialogKind.value = "info";
    updateDialogTitle.value = t("about.portablePendingTitle");
    updateDialogBody.value = buildPortablePendingDialogBody(pending);
    updateDialogPrimaryAction.value = null;
    updateProgressPercent.value = null;
    updateStage.value = "failed";
    updateDialogOpen.value = true;
  }
  async function checkAndShowPortablePending() {
    const pending = await refreshPortablePending();
    if (pending) {
      await openPortablePendingDialog();
      return true;
    }
    return false;
  }
  async function dismissPortablePending() {
    try {
      await dismissPortablePendingManualReplace();
    } catch {
      // 非便携版或文件已清也视为已关闭
    }
    portablePending.value = null;
    updateDialogOpen.value = false;
  }
  async function retryPortablePending() {
    try {
      await retryPortablePendingManualReplace();
    } catch (error) {
      portablePending.value = null;
      updateDialogKind.value = "error";
      updateDialogTitle.value = t("about.portablePendingTitle");
      updateDialogBody.value = String(error ?? t("about.applyUpdateFailed", { error: "" }));
      updateDialogOpen.value = true;
    }
  }

  updateProgressUnlisten = onTransportNotification("easy-call:update-status", (payload) => {
    if (isUpdateProgressPayload(payload)) handleUpdateProgressPayload(payload);
  });

  onMounted(() => {
    // 仅主窗口自动弹窗，避免 chat/config 双窗口同时弹窗打扰
    if (options.viewMode.value !== "chat") return;
    void checkAndShowPortablePending();
  });

  onBeforeUnmount(() => {
    updateProgressUnlisten?.();
    updateProgressUnlisten = null;
  });

  return {
    checkingUpdate,
    hasAvailableUpdate,
    updateReadyToRestart,
    updateInProgress,
    updateCancelPending,
    latestCheckResult,
    currentUpdateState,
    updateDialogOpen,
    updateDialogTitle,
    updateDialogBody,
    updateDialogKind,
    updateDialogReleaseUrl,
    updateDialogPrimaryAction,
    updateProgressPercent,
    updateDialogSkipVersionVisible,
    updateDialogCancelUpdateVisible,
    closeUpdateDialog,
    openUpdateRelease,
    confirmUpdateDialogPrimary,
    refreshGithubUpdateState,
    manualCheckGithubUpdate,
    triggerUpdateToLatest,
    cancelGithubUpdate,
    skipCurrentUpdateVersion,
    showUpdateToLatestButton,
    portablePending,
    refreshPortablePending,
    openPortablePendingDialog,
    checkAndShowPortablePending,
    dismissPortablePending,
    retryPortablePending,
    openPortablePendingDir,
  };
}
