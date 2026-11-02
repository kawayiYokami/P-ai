import { describe, expect, it } from "vitest";
import {
  directoryPathChain,
  fileKindFromPath,
  isPreviewMediaKind,
  isTextFileKind,
  isUnsupportedFileExtension,
} from "./utils";

describe("fileKindFromPath", () => {
  it("treats TypeScript .ts as code, not unsupported media", () => {
    expect(fileKindFromPath("E:/repo/src/toolcall-preview.ts")).toBe("code");
    expect(isUnsupportedFileExtension("ts")).toBe(false);
  });

  it("still rejects explicit mpeg-ts containers", () => {
    expect(fileKindFromPath("E:/video/sample.mts")).toBe("unsupported");
    expect(fileKindFromPath("E:/video/sample.m2ts")).toBe("unsupported");
  });

  it("单列 html kind，源码与沙箱渲染都按整段原文读", () => {
    expect(fileKindFromPath("E:/repo/docs/demo.html")).toBe("html");
    expect(fileKindFromPath("E:/repo/docs/demo.htm")).toBe("html");
    expect(fileKindFromPath("E:/repo/docs/demo.HTML")).toBe("html");
    expect(isTextFileKind("html")).toBe(true);
    // html 不是媒体 kind，不能走进 image/audio/video 预览分支
    expect(isPreviewMediaKind("html")).toBe(false);
  });
});

describe("directoryPathChain", () => {
  it("returns the directories from root to the target directory", () => {
    expect(directoryPathChain("E:/repo", "E:/repo/src/features")).toEqual([
      "E:/repo",
      "E:/repo/src",
      "E:/repo/src/features",
    ]);
  });

  it("handles Windows drive roots and case-insensitive matching", () => {
    expect(directoryPathChain("e:/", "E:/Repo/src")).toEqual([
      "e:/",
      "e:/Repo",
      "e:/Repo/src",
    ]);
  });

  it("returns an empty chain for paths outside the root", () => {
    expect(directoryPathChain("E:/repo", "E:/other/src")).toEqual([]);
  });
});
