import { describe, expect, it } from "vitest";
import {
  buildSandboxSrcdoc,
  isInteractiveHtmlLang,
  isPlainHtmlLang,
  normalizeSandboxUrl,
} from "./sandbox-html-builder";

describe("isInteractiveHtmlLang", () => {
  it("识别 html:interactive / html:preview / html:widget", () => {
    expect(isInteractiveHtmlLang("html:interactive")).toBe(true);
    expect(isInteractiveHtmlLang("HTML:Preview")).toBe(true);
    expect(isInteractiveHtmlLang("html:widget")).toBe(true);
  });

  it("不识别普通 html 或其他语言", () => {
    expect(isInteractiveHtmlLang("html")).toBe(false);
    expect(isInteractiveHtmlLang("svg")).toBe(false);
    expect(isInteractiveHtmlLang("")).toBe(false);
    expect(isInteractiveHtmlLang("htmlx:interactive")).toBe(false);
    expect(isInteractiveHtmlLang("html:other")).toBe(false);
  });
});

describe("isPlainHtmlLang", () => {
  it("识别 html / htm", () => {
    expect(isPlainHtmlLang("html")).toBe(true);
    expect(isPlainHtmlLang("HTM")).toBe(true);
  });

  it("不识别 html:interactive", () => {
    expect(isPlainHtmlLang("html:interactive")).toBe(false);
  });
});

describe("buildSandboxSrcdoc", () => {
  const tokens = { "--color-base-100": "#ffffff", "--color-primary": "#3366ff" };

  it("为裸片段补齐文档骨架", () => {
    const out = buildSandboxSrcdoc("<div>hi</div>", tokens, false);
    expect(out).toContain("<!DOCTYPE html>");
    expect(out).toContain("<div>hi</div>");
  });

  it("注入 CSP meta", () => {
    const out = buildSandboxSrcdoc(
      '<!DOCTYPE html><html><head></head><body><p>x</p></body></html>',
      tokens,
      false,
    );
    expect(out).toContain("Content-Security-Policy");
    expect(out).toContain("default-src 'none'");
  });

  it("替换已有 CSP meta", () => {
    const out = buildSandboxSrcdoc(
      '<html><head><meta http-equiv="Content-Security-Policy" content="default-src *"></head><body>x</body></html>',
      tokens,
      false,
    );
    expect(out).toContain("default-src 'none'");
    expect(out).not.toContain("default-src *");
  });

  it("注入主题变量与明暗模式", () => {
    const out = buildSandboxSrcdoc("<p>x</p>", tokens, true);
    expect(out).toContain("--color-base-100: #ffffff");
    expect(out).toContain("--color-primary: #3366ff");
    expect(out).toContain('color-scheme: dark');
  });

  it("注入 Bridge 脚本", () => {
    const out = buildSandboxSrcdoc("<p>x</p>", tokens, false);
    expect(out).toContain("pai-sandbox:resize");
    expect(out).toContain("data-pai-bridge");
  });

  it("空输入返回空串", () => {
    expect(buildSandboxSrcdoc("   ", tokens, false)).toBe("");
  });
});

describe("normalizeSandboxUrl", () => {
  it("放行本机回环地址，端口与路径不限", () => {
    expect(normalizeSandboxUrl("http://127.0.0.1:1420")).toBe("http://127.0.0.1:1420/");
    expect(normalizeSandboxUrl("http://localhost:3000/app?x=1#y"))
      .toBe("http://localhost:3000/app?x=1#y");
    expect(normalizeSandboxUrl("  http://127.0.0.1:5173/  ")).toBe("http://127.0.0.1:5173/");
  });

  it("没写协议时按 http 补全，别把 localhost: 当成协议名", () => {
    expect(normalizeSandboxUrl("127.0.0.1:1420")).toBe("http://127.0.0.1:1420/");
    expect(normalizeSandboxUrl("localhost:8080")).toBe("http://localhost:8080/");
  });

  it("拒绝外网地址", () => {
    expect(normalizeSandboxUrl("https://github.com")).toBeNull();
    expect(normalizeSandboxUrl("http://github.com")).toBeNull();
    expect(normalizeSandboxUrl("http://192.168.1.5:1420")).toBeNull();
    // 长得像 loopback 的伪装域名不放行
    expect(normalizeSandboxUrl("http://127.0.0.1.evil.com:1420")).toBeNull();
    expect(normalizeSandboxUrl("http://localhost.evil.com")).toBeNull();
  });

  it("拒绝非 http 协议与无效输入", () => {
    expect(normalizeSandboxUrl("file:///C:/Users/x/index.html")).toBeNull();
    expect(normalizeSandboxUrl("javascript:alert(1)")).toBeNull();
    expect(normalizeSandboxUrl("https://localhost:1420")).toBeNull();
    expect(normalizeSandboxUrl("")).toBeNull();
    expect(normalizeSandboxUrl("   ")).toBeNull();
    expect(normalizeSandboxUrl("http://")).toBeNull();
  });
});
