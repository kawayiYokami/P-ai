import { describe, it, expect } from "vitest";
import {
  listSessionFilePaths,
  sessionActiveFilePath,
  removeFilePathFromSessionState,
} from "./file-reader-session";

const A = "E:/repo/src/a.ts";
const B = "E:/repo/src/b.ts";
const C = "E:/repo/src/c.ts";

describe("listSessionFilePaths", () => {
  it("归一化反斜杠与扩展路径前缀，保持标签顺序", () => {
    expect(listSessionFilePaths({ tabs: ["E:\\repo\\src\\a.ts", "\\\\?\\E:\\repo\\src\\b.ts"] }))
      .toEqual([A, B]);
  });

  it("去重，并剔除 diff 伪路径与空项", () => {
    expect(listSessionFilePaths({ tabs: [A, A, "git-diff:E:/repo:worktree:src/a.ts", "", "  "] })).toEqual([A]);
  });

  it("没有 tabs 时为空数组", () => {
    expect(listSessionFilePaths({})).toEqual([]);
    expect(listSessionFilePaths(null)).toEqual([]);
  });
});

describe("sessionActiveFilePath", () => {
  it("当前文件在列表内时原样返回", () => {
    expect(sessionActiveFilePath({ tabs: [A, B], activePath: B })).toBe(B);
  });

  it("当前文件是伪路径或已不在列表内时，与面板恢复口径一致落到第一个文件", () => {
    expect(sessionActiveFilePath({ tabs: [A, B], activePath: "git-diff:E:/repo:staged:src/a.ts" })).toBe(A);
    expect(sessionActiveFilePath({ tabs: [A, B], activePath: "E:/repo/src/gone.ts" })).toBe(A);
    expect(sessionActiveFilePath({ tabs: [A, B] })).toBe(A);
  });

  it("没有文件时为空", () => {
    expect(sessionActiveFilePath({ tabs: [], activePath: A })).toBe("");
  });
});

describe("removeFilePathFromSessionState", () => {
  it("移除非当前文件时保留当前文件", () => {
    const next = removeFilePathFromSessionState({ tabs: [A, B, C], activePath: C }, B);
    expect(next.tabs).toEqual([A, C]);
    expect(next.activePath).toBe(C);
  });

  it("移除当前文件时接替左邻，与面板关闭标签一致", () => {
    expect(removeFilePathFromSessionState({ tabs: [A, B, C], activePath: B }, B).activePath).toBe(A);
    expect(removeFilePathFromSessionState({ tabs: [A, B, C], activePath: C }, C).activePath).toBe(B);
  });

  it("移除第一个且是当前文件时接替右邻", () => {
    expect(removeFilePathFromSessionState({ tabs: [A, B, C], activePath: A }, A).activePath).toBe(B);
  });

  it("移除最后一个文件后当前文件为空", () => {
    const next = removeFilePathFromSessionState({ tabs: [A], activePath: A }, A);
    expect(next.tabs).toEqual([]);
    expect(next.activePath).toBe("");
  });

  it("目标不在列表内时只做一次归一化整理", () => {
    const next = removeFilePathFromSessionState({ tabs: ["E:\\repo\\src\\a.ts", A], activePath: A }, "E:/repo/src/gone.ts");
    expect(next.tabs).toEqual([A]);
    expect(next.activePath).toBe(A);
  });

  it("保留目录根、侧栏模式等其它字段", () => {
    const next = removeFilePathFromSessionState(
      { tabs: [A, B], activePath: A, directoryRootPath: "E:/repo", directoryTreeWidth: 280, asideMode: "git" },
      B,
    );
    expect(next.directoryRootPath).toBe("E:/repo");
    expect(next.directoryTreeWidth).toBe(280);
    expect(next.asideMode).toBe("git");
  });

  it("归一化路径后再比对，反斜杠写法也能移除", () => {
    const next = removeFilePathFromSessionState({ tabs: ["E:\\repo\\src\\a.ts", B], activePath: A }, "E:\\repo\\src\\a.ts");
    expect(next.tabs).toEqual([B]);
    expect(next.activePath).toBe(B);
  });
});
