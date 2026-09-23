import { computed, ref } from "vue";
import {
  getTransportCapabilities,
  listTransportFileReaderDirectoryOpenTargets,
  openTransportFileReaderDirectoryTarget,
  openTransportLocalDirectory,
} from "../../../services/tauri-api";
import { normalizePath } from "../utils";

export type DirectoryOpenTargetOption = {
  kind: string;
  label: string;
  type: "shell" | "vscode" | "explorer";
  iconDataUrl?: string;
};

export type DirectoryOpenTargetsResult = {
  options?: DirectoryOpenTargetOption[];
};

export const FILE_READER_OPEN_TARGET_STORAGE_KEY = "easy-call.file-reader.directory-open-target.v1";

export const DEFAULT_DIRECTORY_OPEN_TARGETS: DirectoryOpenTargetOption[] = [
  { kind: "explorer", label: "资源管理器", type: "explorer" },
  { kind: "vscode", label: "VS Code", type: "vscode" },
];

export function normalizeDirectoryOpenTargetLabel(item: DirectoryOpenTargetOption): string {
  const raw = String(item.label || "").trim();
  if (!raw) return "打开目标";
  if (item.type !== "shell") return raw;
  return raw.replace(/\s*\([^()]*\)\s*$/, "").trim() || raw;
}

export function normalizeDirectoryOpenTargetOptions(options: DirectoryOpenTargetOption[]): DirectoryOpenTargetOption[] {
  const seen = new Set<string>();
  return options
    .filter((item) => {
      const kind = String(item.kind || "").trim();
      if (!kind || kind === "auto" || seen.has(kind)) return false;
      seen.add(kind);
      return true;
    })
    .map((item): DirectoryOpenTargetOption => ({
      kind: String(item.kind || "").trim(),
      label: normalizeDirectoryOpenTargetLabel(item),
      type: item.type === "vscode" || item.type === "explorer" ? item.type : "shell",
      iconDataUrl: String(item.iconDataUrl || "").trim() || undefined,
    }));
}

export function readStoredDirectoryOpenTargetKind(): string {
  if (typeof window === "undefined") return "";
  try {
    return String(window.localStorage.getItem(FILE_READER_OPEN_TARGET_STORAGE_KEY) || "").trim();
  } catch {
    return "";
  }
}

export function storeDirectoryOpenTargetKind(kind: string): void {
  if (typeof window === "undefined") return;
  const normalized = String(kind || "").trim();
  try {
    if (normalized) {
      window.localStorage.setItem(FILE_READER_OPEN_TARGET_STORAGE_KEY, normalized);
    } else {
      window.localStorage.removeItem(FILE_READER_OPEN_TARGET_STORAGE_KEY);
    }
  } catch {
    // 忽略本地存储失败
  }
}

export function useDirectoryOpenTargets() {
  const localFileSystemAvailable = Boolean(getTransportCapabilities().localFileSystem);
  const directoryOpenTargetsLoading = ref(false);
  const directoryOpenTargetOptions = ref<DirectoryOpenTargetOption[]>([]);
  const selectedDirectoryOpenTargetKind = ref("explorer");

  const directoryOpenTargets = computed<DirectoryOpenTargetOption[]>(() => {
    const normalized = normalizeDirectoryOpenTargetOptions(directoryOpenTargetOptions.value);
    return normalized.length ? normalized : DEFAULT_DIRECTORY_OPEN_TARGETS;
  });

  function normalizeTargetKind(kind: string, options = directoryOpenTargets.value) {
    const normalized = String(kind || "").trim();
    if (normalized && options.some((item) => item.kind === normalized)) return normalized;
    return options[0]?.kind || "explorer";
  }

  function currentDirectoryOpenTargetKind() {
    return normalizeTargetKind(selectedDirectoryOpenTargetKind.value);
  }

  const currentDirectoryOpenTarget = computed<DirectoryOpenTargetOption>(() => {
    const currentKind = currentDirectoryOpenTargetKind();
    return directoryOpenTargets.value.find((item) => item.kind === currentKind)
      || directoryOpenTargets.value[0]
      || DEFAULT_DIRECTORY_OPEN_TARGETS[0];
  });

  const selectedDirectoryOpenTargetTitle = computed(
    () => `用 ${currentDirectoryOpenTarget.value.label} 打开当前目录`
  );

  async function loadDirectoryOpenTargets() {
    if (!localFileSystemAvailable) return;
    directoryOpenTargetsLoading.value = true;
    try {
      const payload = await listTransportFileReaderDirectoryOpenTargets<DirectoryOpenTargetsResult>();
      directoryOpenTargetOptions.value = normalizeDirectoryOpenTargetOptions(
        Array.isArray(payload.options) ? payload.options : []
      );
    } catch {
      directoryOpenTargetOptions.value = [];
    } finally {
      const stored = readStoredDirectoryOpenTargetKind();
      const fallbackKind = directoryOpenTargets.value[0]?.kind || "explorer";
      const nextKind = normalizeTargetKind(stored || fallbackKind);
      selectedDirectoryOpenTargetKind.value = nextKind;
      if (!stored || stored !== nextKind) {
        storeDirectoryOpenTargetKind(nextKind);
      }
      directoryOpenTargetsLoading.value = false;
    }
  }

  function selectDirectoryOpenTarget(kind: string) {
    const nextKind = normalizeTargetKind(kind);
    selectedDirectoryOpenTargetKind.value = nextKind;
    storeDirectoryOpenTargetKind(nextKind);
    return nextKind;
  }

  async function openDirectoryWithTarget(path: string, targetKind = currentDirectoryOpenTargetKind()) {
    if (!localFileSystemAvailable) return false;
    const normalizedPath = normalizePath(path);
    if (!normalizedPath) return false;
    return await openTransportFileReaderDirectoryTarget(normalizedPath, targetKind);
  }

  async function openDirectoryInFileManager(path: string) {
    if (!localFileSystemAvailable) return false;
    const normalizedPath = normalizePath(path);
    if (!normalizedPath) return false;
    return await openTransportLocalDirectory(normalizedPath);
  }

  return {
    localFileSystemAvailable,
    directoryOpenTargetsLoading,
    directoryOpenTargetOptions,
    selectedDirectoryOpenTargetKind,
    directoryOpenTargets,
    currentDirectoryOpenTarget,
    selectedDirectoryOpenTargetTitle,
    loadDirectoryOpenTargets,
    selectDirectoryOpenTarget,
    openDirectoryWithTarget,
    openDirectoryInFileManager,
  };
}
