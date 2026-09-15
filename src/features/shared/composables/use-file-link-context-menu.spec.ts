import { describe, expect, it } from "vitest";
import { resolveFileLinkReference, resolveLinkMenuTarget } from "./use-file-link-context-menu";

function anchorWith(attributes: Record<string, string>, textContent = "") {
  return {
    getAttribute: (name: string) => attributes[name] ?? null,
    textContent,
  };
}

describe("Markdown 文件链接命中判定", () => {
  it("相对路径按工作区根补全，并带出前后行列", () => {
    const resolved = resolveFileLinkReference(
      anchorWith({ "data-href": "src/app.ts:57:3" }, "app.ts:57"),
      "E:/repo/",
    );
    expect(resolved).toEqual({
      path: "E:/repo/src/app.ts",
      line: 57,
      column: 3,
      label: "app.ts:57",
      raw: "src/app.ts:57:3",
    });
  });

  it("保留链接原文：复制动作原样输出，不做补全", () => {
    const resolved = resolveFileLinkReference(
      anchorWith({ "data-href": "  src/app.ts:57  " }, "app.ts:57"),
      "E:/repo",
    );
    expect(resolved?.raw).toBe("src/app.ts:57");
  });

  it("绝对路径不拼接工作区根", () => {
    const resolved = resolveFileLinkReference(
      anchorWith({ "data-href": "E:/other/file.md" }),
      "E:/repo",
    );
    expect(resolved?.path).toBe("E:/other/file.md");
    expect(resolved?.line).toBeUndefined();
  });

  it("助理空间路径保持原样，不拼工作区根", () => {
    const resolved = resolveFileLinkReference(
      anchorWith({ "data-href": "{Assistant Space}/notes/a.md" }),
      "E:/repo",
    );
    expect(resolved?.path).toBe("{Assistant Space}/notes/a.md");
  });

  it("外链（只写 href）不进本菜单", () => {
    expect(resolveFileLinkReference(anchorWith({ href: "https://example.com" }), "E:/repo")).toBeNull();
  });

  it("没有 data-href、站内锚点或空锚点都不命中", () => {
    expect(resolveFileLinkReference(anchorWith({}), "E:/repo")).toBeNull();
    expect(resolveFileLinkReference(anchorWith({ "data-href": "#" }), "E:/repo")).toBeNull();
    expect(resolveFileLinkReference(null, "E:/repo")).toBeNull();
  });
});

describe("Markdown 链接菜单目标判定", () => {
  it("本地文件链接判定为 file，沿用文件解析口径", () => {
    const target = resolveLinkMenuTarget(
      anchorWith({ "data-href": "src/app.ts:57" }, "app.ts:57"),
      "E:/repo",
    );
    expect(target).toEqual({
      kind: "file",
      path: "E:/repo/src/app.ts",
      line: 57,
      label: "app.ts:57",
      raw: "src/app.ts:57",
    });
  });

  it("外链判定为 url，标签取锚点文本", () => {
    const target = resolveLinkMenuTarget(
      anchorWith({ href: "https://example.com/docs" }, "官方文档"),
      "E:/repo",
    );
    expect(target).toEqual({ kind: "url", url: "https://example.com/docs", label: "官方文档" });
  });

  it("外链没有文本时用链接本身当标签", () => {
    const target = resolveLinkMenuTarget(anchorWith({ href: "http://example.com" }), "E:/repo");
    expect(target).toEqual({ kind: "url", url: "http://example.com", label: "http://example.com" });
  });

  it("站内锚点、非 http 协议与空锚点都不命中", () => {
    expect(resolveLinkMenuTarget(anchorWith({ href: "#" }), "E:/repo")).toBeNull();
    expect(resolveLinkMenuTarget(anchorWith({ href: "file:///E:/a.md" }), "E:/repo")).toBeNull();
    expect(resolveLinkMenuTarget(null, "E:/repo")).toBeNull();
  });

  it("站内锚点 #x 不命中：渲染器把锚点写在 data-href 上", () => {
    expect(resolveLinkMenuTarget(anchorWith({ "data-href": "#section" }, "标题"), "E:/repo")).toBeNull();
    expect(resolveLinkMenuTarget(anchorWith({ "data-href": "#" }, "标题"), "E:/repo")).toBeNull();
  });

  it("mailto 不命中：渲染器把 mailto 写在 data-href 上", () => {
    expect(resolveLinkMenuTarget(anchorWith({ "data-href": "mailto:a@b.c" }, "写信"), "E:/repo")).toBeNull();
  });

  it("盘符路径不会被当成协议", () => {
    const target = resolveLinkMenuTarget(anchorWith({ "data-href": "E:/repo/src/app.ts" }), "E:/other");
    expect(target).toEqual({
      kind: "file",
      path: "E:/repo/src/app.ts",
      label: "E:/repo/src/app.ts",
      raw: "E:/repo/src/app.ts",
    });
  });
});
