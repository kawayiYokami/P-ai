import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import HomeWorkspaceCard from "./HomeWorkspaceCard.vue";
import { i18n } from "../../../../i18n";

const isMobileMock = vi.fn().mockReturnValue(false);

vi.mock("../../../../services/tauri-api", () => ({
  getTransportCapabilities: () => ({ localFileSystem: true }),
  listTransportFileReaderDirectoryOpenTargets: vi.fn().mockResolvedValue({
    options: [
      { kind: "pwsh", label: "PowerShell 7", type: "shell" },
      { kind: "vscode", label: "VS Code", type: "vscode" },
      { kind: "explorer", label: "资源管理器", type: "explorer" },
    ],
  }),
  openTransportFileReaderDirectoryTarget: vi.fn().mockResolvedValue(true),
  openTransportLocalDirectory: vi.fn().mockResolvedValue(true),
}));

vi.mock("../../../shared/utils/mobile-viewport", () => ({
  isMobileTouchViewport: () => isMobileMock(),
}));

describe("HomeWorkspaceCard", () => {
  const originalWindow = globalThis.window;
  let storage = new Map<string, string>();

  beforeEach(() => {
    isMobileMock.mockReturnValue(false);
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

  it("renders workspace name and tile label", async () => {
    const app = createSSRApp({
      render: () =>
        h(HomeWorkspaceCard, {
          workspaceRootPath: "E:/github/easy_call_ai",
        }),
    });
    app.use(i18n);
    const html = await renderToString(app);

    expect(html).toContain("easy_call_ai");
    expect(html).toContain("工作目录");
  });

  it("renders open target trigger buttons when workspaceRootPath exists on desktop", async () => {
    const app = createSSRApp({
      render: () =>
        h(HomeWorkspaceCard, {
          workspaceRootPath: "E:/github/easy_call_ai",
        }),
    });
    app.use(i18n);
    const html = await renderToString(app);

    expect(html).toContain("切换打开目标");
    expect(html).toContain("打开当前目录");
  });

  it("does not render open target action group when workspaceRootPath is empty", async () => {
    const app = createSSRApp({
      render: () =>
        h(HomeWorkspaceCard, {
          workspaceRootPath: "",
        }),
    });
    app.use(i18n);
    const html = await renderToString(app);

    expect(html).not.toContain("切换打开目标");
  });

  it("does not render open target action group on mobile touch viewport", async () => {
    isMobileMock.mockReturnValue(true);
    const app = createSSRApp({
      render: () =>
        h(HomeWorkspaceCard, {
          workspaceRootPath: "E:/github/easy_call_ai",
        }),
    });
    app.use(i18n);
    const html = await renderToString(app);

    expect(html).toContain("easy_call_ai");
    expect(html).toContain("工作目录");
    expect(html).not.toContain("切换打开目标");
    expect(html).not.toContain("打开当前目录");
  });

  it("renders worktree card and toggle button when in worktree mode with distinct worktreePath", async () => {
    const app = createSSRApp({
      render: () =>
        h(HomeWorkspaceCard, {
          workspaceRootPath: "E:/github/easy_call_ai",
          worktreePath: "E:/github/easy_call_ai/.pai/.worktree/feat",
          worktreeBranch: "feature/backend-tantivy",
          workMode: "worktree",
        }),
    });
    app.use(i18n);
    const html = await renderToString(app);

    expect(html).toContain("工作树");
    expect(html).toContain("feature/backend-tantivy");
    expect(html).toContain("ecall-home-workspace-toggle");
    expect(html).toContain("切换到工作目录");
  });

  it("does not render toggle button when worktreePath equals workspaceRootPath", async () => {
    const app = createSSRApp({
      render: () =>
        h(HomeWorkspaceCard, {
          workspaceRootPath: "E:/github/easy_call_ai",
          worktreePath: "E:/github/easy_call_ai",
          worktreeBranch: "feature/backend-tantivy",
          workMode: "worktree",
        }),
    });
    app.use(i18n);
    const html = await renderToString(app);

    expect(html).toContain("工作目录");
    expect(html).not.toContain("ecall-home-workspace-toggle");
  });
});
