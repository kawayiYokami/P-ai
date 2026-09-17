import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  isSameRepoPath,
  readRememberedRepoRoot,
  rememberRepoRoot,
} from "./use-workspace-git-status";

describe("isSameRepoPath", () => {
  it("斜杠方向与盘符大小写差异视为同一仓库", () => {
    expect(isSameRepoPath("E:\\github\\easy_call_ai", "e:/github/easy_call_ai")).toBe(true);
  });

  it("尾斜杠不影响比较", () => {
    expect(isSameRepoPath("E:/repo/", "E:/repo")).toBe(true);
  });

  it("不同仓库不视为同一个", () => {
    expect(isSameRepoPath("E:/repo-a", "E:/repo-b")).toBe(false);
  });

  it("空路径不与任何路径（含空路径）视为同一个", () => {
    expect(isSameRepoPath("", "")).toBe(false);
    expect(isSameRepoPath("", "E:/repo")).toBe(false);
    expect(isSameRepoPath("E:/repo", "")).toBe(false);
  });
});

// 共享源按 globalThis.localStorage 存取会话级仓库记忆；node 环境没有 localStorage，测试里注入最小实现
function installMemoryStorage(): Map<string, string> {
  const store = new Map<string, string>();
  (globalThis as { localStorage?: Storage }).localStorage = {
    getItem: (key: string) => store.get(key) ?? null,
    setItem: (key: string, value: string) => {
      store.set(key, String(value));
    },
    removeItem: (key: string) => {
      store.delete(key);
    },
    clear: () => {
      store.clear();
    },
    key: (index: number) => Array.from(store.keys())[index] ?? null,
    get length() {
      return store.size;
    },
  } as unknown as Storage;
  return store;
}

describe("会话级仓库选择记忆", () => {
  const sessionKey = "easy_call.chat_file_reader_session.conv-1.v1";
  const otherSessionKey = "easy_call.chat_file_reader_session.conv-2.v1";
  let store: Map<string, string>;
  let originalStorage: Storage | undefined;

  beforeEach(() => {
    originalStorage = (globalThis as { localStorage?: Storage }).localStorage;
    store = installMemoryStorage();
  });

  afterEach(() => {
    if (originalStorage) {
      (globalThis as { localStorage?: Storage }).localStorage = originalStorage;
    } else {
      delete (globalThis as { localStorage?: Storage }).localStorage;
    }
  });

  it("记住的仓库仍在探测列表里时，返回列表里的那份路径", () => {
    rememberRepoRoot(sessionKey, "E:\\repos\\sub");
    expect(readRememberedRepoRoot(sessionKey, ["E:/repos/root", "e:/repos/sub"])).toBe("e:/repos/sub");
  });

  it("记住的仓库已不在列表里时回落默认，并清掉记忆", () => {
    rememberRepoRoot(sessionKey, "E:/repos/gone");
    expect(readRememberedRepoRoot(sessionKey, ["E:/repos/root"])).toBe("");
    expect(store.has(`${sessionKey}:git-panel-repo`)).toBe(false);
  });

  it("探测列表为空时保留记忆，不做破坏性清理", () => {
    rememberRepoRoot(sessionKey, "E:/repos/sub");
    expect(readRememberedRepoRoot(sessionKey, [])).toBe("");
    expect(store.get(`${sessionKey}:git-panel-repo`)).toBe("E:/repos/sub");
  });

  it("记忆按会话隔离，不互相影响", () => {
    rememberRepoRoot(sessionKey, "E:/repos/a");
    expect(readRememberedRepoRoot(otherSessionKey, ["E:/repos/a"])).toBe("");
    expect(readRememberedRepoRoot(sessionKey, ["E:/repos/a"])).toBe("E:/repos/a");
  });

  it("sessionKey 为空时不写也不读", () => {
    rememberRepoRoot("", "E:/repos/a");
    expect(store.size).toBe(0);
    expect(readRememberedRepoRoot("", ["E:/repos/a"])).toBe("");
  });
});
