import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  DEFAULT_DIRECTORY_OPEN_TARGETS,
  FILE_READER_OPEN_TARGET_STORAGE_KEY,
  normalizeDirectoryOpenTargetLabel,
  normalizeDirectoryOpenTargetOptions,
  readStoredDirectoryOpenTargetKind,
  storeDirectoryOpenTargetKind,
  useDirectoryOpenTargets,
  type DirectoryOpenTargetOption,
} from "./use-directory-open-targets";

vi.mock("../../../services/tauri-api", () => ({
  getTransportCapabilities: () => ({ localFileSystem: true }),
  listTransportFileReaderDirectoryOpenTargets: vi.fn().mockResolvedValue({
    options: [
      { kind: "pwsh", label: "PowerShell 7", type: "shell" },
      { kind: "git-bash", label: "Git Bash", type: "shell" },
      { kind: "vscode", label: "VS Code", type: "vscode" },
      { kind: "explorer", label: "资源管理器", type: "explorer" },
    ],
  }),
  openTransportFileReaderDirectoryTarget: vi.fn().mockResolvedValue(true),
  openTransportLocalDirectory: vi.fn().mockResolvedValue(true),
}));

describe("use-directory-open-targets", () => {
  const originalWindow = globalThis.window;
  let storage = new Map<string, string>();

  beforeEach(() => {
    storage = new Map<string, string>();
    const fakeWindow: Record<string, unknown> = {
      localStorage: {
        getItem: (key: string) => storage.get(key) ?? null,
        setItem: (key: string, value: string) => storage.set(key, String(value)),
        removeItem: (key: string) => storage.delete(key),
        clear: () => storage.clear(),
      },
    };
    (globalThis as unknown as { window: unknown }).window = fakeWindow;
  });

  afterEach(() => {
    (globalThis as unknown as { window: unknown }).window = originalWindow;
  });

  describe("helper functions", () => {
    it("normalizeDirectoryOpenTargetLabel strips shell suffixes", () => {
      expect(normalizeDirectoryOpenTargetLabel({ kind: "pwsh", label: "PowerShell 7 (pwsh.exe)", type: "shell" })).toBe("PowerShell 7");
      expect(normalizeDirectoryOpenTargetLabel({ kind: "vscode", label: "VS Code (Code.exe)", type: "vscode" })).toBe("VS Code (Code.exe)");
      expect(normalizeDirectoryOpenTargetLabel({ kind: "custom", label: "", type: "shell" })).toBe("打开目标");
    });

    it("normalizeDirectoryOpenTargetOptions filters invalid and deduplicates", () => {
      const options: DirectoryOpenTargetOption[] = [
        { kind: "auto", label: "Auto", type: "shell" },
        { kind: "", label: "Empty", type: "shell" },
        { kind: "pwsh", label: "PowerShell 7", type: "shell" },
        { kind: "pwsh", label: "Duplicate", type: "shell" },
        { kind: "vscode", label: "VS Code", type: "vscode" },
      ];
      const result = normalizeDirectoryOpenTargetOptions(options);
      expect(result).toHaveLength(2);
      expect(result[0].kind).toBe("pwsh");
      expect(result[1].kind).toBe("vscode");
    });

    it("reads and stores target kind in localStorage", () => {
      expect(readStoredDirectoryOpenTargetKind()).toBe("");
      storeDirectoryOpenTargetKind("git-bash");
      expect(readStoredDirectoryOpenTargetKind()).toBe("git-bash");
      expect(storage.get(FILE_READER_OPEN_TARGET_STORAGE_KEY)).toBe("git-bash");
      storeDirectoryOpenTargetKind("");
      expect(readStoredDirectoryOpenTargetKind()).toBe("");
    });
  });

  describe("useDirectoryOpenTargets composable", () => {
    it("provides default targets when no options are loaded yet", () => {
      const { directoryOpenTargets, currentDirectoryOpenTarget } = useDirectoryOpenTargets();
      expect(directoryOpenTargets.value).toEqual(DEFAULT_DIRECTORY_OPEN_TARGETS);
      expect(currentDirectoryOpenTarget.value.kind).toBe("explorer");
    });

    it("loads targets and selects stored target if available", async () => {
      storeDirectoryOpenTargetKind("git-bash");
      const { loadDirectoryOpenTargets, directoryOpenTargets, currentDirectoryOpenTarget, selectedDirectoryOpenTargetKind } = useDirectoryOpenTargets();
      await loadDirectoryOpenTargets();
      expect(directoryOpenTargets.value).toHaveLength(4);
      expect(selectedDirectoryOpenTargetKind.value).toBe("git-bash");
      expect(currentDirectoryOpenTarget.value.kind).toBe("git-bash");
      expect(currentDirectoryOpenTarget.value.label).toBe("Git Bash");
    });

    it("selects a target and stores it", async () => {
      const { selectDirectoryOpenTarget, selectedDirectoryOpenTargetKind } = useDirectoryOpenTargets();
      selectDirectoryOpenTarget("vscode");
      expect(selectedDirectoryOpenTargetKind.value).toBe("vscode");
      expect(readStoredDirectoryOpenTargetKind()).toBe("vscode");
    });

    it("calls openTransportFileReaderDirectoryTarget on openDirectoryWithTarget", async () => {
      const { openDirectoryWithTarget } = useDirectoryOpenTargets();
      const result = await openDirectoryWithTarget("D:/project", "pwsh");
      expect(result).toBe(true);
    });
  });
});
