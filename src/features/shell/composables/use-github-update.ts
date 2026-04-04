import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, onBeforeUnmount, ref, type Ref } from "vue";
import { invokeTauri } from "../../../services/tauri-api";
import type { GithubUpdateInfo, UpdateProgressPayload } from "../types/update";

type ViewModeRef = Ref<"chat" | "archives" | "config">;

type UseGithubUpdateOptions = {
  viewMode: ViewModeRef;
  status: Ref<string>;
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
  const updateDialogOpen = ref(false);
  const updateDialogTitle = ref("检查更新");
  const updateDialogBody = ref("");
  const updateDialogKind = ref<"info" | "error">("info");
  const updateDialogReleaseUrl = ref("");
  const updateDialogPrimaryAction = ref<"update" | "force" | null>(null);
  const updateProgressPercent = ref<number | null>(null);
  const updateRuntimeKind = ref<"installer" | "portable">("installer");
  const latestCheckResult = ref<GithubUpdateInfo | null>(null);
  const checkingUpdate = computed(() => checkingUpdateRequest.value || updateInProgress.value);

  let updateProgressUnlisten: UnlistenFn | null = null;

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
    latestCheckResult.value = result;
    updateRuntimeKind.value = result.runtimeKind;
    updateDialogReleaseUrl.value = result.releaseUrl || "";
    updateDialogBody.value = buildCheckDialogBody(result);
    updateDialogKind.value = "info";
    updateDialogPrimaryAction.value = result.hasUpdate ? "update" : "force";
    updateProgressPercent.value = null;
    updateDialogTitle.value = result.hasUpdate ? "发现可更新版本" : "当前已是最新版本";
    updateDialogOpen.value = true;
  }

  function syncDialogFromProgress(payload: UpdateProgressPayload) {
    updateRuntimeKind.value = payload.runtimeKind;
    updateDialogReleaseUrl.value = latestCheckResult.value?.releaseUrl || "";
    updateDialogOpen.value = true;
    updateDialogPrimaryAction.value = null;
    updateProgressPercent.value = Number.isFinite(payload.percent) ? payload.percent ?? null : null;
    if (payload.stage === "failed") {
      updateInProgress.value = false;
      updateDialogKind.value = "error";
      updateDialogTitle.value = "更新失败";
      updateDialogBody.value = payload.error ? `${payload.message}\n\n${payload.error}` : payload.message;
      return;
    }
    updateDialogKind.value = "info";
    updateDialogTitle.value = payload.stage === "completed" ? "更新完成" : "正在更新";
    const progressLine =
      Number.isFinite(payload.downloadedBytes) || Number.isFinite(payload.contentLength)
        ? `\n\n下载进度：${formatBytes(payload.downloadedBytes)} / ${formatBytes(payload.contentLength)}${
            Number.isFinite(payload.percent) ? ` (${Math.max(0, Math.min(100, payload.percent || 0)).toFixed(1)}%)` : ""
          }`
        : "";
    updateDialogBody.value = `${payload.message}\n\n当前形态：${runtimeLabel(payload.runtimeKind)}${progressLine}`;
    if (payload.stage === "completed") {
      updateInProgress.value = false;
    }
  }

  async function checkGithubUpdate(silent: boolean) {
    if (options.viewMode.value !== "config") return;
    if (checkingUpdate.value) return;
    checkingUpdateRequest.value = true;
    try {
      options.status.value = "检查更新中...";
      const result = await invokeTauri<GithubUpdateInfo>("check_github_update");
      latestCheckResult.value = result;
      updateRuntimeKind.value = result.runtimeKind;
      if (!result?.hasUpdate) {
        options.status.value = `当前已是最新版本 ${result.currentVersion}`;
        if (!silent) {
          openCheckResultDialog(result);
        }
        return;
      }
      options.status.value = `发现新版本 ${result.latestVersion}（当前 ${result.currentVersion}）`;
      if (!silent) {
        openCheckResultDialog(result);
      }
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

  async function startGithubUpdate(force: boolean) {
    if (checkingUpdate.value) return;
    updateInProgress.value = true;
    updateDialogPrimaryAction.value = null;
    updateDialogKind.value = "info";
    updateDialogTitle.value = force ? "准备强制更新" : "准备更新";
    updateDialogBody.value = force ? "正在准备强制更新..." : "正在准备更新...";
    updateDialogOpen.value = true;
    updateProgressPercent.value = null;
    options.status.value = force ? "正在准备强制更新..." : "正在准备更新...";
    try {
      await invokeTauri("start_github_update", { force });
    } catch (error) {
      updateInProgress.value = false;
      updateDialogKind.value = "error";
      updateDialogTitle.value = "更新失败";
      updateDialogBody.value = `启动更新失败：${String(error)}`;
      updateDialogOpen.value = true;
      options.status.value = `更新失败：${String(error)}`;
      console.warn("[UPDATE] start_github_update failed:", error);
    }
  }

  function confirmUpdateDialogPrimary() {
    if (updateDialogPrimaryAction.value === "update") {
      void startGithubUpdate(false);
      return;
    }
    if (updateDialogPrimaryAction.value === "force") {
      void startGithubUpdate(true);
    }
  }

  async function autoCheckGithubUpdate() {
    await checkGithubUpdate(true);
  }

  async function manualCheckGithubUpdate() {
    await checkGithubUpdate(false);
  }

  void listen<UpdateProgressPayload>("easy-call:update-status", (event) => {
    const payload = event.payload;
    if (!payload) return;
    updateInProgress.value = payload.stage !== "failed" && payload.stage !== "completed";
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
  });

  return {
    checkingUpdate,
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
  };
}
