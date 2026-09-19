import { describe, expect, it } from "vitest";
import { renameFirstMcpServerMember } from "./mcp-definition";

function parsed(json: string | null): unknown {
  expect(json).not.toBeNull();
  return JSON.parse(json as string);
}

describe("renameFirstMcpServerMember", () => {
  it("mcpServers 对象：改写第一个键，其余成员与字段保留", () => {
    const json = JSON.stringify({
      mcpServers: {
        deepwiki: { type: "streamable-http", url: "https://mcp.deepwiki.com/mcp" },
        other: { command: "npx", args: ["-y", "x"] },
      },
    });
    const result = parsed(renameFirstMcpServerMember(json, "维基百科"));
    expect(result).toEqual({
      mcpServers: {
        维基百科: { type: "streamable-http", url: "https://mcp.deepwiki.com/mcp" },
        other: { command: "npx", args: ["-y", "x"] },
      },
    });
  });

  it("mcpServers 对象：改写后第一个键仍排在最前", () => {
    const json = JSON.stringify({
      mcpServers: { a: { url: "https://a" }, b: { url: "https://b" } },
    });
    const result = parsed(renameFirstMcpServerMember(json, "z")) as {
      mcpServers: Record<string, unknown>;
    };
    expect(Object.keys(result.mcpServers)).toEqual(["z", "b"]);
  });

  it("mcpServers 数组：改写第一项的 name 字段", () => {
    const json = JSON.stringify({
      mcpServers: [{ name: "a", url: "https://a" }, { name: "b", url: "https://b" }],
    });
    const result = parsed(renameFirstMcpServerMember(json, "renamed")) as {
      mcpServers: Array<{ name: string }>;
    };
    expect(result.mcpServers.map((m) => m.name)).toEqual(["renamed", "b"]);
  });

  it("根级数组：改写第一项的 name 字段", () => {
    const json = JSON.stringify([{ name: "a", url: "https://a" }, { name: "b", url: "https://b" }]);
    const result = parsed(renameFirstMcpServerMember(json, "renamed")) as Array<{ name: string }>;
    expect(result.map((m) => m.name)).toEqual(["renamed", "b"]);
  });

  it("单 server 直接字段：改写根上的 name", () => {
    const json = JSON.stringify({ command: "npx", args: ["-y", "x"] });
    const result = parsed(renameFirstMcpServerMember(json, "本地服务"));
    expect(result).toEqual({ command: "npx", args: ["-y", "x"], name: "本地服务" });
  });

  it("平铺命名集合：改写第一个键", () => {
    const json = JSON.stringify({ a: { url: "https://a" }, b: { url: "https://b" } });
    const result = parsed(renameFirstMcpServerMember(json, "z")) as Record<string, unknown>;
    expect(Object.keys(result)).toEqual(["z", "b"]);
  });

  it("名字两端空白被裁掉", () => {
    const json = JSON.stringify({ mcpServers: { a: { url: "https://a" } } });
    const result = parsed(renameFirstMcpServerMember(json, "  spaced  ")) as {
      mcpServers: Record<string, unknown>;
    };
    expect(Object.keys(result.mcpServers)).toEqual(["spaced"]);
  });

  it("空名字不改动，返回 null", () => {
    const json = JSON.stringify({ mcpServers: { a: { url: "https://a" } } });
    expect(renameFirstMcpServerMember(json, "   ")).toBeNull();
  });

  it("无效 JSON 返回 null", () => {
    expect(renameFirstMcpServerMember("{ not json", "x")).toBeNull();
  });

  it("没有可命名成员的裸对象返回 null", () => {
    expect(renameFirstMcpServerMember(JSON.stringify({}), "x")).toBeNull();
  });
});
