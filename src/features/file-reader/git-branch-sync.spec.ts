import { describe, it, expect } from "vitest";
import { branchSyncStatus, branchSyncArrows, branchSyncColorClass, type BranchSyncLike } from "./git-branch-sync";

function mk(over: Partial<BranchSyncLike>): BranchSyncLike {
  return { isRemote: false, upstream: "", upstreamMissing: false, ahead: 0, behind: 0, ...over };
}

describe("branchSyncStatus", () => {
  it("上游被删时优先返回 missingUpstream", () => {
    expect(branchSyncStatus(mk({ upstream: "origin/x", upstreamMissing: true, ahead: 2 }), true)).toBe("missingUpstream");
  });

  it("diverged：既领先又落后", () => {
    expect(branchSyncStatus(mk({ upstream: "origin/x", ahead: 1, behind: 1 }), true)).toBe("diverged");
  });

  it("ahead / behind 单向", () => {
    expect(branchSyncStatus(mk({ upstream: "origin/x", ahead: 3 }), true)).toBe("ahead");
    expect(branchSyncStatus(mk({ upstream: "origin/x", behind: 2 }), true)).toBe("behind");
  });

  it("有上游且数字都为 0：同步", () => {
    expect(branchSyncStatus(mk({ upstream: "origin/x" }), true)).toBe("upToDate");
  });

  it("无上游：有远程判未发布，无远程判纯本地", () => {
    const local = mk({});
    expect(branchSyncStatus(local, true)).toBe("unpublished");
    expect(branchSyncStatus(local, false)).toBe("local");
  });

  it("远程分支自身不参与", () => {
    expect(branchSyncStatus(mk({ isRemote: true, upstream: "x" }), true)).toBeNull();
  });
});

describe("branchSyncArrows", () => {
  it("ahead 只上、behind 只下、diverged 双向、其余无箭头", () => {
    expect(branchSyncArrows("ahead")).toEqual({ up: true, down: false });
    expect(branchSyncArrows("behind")).toEqual({ up: false, down: true });
    expect(branchSyncArrows("diverged")).toEqual({ up: true, down: true });
    expect(branchSyncArrows("upToDate")).toEqual({ up: false, down: false });
    expect(branchSyncArrows("missingUpstream")).toEqual({ up: false, down: false });
    expect(branchSyncArrows(null)).toEqual({ up: false, down: false });
  });
});

describe("branchSyncColorClass", () => {
  it("颜色映射", () => {
    expect(branchSyncColorClass("ahead")).toBe("text-success");
    expect(branchSyncColorClass("unpublished")).toBe("text-success");
    expect(branchSyncColorClass("behind")).toBe("text-error");
    expect(branchSyncColorClass("missingUpstream")).toBe("text-error");
    expect(branchSyncColorClass("diverged")).toBe("text-warning");
    expect(branchSyncColorClass("upToDate")).toBe("");
    expect(branchSyncColorClass("local")).toBe("");
  });
});
