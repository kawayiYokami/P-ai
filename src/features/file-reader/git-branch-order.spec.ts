import { describe, it, expect } from "vitest";
import { buildBranchTree, sortBranchesForDisplay, type BranchLike } from "./git-branch-order";

function mk(name: string, committerDate: string, isCurrent = false): BranchLike {
  return { name, isCurrent, committerDate };
}

/** 把树拍平成便于断言的形状：文件夹写成 "folder:名"，分支写成 "branch:名" */
function flatten(nodes: ReturnType<typeof buildBranchTree<BranchLike>>, depth = 0): string[] {
  const out: string[] = [];
  for (const node of nodes) {
    const pad = "  ".repeat(depth);
    if (node.kind === "branch") {
      out.push(`${pad}branch:${node.branch.name}`);
    } else {      out.push(`${pad}folder:${node.name}`);
      out.push(...flatten(node.children, depth + 1));
    }
  }
  return out;
}

describe("sortBranchesForDisplay", () => {
  it("当前分支置顶，即使它时间最老", () => {
    const sorted = sortBranchesForDisplay([
      mk("newest", "2026-09-19T10:00:00+08:00"),
      mk("current", "2020-01-01T10:00:00+08:00", true),
      mk("older", "2025-01-01T10:00:00+08:00"),
    ]);
    expect(sorted.map((b) => b.name)).toEqual(["current", "newest", "older"]);
  });

  it("其余按末次提交时间倒序", () => {
    const sorted = sortBranchesForDisplay([
      mk("b", "2026-01-01T10:00:00+08:00"),
      mk("a", "2026-09-01T10:00:00+08:00"),
      mk("c", "2026-05-01T10:00:00+08:00"),
    ]);
    expect(sorted.map((b) => b.name)).toEqual(["a", "c", "b"]);
  });

  it("时间相同时按名称升序，保证顺序稳定", () => {
    const same = "2026-09-19T10:00:00+08:00";
    const sorted = sortBranchesForDisplay([mk("zeta", same), mk("alpha", same), mk("mid", same)]);
    expect(sorted.map((b) => b.name)).toEqual(["alpha", "mid", "zeta"]);
  });

  it("日期无效的分支排在最后", () => {
    const sorted = sortBranchesForDisplay([
      mk("broken", ""),
      mk("valid", "2026-01-01T10:00:00+08:00"),
    ]);
    expect(sorted.map((b) => b.name)).toEqual(["valid", "broken"]);
  });

  it("不改动入参数组", () => {
    const input = [mk("b", "2026-01-01T10:00:00+08:00"), mk("a", "2026-09-01T10:00:00+08:00")];
    sortBranchesForDisplay(input);
    expect(input.map((b) => b.name)).toEqual(["b", "a"]);
  });
});

describe("buildBranchTree", () => {
  it("无斜杠的分支平铺在根级", () => {
    const tree = buildBranchTree(
      sortBranchesForDisplay([mk("main", "2026-09-19T10:00:00+08:00"), mk("dev", "2026-09-01T10:00:00+08:00")]),
    );
    expect(flatten(tree)).toEqual(["branch:main", "branch:dev"]);
  });

  it("按 / 折叠成文件夹，组内继承时间顺序", () => {
    const tree = buildBranchTree(
      sortBranchesForDisplay([
        mk("feature/login", "2026-09-19T10:00:00+08:00"),
        mk("feature/payment", "2026-09-10T10:00:00+08:00"),
        mk("fix/crash", "2026-09-01T10:00:00+08:00"),
      ]),
    );
    expect(flatten(tree)).toEqual([
      "folder:feature",
      "  branch:feature/login",
      "  branch:feature/payment",
      "folder:fix",
      "  branch:fix/crash",
    ]);
  });

  it("组与组之间按各自最新成员的时间排", () => {
    const tree = buildBranchTree(
      sortBranchesForDisplay([
        mk("fix/hot", "2026-09-19T10:00:00+08:00"),
        mk("feature/cold", "2026-08-01T10:00:00+08:00"),
      ]),
    );
    expect(flatten(tree)[0]).toBe("folder:fix");
  });

  it("当前分支不折进文件夹，取全名当顶层叶子", () => {
    const tree = buildBranchTree(
      sortBranchesForDisplay([
        mk("feature/login", "2026-09-19T10:00:00+08:00", true),
        mk("feature/payment", "2026-09-10T10:00:00+08:00"),
      ]),
    );
    expect(flatten(tree)).toEqual([
      "branch:feature/login",
      "folder:feature",
      "  branch:feature/payment",
    ]);
  });

  it("单链目录合并成一行", () => {
    const tree = buildBranchTree(
      sortBranchesForDisplay([mk("backup/before/20260416", "2026-04-16T10:00:00+08:00")]),
    );
    expect(flatten(tree)).toEqual(["folder:backup/before", "  branch:backup/before/20260416"]);
  });

  it("多子目录不合并", () => {
    const tree = buildBranchTree(
      sortBranchesForDisplay([
        mk("a/b/one", "2026-09-19T10:00:00+08:00"),
        mk("a/c/two", "2026-09-18T10:00:00+08:00"),
      ]),
    );
    expect(flatten(tree)).toEqual([
      "folder:a",
      "  folder:b",
      "    branch:a/b/one",
      "  folder:c",
      "    branch:a/c/two",
    ]);
  });

  it("文件夹与同级叶子按时间交错，不把叶子全挤到后面", () => {
    const tree = buildBranchTree(
      sortBranchesForDisplay([
        mk("feature/a", "2026-09-19T10:00:00+08:00"),
        mk("main", "2026-09-18T10:00:00+08:00"),
        mk("fix/b", "2026-09-17T10:00:00+08:00"),
      ]),
    );
    expect(flatten(tree)).toEqual([
      "folder:feature",
      "  branch:feature/a",
      "branch:main",
      "folder:fix",
      "  branch:fix/b",
    ]);
  });

  it("空列表返回空树", () => {
    expect(buildBranchTree([])).toEqual([]);
  });

  it("文件夹带完整 path，供调用方生成唯一 key", () => {
    const tree = buildBranchTree(
      sortBranchesForDisplay([
        mk("feature/auth/login", "2026-09-19T10:00:00+08:00"),
        mk("feature/auth/signup", "2026-09-18T10:00:00+08:00"),
      ]),
    );
    const folder = tree[0];
    expect(folder.kind).toBe("folder");
    if (folder.kind === "folder") {
      expect(folder.name).toBe("feature/auth");
      expect(folder.path).toBe("feature/auth");
    }
  });

  it("折进文件夹的分支只显示末段，根级分支显示全名", () => {
    const tree = buildBranchTree(
      sortBranchesForDisplay([
        mk("feature/auth/login", "2026-09-19T10:00:00+08:00"),
        mk("plain", "2026-09-18T10:00:00+08:00"),
      ]),
    );

    const labels: string[] = [];
    const walk = (nodes: ReturnType<typeof buildBranchTree<BranchLike>>) => {
      for (const node of nodes) {
        if (node.kind === "branch") labels.push(node.label);
        else walk(node.children);
      }
    };
    walk(tree);
    expect(labels).toEqual(["login", "plain"]);
  });

  it("单链合并后组的末段仍正确", () => {
    const tree = buildBranchTree(
      sortBranchesForDisplay([mk("backup/before/20260416", "2026-04-16T10:00:00+08:00")]),
    );
    const folder = tree[0];
    if (folder.kind === "folder") {
      const leaf = folder.children[0];
      expect(leaf.kind).toBe("branch");
      if (leaf.kind === "branch") expect(leaf.label).toBe("20260416");
    }
  });
});
