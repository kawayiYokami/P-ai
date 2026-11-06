import { describe, expect, it } from "vitest";
import {
  MCP_INSTRUCTIONS_SUMMARY_MAX_CHARS,
  firstSentenceOfInstructions,
  summarizeMcpInstructions,
} from "./mcp-instructions";

describe("firstSentenceOfInstructions", () => {
  it("取中文首句，丢掉句号本身", () => {
    expect(firstSentenceOfInstructions("可以查询代码仓库文档。也支持搜索。")).toBe(
      "可以查询代码仓库文档",
    );
  });

  it("英文句点必须后接空白才算句界", () => {
    expect(firstSentenceOfInstructions("Search repos. Then read files.")).toBe("Search repos");
    // 版本号与域名里的点不能把句子切断
    expect(firstSentenceOfInstructions("Works with v1.2 API and example.com docs")).toBe(
      "Works with v1.2 API and example.com docs",
    );
  });

  it("换行也作为句界", () => {
    expect(firstSentenceOfInstructions("第一行介绍\n第二行细节")).toBe("第一行介绍");
  });
});

describe("summarizeMcpInstructions", () => {
  it("空说明返回空串，调用方据此回落地址", () => {
    expect(summarizeMcpInstructions("")).toBe("");
    expect(summarizeMcpInstructions("   \n  ")).toBe("");
  });

  it("短说明原样返回，不加省略号", () => {
    expect(summarizeMcpInstructions("可以查询代码仓库文档。")).toBe("可以查询代码仓库文档");
  });

  it("超长首句裁到上限并加省略号", () => {
    const long = `${"很长的服务说明".repeat(30)}。`;
    const summary = summarizeMcpInstructions(long);
    expect(summary.length).toBe(MCP_INSTRUCTIONS_SUMMARY_MAX_CHARS + 3);
    expect(summary.endsWith("...")).toBe(true);
    expect(summary.startsWith("很长的服务说明")).toBe(true);
  });

  it("deepwiki 的真实 instructions：首句完整落在上限内", () => {
    const instructions = [
      "DeepWiki MCP provides AI-powered documentation for GitHub repositories.",
      "",
      "With a user OAuth token, all devin_* tools act in one organization.",
    ].join("\n");
    expect(summarizeMcpInstructions(instructions)).toBe(
      "DeepWiki MCP provides AI-powered documentation for GitHub repositories",
    );
  });

  it("context7 的真实 instructions：首句过长时截断而不是整段铺开", () => {
    const instructions =
      "Use this server to fetch current documentation whenever the user asks about a library, framework, SDK, API, CLI tool, or cloud service — even well-known ones like React, Next.js, Prisma, Express, Tailwind, Django, or Spring Boot. This includes API syntax, configuration, version migration.";
    const summary = summarizeMcpInstructions(instructions);
    expect(summary.length).toBe(MCP_INSTRUCTIONS_SUMMARY_MAX_CHARS + 3);
    expect(summary.endsWith("...")).toBe(true);
    expect(summary).not.toContain("This includes API syntax");
  });
});
