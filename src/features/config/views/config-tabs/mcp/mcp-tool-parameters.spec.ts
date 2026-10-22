import { describe, expect, it } from "vitest";
import {
  parseToolParameters,
  parseToolExamples,
  formatSchemaConstraints,
  toolSchemaTypeText,
} from "./mcp-tool-parameters";

describe("mcp-tool-parameters", () => {
  it("解析空的或非对象参数时返回空数组", () => {
    expect(parseToolParameters(null)).toEqual([]);
    expect(parseToolParameters(undefined)).toEqual([]);
    expect(parseToolParameters("not object")).toEqual([]);
    expect(parseToolParameters({})).toEqual([]);
    expect(parseToolParameters({ properties: null })).toEqual([]);
  });

  it("正确解析常见 MCP 工具 schema 中的字段（包含必填、类型、描述、枚举、默认值与约束）", () => {
    const schema = {
      type: "object",
      properties: {
        world: {
          type: "string",
          description: "游戏世界标识，如 gi (原神) / hsr (星穹铁道)",
          enum: ["gi", "hsr", "bh3"],
        },
        keyword: {
          type: "string",
          description: "查询关键词",
        },
        maxResults: {
          type: "integer",
          description: "最大返回条数",
          minimum: 1,
          maximum: 50,
          default: 20,
        },
        tags: {
          type: "array",
          items: {
            type: "string",
          },
          description: "标签过滤列表",
        },
      },
      required: ["world", "keyword"],
    };

    const result = parseToolParameters(schema);
    expect(result).toHaveLength(4);

    const world = result.find((p) => p.name === "world");
    expect(world).toBeDefined();
    expect(world?.required).toBe(true);
    expect(world?.type).toBe("string");
    expect(world?.description).toBe("游戏世界标识，如 gi (原神) / hsr (星穹铁道)");
    expect(world?.enumValues).toEqual(["gi", "hsr", "bh3"]);
    expect(world?.summaryText).toBe("*world: string [gi, hsr, bh3]");
    expect(world?.tooltipText).toContain("world: string (必填)");
    expect(world?.tooltipText).toContain("可选值: [gi, hsr, bh3]");

    const keyword = result.find((p) => p.name === "keyword");
    expect(keyword?.required).toBe(true);
    expect(keyword?.description).toBe("查询关键词");

    const maxResults = result.find((p) => p.name === "maxResults");
    expect(maxResults?.required).toBe(false);
    expect(maxResults?.type).toBe("integer");
    expect(maxResults?.defaultValue).toBe("20");
    expect(maxResults?.constraints).toBe(">= 1, <= 50");
    expect(maxResults?.summaryText).toBe("maxResults: integer >= 1 <= 50");

    const tags = result.find((p) => p.name === "tags");
    expect(tags?.required).toBe(false);
    expect(tags?.type).toBe("string[]");
    expect(tags?.description).toBe("标签过滤列表");
  });

  it("支持嵌套对象与数组内对象的层级解析", () => {
    const schema = {
      type: "object",
      properties: {
        filter: {
          type: "object",
          description: "筛选条件",
          properties: {
            author: {
              type: "string",
              description: "作者名",
            },
          },
          required: ["author"],
        },
      },
    };

    const result = parseToolParameters(schema);
    expect(result.map((r) => r.name)).toEqual(["filter", "filter.author"]);
    const author = result.find((r) => r.name === "filter.author");
    expect(author?.required).toBe(true);
    expect(author?.description).toBe("作者名");
  });

  it("正确提取参数示例", () => {
    const schema = {
      type: "object",
      properties: {
        payload: {
          type: "string",
          example: '{"test": 123}',
        },
      },
    };

    const examples = parseToolExamples(schema);
    expect(examples).toHaveLength(1);
    expect(examples[0]).toContain("payload 示例:");
    expect(examples[0]).toContain('{"test": 123}');
  });
});
