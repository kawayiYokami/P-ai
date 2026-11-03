import { describe, expect, it } from "vitest";
import { classifyQuickModelError } from "./quick-model-error-hint";

describe("classifyQuickModelError", () => {
  it("recognizes a server error and keeps the status code", () => {
    const raw = `GenAI 流式收集失败：WebStream { model_iden: ModelIden { adapter_kind: OpenAIResp, model_name: ModelName(Shared("gpt-6-astra")) }, cause: "HTTP error. Status: 502 Bad Gateway" }`;
    const hint = classifyQuickModelError(raw);
    expect(hint.kind).toBe("server");
    expect(hint.status).toBe(502);
    expect(hint.detail).not.toContain("\n");
  });

  it("treats 401 / 403 as auth problems", () => {
    expect(classifyQuickModelError("HTTP error. Status: 401 Unauthorized").kind).toBe("auth");
    expect(classifyQuickModelError("HTTP error. Status: 403 Forbidden").kind).toBe("auth");
    expect(classifyQuickModelError("invalid api key").kind).toBe("auth");
  });

  it("treats 404 and 模型不存在 as notFound", () => {
    expect(classifyQuickModelError("HTTP error. Status: 404 Not Found").kind).toBe("notFound");
    expect(classifyQuickModelError("模型不存在").kind).toBe("notFound");
  });

  it("recognizes timeouts in both languages", () => {
    expect(classifyQuickModelError("request timed out after 60s").kind).toBe("timeout");
    expect(classifyQuickModelError("请求超时").kind).toBe("timeout");
  });

  it("falls back to unknown and truncates the detail to 80 chars", () => {
    const raw = "x".repeat(200);
    const hint = classifyQuickModelError(raw);
    expect(hint.kind).toBe("unknown");
    expect(hint.status).toBeNull();
    expect(hint.detail).toBe(`${"x".repeat(80)}…`);
    expect(hint.detail.length).toBe(81);
  });

  it("handles empty input without throwing", () => {
    expect(classifyQuickModelError("")).toEqual({ kind: "unknown", status: null, detail: "" });
    expect(classifyQuickModelError(null).kind).toBe("unknown");
  });
});
